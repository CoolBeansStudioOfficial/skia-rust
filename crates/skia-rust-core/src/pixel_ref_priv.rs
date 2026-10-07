// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPixelRefPriv.h, src/core/SkPixelRef.cpp (SkMakePixelRefWithProc)

//! Private helpers for [`PixelRef`] (`SkPixelRefPriv.h`).

use crate::pixel_ref::{PixelRef, ReleaseProc};

/// Returns a new [`PixelRef`] with the provided pixel storage and `row_bytes`. On destruction,
/// `release_proc` is called with the pixel bytes.
///
/// If `release_proc` is `None`, the pixels are simply dropped with the pixel ref.
///
/// skia-rust: the C++ takes a raw `addr` that stays owned by the caller (so that, with a null
/// `releaseProc`, stack storage could be used). Here the pixel ref takes ownership of `pixels`
/// and `release_proc` receives them back; the context pointer is whatever the closure captures.
// Port of: src/core/SkPixelRef.cpp#L138-L153 (chrome/m156)
#[doc(alias = "SkMakePixelRefWithProc")]
#[must_use]
pub fn make_pixel_ref_with_proc(
    width: i32,
    height: i32,
    row_bytes: usize,
    pixels: Vec<u8>,
    release_proc: Option<ReleaseProc>,
) -> PixelRef {
    debug_assert!(width >= 0 && height >= 0);
    PixelRef::with_release_proc(width, height, pixels, row_bytes, release_proc)
}
