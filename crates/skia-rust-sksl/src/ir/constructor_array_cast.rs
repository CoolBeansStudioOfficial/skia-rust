// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorArrayCast.cpp (`ConstructorArrayCast::Make` and
// `cast_constant_array`).

//! [`ConstructorArrayCast`]'s factory.

use super::constructor::set_position;
use super::{
    ConstructorArray, ConstructorArrayCast, ConstructorCompoundCast, ConstructorScalarCast,
    Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;
use crate::{analysis, constant_folder};

impl ConstructorArrayCast {
    /// `ConstructorArrayCast::Make`: an array cast of `arg` to `ty`. A no-op cast returns `arg`,
    /// and a cast of a compile-time constant is folded into a new array constructor.
    // Port of: src/sksl/ir/SkSLConstructorArrayCast.cpp#L46-L70 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_array());
        debug_assert_eq!(
            ctx.pool.ty(ty).columns(),
            ctx.pool.ty(ctx.pool.expression(arg).ty).columns()
        );

        // If this is a no-op cast, return the expression as-is.
        let arg_ty = ctx.pool.expression(arg).ty;
        if ctx.pool.ty(ty).matches(arg_ty) {
            set_position(&mut ctx.pool, arg, pos);
            return arg;
        }

        // Look up the value of constant variables. This allows constant-expressions like
        // `myArray` to be replaced with the compile-time constant `int[2](0, 1)`.
        let arg = constant_folder::make_constant_value_for_variable(ctx, pos, arg);

        // We can cast a vector of compile-time constants at compile-time.
        if analysis::is_compile_time_constant(&ctx.pool, arg) {
            return cast_constant_array(ctx, pos, ty, arg);
        }
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorArrayCast(ConstructorArrayCast { argument: arg }),
        ))
    }
}

/// `cast_constant_array`: casts each element of a constant array constructor, and builds the
/// result as a new array constructor.
// Port of: src/sksl/ir/SkSLConstructorArrayCast.cpp#L22-L44 (chrome/m156)
fn cast_constant_array(
    ctx: &mut Context,
    pos: Position,
    dest_type: TypeId,
    const_ctor: ExprId,
) -> ExprId {
    let scalar_type = ctx.pool.ty(dest_type).component_type().id();

    // Create a ConstructorArray(...) which typecasts each argument inside.
    let input_args = match &ctx.pool.expression(const_ctor).kind {
        ExpressionKind::ConstructorArray(c) => c.arguments.clone(),
        _ => unreachable!("a compile-time constant array is a ConstructorArray"),
    };
    let mut typecast_args = Vec::with_capacity(input_args.len());
    for arg in input_args {
        let arg_pos = ctx.pool.expression(arg).position;
        let arg_is_scalar = ctx.pool.ty(ctx.pool.expression(arg).ty).is_scalar();
        typecast_args.push(if arg_is_scalar {
            ConstructorScalarCast::make(ctx, arg_pos, scalar_type, arg)
        } else {
            ConstructorCompoundCast::make(ctx, arg_pos, scalar_type, arg)
        });
    }

    ConstructorArray::make(ctx, pos, dest_type, typecast_args)
}
