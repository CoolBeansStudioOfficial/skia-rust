// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMipmap.{h,cpp}, src/core/SkMipmapHQDownSampler.cpp

//! `SkMipmap`: the levels below an image's base level, each half the size of the one above it,
//! and the filters that build them.
//!
//! Any function which deals with mipmap level indices starts with index 0 being the first
//! mipmap level which was generated. Said another way, it does not include the base level in
//! its range.
//!
//! skia-rust: the levels live in one `Vec<u8>` instead of a (possibly discardable)
//! `SkCachedData` block, so `SkDiscardableFactoryProc` and `SkCachedData` are not ported, and
//! [`Mipmap`] is immutable and shared with an `Arc`. The downsampler is the high quality one
//! (`SkMipmapHQDownSampler.cpp`) that Skia builds by default; the drawing downsampler
//! (`SK_USE_DRAWING_MIPMAP_DOWNSAMPLER`) is not.

use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::floating_point::{float_round2int, is_finite};
use crate::image_info::ImageInfo;
use crate::image_info_priv::color_type_min_row_bytes;
use crate::pixmap::Pixmap;
use crate::scalar::scalar_log2;
use crate::size::{ISize, Size};
use skia_rust_simd::rp::contexts::PixelBytes;
use skia_rust_simd::vx::{Vec as VxVec, from_half, to_half};
use std::sync::Arc;

/// One level of a [`Mipmap`] (`SkMipmap::Level`).
// Port of: src/core/SkMipmap.h#L58-L61 (chrome/m156)
#[doc(alias = "SkMipmap::Level")]
#[derive(Debug)]
pub struct MipmapLevel<'a> {
    /// `fPixmap`, with the mipmap's color space.
    pub pixmap: Pixmap<'a>,
    /// `fScale`: the size relative to the base level, below 1.0.
    pub scale: Size,
}

/// The stored description of a level.
#[derive(Clone, Debug)]
struct LevelRec {
    /// The level's info, without a color space.
    info: ImageInfo,
    row_bytes: usize,
    /// The offset of the level's pixels in [`Mipmap::data`].
    offset: usize,
    scale: Size,
}

/// A mipmap: the generated levels (not including the base level) of an image (`SkMipmap`).
// Port of: src/core/SkMipmap.h#L40-L98 (chrome/m156)
#[doc(alias = "SkMipmap")]
#[derive(Debug)]
pub struct Mipmap {
    cs: Option<ColorSpace>,
    levels: Vec<LevelRec>,
    data: Vec<u8>,
}

// The ColorTypeFilters: how a pixel is expanded into a larger type, with space between each
// component, so we can then perform our simple filter (either box or triangle) and store the
// intermediates in the expanded type.
//
// Memory is little-endian here, as Skia assumes throughout.

/// A pixel as stored in memory.
trait Pixel: Copy {
    const BYTES: usize;
    fn read(bytes: &[u8]) -> Self;
    fn write(self, bytes: &mut [u8]);
}

impl Pixel for u8 {
    const BYTES: usize = 1;
    fn read(bytes: &[u8]) -> u8 {
        bytes[0]
    }
    fn write(self, bytes: &mut [u8]) {
        bytes[0] = self;
    }
}

impl Pixel for u16 {
    const BYTES: usize = 2;
    fn read(bytes: &[u8]) -> u16 {
        u16::from_ne_bytes([bytes[0], bytes[1]])
    }
    fn write(self, bytes: &mut [u8]) {
        bytes[..2].copy_from_slice(&self.to_ne_bytes());
    }
}

impl Pixel for u32 {
    const BYTES: usize = 4;
    fn read(bytes: &[u8]) -> u32 {
        u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
    fn write(self, bytes: &mut [u8]) {
        bytes[..4].copy_from_slice(&self.to_ne_bytes());
    }
}

/// Four bytes in memory order (`skvx::byte4::Load`).
impl Pixel for [u8; 4] {
    const BYTES: usize = 4;
    fn read(bytes: &[u8]) -> [u8; 4] {
        [bytes[0], bytes[1], bytes[2], bytes[3]]
    }
    fn write(self, bytes: &mut [u8]) {
        bytes[..4].copy_from_slice(&self);
    }
}

/// Four 16-bit lanes in memory order (`skvx::half4::Load`, `Vec<4, uint16_t>::Load`).
impl Pixel for [u16; 4] {
    const BYTES: usize = 8;
    fn read(bytes: &[u8]) -> [u16; 4] {
        core::array::from_fn(|i| u16::read(&bytes[2 * i..]))
    }
    fn write(self, bytes: &mut [u8]) {
        for (i, lane) in self.into_iter().enumerate() {
            lane.write(&mut bytes[2 * i..]);
        }
    }
}

/// The expanded type a filter sums in: `+` is the C++ operator (wrapping, for the unsigned and
/// vector types), and the shifts are `shift_right`/`shift_left`.
trait Accum: Copy {
    fn add(self, other: Self) -> Self;
    fn shr(self, bits: u32) -> Self;
    fn shl(self, bits: u32) -> Self;
}

impl Accum for u32 {
    fn add(self, other: u32) -> u32 {
        self.wrapping_add(other)
    }
    fn shr(self, bits: u32) -> u32 {
        self >> bits
    }
    fn shl(self, bits: u32) -> u32 {
        self << bits
    }
}

impl Accum for u64 {
    fn add(self, other: u64) -> u64 {
        self.wrapping_add(other)
    }
    fn shr(self, bits: u32) -> u64 {
        self >> bits
    }
    fn shl(self, bits: u32) -> u64 {
        self << bits
    }
}

impl Accum for [u16; 4] {
    fn add(self, other: [u16; 4]) -> [u16; 4] {
        core::array::from_fn(|i| self[i].wrapping_add(other[i]))
    }
    fn shr(self, bits: u32) -> [u16; 4] {
        self.map(|x| x >> bits)
    }
    fn shl(self, bits: u32) -> [u16; 4] {
        self.map(|x| x << bits)
    }
}

impl Accum for [u32; 4] {
    fn add(self, other: [u32; 4]) -> [u32; 4] {
        core::array::from_fn(|i| self[i].wrapping_add(other[i]))
    }
    fn shr(self, bits: u32) -> [u32; 4] {
        self.map(|x| x >> bits)
    }
    fn shl(self, bits: u32) -> [u32; 4] {
        self.map(|x| x << bits)
    }
}

impl Accum for [f32; 4] {
    fn add(self, other: [f32; 4]) -> [f32; 4] {
        core::array::from_fn(|i| self[i] + other[i])
    }
    // Port of: src/core/SkMipmapHQDownSampler.cpp#L168-L170 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors `1.0f / (1 << bits)`
    fn shr(self, bits: u32) -> [f32; 4] {
        let scale = 1.0f32 / (1u32 << bits) as f32;
        self.map(|x| x * scale)
    }
    // Port of: src/core/SkMipmapHQDownSampler.cpp#L176-L178 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors `x * (1 << bits)`
    fn shl(self, bits: u32) -> [f32; 4] {
        let scale = (1u32 << bits) as f32;
        self.map(|x| x * scale)
    }
}

/// `ColorTypeFilter_*`.
trait ColorTypeFilter {
    type Px: Pixel;
    type Acc: Accum;
    fn expand(x: Self::Px) -> Self::Acc;
    fn compact(x: Self::Acc) -> Self::Px;
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L22-L32 (chrome/m156)
struct ColorTypeFilter8888;
impl ColorTypeFilter for ColorTypeFilter8888 {
    type Px = [u8; 4];
    type Acc = [u16; 4];
    fn expand(x: [u8; 4]) -> [u16; 4] {
        x.map(u16::from)
    }
    #[allow(clippy::cast_possible_truncation)] // skvx::cast<uint8_t> truncates
    fn compact(x: [u16; 4]) -> [u8; 4] {
        x.map(|v| v as u8)
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L34-L42 (chrome/m156)
struct ColorTypeFilter565;
impl ColorTypeFilter for ColorTypeFilter565 {
    type Px = u16;
    type Acc = u32;
    fn expand(x: u16) -> u32 {
        use crate::color_data::G16_MASK_IN_PLACE;
        let x = u32::from(x);
        (x & !G16_MASK_IN_PLACE) | ((x & G16_MASK_IN_PLACE) << 16)
    }
    #[allow(clippy::cast_possible_truncation)] // the C++ returns uint16_t
    fn compact(x: u32) -> u16 {
        use crate::color_data::G16_MASK_IN_PLACE;
        (((x & !G16_MASK_IN_PLACE) & 0xFFFF) | ((x >> 16) & G16_MASK_IN_PLACE)) as u16
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L44-L52 (chrome/m156)
struct ColorTypeFilter4444;
impl ColorTypeFilter for ColorTypeFilter4444 {
    type Px = u16;
    type Acc = u32;
    fn expand(x: u16) -> u32 {
        let x = u32::from(x);
        (x & 0xF0F) | ((x & !0xF0F) << 12)
    }
    #[allow(clippy::cast_possible_truncation)] // the C++ returns uint16_t
    fn compact(x: u32) -> u16 {
        ((x & 0xF0F) | ((x >> 12) & !0xF0F)) as u16
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L54-L62 (chrome/m156)
struct ColorTypeFilter8;
impl ColorTypeFilter for ColorTypeFilter8 {
    type Px = u8;
    type Acc = u32;
    fn expand(x: u8) -> u32 {
        u32::from(x)
    }
    #[allow(clippy::cast_possible_truncation)] // `(uint8_t)x`
    fn compact(x: u32) -> u8 {
        x as u8
    }
}

/// `from_half(skvx::half4::Load(&x))` for four lanes of half bits.
fn halves_to_float4(lanes: [u16; 4]) -> [f32; 4] {
    let f = from_half(VxVec::<4, u16>::load(&lanes));
    [f[0], f[1], f[2], f[3]]
}

/// `to_half(x).store(...)`: the four lanes of half bits.
fn float4_to_halves(x: [f32; 4]) -> [u16; 4] {
    let h = to_half(VxVec::<4, f32>::load(&x));
    [h[0], h[1], h[2], h[3]]
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L64-L73 (chrome/m156)
struct ColorTypeFilterF16;
impl ColorTypeFilter for ColorTypeFilterF16 {
    type Px = u16;
    type Acc = [f32; 4];
    fn expand(x: u16) -> [f32; 4] {
        // add 0s out to four lanes (0,0,0,x) (memory order x,0,0,0)
        halves_to_float4([x, 0, 0, 0])
    }
    fn compact(x: [f32; 4]) -> u16 {
        // but ignore the extra 3 here
        float4_to_halves(x)[0]
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L75-L84 (chrome/m156)
struct ColorTypeFilterRgbaF16;
impl ColorTypeFilter for ColorTypeFilterRgbaF16 {
    type Px = [u16; 4];
    type Acc = [f32; 4];
    fn expand(x: [u16; 4]) -> [f32; 4] {
        halves_to_float4(x)
    }
    fn compact(x: [f32; 4]) -> [u16; 4] {
        float4_to_halves(x)
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L86-L94 (chrome/m156)
struct ColorTypeFilter88;
impl ColorTypeFilter for ColorTypeFilter88 {
    type Px = u16;
    type Acc = u32;
    fn expand(x: u16) -> u32 {
        let x = u32::from(x);
        (x & 0xFF) | ((x & !0xFF) << 8)
    }
    #[allow(clippy::cast_possible_truncation)] // the C++ returns uint16_t
    fn compact(x: u32) -> u16 {
        ((x & 0xFF) | ((x >> 8) & !0xFF)) as u16
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L96-L104 (chrome/m156)
// skia-rust: this keeps Skia's arithmetic exactly: `Expand(uint32_t x)` computes
// `(x & 0xFFFF) | ((x & ~0xFFFF) << 16)` in 32 bits (the `<< 16` drops the high half) and
// `Compact` returns a `uint16_t`, so the second channel is lost, as in Skia.
struct ColorTypeFilter1616;
impl ColorTypeFilter for ColorTypeFilter1616 {
    type Px = u32;
    type Acc = u64;
    fn expand(x: u32) -> u64 {
        u64::from((x & 0xFFFF) | ((x & !0xFFFF) << 16))
    }
    #[allow(clippy::cast_possible_truncation)] // the C++ returns uint16_t
    fn compact(x: u64) -> u32 {
        u32::from(((x & 0xFFFF) | ((x >> 16) & !0xFFFF)) as u16)
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L106-L116 (chrome/m156)
struct ColorTypeFilterF16F16;
impl ColorTypeFilter for ColorTypeFilterF16F16 {
    type Px = u32;
    type Acc = [f32; 4];
    fn expand(x: u32) -> [f32; 4] {
        // add 0s out to four lanes (0,0,x,x)
        #[allow(clippy::cast_possible_truncation)] // the low and high halves
        halves_to_float4([x as u16, (x >> 16) as u16, 0, 0])
    }
    fn compact(x: [f32; 4]) -> u32 {
        // but ignore the extra 2 here
        let r = float4_to_halves(x);
        u32::from(r[0]) | (u32::from(r[1]) << 16)
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L118-L127 (chrome/m156)
struct ColorTypeFilter16161616;
impl ColorTypeFilter for ColorTypeFilter16161616 {
    type Px = [u16; 4];
    type Acc = [u32; 4];
    fn expand(x: [u16; 4]) -> [u32; 4] {
        x.map(u32::from)
    }
    #[allow(clippy::cast_possible_truncation)] // skvx::cast<uint16_t> truncates
    fn compact(x: [u32; 4]) -> [u16; 4] {
        x.map(|v| v as u16)
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L129-L137 (chrome/m156)
struct ColorTypeFilter16;
impl ColorTypeFilter for ColorTypeFilter16 {
    type Px = u16;
    type Acc = u32;
    fn expand(x: u16) -> u32 {
        u32::from(x)
    }
    #[allow(clippy::cast_possible_truncation)] // `(uint16_t) x`
    fn compact(x: u32) -> u16 {
        x as u16
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L139-L153 (chrome/m156)
struct ColorTypeFilter1010102;
impl ColorTypeFilter for ColorTypeFilter1010102 {
    type Px = u32;
    type Acc = u64;
    fn expand(x: u32) -> u64 {
        let x = u64::from(x);
        (x & 0x3ff)
            | (((x >> 10) & 0x3ff) << 20)
            | (((x >> 20) & 0x3ff) << 40)
            | (((x >> 30) & 0x3) << 60)
    }
    #[allow(clippy::cast_possible_truncation)] // the C++ returns uint32_t
    fn compact(x: u64) -> u32 {
        ((x & 0x3ff)
            | (((x >> 20) & 0x3ff) << 10)
            | (((x >> 40) & 0x3ff) << 20)
            | (((x >> 60) & 0x3) << 30)) as u32
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L155-L157 (chrome/m156)
fn add_121<A: Accum>(a: A, b: A, c: A) -> A {
    a.add(b).add(b).add(c)
}

// To produce each mip level, we need to filter down by 1/2 (e.g. 100x100 -> 50,50)
// If the starting dimension is odd, we floor the size of the lower level (e.g. 101 -> 50)
// In those (odd) cases, we use a triangle filter, with 1-pixel overlap between samplings,
// else for even cases, we just use a 2x box filter.
//
// This produces 4 possible isotropic filters: 2x2 2x3 3x2 3x3 where WxH indicates the number of
// src pixels we need to sample in each dimension to produce 1 dst pixel.
//
// OpenGL expects a full mipmap stack to contain anisotropic space as well.
// This means a 100x1 image would continue down to a 50x1 image, 25x1 image...
// Because of this, we need 4 more anisotropic filters: 1x2, 1x3, 2x1, 3x1.

/// `FilterProc`: `dst`, the first source row, the source row bytes and the destination count.
type FilterProc = fn(&mut [u8], &[u8], usize, usize);

/// Reads the pixel at column `i` of a row.
fn px<F: ColorTypeFilter>(row: &[u8], i: usize) -> F::Acc {
    F::expand(<F::Px as Pixel>::read(&row[i * <F::Px as Pixel>::BYTES..]))
}

/// Stores `d[i]`.
fn put<F: ColorTypeFilter>(dst: &mut [u8], i: usize, v: F::Acc, bits: u32) {
    F::compact(v.shr(bits)).write(&mut dst[i * <F::Px as Pixel>::BYTES..]);
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L181-L196 (chrome/m156)
fn downsample_1_2<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1) = (0, src_rb);
    for i in 0..count {
        let c00 = px::<F>(&src[p0..], 0);
        let c10 = px::<F>(&src[p1..], 0);

        let c = c00.add(c10);
        put::<F>(dst, i, c, 1);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L198-L215 (chrome/m156)
fn downsample_1_3<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1, mut p2) = (0, src_rb, 2 * src_rb);
    for i in 0..count {
        let c00 = px::<F>(&src[p0..], 0);
        let c10 = px::<F>(&src[p1..], 0);
        let c20 = px::<F>(&src[p2..], 0);

        let c = add_121(c00, c10, c20);
        put::<F>(dst, i, c, 2);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
        p2 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L217-L229 (chrome/m156)
fn downsample_2_1<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], _src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let mut p0 = 0;
    for i in 0..count {
        let c00 = px::<F>(&src[p0..], 0);
        let c01 = px::<F>(&src[p0..], 1);

        let c = c00.add(c01);
        put::<F>(dst, i, c, 1);
        p0 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L231-L250 (chrome/m156)
fn downsample_2_2<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1) = (0, src_rb);
    for i in 0..count {
        let c00 = px::<F>(&src[p0..], 0);
        let c01 = px::<F>(&src[p0..], 1);
        let c10 = px::<F>(&src[p1..], 0);
        let c11 = px::<F>(&src[p1..], 1);

        let c = c00.add(c10).add(c01).add(c11);
        put::<F>(dst, i, c, 2);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L252-L276 (chrome/m156)
fn downsample_2_3<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1, mut p2) = (0, src_rb, 2 * src_rb);
    for i in 0..count {
        let c00 = px::<F>(&src[p0..], 0);
        let c01 = px::<F>(&src[p0..], 1);
        let c10 = px::<F>(&src[p1..], 0);
        let c11 = px::<F>(&src[p1..], 1);
        let c20 = px::<F>(&src[p2..], 0);
        let c21 = px::<F>(&src[p2..], 1);

        let c = add_121(c00, c10, c20).add(add_121(c01, c11, c21));
        put::<F>(dst, i, c, 3);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
        p2 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L278-L295 (chrome/m156)
fn downsample_3_1<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], _src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let mut p0 = 0;
    let mut c02 = px::<F>(&src[p0..], 0);
    for i in 0..count {
        let c00 = c02;
        let c01 = px::<F>(&src[p0..], 1);
        c02 = px::<F>(&src[p0..], 2);

        let c = add_121(c00, c01, c02);
        put::<F>(dst, i, c, 2);
        p0 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L297-L335 (chrome/m156)
fn downsample_3_2<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1) = (0, src_rb);

    // Given pixels:
    // a0 b0 c0 d0 e0 ...
    // a1 b1 c1 d1 e1 ...
    // We want:
    // (a0 + 2*b0 + c0 + a1 + 2*b1 + c1) / 8
    // (c0 + 2*d0 + e0 + c1 + 2*d1 + e1) / 8
    // ...

    let mut c0 = px::<F>(&src[p0..], 0);
    let mut c1 = px::<F>(&src[p1..], 0);
    let mut c = c0.add(c1);
    for i in 0..count {
        let a = c;

        let b0 = px::<F>(&src[p0..], 1);
        let b1 = px::<F>(&src[p1..], 1);
        let b = b0.add(b0).add(b1).add(b1);

        c0 = px::<F>(&src[p0..], 2);
        c1 = px::<F>(&src[p1..], 2);
        c = c0.add(c1);

        let sum = a.add(b).add(c);
        put::<F>(dst, i, sum, 3);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
    }
}

// Port of: src/core/SkMipmapHQDownSampler.cpp#L337-L383 (chrome/m156)
fn downsample_3_3<F: ColorTypeFilter>(dst: &mut [u8], src: &[u8], src_rb: usize, count: usize) {
    debug_assert!(count > 0);
    let bpp = <F::Px as Pixel>::BYTES;
    let (mut p0, mut p1, mut p2) = (0, src_rb, 2 * src_rb);

    // Given pixels:
    // a0 b0 c0 d0 e0 ...
    // a1 b1 c1 d1 e1 ...
    // a2 b2 c2 d2 e2 ...
    // We want:
    // (a0 + 2*b0 + c0 + 2*a1 + 4*b1 + 2*c1 + a2 + 2*b2 + c2) / 16
    // (c0 + 2*d0 + e0 + 2*c1 + 4*d1 + 2*e1 + c2 + 2*d2 + e2) / 16
    // ...

    let mut c0 = px::<F>(&src[p0..], 0);
    let mut c1 = px::<F>(&src[p1..], 0);
    let mut c2 = px::<F>(&src[p2..], 0);
    let mut c = add_121(c0, c1, c2);
    for i in 0..count {
        let a = c;

        let b0 = px::<F>(&src[p0..], 1);
        let b1 = px::<F>(&src[p1..], 1);
        let b2 = px::<F>(&src[p2..], 1);
        let b = add_121(b0, b1, b2).shl(1);

        c0 = px::<F>(&src[p0..], 2);
        c1 = px::<F>(&src[p1..], 2);
        c2 = px::<F>(&src[p2..], 2);
        c = add_121(c0, c1, c2);

        let sum = a.add(b).add(c);
        put::<F>(dst, i, sum, 4);
        p0 += 2 * bpp;
        p1 += 2 * bpp;
        p2 += 2 * bpp;
    }
}

/// The eight filters of one color type (`proc_1_2` ... `proc_3_3`).
fn procs<F: ColorTypeFilter>() -> HqDownSampler {
    HqDownSampler {
        proc_1_2: downsample_1_2::<F>,
        proc_1_3: downsample_1_3::<F>,
        proc_2_1: downsample_2_1::<F>,
        proc_2_2: downsample_2_2::<F>,
        proc_2_3: downsample_2_3::<F>,
        proc_3_1: downsample_3_1::<F>,
        proc_3_2: downsample_3_2::<F>,
        proc_3_3: downsample_3_3::<F>,
    }
}

/// Builds a level from the one above it (`SkMipmapDownSampler`, here the `HQDownSampler`).
// Port of: src/core/SkMipmapHQDownSampler.cpp#L385-L438 (chrome/m156)
#[doc(alias = "SkMipmapDownSampler")]
#[doc(alias = "HQDownSampler")]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)] // Skia's `proc1_2` ... `proc3_3`
pub struct HqDownSampler {
    proc_1_2: FilterProc,
    proc_1_3: FilterProc,
    proc_2_1: FilterProc,
    proc_2_2: FilterProc,
    proc_2_3: FilterProc,
    proc_3_1: FilterProc,
    proc_3_2: FilterProc,
    proc_3_3: FilterProc,
}

impl core::fmt::Debug for HqDownSampler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HqDownSampler").finish_non_exhaustive()
    }
}

impl HqDownSampler {
    /// Builds the level `dst` (`dst_row_bytes` apart rows, `dst_width` x `dst_height`) from the
    /// level above it, `src` (`src_width` x `src_height`) (`buildLevel`).
    #[doc(alias = "buildLevel")]
    pub fn build_level(
        &self,
        dst: &mut [u8],
        dst_row_bytes: usize,
        dst_dims: ISize,
        src: &[u8],
        src_row_bytes: usize,
        src_dims: ISize,
    ) {
        let width = src_dims.width;
        let height = src_dims.height;

        let proc = if height & 1 != 0 {
            if height == 1 {
                // src-height is 1
                if width & 1 != 0 {
                    // src-width is 3
                    self.proc_3_1
                } else {
                    // src-width is 2
                    self.proc_2_1
                }
            } else {
                // src-height is 3
                if width & 1 != 0 {
                    if width == 1 {
                        // src-width is 1
                        self.proc_1_3
                    } else {
                        // src-width is 3
                        self.proc_3_3
                    }
                } else {
                    // src-width is 2
                    self.proc_2_3
                }
            }
        } else {
            // src-height is 2
            if width & 1 != 0 {
                if width == 1 {
                    // src-width is 1
                    self.proc_1_2
                } else {
                    // src-width is 3
                    self.proc_3_2
                }
            } else {
                // src-width is 2
                self.proc_2_2
            }
        };

        let mut src_base = 0;
        let mut dst_base = 0;
        let count = usize::try_from(dst_dims.width).unwrap_or(0);
        for _ in 0..dst_dims.height {
            proc(&mut dst[dst_base..], &src[src_base..], src_row_bytes, count);
            src_base += src_row_bytes * 2; // jump two rows
            dst_base += dst_row_bytes;
        }
    }
}

/// The downsampler for `root`'s color type, or `None` if its color type has none
/// (`SkMipmap::MakeDownSampler`).
// Port of: src/core/SkMipmapHQDownSampler.cpp#L440-L605 (chrome/m156)
#[doc(alias = "MakeDownSampler")]
#[must_use]
pub fn make_down_sampler(root: &Pixmap<'_>) -> Option<HqDownSampler> {
    Some(match root.color_type() {
        ColorType::RGBA8888 | ColorType::BGRA8888 => procs::<ColorTypeFilter8888>(),
        ColorType::RGB565 => procs::<ColorTypeFilter565>(),
        ColorType::ARGB4444 => procs::<ColorTypeFilter4444>(),
        ColorType::Alpha8 | ColorType::Gray8 | ColorType::R8UNorm => procs::<ColorTypeFilter8>(),
        ColorType::RGBAF16Norm | ColorType::RGBAF16 => procs::<ColorTypeFilterRgbaF16>(),
        ColorType::R8G8UNorm => procs::<ColorTypeFilter88>(),
        ColorType::R16G16UNorm => procs::<ColorTypeFilter1616>(),
        ColorType::A16UNorm | ColorType::R16UNorm => procs::<ColorTypeFilter16>(),
        ColorType::RGBA1010102 | ColorType::BGRA1010102 => procs::<ColorTypeFilter1010102>(),
        ColorType::A16Float | ColorType::R16Float => procs::<ColorTypeFilterF16>(),
        ColorType::R16G16Float => procs::<ColorTypeFilterF16F16>(),
        ColorType::R16G16B16A16UNorm => procs::<ColorTypeFilter16161616>(),

        ColorType::Unknown
        | ColorType::SRGBA8888 // TODO: needs careful handling
        | ColorType::RGB888x // TODO: use 8888?
        | ColorType::RGB101010x // TODO: use 1010102?
        | ColorType::BGR101010x // TODO: use 1010102?
        | ColorType::BGR101010xXR // TODO: use 1010102?
        | ColorType::RGBF16F16F16x // TODO: use F16?
        | ColorType::BGRA10101010XR
        | ColorType::RGBA10x6
        | ColorType::RGBAF32 => return None,

    })
}

impl Mipmap {
    /// Determines how many levels a mipmap will have without creating that mipmap. This does
    /// not include the base mipmap level that the user provided when creating the mipmap
    /// (`ComputeLevelCount`).
    // Port of: src/core/SkMipmap.cpp#L129-L162 (chrome/m156)
    #[doc(alias = "ComputeLevelCount")]
    #[must_use]
    pub fn compute_level_count(base_width: i32, base_height: i32) -> i32 {
        if base_width < 1 || base_height < 1 {
            return 0;
        }

        // OpenGL's spec requires that each mipmap level have height/width equal to
        // max(1, floor(original_height / 2^i)
        // (or original_width) where i is the mipmap level.
        // Continue scaling down until both axes are size 1.

        let largest_axis = base_width.max(base_height);
        if largest_axis < 2 {
            // SkMipmap::Build requires a minimum size of 2.
            return 0;
        }
        #[allow(clippy::cast_sign_loss)] // largest_axis >= 2
        let leading_zeros = (largest_axis as u32).leading_zeros();
        // If the value 00011010 has 3 leading 0s then it has 5 significant bits
        // (the bits which are not leading zeros)
        #[allow(clippy::cast_possible_wrap)] // at most 32
        let significant_bits = (u32::BITS - leading_zeros) as i32;
        // This is making the assumption that the size of a byte is 8 bits
        // and that sizeof(uint32_t)'s implementation-defined behavior is 4.
        let mut mip_level_count = significant_bits;

        // SkMipmap does not include the base mip level.
        // For example, it contains levels 1-x instead of 0-x.
        // This is because the image used to create SkMipmap is the base level.
        // So subtract 1 from the mip level count.
        if mip_level_count > 0 {
            mip_level_count -= 1;
        }

        mip_level_count
    }

    /// [`compute_level_count`](Self::compute_level_count) for a size.
    #[doc(alias = "ComputeLevelCount")]
    #[must_use]
    pub fn compute_level_count_size(s: ISize) -> i32 {
        Self::compute_level_count(s.width, s.height)
    }

    /// Determines the size of a given mipmap level. `level` is an index into the generated mipmap
    /// levels. It does not include the base level. So index 0 represents mipmap level 1
    /// (`ComputeLevelSize`).
    // Port of: src/core/SkMipmap.cpp#L164-L187 (chrome/m156)
    #[doc(alias = "ComputeLevelSize")]
    #[must_use]
    pub fn compute_level_size(base_width: i32, base_height: i32, level: i32) -> ISize {
        if base_width < 1 || base_height < 1 {
            return ISize::new(0, 0);
        }

        let max_level_count = Self::compute_level_count(base_width, base_height);
        if level >= max_level_count || level < 0 {
            return ISize::new(0, 0);
        }
        // OpenGL's spec requires that each mipmap level have height/width equal to
        // max(1, floor(original_height / 2^i)
        // (or original_width) where i is the mipmap level.

        // SkMipmap does not include the base mip level.
        // For example, it contains levels 1-x instead of 0-x.
        // This is because the image used to create SkMipmap is the base level.
        // So subtract 1 from the mip level to get the index stored by SkMipmap.
        let width = 1.max(base_width >> (level + 1));
        let height = 1.max(base_height >> (level + 1));

        ISize::new(width, height)
    }

    /// [`compute_level_size`](Self::compute_level_size) for a size.
    #[doc(alias = "ComputeLevelSize")]
    #[must_use]
    pub fn compute_level_size_of(s: ISize, level: i32) -> ISize {
        Self::compute_level_size(s.width, s.height, level)
    }

    /// Computes the fractional level based on the scaling in X and Y. `floor(level)` is the
    /// index of the larger level; `< 0` means failure (`ComputeLevel`).
    // Port of: src/core/SkMipmap.cpp#L191-L216 (chrome/m156)
    #[doc(alias = "ComputeLevel")]
    #[must_use]
    pub fn compute_level(scale_size: Size) -> f32 {
        debug_assert!(scale_size.width >= 0.0 && scale_size.height >= 0.0);

        // Use the smallest scale to match the GPU impl.
        let scale = scale_size.width.min(scale_size.height);

        if scale >= 1.0 || scale <= 0.0 || !is_finite(scale) {
            return -1.0;
        }

        // The -0.5 bias here is to emulate GPU's sharpen mipmap option.
        let l = (-scalar_log2(scale) - 0.5f32).max(0.0f32);
        if !is_finite(l) {
            return -1.0;
        }
        l
    }

    /// Builds a mipmap from `src`'s pixels, or `None` if it is too small, has no downsampler
    /// for its color type or the size overflows (`Build`). If `compute_contents` is false, the
    /// levels are allocated and sized but their pixels are zero.
    ///
    /// skia-rust: there is no discardable memory factory.
    // Port of: src/core/SkMipmap.cpp#L54-L127 (chrome/m156)
    #[must_use]
    pub fn build(src: &Pixmap<'_>, compute_contents: bool) -> Option<Mipmap> {
        if src.width() <= 1 && src.height() <= 1 {
            return None;
        }

        let ct = src.color_type();
        let at = src.alpha_type();

        // whip through our loop to compute the exact size needed
        let mut size: usize = 0;
        let count_levels = Self::compute_level_count(src.width(), src.height());
        for current_mip_level in (0..=count_levels).rev() {
            let mip_size = Self::compute_level_size(src.width(), src.height(), current_mip_level);
            size += color_type_min_row_bytes(ct, mip_size.width)
                * usize::try_from(mip_size.height).unwrap_or(0);
        }

        let storage_size = Self::alloc_levels_size(count_levels, size);
        if 0 == storage_size {
            return None;
        }

        let downsampler = if compute_contents {
            Some(make_down_sampler(src)?)
        } else {
            None
        };

        // init
        let mut mipmap = Mipmap {
            cs: src.info().color_space(),
            levels: Vec::with_capacity(usize::try_from(count_levels).unwrap_or(0)),
            data: vec![0; size],
        };

        let mut addr = 0;
        let mut width = src.width();
        let mut height = src.height();
        let mut src_offset: Option<usize> = None; // None: the source is `src`
        let (mut src_row_bytes, mut src_dims) = (src.row_bytes(), src.dimensions());

        for _ in 0..count_levels {
            width = 1.max(width >> 1);
            height = 1.max(height >> 1);
            let row_bytes = color_type_min_row_bytes(ct, width);

            // We make the Info w/o any colorspace, since that storage is not under our control,
            // and will not be deleted in a controlled fashion. When the caller is given the
            // pixmap for a given level, we augment this pixmap with fCS (which we do manage).
            let info = ImageInfo::new(ISize::new(width, height), ct, at, None);
            #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
            let scale = Size::new(
                width as f32 / src.width() as f32,
                height as f32 / src.height() as f32,
            );
            let offset = addr;
            let dst_dims = ISize::new(width, height);

            if let Some(downsampler) = &downsampler {
                match src_offset {
                    None => {
                        let src_bytes = src.addr()?;
                        downsampler.build_level(
                            &mut mipmap.data[offset..],
                            row_bytes,
                            dst_dims,
                            src_bytes,
                            src_row_bytes,
                            src_dims,
                        );
                    }
                    Some(prev) => {
                        let (head, tail) = mipmap.data.split_at_mut(offset);
                        downsampler.build_level(
                            tail,
                            row_bytes,
                            dst_dims,
                            &head[prev..],
                            src_row_bytes,
                            src_dims,
                        );
                    }
                }
            }
            mipmap.levels.push(LevelRec {
                info,
                row_bytes,
                offset,
                scale,
            });
            src_offset = Some(offset);
            src_row_bytes = row_bytes;
            src_dims = dst_dims;
            addr += row_bytes * usize::try_from(height).unwrap_or(0);
        }
        debug_assert_eq!(addr, size);

        Some(mipmap)
    }

    /// The size of the storage of `level_count` levels holding `pixel_size` bytes of pixels, or
    /// 0 if it does not fit an `int32_t` (`AllocLevelsSize`).
    // Port of: src/core/SkMipmap.cpp#L45-L52 (chrome/m156)
    fn alloc_levels_size(level_count: i32, pixel_size: usize) -> usize {
        const LEVEL_SIZE: i64 = 56;
        if level_count < 0 {
            return 0;
        }
        // sk_64_mul(levelCount + 1, sizeof(Level)) + pixelSize; the level record is a pixmap
        // and a size (Skia: 8-byte aligned, 56 bytes on 64-bit). The record size only decides
        // whether the total fits, which pixel sizes near 2 GiB would also fail.
        let size =
            i64::from(level_count + 1) * LEVEL_SIZE + i64::try_from(pixel_size).unwrap_or(i64::MAX);
        if i32::try_from(size).is_err() {
            return 0;
        }
        usize::try_from(size).unwrap_or(0)
    }

    /// Whether `level`'s scale would pick a level, and that level's pixmap and scale
    /// (`extractLevel`).
    // Port of: src/core/SkMipmap.cpp#L229-L250 (chrome/m156)
    #[doc(alias = "extractLevel")]
    #[must_use]
    pub fn extract_level(&self, scale_size: Size) -> Option<MipmapLevel<'_>> {
        if self.levels.is_empty() {
            return None;
        }

        let l = Self::compute_level(scale_size);
        let mut level = float_round2int(l);
        if level <= 0 {
            return None;
        }

        let count = self.count_levels();
        if level > count {
            level = count;
        }
        self.level_at(usize::try_from(level - 1).ok()?)
    }

    /// The number of mipmap levels generated (which does not include the base mipmap level)
    /// (`countLevels`).
    // Port of: src/core/SkMipmap.cpp#L285-L287 (chrome/m156)
    #[doc(alias = "countLevels")]
    #[must_use]
    pub fn count_levels(&self) -> i32 {
        i32::try_from(self.levels.len()).unwrap_or(i32::MAX)
    }

    /// The level at `index`, an index into the generated mipmap levels. It does not include the
    /// base level. So index 0 represents mipmap level 1 (`getLevel`).
    // Port of: src/core/SkMipmap.cpp#L289-L297 (chrome/m156)
    #[doc(alias = "getLevel")]
    #[must_use]
    pub fn get_level(&self, index: i32) -> Option<MipmapLevel<'_>> {
        if index < 0 {
            return None;
        }
        self.level_at(usize::try_from(index).ok()?)
    }

    /// The level record `index` with the mipmap's color space put back on its pixmap.
    fn level_at(&self, index: usize) -> Option<MipmapLevel<'_>> {
        let rec = self.levels.get(index)?;
        // need to augment with our colorspace
        let info = rec.info.with_color_space(self.cs.clone());
        let pixmap = Pixmap::new_readonly(&info, &self.data[rec.offset..], rec.row_bytes)?;
        Some(MipmapLevel {
            pixmap,
            scale: rec.scale,
        })
    }

    /// The writable level `index` for `SkMipmapBuilder::level`: the level's info with the
    /// mipmap's colour space, its row bytes and its pixel bytes.
    // Port of: src/core/SkMipmap.cpp#L289-L297 (chrome/m156), getLevel (writable, as the builder
    // hands out the level's pixmap)
    pub(crate) fn level_pixels_mut(
        &mut self,
        index: usize,
    ) -> Option<(ImageInfo, usize, &mut [u8])> {
        let rec = self.levels.get(index)?.clone();
        let info = rec.info.with_color_space(self.cs.clone());
        let len = rec.row_bytes * usize::try_from(rec.info.height()).ok()?;
        let bytes = self.data.get_mut(rec.offset..rec.offset + len)?;
        Some((info, rec.row_bytes, bytes))
    }

    /// Whether this mipmap can be attached to an image whose base level has the info `root`
    /// (`validForRootLevel`).
    // Port of: src/core/SkMipmap.cpp#L252-L276 (chrome/m156)
    #[doc(alias = "validForRootLevel")]
    #[must_use]
    pub fn valid_for_root_level(&self, root: &ImageInfo) -> bool {
        if self.levels.is_empty() {
            return false;
        }

        let dimension = root.dimensions();
        if dimension.width <= 1 && dimension.height <= 1 {
            return false;
        }

        if self.levels[0].info.width() != 1.max(dimension.width >> 1)
            || self.levels[0].info.height() != 1.max(dimension.height >> 1)
        {
            return false;
        }

        for level in &self.levels {
            if level.info.color_type() != root.color_type()
                || level.info.alpha_type() != root.alpha_type()
            {
                return false;
            }
        }
        true
    }

    /// The color space the levels are in.
    #[must_use]
    pub fn color_space(&self) -> Option<ColorSpace> {
        self.cs.clone()
    }

    /// The pixels of level `index`, from its first pixel on.
    #[must_use]
    pub fn level_bytes(&self, index: usize) -> Option<&[u8]> {
        let rec = self.levels.get(index)?;
        self.data.get(rec.offset..)
    }

    // Writable access to a level's pixels, for copying levels into an unfilled mipmap
    // (`getLevel` plus the writable pixmap in `copy_mipmaps`).
    pub(crate) fn level_bytes_mut(
        &mut self,
        index: usize,
    ) -> Option<(&ImageInfo, usize, &mut [u8])> {
        let rec = self.levels.get(index)?;
        let (info, row_bytes, offset) = (&rec.info, rec.row_bytes, rec.offset);
        Some((info, row_bytes, &mut self.data[offset..]))
    }
}

/// The pixels of one level of a shared [`Mipmap`], as the bytes a gather stage samples
/// ([`PixelBytes`]).
#[derive(Clone, Debug)]
pub struct MipLevelBytes {
    mips: Arc<Mipmap>,
    index: usize,
}

impl MipLevelBytes {
    /// The bytes of level `index` of `mips`.
    #[must_use]
    pub fn new(mips: Arc<Mipmap>, index: usize) -> MipLevelBytes {
        MipLevelBytes { mips, index }
    }
}

impl PixelBytes for MipLevelBytes {
    fn bytes(&self) -> &[u8] {
        self.mips.level_bytes(self.index).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha_type::AlphaType;

    fn pm_info(w: i32, h: i32, ct: ColorType) -> ImageInfo {
        ImageInfo::new(ISize::new(w, h), ct, AlphaType::Premul, None)
    }

    #[test]
    fn level_counts_and_sizes() {
        assert_eq!(Mipmap::compute_level_count(0, 5), 0);
        assert_eq!(Mipmap::compute_level_count(1, 1), 0);
        assert_eq!(Mipmap::compute_level_count(2, 1), 1);
        assert_eq!(Mipmap::compute_level_count(5, 3), 2);
        assert_eq!(Mipmap::compute_level_count(256, 256), 8);
        assert_eq!(Mipmap::compute_level_size(5, 3, 0), ISize::new(2, 1));
        assert_eq!(Mipmap::compute_level_size(5, 3, 1), ISize::new(1, 1));
        assert_eq!(Mipmap::compute_level_size(5, 3, 2), ISize::new(0, 0));
        assert_eq!(Mipmap::compute_level_size(5, 3, -1), ISize::new(0, 0));
    }

    #[test]
    fn compute_level() {
        assert!(Mipmap::compute_level(Size::new(1.0, 2.0)) < 0.0);
        assert!(Mipmap::compute_level(Size::new(0.0, 0.5)) < 0.0);
        assert_eq!(Mipmap::compute_level(Size::new(0.75, 0.9)), 0.0);
        assert_eq!(Mipmap::compute_level(Size::new(0.25, 0.9)), 1.5);
    }

    #[test]
    fn box_filter_8888() {
        // A 4x2 image: each 2x2 box averages (with truncation).
        let info = pm_info(4, 2, ColorType::RGBA8888);
        let mut bytes = vec![0u8; 4 * 4 * 2];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::try_from(i * 8).unwrap();
        }
        let src = Pixmap::new_readonly(&info, &bytes, 16).unwrap();
        let mips = Mipmap::build(&src, true).unwrap();
        assert_eq!(mips.count_levels(), 2);
        let l0 = mips.get_level(0).unwrap();
        assert_eq!(l0.pixmap.dimensions(), ISize::new(2, 1));
        let px = l0.pixmap.addr().unwrap();
        // First dst pixel: pixels 0, 1 of row 0 and 4, 5 of row 1; channel 0.
        let sum =
            u32::from(bytes[0]) + u32::from(bytes[4]) + u32::from(bytes[16]) + u32::from(bytes[20]);
        assert_eq!(u32::from(px[0]), sum >> 2);
        let l1 = mips.get_level(1).unwrap();
        assert_eq!(l1.pixmap.dimensions(), ISize::new(1, 1));
        assert!(mips.get_level(2).is_none());
        assert!(mips.valid_for_root_level(&info));
    }

    #[test]
    fn too_small_or_unsupported() {
        let info = pm_info(1, 1, ColorType::RGBA8888);
        let bytes = [0u8; 4];
        let src = Pixmap::new_readonly(&info, &bytes, 4).unwrap();
        assert!(Mipmap::build(&src, true).is_none());
        let info = pm_info(4, 4, ColorType::RGBAF32);
        let bytes = [0u8; 4 * 16 * 4];
        let src = Pixmap::new_readonly(&info, &bytes, 64).unwrap();
        assert!(Mipmap::build(&src, true).is_none());
        // ...but the sizes can still be computed.
        assert_eq!(Mipmap::build(&src, false).unwrap().count_levels(), 2);
    }
}
