// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1257515.cpp (chrome/m156)

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
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/crbug_1257515.cpp#L12-L83 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
#[allow(clippy::too_many_lines)]
fn draw_crbug_1257515(canvas: &Canvas) {
    // <svg width="1139" height="400" viewBox="0 0 1139 400">
    //    <g transform="translate(46,60) scale(1 1)">
    //       <path fill="none" d="M 45.125 102.53701800000002
    //                            L 135.375 162.666156 L 225.625 116.622276
    //                            L 315.875 121.52087700000001 L 406.125 134.632899
    //                            L 496.375 192.317736 L 586.625 138.82944899999998
    //                            L 676.875 234.212031 L 767.125 207.082926 L 857.375 128.083857
    //                            L 947.625 127.95689999999999 L 1037.875 113.956785"
    //             stroke="red" stroke-width="2" stroke-linejoin="round" stroke-linecap="round">
    //       </path>
    //    </g>
    // </svg>
    let mut b = PathBuilder::new();
    b.move_to((45.125, 102.537_018_000_000_02))
        .line_to((135.375, 162.666_156))
        .line_to((225.625, 116.622_276))
        .line_to((315.875, 121.520_877_000_000_01))
        .line_to((406.125, 134.632_899))
        .line_to((496.375, 192.317_736))
        .line_to((586.625, 138.829_448_999_999_98))
        .line_to((676.875, 234.212_031))
        .line_to((767.125, 207.082_926))
        .line_to((857.375, 128.083_857))
        .line_to((947.625, 127.956_899_999_999_99))
        .line_to((1037.875, 113.956_785));

    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_stroke_width(2.0);
    p.set_style(Style::Stroke);
    p.set_stroke_cap(Cap::Round);
    p.set_stroke_join(Join::Round);
    p.set_anti_alias(true);

    canvas.save();
    canvas.translate((-50.0, -200.0));
    canvas.scale((2.0, 2.0));
    canvas.draw_path(&b.detach(), &p);
    canvas.restore();

    // <svg width="1148" height="700" viewBox="0 0 1148 700">
    //    <path fill="none" d="M 129.5307 587.5728 L 232.4748 617.037 L 335.4189 624.8472
    //                         L 438.3631 630.5933 L 541.3073 625.1138 L 644.2513 626.8717
    //                         L 747.1955 629.9542 L 850.1396 629.6956 L 953.0838 616.4909
    //                         L 1056.028 613.8181"
    //          stroke="rgba(47,136,255,1)" stroke-width="3"
    //          stroke-linecap="butt" stroke-linejoin="bevel" stroke-miterlimit="10">
    //    </path>
    // </svg>
    b.move_to((128.5307, 587.5728))
        .line_to((232.4748, 617.037))
        .line_to((335.4189, 624.8472))
        .line_to((438.3631, 630.5933))
        .line_to((541.3073, 625.1138))
        .line_to((644.2513, 626.8717))
        .line_to((747.1955, 629.9542))
        .line_to((850.1396, 629.6956))
        .line_to((953.0838, 616.4909))
        .line_to((1056.028, 613.8181));
    p.set_color(Color::from_argb(255, 47, 136, 255));
    p.set_stroke_width(3.0);
    p.set_stroke_cap(Cap::Butt);
    p.set_stroke_join(Join::Bevel);
    p.set_stroke_miter(10.0);

    canvas.save();
    canvas.translate((-300.0, -900.0));
    canvas.scale((2.0, 2.0));
    canvas.draw_path(&b.detach(), &p);
    canvas.restore();
}

// Port of: gm/crbug_1257515.cpp#L12-L83 (chrome/m156)
crate::def_simple_gm!(crbug_1257515, canvas, 1139, 400, {
    draw_crbug_1257515(canvas);
});
