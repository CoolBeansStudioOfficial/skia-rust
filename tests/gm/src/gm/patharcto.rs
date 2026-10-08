// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/patharcto.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::{AddPathMode, Path};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::utils::parse_path;

// Port of: gm/patharcto.cpp#L15-L40 (chrome/m156)
// Port of: gm/patharcto.cpp#L15-L40 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn draw_shallow_angle_path_arcto(canvas: &Canvas) {
    let mut path = PathBuilder::new();
    let mut paint = Paint::default();
    paint.set_style(skia_rust_core::paint::Style::Stroke);

    path.move_to((313.441_890_963_311_55, 106.600_942_358_921_2))
        .arc_to_tangent(
            (284.311_308_200_846_2, 207.140_771_915_706_3),
            (255.150_537_771_297_28, 307.671_850_541_637_4),
            697_212.001_105_452_4,
        )
        .line_to((255.150_537_771_297_28, 307.671_850_541_637_4))
        .arc_to_tangent(
            (340.473_746_598_101_8, 252.690_731_934_697_1),
            (433.543_334_777_161_53, 212.181_163_633_453_37),
            1_251.248_427_790_725_1,
        )
        .line_to((433.543_334_777_161_53, 212.181_163_633_453_37))
        .arc_to_tangent(
            (350.195_138_338_394_66, 185.892_800_148_383_69),
            (313.441_890_963_311_55, 106.600_942_358_921_2),
            198.031_168_853_278_13,
        );

    canvas.translate((-200.0, -50.0));
    canvas.draw_path(&path.detach(), &paint);
}

// Port of: gm/patharcto.cpp#L15-L40 (chrome/m156)
crate::def_simple_gm!(shallow_angle_path_arcto, canvas, 300, 300, {
    draw_shallow_angle_path_arcto(canvas);
});

// Port of: gm/patharcto.cpp#L42-L56 (chrome/m156)
fn draw_arcto_skbug_9272(canvas: &Canvas) {
    let str_ = "M66.652,65.509c0.663,-2 -0.166,-4.117 -2.117,-5.212 -0.673,-0.378 -1.36,-0.733 -2.04,-1.1a1647300864,1647300864 0,0 1,-31.287 -16.86c-5.39,-2.903 -10.78,-5.808 -16.171,-8.713 -1.626,-0.876 -3.253,-1.752 -4.88,-2.63 -1.224,-0.659 -2.4,-1.413 -3.851,-1.413 -1.135,0 -2.242,0.425 -3.049,1.197 0.08,-0.083 0.164,-0.164 0.248,-0.246l5.309,-5.13 9.37,-9.054 9.525,-9.204 5.903,-5.704C34.237,0.836 34.847,0.297 35.75,0.13c0.982,-0.182 1.862,0.127 2.703,0.592l6.23,3.452L55.76,10.31l11.951,6.62 9.02,4.996c1.74,0.963 4.168,1.854 4.205,4.21 0.011,0.678 -0.246,1.28 -0.474,1.9l-1.005,2.733 -5.665,15.42 -7.106,19.338 -0.034,-0.018z";
    let path = parse_path::from_svg(str_).unwrap_or_default();

    let str2 = "M10.156,30.995l4.881,2.63 16.17,8.713a1647300736,1647300736 0,0 0,31.287 16.86c0.68,0.366 1.368,0.721 2.041,1.1 2.242,1.257 3.002,3.864 1.72,6.094 -0.659,1.147 -1.296,2.31 -1.978,3.442 -1.276,2.117 -3.973,2.632 -6.102,1.536 -0.244,-0.125 -0.485,-0.259 -0.727,-0.388l-4.102,-2.19 -15.401,-8.225 -18.536,-9.9 -13.893,-7.419c-0.939,-0.501 -1.88,-0.998 -2.816,-1.504C1.2,40.935 0.087,39.5 0.004,37.75c-0.08,-1.672 1.078,-3.277 1.826,-4.702 0.248,-0.471 0.479,-0.958 0.75,-1.416 0.772,-1.306 2.224,-2.05 3.726,-2.05 1.45,0 2.627,0.754 3.85,1.414z";
    let path2 = parse_path::from_svg(str2).unwrap_or_default();

    let mut paint = Paint::default();
    paint.set_style(skia_rust_core::paint::Style::Stroke);
    canvas.translate((30.0, 30.0));
    canvas.draw_path(&path, &paint);
    canvas.draw_path(&path2, &paint);
}

// Port of: gm/patharcto.cpp#L42-L56 (chrome/m156)
crate::def_simple_gm!(arcto_skbug_9272, canvas, 150, 150, {
    draw_arcto_skbug_9272(canvas);
});

// Port of: gm/patharcto.cpp#L58-L62 (chrome/m156)
fn old_school_polygon(pts: &[Point], is_closed: bool) -> Path {
    let mut builder = PathBuilder::new();
    builder.add_polygon(pts, is_closed);
    builder.detach()
}

// Port of: gm/patharcto.cpp#L64-L66 (chrome/m156)
fn new_school_polygon(pts: &[Point], is_closed: bool) -> Path {
    Path::polygon(pts, is_closed, None, None)
}

// Port of: gm/patharcto.cpp#L68-L135 (chrome/m156)
#[allow(clippy::too_many_lines)]
fn draw_path_append_extend(canvas: &Canvas) {
    let p0 = [
        Point::new(10.0, 30.0),
        Point::new(30.0, 10.0),
        Point::new(50.0, 30.0),
    ];
    let p1 = [
        Point::new(10.0, 50.0),
        Point::new(30.0, 70.0),
        Point::new(50.0, 50.0),
    ];

    let path1 = Path::polygon(&p1, false, None, None);

    let mut paint = Paint::default();
    paint.set_style(skia_rust_core::paint::Style::Stroke);
    paint.set_stroke_width(9.0);
    paint.set_anti_alias(true);

    // addPath() sometimes checks for perspective, so we want to test that
    let x: f32 = 0.0001; // tiny amount of perspective
    let perspective = Matrix::new_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0, 1.0);

    let procs: [fn(&[Point], bool) -> Path; 2] = [old_school_polygon, new_school_polygon];
    for is_closed in [false, true] {
        for proc_ in procs {
            canvas.save();

            let path0 = proc_(&p0, is_closed);

            canvas.draw_path(&path0, &paint);
            canvas.draw_path(&path1, &paint);

            canvas.translate((80.0, 0.0));
            {
                let path = PathBuilder::new_path(&path0)
                    .add_path(&path1, AddPathMode::Append)
                    .detach();
                canvas.draw_path(&path, &paint);
            }

            canvas.translate((80.0, 0.0));
            {
                let path = PathBuilder::new_path(&path0)
                    .add_path_with_transform(&path1, &perspective, AddPathMode::Append)
                    .detach();
                canvas.draw_path(&path, &paint);
            }

            canvas.translate((80.0, 0.0));
            {
                let path = PathBuilder::new_path(&path0)
                    .add_path(&path1, AddPathMode::Extend)
                    .detach();
                canvas.draw_path(&path, &paint);
            }

            canvas.translate((80.0, 0.0));
            {
                let path = PathBuilder::new_path(&path0)
                    .add_path_with_transform(&path1, &perspective, AddPathMode::Extend)
                    .detach();
                canvas.draw_path(&path, &paint);
            }

            canvas.restore();
            canvas.translate((0.0, 100.0));
        }
    }
}

crate::def_simple_gm!(path_append_extend, canvas, 400, 400, {
    draw_path_append_extend(canvas);
});
