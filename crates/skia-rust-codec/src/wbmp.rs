// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkWbmpCodec.cpp#L33-L240, src/codec/SkWbmpCodec.h (chrome/m156)
// Ported from: src/codec/SkWbmpCodec.cpp, src/codec/SkWbmpCodec.h, include/codecs/SkWbmpDecoder.h

//! The WBMP (wireless bitmap) decoder: 1-bit black-and-white images with a variable-length
//! header.

use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::codec::{Codec, CodecBase, CodecImpl, Options, Result};
use crate::codec_priv::valid_alpha;
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::sampler::Sampler;
use crate::swizzler::Swizzler;

// Port of: src/codec/SkWbmpCodec.cpp#L33-L35 (get_src_row_bytes: SkAlign8(width) >> 3)
#[allow(clippy::cast_sign_loss)] // widths are checked to be positive by the header reader
fn get_src_row_bytes(width: i32) -> usize {
    ((width as usize + 7) & !7) >> 3
}

// Port of: src/codec/SkWbmpCodec.cpp#L37-L49 (valid_color_type)
fn valid_color_type(dst: &ImageInfo) -> bool {
    match dst.color_type() {
        ColorType::RGBA8888 | ColorType::BGRA8888 | ColorType::Gray8 | ColorType::RGB565 => true,
        ColorType::RGBAF16 => dst.color_space().is_some(),
        _ => false,
    }
}

// Port of: src/codec/SkWbmpCodec.cpp#L51-L55 (read_byte)
fn read_byte(stream: &mut dyn Stream) -> Option<u8> {
    let mut data = [0u8; 1];
    (stream.read(&mut data) == 1).then_some(data[0])
}

// Port of: src/codec/SkWbmpCodec.cpp#L57-L73 (read_mbf: a multi-byte, big-endian, 7-bits-per-byte
// integer, with the top bit as a continuation flag)
fn read_mbf(stream: &mut dyn Stream) -> Option<u64> {
    const K_LIMIT: u64 = 0xFE00_0000_0000_0000;
    let mut n: u64 = 0;
    loop {
        // Will overflow on shift by 7.
        if n & K_LIMIT != 0 {
            return None;
        }
        let data = read_byte(stream)?;
        n = (n << 7) | u64::from(data & 0x7F);
        if data & 0x80 == 0 {
            return Some(n);
        }
    }
}

// Port of: src/codec/SkWbmpCodec.cpp#L75-L97 (read_header)
// The width and height are checked to be in 1..=0xFFFF before they are narrowed to i32.
#[allow(clippy::cast_possible_truncation)]
fn read_header(stream: &mut dyn Stream) -> Option<ISize> {
    // Unknown type, or a fixed header with bits set that this decoder does not know.
    if read_byte(stream)? != 0 {
        return None;
    }
    if read_byte(stream)? & 0x9F != 0 {
        return None;
    }
    let width = read_mbf(stream)?;
    if width > 0xFFFF || width == 0 {
        return None;
    }
    let height = read_mbf(stream)?;
    if height > 0xFFFF || height == 0 {
        return None;
    }
    Some(ISize::new(width as i32, height as i32))
}

/// Port of `SkWbmpCodec::IsWbmp`: whether `buffer` starts with a valid WBMP header.
// Port of: src/codec/SkWbmpCodec.cpp#L157-L160 (chrome/m156)
#[doc(alias = "IsWbmp")]
#[must_use]
pub fn is_wbmp(buffer: &[u8]) -> bool {
    let mut stream = MemoryStream::make_copy(buffer);
    read_header(&mut *stream).is_some()
}

/// The WBMP decoder state. Port of `SkWbmpCodec`.
#[derive(Debug)]
#[doc(alias = "SkWbmpCodec")]
pub struct WbmpCodec {
    src_row_bytes: usize,
    src_buffer: Vec<u8>,
    swizzler: Option<Swizzler>,
}

impl WbmpCodec {
    // Port of: src/codec/SkWbmpCodec.cpp#L110-L116 (the constructor)
    fn new(width: i32) -> Self {
        Self {
            src_row_bytes: get_src_row_bytes(width),
            src_buffer: Vec::new(),
            swizzler: None,
        }
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L106-L108 (readRow)
    fn read_row(stream: &mut dyn Stream, row: &mut [u8]) -> bool {
        stream.read(row) == row.len()
    }

    /// Port of `SkWbmpCodec::MakeFromStream`: reads the header and builds the codec. The header is
    /// consumed, so the codec's first decode reads pixel rows straight after it.
    ///
    /// # Errors
    /// `CouldNotRewind` if the header cannot be read. It was valid when the format was sniffed, so
    /// this means the stream changed in between.
    // Port of: src/codec/SkWbmpCodec.cpp#L162-L180 (chrome/m156)
    #[doc(alias = "SkWbmpCodec::MakeFromStream")]
    pub fn make_from_stream<'a>(
        mut stream: Box<dyn Stream + Send + 'a>,
    ) -> std::result::Result<Codec<'a>, Result> {
        let Some(size) = read_header(&mut *stream) else {
            // This already succeeded in IsWbmp, so this stream was corrupted in/after rewind.
            return Err(Result::CouldNotRewind);
        };
        let info = EncodedInfo::make(size.width, size.height, Color::Gray, Alpha::Opaque, 1);
        let imp = Box::new(Self::new(size.width));
        Ok(Codec::new(
            info,
            imp,
            Some(stream),
            EncodedOrigin::TopLeft,
            None,
        ))
    }
}

impl CodecImpl for WbmpCodec {
    // Port of: src/codec/SkWbmpCodec.h#L47-L50 (getSampler)
    fn on_get_sampler(
        &mut self,
        _base: &CodecBase<'_>,
        _create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        self.swizzler
            .as_mut()
            .map(|swizzler| swizzler as &mut dyn Sampler)
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L118-L120 (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::WBMP
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L99-L104 (onRewind)
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        if !base.rewind_stream() {
            return false;
        }
        match base.stream() {
            Some(stream) => read_header(stream).is_some(),
            None => false,
        }
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L122-L125 (conversionSupported)
    fn conversion_supported(
        &self,
        _base: &CodecBase<'_>,
        dst: &ImageInfo,
        src_is_opaque: bool,
        _needs_color_xform: bool,
    ) -> bool {
        valid_color_type(dst) && valid_alpha(dst.alpha_type(), src_is_opaque)
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L127-L155 (onGetPixels)
    #[allow(clippy::cast_sign_loss)] // row indices and counts are non-negative
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        if options.subset.is_some() {
            return Result::Unimplemented;
        }
        let Some(swizzler) = Swizzler::make(base.encoded_info(), None, info, options, None) else {
            return Result::InternalError;
        };
        let width_src = self.src_row_bytes;
        let mut src = vec![0u8; width_src];
        let height = info.height();
        for y in 0..height {
            let Some(stream) = base.stream() else {
                *rows_decoded = y;
                return Result::IncompleteInput;
            };
            if !Self::read_row(stream, &mut src) {
                *rows_decoded = y;
                return Result::IncompleteInput;
            }
            swizzler.swizzle(&mut dst[y as usize * row_bytes..], &src);
        }
        Result::Success
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L199-L212 (onStartScanlineDecode)
    fn on_start_scanline_decode(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        if options.subset.is_some() {
            return Result::Unimplemented;
        }
        self.swizzler = Swizzler::make(base.encoded_info(), None, dst_info, options, None);
        self.src_buffer = vec![0u8; self.src_row_bytes];
        if self.swizzler.is_none() {
            return Result::InternalError;
        }
        Result::Success
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L182-L192 (onGetScanlines)
    #[allow(clippy::cast_sign_loss)] // row indices and counts are non-negative
    fn on_get_scanlines(
        &mut self,
        base: &mut CodecBase<'_>,
        dst: &mut [u8],
        count: i32,
        row_bytes: usize,
    ) -> i32 {
        let Some(swizzler) = self.swizzler.as_ref() else {
            return 0;
        };
        for y in 0..count {
            let Some(stream) = base.stream() else {
                return y;
            };
            if !Self::read_row(stream, &mut self.src_buffer) {
                return y;
            }
            swizzler.swizzle(&mut dst[y as usize * row_bytes..], &self.src_buffer);
        }
        count
    }

    // Port of: src/codec/SkWbmpCodec.cpp#L194-L197 (onSkipScanlines)
    #[allow(clippy::cast_sign_loss)] // the skip count is non-negative
    fn on_skip_scanlines(&mut self, base: &mut CodecBase<'_>, count: i32) -> bool {
        let bytes_to_skip = count as usize * self.src_row_bytes;
        match base.stream() {
            Some(stream) => stream.skip(bytes_to_skip) == bytes_to_skip,
            None => false,
        }
    }
}
