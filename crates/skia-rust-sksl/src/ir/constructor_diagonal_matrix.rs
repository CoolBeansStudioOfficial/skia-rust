// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorDiagonalMatrix.cpp (`ConstructorDiagonalMatrix::Make`).
// Its `getConstantValue` is in `constructor.rs`, with the other constant values.

//! [`ConstructorDiagonalMatrix`]'s factory.

use super::{
    ConstructorDiagonalMatrix, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::constant_folder;
use crate::context::Context;
use crate::position::Position;

impl ConstructorDiagonalMatrix {
    /// `ConstructorDiagonalMatrix::Make`: a matrix of `ty` with `arg` on its diagonal.
    // Port of: src/sksl/ir/SkSLConstructorDiagonalMatrix.cpp#L16-L30 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_matrix());
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert!(ctx.pool.ty(ctx.pool.expression(arg).ty).is_scalar());

        // Look up the value of constant variables. This allows constant-expressions like
        // `mat4(five)` to be replaced with `mat4(5.0)`.
        let arg = constant_folder::make_constant_value_for_variable(ctx, pos, arg);

        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument: arg }),
        ))
    }
}
