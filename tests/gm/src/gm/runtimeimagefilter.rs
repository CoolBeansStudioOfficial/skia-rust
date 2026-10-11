// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimeimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{color_to_565, get_resource_as_image};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, runtime_shader, runtime_shader_children};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/runtimeimagefilter.cpp#L21-L42 (chrome/m156), make_filter
fn make_filter() -> Option<ImageFilter> {
    let effect = RuntimeEffect::make_for_shader(
        r"
        uniform shader child;
        half4 main(float2 coord) {
            coord.x += sin(coord.y / 3) * 4;
            return child.eval(coord);
        }
    ",
        None,
    )
    .expect("the distort effect compiles");
    let builder = RuntimeShaderBuilder::new(effect);
    // SkImageFilters::RuntimeShader(builder, /*sampleRadius=*/4, /*childShaderName=*/"",
    // /*input=*/nullptr)
    runtime_shader(&builder, 4.0, "", None, false)
}

// Port of: gm/runtimeimagefilter.cpp#L44-L79 (chrome/m156), rtif_distort
crate::def_simple_gm_bg!(
    #[ignore = "see notes/gm_runtimeimagefilter_cpp_rtif_distort.md"]
    rtif_distort,
    canvas,
    500,
    750,
    Color::BLACK,
    {
        let clip = Rect::from_wh(250.0, 250.0);
        let mut filter_paint = Paint::default();
        filter_paint.set_image_filter(make_filter());

        let draw_layer = |tx: f32, ty: f32, m: Matrix| {
            canvas.save();
            canvas.translate((tx, ty));
            canvas.clip_rect(clip, None, None);
            canvas.concat(&m);
            canvas.save_layer(&SaveLayerRec::default().paint(&filter_paint));
            let text = "The quick brown fox jumped over the lazy dog.";
            let mut rand = Random::default();
            let mut font = default_portable_font();
            for _ in 0..25 {
                let x = rand.next_u_less_than(500);
                let y = rand.next_u_less_than(500);
                let mut paint = Paint::default();
                paint.set_color(color_to_565(Color::new(rand.next_bits(24) | 0xFF00_0000)));
                font.set_size(rand.next_range_scalar(0.0, 300.0));
                // SkIntToScalar(x): x and y are below 500, so the casts are exact.
                #[allow(clippy::cast_precision_loss)]
                canvas.draw_str(text, (x as f32, y as f32), &font, &paint);
            }
            canvas.restore();
            canvas.restore();
        };

        draw_layer(0.0, 0.0, Matrix::new_identity());
        draw_layer(250.0, 0.0, Matrix::scale((0.5, 0.5)));
        draw_layer(0.0, 250.0, Matrix::rotate_deg_pivot(45.0, (125.0, 125.0)));
        draw_layer(
            250.0,
            250.0,
            Matrix::concat(
                &Matrix::scale((0.5, 0.5)),
                &Matrix::rotate_deg_pivot(45.0, (125.0, 125.0)),
            ),
        );
        draw_layer(0.0, 500.0, Matrix::skew((-0.5, 0.0)));
        let mut p = Matrix::new_identity();
        p.set_persp_x(0.0015);
        p.set_persp_y(-0.0015);
        draw_layer(250.0, 500.0, p);
    }
);

// Port of: gm/runtimeimagefilter.cpp#L81-L108 (chrome/m156), rtif_unsharp
crate::def_simple_gm!(rtif_unsharp, canvas, 512, 256, {
    // Similar to "unsharp_rt", which does the entire unsharp filter in a single shader. This uses
    // the image filter DAG to compute the blurred version, then does the weighted subtraction.
    let effect = RuntimeEffect::make_for_shader(
        r"
        uniform shader content;
        uniform shader blurred;
        vec4 main(vec2 coord) {
            vec4 c = content.eval(coord);
            vec4 b = blurred.eval(coord);
            return c + (c - b) * 4;
        }
    ",
        None,
    )
    .expect("the unsharp effect compiles");
    let builder = RuntimeShaderBuilder::new(effect);

    let image = get_resource_as_image("images/mandrill_256.png")
        .expect("images/mandrill_256.png (set SKIA_RESOURCES)");
    // SkImageFilters::Blur(1, 1, /*input=*/nullptr) defaults to the decal tile mode.
    let blurred_src = blur(1.0, 1.0, TileMode::Decal, None, None);

    let child_names = ["content", "blurred"];
    let child_nodes = [None, blurred_src];
    let sharpened = runtime_shader_children(&builder, 0.0, &child_names, &child_nodes, false);

    canvas.draw_image(&image, (0.0, 0.0), None);
    canvas.translate((256.0, 0.0));

    let mut paint = Paint::default();
    paint.set_image_filter(sharpened);
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&Rect::from_ltrb(0.0, 0.0, 256.0, 256.0))
            .paint(&paint),
    );
    canvas.draw_image(&image, (0.0, 0.0), None);
    canvas.restore();
});
