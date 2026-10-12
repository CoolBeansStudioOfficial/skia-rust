// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/MSAA_LUT.h

//! [`MsaaLut`]: the lookup table of MSAA coverage masks of a line slicing through a pixel.
//!
//! Naively, calculating MSAA coverage in software is O(n) per each sample location. Instead we use
//! a variation of a lookup technique introduced by Laine and Karras, adapted to 2D by Li et al.,
//! and refined by the Vello project:
//! <https://dl.acm.org/doi/abs/10.1111/j.1467-8659.2010.01728.x>,
//! <https://dl.acm.org/doi/10.1145/2980179.2982434>,
//! <https://github.com/linebender/vello/blob/main/vello_shaders/shader/fine.wgsl>,
//! <https://github.com/linebender/vello/pull/64>.
//!
//! To precompute the MSAA coverage of a line segment slicing through a 1x1 pixel, the line is
//! parameterized by a continuous parameterization introduced by the Vello project: the infinite
//! range of standard slopes `m` and y-intercepts `b` is mapped into two new variables, `s`
//! (normalized slope, `s = 1 / (m + 1)`) and `t` (normalized translation,
//! `t = (b + m) / (m + 1)`), strictly bounded between 0 and 1. The half-plane equation becomes
//! `(x - (1 - t))(1 - s) - (y - t)s >= 0`.
//!
//! For each subsample location `(x, y)`, the equation is evaluated at discretized points, according
//! to the resolution of the LUT. The bottom half (`v >= kHeight / 2`) stores masks for positive
//! slopes; the top half stores masks for negative slopes, which flip the y-axis of the subsample
//! points (`y = 1.0 - y`). The `s` and `t` values are taken from the exact center of each grid
//! cell. If the result is `>= 0.0`, the sample is covered, and its bit is set in the mask.
//!
//! The actual positions of the subsample points use the D3D11 standard multisample pattern:
//! <https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels>

use std::marker::PhantomData;

use crate::graphite::sparse_strips::types::MsaaMask;

/// `MSAA_LUT<T>`.
// Port of: src/gpu/graphite/sparse_strips/MSAA_LUT.h#L107-L159 (chrome/m156)
#[doc(alias = "MSAA_LUT")]
#[derive(Clone, Copy, Debug)]
pub struct MsaaLut<T: MsaaMask>(PhantomData<T>);

impl<T: MsaaMask> MsaaLut<T> {
    /// `kWidth`.
    pub const K_WIDTH: i32 = 64;
    /// `kHeight`.
    pub const K_HEIGHT: i32 = 64;
    /// `kHalfHeight`.
    pub const K_HALF_HEIGHT: i32 = Self::K_HEIGHT / 2;
    /// `kPattern`: the x subsample position of each sample.
    pub const K_PATTERN: &'static [u8] = T::PATTERN;

    /// `kSampleCount`.
    #[must_use]
    pub fn k_sample_count() -> usize {
        size_of::<T>() * 8
    }

    /// `Make()`: generates the lookup table.
    // Port of: src/gpu/graphite/sparse_strips/MSAA_LUT.h#L116-L158 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::many_single_char_names)] // mirrors the C++ int to float casts of small values and its `u`, `v`, `s`, `t`, `x`, `y`
    pub fn make() -> Vec<T> {
        let sample_count = Self::k_sample_count();
        let scale = 1.0f32 / sample_count as f32;
        let mut sub_x = vec![0.0f32; sample_count];
        let mut sub_y = vec![0.0f32; sample_count];
        for k in 0..sample_count {
            sub_x[k] = (f32::from(Self::K_PATTERN[k]) + 0.5f32) * scale;
            sub_y[k] = (k as f32 + 0.5f32) * scale;
        }

        let mut lut: Vec<T> = Vec::with_capacity((Self::K_WIDTH * Self::K_HEIGHT) as usize);
        for i in 0..Self::K_WIDTH * Self::K_HEIGHT {
            let u = i % Self::K_WIDTH;
            let v = i / Self::K_WIDTH;
            let is_pos = v >= Self::K_HALF_HEIGHT;

            // Extract continuous parameters from the center of the grid cells
            let t = (u as f32 + 0.5f32) / Self::K_WIDTH as f32;
            let s = ((v % Self::K_HALF_HEIGHT) as f32 + 0.5f32) / Self::K_HALF_HEIGHT as f32;

            let mut mask = T::default();
            for k in 0..sample_count {
                let x = sub_x[k];
                let mut y = sub_y[k];
                if !is_pos {
                    y = 1.0f32 - y;
                }
                let val = (x - (1.0f32 - t)) * (1.0f32 - s) - (y - t) * s;
                if val >= 0.0f32 {
                    mask = mask | T::bit(k);
                }
            }
            lut.push(mask);
        }
        lut
    }
}

/// `GenerateMSAALUT<T>()`.
// Port of: src/gpu/graphite/sparse_strips/MSAA_LUT.h#L162-L169 (chrome/m156)
#[doc(alias = "GenerateMSAALUT")]
#[must_use]
pub fn generate_msaa_lut<T: MsaaMask>() -> Vec<T> {
    MsaaLut::<T>::make()
}
