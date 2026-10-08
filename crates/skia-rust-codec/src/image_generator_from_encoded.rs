// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/codec/SkImageGenerator_FromEncoded.cpp

//! `SkImageGenerators::MakeFromEncoded`: the image generator for encoded data.
//!
//! skia-rust: `SkGraphics::SetImageGeneratorFromEncodedDataFactory` (the platform factory hook,
//! which only the Skia test `ImageGeneratorTest` sets, and only in a disabled branch) is not
//! ported, so the codec generator is always used.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::data::Data;
use skia_rust_core::image_generator::ImageGenerator;

use crate::codec_image_generator::CodecImageGenerator;

/// An image generator for the encoded `data`, decoded with the codec the data is recognised by
/// (`SkImageGenerators::MakeFromEncoded`). `alpha_type` overrides the alpha type of the
/// generator's pixels; `Opaque` is refused. Returns `None` if the data is missing, the alpha
/// type is `Opaque`, or no codec recognises the data.
// Port of: src/codec/SkImageGenerator_FromEncoded.cpp#L27-L37 (chrome/m156)
#[doc(alias = "MakeFromEncoded")]
#[must_use]
pub fn make_from_encoded(
    data: Option<Data>,
    alpha_type: Option<AlphaType>,
) -> Option<Box<dyn ImageGenerator>> {
    let data = data?;
    if alpha_type == Some(AlphaType::Opaque) {
        return None;
    }
    CodecImageGenerator::make_from_encoded_codec(data, alpha_type)
}
