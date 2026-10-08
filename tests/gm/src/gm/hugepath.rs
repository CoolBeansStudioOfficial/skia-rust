// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hugepath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::{AutoCanvasRestore, SrcRectConstraint};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tiled_image_utils;
use skia_rust_raster::surfaces;

// Port of: gm/hugepath.cpp#L17-L36 (chrome/m156)
crate::def_simple_gm!(path_huge_crbug_800804, canvas, 50, 600, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);

    // exercise various special-cases (e.g. hairlines or not)
    let widths = [0.9_f32, 1.0, 1.1];

    for w in widths {
        paint.set_stroke_width(w);

        #[allow(clippy::excessive_precision)] // the C++ literal, digit for digit
        let path = Path::line((-1000.0, 12_345_678_901_234_567_890.0_f32), (10.5, 200.0));
        canvas.draw_path(&path, &paint);

        #[allow(clippy::excessive_precision)] // the C++ literal, digit for digit
        let path = Path::line(
            (30.5, 400.0),
            (1000.0, -9.876_543_210_987_654_321_0e+19_f32),
        );
        canvas.draw_path(&path, &paint);

        canvas.translate((3.0, 0.0));
    }
});

// Test that we can draw into a huge surface ( > 64K ) and still retain paths and antialiasing.
// Port of: gm/hugepath.cpp#L39-L70 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to SkScalar
fn draw_huge_path(canvas: &Canvas, w: i32, h: i32, manual: bool) {
    let _acr = AutoCanvasRestore::guard(canvas, true);

    let mut surf =
        surfaces::raster(&ImageInfo::new_n32_premul((w, h), None), None, None).expect("a surface");

    let mut paint = Paint::default();
    let path = Path::rrect_xy(
        Rect::from_xywh(4.0, 4.0, (w - 8) as f32, (h - 8) as f32),
        12.0,
        12.0,
        None,
    );

    let draw = |canvas: &Canvas, image: &skia_rust_core::image::Image| {
        if manual {
            tiled_image_utils::draw_image(
                canvas,
                image,
                (64 - w) as f32,
                0.0,
                &SamplingOptions::default(),
                None,
                SrcRectConstraint::Fast,
            );
        } else {
            canvas.draw_image(image, ((64 - w) as f32, 0.0), None);
        }
    };

    canvas.save();
    canvas.clip_rect(Rect::from_xywh(4.0, 4.0, 64.0, 64.0), None, None);
    surf.canvas().draw_path(&path, &paint);
    draw(canvas, &surf.image_snapshot().expect("a snapshot"));
    canvas.restore();

    canvas.translate((80.0, 0.0));
    canvas.save();
    canvas.clip_rect(Rect::from_xywh(4.0, 4.0, 64.0, 64.0), None, None);
    surf.canvas().clear(Color::new(0));
    paint.set_anti_alias(true);
    surf.canvas().draw_path(&path, &paint);
    draw(canvas, &surf.image_snapshot().expect("a snapshot"));
    canvas.restore();
}

// Port of: gm/hugepath.cpp#L72-L76 (chrome/m156)
crate::def_simple_gm!(path_huge_aa, canvas, 200, 200, {
    draw_huge_path(canvas, 100, 60, /* manual= */ false);
    canvas.translate((0.0, 80.0));
    draw_huge_path(canvas, 100 * 1024, 60, /* manual= */ false);
});

// Port of: gm/hugepath.cpp#L78-L82 (chrome/m156)
crate::def_simple_gm!(path_huge_aa_manual, canvas, 200, 200, {
    draw_huge_path(canvas, 100, 60, /* manual= */ true);
    canvas.translate((0.0, 80.0));
    draw_huge_path(canvas, 100 * 1024, 60, /* manual= */ true);
});
