// Copyright 2011 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp's status codes (`webp/decode.h`, `VP8StatusCode`) and of the output side of
//! the decoder: `VP8Io` (`src/dec/vp8i_dec.h`) plus the external RGBA buffer of `WebPDecBuffer`.

use crate::lossless::CspMode;

/// Port of `VP8StatusCode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[doc(alias = "VP8StatusCode")]
pub enum Status {
    #[default]
    Ok,
    OutOfMemory,
    InvalidParam,
    BitstreamError,
    UnsupportedFeature,
    Suspended,
    NotEnoughData,
}

/// Port of `WebPIsRGBMode`.
#[must_use]
pub fn is_rgb_mode(mode: CspMode) -> bool {
    matches!(
        mode,
        CspMode::Rgb
            | CspMode::Rgba
            | CspMode::Bgr
            | CspMode::Bgra
            | CspMode::Argb
            | CspMode::Rgba4444
            | CspMode::Rgb565
            | CspMode::RgbA
            | CspMode::BgrA
            | CspMode::ArgbPremultiplied
            | CspMode::RgbA4444
    )
}

/// Port of `WebPIsAlphaMode`: the colour spaces with an alpha channel.
#[must_use]
pub fn is_alpha_mode(mode: CspMode) -> bool {
    matches!(
        mode,
        CspMode::Rgba
            | CspMode::Bgra
            | CspMode::Argb
            | CspMode::RgbA
            | CspMode::BgrA
            | CspMode::ArgbPremultiplied
            | CspMode::RgbA4444
    )
}

/// Port of `WebPIsPremultipliedMode`.
#[must_use]
pub fn is_premultiplied_mode(mode: CspMode) -> bool {
    matches!(
        mode,
        CspMode::RgbA | CspMode::BgrA | CspMode::ArgbPremultiplied | CspMode::RgbA4444
    )
}

/// Port of `VP8Io` together with the decode parameters that `WebPIoInitFromOptions` and the
/// output buffer provide. The output is the caller's memory (`WebPRGBABuffer`): `out` starts at
/// the first pixel of the image and rows are `out_stride` bytes apart.
// clippy::struct_excessive_bools: the flags are the C struct's `use_cropping`, `use_scaling`,
// `fancy_upsampling` and `bypass_filtering` fields of `VP8Io`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
#[doc(alias = "VP8Io")]
pub struct Io<'a> {
    pub width: i32,
    pub height: i32,
    /// First row of the current batch, relative to the crop window.
    pub mb_y: i32,
    pub mb_w: i32,
    pub mb_h: i32,
    pub crop_top: i32,
    pub crop_left: i32,
    pub crop_right: i32,
    pub crop_bottom: i32,
    pub use_cropping: bool,
    pub use_scaling: bool,
    pub scaled_width: i32,
    pub scaled_height: i32,
    /// `io->fancy_upsampling`: the fancy upsampler (off when scaling, as in `WebPIoInitFromOptions`).
    pub fancy_upsampling: bool,
    /// `io->bypass_filtering`: the loop filter is disabled (see `vp8_dec::decode_with_bypass`).
    pub bypass_filtering: bool,
    pub colorspace: CspMode,
    pub out: &'a mut [u8],
    pub out_stride: usize,
    /// `WebPDecParams::last_y`: rows written so far.
    pub last_y: i32,
}

impl<'a> Io<'a> {
    /// Creates the I/O for a `width` x `height` image written to `out`.
    #[must_use]
    pub fn new(
        out: &'a mut [u8],
        out_stride: usize,
        colorspace: CspMode,
        width: i32,
        height: i32,
    ) -> Self {
        Self {
            width,
            height,
            mb_y: 0,
            mb_w: 0,
            mb_h: 0,
            crop_top: 0,
            crop_left: 0,
            crop_right: width,
            crop_bottom: height,
            use_cropping: false,
            use_scaling: false,
            scaled_width: width,
            scaled_height: height,
            fancy_upsampling: true,
            bypass_filtering: false,
            colorspace,
            out,
            out_stride,
            last_y: 0,
        }
    }
}
