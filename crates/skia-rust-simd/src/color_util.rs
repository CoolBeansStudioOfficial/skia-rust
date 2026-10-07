// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkColorPriv.h, src/core/SkMathPriv.h (the helpers the `SkOpts`
// blit kernels use)

//! The few `SkColorPriv.h` helpers the blit kernels need.
//!
//! `skia-rust-core` depends on this crate, so it cannot supply them; they are small pure
//! integer functions. The `SkPMColor` byte order must equal `skia_rust_core::color_priv`'s
//! (`R32_SHIFT`: BGRA on Windows, RGBA elsewhere); a test in `tests/` checks that.

/// `SK_R32_SHIFT`: Skia's default, BGRA on Windows and RGBA elsewhere.
// Port of: include/core/SkTypes.h#L36-L55 (chrome/m156)
pub const R32_SHIFT: u32 = if cfg!(windows) { 16 } else { 0 };
/// `SK_G32_SHIFT`.
pub const G32_SHIFT: u32 = 8;
/// `SK_B32_SHIFT`.
pub const B32_SHIFT: u32 = 16 - R32_SHIFT;
/// `SK_A32_SHIFT`.
pub const A32_SHIFT: u32 = 24;

/// `SK_ColorBLACK`.
pub(crate) const COLOR_BLACK: u32 = 0xFF00_0000;

/// `SkAlpha255To256`.
// Port of: src/core/SkColorPriv.h#L24-L29 (chrome/m156)
#[inline]
pub(crate) fn alpha_255_to_256(alpha: u32) -> u32 {
    debug_assert!(alpha <= 0xFF);
    alpha + 1
}

/// `SkMulDiv255Round(a, b)` (`SkMul16ShiftRound(a, b, 8)`).
// Port of: src/core/SkMathPriv.h (SkMul16ShiftRound), include/private/base/SkMath.h (chrome/m156)
#[inline]
pub(crate) fn mul_div_255_round(a: u32, b: u32) -> u32 {
    debug_assert!(a <= 0xFFFF && b <= 0xFFFF);
    let prod = a * b + (1 << 7);
    (prod + (prod >> 8)) >> 8
}

/// `kMask` of `SkAlphaMulQ` and `SkPMSrcOver`.
const MASK: u32 = 0x00FF_00FF;

/// `SkAlphaMulQ(c, scale)`: scales each byte of `c` by `scale / 256`.
// Port of: src/core/SkColorPriv.h#L135-L143 (chrome/m156)
#[inline]
pub(crate) fn alpha_mul_q(c: u32, scale: u32) -> u32 {
    let rb = (c & MASK).wrapping_mul(scale) >> 8;
    let ag = ((c >> 8) & MASK).wrapping_mul(scale);
    (rb & MASK) | (ag & !MASK)
}

/// `SkPMSrcOver(src, dst)`.
// Port of: src/core/SkColorPriv.h#L145-L160 (chrome/m156)
#[inline]
pub(crate) fn pm_src_over(src: u32, dst: u32) -> u32 {
    let scale = alpha_255_to_256(255 - (src >> A32_SHIFT));

    let mut rb = ((dst & MASK).wrapping_mul(scale) >> 8) & MASK;
    let mut ag = ((dst >> 8) & MASK).wrapping_mul(scale) & !MASK;

    rb = rb.wrapping_add(src & MASK);
    ag = ag.wrapping_add(src & !MASK);

    // Color channels (but not alpha) can overflow, so we have to saturate to 0xFF in each lane.
    (rb & 0x0000_01FF).min(0x0000_00FF)
        | (ag & 0x0001_FF00).min(0x0000_FF00)
        | (rb & 0x01FF_0000).min(0x00FF_0000)
        | (ag & 0xFF00_0000)
}

/// `SkPreMultiplyColor(c)` for an `SkColor` (`0xAARRGGBB`), packed in `SkPMColor`'s byte order.
// Port of: src/core/SkColor.cpp#L22-L25, src/core/SkColorPriv.h#L120-L134 (chrome/m156)
#[inline]
#[allow(clippy::many_single_char_names)] // Skia's a, r, g, b
pub(crate) fn pre_multiply_color(c: u32) -> u32 {
    let a = c >> 24;
    let mut r = (c >> 16) & 0xFF;
    let mut g = (c >> 8) & 0xFF;
    let mut b = c & 0xFF;
    if a != 255 {
        r = mul_div_255_round(r, a);
        g = mul_div_255_round(g, a);
        b = mul_div_255_round(b, a);
    }
    (a << A32_SHIFT) | (r << R32_SHIFT) | (g << G32_SHIFT) | (b << B32_SHIFT)
}
