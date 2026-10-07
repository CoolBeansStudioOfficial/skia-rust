// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMallocPixelRef.h, src/core/SkMallocPixelRef.cpp

//! `SkMallocPixelRef`: [`PixelRef`]s that allocate (or wrap) their own pixel memory.
//!
use crate::alpha_type::AlphaType;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::image_info::ImageInfo;
use crate::pixel_ref::PixelRef;

// Port of: src/core/SkMallocPixelRef.cpp#L18-L26 (chrome/m156)
fn is_valid(info: &ImageInfo) -> bool {
    // The C++ also rejects out-of-range enum values; a Rust enum cannot hold one.
    !(info.width() < 0
        || info.height() < 0
        || (info.color_type() as u32) > (ColorType::LAST_ENUM as u32)
        || (info.alpha_type() as u32) > (AlphaType::LAST_ENUM as u32))
}

/// Returns a new [`PixelRef`], automatically allocating storage for the pixels. If `row_bytes` is
/// 0, an optimal value will be chosen automatically. If `row_bytes` is > 0, then it will be
/// respected, or `None` will be returned if `row_bytes` is invalid for the specified `info`.
///
/// All pixel bytes are zeroed.
///
/// Returns `None` on failure.
// Port of: src/core/SkMallocPixelRef.cpp#L28-L55 (chrome/m156)
#[doc(alias = "SkMallocPixelRef::MakeAllocate")]
#[must_use]
pub fn make_allocate(info: &ImageInfo, mut row_bytes: usize) -> Option<PixelRef> {
    if row_bytes == 0 {
        row_bytes = info.min_row_bytes();
        // row_bytes can still be zero, if it overflowed (width * bytesPerPixel > size_t)
        // or if colortype is unknown
    }
    if !is_valid(info) || !info.valid_row_bytes(row_bytes) {
        return None;
    }
    let size = info.compute_byte_size(row_bytes);
    if ImageInfo::byte_size_overflowed(size) {
        return None;
    }
    // sk_calloc_canfail(size): a zeroed allocation that reports failure instead of aborting.
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(size).ok()?;
    pixels.resize(size, 0u8);

    Some(PixelRef::new(
        info.width(),
        info.height(),
        pixels,
        row_bytes,
    ))
}

/// Returns a new [`PixelRef`] that will use the provided [`Data`] and `row_bytes` as pixel
/// storage. The [`Data`] is shared, and released when the pixel ref is destroyed. The pixel ref
/// is immutable (the data is), and its pixels cannot be written to.
///
/// Returns `None` on failure.
// Port of: src/core/SkMallocPixelRef.cpp#L57-L81 (chrome/m156)
#[doc(alias = "SkMallocPixelRef::MakeWithData")]
#[must_use]
pub fn make_with_data(info: &ImageInfo, row_bytes: usize, data: Data) -> Option<PixelRef> {
    if !is_valid(info) {
        return None;
    }
    // TODO: what should we return if computeByteSize returns 0?
    // - the info was empty?
    // - we overflowed computing the size?
    if (row_bytes < info.min_row_bytes()) || (data.size() < info.compute_byte_size(row_bytes)) {
        return None;
    }
    let pr = PixelRef::with_data(info.width(), info.height(), row_bytes, data);
    pr.set_immutable(); // since we were created with (immutable) data
    Some(pr)
}
