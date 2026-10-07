// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkColorData.h

//! Packed-pixel color helpers (`SkColorData.h`): 565 / 4444 conversions, 4-byte interpolation,
//! splay/unsplay and the premultiplied float color constants.
//!
//! The `SkPMColor` byte order is fixed to `RGBA`; see [`crate::color_priv`].

use crate::color::{Color, PMColor, PMColor4f};
use crate::color_priv::{
    A32_SHIFT, B32_BITS, B32_SHIFT, G32_BITS, G32_SHIFT, R32_BITS, R32_SHIFT, alpha_255_to_256,
    alpha_mul, get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
};
use crate::floating_point::FLOAT_NEGATIVE_INFINITY;
use crate::math::{U8CPU, U16CPU};
use crate::to::{to_u8, to_u16};

// Convert a 16bit pixel to a 32bit pixel

#[doc(alias = "SK_R16_BITS")]
pub const R16_BITS: u32 = 5;
#[doc(alias = "SK_G16_BITS")]
pub const G16_BITS: u32 = 6;
#[doc(alias = "SK_B16_BITS")]
pub const B16_BITS: u32 = 5;

#[doc(alias = "SK_R16_SHIFT")]
pub const R16_SHIFT: u32 = B16_BITS + G16_BITS;
#[doc(alias = "SK_G16_SHIFT")]
pub const G16_SHIFT: u32 = B16_BITS;
#[doc(alias = "SK_B16_SHIFT")]
pub const B16_SHIFT: u32 = 0;

#[doc(alias = "SK_R16_MASK")]
pub const R16_MASK: u32 = (1 << R16_BITS) - 1;
#[doc(alias = "SK_G16_MASK")]
pub const G16_MASK: u32 = (1 << G16_BITS) - 1;
#[doc(alias = "SK_B16_MASK")]
pub const B16_MASK: u32 = (1 << B16_BITS) - 1;

#[doc(alias = "SK_R16_MASK_IN_PLACE")]
pub const R16_MASK_IN_PLACE: u32 = R16_MASK << R16_SHIFT;
#[doc(alias = "SK_G16_MASK_IN_PLACE")]
pub const G16_MASK_IN_PLACE: u32 = G16_MASK << G16_SHIFT;
#[doc(alias = "SK_B16_MASK_IN_PLACE")]
pub const B16_MASK_IN_PLACE: u32 = B16_MASK << B16_SHIFT;

/// Red of a 565 pixel.
// Port of: src/core/SkColorData.h#L31 (chrome/m156)
#[doc(alias = "SkGetPackedR16")]
#[must_use]
pub const fn get_packed_r16(color: u32) -> u32 {
    (color >> R16_SHIFT) & R16_MASK
}

/// Green of a 565 pixel.
// Port of: src/core/SkColorData.h#L32 (chrome/m156)
#[doc(alias = "SkGetPackedG16")]
#[must_use]
pub const fn get_packed_g16(color: u32) -> u32 {
    (color >> G16_SHIFT) & G16_MASK
}

/// Blue of a 565 pixel.
// Port of: src/core/SkColorData.h#L33 (chrome/m156)
#[doc(alias = "SkGetPackedB16")]
#[must_use]
pub const fn get_packed_b16(color: u32) -> u32 {
    (color >> B16_SHIFT) & B16_MASK
}

// Port of: src/core/SkColorData.h#L35-L37 (chrome/m156)
#[doc(alias = "SkR16ToR32")]
#[must_use]
pub const fn r16_to_r32(r: u32) -> u32 {
    (r << (8 - R16_BITS)) | (r >> (2 * R16_BITS - 8))
}

// Port of: src/core/SkColorData.h#L39-L41 (chrome/m156)
#[doc(alias = "SkG16ToG32")]
#[must_use]
pub const fn g16_to_g32(g: u32) -> u32 {
    (g << (8 - G16_BITS)) | (g >> (2 * G16_BITS - 8))
}

// Port of: src/core/SkColorData.h#L43-L45 (chrome/m156)
#[doc(alias = "SkB16ToB32")]
#[must_use]
pub const fn b16_to_b32(b: u32) -> u32 {
    (b << (8 - B16_BITS)) | (b >> (2 * B16_BITS - 8))
}

// Port of: src/core/SkColorData.h#L47-L49 (chrome/m156)
#[doc(alias = "SkPacked16ToR32")]
#[must_use]
pub const fn packed16_to_r32(c: u32) -> u32 {
    r16_to_r32(get_packed_r16(c))
}

// Port of: src/core/SkColorData.h#L47-L49 (chrome/m156)
#[doc(alias = "SkPacked16ToG32")]
#[must_use]
pub const fn packed16_to_g32(c: u32) -> u32 {
    g16_to_g32(get_packed_g16(c))
}

// Port of: src/core/SkColorData.h#L47-L49 (chrome/m156)
#[doc(alias = "SkPacked16ToB32")]
#[must_use]
pub const fn packed16_to_b32(c: u32) -> u32 {
    b16_to_b32(get_packed_b16(c))
}

/// Swaps the red and blue bytes of a packed pixel.
// Port of: src/core/SkColorData.h#L58-L64 (chrome/m156)
#[doc(alias = "SkSwizzle_RB")]
#[must_use]
pub const fn swizzle_rb(c: u32) -> u32 {
    const K_RB_MASK: u32 = (0xFF << R32_SHIFT) | (0xFF << B32_SHIFT);

    let c0 = (c >> R32_SHIFT) & 0xFF;
    let c1 = (c >> B32_SHIFT) & 0xFF;
    (c & !K_RB_MASK) | (c0 << B32_SHIFT) | (c1 << R32_SHIFT)
}

// Port of: src/core/SkColorData.h#L66-L74 (chrome/m156)
#[doc(alias = "SkPackARGB_as_RGBA")]
#[must_use]
pub fn pack_argb_as_rgba(a: U8CPU, r: U8CPU, g: U8CPU, b: U8CPU) -> u32 {
    use crate::color_priv::{RGBA_A32_SHIFT, RGBA_B32_SHIFT, RGBA_G32_SHIFT, RGBA_R32_SHIFT};
    debug_assert!(a <= 0xFF && r <= 0xFF && g <= 0xFF && b <= 0xFF);
    (a << RGBA_A32_SHIFT) | (r << RGBA_R32_SHIFT) | (g << RGBA_G32_SHIFT) | (b << RGBA_B32_SHIFT)
}

// Port of: src/core/SkColorData.h#L76-L84 (chrome/m156)
#[doc(alias = "SkPackARGB_as_BGRA")]
#[must_use]
pub fn pack_argb_as_bgra(a: U8CPU, r: U8CPU, g: U8CPU, b: U8CPU) -> u32 {
    use crate::color_priv::{BGRA_A32_SHIFT, BGRA_B32_SHIFT, BGRA_G32_SHIFT, BGRA_R32_SHIFT};
    debug_assert!(a <= 0xFF && r <= 0xFF && g <= 0xFF && b <= 0xFF);
    (a << BGRA_A32_SHIFT) | (r << BGRA_R32_SHIFT) | (g << BGRA_G32_SHIFT) | (b << BGRA_B32_SHIFT)
}

// Port of: src/core/SkColorData.h#L86-L92 (chrome/m156)
#[doc(alias = "SkSwizzle_RGBA_to_PMColor")]
#[must_use]
pub const fn swizzle_rgba_to_pm_color(c: u32) -> PMColor {
    // SK_PMCOLOR_IS_RGBA
    c
}

// Port of: src/core/SkColorData.h#L94-L100 (chrome/m156)
#[doc(alias = "SkSwizzle_BGRA_to_PMColor")]
#[must_use]
pub const fn swizzle_bgra_to_pm_color(c: u32) -> PMColor {
    // not SK_PMCOLOR_IS_BGRA
    swizzle_rb(c)
}

/// See ITU-R Recommendation BT.709.
#[doc(alias = "SK_ITU_BT709_LUM_COEFF_R")]
pub const ITU_BT709_LUM_COEFF_R: f32 = 0.2126;
#[doc(alias = "SK_ITU_BT709_LUM_COEFF_G")]
pub const ITU_BT709_LUM_COEFF_G: f32 = 0.7152;
#[doc(alias = "SK_ITU_BT709_LUM_COEFF_B")]
pub const ITU_BT709_LUM_COEFF_B: f32 = 0.0722;

#[doc(alias = "SK_LUM_COEFF_R")]
pub const LUM_COEFF_R: f32 = ITU_BT709_LUM_COEFF_R;
#[doc(alias = "SK_LUM_COEFF_G")]
pub const LUM_COEFF_G: f32 = ITU_BT709_LUM_COEFF_G;
#[doc(alias = "SK_LUM_COEFF_B")]
pub const LUM_COEFF_B: f32 = ITU_BT709_LUM_COEFF_B;

/// Computes the luminance from the given r, g, and b. For correct results, they should be in
/// linear space.
// Port of: src/core/SkColorData.h#L120-L126 (chrome/m156)
#[doc(alias = "SkComputeLuminance")]
#[must_use]
pub fn compute_luminance(r: U8CPU, g: U8CPU, b: U8CPU) -> U8CPU {
    // The following is
    // r * SK_LUM_COEFF_R + g * SK_LUM_COEFF_G + b * SK_LUM_COEFF_B
    // with SK_LUM_COEFF_X in 1.8 fixed point (rounding adjusted to sum to 256).
    (r * 54 + g * 183 + b * 19) >> 8
}

/// Calculates `256 - (value * alpha256) / 255` in range `[0,256]`, for `[0,255]` value and
/// `[0,256]` alpha256.
// Port of: src/core/SkColorData.h#L131-L134 (chrome/m156)
#[doc(alias = "SkAlphaMulInv256")]
#[must_use]
pub fn alpha_mul_inv256(value: U16CPU, alpha256: U16CPU) -> U16CPU {
    let prod = 0xFFFFu32.wrapping_sub(value.wrapping_mul(alpha256));
    (prod + (prod >> 8)) >> 8
}

/// The caller may want negative values, so all params are signed.
// Port of: src/core/SkColorData.h#L138-L141 (chrome/m156)
#[doc(alias = "SkAlphaBlend")]
#[must_use]
pub fn alpha_blend(src: i32, dst: i32, scale256: i32) -> i32 {
    debug_assert!((0..=256).contains(&scale256));
    dst + alpha_mul(src - dst, scale256)
}

// Port of: src/core/SkColorData.h#L143-L149 (chrome/m156)
#[doc(alias = "SkPackRGB16")]
#[must_use]
pub fn pack_rgb16(r: u32, g: u32, b: u32) -> u16 {
    debug_assert!(r <= R16_MASK);
    debug_assert!(g <= G16_MASK);
    debug_assert!(b <= B16_MASK);

    to_u16((r << R16_SHIFT) | (g << G16_SHIFT) | (b << B16_SHIFT))
}

/// Abstract 4-byte interpolation. `scale` is `[0..256]`, unlike [`four_byte_interp`] which takes
/// `[0..255]`. `(src, dst, 0)` returns `dst`; `(src, dst, 0xFF)` returns `src`.
// Port of: src/core/SkColorData.h#L163-L170 (chrome/m156)
#[doc(alias = "SkFourByteInterp256")]
#[must_use]
pub fn four_byte_interp256(src: PMColor, dst: PMColor, scale: i32) -> PMColor {
    // the C++ SkGetPacked*32 results are unsigned, converted to int by the call (values <= 255)
    #[allow(clippy::cast_possible_wrap)]
    let blend =
        |s: u32, d: u32| -> u32 { u32::from(to_u8(alpha_blend(s as i32, d as i32, scale))) };
    let a = blend(get_packed_a32(src), get_packed_a32(dst));
    let r = blend(get_packed_r32(src), get_packed_r32(dst));
    let g = blend(get_packed_g32(src), get_packed_g32(dst));
    let b = blend(get_packed_b32(src), get_packed_b32(dst));

    pack_argb32(a, r, g, b)
}

/// Abstract 4-byte interpolation: `(src, dst, 0)` returns `dst`, `(src, dst, 0xFF)` returns
/// `src` (not exactly, see `SkAlpha255To256`).
// Port of: src/core/SkColorData.h#L178-L181 (chrome/m156)
#[doc(alias = "SkFourByteInterp")]
#[must_use]
pub fn four_byte_interp(src: PMColor, dst: PMColor, src_weight: U8CPU) -> PMColor {
    // alpha_255_to_256 returns a value <= 256
    #[allow(clippy::cast_possible_wrap)]
    let scale = alpha_255_to_256(src_weight) as i32;
    four_byte_interp256(src, dst, scale)
}

/// `0xAARRGGBB -> (0x00AA00GG, 0x00RR00BB)`.
// Port of: src/core/SkColorData.h#L186-L190 (chrome/m156)
#[doc(alias = "SkSplay")]
#[must_use]
pub fn splay(color: u32) -> (u32, u32) {
    const K_MASK: u32 = 0x00FF_00FF;
    ((color >> 8) & K_MASK, color & K_MASK)
}

/// `0xAARRGGBB -> 0x00AA00GG00RR00BB` (note, ARGB -> AGRB).
// Port of: src/core/SkColorData.h#L196-L202 (chrome/m156)
#[doc(alias = "SkSplay")]
#[must_use]
pub fn splay64(color: u32) -> u64 {
    const K_MASK: u32 = 0x00FF_00FF;
    let mut agrb = u64::from((color >> 8) & K_MASK); // 0x0000000000AA00GG
    agrb <<= 32; // 0x00AA00GG00000000
    agrb |= u64::from(color & K_MASK); // 0x00AA00GG00RR00BB
    agrb
}

/// `0xAAxxGGxx, 0xRRxxBBxx -> 0xAARRGGBB`.
// Port of: src/core/SkColorData.h#L207-L210 (chrome/m156)
#[doc(alias = "SkUnsplay")]
#[must_use]
pub fn unsplay(ag: u32, rb: u32) -> u32 {
    const K_MASK: u32 = 0xFF00_FF00;
    (ag & K_MASK) | ((rb & K_MASK) >> 8)
}

/// `0xAAxxGGxxRRxxBBxx -> 0xAARRGGBB` (note, AGRB -> ARGB).
// Port of: src/core/SkColorData.h#L216-L221 (chrome/m156)
#[doc(alias = "SkUnsplay")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors SkPMColor(...) of the 64-bit expression
pub fn unsplay64(agrb: u64) -> u32 {
    const K_MASK: u64 = 0xFF00_FF00;
    (((agrb & K_MASK) >> 8) | ((agrb >> 32) & K_MASK)) as u32
}

// Port of: src/core/SkColorData.h#L223-L235 (chrome/m156)
#[doc(alias = "SkFastFourByteInterp256_32")]
#[must_use]
pub fn fast_four_byte_interp256_32(src: PMColor, dst: PMColor, scale: u32) -> PMColor {
    debug_assert!(scale <= 256);

    // Two 8-bit blends per two 32-bit registers, with space to make sure the math doesn't collide.
    let (src_ag, src_rb) = splay(src);
    let (dst_ag, dst_rb) = splay(dst);

    let ret_ag = src_ag
        .wrapping_mul(scale)
        .wrapping_add((256 - scale).wrapping_mul(dst_ag));
    let ret_rb = src_rb
        .wrapping_mul(scale)
        .wrapping_add((256 - scale).wrapping_mul(dst_rb));

    unsplay(ret_ag, ret_rb)
}

// Port of: src/core/SkColorData.h#L237-L241 (chrome/m156)
#[doc(alias = "SkFastFourByteInterp256_64")]
#[must_use]
pub fn fast_four_byte_interp256_64(src: PMColor, dst: PMColor, scale: u32) -> PMColor {
    debug_assert!(scale <= 256);
    // Four 8-bit blends in one 64-bit register, with space to make sure the math doesn't collide.
    unsplay64(
        splay64(src)
            .wrapping_mul(u64::from(scale))
            .wrapping_add(u64::from(256 - scale).wrapping_mul(splay64(dst))),
    )
}

/// Same as [`four_byte_interp256`], but faster.
// Port of: src/core/SkColorData.h#L247-L254 (chrome/m156)
#[doc(alias = "SkFastFourByteInterp256")]
#[must_use]
pub fn fast_four_byte_interp256(src: PMColor, dst: PMColor, scale: u32) -> PMColor {
    // On a 64-bit machine, _64 is about 10% faster than _32 (sizeof(void*) == 8).
    fast_four_byte_interp256_64(src, dst, scale)
}

/// Nearly the same as [`four_byte_interp`], but faster and a touch more accurate, due to better
/// `src_weight` scaling to `[0, 256]`.
// Port of: src/core/SkColorData.h#L260-L265 (chrome/m156)
#[doc(alias = "SkFastFourByteInterp")]
#[must_use]
pub fn fast_four_byte_interp(src: PMColor, dst: PMColor, src_weight: U8CPU) -> PMColor {
    debug_assert!(src_weight <= 255);
    // scale = srcWeight + (srcWeight >> 7) is more accurate than
    // scale = srcWeight + 1, but 7% slower
    fast_four_byte_interp256(src, dst, src_weight + (src_weight >> 7))
}

/// Interpolates between colors `src` and `dst` using `[0,256]` scale.
// Port of: src/core/SkColorData.h#L270-L272 (chrome/m156)
#[doc(alias = "SkPMLerp")]
#[must_use]
pub fn pm_lerp(src: PMColor, dst: PMColor, scale: u32) -> PMColor {
    fast_four_byte_interp256(src, dst, scale)
}

// Port of: src/core/SkColorData.h#L274-L290 (chrome/m156)
#[doc(alias = "SkBlendARGB32")]
#[must_use]
pub fn blend_argb32(src: PMColor, dst: PMColor, aa: U8CPU) -> PMColor {
    const K_MASK: u32 = 0x00FF_00FF;

    debug_assert!(aa <= 255);

    let src_scale = alpha_255_to_256(aa);
    let dst_scale = alpha_mul_inv256(get_packed_a32(src), src_scale);

    let src_rb = (src & K_MASK).wrapping_mul(src_scale);
    let src_ag = ((src >> 8) & K_MASK).wrapping_mul(src_scale);

    let dst_rb = (dst & K_MASK).wrapping_mul(dst_scale);
    let dst_ag = ((dst >> 8) & K_MASK).wrapping_mul(dst_scale);

    ((src_rb.wrapping_add(dst_rb) >> 8) & K_MASK) | (src_ag.wrapping_add(dst_ag) & !K_MASK)
}

// Convert a 32bit pixel to a 16bit pixel (no dither)

// Port of: src/core/SkColorData.h#L296-L298 (chrome/m156)
#[doc(alias = "SkR32ToR16")]
#[must_use]
pub fn r32_to_r16(r: u32) -> u32 {
    debug_assert!(r <= crate::color_priv::R32_MASK);
    r >> (R32_BITS - R16_BITS)
}

// Port of: src/core/SkColorData.h#L296-L298 (chrome/m156)
#[doc(alias = "SkG32ToG16")]
#[must_use]
pub fn g32_to_g16(g: u32) -> u32 {
    debug_assert!(g <= crate::color_priv::G32_MASK);
    g >> (G32_BITS - G16_BITS)
}

// Port of: src/core/SkColorData.h#L296-L298 (chrome/m156)
#[doc(alias = "SkB32ToB16")]
#[must_use]
pub fn b32_to_b16(b: u32) -> u32 {
    debug_assert!(b <= crate::color_priv::B32_MASK);
    b >> (B32_BITS - B16_BITS)
}

// Port of: src/core/SkColorData.h#L322-L327 (chrome/m156)
#[doc(alias = "SkPixel32ToPixel16")]
#[must_use]
pub fn pixel32_to_pixel16(c: PMColor) -> U16CPU {
    let r = ((c >> (R32_SHIFT + (8 - R16_BITS))) & R16_MASK) << R16_SHIFT;
    let g = ((c >> (G32_SHIFT + (8 - G16_BITS))) & G16_MASK) << G16_SHIFT;
    let b = ((c >> (B32_SHIFT + (8 - B16_BITS))) & B16_MASK) << B16_SHIFT;
    r | g | b
}

// Port of: src/core/SkColorData.h#L329-L333 (chrome/m156)
#[doc(alias = "SkPack888ToRGB16")]
#[must_use]
pub fn pack888_to_rgb16(r: U8CPU, g: U8CPU, b: U8CPU) -> U16CPU {
    (r32_to_r16(r) << R16_SHIFT) | (g32_to_g16(g) << G16_SHIFT) | (b32_to_b16(b) << B16_SHIFT)
}

// Port of: src/core/SkColorData.h#L337-L349 (chrome/m156)
#[doc(alias = "SkPixel16ToColor")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // each 16->32 expanded component is <= 255
pub fn pixel16_to_color(src: U16CPU) -> Color {
    debug_assert_eq!(u32::from(to_u16(src)), src);

    let r = packed16_to_r32(src);
    let g = packed16_to_g32(src);
    let b = packed16_to_b32(src);

    debug_assert_eq!(r >> (8 - R16_BITS), get_packed_r16(src));
    debug_assert_eq!(g >> (8 - G16_BITS), get_packed_g16(src));
    debug_assert_eq!(b >> (8 - B16_BITS), get_packed_b16(src));

    Color::from_rgb(r as u8, g as u8, b as u8)
}

/// 16-bit premultiplied 4444 color.
// Port of: src/core/SkColorData.h#L353 (chrome/m156)
#[doc(alias = "SkPMColor16")]
pub type PMColor16 = u16;

// Put in OpenGL order (r g b a)
#[doc(alias = "SK_A4444_SHIFT")]
pub const A4444_SHIFT: u32 = 0;
#[doc(alias = "SK_R4444_SHIFT")]
pub const R4444_SHIFT: u32 = 12;
#[doc(alias = "SK_G4444_SHIFT")]
pub const G4444_SHIFT: u32 = 8;
#[doc(alias = "SK_B4444_SHIFT")]
pub const B4444_SHIFT: u32 = 4;

// Port of: src/core/SkColorData.h#L361-L364 (chrome/m156)
#[doc(alias = "SkReplicateNibble")]
#[must_use]
pub fn replicate_nibble(nib: u32) -> U8CPU {
    debug_assert!(nib <= 0xF);
    (nib << 4) | nib
}

// Port of: src/core/SkColorData.h#L366 (chrome/m156)
#[doc(alias = "SkGetPackedA4444")]
#[must_use]
pub const fn get_packed_a4444(c: u32) -> u32 {
    (c >> A4444_SHIFT) & 0xF
}

// Port of: src/core/SkColorData.h#L367 (chrome/m156)
#[doc(alias = "SkGetPackedR4444")]
#[must_use]
pub const fn get_packed_r4444(c: u32) -> u32 {
    (c >> R4444_SHIFT) & 0xF
}

// Port of: src/core/SkColorData.h#L368 (chrome/m156)
#[doc(alias = "SkGetPackedG4444")]
#[must_use]
pub const fn get_packed_g4444(c: u32) -> u32 {
    (c >> G4444_SHIFT) & 0xF
}

// Port of: src/core/SkColorData.h#L369 (chrome/m156)
#[doc(alias = "SkGetPackedB4444")]
#[must_use]
pub const fn get_packed_b4444(c: u32) -> u32 {
    (c >> B4444_SHIFT) & 0xF
}

// Port of: src/core/SkColorData.h#L371 (chrome/m156)
#[doc(alias = "SkPacked4444ToA32")]
#[must_use]
pub fn packed4444_to_a32(c: u32) -> U8CPU {
    replicate_nibble(get_packed_a4444(c))
}

// Port of: src/core/SkColorData.h#L373-L380 (chrome/m156)
#[doc(alias = "SkPixel4444ToPixel32")]
#[must_use]
pub fn pixel4444_to_pixel32(c: U16CPU) -> PMColor {
    let d = (get_packed_a4444(c) << A32_SHIFT)
        | (get_packed_r4444(c) << R32_SHIFT)
        | (get_packed_g4444(c) << G32_SHIFT)
        | (get_packed_b4444(c) << B32_SHIFT);
    d | (d << 4)
}

// Port of: src/core/SkColorData.h#L384-L386 (chrome/m156)
#[doc(alias = "SK_PMColor4fTRANSPARENT")]
pub const PM_COLOR4F_TRANSPARENT: PMColor4f = PMColor4f::new(0.0, 0.0, 0.0, 0.0);
// Port of: src/core/SkColorData.h#L384-L386 (chrome/m156)
#[doc(alias = "SK_PMColor4fBLACK")]
pub const PM_COLOR4F_BLACK: PMColor4f = PMColor4f::new(0.0, 0.0, 0.0, 1.0);
// Port of: src/core/SkColorData.h#L384-L386 (chrome/m156)
#[doc(alias = "SK_PMColor4fWHITE")]
pub const PM_COLOR4F_WHITE: PMColor4f = PMColor4f::new(1.0, 1.0, 1.0, 1.0);
// Port of: src/core/SkColorData.h#L387-L390 (chrome/m156)
#[doc(alias = "SK_PMColor4fILLEGAL")]
pub const PM_COLOR4F_ILLEGAL: PMColor4f = PMColor4f::new(
    FLOAT_NEGATIVE_INFINITY,
    FLOAT_NEGATIVE_INFINITY,
    FLOAT_NEGATIVE_INFINITY,
    FLOAT_NEGATIVE_INFINITY,
);
