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
//! 32 bits. The differential harness in `oracle/codec-diff` checks the output against the C
//! library, byte for byte.
//!
//! Not ported (not reached by Skia's codec, or not yet): lossless and 12/16-bit precision,
//! arithmetic coding (`jdarith.c`), progressive input and the buffered-image API (`jdcoefct.c`
//! multi-pass, `jdphuff.c`, block smoothing), merged upsampling (`jdmerge.c`), colour
//! quantization, the RGB565 output (`jdcol565.c`), the DCT methods IFAST and FLOAT, and the
//! 9..16 scaled IDCTs. Each case returns [`Error::NotImplemented`] or
//! [`Error::ArithNotImplemented`] rather than decoding differently.
//!
//! The crate is `unsafe`-free.

mod coef;
mod color;
mod decompress;
pub mod error;
mod huff;
#[cfg(test)]
mod idct_check;
mod idct;
mod input;
mod main_ctl;
mod marker;
mod master;
mod source;
mod srcio;
mod tables;
mod upsample;

pub use decompress::{DctMethod, Decompress, DitherMode, HeaderResult};
pub use error::{Error, Result};
pub use marker::{ConsumeResult, SavedMarker};
pub use source::{JpegSource, SrcBuf};
pub use tables::{ColorSpace, CompInfo, JHuffTbl, JQuantTbl};
