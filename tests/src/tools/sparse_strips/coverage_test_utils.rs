// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h (chrome/m156)

//! `CoverageTestUtils`: point in polygon verification and the diagnostics the coverage tests print
//! when they fail.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments,
    clippy::similar_names,
    // `std::max(0.0f, std::min(1.0f, t))` of the C++ is not `clamp` for a NaN `t`.
    clippy::manual_clamp
)]

use std::fmt::Write as _;

use skia_rust_core::point::Point;
use skia_rust_gpu::graphite::sparse_strips::msaa_lut::MsaaLut;
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tiles;
use skia_rust_gpu::graphite::sparse_strips::types::IntersectionBits;
use skia_rust_simd::vx::Int8;

use super::oracle::{RowWindingInterval, ScanlineOracle8x};
use crate::{Reporter, infof};

/// Simplified point-in-polygon verification using Even-Odd rule.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L33-L46 (chrome/m156)
#[doc(alias = "PointInPolygon")]
#[must_use]
pub fn point_in_polygon(pt: Point, polyline: &Polyline) -> bool {
    let mut inside = false;
    for (line, _idx) in polyline {
        if (line.p0.y > pt.y) != (line.p1.y > pt.y) {
            let t = (pt.y - line.p0.y) / (line.p1.y - line.p0.y);
            let x_int = line.p0.x + t * (line.p1.x - line.p0.x);
            if pt.x < x_int {
                inside = !inside;
            }
        }
    }
    inside
}

/// Prints ASCII map diagnostic comparing expected polygon coverage against actual rasterized
/// mask.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L50-L73 (chrome/m156)
#[doc(alias = "PrintCoverageDiagnostics")]
pub fn print_coverage_diagnostics<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
    reporter: &mut Reporter,
    polyline: &Polyline,
    tiler: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
    fail_x: i32,
    fail_y: i32,
    actual_masks: &[u8],
    tile_start_idx: usize,
) {
    let mut out = String::from("\n--- FAILURE DIAGNOSTICS ---\n");
    append_geometry_lines(&mut out, Some(polyline), -1, -1);
    append_tile_intersections(&mut out, Some(tiler), -1, -1);
    append_tile_ascii_map::<K_TILE_WIDTH, K_TILE_HEIGHT>(
        &mut out,
        fail_x as u16,
        fail_y as u16,
        Some(polyline),
        &mut |_px, _py, _sy, cp| point_in_polygon(cp, polyline),
        &mut |_px, _py, sy, local_x, local_y| {
            let buf_idx = tile_start_idx
                + (usize::from(local_y) * usize::from(K_TILE_WIDTH) + usize::from(local_x));
            buf_idx < actual_masks.len() && (actual_masks[buf_idx] & (1 << sy)) != 0
        },
        "Expected",
        "Actual",
    );
    infof!(reporter, "{}", out);
}

/// Prints ASCII map diagnostic comparing Oracle expected coverage against actual winding.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L76-L137 (chrome/m156)
#[doc(alias = "PrintWindingDiagnostics")]
pub fn print_winding_diagnostics<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
    reporter: &mut Reporter,
    test_name: &str,
    tile_start_x: u16,
    tile_start_y: u16,
    px: u16,
    py: u16,
    actual_winding: Int8,
    oracle_winding: Int8,
    polyline: Option<&Polyline>,
    tiler: Option<&Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>>,
    exact_windings: &[Int8],
    tile_start_alpha_idx: u32,
    oracle: Option<&mut ScanlineOracle8x>,
) {
    let mut out = String::new();
    let _ = writeln!(out, "\n=== MISMATCH DIAGNOSTIC [{test_name}] ===");
    let _ = writeln!(
        out,
        "Location: Tile({},{}), Pixel({},{}) [Global px={}, py={}]",
        tile_start_x / K_TILE_WIDTH,
        tile_start_y / K_TILE_HEIGHT,
        i32::from(px) - i32::from(tile_start_x),
        i32::from(py) - i32::from(tile_start_y),
        px,
        py
    );
    let _ = writeln!(
        out,
        "Actual Winding: [{}, {}, {}, {}, {}, {}, {}, {}]",
        actual_winding[0],
        actual_winding[1],
        actual_winding[2],
        actual_winding[3],
        actual_winding[4],
        actual_winding[5],
        actual_winding[6],
        actual_winding[7]
    );
    let _ = writeln!(
        out,
        "Oracle Winding: [{}, {}, {}, {}, {}, {}, {}, {}]",
        oracle_winding[0],
        oracle_winding[1],
        oracle_winding[2],
        oracle_winding[3],
        oracle_winding[4],
        oracle_winding[5],
        oracle_winding[6],
        oracle_winding[7]
    );

    append_geometry_lines(&mut out, polyline, i32::from(px), i32::from(py));
    append_tile_intersections(
        &mut out,
        tiler,
        i32::from(tile_start_x / K_TILE_WIDTH),
        i32::from(tile_start_y / K_TILE_HEIGHT),
    );

    let mut oracle_intervals: Vec<RowWindingInterval> = Vec::new();
    let mut cached_row = -1;
    let mut oracle = oracle;

    append_tile_ascii_map::<K_TILE_WIDTH, K_TILE_HEIGHT>(
        &mut out,
        tile_start_x,
        tile_start_y,
        polyline,
        &mut |curr_x, curr_y, sy, _cp| {
            if let Some(oracle) = oracle.as_deref_mut() {
                if cached_row != curr_y {
                    oracle_intervals = oracle.build_row_intervals(curr_y);
                    cached_row = curr_y;
                }
                let exp_w = ScanlineOracle8x::get_oracle_winding(curr_x, &oracle_intervals);
                return exp_w[sy as usize] != 0;
            }
            false
        },
        &mut |_curr_x, _curr_y, sy, local_x, local_y| {
            let buf_idx = tile_start_alpha_idx
                + (u32::from(local_y) * u32::from(K_TILE_WIDTH) + u32::from(local_x));
            (buf_idx as usize) < exact_windings.len()
                && exact_windings[buf_idx as usize][sy as usize] != 0
        },
        "Oracle Expected",
        "Actual",
    );
    infof!(reporter, "{}", out);
}

// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L140-L169 (chrome/m156)
fn append_geometry_lines(
    out: &mut String,
    polyline: Option<&Polyline>,
    near_px: i32,
    near_py: i32,
) {
    let Some(polyline) = polyline else {
        return;
    };
    if near_px < 0 {
        out.push_str("Geometry Lines: {\n");
        for (l, _idx) in polyline {
            let _ = writeln!(
                out,
                "  {{{{{:.6}, {:.6}}}, {{{:.6}, {:.6}}}}},",
                l.p0.x, l.p0.y, l.p1.x, l.p1.y
            );
        }
        out.push_str("}\n\n");
    } else {
        out.push_str("Polyline Lines near pixel:\n");
        for (l, idx) in polyline {
            let min_x = l.p0.x.min(l.p1.x);
            let max_x = l.p0.x.max(l.p1.x);
            let min_y = l.p0.y.min(l.p1.y);
            let max_y = l.p0.y.max(l.p1.y);
            if max_x >= (near_px - 1) as f32
                && min_x <= (near_px + 2) as f32
                && max_y >= (near_py - 1) as f32
                && min_y <= (near_py + 2) as f32
            {
                let _ = writeln!(
                    out,
                    "  Line {}: ({:.2}, {:.2}) -> ({:.2}, {:.2})",
                    idx, l.p0.x, l.p0.y, l.p1.x, l.p1.y
                );
            }
        }
    }
}

// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L171-L191 (chrome/m156)
fn append_tile_intersections<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
    out: &mut String,
    tiler: Option<&Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>>,
    tile_x: i32,
    tile_y: i32,
) {
    let Some(tiler) = tiler else {
        return;
    };
    out.push_str("Tile Intersections:\n");
    for tile in tiler.get_tiles() {
        if tile_x < 0 || (i32::from(tile.x) == tile_x && i32::from(tile.y) == tile_y) {
            let mask = tile.intersection_mask();
            let line_idx = tile.line_idx();
            let _ = writeln!(
                out,
                "  Tile({},{}) Line {} Mask: {}",
                tile.x,
                tile.y,
                line_idx,
                IntersectionBits::mask_to_string(mask)
            );
        }
    }
}

/// Checks if a point is within threshold distance to any line in polyline.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L194-L220 (chrome/m156)
fn point_near_line(pt: Point, polyline: &Polyline, threshold: f32) -> bool {
    for (line, _idx) in polyline {
        let l2 = (line.p0.x - line.p1.x) * (line.p0.x - line.p1.x)
            + (line.p0.y - line.p1.y) * (line.p0.y - line.p1.y);
        let dist = if l2 == 0.0 {
            let dx = pt.x - line.p0.x;
            let dy = pt.y - line.p0.y;
            (dx * dx + dy * dy).sqrt()
        } else {
            let t = f32::max(
                0.0,
                f32::min(
                    1.0,
                    ((pt.x - line.p0.x) * (line.p1.x - line.p0.x)
                        + (pt.y - line.p0.y) * (line.p1.y - line.p0.y))
                        / l2,
                ),
            );
            let proj_x = line.p0.x + t * (line.p1.x - line.p0.x);
            let proj_y = line.p0.y + t * (line.p1.y - line.p0.y);
            ((pt.x - proj_x) * (pt.x - proj_x) + (pt.y - proj_y) * (pt.y - proj_y)).sqrt()
        };
        if dist <= threshold {
            return true;
        }
    }
    false
}

/// Formats a side-by-side ASCII map comparing expected and actual tile coverage.
// Port of: tests/graphite/sparse_strips/CoverageTestUtils.h#L223-L277 (chrome/m156)
fn append_tile_ascii_map<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
    out: &mut String,
    tile_start_x: u16,
    tile_start_y: u16,
    polyline: Option<&Polyline>,
    is_expected_active: &mut dyn FnMut(i32, i32, i32, Point) -> bool,
    is_actual_active: &mut dyn FnMut(i32, i32, i32, u16, u16) -> bool,
    left_label: &str,
    right_label: &str,
) {
    let _ = write!(
        out,
        "\nASCII Map Tile({},{}) [Left: {} | Right: {}]\n",
        tile_start_x / K_TILE_WIDTH,
        tile_start_y / K_TILE_HEIGHT,
        left_label,
        right_label
    );

    let mut border = String::from("+");
    for _x in 0..K_TILE_WIDTH {
        border.push_str("----------------+");
    }

    for local_y in 0..K_TILE_HEIGHT {
        let curr_y = tile_start_y + local_y;
        let _ = writeln!(out, "{border}   {border}");

        for sy in 0..8 {
            for side in 0..2 {
                // 0: Left/Expected, 1: Right/Actual
                out.push('|');
                for local_x in 0..K_TILE_WIDTH {
                    let curr_x = tile_start_x + local_x;
                    for sx in 0..8 {
                        let cp = Point::new(
                            f32::from(curr_x) + (sx as f32 + 0.5) / 8.0,
                            f32::from(curr_y) + (sy as f32 + 0.5) / 8.0,
                        );
                        let on_line = polyline
                            .is_some_and(|polyline| point_near_line(cp, polyline, 0.6 / 8.0));
                        let active = if side == 0 {
                            is_expected_active(i32::from(curr_x), i32::from(curr_y), sy, cp)
                        } else {
                            is_actual_active(
                                i32::from(curr_x),
                                i32::from(curr_y),
                                sy,
                                local_x,
                                local_y,
                            )
                        };

                        if i32::from(MsaaLut::<u8>::K_PATTERN[sy as usize]) == sx {
                            out.push_str(match (active, on_line) {
                                (true, true) => "*#",
                                (true, false) => " #",
                                (false, true) => "*o",
                                (false, false) => " o",
                            });
                        } else {
                            out.push_str(if on_line { "**" } else { "  " });
                        }
                    }
                    out.push('|');
                }
                if side == 0 {
                    out.push_str("   ");
                }
            }
            out.push('\n');
        }
    }
    let _ = writeln!(out, "{border}   {border}");
}
