// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorMatrixResize.cpp (`ConstructorMatrixResize::Make`).
// Its `getConstantValue` is in `constructor.rs`, with the other constant values.

//! [`ConstructorMatrixResize`]'s factory.

use super::{
    ConstructorMatrixResize, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;

impl ConstructorMatrixResize {
    /// `ConstructorMatrixResize::Make`: a matrix of `ty` built from the matrix `arg`. A matrix of
    /// the same shape is returned unchanged.
    // Port of: src/sksl/ir/SkSLConstructorMatrixResize.cpp#L15-L28 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_matrix());
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        let arg_ty = ctx.pool.expression(arg).ty;
        debug_assert!(
            ctx.pool
                .ty(arg_ty)
                .component_type()
                .matches(ctx.pool.ty(ty).component_type().id())
        );

        // If the matrix isn't actually changing size, return it as-is.
        if ctx.pool.ty(ty).rows() == ctx.pool.ty(arg_ty).rows()
            && ctx.pool.ty(ty).columns() == ctx.pool.ty(arg_ty).columns()
        {
            return arg;
        }

        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorMatrixResize(ConstructorMatrixResize { argument: arg }),
        ))
    }
}
