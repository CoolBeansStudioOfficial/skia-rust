// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkCodec.h, src/codec/ (chrome/m156), the first wave of
// docs/design/codecs.md (C1, C2 partial, C3 WBMP).

//! Skia's codec layer: the [`Codec`] base, its decoders, and the helpers they share.
//!
//! What is ported so far: the base (`SkCodec`) for still images, scanline decoding, the sampler
//! (`SkSampler`, its fill and sample-Y state), and the swizzler (`SkSwizzler`, every row routine
//! and `Make`/`MakeSimple`/`setSampleX`), the mask swizzler, and the PNG, BMP and WBMP decoders.
//! The ICO and JPEG decoders, the Android and sampled codecs, and the animation, incremental and
//! lazy-image parts follow in later waves (`docs/design/codecs.md`).

pub mod bmp;
pub mod codec;
mod codec_priv;
pub mod codecs;
pub mod encoded_info;
mod mask_swizzler;
mod masks;
pub mod png_codec;
mod png_codec_base;
pub mod png_composite_chunk_reader;
pub mod sampler;
pub mod swizzler;
pub mod wbmp;

pub use codec::{
    Codec, NO_FRAME, Options, Result, ScanlineOrder, SelectionPolicy, ZeroInitialized,
};
pub use codecs::{Decoder, decoders};
pub use encoded_info::{Alpha, Color, EncodedInfo};
