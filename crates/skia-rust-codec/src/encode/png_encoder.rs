// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/encode/SkPngEncoder.h and the public functions of src/encode/SkPngEncoderImpl.cpp
// (chrome/m156), with the skia-safe API shape (`png_encoder::{encode, encode_pixmap, Options,
// FilterFlag}`).
//
// Skia writes the PNG to an `SkWStream` as libpng produces it. This port collects the bytes and
// writes them to the `io::Write` when the encoding is complete. The bytes are the same, and so is
// the result: `false` if the encoding fails.

use std::io;
use std::sync::{Arc, Mutex};

use bitflags::bitflags;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::pixmap::Pixmap;

bitflags! {
    /// Port of `SkPngEncoder::FilterFlag` (SkPngEncoder.h#L31-L39): the filters the encoder may use.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FilterFlag: u32 {
        /// Port of `kZero`: no filter bits, which libpng treats as "use all filters".
        const ZERO = 0x00;
        /// Port of `kNone`.
        const NONE = 0x08;
        /// Port of `kSub`.
        const SUB = 0x10;
        /// Port of `kUp`.
        const UP = 0x20;
        /// Port of `kAvg`.
        const AVG = 0x40;
        /// Port of `kPaeth`.
        const PAETH = 0x80;
        /// Port of `kAll`.
        const ALL = Self::NONE.bits() | Self::SUB.bits() | Self::UP.bits() | Self::AVG.bits() | Self::PAETH.bits();
    }
}

/// A keyword and text pair written as a `tEXt` chunk (`SkPngEncoder::Options::fComments`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The keyword: at most 79 bytes are written.
    pub keyword: String,
    /// The text.
    pub text: String,
}

/// PNG encoding options (`SkPngEncoder::Options`, SkPngEncoder.h#L51-L76).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// Which filters to use. A single filter is used on every row. Several filters let libpng
    /// pick the smallest per row with its heuristic. Port of `fFilterFlags`, default `ALL`.
    pub filter_flags: FilterFlag,
    /// The zlib level, `0..=9` (clamped). Port of `fZLibLevel`, default 6.
    pub z_lib_level: i32,
    /// The text comments. Port of `fComments`.
    pub comments: Vec<Comment>,
}

impl Default for Options {
    /// Port of the defaults of `SkPngEncoder::Options` (SkPngEncoder.h#L58-L74).
    fn default() -> Self {
        Self {
            filter_flags: FilterFlag::ALL,
            z_lib_level: 6,
            comments: Vec::new(),
        }
    }
}

/// Port of `SkPngEncoder::Encode` (SkPngEncoderImpl.cpp): the PNG bytes of `src`, or `None` if
/// the pixmap is not supported or the encoding fails.
#[doc(alias = "SkPngEncoder::Encode")]
fn encode_to_vec(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    // The encoder reads the same pixels through its own read-only view of the pixmap.
    let view = Pixmap::new_readonly(src.info(), src.addr()?, src.row_bytes())?;
    let out = Arc::new(Mutex::new(Vec::new()));
    let height = src.height();
    let mut encoder = make(Arc::clone(&out), view, options)?;
    if !encoder.encode_rows(height) {
        return None;
    }
    drop(encoder);
    let bytes = out.lock().ok()?.clone();
    Some(bytes)
}

/// Encodes the pixels of `pixmap` as a PNG into `writer`. Returns `true` on success, and `false`
/// for an invalid or unsupported pixmap, or a failed write.
///
/// Port of `SkPngEncoder::Encode(SkWStream*, const SkPixmap&, const Options&)`.
#[doc(alias = "SkPngEncoder::Encode")]
pub fn encode<W: io::Write>(pixmap: &Pixmap<'_>, writer: &mut W, options: &Options) -> bool {
    match encode_to_vec(pixmap, options) {
        Some(bytes) => writer.write_all(&bytes).is_ok(),
        None => false,
    }
}

/// Returns the PNG bytes of `src`, or `None` if the encoding fails.
///
/// Port of `SkPngEncoder::Encode(const SkPixmap&, const Options&)`.
#[doc(alias = "SkPngEncoder::Encode")]
#[must_use]
pub fn encode_pixmap(src: &Pixmap<'_>, options: &Options) -> Option<Data> {
    encode_to_vec(src, options).map(|bytes| Data::new_copy(&bytes))
}

/// Returns the PNG bytes of an image, or `None` if its pixels cannot be read or the encoding fails.
///
/// Port of `SkPngEncoder::Encode(GrDirectContext*, const SkImage*, const Options&)` for raster
/// images: the pixels come from the image's legacy bitmap (`getROPixels`), with no GPU context.
// Port of: src/encode/SkPngEncoderImpl.cpp#L506-L518 (chrome/m156)
#[doc(alias = "SkPngEncoder::Encode")]
#[must_use]
pub fn encode_image(img: &Image, options: &Options) -> Option<Data> {
    let bitmap = img.as_legacy_bitmap()?;
    let pixmap = bitmap.peek_pixels()?;
    encode_pixmap(&pixmap, options)
}

/// The streaming PNG encoder of `SkPngEncoder::Make`: an `SkEncoder` that takes the rows of its
/// source through [`PngEncoder::encode_rows`].
pub struct PngEncoder<'a>(crate::encode::png_encoder_impl::PngEncoderImpl<'a>);

impl std::fmt::Debug for PngEncoder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngEncoder").finish_non_exhaustive()
    }
}

impl PngEncoder<'_> {
    /// Port of `SkEncoder::encodeRows`: encodes up to `num_rows` further rows, and finishes the
    /// file after the last row.
    #[doc(alias = "encodeRows")]
    pub fn encode_rows(&mut self, num_rows: i32) -> bool {
        self.0.encode_rows(num_rows)
    }
}

/// Port of `SkPngEncoder::Make` (SkPngEncoderImpl.cpp#L462-L490): the streaming encoder for `src`,
/// with the file going to `out`.
// Port of: src/encode/SkPngEncoderImpl.cpp#L462-L490 (chrome/m156)
#[doc(alias = "SkPngEncoder::Make")]
#[must_use]
pub fn make<'a>(
    out: Arc<Mutex<Vec<u8>>>,
    src: Pixmap<'a>,
    options: &Options,
) -> Option<PngEncoder<'a>> {
    crate::encode::png_encoder_impl::make(out, src, options).map(PngEncoder)
}
