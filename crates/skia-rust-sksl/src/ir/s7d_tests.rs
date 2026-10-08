// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the statement and declaration factories of task S7d (`docs/design/sksl.md` §10): the
//! checks and error texts of `Convert`, and the shapes `Make` builds. The IR is built by hand,
//! because the parser is not ported yet. The messages are Skia's, as `tests/sksl/errors` has them.

use super::*;
use crate::context::Context;
use crate::defines::SkslInt;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::mangler::Mangler;
use crate::modules::ModuleType;
use crate::position::{ForLoopPositions, Position};
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

fn context(kind: ProgramKind) -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        kind,
        ProgramSettings::default(),
    ));
    ctx
}

/// The messages reported so far, in order.
fn messages(ctx: &Context) -> Vec<String> {
    match ctx.errors.sink() {
        ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
        other => panic!("expected a forwarding reporter, found {other:?}"),
    }
}

fn at() -> Position {
    Position::default()
}

fn symbol_table(ctx: &mut Context) -> SymTabId {
    let table = ctx.pool.add_symbol_table(SymbolTable::new(None, false));
    ctx.symbol_table = Some(table);
    table
}

fn bool_lit(ctx: &mut Context, value: bool) -> ExprId {
    Literal::make_bool(&mut ctx.pool, at(), value, TypeId::BOOL)
}

fn int_lit(ctx: &mut Context, value: SkslInt) -> ExprId {
    Literal::make_int(&mut ctx.pool, at(), value, TypeId::INT)
}

fn modifiers(flags: ModifierFlags) -> Modifiers {
    Modifiers {
        position: at(),
        layout: Layout::new(),
        flags,
    }
}

/// A local declaration `ty name = value;`, or `None` when the declaration is rejected.
fn local_decl(
    ctx: &mut Context,
    name: &str,
    ty: TypeId,
    flags: ModifierFlags,
    value: Option<ExprId>,
) -> Option<StmtId> {
    VarDeclaration::convert(
        ctx,
        at(),
        &modifiers(flags),
        ty,
        name,
        VariableStorage::Local,
        value,
    )
}

fn global_decl(ctx: &mut Context, name: &str, ty: TypeId) -> Option<StmtId> {
    VarDeclaration::convert(
        ctx,
        at(),
        &modifiers(ModifierFlags::empty()),
        ty,
        name,
        VariableStorage::Global,
        None,
    )
}

fn var_of(ctx: &Context, stmt: StmtId) -> VarId {
    ctx.pool
        .statement(stmt)
        .as_var_declaration()
        .expect("a variable declaration")
        .var
}

fn is_nop(ctx: &Context, stmt: StmtId) -> bool {
    matches!(ctx.pool.statement(stmt).kind, StatementKind::Nop(_))
}

fn block_children(ctx: &Context, stmt: StmtId) -> Vec<StmtId> {
    match &ctx.pool.statement(stmt).kind {
        StatementKind::Block(block) => block.children.clone(),
        other => panic!("expected a block, found {other:?}"),
    }
}

#[test]
fn unscoped_declaration_in_if_is_reported() {
    // Golden: errors/UnscopedVariableInIf.glsl
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let decl = local_decl(&mut ctx, "x", TypeId::HALF4, ModifierFlags::empty(), None)
        .expect("a local declaration is valid");
    let test = bool_lit(&mut ctx, true);
    assert_eq!(IfStatement::convert(&mut ctx, at(), test, decl, None), None);
    assert_eq!(messages(&ctx), ["variable 'x' must be created in a scope"]);
}

#[test]
fn duplicate_case_value_is_reported() {
    // Golden: errors/SwitchDuplicateCase.glsl
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let value = int_lit(&mut ctx, 0);
    let first = int_lit(&mut ctx, 0);
    let second = int_lit(&mut ctx, 0);
    let body_a = Nop::make(&mut ctx.pool);
    let body_b = Nop::make(&mut ctx.pool);
    let table = ctx.symbol_table.expect("a table");
    let result = SwitchStatement::convert(
        &mut ctx,
        at(),
        value,
        vec![Some(first), Some(second)],
        vec![body_a, body_b],
        table,
    );
    assert_eq!(result, None);
    assert_eq!(messages(&ctx), ["duplicate case value '0'"]);
}

#[test]
fn duplicate_default_case_is_reported() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let value = int_lit(&mut ctx, 0);
    let body_a = Nop::make(&mut ctx.pool);
    let body_b = Nop::make(&mut ctx.pool);
    let result = SwitchStatement::convert(
        &mut ctx,
        at(),
        value,
        vec![None, None],
        vec![body_a, body_b],
        table,
    );
    assert_eq!(result, None);
    assert_eq!(messages(&ctx), ["duplicate default case"]);
}

#[test]
fn non_constant_const_initializer_is_reported() {
    // Golden: errors/BadConstInitializers.glsl (`float u; const float x = u;`)
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let u = local_decl(&mut ctx, "u", TypeId::FLOAT, ModifierFlags::empty(), None)
        .expect("a local declaration is valid");
    let u_var = var_of(&ctx, u);
    let init = VariableReference::make(&mut ctx.pool, at(), u_var, VariableRefKind::Read);
    let result = local_decl(
        &mut ctx,
        "x",
        TypeId::FLOAT,
        ModifierFlags::CONST,
        Some(init),
    );
    assert_eq!(result, None);
    assert_eq!(
        messages(&ctx),
        ["'const' variable initializer must be a constant expression"]
    );
}

#[test]
fn rt_adjust_must_be_float4() {
    // Golden: errors/RTAdjustType.glsl
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    assert_eq!(global_decl(&mut ctx, "sk_RTAdjust", TypeId::FLOAT3), None);
    assert_eq!(messages(&ctx), ["sk_RTAdjust must have type 'float4'"]);
}

#[test]
fn duplicate_global_symbol_is_reported() {
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    assert!(global_decl(&mut ctx, "g", TypeId::FLOAT).is_some());
    assert_eq!(global_decl(&mut ctx, "g", TypeId::FLOAT), None);
    assert_eq!(messages(&ctx), ["symbol 'g' was already defined"]);
}

#[test]
fn global_declaration_records_its_element() {
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let stmt = global_decl(&mut ctx, "g", TypeId::FLOAT).expect("a global is valid");
    let var = var_of(&ctx, stmt);
    let element = GlobalVarDeclaration::make(&mut ctx.pool, stmt);
    assert_eq!(
        ctx.pool.variable(var).global_var_declaration(),
        Some(element)
    );
    assert_eq!(ctx.pool.statement(stmt).position, at());
}

#[test]
fn discard_is_only_permitted_in_fragment_shaders() {
    // Golden: errors/Discard*.glsl (`discard statement is only permitted in fragment shaders`)
    let mut ctx = context(ProgramKind::Vertex);
    assert_eq!(DiscardStatement::convert(&mut ctx, at()), None);
    assert_eq!(
        messages(&ctx),
        ["discard statement is only permitted in fragment shaders"]
    );

    let mut fragment = context(ProgramKind::Fragment);
    let discard = DiscardStatement::convert(&mut fragment, at()).expect("fragment discard");
    assert_eq!(
        fragment.pool.statement(discard).kind,
        StatementKind::Discard(DiscardStatement)
    );
}

#[test]
fn while_and_do_loops_are_rejected_in_strict_es2() {
    // A runtime shader in the default settings is strict ES2.
    let mut ctx = context(ProgramKind::RuntimeShader);
    assert!(ctx.config().strict_es2_mode());
    let test = bool_lit(&mut ctx, true);
    let body = Nop::make(&mut ctx.pool);
    assert_eq!(
        ForStatement::convert_while(&mut ctx, at(), test, body),
        None
    );
    let test = bool_lit(&mut ctx, true);
    assert_eq!(DoStatement::convert(&mut ctx, at(), body, test), None);
    assert_eq!(
        messages(&ctx),
        [
            "while loops are not supported",
            "do-while loops are not supported"
        ]
    );
}

#[test]
fn for_initializer_must_be_simple() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let initializer = BreakStatement::make(&mut ctx.pool, at());
    let body = Nop::make(&mut ctx.pool);
    let result = ForStatement::convert(
        &mut ctx,
        at(),
        ForLoopPositions::default(),
        Some(initializer),
        None,
        None,
        body,
        Some(table),
    );
    assert_eq!(result, None);
    assert_eq!(messages(&ctx), ["invalid for loop initializer"]);
}

#[test]
fn block_simplifies_as_skia_does() {
    let mut ctx = context(ProgramKind::Fragment);
    // No statements: a Nop.
    let empty = Block::make(&mut ctx.pool, at(), vec![], BlockKind::UnbracedBlock, None);
    assert!(is_nop(&ctx, empty));
    // One non-empty statement among empties: that statement.
    let brk = BreakStatement::make(&mut ctx.pool, at());
    let nop_a = Nop::make(&mut ctx.pool);
    let nop_b = Nop::make(&mut ctx.pool);
    let single = Block::make(
        &mut ctx.pool,
        at(),
        vec![nop_a, brk, nop_b],
        BlockKind::UnbracedBlock,
        None,
    );
    assert_eq!(single, brk);
    // Two non-empty statements: a block of both.
    let cont = ContinueStatement::make(&mut ctx.pool, at());
    let pair = Block::make(
        &mut ctx.pool,
        at(),
        vec![brk, cont],
        BlockKind::UnbracedBlock,
        None,
    );
    assert_eq!(block_children(&ctx, pair), [brk, cont]);
    // A braced scope is always kept.
    let scope = Block::make(&mut ctx.pool, at(), vec![brk], BlockKind::BracedScope, None);
    assert_eq!(block_children(&ctx, scope), [brk]);
}

#[test]
fn compound_statement_appends_to_an_existing_compound() {
    let mut ctx = context(ProgramKind::Fragment);
    let brk = BreakStatement::make(&mut ctx.pool, at());
    let cont = ContinueStatement::make(&mut ctx.pool, at());
    let discard = DiscardStatement::make(&mut ctx, at());
    let pair = Block::make_compound_statement(&mut ctx.pool, Some(brk), Some(cont))
        .expect("two statements make a block");
    let grown = Block::make_compound_statement(&mut ctx.pool, Some(pair), Some(discard));
    assert_eq!(grown, Some(pair));
    assert_eq!(block_children(&ctx, pair), [brk, cont, discard]);
    // A missing statement is dropped.
    assert_eq!(
        Block::make_compound_statement(&mut ctx.pool, None, Some(brk)),
        Some(brk)
    );
    assert_eq!(
        Block::make_compound_statement(&mut ctx.pool, Some(brk), None),
        Some(brk)
    );
}

#[test]
fn if_with_constant_test_keeps_one_branch() {
    let mut ctx = context(ProgramKind::Fragment);
    let yes = BreakStatement::make(&mut ctx.pool, at());
    let no = ContinueStatement::make(&mut ctx.pool, at());
    let test = bool_lit(&mut ctx, true);
    assert_eq!(IfStatement::make(&mut ctx, at(), test, yes, Some(no)), yes);
    let test = bool_lit(&mut ctx, false);
    assert_eq!(IfStatement::make(&mut ctx, at(), test, yes, Some(no)), no);
}

#[test]
fn if_with_two_empty_branches_becomes_its_test() {
    let mut ctx = context(ProgramKind::Fragment);
    let test = bool_lit(&mut ctx, true);
    let empty_true = Nop::make(&mut ctx.pool);
    let stmt = IfStatement::make(&mut ctx, at(), test, empty_true, None);
    assert!(is_nop(&ctx, stmt));
}

#[test]
fn unrollable_loop_that_never_runs_is_a_nop() {
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let index = local_decl(&mut ctx, "i", TypeId::INT, ModifierFlags::empty(), None)
        .map(|stmt| var_of(&ctx, stmt))
        .expect("a local declaration is valid");
    let body = BreakStatement::make(&mut ctx.pool, at());
    let info = LoopUnrollInfo {
        index,
        start: 0.0,
        delta: 1.0,
        count: 0,
    };
    let stmt = ForStatement::make(
        &mut ctx.pool,
        at(),
        ForLoopPositions::default(),
        None,
        None,
        None,
        body,
        Some(info),
        None,
    );
    assert!(is_nop(&ctx, stmt));
}

#[test]
fn static_switch_keeps_only_the_matching_case() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    // case 0: continue; case 1: break; default: nop
    let body0 = ContinueStatement::make(&mut ctx.pool, at());
    let body1 = BreakStatement::make(&mut ctx.pool, at());
    let body_default = Nop::make(&mut ctx.pool);
    let case0 = SwitchCase::make(&mut ctx.pool, at(), 0, body0);
    let case1 = SwitchCase::make(&mut ctx.pool, at(), 1, body1);
    let case_default = SwitchCase::make_default(&mut ctx.pool, at(), body_default);
    let case_block = Block::make_block(
        &mut ctx.pool,
        at(),
        vec![case0, case1, case_default],
        BlockKind::BracedScope,
        Some(table),
    );
    let value = int_lit(&mut ctx, 1);
    let stmt = SwitchStatement::make(&mut ctx, at(), value, case_block);
    // The switch becomes its matching case block, which falls out at the `break`. The `break`
    // is replaced by a `nop`.
    assert_eq!(stmt, case_block);
    let children = block_children(&ctx, case_block);
    assert_eq!(children.len(), 1);
    assert!(is_nop(&ctx, children[0]));
    assert_eq!(children[0], body1);
}

#[test]
fn static_switch_without_a_match_or_default_is_removed() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let body0 = BreakStatement::make(&mut ctx.pool, at());
    let case0 = SwitchCase::make(&mut ctx.pool, at(), 0, body0);
    let case_block = Block::make_block(
        &mut ctx.pool,
        at(),
        vec![case0],
        BlockKind::BracedScope,
        Some(table),
    );
    let value = int_lit(&mut ctx, 5);
    let stmt = SwitchStatement::make(&mut ctx, at(), value, case_block);
    assert_eq!(stmt, case_block);
    assert_eq!(block_children(&ctx, case_block), Vec::<StmtId>::new());
}

#[test]
fn top_level_switch_declarations_are_hoisted() {
    // A declaration with an initial value becomes an assignment in its case, and the declaration
    // moves into a scoped block around the switch, with its symbol.
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let initial = int_lit(&mut ctx, 3);
    let decl = local_decl(
        &mut ctx,
        "y",
        TypeId::INT,
        ModifierFlags::empty(),
        Some(initial),
    )
    .expect("a local declaration is valid");
    let case_body = decl;
    let case_value = int_lit(&mut ctx, 0);
    let switch_value = int_lit(&mut ctx, 0);
    let result = SwitchStatement::convert(
        &mut ctx,
        at(),
        switch_value,
        vec![Some(case_value)],
        vec![case_body],
        table,
    )
    .expect("the switch is valid");
    // The result is the scope that holds the hoisted declaration, then the switch.
    let StatementKind::Block(scope_block) = &ctx.pool.statement(result).kind else {
        panic!("the hoisted scope is a block");
    };
    let children = scope_block.children.clone();
    let scope_table = scope_block.symbol_table.expect("the scope has symbols");
    assert_eq!(children.len(), 2, "the declaration, then the switch");
    let hoisted_decl = ctx
        .pool
        .statement(children[0])
        .as_var_declaration()
        .expect("the declaration moved into the scope");
    assert!(hoisted_decl.value.is_none(), "the initial value moved out");
    assert!(ctx.pool.symbol_table(scope_table).find_local("y").is_some());
    assert!(ctx.pool.symbol_table(table).find_local("y").is_none());
    // The value 0 matches the only case, so the case block keeps the assignment `y = 3;`.
    let cases = block_children(&ctx, children[1]);
    assert_eq!(cases.len(), 1);
    assert!(matches!(
        ctx.pool.statement(cases[0]).kind,
        StatementKind::Expression(_)
    ));
}

#[test]
fn extension_directives_are_checked() {
    let mut ctx = context(ProgramKind::Fragment);
    assert_eq!(Extension::convert(&mut ctx, at(), "GL_X", "disable"), None);
    assert_eq!(messages(&ctx), Vec::<String>::new());
    assert_eq!(Extension::convert(&mut ctx, at(), "GL_X", "bogus"), None);
    assert_eq!(
        messages(&ctx),
        ["expected 'require', 'enable', 'warn', or 'disable'"]
    );
    let elem = Extension::convert(&mut ctx, at(), "GL_X", "require").expect("an extension");
    assert_eq!(
        ctx.pool.element(elem).description(&ctx.pool),
        "#extension GL_X : enable"
    );

    let mut runtime = context(ProgramKind::RuntimeShader);
    assert_eq!(
        Extension::convert(&mut runtime, at(), "GL_X", "require"),
        None
    );
    assert_eq!(messages(&runtime), ["unsupported directive '#extension'"]);
}

#[test]
fn local_size_qualifiers_are_checked() {
    // Golden: errors/InvalidLocalSizeQualifier.glsl
    let mut ctx = context(ProgramKind::Compute);
    let mut zero = modifiers(ModifierFlags::IN);
    zero.layout.local_size_x = 0;
    assert_eq!(ModifiersDeclaration::convert(&mut ctx, &zero), None);
    let mut out = modifiers(ModifierFlags::OUT);
    out.layout.local_size_x = 16;
    assert_eq!(ModifiersDeclaration::convert(&mut ctx, &out), None);
    assert_eq!(
        messages(&ctx),
        [
            "local size qualifiers cannot be zero",
            "local size layout qualifiers must be defined using an 'in' declaration",
        ]
    );
    let mut good = modifiers(ModifierFlags::IN);
    good.layout.local_size_x = 16;
    assert!(ModifiersDeclaration::convert(&mut ctx, &good).is_some());
}

#[test]
fn interface_blocks_are_only_in_some_programs() {
    let mut ctx = context(ProgramKind::RuntimeShader);
    symbol_table(&mut ctx);
    let result = InterfaceBlock::convert(
        &mut ctx,
        at(),
        &modifiers(ModifierFlags::UNIFORM),
        "B",
        vec![],
        "b",
        0,
    );
    assert_eq!(result, None);
    assert_eq!(
        messages(&ctx),
        ["interface blocks are not allowed in this kind of program"]
    );
}

#[test]
fn scratch_variable_is_named_and_declared() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let mut mangler = Mangler::new();
    let scratch = Variable::make_scratch_variable(
        &mut ctx,
        &mut mangler,
        "tmp",
        TypeId::FLOAT2X2,
        table,
        None,
    );
    assert_eq!(&*ctx.pool.variable(scratch.var).name, "_0_tmp");
    assert!(ctx.pool.symbol_table(table).find_local("_0_tmp").is_some());
    let decl = ctx
        .pool
        .statement(scratch.decl)
        .as_var_declaration()
        .expect("the scratch declaration");
    assert_eq!(decl.base_type, TypeId::FLOAT2X2);
    assert_eq!(decl.array_size, 0);
    assert_eq!(decl.var, scratch.var);
}

#[test]
fn scratch_array_variable_declares_its_element_type() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let array = add_array_dimension(&mut ctx, table, TypeId::FLOAT, 2);
    let mut mangler = Mangler::new();
    let scratch = Variable::make_scratch_variable(&mut ctx, &mut mangler, "a", array, table, None);
    assert_eq!(ctx.pool.variable(scratch.var).ty, array);
    let decl = ctx
        .pool
        .statement(scratch.decl)
        .as_var_declaration()
        .expect("the scratch declaration");
    assert_eq!(decl.base_type, TypeId::FLOAT);
    assert_eq!(decl.array_size, 2);
}

#[test]
fn struct_definition_is_added_to_the_table() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let elem = StructDefinition::convert(&mut ctx, at(), "S", vec![]);
    assert!(ctx.pool.symbol_table(table).find_local("S").is_some());
    assert_eq!(
        ctx.pool.element(elem).description(&ctx.pool),
        "struct S { };"
    );
}

#[test]
fn expression_statement_convert_wraps_a_complete_expression() {
    let mut ctx = context(ProgramKind::Fragment);
    let lit = int_lit(&mut ctx, 1);
    let stmt = ExpressionStatement::convert(&mut ctx, lit).expect("a complete expression");
    // A literal has no side effects, so the optimizer replaces it with a Nop.
    assert!(is_nop(&ctx, stmt));
}
