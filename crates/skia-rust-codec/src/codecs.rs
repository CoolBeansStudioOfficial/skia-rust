// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkCodec.cpp#L86-L169 (the default decoder list, and the registry functions
// Register, HasDecoder and get_decoders; chrome/m156)
// Ported from: src/codec/SkCodec.cpp (SkCodecs::get_decoders, Register, HasDecoder)

//! The decoders, in the order `SkCodec::MakeFromStream` tries them.
//!
//! Only the decoders ported so far are listed: PNG, ICO, BMP and WBMP. The rest of Skia's default
//! list (JPEG, WebP, GIF, and so on) joins as each decoder lands. The list is a process-wide
//! registry, as Skia's is: [`register`] replaces a decoder with the same id or appends it. Codecs
//! that Skia makes through `SkCodec::MakeFromStream` read the registry ([`make_codec_from_stream`]),
//! and the ICO decoder asks it for its PNG decoder.

use std::sync::{LazyLock, RwLock};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::image::Image;
use skia_rust_core::images::deferred_from_generator;
use skia_rust_core::stream::Stream;

use crate::bmp::{is_bmp, make_from_stream as make_bmp_from_stream};
use crate::codec::{Codec, Result};
use crate::codec_image_generator::CodecImageGenerator;
use crate::ico_codec::{self, is_ico};
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

static DECODERS: [Decoder; 4] = [
    Decoder {
        id: "png",
        is_format: is_png_format,
        make_from_stream: png_codec::make_from_stream,
    },
    Decoder {
        id: "ico",
        is_format: is_ico,
        make_from_stream: ico_codec::make_from_stream,
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

// The process-wide registry. Port of `get_decoders_for_editing()`: a list that starts as the
// default decoders and is changed only by `register`, `set_registered` and the test helpers.
static REGISTRY: LazyLock<RwLock<Vec<Decoder>>> = LazyLock::new(|| RwLock::new(DECODERS.to_vec()));

/// The default decoder list. Port of `SkCodecs::get_decoders()` before any registration.
#[must_use]
pub fn decoders() -> &'static [Decoder] {
    &DECODERS
}

/// The decoders as they are registered now. Port of `SkCodecs::get_decoders()`: a snapshot, so a
/// decode that uses it is not affected by a later registration.
#[must_use]
#[doc(alias = "SkCodecs::get_decoders")]
pub fn registered() -> Vec<Decoder> {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Port of `SkCodecs::Register`: replaces the decoder with the same id, or appends the decoder if
/// no decoder has that id.
// Port of: src/codec/SkCodec.cpp#L149-L158 (chrome/m156)
#[doc(alias = "SkCodecs::Register")]
pub fn register(decoder: Decoder) {
    let mut decoders = REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(existing) = decoders.iter_mut().find(|d| d.id == decoder.id) {
        *existing = decoder;
    } else {
        decoders.push(decoder);
    }
}

/// Replaces the whole registry. Used to restore a saved list, and to clear it, as Skia's
/// `ScopedCodecDecoders` does through `get_decoders()`.
pub fn set_registered(list: Vec<Decoder>) {
    *REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = list;
}

/// Port of `SkCodecs::HasDecoder`: whether a decoder with this name is registered.
// Port of: src/codec/SkCodec.cpp#L160-L166 (chrome/m156)
#[must_use]
pub fn has_decoder(id: &str) -> bool {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .any(|decoder| decoder.id == id)
}

/// Port of `SkCodec::MakeFromStream(stream)` with no result: makes a codec with the registered
/// decoders.
///
/// # Errors
/// As [`Codec::make_from_stream`].
// Port of: src/codec/SkCodec.cpp#L177-L250 (chrome/m156), with the registry as the decoder list
#[doc(alias = "SkCodec::MakeFromStream")]
pub fn make_codec_from_stream<'a>(
    stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    Codec::make_from_stream(stream, &registered())
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
