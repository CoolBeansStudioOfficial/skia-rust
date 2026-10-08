// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionDefinition.{h,cpp}: `Convert`, `Make` and the
// `Finalizer` that checks a body (loops, returns, local variable slots) and fuses declarations
// with their first assignment when optimizing.
//
//! [`FunctionDefinition`] construction: `FunctionDefinition::Convert` and `Make`.

use super::{
    BinaryExpression, Block, ExprId, ExpressionKind, FnId, FunctionDefinition, IrPool, Nop,
    ProgramElement, ProgramElementKind, Statement, StatementKind, StmtId, SymTabId, SymbolId,
    VarId, VariableStorage, ids::ElemId, swizzle_component,
};
use crate::analysis::{ProgramVisitor, walk_expression};
use crate::context::Context;
use crate::defines::VARIABLE_SLOT_LIMIT;
use crate::operator::OperatorKind;
use crate::position::Position;
use crate::program_settings::ProgramConfig;
use crate::transform::ProgramWriter;

/// `Compiler::RTADJUST_NAME`: the uniform that a vertex program's `sk_Position` is adjusted by.
// Port of: src/sksl/SkSLCompiler.h#L69 (chrome/m156)
const RTADJUST_NAME: &str = "sk_RTAdjust";

impl FunctionDefinition {
    /// `FunctionDefinition::Convert`: checks a function body and makes the definition. Returns
    /// `None` after reporting an error. `body` is `None` when the parser produced no body.
    // Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L209-L330 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        function: FnId,
        body: Option<StmtId>,
    ) -> Option<ElemId> {
        // We don't allow modules to define actual functions with intrinsic names. (Those should be
        // reserved for actual intrinsics.)
        if ctx.pool.function(function).is_intrinsic() {
            let msg = format!(
                "intrinsic function '{}' should not have a definition",
                ctx.pool.function(function).name
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        // A function body must always be a braced block. (The parser should enforce this already,
        // but we rely on it, so it's best to be certain.)
        let Some(body) = body.filter(|&b| {
            ctx.pool
                .statement(b)
                .as_block()
                .is_some_and(Block::is_scope)
        }) else {
            let msg = format!(
                "function body '{}' must be a braced block",
                ctx.pool.function(function).description(&ctx.pool)
            );
            ctx.errors.error(pos, &msg);
            return None;
        };

        // A function can't have more than one definition.
        if ctx.pool.function(function).definition.is_some() {
            let msg = format!(
                "function '{}' was already defined",
                ctx.pool.function(function).description(&ctx.pool)
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        // Run the function finalizer. This checks for illegal constructs and missing return
        // statements, and also performs some simple code cleanup.
        let mut finalizer = Finalizer::new(ctx, function, pos);
        finalizer.visit_statement_ptr(ctx, body);

        let config = *ctx.config();
        if ctx.pool.function(function).is_main && ProgramConfig::is_vertex(config.kind) {
            append_rtadjust_fixup_to_vertex_main(ctx, body);
        }

        if can_exit_without_returning_value(&ctx.pool, function, body) {
            let msg = format!(
                "function '{}' can exit without returning a value",
                ctx.pool.function(function).name
            );
            let body_pos = ctx.pool.statement(body).position;
            ctx.errors.error(body_pos, &msg);
        }

        Some(Self::make(ctx, pos, function, body))
    }

    /// `FunctionDefinition::Make`: allocates the definition element and records it as the
    /// function's definition.
    ///
    /// # Panics
    ///
    /// If `function` is an intrinsic, has a definition already, or `body` is not a braced block.
    // Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L332-L343 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, function: FnId, body: StmtId) -> ElemId {
        debug_assert!(!ctx.pool.function(function).is_intrinsic());
        debug_assert!(
            ctx.pool
                .statement(body)
                .as_block()
                .is_some_and(Block::is_scope)
        );
        debug_assert!(ctx.pool.function(function).definition.is_none());
        let element = ctx.pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::Function(Self {
                declaration: function,
                body,
            }),
        ));
        ctx.pool.function_mut(function).set_definition(element);
        element
    }
}

/// `append_rtadjust_fixup_to_vertex_main`: when `sk_RTAdjust` is in scope, appends to the body of
/// a vertex `main` the statement that fixes up `sk_Position`.
// Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L20-L43 (chrome/m156)
fn append_rtadjust_fixup_to_vertex_main(ctx: &mut Context, body: StmtId) {
    let table = ctx
        .symbol_table
        .expect("FunctionDefinition::Convert: no current symbol table");
    // If this program uses RTAdjust...
    if let Some(rt_adjust) = ctx.pool.find_symbol(table, RTADJUST_NAME) {
        // ...append a line to the end of the function body which fixes up sk_Position.
        let fixup = make_fixup_stmt(ctx, table, rt_adjust);
        if let StatementKind::Block(block) = &mut ctx.pool.statement_mut(body).kind {
            block.children.push(fixup);
        }
    }
}

/// `AppendRTAdjustFixupHelper::makeFixupStmt`, with the `IRHelpers` it uses (`SkSLIRHelpers.h`):
/// `sk_Position = float4(sk_Position.xy * rtAdjust.xz + sk_Position.ww * rtAdjust.yw, 0,
/// sk_Position.w);`.
// Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L45-L81 (chrome/m156)
fn make_fixup_stmt(ctx: &mut Context, table: SymTabId, rt_adjust: SymbolId) -> StmtId {
    // The field of `sk_Position` and the variable it belongs to.
    let position_field = ctx
        .pool
        .find_symbol(table, "sk_Position")
        .expect("sk_Position is declared in every vertex program");
    let SymbolId::Field(field) = position_field else {
        unreachable!("sk_Position is an interface-block field");
    };
    let (owner, field_index) = {
        let f = ctx.pool.field_symbol(field);
        (f.owner, f.field_index)
    };
    let pos = |ctx: &mut Context| helpers::field(ctx, owner, field_index);
    let adjust = |ctx: &mut Context| {
        rt_adjust
            .instantiate(ctx, Position::default())
            .expect("sk_RTAdjust is a variable")
    };
    let xy = {
        let base = pos(ctx);
        helpers::swizzle(ctx, base, &[swizzle_component::X, swizzle_component::Y])
    };
    let adj_xz = {
        let base = adjust(ctx);
        helpers::swizzle(ctx, base, &[swizzle_component::X, swizzle_component::Z])
    };
    let ww = {
        let base = pos(ctx);
        helpers::swizzle(ctx, base, &[swizzle_component::W, swizzle_component::W])
    };
    let adj_yw = {
        let base = adjust(ctx);
        helpers::swizzle(ctx, base, &[swizzle_component::Y, swizzle_component::W])
    };
    let xy_term = helpers::mul(ctx, xy, adj_xz);
    let ww_term = helpers::mul(ctx, ww, adj_yw);
    let sum = helpers::add(ctx, xy_term, ww_term);
    let w = {
        let base = pos(ctx);
        helpers::swizzle(ctx, base, &[swizzle_component::W])
    };
    let zero = helpers::float(ctx, 0.0);
    let value = helpers::ctor_xyzw(ctx, sum, zero, w);
    let target = pos(ctx);
    helpers::assign(ctx, target, value)
}

/// The `IRHelpers` members that the `sk_Position` fixup uses. Each one is named after its Skia
/// member; positions are `Position()` unless Skia derives them from an operand.
mod helpers {
    use crate::analysis;
    use crate::context::Context;
    use crate::ir::{
        BinaryExpression, ComponentArray, ConstructorCompound, ExprId, ExpressionStatement,
        FieldAccess, FieldAccessOwnerKind, Literal, StmtId, Swizzle, TypeId, VarId,
        VariableRefKind, VariableReference,
    };
    use crate::operator::{Operator, OperatorKind};
    use crate::position::Position;

    /// `IRHelpers::Field(var, idx)`.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L50-L53 (chrome/m156)
    pub(super) fn field(ctx: &mut Context, var: VarId, idx: usize) -> ExprId {
        let base = VariableReference::make(
            &mut ctx.pool,
            Position::default(),
            var,
            VariableRefKind::Read,
        );
        FieldAccess::make(
            ctx,
            Position::default(),
            base,
            idx,
            FieldAccessOwnerKind::AnonymousInterfaceBlock,
        )
    }

    /// `IRHelpers::Swizzle(base, c)`: positioned at the base.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L55-L59 (chrome/m156)
    pub(super) fn swizzle(ctx: &mut Context, base: ExprId, components: &[i8]) -> ExprId {
        let pos = ctx.pool.expression(base).position;
        Swizzle::make(ctx, pos, base, ComponentArray::from_slice(components))
    }

    /// `IRHelpers::Binary(l, op, r)`: positioned from the left operand through the right one.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L61-L66 (chrome/m156)
    pub(super) fn binary(ctx: &mut Context, l: ExprId, op: Operator, r: ExprId) -> ExprId {
        let pos = ctx
            .pool
            .expression(l)
            .position
            .range_through(ctx.pool.expression(r).position);
        BinaryExpression::make(ctx, pos, l, op, r)
    }

    /// `IRHelpers::Mul(l, r)`.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L68-L71 (chrome/m156)
    pub(super) fn mul(ctx: &mut Context, l: ExprId, r: ExprId) -> ExprId {
        binary(ctx, l, Operator::from(OperatorKind::Star), r)
    }

    /// `IRHelpers::Add(l, r)`.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L73-L76 (chrome/m156)
    pub(super) fn add(ctx: &mut Context, l: ExprId, r: ExprId) -> ExprId {
        binary(ctx, l, Operator::from(OperatorKind::Plus), r)
    }

    /// `IRHelpers::Float(value)`.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L78-L81 (chrome/m156)
    pub(super) fn float(ctx: &mut Context, value: f32) -> ExprId {
        Literal::make_float(&mut ctx.pool, Position::default(), value, TypeId::FLOAT)
    }

    /// `IRHelpers::CtorXYZW(xy, z, w)`: a `float4` of the three arguments.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L88-L97 (chrome/m156)
    pub(super) fn ctor_xyzw(ctx: &mut Context, xy: ExprId, z: ExprId, w: ExprId) -> ExprId {
        ConstructorCompound::make(ctx, Position::default(), TypeId::FLOAT4, vec![xy, z, w])
    }

    /// `IRHelpers::Assign(l, r)`: the statement `l = r;`.
    // Port of: src/sksl/ir/SkSLIRHelpers.h#L99-L104 (chrome/m156)
    pub(super) fn assign(ctx: &mut Context, l: ExprId, r: ExprId) -> StmtId {
        let ok = analysis::update_variable_ref_kind(&mut ctx.pool, l, VariableRefKind::Write, None);
        debug_assert!(ok, "the fixup target is assignable");
        let expr = binary(ctx, l, Operator::from(OperatorKind::Eq), r);
        ExpressionStatement::make(ctx, expr)
    }
}

/// `Finalizer`: a program writer that checks a function body and fuses declarations with their
/// first assignment (when optimizing). Each variable's slots are counted against the stack limit.
// Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L28-L200 (chrome/m156)
struct Finalizer {
    /// `fFunction`: the function being finalized.
    function: FnId,
    /// Whether `function` is `main`.
    is_main: bool,
    /// Whether `function` returns a value (`!returnType().isVoid()`).
    returns_value: bool,
    /// `fBreakableLevel`: how deeply nested we are in breakable constructs (`for`, `do`, `switch`).
    breakable_level: i32,
    /// `fSlotsUsed`: slots consumed by all variables declared in the function.
    slots_used: usize,
    /// `fContinuableLevel`: a stack of how deeply nested we are in continuable constructs. The
    /// front of Skia's `forward_list` is the last element here; a `switch` pushes a new level so
    /// that `continue` is rejected inside it.
    continuable_level: Vec<i32>,
    /// `fUninitializedVarDecl`: a declaration with no initializer that may take the next
    /// assignment.
    uninitialized_var_decl: Option<StmtId>,
}

impl Finalizer {
    /// The constructor: function parameters count as local variables.
    fn new(ctx: &mut Context, function: FnId, pos: Position) -> Self {
        let decl = ctx.pool.function(function);
        let mut finalizer = Self {
            function,
            is_main: decl.is_main,
            returns_value: !ctx.pool.ty(decl.return_type).is_void(),
            breakable_level: 0,
            slots_used: 0,
            continuable_level: vec![0],
            uninitialized_var_decl: None,
        };
        for &var in &ctx.pool.function(function).parameters.clone() {
            finalizer.add_local_variable(ctx, var, pos);
        }
        finalizer
    }

    /// `addLocalVariable`: unsized arrays are only allowed as parameters, and the slots a
    /// variable takes count against the stack limit.
    fn add_local_variable(&mut self, ctx: &mut Context, var: VarId, pos: Position) {
        let (is_unsized, is_parameter, name, slots) = {
            let v = ctx.pool.variable(var);
            let t = ctx.pool.ty(v.ty);
            let is_unsized = t.is_or_contains_unsized_array();
            (
                is_unsized,
                v.storage == VariableStorage::Parameter,
                v.name.clone(),
                // The slot count of an unsized array is undefined, and not asked for.
                if is_unsized { 0 } else { t.slot_count() },
            )
        };
        if is_unsized {
            if !is_parameter {
                ctx.errors
                    .error(pos, "unsized arrays are not permitted here");
            }
            // Number of slots does not apply to unsized arrays since they are dynamically sized.
            return;
        }
        // We count the number of slots used, but don't consider the precision of the base type.
        // In practice, this reflects what GPUs actually do pretty well. (i.e., RelaxedPrecision
        // math doesn't mean your variable takes less space.) We also don't attempt to reclaim
        // slots at the end of a Block.
        let limit = usize::try_from(VARIABLE_SLOT_LIMIT).unwrap_or(usize::MAX);
        let prev_slots_used = self.slots_used;
        // `SkSafeMath::Add` saturates on overflow.
        self.slots_used = self.slots_used.saturating_add(slots);
        // To avoid overzealous error reporting, only trigger the error at the first place where
        // the stack limit is exceeded.
        if prev_slots_used < limit && self.slots_used >= limit {
            ctx.errors.error(
                pos,
                &format!("variable '{name}' exceeds the stack size limit"),
            );
        }
    }

    /// `fuseVariableDeclarationsWithInitialization`: when `stmt` is `x = expr;` directly after
    /// `T x;`, and `expr` does not read `x`, the assignment becomes the declaration's initializer.
    fn fuse_variable_declarations_with_initialization(&mut self, ctx: &mut Context, stmt: StmtId) {
        let expression = match &ctx.pool.statement(stmt).kind {
            // Blocks and no-ops are inert; it is safe to fuse a variable declaration with its
            // initialization across a nop or an open-brace, so we don't clear the pending
            // declaration here.
            StatementKind::Nop(_) | StatementKind::Block(_) => return,
            // Look for variable declarations without an initializer.
            StatementKind::VarDeclaration(decl) if decl.value.is_none() => {
                self.uninitialized_var_decl = Some(stmt);
                return;
            }
            StatementKind::Expression(e) => e.expression,
            // We found an intervening statement; it's not safe to fuse a declaration with an
            // initializer if we encounter any other code.
            _ => {
                self.uninitialized_var_decl = None;
                return;
            }
        };
        // We found an expression-statement. If there was a variable declaration immediately above
        // it, it might be possible to fuse them.
        let Some(decl_stmt) = self.uninitialized_var_decl.take() else {
            return;
        };
        // This statement must be a binary-expression performing simple `var = expr` assignment,
        // directly into the variable (not a field/swizzle), and the variable must be the one that
        // was declared.
        let (left, operator, right) = match &ctx.pool.expression(expression).kind {
            ExpressionKind::Binary(BinaryExpression {
                left,
                operator,
                right,
            }) => (*left, *operator, *right),
            _ => return,
        };
        if operator.kind() != OperatorKind::Eq {
            return;
        }
        let var = match &ctx.pool.expression(left).kind {
            ExpressionKind::VariableReference(var_ref) => var_ref.variable,
            _ => return,
        };
        let decl_var = match &ctx.pool.statement(decl_stmt).kind {
            StatementKind::VarDeclaration(decl) => decl.var,
            _ => return,
        };
        if var != decl_var {
            return;
        }
        // The init-expression must not reference the variable. `int x; x = x = 0;` is legal SkSL,
        // but `int x = x = 0;` is not.
        if contains_variable(&ctx.pool, right, var) {
            return;
        }
        // We found a match! Move the init-expression directly onto the vardecl, and turn the
        // assignment into a no-op.
        if let StatementKind::VarDeclaration(decl) = &mut ctx.pool.statement_mut(decl_stmt).kind {
            decl.value = Some(right);
        }
        // Turn the expression-statement into a no-op.
        ctx.pool.replace_statement(
            stmt,
            Statement::new(Position::default(), StatementKind::Nop(Nop)),
        );
    }
}

impl ProgramWriter for Finalizer {
    /// We don't need to scan expressions.
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        false
    }

    /// Fuses declarations when optimizing, checks each statement, and recurses.
    // Port of: src/sksl/ir/SkSLFunctionDefinition.cpp#L86-L200 (chrome/m156), visitStatementPtr
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        // When the optimizer is on, we look for variable declarations that are immediately
        // followed by an initialization expression, and fuse them into one statement.
        // (e.g.: `int i; i = 1;` can become `int i = 1;`)
        if ctx.config().settings.optimize {
            self.fuse_variable_declarations_with_initialization(ctx, stmt);
        }

        // Perform error checking.
        let pos = ctx.pool.statement(stmt).position;
        let check = match &ctx.pool.statement(stmt).kind {
            StatementKind::VarDeclaration(decl) => Check::VarDeclaration(decl.var),
            StatementKind::Return(ret) => Check::Return(ret.expression),
            StatementKind::Do(_) | StatementKind::For(_) => Check::Loop,
            StatementKind::Switch(_) => Check::Switch,
            StatementKind::Break(_) => Check::Break,
            StatementKind::Continue(_) => Check::Continue,
            _ => Check::Other,
        };
        match check {
            Check::VarDeclaration(var) => self.add_local_variable(ctx, var, pos),
            Check::Return(expression) => {
                // Early returns from a vertex main() function will bypass sk_Position
                // normalization, so SkASSERT that we aren't doing that. If this becomes an issue,
                // we can add normalization before each return statement.
                if ProgramConfig::is_vertex(ctx.config().kind) && self.is_main {
                    ctx.errors
                        .error(pos, "early returns from vertex programs are not supported");
                }
                // Verify that the return statement matches the function's return type.
                if let Some(expression) = expression {
                    if self.returns_value {
                        // Coerce return expression to the function's return type.
                        let return_type = ctx.pool.function(self.function).return_type;
                        let coerced = return_type.coerce_expression(ctx, expression);
                        if let StatementKind::Return(ret) = &mut ctx.pool.statement_mut(stmt).kind {
                            ret.expression = coerced;
                        }
                    } else {
                        // Returning something from a function with a void return type.
                        let expr_pos = ctx.pool.expression(expression).position;
                        ctx.errors
                            .error(expr_pos, "may not return a value from a void function");
                        if let StatementKind::Return(ret) = &mut ctx.pool.statement_mut(stmt).kind {
                            ret.expression = None;
                        }
                    }
                } else if self.returns_value {
                    // Returning nothing from a function with a non-void return type.
                    let msg = format!(
                        "expected function to return '{}'",
                        ctx.pool
                            .ty(ctx.pool.function(self.function).return_type)
                            .display_name()
                    );
                    ctx.errors.error(pos, &msg);
                }
            }
            Check::Loop => {
                self.breakable_level += 1;
                if let Some(level) = self.continuable_level.last_mut() {
                    *level += 1;
                }
                let result = self.visit_statement(ctx, stmt);
                if let Some(level) = self.continuable_level.last_mut() {
                    *level -= 1;
                }
                self.breakable_level -= 1;
                return result;
            }
            Check::Switch => {
                self.breakable_level += 1;
                self.continuable_level.push(0);
                let result = self.visit_statement(ctx, stmt);
                self.continuable_level.pop();
                self.breakable_level -= 1;
                return result;
            }
            Check::Break => {
                if self.breakable_level == 0 {
                    ctx.errors
                        .error(pos, "break statement must be inside a loop or switch");
                }
            }
            Check::Continue => {
                if self.continuable_level.last().copied().unwrap_or(0) == 0 {
                    if self.continuable_level.iter().any(|&level| level > 0) {
                        ctx.errors
                            .error(pos, "continue statement cannot be used in a switch");
                    } else {
                        ctx.errors
                            .error(pos, "continue statement must be inside a loop");
                    }
                }
            }
            Check::Other => {}
        }
        self.visit_statement(ctx, stmt)
    }
}

/// What the `Finalizer` needs from a statement, copied out of the pool before it reports.
enum Check {
    VarDeclaration(VarId),
    Return(Option<ExprId>),
    Loop,
    Switch,
    Break,
    Continue,
    Other,
}

/// `Analysis::ContainsVariable`: whether `var` is read or written anywhere in `expr`.
// Port of: src/sksl/SkSLAnalysis.cpp#L429-L450 (chrome/m156)
fn contains_variable(pool: &IrPool, expr: ExprId, var: VarId) -> bool {
    struct ContainsVariableVisitor {
        variable: VarId,
    }
    impl ProgramVisitor for ContainsVariableVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            if let ExpressionKind::VariableReference(v) = &pool.expression(expr).kind
                && v.variable == self.variable
            {
                return true;
            }
            walk_expression(self, pool, expr)
        }
    }
    ContainsVariableVisitor { variable: var }.visit_expression(pool, expr)
}

/// The `ReturnsOnAllPathsVisitor` of `Analysis::CanExitWithoutReturningValue`: which kinds of
/// exit a statement is certain to reach.
#[derive(Default)]
struct ReturnsOnAllPaths {
    found_return: bool,
    found_break: bool,
    found_continue: bool,
}

impl ReturnsOnAllPaths {
    /// `visitStatement`: returns `true` to stop the scan.
    // Port of: src/sksl/SkSLCanExitWithoutReturningValue.cpp#L16-L110 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            // Returns, breaks, or continues will stop the scan, so only one of these should ever
            // be true.
            StatementKind::Return(_) => {
                self.found_return = true;
                true
            }
            StatementKind::Break(_) => {
                self.found_break = true;
                true
            }
            StatementKind::Continue(_) => {
                self.found_continue = true;
                true
            }
            StatementKind::If(i) => {
                let mut true_visitor = Self::default();
                let mut false_visitor = Self::default();
                true_visitor.visit_statement(pool, i.if_true);
                if let Some(if_false) = i.if_false {
                    false_visitor.visit_statement(pool, if_false);
                }
                // If either branch leads to a break or continue, we report the entire if as
                // containing a break or continue, since we don't know which side will be reached.
                self.found_break = true_visitor.found_break || false_visitor.found_break;
                self.found_continue = true_visitor.found_continue || false_visitor.found_continue;
                // On the other hand, we only want to report returns that definitely happen, so we
                // require those to be found on both sides.
                self.found_return = true_visitor.found_return && false_visitor.found_return;
                self.found_break || self.found_continue || self.found_return
            }
            StatementKind::For(f) => {
                // We assume a for/while loop runs for at least one iteration; this isn't strictly
                // guaranteed, but it's better to be slightly over-permissive here than to fail on
                // reasonable code. A loop's break or continue only exits the loop, so only a
                // return counts.
                let mut for_visitor = Self::default();
                for_visitor.visit_statement(pool, f.statement);
                self.found_return = for_visitor.found_return;
                self.found_return
            }
            StatementKind::Do(d) => {
                // Do-while blocks are always entered at least once. As with `for`, only a return
                // counts.
                let mut do_visitor = Self::default();
                do_visitor.visit_statement(pool, d.statement);
                self.found_return = do_visitor.found_return;
                self.found_return
            }
            // Blocks are definitely entered and don't imply any additional control flow. If the
            // block contains a break, continue or return, we want to keep that.
            StatementKind::Block(b) => b
                .children
                .iter()
                .any(|&child| self.visit_statement(pool, child)),
            StatementKind::Switch(s) => {
                // We need to verify that:
                // - a default case exists, so that every possible input value is covered
                // - every switch-case either (a) returns unconditionally, or
                //                            (b) falls through to another case that does
                let mut found_default = false;
                let mut fell_through = false;
                for &case in s.cases(pool) {
                    // A switch without a default case cannot definitively return, as its value
                    // might not be in the cases list.
                    if let StatementKind::SwitchCase(sc) = &pool.statement(case).kind
                        && sc.is_default
                    {
                        found_default = true;
                    }
                    // Scan this switch-case for any exit (break, continue or return).
                    let mut case_visitor = Self::default();
                    case_visitor.visit_statement(pool, case);
                    // If we found a break or continue, whether conditional or not, this switch
                    // case can't be called an unconditional return. Switches absorb breaks but not
                    // continues.
                    if case_visitor.found_continue {
                        self.found_continue = true;
                        return false;
                    }
                    if case_visitor.found_break {
                        return false;
                    }
                    // We just confirmed that there weren't any breaks or continues. If we didn't
                    // find an unconditional return either, the switch is considered fallen-through.
                    // (There might be a conditional return, but that doesn't count.)
                    fell_through = !case_visitor.found_return;
                }
                // If we didn't find a default case, or the very last case fell through, this
                // switch doesn't meet our criteria.
                if fell_through || !found_default {
                    return false;
                }
                // We scanned the entire switch, found a default case, and every section either
                // fell through or contained an unconditional return.
                self.found_return = true;
                true
            }
            // Recurse into the switch-case.
            StatementKind::SwitchCase(sc) => self.visit_statement(pool, sc.statement),
            // None of these statements could contain a return.
            StatementKind::Discard(_)
            | StatementKind::Expression(_)
            | StatementKind::Nop(_)
            | StatementKind::VarDeclaration(_) => false,
        }
    }
}

/// `Analysis::CanExitWithoutReturningValue`: whether a non-void function can reach its end
/// without returning a value.
// Port of: src/sksl/analysis/SkSLCanExitWithoutReturningValue.cpp#L163-L176 (chrome/m156)
fn can_exit_without_returning_value(pool: &IrPool, function: FnId, body: StmtId) -> bool {
    if pool.ty(pool.function(function).return_type).is_void() {
        return false;
    }
    let mut visitor = ReturnsOnAllPaths::default();
    visitor.visit_statement(pool, body);
    !visitor.found_return
}

#[cfg(test)]
mod tests {
    use crate::context::Context;
    use crate::error_reporter::{ErrorReporter, ErrorSink};
    use crate::ir::{
        BinaryExpression, Block, BlockKind, BreakStatement, ContinueStatement, Expression,
        ExpressionKind, ExpressionStatement, FnId, FunctionDefinition, Literal, ModifierFlags,
        Modifiers, Statement, StatementKind, StmtId, SymbolTable, Type, TypeId, VarDeclaration,
        VarId, Variable, VariableRefKind, VariableReference, VariableStorage,
    };
    use crate::modules::ModuleType;
    use crate::operator::{Operator, OperatorKind};
    use crate::position::Position;
    use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

    fn context(kind: ProgramKind) -> Context {
        let mut ctx = Context::new(ErrorReporter::forwarding());
        ctx.config = Some(ProgramConfig::new(
            ModuleType::Program,
            kind,
            ProgramSettings::default(),
        ));
        ctx.symbol_table = Some(ctx.pool.add_symbol_table(SymbolTable::new(None, false)));
        ctx
    }

    fn errors(ctx: &Context) -> Vec<String> {
        match ctx.errors.sink() {
            ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
            other => panic!("expected a forwarding reporter, found {other:?}"),
        }
    }

    fn statement(ctx: &mut Context, kind: StatementKind) -> StmtId {
        ctx.pool
            .add_statement(Statement::new(Position::default(), kind))
    }

    fn block(ctx: &mut Context, children: Vec<StmtId>) -> StmtId {
        statement(
            ctx,
            StatementKind::Block(Block {
                children,
                block_kind: BlockKind::BracedScope,
                symbol_table: None,
            }),
        )
    }

    fn declare(ctx: &mut Context, name: &str, return_type: TypeId) -> FnId {
        crate::ir::FunctionDeclaration::convert(
            ctx,
            Position::default(),
            &Modifiers::default(),
            name,
            &[],
            Position::default(),
            return_type,
        )
        .expect("declaration")
    }

    fn variable(ctx: &mut Context, name: &str, ty: TypeId, storage: VariableStorage) -> VarId {
        ctx.pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            ModifierFlags::empty(),
            name,
            ty,
            false,
            storage,
        ))
    }

    #[test]
    fn a_body_must_be_a_braced_block() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::VOID);
        let body = statement(&mut ctx, StatementKind::Nop(crate::ir::Nop));
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_none()
        );
        assert_eq!(
            errors(&ctx),
            ["function body 'void f()' must be a braced block"]
        );
    }

    #[test]
    fn a_function_is_defined_once() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::VOID);
        let body = block(&mut ctx, vec![]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        let again = block(&mut ctx, vec![]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(again)).is_none()
        );
        assert_eq!(errors(&ctx), ["function 'void f()' was already defined"]);
    }

    #[test]
    fn break_and_continue_need_a_loop() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::VOID);
        let brk = statement(&mut ctx, StatementKind::Break(BreakStatement));
        let cont = statement(&mut ctx, StatementKind::Continue(ContinueStatement));
        let body = block(&mut ctx, vec![brk, cont]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        assert_eq!(
            errors(&ctx),
            [
                "break statement must be inside a loop or switch",
                "continue statement must be inside a loop",
            ]
        );
    }

    #[test]
    fn a_non_void_function_must_return_on_every_path() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::FLOAT);
        let body = block(&mut ctx, vec![]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        assert_eq!(
            errors(&ctx),
            ["function 'f' can exit without returning a value"]
        );
    }

    #[test]
    fn a_non_void_function_with_a_bare_return_names_its_type() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::FLOAT);
        let ret = statement(
            &mut ctx,
            StatementKind::Return(crate::ir::ReturnStatement { expression: None }),
        );
        let body = block(&mut ctx, vec![ret]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        assert_eq!(errors(&ctx), ["expected function to return 'float'"]);
    }

    #[test]
    fn a_declaration_is_fused_with_its_first_assignment_when_optimizing() {
        let mut ctx = context(ProgramKind::Compute);
        ctx.config.as_mut().expect("config").settings.optimize = true;
        let f = declare(&mut ctx, "f", TypeId::VOID);
        let x = variable(&mut ctx, "x", TypeId::FLOAT, VariableStorage::Local);
        let decl = statement(
            &mut ctx,
            StatementKind::VarDeclaration(crate::ir::VarDeclaration {
                var: x,
                base_type: TypeId::FLOAT,
                array_size: 0,
                value: None,
            }),
        );
        let x_ref = ctx.pool.add_expression(Expression::new(
            Position::default(),
            TypeId::FLOAT,
            ExpressionKind::VariableReference(VariableReference {
                variable: x,
                ref_kind: VariableRefKind::Write,
            }),
        ));
        let one = ctx.pool.add_expression(Expression::new(
            Position::default(),
            TypeId::FLOAT,
            ExpressionKind::Literal(Literal { value: 1.0 }),
        ));
        let assign = ctx.pool.add_expression(Expression::new(
            Position::default(),
            TypeId::FLOAT,
            ExpressionKind::Binary(BinaryExpression {
                left: x_ref,
                operator: Operator::from(OperatorKind::Eq),
                right: one,
            }),
        ));
        let assign_stmt = statement(
            &mut ctx,
            StatementKind::Expression(ExpressionStatement { expression: assign }),
        );
        let body = block(&mut ctx, vec![decl, assign_stmt]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        assert_eq!(errors(&ctx), Vec::<String>::new());
        let StatementKind::VarDeclaration(d) = &ctx.pool.statement(decl).kind else {
            panic!("expected a declaration");
        };
        assert_eq!(d.value, Some(one));
        assert!(matches!(
            ctx.pool.statement(assign_stmt).kind,
            StatementKind::Nop(_)
        ));
    }

    #[test]
    fn a_variable_over_the_stack_limit_is_reported() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", TypeId::VOID);
        // A `float4x4` has 16 slots, so 6250 of them is exactly the 100000-slot limit.
        let array = Type::make_array_type(&ctx, "float4x4[6250]", TypeId::FLOAT4X4, 6250);
        let array = ctx.pool.add_type(array);
        let arr = variable(&mut ctx, "arr", array, VariableStorage::Local);
        let decl = statement(
            &mut ctx,
            StatementKind::VarDeclaration(VarDeclaration {
                var: arr,
                base_type: TypeId::FLOAT4X4,
                array_size: 6250,
                value: None,
            }),
        );
        let body = block(&mut ctx, vec![decl]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), f, Some(body)).is_some()
        );
        assert_eq!(
            errors(&ctx),
            ["variable 'arr' exceeds the stack size limit"]
        );
    }

    #[test]
    fn intrinsic_functions_are_not_defined() {
        let mut ctx = context(ProgramKind::Compute);
        ctx.config.as_mut().expect("config").module_type = ModuleType::SkslShared;
        let a = variable(&mut ctx, "a", TypeId::FLOAT, VariableStorage::Parameter);
        let abs = crate::ir::FunctionDeclaration::convert(
            &mut ctx,
            Position::default(),
            &Modifiers::default(),
            "abs",
            &[a],
            Position::default(),
            TypeId::FLOAT,
        )
        .expect("builtin abs");
        let body = block(&mut ctx, vec![]);
        assert!(
            FunctionDefinition::convert(&mut ctx, Position::default(), abs, Some(body)).is_none()
        );
        assert_eq!(
            errors(&ctx),
            ["intrinsic function 'abs' should not have a definition"]
        );
    }
}
