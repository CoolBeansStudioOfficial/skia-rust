// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColor.h, src/core/SkColor.cpp

//! Types, consts, functions, and macros for colors.
//!
//! Mirrors `skia_safe::core::color`. `SkRGBA4f<kAT>` is modelled as two concrete types, like
//! `skia-safe` does for `SkColor4f`: [`Color4f`] (unpremultiplied) and [`PMColor4f`]
//! (premultiplied), sharing their implementation through a private macro.
//!
//! `SkPMColor` has the byte order `RGBA` (`SK_R32_SHIFT == 0`), Skia's default off Windows,
//! which is what the oracle uses. See [`crate::color_priv`].

use std::ops::{BitAnd, BitOr, Index, IndexMut, Mul};

use skia_rust_simd::vx::{Byte4, Float4, shuffle};

use crate::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
    premultiply_argb_inline,
};
use crate::math::U8CPU;
use crate::scalar::{
    SCALAR_1, Scalar, int_to_scalar, scalar, scalar_floor_to_scalar, scalar_round_to_int,
};
use crate::t_pin::t_pin;

/// 8-bit type for an alpha value. 255 is 100% opaque, zero is 100% transparent.
// Port of: include/core/SkColor.h#L25 (chrome/m156)
#[doc(alias = "SkAlpha")]
pub type Alpha = u8;

/// Alpha value of zero: fully transparent.
// Port of: include/core/SkColor.h#L109 (chrome/m156)
#[doc(alias = "SK_AlphaTRANSPARENT")]
pub const ALPHA_TRANSPARENT: Alpha = 0x00;

/// Alpha value of 255: fully opaque.
// Port of: include/core/SkColor.h#L113 (chrome/m156)
#[doc(alias = "SK_AlphaOPAQUE")]
pub const ALPHA_OPAQUE: Alpha = 0xFF;

/// 32-bit ARGB color value, unpremultiplied. Color components are always in a known order
/// (`0xAARRGGBB`). This is different from [`PMColor`], which has its bytes in a configuration
/// dependent order.
// Port of: include/core/SkColor.h#L36 (chrome/m156)
#[doc(alias = "SkColor")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(transparent)]
pub struct Color(u32);

impl From<u32> for Color {
    fn from(argb: u32) -> Self {
        Color::new(argb)
    }
}

// skia-rust: `skia-safe` keeps the packed value private (`into_native`); this is the safe
// equivalent.
impl From<Color> for u32 {
    fn from(c: Color) -> Self {
        c.0
    }
}

impl From<RGB> for Color {
    fn from(rgb: RGB) -> Self {
        Color::from_rgb(rgb.r, rgb.g, rgb.b)
    }
}

impl BitOr for Color {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Color(self.0 | rhs.0)
    }
}

impl BitAnd for Color {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Color(self.0 & rhs.0)
    }
}

impl BitOr<u32> for Color {
    type Output = Self;

    fn bitor(self, rhs: u32) -> Self::Output {
        Color(self.0 | rhs)
    }
}

impl BitAnd<u32> for Color {
    type Output = Self;

    fn bitand(self, rhs: u32) -> Self::Output {
        Color(self.0 & rhs)
    }
}

impl Color {
    /// Wraps a packed `0xAARRGGBB` value.
    #[must_use]
    pub const fn new(argb: u32) -> Self {
        Self(argb)
    }

    /// Returns color value from 8-bit component values.
    // Port of: include/core/SkColor.h#L48-L51 (chrome/m156)
    #[doc(alias = "SkColorSetARGB")]
    #[must_use]
    pub const fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Color {
        Self(((a as U8CPU) << 24) | ((r as U8CPU) << 16) | ((g as U8CPU) << 8) | (b as U8CPU))
    }

    /// Returns color value from 8-bit component values, with alpha set fully opaque.
    // Port of: include/core/SkColor.h#L57 (chrome/m156)
    #[doc(alias = "SkColorSetRGB")]
    #[must_use]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Color {
        Self::from_argb(0xff, r, g, b)
    }

    /// Alpha component.
    // Port of: include/core/SkColor.h#L61 (chrome/m156)
    #[doc(alias = "SkColorGetA")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // masked to 8 bits, as SkColorGetA
    pub const fn a(self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// Red component.
    // Port of: include/core/SkColor.h#L65 (chrome/m156)
    #[doc(alias = "SkColorGetR")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // masked to 8 bits, as SkColorGetR
    pub const fn r(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Green component.
    // Port of: include/core/SkColor.h#L69 (chrome/m156)
    #[doc(alias = "SkColorGetG")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // masked to 8 bits, as SkColorGetG
    pub const fn g(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Blue component.
    // Port of: include/core/SkColor.h#L73 (chrome/m156)
    #[doc(alias = "SkColorGetB")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // masked to 8 bits, as SkColorGetB
    pub const fn b(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Returns a color with the alpha replaced by `a`.
    // Port of: include/core/SkColor.h#L83-L85 (chrome/m156)
    #[doc(alias = "SkColorSetA")]
    #[must_use]
    pub const fn with_a(self, a: u8) -> Self {
        Self((self.0 & 0x00FF_FFFF) | ((a as U8CPU) << 24))
    }

    /// Transparent, equivalent to `0x00000000`.
    #[doc(alias = "SK_ColorTRANSPARENT")]
    pub const TRANSPARENT: Self = Self::from_argb(0x00, 0x00, 0x00, 0x00);
    /// Black, equivalent to `0xFF000000`.
    #[doc(alias = "SK_ColorBLACK")]
    pub const BLACK: Self = Self::from_argb(0xFF, 0x00, 0x00, 0x00);
    /// Dark gray, equivalent to `0xFF444444`.
    #[doc(alias = "SK_ColorDKGRAY")]
    pub const DARK_GRAY: Self = Self::from_argb(0xFF, 0x44, 0x44, 0x44);
    /// Gray, equivalent to `0xFF888888`.
    #[doc(alias = "SK_ColorGRAY")]
    pub const GRAY: Self = Self::from_argb(0xFF, 0x88, 0x88, 0x88);
    /// Light gray, equivalent to `0xFFCCCCCC`.
    #[doc(alias = "SK_ColorLTGRAY")]
    pub const LIGHT_GRAY: Self = Self::from_argb(0xFF, 0xCC, 0xCC, 0xCC);
    /// White, equivalent to `0xFFFFFFFF`.
    #[doc(alias = "SK_ColorWHITE")]
    pub const WHITE: Self = Self::from_argb(0xFF, 0xFF, 0xFF, 0xFF);
    /// Red, equivalent to `0xFFFF0000`.
    #[doc(alias = "SK_ColorRED")]
    pub const RED: Self = Self::from_argb(0xFF, 0xFF, 0x00, 0x00);
    /// Green, equivalent to `0xFF00FF00`.
    #[doc(alias = "SK_ColorGREEN")]
    pub const GREEN: Self = Self::from_argb(0xFF, 0x00, 0xFF, 0x00);
    /// Blue, equivalent to `0xFF0000FF`.
    #[doc(alias = "SK_ColorBLUE")]
    pub const BLUE: Self = Self::from_argb(0xFF, 0x00, 0x00, 0xFF);
    /// Yellow, equivalent to `0xFFFFFF00`.
    #[doc(alias = "SK_ColorYELLOW")]
    pub const YELLOW: Self = Self::from_argb(0xFF, 0xFF, 0xFF, 0x00);
    /// Cyan, equivalent to `0xFF00FFFF`.
    #[doc(alias = "SK_ColorCYAN")]
    pub const CYAN: Self = Self::from_argb(0xFF, 0x00, 0xFF, 0xFF);
    /// Magenta, equivalent to `0xFFFF00FF`.
    #[doc(alias = "SK_ColorMAGENTA")]
    pub const MAGENTA: Self = Self::from_argb(0xFF, 0xFF, 0x00, 0xFF);

    /// The red, green and blue components.
    #[must_use]
    pub fn to_rgb(self) -> RGB {
        (self.r(), self.g(), self.b()).into()
    }

    /// Converts to HSV (`SkColorToHSV`).
    // Port of: include/core/SkColor.h#L193-L195 (chrome/m156)
    #[doc(alias = "SkColorToHSV")]
    #[must_use]
    pub fn to_hsv(self) -> HSV {
        self.to_rgb().to_hsv()
    }
}

/// Red, green and blue components, each 0 to 255.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<(u8, u8, u8)> for RGB {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self { r, g, b }
    }
}

// Port of: src/core/SkColor.cpp#L52-L59 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // x <= 255
fn byte_to_scalar(x: U8CPU) -> scalar {
    debug_assert!(x <= 255);
    int_to_scalar(x as i32) / 255.0
}

// Port of: src/core/SkColor.cpp#L61-L64 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // C++ "(int)denom" cast, denom <= 255
fn byte_div_to_scalar(numer: i32, denom: U8CPU) -> scalar {
    // cast to keep the answer signed
    int_to_scalar(numer) / int_to_scalar(denom as i32)
}

impl RGB {
    /// Converts RGB to its HSV equivalent. Hue is from zero to less than 360, saturation and
    /// value are from zero to one.
    // Port of: src/core/SkColor.cpp#L66-L103 (chrome/m156)
    #[doc(alias = "SkRGBToHSV")]
    #[must_use]
    #[allow(clippy::many_single_char_names, clippy::cast_possible_wrap)]
    // mirrors SkRGBToHSV's variable names; delta <= 255
    pub fn to_hsv(self) -> HSV {
        let r = U8CPU::from(self.r);
        let g = U8CPU::from(self.g);
        let b = U8CPU::from(self.b);

        let min = r.min(g.min(b));
        let max = r.max(g.max(b));
        let delta = max - min;

        let v = byte_to_scalar(max);
        debug_assert!((0.0..=SCALAR_1).contains(&v));

        if 0 == delta {
            // we're a shade of gray
            return HSV { h: 0.0, s: 0.0, v };
        }

        // `numer` arguments below are differences of bytes, so they fit in an i32.
        let (ri, gi, bi) = (i32::from(self.r), i32::from(self.g), i32::from(self.b));
        let s = byte_div_to_scalar(delta as i32, max);
        debug_assert!((0.0..=SCALAR_1).contains(&s));

        let mut h: scalar = if r == max {
            byte_div_to_scalar(gi - bi, delta)
        } else if g == max {
            int_to_scalar(2) + byte_div_to_scalar(bi - ri, delta)
        } else {
            // b == max
            int_to_scalar(4) + byte_div_to_scalar(ri - gi, delta)
        };

        h *= 60.0;
        if h < 0.0 {
            h += int_to_scalar(360);
        }
        debug_assert!(h >= 0.0 && h < int_to_scalar(360));

        HSV { h, s, v }
    }
}

/// Hue, saturation and value. Hue is an angle from zero to less than 360; saturation and value
/// are from zero to one.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct HSV {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl From<(f32, f32, f32)> for HSV {
    fn from((h, s, v): (f32, f32, f32)) -> Self {
        Self { h, s, v }
    }
}

impl HSV {
    /// Converts HSV to its color equivalent, with the given `alpha`. Out of range hsv values are
    /// pinned.
    // Port of: src/core/SkColor.cpp#L105-L136 (chrome/m156)
    #[doc(alias = "SkHSVToColor")]
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::many_single_char_names
    )]
    // mirrors the SkHSVToColor variable names and the (unsigned)/U8CPU casts of values that are in [0, 255] / [0, 6)
    pub fn to_color(self, alpha: u8) -> Color {
        let hsv = [self.h, self.s, self.v];

        let s = t_pin(hsv[1], 0.0f32, 1.0f32);
        let v = t_pin(hsv[2], 0.0f32, 1.0f32);

        let v_byte = scalar_round_to_int(v * 255.0) as U8CPU;

        if s.nearly_zero(None) {
            // shade of gray
            let v = v_byte as u8;
            return Color::from_argb(alpha, v, v, v);
        }
        let hx: scalar = if hsv[0] < 0.0 || hsv[0] >= int_to_scalar(360) {
            0.0
        } else {
            hsv[0] / 60.0
        };
        let w = scalar_floor_to_scalar(hx);
        let f = hx - w;

        let p = scalar_round_to_int((SCALAR_1 - s) * v * 255.0) as u32;
        let q = scalar_round_to_int((SCALAR_1 - (s * f)) * v * 255.0) as u32;
        let t = scalar_round_to_int((SCALAR_1 - (s * (SCALAR_1 - f))) * v * 255.0) as u32;

        debug_assert!((w as u32) < 6);
        let (r, g, b) = match w as u32 {
            0 => (v_byte, t, p),
            1 => (q, v_byte, p),
            2 => (p, v_byte, t),
            3 => (p, q, v_byte),
            4 => (t, p, v_byte),
            _ => (v_byte, p, q),
        };
        Color::from_argb(alpha, r as u8, g as u8, b as u8)
    }
}

/// 32-bit ARGB color value, premultiplied. The byte order for this value is configuration
/// dependent, matching the format of `kBGRA_8888` bitmaps (here: `RGBA` in memory).
// Port of: include/core/SkColor.h#L228 (chrome/m156)
#[doc(alias = "SkPMColor")]
pub type PMColor = u32;

/// Returns a premultiplied color from unpremultiplied 8-bit components.
// Port of: src/core/SkColor.cpp#L18-L20 (chrome/m156)
#[doc(alias = "SkPreMultiplyARGB")]
#[must_use]
pub fn pre_multiply_argb(a: U8CPU, r: U8CPU, g: U8CPU, b: U8CPU) -> PMColor {
    premultiply_argb_inline(a, r, g, b)
}

/// Returns a premultiplied color from an unpremultiplied color.
// Port of: src/core/SkColor.cpp#L22-L25 (chrome/m156)
#[doc(alias = "SkPreMultiplyColor")]
#[must_use]
pub fn pre_multiply_color(c: impl Into<Color>) -> PMColor {
    let c = c.into();
    premultiply_argb_inline(
        U8CPU::from(c.a()),
        U8CPU::from(c.r()),
        U8CPU::from(c.g()),
        U8CPU::from(c.b()),
    )
}

/// Packs 8-bit components into a [`PMColor`].
// Port of: src/core/SkColor.cpp#L27-L29 (chrome/m156)
#[doc(alias = "SkPMColorSetARGB")]
#[must_use]
pub fn pm_color_set_argb(a: u8, r: u8, g: u8, b: u8) -> PMColor {
    pack_argb32(
        U8CPU::from(a),
        U8CPU::from(r),
        U8CPU::from(g),
        U8CPU::from(b),
    )
}

/// Alpha of a [`PMColor`].
// Port of: src/core/SkColor.cpp#L31-L33 (chrome/m156)
#[doc(alias = "SkPMColorGetA")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // get_packed_* returns a value in 0..=255
pub fn pm_color_get_a(c: PMColor) -> Alpha {
    get_packed_a32(c) as u8
}

/// Red of a [`PMColor`].
// Port of: src/core/SkColor.cpp#L35-L37 (chrome/m156)
#[doc(alias = "SkPMColorGetR")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // get_packed_* returns a value in 0..=255
pub fn pm_color_get_r(c: PMColor) -> u8 {
    get_packed_r32(c) as u8
}

/// Green of a [`PMColor`].
// Port of: src/core/SkColor.cpp#L39-L41 (chrome/m156)
#[doc(alias = "SkPMColorGetG")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // get_packed_* returns a value in 0..=255
pub fn pm_color_get_g(c: PMColor) -> u8 {
    get_packed_g32(c) as u8
}

/// Blue of a [`PMColor`].
// Port of: src/core/SkColor.cpp#L43-L45 (chrome/m156)
#[doc(alias = "SkPMColorGetB")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // get_packed_* returns a value in 0..=255
pub fn pm_color_get_b(c: PMColor) -> u8 {
    get_packed_b32(c) as u8
}

/// Describes different color channels one can manipulate.
// Port of: include/core/SkColor.h#L248-L255 (chrome/m156)
#[doc(alias = "SkColorChannel")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(i32)]
pub enum ColorChannel {
    /// the red channel
    R = 0,
    /// the green channel
    G = 1,
    /// the blue channel
    B = 2,
    /// the alpha channel
    A = 3,
}

impl ColorChannel {
    /// `SkColorChannel::kLastEnum`.
    #[doc(alias = "kLastEnum")]
    pub const LAST_ENUM: Self = Self::A;
}

/// Used to represent the channels available in a color type or texture format as a mask.
// Port of: include/core/SkColor.h#L257-L270 (chrome/m156)
#[doc(alias = "SkColorChannelFlag")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ColorChannelFlag(u32);

impl ColorChannelFlag {
    pub const RED: Self = Self(1 << (ColorChannel::R as u32));
    pub const GREEN: Self = Self(1 << (ColorChannel::G as u32));
    pub const BLUE: Self = Self(1 << (ColorChannel::B as u32));
    pub const ALPHA: Self = Self(1 << (ColorChannel::A as u32));
    pub const GRAY: Self = Self(0x10);
    pub const GRAY_ALPHA: Self = Self(Self::GRAY.0 | Self::ALPHA.0);
    pub const RG: Self = Self(Self::RED.0 | Self::GREEN.0);
    pub const RGB: Self = Self(Self::RG.0 | Self::BLUE.0);
    pub const RGBA: Self = Self(Self::RGB.0 | Self::ALPHA.0);

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// True if all of `other`'s bits are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for ColorChannelFlag {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitAnd for ColorChannelFlag {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// static_assert(0 == (kGray_SkColorChannelFlag & kRGBA_SkColorChannelFlags), "bitfield conflict");
const _: () = assert!(0 == (ColorChannelFlag::GRAY.0 & ColorChannelFlag::RGBA.0));

// Port of: src/core/SkSwizzlePriv.h#L37-L39 (chrome/m156)
fn swizzle_rb(x: Float4) -> Float4 {
    shuffle(x, [2, 1, 0, 3])
}

// Port of: src/core/SkSwizzlePriv.h#L49-L51 (chrome/m156)
fn sk4f_from_l32(px: u32) -> Float4 {
    Byte4::load(&px.to_le_bytes()).cast::<f32>() * (1.0f32 / 255.0f32)
}

// Port of: src/core/SkSwizzlePriv.h#L53-L60 (chrome/m156)
fn sk4f_to_l32(px: Float4) -> u32 {
    // For the expected positive color values, the +0.5 before the pin and cast effectively rounds
    // to the nearest int without having to call round() or lrint().
    let bytes = (px * 255.0f32 + 0.5f32).pin(0.0f32, 255.0f32).cast::<u8>();
    let mut l32 = [0u8; 4];
    bytes.store(&mut l32);
    u32::from_le_bytes(l32)
}

fn load4(c: [f32; 4]) -> Float4 {
    Float4::load(&c)
}

fn store4(v: Float4) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    v.store(&mut out);
    out
}

// Shared implementation of `SkRGBA4f<kAT>`; the alpha-type specific parts live in the two impls
// that follow.
macro_rules! impl_rgba4f {
    ($name:ident) => {
        impl Mul<f32> for $name {
            type Output = Self;

            // Port of: include/core/SkColor.h#L318-L320 (chrome/m156)
            fn mul(self, scale: f32) -> Self {
                Self {
                    r: self.r * scale,
                    g: self.g * scale,
                    b: self.b * scale,
                    a: self.a * scale,
                }
            }
        }

        impl Mul for $name {
            type Output = Self;

            fn mul(self, scale: Self) -> Self {
                self.mul(&scale)
            }
        }

        impl Mul<&Self> for $name {
            type Output = Self;

            // Port of: include/core/SkColor.h#L326-L328 (chrome/m156)
            fn mul(self, scale: &Self) -> Self {
                Self {
                    r: self.r * scale.r,
                    g: self.g * scale.g,
                    b: self.b * scale.b,
                    a: self.a * scale.a,
                }
            }
        }

        impl Index<usize> for $name {
            type Output = f32;

            // Port of: include/core/SkColor.h#L343-L346 (chrome/m156)
            fn index(&self, index: usize) -> &f32 {
                match index {
                    0 => &self.r,
                    1 => &self.g,
                    2 => &self.b,
                    3 => &self.a,
                    _ => panic!("color component index out of range: {index}"),
                }
            }
        }

        impl IndexMut<usize> for $name {
            // Port of: include/core/SkColor.h#L352-L355 (chrome/m156)
            fn index_mut(&mut self, index: usize) -> &mut f32 {
                match index {
                    0 => &mut self.r,
                    1 => &mut self.g,
                    2 => &mut self.b,
                    3 => &mut self.a,
                    _ => panic!("color component index out of range: {index}"),
                }
            }
        }

        impl AsRef<Self> for $name {
            fn as_ref(&self) -> &Self {
                self
            }
        }

        impl $name {
            /// Creates a color from its components.
            #[must_use]
            pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
                Self { r, g, b, a }
            }

            /// The components as `[r, g, b, a]`.
            // Port of: include/core/SkColor.h#L334 (chrome/m156)
            #[doc(alias = "array")]
            #[must_use]
            pub const fn as_array(&self) -> [f32; 4] {
                [self.r, self.g, self.b, self.a]
            }

            /// True if alpha is exactly 1.
            // Port of: include/core/SkColor.h#L360-L363 (chrome/m156)
            #[doc(alias = "isOpaque")]
            #[must_use]
            #[allow(clippy::float_cmp)] // Skia compares exactly
            pub fn is_opaque(&self) -> bool {
                debug_assert!(self.a <= 1.0 && self.a >= 0.0);
                self.a == 1.0
            }

            /// True if r, g and b are all within `[0, 1]`.
            // Port of: include/core/SkColor.h#L365-L370 (chrome/m156)
            #[doc(alias = "fitsInBytes")]
            #[must_use]
            pub fn fits_in_bytes(&self) -> bool {
                debug_assert!(self.a >= 0.0 && self.a <= 1.0);
                self.r >= 0.0
                    && self.r <= 1.0
                    && self.g >= 0.0
                    && self.g <= 1.0
                    && self.b >= 0.0
                    && self.b <= 1.0
            }

            /// Bytes in RGBA order (like `GrColor`). The implementation is the same regardless of
            /// alpha type.
            // Port of: src/core/SkColor.cpp#L142-L145 (chrome/m156)
            #[doc(alias = "toBytes_RGBA")]
            #[must_use]
            pub fn to_bytes(self) -> u32 {
                sk4f_to_l32(load4(self.as_array()))
            }

            /// Inverse of [`Self::to_bytes`].
            // Port of: src/core/SkColor.cpp#L147-L152 (chrome/m156)
            #[doc(alias = "FromBytes_RGBA")]
            #[must_use]
            pub fn from_bytes_rgba(color: u32) -> Self {
                let [r, g, b, a] = store4(sk4f_from_l32(color));
                Self { r, g, b, a }
            }

            /// A copy with alpha set to 1.
            // Port of: include/core/SkColor.h#L426-L428 (chrome/m156)
            #[doc(alias = "makeOpaque")]
            #[must_use]
            pub fn to_opaque(self) -> Self {
                Self { a: 1.0, ..self }
            }

            /// A copy with alpha pinned to `[0, 1]`.
            // Port of: include/core/SkColor.h#L435-L437 (chrome/m156)
            #[doc(alias = "pinAlpha")]
            #[must_use]
            pub fn pin_alpha(self) -> Self {
                Self {
                    a: t_pin(self.a, 0.0f32, 1.0f32),
                    ..self
                }
            }

            /// A copy with alpha set to `a`.
            // Port of: include/core/SkColor.h#L439-L441 (chrome/m156)
            #[doc(alias = "withAlpha")]
            #[must_use]
            pub fn with_alpha(self, a: f32) -> Self {
                Self { a, ..self }
            }

            /// A copy with alpha set to `a / 255`.
            // Port of: include/core/SkColor.h#L443-L445 (chrome/m156)
            #[doc(alias = "withAlphaByte")]
            #[must_use]
            pub fn with_alpha_byte(self, a: u8) -> Self {
                Self {
                    a: f32::from(a) / 255.0f32,
                    ..self
                }
            }
        }
    };
}

/// RGBA color value, holding four floating point components, unpremultiplied
/// (`SkRGBA4f<kUnpremul_SkAlphaType>`, a.k.a. `SkColor4f`).
// Port of: include/core/SkColor.h#L282-L474 (chrome/m156)
#[doc(alias = "SkColor4f")]
#[doc(alias = "SkRGBA4f")]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// RGBA color value, holding four floating point components, premultiplied by alpha
/// (`SkRGBA4f<kPremul_SkAlphaType>`, a.k.a. `SkPMColor4f`).
// Port of: include/core/SkColor.h#L282-L474 (chrome/m156)
#[doc(alias = "SkPMColor4f")]
#[doc(alias = "SkRGBA4f")]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct PMColor4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl_rgba4f!(Color4f);
impl_rgba4f!(PMColor4f);

impl From<Color> for Color4f {
    fn from(color: Color) -> Self {
        Color4f::from_color(color)
    }
}

impl From<u32> for Color4f {
    fn from(color: u32) -> Self {
        Color::from(color).into()
    }
}

impl From<RGB> for Color4f {
    fn from(rgb: RGB) -> Self {
        Color::from(rgb).into()
    }
}

impl Color4f {
    /// Converts an [`Color`] (`0xAARRGGBB`) to a float color.
    // Port of: src/core/SkColor.cpp#L127-L131 (chrome/m156)
    #[doc(alias = "FromColor")]
    #[must_use]
    pub fn from_color(bgra: Color) -> Self {
        let [r, g, b, a] = store4(swizzle_rb(sk4f_from_l32(bgra.0)));
        Self { r, g, b, a }
    }

    /// Converts to a [`Color`], rounding each component to the nearest byte.
    // Port of: src/core/SkColor.cpp#L133-L136 (chrome/m156)
    #[doc(alias = "toSkColor")]
    #[must_use]
    pub fn to_color(self) -> Color {
        Color(sk4f_to_l32(swizzle_rb(load4(self.as_array()))))
    }

    /// Premultiplies r, g and b by alpha.
    // Port of: include/core/SkColor.h#L398-L401 (chrome/m156)
    #[must_use]
    pub fn premul(self) -> PMColor4f {
        PMColor4f {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }
}

impl PMColor4f {
    /// Converts a [`PMColor`] to a float color.
    // Port of: src/core/SkColor.cpp#L138-L142 (chrome/m156)
    #[doc(alias = "FromPMColor")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // r, g, b, a components
    pub fn from_pm_color(c: PMColor) -> Self {
        // swizzle_rb_if_bgra: SkPMColor is RGBA, so no swizzle.
        let [r, g, b, a] = store4(sk4f_from_l32(c));
        Self { r, g, b, a }
    }

    /// Divides r, g and b by alpha.
    // Port of: include/core/SkColor.h#L410-L420 (chrome/m156)
    #[must_use]
    pub fn unpremul(self) -> Color4f {
        if self.a == 0.0 {
            Color4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
        } else {
            let inv_alpha = 1.0f32 / self.a;
            Color4f {
                r: self.r * inv_alpha,
                g: self.g * inv_alpha,
                b: self.b * inv_alpha,
                a: self.a,
            }
        }
    }
}

/// Predefined [`Color4f`] values (`SkColors::`).
// Port of: include/core/SkColor.h#L455-L467 (chrome/m156)
#[doc(alias = "SkColors")]
pub mod colors {
    use super::Color4f;

    pub const TRANSPARENT: Color4f = Color4f::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color4f = Color4f::new(0.0, 0.0, 0.0, 1.0);
    pub const DARK_GREY: Color4f = Color4f::new(0.25, 0.25, 0.25, 1.0);
    pub const GREY: Color4f = Color4f::new(0.5, 0.5, 0.5, 1.0);
    pub const LIGHT_GREY: Color4f = Color4f::new(0.75, 0.75, 0.75, 1.0);
    pub const WHITE: Color4f = Color4f::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Color4f = Color4f::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color4f = Color4f::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color4f = Color4f::new(0.0, 0.0, 1.0, 1.0);
    pub const YELLOW: Color4f = Color4f::new(1.0, 1.0, 0.0, 1.0);
    pub const CYAN: Color4f = Color4f::new(0.0, 1.0, 1.0, 1.0);
    pub const MAGENTA: Color4f = Color4f::new(1.0, 0.0, 1.0, 1.0);
}
