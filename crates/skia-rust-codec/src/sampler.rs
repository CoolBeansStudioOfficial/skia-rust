// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkSampler.cpp#L20-L70 (chrome/m156), the Fill half of src/codec/SkSampler.h
// Ported from: src/codec/SkSampler.cpp, src/codec/SkSampler.h

//! Samplers write rows of the destination; this module holds the part the codec core uses: filling
//! rows that a truncated or failed decode never produced.

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::codec::ZeroInitialized;
use crate::codec_priv::get_start_coord;

/// The sample-Y state every sampler shares. Port of the `fSampleY` member of `SkSampler`.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "SkSampler")]
pub struct SamplerBase {
    sample_y: i32,
}

impl Default for SamplerBase {
    // Port of: src/codec/SkSampler.h (SkSampler constructor: fSampleY(1))
    fn default() -> Self {
        Self { sample_y: 1 }
    }
}

impl SamplerBase {
    /// Port of `SkSampler::setSampleY`.
    pub fn set_sample_y(&mut self, sample_y: i32) {
        self.sample_y = sample_y;
    }

    /// Port of `SkSampler::sampleY`.
    #[must_use]
    pub fn sample_y(&self) -> i32 {
        self.sample_y
    }

    /// Port of `SkSampler::rowNeeded`: whether `row` (counted from the first row of the subset)
    /// is one the sampler keeps.
    // Port of: src/codec/SkSampler.h (rowNeeded)
    #[must_use]
    pub fn row_needed(&self, row: i32) -> bool {
        (row - get_start_coord(self.sample_y)) % self.sample_y == 0
    }
}

/// A sampler: the part of a decoder that writes sampled rows. Port of `SkSampler`.
///
/// Every decoder that supports sampling implements this for its sampler object (the swizzler, or
/// the codec itself where Skia has a separate `SkBmpRLESampler`). `SkSampledCodec` drives it through
/// [`Sampler::set_sample_x`] and [`Sampler::set_sample_y`].
#[doc(alias = "SkSampler")]
pub trait Sampler {
    /// Port of `SkSampler::onSetSampleX`: updates the sampler to take every `sample_x`th pixel, and
    /// returns the width after sampling.
    fn on_set_sample_x(&mut self, sample_x: i32) -> i32;

    /// Port of `SkSampler::fillWidth`: the width of the rows this sampler writes.
    fn fill_width(&self) -> i32;

    /// The sample-Y state of this sampler (Skia's `fSampleY` in `SkSampler`).
    fn sampler_base(&self) -> &SamplerBase;

    /// The sample-Y state of this sampler, mutably.
    fn sampler_base_mut(&mut self) -> &mut SamplerBase;

    /// Port of `SkSampler::setSampleX`.
    fn set_sample_x(&mut self, sample_x: i32) -> i32 {
        self.on_set_sample_x(sample_x)
    }

    /// Port of `SkSampler::setSampleY`.
    fn set_sample_y(&mut self, sample_y: i32) {
        self.sampler_base_mut().set_sample_y(sample_y);
    }

    /// Port of `SkSampler::sampleY`.
    fn sample_y(&self) -> i32 {
        self.sampler_base().sample_y()
    }

    /// Port of `SkSampler::rowNeeded`.
    fn row_needed(&self, row: i32) -> bool {
        self.sampler_base().row_needed(row)
    }
}

/// Port of `SkSampler::Fill`: writes zeros over the rows of `info` at `dst` (`rowBytes` apart).
///
/// Skia zeroes only the colour types a decoder can produce incomplete images in (N32, 565,
/// Gray8 and RGBA F16); other colour types are left alone, as Skia's default case does.
#[doc(alias = "SkSampler::Fill")]
// Image dimensions are non-negative, so the casts to usize are exact.
#[allow(clippy::cast_sign_loss)]
pub fn fill(info: &ImageInfo, dst: &mut [u8], row_bytes: usize, zero_init: ZeroInitialized) {
    // Port of: src/codec/SkSampler.cpp#L24-L26
    if zero_init == ZeroInitialized::Yes {
        return;
    }
    let bytes_per_pixel = match info.color_type() {
        ColorType::RGBA8888 | ColorType::BGRA8888 => 4,
        ColorType::RGB565 => 2,
        ColorType::Gray8 => 1,
        ColorType::RGBAF16 => 8,
        _ => return,
    };
    let width = info.width() as usize;
    let row_len = width * bytes_per_pixel;
    for row in 0..info.height() as usize {
        let start = row * row_bytes;
        dst[start..start + row_len].fill(0);
    }
}
