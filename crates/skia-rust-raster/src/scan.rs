// Copyright 2011 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan.h

//! The scan converters' shared declarations (`SkScan.h`).
//!
//! skia-rust: `class SkScan` is a namespace of static functions; each one is a free function in
//! the module of the C++ file that defines it (`scan_anti_path::anti_fill_path`,
//! `scan_antihair::anti_fill_rect`, ...). This module keeps the header's `SkXRect` helpers.

use skia_rust_core::fixed::{
    fixed_ceil_to_int, fixed_floor_to_int, fixed_round_to_int, int_to_fixed, scalar_to_fixed,
};
use skia_rust_core::rect::{IRect, Rect};

/// A fixed-point rectangle: identical to [`IRect`], but its coordinates are treated as 16.16
/// [`Fixed`](skia_rust_core::fixed::Fixed) rather than `i32`.
// Port of: src/core/SkScan.h#L22-L25 (chrome/m156)
#[doc(alias = "SkXRect")]
pub type XRect = IRect;

/// Assigns an [`XRect`] from an [`IRect`] by promoting the coordinates from `i32` to [`Fixed`](skia_rust_core::fixed::Fixed).
/// Does not check for overflow if the coordinates exceed 32K.
// Port of: src/core/SkScan.h#L94-L103 (chrome/m156)
#[doc(alias = "XRect_set")]
#[must_use]
pub fn xrect_from_irect(src: &IRect) -> XRect {
    XRect {
        left: int_to_fixed(src.left),
        top: int_to_fixed(src.top),
        right: int_to_fixed(src.right),
        bottom: int_to_fixed(src.bottom),
    }
}

/// Assigns an [`XRect`] from a [`Rect`] by converting the coordinates to [`Fixed`](skia_rust_core::fixed::Fixed). Does not
/// check for overflow if the coordinates exceed 32K.
// Port of: src/core/SkScan.h#L105-L114 (chrome/m156)
#[doc(alias = "XRect_set")]
#[must_use]
pub fn xrect_from_rect(src: &Rect) -> XRect {
    XRect {
        left: scalar_to_fixed(src.left),
        top: scalar_to_fixed(src.top),
        right: scalar_to_fixed(src.right),
        bottom: scalar_to_fixed(src.bottom),
    }
}

/// Rounds the [`XRect`] coordinates.
// Port of: src/core/SkScan.h#L116-L123 (chrome/m156)
#[doc(alias = "XRect_round")]
#[must_use]
pub fn xrect_round(xr: &XRect) -> IRect {
    IRect {
        left: fixed_round_to_int(xr.left),
        top: fixed_round_to_int(xr.top),
        right: fixed_round_to_int(xr.right),
        bottom: fixed_round_to_int(xr.bottom),
    }
}

/// Rounds the [`XRect`] coordinates out (floor for left/top, ceiling for right/bottom).
// Port of: src/core/SkScan.h#L125-L133 (chrome/m156)
#[doc(alias = "XRect_roundOut")]
#[must_use]
pub fn xrect_round_out(xr: &XRect) -> IRect {
    IRect {
        left: fixed_floor_to_int(xr.left),
        top: fixed_floor_to_int(xr.top),
        right: fixed_ceil_to_int(xr.right),
        bottom: fixed_ceil_to_int(xr.bottom),
    }
}
