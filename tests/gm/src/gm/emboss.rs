// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/emboss.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::emboss_mask_filter::{self, Light};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/emboss.cpp#L9-L16 (chrome/m156)
fn make_bm() -> Option<Image> {
    let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None)?;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    surf.canvas().draw_circle((50.0, 50.0), 50.0, &paint);
    surf.image_snapshot()
}

// Port of: gm/emboss.cpp#L18-L60 (chrome/m156)
struct EmbossGm;

impl GM for EmbossGm {
    fn name(&self) -> String {
        "emboss".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 120)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let img = make_bm().expect("an image");
        canvas.draw_image(&img, (10.0, 10.0), None);
        canvas.translate((img.width() as f32 + 10.0, 0.0));
        paint.set_mask_filter(
            emboss_mask_filter::new(
                BlurMask::convert_radius_to_sigma(3.0),
                &Light {
                    direction: [1.0, 1.0, 1.0],
                    pad: 0,
                    ambient: 128,
                    specular: 16 * 2,
                },
            )
            .expect("a mask filter"),
        );
        canvas.draw_image_with_sampling_options(
            &img,
            (10.0, 10.0),
            SamplingOptions::default(),
            Some(&paint),
        );
        canvas.translate((img.width() as f32 + 10.0, 0.0));
        paint.set_color_filter(color_filters::blend(
            Color4f::from_color(Color::new(0xFFFF0000)),
            None,
            BlendMode::SrcATop,
        ));
        canvas.draw_image_with_sampling_options(
            &img,
            (10.0, 10.0),
            SamplingOptions::default(),
            Some(&paint),
        );
        canvas.translate((img.width() as f32 + 10.0, 0.0));
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(10.0);
        paint.set_mask_filter(
            emboss_mask_filter::new(
                BlurMask::convert_radius_to_sigma(4.0),
                &Light {
                    direction: [1.0, 1.0, 1.0],
                    pad: 0,
                    ambient: 128,
                    specular: 16 * 2,
                },
            )
            .expect("a mask filter"),
        );
        paint.set_color_filter(None);
        paint.set_shader(shaders::color(Color::BLUE));
        paint.set_dither(true);
        canvas.draw_circle((50.0, 50.0), 30.0, &paint);
        canvas.translate((100.0, 0.0));
        let font = Font::from_size(default_portable_typeface(), 50.0);
        paint.set_style(Style::Fill);
        draw_string(canvas, "Hello", 0.0, 50.0, &font, &paint, Align::Left);
        paint.set_shader(None);
        paint.set_color(Color::GREEN);
        draw_string(canvas, "World", 0.0, 100.0, &font, &paint, Align::Left);
    }
}

crate::def_gm!(
    #[ignore = "see notes/gm_emboss_cpp_EmbossGM.md"]
    EmbossGM,
    EmbossGm
);

// Port of: gm/emboss.cpp#L326-L340 (chrome/m156)
crate::def_simple_gm!(smallemboss, canvas, 50, 50, {
    let emboss_filter = emboss_mask_filter::new(
        3.0,
        &Light {
            direction: [1.0, 1.0, 1.0],
            pad: 0,
            ambient: 0,
            specular: 16,
        },
    );
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLACK);
    paint.set_mask_filter(emboss_filter.expect("a mask filter"));
    let info = ImageInfo::new_n32_premul((50, 50), None);
    let mut surface = match canvas.new_surface(&info, None) {
        Some(surface) => surface,
        None => surfaces::raster(&info, None, None).expect("a surface"),
    };
    let canv = surface.canvas();
    canv.draw_rect(
        skia_rust_core::rect::Rect::from_xywh(1.0, 1.0, 3.0, 3.0),
        &paint,
    );
    canvas.scale((30.0, 30.0));
    let img = surface.image_snapshot().expect("a snapshot");
    canvas.draw_image(&img, (0.0, 0.0), None);
});
