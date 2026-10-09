// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/Rectanizer.h, src/gpu/RectanizerPow2.cpp (factory note),
// src/gpu/RectanizerSkyline.cpp (factory)

//! `skgpu::Rectanizer`: packs rectangles into a fixed-size atlas. Two implementations live in
//! [`crate::gpu::rectanizer_pow2`] and [`crate::gpu::rectanizer_skyline`]; [`factory`] returns the
//! one Skia uses.

use crate::gpu::rectanizer_skyline::RectanizerSkyline;

/// `SkIPoint16`: a point with 16-bit integer coordinates (`src/core/SkIPoint16.h`). Only used
/// for atlas locations, so it lives with the rectanizers.
// Port of: src/core/SkIPoint16.h#L14-L28 (chrome/m156)
#[doc(alias = "SkIPoint16")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IPoint16 {
    /// `fX`.
    pub x: i16,
    /// `fY`.
    pub y: i16,
}

impl IPoint16 {
    // Port of: src/core/SkIPoint16.h#L20-L22 (chrome/m156)
    /// `SkIPoint16::Make(x, y)`.
    #[must_use]
    pub const fn new(x: i16, y: i16) -> IPoint16 {
        IPoint16 { x, y }
    }

    // Port of: src/core/SkIPoint16.h#L24 (chrome/m156)
    /// `set(x, y)`.
    pub fn set(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }
}

/// Packs rectangles into a `width` x `height` area, tracking how full it is.
// Port of: src/gpu/Rectanizer.h#L15-L51 (chrome/m156)
#[doc(alias = "skgpu::Rectanizer")]
pub trait Rectanizer {
    /// Frees all rectangles; the area is empty again.
    // Port of: src/gpu/Rectanizer.h#L24 (chrome/m156)
    fn reset(&mut self);

    /// Width of the packed area.
    // Port of: src/gpu/Rectanizer.h#L26 (chrome/m156)
    fn width(&self) -> i32;

    /// Height of the packed area.
    // Port of: src/gpu/Rectanizer.h#L27 (chrome/m156)
    fn height(&self) -> i32;

    /// Attempts to add a rect. On success returns `Some(location)`, the position in the atlas;
    /// on failure returns `None`.
    // Port of: src/gpu/Rectanizer.h#L31 (chrome/m156)
    fn add_rect(&mut self, width: i32, height: i32) -> Option<IPoint16>;

    /// Fraction of the area that is in use.
    // Port of: src/gpu/Rectanizer.h#L32 (chrome/m156)
    fn percent_full(&self) -> f32;

    /// Adds a rect with `padding` on every side; the returned location is that of the rect
    /// itself, inside the padding.
    // Port of: src/gpu/Rectanizer.h#L34-L42 (chrome/m156)
    fn add_padded_rect(&mut self, width: i32, height: i32, padding: i16) -> Option<IPoint16> {
        let padding_i32 = i32::from(padding);
        let mut loc = self.add_rect(width + 2 * padding_i32, height + 2 * padding_i32)?;
        loc.x += padding;
        loc.y += padding;
        Some(loc)
    }
}

/// `Rectanizer::Factory(width, height)`: the rectanizer Skia uses (the skyline packer).
// Port of: src/gpu/RectanizerSkyline.cpp#L125-L127 (chrome/m156)
#[doc(alias = "skgpu::Rectanizer::Factory")]
#[must_use]
pub fn factory(width: i32, height: i32) -> Box<dyn Rectanizer> {
    Box::new(RectanizerSkyline::new(width, height))
}
