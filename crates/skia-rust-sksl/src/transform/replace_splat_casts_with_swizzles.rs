// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLReplaceSplatCastsWithSwizzles.cpp (chrome/m156).

//! [`replace_splat_casts_with_swizzles`]: `float4(myFloat)` becomes `myFloat.xxxx`.

use super::ProgramWriter;
use crate::context::Context;
use crate::ir::{
    ComponentArray, ElemId, ExprId, ExpressionKind, ProgramElementKind, Swizzle, swizzle_component,
};

/// `SwizzleWriter`.
struct SwizzleWriter;

impl ProgramWriter for SwizzleWriter {
    // Port of: src/sksl/transform/SkSLReplaceSplatCastsWithSwizzles.cpp#L32-L52 (chrome/m156)
    fn visit_expression_ptr(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        if self.visit_expression(ctx, expr) {
            return true;
        }
        let argument = match &ctx.pool.expression(expr).kind {
            ExpressionKind::ConstructorSplat(splat) => splat.argument,
            _ => return false,
        };
        // If the argument is a literal, only allow floats. The swizzled-literal syntax only works
        // properly for floats.
        let arg_is_literal = matches!(
            ctx.pool.expression(argument).kind,
            ExpressionKind::Literal(_)
        );
        let arg_type = ctx.pool.ty(ctx.pool.expression(argument).ty);
        if !arg_is_literal || (arg_type.is_float() && arg_type.high_precision()) {
            // Synthesize a splat like `.xxxx`, matching the column count of the splat.
            let columns = ctx.pool.ty(ctx.pool.expression(expr).ty).columns();
            let columns =
                usize::try_from(columns).expect("a splat has a non-negative column count");
            let components = ComponentArray::from_slice(&[swizzle_component::X; 4][..columns]);

            // Replace the splat expression with the swizzle.
            let position = ctx.pool.expression(expr).position;
            let swizzle = Swizzle::make_exact(ctx, position, argument, components);
            ctx.pool.move_expression_into(expr, swizzle);
        }
        false
    }
}

/// `Transform::ReplaceSplatCastsWithSwizzles(const Context&, Module&)`: rewrites the splats in the
/// function bodies of `elements`.
// Port of: src/sksl/transform/SkSLReplaceSplatCastsWithSwizzles.cpp#L27-L64 (chrome/m156)
#[doc(alias = "SkSL::Transform::ReplaceSplatCastsWithSwizzles")]
pub fn replace_splat_casts_with_swizzles(ctx: &mut Context, elements: &[ElemId]) {
    for &element in elements {
        let body = match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => def.body,
            _ => continue,
        };
        SwizzleWriter.visit_statement_ptr(ctx, body);
    }
}
