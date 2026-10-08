// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the operator expressions of task S7b (`docs/design/sksl.md` §10): the checks and error
//! texts of `Convert`, and the shapes `Make` builds. The IR is built by hand, because the parser is
//! not ported yet. The messages are Skia's, as `tests/sksl/errors` has them.

use super::*;
use crate::compiler::Compiler;
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::modules::ModuleType;
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};
use crate::util::ShaderCapsFactory;

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

fn op(kind: OperatorKind) -> Operator {
    Operator::from(kind)
}

fn at() -> Position {
    Position::default()
}

/// A swizzle mask's position in some source text. `Swizzle::convert` reads its offsets.
fn mask() -> Position {
    Position::range(10, 20)
}

fn expr(ctx: &mut Context, ty: TypeId, kind: ExpressionKind) -> ExprId {
    ctx.pool.add_expression(Expression::new(at(), ty, kind))
}

fn lit(ctx: &mut Context, ty: TypeId, value: f64) -> ExprId {
    expr(ctx, ty, ExpressionKind::Literal(Literal { value }))
}

fn var(ctx: &mut Context, name: &str, ty: TypeId, flags: ModifierFlags) -> VarId {
    ctx.pool.add_variable(Variable::new(
        at(),
        at(),
        flags,
        name,
        ty,
        false,
        VariableStorage::Local,
    ))
}

fn var_ref(ctx: &mut Context, variable: VarId, ref_kind: VariableRefKind) -> ExprId {
    VariableReference::make(&mut ctx.pool, at(), variable, ref_kind)
}

fn symbol_table(ctx: &mut Context) -> SymTabId {
    let table = ctx.pool.add_symbol_table(SymbolTable::new(None, false));
    ctx.symbol_table = Some(table);
    table
}

/// The kind of the expression `id`, cloned.
fn kind(ctx: &Context, id: ExprId) -> ExpressionKind {
    ctx.pool.expression(id).kind.clone()
}

#[test]
fn binary_type_mismatch_matches_golden() {
    // tests/sksl/errors/BinaryTypeMismatch.glsl: `3 * true`.
    let mut ctx = context(ProgramKind::Fragment);
    let three = lit(&mut ctx, TypeId::INT_LITERAL, 3.0);
    let t = lit(&mut ctx, TypeId::BOOL, 1.0);
    let result = BinaryExpression::convert(&mut ctx, at(), three, op(OperatorKind::Star), t);
    assert_eq!(result, None);
    assert_eq!(
        messages(&ctx),
        ["type mismatch: '*' cannot operate on 'int', 'bool'"]
    );
}

#[test]
fn binary_logical_mismatch_matches_golden() {
    // `1 || 2.0`.
    let mut ctx = context(ProgramKind::Fragment);
    let one = lit(&mut ctx, TypeId::INT_LITERAL, 1.0);
    let two = lit(&mut ctx, TypeId::FLOAT_LITERAL, 2.0);
    let result = BinaryExpression::convert(&mut ctx, at(), one, op(OperatorKind::LogicalOr), two);
    assert_eq!(result, None);
    assert_eq!(
        messages(&ctx),
        ["type mismatch: '||' cannot operate on 'int', 'float'"]
    );
}

#[test]
fn binary_equality_mismatch_matches_golden() {
    // `float2(0) == 0`.
    let mut ctx = context(ProgramKind::Fragment);
    let float2 = lit(&mut ctx, TypeId::FLOAT2, 0.0);
    let zero = lit(&mut ctx, TypeId::INT_LITERAL, 0.0);
    let result = BinaryExpression::convert(&mut ctx, at(), float2, op(OperatorKind::EqEq), zero);
    assert_eq!(result, None);
    assert_eq!(
        messages(&ctx),
        ["type mismatch: '==' cannot operate on 'float2', 'int'"]
    );
}

#[test]
fn binary_assignment_sets_the_reference_kind() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let target = var_ref(&mut ctx, x, VariableRefKind::Read);
    let value = lit(&mut ctx, TypeId::FLOAT, 1.0);
    let assign = BinaryExpression::convert(&mut ctx, at(), target, op(OperatorKind::Eq), value)
        .expect("x = 1.0 converts");
    assert_eq!(messages(&ctx), Vec::<String>::new());
    assert_eq!(ctx.pool.expression(assign).ty, TypeId::FLOAT);
    assert!(matches!(
        kind(&ctx, target),
        ExpressionKind::VariableReference(VariableReference {
            ref_kind: VariableRefKind::Write,
            ..
        })
    ));
}

#[test]
fn compound_assignment_reads_and_writes() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let target = var_ref(&mut ctx, x, VariableRefKind::Read);
    let value = lit(&mut ctx, TypeId::FLOAT, 1.0);
    BinaryExpression::convert(&mut ctx, at(), target, op(OperatorKind::PlusEq), value)
        .expect("x += 1.0 converts");
    assert!(matches!(
        kind(&ctx, target),
        ExpressionKind::VariableReference(VariableReference {
            ref_kind: VariableRefKind::ReadWrite,
            ..
        })
    ));
}

#[test]
fn assignment_to_const_is_rejected() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::CONST);
    let target = var_ref(&mut ctx, x, VariableRefKind::Read);
    let value = lit(&mut ctx, TypeId::FLOAT, 1.0);
    assert_eq!(
        BinaryExpression::convert(&mut ctx, at(), target, op(OperatorKind::Eq), value),
        None
    );
    assert_eq!(messages(&ctx), ["cannot modify immutable variable 'x'"]);
}

#[test]
fn assignment_to_a_literal_is_rejected() {
    let mut ctx = context(ProgramKind::Fragment);
    let target = lit(&mut ctx, TypeId::FLOAT, 1.0);
    let value = lit(&mut ctx, TypeId::FLOAT, 2.0);
    assert_eq!(
        BinaryExpression::convert(&mut ctx, at(), target, op(OperatorKind::Eq), value),
        None
    );
    assert_eq!(messages(&ctx), ["cannot assign to this expression"]);
}

#[test]
fn assignment_out_of_range_literal_is_reported_once() {
    // `int i; i = 3000000000;` The value is reported by the cast to `int`, and then zeroed.
    let mut ctx = context(ProgramKind::Fragment);
    let i = var(&mut ctx, "i", TypeId::INT, ModifierFlags::empty());
    let target = var_ref(&mut ctx, i, VariableRefKind::Read);
    let value = lit(&mut ctx, TypeId::INT_LITERAL, 3_000_000_000.0);
    assert!(
        BinaryExpression::convert(&mut ctx, at(), target, op(OperatorKind::Eq), value).is_some()
    );
    assert_eq!(
        messages(&ctx),
        ["value is out of range for type 'int': 3000000000"]
    );
}

#[test]
fn binary_make_takes_the_result_type_of_the_operator() {
    let mut ctx = context(ProgramKind::Fragment);
    // Non-constant operands: constant operands would be folded (ConstantFolder::Simplify).
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let a = var_ref(&mut ctx, x, VariableRefKind::Read);
    let b = lit(&mut ctx, TypeId::FLOAT, 2.0);
    let compare = BinaryExpression::make(&mut ctx, at(), a, op(OperatorKind::Lt), b);
    assert_eq!(ctx.pool.expression(compare).ty, TypeId::BOOL);
    assert_eq!(ctx.pool.expression_description(compare), "x < 2.0");
}

#[test]
fn check_ref_follows_writable_references() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let read = var_ref(&mut ctx, x, VariableRefKind::Read);
    let write = var_ref(&mut ctx, x, VariableRefKind::Write);
    assert!(!BinaryExpression::check_ref(&ctx.pool, read));
    assert!(BinaryExpression::check_ref(&ctx.pool, write));
    let literal = lit(&mut ctx, TypeId::FLOAT, 1.0);
    assert!(!BinaryExpression::check_ref(&ctx.pool, literal));
}

#[test]
fn prefix_errors_match_golden_texts() {
    let mut ctx = context(ProgramKind::Fragment);
    let t = lit(&mut ctx, TypeId::BOOL, 1.0);
    assert_eq!(
        PrefixExpression::convert(&mut ctx, at(), op(OperatorKind::Minus), t),
        None
    );
    let one = lit(&mut ctx, TypeId::INT_LITERAL, 1.0);
    assert_eq!(
        PrefixExpression::convert(&mut ctx, at(), op(OperatorKind::LogicalNot), one),
        None
    );
    let f = lit(&mut ctx, TypeId::FLOAT_LITERAL, 1.0);
    assert_eq!(
        PrefixExpression::convert(&mut ctx, at(), op(OperatorKind::BitwiseNot), f),
        None
    );
    let literal = lit(&mut ctx, TypeId::INT_LITERAL, 1.0);
    assert_eq!(
        PrefixExpression::convert(&mut ctx, at(), op(OperatorKind::PlusPlus), literal),
        None
    );
    assert_eq!(
        messages(&ctx),
        [
            "'-' cannot operate on 'bool'",
            "'!' cannot operate on 'int'",
            "'~' cannot operate on 'float'",
            "cannot assign to this expression",
        ]
    );
}

#[test]
fn double_negation_becomes_the_operand() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let operand = var_ref(&mut ctx, x, VariableRefKind::Read);
    let once = PrefixExpression::make(&mut ctx, at(), op(OperatorKind::Minus), operand);
    let twice = PrefixExpression::make(&mut ctx, at(), op(OperatorKind::Minus), once);
    assert!(matches!(
        kind(&ctx, twice),
        ExpressionKind::VariableReference(VariableReference { variable, .. }) if variable == x
    ));
}

#[test]
fn logical_not_of_a_comparison_flips_the_comparison() {
    let mut ctx = context(ProgramKind::Fragment);
    // Non-constant operands: constant operands would be folded (ConstantFolder::Simplify).
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let a = var_ref(&mut ctx, x, VariableRefKind::Read);
    let b = lit(&mut ctx, TypeId::FLOAT, 2.0);
    let equal = BinaryExpression::make_with_result_type(
        &mut ctx,
        at(),
        a,
        op(OperatorKind::EqEq),
        b,
        TypeId::BOOL,
    );
    let negated = PrefixExpression::make(&mut ctx, at(), op(OperatorKind::LogicalNot), equal);
    assert!(matches!(
        kind(&ctx, negated),
        ExpressionKind::Binary(BinaryExpression { operator, .. })
            if operator.kind() == OperatorKind::Neq
    ));
}

#[test]
fn postfix_checks_and_writes() {
    let mut ctx = context(ProgramKind::Fragment);
    let t = lit(&mut ctx, TypeId::BOOL, 1.0);
    assert_eq!(
        PostfixExpression::convert(&mut ctx, at(), t, op(OperatorKind::PlusPlus)),
        None
    );
    assert_eq!(messages(&ctx), ["'++' cannot operate on 'bool'"]);

    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let operand = var_ref(&mut ctx, x, VariableRefKind::Read);
    let post = PostfixExpression::convert(&mut ctx, at(), operand, op(OperatorKind::PlusPlus))
        .expect("x++ converts");
    assert_eq!(ctx.pool.expression(post).ty, TypeId::FLOAT);
    assert!(matches!(
        kind(&ctx, operand),
        ExpressionKind::VariableReference(VariableReference {
            ref_kind: VariableRefKind::ReadWrite,
            ..
        })
    ));
}

#[test]
fn ternary_errors_match_skia_texts() {
    let mut ctx = context(ProgramKind::Fragment);
    let a = lit(&mut ctx, TypeId::FLOAT, 1.0);
    let b = lit(&mut ctx, TypeId::FLOAT, 2.0);
    let int_test = lit(&mut ctx, TypeId::INT_LITERAL, 1.0);
    assert_eq!(
        TernaryExpression::convert(&mut ctx, at(), int_test, a, b),
        None
    );

    let test = lit(&mut ctx, TypeId::BOOL, 1.0);
    let float2 = lit(&mut ctx, TypeId::FLOAT2, 0.0);
    assert_eq!(
        TernaryExpression::convert(&mut ctx, at(), test, float2, b),
        None
    );

    let void_true = EmptyExpression::make(&mut ctx, at());
    let void_false = EmptyExpression::make(&mut ctx, at());
    assert_eq!(
        TernaryExpression::convert(&mut ctx, at(), test, void_true, void_false),
        None
    );
    assert_eq!(
        messages(&ctx),
        [
            "expected 'bool', but found 'int'",
            "ternary operator result mismatch: 'float2', 'float'",
            "ternary expression of type 'void' is not allowed",
        ]
    );
}

#[test]
fn ternary_with_a_static_test_returns_a_branch() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let a = var_ref(&mut ctx, x, VariableRefKind::Read);
    let b = lit(&mut ctx, TypeId::FLOAT, 2.0);
    let yes = lit(&mut ctx, TypeId::BOOL, 1.0);
    assert_eq!(TernaryExpression::make(&mut ctx, at(), yes, a, b), a);
    let no = lit(&mut ctx, TypeId::BOOL, 0.0);
    assert_eq!(TernaryExpression::make(&mut ctx, at(), no, a, b), b);
}

#[test]
fn ternary_node_has_the_type_of_its_true_branch() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    let a = var_ref(&mut ctx, x, VariableRefKind::Read);
    let b = var_ref(&mut ctx, x, VariableRefKind::Read);
    let test_var = var(&mut ctx, "c", TypeId::BOOL, ModifierFlags::empty());
    let test = var_ref(&mut ctx, test_var, VariableRefKind::Read);
    let ternary = TernaryExpression::convert(&mut ctx, at(), test, a, b).expect("converts");
    assert_eq!(ctx.pool.expression(ternary).ty, TypeId::FLOAT);
    assert_eq!(ctx.pool.expression_description(ternary), "c ? x : x");
}

#[test]
fn index_errors_match_golden_texts() {
    let mut ctx = context(ProgramKind::Fragment);
    let base = lit(&mut ctx, TypeId::INT_LITERAL, 2.0);
    let zero = lit(&mut ctx, TypeId::INT_LITERAL, 0.0);
    assert_eq!(IndexExpression::convert(&mut ctx, at(), base, zero), None);
    assert_eq!(messages(&ctx), ["expected array, but found 'int'"]);
}

#[test]
fn index_out_of_range_matches_golden_text() {
    // tests/sksl/errors/ArrayIndexOutOfRange.glsl: `int a[123]; a[-1]`.
    let mut ctx = context(ProgramKind::Fragment);
    let (name, abbrev) = {
        let int = ctx.pool.ty(TypeId::INT);
        (int.array_name(123), int.abbreviated_name)
    };
    let array = ctx
        .pool
        .add_type(Type::new_array_type(name, abbrev, TypeId::INT, 123, false));
    let a = var(&mut ctx, "a", array, ModifierFlags::empty());
    let base = var_ref(&mut ctx, a, VariableRefKind::Read);
    let index = lit(&mut ctx, TypeId::INT_LITERAL, -1.0);
    assert_eq!(IndexExpression::convert(&mut ctx, at(), base, index), None);
    assert_eq!(messages(&ctx), ["index -1 out of range for 'int[123]'"]);
}

#[test]
fn index_into_a_vector_becomes_a_swizzle() {
    // `v[2]` is `v.z`.
    let mut ctx = context(ProgramKind::Fragment);
    let v = var(&mut ctx, "v", TypeId::FLOAT4, ModifierFlags::empty());
    let base = var_ref(&mut ctx, v, VariableRefKind::Read);
    let index = lit(&mut ctx, TypeId::INT_LITERAL, 2.0);
    let node = IndexExpression::convert(&mut ctx, at(), base, index).expect("v[2] converts");
    assert_eq!(ctx.pool.expression(node).ty, TypeId::FLOAT);
    assert!(matches!(
        kind(&ctx, node),
        ExpressionKind::Swizzle(Swizzle { components, .. }) if components.as_slice() == [2]
    ));
}

#[test]
fn index_into_a_matrix_is_a_column() {
    let mut ctx = context(ProgramKind::Fragment);
    let m = var(&mut ctx, "m", TypeId::FLOAT4X4, ModifierFlags::empty());
    let base = var_ref(&mut ctx, m, VariableRefKind::Read);
    let index = var(&mut ctx, "i", TypeId::INT, ModifierFlags::empty());
    let index = var_ref(&mut ctx, index, VariableRefKind::Read);
    let node = IndexExpression::convert(&mut ctx, at(), base, index).expect("m[i] converts");
    assert_eq!(ctx.pool.expression(node).ty, TypeId::FLOAT4);
}

#[test]
fn index_of_a_type_name_declares_an_array_type() {
    // `int[10]`.
    let mut ctx = context(ProgramKind::Fragment);
    symbol_table(&mut ctx);
    let base = TypeReference::make(&mut ctx, at(), TypeId::INT);
    let size = lit(&mut ctx, TypeId::INT_LITERAL, 10.0);
    let node = IndexExpression::convert(&mut ctx, at(), base, size).expect("int[10] converts");
    let ExpressionKind::TypeReference(reference) = kind(&ctx, node) else {
        panic!("expected a type reference");
    };
    assert_eq!(ctx.pool.ty(reference.value).name(), "int[10]");
}

#[test]
fn swizzle_masks_convert_and_type() {
    let mut ctx = context(ProgramKind::Fragment);
    let v = var(&mut ctx, "v", TypeId::FLOAT4, ModifierFlags::empty());
    let base = var_ref(&mut ctx, v, VariableRefKind::Read);
    let xy = Swizzle::convert(&mut ctx, at(), mask(), base, "xy").expect("v.xy converts");
    assert_eq!(ctx.pool.expression(xy).ty, TypeId::FLOAT2);
    assert_eq!(ctx.pool.expression_description(xy), "v.xy");

    // The identity swizzle is the base itself.
    let identity = Swizzle::convert(&mut ctx, at(), mask(), base, "xyzw").expect("v.xyzw converts");
    assert_eq!(identity, base);
    assert_eq!(messages(&ctx), Vec::<String>::new());
}

#[test]
fn swizzle_errors_match_skia_texts() {
    let mut ctx = context(ProgramKind::Fragment);
    let v = var(&mut ctx, "v", TypeId::FLOAT4, ModifierFlags::empty());
    let base = var_ref(&mut ctx, v, VariableRefKind::Read);
    assert_eq!(
        Swizzle::convert(&mut ctx, at(), mask(), base, "xyzwx"),
        None
    );
    assert_eq!(Swizzle::convert(&mut ctx, at(), mask(), base, "k"), None);
    assert_eq!(Swizzle::convert(&mut ctx, at(), mask(), base, "xr"), None);
    assert_eq!(Swizzle::convert(&mut ctx, at(), mask(), base, "00"), None);

    let s = var(&mut ctx, "s", TypeId::FLOAT, ModifierFlags::empty());
    let scalar = var_ref(&mut ctx, s, VariableRefKind::Read);
    assert_eq!(Swizzle::convert(&mut ctx, at(), mask(), scalar, "y"), None);
    assert_eq!(
        messages(&ctx),
        [
            "too many components in swizzle mask",
            "invalid swizzle component 'k'",
            "invalid swizzle mask 'xr'",
            "swizzle must refer to base expression",
            "invalid swizzle component 'y'",
        ]
    );
}

#[test]
fn swizzle_of_a_scalar_splats() {
    // `s.xxx` on a float is `float3(s)`.
    let mut ctx = context(ProgramKind::Fragment);
    let s = var(&mut ctx, "s", TypeId::FLOAT, ModifierFlags::empty());
    let scalar = var_ref(&mut ctx, s, VariableRefKind::Read);
    let splat = Swizzle::convert(&mut ctx, at(), mask(), scalar, "xxx").expect("s.xxx converts");
    assert_eq!(ctx.pool.expression(splat).ty, TypeId::FLOAT3);
    assert!(matches!(
        kind(&ctx, splat),
        ExpressionKind::ConstructorSplat(_)
    ));
}

#[test]
fn field_access_on_a_scalar_errors() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::INT, ModifierFlags::empty());
    let base = var_ref(&mut ctx, x, VariableRefKind::Read);
    assert_eq!(FieldAccess::convert(&mut ctx, at(), base, "foo"), None);
    assert_eq!(
        messages(&ctx),
        ["type 'int' does not have a field named 'foo'"]
    );
}

#[test]
fn field_access_on_sk_caps_is_a_setting() {
    let mut ctx = context(ProgramKind::Fragment);
    let caps = var(&mut ctx, "sk_Caps", TypeId::SK_CAPS, ModifierFlags::empty());
    let base = var_ref(&mut ctx, caps, VariableRefKind::Read);
    let node = FieldAccess::convert(&mut ctx, at(), base, "integerSupport").expect("converts");
    assert_eq!(
        ctx.pool.expression_description(node),
        "sk_Caps.integerSupport"
    );
}

#[test]
fn setting_errors_and_literal_value() {
    let mut ctx = context(ProgramKind::Fragment);
    assert_eq!(Setting::convert(&mut ctx, at(), "nope"), None);
    assert_eq!(messages(&ctx), ["unknown capability flag 'nope'"]);

    let node = Setting::convert(&mut ctx, at(), "integerSupport").expect("known flag");
    assert_eq!(ctx.pool.expression(node).ty, TypeId::BOOL);
    let ExpressionKind::Setting(setting) = kind(&ctx, node) else {
        panic!("expected a setting");
    };
    let caps = ShaderCapsFactory::default_caps();
    let literal = setting.to_literal(&mut ctx, at(), TypeId::BOOL, caps);
    let value = ctx
        .pool
        .expression(literal)
        .as_literal()
        .expect("a literal")
        .value;
    assert_eq!(value != 0.0, caps.integer_support);
}

#[test]
fn setting_is_reserved_in_runtime_shaders() {
    let mut ctx = context(ProgramKind::RuntimeShader);
    assert_eq!(Setting::convert(&mut ctx, at(), "integerSupport"), None);
    assert_eq!(messages(&ctx), ["name 'sk_Caps' is reserved"]);
}

#[test]
fn variable_reference_takes_the_variable_type() {
    let mut ctx = context(ProgramKind::Fragment);
    let x = var(&mut ctx, "x", TypeId::FLOAT2, ModifierFlags::empty());
    let node = var_ref(&mut ctx, x, VariableRefKind::Read);
    assert_eq!(ctx.pool.expression(node).ty, TypeId::FLOAT2);
    assert_eq!(ctx.pool.expression_description(node), "x");
}

#[test]
fn type_reference_rejects_generic_types() {
    let mut ctx = context(ProgramKind::Fragment);
    assert_eq!(
        TypeReference::convert(&mut ctx, at(), TypeId::GEN_TYPE),
        None
    );
    assert_eq!(messages(&ctx), ["type '$genType' is generic"]);
}

#[test]
fn instantiate_reports_unknown_identifiers() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    assert_eq!(instantiate_symbol_ref(&mut ctx, table, "nope", at()), None);
    assert_eq!(messages(&ctx), ["unknown identifier 'nope'"]);
}

#[test]
fn instantiate_a_variable_reads_it() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let x = var(&mut ctx, "x", TypeId::FLOAT, ModifierFlags::empty());
    add_symbol(&mut ctx, table, SymbolId::Variable(x));
    let node = instantiate_symbol_ref(&mut ctx, table, "x", at()).expect("x is declared");
    assert!(matches!(
        kind(&ctx, node),
        ExpressionKind::VariableReference(VariableReference {
            ref_kind: VariableRefKind::Read,
            ..
        })
    ));
    assert_eq!(ctx.pool.expression(node).ty, TypeId::FLOAT);
}

#[test]
fn instantiate_a_type_names_it() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    add_symbol(&mut ctx, table, SymbolId::Type(TypeId::FLOAT));
    let node = instantiate_symbol_ref(&mut ctx, table, "float", at()).expect("float is a type");
    assert!(matches!(
        kind(&ctx, node),
        ExpressionKind::TypeReference(TypeReference { value }) if value == TypeId::FLOAT
    ));
}

#[test]
fn child_call_describes_its_arguments() {
    let mut ctx = context(ProgramKind::Fragment);
    let child = var(&mut ctx, "child", TypeId::SHADER, ModifierFlags::empty());
    let p = var(&mut ctx, "p", TypeId::FLOAT2, ModifierFlags::empty());
    let arg = var_ref(&mut ctx, p, VariableRefKind::Read);
    let call = ChildCall::make(&mut ctx, at(), TypeId::HALF4, child, vec![arg]);
    assert_eq!(ctx.pool.expression(call).ty, TypeId::HALF4);
    assert_eq!(ctx.pool.expression_description(call), "child.eval(p)");
}

#[test]
fn empty_and_poison_expressions() {
    let mut ctx = context(ProgramKind::Fragment);
    let empty = EmptyExpression::make(&mut ctx, at());
    assert_eq!(ctx.pool.expression(empty).ty, TypeId::VOID);
    assert_eq!(ctx.pool.expression_description(empty), "false");
    let poison = Poison::make(&mut ctx, at());
    assert_eq!(ctx.pool.expression(poison).ty, TypeId::POISON);
    assert_eq!(
        ctx.pool.expression_description(poison),
        Compiler::POISON_TAG
    );
}

#[test]
fn symbol_instantiation_of_a_function_references_it() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = symbol_table(&mut ctx);
    let function = ctx.pool.add_function(FunctionDeclaration {
        position: at(),
        name: "f".into(),
        definition: None,
        next_overload: None,
        parameters: Vec::new(),
        return_type: TypeId::VOID,
        modifier_flags: ModifierFlags::empty(),
        intrinsic_kind: None,
        module_type: ModuleType::Program,
        is_main: false,
        has_main_coords_parameter: false,
        has_main_input_color_parameter: false,
        has_main_dest_color_parameter: false,
    });
    add_symbol(&mut ctx, table, SymbolId::FunctionDeclaration(function));
    let node = instantiate_symbol_ref(&mut ctx, table, "f", at()).expect("f is declared");
    assert!(matches!(
        kind(&ctx, node),
        ExpressionKind::FunctionReference(FunctionReference { overload_chain }) if overload_chain == function
    ));
    assert_eq!(ctx.pool.expression(node).ty, TypeId::INVALID);
}
