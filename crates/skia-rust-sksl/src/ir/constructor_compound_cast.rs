// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorCompoundCast.cpp (`ConstructorCompoundCast::Make`
// and `cast_constant_composite`).

//! [`ConstructorCompoundCast`]'s factory.

use super::constructor::set_position;
use super::{
    ConstructorCompound, ConstructorCompoundCast, ConstructorDiagonalMatrix, ConstructorScalarCast,
    ConstructorSplat, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;
use crate::{analysis, constant_folder};

impl ConstructorCompoundCast {
    /// `ConstructorCompoundCast::Make`: a vector or matrix cast of `arg` to `ty`. A no-op cast
    /// returns `arg`, and a cast of a compile-time constant is folded.
    // Port of: src/sksl/ir/SkSLConstructorCompoundCast.cpp#L72-L98 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_vector() || ctx.pool.ty(ty).is_matrix());
        debug_assert_eq!(
            ctx.pool.ty(ty).columns(),
            ctx.pool.ty(ctx.pool.expression(arg).ty).columns()
        );
        debug_assert_eq!(
            ctx.pool.ty(ty).rows(),
            ctx.pool.ty(ctx.pool.expression(arg).ty).rows()
        );

        // If this is a no-op cast, return the expression as-is.
        let arg_ty = ctx.pool.expression(arg).ty;
        if ctx.pool.ty(ty).matches(arg_ty) {
            set_position(&mut ctx.pool, arg, pos);
            return arg;
        }
        // Look up the value of constant variables. This allows constant-expressions like
        // `int4(colorGreen)` to be replaced with the compile-time constant `int4(0, 1, 0, 1)`.
        let arg = constant_folder::make_constant_value_for_variable(ctx, pos, arg);

        // We can cast a vector of compile-time constants at compile-time.
        if analysis::is_compile_time_constant(&ctx.pool, arg) {
            return cast_constant_composite(ctx, pos, ty, arg);
        }
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorCompoundCast(ConstructorCompoundCast { argument: arg }),
        ))
    }
}

/// `cast_constant_composite`: casts the slots of a constant vector or matrix, folding the cast
/// into a splat, a diagonal matrix or a compound of literals.
// Port of: src/sksl/ir/SkSLConstructorCompoundCast.cpp#L25-L70 (chrome/m156)
fn cast_constant_composite(
    ctx: &mut Context,
    pos: Position,
    dest_type: TypeId,
    const_ctor: ExprId,
) -> ExprId {
    let scalar_type = ctx.pool.ty(dest_type).component_type().id();

    // We generate nicer code for splats and diagonal matrices by handling them separately instead
    // of relying on the constant-subexpression code below. This is not truly necessary but it
    // makes our output look a little better; human beings prefer `half4(0)` to `half4(0, 0, 0, 0)`.
    let (splat_arg, diagonal_arg) = match &ctx.pool.expression(const_ctor).kind {
        // This is a typecast of a splat containing a constant value, e.g. `half4(7)`. We can
        // replace it with a splat of a different type, e.g. `int4(7)`.
        ExpressionKind::ConstructorSplat(ConstructorSplat { argument }) => (Some(*argument), None),
        // This is a typecast of a constant diagonal matrix, e.g. `float3x3(2)`. We can replace it
        // with a diagonal matrix of a different type, e.g. `half3x3(2)`.
        ExpressionKind::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument })
            if ctx.pool.ty(dest_type).is_matrix() =>
        {
            (None, Some(*argument))
        }
        _ => (None, None),
    };
    if let Some(argument) = splat_arg {
        let cast = ConstructorScalarCast::make(ctx, pos, scalar_type, argument);
        return ConstructorSplat::make(ctx, pos, dest_type, cast);
    }
    if let Some(argument) = diagonal_arg {
        let cast = ConstructorScalarCast::make(ctx, pos, scalar_type, argument);
        return ConstructorDiagonalMatrix::make(ctx, pos, dest_type, cast);
    }

    // Create a compound Constructor(literal, ...) which typecasts each scalar value inside.
    let num_slots = ctx.pool.ty(dest_type).slot_count();
    debug_assert_eq!(
        num_slots,
        ctx.pool.ty(ctx.pool.expression(const_ctor).ty).slot_count()
    );

    let const_pos = ctx.pool.expression(const_ctor).position;
    let mut typecast_args = Vec::with_capacity(num_slots);
    for index in 0..num_slots {
        let mut slot_val = ctx
            .pool
            .expression(const_ctor)
            .get_constant_value(&ctx.pool, index)
            .expect("a compile-time constant has a value in every slot");
        if scalar_type.check_for_out_of_range_literal_value(ctx, slot_val, const_pos) {
            // We've reported an error because the literal is out of range for this type. Zero out
            // the value to avoid a cascade of errors.
            slot_val = 0.0;
        }
        typecast_args.push(slot_val);
    }

    ConstructorCompound::make_from_constants(ctx, pos, dest_type, &typecast_args)
}
