// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the S9a analyses on hand-built IR. Skia has no unit tests of these functions (they are
//! exercised through the compiler's golden tests, which later tasks bring in), so the expectations
//! here follow the C++ rules directly and use Skia's error texts.

use super::expression_queries::RTADJUST_NAME;
use super::*;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::ir::{
    BinaryExpression, Block, BlockKind, ComponentArray, ConstructorCompound, ConstructorSplat,
    ElemId, ExprId, Expression, ExpressionKind, ExpressionStatement, FnId, FunctionCall,
    FunctionDeclaration, FunctionDefinition, IndexExpression, IrPool, Literal, ModifierFlags,
    PrefixExpression, ProgramElement, ProgramElementKind, ReturnStatement, Statement,
    StatementKind, StmtId, Swizzle, SymTabId, SymbolTable, TernaryExpression, TypeId,
    VarDeclaration, VarId, Variable, VariableRefKind, VariableReference, VariableStorage,
};
use crate::modules::ModuleType;
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;

/// Builds IR by hand, as the converters of later tasks will.
struct Builder {
    pool: IrPool,
}

impl Builder {
    fn new() -> Self {
        Self {
            pool: IrPool::new(),
        }
    }

    fn expr(&mut self, ty: TypeId, kind: ExpressionKind) -> ExprId {
        self.pool
            .add_expression(Expression::new(Position::default(), ty, kind))
    }

    fn stmt(&mut self, kind: StatementKind) -> StmtId {
        self.pool
            .add_statement(Statement::new(Position::default(), kind))
    }

    fn elem(&mut self, kind: ProgramElementKind) -> ElemId {
        self.pool
            .add_element(ProgramElement::new(Position::default(), kind))
    }

    fn var(
        &mut self,
        name: &str,
        ty: TypeId,
        flags: ModifierFlags,
        storage: VariableStorage,
    ) -> VarId {
        self.pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            flags,
            name,
            ty,
            false,
            storage,
        ))
    }

    fn local(&mut self, name: &str, ty: TypeId) -> VarId {
        self.var(name, ty, ModifierFlags::empty(), VariableStorage::Local)
    }

    fn func(&mut self, name: &str, ret: TypeId, params: Vec<VarId>, flags: ModifierFlags) -> FnId {
        self.pool.add_function(FunctionDeclaration {
            position: Position::default(),
            name: name.into(),
            definition: None,
            next_overload: None,
            parameters: params,
            return_type: ret,
            modifier_flags: flags,
            intrinsic_kind: None,
            module_type: ModuleType::Program,
            is_main: name == "main",
            has_main_coords_parameter: false,
            has_main_input_color_parameter: false,
            has_main_dest_color_parameter: false,
        })
    }

    fn vref(&mut self, var: VarId, ref_kind: VariableRefKind) -> ExprId {
        let ty = self.pool.variable(var).ty;
        self.expr(
            ty,
            ExpressionKind::VariableReference(VariableReference {
                variable: var,
                ref_kind,
            }),
        )
    }

    fn float(&mut self, value: f64) -> ExprId {
        self.expr(TypeId::FLOAT, ExpressionKind::Literal(Literal { value }))
    }

    fn binary(&mut self, left: ExprId, op: OperatorKind, right: ExprId) -> ExprId {
        let ty = self.pool.expression(left).ty;
        self.expr(
            ty,
            ExpressionKind::Binary(BinaryExpression {
                left,
                operator: Operator::from(op),
                right,
            }),
        )
    }

    fn call(&mut self, function: FnId, arguments: Vec<ExprId>) -> ExprId {
        let ty = self.pool.function(function).return_type;
        let stable_pointer = self.pool.next_expression_id();
        self.expr(
            ty,
            ExpressionKind::FunctionCall(FunctionCall {
                function,
                arguments,
                stable_pointer,
            }),
        )
    }

    fn swizzle(&mut self, base: ExprId, components: &[i8]) -> ExprId {
        let ty = match components.len() {
            1 => TypeId::FLOAT,
            2 => TypeId::FLOAT2,
            _ => TypeId::FLOAT4,
        };
        self.expr(
            ty,
            ExpressionKind::Swizzle(Swizzle {
                base,
                components: ComponentArray::from_slice(components),
            }),
        )
    }

    fn stmt_expr(&mut self, expression: ExprId) -> StmtId {
        self.stmt(StatementKind::Expression(ExpressionStatement {
            expression,
        }))
    }

    fn ret(&mut self, expression: Option<ExprId>) -> StmtId {
        self.stmt(StatementKind::Return(ReturnStatement { expression }))
    }

    fn block(&mut self, children: Vec<StmtId>, symbol_table: Option<SymTabId>) -> StmtId {
        self.stmt(StatementKind::Block(Block {
            children,
            block_kind: BlockKind::BracedScope,
            symbol_table,
        }))
    }

    /// A `half4 main(half4 c) { return <body>; }`-shaped function definition.
    fn function_definition(&mut self, decl: FnId, body: StmtId) -> ElemId {
        self.elem(ProgramElementKind::Function(FunctionDefinition {
            declaration: decl,
            body,
        }))
    }
}

/// The messages reported to a forwarding reporter, in order.
fn messages(errors: &ErrorReporter) -> Vec<String> {
    match errors.sink() {
        ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
        _ => panic!("expected a forwarding reporter"),
    }
}

#[test]
fn usage_counts_reads_writes_and_declarations() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let one = b.float(1.0);
    let decl = b.stmt(StatementKind::VarDeclaration(VarDeclaration {
        var: x,
        base_type: TypeId::FLOAT,
        array_size: 0,
        value: Some(one),
    }));
    let read = b.vref(x, VariableRefKind::Read);
    let read_stmt = b.stmt_expr(read);

    let mut usage = ProgramUsage::default();
    usage.add_statement(&b.pool, decl);
    usage.add_statement(&b.pool, read_stmt);

    let counts = usage.get_variable(x);
    assert_eq!(counts.var_exists, 1);
    assert_eq!(counts.read, 1);
    // The initial value counts as a write.
    assert_eq!(counts.write, 1);

    // Removing the same nodes returns every count to zero.
    usage.remove_statement(&b.pool, read_stmt);
    usage.remove_statement(&b.pool, decl);
    assert_eq!(usage.get_variable(x), VariableCounts::default());
}

#[test]
fn usage_counts_calls_and_read_write_references() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let f = b.func("f", TypeId::FLOAT, vec![], ModifierFlags::empty());
    let call = b.call(f, vec![]);
    let target = b.vref(x, VariableRefKind::ReadWrite);
    let assign = b.binary(target, OperatorKind::Eq, call);

    let mut usage = ProgramUsage::default();
    usage.add_expression(&b.pool, assign);

    assert_eq!(usage.get_call_count(f), 1);
    let counts = usage.get_variable(x);
    assert_eq!((counts.read, counts.write), (1, 1));
}

#[test]
fn dead_variables_are_unread_and_unwritten() {
    let mut b = Builder::new();
    let plain = b.local("plain", TypeId::FLOAT);
    let uniform = b.var(
        "u",
        TypeId::FLOAT,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let read = b.local("read", TypeId::FLOAT);

    let mut usage = ProgramUsage::default();
    usage
        .variable_counts
        .insert(plain, VariableCounts::default());
    usage
        .variable_counts
        .insert(uniform, VariableCounts::default());
    usage.variable_counts.insert(
        read,
        VariableCounts {
            var_exists: 1,
            read: 1,
            write: 0,
        },
    );

    assert!(usage.is_dead(&b.pool, plain));
    // Uniforms are never eliminated, even when unused.
    assert!(!usage.is_dead(&b.pool, uniform));
    assert!(!usage.is_dead(&b.pool, read));
}

#[test]
fn usage_equality_ignores_zero_entries() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let read = b.vref(x, VariableRefKind::Read);

    let mut with_zero = ProgramUsage::default();
    with_zero.add_expression(&b.pool, read);
    with_zero.remove_expression(&b.pool, read);
    let fresh = ProgramUsage::default();

    assert_eq!(with_zero, fresh);

    let mut different = ProgramUsage::default();
    different.add_expression(&b.pool, read);
    assert_ne!(different, fresh);
}

#[test]
fn side_effects_follow_purity_assignments_and_increments() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let pure = b.func("cos", TypeId::FLOAT, vec![], ModifierFlags::PURE);
    let impure = b.func("noisy", TypeId::FLOAT, vec![], ModifierFlags::empty());

    let literal = b.float(1.0);
    assert!(!has_side_effects(&b.pool, literal));

    let pure_call = b.call(pure, vec![]);
    assert!(!has_side_effects(&b.pool, pure_call));

    let impure_call = b.call(impure, vec![]);
    assert!(has_side_effects(&b.pool, impure_call));

    let target = b.vref(x, VariableRefKind::Write);
    let assign = b.binary(target, OperatorKind::Eq, literal);
    assert!(has_side_effects(&b.pool, assign));

    let target = b.vref(x, VariableRefKind::ReadWrite);
    let increment = b.expr(
        TypeId::FLOAT,
        ExpressionKind::Prefix(PrefixExpression {
            operator: Operator::from(OperatorKind::PlusPlus),
            operand: target,
        }),
    );
    assert!(has_side_effects(&b.pool, increment));

    // A nested side effect is found through the tree.
    let sum = b.binary(literal, OperatorKind::Plus, impure_call);
    assert!(has_side_effects(&b.pool, sum));
}

#[test]
fn constant_expressions_and_compile_time_constants() {
    let mut b = Builder::new();
    let literal = b.float(2.0);
    assert!(is_constant_expression(&b.pool, literal));
    assert!(is_compile_time_constant(&b.pool, literal));

    let plain = b.local("plain", TypeId::FLOAT);
    let plain_ref = b.vref(plain, VariableRefKind::Read);
    assert!(!is_constant_expression(&b.pool, plain_ref));
    assert!(!is_compile_time_constant(&b.pool, plain_ref));

    let konst = b.var(
        "k",
        TypeId::FLOAT,
        ModifierFlags::CONST,
        VariableStorage::Local,
    );
    let konst_ref = b.vref(konst, VariableRefKind::Read);
    assert!(is_constant_expression(&b.pool, konst_ref));
    // A const variable is a constant expression but not a compile-time constant.
    assert!(!is_compile_time_constant(&b.pool, konst_ref));

    // A splat of a literal is a compile-time constant.
    let splat = b.expr(
        TypeId::FLOAT4,
        ExpressionKind::ConstructorSplat(ConstructorSplat { argument: literal }),
    );
    assert!(is_compile_time_constant(&b.pool, splat));
    assert!(is_constant_expression(&b.pool, splat));

    // A function call never is.
    let f = b.func("f", TypeId::FLOAT, vec![], ModifierFlags::PURE);
    let call = b.call(f, vec![]);
    assert!(!is_constant_expression(&b.pool, call));
}

#[test]
fn trivial_expressions_are_cheap_to_repeat() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT4);
    let x_ref = b.vref(x, VariableRefKind::Read);
    let x_alpha = b.swizzle(x_ref, &[3]);
    assert!(is_trivial_expression(&b.pool, x_alpha));

    let literal = b.float(1.0);
    let negated = b.expr(
        TypeId::FLOAT,
        ExpressionKind::Prefix(PrefixExpression {
            operator: Operator::from(OperatorKind::Minus),
            operand: literal,
        }),
    );
    assert!(is_trivial_expression(&b.pool, negated));

    let f = b.func("f", TypeId::FLOAT, vec![], ModifierFlags::PURE);
    let call = b.call(f, vec![]);
    assert!(!is_trivial_expression(&b.pool, call));

    // A ternary is never trivial.
    let ternary = b.expr(
        TypeId::FLOAT,
        ExpressionKind::Ternary(TernaryExpression {
            test: literal,
            if_true: literal,
            if_false: literal,
        }),
    );
    assert!(!is_trivial_expression(&b.pool, ternary));
}

#[test]
fn same_expression_trees_compare_structure() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT4);
    let y = b.local("y", TypeId::FLOAT4);

    let x1 = b.vref(x, VariableRefKind::Read);
    let x2 = b.vref(x, VariableRefKind::Read);
    let y1 = b.vref(y, VariableRefKind::Read);
    assert!(is_same_expression_tree(&b.pool, x1, x2));
    assert!(!is_same_expression_tree(&b.pool, x1, y1));

    let xa = b.swizzle(x1, &[3]);
    let xa_again = b.swizzle(x2, &[3]);
    let xr = b.swizzle(x2, &[0]);
    assert!(is_same_expression_tree(&b.pool, xa, xa_again));
    assert!(!is_same_expression_tree(&b.pool, xa, xr));

    let a = b.float(0.5);
    let a_again = b.float(0.5);
    let other = b.float(0.25);
    assert!(is_same_expression_tree(&b.pool, a, a_again));
    assert!(!is_same_expression_tree(&b.pool, a, other));

    // Different kinds are never the same.
    assert!(!is_same_expression_tree(&b.pool, a, x1));

    let first = b.expr(
        TypeId::FLOAT4,
        ExpressionKind::ConstructorCompound(ConstructorCompound {
            arguments: vec![a, a],
        }),
    );
    let second = b.expr(
        TypeId::FLOAT4,
        ExpressionKind::ConstructorCompound(ConstructorCompound {
            arguments: vec![a_again, a_again],
        }),
    );
    let third = b.expr(
        TypeId::FLOAT4,
        ExpressionKind::ConstructorCompound(ConstructorCompound {
            arguments: vec![a, other],
        }),
    );
    assert!(is_same_expression_tree(&b.pool, first, second));
    assert!(!is_same_expression_tree(&b.pool, first, third));
}

#[test]
fn dynamically_uniform_expressions_use_only_uniforms_and_constants() {
    let mut b = Builder::new();
    let uniform = b.var(
        "u",
        TypeId::FLOAT,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let local = b.local("l", TypeId::FLOAT);
    let u_ref = b.vref(uniform, VariableRefKind::Read);
    let l_ref = b.vref(local, VariableRefKind::Read);
    let literal = b.float(1.0);

    let from_uniform = b.binary(u_ref, OperatorKind::Star, literal);
    assert!(is_dynamically_uniform_expression(&b.pool, from_uniform));

    let from_local = b.binary(u_ref, OperatorKind::Star, l_ref);
    assert!(!is_dynamically_uniform_expression(&b.pool, from_local));
}

#[test]
fn sample_usage_merges_to_the_greater_kind() {
    let mut usage = SampleUsage::default();
    assert_eq!(usage.kind(), SampleUsageKind::None);
    assert!(!usage.is_sampled());

    usage.merge(&SampleUsage::pass_through());
    assert_eq!(usage.kind(), SampleUsageKind::PassThrough);
    assert!(usage.is_pass_through());

    usage.merge(&SampleUsage::explicit());
    assert!(usage.is_explicit());

    // Explicit wins over a later pass-through.
    usage.merge(&SampleUsage::pass_through());
    assert!(usage.is_explicit());
    assert!(usage.is_sampled());
    assert!(!usage.has_perspective());

    assert_eq!(SampleUsage::frag_coord().kind(), SampleUsageKind::FragCoord);
    assert!(SampleUsage::uniform_matrix(true).has_perspective());
    assert_eq!(SampleUsage::matrix_uniform_name(), "matrix");
}

#[test]
fn assignability_reports_each_kind_of_bad_target() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT4);
    let konst = b.var(
        "k",
        TypeId::FLOAT,
        ModifierFlags::CONST,
        VariableStorage::Local,
    );
    let input = b.var(
        "in",
        TypeId::FLOAT4,
        ModifierFlags::IN,
        VariableStorage::Global,
    );

    let mut errors = ErrorReporter::forwarding();

    // A plain local is assignable, and the write is recorded.
    let x_ref = b.vref(x, VariableRefKind::Read);
    let mut info = AssignmentInfo::default();
    assert!(is_assignable(
        &b.pool,
        x_ref,
        Some(&mut info),
        Some(&mut errors)
    ));
    assert_eq!(info.assigned_var, Some(x_ref));

    // A swizzle of a local is assignable.
    let x_xy = b.swizzle(x_ref, &[0, 1]);
    assert!(is_assignable(&b.pool, x_xy, None, Some(&mut errors)));

    // Writing the same component twice is not.
    let x_twice = b.swizzle(x_ref, &[0, 0]);
    assert!(!is_assignable(&b.pool, x_twice, None, Some(&mut errors)));

    // Const and pipeline inputs are not.
    let k_ref = b.vref(konst, VariableRefKind::Read);
    assert!(!is_assignable(&b.pool, k_ref, None, Some(&mut errors)));
    let in_ref = b.vref(input, VariableRefKind::Read);
    assert!(!is_assignable(&b.pool, in_ref, None, Some(&mut errors)));

    // A literal is not assignable at all.
    let literal = b.float(1.0);
    assert!(!is_assignable(&b.pool, literal, None, Some(&mut errors)));

    assert_eq!(
        messages(&errors),
        vec![
            "cannot write to the same swizzle field more than once".to_owned(),
            "cannot modify immutable variable 'k'".to_owned(),
            "cannot modify pipeline input variable 'in'".to_owned(),
            "cannot assign to this expression".to_owned(),
        ]
    );
}

#[test]
fn update_variable_ref_kind_marks_the_written_reference() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let x_ref = b.vref(x, VariableRefKind::Read);

    assert!(update_variable_ref_kind(
        &mut b.pool,
        x_ref,
        VariableRefKind::Write,
        None
    ));
    let ExpressionKind::VariableReference(reference) = &b.pool.expression(x_ref).kind else {
        panic!("not a variable reference");
    };
    assert_eq!(reference.ref_kind, VariableRefKind::Write);

    // A literal cannot be assigned, and the error names the expression.
    let literal = b.float(1.0);
    let mut errors = ErrorReporter::forwarding();
    assert!(!update_variable_ref_kind(
        &mut b.pool,
        literal,
        VariableRefKind::Write,
        Some(&mut errors)
    ));
    assert_eq!(
        messages(&errors),
        vec!["cannot assign to this expression".to_owned()]
    );
}

#[test]
fn rt_adjust_variables_and_root_variables_are_found() {
    let mut b = Builder::new();
    let rt = b.var(
        RTADJUST_NAME,
        TypeId::FLOAT4,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let rt_ref = b.vref(rt, VariableRefKind::Read);
    assert!(contains_rt_adjust(&b.pool, rt_ref));

    let x = b.local("x", TypeId::FLOAT4);
    let x_ref = b.vref(x, VariableRefKind::Read);
    assert!(!contains_rt_adjust(&b.pool, x_ref));
    assert!(contains_variable(&b.pool, x_ref, x));
    assert!(!contains_variable(&b.pool, x_ref, rt));

    let x_xy = b.swizzle(x_ref, &[0, 1]);
    assert_eq!(get_root_variable(&b.pool, x_xy), Some(x));
    let literal = b.float(1.0);
    assert_eq!(get_root_variable(&b.pool, literal), None);

    let index = b.expr(
        TypeId::FLOAT,
        ExpressionKind::Index(IndexExpression {
            base: x_ref,
            index: literal,
        }),
    );
    assert_eq!(get_root_variable(&b.pool, index), Some(x));
}

#[test]
fn color_filters_that_return_input_alpha_are_recognized() {
    let mut b = Builder::new();
    let input = b.local("c", TypeId::HALF4);
    let main = b.func("main", TypeId::HALF4, vec![input], ModifierFlags::empty());
    let usage = ProgramUsage::default();

    // return c.a;  -- the alpha of the input is preserved.
    let input_ref = b.vref(input, VariableRefKind::Read);
    let alpha = b.swizzle(input_ref, &[3]);
    let ret = b.ret(Some(alpha));
    let body = b.block(vec![ret], None);
    let element = b.function_definition(main, body);
    assert!(returns_input_alpha(&b.pool, element, &usage));

    // return c.r;  -- it is not.
    let input_ref = b.vref(input, VariableRefKind::Read);
    let red = b.swizzle(input_ref, &[0]);
    let ret = b.ret(Some(red));
    let body = b.block(vec![ret], None);
    let element = b.function_definition(main, body);
    assert!(!returns_input_alpha(&b.pool, element, &usage));

    // A written input is never trusted, even if it is returned as-is at the end.
    let mut written = ProgramUsage::default();
    written.variable_counts.insert(
        input,
        VariableCounts {
            var_exists: 1,
            read: 1,
            write: 1,
        },
    );
    let input_ref = b.vref(input, VariableRefKind::Read);
    let alpha = b.swizzle(input_ref, &[3]);
    let ret = b.ret(Some(alpha));
    let body = b.block(vec![ret], None);
    let element = b.function_definition(main, body);
    assert!(!returns_input_alpha(&b.pool, element, &written));
}

#[test]
fn symbol_table_stack_pushes_block_tables_and_pops_on_finish() {
    let mut b = Builder::new();
    let table = b.pool.add_symbol_table(SymbolTable::new(None, false));
    let scoped = b.block(vec![], Some(table));
    let unscoped = b.block(vec![], None);

    let mut stack = Vec::new();
    let builder = SymbolTableStackBuilder::new(&b.pool, Some(scoped), &mut stack);
    assert!(builder.found_symbol_table());
    assert_eq!(stack, vec![table]);
    builder.finish(&mut stack);
    assert_eq!(stack, Vec::new());

    let builder = SymbolTableStackBuilder::new(&b.pool, Some(unscoped), &mut stack);
    assert!(!builder.found_symbol_table());
    builder.finish(&mut stack);
    assert_eq!(stack, Vec::new());

    let builder = SymbolTableStackBuilder::new(&b.pool, None, &mut stack);
    builder.finish(&mut stack);
    assert_eq!(stack, Vec::new());
}

#[test]
fn statement_writes_are_found_through_the_statement() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let y = b.local("y", TypeId::FLOAT);
    let x_write = b.vref(x, VariableRefKind::Write);
    let literal = b.float(1.0);
    let assign = b.binary(x_write, OperatorKind::Eq, literal);
    let stmt = b.stmt_expr(assign);

    assert!(statement_writes_to_variable(&b.pool, stmt, x));
    assert!(!statement_writes_to_variable(&b.pool, stmt, y));
}

#[test]
fn node_count_stops_at_the_limit() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let read = b.vref(x, VariableRefKind::Read);
    let body_stmt = b.stmt_expr(read);
    let body = b.block(vec![body_stmt], None);
    let f = b.func("f", TypeId::FLOAT, vec![], ModifierFlags::empty());
    let def = FunctionDefinition {
        declaration: f,
        body,
    };

    // The block, the expression statement and the variable reference.
    assert_eq!(node_count_up_to_limit(&b.pool, &def, 100), 3);
    assert_eq!(node_count_up_to_limit(&b.pool, &def, 2), 2);
}

#[test]
fn variable_declarations_without_scope_are_detected() {
    let mut b = Builder::new();
    let x = b.local("x", TypeId::FLOAT);
    let decl = b.stmt(StatementKind::VarDeclaration(VarDeclaration {
        var: x,
        base_type: TypeId::FLOAT,
        array_size: 0,
        value: None,
    }));
    let mut errors = ErrorReporter::forwarding();
    assert!(detect_var_declaration_without_scope(
        &b.pool,
        decl,
        Some(&mut errors)
    ));
    assert_eq!(
        messages(&errors),
        vec!["variable 'x' must be created in a scope".to_owned()]
    );

    // A braced scope holding the same declaration is fine.
    let scoped = b.block(vec![decl], None);
    assert!(!detect_var_declaration_without_scope(&b.pool, scoped, None));
}
