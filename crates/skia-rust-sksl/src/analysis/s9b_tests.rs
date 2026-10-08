// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the S9b analyses. The IR is built by hand, because the parser does not exist yet.
//! The expected error texts are copied from the `tests/sksl` goldens of the pinned tree, which
//! the comments name.

use super::{
    LoopControlFlowInfo, ReturnComplexity, SpecializationInfo, SymbolTableStackBuilder,
    UNSPECIALIZED, can_exit_without_returning_value, check_program_structure,
    check_symbol_table_correctness, detect_var_declaration_without_scope, do_finalization_checks,
    find_functions_to_specialize, find_specialization_index_for_call,
    find_specialized_parameters_for_function, get_loop_control_flow_info, get_loop_unroll_info,
    get_parameter_mappings_for_function, get_return_complexity,
    switch_case_contains_conditional_exit, switch_case_contains_unconditional_exit,
    validate_indexing_for_es2,
};
use crate::analysis::s9b_shims::WriteCounts;
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::ir::{
    BinaryExpression, Block, BlockKind, BreakStatement, ContinueStatement, DoStatement, ElemId,
    ExprId, Expression, ExpressionKind, ExpressionStatement, FnId, ForStatement, FunctionCall,
    FunctionDeclaration, FunctionDefinition, GlobalVarDeclaration, IfStatement, IndexExpression,
    InterfaceBlock, IrPool, Layout, Literal, LoopUnrollInfo, ModifierFlags, ModifiersDeclaration,
    PostfixExpression, ProgramElement, ProgramElementKind, ReturnStatement, Statement,
    StatementKind, StmtId, SwitchCase, SwitchStatement, SymTabId, SymbolId, SymbolTable, Type,
    TypeId, VarDeclaration, VarId, Variable, VariableRefKind, VariableReference, VariableStorage,
};
use crate::modules::ModuleType;
use crate::operator::{Operator, OperatorKind};
use crate::position::{ForLoopPositions, Position};
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

/// Builds IR by hand.
struct Ir {
    pool: IrPool,
}

impl Ir {
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

    fn array_type(&mut self, component: TypeId, count: i32) -> TypeId {
        let ty = self.pool.ty(component);
        let name = ty.array_name(count);
        let abbrev = ty.abbreviated_name;
        self.pool
            .add_type(Type::new_array_type(name, abbrev, component, count, false))
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

    fn param(&mut self, name: &str, ty: TypeId) -> VarId {
        self.var(name, ty, ModifierFlags::empty(), VariableStorage::Parameter)
    }

    fn local(&mut self, name: &str, ty: TypeId) -> VarId {
        self.var(name, ty, ModifierFlags::empty(), VariableStorage::Local)
    }

    fn func(&mut self, name: &str, ret: TypeId, params: Vec<VarId>) -> FnId {
        self.pool.add_function(FunctionDeclaration {
            position: Position::default(),
            name: name.into(),
            definition: None,
            next_overload: None,
            parameters: params,
            return_type: ret,
            modifier_flags: ModifierFlags::empty(),
            intrinsic_kind: None,
            module_type: ModuleType::Program,
            is_main: name == "main",
            has_main_coords_parameter: false,
            has_main_input_color_parameter: false,
            has_main_dest_color_parameter: false,
        })
    }

    fn read(&mut self, var: VarId) -> ExprId {
        self.vref(var, VariableRefKind::Read)
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

    fn int(&mut self, value: i32) -> ExprId {
        self.expr(
            TypeId::INT,
            ExpressionKind::Literal(Literal {
                value: f64::from(value),
            }),
        )
    }

    fn float(&mut self, value: f64) -> ExprId {
        self.expr(TypeId::FLOAT, ExpressionKind::Literal(Literal { value }))
    }

    fn binary(&mut self, left: ExprId, op: OperatorKind, right: ExprId, ty: TypeId) -> ExprId {
        self.expr(
            ty,
            ExpressionKind::Binary(BinaryExpression {
                left,
                operator: Operator::from(op),
                right,
            }),
        )
    }

    fn postfix(&mut self, operand: ExprId, op: OperatorKind) -> ExprId {
        let ty = self.pool.expression(operand).ty;
        self.expr(
            ty,
            ExpressionKind::Postfix(PostfixExpression {
                operand,
                operator: Operator::from(op),
            }),
        )
    }

    fn index(&mut self, base: ExprId, index: ExprId) -> ExprId {
        self.expr(
            TypeId::INT,
            ExpressionKind::Index(IndexExpression { base, index }),
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

    fn ret(&mut self, value: Option<ExprId>) -> StmtId {
        self.stmt(StatementKind::Return(ReturnStatement { expression: value }))
    }

    fn expr_stmt(&mut self, expression: ExprId) -> StmtId {
        self.stmt(StatementKind::Expression(ExpressionStatement {
            expression,
        }))
    }

    fn block(&mut self, children: Vec<StmtId>) -> StmtId {
        self.scope(children, None)
    }

    fn scope(&mut self, children: Vec<StmtId>, symbol_table: Option<SymTabId>) -> StmtId {
        self.stmt(StatementKind::Block(Block {
            children,
            block_kind: BlockKind::BracedScope,
            symbol_table,
        }))
    }

    fn if_stmt(&mut self, test: ExprId, if_true: StmtId, if_false: Option<StmtId>) -> StmtId {
        self.stmt(StatementKind::If(IfStatement {
            test,
            if_true,
            if_false,
        }))
    }

    fn var_decl(
        &mut self,
        var: VarId,
        base_type: TypeId,
        array_size: i32,
        value: Option<ExprId>,
    ) -> StmtId {
        let id = self.stmt(StatementKind::VarDeclaration(VarDeclaration {
            var,
            base_type,
            array_size,
            value,
        }));
        self.pool.variable_mut(var).set_var_declaration(id);
        id
    }

    fn for_stmt(
        &mut self,
        initializer: Option<StmtId>,
        test: Option<ExprId>,
        next: Option<ExprId>,
        statement: StmtId,
    ) -> StmtId {
        self.stmt(StatementKind::For(ForStatement {
            for_loop_positions: ForLoopPositions::default(),
            symbol_table: None,
            initializer,
            test,
            next,
            statement,
            unroll_info: None,
        }))
    }

    /// A function definition whose body is the statement `body`.
    fn define_body(&mut self, function: FnId, body: StmtId) -> ElemId {
        let elem = self.elem(ProgramElementKind::Function(FunctionDefinition {
            declaration: function,
            body,
        }));
        self.pool.function_mut(function).set_definition(elem);
        elem
    }

    /// A function definition whose body is a block of `children`.
    fn define(&mut self, function: FnId, children: Vec<StmtId>) -> ElemId {
        let body = self.block(children);
        self.define_body(function, body)
    }

    /// The payload of a function definition element.
    fn function_def(&self, elem: ElemId) -> FunctionDefinition {
        match &self.pool.element(elem).kind {
            ProgramElementKind::Function(def) => def.clone(),
            _ => panic!("expected a function definition"),
        }
    }
}

/// A context over `pool` whose program configuration is of `kind`.
fn context(pool: IrPool, kind: ProgramKind) -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    ctx.pool = pool;
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        kind,
        ProgramSettings::default(),
    ));
    ctx
}

/// The messages that a forwarding reporter recorded, in order.
fn messages(errors: &ErrorReporter) -> Vec<String> {
    match errors.sink() {
        ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
        _ => panic!("the test reporter must be a forwarding reporter"),
    }
}

/// Runs the unroll analysis on `for (index; test; next) body`, reporting errors.
fn unroll(
    ctx: &mut Context,
    initializer: StmtId,
    test: ExprId,
    next: ExprId,
    body: StmtId,
) -> Option<LoopUnrollInfo> {
    let mut test_slot = Some(test);
    get_loop_unroll_info(
        ctx,
        Position::default(),
        &ForLoopPositions::default(),
        Some(initializer),
        &mut test_slot,
        Some(next),
        body,
        true,
    )
}

#[test]
fn finalization_reports_an_out_parameter_that_is_never_assigned() {
    // Golden: errors/UnassignedOutParameter.glsl.
    let mut ir = Ir::new();
    let a = ir.var(
        "a",
        TypeId::INT,
        ModifierFlags::OUT,
        VariableStorage::Parameter,
    );
    let f = ir.func("testOut", TypeId::VOID, vec![a]);
    let def = ir.define(f, vec![]);
    let usage = WriteCounts::for_elements(&ir.pool, &[def]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    do_finalization_checks(&mut ctx, &usage, &[def]);
    assert_eq!(
        messages(&ctx.errors),
        ["function 'testOut' never assigns a value to out parameter 'a'"]
    );
}

#[test]
fn finalization_accepts_an_out_parameter_that_is_assigned() {
    let mut ir = Ir::new();
    let a = ir.var(
        "a",
        TypeId::INT,
        ModifierFlags::OUT,
        VariableStorage::Parameter,
    );
    let f = ir.func("testOut", TypeId::VOID, vec![a]);
    let target = ir.vref(a, VariableRefKind::Write);
    let value = ir.int(1);
    let assign = ir.binary(target, OperatorKind::Eq, value, TypeId::INT);
    let stmt = ir.expr_stmt(assign);
    let def = ir.define(f, vec![stmt]);
    let usage = WriteCounts::for_elements(&ir.pool, &[def]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    do_finalization_checks(&mut ctx, &usage, &[def]);
    assert_eq!(messages(&ctx.errors), Vec::<String>::new());
}

#[test]
fn finalization_reports_a_duplicate_binding() {
    // Golden: errors/DuplicateBinding.glsl, the first line of its errors.
    let mut ir = Ir::new();
    let mut layout = Layout::new();
    layout.set = 0;
    layout.binding = 0;
    let first = ir.var(
        "bufferTwo",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::InterfaceBlock,
    );
    ir.pool.variable_mut(first).layout = layout;
    let second = ir.var(
        "bufferFour",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::InterfaceBlock,
    );
    ir.pool.variable_mut(second).layout = layout;
    let a = ir.elem(ProgramElementKind::InterfaceBlock(InterfaceBlock {
        var: first,
    }));
    let b = ir.elem(ProgramElementKind::InterfaceBlock(InterfaceBlock {
        var: second,
    }));
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    do_finalization_checks(&mut ctx, &WriteCounts::default(), &[a, b]);
    assert_eq!(
        messages(&ctx.errors),
        ["layout(set=0, binding=0) has already been defined"]
    );
}

#[test]
fn finalization_reports_a_local_size_given_twice() {
    // Golden: errors/DuplicateWorkgroupSize.glsl.
    let mut ir = Ir::new();
    let mut layout = Layout::new();
    layout.local_size_x = 16;
    let first = ir.elem(ProgramElementKind::Modifiers(ModifiersDeclaration {
        layout,
        flags: ModifierFlags::empty(),
    }));
    let second = ir.elem(ProgramElementKind::Modifiers(ModifiersDeclaration {
        layout,
        flags: ModifierFlags::empty(),
    }));
    let mut ctx = context(ir.pool, ProgramKind::Compute);
    do_finalization_checks(&mut ctx, &WriteCounts::default(), &[first, second]);
    assert_eq!(
        messages(&ctx.errors),
        ["'local_size_x' was specified more than once"]
    );
}

#[test]
fn finalization_requires_a_workgroup_size_in_compute_programs() {
    let mut ctx = context(IrPool::new(), ProgramKind::Compute);
    do_finalization_checks(&mut ctx, &WriteCounts::default(), &[]);
    assert_eq!(
        messages(&ctx.errors),
        ["compute programs must specify a workgroup size"]
    );
}

#[test]
fn finalization_reports_a_global_past_the_size_limit() {
    // Golden: errors/ProgramTooLarge_Globals.glsl, the variable 'extra_large'.
    let mut ir = Ir::new();
    let big = ir.array_type(TypeId::FLOAT, 100_000);
    let var = ir.var(
        "extra_large",
        big,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let decl = ir.var_decl(var, big, 0, None);
    let global = ir.elem(ProgramElementKind::GlobalVar(GlobalVarDeclaration {
        declaration: decl,
    }));
    let mut ctx = context(ir.pool, ProgramKind::RuntimeShader);
    do_finalization_checks(&mut ctx, &WriteCounts::default(), &[global]);
    assert_eq!(
        messages(&ctx.errors),
        ["global variable 'extra_large' exceeds the size limit"]
    );
}

#[test]
fn check_program_structure_reports_self_recursion() {
    // Golden: errors/IllegalRecursionSimple.glsl.
    let mut ir = Ir::new();
    let n = ir.param("n", TypeId::INT);
    let fib = ir.func("fibonacci", TypeId::INT, vec![n]);
    let arg = ir.read(n);
    let recursive = ir.call(fib, vec![arg]);
    let ret = ir.ret(Some(recursive));
    let def = ir.define(fib, vec![ret]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert!(check_program_structure(&mut ctx, &[def]));
    assert_eq!(
        messages(&ctx.errors),
        [
            "potential recursion (function call cycle) not allowed:\n\tint fibonacci(int n)\n\tint fibonacci(int n)"
        ]
    );
}

#[test]
fn check_program_structure_reports_mutual_recursion() {
    // Golden: errors/IllegalRecursionMutual.glsl.
    let mut ir = Ir::new();
    let n_odd = ir.param("n", TypeId::INT);
    let n_even = ir.param("n", TypeId::INT);
    let is_odd = ir.func("is_odd", TypeId::BOOL, vec![n_odd]);
    let is_even = ir.func("is_even", TypeId::BOOL, vec![n_even]);
    let arg_even = ir.read(n_odd);
    let call_even = ir.call(is_even, vec![arg_even]);
    let ret_odd = ir.ret(Some(call_even));
    let odd_def = ir.define(is_odd, vec![ret_odd]);
    let arg_odd = ir.read(n_even);
    let call_odd = ir.call(is_odd, vec![arg_odd]);
    let ret_even = ir.ret(Some(call_odd));
    let even_def = ir.define(is_even, vec![ret_even]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    check_program_structure(&mut ctx, &[odd_def, even_def]);
    assert_eq!(
        messages(&ctx.errors),
        [
            "potential recursion (function call cycle) not allowed:\n\tbool is_odd(int n)\n\tbool is_even(int n)\n\tbool is_odd(int n)"
        ]
    );
}

#[test]
fn unroll_info_counts_an_ascending_loop() {
    // for (int i = 0; i < 4; i++) {}
    let mut ir = Ir::new();
    let i = ir.local("i", TypeId::INT);
    let zero = ir.int(0);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(zero));
    let read = ir.read(i);
    let four = ir.int(4);
    let test = ir.binary(read, OperatorKind::Lt, four, TypeId::BOOL);
    let read_next = ir.read(i);
    let next = ir.postfix(read_next, OperatorKind::PlusPlus);
    let body = ir.block(vec![]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert_eq!(
        unroll(&mut ctx, init, test, next, body),
        Some(LoopUnrollInfo {
            index: i,
            start: 0.0,
            delta: 1.0,
            count: 4,
        })
    );
    assert_eq!(messages(&ctx.errors), Vec::<String>::new());
}

#[test]
fn unroll_info_counts_a_descending_loop() {
    // for (int i = 4; i > 0; i -= 1) {}: four iterations.
    let mut ir = Ir::new();
    let i = ir.local("i", TypeId::INT);
    let four = ir.int(4);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(four));
    let read = ir.read(i);
    let zero = ir.int(0);
    let test = ir.binary(read, OperatorKind::Gt, zero, TypeId::BOOL);
    let target = ir.read(i);
    let step = ir.int(1);
    let next = ir.binary(target, OperatorKind::MinusEq, step, TypeId::INT);
    let body = ir.block(vec![]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert_eq!(
        unroll(&mut ctx, init, test, next, body).map(|info| info.count),
        Some(4)
    );
}

#[test]
fn unroll_info_rewrites_a_float_inequality_test() {
    // for (float x = 0.0; x != 1.0; x += 0.25) {}: four iterations. The test becomes `x < 1.0`.
    let mut ir = Ir::new();
    let x = ir.local("x", TypeId::FLOAT);
    let start = ir.float(0.0);
    let init = ir.var_decl(x, TypeId::FLOAT, 0, Some(start));
    let read = ir.read(x);
    let end = ir.float(1.0);
    let test = ir.binary(read, OperatorKind::Neq, end, TypeId::BOOL);
    let target = ir.read(x);
    let step = ir.float(0.25);
    let next = ir.binary(target, OperatorKind::PlusEq, step, TypeId::FLOAT);
    let body = ir.block(vec![]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    let mut test_slot = Some(test);
    let info = get_loop_unroll_info(
        &mut ctx,
        Position::default(),
        &ForLoopPositions::default(),
        Some(init),
        &mut test_slot,
        Some(next),
        body,
        true,
    );
    assert_eq!(info.map(|info| info.count), Some(4));
    let rewritten = test_slot.expect("the test is kept");
    let ExpressionKind::Binary(b) = &ctx.pool.expression(rewritten).kind else {
        panic!("the rewritten test is a binary expression");
    };
    assert_eq!(b.operator.kind(), OperatorKind::Lt);
    assert_eq!(messages(&ctx.errors), Vec::<String>::new());
}

#[test]
fn unroll_info_reports_an_int_loop_that_overflows() {
    // Golden: errors/ForLoopOverflow.glsl.
    let mut ir = Ir::new();
    let i = ir.local("i", TypeId::INT);
    let start = ir.int(2_147_483_640);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(start));
    let read = ir.read(i);
    let end = ir.int(2_147_483_647);
    let test = ir.binary(read, OperatorKind::Lt, end, TypeId::BOOL);
    let target = ir.read(i);
    let step = ir.int(100);
    let next = ir.binary(target, OperatorKind::PlusEq, step, TypeId::INT);
    let body = ir.block(vec![]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert_eq!(unroll(&mut ctx, init, test, next, body), None);
    assert_eq!(
        messages(&ctx.errors),
        ["loop must guarantee termination in fewer iterations"]
    );
}

#[test]
fn unroll_info_reports_a_not_equal_loop_that_never_hits_its_end() {
    // Golden: errors/ForLoopNEQOverflow.glsl.
    let mut ir = Ir::new();
    let i = ir.local("i", TypeId::INT);
    let start = ir.int(2_000_000_000);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(start));
    let read = ir.read(i);
    let end = ir.int(-2_000_000_000);
    let test = ir.binary(read, OperatorKind::Neq, end, TypeId::BOOL);
    let target = ir.read(i);
    let step = ir.int(1_000_000_000);
    let next = ir.binary(target, OperatorKind::PlusEq, step, TypeId::INT);
    let body = ir.block(vec![]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert_eq!(unroll(&mut ctx, init, test, next, body), None);
    assert_eq!(
        messages(&ctx.errors),
        ["loop must guarantee termination in fewer iterations"]
    );
}

#[test]
fn unroll_info_rejects_a_body_that_writes_the_index() {
    // for (int i = 0; i < 4; i++) { i = 2; }
    let mut ir = Ir::new();
    let i = ir.local("i", TypeId::INT);
    let zero = ir.int(0);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(zero));
    let read = ir.read(i);
    let four = ir.int(4);
    let test = ir.binary(read, OperatorKind::Lt, four, TypeId::BOOL);
    let read_next = ir.read(i);
    let next = ir.postfix(read_next, OperatorKind::PlusPlus);
    let write = ir.vref(i, VariableRefKind::Write);
    let two = ir.int(2);
    let assign = ir.binary(write, OperatorKind::Eq, two, TypeId::INT);
    let assign_stmt = ir.expr_stmt(assign);
    let body = ir.block(vec![assign_stmt]);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    assert_eq!(unroll(&mut ctx, init, test, next, body), None);
    assert_eq!(
        messages(&ctx.errors),
        ["loop index must not be modified within body of the loop"]
    );
}

#[test]
fn loop_control_flow_finds_exits_that_affect_the_loop() {
    let mut ir = Ir::new();
    let c = ir.param("c", TypeId::BOOL);
    // { if (c) break; } : the break affects this loop.
    let test = ir.read(c);
    let brk = ir.stmt(StatementKind::Break(BreakStatement));
    let if_break = ir.if_stmt(test, brk, None);
    let direct = ir.block(vec![if_break]);
    assert_eq!(
        get_loop_control_flow_info(&ir.pool, direct),
        LoopControlFlowInfo {
            has_break: true,
            ..LoopControlFlowInfo::default()
        }
    );
    // { do { break; } while (c); continue; } : the break is inside a nested loop, so only the
    // continue affects this loop.
    let inner_break = ir.stmt(StatementKind::Break(BreakStatement));
    let inner_test = ir.read(c);
    let inner = ir.stmt(StatementKind::Do(DoStatement {
        statement: inner_break,
        test: inner_test,
    }));
    let cont = ir.stmt(StatementKind::Continue(ContinueStatement));
    let nested = ir.block(vec![inner, cont]);
    assert_eq!(
        get_loop_control_flow_info(&ir.pool, nested),
        LoopControlFlowInfo {
            has_continue: true,
            ..LoopControlFlowInfo::default()
        }
    );
}

#[test]
fn can_exit_finds_paths_that_skip_the_return() {
    // Golden: errors/CanExitWithoutReturningValue.glsl.
    let mut ir = Ir::new();
    let variable = ir.param("variable", TypeId::INT);
    let zero = ir.int(0);
    let one = ir.int(1);

    // int if_only() { if (variable == 1) return 0; }
    let t1 = ir.read(variable);
    let test1 = ir.binary(t1, OperatorKind::EqEq, one, TypeId::BOOL);
    let r1 = ir.ret(Some(zero));
    let if_only = ir.if_stmt(test1, r1, None);
    let body_if_only = ir.block(vec![if_only]);

    // int both() { if (variable == 1) return 0; else return 1; }
    let t2 = ir.read(variable);
    let test2 = ir.binary(t2, OperatorKind::EqEq, one, TypeId::BOOL);
    let r2a = ir.ret(Some(zero));
    let r2b = ir.ret(Some(one));
    let both = ir.if_stmt(test2, r2a, Some(r2b));
    let body_both = ir.block(vec![both]);

    // int for_with_conditional_return() { for (;;) { if (variable == 1) return 0; } }
    let t3 = ir.read(variable);
    let test3 = ir.binary(t3, OperatorKind::EqEq, one, TypeId::BOOL);
    let r3 = ir.ret(Some(zero));
    let if3 = ir.if_stmt(test3, r3, None);
    let loop_body3 = ir.block(vec![if3]);
    let loop3 = ir.for_stmt(None, None, None, loop_body3);
    let body_for_return = ir.block(vec![loop3]);

    // int for_with_conditional_break() { for (;;) { if (variable == 1) break; return 0; } }
    let t4 = ir.read(variable);
    let test4 = ir.binary(t4, OperatorKind::EqEq, one, TypeId::BOOL);
    let brk = ir.stmt(StatementKind::Break(BreakStatement));
    let if4 = ir.if_stmt(test4, brk, None);
    let r4 = ir.ret(Some(zero));
    let loop_body4 = ir.block(vec![if4, r4]);
    let loop4 = ir.for_stmt(None, None, None, loop_body4);
    let body_for_break = ir.block(vec![loop4]);

    let f = ir.func("f", TypeId::INT, vec![]);
    let void_f = ir.func("g", TypeId::VOID, vec![]);
    let pool = &ir.pool;
    assert!(can_exit_without_returning_value(pool, f, body_if_only));
    assert!(!can_exit_without_returning_value(pool, f, body_both));
    assert!(can_exit_without_returning_value(pool, f, body_for_return));
    assert!(can_exit_without_returning_value(pool, f, body_for_break));
    // A void function never exits without a value.
    assert!(!can_exit_without_returning_value(
        pool,
        void_f,
        body_if_only
    ));
}

#[test]
fn return_complexity_classifies_the_returns() {
    let mut ir = Ir::new();
    let c = ir.param("c", TypeId::BOOL);

    // int single() { return 1; }
    let f1 = ir.func("single", TypeId::INT, vec![]);
    let one = ir.int(1);
    let ret1 = ir.ret(Some(one));
    let single = ir.define(f1, vec![ret1]);

    // int early() { if (c) return 1; return 2; }: a return before the end of the control flow.
    let f2 = ir.func("early", TypeId::INT, vec![]);
    let test2 = ir.read(c);
    let one2 = ir.int(1);
    let early_return = ir.ret(Some(one2));
    let if2 = ir.if_stmt(test2, early_return, None);
    let two = ir.int(2);
    let final_return = ir.ret(Some(two));
    let early = ir.define(f2, vec![if2, final_return]);

    // int scoped() { if (c) return 1; else return 2; }: two returns, both at the end.
    let f3 = ir.func("scoped", TypeId::INT, vec![]);
    let test3 = ir.read(c);
    let one3 = ir.int(1);
    let scoped_then = ir.ret(Some(one3));
    let two3 = ir.int(2);
    let scoped_else = ir.ret(Some(two3));
    let if3 = ir.if_stmt(test3, scoped_then, Some(scoped_else));
    let scoped = ir.define(f3, vec![if3]);

    let single_def = ir.function_def(single);
    let early_def = ir.function_def(early);
    let scoped_def = ir.function_def(scoped);
    assert_eq!(
        get_return_complexity(&ir.pool, &single_def),
        ReturnComplexity::SingleSafeReturn
    );
    assert_eq!(
        get_return_complexity(&ir.pool, &early_def),
        ReturnComplexity::EarlyReturns
    );
    assert_eq!(
        get_return_complexity(&ir.pool, &scoped_def),
        ReturnComplexity::ScopedReturns
    );
}

#[test]
fn switch_exits_are_unconditional_or_conditional() {
    let mut ir = Ir::new();
    let c = ir.param("c", TypeId::BOOL);
    // case 1: return 0;
    let zero = ir.int(0);
    let ret = ir.ret(Some(zero));
    let always = ir.block(vec![ret]);
    assert!(switch_case_contains_unconditional_exit(&ir.pool, always));
    assert!(!switch_case_contains_conditional_exit(&ir.pool, always));
    // case 1: if (c) break;
    let test = ir.read(c);
    let brk = ir.stmt(StatementKind::Break(BreakStatement));
    let maybe = ir.if_stmt(test, brk, None);
    assert!(!switch_case_contains_unconditional_exit(&ir.pool, maybe));
    assert!(switch_case_contains_conditional_exit(&ir.pool, maybe));
    // A switch inside the case absorbs its own breaks.
    let value = ir.read(c);
    let inner_brk = ir.stmt(StatementKind::Break(BreakStatement));
    let inner_case = ir.stmt(StatementKind::SwitchCase(SwitchCase {
        is_default: true,
        value: 0,
        statement: inner_brk,
    }));
    let cases = ir.block(vec![inner_case]);
    let nested = ir.stmt(StatementKind::Switch(SwitchStatement {
        value,
        case_block: cases,
    }));
    assert!(!switch_case_contains_conditional_exit(&ir.pool, nested));
}

#[test]
fn symbol_table_stack_pushes_the_scope_a_block_opens() {
    let mut ir = Ir::new();
    let root = ir.pool.add_symbol_table(SymbolTable::new(None, false));
    let scope = ir
        .pool
        .add_symbol_table(SymbolTable::new(Some(root), false));
    let block = ir.scope(vec![], Some(scope));
    let mut stack = vec![root];
    let builder = SymbolTableStackBuilder::new(&ir.pool, Some(block), &mut stack);
    assert_eq!(stack, [root, scope]);
    builder.finish(&mut stack);
    assert_eq!(stack, [root]);
}

#[test]
fn symbol_table_check_reports_a_variable_outside_its_scope() {
    // A variable declared in a block whose table does not hold it.
    let mut ir = Ir::new();
    let root = ir.pool.add_symbol_table(SymbolTable::new(None, false));
    let scope = ir
        .pool
        .add_symbol_table(SymbolTable::new(Some(root), false));
    let x = ir.local("x", TypeId::INT);
    let decl = ir.var_decl(x, TypeId::INT, 0, None);
    let body = ir.scope(vec![decl], Some(scope));
    let f = ir.func("f", TypeId::VOID, vec![]);
    let def = ir.define_body(f, body);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    check_symbol_table_correctness(&mut ctx, root, &[def]);
    assert_eq!(
        messages(&ctx.errors),
        ["internal error (variable 'x' is incorrectly scoped)"]
    );

    // The same declaration is fine once its table holds the variable.
    let mut ir = Ir::new();
    let root = ir.pool.add_symbol_table(SymbolTable::new(None, false));
    let scope = ir
        .pool
        .add_symbol_table(SymbolTable::new(Some(root), false));
    let x = ir.local("x", TypeId::INT);
    let decl = ir.var_decl(x, TypeId::INT, 0, None);
    ir.pool
        .symbol_table_mut(scope)
        .set("x", SymbolId::Variable(x));
    let body = ir.scope(vec![decl], Some(scope));
    let f = ir.func("f", TypeId::VOID, vec![]);
    let def = ir.define_body(f, body);
    let mut ctx = context(ir.pool, ProgramKind::Fragment);
    check_symbol_table_correctness(&mut ctx, root, &[def]);
    assert_eq!(messages(&ctx.errors), Vec::<String>::new());
}

#[test]
fn es2_indexing_accepts_loop_indices_and_rejects_variables() {
    let mut ir = Ir::new();
    let arr_ty = ir.array_type(TypeId::INT, 4);
    let a = ir.local("a", arr_ty);
    let x = ir.local("x", TypeId::INT);
    // void good() { for (int i = 0; i < 4; i++) a[i]; }
    let i = ir.local("i", TypeId::INT);
    let zero = ir.int(0);
    let init = ir.var_decl(i, TypeId::INT, 0, Some(zero));
    let read_i = ir.read(i);
    let four = ir.int(4);
    let test = ir.binary(read_i, OperatorKind::Lt, four, TypeId::BOOL);
    let read_i_next = ir.read(i);
    let next = ir.postfix(read_i_next, OperatorKind::PlusPlus);
    let base = ir.read(a);
    let index_i = ir.read(i);
    let access_i = ir.index(base, index_i);
    let access_stmt = ir.expr_stmt(access_i);
    let loop_body = ir.block(vec![access_stmt]);
    let loop_stmt = ir.for_stmt(Some(init), Some(test), Some(next), loop_body);
    let good_f = ir.func("good", TypeId::VOID, vec![]);
    let good = ir.define(good_f, vec![loop_stmt]);

    // void bad() { a[x]; } where `x` is an ordinary local.
    let base2 = ir.read(a);
    let index_x = ir.read(x);
    let access_x = ir.index(base2, index_x);
    let bad_stmt = ir.expr_stmt(access_x);
    let bad_f = ir.func("bad", TypeId::VOID, vec![]);
    let bad = ir.define(bad_f, vec![bad_stmt]);

    let mut ok_errors = ErrorReporter::forwarding();
    validate_indexing_for_es2(&ir.pool, good, &mut ok_errors);
    assert_eq!(messages(&ok_errors), Vec::<String>::new());

    let mut bad_errors = ErrorReporter::forwarding();
    validate_indexing_for_es2(&ir.pool, bad, &mut bad_errors);
    assert_eq!(messages(&bad_errors), ["index expression must be constant"]);
}

#[test]
fn var_declaration_without_scope_is_reported() {
    // Golden: errors/UnscopedVariableInIf.glsl, the variable 's'.
    let mut ir = Ir::new();
    let s = ir.local("s", TypeId::INT);
    let lone = ir.var_decl(s, TypeId::INT, 0, None);
    let mut errors = ErrorReporter::forwarding();
    assert!(detect_var_declaration_without_scope(
        &ir.pool,
        lone,
        Some(&mut errors)
    ));
    assert_eq!(
        messages(&errors),
        ["variable 's' must be created in a scope"]
    );

    // An unscoped block that starts with a declaration is the same problem.
    let t = ir.local("t", TypeId::INT);
    let decl_t = ir.var_decl(t, TypeId::INT, 0, None);
    let unscoped = ir.stmt(StatementKind::Block(Block {
        children: vec![decl_t],
        block_kind: BlockKind::UnbracedBlock,
        symbol_table: None,
    }));
    assert!(detect_var_declaration_without_scope(
        &ir.pool, unscoped, None
    ));

    // A scoped block is fine.
    let scoped = ir.block(vec![decl_t]);
    assert!(!detect_var_declaration_without_scope(
        &ir.pool, scoped, None
    ));
}

#[test]
fn specialization_records_a_global_argument() {
    // float f(float s) { return s; } void main() { f(u); }, where `u` is a uniform.
    let mut ir = Ir::new();
    let u = ir.var(
        "u",
        TypeId::FLOAT,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let s = ir.param("s", TypeId::FLOAT);
    let f = ir.func("f", TypeId::FLOAT, vec![s]);
    let read_s = ir.read(s);
    let ret_s = ir.ret(Some(read_s));
    let f_def = ir.define(f, vec![ret_s]);

    let main = ir.func("main", TypeId::VOID, vec![]);
    let read_u = ir.read(u);
    let call = ir.call(f, vec![read_u]);
    let call_stmt = ir.expr_stmt(call);
    let main_def = ir.define(main, vec![call_stmt]);

    let matches = |_: &Variable| true;
    let mut info = SpecializationInfo::default();
    find_functions_to_specialize(&ir.pool, &[f_def, main_def], &mut info, &matches);

    let ExpressionKind::FunctionCall(payload) = &ir.pool.expression(call).kind else {
        panic!("expected a call");
    };
    assert_eq!(
        find_specialization_index_for_call(payload, &info, UNSPECIALIZED),
        0
    );
    assert_eq!(
        find_specialized_parameters_for_function(&ir.pool, f, &info),
        [true]
    );

    let mut mappings = Vec::new();
    get_parameter_mappings_for_function(&ir.pool, f, &info, 0, |index, param, expr| {
        mappings.push((index, param, expr));
    });
    assert_eq!(mappings, [(0, s, read_u)]);

    // The generic copy maps no parameters.
    let mut generic = Vec::new();
    get_parameter_mappings_for_function(&ir.pool, f, &info, UNSPECIALIZED, |i, p, e| {
        generic.push((i, p, e));
    });
    assert_eq!(generic, Vec::new());
}
