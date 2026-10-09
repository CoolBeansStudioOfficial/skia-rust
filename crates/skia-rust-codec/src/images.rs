// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/codec/SkImageGenerator_FromEncoded.cpp (the SkImages namespace part)

//! `SkImages::DeferredFromEncodedData`: lazy images of encoded data.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::images::deferred_from_generator;

use crate::image_generator_from_encoded::make_from_encoded;

/// A lazy image whose pixels are decoded from `encoded` when they are needed, with the alpha
/// type `alpha_type` if given (`SkImages::DeferredFromEncodedData`). Returns `None` if the data
/// is missing or empty, or no codec recognises it.
// Port of: src/codec/SkImageGenerator_FromEncoded.cpp#L58-L66 (chrome/m156)
#[doc(alias = "DeferredFromEncodedData")]
#[must_use]
pub fn deferred_from_encoded_data(
    encoded: Option<Data>,
    alpha_type: Option<AlphaType>,
) -> Option<Image> {
    match &encoded {
        None => return None,
        Some(data) if data.is_empty() => return None,
        Some(_) => {}
    }
    deferred_from_generator(make_from_encoded(encoded, alpha_type))
}
