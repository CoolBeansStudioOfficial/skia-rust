// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hairmodes.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_PI, scalar, scalar_cos, scalar_sin};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/hairmodes.cpp#L28-L41 (chrome/m156)
const G_MODES: [BlendMode; 12] = [
    BlendMode::Clear,
    BlendMode::Src,
    BlendMode::Dst,
    BlendMode::SrcOver,
    BlendMode::DstOver,
    BlendMode::SrcIn,
    BlendMode::DstIn,
    BlendMode::SrcOut,
    BlendMode::DstOut,
    BlendMode::SrcATop,
    BlendMode::DstATop,
    BlendMode::Xor,
];

// Port of: gm/hairmodes.cpp#L43-L46 (chrome/m156)
const G_WIDTH: i32 = 64;
const G_HEIGHT: i32 = 64;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const W: scalar = G_WIDTH as scalar;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const H: scalar = G_HEIGHT as scalar;

// Port of: gm/hairmodes.cpp#L48-L75 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar and int * SkScalar arithmetic
fn draw_cell(canvas: &Canvas, mode: BlendMode, a0: u8, a1: u8) -> scalar {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut r = Rect::from_wh(W, H);
    r.inset((W / 10.0, H / 10.0));

    paint.set_color(Color::BLUE);
    paint.set_alpha(a0);
    canvas.draw_oval(r, &paint);

    paint.set_color(Color::RED);
    paint.set_alpha(a1);
    paint.set_blend_mode(mode);
    for angle in 0..24 {
        let x = scalar_cos(angle as scalar * (SCALAR_PI * 2.0) / 24.0) * G_WIDTH as scalar;
        let y = scalar_sin(angle as scalar * (SCALAR_PI * 2.0) / 24.0) * G_HEIGHT as scalar;
        paint.set_stroke_width(1.0 * angle as scalar * 2.0 / 24.0);
        canvas.draw_line((W / 2.0, H / 2.0), (W / 2.0 + x, H / 2.0 + y), &paint);
    }

    H
}

// Port of: gm/hairmodes.cpp#L77-L86 (chrome/m156)
fn make_bg_shader() -> Option<Shader> {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2, 2), None);
    bm.set_addr32(0, 0, 0xFFFF_FFFF);
    bm.set_addr32(1, 1, 0xFFFF_FFFF);
    let v = pack_argb32(0xFF, 0xCE, 0xCF, 0xCE);
    bm.set_addr32(1, 0, v);
    bm.set_addr32(0, 1, v);

    let mut m = Matrix::new_identity();
    m.set_scale((6.0, 6.0), None);
    bm.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        &m,
    )
}

// Port of: gm/hairmodes.cpp#L88-L135 (chrome/m156)
struct HairModesGm {
    bg_paint: Paint,
}

impl HairModesGm {
    fn new() -> HairModesGm {
        HairModesGm {
            bg_paint: Paint::default(),
        }
    }
}

impl GM for HairModesGm {
    fn name(&self) -> String {
        "hairmodes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_once_before_draw(&mut self) {
        self.bg_paint.set_shader(make_bg_shader());
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const G_ALPHA_VALUE: [u8; 3] = [0xFF, 0x88, 0x88];
        let bounds = Rect::from_wh(W, H);

        canvas.translate((4.0, 4.0));

        for alpha in 0..4usize {
            canvas.save();
            canvas.save();
            for (i, mode) in G_MODES.iter().enumerate() {
                if 6 == i {
                    canvas.restore();
                    canvas.translate((W * 5.0, 0.0));
                    canvas.save();
                }

                canvas.draw_rect(bounds, &self.bg_paint);
                canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
                let dy = draw_cell(
                    canvas,
                    *mode,
                    G_ALPHA_VALUE[alpha & 1],
                    G_ALPHA_VALUE[alpha & 2],
                );
                canvas.restore();

                canvas.translate((0.0, dy * 5.0 / 4.0));
            }
            canvas.restore();
            canvas.restore();
            canvas.translate((W * 5.0 / 4.0, 0.0));
        }
    }
}

// Port of: gm/hairmodes.cpp#L135 (chrome/m156)
crate::def_gm!(HairModesGM, HairModesGm::new());
