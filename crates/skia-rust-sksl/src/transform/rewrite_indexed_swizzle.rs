// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLRewriteIndexedSwizzle.cpp (chrome/m156).

//! [`rewrite_indexed_swizzle`]: `myVec.zyx[i]` as a lookup into a constant vector.

use crate::context::Context;
use crate::ir::{
    ComponentArray, ConstructorCompound, ExprId, ExpressionKind, IndexExpression, TypeId,
};

/// `Transform::RewriteIndexedSwizzle`: rewrites the index expression `index_expr`, whose base is a
/// swizzle, as `base[vec(components...)[index]]`. Returns `None` when the base is not a swizzle.
///
/// # Panics
///
/// If `index_expr` is not an `IndexExpression` (Skia takes an `IndexExpression&`).
// Port of: src/sksl/transform/SkSLRewriteIndexedSwizzle.cpp#L22-L49 (chrome/m156)
#[doc(alias = "SkSL::Transform::RewriteIndexedSwizzle")]
pub fn rewrite_indexed_swizzle(ctx: &mut Context, index_expr: ExprId) -> Option<ExprId> {
    let ExpressionKind::Index(index) = &ctx.pool.expression(index_expr).kind else {
        panic!("RewriteIndexedSwizzle takes an IndexExpression");
    };
    let (base, index_value) = (index.base, index.index);
    let pos = ctx.pool.expression(index_expr).position;

    // The index expression _must_ have a swizzle base for this transformation to be valid.
    let ExpressionKind::Swizzle(swizzle) = &ctx.pool.expression(base).kind else {
        return None;
    };
    let swizzle_base = swizzle.base;
    let components: ComponentArray = swizzle.components;

    // Convert the swizzle components to a literal array.
    let mut vec_array = [0.0_f64; 4];
    for (index, &component) in components.iter().enumerate() {
        vec_array[index] = f64::from(component);
    }

    // Make a compound constructor with the literal array.
    let count = i32::try_from(components.len()).expect("a swizzle has at most four components");
    let vec_type = ctx.pool.ty(TypeId::INT).to_compound(count, 1);
    let vec = ConstructorCompound::make_from_constants(ctx, pos, vec_type, &vec_array);

    // Create a rewritten inner-expression corresponding to `vec(1,2,3)[originalIndex]`.
    let index_copy = ctx.pool.clone_expression(index_value);
    let inner_expr = IndexExpression::make(ctx, pos, vec, index_copy);

    // Return a rewritten outer-expression corresponding to `base[vec(1,2,3)[originalIndex]]`.
    let base_copy = ctx.pool.clone_expression(swizzle_base);
    Some(IndexExpression::make(ctx, pos, base_copy, inner_expr))
}
