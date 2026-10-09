// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScalerContext.h (SkScalerContextRec and the flag enums)

//! [`ScalerContextRec`]: the record that describes how one scaler context rasterizes glyphs, and
//! its byte image in a descriptor (`SkScalerContextRec`).

use bitflags::bitflags;

use crate::color::Color;
use crate::font_types::FontHinting;
use crate::mask::MaskFormat;
use crate::scalar::scalar;

mod engine;
pub use engine::{
    AxisAlignment, GeneratedPath, GlyphMetrics, GlyphPathRasterizer, NO_PATH_RASTERIZER,
    NoPathRasterizer, PreMatrixScale, ScalerContext, ScalerContextBase, ScalerContextBuildFlags,
    ScalerContextEffects, ScalerContextImpl, cached_mask_gamma_for, get_gamma_lut_data,
    get_gamma_lut_size, get_mask_pre_blend, make_text_matrix,
};

/// `sizeof(SkScalerContextRec)` on every target. The record has no pointers and is dense
/// (`SK_BEGIN_REQUIRE_DENSE`), so the size is the same on 32-bit and 64-bit platforms.
// Port of: src/core/SkScalerContext.h#L68-L241 (SK_BEGIN/END_REQUIRE_DENSE, chrome/m156)
pub const SCALER_CONTEXT_REC_SIZE: usize = 56;

/// `SK_ColorBLACK`, the default foreground color of a record.
const COLOR_BLACK: u32 = 0xFF00_0000;

bitflags! {
    /// Flags of a [`ScalerContextRec`] (`SkScalerContext::Flags`). Stored in a `u16`. The two
    /// hinting bits together hold a [`FontHinting`] value, see
    /// [`ScalerContextRec::hinting`].
    // Port of: src/core/SkScalerContext.h#L258-L285 (chrome/m156)
    #[doc(alias = "SkScalerContext::Flags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct ScalerContextFlags: u16 {
        /// Draws the frame and fill separately (`kFrameAndFill_Flag`).
        const FRAME_AND_FILL = 0x0001;
        /// `kEmbeddedBitmapText_Flag`.
        const EMBEDDED_BITMAP_TEXT = 0x0004;
        /// `kEmbolden_Flag`: fake bold.
        const EMBOLDEN = 0x0008;
        /// `kSubpixelPositioning_Flag`.
        const SUBPIXEL_POSITIONING = 0x0010;
        /// `kForceAutohinting_Flag`: use auto instead of bytecode hinting.
        const FORCE_AUTOHINTING = 0x0020;
        /// `kHintingBit1_Flag`: bit 0 of the hinting value.
        const HINTING_BIT1 = 0x0080;
        /// `kHintingBit2_Flag`: bit 1 of the hinting value.
        const HINTING_BIT2 = 0x0100;
        /// `kLCD_Vertical_Flag`: LCD stripes are vertical (only for LCD16).
        const LCD_VERTICAL = 0x0200;
        /// `kLCD_BGROrder_Flag`: LCD order is BGR (only for LCD16).
        const LCD_BGR_ORDER = 0x0400;
        /// `kGenA8FromLCD_Flag`: generate A8 from an LCD source (only for A8).
        const GEN_A8_FROM_LCD = 0x0800;
        /// `kLinearMetrics_Flag`.
        const LINEAR_METRICS = 0x1000;
        /// `kBaselineSnap_Flag`.
        const BASELINE_SNAP = 0x2000;
        /// `kNeedsForegroundColor_Flag`.
        const NEEDS_FOREGROUND_COLOR = 0x4000;
        /// `kHinting_Mask`: both hinting bits.
        const HINTING_MASK = 0x0080 | 0x0100;
    }
}

/// `SkScalerContext::kHinting_Shift`: the hinting value starts at this bit.
// Port of: src/core/SkScalerContext.h#L268 (chrome/m156)
const HINTING_SHIFT: u32 = 7;

/// The record that describes one scaler context (`SkScalerContextRec`).
///
/// Its byte image ([`ScalerContextRec::to_bytes`]) is the `kRec_SkDescriptorTag` entry of a
/// [`Descriptor`](crate::descriptor::Descriptor). The layout is the C++ one: fields in declaration
/// order, the device gamma, contrast and the stroke join and cap packed as C++ packs them, and the
/// reserved bytes zero:
///
/// | offset | size | field |
/// |---|---|---|
/// | 0 | 4 | `typeface_id` |
/// | 4 | 12 | `text_size`, `pre_scale_x`, `pre_skew_x` |
/// | 16 | 16 | `post_2x2` (row-major) |
/// | 32 | 8 | `frame_width`, `miter_limit` |
/// | 40 | 4 | `foreground_color` |
/// | 44 | 4 | luminance color (`lum_bits`) |
/// | 48 | 1 | device gamma (2.6 fixed point) |
/// | 49 | 1 | reserved, zero |
/// | 50 | 1 | contrast (0.8 fixed point) |
/// | 51 | 1 | reserved, zero |
/// | 52 | 1 | mask format |
/// | 53 | 1 | stroke join (low nibble), stroke cap (high nibble) |
/// | 54 | 2 | flags |
///
/// Integers and floats use the host byte order, as the C++ memory image does.
// Port of: src/core/SkScalerContext.h#L69-L240 (chrome/m156)
#[doc(alias = "SkScalerContextRec")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ScalerContextRec {
    /// Unique id of the typeface (`SkTypefaceID`).
    pub typeface_id: u32,
    /// Text size, before the pre-scale and the 2x2 matrix.
    pub text_size: scalar,
    /// Pre-scale in x.
    pub pre_scale_x: scalar,
    /// Pre-skew in x.
    pub pre_skew_x: scalar,
    /// The 2x2 post matrix, `[row][column]`.
    pub post_2x2: [[scalar; 2]; 2],
    /// Frame width for stroked and framed glyphs.
    pub frame_width: scalar,
    /// Miter limit for stroked glyphs.
    pub miter_limit: scalar,
    /// Foreground color, `0xAARRGGBB`.
    pub foreground_color: u32,
    /// Luminance color packed as `0xAARRGGBB`, set by `setLuminanceColor`.
    lum_bits: u32,
    /// Device gamma in 2.6 fixed point, `0` for sRGB.
    device_gamma: u8,
    /// Contrast in 0.8 fixed point.
    contrast: u8,
    /// The mask format of the glyph images this context produces.
    pub mask_format: MaskFormat,
    /// Stroke join (low nibble) and cap (high nibble), `SkPaint` values.
    stroke_join_cap: u8,
    /// The [`ScalerContextFlags`].
    pub flags: ScalerContextFlags,
}

impl Default for ScalerContextRec {
    /// `SkScalerContextRec rec;`: every field zero, except the foreground color, which defaults
    /// to `SK_ColorBLACK`.
    // Port of: src/core/SkScalerContext.h#L78 (chrome/m156)
    fn default() -> Self {
        Self {
            typeface_id: 0,
            text_size: 0.0,
            pre_scale_x: 0.0,
            pre_skew_x: 0.0,
            post_2x2: [[0.0; 2]; 2],
            frame_width: 0.0,
            miter_limit: 0.0,
            foreground_color: COLOR_BLACK,
            lum_bits: 0,
            device_gamma: 0,
            contrast: 0,
            mask_format: MaskFormat::BW,
            stroke_join_cap: 0,
            flags: ScalerContextFlags::empty(),
        }
    }
}

impl ScalerContextRec {
    /// `ScalerContextRec::ExternalGammaFromInternal`.
    // Port of: src/core/SkScalerContext.h#L88-L90 (chrome/m156)
    #[must_use]
    pub fn external_gamma_from_internal(g: u8) -> scalar {
        f32::from(g) / f32::from(1u8 << 6)
    }

    /// `ScalerContextRec::InternalGammaFromExternal`.
    // Port of: src/core/SkScalerContext.h#L91-L94 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors static_cast<uint8_t> in ported arithmetic
    pub fn internal_gamma_from_external(g: scalar) -> u8 {
        (g * f32::from(1u8 << 6)) as u8
    }

    /// `ScalerContextRec::ExternalContrastFromInternal`.
    // Port of: src/core/SkScalerContext.h#L95-L97 (chrome/m156)
    #[must_use]
    pub fn external_contrast_from_internal(c: u8) -> scalar {
        f32::from(c) / f32::from((1u16 << 8) - 1)
    }

    /// `ScalerContextRec::InternalContrastFromExternal`.
    // Port of: src/core/SkScalerContext.h#L98-L101 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors static_cast<uint8_t> in ported arithmetic
    pub fn internal_contrast_from_external(c: scalar) -> u8 {
        (c * f32::from((1u16 << 8) - 1) + 0.5) as u8
    }

    /// `ScalerContextRec::setDeviceGamma`. The range is checked in debug builds, as the C++ assert
    /// checks it.
    // Port of: src/core/SkScalerContext.h#L103-L108 (chrome/m156)
    pub fn set_device_gamma(&mut self, g: scalar) {
        debug_assert!((0.0..4.0).contains(&g));
        self.device_gamma = Self::internal_gamma_from_external(g);
    }

    /// The device gamma, as the 2.6 fixed-point byte stored in the record.
    // Port of: src/core/SkScalerContext.h#L83 (fDeviceGamma, chrome/m156)
    #[must_use]
    pub fn device_gamma(&self) -> u8 {
        self.device_gamma
    }

    /// `ScalerContextRec::setContrast`. The range is checked in debug builds.
    // Port of: src/core/SkScalerContext.h#L110-L115 (chrome/m156)
    pub fn set_contrast(&mut self, c: scalar) {
        debug_assert!((0.0..=1.0).contains(&c));
        self.contrast = Self::internal_contrast_from_external(c);
    }

    /// The contrast, as the 0.8 fixed-point byte stored in the record.
    // Port of: src/core/SkScalerContext.h#L85 (fContrast, chrome/m156)
    #[must_use]
    pub fn contrast(&self) -> u8 {
        self.contrast
    }

    /// `ScalerContextRec::getHinting`: the two hinting bits as a [`FontHinting`].
    // Port of: src/core/SkScalerContext.h#L488-L493 (chrome/m156)
    #[must_use]
    pub fn hinting(&self) -> FontHinting {
        let hint = (self.flags.bits() & ScalerContextFlags::HINTING_MASK.bits()) >> HINTING_SHIFT;
        match hint {
            0 => FontHinting::None,
            1 => FontHinting::Slight,
            2 => FontHinting::Normal,
            _ => FontHinting::Full,
        }
    }

    /// `ScalerContextRec::setHinting`: replaces the two hinting bits.
    // Port of: src/core/SkScalerContext.h#L494-L497 (chrome/m156)
    pub fn set_hinting(&mut self, hinting: FontHinting) {
        let value = hinting as u16;
        let bits = (self.flags.bits() & !ScalerContextFlags::HINTING_MASK.bits())
            | (value << HINTING_SHIFT);
        self.flags = ScalerContextFlags::from_bits_retain(bits);
    }

    /// `ScalerContextRec::getFormat`.
    // Port of: src/core/SkScalerContext.h#L225-L227 (chrome/m156)
    #[must_use]
    pub fn format(&self) -> MaskFormat {
        self.mask_format
    }

    /// `ScalerContextRec::getLuminanceColor`.
    // Port of: src/core/SkScalerContext.h#L229-L231 (chrome/m156)
    #[must_use]
    pub fn luminance_color(&self) -> Color {
        Color::new(self.lum_bits)
    }

    /// The stroke join, `SkPaint::Join` as its low nibble.
    // Port of: src/core/SkScalerContext.h#L146 (fStrokeJoin, chrome/m156)
    #[must_use]
    pub fn stroke_join(&self) -> u8 {
        self.stroke_join_cap & 0x0F
    }

    /// The stroke cap, `SkPaint::Cap` as its high nibble.
    // Port of: src/core/SkScalerContext.h#L147 (fStrokeCap, chrome/m156)
    #[must_use]
    pub fn stroke_cap(&self) -> u8 {
        self.stroke_join_cap >> 4
    }

    /// Sets the stroke join and cap. Each is truncated to four bits, as the C++ bit-fields are.
    // Port of: src/core/SkScalerContext.h#L146-L147 (chrome/m156)
    pub fn set_stroke_join_cap(&mut self, join: u8, cap: u8) {
        self.stroke_join_cap = (join & 0x0F) | ((cap & 0x0F) << 4);
    }

    /// The byte image of the record, as stored in the `kRec_SkDescriptorTag` entry.
    // Port of: src/core/SkScalerContext.h#L69-L240 (memory image of SkScalerContextRec)
    #[must_use]
    pub fn to_bytes(&self) -> [u8; SCALER_CONTEXT_REC_SIZE] {
        let mut out = [0u8; SCALER_CONTEXT_REC_SIZE];
        let mut put =
            |offset: usize, bytes: [u8; 4]| out[offset..offset + 4].copy_from_slice(&bytes);
        put(0, self.typeface_id.to_ne_bytes());
        put(4, self.text_size.to_ne_bytes());
        put(8, self.pre_scale_x.to_ne_bytes());
        put(12, self.pre_skew_x.to_ne_bytes());
        put(16, self.post_2x2[0][0].to_ne_bytes());
        put(20, self.post_2x2[0][1].to_ne_bytes());
        put(24, self.post_2x2[1][0].to_ne_bytes());
        put(28, self.post_2x2[1][1].to_ne_bytes());
        put(32, self.frame_width.to_ne_bytes());
        put(36, self.miter_limit.to_ne_bytes());
        put(40, self.foreground_color.to_ne_bytes());
        put(44, self.lum_bits.to_ne_bytes());
        out[48] = self.device_gamma;
        out[49] = 0; // fReservedAlign2
        out[50] = self.contrast;
        out[51] = 0; // fReservedAlign
        out[52] = self.mask_format as u8;
        out[53] = self.stroke_join_cap;
        out[54..56].copy_from_slice(&self.flags.bits().to_ne_bytes());
        out
    }

    /// Reads a record back from its byte image. The reserved bytes are ignored.
    /// # Panics
    ///
    /// Does not panic: every offset is a constant inside the array.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; SCALER_CONTEXT_REC_SIZE]) -> Self {
        let f32_at = |o: usize| f32::from_ne_bytes(bytes[o..o + 4].try_into().expect("4 bytes"));
        let u32_at = |o: usize| u32::from_ne_bytes(bytes[o..o + 4].try_into().expect("4 bytes"));
        Self {
            typeface_id: u32_at(0),
            text_size: f32_at(4),
            pre_scale_x: f32_at(8),
            pre_skew_x: f32_at(12),
            post_2x2: [[f32_at(16), f32_at(20)], [f32_at(24), f32_at(28)]],
            frame_width: f32_at(32),
            miter_limit: f32_at(36),
            foreground_color: u32_at(40),
            lum_bits: u32_at(44),
            device_gamma: bytes[48],
            contrast: bytes[50],
            mask_format: match bytes[52] {
                0 => MaskFormat::BW,
                1 => MaskFormat::A8,
                2 => MaskFormat::ThreeD,
                3 => MaskFormat::Argb32,
                4 => MaskFormat::Lcd16,
                _ => MaskFormat::Sdf,
            },
            stroke_join_cap: bytes[53],
            flags: ScalerContextFlags::from_bits_retain(u16::from_ne_bytes([bytes[54], bytes[55]])),
        }
    }
}
