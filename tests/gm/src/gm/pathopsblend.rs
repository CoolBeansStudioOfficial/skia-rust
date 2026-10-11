// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathopsblend.cpp (chrome/m156)

// Mirrors the C++ int/scalar casts and sizes of the GM: the values are small constants.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use crate::tool_utils::draw_checkerboard;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::op as pathops_op;
use skia_rust_pathops::path_op::PathOp;

// Port of: gm/pathopsblend.cpp#L11-L17 (chrome/m156), cross
fn cross() -> Path {
    let mut builder = PathBuilder::new();
    builder.add_rect(Rect::from_ltrb(15.0, 0.0, 35.0, 50.0), None, None);
    builder.add_rect(Rect::from_ltrb(0.0, 15.0, 50.0, 35.0), None, None);
    builder.detach()
}

// Port of: gm/pathopsblend.cpp#L18 (chrome/m156), circle
fn circle() -> Path {
    Path::circle((25.0, 25.0), 20.0, None)
}

// We implement every op except ReverseDifference: That one can be handled by swapping the paths
// and using the Difference logic.
// Port of: gm/pathopsblend.cpp#L21-L26 (chrome/m156), kOps
const K_OPS: [PathOp; 4] = [
    PathOp::Difference,
    PathOp::Intersect,
    PathOp::Union,
    PathOp::Xor,
];

// Port of: gm/pathopsblend.cpp#L28-L32 (chrome/m156), OpAsBlend
struct OpAsBlend {
    mode: BlendMode,
    inverse: bool,
}

// Port of: gm/pathopsblend.cpp#L34-L50 (chrome/m156), op_blend_mode
fn op_blend_mode(op: PathOp) -> OpAsBlend {
    match op {
        PathOp::Difference => OpAsBlend {
            mode: BlendMode::Clear,
            inverse: false,
        },
        PathOp::Intersect => OpAsBlend {
            mode: BlendMode::Clear,
            inverse: true,
        },
        PathOp::Union => OpAsBlend {
            mode: BlendMode::Plus,
            inverse: false,
        },
        PathOp::Xor => OpAsBlend {
            mode: BlendMode::Xor,
            inverse: false,
        },
        // We don't implement kReverseDifference (see note above)
        PathOp::ReverseDifference => OpAsBlend {
            mode: BlendMode::SrcOver,
            inverse: false,
        },
    }
}

// Port of: gm/pathopsblend.cpp#L52-L131 (chrome/m156), DEF_SIMPLE_GM(pathops_blend)
crate::def_simple_gm!(
    pathops_blend,
    canvas,
    130,
    60 * K_OPS.len() as i32 + 60 + 10,
    {
        // Checkerboard background to demonstrate that we're only covering the pixels we want:
        draw_checkerboard(
            canvas,
            Color::from(0xFF99_9999),
            Color::from(0xFF66_6666),
            8,
        );

        // Two paths that overlap in interesting ways:
        let p1 = cross();
        let p2 = circle();

        // One path op (intersect) requires one path be drawn using inverse-fill:
        let p2inv = p2.with_toggle_inverse_fill_type();

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        canvas.translate((10.0, 10.0));

        // Draw the two paths by themselves:
        {
            canvas.save();
            canvas.draw_path(&p1, &paint);
            canvas.translate((60.0, 0.0));
            canvas.draw_path(&p2, &paint);
            canvas.restore();
            canvas.translate((0.0, 60.0));
        }

        for op in K_OPS {
            canvas.save();

            // Use PathOps to compute new path, then draw it:
            if let Some(op_path) = pathops_op(&p1, &p2, op) {
                canvas.draw_path(&op_path, &paint);
            }
            canvas.translate((60.0, 0.0));

            // Do raster version of op
            {
                let blend = op_blend_mode(op);
                // Create a layer. We will use blending to build a mask of the shape we want here.
                // Note that we're always going to get a SrcOver blend of the final shape when this
                // layer is restored. The math doesn't work out for most blend modes, because we're
                // turning the coverage of the resulting shape into the layer's alpha.
                canvas.save_layer(&SaveLayerRec::default().bounds(&Rect::from_wh(50.0, 50.0)));

                // We reuse this paint to apply various blend modes:
                let mut p = Paint::default();
                p.set_anti_alias(true);

                // Draw the first shape, using SrcOver. This fills the layer with a mask of that path:
                p.set_blend_mode(BlendMode::SrcOver);
                canvas.draw_path(&p1, &p);

                // Based on the PathOp we're emulating, we set a specific blend mode, and then fill
                // either the second path -- or its inverse.
                p.set_blend_mode(blend.mode);
                canvas.draw_path(if blend.inverse { &p2inv } else { &p2 }, &p);

                // The layer's alpha channel now contains a mask of the desired shape. Cover the entire
                // rectangle with whatever paint we ACTUALLY want to draw (eg, blue), using kSrcIn.
                // This will only draw where the mask was present:
                p.set_blend_mode(BlendMode::SrcIn);
                p.set_color(Color::BLUE);
                canvas.draw_paint(&p);

                canvas.restore();
            }

            canvas.restore();
            canvas.translate((0.0, 60.0));
        }
    }
);
