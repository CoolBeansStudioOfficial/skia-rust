// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Copyright (C) 2025 The skia-rust Authors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
//
//! A port of the libpng 1.6.56 read path that Skia's PNG codec uses
//! (`skia.googlesource.com/third_party/libpng@d5515b5b`), used by skia-rust's codecs.
//!
//! Skia reads PNG files with libpng's progressive reader: it feeds bytes to
//! [`PngStruct::process_data`], gets header and row callbacks through [`ProgressiveHandler`], and
//! turns the libpng transforms on with the `set_*` functions before the header is read. Every
//! place where libpng would longjmp returns a [`PngError`] instead, and the chunk checks, the
//! zlib use and the transforms follow the C code. The differential harness in
//! `oracle/codec-diff` checks the output against the C library.
//!
//! The crate is `unsafe`-free and depends on `skia-rust-zlib` for the inflate stream.
//!
//! Not ported (not reached by Skia's read path, and without a public entry point here): the
//! sequential reader (`png_read_info`, `png_read_row`, `png_read_image`), the simplified API,
//! gamma and background compositing, RGB-to-gray, palette expansion, the alpha, filler and user
//! transforms, the write side, and the `sPLT`/`pCAL`/`sCAL` values (which are validated but not
//! kept).

mod ancillary;
mod chunks;
pub mod error;
mod filter;
mod get;
mod handle;
mod interlace;
mod png;
mod pread;
mod rtran;
mod rutil;
mod set;
mod structs;

pub use error::{PngError, PngResult};
pub use get::{
    get_chrm_fixed, get_gama_fixed, get_iccp, get_ihdr, get_plte, get_rowbytes, get_sbit, get_srgb,
    get_trns, get_valid,
};
pub use png::{
    PNG_HANDLE_CHUNK_ALWAYS, PNG_HANDLE_CHUNK_AS_DEFAULT, PNG_HANDLE_CHUNK_IF_SAFE,
    PNG_HANDLE_CHUNK_NEVER, PNG_MAXIMUM_INFLATE_WINDOW, PNG_OPTION_ON, ProgressiveHandler,
    UserChunkReader,
};
pub use set::{TextChunk, TextCompression};
pub use structs::{PngColor, PngColor8, PngColor16, PngInfo, PngStruct, UnknownChunk, info};

/// Port of `PNG_LIBPNG_VER_STRING` (png.h): the library version this port follows.
pub const PNG_LIBPNG_VER_STRING: &str = "1.6.56";

/// Port of `png_create_info_struct`: a new, empty header.
#[doc(alias = "png_create_info_struct")]
#[must_use]
pub fn create_info_struct() -> PngInfo {
    PngInfo::default()
}
