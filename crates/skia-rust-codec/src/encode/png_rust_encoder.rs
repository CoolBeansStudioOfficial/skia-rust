// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkPngRustEncoder.cpp, src/encode/SkPngRustEncoderImpl.{h,cpp},
// include/encode/SkPngRustEncoder.h (chrome/m156). The `png` crate writes the PNG; the FFI layer
// of Skia (`rust/png/FFI.rs`) is the Rust code in `png_rust_codec`'s sibling `png_rust_ffi`.

//! `SkPngRustEncoder`: the PNG encoder built on the `png` crate. Shares the row conversion of the
//! libpng encoder (`SkPngEncoderBase`), and adds the byte-level row transforms the `png` writer
//! needs for opaque and 16-bit rows.

use std::io;
use std::sync::{Arc, Mutex};

use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;

use crate::encode::icc::write_icc_profile;
use crate::encode::png_encoder_base::{PngEncoderBase, PngRowSink, TargetInfo, get_target_info};
use crate::encoded_info::Color;

/// Port of `SkPngRustEncoder::CompressionLevel`.
// Port of: include/encode/SkPngRustEncoder.h (CompressionLevel, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionLevel {
    /// Port of `kLow`: `Level1WithUpFilter`, the M136 behaviour kept for compatibility.
    Low,
    /// Port of `kMedium`, the default.
    #[default]
    Medium,
    /// Port of `kHigh`.
    High,
}

/// Port of `SkPngRustEncoder::Options`.
// Port of: include/encode/SkPngRustEncoder.h (Options, chrome/m156), without the gainmap and
// HDR fields, which the non-gainmap encoder path does not write.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Options {
    /// Port of `fCompressionLevel`.
    pub compression_level: CompressionLevel,
    /// Port of `fComments`: the (keyword, text) pairs of the `tEXt` chunks. Skia's `SkDataTable`
    /// holds the same pairs as alternating entries, and an odd count is an error there.
    pub comments: Option<Vec<(Vec<u8>, Vec<u8>)>>,
}

/// Port of `ExtraRowTransform`: the byte-level conversions a row needs after the base conversion.
// Port of: src/encode/SkPngRustEncoderImpl.h (ExtraRowTransform, chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraRowTransform {
    None,
    /// `kRgba8ToRgb8_ExtraRowTransform`: drops the alpha of 8-bit RGBA.
    Rgba8ToRgb8,
    /// `kRgba16leToRgba16be_ExtraRowTransform`: byte-swaps 16-bit RGBA.
    Rgba16leToRgba16be,
    /// `kRgba16leToRgb16be_ExtraRowTransform`: byte-swaps 16-bit RGB, dropping the alpha.
    Rgba16leToRgb16be,
}

/// The `png` writer's output, shared with the caller. `WriteTraitAdapterForSkWStream` in Skia.
// Port of: src/encode/SkPngRustEncoder.cpp (WriteTraitAdapterForSkWStream, chrome/m156)
#[derive(Clone)]
struct SharedOutput(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("output poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The `png` stream writer and the row transform: the sink of `SkPngRustEncoderImpl`.
// Port of: src/encode/SkPngRustEncoderImpl.h (fStreamWriter, fExtraRowTransform, fExtraRowBuffer)
struct RustSink {
    stream_writer: Option<png::StreamWriter<'static, SharedOutput>>,
    extra_row_transform: ExtraRowTransform,
    extra_row_buffer: Vec<u8>,
    /// Port of `this->targetInfo().fDstRowInfo->minRowBytes()`.
    src_row_bytes: usize,
    /// Port of `this->targetInfo().fDstRowInfo->width()`.
    width: usize,
}

impl RustSink {
    // Port of: src/encode/SkPngRustEncoderImpl.cpp#L330-L380 (the extra row transform, chrome/m156)
    fn transform_row(&mut self, row: &[u8]) -> bool {
        let src_row_bytes = self.src_row_bytes;
        let width = self.width;
        let (dst_row_bytes, convert): (usize, fn(&[u8], &mut [u8], usize)) =
            match self.extra_row_transform {
                ExtraRowTransform::None => return true,
                ExtraRowTransform::Rgba8ToRgb8 => (
                    src_row_bytes - (src_row_bytes / 4),
                    rgba8_to_rgb8,
                ),
                ExtraRowTransform::Rgba16leToRgba16be => {
                    (src_row_bytes, rgba16le_to_rgba16be)
                }
                ExtraRowTransform::Rgba16leToRgb16be => (
                    src_row_bytes - (src_row_bytes / 4),
                    rgba16le_to_rgb16be,
                ),
            };
        self.extra_row_buffer.clear();
        self.extra_row_buffer.resize(dst_row_bytes, 0x00);
        convert(row, &mut self.extra_row_buffer, width);
        true
    }
}

/// Drops the alpha of `width` 8-bit RGBA pixels. A byte copy: skcms does no arithmetic here.
fn rgba8_to_rgb8(src: &[u8], dst: &mut [u8], width: usize) {
    for x in 0..width {
        dst[3 * x..3 * x + 3].copy_from_slice(&src[4 * x..4 * x + 3]);
    }
}

/// Converts `width` little-endian 16-bit RGBA pixels to big-endian, keeping the alpha.
fn rgba16le_to_rgba16be(src: &[u8], dst: &mut [u8], width: usize) {
    for i in 0..4 * width {
        dst[2 * i] = src[2 * i + 1];
        dst[2 * i + 1] = src[2 * i];
    }
}

/// Converts `width` little-endian 16-bit RGBA pixels to big-endian RGB, dropping the alpha.
fn rgba16le_to_rgb16be(src: &[u8], dst: &mut [u8], width: usize) {
    for x in 0..width {
        for c in 0..3 {
            let i = 4 * x + c;
            dst[6 * x + 2 * c] = src[2 * i + 1];
            dst[6 * x + 2 * c + 1] = src[2 * i];
        }
    }
}

// Port of: src/encode/SkPngRustEncoderImpl.cpp#L120-L125 (getDataTableEntry, chrome/m156): the
// entry without its trailing zero bytes.
fn trim_trailing_zeros(entry: &[u8]) -> &[u8] {
    let mut len = entry.len();
    while len > 0 && entry[len - 1] == 0 {
        len -= 1;
    }
    &entry[..len]
}

// Port of: src/encode/SkPngRustEncoderImpl.cpp#L127-L146 (EncodeComments, chrome/m156), with the
// `png` crate's `write_text_chunk`, which checks the Latin-1 bytes the same way.
fn encode_comments(
    writer: &mut png::Writer<SharedOutput>,
    comments: Option<&[(Vec<u8>, Vec<u8>)]>,
) -> bool {
    let Some(comments) = comments else {
        return true;
    };
    for (keyword, text) in comments {
        if !write_text_chunk(writer, trim_trailing_zeros(keyword), trim_trailing_zeros(text)) {
            return false;
        }
    }
    true
}

// Port of: src/encode/SkPngRustEncoderImpl.cpp (FFI `Writer::write_text_chunk`, chrome/m156), in
// rust/png/FFI.rs#L1074-L1106: Latin-1 text and keyword only, with no non-breaking space in the
// keyword.
fn write_text_chunk(writer: &mut png::Writer<SharedOutput>, keyword: &[u8], text: &[u8]) -> bool {
    let is_latin1_byte = |b: &u8| (0x20..=0x7E).contains(b) || (0xA0..=0xFF).contains(b);
    let is_linefeed_byte = |b: &u8| *b == 10;
    if !text.iter().all(|b| is_latin1_byte(b) || is_linefeed_byte(b)) {
        return false;
    }
    if !keyword.iter().all(|b| is_latin1_byte(b) && *b != 0xA0) {
        return false;
    }
    let latin1 = |bytes: &[u8]| bytes.iter().map(|&b| char::from(b)).collect::<String>();
    let chunk = png::text_metadata::TEXtChunk {
        keyword: latin1(keyword),
        text: latin1(text),
    };
    writer.write_text_chunk(&chunk).is_ok()
}

// Port of: src/encode/SkPngRustEncoderImpl.cpp#L152-L153 (WriteSbitChunk, chrome/m156): the
// `sBIT` chunk is not written by the non-android build.
fn write_sbit_chunk() -> bool {
    true
}

// Port of: src/encode/SkPngRustEncoderImpl.cpp#L154-L155 (WriteGainmapChunks, chrome/m156): the
// gainmap chunks are not written by the non-android build.
fn write_gainmap_chunks() -> bool {
    true
}

/// Port of `SkPngRustEncoderImpl::Make`'s choice of the `png` colour type and row transform.
// Port of: src/encode/SkPngRustEncoderImpl.cpp#L186-L236 (Make, chrome/m156)
fn choose_color(
    target: &TargetInfo,
) -> Option<(png::ColorType, ExtraRowTransform)> {
    let dst_info = &target.dst_info;
    let mut extra = ExtraRowTransform::None;
    let color = match dst_info.color() {
        Color::RGB => png::ColorType::Rgb,
        Color::RGBA => {
            let mut color = png::ColorType::Rgba;
            if let Some(dst_row_info) = target.dst_row_info.as_ref() {
                if dst_row_info.is_opaque() {
                    color = png::ColorType::Rgb;
                    if dst_row_info.color_type() == skia_rust_core::color_type::ColorType::R16G16B16A16UNorm {
                        extra = ExtraRowTransform::Rgba16leToRgb16be;
                    } else {
                        extra = ExtraRowTransform::Rgba8ToRgb8;
                    }
                } else if dst_row_info.color_type()
                    == skia_rust_core::color_type::ColorType::R16G16B16A16UNorm
                {
                    extra = ExtraRowTransform::Rgba16leToRgba16be;
                }
            }
            color
        }
        Color::Gray => png::ColorType::Grayscale,
        Color::GrayAlpha => png::ColorType::GrayscaleAlpha,
        _ => return None,
    };
    Some((color, extra))
}

/// Port of `ToCompression`: applies the level to the `png` encoder.
// Port of: src/encode/SkPngRustEncoderImpl.cpp#L30-L40 (ToCompression, chrome/m156), and the
// `Compression::apply` of rust/png/FFI.rs#L316-L330
fn apply_compression(encoder: &mut png::Encoder<'_, SharedOutput>, level: CompressionLevel) {
    match level {
        CompressionLevel::Low => {
            encoder.set_deflate_compression(png::DeflateCompression::Level(1));
            encoder.set_filter(png::Filter::Up);
        }
        CompressionLevel::Medium => encoder.set_compression(png::Compression::Balanced),
        CompressionLevel::High => encoder.set_compression(png::Compression::High),
    }
}

/// The encoder for one image. Port of `SkPngRustEncoderImpl`.
// Port of: src/encode/SkPngRustEncoderImpl.h#L24-L60 (chrome/m156)
#[doc(alias = "SkPngRustEncoderImpl")]
struct PngRustEncoder<'a> {
    base: PngEncoderBase<'a>,
    sink: RustSink,
}

impl PngRustEncoder<'_> {
    /// Port of `SkEncoder::encodeRows` for the rows of this encoder.
    fn encode_rows(&mut self, num_rows: i32) -> bool {
        self.base.encode_rows(num_rows, &mut self.sink)
    }
}

impl PngRowSink for RustSink {
    // Port of: src/encode/SkPngRustEncoderImpl.cpp#L394-L421 (onEncodeRow, chrome/m156)
    fn on_encode_row(&mut self, row: &[u8]) -> bool {
        if !self.transform_row(row) {
            return false;
        }
        let data: &[u8] = if self.extra_row_transform == ExtraRowTransform::None {
            row
        } else {
            &self.extra_row_buffer
        };
        match self.stream_writer.as_mut() {
            Some(writer) => io::Write::write(writer, data).is_ok(),
            None => false,
        }
    }

    // Port of: src/encode/SkPngRustEncoderImpl.cpp (onFinishEncoding, chrome/m156)
    fn on_finish_encoding(&mut self) -> bool {
        match self.stream_writer.take() {
            Some(writer) => writer.finish().is_ok(),
            None => false,
        }
    }
}

/// Port of `SkPngRustEncoderImpl::Make`: writes the PNG header, the comments, and the stream
/// writer, and returns the encoder that writes the rows into `out`.
// Port of: src/encode/SkPngRustEncoderImpl.cpp#L166-L285 (Make, chrome/m156)
fn make<'a>(
    out: SharedOutput,
    src: Pixmap<'a>,
    options: &Options,
) -> Option<PngRustEncoder<'a>> {
    let target = get_target_info(src.info())?;
    let width = u32::try_from(target.dst_info.width()).ok()?;
    let height = u32::try_from(target.dst_info.height()).ok()?;

    let encoded_profile = src
        .info()
        .color_space()
        .filter(|cs| !cs.is_srgb())
        .and_then(|cs| write_icc_profile(Some(&cs)));

    let (color, extra_row_transform) = choose_color(&target)?;
    let bits = target.dst_info.bits_per_component();

    let mut info = png::Info::with_size(width, height);
    info.color_type = color;
    info.bit_depth = match bits {
        8 => png::BitDepth::Eight,
        16 => png::BitDepth::Sixteen,
        _ => return None,
    };
    if let Some(profile) = encoded_profile {
        info.icc_profile = Some(std::borrow::Cow::Owned(profile));
    }
    let mut encoder = png::Encoder::with_info(out.clone(), info).ok()?;
    apply_compression(&mut encoder, options.compression_level);
    let mut writer = encoder.write_header().ok()?;

    if !encode_comments(&mut writer, options.comments.as_deref()) {
        return None;
    }
    if !write_sbit_chunk() || !write_gainmap_chunks() {
        return None;
    }
    let stream_writer = writer.into_stream_writer().ok()?;

    let dst_row_info = target.dst_row_info.clone().unwrap_or_else(|| {
        ImageInfo::new_unknown(None)
    });
    let sink = RustSink {
        stream_writer: Some(stream_writer),
        extra_row_transform,
        extra_row_buffer: Vec::new(),
        src_row_bytes: dst_row_info.min_row_bytes(),
        width: usize::try_from(dst_row_info.width()).ok()?,
    };
    Some(PngRustEncoder {
        base: PngEncoderBase::new(target, src),
        sink,
    })
}

/// Port of `SkPngRustEncoder::Encode(SkWStream*, const SkPixmap&, const Options&)`: the PNG bytes
/// of `src`, or `None` if the encoding fails.
// Port of: src/encode/SkPngRustEncoder.cpp#L17-L21 (Encode, chrome/m156)
#[doc(alias = "SkPngRustEncoder::Encode")]
#[must_use]
pub fn encode_to_vec(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    let view = Pixmap::new_readonly(src.info(), src.addr()?, src.row_bytes())?;
    let out = SharedOutput(Arc::new(Mutex::new(Vec::new())));
    let height = src.height();
    let mut encoder = make(out.clone(), view, options)?;
    // The base finishes the PNG after its last row (`onFinishEncoding`), as `encodeRows` does.
    if !encoder.encode_rows(height) {
        return None;
    }
    let bytes = out.0.lock().ok()?.clone();
    Some(bytes)
}

/// Port of `SkPngRustEncoder::Encode(SkWStream*, const SkPixmap&, const Options&)`: writes the PNG
/// bytes of `src` to `dst`. Returns `false` if the encoding or the write fails.
// Port of: src/encode/SkPngRustEncoder.cpp#L17-L21 (Encode(SkWStream*), chrome/m156)
#[doc(alias = "SkPngRustEncoder::Encode")]
pub fn encode<W: io::Write>(dst: &mut W, src: &Pixmap<'_>, options: &Options) -> bool {
    match encode_to_vec(src, options) {
        Some(bytes) => dst.write_all(&bytes).is_ok(),
        None => false,
    }
}

/// Port of `SkPngRustEncoder::Encode(const SkPixmap&, const Options&)`: the PNG data of `src`.
// Port of: src/encode/SkPngRustEncoder.cpp#L23-L30 (Encode(SkPixmap), chrome/m156)
#[doc(alias = "SkPngRustEncoder::Encode")]
#[must_use]
pub fn encode_pixmap(src: &Pixmap<'_>, options: &Options) -> Option<Data> {
    encode_to_vec(src, options).map(|bytes| Data::new_copy(&bytes))
}
