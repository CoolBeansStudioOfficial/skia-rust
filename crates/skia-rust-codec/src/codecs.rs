// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkCodec.cpp#L86-L169 (the default decoder list; chrome/m156)
// Ported from: src/codec/SkCodec.cpp (SkCodecs::get_decoders and the format checks)

//! The decoders, in the order `SkCodec::MakeFromStream` tries them.
//!
//! Only the decoders ported so far are listed: PNG, BMP and WBMP. The rest of Skia's default list
//! (JPEG, WebP, GIF, ICO, and so on) joins as each decoder lands. `SkCodecs::Register` is not ported
//! yet, so the list is fixed.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::image::Image;
use skia_rust_core::images::deferred_from_generator;
use skia_rust_core::stream::Stream;

use crate::bmp::{is_bmp, make_from_stream as make_bmp_from_stream};
use crate::codec::{Codec, Result};
use crate::codec_image_generator::CodecImageGenerator;
use crate::png_codec::{self, is_png_format};
use crate::wbmp::{WbmpCodec, is_wbmp};

/// One entry of the decoder list. Port of `SkCodecs::Decoder`.
#[derive(Clone, Copy)]
#[doc(alias = "SkCodecs::Decoder")]
pub struct Decoder {
    /// The decoder's name, as Skia's registry uses it (`"wbmp"`, `"png"`, ...).
    pub id: &'static str,
    /// Port of `isFormat`: whether the first bytes of a stream are this format.
    pub is_format: fn(&[u8]) -> bool,
    /// Port of `makeFromStream`: builds a codec over the stream, which has been rewound.
    pub make_from_stream:
        for<'a> fn(Box<dyn Stream + Send + 'a>) -> std::result::Result<Codec<'a>, Result>,
}

impl std::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decoder")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

static DECODERS: [Decoder; 3] = [
    Decoder {
        id: "png",
        is_format: is_png_format,
        make_from_stream: png_codec::make_from_stream,
    },
    Decoder {
        id: "bmp",
        is_format: is_bmp,
        make_from_stream: make_bmp_from_stream,
    },
    Decoder {
        id: "wbmp",
        is_format: is_wbmp,
        make_from_stream: WbmpCodec::make_from_stream,
    },
];

/// The default decoder list. Port of `SkCodecs::get_decoders()`.
#[must_use]
pub fn decoders() -> &'static [Decoder] {
    &DECODERS
}

/// A lazy image that decodes with `codec` when its pixels are needed, with the alpha type
/// `alpha_type` if given (`SkCodecs::DeferredImage`). Returns `None` if there is no codec.
// Port of: src/codec/SkImageGenerator_FromEncoded.cpp#L71-L76 (chrome/m156)
#[doc(alias = "DeferredImage")]
#[must_use]
pub fn deferred_image(
    codec: Option<Codec<'static>>,
    alpha_type: Option<AlphaType>,
) -> Option<Image> {
    deferred_from_generator(CodecImageGenerator::make_from_codec(codec, alpha_type))
}
