// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/ClipTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::too_many_lines
)]

use skia_rust_core::point::Point;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tile;
use skia_rust_gpu::graphite::sparse_strips::types::Line;

use crate::tools::sparse_strips::tile_test_cases;
use crate::{Reporter, def_test, errorf};

/// Liang-Barsky style as ground truth. If there exists a clipped segment of the line, returns the
/// the segment's endpoints (in tile space).
// Port of: tests/graphite/sparse_strips/ClipTest.cpp#L24-L70 (chrome/m156)
fn reference_clip(
    line: &Line,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> Option<[Point; 2]> {
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    let dx = line.p1.x - line.p0.x;
    let dy = line.p1.y - line.p0.y;

    let p = [-dx, dx, -dy, dy];
    let q = [
        line.p0.x - min_x,
        max_x - line.p0.x,
        line.p0.y - min_y,
        max_y - line.p0.y,
    ];

    for i in 0..4 {
        if p[i] == 0.0 {
            if q[i] < 0.0 {
                return None; // Parallel and outside
            }
        } else {
            let t = q[i] / p[i];
            if p[i] < 0.0 {
                if t > t1 {
                    return None;
                }
                if t > t0 {
                    t0 = t;
                }
            } else {
                if t < t0 {
                    return None;
                }
                if t < t1 {
                    t1 = t;
                }
            }
        }
    }

    if t0 > t1 {
        return None;
    }

    let mut p_entry = Point::new(line.p0.x + t0 * dx, line.p0.y + t0 * dy);
    let mut p_exit = Point::new(line.p0.x + t1 * dx, line.p0.y + t1 * dy);

    p_entry.x -= min_x;
    p_entry.y -= min_y;
    p_exit.x -= min_x;
    p_exit.y -= min_y;

    let width = max_x - min_x;
    let height = max_y - min_y;
    let clamp_pt = |pt: &mut Point| {
        pt.x = pt.x.clamp(0.0, width);
        pt.y = pt.y.clamp(0.0, height);
    };
    clamp_pt(&mut p_entry);
    clamp_pt(&mut p_exit);

    if p_exit.y < p_entry.y {
        return Some([p_exit, p_entry]);
    }
    Some([p_entry, p_exit])
}

/// `ClipTestRunner<kWidth, kHeight>::VerifyClip`.
// Port of: tests/graphite/sparse_strips/ClipTest.cpp#L80-L164 (chrome/m156)
fn verify_clip<const K_WIDTH: u16, const K_HEIGHT: u16>(
    reporter: &mut Reporter,
    line: &Line,
    tile: Tile,
    test_name: &str,
) {
    let dx = line.p1.x - line.p0.x;
    let dy = line.p1.y - line.p0.y;

    let is_vertical = dx.abs() <= f32::EPSILON;
    let is_horizontal = dy.abs() <= f32::EPSILON;

    let idx = if is_vertical { 0.0 } else { 1.0 / dx };
    let idy = if is_horizontal { 0.0 } else { 1.0 / dy };

    let tile_min_x = f32::from(tile.x) * f32::from(K_WIDTH);
    let tile_min_y = f32::from(tile.y) * f32::from(K_HEIGHT);
    let tile_max_x = tile_min_x + f32::from(K_WIDTH);
    let tile_max_y = tile_min_y + f32::from(K_HEIGHT);

    let tile_bounds = [
        Point::new(tile_min_x, tile_min_y),
        Point::new(tile_max_x, tile_max_y),
    ];
    let derivatives = [dx, dy, idx, idy];
    let mask = tile.intersection_mask();

    let canonical_x_dir = dx >= 0.0;
    let canonical_y_dir = dy >= 0.0;

    let (result_line, _enters_left, _exits_left) = Tile::clip_to_tile::<K_WIDTH, K_HEIGHT>(
        line,
        &tile_bounds,
        &derivatives,
        mask,
        canonical_x_dir,
        canonical_y_dir,
    );
    let result = [result_line.p0, result_line.p1];

    // Get the expected clip using Liang-Barsky
    let expected_clip_lb = reference_clip(line, tile_min_x, tile_min_y, tile_max_x, tile_max_y);

    // If a line lies entirely outside the tile but touches it at a single point (e.g., grazing
    // a corner), Liang-Barsky (correctly) reports only that single intersection. However,
    // ClipToTile safely handles these cases by clamping the coordinates to create a zero-width
    // degenerate line. We calculate a fallback expected value here to match and verify that
    // clamping behavior.
    let raw_p0 = Point::new(
        (line.p0.x - tile_min_x).clamp(0.0, f32::from(K_WIDTH)),
        (line.p0.y - tile_min_y).clamp(0.0, f32::from(K_HEIGHT)),
    );
    let raw_p1 = Point::new(
        (line.p1.x - tile_min_x).clamp(0.0, f32::from(K_WIDTH)),
        (line.p1.y - tile_min_y).clamp(0.0, f32::from(K_HEIGHT)),
    );
    let fallback_exp = if raw_p1.y < raw_p0.y {
        [raw_p1, raw_p0]
    } else {
        [raw_p0, raw_p1]
    };

    const K_TOL: f32 = 0.005;
    let match_pt = |a: &Point, b: &Point| (a.x - b.x).abs() < K_TOL && (a.y - b.y).abs() < K_TOL;

    // Check the result against both Liang-Barsky value (covers almost all the cases) and
    // the fallback value
    let mut matches = true;
    for i in 0..2 {
        let mut matches_lb = false;
        if let Some(expected_clip_lb) = &expected_clip_lb {
            matches_lb = match_pt(&result[i], &expected_clip_lb[i]);
        }

        let matches_fallback = match_pt(&result[i], &fallback_exp[i]);
        if !matches_lb && !matches_fallback {
            matches = false;
            break;
        }
    }

    if !matches {
        let exp = expected_clip_lb.unwrap_or(fallback_exp);
        errorf!(
            reporter,
            "[{}] Clip mismatch at Tile({}, {}).\nLine: ({:.2}, {:.2}) -> ({:.2}, {:.2})\nExp: \
             [({:.2}, {:.2}), ({:.2}, {:.2})]\nGot: [({:.2}, {:.2}), ({:.2}, {:.2})]\n",
            test_name,
            tile.x,
            tile.y,
            line.p0.x,
            line.p0.y,
            line.p1.x,
            line.p1.y,
            exp[0].x,
            exp[0].y,
            exp[1].x,
            exp[1].y,
            result[0].x,
            result[0].y,
            result[1].x,
            result[1].y
        );
    }
}

/// `ClipTestRunner<kWidth, kHeight>::RunClipTest`.
// Port of: tests/graphite/sparse_strips/ClipTest.cpp#L166-L181 (chrome/m156)
fn run_clip_test<const K_WIDTH: u16, const K_HEIGHT: u16>(
    reporter: &mut Reporter,
    lines: &[Line],
    expected_tiles: &[Tile],
    test_name: &str,
) {
    for tile in expected_tiles {
        let line_idx = tile.line_idx();
        if (line_idx as usize) < lines.len() {
            verify_clip::<K_WIDTH, K_HEIGHT>(reporter, &lines[line_idx as usize], *tile, test_name);
        } else {
            errorf!(
                reporter,
                "[{}] Test setup error: Tile references invalid line index {}",
                test_name,
                line_idx
            );
        }
    }
}

/// `ClipTestRunner<kWidth, kHeight>::RunAll`.
// Port of: tests/graphite/sparse_strips/ClipTest.cpp#L184-L189 (chrome/m156)
fn run_all<const K_WIDTH: u16, const K_HEIGHT: u16>(reporter: &mut Reporter) {
    let k_scale = f32::from(K_WIDTH) / 4.0;
    const K_VIEWPORT_DIMF: f32 = 100.0;
    let test_cases = tile_test_cases::get(k_scale, K_VIEWPORT_DIMF as u16);
    for test_case in &test_cases {
        run_clip_test::<K_WIDTH, K_HEIGHT>(
            reporter,
            &test_case.lines,
            &test_case.expected,
            test_case.name,
        );
    }
}

def_test!(SparseStrips_Clip_4x4, |reporter| {
    // Port of: tests/graphite/sparse_strips/ClipTest.cpp#L192-L192 (chrome/m156)
    run_all::<4, 4>(reporter);
});

def_test!(SparseStrips_Clip_8x8, |reporter| {
    // Port of: tests/graphite/sparse_strips/ClipTest.cpp#L194-L194 (chrome/m156)
    run_all::<8, 8>(reporter);
});
