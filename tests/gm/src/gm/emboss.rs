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
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shaders;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::emboss_mask_filter::{self, Light};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::{default_portable_font, default_portable_typeface};

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

crate::def_gm!(EmbossGM, EmbossGm);

// Port of: gm/emboss.cpp#L94-L136 (chrome/m156)
fn draw_emboss_example(
    canvas: &Canvas,
    bounds: Rect,
    light: &Light,
    blur_sigma: scalar,
    label: &str,
) {
    canvas.save();
    canvas.clip_rect(bounds, None, None);
    canvas.translate((bounds.left, bounds.top));

    let content_bounds = Rect::from_wh(bounds.width(), bounds.height());

    let Some(emboss_filter) = emboss_mask_filter::new(blur_sigma, light) else {
        // SkDebugf("Failed to create emboss filter for: %s\n", label);
        return;
    };

    let mut paint1 = Paint::default();
    let mut paint2 = Paint::default();
    paint1.set_anti_alias(true);
    paint1.set_color(Color::RED);
    paint1.set_mask_filter(emboss_filter.clone()); // Apply the same filter

    paint2.set_anti_alias(true);
    paint2.set_color(Color::BLUE);
    paint2.set_mask_filter(emboss_filter); // Apply the same filter

    // Draw shapes overlapping
    let radius = content_bounds.width() * 0.25;
    let offset = radius * 0.3;

    canvas.draw_circle(
        (
            content_bounds.center_x() - offset,
            content_bounds.center_y(),
        ),
        radius,
        &paint1,
    );
    let rect = Rect::from_xywh(
        content_bounds.center_x() + offset - radius,
        content_bounds.center_y() - radius,
        radius * 2.0,
        radius * 2.0,
    );
    canvas.draw_rect(rect, &paint2);

    let mut text_paint = Paint::default();
    text_paint.set_color(Color::BLACK);
    draw_string(
        canvas,
        label,
        5.0,
        15.0,
        &default_portable_font(),
        &text_paint,
        Align::Left,
    );

    canvas.restore();
}

// Port of: gm/emboss.cpp#L138-L158 (chrome/m156)
const BLEND_MODE_STRS: [(BlendMode, &str); 18] = [
    (BlendMode::Src, "Src"),
    (BlendMode::Dst, "Dst"),
    (BlendMode::SrcOver, "SrcOver"),
    (BlendMode::DstOver, "DstOver"),
    (BlendMode::SrcIn, "SrcIn"),
    (BlendMode::DstIn, "DstIn"),
    (BlendMode::SrcOut, "SrcOut"),
    (BlendMode::DstOut, "DstOut"),
    (BlendMode::SrcATop, "SrcATop"),
    (BlendMode::DstATop, "DstATop"),
    (BlendMode::Xor, "Xor"),
    (BlendMode::Plus, "Plus"),
    (BlendMode::Modulate, "Modulate"),
    (BlendMode::Screen, "Screen"),
    (BlendMode::Overlay, "Overlay"),
    (BlendMode::Multiply, "Multiply"),
    (BlendMode::Darken, "Darken"),
    (BlendMode::Lighten, "Lighten"),
];

// Port of: gm/emboss.cpp#L160-L200 (chrome/m156)
fn draw_emboss_blend_example(
    canvas: &Canvas,
    bounds: Rect,
    emboss_filter: MaskFilter,
    blend_mode: BlendMode,
    label: &str,
) {
    canvas.save();
    canvas.clip_rect(bounds, None, None);
    canvas.translate((bounds.left, bounds.top));

    let content_bounds = Rect::from_wh(bounds.width(), bounds.height());

    let mut paint1 = Paint::default();
    let mut paint2 = Paint::default();
    paint1.set_anti_alias(true);
    paint1.set_color(Color::RED);

    paint2.set_anti_alias(true);
    paint2.set_color(Color::BLUE);
    paint2.set_mask_filter(emboss_filter);
    paint2.set_blend_mode(blend_mode); // Apply the specified blend mode

    // Draw shapes overlapping
    let radius = content_bounds.width() * 0.25;
    let offset = radius * 0.3;

    // Draw red circle
    canvas.draw_circle(
        (
            content_bounds.center_x() - offset,
            content_bounds.center_y(),
        ),
        radius,
        &paint1,
    );

    // Draw blue rect over the red circle
    let rect = Rect::from_xywh(
        content_bounds.center_x() + offset - radius,
        content_bounds.center_y() - radius,
        radius * 2.0,
        radius * 2.0,
    );
    canvas.draw_rect(rect, &paint2);

    let mut text_paint = Paint::default();
    text_paint.set_color(Color::RED);
    draw_string(
        canvas,
        label,
        5.0,
        15.0,
        &default_portable_font(),
        &text_paint,
        Align::Left,
    );

    canvas.restore();
}

// Port of: gm/emboss.cpp#L202-L324 (chrome/m156)
crate::def_simple_gm!(embossmaskfilter, canvas, 640, 960, {
    let total_width = 640;
    let top_height = 480;
    let bottom_height = 480;

    // Test different values for light:
    {
        let grid_cols = 2;
        let grid_rows = 2;
        let cell_width = total_width as f32 / grid_cols as f32;
        let cell_height = top_height as f32 / grid_rows as f32;

        let mut emboss_recs: Vec<Light> = Vec::new();
        let mut labels: Vec<&str> = Vec::new();
        let blur_sigma: scalar = 2.5;

        // Example 1: Top-left light, medium ambient, low specular
        emboss_recs.push(Light {
            direction: [-1.0, -1.0, 1.0],
            pad: 0,
            ambient: 80,
            specular: 16,
        });
        labels.push("Light: TL, Amb: 80, Spec: 16");

        // Example 2: Bottom-right light, high ambient, low specular
        emboss_recs.push(Light {
            direction: [1.0, 1.0, 0.8],
            pad: 0,
            ambient: 180,
            specular: 16,
        });
        labels.push("Light: BR, Amb: 180, Spec: 16");

        // Example 3: Direct top light, low ambient, high specular
        emboss_recs.push(Light {
            direction: [0.0, -1.0, 1.0],
            pad: 0,
            ambient: 30,
            specular: 128,
        });
        labels.push("Light: Top, Amb: 30, Spec: 128");

        // Example 4: Left light, medium ambient, medium specular
        emboss_recs.push(Light {
            direction: [-1.0, 0.0, 0.5],
            pad: 0,
            ambient: 80,
            specular: 64,
        });
        labels.push("Light: Left, Amb: 80, Spec: 64");

        let mut example_index = 0;
        for r in 0..grid_rows {
            for c in 0..grid_cols {
                if example_index < emboss_recs.len() {
                    // Calculate bounds within the top half
                    let cell_bounds = Rect::from_xywh(
                        c as f32 * cell_width,
                        r as f32 * cell_height,
                        cell_width,
                        cell_height,
                    );
                    draw_emboss_example(
                        canvas,
                        cell_bounds.with_inset((10.0, 10.0)), // Add padding
                        &emboss_recs[example_index],
                        blur_sigma,
                        labels[example_index],
                    );
                    example_index += 1;
                }
            }
        }
    }

    // Test different blend modes with the mask filter
    {
        let grid_cols = 6;
        let grid_rows = 3;
        let cell_width = total_width as f32 / grid_cols as f32;
        let cell_height = bottom_height as f32 / grid_rows as f32;

        let emboss_rec = Light {
            direction: [-0.707, -0.707, 0.707],
            pad: 0,
            ambient: 60,
            specular: 48,
        };
        let blur_sigma: scalar = 2.0;

        let Some(emboss_mf) = emboss_mask_filter::new(blur_sigma, &emboss_rec) else {
            // SkDebugf("Failed to create shared emboss filter for blend examples.\n");
            return;
        };

        // Blend Modes to Test
        let blend_modes_to_test = [
            BlendMode::Src, // Incorrect for Graphite
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,  // Incorrect for Graphite
            BlendMode::DstIn,  // Incorrect for Graphite
            BlendMode::SrcOut, // Incorrect for Graphite
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::DstATop, // Incorrect for Graphite
            BlendMode::Xor,
            BlendMode::Plus, // Incorrect for Graphite
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Multiply,
            BlendMode::Darken,
            BlendMode::Lighten,
        ];

        let mut example_index = 0;
        for r in 0..grid_rows {
            for c in 0..grid_cols {
                if example_index < blend_modes_to_test.len() {
                    let current_mode = blend_modes_to_test[example_index];
                    let mode_name = BLEND_MODE_STRS
                        .iter()
                        .find(|(mode, _)| *mode == current_mode)
                        .map_or("Unknown", |(_, name)| *name);

                    // Calculate bounds within the bottom half (offset by topHeight)
                    let cell_bounds = Rect::from_xywh(
                        c as f32 * cell_width,
                        top_height as f32 + r as f32 * cell_height, // Offset Y
                        cell_width,
                        cell_height,
                    );
                    draw_emboss_blend_example(
                        canvas,
                        cell_bounds.with_inset((10.0, 10.0)), // Add padding
                        emboss_mf.clone(),                    // Use the same filter instance
                        current_mode,
                        mode_name,
                    );
                    example_index += 1;
                }
            }
        }
    }
});

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
