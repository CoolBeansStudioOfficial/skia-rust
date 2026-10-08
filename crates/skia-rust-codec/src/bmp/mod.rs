// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkBmpCodec.cpp#L1-L698, src/codec/SkBmpCodec.h, src/codec/SkBmpBaseCodec.h,
// src/codec/SkBmpBaseCodec.cpp, include/codec/SkBmpDecoder.h (chrome/m156)
// Ported from: src/codec/SkBmpCodec.cpp, src/codec/SkBmpCodec.h, src/codec/SkBmpBaseCodec.h,
// src/codec/SkBmpBaseCodec.cpp, include/codec/SkBmpDecoder.h
//
// Not ported yet: the BMP-in-ICO variant (`inIco`: the ICO codec calls ReadHeader with it, and
// SkIcoCodec is not ported), sampling (`getSampler`, which SkSampledCodec uses), and the
// `SkCodecPrintf` diagnostics. Every other path of ReadHeader and of the three codecs is here.

//! The BMP decoder. [`read_header`] parses the file headers exactly as Skia does; the three
//! codecs it can create (standard, bit-mask and RLE) live in the submodules.

mod mask;
mod rle;
mod standard;

use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::stream::Stream;
use skia_rust_skcms::PixelFormat;

use crate::codec::{Codec, CodecBase, Result, ScanlineOrder};
use crate::codec_priv::{
    PMCOLOR_IS_RGBA, pack_argb_as_bgra, pack_argb_as_rgba, premultiply_argb_as_bgra,
    premultiply_argb_as_rgba,
};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::masks::{InputMasks, Masks};

use self::mask::BmpMaskCodec;
use self::rle::BmpRleCodec;
use self::standard::BmpStandardCodec;

// Port of: src/codec/SkBmpCodec.cpp#L94-L105 (the header size constants)
const BMP_HEADER_BYTES: u32 = 14;
const BMP_HEADER_BYTES_PLUS_FOUR: usize = 18;
const BMP_OS2V1_BYTES: u32 = 12;
const BMP_MASK_BYTES: usize = 12;

// Port of: src/codec/SkBmpCodec.cpp#L49-L58 (the compression methods this decoder reads)
const K_NONE_COMPRESSION: u32 = 0;
const K_8BIT_RLE_COMPRESSION: u32 = 1;
const K_4BIT_RLE_COMPRESSION: u32 = 2;
const K_BIT_MASKS_COMPRESSION: u32 = 3;
const K_JPEG_COMPRESSION: u32 = 4;
const K_PNG_COMPRESSION: u32 = 5;
const K_ALPHA_BIT_MASKS_COMPRESSION: u32 = 6;
const K_CMYK_COMPRESSION: u32 = 11;
const K_CMYK_8BIT_RLE_COMPRESSION: u32 = 12;
const K_CMYK_4BIT_RLE_COMPRESSION: u32 = 13;

// Arbitrary maximum width and height of a BMP. Port of `kMaxDim` in ReadHeader (matches Chromium).
const K_MAX_DIM: i32 = 1 << 16;

// Port of: src/codec/SkBmpCodec.cpp#L88-L93 (the BMP-to-xform source format: BGRA, since BMPs are
// typically BGRA/BGR, `SkBmpCodec::kXformSrcColorFormat`)
const XFORM_SRC_COLOR_FORMAT: PixelFormat = PixelFormat::Bgra8888;

/// Port of `SkBmpCodec::IsBmp`: whether `buffer` starts with the `BM` signature.
// Port of: src/codec/SkBmpCodec.cpp#L44-L50 (chrome/m156)
#[doc(alias = "IsBmp")]
#[must_use]
pub fn is_bmp(buffer: &[u8]) -> bool {
    buffer.starts_with(b"BM")
}

/// The second-header versions. Port of `BmpHeaderType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderType {
    InfoV1,
    InfoV2,
    InfoV3,
    InfoV4,
    InfoV5,
    Os2V1,
    Os2Vx,
}

// Port of: src/codec/SkBmpCodec.cpp#L125-L167 (get_header_type)
fn get_header_type(info_bytes: u32) -> Option<HeaderType> {
    if info_bytes >= 16 {
        return match info_bytes {
            40 => Some(HeaderType::InfoV1),
            52 => Some(HeaderType::InfoV2),
            56 => Some(HeaderType::InfoV3),
            108 => Some(HeaderType::InfoV4),
            124 => Some(HeaderType::InfoV5),
            16 | 20 | 24 | 28 | 32 | 36 | 42 | 46 | 48 | 60 | 64 => Some(HeaderType::Os2Vx),
            _ => None,
        };
    }
    // The OS/2 1.x header is treated separately because it has a unique format.
    if info_bytes >= BMP_OS2V1_BYTES {
        Some(HeaderType::Os2V1)
    } else {
        None
    }
}

// Which codec the file needs. Port of `BmpInputFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Standard,
    Rle,
    BitMask,
}

/// The format-specific part of a parsed header. Port of the `inputFormat` switch in `ReadHeader`.
#[derive(Debug, Clone, Copy)]
enum Format {
    // The colour kind and bits per component the swizzler reads.
    Standard {
        color: Color,
        bits_per_component: u8,
    },
    // The raw masks from the file, trimmed and checked when the codec is made.
    Mask {
        masks: InputMasks,
    },
    Rle,
}

/// What [`read_header`] learned about the file. Port of the locals that `ReadHeader` hands to each
/// codec's constructor.
#[derive(Debug, Clone, Copy)]
struct Header {
    width: i32,
    height: i32,
    row_order: ScanlineOrder,
    bits_per_pixel: u16,
    num_colors: u32,
    bytes_per_color: u32,
    // The offset of the pixel data from the end of the headers (`offset - bytesRead` in C++).
    offset: u32,
    format: Format,
}

// Little-endian reads, as Skia's `SkCodecPriv::UnsafeGetInt`/`UnsafeGetShort` produce on every
// host. Callers check the buffer length first.
fn le_u32(buffer: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]])
}

fn le_u16(buffer: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([buffer[i], buffer[i + 1]])
}

// Port of: src/codec/SkBmpCodec.cpp#L169-L455 (SkBmpCodec::ReadHeader, the non-ICO path). The
// stream is read up to the pixel data, and for bit-mask files it is also moved to the pixel data.
// It is called with the codec-making path or the rewind path, which differ only in the codec
// they keep (see `Header::into_codec`).
// The width and height are checked to be in 1..0x10000 before the codec is made, so the casts
// from the header's `u32` fields to `i32` are the C++ `int` reads.
#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)] // one branch per header type and compression method, as in C++
fn read_header(stream: &mut dyn Stream) -> std::result::Result<Header, Result> {
    // Read the first header and the size of the second header.
    let mut h_buffer = [0u8; BMP_HEADER_BYTES_PLUS_FOUR];
    if stream.read(&mut h_buffer) != h_buffer.len() {
        return Err(Result::IncompleteInput);
    }
    let total_bytes = le_u32(&h_buffer, 2);
    let offset = le_u32(&h_buffer, 10);
    if offset < BMP_HEADER_BYTES + BMP_OS2V1_BYTES {
        return Err(Result::InvalidInput);
    }
    // The size is the first field of the second header, so the first four bytes are already read.
    let info_bytes = le_u32(&h_buffer, 14);
    if info_bytes < BMP_OS2V1_BYTES {
        return Err(Result::InvalidInput);
    }

    // Determine image information depending on the second header format.
    let Some(header_type) = get_header_type(info_bytes) else {
        return Err(Result::InvalidInput);
    };

    // We already read the first four bytes of the info header to get the size.
    let info_bytes_remaining = info_bytes - 4;
    let mut i_buffer = vec![0u8; info_bytes_remaining as usize];
    if stream.read(&mut i_buffer) != i_buffer.len() {
        return Err(Result::IncompleteInput);
    }

    // The width, height, bits per pixel and compression come from the second header.
    let mut bits_per_pixel: u16;
    let compression: u32;
    let num_colors: u32;
    let bytes_per_color: u32;
    let width: i32;
    let mut height: i32;
    if header_type == HeaderType::Os2V1 {
        // The OS2V1 header has 16-bit dimensions and 3-byte colour table entries.
        width = i32::from(le_u16(&i_buffer, 0));
        height = i32::from(le_u16(&i_buffer, 2));
        bits_per_pixel = le_u16(&i_buffer, 6);
        compression = K_NONE_COMPRESSION;
        num_colors = 0;
        bytes_per_color = 3;
    } else {
        // Every other header that reaches here has at least the fields read below.
        width = le_u32(&i_buffer, 0) as i32;
        height = le_u32(&i_buffer, 4) as i32;
        bits_per_pixel = le_u16(&i_buffer, 10);
        // Some versions do not have these fields, so they are only read when the header has them.
        let mut c = K_NONE_COMPRESSION;
        let mut n = 0;
        if info_bytes_remaining >= 16 {
            c = le_u32(&i_buffer, 12);
            if info_bytes_remaining >= 32 {
                n = le_u32(&i_buffer, 28);
            }
        }
        compression = c;
        num_colors = n;
        // Every header that reaches here stores colour table entries in 4 bytes.
        bytes_per_color = 4;
    }

    // Check for valid dimensions from the header. A negative height means top-down rows.
    let mut row_order = ScanlineOrder::BottomUp;
    if height < 0 {
        // We can't negate INT32_MIN.
        if height == i32::MIN {
            return Err(Result::InvalidInput);
        }
        height = -height;
        row_order = ScanlineOrder::TopDown;
    }

    // Arbitrary maximum. Matches Chromium.
    if width <= 0 || height <= 0 || width >= K_MAX_DIM || height >= K_MAX_DIM {
        return Err(Result::InvalidInput);
    }

    // Determine the input compression format, and set the bit masks where the file has them.
    let mut input_masks = InputMasks::default();
    let mut mask_bytes: u32 = 0;
    let input_format = match compression {
        K_NONE_COMPRESSION => {
            // Besides the standard formats, BMP supports bit masks. The standard 16-bit format is
            // 555 (XRRRRRGGGGGBBBBB), which no Skia colour type matches, so 16 bits per pixel is
            // always read through masks.
            if bits_per_pixel == 16 {
                input_masks.red = 0x7C00;
                input_masks.green = 0x03E0;
                input_masks.blue = 0x001F;
                InputFormat::BitMask
            } else {
                InputFormat::Standard
            }
        }
        K_8BIT_RLE_COMPRESSION => {
            // A wrong bit count is corrected, as Skia does.
            bits_per_pixel = 8;
            InputFormat::Rle
        }
        K_4BIT_RLE_COMPRESSION => {
            bits_per_pixel = 4;
            InputFormat::Rle
        }
        K_ALPHA_BIT_MASKS_COMPRESSION | K_BIT_MASKS_COMPRESSION => {
            // Load the masks.
            match header_type {
                HeaderType::InfoV1 => {
                    // The V1 header stores the bit masks after the header.
                    let mut buffer = [0u8; BMP_MASK_BYTES];
                    if stream.read(&mut buffer) != buffer.len() {
                        return Err(Result::IncompleteInput);
                    }
                    mask_bytes = BMP_MASK_BYTES as u32;
                    input_masks.red = le_u32(&buffer, 0);
                    input_masks.green = le_u32(&buffer, 4);
                    input_masks.blue = le_u32(&buffer, 8);
                }
                HeaderType::InfoV2
                | HeaderType::InfoV3
                | HeaderType::InfoV4
                | HeaderType::InfoV5 => {
                    // The masks are inside the info header, which is at least 52 bytes here.
                    input_masks.red = le_u32(&i_buffer, 36);
                    input_masks.green = le_u32(&i_buffer, 40);
                    input_masks.blue = le_u32(&i_buffer, 44);
                    // V4 and V5 files have an alpha mask. V2 has none, and neither does a V3 file
                    // (V3 files are mostly opaque with a blank alpha channel).
                    if matches!(header_type, HeaderType::InfoV4 | HeaderType::InfoV5) {
                        input_masks.alpha = le_u32(&i_buffer, 48);
                    }
                }
                HeaderType::Os2Vx => {
                    // Unsupported in the previous version and in Chromium, and no test uses it.
                    return Err(Result::Unimplemented);
                }
                HeaderType::Os2V1 => return Err(Result::InvalidInput),
            }
            InputFormat::BitMask
        }
        K_JPEG_COMPRESSION if bits_per_pixel == 24 => InputFormat::Rle,
        // JPEG at other bit counts, PNG, and the CMYK forms are not supported.
        K_JPEG_COMPRESSION
        | K_PNG_COMPRESSION
        | K_CMYK_COMPRESSION
        | K_CMYK_8BIT_RLE_COMPRESSION
        | K_CMYK_4BIT_RLE_COMPRESSION => return Err(Result::Unimplemented),
        _ => return Err(Result::InvalidInput),
    };

    // Calculate the number of bytes read so far.
    let bytes_read = BMP_HEADER_BYTES + info_bytes + mask_bytes;
    if offset < bytes_read {
        // Skia also fails here rather than guessing the offset of the pixel data.
        return Err(Result::InvalidInput);
    }
    let offset_after_headers = offset - bytes_read;

    let format = match input_format {
        InputFormat::Standard => {
            // BMPs are opaque, and the palette and direct forms are the only ones read here.
            let (color, bits_per_component) = match bits_per_pixel {
                1 | 2 | 4 | 8 => (Color::Palette, bits_per_pixel as u8),
                24 => (Color::BGR, 8),
                32 => (Color::BGRX, 8),
                _ => return Err(Result::InvalidInput),
            };
            Format::Standard {
                color,
                bits_per_component,
            }
        }
        InputFormat::BitMask => {
            match bits_per_pixel {
                16 | 24 | 32 => {}
                _ => return Err(Result::InvalidInput),
            }
            // Skip to the start of the pixel array. This is possible here because there is no
            // colour table in bit-mask mode.
            let to_skip = offset_after_headers as usize;
            if stream.skip(to_skip) != to_skip {
                return Err(Result::IncompleteInput);
            }
            Format::Mask { masks: input_masks }
        }
        InputFormat::Rle => {
            // RLE needs a valid total size, since its end is found from it.
            if total_bytes <= offset {
                return Err(Result::InvalidInput);
            }
            Format::Rle
        }
    };

    Ok(Header {
        width,
        height,
        row_order,
        bits_per_pixel,
        num_colors,
        bytes_per_color,
        offset: offset_after_headers,
        format,
    })
}

// Port of: src/core/SkColorData.h and src/codec/SkCodecPriv.h (IsRGBA)
fn is_rgba(color_type: ColorType) -> bool {
    if PMCOLOR_IS_RGBA {
        color_type != ColorType::BGRA8888
    } else {
        color_type == ColorType::RGBA8888
    }
}

/// Port of `SkCodecPriv::ChoosePackColorProc`: the packer for a colour type, premultiplied or not.
// Port of: src/codec/SkCodecPriv.h#L326-L340 (chrome/m156)
fn choose_pack_argb(is_premul: bool, color_type: ColorType) -> fn(u32, u32, u32, u32) -> u32 {
    match (is_premul, is_rgba(color_type)) {
        (true, true) => premultiply_argb_as_rgba,
        (true, false) => premultiply_argb_as_bgra,
        (false, true) => pack_argb_as_rgba,
        (false, false) => pack_argb_as_bgra,
    }
}

/// Reads exactly `buffer.len()` bytes from the codec's stream, or fails.
pub(crate) fn read_exact(base: &mut CodecBase<'_>, buffer: &mut [u8]) -> bool {
    base.stream()
        .is_some_and(|stream| stream.read(buffer) == buffer.len())
}

/// The state every BMP codec shares. Port of the members of `SkBmpCodec` and `SkBmpBaseCodec`.
#[derive(Debug)]
struct BmpBase {
    bits_per_pixel: u16,
    row_order: ScanlineOrder,
    // Bytes in one row of the file, padded to four bytes.
    src_row_bytes: usize,
    // Decoded BGRA pixels for a colour transform that runs on decode.
    xform_buffer: Vec<u8>,
}

impl BmpBase {
    // Port of: src/codec/SkBmpCodec.cpp#L572-L575 (the SkBmpCodec constructor's fSrcRowBytes)
    fn new(width: i32, bits_per_pixel: u16, row_order: ScanlineOrder) -> Self {
        Self {
            bits_per_pixel,
            row_order,
            src_row_bytes: compute_row_bytes(width, u32::from(bits_per_pixel)).next_multiple_of(4),
            xform_buffer: Vec::new(),
        }
    }

    /// Port of `SkBmpCodec::getDstRow`: the destination row for encoded row `y`. Bottom-up files
    /// store the last row first.
    // Port of: src/codec/SkBmpCodec.cpp#L610-L619 (chrome/m156)
    fn get_dst_row(&self, y: i32, height: i32) -> i32 {
        if self.row_order == ScanlineOrder::TopDown {
            return y;
        }
        height - y - 1
    }

    /// Port of `SkBmpCodec::skipRows`.
    // Port of: src/codec/SkBmpCodec.cpp#L685-L688 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the skip count is non-negative (checked by the caller)
    fn skip_rows(&self, base: &mut CodecBase<'_>, count: i32) -> bool {
        let bytes_to_skip = count as usize * self.src_row_bytes;
        base.stream()
            .is_some_and(|stream| stream.skip(bytes_to_skip) == bytes_to_skip)
    }
}

/// Port of `SkCodecPriv::ComputeRowBytes`: the bytes one row of `width` pixels takes.
// Port of: src/codec/SkCodecPriv.h#L217-L227 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // widths are positive (checked by the header reader)
fn compute_row_bytes(width: i32, bits_per_pixel: u32) -> usize {
    let width = width as usize;
    if bits_per_pixel < 16 {
        let pixels_per_byte = (8 / bits_per_pixel) as usize;
        width.div_ceil(pixels_per_byte)
    } else {
        width * (bits_per_pixel / 8) as usize
    }
}

/// Port of `SkBmpCodec::onRewind`: rewinds the stream and reads the header again, so the stream
/// is at the pixel data (or the colour table) of a fresh decode.
// Port of: src/codec/SkBmpCodec.cpp#L604-L608 (chrome/m156)
fn rewind(base: &mut CodecBase<'_>) -> bool {
    if !base.rewind_stream() {
        return false;
    }
    match base.stream() {
        Some(stream) => read_header(stream).is_ok(),
        None => false,
    }
}

impl Header {
    /// The codec-making half of `ReadHeader` (its `codecOut` branches). The stream moves into the
    /// codec.
    // Port of: src/codec/SkBmpCodec.cpp#L340-L475 (the codecOut branches of ReadHeader)
    fn into_codec<'a>(
        self,
        stream: Box<dyn Stream + Send + 'a>,
    ) -> std::result::Result<Codec<'a>, Result> {
        let Self {
            width,
            height,
            row_order,
            bits_per_pixel,
            num_colors,
            bytes_per_color,
            offset,
            format,
        } = self;
        match format {
            Format::Standard {
                color,
                bits_per_component,
            } => {
                // Standard BMPs are opaque (the ICO mask is not ported).
                let info =
                    EncodedInfo::make(width, height, color, Alpha::Opaque, bits_per_component);
                let imp = BmpStandardCodec::new(
                    width,
                    bits_per_pixel,
                    num_colors,
                    bytes_per_color,
                    offset,
                    row_order,
                );
                Ok(Codec::new(
                    info,
                    Box::new(imp),
                    Some(stream),
                    EncodedOrigin::TopLeft,
                    Some(XFORM_SRC_COLOR_FORMAT),
                ))
            }
            Format::Mask { masks } => {
                // Check that the input bit masks are valid and create the masks object.
                let Some(masks) = Masks::create(masks, u32::from(bits_per_pixel) / 8) else {
                    return Err(Result::InvalidInput);
                };
                // Masked BMPs are not a fit for `EncodedInfo` (arbitrary component orders and
                // widths), so the closest match is chosen. The mask codec has its own swizzler.
                let (color, alpha) = if masks.alpha_mask() != 0 {
                    (Color::BGRA, Alpha::Unpremul)
                } else {
                    (Color::BGR, Alpha::Opaque)
                };
                let info = EncodedInfo::make(width, height, color, alpha, 8);
                let imp = BmpMaskCodec::new(width, bits_per_pixel, masks, row_order);
                Ok(Codec::new(
                    info,
                    Box::new(imp),
                    Some(stream),
                    EncodedOrigin::TopLeft,
                    Some(XFORM_SRC_COLOR_FORMAT),
                ))
            }
            Format::Rle => {
                // RLE inputs may skip pixels, leaving them transparent, so the image is always
                // BGRA with binary alpha.
                let info = EncodedInfo::make(width, height, Color::BGRA, Alpha::Binary, 8);
                let imp = BmpRleCodec::new(
                    width,
                    bits_per_pixel,
                    num_colors,
                    bytes_per_color,
                    offset,
                    row_order,
                );
                Ok(Codec::new(
                    info,
                    Box::new(imp),
                    Some(stream),
                    EncodedOrigin::TopLeft,
                    Some(XFORM_SRC_COLOR_FORMAT),
                ))
            }
        }
    }
}

/// Port of `SkBmpDecoder::Decode` (the stream form) and `SkBmpCodec::MakeFromStream`: reads the
/// header and builds the codec. The stream is handed to the codec.
///
/// # Errors
/// The [`Result`] that says why the file is not a BMP the decoder supports.
// Port of: src/codec/SkBmpCodec.cpp#L474-L490 (chrome/m156)
#[doc(alias = "SkBmpCodec::MakeFromStream")]
pub fn make_from_stream<'a>(
    mut stream: Box<dyn Stream + Send + 'a>,
) -> std::result::Result<Codec<'a>, Result> {
    let header = read_header(&mut *stream)?;
    header.into_codec(stream)
}
