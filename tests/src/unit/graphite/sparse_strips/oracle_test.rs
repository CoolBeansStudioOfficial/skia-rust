// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/OracleTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::similar_names,
    // `0.7071f` of the C++ is a literal, not `FRAC_1_SQRT_2`.
    clippy::approx_constant,
    clippy::too_many_lines
)]

use std::fmt::Write as _;

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::graphite::sparse_strips::types::K_MSAA_PATTERN_U8;
use skia_rust_simd::vx::{Byte8, Float8, Int8, all, ceil, if_then_else};

use crate::tools::sparse_strips::oracle::ScanlineOracle8x;
use crate::{Reporter, def_test, errorf, reporter_assert};

/// `TestCase`.
// Port of: tests/graphite/sparse_strips/OracleTest.cpp#L27-L32 (chrome/m156)
struct TestCase {
    name: &'static str,
    path: Path,
    min_row_y: i32,
    max_row_y: i32,
}

// Port of: tests/graphite/sparse_strips/OracleTest.cpp#L34-L135 (chrome/m156)
fn validate_oracle_path(reporter: &mut Reporter, tc: &TestCase) {
    let k_sub_x: Float8 = (Byte8::from_list(&K_MSAA_PATTERN_U8).cast::<f32>() + 0.5f32) / 8.0f32;

    let bounds = *tc.path.bounds();
    let min_x = bounds.left.floor() as i32;
    let max_x = bounds.right.ceil() as i32;
    let row_width = std::cmp::max(1, max_x - min_x + 1);

    let mut oracle = ScanlineOracle8x::with_path(&tc.path);
    for py in tc.min_row_y..=tc.max_row_y {
        oracle.build_row(py);
        let hits = oracle.hits().to_vec();

        let mut row_windings: Vec<Int8> = vec![Int8::splat(0); row_width as usize];
        let mut total_winding = Int8::splat(0);

        for hit in &hits {
            let px: Int8 = ceil(hit.x - k_sub_x).cast::<i32>();
            let idx: Int8 = px - min_x;

            let has_hit = hit.dir.ne_mask(0);
            total_winding += if_then_else(has_hit & idx.le_mask(0), hit.dir, Int8::splat(0));

            let in_row = has_hit & idx.gt_mask(0) & idx.lt_mask(row_width);
            for k in 0..8usize {
                if in_row[k] != 0 {
                    row_windings[idx[k] as usize][k] += hit.dir[k];
                }
            }
        }
        row_windings[0] += total_winding;

        // Inclusive prefix sum across the row array
        for i in 1..row_width as usize {
            let prev = row_windings[i - 1];
            row_windings[i] += prev;
        }

        // Validate against compressed oracle RowIntervals [x_k, x_{k+1})
        let intervals = oracle.row_intervals().to_vec();
        for k in 0..intervals.len().saturating_sub(1) {
            let x_start = intervals[k].x;
            let x_end = intervals[k + 1].x;
            let expected_winding = intervals[k].winding;

            for px in x_start..x_end {
                let idx = px - min_x;
                if idx >= 0 && idx < row_width {
                    let matches = all(row_windings[idx as usize].eq_mask(expected_winding));
                    if !matches {
                        let mut diag = String::new();
                        let _ = write!(
                            diag,
                            "\n=== MISMATCH DEBUG at Row {py}, Pixel {px} (minX={min_x}, \
                             idx={idx}) ===\n"
                        );
                        diag.push_str("Lanes [0..7]:\n");
                        for lane in 0..8usize {
                            let v_px = px as f32 + k_sub_x[lane];
                            let _ = writeln!(
                                diag,
                                "  Lane {}: subX={:.4}, vPx={:.4} | prefixSum={}, \
                                 oracleWinding={}",
                                lane,
                                k_sub_x[lane],
                                v_px,
                                row_windings[idx as usize][lane],
                                expected_winding[lane]
                            );
                        }
                        diag.push_str("Hits near this pixel:\n");
                        for (h, hit) in hits.iter().enumerate() {
                            for lane in 0..8usize {
                                if hit.dir[lane] != 0 && (hit.x[lane] - px as f32).abs() < 3.0 {
                                    let hit_x = hit.x[lane];
                                    let calc_px = (hit_x - k_sub_x[lane]).ceil() as i32;
                                    let v_px = px as f32 + k_sub_x[lane];
                                    let _ = writeln!(
                                        diag,
                                        "    Hit[{}] Lane {}: hitX={:.6}, dir={}, calcPx={}, \
                                         (vPx >= hitX)={}",
                                        h,
                                        lane,
                                        hit_x,
                                        hit.dir[lane],
                                        calc_px,
                                        i32::from(v_px >= hit_x)
                                    );
                                }
                            }
                        }
                        errorf!(reporter, "{}", diag);
                    }
                    reporter_assert!(
                        reporter,
                        matches,
                        "[{}] Row {} Pixel {}: prefix-sum winding != oracle winding",
                        tc.name,
                        py,
                        px
                    );
                }
            }
        }
    }
}

def_test!(SparseStrips_Oracle_Simple, |reporter| {
    // Port of: tests/graphite/sparse_strips/OracleTest.cpp#L139-L236 (chrome/m156)
    let k_test_cases = [
        TestCase {
            name: "Line_Down",
            path: PathBuilder::new()
                .move_to((1.0, -0.5))
                .line_to((5.0, 1.5))
                .line_to((1.0, 1.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Line_Up",
            path: PathBuilder::new()
                .move_to((5.0, 1.5))
                .line_to((1.0, -0.5))
                .line_to((5.0, -0.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Quad",
            path: PathBuilder::new()
                .move_to((1.0, -0.5))
                .quad_to((6.0, 0.5), (2.0, 1.5))
                .line_to((1.0, 1.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Conic",
            path: PathBuilder::new()
                .move_to((2.0, -0.4))
                .conic_to((5.0, 0.4), (1.0, 1.4), 0.7071)
                .line_to((2.0, 1.4))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Cubic_Cusp",
            path: PathBuilder::new()
                .move_to((1.0, -0.5))
                .cubic_to((5.0, 1.5), (5.0, -0.5), (1.0, 1.5))
                .line_to((1.0, -0.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Cubic_Swallowtail",
            path: PathBuilder::new()
                .move_to((1.0, -0.5))
                .cubic_to((7.0, 2.0), (1.0, 2.0), (7.0, -0.5))
                .line_to((1.0, -0.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Cubic_Serpentine",
            path: PathBuilder::new()
                .move_to((1.0, -0.5))
                .cubic_to((6.0, 0.2), (-1.0, 0.8), (5.0, 1.5))
                .line_to((1.0, 1.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Mixed_Closed_Shape",
            path: PathBuilder::new()
                .move_to((2.0, 0.0))
                .line_to((8.0, 1.0))
                .quad_to((12.0, 4.0), (8.0, 7.0))
                .conic_to((4.0, 9.0), (1.0, 6.0), 0.7071)
                .cubic_to((0.0, 4.0), (0.0, 2.0), (2.0, 0.0))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 8,
        },
        TestCase {
            name: "Negative_Coordinates",
            path: PathBuilder::new()
                .move_to((-5.0, -0.5))
                .line_to((-1.0, 1.5))
                .line_to((-5.0, 1.5))
                .close()
                .detach(),
            min_row_y: 0,
            max_row_y: 1,
        },
        TestCase {
            name: "Donut_Hole",
            path: PathBuilder::new()
                .add_rect(Rect::from_xywh(1.0, 0.0, 6.0, 6.0), PathDirection::CW, None)
                .add_rect(
                    Rect::from_xywh(3.0, 2.0, 2.0, 2.0),
                    PathDirection::CCW,
                    None,
                )
                .detach(),
            min_row_y: 0,
            max_row_y: 6,
        },
    ];

    for tc in &k_test_cases {
        validate_oracle_path(reporter, tc);
    }
});

def_test!(SparseStrips_Oracle_ManyPathsRow, |reporter| {
    // Port of: tests/graphite/sparse_strips/OracleTest.cpp#L238-L313 (chrome/m156)
    let mut rand = Random::new(12345);
    let mut builder = PathBuilder::new();
    const K_NUM_PATHS: i32 = 600;

    let mut current_x = -50.0f32;
    for _i in 0..K_NUM_PATHS {
        let x_offset = current_x;
        let width = rand.next_range_f(2.0, 6.0);
        let y0 = rand.next_range_f(-0.8, -0.2);
        let y1 = rand.next_range_f(1.2, 1.8);

        let shape_type = rand.next_u_less_than(6);
        match shape_type {
            0 => {
                // Line triangle
                builder
                    .move_to((x_offset, y0))
                    .line_to((x_offset + width, y1))
                    .line_to((x_offset, y1))
                    .close();
            }
            1 => {
                // Quadratic
                let ctrl_x = x_offset + rand.next_range_f(0.0, width * 1.5);
                let ctrl_y = rand.next_range_f(-0.5, 1.5);
                builder
                    .move_to((x_offset, y0))
                    .quad_to((ctrl_x, ctrl_y), (x_offset + width, y1))
                    .line_to((x_offset, y1))
                    .close();
            }
            2 => {
                // Conic
                let ctrl_x = x_offset + rand.next_range_f(0.0, width * 1.5);
                let ctrl_y = rand.next_range_f(-0.5, 1.5);
                let weight = rand.next_range_f(0.3, 2.0);
                builder
                    .move_to((x_offset, y0))
                    .conic_to((ctrl_x, ctrl_y), (x_offset + width, y1), weight)
                    .line_to((x_offset, y1))
                    .close();
            }
            3 => {
                // Cubic
                let ctrl_x1 = x_offset + rand.next_range_f(-1.0, width);
                let ctrl_y1 = rand.next_range_f(-0.5, 1.5);
                let ctrl_x2 = x_offset + rand.next_range_f(0.0, width + 1.0);
                let ctrl_y2 = rand.next_range_f(-0.5, 1.5);
                builder
                    .move_to((x_offset, y0))
                    .cubic_to(
                        (ctrl_x1, ctrl_y1),
                        (ctrl_x2, ctrl_y2),
                        (x_offset + width, y1),
                    )
                    .line_to((x_offset, y1))
                    .close();
            }
            4 => {
                // Rectangle / Polygon
                builder
                    .move_to((x_offset, y0))
                    .line_to((x_offset + width, y0))
                    .line_to((x_offset + width, y1))
                    .line_to((x_offset, y1))
                    .close();
            }
            5 => {
                // Donut / nested contour
                let margin = width * 0.25;
                builder.add_rect(
                    Rect::from_ltrb(x_offset, y0, x_offset + width, y1),
                    PathDirection::CW,
                    None,
                );
                builder.add_rect(
                    Rect::from_ltrb(
                        x_offset + margin,
                        y0 + 0.3,
                        x_offset + width - margin,
                        y1 - 0.3,
                    ),
                    PathDirection::CCW,
                    None,
                );
            }
            _ => {}
        }

        // Advance X with varying spacing
        current_x += width + rand.next_range_f(0.5, 3.0);
    }

    let tc = TestCase {
        name: "Many_Paths_Row_600",
        path: builder.detach(),
        min_row_y: 0,
        max_row_y: 0,
    };

    validate_oracle_path(reporter, &tc);
});
