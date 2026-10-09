// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionCall.cpp, the constant folding of intrinsics
// (`has_compile_time_constant_arguments`, the `Intrinsics` namespace and `optimize_intrinsic_call`,
// chrome/m156).
//
// Each intrinsic is evaluated in doubles, as Skia does. The values that Skia passes through a
// `float` are converted to `f32` at the same points, and the arithmetic on them is done in `f32`
// where the C++ does it on `float`s. The transcendental functions call the host `f64` functions
// (`// skia-rust: libm`), which is what `std::sin` and friends resolve to in the reference build.

//! Constant folding of intrinsic calls, as `FunctionCall::Make` does them.

// The folder mirrors the implicit conversions between double, float and integer types that the
// C++ does on the folded values (`float x = double`, `(int)std::round(...)`, the `SKSL_INT`
// conversions of the packing functions). Each one is noted where it happens.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use super::{ConstructorCompound, ExprId, Expression, ExpressionKind, IrPool, Literal, TypeId};
use crate::base_helpers::{double_to_float, float_to_half, half_to_float, ieee_double_divide};
use crate::constant_folder;
use crate::context::Context;
use crate::intrinsic_list::IntrinsicKind;
use crate::matrix_invert::{invert_2x2_matrix, invert_3x3_matrix, invert_4x4_matrix};
use crate::position::Position;

/// `IntrinsicArguments`: up to three argument expressions, with `None` for the missing ones.
type IntrinsicArguments = [Option<ExprId>; 3];

/// `CoalesceFn`.
type CoalesceFn = fn(f64, f64, f64) -> f64;
/// `FinalizeFn`.
type FinalizeFn = fn(f64) -> f64;
/// `EvaluateFn`.
type EvaluateFn = fn(f64, f64, f64) -> f64;
/// `CompareFn`.
type CompareFn = fn(f64, f64) -> bool;

/// A dimension (`columns()`, `rows()`, both `int` in C++) as a `usize`. Dimensions are never
/// negative.
fn dim(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// The `n`th slot of the constant `expr`. Skia dereferences the optional here, so a non-constant
/// slot is a bug in the caller.
fn slot(pool: &IrPool, expr: ExprId, n: usize) -> f64 {
    pool.expression(expr)
        .get_constant_value(pool, n)
        .expect("an intrinsic argument has a constant value in every slot")
}

/// `Get(idx, col)` of `optimize_intrinsic_call`: the slot as a `float`.
fn get(pool: &IrPool, args: &[ExprId], idx: usize, col: usize) -> f32 {
    slot(pool, args[idx], col) as f32
}

/// `Expression::type()`.
fn type_of(pool: &IrPool, expr: ExprId) -> TypeId {
    pool.expression(expr).ty
}

/// `arguments[i]->type().componentType()`.
fn component_of(pool: &IrPool, expr: ExprId) -> TypeId {
    pool.ty(type_of(pool, expr)).component_type().id()
}

/// `Literal{Position{}, value, type}`: a literal that keeps `value` as given, without the
/// rounding that `Literal::Make` applies. The intermediate results of the folder use these.
fn raw_literal(ctx: &mut Context, value: f64, ty: TypeId) -> ExprId {
    ctx.pool.add_expression(Expression::new(
        Position::default(),
        ty,
        ExpressionKind::Literal(Literal { value }),
    ))
}

/// `has_compile_time_constant_arguments`: every argument, with constant variables replaced by
/// their values, is a compile-time constant.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L55-L63 (chrome/m156)
pub(crate) fn has_compile_time_constant_arguments(pool: &IrPool, arguments: &[ExprId]) -> bool {
    arguments.iter().all(|&arg| {
        let expr = constant_folder::get_constant_value_for_variable(pool, arg);
        crate::analysis::is_compile_time_constant(pool, expr)
    })
}

/// `coalesce_n_way_vector`: folds up to two vector or scalar arguments into a scalar, in sequence.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L86-L145 (chrome/m156)
fn coalesce_n_way_vector(
    ctx: &mut Context,
    arg0: ExprId,
    arg1: Option<ExprId>,
    starting_state: f64,
    return_type: TypeId,
    coalesce: CoalesceFn,
    finalize: Option<FinalizeFn>,
) -> Option<ExprId> {
    let pool = &ctx.pool;
    let pos = pool.expression(arg0).position;
    let minimum_value = pool.ty(return_type).component_type().minimum_value();
    let maximum_value = pool.ty(return_type).component_type().maximum_value();

    let arg0_ty = type_of(pool, arg0);
    let arg1_ty = arg1.map(|arg| type_of(pool, arg));
    let vec_ty = if pool.ty(arg0_ty).is_vector() {
        arg0_ty
    } else if arg1_ty.is_some_and(|ty| pool.ty(ty).is_vector()) {
        arg1_ty.unwrap_or(arg0_ty)
    } else {
        arg0_ty
    };

    let mut value = starting_state;
    let mut arg0_index = 0;
    let mut arg1_index = 0;
    for _ in 0..dim(pool.ty(vec_ty).columns()) {
        let arg0_value = slot(pool, arg0, arg0_index);
        arg0_index += usize::from(pool.ty(arg0_ty).is_vector());

        let arg1_value = match arg1 {
            Some(arg1) => {
                let v = slot(pool, arg1, arg1_index);
                arg1_index += usize::from(pool.ty(type_of(pool, arg1)).is_vector());
                v
            }
            None => 0.0,
        };

        value = coalesce(value, arg0_value, arg1_value);

        // The value is outside the range of the return type, or is NaN: do not optimize.
        if !(minimum_value..=maximum_value).contains(&value) {
            return None;
        }
    }

    if let Some(finalize) = finalize {
        value = finalize(value);
    }

    Some(Literal::make(&mut ctx.pool, pos, value, return_type))
}

/// `coalesce_vector`: `coalesce_n_way_vector` over one argument.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L147-L159 (chrome/m156)
fn coalesce_vector(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    starting_state: f64,
    return_type: TypeId,
    coalesce: CoalesceFn,
    finalize: Option<FinalizeFn>,
) -> Option<ExprId> {
    coalesce_n_way_vector(
        ctx,
        arguments[0].expect("one argument"),
        None,
        starting_state,
        return_type,
        coalesce,
        finalize,
    )
}

/// `coalesce_pairwise_vectors`: `coalesce_n_way_vector` over two arguments.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L161-L176 (chrome/m156)
fn coalesce_pairwise_vectors(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    starting_state: f64,
    return_type: TypeId,
    coalesce: CoalesceFn,
    finalize: Option<FinalizeFn>,
) -> Option<ExprId> {
    coalesce_n_way_vector(
        ctx,
        arguments[0].expect("two arguments"),
        arguments[1],
        starting_state,
        return_type,
        coalesce,
        finalize,
    )
}

/// `optimize_comparison`: a bool vector of `compare` applied to each slot.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L178-L206 (chrome/m156)
fn optimize_comparison(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    compare: CompareFn,
) -> ExprId {
    let left = arguments[0].expect("two arguments");
    let right = arguments[1].expect("two arguments");
    let pool = &ctx.pool;
    let ty = type_of(pool, left);
    let columns = dim(pool.ty(ty).columns());
    let pos = pool.expression(left).position;

    let mut array = [0.0_f64; 4];
    for (index, out) in array.iter_mut().enumerate().take(columns) {
        let left_value = slot(pool, left, index);
        let right_value = slot(pool, right, index);
        *out = if compare(left_value, right_value) {
            1.0
        } else {
            0.0
        };
    }

    let bvec_type = pool.ty(TypeId::BOOL).to_compound(pool_columns(columns), 1);
    ConstructorCompound::make_from_constants(ctx, pos, bvec_type, &array[..columns])
}

/// The `int` column count of a vector type, as `to_compound` takes it.
fn pool_columns(columns: usize) -> i32 {
    i32::try_from(columns).unwrap_or(0)
}

/// `evaluate_n_way_intrinsic`: evaluates up to three arguments in tandem, slot by slot, and
/// builds a compound of the results.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L208-L264 (chrome/m156)
fn evaluate_n_way_intrinsic(
    ctx: &mut Context,
    arg0: ExprId,
    arg1: Option<ExprId>,
    arg2: Option<ExprId>,
    return_type: TypeId,
    eval: EvaluateFn,
) -> Option<ExprId> {
    let pool = &ctx.pool;
    let minimum_value = pool.ty(return_type).component_type().minimum_value();
    let maximum_value = pool.ty(return_type).component_type().maximum_value();
    let slots = pool.ty(return_type).slot_count();
    let mut array = [0.0_f64; 16];

    let (mut arg0_index, mut arg1_index, mut arg2_index) = (0, 0, 0);
    for out in array.iter_mut().take(slots) {
        let arg0_value = slot(pool, arg0, arg0_index);
        arg0_index += usize::from(!pool.ty(type_of(pool, arg0)).is_scalar());

        let arg1_value = match arg1 {
            Some(arg1) => {
                let v = slot(pool, arg1, arg1_index);
                arg1_index += usize::from(!pool.ty(type_of(pool, arg1)).is_scalar());
                v
            }
            None => 0.0,
        };

        let arg2_value = match arg2 {
            Some(arg2) => {
                let v = slot(pool, arg2, arg2_index);
                arg2_index += usize::from(!pool.ty(type_of(pool, arg2)).is_scalar());
                v
            }
            None => 0.0,
        };

        *out = eval(arg0_value, arg1_value, arg2_value);

        // The value is outside the range of the return type, or is NaN: do not optimize.
        if !(minimum_value..=maximum_value).contains(out) {
            return None;
        }
    }

    let pos = pool.expression(arg0).position;
    Some(ConstructorCompound::make_from_constants(
        ctx,
        pos,
        return_type,
        &array[..slots],
    ))
}

/// `evaluate_intrinsic<T>`: a one-argument evaluation.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L266-L277 (chrome/m156)
fn evaluate_intrinsic(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    return_type: TypeId,
    eval: EvaluateFn,
) -> Option<ExprId> {
    evaluate_n_way_intrinsic(
        ctx,
        arguments[0].expect("one argument"),
        None,
        None,
        return_type,
        eval,
    )
}

/// Whether the component type of `expr` is a float or an integer, which are the types the
/// evaluators handle.
fn is_numeric(pool: &IrPool, expr: ExprId) -> bool {
    let component = pool.ty(type_of(pool, expr)).component_type();
    component.is_float() || component.is_integer()
}

/// `evaluate_intrinsic_numeric`: a one-argument evaluation for a float or integer argument.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L278-L296 (chrome/m156)
fn evaluate_intrinsic_numeric(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    return_type: TypeId,
    eval: EvaluateFn,
) -> Option<ExprId> {
    let arg0 = arguments[0].expect("one argument");
    if !is_numeric(&ctx.pool, arg0) {
        return None;
    }
    evaluate_intrinsic(ctx, arguments, return_type, eval)
}

/// `evaluate_pairwise_intrinsic`: a two-argument evaluation for float or integer arguments.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L297-L319 (chrome/m156)
fn evaluate_pairwise_intrinsic(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    return_type: TypeId,
    eval: EvaluateFn,
) -> Option<ExprId> {
    let arg0 = arguments[0].expect("two arguments");
    if !is_numeric(&ctx.pool, arg0) {
        return None;
    }
    evaluate_n_way_intrinsic(ctx, arg0, arguments[1], None, return_type, eval)
}

/// `evaluate_3_way_intrinsic`: a three-argument evaluation for float or integer arguments.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L321-L346 (chrome/m156)
fn evaluate_3_way_intrinsic(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    return_type: TypeId,
    eval: EvaluateFn,
) -> Option<ExprId> {
    let arg0 = arguments[0].expect("three arguments");
    if !is_numeric(&ctx.pool, arg0) {
        return None;
    }
    evaluate_n_way_intrinsic(ctx, arg0, arguments[1], arguments[2], return_type, eval)
}

/// `pun_value<T1, T2>`: the bits of `val` read as a `T1`, then as a `T2`, returned as a double.
/// The bit patterns are the same in Rust and C++; the out-of-range conversions of the double
/// are not reached by folded arguments.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L348-L358 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ `(T1)val` truncation.
fn pun_float_to_int(val: f64) -> f64 {
    f64::from(double_to_float(val).to_bits().cast_signed())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ `(T1)val` truncation.
fn pun_float_to_uint(val: f64) -> f64 {
    f64::from(double_to_float(val).to_bits())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ `(T1)val` truncation.
fn pun_int_to_float(val: f64) -> f64 {
    f64::from(f32::from_bits(val as i32 as u32))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ `(T1)val` truncation.
fn pun_uint_to_float(val: f64) -> f64 {
    f64::from(f32::from_bits(val as u32))
}

// The `Intrinsics` namespace. The functions that evaluate one slot are named `eval_*`; the ones
// that build a whole expression are named `evaluate_*`, as in Skia.

fn coalesce_length(a: f64, b: f64, _: f64) -> f64 {
    a + (b * b)
}
fn finalize_length(a: f64) -> f64 {
    a.sqrt() // skia-rust: libm
}

fn coalesce_distance(a: f64, mut b: f64, c: f64) -> f64 {
    b -= c;
    a + (b * b)
}
fn finalize_distance(a: f64) -> f64 {
    a.sqrt() // skia-rust: libm
}

fn coalesce_dot(a: f64, b: f64, c: f64) -> f64 {
    a + (b * c)
}
fn coalesce_any(a: f64, b: f64, _: f64) -> f64 {
    // C++ `a || b`: a NaN is true.
    if a != 0.0 || b != 0.0 { 1.0 } else { 0.0 }
}
fn coalesce_all(a: f64, b: f64, _: f64) -> f64 {
    if a != 0.0 && b != 0.0 { 1.0 } else { 0.0 }
}

fn compare_less_than(a: f64, b: f64) -> bool {
    a < b
}
fn compare_less_than_equal(a: f64, b: f64) -> bool {
    a <= b
}
fn compare_greater_than(a: f64, b: f64) -> bool {
    a > b
}
fn compare_greater_than_equal(a: f64, b: f64) -> bool {
    a >= b
}
// Skia compares the slots with `==` and `!=` on doubles.
#[allow(clippy::float_cmp)]
fn compare_equal(a: f64, b: f64) -> bool {
    a == b
}
#[allow(clippy::float_cmp)]
fn compare_not_equal(a: f64, b: f64) -> bool {
    a != b
}

fn eval_radians(a: f64, _: f64, _: f64) -> f64 {
    a * 0.017_453_292_5
}
fn eval_degrees(a: f64, _: f64, _: f64) -> f64 {
    a * 57.295_779_5
}
fn eval_sin(a: f64, _: f64, _: f64) -> f64 {
    a.sin() // skia-rust: libm
}
fn eval_cos(a: f64, _: f64, _: f64) -> f64 {
    a.cos() // skia-rust: libm
}
fn eval_tan(a: f64, _: f64, _: f64) -> f64 {
    a.tan() // skia-rust: libm
}
fn eval_asin(a: f64, _: f64, _: f64) -> f64 {
    a.asin() // skia-rust: libm
}
fn eval_acos(a: f64, _: f64, _: f64) -> f64 {
    a.acos() // skia-rust: libm
}
fn eval_atan(a: f64, _: f64, _: f64) -> f64 {
    a.atan() // skia-rust: libm
}
fn eval_atan2(a: f64, b: f64, _: f64) -> f64 {
    a.atan2(b) // skia-rust: libm
}
fn eval_asinh(a: f64, _: f64, _: f64) -> f64 {
    a.asinh() // skia-rust: libm
}
fn eval_acosh(a: f64, _: f64, _: f64) -> f64 {
    a.acosh() // skia-rust: libm
}
fn eval_atanh(a: f64, _: f64, _: f64) -> f64 {
    a.atanh() // skia-rust: libm
}

fn eval_pow(a: f64, b: f64, _: f64) -> f64 {
    a.powf(b) // skia-rust: libm
}
fn eval_exp(a: f64, _: f64, _: f64) -> f64 {
    a.exp() // skia-rust: libm
}
fn eval_log(a: f64, _: f64, _: f64) -> f64 {
    a.ln() // skia-rust: libm
}
fn eval_exp2(a: f64, _: f64, _: f64) -> f64 {
    a.exp2() // skia-rust: libm
}
fn eval_log2(a: f64, _: f64, _: f64) -> f64 {
    a.log2() // skia-rust: libm
}
fn eval_sqrt(a: f64, _: f64, _: f64) -> f64 {
    a.sqrt() // skia-rust: libm
}
fn eval_inversesqrt(a: f64, _: f64, _: f64) -> f64 {
    ieee_double_divide(1.0, a.sqrt()) // skia-rust: libm
}

fn eval_add(a: f64, b: f64, _: f64) -> f64 {
    a + b
}
fn eval_sub(a: f64, b: f64, _: f64) -> f64 {
    a - b
}
fn eval_mul(a: f64, b: f64, _: f64) -> f64 {
    a * b
}
fn eval_div(a: f64, b: f64, _: f64) -> f64 {
    ieee_double_divide(a, b)
}
fn eval_abs(a: f64, _: f64, _: f64) -> f64 {
    a.abs()
}
fn eval_sign(a: f64, _: f64, _: f64) -> f64 {
    f64::from(i32::from(a > 0.0) - i32::from(a < 0.0))
}
fn eval_opposite_sign(a: f64, _: f64, _: f64) -> f64 {
    f64::from(i32::from(a < 0.0) - i32::from(a > 0.0))
}
fn eval_floor(a: f64, _: f64, _: f64) -> f64 {
    a.floor()
}
fn eval_ceil(a: f64, _: f64, _: f64) -> f64 {
    a.ceil()
}
fn eval_fract(a: f64, _: f64, _: f64) -> f64 {
    a - a.floor()
}
fn eval_min(a: f64, b: f64, _: f64) -> f64 {
    if a < b { a } else { b }
}
fn eval_max(a: f64, b: f64, _: f64) -> f64 {
    if a > b { a } else { b }
}
fn eval_clamp(x: f64, l: f64, h: f64) -> f64 {
    if x < l {
        l
    } else if x > h {
        h
    } else {
        x
    }
}
fn eval_fma(a: f64, b: f64, c: f64) -> f64 {
    a * b + c
}
fn eval_saturate(a: f64, _: f64, _: f64) -> f64 {
    // `(a < 0) ? 0 : (a > 1) ? 1 : a`, which is `clamp` (and keeps a NaN).
    a.clamp(0.0, 1.0)
}
fn eval_mix(x: f64, y: f64, a: f64) -> f64 {
    x * (1.0 - a) + y * a
}
fn eval_step(e: f64, x: f64, _: f64) -> f64 {
    if x < e { 0.0 } else { 1.0 }
}
fn eval_mod(a: f64, b: f64, _: f64) -> f64 {
    a - b * ieee_double_divide(a, b).floor() // skia-rust: libm
}
fn eval_smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ieee_double_divide(x - edge0, edge1 - edge0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn eval_matrix_comp_mult(x: f64, y: f64, _: f64) -> f64 {
    x * y
}

// C++ `!a`: true when `a` is zero, and false for NaN.
#[allow(clippy::float_cmp)] // The comparison with zero is the C++ `!a`.
fn eval_not(a: f64, _: f64, _: f64) -> f64 {
    if a == 0.0 { 1.0 } else { 0.0 }
}
fn eval_sinh(a: f64, _: f64, _: f64) -> f64 {
    a.sinh() // skia-rust: libm
}
fn eval_cosh(a: f64, _: f64, _: f64) -> f64 {
    a.cosh() // skia-rust: libm
}
fn eval_tanh(a: f64, _: f64, _: f64) -> f64 {
    a.tanh() // skia-rust: libm
}
fn eval_trunc(a: f64, _: f64, _: f64) -> f64 {
    a.trunc()
}
fn eval_round(a: f64, _: f64, _: f64) -> f64 {
    // `std::remainder(a, 1.0)` rounds to even, so it is `a - roundEven(a)`. The remainder is then
    // `a - n`, and subtracting it from `a` gives back `n`, which is exact.
    let remainder = a - a.round_ties_even();
    a - remainder
}
fn eval_float_bits_to_int(a: f64, _: f64, _: f64) -> f64 {
    pun_float_to_int(a)
}
fn eval_float_bits_to_uint(a: f64, _: f64, _: f64) -> f64 {
    pun_float_to_uint(a)
}
fn eval_int_bits_to_float(a: f64, _: f64, _: f64) -> f64 {
    pun_int_to_float(a)
}
fn eval_uint_bits_to_float(a: f64, _: f64, _: f64) -> f64 {
    pun_uint_to_float(a)
}

/// `evaluate_length`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L446-L451 (chrome/m156)
fn evaluate_length(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let ty = component_of(&ctx.pool, arguments[0].expect("one argument"));
    coalesce_vector(
        ctx,
        arguments,
        0.0,
        ty,
        coalesce_length,
        Some(finalize_length),
    )
}

/// `evaluate_distance`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L453-L457 (chrome/m156)
fn evaluate_distance(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let ty = component_of(&ctx.pool, arguments[0].expect("two arguments"));
    coalesce_pairwise_vectors(
        ctx,
        arguments,
        0.0,
        ty,
        coalesce_distance,
        Some(finalize_distance),
    )
}

/// `evaluate_dot`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L459-L464 (chrome/m156)
fn evaluate_dot(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let ty = component_of(&ctx.pool, arguments[0].expect("two arguments"));
    coalesce_pairwise_vectors(ctx, arguments, 0.0, ty, coalesce_dot, None)
}

/// `evaluate_sign`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L466-L470 (chrome/m156)
fn evaluate_sign(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let ty = type_of(&ctx.pool, arguments[0].expect("one argument"));
    evaluate_intrinsic_numeric(ctx, arguments, ty, eval_sign)
}

/// `evaluate_opposite_sign`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L472-L476 (chrome/m156)
fn evaluate_opposite_sign(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let ty = type_of(&ctx.pool, arguments[0].expect("one argument"));
    evaluate_intrinsic_numeric(ctx, arguments, ty, eval_opposite_sign)
}

/// `evaluate_add`, `evaluate_sub`, `evaluate_mul` and `evaluate_div`: pairwise, with the type of
/// the first argument.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L478-L500 (chrome/m156)
fn evaluate_pairwise_with_first_type(
    ctx: &mut Context,
    arguments: &IntrinsicArguments,
    eval: EvaluateFn,
) -> Option<ExprId> {
    let ty = type_of(&ctx.pool, arguments[0].expect("two arguments"));
    evaluate_pairwise_intrinsic(ctx, arguments, ty, eval)
}

/// `evaluate_normalize`: `v / length(v)`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L502-L510 (chrome/m156)
fn evaluate_normalize(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    // normalize(v): v / length(v)
    let length = evaluate_length(ctx, arguments)?;
    let div_args = [arguments[0], Some(length), None];
    evaluate_pairwise_with_first_type(ctx, &div_args, eval_div)
}

/// `evaluate_faceforward`: `N * -sign(dot(I, NRef))`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L512-L529 (chrome/m156)
fn evaluate_faceforward(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let n = arguments[0].expect("three arguments"); // vector
    let i = arguments[1].expect("three arguments"); // vector
    let n_ref = arguments[2].expect("three arguments"); // vector

    // faceforward(N,I,NRef): N * -sign(dot(I, NRef))
    let dot_args = [Some(i), Some(n_ref), None];
    let dot_expr = evaluate_dot(ctx, &dot_args)?;

    let sign_args = [Some(dot_expr), None, None];
    let sign_expr = evaluate_opposite_sign(ctx, &sign_args)?;

    let mul_args = [Some(n), Some(sign_expr), None];
    evaluate_pairwise_with_first_type(ctx, &mul_args, eval_mul)
}

/// `evaluate_reflect`: `I - 2 * dot(N, I) * N`, as `I - (temp + temp)` with `temp = N * dot`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L531-L551 (chrome/m156)
fn evaluate_reflect(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let i = arguments[0].expect("two arguments"); // vector
    let n = arguments[1].expect("two arguments"); // vector

    // reflect(I,N): temp = (N * dot(N, I)); reflect = I - (temp + temp)
    let dot_args = [Some(n), Some(i), None];
    let dot_expr = evaluate_dot(ctx, &dot_args)?;

    let mul_args = [Some(n), Some(dot_expr), None];
    let mul_expr = evaluate_pairwise_with_first_type(ctx, &mul_args, eval_mul)?;

    let add_args = [Some(mul_expr), Some(mul_expr), None];
    let add_expr = evaluate_pairwise_with_first_type(ctx, &add_args, eval_add)?;

    let sub_args = [Some(i), Some(add_expr), None];
    evaluate_pairwise_with_first_type(ctx, &sub_args, eval_sub)
}

/// `evaluate_refract`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L553-L628 (chrome/m156)
fn evaluate_refract(ctx: &mut Context, arguments: &IntrinsicArguments) -> Option<ExprId> {
    let i = arguments[0].expect("three arguments"); // vector
    let n = arguments[1].expect("three arguments"); // vector
    let eta = arguments[2].expect("three arguments"); // scalar

    // K = 1.0 - Eta^2 * (1.0 - Dot(N, I)^2);

    // DotNI = Dot(N, I)
    let dot_ni_args = [Some(n), Some(i), None];
    let dot_ni_expr = evaluate_dot(ctx, &dot_ni_args)?;

    // DotNI2 = DotNI * DotNI
    let dot_ni2_args = [Some(dot_ni_expr), Some(dot_ni_expr), None];
    let dot_ni_squared = evaluate_pairwise_with_first_type(ctx, &dot_ni2_args, eval_mul)?;

    // OneMinusDot = 1 - DotNI2
    let one_literal = raw_literal(ctx, 1.0, type_of(&ctx.pool, dot_ni_squared));
    let one_minus_dot_args = [Some(one_literal), Some(dot_ni_squared), None];
    let one_minus_dot_expr = evaluate_pairwise_with_first_type(ctx, &one_minus_dot_args, eval_sub)?;

    // Eta2 = Eta * Eta
    let eta2_args = [Some(eta), Some(eta), None];
    let eta2_expr = evaluate_pairwise_with_first_type(ctx, &eta2_args, eval_mul)?;

    // Eta2xDot = Eta2 * OneMinusDot
    let eta2_x_dot_args = [Some(eta2_expr), Some(one_minus_dot_expr), None];
    let eta2_x_dot_expr = evaluate_pairwise_with_first_type(ctx, &eta2_x_dot_args, eval_mul)?;

    // K = 1.0 - Eta2xDot
    let k_args = [Some(one_literal), Some(eta2_x_dot_expr), None];
    let k_expr = evaluate_pairwise_with_first_type(ctx, &k_args, eval_sub)?;
    let ExpressionKind::Literal(k_literal) = ctx.pool.expression(k_expr).kind else {
        return None;
    };

    // When K < 0, Refract(I, N, Eta) = vec(0)
    let k_value = k_literal.value;
    if k_value < 0.0 {
        let k_zero = [0.0_f64; 4];
        let pos = ctx.pool.expression(i).position;
        let ty = type_of(&ctx.pool, i);
        return Some(ConstructorCompound::make_from_constants(
            ctx, pos, ty, &k_zero,
        ));
    }

    // When K ≥ 0, Refract(I, N, Eta) = (I * Eta) - N * (Eta * Dot(N,I) + Sqrt(K))

    // EtaDot = Eta * DotNI
    let eta_dot_args = [Some(eta), Some(dot_ni_expr), None];
    let eta_dot_expr = evaluate_pairwise_with_first_type(ctx, &eta_dot_args, eval_mul)?;

    // EtaDotSqrt = EtaDot + Sqrt(K)
    let sqrt_k_literal = raw_literal(ctx, k_value.sqrt(), type_of(&ctx.pool, eta));
    let eta_dot_sqrt_args = [Some(eta_dot_expr), Some(sqrt_k_literal), None];
    let eta_dot_sqrt_expr = evaluate_pairwise_with_first_type(ctx, &eta_dot_sqrt_args, eval_add)?;

    // NxEDS = N * EtaDotSqrt
    let n_x_eds_args = [Some(n), Some(eta_dot_sqrt_expr), None];
    let n_x_eds_expr = evaluate_pairwise_with_first_type(ctx, &n_x_eds_args, eval_mul)?;

    // IEta = I * Eta
    let i_eta_args = [Some(i), Some(eta), None];
    let i_eta_expr = evaluate_pairwise_with_first_type(ctx, &i_eta_args, eval_mul)?;

    // Refract = IEta - NxEDS
    let refract_args = [Some(i_eta_expr), Some(n_x_eds_expr), None];
    evaluate_pairwise_with_first_type(ctx, &refract_args, eval_sub)
}

/// `extract_matrix`: the slots of `expr` as floats.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L630-L635 (chrome/m156)
fn extract_matrix(pool: &IrPool, expr: ExprId, mat: &mut [f32; 16]) {
    let num_slots = pool.ty(type_of(pool, expr)).slot_count();
    for (index, out) in mat.iter_mut().enumerate().take(num_slots) {
        *out = slot(pool, expr, index) as f32;
    }
}

/// `optimize_intrinsic_call`: the constant value of a call to `intrinsic` whose arguments are
/// constants, or `None` when the call is not folded.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L637-L1005 (chrome/m156)
#[allow(clippy::too_many_lines)] // One arm per intrinsic, as Skia's switch.
pub(crate) fn optimize_intrinsic_call(
    ctx: &mut Context,
    intrinsic: IntrinsicKind,
    arg_array: &[ExprId],
    return_type: TypeId,
) -> Option<ExprId> {
    // Replace constant variables with their literal values.
    use IntrinsicKind as K;
    if arg_array.is_empty() {
        return None;
    }
    let args: Vec<ExprId> = arg_array
        .iter()
        .map(|&arg| constant_folder::get_constant_value_for_variable(&ctx.pool, arg))
        .collect();
    let mut arguments: IntrinsicArguments = [None; 3];
    for (index, &arg) in args.iter().enumerate() {
        arguments[index] = Some(arg);
    }
    let pos = ctx.pool.expression(args[0]).position;

    match intrinsic {
        // 8.1 : Angle and Trigonometry Functions
        K::Radians => evaluate_intrinsic(ctx, &arguments, return_type, eval_radians),
        K::Degrees => evaluate_intrinsic(ctx, &arguments, return_type, eval_degrees),
        K::Sin => evaluate_intrinsic(ctx, &arguments, return_type, eval_sin),
        K::Cos => evaluate_intrinsic(ctx, &arguments, return_type, eval_cos),
        K::Tan => evaluate_intrinsic(ctx, &arguments, return_type, eval_tan),
        K::Sinh => evaluate_intrinsic(ctx, &arguments, return_type, eval_sinh),
        K::Cosh => evaluate_intrinsic(ctx, &arguments, return_type, eval_cosh),
        K::Tanh => evaluate_intrinsic(ctx, &arguments, return_type, eval_tanh),
        K::Asin => evaluate_intrinsic(ctx, &arguments, return_type, eval_asin),
        K::Acos => evaluate_intrinsic(ctx, &arguments, return_type, eval_acos),
        K::Atan => {
            if args.len() == 1 {
                evaluate_intrinsic(ctx, &arguments, return_type, eval_atan)
            } else {
                evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_atan2)
            }
        }
        K::Asinh => evaluate_intrinsic(ctx, &arguments, return_type, eval_asinh),
        K::Acosh => evaluate_intrinsic(ctx, &arguments, return_type, eval_acosh),
        K::Atanh => evaluate_intrinsic(ctx, &arguments, return_type, eval_atanh),
        // 8.2 : Exponential Functions
        K::Pow => evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_pow),
        K::Exp => evaluate_intrinsic(ctx, &arguments, return_type, eval_exp),
        K::Log => evaluate_intrinsic(ctx, &arguments, return_type, eval_log),
        K::Exp2 => evaluate_intrinsic(ctx, &arguments, return_type, eval_exp2),
        K::Log2 => evaluate_intrinsic(ctx, &arguments, return_type, eval_log2),
        K::Sqrt => evaluate_intrinsic(ctx, &arguments, return_type, eval_sqrt),
        K::Inversesqrt => evaluate_intrinsic(ctx, &arguments, return_type, eval_inversesqrt),
        // 8.3 : Common Functions
        K::Abs => evaluate_intrinsic_numeric(ctx, &arguments, return_type, eval_abs),
        K::Sign => evaluate_sign(ctx, &arguments),
        K::Floor => evaluate_intrinsic(ctx, &arguments, return_type, eval_floor),
        K::Ceil => evaluate_intrinsic(ctx, &arguments, return_type, eval_ceil),
        K::Fract => evaluate_intrinsic(ctx, &arguments, return_type, eval_fract),
        K::Mod => evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_mod),
        K::Min => evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_min),
        K::Max => evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_max),
        K::Clamp => evaluate_3_way_intrinsic(ctx, &arguments, return_type, eval_clamp),
        K::Fma => evaluate_3_way_intrinsic(ctx, &arguments, return_type, eval_fma),
        K::Saturate => evaluate_intrinsic(ctx, &arguments, return_type, eval_saturate),
        K::Mix => {
            let third = arguments[2].expect("three arguments");
            if ctx.pool.ty(component_of(&ctx.pool, third)).is_boolean() {
                let numeric = component_of(&ctx.pool, arguments[0].expect("three arguments"));
                let numeric_ok = {
                    let t = ctx.pool.ty(numeric);
                    t.is_float() || t.is_integer() || t.is_boolean()
                };
                if !numeric_ok {
                    return None;
                }
                evaluate_n_way_intrinsic(
                    ctx,
                    arguments[0].expect("three arguments"),
                    arguments[1],
                    Some(third),
                    return_type,
                    eval_mix,
                )
            } else {
                evaluate_3_way_intrinsic(ctx, &arguments, return_type, eval_mix)
            }
        }
        K::Step => evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_step),
        K::Smoothstep => evaluate_3_way_intrinsic(ctx, &arguments, return_type, eval_smoothstep),
        K::Trunc => evaluate_intrinsic(ctx, &arguments, return_type, eval_trunc),
        // GLSL `round` documents its rounding mode as unspecified and is allowed to behave
        // identically to `roundEven`.
        K::Round | K::RoundEven => evaluate_intrinsic(ctx, &arguments, return_type, eval_round),
        K::FloatBitsToInt => {
            evaluate_intrinsic(ctx, &arguments, return_type, eval_float_bits_to_int)
        }
        K::FloatBitsToUint => {
            evaluate_intrinsic(ctx, &arguments, return_type, eval_float_bits_to_uint)
        }
        K::IntBitsToFloat => {
            evaluate_intrinsic(ctx, &arguments, return_type, eval_int_bits_to_float)
        }
        K::UintBitsToFloat => {
            evaluate_intrinsic(ctx, &arguments, return_type, eval_uint_bits_to_float)
        }
        // 8.4 : Floating-Point Pack and Unpack Functions
        K::PackUnorm2x16 => {
            let packed = pack_two(ctx, &args, |x| pack_clamped(x, 0.0, 1.0, 65535.0));
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::UINT,
                &[packed],
            ))
        }
        K::PackSnorm2x16 => {
            let packed = pack_two(ctx, &args, |x| pack_clamped(x, -1.0, 1.0, 32767.0));
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::UINT,
                &[packed],
            ))
        }
        K::PackHalf2x16 => {
            let packed = pack_two(ctx, &args, |x| u32::from(float_to_half(x)));
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::UINT,
                &[packed],
            ))
        }
        K::UnpackUnorm2x16 => {
            let x = slot(&ctx.pool, args[0], 0) as i64;
            let a = (x & 0x0000_FFFF) as u16;
            let b = ((x >> 16) & 0x0000_FFFF) as u16;
            let unpacked = [f64::from(a) / 65535.0, f64::from(b) / 65535.0];
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::FLOAT2,
                &unpacked,
            ))
        }
        K::UnpackSnorm2x16 => {
            let x = slot(&ctx.pool, args[0], 0) as i64;
            let a = (x & 0x0000_FFFF) as i16;
            let b = ((x >> 16) & 0x0000_FFFF) as i16;
            let unpacked = [
                eval_clamp(f64::from(a) / 32767.0, -1.0, 1.0),
                eval_clamp(f64::from(b) / 32767.0, -1.0, 1.0),
            ];
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::FLOAT2,
                &unpacked,
            ))
        }
        K::UnpackHalf2x16 => {
            let x = slot(&ctx.pool, args[0], 0) as i64;
            let a = (x & 0x0000_FFFF) as u16;
            let b = ((x >> 16) & 0x0000_FFFF) as u16;
            let unpacked = [f64::from(half_to_float(a)), f64::from(half_to_float(b))];
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                TypeId::FLOAT2,
                &unpacked,
            ))
        }
        // 8.5 : Geometric Functions
        K::Length => evaluate_length(ctx, &arguments),
        K::Distance => evaluate_distance(ctx, &arguments),
        K::Dot => evaluate_dot(ctx, &arguments),
        K::Cross => {
            let pool = &ctx.pool;
            // `X` and `Y` return floats, so the products and the difference are in `float`.
            let x = |n| get(pool, &args, 0, n);
            let y = |n| get(pool, &args, 1, n);
            // The vec2 form is not a real intrinsic.
            let vec = [
                f64::from(x(1) * y(2) - y(1) * x(2)),
                f64::from(x(2) * y(0) - y(2) * x(0)),
                f64::from(x(0) * y(1) - y(0) * x(1)),
            ];
            let pos = pool.expression(args[0]).position;
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                return_type,
                &vec,
            ))
        }
        K::Normalize => evaluate_normalize(ctx, &arguments),
        K::Faceforward => evaluate_faceforward(ctx, &arguments),
        K::Reflect => evaluate_reflect(ctx, &arguments),
        K::Refract => evaluate_refract(ctx, &arguments),
        // 8.6 : Matrix Functions
        K::MatrixCompMult => {
            evaluate_pairwise_intrinsic(ctx, &arguments, return_type, eval_matrix_comp_mult)
        }
        K::Transpose => {
            let pool = &ctx.pool;
            let columns = dim(pool.ty(return_type).columns());
            let rows = dim(pool.ty(return_type).rows());
            let mut mat = [0.0_f64; 16];
            let mut index = 0;
            for c in 0..columns {
                for r in 0..rows {
                    mat[index] = f64::from(get(pool, &args, 0, (columns * r) + c));
                    index += 1;
                }
            }
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                return_type,
                &mat[..columns * rows],
            ))
        }
        K::OuterProduct => {
            let pool = &ctx.pool;
            let columns = dim(pool.ty(return_type).columns());
            let rows = dim(pool.ty(return_type).rows());
            let mut mat = [0.0_f64; 16];
            let mut index = 0;
            for c in 0..columns {
                for r in 0..rows {
                    // `float * float`, stored as a double.
                    mat[index] = f64::from(get(pool, &args, 0, r) * get(pool, &args, 1, c));
                    index += 1;
                }
            }
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                return_type,
                &mat[..columns * rows],
            ))
        }
        K::Determinant => {
            let mut mat = [0.0_f32; 16];
            extract_matrix(&ctx.pool, args[0], &mut mat);
            let determinant = match ctx.pool.ty(type_of(&ctx.pool, args[0])).slot_count() {
                4 => invert_2x2_matrix(&first::<4>(&mat), None),
                9 => invert_3x3_matrix(&first::<9>(&mat), None),
                16 => invert_4x4_matrix(&mat, None),
                _ => return None,
            };
            Some(Literal::make_float(
                &mut ctx.pool,
                pos,
                determinant,
                return_type,
            ))
        }
        K::Inverse => {
            let mut mat = [0.0_f32; 16];
            extract_matrix(&ctx.pool, args[0], &mut mat);
            match ctx.pool.ty(type_of(&ctx.pool, args[0])).slot_count() {
                4 => {
                    let input = first::<4>(&mat);
                    let mut out = [0.0_f32; 4];
                    if invert_2x2_matrix(&input, Some(&mut out)) == 0.0 {
                        return None;
                    }
                    mat[..4].copy_from_slice(&out);
                }
                9 => {
                    let input = first::<9>(&mat);
                    let mut out = [0.0_f32; 9];
                    if invert_3x3_matrix(&input, Some(&mut out)) == 0.0 {
                        return None;
                    }
                    mat[..9].copy_from_slice(&out);
                }
                16 => {
                    let input = mat;
                    if invert_4x4_matrix(&input, Some(&mut mat)) == 0.0 {
                        return None;
                    }
                }
                _ => return None,
            }
            let dmat = mat.map(f64::from);
            Some(ConstructorCompound::make_from_constants(
                ctx,
                pos,
                return_type,
                &dmat,
            ))
        }
        // 8.7 : Vector Relational Functions
        K::LessThan => Some(optimize_comparison(ctx, &arguments, compare_less_than)),
        K::LessThanEqual => Some(optimize_comparison(
            ctx,
            &arguments,
            compare_less_than_equal,
        )),
        K::GreaterThan => Some(optimize_comparison(ctx, &arguments, compare_greater_than)),
        K::GreaterThanEqual => Some(optimize_comparison(
            ctx,
            &arguments,
            compare_greater_than_equal,
        )),
        K::Equal => Some(optimize_comparison(ctx, &arguments, compare_equal)),
        K::NotEqual => Some(optimize_comparison(ctx, &arguments, compare_not_equal)),
        K::Any => coalesce_vector(ctx, &arguments, 0.0, return_type, coalesce_any, None),
        K::All => coalesce_vector(ctx, &arguments, 1.0, return_type, coalesce_all, None),
        K::Not => evaluate_intrinsic(ctx, &arguments, return_type, eval_not),
        _ => None,
    }
}

/// The first `N` elements of `mat`, as an array.
fn first<const N: usize>(mat: &[f32; 16]) -> [f32; N] {
    let mut out = [0.0_f32; N];
    out.copy_from_slice(&mat[..N]);
    out
}

/// `Pack(n)` of the packing intrinsics, for both components: `pack(x)` is the packed value of
/// the `x` component, and the result combines the two as the C++ does.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L700-L719 (chrome/m156)
fn pack_two(ctx: &Context, args: &[ExprId], pack: impl Fn(f32) -> u32) -> f64 {
    let p0 = pack(get(&ctx.pool, args, 0, 0));
    let p1 = pack(get(&ctx.pool, args, 0, 1));
    f64::from((p0 & 0x0000_FFFF) | ((p1 << 16) & 0xFFFF_0000))
}

/// The packing of one component to 16 bits: `(int)std::round(clamp(x, lo, hi) * scale)`, read as
/// an unsigned int.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L703-L707 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ `(int)` of a rounded value.
fn pack_clamped(x: f32, lo: f64, hi: f64, scale: f64) -> u32 {
    let value = eval_clamp(f64::from(x), lo, hi) * scale;
    (value.round() as i32) as u32
}
