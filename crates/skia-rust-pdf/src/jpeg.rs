// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/docs/SkPDFJpegHelpers.h (chrome/m156)

//! `SkPDF::JPEG`: the JPEG decoder and encoder callbacks that let the PDF backend embed a JPEG
//! as it is, and encode opaque images as JPEGs.

use skia_rust_codec::codec::Codec;
use skia_rust_codec::encode::jpeg_encoder::{self, Options};
use skia_rust_codec::jpeg_codec;
use skia_rust_core::data::Data;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::{MemoryStream, WStream};

use crate::metadata::Metadata;

/// `SkPDF::JPEG::Decode`: the codec for a JPEG, with the decoder of the codec crate.
// Port of: include/docs/SkPDFJpegHelpers.h#L22-L24 (chrome/m156)
#[doc(alias = "SkPDF::JPEG::Decode")]
#[must_use]
pub fn decode(data: Data) -> Option<Codec<'static>> {
    jpeg_codec::make_from_stream(Box::new(MemoryStream::from_data(Some(data)))).ok()
}

/// `SkPDF::JPEG::Encode`: writes `src` to `dst` as a JPEG of the given quality.
// Port of: include/docs/SkPDFJpegHelpers.h#L26-L30 (chrome/m156)
#[doc(alias = "SkPDF::JPEG::Encode")]
pub fn encode(dst: &mut dyn WStream, src: &Pixmap<'_>, quality: i32) -> bool {
    let options = Options {
        quality: u32::try_from(quality).unwrap_or(0),
        ..Options::default()
    };
    match jpeg_encoder::encode_pixmap(src, &options) {
        Some(data) => dst.write(data.as_bytes()),
        None => false,
    }
}

/// `SkPDF::JPEG::MetadataWithCallbacks`: the default metadata with the JPEG callbacks set.
// Port of: include/docs/SkPDFJpegHelpers.h#L32-L37 (chrome/m156)
#[doc(alias = "SkPDF::JPEG::MetadataWithCallbacks")]
#[must_use]
pub fn metadata_with_callbacks() -> Metadata {
    Metadata {
        jpeg_decoder: Some(decode),
        jpeg_encoder: Some(encode),
        ..Metadata::default()
    }
}
