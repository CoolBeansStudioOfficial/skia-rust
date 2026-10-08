// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the constant folder (`ConstantFolder::Simplify` and its helpers). The IR is built by
//! hand, since the parser is not ported yet. Error texts are Skia's, byte for byte.

use super::{get_constant_int, get_constant_value, simplify};
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::ir::{
    ConstructorCompound, ConstructorDiagonalMatrix, ConstructorSplat, ExprId, Expression,
    ExpressionKind, Literal, ModifierFlags, TypeId, Variable, VariableRefKind, VariableReference,
    VariableStorage,
};
use crate::modules::ModuleType;
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

fn context(optimize: bool) -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    let settings = ProgramSettings {
        optimize,
        ..ProgramSettings::default()
    };
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        ProgramKind::Fragment,
        settings,
    ));
    ctx
}

fn add(ctx: &mut Context, ty: TypeId, kind: ExpressionKind) -> ExprId {
    ctx.pool
        .add_expression(Expression::new(Position::default(), ty, kind))
}

fn literal(ctx: &mut Context, ty: TypeId, value: f64) -> ExprId {
    add(ctx, ty, ExpressionKind::Literal(Literal { value }))
}

/// A read of a non-const local variable: not a compile-time constant.
fn variable(ctx: &mut Context, name: &str, ty: TypeId) -> ExprId {
    let var = ctx.pool.add_variable(Variable::new(
        Position::default(),
        Position::default(),
        ModifierFlags::empty(),
        name,
        ty,
        false,
        VariableStorage::Local,
    ));
    add(
        ctx,
        ty,
        ExpressionKind::VariableReference(VariableReference {
            variable: var,
            ref_kind: VariableRefKind::Read,
        }),
    )
}

fn compound(ctx: &mut Context, ty: TypeId, arguments: Vec<ExprId>) -> ExprId {
    add(
        ctx,
        ty,
        ExpressionKind::ConstructorCompound(ConstructorCompound { arguments }),
    )
}

fn op(kind: OperatorKind) -> Operator {
    kind.into()
}

fn describe(ctx: &Context, id: ExprId) -> String {
    ctx.pool.expression_description(id)
}

/// The messages reported so far, in order.
fn errors(ctx: &Context) -> Vec<String> {
    match ctx.errors.sink() {
        ErrorSink::Forwarding { errors } => errors
            .iter()
            .map(|(msg, _)| String::from_utf8(msg.clone()).expect("a UTF-8 message"))
            .collect(),
        other => panic!("expected a forwarding reporter, found {other:?}"),
    }
}

#[test]
fn float_addition_folds_to_a_literal() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::FLOAT, 1.5);
    let right = literal(&mut ctx, TypeId::FLOAT, 2.25);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Plus),
        right,
        TypeId::FLOAT,
    )
    .expect("1.5 + 2.25 folds");
    assert_eq!(describe(&ctx, result), "3.75");
    assert_eq!(errors(&ctx), Vec::<String>::new());
}

#[test]
fn int_division_truncates() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::INT_LITERAL, 7.0);
    let right = literal(&mut ctx, TypeId::INT_LITERAL, 2.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Slash),
        right,
        TypeId::INT_LITERAL,
    )
    .expect("7 / 2 folds");
    assert_eq!(describe(&ctx, result), "3");
}

#[test]
fn int_result_outside_the_type_is_not_folded() {
    // 2147483647 + 1 does not fit `int`: no fold, and no error (Skia's fold_expression returns
    // null).
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::INT, 2_147_483_647.0);
    let right = literal(&mut ctx, TypeId::INT, 1.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Plus),
        right,
        TypeId::INT,
    );
    assert_eq!(result, None);
    assert_eq!(errors(&ctx), Vec::<String>::new());
}

#[test]
fn division_by_a_constant_zero_is_an_error() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::FLOAT, 1.0);
    let right = literal(&mut ctx, TypeId::FLOAT, 0.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Slash),
        right,
        TypeId::FLOAT,
    );
    assert_eq!(result, None);
    assert_eq!(errors(&ctx), ["division by zero"]);
}

#[test]
fn int_min_divided_by_minus_one_overflows() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::INT_LITERAL, -9_223_372_036_854_775_808.0);
    let right = literal(&mut ctx, TypeId::INT_LITERAL, -1.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Slash),
        right,
        TypeId::INT_LITERAL,
    );
    assert_eq!(result, None);
    assert_eq!(errors(&ctx), ["arithmetic overflow"]);
}

#[test]
fn shift_past_31_is_an_error() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::INT_LITERAL, 1.0);
    let right = literal(&mut ctx, TypeId::INT_LITERAL, 40.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Shl),
        right,
        TypeId::INT_LITERAL,
    );
    assert_eq!(result, None);
    assert_eq!(errors(&ctx), ["shift value out of range"]);
}

#[test]
fn false_and_anything_is_false() {
    let mut ctx = context(true);
    let left = literal(&mut ctx, TypeId::BOOL, 0.0);
    let right = variable(&mut ctx, "x", TypeId::BOOL);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::LogicalAnd),
        right,
        TypeId::BOOL,
    )
    .expect("false && x folds");
    assert_eq!(describe(&ctx, result), "false");
}

#[test]
fn anything_and_true_is_the_anything() {
    let mut ctx = context(true);
    let left = variable(&mut ctx, "x", TypeId::BOOL);
    let right = literal(&mut ctx, TypeId::BOOL, 1.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::LogicalAnd),
        right,
        TypeId::BOOL,
    )
    .expect("x && true folds");
    assert_eq!(describe(&ctx, result), "x");
}

#[test]
fn constant_vectors_compare_slot_by_slot() {
    let mut ctx = context(true);
    let (a, b) = (
        literal(&mut ctx, TypeId::FLOAT, 1.0),
        literal(&mut ctx, TypeId::FLOAT, 2.0),
    );
    let left = compound(&mut ctx, TypeId::FLOAT2, vec![a, b]);
    let (c, d) = (
        literal(&mut ctx, TypeId::FLOAT, 1.0),
        literal(&mut ctx, TypeId::FLOAT, 3.0),
    );
    let right = compound(&mut ctx, TypeId::FLOAT2, vec![c, d]);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::EqEq),
        right,
        TypeId::BOOL,
    )
    .expect("float2(1, 2) == float2(1, 3) folds");
    assert_eq!(describe(&ctx, result), "false");
}

#[test]
fn splat_plus_scalar_folds_to_a_splat() {
    let mut ctx = context(true);
    let one = literal(&mut ctx, TypeId::FLOAT, 1.0);
    let left = add(
        &mut ctx,
        TypeId::FLOAT2,
        ExpressionKind::ConstructorSplat(ConstructorSplat { argument: one }),
    );
    let right = literal(&mut ctx, TypeId::FLOAT, 2.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Plus),
        right,
        TypeId::FLOAT2,
    )
    .expect("float2(1.0) + 2.0 folds");
    assert_eq!(describe(&ctx, result), "float2(3.0)");
}

#[test]
fn matrix_times_vector_folds_componentwise() {
    // mat2(1.0) * vec2(2.0, 3.0) == vec2(2.0, 3.0)
    let mut ctx = context(true);
    let one = literal(&mut ctx, TypeId::FLOAT, 1.0);
    let left = add(
        &mut ctx,
        TypeId::FLOAT2X2,
        ExpressionKind::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument: one }),
    );
    let (a, b) = (
        literal(&mut ctx, TypeId::FLOAT, 2.0),
        literal(&mut ctx, TypeId::FLOAT, 3.0),
    );
    let right = compound(&mut ctx, TypeId::FLOAT2, vec![a, b]);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Star),
        right,
        TypeId::FLOAT2,
    )
    .expect("mat2(1.0) * vec2(2, 3) folds");
    assert_eq!(describe(&ctx, result), "float2(2.0, 3.0)");
}

#[test]
fn multiplying_by_one_keeps_the_operand() {
    let mut ctx = context(true);
    let left = variable(&mut ctx, "x", TypeId::FLOAT2);
    let right = literal(&mut ctx, TypeId::FLOAT, 1.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Star),
        right,
        TypeId::FLOAT2,
    )
    .expect("x * 1.0 simplifies");
    assert_eq!(describe(&ctx, result), "x");
}

#[test]
fn multiplying_by_zero_gives_a_zero_of_the_result_type() {
    // x * 0.0 with a side-effect-free x is float2(0.0).
    let mut ctx = context(true);
    let left = variable(&mut ctx, "x", TypeId::FLOAT2);
    let right = literal(&mut ctx, TypeId::FLOAT, 0.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Star),
        right,
        TypeId::FLOAT2,
    )
    .expect("x * 0.0 simplifies");
    assert_eq!(describe(&ctx, result), "float2(0.0)");
}

#[test]
fn matrix_divided_by_scalar_multiplies_by_the_reciprocal() {
    // m / 2.0 becomes m * 0.5 (`simplify_matrix_division`).
    let mut ctx = context(true);
    let left = variable(&mut ctx, "m", TypeId::FLOAT2X2);
    let right = literal(&mut ctx, TypeId::FLOAT, 2.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Slash),
        right,
        TypeId::FLOAT2X2,
    )
    .expect("m / 2.0 simplifies");
    let ExpressionKind::Binary(binary) = &ctx.pool.expression(result).kind else {
        panic!("expected a binary expression");
    };
    assert_eq!(binary.operator.kind(), OperatorKind::Star);
    assert_eq!(get_constant_value(&ctx.pool, binary.right), Some(0.5));
}

#[test]
fn optimizer_off_skips_arithmetic_rules() {
    // Only full constant folding runs without the optimizer, so `x * 1.0` is left alone.
    let mut ctx = context(false);
    let left = variable(&mut ctx, "x", TypeId::FLOAT2);
    let right = literal(&mut ctx, TypeId::FLOAT, 1.0);
    let result = simplify(
        &mut ctx,
        Position::default(),
        left,
        op(OperatorKind::Star),
        right,
        TypeId::FLOAT2,
    );
    assert_eq!(result, None);
}

#[test]
fn integer_literals_are_read_back() {
    let mut ctx = context(true);
    let id = literal(&mut ctx, TypeId::INT_LITERAL, 42.0);
    assert_eq!(get_constant_int(&ctx.pool, id), Some(42));
    let var = variable(&mut ctx, "i", TypeId::INT);
    assert_eq!(get_constant_int(&ctx.pool, var), None);
}
