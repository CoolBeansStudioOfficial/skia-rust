// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/StripProcessorScalar.h

//! [`StripProcessorScalar`]: the scalar implementation of the per-tile coverage generation of
//! sparse strips.
//!
//! The casts between the narrow integer types and `f32` below mirror the C++ integer promotions
//! and `static_cast`s of the ported arithmetic.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::similar_names,
    clippy::too_many_lines
)]

use skia_rust_core::point::Point;
use skia_rust_simd::vx::Int8;

use crate::graphite::sparse_strips::config::SparseStripConfig;
use crate::graphite::sparse_strips::make_strips::StripProcessor;
use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::tiler::Tile;
use crate::graphite::sparse_strips::types::{Line, MsaaExactMaskObserver};

/// The number of subsamples per pixel.
const K_SUBSAMPLES: usize = SparseStripConfig::K_NUM_SUB_SAMPLES as usize;

/// The most tile rows a processor supports (the row intersection array is fixed-size).
const K_MAX_TILE_ROWS: usize = 16;

/// `StripProcessorScalar<kTileWidth, kTileHeight, kIsWinding>::LineStepParams`.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L786-L792 (chrome/m156)
struct LineStepParams {
    lut_row: i32,
    step_x: f32,
    step_y: f32,
    t_base: f32,
    sorted_x_dir: bool,
}

/// The scalar strip processor. See the C++ for the long derivation of the algorithm: the
/// hierarchical winding (a coarse winding carried at the top left corner of the tile, and a
/// subsample winding per pixel), the LUT lookup of the subsample coverage of a line, and the
/// truncation of the LUT masks at line endpoints that lie inside a pixel.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L21-L1066 (chrome/m156)
#[doc(alias = "skgpu::graphite::StripProcessorScalar")]
pub struct StripProcessorScalar<
    'a,
    const K_TILE_WIDTH: u16,
    const K_TILE_HEIGHT: u16,
    const K_IS_WINDING: bool,
> {
    /// `fSubsampleWinding[kTileHeight][kTileWidth][kNumSubSamples]`, flattened.
    subsample_winding: Vec<i16>,
    coarse_winding: i32,
    is_inverse: bool,
    polyline: &'a Polyline,
    mask_lut: &'a [u8],
    observer: Option<MsaaExactMaskObserver<'a>>,
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool> std::fmt::Debug
    for StripProcessorScalar<'_, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StripProcessorScalar")
            .field("coarse_winding", &self.coarse_winding)
            .field("is_inverse", &self.is_inverse)
            .finish_non_exhaustive()
    }
}

impl<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool>
    StripProcessorScalar<'a, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    const W: usize = K_TILE_WIDTH as usize;
    const H: usize = K_TILE_HEIGHT as usize;

    /// `StripProcessorScalar(isInverse, polyline, maskLut, observer)`.
    ///
    /// # Panics
    /// If the tile is taller than the fixed row intersection array.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L23-L39 (chrome/m156)
    #[must_use]
    pub fn new(
        is_inverse: bool,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) -> Self {
        assert!(Self::H < K_MAX_TILE_ROWS);
        let mut processor = Self {
            subsample_winding: vec![0; Self::H * Self::W * K_SUBSAMPLES],
            coarse_winding: 0,
            is_inverse,
            polyline,
            mask_lut,
            observer,
        };
        processor.clear_winding_for_new_row();
        processor
    }

    fn index(row: usize, column: usize, k: usize) -> usize {
        (row * Self::W + column) * K_SUBSAMPLES + k
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L41-L49 (chrome/m156)
    fn clear_winding(&mut self, value: i16) {
        self.subsample_winding.fill(value);
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L798-L811 (chrome/m156)
    fn observe_pixel(&mut self, row: usize, column: usize) {
        let mut exact_mask: u8 = 0;
        let mut winding = Int8::splat(0);
        for k in 0..K_SUBSAMPLES {
            let w = self.subsample_winding[Self::index(row, column, k)];
            winding[k] = i32::from(w as i8);
            if Self::should_fill(i32::from(w)) {
                exact_mask |= 1 << k;
            }
        }
        if self.is_inverse {
            exact_mask = !exact_mask & ((1u32 << K_SUBSAMPLES) - 1) as u8;
        }
        if let Some(observer) = self.observer.as_mut() {
            observer(exact_mask, winding);
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L817-L842 (chrome/m156)
    fn process_pixel(
        &mut self,
        row: usize,
        column: usize,
        tile_alpha_base: &mut [u8],
        local_write_idx: usize,
    ) {
        let mut active_samples: i32 = 0;
        for k in 0..K_SUBSAMPLES {
            if Self::should_fill(i32::from(
                self.subsample_winding[Self::index(row, column, k)],
            )) {
                active_samples += 1;
            }
        }
        if self.is_inverse {
            active_samples = SparseStripConfig::K_NUM_SUB_SAMPLES - active_samples;
        }

        if self.observer.is_some() {
            self.observe_pixel(row, column);
        }

        let alpha = ((active_samples * 255 + (SparseStripConfig::K_NUM_SUB_SAMPLES / 2))
            / SparseStripConfig::K_NUM_SUB_SAMPLES) as u8;
        tile_alpha_base[local_write_idx] = alpha;
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L846-L867 (chrome/m156)
    fn fill_left(&mut self, y_edge: f32, canonical_x_dir: bool) {
        let start_y = y_edge.ceil() as i32;
        let val: i32 = if K_IS_WINDING {
            if canonical_x_dir { -1 } else { 1 }
        } else {
            1
        };
        for row in start_y..i32::from(K_TILE_HEIGHT) {
            for column in 0..Self::W {
                for k in 0..K_SUBSAMPLES {
                    let w = &mut self.subsample_winding[Self::index(row as usize, column, k)];
                    if K_IS_WINDING {
                        *w = (i32::from(*w) + val) as i16;
                    } else {
                        *w = (i32::from(*w) ^ val) as i16;
                    }
                }
            }
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L869-L882 (chrome/m156)
    fn find_row_intersections(
        p_top: Point,
        p_bot: Point,
        dxdy: f32,
        start_y: i32,
        end_y: i32,
    ) -> [Point; K_MAX_TILE_ROWS + 1] {
        let mut row_int = [Point::new(f32::NAN, f32::NAN); K_MAX_TILE_ROWS + 1];
        row_int[start_y as usize] = p_top;
        for row in (start_y + 1)..end_y {
            let gy = row as f32;
            let gx = p_top.x + (gy - p_top.y) * dxdy;
            row_int[row as usize] = Point::new(gx, gy);
        }
        row_int[end_y as usize] = p_bot;
        row_int
    }

    /// Each subsample occupies its own distinct 1/8th vertical slice of the pixel (`MSAA_LUT.h`),
    /// so the subsamples are ordered sequentially by their Y-coordinates: bitshifting left/right
    /// correctly corresponds to truncating top/bottom.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L927-L935 (chrome/m156)
    fn get_truncation_mask<const K_IS_START: bool>(p: f32, row: f32) -> u8 {
        let shift = (8.0f32 * (p - row)).round() as u32;
        let shifted = 0xffi32.checked_shl(shift).unwrap_or(0);
        if K_IS_START {
            shifted as u8
        } else {
            (!shifted) as u8
        }
    }

    /// Computes the normalized parameter space (s, t) `LineStepParams` used to index into the
    /// precomputed MSAA look-up table, with the line stepped DDA-style from pixel to pixel. See
    /// the C++ for the derivation.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L989-L1036 (chrome/m156)
    fn compute_line_step_params(p_top: Point, p_bot: Point, dx: f32, dy: f32) -> LineStepParams {
        let mut normal_x = dy;
        let mut normal_y = -dx;
        if normal_x < 0.0 {
            normal_x = -normal_x;
            normal_y = -normal_y;
        }

        let d = normal_x + normal_y.abs();
        let inv_d = if d < SparseStripConfig::K_STRIP_EPSILON {
            0.0
        } else {
            1.0 / d
        };
        let has_positive_slope = normal_y <= 0.0;
        let c = normal_x * p_top.x + normal_y * p_top.y;
        let s = normal_y.abs() * inv_d;

        let lut_row_offset = ((s * (SparseStripConfig::K_LUT_MASK_HEIGHT / 2) as f32).floor()
            as i32)
            .clamp(0, (SparseStripConfig::K_LUT_MASK_HEIGHT / 2) - 1);
        let lut_row = if has_positive_slope {
            lut_row_offset + SparseStripConfig::K_LUT_MASK_HEIGHT / 2
        } else {
            lut_row_offset
        };

        let step_x = normal_x * inv_d;
        let step_y = normal_y * inv_d;
        let t_base = ((if has_positive_slope { normal_x } else { d }) - c) * inv_d;
        let sorted_x_dir = p_top.x <= p_bot.x;

        LineStepParams {
            lut_row,
            step_x,
            step_y,
            t_base,
            sorted_x_dir,
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L1042-L1049 (chrome/m156)
    fn lut_lookup(&self, t: f32, lut_row: i32) -> u8 {
        let u = ((t * SparseStripConfig::K_LUT_MASK_WIDTH_F).floor() as i32)
            .clamp(0, SparseStripConfig::K_LUT_MASK_WIDTH_EXCL);
        let index = lut_row * SparseStripConfig::K_LUT_MASK_WIDTH + u;
        debug_assert!((index as usize) < self.mask_lut.len());
        self.mask_lut[index as usize]
    }
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool> StripProcessor
    for StripProcessorScalar<'_, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L51-L53 (chrome/m156)
    fn clear_with_coarse_winding(&mut self) {
        self.clear_winding(self.coarse_winding as i16);
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L55 (chrome/m156)
    fn clear_winding_for_new_row(&mut self) {
        self.clear_winding(0);
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L57-L63 (chrome/m156)
    fn should_fill(w: i32) -> bool {
        if K_IS_WINDING {
            w != 0 // NonZero
        } else {
            (w & 1) != 0 // EvenOdd
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L65 (chrome/m156)
    fn coarse_winding(&self) -> i32 {
        self.coarse_winding
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L66 (chrome/m156)
    fn set_coarse_winding(&mut self, val: i32) {
        self.coarse_winding = val;
    }

    /// Once all tiles at the same geometric location have been combined, the subsample winding
    /// must be converted to alpha values consumable by the fragment shader.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L70-L78 (chrome/m156)
    fn resolve_winding_to_alpha(&mut self, tile_alpha_base: &mut [u8]) {
        let mut local_write_idx = 0;

        for row in 0..Self::H {
            for column in 0..Self::W {
                self.process_pixel(row, column, tile_alpha_base, local_write_idx);
                local_write_idx += 1;
            }
        }
    }

    /// The core function of strip generation. It takes the line, coarse winding, and
    /// intersection mask and converts it to subsample winding. There are two core techniques that
    /// make MSAA calculation on the CPU feasible:
    ///
    ///  1. LUT Lookup (Sub-pixel Coverage): a fixed set of subsample coverage patterns are
    ///     precomputed into a look-up table.
    ///  2. Hierarchical Winding: the "coarse winding" is only carried at the top left corner of
    ///     the tile and each subsample reconstructs its winding from that point using the
    ///     intersected line.
    ///
    /// See the C++ for the diagrams of the coarse phase (crossing top and propagating right, fill
    /// left) and of the fine phase (per row intersections, LUT lookup, truncation, truncation for
    /// left-edge touches, and winding and coverage inversion).
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorScalar.h#L601-L781 (chrome/m156)
    fn rasterize_line_to_tile(&mut self, tile: &Tile, tile_bounds: [Point; 2]) {
        let line: Line = self.polyline.get_line(tile.line_idx());
        let canonical_x_dir = line.p1.x >= line.p0.x;
        let canonical_y_dir = line.p1.y >= line.p0.y;

        let winding_bit: u32 = u32::from(tile.coarse_winding());
        if K_IS_WINDING {
            self.coarse_winding += (if canonical_y_dir { 1 } else { -1 }) * (winding_bit as i32);
        } else {
            self.coarse_winding ^= winding_bit as i32;
        }

        let right_edge = if canonical_x_dir {
            line.p1.x
        } else {
            line.p0.x
        };
        if right_edge < 0.0 {
            return;
        }

        let dx = line.p1.x - line.p0.x;
        let dy = line.p1.y - line.p0.y;
        let inv_dx = if dx.abs() <= SparseStripConfig::K_STRIP_EPSILON {
            0.0
        } else {
            1.0 / dx
        };
        let inv_dy = if dy.abs() <= SparseStripConfig::K_STRIP_EPSILON {
            0.0
        } else {
            1.0 / dy
        };
        let dxdy = dx * inv_dy;

        let derivs = [dx, dy, inv_dx, inv_dy];
        let (clipped_line, top_is_on_left_edge, bot_is_on_left_edge) =
            Tile::clip_to_tile::<K_TILE_WIDTH, K_TILE_HEIGHT>(
                &line,
                &tile_bounds,
                &derivs,
                tile.intersection_mask(),
                canonical_x_dir,
                canonical_y_dir,
            );

        let p_top = clipped_line.p0;
        let p_bot = clipped_line.p1;

        if tile.has_left_intersection() {
            let y_edge = if p_top.x < p_bot.x { p_top.y } else { p_bot.y };
            self.fill_left(y_edge, canonical_x_dir);
        }

        if dy.abs() < SparseStripConfig::K_STRIP_EPSILON && p_top.y == p_top.y.floor() {
            return;
        }

        let start_y = p_top.y.floor() as i32;
        let end_y = p_bot.y.ceil() as i32;
        let row_int = Self::find_row_intersections(p_top, p_bot, dxdy, start_y, end_y);
        let step_params = Self::compute_line_step_params(p_top, p_bot, dx, dy);

        for row in start_y..end_y {
            let p_top_x = row_int[row as usize].x;
            let p_top_y = row_int[row as usize].y;
            let p_bot_x = row_int[(row + 1) as usize].x;
            let p_bot_y = row_int[(row + 1) as usize].y;
            if p_top_x.is_nan() || p_bot_x.is_nan() {
                continue;
            }

            let x_min = p_top_x.min(p_bot_x);
            let x_max = p_top_x.max(p_bot_x);
            let x_start = (x_min.floor() as i32).clamp(0, i32::from(K_TILE_WIDTH) - 1);
            let x_end = (x_max.floor() as i32).clamp(0, i32::from(K_TILE_WIDTH) - 1);

            let mut t_val = step_params.t_base
                + step_params.step_y * (row as f32)
                + step_params.step_x * (x_start as f32);

            let is_start_y = row == start_y;
            let is_end_y_minus1 = row == end_y - 1;

            let crossed_top = p_top_y == p_top_y.floor();
            let default_invert = !crossed_top && step_params.sorted_x_dir;

            let canonical_start_x = if step_params.sorted_x_dir {
                x_start
            } else {
                x_end
            };
            let canonical_end_x = if step_params.sorted_x_dir {
                x_end
            } else {
                x_start
            };

            let mut start_mask: u8 = 0xff;
            let mut start_invert = default_invert;
            if is_start_y {
                start_invert = top_is_on_left_edge && !crossed_top;
                if !top_is_on_left_edge {
                    start_mask = Self::get_truncation_mask::<true>(p_top_y, row as f32);
                }
            }

            let mut end_mask: u8 = 0xff;
            if is_end_y_minus1 && !bot_is_on_left_edge {
                end_mask = Self::get_truncation_mask::<false>(p_bot_y, row as f32);
            }

            for column in x_start..=x_end {
                let mut mask_val = self.lut_lookup(t_val, step_params.lut_row);

                let mut edge_mask: u8 = 0xff;
                if column == canonical_start_x {
                    edge_mask &= start_mask;
                }
                if column == canonical_end_x {
                    edge_mask &= end_mask;
                }
                mask_val &= edge_mask;

                let should_invert = i32::from(if is_start_y && column == canonical_start_x {
                    start_invert
                } else {
                    default_invert
                });

                for k in 0..K_SUBSAMPLES {
                    let mut mask_bit = i32::from((mask_val & (1 << k)) != 0);
                    let w =
                        &mut self.subsample_winding[Self::index(row as usize, column as usize, k)];
                    if K_IS_WINDING {
                        mask_bit -= should_invert;
                        if mask_bit != 0 {
                            if canonical_y_dir {
                                *w = (i32::from(*w) + mask_bit) as i16;
                            } else {
                                *w = (i32::from(*w) - mask_bit) as i16;
                            }
                        }
                    } else {
                        *w = (i32::from(*w) ^ (mask_bit ^ should_invert)) as i16;
                    }
                }
                t_val += step_params.step_x;
            }

            if crossed_top {
                let val: i32 = if K_IS_WINDING {
                    if canonical_y_dir { 1 } else { -1 }
                } else {
                    1
                };
                for column in (x_end + 1)..i32::from(K_TILE_WIDTH) {
                    for k in 0..K_SUBSAMPLES {
                        let w = &mut self.subsample_winding
                            [Self::index(row as usize, column as usize, k)];
                        if K_IS_WINDING {
                            *w = (i32::from(*w) + val) as i16;
                        } else {
                            *w = (i32::from(*w) ^ val) as i16;
                        }
                    }
                }
            }
        }
    }
}
