// Copyright (C) 1991-2025, Thomas G. Lane, Guido Vollbeding and the libjpeg-turbo Project.
// Copyright (C) 2025 The skia-rust Authors.
// Use of this source code is governed by the IJG, BSD-3-Clause and zlib licences in the LICENSE file.
//
//! A port of the 8-bit decompressor of libjpeg-turbo 3.1.0 (`libjpeg_turbo@e14cbfaa`), the
//! subset that Skia's JPEG codec calls, used by skia-rust's codecs.
//!
//! Skia decodes with `jpeg_read_header`, `jpeg_calc_output_dimensions` (with `scale_num` and
//! `scale_denom` for sampling), `jpeg_start_decompress`, `jpeg_read_scanlines`,
//! `jpeg_skip_scanlines`, `jpeg_crop_scanline`, and `jpeg_read_raw_data` for YUV planes. The
//! data comes through a [`JpegSource`]: Skia's memory and buffered streams suspend by returning
//! `false` from `fill_input_buffer`, and this crate reports that as
//! [`HeaderResult::Suspended`] / [`ConsumeResult::Suspended`].
//!
//! Output is the same as libjpeg-turbo's C code built for x64 without SIMD (the goldens' build).
//! The IDCT, upsampling and colour conversion are the C integer arithmetic, with `JLONG` as
//! 32 bits. The differential harness in `oracle/codec-diff/libjpeg` (built from the pinned commit
//! by its `build.sh`) checks the whole-image output, the `jpeg_crop_scanline` and
//! `jpeg_skip_scanlines` results, and the inverse DCTs against the C library, byte for byte.
//!
//! Ported: sequential and progressive Huffman input, the 1/8 .. 8/8 scaled decode (IDCTs 1..16,
//! so chroma can be scaled through the IDCT), fancy upsampling, the colour conversions including
//! RGB565 with `JDITHER_NONE`, the buffered-image API (`jpeg_start_output`, `jpeg_consume_input`,
//! `jpeg_finish_output`), block smoothing, `jpeg_crop_scanline` ([`Decompress::crop_scanline`])
//! and `jpeg_skip_scanlines` ([`Decompress::skip_scanlines`]), with the context-row state of the
//! main controller and the upsampler re-initialisation.
//!
//! Not ported: lossless and 12/16-bit precision, arithmetic coding (`jdarith.c`; no resource
//! uses it), merged upsampling (`jdmerge.c`, which Skia never selects because fancy upsampling is on),
//! colour quantization, RGB565 with ordered dithering, and the DCT methods IFAST and FLOAT.
//! Each case returns [`Error::NotImplemented`] or [`Error::ArithNotImplemented`] rather than
//! decoding differently.
//!
//! The crate is `unsafe`-free.

mod apistd;
mod coef;
mod coef_buf;
mod color;
mod decompress;
pub mod error;
mod huff;
mod idct;
#[cfg(test)]
mod idct_check;
mod input;
mod main_ctl;
mod marker;
mod master;
mod phuff;
mod source;
mod srcio;
mod tables;
mod upsample;

pub use decompress::{DctMethod, Decompress, DitherMode, HeaderResult};
pub use error::{Error, Result};
pub use marker::{ConsumeResult, SavedMarker};
pub use source::{JpegSource, SrcBuf};

// A decompressor owns its source and all of its state, so it can move between threads. The
// codec layer keeps one alive across calls, which requires `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<decompress::Decompress>();
};
pub use tables::{ColorSpace, CompInfo, JHuffTbl, JQuantTbl};
