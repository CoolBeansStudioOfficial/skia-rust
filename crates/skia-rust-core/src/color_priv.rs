// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkColorPriv.h

//! Private color helpers (`SkColorPriv.h`).
//!
//! `SkPMColor` is `RGBA` in memory (`SK_R32_SHIFT == 0`), Skia's default off Windows, which is
//! what the oracle uses. `SkColorConverter` is not ported yet.

use crate::color::PMColor;
use crate::math::{U8CPU, mul_div_255_round};
use crate::scalar::scalar;
use crate::t_pin::t_pin;
use crate::to::to_u8;

/// Turn 0..255 into 0..256 by adding 1 at the half-way point. Used to turn a byte into a scale
/// value, so that we can say `scale * value >> 8` instead of `alpha * value / 255`.
// Port of: src/core/SkColorPriv.h#L24-L29 (chrome/m156)
#[doc(alias = "SkAlpha255To256")]
#[must_use]
pub fn alpha_255_to_256(alpha: U8CPU) -> u32 {
    debug_assert_eq!(u32::from(to_u8(alpha)), alpha);
    // this one assues that blending on top of an opaque dst keeps it that way
    // even though it is less accurate than a+(a>>7) for non-opaque dsts
    alpha + 1
}

/// Multiply `value` by 0..256, and shift the result down 8 (`(value * alpha256) >> 8`).
// Port of: src/core/SkColorPriv.h#L35 (chrome/m156)
#[doc(alias = "SkAlphaMul")]
#[must_use]
pub fn alpha_mul(value: i32, alpha256: i32) -> i32 {
    value.wrapping_mul(alpha256) >> 8
}

/// Clamps `x` to `[0, 1]` and converts to a byte, rounding.
// Port of: src/core/SkColorPriv.h#L37-L39 (chrome/m156)
#[doc(alias = "SkUnitScalarClampToByte")]
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the static_cast<U8CPU>; the pinned value is within [0, 255.5]
pub fn unit_scalar_clamp_to_byte(x: scalar) -> U8CPU {
    (t_pin(x, 0.0f32, 1.0f32) * 255.0 + 0.5f32) as U8CPU
}

#[doc(alias = "SK_A32_BITS")]
pub const A32_BITS: u32 = 8;
#[doc(alias = "SK_R32_BITS")]
pub const R32_BITS: u32 = 8;
#[doc(alias = "SK_G32_BITS")]
pub const G32_BITS: u32 = 8;
#[doc(alias = "SK_B32_BITS")]
pub const B32_BITS: u32 = 8;

#[doc(alias = "SK_A32_MASK")]
pub const A32_MASK: u32 = (1 << A32_BITS) - 1;
#[doc(alias = "SK_R32_MASK")]
pub const R32_MASK: u32 = (1 << R32_BITS) - 1;
#[doc(alias = "SK_G32_MASK")]
pub const G32_MASK: u32 = (1 << G32_BITS) - 1;
#[doc(alias = "SK_B32_MASK")]
pub const B32_MASK: u32 = (1 << B32_BITS) - 1;

#[doc(alias = "SK_RGBA_R32_SHIFT")]
pub const RGBA_R32_SHIFT: u32 = 0;
#[doc(alias = "SK_RGBA_G32_SHIFT")]
pub const RGBA_G32_SHIFT: u32 = 8;
#[doc(alias = "SK_RGBA_B32_SHIFT")]
pub const RGBA_B32_SHIFT: u32 = 16;
#[doc(alias = "SK_RGBA_A32_SHIFT")]
pub const RGBA_A32_SHIFT: u32 = 24;

#[doc(alias = "SK_BGRA_B32_SHIFT")]
pub const BGRA_B32_SHIFT: u32 = 0;
#[doc(alias = "SK_BGRA_G32_SHIFT")]
pub const BGRA_G32_SHIFT: u32 = 8;
#[doc(alias = "SK_BGRA_R32_SHIFT")]
pub const BGRA_R32_SHIFT: u32 = 16;
#[doc(alias = "SK_BGRA_A32_SHIFT")]
pub const BGRA_A32_SHIFT: u32 = 24;

// Port of: include/core/SkTypes.h#L36-L55 (chrome/m156)
// skia-rust: SK_R32_SHIFT is fixed to 0 (RGBA), Skia's default everywhere except Windows.
#[doc(alias = "SK_R32_SHIFT")]
pub const R32_SHIFT: u32 = 0;
#[doc(alias = "SK_G32_SHIFT")]
pub const G32_SHIFT: u32 = 8;
#[doc(alias = "SK_B32_SHIFT")]
pub const B32_SHIFT: u32 = 16 - R32_SHIFT;
#[doc(alias = "SK_A32_SHIFT")]
pub const A32_SHIFT: u32 = 24;

/// True if `SkPMColor` is `RGBA` in memory (`SK_PMCOLOR_IS_RGBA`).
#[doc(alias = "SK_PMCOLOR_IS_RGBA")]
pub const PMCOLOR_IS_RGBA: bool = R32_SHIFT == RGBA_R32_SHIFT
    && G32_SHIFT == RGBA_G32_SHIFT
    && B32_SHIFT == RGBA_B32_SHIFT
    && A32_SHIFT == RGBA_A32_SHIFT;

/// True if `SkPMColor` is `BGRA` in memory (`SK_PMCOLOR_IS_BGRA`).
#[doc(alias = "SK_PMCOLOR_IS_BGRA")]
pub const PMCOLOR_IS_BGRA: bool = R32_SHIFT == BGRA_R32_SHIFT
    && G32_SHIFT == BGRA_G32_SHIFT
    && B32_SHIFT == BGRA_B32_SHIFT
    && A32_SHIFT == BGRA_A32_SHIFT;

// need 32bit packing to be either RGBA or BGRA
const _: () = assert!(PMCOLOR_IS_RGBA || PMCOLOR_IS_BGRA);

/// Alpha component of a packed [`PMColor`].
// Port of: src/core/SkColorPriv.h#L97 (chrome/m156)
#[doc(alias = "SkGetPackedA32")]
#[must_use]
pub const fn get_packed_a32(packed: u32) -> u32 {
    (packed << (24 - A32_SHIFT)) >> 24
}

/// Red component of a packed [`PMColor`].
// Port of: src/core/SkColorPriv.h#L98 (chrome/m156)
#[doc(alias = "SkGetPackedR32")]
#[must_use]
pub const fn get_packed_r32(packed: u32) -> u32 {
    (packed << (24 - R32_SHIFT)) >> 24
}

/// Green component of a packed [`PMColor`].
// Port of: src/core/SkColorPriv.h#L99 (chrome/m156)
#[doc(alias = "SkGetPackedG32")]
#[must_use]
pub const fn get_packed_g32(packed: u32) -> u32 {
    (packed << (24 - G32_SHIFT)) >> 24
}

/// Blue component of a packed [`PMColor`].
// Port of: src/core/SkColorPriv.h#L100 (chrome/m156)
#[doc(alias = "SkGetPackedB32")]
#[must_use]
pub const fn get_packed_b32(packed: u32) -> u32 {
    (packed << (24 - B32_SHIFT)) >> 24
}

/// Pack the components into a [`PMColor`].
// Port of: src/core/SkColorPriv.h#L110-L118 (chrome/m156)
#[doc(alias = "SkPackARGB32")]
#[must_use]
pub fn pack_argb32(a: U8CPU, r: U8CPU, g: U8CPU, b: U8CPU) -> PMColor {
    debug_assert!(a <= A32_MASK);
    debug_assert!(r <= R32_MASK);
    debug_assert!(g <= G32_MASK);
    debug_assert!(b <= B32_MASK);

    (a << A32_SHIFT) | (r << R32_SHIFT) | (g << G32_SHIFT) | (b << B32_SHIFT)
}

/// Premultiplies 8-bit components and packs them.
// Port of: src/core/SkColorPriv.h#L120-L134 (chrome/m156)
#[doc(alias = "SkPremultiplyARGBInline")]
#[must_use]
pub fn premultiply_argb_inline(a: U8CPU, mut r: U8CPU, mut g: U8CPU, mut b: U8CPU) -> PMColor {
    debug_assert!(a <= A32_MASK);
    debug_assert!(r <= R32_MASK);
    debug_assert!(g <= G32_MASK);
    debug_assert!(b <= B32_MASK);

    if a != 255 {
        r = mul_div_255_round(r, a);
        g = mul_div_255_round(g, a);
        b = mul_div_255_round(b, a);
    }
    pack_argb32(a, r, g, b)
}

/// Scales all four channels of `c` by `scale / 256`.
// Port of: src/core/SkColorPriv.h#L138-L144 (chrome/m156)
#[doc(alias = "SkAlphaMulQ")]
#[must_use]
pub fn alpha_mul_q(c: u32, scale: u32) -> u32 {
    const K_MASK: u32 = 0x00FF_00FF;

    let rb = ((c & K_MASK).wrapping_mul(scale)) >> 8;
    let ag = ((c >> 8) & K_MASK).wrapping_mul(scale);
    (rb & K_MASK) | (ag & !K_MASK)
}

/// Source-over of premultiplied `src` onto `dst`.
// Port of: src/core/SkColorPriv.h#L146-L161 (chrome/m156)
#[doc(alias = "SkPMSrcOver")]
#[must_use]
pub fn pm_src_over(src: PMColor, dst: PMColor) -> PMColor {
    const K_MASK: u32 = 0x00FF_00FF;

    let scale = alpha_255_to_256(255 - get_packed_a32(src));
    let mut rb = ((dst & K_MASK).wrapping_mul(scale) >> 8) & K_MASK;
    let mut ag = (((dst >> 8) & K_MASK).wrapping_mul(scale)) & !K_MASK;

    rb = rb.wrapping_add(src & K_MASK);
    ag = ag.wrapping_add(src & !K_MASK);

    // Color channels (but not alpha) can overflow, so we have to saturate to 0xFF in each lane.
    (rb & 0x0000_01FF).min(0x0000_00FF)
        | (ag & 0x0001_FF00).min(0x0000_FF00)
        | (rb & 0x01FF_0000).min(0x00FF_0000)
        | (ag & 0xFF00_0000)
}
