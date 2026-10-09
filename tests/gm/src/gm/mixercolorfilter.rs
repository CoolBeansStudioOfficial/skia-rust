// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mixercolorfilter.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::luma_color_filter;

// A tint filter maps colors to a given range (gradient), based on the input luminance:
//
//   c' = lerp(lo, hi, luma(c))
//
// Port of: gm/mixercolorfilter.cpp#L14-L41 (chrome/m156), MakeTintColorFilter
fn make_tint_color_filter(lo: Color, hi: Color) -> Option<ColorFilter> {
    let (r_lo, g_lo, b_lo, a_lo) = (lo.r(), lo.g(), lo.b(), lo.a());
    let (r_hi, g_hi, b_hi, a_hi) = (hi.r(), hi.g(), hi.b(), hi.a());

    // We map component-wise:
    //
    //   r' = lo.r + (hi.r - lo.r) * luma
    //   g' = lo.g + (hi.g - lo.g) * luma
    //   b' = lo.b + (hi.b - lo.b) * luma
    //   a' = lo.a + (hi.a - lo.a) * luma
    //
    // The input luminance is stored in the alpha channel
    // (and RGB are cleared -- see SkLumaColorFilter). Thus:
    #[rustfmt::skip]
    let tint_matrix = ColorMatrix::new(
        0.0, 0.0, 0.0, int_to_scalar(i32::from(r_hi) - i32::from(r_lo)) / 255.0, int_to_scalar(i32::from(r_lo)) / 255.0,
        0.0, 0.0, 0.0, int_to_scalar(i32::from(g_hi) - i32::from(g_lo)) / 255.0, int_to_scalar(i32::from(g_lo)) / 255.0,
        0.0, 0.0, 0.0, int_to_scalar(i32::from(b_hi) - i32::from(b_lo)) / 255.0, int_to_scalar(i32::from(b_lo)) / 255.0,
        0.0, 0.0, 0.0, int_to_scalar(i32::from(a_hi) - i32::from(a_lo)) / 255.0, int_to_scalar(i32::from(a_lo)) / 255.0,
    );
    let matrix = color_filters::matrix(&tint_matrix, Clamp::Yes);
    matrix?.composed(luma_color_filter::make()).into()
}

// Port of: gm/mixercolorfilter.cpp#L43-L100 (chrome/m156), MixerCFGM
struct MixerCfGm {
    width: f32,
    height: f32,
    count: usize,
}

impl MixerCfGm {
    // Port of: gm/mixercolorfilter.cpp#L50-L51 (chrome/m156), the constructor
    fn new(width: f32, height: f32, count: usize) -> Self {
        Self {
            width,
            height,
            count,
        }
    }

    // Port of: gm/mixercolorfilter.cpp#L76-L95 (chrome/m156), mixRow
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(i) / (fTileCount - 1)
    fn mix_row(
        &self,
        canvas: &Canvas,
        paint: &mut Paint,
        cf0: Option<&ColorFilter>,
        cf1: Option<&ColorFilter>,
    ) {
        // We cycle through paint colors on each row, to test how the paint color flows through
        // the color-filter network
        let paint_colors = [
            Color4f::new(1.0, 1.0, 1.0, 1.0), // Opaque white
            Color4f::new(1.0, 1.0, 1.0, 0.5), // Translucent white
            Color4f::new(0.5, 0.5, 1.0, 1.0), // Opaque pale blue
            Color4f::new(0.5, 0.5, 1.0, 0.5), // Translucent pale blue
        ];

        canvas.translate((0.0, self.height * 0.1));
        {
            canvas.save();
            for i in 0..self.count {
                paint.set_color4f(paint_colors[i % paint_colors.len()], None);
                let t = i as f32 / (self.count - 1) as f32;
                paint.set_color_filter(color_filters::lerp(t, cf0.cloned(), cf1.cloned()));
                canvas.translate((self.width * 0.1, 0.0));
                canvas.draw_rect(Rect::new(0.0, 0.0, self.width, self.height), paint);
                canvas.translate((self.width * 1.1, 0.0));
            }
            canvas.restore();
        }
        canvas.translate((0.0, self.height * 1.1));
    }
}

impl GM for MixerCfGm {
    // Port of: gm/mixercolorfilter.cpp#L56-L57 (chrome/m156), getName
    fn name(&self) -> String {
        "mixerCF".to_string()
    }

    // Port of: gm/mixercolorfilter.cpp#L59-L63 (chrome/m156), getISize (3 rows)
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // mirror the float tile size times the size_t count, and SkISize::Make's float-to-int
    fn size(&mut self) -> ISize {
        ISize::new(
            (self.width * 1.2 * self.count as f32) as i32,
            (self.height * 1.2 * 3.0) as i32,
        )
    }

    // Port of: gm/mixercolorfilter.cpp#L65-L75 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let gradient_colors = [
            Color4f::new(1.0, 0.0, 0.0, 1.0), // SkColors::kRed
            Color4f::new(0.0, 1.0, 0.0, 1.0), // SkColors::kGreen
            Color4f::new(0.0, 0.0, 1.0, 1.0), // SkColors::kBlue
            Color4f::new(1.0, 0.0, 0.0, 1.0), // SkColors::kRed
        ];
        let center = (self.width / 2.0, self.height / 2.0);
        paint.set_shader(shaders::sweep_gradient(
            center,
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&gradient_colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));

        let cf0 = make_tint_color_filter(Color::from(0xff30_0000), Color::from(0xffa0_0000)); // red tint
        let cf1 = make_tint_color_filter(Color::from(0xff00_3000), Color::from(0xff00_a000)); // green tint
        self.mix_row(canvas, &mut paint, None, cf1.as_ref());
        self.mix_row(canvas, &mut paint, cf0.as_ref(), None);
        self.mix_row(canvas, &mut paint, cf0.as_ref(), cf1.as_ref());
    }
}

// Port of: gm/mixercolorfilter.cpp#L102 (chrome/m156)
crate::def_gm!(
    MixerCFGM_200_250_5 = "MixerCFGM(SkSize::Make(200, 250), 5)",
    MixerCfGm::new(200.0, 250.0, 5)
);
