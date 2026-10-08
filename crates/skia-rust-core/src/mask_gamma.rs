// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMaskGamma.h, src/core/SkMaskGamma.cpp

//! Gamma-correcting mask tables (`SkMaskGamma`), and the color-space luminance functions they are
//! built from (`SkColorSpaceLuminance`).
//!
//! A regular glyph mask holds linear alpha. A gamma-correcting mask holds alpha that, once blended
//! linearly, looks as if it had been blended in the device's gamma. [`TMaskGamma`] builds one
//! 256-entry table per canonical luminance, and [`PreBlend`] selects the three tables for a color.

use std::sync::Arc;

use crate::color::Color;
use crate::color_data::{ITU_BT709_LUM_COEFF_B, ITU_BT709_LUM_COEFF_G, ITU_BT709_LUM_COEFF_R};
use crate::floating_point::float_round2int;
use crate::scalar::{SCALAR_1, scalar, scalar_round_to_int};

/// Converts luminances to and from linear and perceptual (color space) values
/// (`SkColorSpaceLuminance`). Luma is a linear luminance in `[0, 1]`; luminance is in an
/// arbitrary color space in `[0, 1]`.
// Port of: src/core/SkMaskGamma.h#L31-L56 (chrome/m156)
#[doc(alias = "SkColorSpaceLuminance")]
pub trait ColorSpaceLuminance: Send + Sync {
    /// Converts a color component luminance in the color space to a linear luma.
    // Port of: src/core/SkMaskGamma.h#L36-L37 (chrome/m156)
    fn to_luma(&self, gamma: scalar, luminance: scalar) -> scalar;

    /// Converts a linear luma to a color component luminance in the color space.
    // Port of: src/core/SkMaskGamma.h#L38-L39 (chrome/m156)
    #[allow(clippy::wrong_self_convention)] // the name mirrors SkColorSpaceLuminance::fromLuma
    fn from_luma(&self, gamma: scalar, luma: scalar) -> scalar;
}

/// `SkLinearColorSpaceLuminance`: gamma 1, so luminance and luma are the same.
// Port of: src/core/SkMaskGamma.cpp#L17-L26 (chrome/m156)
#[derive(Debug)]
struct LinearColorSpaceLuminance;

impl ColorSpaceLuminance for LinearColorSpaceLuminance {
    fn to_luma(&self, _gamma: scalar, luminance: scalar) -> scalar {
        luminance
    }

    #[allow(clippy::wrong_self_convention)] // mirrors SkColorSpaceLuminance::fromLuma
    fn from_luma(&self, _gamma: scalar, luma: scalar) -> scalar {
        luma
    }
}

/// `SkGammaColorSpaceLuminance`: a plain power curve with the given gamma.
// Port of: src/core/SkMaskGamma.cpp#L28-L35 (chrome/m156)
#[derive(Debug)]
struct GammaColorSpaceLuminance;

impl ColorSpaceLuminance for GammaColorSpaceLuminance {
    fn to_luma(&self, gamma: scalar, luminance: scalar) -> scalar {
        luminance.powf(gamma)
    }

    fn from_luma(&self, gamma: scalar, luma: scalar) -> scalar {
        luma.powf(SCALAR_1 / gamma)
    }
}

/// `SkSRGBColorSpaceLuminance`: the sRGB transfer curve (gamma 0 selects it).
// Port of: src/core/SkMaskGamma.cpp#L37-L58 (chrome/m156)
#[derive(Debug)]
struct SrgbColorSpaceLuminance;

impl ColorSpaceLuminance for SrgbColorSpaceLuminance {
    fn to_luma(&self, _gamma: scalar, luminance: scalar) -> scalar {
        if luminance <= 0.04045_f32 {
            return luminance / 12.92_f32;
        }
        ((luminance + 0.055_f32) / 1.055_f32).powf(2.4_f32)
    }

    #[allow(clippy::wrong_self_convention)] // mirrors SkColorSpaceLuminance::fromLuma
    fn from_luma(&self, _gamma: scalar, luma: scalar) -> scalar {
        if luma <= 0.003_130_8_f32 {
            return luma * 12.92_f32;
        }
        1.055_f32 * luma.powf(SCALAR_1 / 2.4_f32) - 0.055_f32
    }
}

static LINEAR: LinearColorSpaceLuminance = LinearColorSpaceLuminance;
static GAMMA: GammaColorSpaceLuminance = GammaColorSpaceLuminance;
static SRGB: SrgbColorSpaceLuminance = SrgbColorSpaceLuminance;

/// The luminance functions for `gamma` (`SkColorSpaceLuminance::Fetch`): sRGB for 0, linear for
/// 1, and a power curve otherwise.
// Port of: src/core/SkMaskGamma.cpp#L60-L72 (chrome/m156)
#[doc(alias = "SkColorSpaceLuminance::Fetch")]
#[must_use]
#[allow(clippy::float_cmp)] // gamma is compared exactly, as in Skia
pub fn fetch_color_space_luminance(gamma: scalar) -> &'static dyn ColorSpaceLuminance {
    if gamma == 0.0 {
        &SRGB
    } else if gamma == SCALAR_1 {
        &LINEAR
    } else {
        &GAMMA
    }
}

/// `SkColorSpaceLuminance::computeLuminance`: the luminance of `color` in a color space of
/// `gamma`, scaled to `[0, 255]`.
// Port of: src/core/SkMaskGamma.h#L42-L53 (chrome/m156)
#[doc(alias = "SkColorSpaceLuminance::computeLuminance")]
#[must_use]
#[allow(clippy::cast_sign_loss)] // mirrors the C++ conversion of the rounded value to U8CPU
pub fn compute_luminance(gamma: scalar, color: Color) -> u32 {
    let luminance = fetch_color_space_luminance(gamma);
    let r = luminance.to_luma(gamma, f32::from(color.r()) / 255.0);
    let g = luminance.to_luma(gamma, f32::from(color.g()) / 255.0);
    let b = luminance.to_luma(gamma, f32::from(color.b()) / 255.0);
    let luma = r * ITU_BT709_LUM_COEFF_R + g * ITU_BT709_LUM_COEFF_G + b * ITU_BT709_LUM_COEFF_B;
    debug_assert!(luma <= SCALAR_1);
    scalar_round_to_int(luminance.from_luma(gamma, luma) * 255.0) as u32
}

/// Scales `base`, which has `N` bits (`1..=8`), to `0..=255` by replicating its bits
/// (`sk_t_scale255`).
// Port of: src/core/SkMaskGamma.h#L64-L80 (chrome/m156)
#[must_use]
pub fn sk_t_scale255<const N: u32>(base: u32) -> u32 {
    // The C++ specializations for 1, 2, 4 and 8 are the same values as the loop below; they are
    // written out as the same multiplications.
    match N {
        1 => base * 0xFF,
        2 => base * 0x55,
        4 => base * 0x11,
        8 => base,
        _ => {
            let base = base << (8 - N);
            let mut lum = base;
            let mut i = N;
            while i < 8 {
                lum |= base >> i;
                i += N;
            }
            lum
        }
    }
}

/// `SkTMaskGamma_build_correcting_lut`: fills one 256-entry table for the source luminance
/// `src_i` (0..=255) and the device gamma.
// Port of: src/core/SkMaskGamma.cpp#L78-L128 (chrome/m156)
#[allow(clippy::float_cmp, clippy::cast_precision_loss)] // mirrors the C++ float arithmetic; src_i <= 255 is exact in f32
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // SkToU8: static_cast<uint8_t>
fn build_correcting_lut(
    table: &mut [u8],
    src_i: u32,
    contrast: scalar,
    dst_convert: &dyn ColorSpaceLuminance,
    dst_gamma: scalar,
) {
    let src_convert = dst_convert;
    let src_gamma = dst_gamma;
    let src: f32 = src_i as f32 / 255.0;
    let lin_src = src_convert.to_luma(src_gamma, src);
    let dst: f32 = 1.0 - src;
    let lin_dst = dst_convert.to_luma(dst_gamma, dst);
    let adjusted_contrast = contrast * lin_dst;

    if (src - dst).abs() < 1.0 / 256.0 {
        let mut ii = 0.0_f32;
        for entry in table.iter_mut().take(256) {
            let raw_srca = ii / 255.0;
            let srca = apply_contrast(raw_srca, adjusted_contrast);
            *entry = float_round2int(255.0 * srca) as u8;
            ii += 1.0;
        }
    } else {
        let mut ii = 0.0_f32;
        for entry in table.iter_mut().take(256) {
            let raw_srca = ii / 255.0;
            let srca = apply_contrast(raw_srca, adjusted_contrast);
            let dsta = 1.0 - srca;
            let lin_out = lin_src * srca + dsta * lin_dst;
            let out = dst_convert.from_luma(dst_gamma, lin_out);
            let result = (out - dst) / (src - dst);
            *entry = float_round2int(255.0 * result) as u8;
            ii += 1.0;
        }
    }
}

/// `apply_contrast`: adds artificial contrast to a linear alpha value.
// Port of: src/core/SkMaskGamma.cpp#L74-L76 (chrome/m156)
fn apply_contrast(srca: f32, contrast: f32) -> f32 {
    srca + ((1.0 - srca) * contrast * srca)
}

/// Gamma-correcting alpha tables for masks, with `R`, `G` and `B` luminance bits per channel
/// (`SkTMaskGamma`). A default value is linear and has no tables.
///
/// `R`, `G` and `B` are in `1..=8`. The scaler context uses `TMaskGamma<3, 3, 3>`, the
/// [`MaskGamma`] alias.
// Port of: src/core/SkMaskGamma.h#L102-L199 (chrome/m156)
#[doc(alias = "SkTMaskGamma")]
#[derive(Debug, Default)]
pub struct TMaskGamma<const R: u32, const G: u32, const B: u32> {
    /// `fGammaTables`: `kNumTables` rows of `kTableWidth` bytes. `None` for a linear table.
    gamma_tables: Option<Box<[u8]>>,
}

impl<const R: u32, const G: u32, const B: u32> TMaskGamma<R, G, B> {
    /// `kMaxLumBits`.
    // Port of: src/core/SkMaskGamma.h#L173 (chrome/m156)
    const MAX_LUM_BITS: u32 = max3(R, G, B);
    /// `kNumTables`: one table per canonical value of the widest channel.
    // Port of: src/core/SkMaskGamma.h#L174 (chrome/m156)
    const NUM_TABLES: usize = 1 << Self::MAX_LUM_BITS;
    /// `kTableWidth`.
    // Port of: src/core/SkMaskGamma.h#L175 (chrome/m156)
    const TABLE_WIDTH: usize = 256;
    /// `kTableNumElements`.
    // Port of: src/core/SkMaskGamma.h#L176 (chrome/m156)
    const TABLE_NUM_ELEMENTS: usize = Self::NUM_TABLES * Self::TABLE_WIDTH;

    /// `SkTMaskGamma(contrast, deviceGamma)`: builds the tables for the device gamma and contrast
    /// (`contrast` is in `[0, 1]`).
    // Port of: src/core/SkMaskGamma.h#L117-L126 (chrome/m156)
    #[must_use]
    pub fn new(contrast: scalar, device_gamma: scalar) -> Self {
        let mut tables = vec![0u8; Self::TABLE_NUM_ELEMENTS].into_boxed_slice();
        let device_convert = fetch_color_space_luminance(device_gamma);
        for i in 0..(1_u32 << Self::MAX_LUM_BITS) {
            let lum = sk_t_scale255_bits(Self::MAX_LUM_BITS, i);
            let index = i as usize;
            let row = &mut tables[index * Self::TABLE_WIDTH..(index + 1) * Self::TABLE_WIDTH];
            build_correcting_lut(row, lum, contrast, device_convert, device_gamma);
        }
        Self {
            gamma_tables: Some(tables),
        }
    }

    /// `SkTMaskGamma::CanonicalColor`: the closest color that has the same luminance bits.
    // Port of: src/core/SkMaskGamma.h#L129-L134 (chrome/m156)
    #[must_use]
    pub fn canonical_color(color: Color) -> Color {
        let r = sk_t_scale255_bits(R, u32::from(color.r()) >> (8 - R));
        let g = sk_t_scale255_bits(G, u32::from(color.g()) >> (8 - G));
        let b = sk_t_scale255_bits(B, u32::from(color.b()) >> (8 - B));
        // SkColorSetRGB(r, g, b) == SkColorSetARGB(0xFF, r, g, b)
        Color::new((0xFF << 24) | (r << 16) | (g << 8) | b)
    }

    /// `SkTMaskGamma::getGammaTableDimensions`: `(tableWidth, numTables)`.
    // Port of: src/core/SkMaskGamma.h#L150-L153 (chrome/m156)
    #[must_use]
    pub const fn gamma_table_dimensions() -> (usize, usize) {
        (Self::TABLE_WIDTH, Self::NUM_TABLES)
    }

    /// `SkTMaskGamma::getGammaTableSizeInBytes`.
    // Port of: src/core/SkMaskGamma.h#L159-L161 (chrome/m156)
    #[must_use]
    pub const fn gamma_table_size_in_bytes() -> usize {
        Self::TABLE_NUM_ELEMENTS
    }

    /// `SkTMaskGamma::getGammaTables`: the flattened tables, or `None` for a linear value.
    // Port of: src/core/SkMaskGamma.h#L168-L170 (chrome/m156)
    #[must_use]
    pub fn gamma_tables(&self) -> Option<&[u8]> {
        self.gamma_tables.as_deref()
    }

    /// `SkTMaskGamma::preBlend`: the tables for the channels of `color`. A linear value gives a
    /// [`PreBlend`] that is not applicable.
    // Port of: src/core/SkMaskGamma.h#L229-L246 (chrome/m156)
    #[must_use]
    pub fn pre_blend(self: &Arc<Self>, color: Color) -> PreBlend<R, G, B> {
        if self.gamma_tables.is_none() {
            return PreBlend::not_applicable();
        }
        let lum_shift = 8 - Self::MAX_LUM_BITS;
        let r_index = (usize::from(color.r()) >> lum_shift) * Self::TABLE_WIDTH;
        let g_index = (usize::from(color.g()) >> lum_shift) * Self::TABLE_WIDTH;
        let b_index = (usize::from(color.b()) >> lum_shift) * Self::TABLE_WIDTH;
        debug_assert!(
            r_index < Self::TABLE_NUM_ELEMENTS
                && g_index < Self::TABLE_NUM_ELEMENTS
                && b_index < Self::TABLE_NUM_ELEMENTS
        );
        PreBlend {
            parent: Some(Arc::clone(self)),
            r: r_index,
            g: g_index,
            b: b_index,
        }
    }
}

/// The mask gamma of the scaler context, `SkMaskGamma` (`SkTMaskGamma<3, 3, 3>`).
// Port of: src/core/SkScalerContext.h#L51 (typedef SkTMaskGamma<3, 3, 3> SkMaskGamma, chrome/m156)
#[doc(alias = "SkMaskGamma")]
pub type MaskGamma = TMaskGamma<3, 3, 3>;

/// The three per-channel tables that convert linear alpha to gamma-correct alpha for one color
/// (`SkTMaskPreBlend`). Immutable. The parent keeps the tables alive.
///
/// When the value is not applicable, [`PreBlend::r`], [`PreBlend::g`] and [`PreBlend::b`] are
/// `None` (the C++ pointers are null).
// Port of: src/core/SkMaskGamma.h#L200-L227 (chrome/m156)
#[doc(alias = "SkTMaskPreBlend")]
#[derive(Debug, Clone, Default)]
pub struct PreBlend<const R: u32, const G: u32, const B: u32> {
    parent: Option<Arc<TMaskGamma<R, G, B>>>,
    r: usize,
    g: usize,
    b: usize,
}

/// The pre-blend of the scaler context, `SkMaskPreBlend` (`SkTMaskPreBlend<3, 3, 3>`).
#[doc(alias = "SkMaskGamma::PreBlend")]
pub type MaskPreBlend = PreBlend<3, 3, 3>;

impl<const R: u32, const G: u32, const B: u32> PreBlend<R, G, B> {
    /// `SkTMaskPreBlend()`: not applicable.
    // Port of: src/core/SkMaskGamma.h#L209-L210 (chrome/m156)
    #[must_use]
    pub fn not_applicable() -> Self {
        Self {
            parent: None,
            r: 0,
            g: 0,
            b: 0,
        }
    }

    /// `SkTMaskPreBlend::isApplicable`: true when there are tables to apply.
    // Port of: src/core/SkMaskGamma.h#L222 (chrome/m156)
    #[must_use]
    pub fn is_applicable(&self) -> bool {
        self.parent.is_some()
    }

    /// The red table: 256 entries. `None` when not applicable.
    // Port of: src/core/SkMaskGamma.h#L224 (fR, chrome/m156)
    #[must_use]
    pub fn r(&self) -> Option<&[u8]> {
        self.table(self.r)
    }

    /// The green table: 256 entries. `None` when not applicable.
    // Port of: src/core/SkMaskGamma.h#L225 (fG, chrome/m156)
    #[must_use]
    pub fn g(&self) -> Option<&[u8]> {
        self.table(self.g)
    }

    /// The blue table: 256 entries. `None` when not applicable.
    // Port of: src/core/SkMaskGamma.h#L226 (fB, chrome/m156)
    #[must_use]
    pub fn b(&self) -> Option<&[u8]> {
        self.table(self.b)
    }

    fn table(&self, offset: usize) -> Option<&[u8]> {
        let tables = self.parent.as_ref()?.gamma_tables.as_deref()?;
        tables.get(offset..offset + TMaskGamma::<R, G, B>::TABLE_WIDTH)
    }
}

/// `SkTMaskGamma`'s `MAX` over the three bit counts, usable in a `const` item.
const fn max3(a: u32, b: u32, c: u32) -> u32 {
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
}

/// `sk_t_scale255<bits>(base)` where `bits` is a run-time value: `kMaxLumBits`, `R_LUM_BITS`
/// and the like are const generics here, and stable Rust cannot pass them as a const argument of
/// `sk_t_scale255`.
fn sk_t_scale255_bits(bits: u32, base: u32) -> u32 {
    match bits {
        1 => sk_t_scale255::<1>(base),
        2 => sk_t_scale255::<2>(base),
        3 => sk_t_scale255::<3>(base),
        4 => sk_t_scale255::<4>(base),
        5 => sk_t_scale255::<5>(base),
        6 => sk_t_scale255::<6>(base),
        7 => sk_t_scale255::<7>(base),
        _ => sk_t_scale255::<8>(base),
    }
}
