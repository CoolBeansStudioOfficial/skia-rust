// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/manypathatlases.cpp (chrome/m156)

// The int-to-scalar casts mirror the C++ arithmetic of the GM (small values, exact in f32).
#![allow(clippy::cast_precision_loss)]

// This test originally ensured that the ccpr path cache preserved fill rules properly. CCPR is
// gone now, but we decided to keep the test.
//
// The GPU-only parts of the C++ (the `modifyGrContextOptions` atlas size and the `dContext->flush()`
// on a direct context) do not run on the raster sinks and are not ported. `setIsVolatile(true)`
// is a cache hint with no effect on the raster output and is not exposed by the Rust `Path`.

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::colors;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::scalar::scalar;

// Port of: gm/manypathatlases.cpp (chrome/m156), class ManyPathAtlasesGM
#[derive(Debug)]
pub struct ManyPathAtlasesGm {
    max_atlas_size: i32,
}

impl ManyPathAtlasesGm {
    // Port of: gm/manypathatlases.cpp (chrome/m156), ManyPathAtlasesGM(int)
    #[must_use]
    pub fn new(max_atlas_size: i32) -> Self {
        Self { max_atlas_size }
    }
}

impl GM for ManyPathAtlasesGm {
    // Port of: gm/manypathatlases.cpp (chrome/m156), getName
    fn name(&self) -> String {
        format!("manypathatlases_{}", self.max_atlas_size)
    }

    fn size(&mut self) -> ISize {
        ISize::new(128, 128)
    }

    // Port of: gm/manypathatlases.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(colors::YELLOW);

        let clip: Path = PathBuilder::new()
            .move_to((-50.0, 20.0))
            .cubic_to((-50.0, -20.0), (50.0, -20.0), (50.0, 40.0))
            .cubic_to((20.0, 0.0), (-20.0, 0.0), (-50.0, 20.0))
            .transform(&Matrix::translate((64.0, 70.0)))
            .detach();
        for i in 0..4 {
            let rotated_clip = clip.make_transform(&Matrix::rotate_deg_pivot(
                30.0 * i as scalar + 128.0,
                (64.0, 70.0),
            ));
            canvas.clip_path(&rotated_clip, ClipOp::Difference, true);
        }
        let path: Path = PathBuilder::new()
            .move_to((20.0, 0.0))
            .line_to((108.0, 0.0))
            .cubic_to((108.0, 20.0), (108.0, 20.0), (128.0, 20.0))
            .line_to((128.0, 108.0))
            .cubic_to((108.0, 108.0), (108.0, 108.0), (108.0, 128.0))
            .line_to((20.0, 128.0))
            .cubic_to((20.0, 108.0), (20.0, 108.0), (0.0, 108.0))
            .line_to((0.0, 20.0))
            .cubic_to((20.0, 20.0), (20.0, 20.0), (20.0, 0.0))
            .detach();
        let mut teal = Paint::default();
        teal.set_color4f(Color4f::new(0.03, 0.91, 0.87, 1.0), None);
        teal.set_anti_alias(true);
        canvas.draw_path(&path, &teal);
    }
}

// Port of: gm/manypathatlases.cpp (chrome/m156), DEF_GM( return new ManyPathAtlasesGM(128); )
crate::def_gm!(
    ManyPathAtlasesGM_128 = "ManyPathAtlasesGM(128)",
    ManyPathAtlasesGm::new(128)
);
// Port of: gm/manypathatlases.cpp (chrome/m156), DEF_GM( return new ManyPathAtlasesGM(2048); )
crate::def_gm!(
    ManyPathAtlasesGM_2048 = "ManyPathAtlasesGM(2048)",
    ManyPathAtlasesGm::new(2048)
);
