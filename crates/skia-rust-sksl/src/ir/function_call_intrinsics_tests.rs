// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the constant folding of intrinsics (`function_call_intrinsics.rs`). The expected values
//! are the ones Skia's folder computes for the same calls.

use super::function_call_intrinsics::optimize_intrinsic_call;
use super::{ConstructorCompound, ExprId, Expression, ExpressionKind, Literal, TypeId};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::intrinsic_list::IntrinsicKind;
use crate::modules::ModuleType;
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

fn context() -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        ProgramKind::Fragment,
        ProgramSettings::default(),
    ));
    ctx
}

fn lit(ctx: &mut Context, ty: TypeId, value: f64) -> ExprId {
    ctx.pool.add_expression(Expression::new(
        Position::default(),
        ty,
        ExpressionKind::Literal(Literal { value }),
    ))
}

/// The value of a folded result: a literal of `expected_ty`, or a compound of literals.
fn slots(ctx: &Context, expr: ExprId) -> Vec<f64> {
    let e = ctx.pool.expression(expr);
    let n = ctx.pool.ty(e.ty).slot_count();
    (0..n)
        .map(|i| {
            e.get_constant_value(&ctx.pool, i)
                .expect("the folded result is constant")
        })
        .collect()
}

#[test]
fn length_of_a_constant_vector_is_folded() {
    let mut ctx = context();
    let x = lit(&mut ctx, TypeId::FLOAT, 3.0);
    let y = lit(&mut ctx, TypeId::FLOAT, 4.0);
    let v = ConstructorCompound::make(&mut ctx, Position::default(), TypeId::FLOAT2, vec![x, y]);
    let folded = optimize_intrinsic_call(&mut ctx, IntrinsicKind::Length, &[v], TypeId::FLOAT)
        .expect("length of constants folds");
    assert_eq!(slots(&ctx, folded), [5.0]);
}

#[test]
fn cross_of_two_unit_vectors_is_folded() {
    let mut ctx = context();
    let one = lit(&mut ctx, TypeId::FLOAT, 1.0);
    let zero = lit(&mut ctx, TypeId::FLOAT, 0.0);
    let x_axis = ConstructorCompound::make(
        &mut ctx,
        Position::default(),
        TypeId::FLOAT3,
        vec![one, zero, zero],
    );
    let y_axis = ConstructorCompound::make(
        &mut ctx,
        Position::default(),
        TypeId::FLOAT3,
        vec![zero, one, zero],
    );
    let folded = optimize_intrinsic_call(
        &mut ctx,
        IntrinsicKind::Cross,
        &[x_axis, y_axis],
        TypeId::FLOAT3,
    )
    .expect("cross of constants folds");
    assert_eq!(slots(&ctx, folded), [0.0, 0.0, 1.0]);
}

#[test]
fn sign_of_a_negative_float_is_folded() {
    let mut ctx = context();
    let minus_two = lit(&mut ctx, TypeId::FLOAT, -2.0);
    let folded =
        optimize_intrinsic_call(&mut ctx, IntrinsicKind::Sign, &[minus_two], TypeId::FLOAT)
            .expect("sign of a constant folds");
    assert_eq!(slots(&ctx, folded), [-1.0]);
}

#[test]
fn an_intrinsic_whose_result_leaves_the_type_range_is_not_folded() {
    let mut ctx = context();
    // exp(1000) overflows a float, so Skia leaves the call alone.
    let big = lit(&mut ctx, TypeId::FLOAT, 1000.0);
    assert!(optimize_intrinsic_call(&mut ctx, IntrinsicKind::Exp, &[big], TypeId::FLOAT).is_none());
}
