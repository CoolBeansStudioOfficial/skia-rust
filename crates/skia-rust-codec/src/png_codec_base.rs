// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkPngCodecBase.h (chrome/m156), src/codec/SkPngCodecBase.cpp (chrome/m156)
// Ported from: src/codec/SkPngCodecBase.{h,cpp}
//
// Not ported yet: `onDecodeGainmap`, `onGetGainmapInfo` and `onGetGainmapCodec` (gainmap decoding
// waits for core's gainmap info, codecs.md C13), and `getSampler`, which the sampled codec needs.

//! The state and the row transforms that the PNG decoders share: the swizzler or colour transform
//! for each row, the palette colour table, and the storage for colour-transformed rows.

use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_libpng::PngColor;
use skia_rust_skcms::{IccProfile, PixelFormat};

use crate::codec::{CodecBase, Options, Result, ZeroInitialized};
use crate::codec_priv::{
    pack_argb_as_bgra, pack_argb_as_rgba, premultiply_argb_as_bgra, premultiply_argb_as_rgba,
};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::png_composite_chunk_reader::PngCompositeChunkReader;
use crate::swizzler::Swizzler;

/// Port of `kXformSrcColorType`: colour-transformed rows are first swizzled to `RGBA_8888`.
// Port of: src/codec/SkPngCodecBase.cpp#L20 (chrome/m156)
pub(crate) const XFORM_SRC_COLOR_TYPE: ColorType = ColorType::RGBA8888;

/// Port of `SkPngCodecBase::XformMode`: which passes a row needs.
// Port of: src/codec/SkPngCodecBase.h#L118-L124 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XformMode {
    /// Requires only a swizzle pass.
    SwizzleOnly,
    /// Requires only a colour transform pass.
    ColorOnly,
    /// Requires a swizzle and a colour transform.
    SwizzleColor,
}

/// The palette and transparency chunks that the colour table is built from. Port of the
/// `onTryGetPlteChunk` and `onTryGetTrnsChunk` virtuals, which read them from libpng's info.
pub(crate) struct PaletteSource<'x> {
    /// Port of `onTryGetPlteChunk`: `None` when there is no PLTE chunk.
    pub(crate) plte: Option<&'x [PngColor]>,
    /// Port of `onTryGetTrnsChunk`: `None` when there is no tRNS chunk.
    pub(crate) trns: Option<&'x [u8]>,
}

/// The data members of `SkPngCodecBase`.
// Port of: src/codec/SkPngCodecBase.h#L104-L131 (chrome/m156)
pub(crate) struct PngCodecBase {
    pub(crate) xform_mode: XformMode,
    pub(crate) swizzler: Option<Swizzler>,
    pub(crate) storage: Vec<u8>,
    pub(crate) xform_width: i32,
    pub(crate) color_table: Option<Vec<u32>>,
    pub(crate) encoded_row_bytes: usize,
    pub(crate) dst_row_size: usize,
    pub(crate) dst_info_of_previous_color_table: Option<ImageInfo>,
    pub(crate) chunk_reader: Arc<Mutex<PngCompositeChunkReader>>,
}

// Port of: src/codec/SkPngCodecBase.cpp#L17-L22 (chrome/m156) and SkCodecPriv.h (IsRGBA). The
// macro SK_PMCOLOR_IS_RGBA follows the native N32 layout, as in codec_priv.
#[must_use]
fn is_rgba(color_type: ColorType) -> bool {
    if crate::codec_priv::PMCOLOR_IS_RGBA {
        color_type != ColorType::BGRA8888
    } else {
        color_type == ColorType::RGBA8888
    }
}

// Port of: src/codec/SkPngCodecBase.cpp#L24-L38 (ToPixelFormat)
#[must_use]
pub(crate) fn to_pixel_format(info: &EncodedInfo) -> PixelFormat {
    // Colour PNGs are always RGB or RGBA; 16-bit gray has no format of its own.
    if info.bits_per_component() == 16 {
        if info.color() == Color::RGBA {
            return PixelFormat::Rgba16161616Be;
        } else if info.color() == Color::RGB {
            return PixelFormat::Rgb161616Be;
        }
    } else if info.color() == Color::Gray {
        return PixelFormat::G8;
    }
    PixelFormat::Rgba8888
}

// Port of: src/codec/SkPngCodecBase.cpp#L49-L52 (needs_premul)
#[must_use]
fn needs_premul(dst_alpha: AlphaType, encoded_alpha: Alpha) -> bool {
    dst_alpha == AlphaType::Premul && encoded_alpha == Alpha::Unpremul
}

// Port of: src/codec/SkPngCodecBase.cpp#L54-L60 (IsPng)
#[doc(alias = "IsPng")]
#[must_use]
pub(crate) fn is_png(buf: &[u8]) -> bool {
    const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    buf.len() >= PNG_SIGNATURE.len() && buf[..PNG_SIGNATURE.len()] == PNG_SIGNATURE
}

/// The `skcms` data-colour-space signatures: `'CMYK'`, `'GRAY'` and `'RGB '`.
const SIGNATURE_CMYK: u32 = 0x434D_594B;
const SIGNATURE_GRAY: u32 = 0x4752_4159;

// Port of: src/codec/SkPngCodecBase.cpp#L62-L77 (isCompatibleColorProfileAndType), with
// SkCodecs::ColorProfile::dataSpace (src/codec/SkCodecColorProfile.cpp#L128-L140) inlined
#[must_use]
pub(crate) fn is_compatible_color_profile_and_type(
    profile: Option<&IccProfile>,
    color: Color,
) -> bool {
    let Some(profile) = profile else {
        return true;
    };
    match profile.data_color_space {
        // CMYK profiles are not supported for PNG.
        SIGNATURE_CMYK => false,
        SIGNATURE_GRAY => matches!(color, Color::Gray | Color::GrayAlpha),
        _ => true,
    }
}

/// Port of `SkCodecPriv::ComputeRowBytes`. Widths are checked to be positive by the header
/// reader, so the conversion to `usize` is exact.
// Port of: src/codec/SkCodecPriv.h#L217-L227 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // widths are positive
fn compute_row_bytes(width: i32, bits_per_pixel: u32) -> usize {
    let width = width as usize;
    if bits_per_pixel < 16 {
        let pixels_per_byte = (8 / bits_per_pixel) as usize;
        width.div_ceil(pixels_per_byte)
    } else {
        width * (bits_per_pixel / 8) as usize
    }
}

/// Port of `SkCodecPriv::ComputeRowBytesBytesPerPixel`.
// Port of: src/codec/SkCodecPriv.h#L207-L212 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // widths are positive
fn compute_row_bytes_bytes_per_pixel(width: i32, bytes_per_pixel: usize) -> usize {
    width as usize * bytes_per_pixel
}

impl PngCodecBase {
    /// The state before any header or transform is set up.
    #[must_use]
    pub(crate) fn new(chunk_reader: Arc<Mutex<PngCompositeChunkReader>>) -> Self {
        Self {
            xform_mode: XformMode::SwizzleOnly,
            swizzler: None,
            storage: Vec::new(),
            xform_width: -1,
            color_table: None,
            encoded_row_bytes: 0,
            dst_row_size: 0,
            dst_info_of_previous_color_table: None,
            chunk_reader,
        }
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L125-L136 (SkPngCodecBase::initializeXforms, the
    // frame-width part; the caller reads the header and updates libpng first)
    pub(crate) fn initialize_xforms(
        &mut self,
        base: &mut CodecBase<'_>,
        options: &Options,
        frame_width: i32,
        palette: &PaletteSource<'_>,
    ) -> Result {
        if frame_width != base.dst_info().width() && options.subset.is_some() {
            return Result::InvalidParameters;
        }
        self.xform_width = frame_width;

        // Port of: SkPngCodecBase.cpp#L137-L146 (the encoded row bytes)
        let encoded_bits_per_pixel = u32::from(base.encoded_info().bits_per_pixel());
        self.encoded_row_bytes = compute_row_bytes(frame_width, encoded_bits_per_pixel);

        // Reset the swizzler and the colour transform. They cannot be reset in onRewind because
        // the interlaced scanline decoder may need to rewind.
        self.swizzler = None;

        // If skcms directly supports the encoded PNG format, the swizzler can skip format
        // conversion (or skip swizzling altogether).
        let encoded = base.encoded_info();
        let skip_format_conversion = match encoded.color() {
            Color::RGB if encoded.bits_per_component() == 16 => base.color_xform(),
            Color::RGBA | Color::Gray => base.color_xform(),
            _ => false,
        };

        if skip_format_conversion && options.subset.is_none() {
            self.xform_mode = XformMode::ColorOnly;
        } else {
            if base.encoded_info().color() == Color::Palette {
                let dst_info = base.dst_info().clone();
                if !self.create_color_table(base, &dst_info, palette) {
                    return Result::InvalidInput;
                }
            }
            let result =
                self.initialize_swizzler(base, options, skip_format_conversion, frame_width);
            if result != Result::Success {
                return result;
            }
        }

        self.allocate_storage(base);

        // The transform parameters wait for `initialize_xform_params`: `swizzleWidth` may change
        // after `onStartIncrementalDecode`, when a sampled decode sets the sample factor.
        Result::Success
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L148-L174 (SkPngCodecBase::initializeXformParams)
    pub(crate) fn initialize_xform_params(&mut self, base: &CodecBase<'_>) {
        match self.xform_mode {
            XformMode::SwizzleOnly | XformMode::SwizzleColor => {
                if let Some(swizzler) = self.swizzler.as_ref() {
                    self.xform_width = swizzler.swizzle_width();
                }
            }
            XformMode::ColorOnly => {
                debug_assert!(self.swizzler.is_none());
            }
        }
        // The destination row size is only used in asserts in Skia; it is kept for the same
        // checks.
        self.dst_row_size =
            compute_row_bytes_bytes_per_pixel(self.xform_width, base.dst_info().bytes_per_pixel());
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L176-L201 (SkPngCodecBase::allocateStorage)
    fn allocate_storage(&mut self, base: &CodecBase<'_>) {
        match self.xform_mode {
            XformMode::SwizzleOnly => {}
            // A swizzler may still be created later when sampling, so the storage is allocated
            // for the swizzle as well.
            XformMode::ColorOnly | XformMode::SwizzleColor => {
                let bits_per_pixel = u32::from(base.encoded_info().bits_per_pixel());
                // With more than 8 bits per component, the extra precision is kept. Otherwise the
                // row is swizzled to RGBA_8888 before the transform.
                let bytes_per_pixel = if bits_per_pixel > 32 {
                    (bits_per_pixel / 8) as usize
                } else {
                    4
                };
                let width = usize::try_from(base.dst_info().width()).unwrap_or(0);
                self.storage = vec![0; width * bytes_per_pixel];
            }
        }
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L203-L244 (SkPngCodecBase::initializeSwizzler)
    fn initialize_swizzler(
        &mut self,
        base: &CodecBase<'_>,
        options: &Options,
        skip_format_conversion: bool,
        frame_width: i32,
    ) -> Result {
        let mut swizzler_info = base.dst_info().clone();
        let mut swizzler_options = options.clone();
        self.xform_mode = XformMode::SwizzleOnly;

        if base.color_xform() && base.xform_on_decode() {
            if base.encoded_info().color() == Color::Gray {
                swizzler_info = swizzler_info.with_color_type(ColorType::Gray8);
            } else {
                swizzler_info = swizzler_info.with_color_type(XFORM_SRC_COLOR_TYPE);
            }
            if base.dst_info().alpha_type() == AlphaType::Premul {
                swizzler_info = swizzler_info.with_alpha_type(AlphaType::Unpremul);
            }
            self.xform_mode = XformMode::SwizzleColor;
            // The swizzle writes into temporary memory, which is not zero initialized.
            swizzler_options.zero_initialized = ZeroInitialized::No;
        }

        // Without a subset the swizzler writes a single frame row; a subset decode has the frame
        // width of the destination.
        let frame_rect = if options.subset.is_some() {
            None
        } else {
            Some(IRect::from_wh(frame_width, 1))
        };

        if skip_format_conversion {
            // A colour table is never used together with format skipping.
            debug_assert!(self.color_table.is_none());
            let encoded = base.encoded_info();
            let src_bpp: usize = match encoded.color() {
                Color::RGB => {
                    debug_assert_eq!(encoded.bits_per_component(), 16);
                    6
                }
                Color::RGBA => (encoded.bits_per_component() / 2) as usize,
                Color::Gray => 1,
                _ => {
                    debug_assert!(false, "format skipping needs an RGB, RGBA or gray PNG");
                    0
                }
            };
            self.swizzler =
                Swizzler::make_simple(src_bpp, &swizzler_info, &swizzler_options, frame_rect);
        } else {
            self.swizzler = Swizzler::make(
                base.encoded_info(),
                self.color_table.as_deref(),
                &swizzler_info,
                &swizzler_options,
                frame_rect,
            );
        }

        if self.swizzler.is_some() {
            Result::Success
        } else {
            Result::Unimplemented
        }
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L246-L266 (SkPngCodecBase::applyXformRow)
    pub(crate) fn apply_xform_row(
        &mut self,
        base: &CodecBase<'_>,
        dst_row: &mut [u8],
        src_row: &[u8],
    ) {
        match self.xform_mode {
            XformMode::SwizzleOnly => {
                if let Some(swizzler) = self.swizzler.as_ref() {
                    swizzler.swizzle(dst_row, src_row);
                }
            }
            XformMode::ColorOnly => {
                base.apply_color_xform(
                    dst_row,
                    src_row,
                    usize::try_from(self.xform_width).unwrap_or(0),
                );
            }
            XformMode::SwizzleColor => {
                if let Some(swizzler) = self.swizzler.as_ref() {
                    swizzler.swizzle(&mut self.storage, src_row);
                }
                let width = usize::try_from(self.xform_width).unwrap_or(0);
                base.apply_color_xform(dst_row, &self.storage, width);
            }
        }
    }

    // Port of: src/codec/SkPngCodecBase.cpp#L268-L355 (SkPngCodecBase::createColorTable)
    // Port of: SkCodecPriv::ChoosePackColorProc (src/codec/SkCodecPriv.h#L326-L345)
    pub(crate) fn create_color_table(
        &mut self,
        base: &mut CodecBase<'_>,
        dst_info: &ImageInfo,
        palette: &PaletteSource<'_>,
    ) -> bool {
        if self
            .dst_info_of_previous_color_table
            .as_ref()
            .is_some_and(|previous| previous == dst_info)
        {
            return self.color_table.is_some();
        }
        self.color_table = None;
        self.dst_info_of_previous_color_table = Some(dst_info.clone());

        let Some(plte) = palette.plte else {
            return false;
        };
        // A PNG palette has at most 256 entries.
        let num_colors = plte.len().min(256);

        // The table holds an SkPMColor per entry. Its contents depend on the table colour type
        // and on whether the colours are premultiplied here.
        let mut color_table = vec![0u32; 256];
        let table_color_type = if base.color_xform() {
            XFORM_SRC_COLOR_TYPE
        } else {
            dst_info.color_type()
        };

        let num_colors_with_alpha = match palette.trns {
            Some(alphas) => alphas.len().min(num_colors),
            None => 0,
        };
        let should_apply_color_xform_to_table = base.color_xform() && !base.xform_on_decode();

        if let Some(alphas) = palette.trns {
            let premultiply = !should_apply_color_xform_to_table
                && needs_premul(dst_info.alpha_type(), base.encoded_info().alpha());
            let pack = choose_pack_color_proc(premultiply, table_color_type);
            for (i, entry) in plte.iter().enumerate().take(num_colors_with_alpha) {
                color_table[i] = pack(
                    u32::from(alphas[i]),
                    u32::from(entry.red),
                    u32::from(entry.green),
                    u32::from(entry.blue),
                );
            }
        }

        if num_colors_with_alpha < num_colors {
            // The entries without a tRNS alpha are opaque, in the table's channel order.
            let rgba = is_rgba(table_color_type);
            for (i, entry) in plte
                .iter()
                .enumerate()
                .take(num_colors)
                .skip(num_colors_with_alpha)
            {
                let (r, g, b) = (
                    u32::from(entry.red),
                    u32::from(entry.green),
                    u32::from(entry.blue),
                );
                color_table[i] = if rgba {
                    pack_argb_as_rgba(0xFF, r, g, b)
                } else {
                    pack_argb_as_bgra(0xFF, r, g, b)
                };
            }
        }

        if should_apply_color_xform_to_table {
            let src_bytes: Vec<u8> = color_table[..num_colors]
                .iter()
                .flat_map(|c| c.to_ne_bytes())
                .collect();
            let mut dst_bytes = vec![0u8; src_bytes.len()];
            base.apply_color_xform(&mut dst_bytes, &src_bytes, num_colors);
            for (entry, bytes) in color_table.iter_mut().zip(dst_bytes.as_chunks::<4>().0) {
                *entry = u32::from_ne_bytes(*bytes);
            }
        }

        // Pad the table with its last colour (or black) for indices past the palette. Black with
        // full alpha is 0xFF000000 in either channel order.
        let max_colors = 1usize << base.encoded_info().bits_per_component();
        if num_colors < max_colors {
            let last_color = if num_colors > 0 {
                color_table[num_colors - 1]
            } else {
                0xFF00_0000
            };
            for entry in &mut color_table[num_colors..max_colors] {
                *entry = last_color;
            }
        }

        color_table.truncate(max_colors);
        self.color_table = Some(color_table);
        true
    }
}

/// Port of `SkCodecPriv::PackColorProc`: packs an alpha and a colour into an `SkPMColor`.
type PackColorProc = fn(u32, u32, u32, u32) -> u32;

// Port of: src/codec/SkCodecPriv.h#L326-L345 (ChoosePackColorProc)
#[must_use]
fn choose_pack_color_proc(is_premul: bool, color_type: ColorType) -> PackColorProc {
    let rgba = is_rgba(color_type);
    match (is_premul, rgba) {
        (true, true) => premultiply_argb_as_rgba,
        (true, false) => premultiply_argb_as_bgra,
        (false, true) => pack_argb_as_rgba,
        (false, false) => pack_argb_as_bgra,
    }
}
