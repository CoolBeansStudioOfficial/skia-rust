// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clippedbitmapshaders.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::pre_multiply_color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

// This GM draws a 3x3 grid (with the center element excluded) of rectangles
// filled with a bitmap shader. The bitmap shader is transformed so that the
// pattern cell is at the center (excluded) region.
//
// In Repeat and Mirror mode, this tests that the bitmap shader still draws
// even though the pattern cell is outside the clip.
//
// In Clamp mode, this tests that the clamp is handled properly. For PDF,
// (and possibly other exported formats) this also "tests" that the image itself
// is not stored (well, you'll need to open it up with an external tool to
// verify that).

// Port of: gm/clippedbitmapshaders.cpp#L39-L49 (chrome/m156)
fn create_bitmap() -> Bitmap {
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((2, 2), None);
    bmp.set_addr32(0, 0, pre_multiply_color(Color::RED));
    bmp.set_addr32(1, 0, pre_multiply_color(Color::GREEN));
    bmp.set_addr32(0, 1, pre_multiply_color(Color::BLACK));
    bmp.set_addr32(1, 1, pre_multiply_color(Color::BLUE));

    bmp
}

const RECT_SIZE: f32 = 64.0;
const SLIDE_SIZE: f32 = 300.0;

// Port of: gm/clippedbitmapshaders.cpp#L54-L117 (chrome/m156)
struct ClippedBitmapShadersGm {
    mode: TileMode,
    hq: bool,
}

impl GM for ClippedBitmapShadersGm {
    fn name(&self) -> String {
        let descriptor = match self.mode {
            TileMode::Repeat => "tile",
            TileMode::Mirror => "mirror",
            TileMode::Clamp => "clamp",
            TileMode::Decal => "decal",
        };
        let mut descriptor = format!("clipped-bitmap-shaders-{descriptor}");
        if self.hq {
            descriptor.push_str("-hq");
        }
        descriptor
    }

    fn size(&mut self) -> ISize {
        ISize::new(300, 300)
    }

    #[allow(clippy::cast_precision_loss)] // SLIDE_SIZE / 3 * i
    fn on_draw(&mut self, canvas: &Canvas) {
        let bmp = create_bitmap();
        let mut s = Matrix::new_identity();
        s.reset();
        s.set_scale((8.0, 8.0), None);
        s.post_translate((SLIDE_SIZE / 2.0, SLIDE_SIZE / 2.0));
        let mut paint = Paint::default();
        paint.set_shader(bmp.to_shader(
            (self.mode, self.mode),
            if self.hq {
                SamplingOptions::from(CubicResampler::mitchell())
            } else {
                SamplingOptions::default()
            },
            &s,
        ));

        let margin = (SLIDE_SIZE / 3.0 - RECT_SIZE) / 2.0;
        for i in 0..3 {
            let y_origin = SLIDE_SIZE / 3.0 * i as f32 + margin;
            for j in 0..3 {
                let x_origin = SLIDE_SIZE / 3.0 * j as f32 + margin;
                if i == 1 && j == 1 {
                    continue; // skip center element
                }
                let rect = Rect::from_xywh(x_origin, y_origin, RECT_SIZE, RECT_SIZE);
                canvas.save();
                canvas.clip_rect(rect, None, None);
                canvas.draw_rect(rect, &paint);
                canvas.restore();
            }
        }
    }
}

// Port of: gm/clippedbitmapshaders.cpp#L121-L127 (chrome/m156)
crate::def_gm!(
    ClippedBitmapShadersGM_kRepeat = "ClippedBitmapShadersGM(SkTileMode::kRepeat)",
    ClippedBitmapShadersGm {
        mode: TileMode::Repeat,
        hq: false
    }
);
crate::def_gm!(
    ClippedBitmapShadersGM_kMirror = "ClippedBitmapShadersGM(SkTileMode::kMirror)",
    ClippedBitmapShadersGm {
        mode: TileMode::Mirror,
        hq: false
    }
);
crate::def_gm!(
    ClippedBitmapShadersGM_kClamp = "ClippedBitmapShadersGM(SkTileMode::kClamp)",
    ClippedBitmapShadersGm {
        mode: TileMode::Clamp,
        hq: false
    }
);

crate::def_gm!(
    ClippedBitmapShadersGM_kRepeat_true = "ClippedBitmapShadersGM(SkTileMode::kRepeat, true)",
    ClippedBitmapShadersGm {
        mode: TileMode::Repeat,
        hq: true
    }
);
crate::def_gm!(
    ClippedBitmapShadersGM_kMirror_true = "ClippedBitmapShadersGM(SkTileMode::kMirror, true)",
    ClippedBitmapShadersGm {
        mode: TileMode::Mirror,
        hq: true
    }
);
crate::def_gm!(
    ClippedBitmapShadersGM_kClamp_true = "ClippedBitmapShadersGM(SkTileMode::kClamp, true)",
    ClippedBitmapShadersGm {
        mode: TileMode::Clamp,
        hq: true
    }
);
