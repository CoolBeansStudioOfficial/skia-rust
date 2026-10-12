// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/SparseStripsTypes.h

//! The types shared by the sparse strips stages: the MSAA sample patterns, [`Line`],
//! [`IntersectionBits`] and the exact-mask observer.

use std::ops::BitOr;

use skia_rust_core::point::Point;
use skia_rust_simd::vx::Int8;

/// The sample mask of one pixel: `u8` for 8x MSAA, `u16` for 16x MSAA.
///
/// Port of the `kMsaaPattern<T>` variable template and of the `sizeof(T) * 8` sample counts that
/// `MSAA_LUT<T>` derives from `T`.
pub trait MsaaMask: Copy + Default + BitOr<Output = Self> {
    /// `kMsaaPattern<T>`: the x subsample position of each sample (sample `k` is on the `k`th of
    /// `kSampleCount` horizontal slices of the pixel).
    const PATTERN: &'static [u8];

    /// `static_cast<T>(1) << k`.
    fn bit(k: usize) -> Self;
}

impl MsaaMask for u8 {
    const PATTERN: &'static [u8] = &K_MSAA_PATTERN_U8;

    fn bit(k: usize) -> Self {
        1u8 << k
    }
}

impl MsaaMask for u16 {
    const PATTERN: &'static [u8] = &K_MSAA_PATTERN_U16;

    fn bit(k: usize) -> Self {
        1u16 << k
    }
}

/// `kMsaaPattern<uint8_t>`.
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L24-L26 (chrome/m156)
pub const K_MSAA_PATTERN_U8: [u8; 8] = [0, 5, 3, 7, 1, 4, 6, 2];

/// `kMsaaPattern<uint16_t>`.
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L27-L28 (chrome/m156)
pub const K_MSAA_PATTERN_U16: [u8; 16] = [1, 8, 4, 11, 15, 7, 3, 12, 0, 9, 5, 13, 2, 10, 6, 14];

/// `FlattenMode`.
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L33-L36 (chrome/m156)
#[doc(alias = "skgpu::graphite::FlattenMode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlattenMode {
    /// `kScalar`.
    Scalar,
    /// `kSimd`.
    Simd,
}

/// `Line`: a line segment.
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L38-L41 (chrome/m156)
#[doc(alias = "skgpu::graphite::Line")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line {
    /// `p0`.
    pub p0: Point,
    /// `p1`.
    pub p1: Point,
}

/// Helper struct for the intersection bits. These will be moved into a centralized header soon.
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L44-L72 (chrome/m156)
#[doc(alias = "skgpu::graphite::IntersectionBits")]
#[derive(Clone, Copy, Debug)]
pub struct IntersectionBits;

impl IntersectionBits {
    /// `T`: the top edge.
    pub const T: u32 = 0b00001;
    /// `B`: the bottom edge.
    pub const B: u32 = 0b00010;
    /// `L`: the left edge.
    pub const L: u32 = 0b00100;
    /// `R`: the right edge.
    pub const R: u32 = 0b01000;
    /// `W`: winding.
    pub const W: u32 = 0b10000;

    /// `BOT_SHIFT`.
    pub const BOT_SHIFT: u32 = 1;
    /// `LEFT_SHIFT`.
    pub const LEFT_SHIFT: u32 = 2;
    /// `RIGHT_SHIFT`.
    pub const RIGHT_SHIFT: u32 = 3;
    /// `WINDING_SHIFT`.
    pub const WINDING_SHIFT: u32 = 4;
    /// `INT_MASK_SHIFT`.
    pub const INT_MASK_SHIFT: u32 = 5;

    /// `INTERSECTION_MASK`.
    pub const INTERSECTION_MASK: u32 = Self::W | Self::R | Self::L | Self::B | Self::T;
    /// `MAX_LINES_PER_PATH`.
    pub const MAX_LINES_PER_PATH: u32 = 1 << (32 - Self::INT_MASK_SHIFT);

    /// `MaskToString(mask)`.
    // Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L61-L71 (chrome/m156)
    #[doc(alias = "MaskToString")]
    #[must_use]
    pub fn mask_to_string(mask: u32) -> String {
        if mask == 0 {
            return "empty".to_string();
        }

        let mut s = String::new();
        if mask & Self::W != 0 {
            s += "W";
        }
        if mask & Self::T != 0 {
            s += if s.is_empty() { "T" } else { " | T" };
        }
        if mask & Self::B != 0 {
            s += if s.is_empty() { "B" } else { " | B" };
        }
        if mask & Self::L != 0 {
            s += if s.is_empty() { "L" } else { " | L" };
        }
        if mask & Self::R != 0 {
            s += if s.is_empty() { "R" } else { " | R" };
        }
        s
    }
}

/// `MsaaExactMaskObserver`: called with the exact sample mask and the subsample windings of each
/// resolved pixel (`GPU_TEST_UTILS` only in the C++; always available here).
// Port of: src/gpu/graphite/sparse_strips/SparseStripsTypes.h#L74-L76 (chrome/m156)
pub type MsaaExactMaskObserver<'a> = &'a mut dyn FnMut(u8, Int8);
