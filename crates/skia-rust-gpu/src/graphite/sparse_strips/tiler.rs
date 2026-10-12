// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/Tiler.h

//! [`Tile`] and [`Tiles`]: the tiling stage of sparse strips, which finds the tiles that each line
//! of a [`Polyline`] intersects.
//!
//! The casts between the narrow unsigned types and `i32`/`f32` below mirror the C++ integer
//! promotions and `static_cast`s of the ported arithmetic.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use skia_rust_core::point::Point;

use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::types::{IntersectionBits, Line};

const T: u32 = IntersectionBits::T;
const B: u32 = IntersectionBits::B;
const L: u32 = IntersectionBits::L;
const R: u32 = IntersectionBits::R;
const W: u32 = IntersectionBits::W;
const BOT_SHIFT: u32 = IntersectionBits::BOT_SHIFT;
const LEFT_SHIFT: u32 = IntersectionBits::LEFT_SHIFT;
const RIGHT_SHIFT: u32 = IntersectionBits::RIGHT_SHIFT;

/// A sparse strips tile, contains:
///  1) The top left corner of the tile in device space, in tile sized units.
///  2) The coarse winding (W) of the tile
///  3) The intersection mask (R | L | T | B) of the tile
///  4) The index of their parent line in the array backing polyline
// Port of: src/gpu/graphite/sparse_strips/Tiler.h#L33-L215 (chrome/m156)
#[doc(alias = "skgpu::graphite::Tile")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tile {
    /// `x`.
    pub x: u16,
    /// `y`.
    pub y: u16,
    /// Contains the parent line's index and the intersection/winding mask.
    ///
    /// ```text
    /// MSB                                                            LSB
    /// 31------------------------------------------------------5|4|3|2|1|0|
    /// |            Parent Line Index (27 bits)                 |W|R|L|B|T|
    /// ```
    ///
    /// `fPackedLineIdxIntersectionMask`.
    pub packed_line_idx_intersection_mask: u32,
}

// Make sure the struct is a size convenient to move around, as tiles will be sorted.
// Port of: src/gpu/graphite/sparse_strips/Tiler.h#L218 (chrome/m156)
const _: () = assert!(size_of::<Tile>() == 8);

impl Tile {
    /// `Tile(x, y, lineIdx, intersectionMask)`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L44-L52 (chrome/m156)
    #[must_use]
    pub fn new(x: u16, y: u16, line_idx: u32, intersection_mask: u32) -> Self {
        debug_assert!(intersection_mask < (1 << IntersectionBits::INT_MASK_SHIFT));
        Self {
            x,
            y,
            packed_line_idx_intersection_mask: (line_idx << IntersectionBits::INT_MASK_SHIFT)
                | intersection_mask,
        }
    }

    /// When sorting we place y in the highest bits followed by x, so that tiles are grouped by
    /// row, left to right. Note, lineIdx is placed in the lower bits of the sorting key so that
    /// tiles from the same parent line at the same geometric location are adjacent and have
    /// better cache locality when processed in makeStrips.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L59-L62 (chrome/m156)
    #[doc(alias = "toBits")]
    #[must_use]
    pub fn to_bits(&self) -> u64 {
        (u64::from(self.y) << 48)
            | (u64::from(self.x) << 32)
            | u64::from(self.packed_line_idx_intersection_mask)
    }

    /// `lineIdx()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L68-L70 (chrome/m156)
    #[doc(alias = "lineIdx")]
    #[must_use]
    pub fn line_idx(&self) -> u32 {
        self.packed_line_idx_intersection_mask >> IntersectionBits::INT_MASK_SHIFT
    }

    /// `intersectionMask()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L71-L73 (chrome/m156)
    #[doc(alias = "intersectionMask")]
    #[must_use]
    pub fn intersection_mask(&self) -> u32 {
        self.packed_line_idx_intersection_mask & IntersectionBits::INTERSECTION_MASK
    }

    /// `coarseWinding()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L74-L76 (chrome/m156)
    #[doc(alias = "coarseWinding")]
    #[must_use]
    pub fn coarse_winding(&self) -> bool {
        (self.packed_line_idx_intersection_mask & IntersectionBits::W) != 0
    }

    /// `hasLeftIntersection()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L77-L79 (chrome/m156)
    #[doc(alias = "hasLeftIntersection")]
    #[must_use]
    pub fn has_left_intersection(&self) -> bool {
        (self.packed_line_idx_intersection_mask & IntersectionBits::L) != 0
    }

    /// Responsible for consuming the tile coordinates and intersection mask produced by the
    /// [`Tiles`] and producing the exact points at which the parent line intersects the edges of
    /// the tile. It also safely handles when there is only one edge intersection, either from the
    /// line ending inside the tile body, or a horizontal edge touch.
    ///
    /// This function provides the following guarantees:
    ///  1) Y-Sorting: The resulting points are always sorted top-to-bottom.
    ///  2) Interior Endpoints: If a line ends inside the tile, its exact endpoint is preserved and
    ///     returned.
    ///  3) Zero-Width Degenerates: A single-point intersection (e.g., a horizontal graze or corner
    ///     touch) produces a second point along that exact same edge, yielding safe, zero-width
    ///     geometry.
    ///  4) Boundary Snapping: Edge intersections are explicitly snapped to the exact bounding
    ///     coordinate of the tile.
    ///
    /// (Note: Guarantees 2 and 4 ensure downstream logic can safely rely on floating-point
    /// equality checks).
    ///
    /// Note, this function is tightly coupled to `Tiles` and `MakeStrips`. If the incoming tile
    /// coordinates are incorrect, the results from this function are meaningless. It assumes
    /// that:
    ///  1) Derivatives resulting from division by zero (e.g. perfectly vertical or horizontal
    ///     lines) will be set to zero.
    ///  2) Only two bits are ever set in the mask. (Expects that perfect corner touches are tie
    ///     broken by the Tiler.)
    ///
    /// Returns the clipped line and whether its top and its bottom point are on the left edge.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L105-L214 (chrome/m156)
    #[doc(alias = "ClipToTile")]
    #[must_use]
    pub fn clip_to_tile<const K_WIDTH: u16, const K_HEIGHT: u16>(
        line: &Line,
        tile_bounds: &[Point; 2],
        derivatives: &[f32; 4],
        intersection_mask: u32,
        canonical_x_dir: bool,
        canonical_y_dir: bool,
    ) -> (Line, bool, bool) {
        let width = f32::from(K_WIDTH);
        let height = f32::from(K_HEIGHT);

        let top_left = tile_bounds[0];
        let tile_min_x = tile_bounds[0].x;
        let tile_min_y = tile_bounds[0].y;
        let tile_max_x = tile_bounds[1].x;
        let tile_max_y = tile_bounds[1].y;

        let dx = derivatives[0];
        let dy = derivatives[1];

        // A line's direction dictates which edges it can cross from the outside-in (entry) versus
        // inside-out (exit). For example, if dx > 0 (canonicalXDir = true), the vector is
        // monotonic in the positive X direction. It is impossible to intersect the right edge as
        // an entry point, or the left edge as an exit point. This reduces the number of edges
        // which must be checked from four to two candidate entry and two candidate exit edges.
        let (mask_v_in, bound_v_in, mask_v_out, bound_v_out) = if canonical_x_dir {
            (L, tile_min_x, R, tile_max_x)
        } else {
            (R, tile_max_x, L, tile_min_x)
        };

        let (mask_h_in, bound_h_in, mask_h_out, bound_h_out) = if canonical_y_dir {
            (T, tile_min_y, B, tile_max_y)
        } else {
            (B, tile_max_y, T, tile_min_y)
        };

        let inv_dx = derivatives[2];
        let inv_dy = derivatives[3];

        // Check the candidate edges against the intersection mask
        let entry_hits = intersection_mask & (mask_v_in | mask_h_in);
        let exit_hits = intersection_mask & (mask_v_out | mask_h_out);

        let clip_pt =
            |mut p: Point, hits: u32, mask_h: u32, bound_h: f32, bound_v: f32| -> (Point, bool) {
                let mut is_left = false;
                if hits != 0 {
                    let use_h = (intersection_mask & mask_h) != 0;
                    let bound = if use_h { bound_h } else { bound_v };
                    let start = if use_h { line.p0.y } else { line.p0.x };
                    let inv_d = if use_h { inv_dy } else { inv_dx };

                    let t = (bound - start) * inv_d;

                    p.x = line.p0.x + t * dx;
                    p.y = line.p0.y + t * dy;

                    if use_h {
                        p.y = bound;
                        p.x -= top_left.x;
                        p.y -= top_left.y;
                        p.x = p.x.clamp(0.0, width);
                    } else {
                        p.x = bound;
                        p.x -= top_left.x;
                        p.y -= top_left.y;
                        p.y = p.y.clamp(0.0, height);
                        is_left = bound == tile_min_x;
                    }
                } else {
                    p.x -= top_left.x;
                    p.y -= top_left.y;
                    p.x = p.x.clamp(0.0, width);
                    p.y = p.y.clamp(0.0, height);
                }
                (p, is_left)
            };

        let (p_entry, entry_left) = clip_pt(line.p0, entry_hits, mask_h_in, bound_h_in, bound_v_in);
        let (p_exit, exit_left) = clip_pt(line.p1, exit_hits, mask_h_out, bound_h_out, bound_v_out);

        let top_is_on_left_edge = if canonical_y_dir {
            entry_left
        } else {
            exit_left
        };
        let bot_is_on_left_edge = if canonical_y_dir {
            exit_left
        } else {
            entry_left
        };

        // Guarantee predictable winding order for downstream rasterization stages.
        if canonical_y_dir {
            (
                Line {
                    p0: p_entry,
                    p1: p_exit,
                },
                top_is_on_left_edge,
                bot_is_on_left_edge,
            )
        } else {
            (
                Line {
                    p0: p_exit,
                    p1: p_entry,
                },
                top_is_on_left_edge,
                bot_is_on_left_edge,
            )
        }
    }
}

impl PartialOrd for Tile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Tile {
    /// `operator<`, on `toBits()`.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

/// `Tiles<kTileWidth, kTileHeight>::LineContext`.
// Port of: src/gpu/graphite/sparse_strips/Tiler.h#L430-L439 (chrome/m156)
struct LineContext {
    line_idx: u32,
    top_x: f32,
    top_y: f32,
    bottom_x: f32,
    bottom_y: f32,
    x_slope: f32,
    #[allow(dead_code)] // set by the C++ for completeness; the row loops do not read it
    line_left_x: f32,
    #[allow(dead_code)] // as `line_left_x`
    line_right_x: f32,
    p0_tile_x: i32,
    p0_tile_y: i32,
    p1_tile_x: i32,
    p1_tile_y: i32,
    tile_columns: u16,
}

/// Container that holds the tiles and manages their state (sorting). Generic over the tile size
/// to expose it as a statically tunable parameter.
// Port of: src/gpu/graphite/sparse_strips/Tiler.h#L222-L625 (chrome/m156)
#[doc(alias = "skgpu::graphite::Tiles")]
#[derive(Clone, Debug, Default)]
pub struct Tiles<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16> {
    tile_buf: Vec<Tile>,
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16> Tiles<K_TILE_WIDTH, K_TILE_HEIGHT> {
    // For now, only support square tiles.
    const _SQUARE: () = assert!(K_TILE_WIDTH == K_TILE_HEIGHT);

    /// `Tiles()`.
    #[must_use]
    pub fn new() -> Self {
        let () = Self::_SQUARE;
        Self {
            tile_buf: Vec::new(),
        }
    }

    /// `reset()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L226-L228 (chrome/m156)
    pub fn reset(&mut self) {
        self.tile_buf.clear();
    }

    /// `sortTiles(offset)`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L187-L189 (chrome/m156)
    #[doc(alias = "sortTiles")]
    pub fn sort_tiles_from(&mut self, offset: usize) {
        // Equal tiles have equal bits, so an unstable sort is indistinguishable from `std::sort`.
        self.tile_buf[offset..].sort_unstable();
    }

    /// `sortTiles()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L230-L232 (chrome/m156)
    #[doc(alias = "sortTiles")]
    pub fn sort_tiles(&mut self) {
        self.sort_tiles_from(0);
    }

    /// `getTiles()`.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L238 (chrome/m156)
    #[doc(alias = "getTiles")]
    #[must_use]
    pub fn get_tiles(&self) -> &[Tile] {
        &self.tile_buf
    }

    /// This function has two purposes:
    ///     1) The viewport is divided into tiles, whose dimensions are given by the const
    ///     parameters. Tiles act as a "super coarse rasterization stage," which elides carrying a
    ///     scanline for each MSAA subsample point. (Without this technique, we would require 64
    ///     scanlines for an 8 tall tile with 8xMSAA). So for each line produced by
    ///     flattening---contained inside Polyline---we need to find which tiles this line
    ///     intersects.  Tile edge touches are vertical exclusive, horizontal inclusive. This is
    ///     because:
    ///          A) In the vertical case, inclusivity would cause the coarse winding to be double
    ///          counted or negated by a single-point tile produced by a grazing touch.
    ///          B) In the horizontal case, the inclusivity is necessary because the tile produced
    ///          by the succeeding line may not consider itself left-touching (depending on its
    ///          direction), so the single-point tile is necessary to carry over the left-edge
    ///          winding.
    ///     Using the line's direction enforces mutually exclusive ownership of boundary
    ///     intersections between consecutive segments *of the same direction*, ensuring that the
    ///     left-edge winding is neither lost nor double-counted:
    ///
    ///     ```text
    ///                                  Left edge behavior:
    ///                      +----------------------+----------+------------+
    ///                      | X Direction          | Endpoint | L Bit Set? |
    ///                      +----------------------+----------+------------+
    ///                      | Left-to-Right (L->R) | Start    | No         |
    ///                      | Left-to-Right (L->R) | End      | Yes        |
    ///                      | Right-to-Left (R->L) | Start    | Yes        |
    ///                      | Right-to-Left (R->L) | End      | No         |
    ///                      +----------------------+----------+------------+
    ///     ```
    ///
    ///     2) To enable parallel rasterization, we need to establish a source of truth for the
    ///     line intersection points on tiles, such that adjacent tiles agree where intersections
    ///     occur. While calculating exact intersection coordinates here is feasible, it is
    ///     (relatively) computationally expensive, so instead, we defer the heavy math to the
    ///     GPU/rasterizer and produce a lightweight intersection bitmask. This mask unambiguously
    ///     defines which edges of a tile a line segment touches:
    ///
    ///     Bit representation:
    ///     Bit: 4 | 3 | 2 | 1 | 0
    ///     Val: W | R | L | B | T
    ///
    ///     - W (Winding): Tracks whether the line touched the top edge of the tile.
    ///     - R/L/B/T: Right, Left, Bottom, and Top edge intersections.
    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L281-L424 (chrome/m156)
    #[doc(alias = "makeTilesMSAA")]
    pub fn make_tiles_msaa(
        &mut self,
        polyline: &Polyline,
        viewport_width: u16,
        viewport_height: u16,
    ) {
        debug_assert!(polyline.count() <= IntersectionBits::MAX_LINES_PER_PATH as usize);

        if viewport_width == 0 || viewport_height == 0 {
            return;
        }

        // Will never underflow
        let tile_columns: u16 = Self::div_ceil(viewport_width, K_TILE_WIDTH) - 1;
        let tile_rows: u16 = Self::div_ceil(viewport_height, K_TILE_HEIGHT);

        let inv_w = 1.0f32 / f32::from(K_TILE_WIDTH);
        let inv_h = 1.0f32 / f32::from(K_TILE_HEIGHT);

        for (line, line_idx) in polyline {
            let line_idx = line_idx as u32;

            // map line into tile units
            let p0_x = line.p0.x * inv_w;
            let p0_y = line.p0.y * inv_h;
            let p1_x = line.p1.x * inv_w;
            let p1_y = line.p1.y * inv_h;

            let (line_left_x, line_right_x) = if p0_x < p1_x {
                (p0_x, p1_x)
            } else {
                (p1_x, p0_x)
            };

            // If the leftmost point of this line is right of the viewport, cull it. Although we
            // cull path verbs right of the viewport in the flattening stage, a right edge
            // crossing path verb may still be flattened into lines, some of which may be
            // completely outside of the viewport.
            if line_left_x >= f32::from(tile_columns) + 1.0 {
                // Note, lineLeftX > tileColumns is NOT equiavalent here, and so the + 1 cannot be
                // dropped
                continue;
            }

            let (line_top_y, line_top_x, line_bottom_y, line_bottom_x) = if p0_y < p1_y {
                (p0_y, p0_x, p1_y, p1_x)
            } else {
                (p1_y, p1_x, p0_y, p0_x)
            };

            let y_top_tiles: u16 = Self::f32_to_u16_sat(line_top_y).min(tile_rows);
            let line_bottom_y_ceil = line_bottom_y.ceil();
            let y_bottom_tiles: u16 = Self::f32_to_u16_sat(line_bottom_y_ceil).min(tile_rows);

            // If yTopTiles == yBottomTiles, then the line is either completely above or below the
            // viewport OR it is perfectly horizontal and aligned to the tile grid, contributing
            // no winding. In either case, it should be culled.
            if y_top_tiles >= y_bottom_tiles {
                // Technically, the `>` part of the `>=` is unnecessary due to clamping, but this
                // gives stronger signal.
                continue;
            }

            let p0_tile_x = line_top_x.floor() as i32;
            let p0_tile_y = line_top_y.floor() as i32;
            let p1_tile_x = line_bottom_x.floor() as i32;
            let p1_tile_y = if line_bottom_y == line_bottom_y_ceil {
                (line_bottom_y as i32) - 1
            } else {
                line_bottom_y.floor() as i32
            };

            // Each line processed falls into 1 of three categories:
            //  1) The line produces a single tile.
            //  2) The line is perfectly vertical.
            //  3) The line is neither 1 or 2 (general case)
            let not_same_tile = (p0_tile_y != p1_tile_y) || (p0_tile_x != p1_tile_x);
            if not_same_tile {
                if line_left_x == line_right_x {
                    // Vertical line case
                    let x: u16 = Self::f32_to_u16_sat(line_left_x).min(tile_columns);

                    // Process the Top Row (if visible on screen)
                    let mut y_start = y_top_tiles;
                    let is_start_culled = line_top_y < 0.0;
                    if !is_start_culled {
                        let winding = if f32::from(y_top_tiles) >= line_top_y {
                            W
                        } else {
                            0
                        };
                        let intersection_mask = B | winding;
                        self.tile_buf
                            .push(Tile::new(x, y_start, line_idx, intersection_mask));
                        y_start += 1;
                    }

                    // Process all "fully crossed" tiles (W | T | B).
                    let y_end_idx = p1_tile_y.min(i32::from(tile_rows));
                    for y_idx in i32::from(y_start)..y_end_idx {
                        let intersection_mask = W | T | B;
                        self.tile_buf
                            .push(Tile::new(x, y_idx as u16, line_idx, intersection_mask));
                    }

                    // Process the terminal tile (W | T), if it exists. We only emit this if the
                    // line actually terminates on the screen, and if we haven't already
                    // processed/culled it via `yStart`.
                    if p1_tile_y >= i32::from(y_start) && p1_tile_y < i32::from(tile_rows) {
                        let intersection_mask = W | T;
                        self.tile_buf.push(Tile::new(
                            x,
                            p1_tile_y as u16,
                            line_idx,
                            intersection_mask,
                        ));
                    }
                } else {
                    // General case
                    let dx = p1_x - p0_x;
                    let dy = p1_y - p0_y;
                    let x_slope = dx / dy;

                    // Package for the helper functions, with inlining this should be zero cost.
                    // Changing to a class member regresses perf by ~10% on TilerSortBench
                    let ctx = LineContext {
                        line_idx,
                        top_x: line_top_x,
                        top_y: line_top_y,
                        bottom_x: line_bottom_x,
                        bottom_y: line_bottom_y,
                        x_slope,
                        line_left_x,
                        line_right_x,
                        p0_tile_x,
                        p0_tile_y,
                        p1_tile_x,
                        p1_tile_y,
                        tile_columns,
                    };

                    if line_bottom_x > line_top_x {
                        // cannot be equal at this point
                        self.run_loops::<true>(
                            &ctx,
                            line_top_y,
                            line_bottom_y,
                            y_top_tiles,
                            tile_rows,
                        );
                    } else {
                        self.run_loops::<false>(
                            &ctx,
                            line_top_y,
                            line_bottom_y,
                            y_top_tiles,
                            tile_rows,
                        );
                    }
                }
            } else {
                // Single tile case
                let x_clamped: u16 = Self::f32_to_u16_sat(line_left_x).min(tile_columns);
                let winding = if f32::from(y_top_tiles) >= line_top_y {
                    W
                } else {
                    0
                };
                self.tile_buf
                    .push(Tile::new(x_clamped, y_top_tiles, line_idx, winding));
            }
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L443-L445 (chrome/m156)
    fn div_ceil(a: u16, b: u16) -> u16 {
        // The sum is an `int` in the C++, converted to `uint16_t` on return.
        ((u32::from(a) + u32::from(b) - 1) / u32::from(b)) as u16
    }

    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L447-L451 (chrome/m156)
    fn f32_to_u16_sat(v: f32) -> u16 {
        // std::clamp will catch +/- inf here, but not NaN. However we should never get NaN here.
        debug_assert!(!v.is_nan());
        v.clamp(0.0, 65535.0) as u16
    }

    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L453-L525 (chrome/m156)
    fn push_edge<const K_X_DIR: bool>(
        &mut self,
        ctx: &LineContext,
        x_idx: u16,
        y: u16,
        row_top_x: f32,
        row_bottom_x: f32,
        canonical_start: i32,
        canonical_end: u16,
        winding_input: u32,
        check_start: bool,
        check_end: bool,
    ) {
        // Determine whether this tile is the true start or end of the line within this horizontal
        // row. We need to account for clamping because the line may start or end off-screen (e.g.,
        // X = -5), but we clamp X to the viewport.
        let unc_row_start = u32::from(i32::from(x_idx) == canonical_start);
        let unc_row_end = u32::from(x_idx == canonical_end);

        // Relativize the start/end based on line direction
        let canonical_row_start = if K_X_DIR { unc_row_start } else { unc_row_end };
        let canonical_row_end = if K_X_DIR { unc_row_end } else { unc_row_start };

        // Mask out the Top/Bottom bits if this tile contains the line endpoints.
        let mut not_start_tile: u32 = 1;
        if check_start {
            not_start_tile ^=
                u32::from((i32::from(x_idx) == ctx.p0_tile_x) && (i32::from(y) == ctx.p0_tile_y));
        }

        let mut not_end_tile: u32 = 1;
        if check_end {
            not_end_tile ^=
                u32::from((i32::from(x_idx) == ctx.p1_tile_x) && (i32::from(y) == ctx.p1_tile_y));
        }

        let mut mask = winding_input;
        // If this tile is the start of the row, the line must have entered through the Top edge.
        // (Unless it's the line start).
        mask |= canonical_row_start & not_start_tile;
        // If this tile is the end of the row, the line must have exited through the Bottom edge.
        // (Unless it's the line end).
        mask |= (canonical_row_end & not_end_tile) << BOT_SHIFT;

        // If a tile is NOT the start of the row, it must have been entered horizontally. If it is
        // NOT the end, it must have exited horizontally. Base L/R on the direction of the line.
        if K_X_DIR {
            mask |= (1 ^ canonical_row_start) << LEFT_SHIFT;
            mask |= (1 ^ canonical_row_end) << RIGHT_SHIFT;
        } else {
            mask |= (1 ^ canonical_row_start) << RIGHT_SHIFT;
            mask |= (1 ^ canonical_row_end) << LEFT_SHIFT;
        }

        // Corner handling
        let x_left_f = f32::from(x_idx);
        let x_right_f = f32::from(x_idx) + 1.0;
        let trc = u32::from(row_top_x == x_right_f) & not_start_tile;
        let tlc = u32::from(row_top_x == x_left_f) & not_start_tile;
        let brc = u32::from(row_bottom_x == x_right_f) & not_end_tile;
        let blc = u32::from(row_bottom_x == x_left_f) & not_end_tile;

        // If the line hits the exact Top-Left corner, but it is NOT the canonical start of the
        // row, we must treat it as a Left intersection to properly bridge the mask to the
        // adjacent tile.
        let tie_break = tlc & (canonical_row_start ^ 1);

        // Force corners into into purely horizontal intersections. This makes the downstream
        // intersection calculation logic simpler.
        mask |= (tie_break | blc) << LEFT_SHIFT;
        mask |= (trc | brc) << RIGHT_SHIFT;
        mask &= !(tie_break | trc);
        mask &= !((blc | brc) << BOT_SHIFT);

        self.tile_buf.push(Tile::new(x_idx, y, ctx.line_idx, mask));
    }

    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L527-L576 (chrome/m156)
    fn process_row<const K_X_DIR: bool>(
        &mut self,
        ctx: &LineContext,
        y_idx: u16,
        row_top_x: f32,
        row_bottom_x: f32,
        w_mask: u32,
        check_start: bool,
        check_end: bool,
    ) {
        let lx = row_top_x.min(row_bottom_x);
        let rx = row_top_x.max(row_bottom_x);

        // Convert floating-point boundaries into discrete integer tile indices. Note:
        // `canonicalXStart` preserves the true start (even if negative) before clamping, which is
        // required by `pushEdge` to tie-break in some cases.
        let canonical_x_start = lx.floor() as i32;
        let canonical_x_end: u16 = Self::f32_to_u16_sat(rx);
        let x_start: u16 = Self::f32_to_u16_sat(lx);
        // Clamp the end of the row to the right viewport if necessary.
        let x_end_val: u16 = canonical_x_end.min(ctx.tile_columns);

        if x_start <= x_end_val {
            // Process the Leftmost Tile of the row. If this is the *only* tile in the row,
            // `isSingle`, it is both the row start and row end so the Winding (W) bit is passed
            // regardless of direction.
            let is_single = x_start == x_end_val;
            let w_left = (if K_X_DIR || is_single { W } else { 0 }) & w_mask;
            self.push_edge::<K_X_DIR>(
                ctx,
                x_start,
                y_idx,
                row_top_x,
                row_bottom_x,
                canonical_x_start,
                canonical_x_end,
                w_left,
                check_start,
                check_end,
            );

            // Process all captive "Middle" tiles in this row. These tiles never have vertical
            // crossings, and for the purpose of the intersection mask, are identical as they
            // always recieve [R | L]. Bulk append with `inner count.`
            let inner_count = i32::from(x_end_val) - i32::from(x_start) - 1;
            if inner_count > 0 {
                let inner_mask = R | L;
                for i in 0..inner_count {
                    self.tile_buf.push(Tile::new(
                        (i32::from(x_start) + 1 + i) as u16,
                        y_idx,
                        ctx.line_idx,
                        inner_mask,
                    ));
                }
            }

            // Process the Rightmost Tile of the row. Emitted only if the row spans more than one
            // tile (i.e., we haven't already processed this exact tile as the Leftmost Tile).
            if x_start < x_end_val {
                let w_right = (if K_X_DIR { 0 } else { W }) & w_mask;
                self.push_edge::<K_X_DIR>(
                    ctx,
                    x_end_val,
                    y_idx,
                    row_top_x,
                    row_bottom_x,
                    canonical_x_start,
                    canonical_x_end,
                    w_right,
                    check_start,
                    check_end,
                );
            }
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/Tiler.h#L578-L624 (chrome/m156)
    fn run_loops<const K_X_DIR: bool>(
        &mut self,
        ctx: &LineContext,
        line_top_y: f32,
        line_bottom_y: f32,
        y_top_tiles: u16,
        tile_rows: u16,
    ) {
        // Process the Top Row (if visible on screen)
        let mut y_start = y_top_tiles;
        let is_start_culled = line_top_y < 0.0;
        if !is_start_culled {
            let y = f32::from(y_start);
            let row_bottom_y = (y + 1.0).min(line_bottom_y);
            // Catch perfectly horizontal lines and/or prevent floating point drift
            let row_bottom_x = if row_bottom_y == ctx.bottom_y {
                ctx.bottom_x
            } else {
                ctx.top_x + (row_bottom_y - ctx.top_y) * ctx.x_slope
            };
            let mask = if y >= line_top_y { W } else { 0 };
            // The top row might ALSO be the bottom row, so checkEnd = true
            self.process_row::<K_X_DIR>(
                ctx,
                y_start,
                ctx.top_x,
                row_bottom_x,
                mask,
                /* check_start = */ true,
                /* check_end = */ true,
            );
            y_start += 1;
        }

        // Process all "Middle" fully crossed rows; the tiles cannot be the start or the end
        let y_end_idx = ctx.p1_tile_y.min(i32::from(tile_rows));
        for y_idx in i32::from(y_start)..y_end_idx {
            let y = y_idx as f32;
            // Although this seems like duplicate calculation, finding the intersections
            // independently allows the entire loop to auto-vectorize, and is well worth it.
            let row_top_x = ctx.top_x + (y - ctx.top_y) * ctx.x_slope;
            let row_bottom_x = ctx.top_x + (y + 1.0 - ctx.top_y) * ctx.x_slope;
            self.process_row::<K_X_DIR>(
                ctx,
                y_idx as u16,
                row_top_x,
                row_bottom_x,
                0xffff_ffff,
                /* check_start = */ false,
                /* check_end = */ false,
            );
        }

        // Process the Terminal Row, if it exists. I.e. if it's on-screen AND wasn't already
        // processed as the Top Row.
        if ctx.p1_tile_y >= i32::from(y_start) && ctx.p1_tile_y < i32::from(tile_rows) {
            let y = ctx.p1_tile_y as f32;
            // No guard is necessary here against horizontal lines, as a horizontal line would
            // have been processed as a starting row.
            let row_top_x = ctx.top_x + (y - ctx.top_y) * ctx.x_slope;
            // No need to check start (we are past it), but must check end.
            self.process_row::<K_X_DIR>(
                ctx,
                ctx.p1_tile_y as u16,
                row_top_x,
                ctx.bottom_x,
                0xffff_ffff,
                /* check_start = */ false,
                /* check_end = */ true,
            );
        }
    }
}
