// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmapfilters.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color::pre_multiply_color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::tool_utils::{copy_to, int_to_scalar};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/bitmapfilters.cpp#L118-L158 (chrome/m156)
struct TestExtractAlphaGm {
    bitmap: Bitmap,
    alpha: Bitmap,
}

impl GM for TestExtractAlphaGm {
    // Port of: gm/bitmapfilters.cpp#L119-L134 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        // Make a bitmap with per-pixels alpha (stroked circle)
        self.bitmap.alloc_n32_pixels((100, 100), None);
        {
            let canvas = CoreCanvas::from_bitmap(&mut self.bitmap, None).expect("a canvas");
            canvas.clear(Color::new(0));

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(Color::BLUE);
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(20.0);

            canvas.draw_circle((50.0, 50.0), 39.0, &paint);
        }

        let _ = self.bitmap.extract_alpha(&mut self.alpha, None);
    }

    fn name(&self) -> String {
        "extractalpha".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(540, 330)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::RED);

        let sampling = SamplingOptions::from(FilterMode::Linear);

        // should stay blue (ignore paint's color)
        canvas.draw_image_with_sampling_options(
            self.bitmap.as_image().expect("an image"),
            (10.0, 10.0),
            sampling,
            Some(&paint),
        );
        // should draw red
        canvas.draw_image_with_sampling_options(
            self.alpha.as_image().expect("an image"),
            (120.0, 10.0),
            sampling,
            Some(&paint),
        );
    }
}

// Port of: gm/bitmapfilters.cpp#L160 (chrome/m156)
crate::def_gm!(
    TestExtractAlphaGM,
    TestExtractAlphaGm {
        bitmap: Bitmap::new(),
        alpha: Bitmap::new(),
    }
);

// Port of: gm/bitmapfilters.cpp#L26-L36 (chrome/m156), make_bm
fn make_bm() -> Bitmap {
    let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::WHITE];
    let mut colors_pm = [0_u32; 4];
    for (pm, color) in colors_pm.iter_mut().zip(colors) {
        *pm = pre_multiply_color(color);
    }
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2, 2), true);
    bm.set_addr32(0, 0, colors_pm[0]);
    bm.set_addr32(1, 0, colors_pm[1]);
    bm.set_addr32(0, 1, colors_pm[2]);
    bm.set_addr32(1, 1, colors_pm[3]);
    bm
}

// Port of: gm/bitmapfilters.cpp#L38-L42 (chrome/m156), draw_bm
fn draw_bm(
    canvas: &Canvas,
    img: &Image,
    x: f32,
    y: f32,
    sampling: SamplingOptions,
    paint: &Paint,
) -> f32 {
    canvas.draw_image_with_sampling_options(img, (x, y), sampling, Some(paint));
    int_to_scalar(img.width()) * 5.0 / 4.0
}

// Port of: gm/bitmapfilters.cpp#L44-L50 (chrome/m156), draw_set
fn draw_set(canvas: &Canvas, img: &Image, mut x: f32, p: &mut Paint) -> f32 {
    x += draw_bm(canvas, img, x, 0.0, SamplingOptions::default(), p);
    x += draw_bm(
        canvas,
        img,
        x,
        0.0,
        SamplingOptions::from(FilterMode::Linear),
        p,
    );
    p.set_dither(true);
    x + draw_bm(
        canvas,
        img,
        x,
        0.0,
        SamplingOptions::from(FilterMode::Linear),
        p,
    )
}

// `ToolUtils::colortype_name` for the color types this GM draws. N32 is `BGRA_8888` on the oracle
// (Windows) and `RGBA_8888` on an RGBA host, as in Skia.
// Port of: tools/ToolUtils.cpp#L62-L95 (chrome/m156), colortype_name (the types this GM names)
fn colortype_name(color_type: ColorType) -> &'static str {
    match color_type {
        ColorType::RGB565 => "RGB_565",
        ColorType::ARGB4444 => "ARGB_4444",
        ColorType::RGBA8888 => "RGBA_8888",
        ColorType::BGRA8888 => "BGRA_8888",
        _ => panic!("colortype_name: this GM only names 565, 4444 and N32"),
    }
}

// Port of: gm/bitmapfilters.cpp#L52-L68 (chrome/m156), draw_row
fn draw_row(canvas: &Canvas, img: &Image) -> f32 {
    canvas.save();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut x: f32 = 0.0;
    let scale: f32 = 32.0;
    let font = default_portable_font();
    let name = colortype_name(img.color_type());
    canvas.draw_str(
        name,
        (x, int_to_scalar(img.height()) * scale * 5.0 / 8.0),
        &font,
        &paint,
    );
    canvas.translate((int_to_scalar(48), 0.0));
    canvas.scale((scale, scale));
    x += draw_set(canvas, img, 0.0, &mut paint);
    paint = Paint::default();
    paint.set_alpha_f(0.5);
    draw_set(canvas, img, x, &mut paint);
    canvas.restore();
    x * scale / 3.0
}

// Port of: gm/bitmapfilters.cpp#L75-L114 (chrome/m156), FilterGM
struct FilterGm {
    img32: Option<Image>,
    img4444: Option<Image>,
    img565: Option<Image>,
}

impl FilterGm {
    fn new() -> Self {
        Self {
            img32: None,
            img4444: None,
            img565: None,
        }
    }
}

impl GM for FilterGm {
    fn name(&self) -> String {
        "bitmapfilters".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(540, 250)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    // Port of: gm/bitmapfilters.cpp#L77-L84 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let bm32 = make_bm();
        let mut bm4444 = Bitmap::new();
        copy_to(&mut bm4444, ColorType::ARGB4444, &bm32);
        let mut bm565 = Bitmap::new();
        copy_to(&mut bm565, ColorType::RGB565, &bm32);
        self.img32 = bm32.as_image();
        self.img4444 = bm4444.as_image();
        self.img565 = bm565.as_image();
    }

    // Port of: gm/bitmapfilters.cpp#L92-L104 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let x = int_to_scalar(10);
        let mut y = int_to_scalar(10);
        canvas.translate((x, y));
        if let Some(img) = &self.img4444 {
            y = draw_row(canvas, img);
        }
        canvas.translate((0.0, y));
        if let Some(img) = &self.img565 {
            y = draw_row(canvas, img);
        }
        canvas.translate((0.0, y));
        if let Some(img) = &self.img32 {
            draw_row(canvas, img);
        }
    }
}

// Port of: gm/bitmapfilters.cpp#L114 (chrome/m156), DEF_GM( return new FilterGM; )
crate::def_gm!(
    #[ignore = "see notes/gm_bitmapfilters_cpp_FilterGM.md"]
    FilterGM_ = "FilterGM",
    FilterGm::new()
);
