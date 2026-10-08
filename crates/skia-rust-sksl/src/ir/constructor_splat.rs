// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorSplat.cpp (`ConstructorSplat::Make`). Its
// `getConstantValue` is in `constructor.rs`, with the other constant values.

//! [`ConstructorSplat`]'s factory.

use super::constructor::set_position;
use super::{
    ConstructorSplat, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::constant_folder;
use crate::context::Context;
use crate::position::Position;

impl ConstructorSplat {
    /// `ConstructorSplat::Make`: a vector of `ty` with `arg` in every slot. A splat to a scalar
    /// type is just its argument.
    // Port of: src/sksl/ir/SkSLConstructorSplat.cpp#L14-L36 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_scalar() || ctx.pool.ty(ty).is_vector());
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert!(ctx.pool.ty(ctx.pool.expression(arg).ty).is_scalar());

        // A "splat" to a scalar type is a no-op and can be eliminated.
        if ctx.pool.ty(ty).is_scalar() {
            set_position(&mut ctx.pool, arg, pos);
            return arg;
        }

        // Replace constant variables with their corresponding values, so `float3(five)` can
        // compile down to `float3(5.0)` (the latter is a compile-time constant).
        let arg = constant_folder::make_constant_value_for_variable(ctx, pos, arg);

        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorSplat(ConstructorSplat { argument: arg }),
        ))
    }
}
