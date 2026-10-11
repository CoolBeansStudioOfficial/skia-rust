// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/backdrop.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color::colors;
use skia_rust_core::matrix_priv;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::image_filters::blur;

// Make a noisy (with hard-edges) background, so we can see the effect of the blur
// Port of: gm/backdrop.cpp#L29-L40 (chrome/m156), make_shader
// (`rad` is not used by the shader, as in Skia.)
fn make_shader(cx: f32, cy: f32) -> Option<Shader> {
    let colors = [
        colors::RED,
        colors::RED,
        colors::BLUE,
        colors::BLUE,
        colors::GREEN,
        colors::GREEN,
        colors::RED,
        colors::RED,
        colors::BLUE,
        colors::BLUE,
        colors::GREEN,
        colors::GREEN,
    ];
    let mut pos: [f32; 12] = [0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0];
    for p in &mut pos {
        *p *= 1.0 / 6.0;
    }
    // SkShaders::SweepGradient(center, grad) sweeps 0 to 360 degrees.
    shaders::sweep_gradient(
        (cx, cy),
        (0.0, 360.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/backdrop.cpp#L42-L76 (chrome/m156), do_draw
fn do_draw(canvas: &Canvas, use_clip: bool, use_hint_rect: bool, scale_factor: f32) {
    let guard = AutoCanvasRestore::guard(canvas, true);
    let canvas: &Canvas = &guard;
    canvas.clip_rect(Rect::from_wh(256.0, 256.0), None, None);

    let (cx, cy, rad) = (128.0_f32, 128.0_f32, 100.0_f32);
    let mut p = Paint::default();
    p.set_shader(make_shader(cx, cy));
    p.set_anti_alias(true);
    canvas.draw_circle((cx, cy), rad, &p);

    // now setup a saveLayer that will pull in the backdrop and blur it
    //
    let r = Rect::new(cx - 50.0, cy - 50.0, cx + 50.0, cy + 50.0);
    let drawr: Option<&Rect> = if use_hint_rect { Some(&r) } else { None };
    let sigma: f32 = 10.0;
    if use_clip {
        canvas.clip_rect(r, None, None);
    }

    // Using kClamp because kDecal, the default, produces transparency near the edge of the canvas's
    // device.
    let device_bounds = Rect::from_wh(
        int_to_scalar(canvas.image_info().width()),
        int_to_scalar(canvas.image_info().height()),
    );
    let blur_crop = matrix_priv::inverse_map_rect(&canvas.local_to_device_as_3x3(), &device_bounds)
        .expect("InverseMapRect");
    let blur_filter =
        blur(sigma, sigma, TileMode::Clamp, None, Some(blur_crop)).expect("a blur filter");

    let mut rec = SaveLayerRec::default()
        .backdrop(&blur_filter)
        .backdrop_tile_mode(TileMode::Clamp)
        .experimental_backdrop_scale(scale_factor);
    if let Some(bounds) = drawr {
        rec = rec.bounds(bounds);
    }
    canvas.save_layer(&rec);
    // draw something inside, just to demonstrate that we don't blur the new contents,
    // just the backdrop.
    p.set_color(Color::YELLOW);
    p.set_shader(None);
    canvas.draw_circle((cx, cy), 30.0, &p);
    canvas.restore();
}

/*
 *  Draws a 3x4 grid of sweep circles.
 *  - for a given row, each col should be identical (canvas, picture)
 *  - row:0     no-hint-rect    no-clip-rect        expect big blur (except inner circle)
 *  - row:1     no-hint-rect    clip-rect           expect small blur (except inner circle)
 *  - row:2     hint-rect       no-clip-rect        expect big blur (except inner circle)
 *  - row:3     hint-rect       clip-rect           expect small blur (except inner circle)
 *
 *  The test is that backdrop effects should be independent of the hint-rect, but should
 *  respect the clip-rect.
 */
// Port of: gm/backdrop.cpp#L89-L104 (chrome/m156), DEF_SIMPLE_GM(backdrop_hintrect_clipping)
crate::def_simple_gm!(backdrop_hintrect_clipping, canvas, 512, 1024, {
    for use_hint_rect in [false, true] {
        for use_clip in [false, true] {
            canvas.save();
            do_draw(canvas, use_clip, use_hint_rect, 1.0);

            let mut rec = PictureRecorder::new();
            do_draw(
                rec.begin_recording(Rect::from_wh(256.0, 256.0), false),
                use_clip,
                use_hint_rect,
                1.0,
            );
            canvas.translate((256.0, 0.0));
            if let Some(picture) = rec.finish_recording_as_picture(None) {
                canvas.draw_picture(&picture, None, None);
            }
            canvas.restore();

            canvas.translate((0.0, 256.0));
        }
    }
});

/*
 *  Draws a 2x4 grid of sweep circles.
 *  - for a given row, each col should be identical except that the intermediate scale factor used
 *    to evaluate the backdrop follows (1.0, 0.25, 0.1). Rows follow same pattern as above.
 *
 *  The test is that backdrop effects should be independent of the hint-rect, should respect the
 *  clip rect, and be logically consistent with the reduced intermediate scaling.
 */
// Port of: gm/backdrop.cpp#L114-L128 (chrome/m156), DEF_SIMPLE_GM(backdrop_scalefactor)
crate::def_simple_gm!(backdrop_scalefactor, canvas, 768, 1024, {
    for use_hint_rect in [false, true] {
        for use_clip in [false, true] {
            canvas.save();
            do_draw(canvas, use_clip, use_hint_rect, 1.0);
            canvas.translate((256.0, 0.0));
            do_draw(canvas, use_clip, use_hint_rect, 0.25);
            canvas.translate((256.0, 0.0));
            do_draw(canvas, use_clip, use_hint_rect, 0.1);
            canvas.restore();

            canvas.translate((0.0, 256.0));
        }
    }
});
