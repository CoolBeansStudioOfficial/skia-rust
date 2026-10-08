// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLInliner.{h,cpp}.

//! [`Inliner`]: converts a `FunctionCall` in the IR to a set of statements to be injected ahead of
//! the function call, and a replacement expression. It can also detect cases where inlining isn't
//! cleanly possible (e.g. return statements nested inside of a loop construct). The inliner isn't
//! able to guarantee identical-to-GLSL execution order if the inlined function has visible side
//! effects.
//!
//! Skia's `std::unique_ptr<Statement>*` and `std::unique_ptr<Expression>*` candidates are the
//! ids of the slots (see `docs/design/sksl.md` §4.3): a node that moves keeps its slot, which is
//! rewritten with `relocate_statement`/`replace_statement`/`move_expression_into`.
//!
//! Where Skia evaluates the arguments of a call in the order the C++ compiler picks, they are
//! evaluated left to right, as Clang does (the order is visible only in the mangler's counter).

use std::collections::HashMap;

use crate::analysis::{
    ProgramUsage, ReturnComplexity, SymbolTableStackBuilder, get_return_complexity,
    has_side_effects, is_trivial_expression, node_count_up_to_limit, update_variable_ref_kind,
};
use crate::context::Context;
use crate::ir::{
    BinaryExpression, Block, BlockKind, BreakStatement, ChildCall, ConstructorArray,
    ConstructorArrayCast, ConstructorCompound, ConstructorCompoundCast, ConstructorDiagonalMatrix,
    ConstructorMatrixResize, ConstructorScalarCast, ConstructorSplat, ConstructorStruct,
    ContinueStatement, DiscardStatement, DoStatement, ElemId, EmptyExpression, ExprId,
    ExpressionKind, ExpressionStatement, FieldAccess, FnId, ForStatement, FunctionCall,
    FunctionDefinition, IfStatement, IndexExpression, LoopUnrollInfo, ModifierFlags, Nop,
    PostfixExpression, PrefixExpression, ProgramElementKind, Setting, StatementKind, StmtId,
    SwitchCase, SwitchStatement, Swizzle, SymTabId, SymbolId, SymbolTable, TernaryExpression,
    TypeId, VarDeclaration, VarId, Variable, VariableRefKind, VariableReference, add_symbol,
    would_shadow_symbols_from,
};
use crate::mangler::Mangler;
use crate::operator::{Operator, OperatorKind};
use crate::position::{ForLoopPositions, Position};
use crate::transform::add_const_to_var_modifiers;

/// `Inliner::VariableRewriteMap`: the replacement for each variable of the inlined function.
type VariableRewriteMap = HashMap<VarId, ExprId>;

/// `Inliner::InlinedCall`: the statements to insert above the statement that held the call, and
/// the expression that replaces the call.
#[derive(Clone, Copy, Debug, Default)]
struct InlinedCall {
    /// `fInlinedBody`.
    body: Option<StmtId>,
    /// `fReplacementExpr`.
    replacement: Option<ExprId>,
}

/// The arguments of `inlineStatement` that stay the same through the recursion.
struct StatementEnv<'a> {
    pos: Position,
    return_complexity: ReturnComplexity,
    usage: &'a ProgramUsage,
    is_builtin_code: bool,
}

/// `SkSL::Inliner`.
// Port of: src/sksl/SkSLInliner.h#L35-L123 (chrome/m156)
#[doc(alias = "SkSL::Inliner")]
#[derive(Debug, Default)]
pub struct Inliner {
    /// `fMangler`.
    mangler: Mangler,
    /// `fInlinedStatementCounter`.
    inlined_statement_counter: i32,
}

// Port of: src/sksl/SkSLInliner.cpp#L77-L90 (chrome/m156)
fn is_scopeless_block(pool: &crate::ir::IrPool, stmt: StmtId) -> bool {
    matches!(&pool.statement(stmt).kind, StatementKind::Block(block) if !block.is_scope())
}

// Port of: src/sksl/SkSLInliner.cpp#L92-L110 (chrome/m156)
fn find_parent_statement(pool: &crate::ir::IrPool, stmt_stack: &[StmtId]) -> Option<StmtId> {
    // `stmt_stack` is not empty: the last element is the enclosing statement.
    // Walk the statement stack from back to front, ignoring the last element (which is the
    // enclosing statement). Anything counts as a parent statement other than a scopeless Block.
    stmt_stack
        .iter()
        .rev()
        .skip(1)
        .copied()
        .find(|&stmt| !is_scopeless_block(pool, stmt))
    // Otherwise there wasn't any parent statement to be found.
}

// Port of: src/sksl/SkSLInliner.cpp#L112-L118 (chrome/m156)
fn clone_with_ref_kind(
    ctx: &mut Context,
    expr: ExprId,
    ref_kind: VariableRefKind,
    pos: Position,
) -> ExprId {
    let clone = ctx.pool.clone_expression_at(expr, pos);
    update_variable_ref_kind(&mut ctx.pool, clone, ref_kind, None);
    clone
}

/// The definition `function` has, if any.
fn definition_of(pool: &crate::ir::IrPool, function: FnId) -> Option<FunctionDefinition> {
    let element = pool.function(function).definition?;
    match &pool.element(element).kind {
        ProgramElementKind::Function(def) => Some(def.clone()),
        _ => None,
    }
}

// Port of: src/sksl/SkSLInliner.cpp#L387-L404 (chrome/m156)
fn argument_needs_scratch_variable(
    pool: &crate::ir::IrPool,
    arg: ExprId,
    param: VarId,
    usage: &ProgramUsage,
) -> bool {
    // If the parameter isn't written to within the inline function ...
    let param_usage = usage.get_variable(param);
    if param_usage.write == 0 {
        // ... and it can be inlined trivially (e.g. a swizzle, or a constant array index),
        // or it is any expression without side effects that is only accessed at most once...
        let no_scratch = if param_usage.read > 1 {
            is_trivial_expression(pool, arg)
        } else {
            !has_side_effects(pool, arg)
        };
        if no_scratch {
            // ... we don't need to copy it at all! We can just use the existing expression.
            return false;
        }
    }
    // We need a scratch variable.
    true
}

impl Inliner {
    /// `Inliner(context)`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `RemapVariable`: searches the rewrite map for a rewritten variable for the passed-in one.
    // Port of: src/sksl/SkSLInliner.cpp#L120-L139 (chrome/m156)
    fn remap_variable(ctx: &Context, variable: VarId, var_map: &VariableRewriteMap) -> VarId {
        let Some(&remap) = var_map.get(&variable) else {
            debug_assert!(false, "rewrite map does not contain variable");
            return variable;
        };
        if let ExpressionKind::VariableReference(reference) = &ctx.pool.expression(remap).kind {
            reference.variable
        } else {
            debug_assert!(false, "rewrite map contains non-variable replacement");
            variable
        }
    }

    /// `ensureScopedBlocks`: adds a scope to inlined bodies returned by `inlineCall`, if one is
    /// required.
    // Port of: src/sksl/SkSLInliner.cpp#L141-L184 (chrome/m156)
    fn ensure_scoped_blocks(
        pool: &mut crate::ir::IrPool,
        inlined_body: Option<StmtId>,
        parent_stmt: Option<StmtId>,
    ) {
        // No changes necessary if this statement isn't actually a block.
        let Some(inlined_body) = inlined_body else {
            return;
        };
        if !matches!(pool.statement(inlined_body).kind, StatementKind::Block(_)) {
            return;
        }

        // No changes necessary if the parent statement doesn't require a scope.
        let Some(parent_stmt) = parent_stmt else {
            return;
        };
        if !(matches!(
            pool.statement(parent_stmt).kind,
            StatementKind::If(_) | StatementKind::For(_) | StatementKind::Do(_)
        ) || is_scopeless_block(pool, parent_stmt))
        {
            return;
        }

        // The inliner will create inlined function bodies as a Block containing multiple
        // statements, but no scope. Normally, this is fine, but if this block is used as the
        // statement for a do/for/if/while, the block needs to be scoped for the generated code to
        // match the intent. In the case of Blocks nested inside other Blocks, we add the scope to
        // the outermost block if needed.
        let mut nested_block = inlined_body;
        loop {
            let StatementKind::Block(block) = &pool.statement(nested_block).kind else {
                unreachable!("a nested block is a Block");
            };
            if block.is_scope() {
                // We found an explicit scope; all is well.
                return;
            }
            if block.children.len() == 1
                && matches!(
                    pool.statement(block.children[0]).kind,
                    StatementKind::Block(_)
                )
            {
                // This block wraps another unscoped block; we need to go deeper.
                nested_block = block.children[0];
                continue;
            }
            // We found a block containing real statements (not just more blocks), but no scope.
            // Let's add a scope to the outermost block.
            if let StatementKind::Block(block) = &mut pool.statement_mut(inlined_body).kind {
                block.block_kind = BlockKind::BracedScope;
            }
            return;
        }
    }

    /// `inlineExpression`: copies `expression` with the variables in `var_map` replaced.
    // Port of: src/sksl/SkSLInliner.cpp#L186-L372 (chrome/m156)
    #[allow(clippy::too_many_lines)] // One arm per expression kind, as Skia's switch.
    fn inline_expression(
        ctx: &mut Context,
        pos: Position,
        var_map: &VariableRewriteMap,
        symbol_table_for_expression: SymTabId,
        expression: ExprId,
    ) -> ExprId {
        let node = ctx.pool.expression(expression).clone();
        let table = symbol_table_for_expression;
        macro_rules! expr {
            ($e:expr) => {
                Self::inline_expression(ctx, pos, var_map, table, $e)
            };
        }
        macro_rules! arg_list {
            ($args:expr) => {{
                let mut args = Vec::with_capacity($args.len());
                for &arg in $args.iter() {
                    args.push(expr!(arg));
                }
                args
            }};
        }
        macro_rules! cloned_type {
            () => {
                node.ty
                    .clone_in(ctx, table)
                    .expect("inliner: a type that can be cloned")
            };
        }
        match node.kind {
            ExpressionKind::Binary(b) => {
                let left = expr!(b.left);
                let right = expr!(b.right);
                BinaryExpression::make(ctx, pos, left, b.operator, right)
            }
            ExpressionKind::Empty(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::FunctionReference(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::TypeReference(_) => ctx.pool.clone_expression_at(expression, pos),
            ExpressionKind::ChildCall(c) => {
                // If our variable remapping table contains the passed-in variable, the remapped
                // expression _must_ be another variable reference. SkSL doesn't allow opaque
                // types to participate in complex expressions.
                let child = match var_map.get(&c.child) {
                    Some(&remap) => {
                        if let ExpressionKind::VariableReference(reference) =
                            &ctx.pool.expression(remap).kind
                        {
                            reference.variable
                        } else {
                            debug_assert!(false, "child effect remaps to unexpected expression");
                            c.child
                        }
                    }
                    // There's no remapping for this; use it as-is.
                    None => c.child,
                };
                let ty = cloned_type!();
                let args = arg_list!(c.arguments);
                ChildCall::make(ctx, pos, ty, child, args)
            }
            ExpressionKind::ConstructorArray(c) => {
                let ty = cloned_type!();
                let args = arg_list!(c.arguments);
                ConstructorArray::make(ctx, pos, ty, args)
            }
            ExpressionKind::ConstructorArrayCast(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorArrayCast::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorCompound(c) => {
                let ty = cloned_type!();
                let args = arg_list!(c.arguments);
                ConstructorCompound::make(ctx, pos, ty, args)
            }
            ExpressionKind::ConstructorCompoundCast(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorCompoundCast::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorDiagonalMatrix(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorDiagonalMatrix::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorMatrixResize(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorMatrixResize::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorScalarCast(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorScalarCast::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorSplat(c) => {
                let ty = cloned_type!();
                let arg = expr!(c.argument);
                ConstructorSplat::make(ctx, pos, ty, arg)
            }
            ExpressionKind::ConstructorStruct(c) => {
                let ty = cloned_type!();
                let args = arg_list!(c.arguments);
                ConstructorStruct::make(ctx, pos, ty, args)
            }
            ExpressionKind::FieldAccess(f) => {
                let base = expr!(f.base);
                FieldAccess::make(ctx, pos, base, f.field_index, f.owner_kind)
            }
            ExpressionKind::FunctionCall(f) => {
                let ty = cloned_type!();
                let args = arg_list!(f.arguments);
                FunctionCall::make(ctx, pos, ty, f.function, args)
            }
            ExpressionKind::Index(i) => {
                let base = expr!(i.base);
                let index = expr!(i.index);
                IndexExpression::make(ctx, pos, base, index)
            }
            ExpressionKind::Prefix(p) => {
                let operand = expr!(p.operand);
                PrefixExpression::make(ctx, pos, p.operator, operand)
            }
            ExpressionKind::Postfix(p) => {
                let operand = expr!(p.operand);
                PostfixExpression::make(ctx, pos, operand, p.operator)
            }
            ExpressionKind::Setting(s) => Setting::make(ctx, pos, s.caps),
            ExpressionKind::Swizzle(s) => {
                let base = expr!(s.base);
                Swizzle::make(ctx, pos, base, s.components)
            }
            ExpressionKind::Ternary(t) => {
                let test = expr!(t.test);
                let if_true = expr!(t.if_true);
                let if_false = expr!(t.if_false);
                TernaryExpression::make(ctx, pos, test, if_true, if_false)
            }
            ExpressionKind::VariableReference(v) => {
                if let Some(&remap) = var_map.get(&v.variable) {
                    return clone_with_ref_kind(ctx, remap, v.ref_kind, pos);
                }
                ctx.pool.clone_expression_at(expression, pos)
            }
            ExpressionKind::Poison(_) => {
                panic!(
                    "unsupported expression: {}",
                    ctx.pool.expression_description(expression)
                )
            }
        }
    }

    /// `inlineStatement`: copies `statement` with the variables in `var_map` replaced, giving
    /// every declared variable a unique name.
    // Port of: src/sksl/SkSLInliner.cpp#L374-L578 (chrome/m156)
    #[allow(clippy::too_many_lines)] // One arm per statement kind, as Skia's switch.
    fn inline_statement(
        &mut self,
        ctx: &mut Context,
        env: &StatementEnv<'_>,
        var_map: &mut VariableRewriteMap,
        symbol_table_for_statement: SymTabId,
        result_expr: &mut Option<ExprId>,
        statement: StmtId,
    ) -> StmtId {
        let pos = env.pos;
        let table = symbol_table_for_statement;
        let node = ctx.pool.statement(statement).kind.clone();

        self.inlined_statement_counter += 1;

        match node {
            StatementKind::Block(block) => {
                // `makeWithChildSymbolTable`
                let child_symbols = ctx
                    .pool
                    .add_symbol_table(SymbolTable::new(Some(table), env.is_builtin_code));
                let mut statements = Vec::with_capacity(block.children.len());
                for &child in &block.children {
                    statements.push(self.inline_statement(
                        ctx,
                        env,
                        var_map,
                        child_symbols,
                        result_expr,
                        child,
                    ));
                }
                Block::make(
                    &mut ctx.pool,
                    pos,
                    statements,
                    block.block_kind,
                    Some(child_symbols),
                )
            }
            StatementKind::Break(_) => BreakStatement::make(&mut ctx.pool, pos),
            StatementKind::Continue(_) => ContinueStatement::make(&mut ctx.pool, pos),
            StatementKind::Discard(_) => DiscardStatement::make(ctx, pos),
            StatementKind::Do(d) => {
                let body =
                    self.inline_statement(ctx, env, var_map, table, result_expr, d.statement);
                let test = Self::inline_expression(ctx, pos, var_map, table, d.test);
                DoStatement::make(&mut ctx.pool, pos, body, test)
            }
            StatementKind::Expression(e) => {
                let expression = Self::inline_expression(ctx, pos, var_map, table, e.expression);
                ExpressionStatement::make(ctx, expression)
            }
            StatementKind::For(f) => {
                // `makeWithChildSymbolTable`
                let child_symbols = ctx
                    .pool
                    .add_symbol_table(SymbolTable::new(Some(table), env.is_builtin_code));
                // We need to ensure `initializer` is evaluated first, so that we've already
                // remapped its declaration by the time we evaluate `test` and `next`.
                let initializer = f.initializer.map(|initializer| {
                    self.inline_statement(
                        ctx,
                        env,
                        var_map,
                        child_symbols,
                        result_expr,
                        initializer,
                    )
                });
                let test = f
                    .test
                    .map(|test| Self::inline_expression(ctx, pos, var_map, child_symbols, test));
                let next = f
                    .next
                    .map(|next| Self::inline_expression(ctx, pos, var_map, child_symbols, next));
                let body = self.inline_statement(
                    ctx,
                    env,
                    var_map,
                    child_symbols,
                    result_expr,
                    f.statement,
                );

                let unroll_info = f.unroll_info.map(|info| {
                    // The for loop's unroll-info points to the Variable in the initializer as the
                    // index. This variable has been rewritten into a clone by the inliner, so we
                    // need to update the loop-unroll info to point to the clone.
                    LoopUnrollInfo {
                        index: Self::remap_variable(ctx, info.index, var_map),
                        ..info
                    }
                });

                ForStatement::make(
                    &mut ctx.pool,
                    pos,
                    ForLoopPositions::default(),
                    initializer,
                    test,
                    next,
                    body,
                    unroll_info,
                    Some(child_symbols),
                )
            }
            StatementKind::If(i) => {
                let test = Self::inline_expression(ctx, pos, var_map, table, i.test);
                let if_true =
                    self.inline_statement(ctx, env, var_map, table, result_expr, i.if_true);
                let if_false = i.if_false.map(|if_false| {
                    self.inline_statement(ctx, env, var_map, table, result_expr, if_false)
                });
                IfStatement::make(ctx, pos, test, if_true, if_false)
            }
            StatementKind::Nop(_) => Nop::make(&mut ctx.pool),
            StatementKind::Return(r) => {
                let Some(return_expr) = r.expression else {
                    // This function doesn't return a value. We won't inline functions with early
                    // returns, so a return statement is a no-op and can be treated as such.
                    return Nop::make(&mut ctx.pool);
                };

                // If a function only contains a single return, and it doesn't reference
                // variables from inside an Block's scope, we don't need to store the result in a
                // variable at all. Just replace the function-call expression with the function's
                // return expression.
                if env.return_complexity <= ReturnComplexity::SingleSafeReturn {
                    *result_expr = Some(Self::inline_expression(
                        ctx,
                        pos,
                        var_map,
                        table,
                        return_expr,
                    ));
                    return Nop::make(&mut ctx.pool);
                }

                // For more complex functions, we assign their result into a variable. We refuse
                // to inline anything with early returns, so this should be safe to do; that is,
                // on this control path, this is the last statement that will occur.
                let result = result_expr.expect("the result variable of the inlined function");
                let left = clone_with_ref_kind(ctx, result, VariableRefKind::Write, pos);
                let right = Self::inline_expression(ctx, pos, var_map, table, return_expr);
                let assign =
                    BinaryExpression::make(ctx, pos, left, Operator::from(OperatorKind::Eq), right);
                ExpressionStatement::make(ctx, assign)
            }
            StatementKind::Switch(ss) => {
                let value = Self::inline_expression(ctx, pos, var_map, table, ss.value);
                let case_block =
                    self.inline_statement(ctx, env, var_map, table, result_expr, ss.case_block);
                SwitchStatement::make(ctx, pos, value, case_block)
            }
            StatementKind::SwitchCase(sc) => {
                let body =
                    self.inline_statement(ctx, env, var_map, table, result_expr, sc.statement);
                if sc.is_default {
                    SwitchCase::make_default(&mut ctx.pool, pos, body)
                } else {
                    SwitchCase::make(&mut ctx.pool, pos, sc.value, body)
                }
            }
            StatementKind::VarDeclaration(decl) => {
                let initial_value = decl
                    .value
                    .map(|value| Self::inline_expression(ctx, pos, var_map, table, value));
                let variable = decl.var;

                // We assign unique names to inlined variables--scopes hide most of the problems
                // in this regard, but see `InlinerAvoidsVariableNameOverlap` for a counterexample
                // where unique names are important.
                let (old_name, modifiers_pos, layout, storage, variable_ty) = {
                    let v = ctx.pool.variable(variable);
                    (
                        v.name.clone(),
                        v.modifiers_position,
                        v.layout,
                        v.storage,
                        v.ty,
                    )
                };
                let name = self.mangler.unique_name(&old_name, &ctx.pool, table);
                let flags =
                    add_const_to_var_modifiers(&ctx.pool, variable, initial_value, env.usage);
                let var_ty = variable_ty
                    .clone_in(ctx, table)
                    .expect("inliner: a type that can be cloned");
                let cloned_var = Variable::make(
                    &mut ctx.pool,
                    pos,
                    modifiers_pos,
                    layout,
                    flags,
                    var_ty,
                    &name,
                    String::new(),
                    env.is_builtin_code,
                    storage,
                );
                let reference =
                    VariableReference::make(&mut ctx.pool, pos, cloned_var, VariableRefKind::Read);
                var_map.insert(variable, reference);
                let base_ty: TypeId = decl
                    .base_type
                    .clone_in(ctx, table)
                    .expect("inliner: a type that can be cloned");
                let result = VarDeclaration::make(
                    &mut ctx.pool,
                    cloned_var,
                    base_ty,
                    decl.array_size,
                    initial_value,
                );
                add_symbol(ctx, table, SymbolId::Variable(cloned_var));
                result
            }
        }
    }

    /// `inlineCall`: processes the passed-in `FunctionCall` expression. The call should be
    /// replaced with `replacement`. If present, `body` should be inserted immediately above the
    /// statement containing the inlined expression.
    // Port of: src/sksl/SkSLInliner.cpp#L406-L541 (chrome/m156)
    #[allow(clippy::too_many_lines)] // Mirrors `inlineCall`, which is one function.
    fn inline_call(
        &mut self,
        ctx: &mut Context,
        call: ExprId,
        symbol_table: SymTabId,
        usage: &ProgramUsage,
        caller: FnId,
    ) -> InlinedCall {
        // Inlining is more complicated here than in a typical compiler, because we have to have
        // a high-level IR and can't just drop statements into the middle of an expression or
        // even use gotos.
        //
        // Since we can't insert statements into an expression, we run the inline function as
        // extra statements before the statement we're currently processing, relying on a lack of
        // execution order guarantees.
        let (target, arguments, pos) = {
            let node = ctx.pool.expression(call);
            let ExpressionKind::FunctionCall(function_call) = &node.kind else {
                unreachable!("an inline candidate is a FunctionCall");
            };
            (
                function_call.function,
                function_call.arguments.clone(),
                node.position,
            )
        };
        let function = self
            .definition_of_safe(ctx, target, usage)
            .expect("inlineCall: the function is safe to inline");
        let function_position = ctx
            .pool
            .element(
                ctx.pool
                    .function(target)
                    .definition
                    .expect("a function with a definition"),
            )
            .position;
        let body_children = match &ctx.pool.statement(function.body).kind {
            StatementKind::Block(block) => block.children.clone(),
            _ => unreachable!("a function body is a Block"),
        };
        let return_complexity = get_return_complexity(&ctx.pool, &function);
        let (function_name, return_type, parameters) = {
            let declaration = ctx.pool.function(function.declaration);
            (
                declaration.name.clone(),
                declaration.return_type,
                declaration.parameters.clone(),
            )
        };
        let returns_void = ctx.pool.ty(return_type).is_void();

        let mut inline_statements: Vec<StmtId> =
            Vec::with_capacity(1 + arguments.len() + body_children.len());

        let mut result_expr: Option<ExprId> = None;
        if return_complexity > ReturnComplexity::SingleSafeReturn && !returns_void {
            // Create a variable to hold the result in the extra statements. We don't need to do
            // this for void-return functions, or in cases that are simple enough that we can just
            // replace the function-call node with the result expression.
            let var = Variable::make_scratch_variable(
                ctx,
                &mut self.mangler,
                &function_name,
                return_type,
                symbol_table,
                None,
            );
            inline_statements.push(var.decl);
            result_expr = Some(VariableReference::make(
                &mut ctx.pool,
                Position::default(),
                var.var,
                VariableRefKind::Read,
            ));
        }

        // Create variables in the extra statements to hold the arguments, and assign the
        // arguments to them.
        let mut var_map = VariableRewriteMap::new();
        for (i, &arg) in arguments.iter().enumerate() {
            let param = parameters[i];
            if !argument_needs_scratch_variable(&ctx.pool, arg, param, usage) {
                let cloned = ctx.pool.clone_expression(arg);
                var_map.insert(param, cloned);
                continue;
            }
            let param_name = ctx.pool.variable(param).name.clone();
            let arg_ty = ctx.pool.expression(arg).ty;
            let initial_value = ctx.pool.clone_expression(arg);
            let var = Variable::make_scratch_variable(
                ctx,
                &mut self.mangler,
                &param_name,
                arg_ty,
                symbol_table,
                Some(initial_value),
            );
            inline_statements.push(var.decl);
            let reference = VariableReference::make(
                &mut ctx.pool,
                Position::default(),
                var.var,
                VariableRefKind::Read,
            );
            var_map.insert(param, reference);
        }

        let env = StatementEnv {
            pos,
            return_complexity,
            usage,
            is_builtin_code: ctx.pool.function(caller).is_builtin(),
        };
        for &stmt in &body_children {
            inline_statements.push(self.inline_statement(
                ctx,
                &env,
                &mut var_map,
                symbol_table,
                &mut result_expr,
                stmt,
            ));
        }

        // Wrap all of the generated statements in a block. We need a real Block here, because we
        // need to add another child statement to the Block later.
        let body = Block::make_block(
            &mut ctx.pool,
            pos,
            inline_statements,
            BlockKind::UnbracedBlock,
            None,
        );
        if let Some(result_expr) = result_expr {
            // Return our result expression as-is.
            InlinedCall {
                body: Some(body),
                replacement: Some(result_expr),
            }
        } else if returns_void {
            // It's a void function, so its result is the empty expression.
            InlinedCall {
                body: Some(body),
                replacement: Some(EmptyExpression::make(ctx, pos)),
            }
        } else {
            // It's a non-void function, but it never created a result expression--that is, it
            // never returned anything on any path! This should have been detected in the function
            // finalizer. Still, discard our output and generate an error.
            debug_assert!(
                false,
                "inliner found non-void function that fails to return a value on any path"
            );
            ctx.errors.error(
                function_position,
                &format!(
                    "inliner found non-void function '{function_name}' that fails to return a \
                     value on any path"
                ),
            );
            InlinedCall::default()
        }
    }

    /// The definition of `function` when it is safe to inline (`SkASSERT(isSafeToInline(...))` in
    /// `inlineCall`).
    fn definition_of_safe(
        &self,
        ctx: &Context,
        function: FnId,
        usage: &ProgramUsage,
    ) -> Option<FunctionDefinition> {
        let definition = definition_of(&ctx.pool, function);
        debug_assert!(self.is_safe_to_inline(ctx, definition.as_ref(), usage));
        definition
    }

    /// `overInlineStatementLimit`.
    // Port of: src/sksl/SkSLInliner.cpp#L543-L550 (chrome/m156)
    fn over_inline_statement_limit(&self, ctx: &Context) -> bool {
        // Enforce a limit on inlining to avoid pathological cases.
        // (inliner/ExponentialGrowth.sksl) Modules, which are entirely built-in SkSL, do not need
        // to be limited in this way.
        const INLINED_STATEMENT_LIMIT: i32 = 2500;
        !ctx.config().is_builtin_code() && self.inlined_statement_counter >= INLINED_STATEMENT_LIMIT
    }

    /// `isSafeToInline`: checks whether inlining is viable for a `FunctionCall`, modulo recursion
    /// and function size.
    // Port of: src/sksl/SkSLInliner.cpp#L552-L592 (chrome/m156)
    fn is_safe_to_inline(
        &self,
        ctx: &Context,
        function_def: Option<&FunctionDefinition>,
        usage: &ProgramUsage,
    ) -> bool {
        // A threshold of zero indicates that the inliner is completely disabled, so we can just
        // return.
        if ctx.config().settings.inline_threshold <= 0 {
            return false;
        }

        // Enforce a limit on inlining to avoid pathological cases.
        // (inliner/ExponentialGrowth.sksl)
        if self.over_inline_statement_limit(ctx) {
            return false;
        }

        let Some(function_def) = function_def else {
            // Can't inline something if we don't actually have its definition.
            return false;
        };

        let declaration = ctx.pool.function(function_def.declaration);
        if declaration.modifier_flags.is_no_inline() {
            // Refuse to inline functions decorated with `noinline`.
            return false;
        }

        for &param in &declaration.parameters {
            // We don't allow inlining functions with parameters that are written-to, if they...
            // - are `out` parameters (see skbug.com/40042700 for rationale.)
            // - are arrays or structures (introducing temporary copies is non-trivial)
            let variable = ctx.pool.variable(param);
            let ty = ctx.pool.ty(variable.ty);
            if variable.modifier_flags.contains(ModifierFlags::OUT)
                || ty.is_array()
                || ty.is_struct()
            {
                let counts = usage.get_variable(param);
                if counts.write > 0 {
                    return false;
                }
            }
        }

        // We don't have a mechanism to simulate early returns, so we can't inline if there is
        // one.
        get_return_complexity(&ctx.pool, function_def) < ReturnComplexity::EarlyReturns
    }

    /// `functionCanBeInlined`.
    // Port of: src/sksl/SkSLInliner.cpp#L918-L928 (chrome/m156)
    fn function_can_be_inlined(
        &self,
        ctx: &Context,
        function: FnId,
        usage: &ProgramUsage,
        cache: &mut HashMap<FnId, bool>,
    ) -> bool {
        if let Some(&cached) = cache.get(&function) {
            return cached;
        }
        let definition = definition_of(&ctx.pool, function);
        let inlinability = self.is_safe_to_inline(ctx, definition.as_ref(), usage);
        cache.insert(function, inlinability);
        inlinability
    }

    /// `candidateCanBeInlined`.
    // Port of: src/sksl/SkSLInliner.cpp#L930-L957 (chrome/m156)
    fn candidate_can_be_inlined(
        &self,
        ctx: &Context,
        candidate: &InlineCandidate,
        usage: &ProgramUsage,
        cache: &mut HashMap<FnId, bool>,
    ) -> bool {
        // Check the cache to see if this function is safe to inline.
        let (function, arguments) = candidate_call(&ctx.pool, candidate);
        if !self.function_can_be_inlined(ctx, function, usage, cache) {
            return false;
        }

        // Even if the function is safe, the arguments we are passing may not be. In particular,
        // we can't make copies of opaque values, so we need to reject inline candidates that
        // would need to do this. Every call has different arguments, so this part is not
        // cacheable. (skbug.com/40044923)
        for (i, &arg) in arguments.iter().enumerate() {
            if ctx.pool.ty(ctx.pool.expression(arg).ty).is_opaque() {
                let param = ctx.pool.function(function).parameters[i];
                if argument_needs_scratch_variable(&ctx.pool, arg, param, usage) {
                    return false;
                }
            }
        }

        true
    }

    /// `getFunctionSize`.
    // Port of: src/sksl/SkSLInliner.cpp#L959-L968 (chrome/m156)
    fn get_function_size(ctx: &Context, function: FnId, cache: &mut HashMap<FnId, i32>) -> i32 {
        if let Some(&cached) = cache.get(&function) {
            return cached;
        }
        let definition =
            definition_of(&ctx.pool, function).expect("a candidate function has a definition");
        let size = node_count_up_to_limit(
            &ctx.pool,
            &definition,
            ctx.config().settings.inline_threshold,
        );
        cache.insert(function, size);
        size
    }

    /// `buildCandidateList`.
    // Port of: src/sksl/SkSLInliner.cpp#L970-L1042 (chrome/m156)
    fn build_candidate_list(
        &self,
        ctx: &Context,
        elements: &[ElemId],
        symbols: SymTabId,
        usage: &ProgramUsage,
    ) -> Vec<InlineCandidate> {
        // This is structured much like a ProgramVisitor, but does not actually use
        // ProgramVisitor. The analyzer needs to keep track of the slots of statements and
        // expressions so that they can later be replaced.
        let mut analyzer = InlineCandidateAnalyzer::default();
        analyzer.visit(&ctx.pool, elements, symbols);
        let mut candidates = analyzer.candidates;

        // Early out if there are no inlining candidates.
        if candidates.is_empty() {
            return candidates;
        }

        // Remove candidates that are not safe to inline.
        let mut cache = HashMap::new();
        candidates
            .retain(|candidate| self.candidate_can_be_inlined(ctx, candidate, usage, &mut cache));

        // If the inline threshold is unlimited, or if we have no candidates left, our candidate
        // list is complete.
        let inline_threshold = ctx.config().settings.inline_threshold;
        if inline_threshold == i32::MAX || candidates.is_empty() {
            return candidates;
        }

        // Remove candidates on a per-function basis if the effect of inlining would be to make
        // more than `inlineThreshold` nodes. (i.e. if Func() would be inlined six times and its
        // size is 10 nodes, it should be inlined if the inlineThreshold is 60 or higher.)
        let mut function_size_cache = HashMap::new();
        let mut candidate_total_cost: HashMap<FnId, i32> = HashMap::new();
        for candidate in &candidates {
            let (function, _) = candidate_call(&ctx.pool, candidate);
            *candidate_total_cost.entry(function).or_insert(0) +=
                Self::get_function_size(ctx, function, &mut function_size_cache);
        }

        candidates.retain(|candidate| {
            let (function, _) = candidate_call(&ctx.pool, candidate);
            if ctx.pool.function(function).modifier_flags.is_inline() {
                // Functions marked `inline` ignore size limitations.
                return true;
            }
            if usage.get_call_count(function) == 1 {
                // If a function is only used once, it's cost-free to inline.
                return true;
            }
            if candidate_total_cost[&function] <= inline_threshold {
                // We won't exceed the inline threshold by inlining this.
                return true;
            }
            // Inlining this function will add too many IRNodes.
            false
        });
        candidates
    }

    /// `analyze`: inlines any eligible functions that are found. Returns true if any changes are
    /// made. `symbols` is the symbol table of `elements` (it must be a table of `ctx.pool`).
    // Port of: src/sksl/SkSLInliner.cpp#L1044-L1186 (chrome/m156)
    pub fn analyze(
        &mut self,
        ctx: &mut Context,
        elements: &[ElemId],
        symbols: SymTabId,
        usage: &mut ProgramUsage,
    ) -> bool {
        // A threshold of zero indicates that the inliner is completely disabled, so we can just
        // return.
        if ctx.config().settings.inline_threshold <= 0 {
            return false;
        }

        // Enforce a limit on inlining to avoid pathological cases.
        // (inliner/ExponentialGrowth.sksl)
        if self.over_inline_statement_limit(ctx) {
            return false;
        }

        let candidate_list = self.build_candidate_list(ctx, elements, symbols, usage);

        // Inline the candidates where we've determined that it's safe to do so.
        let mut statement_remapping_table: HashMap<StmtId, StmtId> = HashMap::new();

        let mut made_changes = false;
        for candidate in &candidate_list {
            // Convert the function call to its inlined equivalent.
            let caller = candidate.enclosing_function;
            let inlined_call = self.inline_call(
                ctx,
                candidate.candidate_expr,
                candidate.symbols,
                usage,
                caller,
            );

            // Stop if an error was detected during the inlining process.
            let (Some(body), Some(replacement)) = (inlined_call.body, inlined_call.replacement)
            else {
                break;
            };

            // Ensure that the inlined body has a scope if it needs one.
            Self::ensure_scoped_blocks(&mut ctx.pool, Some(body), candidate.parent_stmt);

            // Add references within the inlined body
            usage.add_statement(&ctx.pool, body);

            // Look up the enclosing statement; remap it if necessary.
            let mut enclosing_stmt = candidate.enclosing_stmt;
            while let Some(&remapped) = statement_remapping_table.get(&enclosing_stmt) {
                enclosing_stmt = remapped;
            }

            // Move the enclosing statement to the end of the unscoped Block containing the
            // inlined function, then replace the enclosing statement with that Block.
            // Before:
            //     fInlinedBody = Block{ stmt1, stmt2, stmt3 }
            //     fEnclosingStmt = stmt4
            // After:
            //     fInlinedBody = null
            //     fEnclosingStmt = Block{ stmt1, stmt2, stmt3, stmt4 }
            let moved = ctx.pool.relocate_statement(enclosing_stmt);
            if let StatementKind::Block(block) = &mut ctx.pool.statement_mut(body).kind {
                block.children.push(moved);
            }
            ctx.pool.move_statement_into(enclosing_stmt, body);

            // Replace the candidate function call with our replacement expression.
            usage.remove_expression(&ctx.pool, candidate.candidate_expr);
            usage.add_expression(&ctx.pool, replacement);
            ctx.pool
                .move_expression_into(candidate.candidate_expr, replacement);
            made_changes = true;

            // If anything else pointed at our enclosing statement, it's now pointing at a Block
            // containing many other statements as well. Maintain a fix-up table to account for
            // this. The moved statement is the last child of the new Block.
            statement_remapping_table.insert(enclosing_stmt, moved);

            // Stop inlining if we've reached our hard cap on new statements.
            if self.over_inline_statement_limit(ctx) {
                break;
            }

            // Note that nothing was destroyed except for the FunctionCall. All other nodes should
            // remain valid.
        }

        made_changes
    }
}

/// `InlineCandidate`: a candidate function for inlining, containing everything that `inlineCall`
/// needs.
// Port of: src/sksl/SkSLInliner.cpp#L594-L601 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct InlineCandidate {
    /// `fSymbols`: the `SymbolTable` of the candidate.
    symbols: SymTabId,
    /// `fParentStmt`: the parent `Statement` of the enclosing stmt.
    parent_stmt: Option<StmtId>,
    /// `fEnclosingStmt`: the `Statement` containing the candidate.
    enclosing_stmt: StmtId,
    /// `fCandidateExpr`: the candidate `FunctionCall` to be inlined.
    candidate_expr: ExprId,
    /// `fEnclosingFunction`: the declaration of the function containing the candidate.
    enclosing_function: FnId,
}

/// The function a candidate calls, and the arguments of the call.
fn candidate_call(pool: &crate::ir::IrPool, candidate: &InlineCandidate) -> (FnId, Vec<ExprId>) {
    match &pool.expression(candidate.candidate_expr).kind {
        ExpressionKind::FunctionCall(call) => (call.function, call.arguments.clone()),
        _ => unreachable!("an inline candidate is a FunctionCall"),
    }
}

/// `InlineCandidateAnalyzer`: finds the function calls that could be inlined.
// Port of: src/sksl/SkSLInliner.cpp#L607-L898 (chrome/m156)
#[derive(Default)]
struct InlineCandidateAnalyzer {
    /// `fCandidateList`: all the inlining candidates we found during analysis.
    candidates: Vec<InlineCandidate>,
    /// `fSymbolTableStack`: a stack of the symbol tables; since most nodes don't have one,
    /// expected to be shallower than the enclosing-statement stack.
    symbol_table_stack: Vec<SymTabId>,
    /// `fEnclosingStmtStack`: a stack of "enclosing" statements--these would be suitable for the
    /// inliner to use for adding new instructions. Not all statements are suitable (e.g. a
    /// for-loop's initializer). The inliner might replace a statement with a block containing the
    /// statement.
    enclosing_stmt_stack: Vec<StmtId>,
    /// `fEnclosingFunction`: the function that we're currently processing (i.e. inlining into).
    enclosing_function: Option<FnId>,
}

impl InlineCandidateAnalyzer {
    fn visit(&mut self, pool: &crate::ir::IrPool, elements: &[ElemId], symbols: SymTabId) {
        self.symbol_table_stack.push(symbols);

        for &pe in elements {
            self.visit_program_element(pool, pe);
        }

        self.symbol_table_stack.pop();
    }

    fn visit_program_element(&mut self, pool: &crate::ir::IrPool, pe: ElemId) {
        // The inliner can't operate outside of a function's scope.
        let ProgramElementKind::Function(func_def) = &pool.element(pe).kind else {
            return;
        };

        // If this function has parameter names that would shadow globally-scoped names, we don't
        // scan it for inline candidates, because it's too late to mangle the names.
        let front = self.symbol_table_stack[0];
        let found_shadowing_parameter_name = pool
            .function(func_def.declaration)
            .parameters
            .iter()
            .any(|&param| {
                pool.find_symbol(front, &pool.variable(param).name)
                    .is_some()
            });

        if !found_shadowing_parameter_name {
            self.enclosing_function = Some(func_def.declaration);
            self.visit_statement(pool, func_def.body, true);
        }
    }

    fn visit_statement(
        &mut self,
        pool: &crate::ir::IrPool,
        stmt: StmtId,
        is_viable_as_enclosing_statement: bool,
    ) {
        let scoped_stack_builder =
            SymbolTableStackBuilder::new(pool, Some(stmt), &mut self.symbol_table_stack);
        // If this statement contains symbols that would shadow globally-scoped names, we don't
        // look for any inline candidates, because it's too late to mangle the names.
        if scoped_stack_builder.found_symbol_table()
            && would_shadow_symbols_from(
                pool,
                *self.symbol_table_stack.last().expect("a symbol table"),
                self.symbol_table_stack[0],
            )
        {
            scoped_stack_builder.finish(&mut self.symbol_table_stack);
            return;
        }

        let old_enclosing_stmt_stack_size = self.enclosing_stmt_stack.len();

        if is_viable_as_enclosing_statement {
            self.enclosing_stmt_stack.push(stmt);
        }

        match &pool.statement(stmt).kind {
            StatementKind::Break(_)
            | StatementKind::Continue(_)
            | StatementKind::Discard(_)
            | StatementKind::Nop(_) => {}

            StatementKind::Block(block) => {
                for &block_stmt in &block.children {
                    self.visit_statement(pool, block_stmt, true);
                }
            }
            StatementKind::Do(do_stmt) => {
                // The loop body is a candidate for inlining.
                self.visit_statement(pool, do_stmt.statement, true);
                // The inliner isn't smart enough to inline the test-expression for a do-while
                // loop at this time. There are two limitations:
                // - We would need to insert the inlined-body block at the very end of the do-
                //   statement's inner fStatement. We don't support that today, but it's doable.
                // - We cannot inline the test expression if the loop uses `continue` anywhere;
                //   that would skip over the inlined block that evaluates the test expression.
                //   There isn't a good fix for this--any workaround would be more complex than
                //   the cost of a function call. However, loops that don't use `continue` would
                //   still be viable candidates for inlining.
            }
            StatementKind::Expression(expr) => {
                self.visit_expression(pool, expr.expression);
            }
            StatementKind::For(for_stmt) => {
                // The initializer and loop body are candidates for inlining.
                if let Some(initializer) = for_stmt.initializer {
                    self.visit_statement(pool, initializer, false);
                }
                self.visit_statement(pool, for_stmt.statement, true);

                // The inliner isn't smart enough to inline the test- or increment-expressions of
                // a for loop loop at this time. There are a handful of limitations:
                // - We would need to insert the test-expression block at the very beginning of
                //   the for-loop's inner fStatement, and the increment-expression block at the
                //   very end. We don't support that today, but it's doable.
                // - The for-loop's built-in test-expression would need to be dropped entirely, and
                //   the loop would be halted via a break statement at the end of the inlined
                //   test-expression. This is again something we don't support today, but it
                //   could be implemented.
                // - We cannot inline the increment-expression if the loop uses `continue`
                //   anywhere; that would skip over the inlined block that evaluates the
                //   increment expression. There isn't a good fix for this--any workaround would
                //   be more complex than the cost of a function call. However, loops that don't
                //   use `continue` would still be viable candidates for increment-expression
                //   inlining.
            }
            StatementKind::If(if_stmt) => {
                self.visit_expression(pool, if_stmt.test);
                self.visit_statement(pool, if_stmt.if_true, true);
                if let Some(if_false) = if_stmt.if_false {
                    self.visit_statement(pool, if_false, true);
                }
            }
            StatementKind::Return(return_stmt) => {
                if let Some(expression) = return_stmt.expression {
                    self.visit_expression(pool, expression);
                }
            }
            StatementKind::Switch(switch_stmt) => {
                self.visit_expression(pool, switch_stmt.value);
                for &switch_case in switch_stmt.cases(pool) {
                    // The switch-case's fValue cannot be a FunctionCall; skip it.
                    let StatementKind::SwitchCase(sc) = &pool.statement(switch_case).kind else {
                        unreachable!("a switch's case block holds SwitchCases");
                    };
                    self.visit_statement(pool, sc.statement, true);
                }
            }
            StatementKind::VarDeclaration(var_decl_stmt) => {
                // Don't need to scan the declaration's sizes; those are always literals.
                if let Some(value) = var_decl_stmt.value {
                    self.visit_expression(pool, value);
                }
            }
            StatementKind::SwitchCase(_) => unreachable!("a SwitchCase is visited by its switch"),
        }

        // Pop our symbol and enclosing-statement stacks.
        self.enclosing_stmt_stack
            .truncate(old_enclosing_stmt_stack_size);
        scoped_stack_builder.finish(&mut self.symbol_table_stack);
    }

    fn visit_expression(&mut self, pool: &crate::ir::IrPool, expr: ExprId) {
        match &pool.expression(expr).kind {
            // Nothing to scan here. (Skia treats `Empty` and `Poison` as unreachable.)
            ExpressionKind::FieldAccess(_)
            | ExpressionKind::FunctionReference(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::Setting(_)
            | ExpressionKind::TypeReference(_)
            | ExpressionKind::VariableReference(_)
            | ExpressionKind::Empty(_)
            | ExpressionKind::Poison(_) => {}

            ExpressionKind::Binary(binary_expr) => {
                self.visit_expression(pool, binary_expr.left);

                // Logical-and and logical-or binary expressions do not inline the right side,
                // because that would invalidate short-circuiting. That is, when evaluating
                // expressions like these:
                //    (false && x())   // always false
                //    (true || y())    // always true
                // It is illegal for side-effects from x() or y() to occur. The simplest way to
                // enforce that rule is to avoid inlining the right side entirely. However, it is
                // safe for other types of binary expression to inline both sides.
                let op = binary_expr.operator;
                let short_circuitable = matches!(
                    op.kind(),
                    OperatorKind::LogicalAnd | OperatorKind::LogicalOr
                );
                if !short_circuitable {
                    self.visit_expression(pool, binary_expr.right);
                }
            }
            ExpressionKind::ChildCall(child_call_expr) => {
                for &arg in &child_call_expr.arguments {
                    self.visit_expression(pool, arg);
                }
            }
            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorArrayCast(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorCompoundCast(_)
            | ExpressionKind::ConstructorDiagonalMatrix(_)
            | ExpressionKind::ConstructorMatrixResize(_)
            | ExpressionKind::ConstructorScalarCast(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_) => {
                let arguments = pool
                    .expression(expr)
                    .any_constructor_arguments()
                    .expect("a constructor has arguments");
                for &arg in arguments {
                    self.visit_expression(pool, arg);
                }
            }
            ExpressionKind::FunctionCall(func_call_expr) => {
                for &arg in &func_call_expr.arguments {
                    self.visit_expression(pool, arg);
                }
                self.add_inline_candidate(pool, expr);
            }
            ExpressionKind::Index(index_expr) => {
                self.visit_expression(pool, index_expr.base);
                self.visit_expression(pool, index_expr.index);
            }
            ExpressionKind::Postfix(postfix_expr) => {
                self.visit_expression(pool, postfix_expr.operand);
            }
            ExpressionKind::Prefix(prefix_expr) => {
                self.visit_expression(pool, prefix_expr.operand);
            }
            ExpressionKind::Swizzle(swizzle_expr) => {
                self.visit_expression(pool, swizzle_expr.base);
            }
            ExpressionKind::Ternary(ternary_expr) => {
                // The test expression is a candidate for inlining.
                self.visit_expression(pool, ternary_expr.test);
                // The true- and false-expressions cannot be inlined, because we are only allowed
                // to evaluate one side.
            }
        }
    }

    fn add_inline_candidate(&mut self, pool: &crate::ir::IrPool, candidate: ExprId) {
        self.candidates.push(InlineCandidate {
            symbols: *self.symbol_table_stack.last().expect("a symbol table"),
            parent_stmt: find_parent_statement(pool, &self.enclosing_stmt_stack),
            enclosing_stmt: *self
                .enclosing_stmt_stack
                .last()
                .expect("an enclosing statement"),
            candidate_expr: candidate,
            enclosing_function: self.enclosing_function.expect("an enclosing function"),
        });
    }
}
