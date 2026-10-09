// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tablecolorfilter.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::color_filter as color_filter_image;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/tablecolorfilter.cpp#L8-L17 (chrome/m156), make_shader0
fn make_shader0(w: i32, h: i32) -> Option<Shader> {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(int_to_scalar(w), int_to_scalar(h)),
    ];
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 1.0), // kBlack
        Color4f::new(0.0, 1.0, 0.0, 1.0), // kGreen
        Color4f::new(0.0, 1.0, 1.0, 1.0), // kCyan
        Color4f::new(1.0, 0.0, 0.0, 1.0), // kRed
        Color4f::new(0.0, 0.0, 0.0, 0.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0), // kBlue
        Color4f::new(1.0, 1.0, 1.0, 1.0), // kWhite
    ];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/tablecolorfilter.cpp#L19-L28 (chrome/m156), make_bm0
fn make_bm0() -> Bitmap {
    let w = 120;
    let h = 120;
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(Color::TRANSPARENT);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        let mut paint = Paint::default();
        paint.set_shader(make_shader0(w, h));
        canvas.draw_paint(&paint);
    }
    bm
}

// Port of: gm/tablecolorfilter.cpp#L30-L36 (chrome/m156), make_shader1
fn make_shader1(w: i32, h: i32) -> Option<Shader> {
    let cx = int_to_scalar(w) / 2.0;
    let cy = int_to_scalar(h) / 2.0;
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0), // kRed
        Color4f::new(0.0, 1.0, 0.0, 1.0), // kGreen
        Color4f::new(0.0, 0.0, 1.0, 1.0), // kBlue
    ];
    gradient_shaders::radial_gradient(
        ((cx, cy), cx),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/tablecolorfilter.cpp#L38-L50 (chrome/m156), make_bm1
fn make_bm1() -> Bitmap {
    let w = 120;
    let h = 120;
    let cx = int_to_scalar(w) / 2.0;
    let cy = int_to_scalar(h) / 2.0;
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(Color::TRANSPARENT);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        let mut paint = Paint::default();
        paint.set_shader(make_shader1(w, h));
        paint.set_anti_alias(true);
        canvas.draw_circle((cx, cy), cx, &paint);
    }
    bm
}

// Port of: gm/tablecolorfilter.cpp#L52-L57 (chrome/m156), make_table0
#[allow(clippy::cast_possible_truncation)] // mirrors the (uint8_t) cast of the table entries
fn make_table0() -> [u8; 256] {
    let mut table = [0u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let n = (i >> 5) as u32;
        *entry = ((n << 5) | (n << 2) | (n >> 1)) as u8;
    }
    table
}

// Port of: gm/tablecolorfilter.cpp#L59-L63 (chrome/m156), make_table1
#[allow(clippy::cast_possible_truncation)] // mirrors the (uint8_t) cast of i * i / 255
fn make_table1() -> [u8; 256] {
    let mut table = [0u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = (i * i / 255) as u8;
    }
    table
}

// Port of: gm/tablecolorfilter.cpp#L65-L70 (chrome/m156), make_table2
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // mirrors i / 255.0f and static_cast<uint8_t>(sqrtf(fi) * 255)
fn make_table2() -> [u8; 256] {
    let mut table = [0u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let fi = i as f32 / 255.0;
        *entry = (fi.sqrt() * 255.0) as u8;
    }
    table
}

// Port of: gm/tablecolorfilter.cpp#L72-L104 (chrome/m156), the color filter makers
fn make_null_cf() -> Option<ColorFilter> {
    None
}

// Port of: gm/tablecolorfilter.cpp#L76-L80 (chrome/m156), make_cf0
#[allow(clippy::unnecessary_wraps)] // the makers return Option<ColorFilter>, as in the C++ array
fn make_cf0() -> Option<ColorFilter> {
    Some(color_filters::table(&make_table0()))
}

// Port of: gm/tablecolorfilter.cpp#L81-L85 (chrome/m156), make_cf1
#[allow(clippy::unnecessary_wraps)] // the makers return Option<ColorFilter>, as in the C++ array
fn make_cf1() -> Option<ColorFilter> {
    Some(color_filters::table(&make_table1()))
}

// Port of: gm/tablecolorfilter.cpp#L86-L90 (chrome/m156), make_cf2
#[allow(clippy::unnecessary_wraps)] // the makers return Option<ColorFilter>, as in the C++ array
fn make_cf2() -> Option<ColorFilter> {
    Some(color_filters::table(&make_table2()))
}

// Port of: gm/tablecolorfilter.cpp#L91-L98 (chrome/m156), make_cf3
fn make_cf3() -> Option<ColorFilter> {
    let table0 = make_table0();
    let table1 = make_table1();
    let table2 = make_table2();
    color_filters::table_argb(None, Some(&table0), Some(&table1), Some(&table2))
}

// Port of: gm/tablecolorfilter.cpp#L100-L186 (chrome/m156), TableColorFilterGM
struct TableColorFilterGm;

impl GM for TableColorFilterGm {
    fn name(&self) -> String {
        "tablecolorfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(700, 1650)
    }

    // Port of: gm/tablecolorfilter.cpp#L105-L184 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::from(0xFFDD_DDDD), None);
        canvas.translate((20.0, 20.0));
        let color_filter_makers: [fn() -> Option<ColorFilter>; 5] =
            [make_null_cf, make_cf0, make_cf1, make_cf2, make_cf3];
        let bitmap_makers: [fn() -> Bitmap; 2] = [make_bm0, make_bm1];

        // This test will be done once for each bitmap with the results stacked vertically.
        // For a single bitmap the resulting image will be the following:
        //  - A first line with the original bitmap, followed by the image drawn once
        //  with each of the N color filters
        //  - N lines of the bitmap drawn N times, this will cover all N*N combinations of
        //  pair of color filters in order to test the collapsing of consecutive table
        //  color filters.
        let mut x: f32;
        let mut y: f32 = 0.0;
        for bitmap_maker in bitmap_makers {
            let bm = bitmap_maker();
            let image = bm.as_image().expect("an image of the bitmap");
            let x_offset = int_to_scalar(bm.width() * 9 / 8);
            let y_offset = int_to_scalar(bm.height() * 9 / 8);
            // Draw the first element of the first line
            x = 0.0;
            let mut paint = Paint::default();
            let sampling = SamplingOptions::default();
            canvas.draw_image(&image, (x, y), None);
            // Draws the rest of the first line for this bitmap
            // each draw being at xOffset of the previous one
            for maker in color_filter_makers.iter().skip(1) {
                x += x_offset;
                paint.set_color_filter(maker());
                canvas.draw_image_with_sampling_options(&image, (x, y), sampling, Some(&paint));
            }
            paint.set_color_filter(None);
            for maker in color_filter_makers {
                let color_filter1 = maker();
                let image_filter1: Option<ImageFilter> =
                    color_filter_image(color_filter1, None, None);
                // Move down to the next line and draw it
                // each draw being at xOffset of the previous one
                y += y_offset;
                x = 0.0;
                for maker2 in color_filter_makers.iter().skip(1) {
                    let color_filter2 = maker2();
                    let image_filter2 =
                        color_filter_image(color_filter2, image_filter1.clone(), None);
                    paint.set_image_filter(image_filter2);
                    canvas.draw_image_with_sampling_options(&image, (x, y), sampling, Some(&paint));
                    x += x_offset;
                }
            }
            // Move down one line to the beginning of the block for next bitmap
            y += y_offset;
        }
    }
}

// Port of: gm/tablecolorfilter.cpp#L186 (chrome/m156)
crate::def_gm!(TableColorFilterGM, TableColorFilterGm);

// Port of: gm/tablecolorfilter.cpp#L188-L224 (chrome/m156), ComposeColorFilterGM
struct ComposeColorFilterGm {
    colors: [Color; 3],
    modes: [BlendMode; 4],
    name: &'static str,
}

impl GM for ComposeColorFilterGm {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(790, 790)
    }

    // Port of: gm/tablecolorfilter.cpp#L202-L224 (chrome/m156), onDraw
    #[allow(clippy::cast_precision_loss)] // mirrors (i + 1) * (r.width() + spacer), int to float
    fn on_draw(&mut self, canvas: &Canvas) {
        const MODE_COUNT: usize = 4;
        const COLOR_COUNT: usize = 3;
        const MODES: usize = MODE_COUNT * COLOR_COUNT;

        let _bm = make_bm1();

        canvas.draw_color(Color::from(0xFFDD_DDDD), None);
        let mut filters: Vec<ColorFilter> = Vec::with_capacity(MODES);
        for mode in self.modes {
            for color in self.colors {
                filters.push(
                    color_filters::blend(Color4f::from(color), None, mode)
                        .expect("a blend color filter"),
                );
            }
        }

        let mut paint = Paint::default();
        paint.set_shader(make_shader1(50, 50));
        let r = Rect::new(0.0, 0.0, 50.0, 50.0);
        let spacer: f32 = 10.0;
        canvas.translate((spacer, spacer));
        canvas.draw_rect(r, &paint); // orig
        for (i, filter) in filters.iter().enumerate() {
            paint.set_color_filter(filter.clone());
            canvas.save();
            canvas.translate(((i + 1) as f32 * (r.width() + spacer), 0.0));
            canvas.draw_rect(r, &paint);
            canvas.restore();
            canvas.save();
            canvas.translate((0.0, (i + 1) as f32 * (r.width() + spacer)));
            canvas.draw_rect(r, &paint);
            canvas.restore();
        }
        canvas.translate((r.width() + spacer, r.width() + spacer));
        for y in 0..MODES {
            canvas.save();
            for x in 0..MODES {
                paint.set_color_filter(filters[y].composed(Some(filters[x].clone())));
                canvas.draw_rect(r, &paint);
                canvas.translate((r.width() + spacer, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, r.height() + spacer));
        }
    }
}

// Port of: gm/tablecolorfilter.cpp#L226 (chrome/m156), gColors0 and gModes0
crate::def_gm!(
    ComposeColorFilterGM_wacky =
        "ComposeColorFilterGM(gColors0, gModes0, \"colorcomposefilter_wacky\")",
    ComposeColorFilterGm {
        colors: [
            Color::from(0xFF00_FFFF), // SK_ColorCYAN
            Color::from(0xFFFF_00FF), // SK_ColorMAGENTA
            Color::from(0xFFFF_FF00), // SK_ColorYELLOW
        ],
        modes: [
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::ColorBurn,
            BlendMode::Exclusion,
        ],
        name: "colorcomposefilter_wacky",
    }
);

// Port of: gm/tablecolorfilter.cpp#L239 (chrome/m156), gColors1 and gModes1
crate::def_gm!(
    ComposeColorFilterGM_alpha =
        "ComposeColorFilterGM(gColors1, gModes1, \"colorcomposefilter_alpha\")",
    ComposeColorFilterGm {
        colors: [
            Color::from(0x80FF_0000),
            Color::from(0x8000_FF00),
            Color::from(0x8000_00FF),
        ],
        modes: [
            BlendMode::SrcOver,
            BlendMode::Xor,
            BlendMode::DstOut,
            BlendMode::SrcATop,
        ],
        name: "colorcomposefilter_alpha",
    }
);
