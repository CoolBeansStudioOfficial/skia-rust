// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkAndroidCodecAdapter.cpp and src/codec/SkAndroidCodecAdapter.h (chrome/m156)
// Ported from: src/codec/SkAndroidCodecAdapter.cpp, src/codec/SkAndroidCodecAdapter.h
//
// SkAndroidCodecAdapter is the AndroidCodec subclass for codecs that scale internally. The C++
// class holds only the codec; its methods are free functions here that take the codec.

//! Android decodes for codecs that do their own scaling: the codec is asked for the size directly.

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;

use crate::android_codec::AndroidOptions;
use crate::codec::{Codec, Result};
use crate::codec_priv::get_scale_from_sample_size;

/// Port of `SkAndroidCodecAdapter::onGetSampledDimensions`: the codec's own scaled size for the
/// sample size.
// Port of: src/codec/SkAndroidCodecAdapter.cpp#L11-L14 (chrome/m156)
#[must_use]
pub(crate) fn on_get_sampled_dimensions(codec: &Codec<'_>, sample_size: i32) -> ISize {
    let scale = get_scale_from_sample_size(sample_size);
    codec.get_scaled_dimensions(scale)
}

/// Port of `SkAndroidCodecAdapter::onGetAndroidPixels`: the codec decodes with the options. The
/// sample size is not passed on, since the codec scales itself.
// Port of: src/codec/SkAndroidCodecAdapter.cpp#L20-L23 (chrome/m156)
pub(crate) fn on_get_android_pixels(
    codec: &mut Codec<'_>,
    info: &ImageInfo,
    pixels: &mut [u8],
    row_bytes: usize,
    options: &AndroidOptions,
) -> Result {
    codec.get_pixels(info, pixels, row_bytes, Some(&options.base))
}
