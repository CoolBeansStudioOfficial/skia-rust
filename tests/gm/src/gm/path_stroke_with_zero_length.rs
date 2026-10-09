// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/path_stroke_with_zero_length.cpp (chrome/m156)

// GM to test combinations of stroking zero length paths with different caps and other settings
// Variables:
// * Antialiasing: On, Off
// * Caps: Butt, Round, Square
// * Stroke width: 0, 0.9, 1, 1.1, 15, 25
// * Path form: M, ML, MLZ, MZ
// * Path contours: 1 or 2
// * Path verbs: Line, Quad, Cubic, Conic
//
// Each test is drawn to a 50x20 offscreen surface, and expected to produce some number (0 - 2) of
// visible pieces of cap geometry. These are counted by scanning horizontally for peaks (blobs).

// The int/float mixing and sizes of the GM mirror the C++ arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::utils::parse_path;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Port of: gm/path_stroke_with_zero_length.cpp#L19-L36 (chrome/m156), draw_path_cell
fn draw_path_cell(
    canvas: &Canvas,
    surface: &mut skia_rust_raster::surface::Surface<'static>,
    expected_caps: i32,
) {
    const K_FAILURE_RED: Color = Color::new(0x7F7F_0000);
    const K_FAILURE_YELLOW: Color = Color::new(0x7F7F_7F00);
    const K_SUCCESS_GREEN: Color = Color::new(0x7F00_7F00);

    let w = surface.width();
    let h = surface.height();

    // Read the pixels back
    let info = ImageInfo::new_n32_premul((w, h), None);
    let row_bytes = w as usize * 4;
    let mut pixels = vec![0_u8; row_bytes * h as usize];
    if !surface.read_pixels(&info, &mut pixels, row_bytes, (0, 0)) {
        return;
    }
    // The packed N32 pixel at (x, y), and its red channel (`SkGetPackedR32`).
    let red_at = |x: i32, y: i32| -> u32 {
        let off = (y as usize * w as usize + x as usize) * 4;
        let px = u32::from_ne_bytes([
            pixels[off],
            pixels[off + 1],
            pixels[off + 2],
            pixels[off + 3],
        ]);
        (px >> 16) & 0xFF
    };

    // To account for rasterization differences, we scan the middle two rows [y, y+1] of the image
    assert_eq!(h % 2, 0);
    let y = (h - 1) / 2;

    let mut in_blob = false;
    let mut num_blobs = 0;
    for x in 0..w {
        // We drew white-on-black. We can look for any non-zero value. Just check red.
        // And we care if either row is non-zero, so just add them to simplify everything.
        let v = red_at(x, y) + red_at(x, y + 1);

        if !in_blob && v != 0 {
            num_blobs += 1;
        }
        in_blob = v != 0;
    }

    let result_color = match num_blobs.cmp(&expected_caps) {
        std::cmp::Ordering::Equal => K_SUCCESS_GREEN, // Green
        // Yellow -- more geometry than expected
        std::cmp::Ordering::Greater => K_FAILURE_YELLOW,
        // Red -- missing some geometry
        std::cmp::Ordering::Less => K_FAILURE_RED,
    };
    let mut result = Paint::default();
    result.set_color(result_color);

    let img = surface.image_snapshot().expect("a snapshot");
    canvas.draw_image(&img, (0.0, 0.0), None);
    canvas.draw_rect(Rect::from_wh(w as scalar, h as scalar), &result);
}

// Port of: gm/path_stroke_with_zero_length.cpp#L38-L40 (chrome/m156), kCaps
const K_CAPS: [Cap; 3] = [Cap::Butt, Cap::Round, Cap::Square];

// Port of: gm/path_stroke_with_zero_length.cpp#L42-L42 (chrome/m156), kWidths
const K_WIDTHS: [scalar; 6] = [0.0, 0.9, 1.0, 1.1, 15.0, 25.0];

// Full set of path structures for single contour case (each primitive with and without a close)
// Port of: gm/path_stroke_with_zero_length.cpp#L44-L56 (chrome/m156), kAllVerbs
const K_ALL_VERBS: [Option<&str>; 10] = [
    None,
    Some("z "),
    Some("l 0 0 "),
    Some("l 0 0 z "),
    Some("q 0 0 0 0 "),
    Some("q 0 0 0 0 z "),
    Some("c 0 0 0 0 0 0 "),
    Some("c 0 0 0 0 0 0 z "),
    Some("a 0 0 0 0 0 0 0 "),
    Some("a 0 0 0 0 0 0 0 z "),
];

// Reduced set of path structures for double contour case, to keep total number of cases down
// Port of: gm/path_stroke_with_zero_length.cpp#L58-L65 (chrome/m156), kSomeVerbs
const K_SOME_VERBS: [Option<&str>; 6] = [
    None,
    Some("z "),
    Some("l 0 0 "),
    Some("l 0 0 z "),
    Some("q 0 0 0 0 "),
    Some("q 0 0 0 0 z "),
];

// Port of: gm/path_stroke_with_zero_length.cpp#L67-L69 (chrome/m156), cell geometry
const K_CELL_WIDTH: i32 = 50;
const K_CELL_HEIGHT: i32 = 20;
const K_CELL_PAD: i32 = 2;

// Port of: gm/path_stroke_with_zero_length.cpp#L71-L76 (chrome/m156), totals
const K_NUM_ROWS: i32 = (K_CAPS.len() * K_WIDTHS.len()) as i32;
const K_NUM_COLUMNS: i32 = K_ALL_VERBS.len() as i32;
const K_TOTAL_WIDTH: i32 = K_NUM_COLUMNS * (K_CELL_WIDTH + K_CELL_PAD) + K_CELL_PAD;
const K_TOTAL_HEIGHT: i32 = K_NUM_ROWS * (K_CELL_HEIGHT + K_CELL_PAD) + K_CELL_PAD;

const K_DBL_CONTOUR_NUM_COLUMNS: i32 = (K_SOME_VERBS.len() * K_SOME_VERBS.len()) as i32;
const K_DBL_CONTOUR_TOTAL_WIDTH: i32 =
    K_DBL_CONTOUR_NUM_COLUMNS * (K_CELL_WIDTH + K_CELL_PAD) + K_CELL_PAD;

// Port of: gm/path_stroke_with_zero_length.cpp#L80-L104 (chrome/m156), draw_zero_length_capped_paths
fn draw_zero_length_capped_paths(canvas: &Canvas, aa: bool) {
    canvas.translate((K_CELL_PAD as scalar, K_CELL_PAD as scalar));

    let info = canvas
        .image_info()
        .with_dimensions((K_CELL_WIDTH, K_CELL_HEIGHT));
    let mut surface = canvas
        .new_surface(&info, None)
        .or_else(|| {
            surfaces::raster(
                &ImageInfo::new_n32_premul((K_CELL_WIDTH, K_CELL_HEIGHT), None),
                None,
                None,
            )
        })
        .expect("a surface");

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(aa);
    paint.set_style(Style::Stroke);

    for cap in K_CAPS {
        for width in K_WIDTHS {
            paint.set_stroke_cap(cap);
            paint.set_stroke_width(width);
            canvas.save();

            for verb in K_ALL_VERBS {
                let mut path_str = format!(
                    "M {:.6} {:.6} ",
                    (K_CELL_WIDTH - 1) as f32 * 0.5,
                    (K_CELL_HEIGHT - 1) as f32 * 0.5
                );
                if let Some(verb) = verb {
                    path_str.push_str(verb);
                }

                let path: Path = parse_path::from_svg(&path_str).unwrap_or_else(Path::new);

                surface.canvas().clear(Color::TRANSPARENT);
                surface.canvas().draw_path(&path, &paint);

                // All cases should draw one cap, except for butt capped, and dangling moves
                // (without a verb or close), which shouldn't draw anything.
                let expected_caps = i32::from(!(cap == Cap::Butt || verb.is_none()));

                draw_path_cell(canvas, &mut surface, expected_caps);
                canvas.translate(((K_CELL_WIDTH + K_CELL_PAD) as scalar, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, (K_CELL_HEIGHT + K_CELL_PAD) as scalar));
        }
    }
}

// Port of: gm/path_stroke_with_zero_length.cpp#L106-L118 (chrome/m156), zero_length_paths_aa
crate::def_simple_gm_bg_can_fail!(
    zero_length_paths_aa,
    canvas,
    error_msg,
    K_TOTAL_WIDTH,
    K_TOTAL_HEIGHT,
    Color::BLACK,
    {
        let _ = error_msg;
        draw_zero_length_capped_paths(canvas, true);
        DrawResult::Ok
    }
);

// Port of: gm/path_stroke_with_zero_length.cpp#L106-L118 (chrome/m156), zero_length_paths_bw
crate::def_simple_gm_bg_can_fail!(
    zero_length_paths_bw,
    canvas,
    error_msg,
    K_TOTAL_WIDTH,
    K_TOTAL_HEIGHT,
    Color::BLACK,
    {
        let _ = error_msg;
        draw_zero_length_capped_paths(canvas, false);
        DrawResult::Ok
    }
);

// Port of: gm/path_stroke_with_zero_length.cpp#L120-L175 (chrome/m156), draw_zero_length_capped_paths_dbl_contour
fn draw_zero_length_capped_paths_dbl_contour(canvas: &Canvas, aa: bool) {
    canvas.translate((K_CELL_PAD as scalar, K_CELL_PAD as scalar));

    let info = canvas
        .image_info()
        .with_dimensions((K_CELL_WIDTH, K_CELL_HEIGHT));
    let mut surface = canvas
        .new_surface(&info, None)
        .or_else(|| {
            surfaces::raster(
                &ImageInfo::new_n32_premul((K_CELL_WIDTH, K_CELL_HEIGHT), None),
                None,
                None,
            )
        })
        .expect("a surface");

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(aa);
    paint.set_style(Style::Stroke);

    for cap in K_CAPS {
        for width in K_WIDTHS {
            paint.set_stroke_cap(cap);
            paint.set_stroke_width(width);
            canvas.save();

            for first_verb in K_SOME_VERBS {
                for second_verb in K_SOME_VERBS {
                    let mut expected_caps = 0;

                    let mut path_str = String::from("M 9.5 9.5 ");
                    if let Some(first_verb) = first_verb {
                        path_str.push_str(first_verb);
                        expected_caps += 1;
                    }
                    path_str.push_str("M 40.5 9.5 ");
                    if let Some(second_verb) = second_verb {
                        path_str.push_str(second_verb);
                        expected_caps += 1;
                    }

                    let path: Path = parse_path::from_svg(&path_str).unwrap_or_else(Path::new);

                    surface.canvas().clear(Color::TRANSPARENT);
                    surface.canvas().draw_path(&path, &paint);

                    if cap == Cap::Butt {
                        expected_caps = 0;
                    }

                    draw_path_cell(canvas, &mut surface, expected_caps);
                    canvas.translate(((K_CELL_WIDTH + K_CELL_PAD) as scalar, 0.0));
                }
            }
            canvas.restore();
            canvas.translate((0.0, (K_CELL_HEIGHT + K_CELL_PAD) as scalar));
        }
    }
}

// Port of: gm/path_stroke_with_zero_length.cpp#L177-L191 (chrome/m156), zero_length_paths_dbl_aa
crate::def_simple_gm_bg_can_fail!(
    zero_length_paths_dbl_aa,
    canvas,
    error_msg,
    K_DBL_CONTOUR_TOTAL_WIDTH,
    K_TOTAL_HEIGHT,
    Color::BLACK,
    {
        let _ = error_msg;
        draw_zero_length_capped_paths_dbl_contour(canvas, true);
        DrawResult::Ok
    }
);

// Port of: gm/path_stroke_with_zero_length.cpp#L177-L191 (chrome/m156), zero_length_paths_dbl_bw
crate::def_simple_gm_bg_can_fail!(
    zero_length_paths_dbl_bw,
    canvas,
    error_msg,
    K_DBL_CONTOUR_TOTAL_WIDTH,
    K_TOTAL_HEIGHT,
    Color::BLACK,
    {
        let _ = error_msg;
        draw_zero_length_capped_paths_dbl_contour(canvas, false);
        DrawResult::Ok
    }
);
