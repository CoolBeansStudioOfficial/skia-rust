// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLConstantFolder.{h,cpp} (chrome/m156). The `ConstantFolder`
// class is a set of free functions here; `simplify` is `ConstantFolder::Simplify`.
//
//! [`simplify`] and the constant queries of Skia's `ConstantFolder`.

// Skia compares constant slot values with `==` on doubles and converts between the integer, float
// and double types implicitly. The exact comparisons and casts are the arithmetic being ported.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::analysis;
use crate::context::Context;
use crate::defines::SkslInt;
use crate::ir::{
    BinaryExpression, ComparisonResult, ConstructorCompound, ConstructorDiagonalMatrix,
    ConstructorSplat, ExprId, ExpressionKind, IrPool, Literal, PrefixExpression, TypeId,
    VariableRefKind,
};
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;

/// `FLT_MAX`, as the double the C++ compares against.
// Port of: <float.h> FLT_MAX (chrome/m156)
const FLT_MAX: f64 = f32::MAX as f64;

/// A dimension of a `Type` (`columns()`, `rows()`, both `int` in C++), as a `usize`. Dimensions are
/// never negative.
fn dim(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// `bool` to `double`, as C++ converts a comparison result when it is passed as a `double`.
fn bool_to_double(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

/// `ConstantFolder::Simplify`: folds `left op right` into a simpler expression of type
/// `result_type`, or returns `None` when no rule applies. Errors such as division by zero are
/// reported to `ctx.errors`. The inputs are not changed.
///
/// `left` and `right` are the operands as the caller holds them; constant variables are replaced
/// by their values first, as Skia does.
// Port of: src/sksl/SkSLConstantFolder.cpp#L811-L903 (chrome/m156)
#[doc(alias = "ConstantFolder::Simplify")]
pub fn simplify(
    ctx: &mut Context,
    pos: Position,
    left_expr: ExprId,
    op: Operator,
    right_expr: ExprId,
    result_type: TypeId,
) -> Option<ExprId> {
    // Replace constant variables with their literal values.
    let left = get_constant_value_for_variable(&ctx.pool, left_expr);
    let right = get_constant_value_for_variable(&ctx.pool, right_expr);

    // If this is the assignment operator, and both sides are the same trivial expression, this is
    // self-assignment (i.e., `var = var`) and can be reduced to just a variable reference (`var`).
    if op.kind() == OperatorKind::Eq && analysis::is_same_expression_tree(&ctx.pool, left, right) {
        return Some(ctx.pool.clone_expression_at(right, pos));
    }

    // Simplify the expression when both sides are constant Boolean literals.
    if is_bool_literal(&ctx.pool, left) && is_bool_literal(&ctx.pool, right) {
        let left_val = bool_value(&ctx.pool, left);
        let right_val = bool_value(&ctx.pool, right);
        let result = match op.kind() {
            OperatorKind::LogicalAnd => left_val && right_val,
            OperatorKind::LogicalOr => left_val || right_val,
            OperatorKind::LogicalXor => left_val ^ right_val,
            OperatorKind::EqEq => left_val == right_val,
            OperatorKind::Neq => left_val != right_val,
            _ => return None,
        };
        return Some(Literal::make_bool_literal(&mut ctx.pool, pos, result));
    }

    // If the left side is a Boolean literal, apply short-circuit optimizations.
    if is_bool_literal(&ctx.pool, left) {
        return short_circuit_boolean(ctx, pos, left, op, right);
    }

    // If the right side is a Boolean literal...
    if is_bool_literal(&ctx.pool, right) {
        // ... and the left side has no side effects...
        if !analysis::has_side_effects(&ctx.pool, left) {
            // We can reverse the expressions and short-circuit optimizations are still valid.
            return short_circuit_boolean(ctx, pos, right, op, left);
        }
        // We can't use short-circuiting, but we can still optimize away no-op Boolean expressions.
        return eliminate_no_op_boolean(ctx, pos, left, op, right);
    }

    if op.kind() == OperatorKind::EqEq && analysis::is_same_expression_tree(&ctx.pool, left, right)
    {
        // With == comparison, if both sides are the same trivial expression, this is self-
        // comparison and is always true. (We are not concerned with NaN.)
        return Some(Literal::make_bool_literal(&mut ctx.pool, pos, true));
    }

    if op.kind() == OperatorKind::Neq && analysis::is_same_expression_tree(&ctx.pool, left, right) {
        // With != comparison, if both sides are the same trivial expression, this is self-
        // comparison and is always false. (We are not concerned with NaN.)
        return Some(Literal::make_bool_literal(&mut ctx.pool, pos, false));
    }

    if error_on_divide_by_zero(ctx, pos, op, right) {
        return None;
    }

    // Perform full constant folding when both sides are compile-time constants.
    let left_side_is_constant = analysis::is_compile_time_constant(&ctx.pool, left);
    let right_side_is_constant = analysis::is_compile_time_constant(&ctx.pool, right);
    if left_side_is_constant && right_side_is_constant {
        return fold_two_constants(ctx, pos, left, op, right, result_type);
    }

    if ctx.config().settings.optimize {
        // If just one side is constant, we might still be able to simplify arithmetic expressions
        // like `x * 1`, `x *= 1`, `x + 0`, `x * 0`, `0 / x`, etc.
        if (left_side_is_constant || right_side_is_constant)
            && let Some(expr) = simplify_arithmetic(ctx, pos, left, op, right, result_type)
        {
            return Some(expr);
        }

        // We can simplify some forms of matrix division even when neither side is constant.
        if let Some(expr) = simplify_matrix_division(ctx, pos, left, op, right) {
            return Some(expr);
        }
    }

    // We aren't able to constant-fold.
    None
}

/// `ConstantFolder::GetConstantInt`: the integer value of `value` (after constant variables are
/// replaced), or `None` if it is not an integer literal.
// Port of: src/sksl/SkSLConstantFolder.cpp#L326-L333 (chrome/m156)
#[must_use]
#[doc(alias = "ConstantFolder::GetConstantInt")]
pub fn get_constant_int(pool: &IrPool, value: ExprId) -> Option<SkslInt> {
    let expr = get_constant_value_for_variable(pool, value);
    if !pool.expression(expr).is_int_literal(pool) {
        return None;
    }
    pool.expression(expr).as_literal().map(|l| l.int_value())
}

/// `ConstantFolder::GetConstantValue`: the value of `value` (after constant variables are
/// replaced), or `None` if it is not a literal.
// Port of: src/sksl/SkSLConstantFolder.cpp#L335-L342 (chrome/m156)
#[must_use]
#[doc(alias = "ConstantFolder::GetConstantValue")]
pub fn get_constant_value(pool: &IrPool, value: ExprId) -> Option<f64> {
    let expr = get_constant_value_for_variable(pool, value);
    pool.expression(expr).as_literal().map(|l| l.value)
}

/// `ConstantFolder::IsConstantSplat`: whether every slot of `expr` is the constant `value`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L355-L364 (chrome/m156)
#[must_use]
#[doc(alias = "ConstantFolder::IsConstantSplat")]
pub fn is_constant_splat(pool: &IrPool, expr: ExprId, value: f64) -> bool {
    let num_slots = pool.ty(pool.expression(expr).ty).slot_count();
    for index in 0..num_slots {
        // `*slotVal != value` on doubles, as in the C++.
        match pool.expression(expr).get_constant_value(pool, index) {
            Some(slot_val) if slot_val == value => {}
            _ => return false,
        }
    }
    true
}

/// `ConstantFolder::GetConstantValueOrNull`: the constant expression that `in_expr` stands for. A
/// read of a `const` variable with a constant initializer stands for that initializer.
// Port of: src/sksl/SkSLConstantFolder.cpp#L441-L460 (chrome/m156)
#[must_use]
#[doc(alias = "ConstantFolder::GetConstantValueOrNull")]
pub fn get_constant_value_or_null(pool: &IrPool, in_expr: ExprId) -> Option<ExprId> {
    let mut expr = in_expr;
    while let ExpressionKind::VariableReference(var_ref) = &pool.expression(expr).kind {
        if var_ref.ref_kind != VariableRefKind::Read {
            return None;
        }
        let var = pool.variable(var_ref.variable);
        if !var.modifier_flags.is_const() {
            return None;
        }
        // Generally, const variables must have initial values. However, function parameters are
        // an exception; they can be const but won't have an initial value.
        expr = var.initial_value(pool)?;
    }
    analysis::is_compile_time_constant(pool, expr).then_some(expr)
}

/// `ConstantFolder::GetConstantValueForVariable`: the constant value of `in_expr`, or `in_expr`
/// itself when it has none.
// Port of: src/sksl/SkSLConstantFolder.cpp#L462-L465 (chrome/m156)
#[must_use]
#[doc(alias = "ConstantFolder::GetConstantValueForVariable")]
pub fn get_constant_value_for_variable(pool: &IrPool, in_expr: ExprId) -> ExprId {
    get_constant_value_or_null(pool, in_expr).unwrap_or(in_expr)
}

/// `ConstantFolder::MakeConstantValueForVariable`: a copy at `pos` of the constant value of
/// `in_expr`, or `in_expr` itself when it has none.
// Port of: src/sksl/SkSLConstantFolder.cpp#L467-L471 (chrome/m156)
#[doc(alias = "ConstantFolder::MakeConstantValueForVariable")]
pub fn make_constant_value_for_variable(
    ctx: &mut Context,
    pos: Position,
    in_expr: ExprId,
) -> ExprId {
    match get_constant_value_or_null(&ctx.pool, in_expr) {
        Some(expr) => ctx.pool.clone_expression_at(expr, pos),
        None => in_expr,
    }
}
/// `ConstantFolder::error_on_divide_by_zero`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L424-L439 (chrome/m156)
fn error_on_divide_by_zero(ctx: &mut Context, pos: Position, op: Operator, right: ExprId) -> bool {
    match op.kind() {
        OperatorKind::Slash
        | OperatorKind::SlashEq
        | OperatorKind::Percent
        | OperatorKind::PercentEq => {
            if contains_constant_zero(&ctx.pool, right) {
                ctx.errors.error(pos, "division by zero");
                return true;
            }
            false
        }
        _ => false,
    }
}

/// `contains_constant_zero`: some slot of `expr` is a constant zero.
// Port of: src/sksl/SkSLConstantFolder.cpp#L344-L353 (chrome/m156)
fn contains_constant_zero(pool: &IrPool, expr: ExprId) -> bool {
    let num_slots = pool.ty(pool.expression(expr).ty).slot_count();
    (0..num_slots).any(|index| pool.expression(expr).get_constant_value(pool, index) == Some(0.0))
}

/// `is_constant_diagonal`: a square matrix whose diagonal is `value` and whose other slots are 0.
// Port of: src/sksl/SkSLConstantFolder.cpp#L366-L385 (chrome/m156)
fn is_constant_diagonal(pool: &IrPool, expr: ExprId, value: f64) -> bool {
    let ty = pool.ty(pool.expression(expr).ty);
    let (columns, rows) = (dim(ty.columns()), dim(ty.rows()));
    if columns != rows {
        return false;
    }
    // Slots are column-major, so the slot of (c, r) is `c * rows + r`.
    for c in 0..columns {
        for r in 0..rows {
            let expectation = if c == r { value } else { 0.0 };
            if pool.expression(expr).get_constant_value(pool, c * rows + r) != Some(expectation) {
                return false;
            }
        }
    }
    true
}

/// `is_constant_value`: a scalar or vector splat of `value`, or a diagonal matrix of it.
// Port of: src/sksl/SkSLConstantFolder.cpp#L387-L391 (chrome/m156)
fn is_constant_value(pool: &IrPool, expr: ExprId, value: f64) -> bool {
    if pool.ty(pool.expression(expr).ty).is_matrix() {
        is_constant_diagonal(pool, expr, value)
    } else {
        is_constant_splat(pool, expr, value)
    }
}

/// `make_reciprocal_expression`: the reciprocal of a constant right-hand side of a division, if it
/// is safe to multiply by it.
// Port of: src/sksl/SkSLConstantFolder.cpp#L397-L422 (chrome/m156)
fn make_reciprocal_expression(ctx: &mut Context, right: ExprId) -> Option<ExprId> {
    let (is_matrix, is_float, num_slots, pos, ty) = {
        let e = ctx.pool.expression(right);
        let t = ctx.pool.ty(e.ty);
        (
            t.is_matrix(),
            t.component_type().is_float(),
            t.slot_count(),
            e.position,
            e.ty,
        )
    };
    if is_matrix || !is_float {
        return None;
    }
    // Verify that each slot contains a finite, non-zero literal, take its reciprocal.
    let mut values = Vec::with_capacity(num_slots);
    for index in 0..num_slots {
        let value = ctx
            .pool
            .expression(right)
            .get_constant_value(&ctx.pool, index)?;
        let reciprocal = 1.0 / value;
        let is_safe = (-FLT_MAX..=FLT_MAX).contains(&reciprocal) && reciprocal != 0.0;
        if !is_safe {
            // The value is outside the 32-bit float range, or is NaN; do not optimize.
            return None;
        }
        values.push(reciprocal);
    }
    // Turn the expression array into a compound constructor. (If this is a single-slot expression,
    // this will return the literal as-is.)
    Some(ConstructorCompound::make_from_constants(
        ctx, pos, ty, &values,
    ))
}

/// `one_over_scalar`: the expression `1.0 / right` for a scalar `right`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L616-L624 (chrome/m156)
fn one_over_scalar(ctx: &mut Context, right: ExprId) -> ExprId {
    let (pos, ty) = {
        let e = ctx.pool.expression(right);
        (e.position, e.ty)
    };
    let one = Literal::make(&mut ctx.pool, pos, 1.0, ty);
    let divisor = ctx.pool.clone_expression(right);
    BinaryExpression::make(ctx, pos, one, OperatorKind::Slash.into(), divisor)
}

/// `simplify_matrix_division`: `m / s` becomes `m * (1.0 / s)`, which SPIR-V and Metal prefer.
// Port of: src/sksl/SkSLConstantFolder.cpp#L626-L652 (chrome/m156)
fn simplify_matrix_division(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> Option<ExprId> {
    match op.kind() {
        OperatorKind::Slash | OperatorKind::SlashEq => {
            let (left_is_matrix, right_is_scalar) = (
                ctx.pool.ty(ctx.pool.expression(left).ty).is_matrix(),
                ctx.pool.ty(ctx.pool.expression(right).ty).is_scalar(),
            );
            if left_is_matrix && right_is_scalar {
                let multiply_op = if op.is_assignment() {
                    OperatorKind::StarEq
                } else {
                    OperatorKind::Star
                };
                let multiplicand = ctx.pool.clone_expression(left);
                let reciprocal = one_over_scalar(ctx, right);
                return Some(BinaryExpression::make(
                    ctx,
                    pos,
                    multiplicand,
                    multiply_op.into(),
                    reciprocal,
                ));
            }
            None
        }
        _ => None,
    }
}

/// `cast_expression`: `expr` converted to `ty` by a splat, a diagonal matrix, or a plain copy.
// Port of: src/sksl/SkSLConstantFolder.cpp#L280-L298 (chrome/m156)
fn cast_expression(ctx: &mut Context, pos: Position, expr: ExprId, ty: TypeId) -> Option<ExprId> {
    let expr_ty = ctx.pool.expression(expr).ty;
    if ctx.pool.ty(expr_ty).is_scalar() {
        if ctx.pool.ty(ty).is_matrix() {
            let copy = ctx.pool.clone_expression(expr);
            return Some(ConstructorDiagonalMatrix::make(ctx, pos, ty, copy));
        }
        if ctx.pool.ty(ty).is_vector() {
            let copy = ctx.pool.clone_expression(expr);
            return Some(ConstructorSplat::make(ctx, pos, ty, copy));
        }
    }
    if ctx.pool.ty(ty).matches(expr_ty) {
        return Some(ctx.pool.clone_expression_at(expr, pos));
    }
    // We can't cast matrices into vectors or vice-versa.
    None
}

/// `splat_scalar`: a vector or matrix of the scalar `scalar`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L261-L278 (chrome/m156)
fn splat_scalar(ctx: &mut Context, scalar: ExprId, ty: TypeId) -> Option<ExprId> {
    let pos = ctx.pool.expression(scalar).position;
    if ctx.pool.ty(ty).is_vector() {
        let copy = ctx.pool.clone_expression(scalar);
        return Some(ConstructorSplat::make(ctx, pos, ty, copy));
    }
    if ctx.pool.ty(ty).is_matrix() {
        let num_slots = ctx.pool.ty(ty).slot_count();
        let args: Vec<ExprId> = (0..num_slots)
            .map(|_| ctx.pool.clone_expression(scalar))
            .collect();
        return Some(ConstructorCompound::make(ctx, pos, ty, args));
    }
    // Skia: SkDEBUGFAILF("unsupported type"); the caller only passes vectors and matrices.
    None
}

/// `zero_expression`: a zero of type `ty`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L300-L315 (chrome/m156)
fn zero_expression(ctx: &mut Context, pos: Position, ty: TypeId) -> Option<ExprId> {
    let component = ctx.pool.ty(ty).component_type().id();
    let zero = Literal::make(&mut ctx.pool, pos, 0.0, component);
    if ctx.pool.ty(ty).is_scalar() {
        return Some(zero);
    }
    if ctx.pool.ty(ty).is_vector() {
        return Some(ConstructorSplat::make(ctx, pos, ty, zero));
    }
    if ctx.pool.ty(ty).is_matrix() {
        return Some(ConstructorDiagonalMatrix::make(ctx, pos, ty, zero));
    }
    // Skia: SkDEBUGFAILF("unsupported type").
    None
}

/// `negate_expression`: `-expr` converted to `ty`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L317-L324 (chrome/m156)
fn negate_expression(ctx: &mut Context, pos: Position, expr: ExprId, ty: TypeId) -> Option<ExprId> {
    let ctor = cast_expression(ctx, pos, expr, ty)?;
    Some(PrefixExpression::make(
        ctx,
        pos,
        OperatorKind::Minus.into(),
        ctor,
    ))
}
/// `short_circuit_boolean`: `left` is the Boolean literal.
// Port of: src/sksl/SkSLConstantFolder.cpp#L72-L88 (chrome/m156)
fn short_circuit_boolean(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> Option<ExprId> {
    let left_val = bool_value(&ctx.pool, left);

    // When the literal is on the left, we can sometimes eliminate the other expression entirely.
    if (op.kind() == OperatorKind::LogicalAnd && !left_val)
        || (op.kind() == OperatorKind::LogicalOr && left_val)
    {
        // (false && expr) -> (false), (true || expr) -> (true)
        return Some(ctx.pool.clone_expression_at(left, pos));
    }

    // We can't eliminate the right-side expression via short-circuit, but we might still be able to
    // simplify away a no-op expression.
    eliminate_no_op_boolean(ctx, pos, right, op, left)
}

/// `eliminate_no_op_boolean`: `right` is the Boolean literal.
// Port of: src/sksl/SkSLConstantFolder.cpp#L53-L70 (chrome/m156)
fn eliminate_no_op_boolean(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> Option<ExprId> {
    let right_val = bool_value(&ctx.pool, right);

    // Detect no-op Boolean expressions and optimize them away.
    // (expr && true) -> (expr), (expr || false) -> (expr), (expr ^^ false) -> (expr),
    // (expr == true) -> (expr), (expr != false) -> (expr)
    let no_op = match op.kind() {
        OperatorKind::LogicalAnd | OperatorKind::EqEq => right_val,
        OperatorKind::LogicalOr | OperatorKind::LogicalXor | OperatorKind::Neq => !right_val,
        _ => false,
    };
    if no_op {
        return Some(ctx.pool.clone_expression_at(left, pos));
    }
    None
}

/// `simplify_constant_equality`: `==` and `!=` of two constants.
// Port of: src/sksl/SkSLConstantFolder.cpp#L90-L111 (chrome/m156)
fn simplify_constant_equality(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> Option<ExprId> {
    if op.kind() == OperatorKind::EqEq || op.kind() == OperatorKind::Neq {
        let mut equality = op.kind() == OperatorKind::EqEq;
        match ctx
            .pool
            .expression(left)
            .compare_constant(&ctx.pool, ctx.pool.expression(right))
        {
            ComparisonResult::NotEqual => {
                equality = !equality;
                return Some(Literal::make_bool_literal(&mut ctx.pool, pos, equality));
            }
            ComparisonResult::Equal => {
                return Some(Literal::make_bool_literal(&mut ctx.pool, pos, equality));
            }
            ComparisonResult::Unknown => {}
        }
    }
    None
}

/// `simplify_matrix_multiplication`: the product of two constant matrices or vectors.
// Port of: src/sksl/SkSLConstantFolder.cpp#L113-L169 (chrome/m156)
// The loops index the fixed-size `[[f64; 4]; 4]` buffers by column and row, as Skia's do.
#[allow(clippy::needless_range_loop)]
fn simplify_matrix_multiplication(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    right: ExprId,
    dims: MatrixDims,
) -> Option<ExprId> {
    let left_ty = ctx.pool.expression(left).ty;
    let component_type = ctx.pool.ty(left_ty).component_type().id();

    // Fetch the left matrix.
    let mut left_vals = [[0.0_f64; 4]; 4];
    for c in 0..dims.left_columns {
        for r in 0..dims.left_rows {
            left_vals[c][r] = ctx
                .pool
                .expression(left)
                .get_constant_value(&ctx.pool, (c * dims.left_rows) + r)?;
        }
    }
    // Fetch the right matrix.
    let mut right_vals = [[0.0_f64; 4]; 4];
    for c in 0..dims.right_columns {
        for r in 0..dims.right_rows {
            right_vals[c][r] = ctx
                .pool
                .expression(right)
                .get_constant_value(&ctx.pool, (c * dims.right_rows) + r)?;
        }
    }

    let mut out_columns = dims.right_columns;
    let mut out_rows = dims.left_rows;

    let mut args = [0.0_f64; 16];
    let mut arg_index = 0;
    for c in 0..out_columns {
        for r in 0..out_rows {
            // Compute a dot product for this position.
            let mut val = 0.0_f64;
            for dot_idx in 0..dims.left_columns {
                val += left_vals[dot_idx][r] * right_vals[c][dot_idx];
            }

            let in_range = (-FLT_MAX..=FLT_MAX).contains(&val);
            if in_range {
                args[arg_index] = val;
                arg_index += 1;
            } else {
                // The value is outside the 32-bit float range, or is NaN; do not optimize.
                return None;
            }
        }
    }

    if out_columns == 1 {
        // Matrix-times-vector conceptually makes a 1-column N-row matrix, but we return vecN.
        std::mem::swap(&mut out_columns, &mut out_rows);
    }

    let result_type = ctx.pool.ty(component_type).to_compound(
        i32::try_from(out_columns).unwrap_or(0),
        i32::try_from(out_rows).unwrap_or(0),
    );
    Some(ConstructorCompound::make_from_constants(
        ctx,
        pos,
        result_type,
        &args[..arg_index],
    ))
}

/// The dimensions of a matrix product's operands (`leftColumns`, `leftRows`, ...).
#[derive(Clone, Copy)]
struct MatrixDims {
    left_columns: usize,
    left_rows: usize,
    right_columns: usize,
    right_rows: usize,
}

/// `simplify_matrix_times_matrix`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L171-L184 (chrome/m156)
fn simplify_matrix_times_matrix(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    right: ExprId,
) -> Option<ExprId> {
    let (lt, rt) = (
        ctx.pool.ty(ctx.pool.expression(left).ty),
        ctx.pool.ty(ctx.pool.expression(right).ty),
    );
    let dims = MatrixDims {
        left_columns: dim(lt.columns()),
        left_rows: dim(lt.rows()),
        right_columns: dim(rt.columns()),
        right_rows: dim(rt.rows()),
    };
    simplify_matrix_multiplication(ctx, pos, left, right, dims)
}

/// `simplify_vector_times_matrix`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L186-L199 (chrome/m156)
fn simplify_vector_times_matrix(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    right: ExprId,
) -> Option<ExprId> {
    let (lt, rt) = (
        ctx.pool.ty(ctx.pool.expression(left).ty),
        ctx.pool.ty(ctx.pool.expression(right).ty),
    );
    let dims = MatrixDims {
        left_columns: dim(lt.columns()),
        left_rows: 1,
        right_columns: dim(rt.columns()),
        right_rows: dim(rt.rows()),
    };
    simplify_matrix_multiplication(ctx, pos, left, right, dims)
}

/// `simplify_matrix_times_vector`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L201-L214 (chrome/m156)
fn simplify_matrix_times_vector(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    right: ExprId,
) -> Option<ExprId> {
    let (lt, rt) = (
        ctx.pool.ty(ctx.pool.expression(left).ty),
        ctx.pool.ty(ctx.pool.expression(right).ty),
    );
    let dims = MatrixDims {
        left_columns: dim(lt.columns()),
        left_rows: dim(lt.rows()),
        right_columns: 1,
        right_rows: dim(rt.columns()),
    };
    simplify_matrix_multiplication(ctx, pos, left, right, dims)
}

/// `simplify_componentwise`: `+ - * /` applied slot by slot to two constant vectors or matrices.
// Port of: src/sksl/SkSLConstantFolder.cpp#L216-L259 (chrome/m156)
fn simplify_componentwise(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> Option<ExprId> {
    // Handle equality operations: == !=
    if let Some(result) = simplify_constant_equality(ctx, pos, left, op, right) {
        return Some(result);
    }

    // Handle floating-point arithmetic: + - * /
    let fold_fn: fn(f64, f64) -> f64 = match op.kind() {
        OperatorKind::Plus => |a, b| a + b,
        OperatorKind::Minus => |a, b| a - b,
        OperatorKind::Star => |a, b| a * b,
        OperatorKind::Slash => |a, b| a / b,
        _ => return None,
    };

    let ty = ctx.pool.expression(left).ty;
    let (minimum_value, maximum_value, num_slots) = {
        let t = ctx.pool.ty(ty);
        let component = t.component_type();
        (
            component.minimum_value(),
            component.maximum_value(),
            t.slot_count(),
        )
    };

    let mut args = vec![0.0_f64; num_slots];
    for (i, arg) in args.iter_mut().enumerate() {
        let left_value = ctx.pool.expression(left).get_constant_value(&ctx.pool, i)?;
        let right_value = ctx
            .pool
            .expression(right)
            .get_constant_value(&ctx.pool, i)?;
        let value = fold_fn(left_value, right_value);
        // NaN passes both comparisons, as it does in the C++.
        if value < minimum_value || value > maximum_value {
            return None;
        }
        *arg = value;
    }
    Some(ConstructorCompound::make_from_constants(
        ctx, pos, ty, &args,
    ))
}

/// `fold_expression`: a literal of `result_type` holding `result`, if it fits the type.
// Port of: src/sksl/SkSLConstantFolder.cpp#L654-L667 (chrome/m156)
fn fold_expression(
    ctx: &mut Context,
    pos: Position,
    result: f64,
    result_type: TypeId,
) -> Option<ExprId> {
    let (is_number, minimum, maximum) = {
        let t = ctx.pool.ty(result_type);
        (t.is_number(), t.minimum_value(), t.maximum_value())
    };
    if is_number && !(result >= minimum && result <= maximum) {
        // The value is outside the range or is NaN (all if-checks fail); do not optimize.
        return None;
    }
    Some(Literal::make(&mut ctx.pool, pos, result, result_type))
}

/// `fold_two_constants`: folds `left op right` when both are compile-time constants.
// Port of: src/sksl/SkSLConstantFolder.cpp#L669-L809 (chrome/m156)
// Skia's `fold_two_constants` is one function of this length, and its cases run in its order.
#[allow(clippy::too_many_lines)]
fn fold_two_constants(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
    result_type: TypeId,
) -> Option<ExprId> {
    let (left_ty, right_ty) = (ctx.pool.expression(left).ty, ctx.pool.expression(right).ty);
    let (left_is_int, right_is_int) = (
        ctx.pool.expression(left).is_int_literal(&ctx.pool),
        ctx.pool.expression(right).is_int_literal(&ctx.pool),
    );
    let (left_is_float, right_is_float) = (
        ctx.pool.expression(left).is_float_literal(&ctx.pool),
        ctx.pool.expression(right).is_float_literal(&ctx.pool),
    );

    // Handle pairs of integer literals.
    if left_is_int && right_is_int {
        let left_val: SkslInt = int_literal_value(&ctx.pool, left);
        let right_val: SkslInt = int_literal_value(&ctx.pool, right);

        // Unsigned wrapping arithmetic on the bit patterns, as the C++ `URESULT` does.
        let unsigned =
            |f: fn(u64, u64) -> u64| -> f64 { f(left_val as u64, right_val as u64) as i64 as f64 };

        return match op.kind() {
            OperatorKind::Plus => {
                fold_expression(ctx, pos, unsigned(u64::wrapping_add), result_type)
            }
            OperatorKind::Minus => {
                fold_expression(ctx, pos, unsigned(u64::wrapping_sub), result_type)
            }
            OperatorKind::Star => {
                fold_expression(ctx, pos, unsigned(u64::wrapping_mul), result_type)
            }
            OperatorKind::Slash => {
                if left_val == SkslInt::MIN && right_val == -1 {
                    ctx.errors.error(pos, "arithmetic overflow");
                    return None;
                }
                let result = (left_val / right_val) as f64;
                fold_expression(ctx, pos, result, result_type)
            }
            OperatorKind::Percent => {
                if left_val == SkslInt::MIN && right_val == -1 {
                    ctx.errors.error(pos, "arithmetic overflow");
                    return None;
                }
                let result = (left_val % right_val) as f64;
                fold_expression(ctx, pos, result, result_type)
            }
            OperatorKind::BitwiseAnd => {
                let result = (left_val & right_val) as f64;
                fold_expression(ctx, pos, result, result_type)
            }
            OperatorKind::BitwiseOr => {
                let result = (left_val | right_val) as f64;
                fold_expression(ctx, pos, result, result_type)
            }
            OperatorKind::BitwiseXor => {
                let result = (left_val ^ right_val) as f64;
                fold_expression(ctx, pos, result, result_type)
            }
            OperatorKind::EqEq => {
                fold_expression(ctx, pos, bool_to_double(left_val == right_val), result_type)
            }
            OperatorKind::Neq => {
                fold_expression(ctx, pos, bool_to_double(left_val != right_val), result_type)
            }
            OperatorKind::Gt => {
                fold_expression(ctx, pos, bool_to_double(left_val > right_val), result_type)
            }
            OperatorKind::GtEq => {
                fold_expression(ctx, pos, bool_to_double(left_val >= right_val), result_type)
            }
            OperatorKind::Lt => {
                fold_expression(ctx, pos, bool_to_double(left_val < right_val), result_type)
            }
            OperatorKind::LtEq => {
                fold_expression(ctx, pos, bool_to_double(left_val <= right_val), result_type)
            }
            OperatorKind::Shl => {
                if (0..=31).contains(&right_val) {
                    // Left-shifting a negative value is undefined in C++, but not in GLSL, so shift
                    // the unsigned bit pattern.
                    fold_expression(
                        ctx,
                        pos,
                        unsigned(|a, b| a.wrapping_shl(u32::try_from(b).unwrap_or(0))),
                        result_type,
                    )
                } else {
                    ctx.errors.error(pos, "shift value out of range");
                    None
                }
            }
            OperatorKind::Shr => {
                if (0..=31).contains(&right_val) {
                    let result = (left_val >> right_val) as f64;
                    fold_expression(ctx, pos, result, result_type)
                } else {
                    ctx.errors.error(pos, "shift value out of range");
                    None
                }
            }
            _ => None,
        };
    }

    // Handle pairs of floating-point literals. These are computed in `float`, as in the C++.
    if left_is_float && right_is_float {
        let left_val = float_literal_value(&ctx.pool, left);
        let right_val = float_literal_value(&ctx.pool, right);
        return match op.kind() {
            OperatorKind::Plus => {
                fold_expression(ctx, pos, f64::from(left_val + right_val), result_type)
            }
            OperatorKind::Minus => {
                fold_expression(ctx, pos, f64::from(left_val - right_val), result_type)
            }
            OperatorKind::Star => {
                fold_expression(ctx, pos, f64::from(left_val * right_val), result_type)
            }
            OperatorKind::Slash => {
                fold_expression(ctx, pos, f64::from(left_val / right_val), result_type)
            }
            OperatorKind::EqEq => {
                fold_expression(ctx, pos, bool_to_double(left_val == right_val), result_type)
            }
            OperatorKind::Neq => {
                fold_expression(ctx, pos, bool_to_double(left_val != right_val), result_type)
            }
            OperatorKind::Gt => {
                fold_expression(ctx, pos, bool_to_double(left_val > right_val), result_type)
            }
            OperatorKind::GtEq => {
                fold_expression(ctx, pos, bool_to_double(left_val >= right_val), result_type)
            }
            OperatorKind::Lt => {
                fold_expression(ctx, pos, bool_to_double(left_val < right_val), result_type)
            }
            OperatorKind::LtEq => {
                fold_expression(ctx, pos, bool_to_double(left_val <= right_val), result_type)
            }
            _ => None,
        };
    }

    let (left_t, right_t) = (ctx.pool.ty(left_ty), ctx.pool.ty(right_ty));
    let (left_is_matrix, right_is_matrix) = (left_t.is_matrix(), right_t.is_matrix());
    let (left_is_vector, right_is_vector) = (left_t.is_vector(), right_t.is_vector());
    let (left_is_scalar, right_is_scalar) = (left_t.is_scalar(), right_t.is_scalar());
    let (left_is_array, right_is_array) = (left_t.is_array(), right_t.is_array());
    let (left_is_struct, right_is_struct) = (left_t.is_struct(), right_t.is_struct());
    let left_component_matches_right = left_t.component_type().matches(right_ty);
    let right_component_matches_left = right_t.component_type().matches(left_ty);
    let left_matches_right = left_t.matches(right_ty);

    // Perform matrix multiplication.
    if op.kind() == OperatorKind::Star {
        if left_is_matrix && right_is_matrix {
            return simplify_matrix_times_matrix(ctx, pos, left, right);
        }
        if left_is_vector && right_is_matrix {
            return simplify_vector_times_matrix(ctx, pos, left, right);
        }
        if left_is_matrix && right_is_vector {
            return simplify_matrix_times_vector(ctx, pos, left, right);
        }
    }

    // Perform constant folding on pairs of vectors/matrices.
    let left_is_vec_or_mat = left_is_vector || left_is_matrix;
    let right_is_vec_or_mat = right_is_vector || right_is_matrix;
    if left_is_vec_or_mat && left_matches_right {
        return simplify_componentwise(ctx, pos, left, op, right);
    }

    // Perform constant folding on vectors/matrices against scalars, e.g.: half4(2) + 2
    if right_is_scalar && left_is_vec_or_mat && left_component_matches_right {
        let splat = splat_scalar(ctx, right, left_ty)?;
        return simplify_componentwise(ctx, pos, left, op, splat);
    }

    // Perform constant folding on scalars against vectors/matrices, e.g.: 2 + half4(2)
    if left_is_scalar && right_is_vec_or_mat && right_component_matches_left {
        let splat = splat_scalar(ctx, left, right_ty)?;
        return simplify_componentwise(ctx, pos, splat, op, right);
    }

    // Perform constant folding on pairs of matrices, arrays or structs.
    if (left_is_matrix && right_is_matrix)
        || (left_is_array && right_is_array)
        || (left_is_struct && right_is_struct)
    {
        return simplify_constant_equality(ctx, pos, left, op, right);
    }

    // We aren't able to constant-fold these expressions.
    None
}

/// `simplify_arithmetic`: rules for an operation with one constant operand, such as `x * 1`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L481-L611 (chrome/m156)
// Skia's `simplify_arithmetic` is one switch of this length; its cases run in its order.
#[allow(clippy::too_many_lines)]
fn simplify_arithmetic(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
    result_type: TypeId,
) -> Option<ExprId> {
    let (left_is_scalar_op_matrix, left_is_matrix_op_scalar) = (
        is_scalar_op_matrix(&ctx.pool, left, right),
        is_scalar_op_matrix(&ctx.pool, right, left),
    );
    match op.kind() {
        OperatorKind::Plus => {
            if !left_is_scalar_op_matrix && is_constant_splat(&ctx.pool, right, 0.0) {
                // x + 0
                if let Some(expr) = cast_expression(ctx, pos, left, result_type) {
                    return Some(expr);
                }
            }
            if !left_is_matrix_op_scalar && is_constant_splat(&ctx.pool, left, 0.0) {
                // 0 + x
                if let Some(expr) = cast_expression(ctx, pos, right, result_type) {
                    return Some(expr);
                }
            }
        }

        OperatorKind::Star => {
            if is_constant_value(&ctx.pool, right, 1.0) {
                // x * 1
                if let Some(expr) = cast_expression(ctx, pos, left, result_type) {
                    return Some(expr);
                }
            }
            if is_constant_value(&ctx.pool, left, 1.0) {
                // 1 * x
                if let Some(expr) = cast_expression(ctx, pos, right, result_type) {
                    return Some(expr);
                }
            }
            if is_constant_value(&ctx.pool, right, 0.0)
                && !analysis::has_side_effects(&ctx.pool, left)
            {
                // x * 0
                return zero_expression(ctx, pos, result_type);
            }
            if is_constant_value(&ctx.pool, left, 0.0)
                && !analysis::has_side_effects(&ctx.pool, right)
            {
                // 0 * x
                return zero_expression(ctx, pos, result_type);
            }
            if is_constant_value(&ctx.pool, right, -1.0) {
                // x * -1 (to `-x`)
                if let Some(expr) = negate_expression(ctx, pos, left, result_type) {
                    return Some(expr);
                }
            }
            if is_constant_value(&ctx.pool, left, -1.0) {
                // -1 * x (to `-x`)
                if let Some(expr) = negate_expression(ctx, pos, right, result_type) {
                    return Some(expr);
                }
            }
        }

        OperatorKind::Minus => {
            if !left_is_scalar_op_matrix && is_constant_splat(&ctx.pool, right, 0.0) {
                // x - 0
                if let Some(expr) = cast_expression(ctx, pos, left, result_type) {
                    return Some(expr);
                }
            }
            if !left_is_matrix_op_scalar && is_constant_splat(&ctx.pool, left, 0.0) {
                // 0 - x
                if let Some(expr) = negate_expression(ctx, pos, right, result_type) {
                    return Some(expr);
                }
            }
        }

        OperatorKind::Slash => {
            if !left_is_scalar_op_matrix && is_constant_splat(&ctx.pool, right, 1.0) {
                // x / 1
                if let Some(expr) = cast_expression(ctx, pos, left, result_type) {
                    return Some(expr);
                }
            }
            if !ctx.pool.ty(ctx.pool.expression(left).ty).is_matrix() {
                // convert `x / 2` into `x * 0.5`
                if let Some(reciprocal) = make_reciprocal_expression(ctx, right) {
                    let multiplicand = ctx.pool.clone_expression(left);
                    return Some(BinaryExpression::make(
                        ctx,
                        pos,
                        multiplicand,
                        OperatorKind::Star.into(),
                        reciprocal,
                    ));
                }
            }
        }

        OperatorKind::PlusEq | OperatorKind::MinusEq => {
            // x += 0, x -= 0
            if is_constant_splat(&ctx.pool, right, 0.0)
                && let Some(var) = cast_expression(ctx, pos, left, result_type)
            {
                analysis::update_variable_ref_kind(&mut ctx.pool, var, VariableRefKind::Read, None);
                return Some(var);
            }
        }

        OperatorKind::StarEq => {
            // x *= 1
            if is_constant_value(&ctx.pool, right, 1.0)
                && let Some(var) = cast_expression(ctx, pos, left, result_type)
            {
                analysis::update_variable_ref_kind(&mut ctx.pool, var, VariableRefKind::Read, None);
                return Some(var);
            }
        }

        OperatorKind::SlashEq => {
            // x /= 1
            if is_constant_splat(&ctx.pool, right, 1.0)
                && let Some(var) = cast_expression(ctx, pos, left, result_type)
            {
                analysis::update_variable_ref_kind(&mut ctx.pool, var, VariableRefKind::Read, None);
                return Some(var);
            }
            if let Some(reciprocal) = make_reciprocal_expression(ctx, right) {
                let multiplicand = ctx.pool.clone_expression(left);
                return Some(BinaryExpression::make(
                    ctx,
                    pos,
                    multiplicand,
                    OperatorKind::StarEq.into(),
                    reciprocal,
                ));
            }
        }

        _ => {}
    }

    None
}

/// `is_scalar_op_matrix(left, right)`: a scalar on the left and a matrix on the right.
// Port of: src/sksl/SkSLConstantFolder.cpp#L473-L479 (chrome/m156)
fn is_scalar_op_matrix(pool: &IrPool, left: ExprId, right: ExprId) -> bool {
    pool.ty(pool.expression(left).ty).is_scalar() && pool.ty(pool.expression(right).ty).is_matrix()
}

// ---------------------------------------------------------------------------------------------
// Small accessors on literal nodes.

fn is_bool_literal(pool: &IrPool, expr: ExprId) -> bool {
    pool.expression(expr).is_bool_literal(pool)
}

/// `Literal::boolValue()` of a literal expression.
fn bool_value(pool: &IrPool, expr: ExprId) -> bool {
    pool.expression(expr)
        .as_literal()
        .is_some_and(|l| l.bool_value())
}

/// `Literal::intValue()` of an integer literal expression.
fn int_literal_value(pool: &IrPool, expr: ExprId) -> SkslInt {
    pool.expression(expr)
        .as_literal()
        .map_or(0, |l| l.int_value())
}

/// `Literal::floatValue()` of a float literal expression.
fn float_literal_value(pool: &IrPool, expr: ExprId) -> f32 {
    pool.expression(expr)
        .as_literal()
        .map_or(0.0, |l| l.float_value())
}

#[cfg(test)]
mod tests;
