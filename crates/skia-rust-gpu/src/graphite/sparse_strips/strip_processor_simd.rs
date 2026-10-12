// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/StripProcessorSimd.h

//! [`StripProcessorSimd`]: the SWAR implementation of the per-tile coverage generation of sparse
//! strips.
//!
//! Each pixel's eight subsample windings are one `SwarPixel` (eight bytes, one byte per
//! subsample): the winding is biased by `0x80` to fit `-128..=127` in a byte (non-zero rule) or is
//! the parity (even-odd rule), and every update is a wrapping byte operation. The C++ applies the
//! byte operations to `skvx` vectors; the byte lanes here are updated by the same operations on
//! `[u8; 8]`, which is bit-identical.
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
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use skia_rust_core::point::Point;
use skia_rust_simd::vx::Int8;

use crate::graphite::sparse_strips::config::SparseStripConfig;
use crate::graphite::sparse_strips::make_strips::StripProcessor;
use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::tiler::Tile;
use crate::graphite::sparse_strips::types::{Line, MsaaExactMaskObserver};

/// `SwarPixel`: the eight subsample windings of a pixel, one byte each.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L39 (chrome/m156)
type SwarPixel = [u8; 8];

/// `PixelBytes`: the eight byte lanes of a pixel.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L39 (chrome/m156)
type PixelBytes = [u8; 8];

/// `kFixedMult`: the 16.16 fixed point scale of the LUT position, times the 64 LUT columns.
const K_FIXED_MULT: f32 = 64.0f32 * 65536.0f32;

/// The most tile rows a processor supports (the row intersection array is fixed-size).
const K_MAX_TILE_ROWS: usize = 16;

/// `StripProcessorSimd<kTileWidth, kTileHeight, kIsWinding>::LineStepParams`.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L232-L238 (chrome/m156)
struct LineStepParams<'a> {
    mask_row_lut: &'a [u8],
    step_x_fixed: i32,
    step_y_fixed: i32,
    t_base_fixed: i32,
    sorted_x_dir: bool,
}

/// The SWAR strip processor. See [`StripProcessorScalar`](super::strip_processor_scalar) for the
/// algorithm; this one keeps the subsample windings as bytes and the LUT position as 16.16 fixed
/// point.
// Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L27-L614 (chrome/m156)
#[doc(alias = "skgpu::graphite::StripProcessorSimd")]
pub struct StripProcessorSimd<
    'a,
    const K_TILE_WIDTH: u16,
    const K_TILE_HEIGHT: u16,
    const K_IS_WINDING: bool,
> {
    /// `fSubsampleWinding[kTileHeight][kTileWidth]`, flattened.
    subsample_winding: Vec<SwarPixel>,
    coarse_winding: i32,
    is_inverse: bool,
    polyline: &'a Polyline,
    mask_lut: &'a [u8],
    observer: Option<MsaaExactMaskObserver<'a>>,
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool> std::fmt::Debug
    for StripProcessorSimd<'_, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StripProcessorSimd")
            .field("coarse_winding", &self.coarse_winding)
            .field("is_inverse", &self.is_inverse)
            .finish_non_exhaustive()
    }
}

impl<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool>
    StripProcessorSimd<'a, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    // -128 / 127 of winding. So a quality of 8 subsamples per pixel (MSAAx8) requires u32x2
    const _SAMPLES: () = assert!(SparseStripConfig::K_NUM_SUB_SAMPLES == 8);

    const W: usize = K_TILE_WIDTH as usize;
    const H: usize = K_TILE_HEIGHT as usize;

    /// `kInitialWinding`.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L230 (chrome/m156)
    const K_INITIAL_WINDING: u8 = if K_IS_WINDING { 0x80 } else { 0 };

    /// `StripProcessorSimd(isInverse, polyline, maskLut, observer)`.
    ///
    /// # Panics
    /// If the tile height is not a multiple of four or is taller than the fixed row intersection
    /// array.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L41-L58 (chrome/m156)
    #[must_use]
    pub fn new(
        is_inverse: bool,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) -> Self {
        let () = Self::_SAMPLES;
        assert!(Self::H < K_MAX_TILE_ROWS && Self::H % 4 == 0);
        let mut processor = Self {
            subsample_winding: vec![[0; 8]; Self::H * Self::W],
            coarse_winding: 0,
            is_inverse,
            polyline,
            mask_lut,
            observer,
        };
        processor.clear_winding(Self::K_INITIAL_WINDING);
        processor
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L59-L61 (chrome/m156)
    fn clear_winding(&mut self, val: u8) {
        self.subsample_winding.fill([val; 8]);
    }

    /// `ApplyWinding<N>(target, fillVal)`: adds (non-zero rule) or xors (even-odd rule) the fill
    /// byte to every byte lane of the pixel.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L240-L252 (chrome/m156)
    fn apply_winding(target: &mut SwarPixel, fill_byte: u8) {
        for byte in target.iter_mut() {
            if K_IS_WINDING {
                *byte = byte.wrapping_add(fill_byte);
            } else {
                *byte ^= fill_byte;
            }
        }
    }

    /// `GetActivesWide(v)`: for each byte lane, 1 if the subsample is active and 0 otherwise.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L258-L269 (chrome/m156)
    fn get_actives_wide(v: SwarPixel) -> SwarPixel {
        if K_IS_WINDING {
            let mut actives = [0u8; 8];
            for (active, byte) in actives.iter_mut().zip(v) {
                *active = u8::from(byte != 0x80);
            }
            actives
        } else {
            debug_assert!(v.iter().all(|byte| *byte <= 1));
            v
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L272-L298 (chrome/m156)
    fn observe_chunk(&mut self, row: usize, column: usize, chunk_size: usize) {
        for x in column..column + chunk_size {
            let v = self.subsample_winding[row * Self::W + x];
            let mut exact_mask: u8 = 0;
            let mut winding = Int8::splat(0);
            for (s, byte) in v.iter().enumerate() {
                let mut sample = *byte as i8;
                if K_IS_WINDING {
                    sample = byte.wrapping_sub(0x80) as i8;
                }
                winding[s] = i32::from(sample);
                if Self::should_fill(i32::from(sample)) {
                    exact_mask |= 1 << s;
                }
            }
            if self.is_inverse {
                exact_mask =
                    !exact_mask & ((1u32 << SparseStripConfig::K_NUM_SUB_SAMPLES) - 1) as u8;
            }
            if let Some(observer) = self.observer.as_mut() {
                observer(exact_mask, winding);
            }
        }
    }

    /// Resolves the subsample windings of a chunk of pixels to their equivalent 8-bit alpha
    /// values:
    ///
    ///  1) `get_actives_wide()`: Evaluates the winding rule, returning for each byte lane 1
    ///     (active) or 0 (inactive).
    ///  2) The active samples of the pixel are summed (the C++ sums the two 32-bit halves, then
    ///     horizontally sums the four byte lanes with a multiply by `0x01010101` and a shift).
    ///  3) Alpha Conversion: The total is multiplied by the maximum alpha value (255), summed by
    ///     subsampleCount / 2 for rounding, and right-shifted by log2(subsampleCount). This keeps
    ///     the entire conversion in integer math and avoids division.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L323-L351 (chrome/m156)
    fn process_chunk(
        &mut self,
        chunk_size: usize,
        row: usize,
        column: usize,
        tile_alpha_base: &mut [u8],
    ) {
        for (i, alpha_out) in tile_alpha_base[..chunk_size].iter_mut().enumerate() {
            let actives =
                Self::get_actives_wide(self.subsample_winding[row * Self::W + column + i]);
            let active_lo = u32::from_le_bytes([actives[0], actives[1], actives[2], actives[3]]);
            let active_hi = u32::from_le_bytes([actives[4], actives[5], actives[6], actives[7]]);
            let combined_bytes = active_lo.wrapping_add(active_hi);
            let active_samples = combined_bytes.wrapping_mul(0x0101_0101) >> 24;
            let inv_mask32: u32 = if self.is_inverse { 0xffff_ffff } else { 0 };
            let alpha32 = ((active_samples * 255 + 4) >> 3) ^ inv_mask32;
            *alpha_out = alpha32 as u8;
        }

        if self.observer.is_some() {
            self.observe_chunk(row, column, chunk_size);
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L353-L371 (chrome/m156)
    fn find_row_intersections(
        p_top: Point,
        p_bot: Point,
        dxdy: f32,
        start_y: i32,
        end_y: i32,
    ) -> [f32; K_MAX_TILE_ROWS + 1] {
        // The C++ leaves the element past the last row uninitialized; it is only read after it is
        // set to `pBot.fX` (`endY == kTileHeight`).
        let mut row_int = [0.0f32; K_MAX_TILE_ROWS + 1];
        // static_assert((kTileHeight & 3) == 0); each group of four rows is one skvx::float4.
        for k in (0..usize::from(K_TILE_HEIGHT)).step_by(4) {
            for i in 0..4 {
                let grid_y = (k as f32) + (i as f32);
                let grid_x = p_top.x + (grid_y - p_top.y) * dxdy;
                row_int[k + i] = grid_x;
            }
        }
        row_int[start_y as usize] = p_top.x;
        row_int[end_y as usize] = p_bot.x;
        row_int
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L373-L381 (chrome/m156)
    fn get_truncation_mask<const K_IS_START: bool>(p: f32, row: f32) -> u8 {
        let shift = (8.0f32 * (p - row)).round() as u32;
        let shifted = 0xffi32.checked_shl(shift).unwrap_or(0);
        if K_IS_START {
            shifted as u8
        } else {
            (!shifted) as u8
        }
    }

    /// Computes the normalized parameter space (s, t) `LineStepParams`, with the DDA steps in
    /// 16.16 fixed point. See the scalar processor for the derivation.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L383-L440 (chrome/m156)
    fn compute_line_step_params(
        &self,
        p_top: Point,
        p_bot: Point,
        dx: f32,
        dy: f32,
    ) -> LineStepParams<'a> {
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
        let mask_row_start = (lut_row * SparseStripConfig::K_LUT_MASK_WIDTH) as usize;
        let mask_row_lut = &self.mask_lut[mask_row_start..];

        let step_x = normal_x * inv_d;
        let step_y = normal_y * inv_d;
        let t_base = ((if has_positive_slope { normal_x } else { d }) - c) * inv_d;

        let step_x_fixed = (step_x * K_FIXED_MULT) as i32;
        let step_y_fixed = (step_y * K_FIXED_MULT) as i32;
        let t_base_fixed = (t_base * K_FIXED_MULT) as i32;

        let sorted_x_dir = p_top.x <= p_bot.x;

        LineStepParams {
            mask_row_lut,
            step_x_fixed,
            step_y_fixed,
            t_base_fixed,
            sorted_x_dir,
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L442-L463 (chrome/m156)
    fn fill_left(&mut self, y_edge: f32, canonical_x_dir: bool) {
        let fill_byte: u8 = if K_IS_WINDING {
            if canonical_x_dir { 0xFF } else { 1 }
        } else {
            1
        };

        let start_y = y_edge.ceil() as i32;
        for row in start_y..i32::from(K_TILE_HEIGHT) {
            // Each chunk of eight (or four) pixels is the same byte operation on every pixel.
            for column in 0..Self::W {
                Self::apply_winding(
                    &mut self.subsample_winding[(row as usize) * Self::W + column],
                    fill_byte,
                );
            }
        }
    }

    /// `processPixel<kCanonicalYDir, kIsEdgePixel>`: adds the LUT mask of the line at `t_fixed` to
    /// the pixel's subsample windings.
    ///
    /// The mask is turned into one byte lane per subsample: lane `k` is `0xFF` if bit `k` of the
    /// mask is set (a vector comparison `!= 0` yields `~0` for true), else `0`.
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L465-L518 (chrome/m156)
    fn process_pixel<const K_CANONICAL_Y_DIR: bool, const K_IS_EDGE_PIXEL: bool>(
        pixel: &mut SwarPixel,
        truncation_mask: u8,
        p_invert: PixelBytes,
        t_fixed: i32,
        mask_row_lut: &[u8],
    ) {
        let column = (t_fixed >> 16).clamp(0, SparseStripConfig::K_LUT_MASK_WIDTH_EXCL);
        let mut mask_val = mask_row_lut[column as usize];
        if K_IS_EDGE_PIXEL {
            mask_val &= truncation_mask;
        }

        let mut cmp = [0u8; 8];
        for (k, lane) in cmp.iter_mut().enumerate() {
            *lane = if (mask_val & (1 << k)) != 0 { 0xFF } else { 0 };
        }

        let mut p_subsample_winding: PixelBytes = *pixel;
        if K_IS_WINDING {
            for k in 0..8 {
                let p_res = cmp[k].wrapping_sub(p_invert[k]);
                if K_CANONICAL_Y_DIR {
                    p_subsample_winding[k] = p_subsample_winding[k].wrapping_sub(p_res);
                } else {
                    p_subsample_winding[k] = p_subsample_winding[k].wrapping_add(p_res);
                }
            }
        } else {
            for k in 0..8 {
                let p_res = (cmp[k] & 1) ^ p_invert[k];
                p_subsample_winding[k] ^= p_res;
            }
        }
        *pixel = p_subsample_winding;
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L522-L604 (chrome/m156)
    #[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ signature
    fn process_row_span<const K_CANONICAL_Y_DIR: bool>(
        &mut self,
        row: i32,
        left_mask: u8,
        right_mask: u8,
        left_invert: bool,
        mid_invert: bool,
        right_invert: bool,
        crossed_top: bool,
        row_int: &[f32; K_MAX_TILE_ROWS + 1],
        params: &LineStepParams<'_>,
    ) {
        let p_top_x = row_int[row as usize];
        let p_bot_x = row_int[(row + 1) as usize];

        let x_min = p_top_x.min(p_bot_x);
        let x_max = p_top_x.max(p_bot_x);
        let x_start = (x_min.floor() as i32).clamp(0, i32::from(K_TILE_WIDTH) - 1);
        let x_end = (x_max.floor() as i32).clamp(0, i32::from(K_TILE_WIDTH) - 1);

        // The C++ computes this in (wrapping, in practice) `int32_t` arithmetic.
        let mut t_fixed = params
            .t_base_fixed
            .wrapping_add(params.step_y_fixed.wrapping_mul(row))
            .wrapping_add(params.step_x_fixed.wrapping_mul(x_start));

        let row_base = (row as usize) * Self::W;
        let invert_byte: u8 = if K_IS_WINDING { 0xff } else { 1 };

        if x_start == x_end {
            let combined_mask = left_mask & right_mask;
            Self::process_pixel::<K_CANONICAL_Y_DIR, true>(
                &mut self.subsample_winding[row_base + x_start as usize],
                combined_mask,
                [if left_invert { invert_byte } else { 0 }; 8],
                t_fixed,
                params.mask_row_lut,
            );
        } else {
            Self::process_pixel::<K_CANONICAL_Y_DIR, true>(
                &mut self.subsample_winding[row_base + x_start as usize],
                left_mask,
                [if left_invert { invert_byte } else { 0 }; 8],
                t_fixed,
                params.mask_row_lut,
            );
            t_fixed = t_fixed.wrapping_add(params.step_x_fixed);

            for column in (x_start + 1)..x_end {
                Self::process_pixel::<K_CANONICAL_Y_DIR, false>(
                    &mut self.subsample_winding[row_base + column as usize],
                    0,
                    [if mid_invert { invert_byte } else { 0 }; 8],
                    t_fixed,
                    params.mask_row_lut,
                );
                t_fixed = t_fixed.wrapping_add(params.step_x_fixed);
            }

            Self::process_pixel::<K_CANONICAL_Y_DIR, true>(
                &mut self.subsample_winding[row_base + x_end as usize],
                right_mask,
                [if right_invert { invert_byte } else { 0 }; 8],
                t_fixed,
                params.mask_row_lut,
            );
        }

        if crossed_top {
            let fill_byte: u8 = if K_IS_WINDING {
                if K_CANONICAL_Y_DIR { 1 } else { 0xFF }
            } else {
                1
            };

            // The C++ applies the fill to chunks of eight (or four) pixels, then to the
            // remaining pixels one by one: the same byte operation on every pixel.
            for column in (x_end + 1)..i32::from(K_TILE_WIDTH) {
                Self::apply_winding(
                    &mut self.subsample_winding[row_base + column as usize],
                    fill_byte,
                );
            }
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L117-L226 (chrome/m156)
    fn rasterize_line_to_tile_impl<const K_CANONICAL_Y_DIR: bool>(
        &mut self,
        tile: Tile,
        tile_bounds: [Point; 2],
        line: &Line,
    ) {
        let canonical_x_dir = line.p1.x >= line.p0.x;

        let winding_bit: u32 = u32::from(tile.coarse_winding());
        if K_IS_WINDING {
            self.coarse_winding += (if K_CANONICAL_Y_DIR { 1 } else { -1 }) * (winding_bit as i32);
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
                line,
                &tile_bounds,
                &derivs,
                tile.intersection_mask(),
                canonical_x_dir,
                K_CANONICAL_Y_DIR,
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

        if start_y < end_y {
            let is_only_row = start_y == end_y - 1;
            let row_int = Self::find_row_intersections(p_top, p_bot, dxdy, start_y, end_y);
            let step_params = self.compute_line_step_params(p_top, p_bot, dx, dy);

            // Process the first row
            {
                let p_top_y = p_top.y;
                let p_bot_y = if is_only_row {
                    p_bot.y
                } else {
                    (start_y + 1) as f32
                };

                let crossed_top = p_top_y == p_top_y.floor();
                let default_invert = !crossed_top && step_params.sorted_x_dir;

                let mut start_mask_val: u8 = 0xff;
                let start_invert = top_is_on_left_edge && !crossed_top;
                if !top_is_on_left_edge {
                    start_mask_val = Self::get_truncation_mask::<true>(p_top_y, start_y as f32);
                }

                let mut end_mask_val: u8 = 0xff;
                if is_only_row && !bot_is_on_left_edge {
                    end_mask_val = Self::get_truncation_mask::<false>(p_bot_y, start_y as f32);
                }

                let left_invert = if step_params.sorted_x_dir {
                    start_invert
                } else {
                    default_invert
                };
                let right_invert = if step_params.sorted_x_dir {
                    default_invert
                } else {
                    start_invert
                };
                let left_mask = if step_params.sorted_x_dir {
                    start_mask_val
                } else {
                    end_mask_val
                };
                let right_mask = if step_params.sorted_x_dir {
                    end_mask_val
                } else {
                    start_mask_val
                };

                self.process_row_span::<K_CANONICAL_Y_DIR>(
                    start_y,
                    left_mask,
                    right_mask,
                    left_invert,
                    default_invert,
                    right_invert,
                    crossed_top,
                    &row_int,
                    &step_params,
                );
            }

            // Process the middle rows
            for row in (start_y + 1)..(end_y - 1) {
                self.process_row_span::<K_CANONICAL_Y_DIR>(
                    row,
                    /* left_mask = */ 0xff,
                    /* right_mask = */ 0xff,
                    /* left_invert = */ false,
                    /* mid_invert = */ false,
                    /* right_invert = */ false,
                    /* crossed_top = */ true,
                    &row_int,
                    &step_params,
                );
            }

            // Process the last row
            if !is_only_row {
                let last_y = end_y - 1;
                let mut end_mask_last: u8 = 0xff;
                if !bot_is_on_left_edge {
                    end_mask_last = Self::get_truncation_mask::<false>(p_bot.y, last_y as f32);
                }

                let left_mask = if step_params.sorted_x_dir {
                    0xff
                } else {
                    end_mask_last
                };
                let right_mask = if step_params.sorted_x_dir {
                    end_mask_last
                } else {
                    0xff
                };

                self.process_row_span::<K_CANONICAL_Y_DIR>(
                    last_y,
                    left_mask,
                    right_mask,
                    /* left_invert = */ false,
                    /* mid_invert = */ false,
                    /* right_invert = */ false,
                    /* crossed_top = */ true,
                    &row_int,
                    &step_params,
                );
            }
        }
    }
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, const K_IS_WINDING: bool> StripProcessor
    for StripProcessorSimd<'_, K_TILE_WIDTH, K_TILE_HEIGHT, K_IS_WINDING>
{
    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L63-L73 (chrome/m156)
    fn clear_with_coarse_winding(&mut self) {
        let winding_byte: u8 = if K_IS_WINDING {
            // Cast to i8 to sign extend before casting to u8, then apply SWAR bias to map
            // -127/128 to 0/255.
            0x80u8.wrapping_add(self.coarse_winding as i8 as u8)
        } else {
            u8::from((self.coarse_winding & 1) != 0)
        };
        self.clear_winding(winding_byte);
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L75 (chrome/m156)
    fn clear_winding_for_new_row(&mut self) {
        self.clear_winding(Self::K_INITIAL_WINDING);
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L77-L83 (chrome/m156)
    fn should_fill(w: i32) -> bool {
        if K_IS_WINDING { w != 0 } else { (w & 1) != 0 }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L85 (chrome/m156)
    fn coarse_winding(&self) -> i32 {
        self.coarse_winding
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L86 (chrome/m156)
    fn set_coarse_winding(&mut self, val: i32) {
        self.coarse_winding = val;
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L91-L105 (chrome/m156)
    fn resolve_winding_to_alpha(&mut self, tile_alpha_base: &mut [u8]) {
        let mut offset = 0;
        for row in 0..Self::H {
            if Self::W % 8 == 0 {
                for column in (0..Self::W).step_by(8) {
                    self.process_chunk(8, row, column, &mut tile_alpha_base[offset..]);
                    offset += 8;
                }
            } else if Self::W % 4 == 0 {
                for column in (0..Self::W).step_by(4) {
                    self.process_chunk(4, row, column, &mut tile_alpha_base[offset..]);
                    offset += 4;
                }
            }
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/StripProcessorSimd.h#L107-L115 (chrome/m156)
    fn rasterize_line_to_tile(&mut self, tile: &Tile, tile_bounds: [Point; 2]) {
        let line = self.polyline.get_line(tile.line_idx());
        let canonical_y_dir = line.p1.y >= line.p0.y;
        if canonical_y_dir {
            self.rasterize_line_to_tile_impl::<true>(*tile, tile_bounds, &line);
        } else {
            self.rasterize_line_to_tile_impl::<false>(*tile, tile_bounds, &line);
        }
    }
}
