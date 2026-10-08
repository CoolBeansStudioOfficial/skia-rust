// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkIcoCodec.cpp#L1-L441, src/codec/SkIcoCodec.h, include/codec/SkIcoDecoder.h
// (chrome/m156)
// Ported from: src/codec/SkIcoCodec.cpp, src/codec/SkIcoCodec.h, include/codec/SkIcoDecoder.h
//
// `getSampler` and `onGetScaledDimensions` are forwarded to the embedded codec in use.

//! The ICO decoder. An ICO file holds several images, each a BMP or a PNG. The codec makes one
//! embedded codec per image and sends every request to the one whose size matches.

use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream};
use skia_rust_core::stream_priv::copy_stream_to_data;

use crate::bmp;
use crate::codec::{Codec, CodecBase, CodecImpl, Options, Result, ScanlineOrder};
use crate::codecs;
use crate::encoded_info::EncodedInfo;
use crate::png_codec::{self, is_png_format};
use crate::sampler::Sampler;

// Header size constants. Port of `kIcoDirectoryBytes` and `kIcoDirEntryBytes`.
const ICO_DIRECTORY_BYTES: u32 = 6;
const ICO_DIR_ENTRY_BYTES: u32 = 16;

/// Port of `SkIcoCodec::IsIco`: whether the buffer starts with the ICO or CUR signature.
// Port of: src/codec/SkIcoCodec.cpp#L22-L30 (chrome/m156)
#[doc(alias = "IsIco")]
#[must_use]
pub fn is_ico(buffer: &[u8]) -> bool {
    buffer.starts_with(&[0x00, 0x00, 0x01, 0x00]) || buffer.starts_with(&[0x00, 0x00, 0x02, 0x00])
}

// One directory entry, the vital fields of it. Port of the `Entry` struct in MakeFromStream.
#[derive(Clone, Copy)]
struct Entry {
    offset: u32,
    size: u32,
}

// Little-endian reads, as Skia's `SkCodecPriv::UnsafeGetInt`/`UnsafeGetShort`. Callers check the
// buffer length first.
fn le_u32(buffer: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]])
}

fn le_u16(buffer: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([buffer[i], buffer[i + 1]])
}

/// Port of `SkIcoCodec::MakeFromStream`, as the decoder entry of the registry: reads the directory,
/// makes an embedded codec for each image that can be decoded, and keeps the largest image's
/// encoded info as the codec's own.
///
/// # Errors
/// The [`Result`] that says why no codec could be made: `InvalidInput` if the directory is empty or
/// no embedded image decodes, and `IncompleteInput` if the directory or an image is truncated.
// Port of: src/codec/SkIcoCodec.cpp#L32-L154 (chrome/m156)
#[doc(alias = "SkIcoCodec::MakeFromStream")]
pub fn make_from_stream<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    // It is helpful to have the entire stream in one buffer. If the stream is already in memory,
    // the codec keeps the stream; otherwise the stream is copied and dropped.
    let (data, kept_stream) = if let Some(bytes) = stream.get_memory_base().map(Data::new_copy) {
        (bytes, Some(stream))
    } else {
        let data = copy_stream_to_data(&mut *stream).ok_or(Result::InvalidInput)?;
        (data, None)
    };
    let bytes = data.as_bytes();

    // Read the directory header.
    if data.size() < ICO_DIRECTORY_BYTES as usize {
        return Err(Result::IncompleteInput);
    }

    // Process the directory header.
    let num_images = u32::from(le_u16(bytes, 4));
    if num_images == 0 {
        return Err(Result::InvalidInput);
    }

    // Iterate over the directory entries. The width, height, depth and palette size are ignored:
    // the embedded image's own header is the one to trust.
    let mut entries = Vec::with_capacity(num_images as usize);
    for i in 0..num_images {
        if data.size() < (ICO_DIRECTORY_BYTES + (i + 1) * ICO_DIR_ENTRY_BYTES) as usize {
            return Err(Result::IncompleteInput);
        }
        let entry = (ICO_DIRECTORY_BYTES + i * ICO_DIR_ENTRY_BYTES) as usize;
        entries.push(Entry {
            size: le_u32(bytes, entry + 8),
            offset: le_u32(bytes, entry + 12),
        });
    }

    // The default result, if no valid embedded codecs are found.
    let mut result = Result::InvalidInput;

    // The images are not required to be stored in order of increasing offset, so sort them by it.
    entries.sort_by_key(|entry| entry.offset);

    // Now make a candidate codec for each of the embedded images.
    let mut bytes_read = ICO_DIRECTORY_BYTES + num_images * ICO_DIR_ENTRY_BYTES;
    let mut codecs: Vec<Codec<'static>> = Vec::new();
    for entry in &entries {
        let offset = entry.offset;
        let size = entry.size;

        // Ensure that the offset is valid.
        if offset < bytes_read {
            // Warning: invalid ico offset.
            continue;
        }

        // If we cannot skip, assume we have reached the end of the stream and stop making codecs.
        if offset as usize >= data.size() {
            // Warning: could not skip to ico offset.
            break;
        }
        bytes_read = offset;

        // The sum wraps in 32 bits, as Skia's `uint32_t` arithmetic does.
        if offset.wrapping_add(size) as usize > data.size() {
            // Warning: could not create embedded stream.
            result = Result::IncompleteInput;
            break;
        }

        let embedded = Data::new_subset(&data, offset as usize, size as usize);
        bytes_read = bytes_read.wrapping_add(size);

        // Check whether the embedded codec is a PNG or a BMP, and make the codec.
        let codec = if is_png_format(embedded.as_bytes()) {
            // Port of `SkCodec::MakeFromStream(std::move(embeddedStream), &ignoredResult)`, which
            // reads the registry.
            let mut codec =
                codecs::make_codec_from_stream(MemoryStream::make(Some(embedded.clone()))).ok();
            // Fall back to the built-in PNG decoder when the caller did not register a PNG decoder in
            // the global registry. The registry always has one, so this is not reached today.
            if codec.is_none() && !codecs::has_decoder("png") {
                codec = png_codec::make_from_stream(MemoryStream::make(Some(embedded))).ok();
            }
            codec
        } else {
            bmp::make_from_ico(MemoryStream::make(Some(embedded))).ok()
        };

        if let Some(codec) = codec {
            codecs.push(codec);
        }
    }

    if codecs.is_empty() {
        // Error: could not find any valid embedded ico codecs.
        return Err(result);
    }

    // Use the largest codec as a "suggestion" for the image info.
    let mut max_size = 0usize;
    let mut max_index = 0usize;
    for (i, codec) in codecs.iter().enumerate() {
        let size = codec.info().compute_min_byte_size();
        if size > max_size {
            max_size = size;
            max_index = i;
        }
    }
    let max_info: EncodedInfo = codecs[max_index].encoded_info().clone();

    Ok(Codec::new(
        max_info,
        Box::new(IcoCodec {
            embedded: codecs,
            curr: None,
        }),
        kept_stream,
        EncodedOrigin::TopLeft,
        None,
    ))
}

/// The ICO decoder's own state: the embedded codecs and the one a scanline or incremental decode
/// is using. Port of the `fEmbeddedCodecs` and `fCurrCodec` members of `SkIcoCodec`.
pub(crate) struct IcoCodec {
    embedded: Vec<Codec<'static>>,
    // Port of `fCurrCodec`: the index of the embedded codec the last scanline or incremental decode
    // started with.
    curr: Option<usize>,
}

impl IcoCodec {
    // Port of: src/codec/SkIcoCodec.cpp#L157-L166 (chooseCodec): the first embedded codec at or after
    // `start_index` whose dimensions are `requested_size`.
    fn choose_codec(&self, requested_size: ISize, start_index: usize) -> Option<usize> {
        (start_index..self.embedded.len())
            .find(|&i| self.embedded[i].dimensions() == requested_size)
    }
}

impl CodecImpl for IcoCodec {
    // Port of: src/codec/SkIcoCodec.h (onGetEncodedFormat)
    fn on_get_encoded_format(&self) -> EncodedImageFormat {
        EncodedImageFormat::ICO
    }

    // Port of: src/codec/SkIcoCodec.cpp#L167-L178 (onDimensionsSupported)
    fn on_dimensions_supported(&self, _base: &CodecBase<'_>, dim: ISize) -> bool {
        self.choose_codec(dim, 0).is_some()
    }

    // Port of: src/codec/SkIcoCodec.cpp#L180-L203 (onGetPixels)
    fn on_get_pixels(
        &mut self,
        _base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result {
        if options.subset.is_some() {
            // Subsets are not supported.
            return Result::Unimplemented;
        }

        let mut index = 0;
        let mut result = Result::InvalidScale;
        while let Some(i) = self.choose_codec(info.dimensions(), index) {
            result = self.embedded[i].get_pixels(info, dst, row_bytes, Some(options));
            match result {
                Result::Success | Result::IncompleteInput => {
                    // The embedded codec fills incomplete images itself, so every row is initialized.
                    *rows_decoded = info.height();
                    return result;
                }
                // Continue trying to find a valid embedded codec on a failed decode.
                _ => index = i + 1,
            }
        }
        // Error: no matching candidate image in ico.
        result
    }

    // Port of: src/codec/SkIcoCodec.cpp#L205-L224 (onStartScanlineDecode)
    fn on_start_scanline_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        options: &Options,
    ) -> Result {
        let mut index = 0;
        let mut result = Result::InvalidScale;
        while let Some(i) = self.choose_codec(dst_info.dimensions(), index) {
            result = self.embedded[i].start_scanline_decode(dst_info, Some(options));
            if result == Result::Success {
                self.curr = Some(i);
                return result;
            }
            index = i + 1;
        }
        // Error: no matching candidate image in ico.
        result
    }

    // Port of: src/codec/SkIcoCodec.cpp#L226-L229 (onGetScanlines)
    fn on_get_scanlines(
        &mut self,
        _base: &mut CodecBase<'_>,
        dst: &mut [u8],
        count: i32,
        row_bytes: usize,
    ) -> i32 {
        match self.curr {
            Some(i) => self.embedded[i].get_scanlines(dst, count, row_bytes),
            None => 0,
        }
    }

    // Port of: src/codec/SkIcoCodec.cpp#L231-L234 (onSkipScanlines)
    fn on_skip_scanlines(&mut self, _base: &mut CodecBase<'_>, count: i32) -> bool {
        match self.curr {
            Some(i) => self.embedded[i].skip_scanlines(count),
            None => false,
        }
    }

    // Port of: src/codec/SkIcoCodec.cpp#L236-L247 (onSupportsIncrementalDecode)
    fn on_supports_incremental_decode(&self, dst: &ImageInfo) -> bool {
        let mut index = 0;
        while let Some(i) = self.choose_codec(dst.dimensions(), index) {
            if self.embedded[i].supports_incremental_decode_imp(dst) {
                return true;
            }
            index = i + 1;
        }
        false
    }

    // Port of: src/codec/SkIcoCodec.cpp#L249-L271 (onStartIncrementalDecode)
    fn on_start_incremental_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> Result {
        let mut index = 0;
        while let Some(i) = self.choose_codec(dst_info.dimensions(), index) {
            // The guard only borrows the destination for the call, so it is dropped at once: the
            // embedded codec keeps the decode's state, and the destination is passed again to
            // `on_incremental_decode`.
            let started = self.embedded[i]
                .start_incremental_decode(dst_info, dst, row_bytes, Some(options))
                .is_ok();
            if started {
                self.curr = Some(i);
                return Result::Success;
            }
            index = i + 1;
        }
        // Error: no matching candidate image in ico.
        Result::InvalidScale
    }

    // Port of: src/codec/SkIcoCodec.cpp#L273-L276 (onIncrementalDecode)
    fn on_incremental_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        dst: &mut [u8],
        rows_decoded: &mut i32,
    ) -> Result {
        match self.curr {
            Some(i) => {
                let (result, rows) = self.embedded[i].incremental_decode_imp(dst);
                *rows_decoded = rows;
                result
            }
            None => Result::InternalError,
        }
    }

    // Port of: src/codec/SkIcoCodec.cpp#L234-L257 (onGetScaledDimensions): the embedded image whose
    // area is closest to the desired area.
    // The float arithmetic mirrors the C++ (int products converted to float).
    #[allow(clippy::cast_precision_loss)] // dimensions are small; the C++ converts them to float too
    fn on_get_scaled_dimensions(&self, base: &CodecBase<'_>, desired_scale: f32) -> ISize {
        let orig_width = base.dimensions().width;
        let orig_height = base.dimensions().height;
        let desired_size = desired_scale * orig_width as f32 * orig_height as f32;
        // At least one image will have smaller error than this initial value.
        let mut min_error = (orig_width * orig_height) as f32 - desired_size + 1.0;
        let mut min_index = None;
        for (i, codec) in self.embedded.iter().enumerate() {
            let dimensions = codec.dimensions();
            let error = ((dimensions.width * dimensions.height) as f32 - desired_size).abs();
            if error < min_error {
                min_error = error;
                min_index = Some(i);
            }
        }
        match min_index {
            Some(i) => self.embedded[i].dimensions(),
            // Skia asserts that an embedded image was found; the codec's own size is the fallback.
            None => base.dimensions(),
        }
    }

    // Port of: src/codec/SkIcoCodec.cpp#L406-L412 (getSampler): the sampler of the embedded codec
    // in use, if a decode has started.
    fn on_get_sampler(
        &mut self,
        _base: &CodecBase<'_>,
        create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        match self.curr {
            Some(i) => self.embedded[i].get_sampler(create_if_necessary),
            None => None,
        }
    }

    // Port of: src/codec/SkIcoCodec.cpp#L278-L287 (onGetScanlineOrder): the current embedded codec's
    // order, or top-down before a decode has started.
    fn on_get_scanline_order(&self) -> ScanlineOrder {
        match self.curr {
            Some(i) => self.embedded[i].scanline_order(),
            None => ScanlineOrder::TopDown,
        }
    }
}
