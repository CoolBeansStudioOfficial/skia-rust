// Copyright 2010 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).
//
//! A port of the libwebp 1.4.0 decoder (`chromium.googlesource.com/webm/libwebp@845d5476`) that
//! Skia's `SkWebpCodec` uses, used by skia-rust's codecs.
//!
//! Skia calls `WebPGetFeatures`, the demux API, `WebPIDecode`/`WebPIUpdate` with a
//! `WebPDecoderConfig` (cropping, scaling, `use_threads = 0`, RGBA/BGRA premultiplied or not, and
//! RGB565 with `WEBP_SWAP_16BIT_CSP`). The port covers the decoder paths those calls reach:
//!
//! - [`vp8l`]: the lossless bitstream (Huffman codes, transforms, colour cache), and the
//!   lossless-coded `ALPH` plane.
//! - [`alpha`]: the `ALPH` chunk (raw or lossless) and its spatial unfilters.
//! - [`lossless`]: the predictor, colour and colour-index transforms and the output conversions.
//!
//! The lossy VP8 decoder, the incremental decoder and the demuxer are being ported in the same
//! way; see the design note `docs/design/codecs.md` §2 and §3.
//!
//! The crate is `unsafe`-free and depends on nothing but `std`. Where libwebp has SSE2/SSE4.1
//! kernels, the port follows the C path; the differential harness in `oracle/codec-diff/libwebp`
//! checks it against the portable C build of the pinned sources (no SIMD).

pub mod alpha;
pub mod alpha_processing;
mod bit_reader;
pub mod huffman;
pub mod io;
pub mod lossless;
mod output;
pub mod vp8_dec;
mod vp8_dsp;
mod vp8_tables;
mod vp8_tables_small;
pub mod vp8l;
pub mod webp_dec;

pub use io::Status;
pub use lossless::CspMode;
pub use webp_dec::{Features, decode as decode_webp, get_features};

/// Port of `WebPGetFeatures` for a bare VP8L bitstream: `(width, height, has_alpha)`.
#[doc(alias = "WebPGetFeatures")]
#[must_use]
pub fn vp8l_get_info(data: &[u8]) -> Option<(i32, i32, bool)> {
    vp8l::get_info(data)
}

/// Decodes a bare VP8L bitstream (the bytes from a `VP8L` chunk payload to the end of the input)
/// into `out` in colour space `mode`, `out_stride` bytes per row. Returns the image size.
///
/// This is the one-shot path (`WebPDecode` for a lossless image). Incremental decoding uses the
/// same decoder, fed one update at a time.
///
/// # Errors
///
/// Returns the `Status` the C decoder would report: `BitstreamError` for a header or stream that
/// fails to parse, or the status recorded by the decoder.
#[doc(alias = "WebPDecode")]
pub fn decode_vp8l(
    data: &[u8],
    mode: CspMode,
    out: &mut [u8],
    out_stride: usize,
) -> Result<(i32, i32), Status> {
    let Some((width, height, _has_alpha)) = vp8l::get_info(data) else {
        return Err(Status::BitstreamError);
    };
    let mut io = io::Io::new(out, out_stride, mode, width, height);
    let mut dec = vp8l::Vp8lDecoder::new();
    if !vp8l::decode_header(&mut dec, data, &mut io) {
        return Err(if dec.status == Status::Ok {
            Status::BitstreamError
        } else {
            dec.status
        });
    }
    if !vp8l::decode_image(&mut dec, data, &mut io) {
        return Err(if dec.status == Status::Ok {
            Status::BitstreamError
        } else {
            dec.status
        });
    }
    Ok((width, height))
}
