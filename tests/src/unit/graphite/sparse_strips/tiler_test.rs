// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/TilerTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::too_many_lines
)]

use std::fmt::Write as _;

use skia_rust_core::point::Point;
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::tiler::{Tile, Tiles};
use skia_rust_gpu::graphite::sparse_strips::types::{IntersectionBits, Line};

use crate::tools::sparse_strips::tile_test_cases;
use crate::{Reporter, def_test, errorf, infof, reporter_assert};

// Port of: tests/graphite/sparse_strips/TilerTest.cpp#L19-L27 (chrome/m156)
fn make_polyline(lines: &[Line]) -> Polyline {
    let mut p = Polyline::new();
    for line in lines {
        p.append_point(Point::new(line.p0.x, line.p0.y));
        p.append_point(Point::new(line.p1.x, line.p1.y));
        p.append_sentinel();
    }
    p
}

// Port of: tests/graphite/sparse_strips/TilerTest.cpp#L29-L42 (chrome/m156)
fn build_polyline_and_mapping(
    lines: &[Line],
    out_polyline: &mut Polyline,
    out_mapping: &mut Vec<u32>,
) {
    out_mapping.clear();
    out_polyline.reset();
    out_mapping.reserve(lines.len());
    out_polyline.reserve(lines.len() * 3);
    for line in lines {
        out_mapping.push(out_polyline.count() as u32);
        out_polyline.append_point(Point::new(line.p0.x, line.p0.y));
        out_polyline.append_point(Point::new(line.p1.x, line.p1.y));
        out_polyline.append_sentinel();
    }
}

// Port of: tests/graphite/sparse_strips/TilerTest.cpp#L44-L168 (chrome/m156)
fn check_tiles_match<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
    reporter: &mut Reporter,
    lines: &[Line],
    expected: &[Tile],
    test_name: &str,
    scaled_dim: u16,
) {
    let mut polyline = Polyline::new();
    let mut expected_to_actual_idx: Vec<u32> = Vec::new();

    build_polyline_and_mapping(lines, &mut polyline, &mut expected_to_actual_idx);

    let mut tiles = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();
    tiles.make_tiles_msaa(&polyline, scaled_dim, scaled_dim);
    let actual = tiles.get_tiles();

    let mut has_failure = actual.len() != expected.len();
    let limit = actual.len().min(expected.len());

    let mut i = 0;
    while i < limit && !has_failure {
        let got = &actual[i];
        let want = &expected[i];
        let want_logical_line = want.line_idx();
        let want_line = if (want_logical_line as usize) < expected_to_actual_idx.len() {
            expected_to_actual_idx[want_logical_line as usize]
        } else {
            want_logical_line
        };

        if got.x != want.x
            || got.y != want.y
            || got.line_idx() != want_line
            || got.intersection_mask() != want.intersection_mask()
        {
            has_failure = true;
        }
        i += 1;
    }

    if has_failure {
        let mut dump = String::new();
        let _ = write!(dump, "\n--- [{test_name}] ---\n");

        dump.push_str("Lines:\n{\n");
        for (i, line) in lines.iter().enumerate() {
            let _ = writeln!(
                dump,
                "    {{{{{}f, {}f}}, {{{}f, {}f}}}}{}",
                line.p0.x,
                line.p0.y,
                line.p1.x,
                line.p1.y,
                if i + 1 < lines.len() { "," } else { "" }
            );
        }
        dump.push_str("}\n\n");

        let mut dump_tiles = |tile_array: &[Tile], label: &str, is_actual: bool| {
            let _ = writeln!(dump, "{label}:");
            for tile in tile_array {
                let raw_line = tile.line_idx();
                let mask = tile.intersection_mask();
                let mut logical_line = raw_line;
                if is_actual {
                    for (j, mapped) in expected_to_actual_idx.iter().enumerate() {
                        if *mapped == raw_line {
                            logical_line = j as u32;
                            break;
                        }
                    }
                }

                let mut mask_str = IntersectionBits::mask_to_string(mask);
                if mask_str.is_empty() {
                    mask_str = "0".to_string();
                }

                let _ = writeln!(
                    dump,
                    "    Tile ({},{}) LineId {} IntersectionMask : {}",
                    tile.x, tile.y, logical_line, mask_str
                );
            }
            dump.push('\n');
        };

        dump_tiles(expected, "Expected", false);
        dump_tiles(actual, "Actual", true);

        dump.push_str("--- Mismatches ---\n");
        if actual.len() != expected.len() {
            let _ = writeln!(
                dump,
                "    Tile count mismatch. Expected {}, got {}",
                expected.len(),
                actual.len()
            );
        }

        for i in 0..limit {
            let got = &actual[i];
            let want = &expected[i];
            let want_logical_line = want.line_idx();
            let want_line = if (want_logical_line as usize) < expected_to_actual_idx.len() {
                expected_to_actual_idx[want_logical_line as usize]
            } else {
                want_logical_line
            };

            if got.x != want.x {
                let _ = writeln!(
                    dump,
                    "    Tile[{}] X mismatch. Want {}, Got {}",
                    i, want.x, got.x
                );
            }
            if got.y != want.y {
                let _ = writeln!(
                    dump,
                    "    Tile[{}] Y mismatch. Want {}, Got {}",
                    i, want.y, got.y
                );
            }
            if got.line_idx() != want_line {
                let _ = writeln!(
                    dump,
                    "    Tile[{}] Line Index mismatch. Want {}, Got {}",
                    i,
                    want_line,
                    got.line_idx()
                );
            }
            let got_mask = got.intersection_mask();
            let want_mask = want.intersection_mask();
            if got_mask != want_mask {
                let _ = writeln!(
                    dump,
                    "    Tile[{}] Mask mismatch. Want [{}], Got [{}]",
                    i,
                    IntersectionBits::mask_to_string(want_mask),
                    IntersectionBits::mask_to_string(got_mask)
                );
            }
        }

        dump.push_str("---------------------------\n");

        infof!(reporter, "{}", dump);
    }

    reporter_assert!(reporter, !has_failure, "{}", test_name);
}

// Port of: tests/graphite/sparse_strips/TilerTest.cpp#L170-L190 (chrome/m156)
fn check_sorted(reporter: &mut Reporter, buf: &[Tile]) {
    if buf.is_empty() {
        return;
    }

    for i in 0..buf.len() - 1 {
        let current = &buf[i];
        let next = &buf[i + 1];

        if current.y > next.y {
            errorf!(reporter, "Sort Failure [Y]: Tile[{}] > Tile[{}]", i, i + 1);
        }
        if current.y == next.y {
            if current.x > next.x {
                errorf!(reporter, "Sort Failure [X]: Tile[{}] > Tile[{}]", i, i + 1);
            }
            if current.x == next.x
                && current.packed_line_idx_intersection_mask
                    > next.packed_line_idx_intersection_mask
            {
                errorf!(
                    reporter,
                    "Sort Failure [Payload]: Tile[{}] > Tile[{}]",
                    i,
                    i + 1
                );
            }
        }
    }
}

/// `TileTestRunner<kTileWidth, kTileHeight>::RunAll(reporter)`.
// Port of: tests/graphite/sparse_strips/TilerTest.cpp#L202-L253 (chrome/m156)
fn run_all<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(reporter: &mut Reporter) {
    // only support square tiles for now
    const { assert!(K_TILE_WIDTH == K_TILE_HEIGHT) };
    let k_scale = f32::from(K_TILE_WIDTH) / 4.0;
    const K_VIEWPORT_DIM: u16 = 100;
    let k_scaled_dim = (f32::from(K_VIEWPORT_DIM) * k_scale) as u16;
    let k_scaled_dim_f = f32::from(k_scaled_dim);

    // General test cases
    for test_case in tile_test_cases::get(k_scale, K_VIEWPORT_DIM) {
        check_tiles_match::<K_TILE_WIDTH, K_TILE_HEIGHT>(
            reporter,
            &test_case.lines,
            &test_case.expected,
            test_case.name,
            k_scaled_dim,
        );
    }

    // Sort test
    {
        let mut polyline = Polyline::new();
        let mut tiles = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();

        let step = 4.0 * k_scale;
        let mut y = k_scaled_dim_f - (10.0 * k_scale);
        let limit = 10.0 * k_scale;

        while y > limit {
            polyline.append_point(Point::new(k_scaled_dim_f - (10.0 * k_scale), y));
            polyline.append_point(Point::new(10.0 * k_scale, y));
            polyline.append_sentinel();

            polyline.append_point(Point::new(k_scaled_dim_f - (12.0 * k_scale), y));
            polyline.append_point(Point::new(12.0 * k_scale, y));
            polyline.append_sentinel();
            y -= step;
        }

        tiles.make_tiles_msaa(&polyline, k_scaled_dim, k_scaled_dim);
        reporter_assert!(
            reporter,
            !tiles.get_tiles().is_empty(),
            "SortTest produced no tiles"
        );
        tiles.sort_tiles();
        check_sorted(reporter, tiles.get_tiles());
    }

    // Crash tests, pass if nothing crashes.
    {
        let mut tiles = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();
        let lines = [Line {
            p0: Point::new(22.0 * k_scale, 552.0 * k_scale),
            p1: Point::new(224.0 * k_scale, 388.0 * k_scale),
        }];
        tiles.make_tiles_msaa(
            &make_polyline(&lines),
            (600.0 * k_scale) as u16,
            (600.0 * k_scale) as u16,
        );
    }

    {
        let mut tiles = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();
        let lines = [Line {
            p0: Point::new(59.60001 * k_scale, 40.78 * k_scale),
            p1: Point::new(520_599.6 * k_scale, 100.18 * k_scale),
        }];
        tiles.make_tiles_msaa(
            &make_polyline(&lines),
            (200.0 * k_scale) as u16,
            (100.0 * k_scale) as u16,
        );
    }
}

def_test!(SparseStrips_Tiler_4x4, |reporter| {
    // Port of: tests/graphite/sparse_strips/TilerTest.cpp#L256-L258 (chrome/m156)
    run_all::<4, 4>(reporter);
});

def_test!(SparseStrips_Tiler_8x8, |reporter| {
    // Port of: tests/graphite/sparse_strips/TilerTest.cpp#L260-L262 (chrome/m156)
    run_all::<8, 8>(reporter);
});
