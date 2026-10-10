// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/src/Transform_inl.h, modules/skcms/src/skcms_Transform.h,
//                   modules/skcms/src/skcms_TransformBaseline.cc

//! The transform pipeline: the stages of `Transform_inl.h` and the program that runs them,
//! as portable scalar code (the baseline path with `N == 1`).
//!
//! Every stage operates on one pixel's `r, g, b, a` floats. The C++ runs the same lane-wise
//! arithmetic on 4 to 16 pixels at a time; per-lane results are identical.

use crate::curve::MAX_TABLE_ENTRIES;
use crate::math::{cvt_i32, floorf_};
use crate::public::{A2B, B2A, ByteView, Curve, Matrix3x3, Matrix3x4, TransferFunction};

// Port of: modules/skcms/src/skcms_Transform.h#L21-L119 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    LoadA8,
    LoadG8,
    LoadGa88,
    Load4444,
    Load565,
    Load888,
    Load8888,
    Load1010102,
    Load101010xXr,
    Load10101010Xr,
    Load161616Le,
    Load16161616Le,
    Load161616Be,
    Load16161616Be,
    LoadHhh,
    LoadHhhh,
    LoadFff,
    LoadFfff,

    SwapRb,
    Clamp,
    Invert,
    ForceOpaque,
    Premul,
    Unpremul,
    Matrix3x3,
    Matrix3x4,

    LabToXyz,
    XyzToLab,

    GammaR,
    GammaG,
    GammaB,
    GammaA,
    GammaRgb,

    TfR,
    TfG,
    TfB,
    TfA,
    TfRgb,

    PqR,
    PqG,
    PqB,
    PqA,
    PqRgb,

    HlgR,
    HlgG,
    HlgB,
    HlgA,
    HlgRgb,
    HlgOotfScale,

    HlginvR,
    HlginvG,
    HlginvB,
    HlginvA,
    HlginvRgb,
    HlginvOotfScale,

    TableR,
    TableG,
    TableB,
    TableA,

    ClutA2B,
    ClutB2A,

    StoreA8,
    StoreG8,
    StoreGa88,
    Store4444,
    Store565,
    Store888,
    Store8888,
    Store1010102,
    Store161616Le,
    Store16161616Le,
    Store161616Be,
    Store16161616Be,
    Store101010xXr,
    Store10101010Xr,
    StoreHhh,
    StoreHhhh,
    StoreFff,
    StoreFfff,
}

/// The context pointer of a stage in C++ (`const void*`), as a typed reference.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Arg<'a> {
    None,
    Tf(&'a TransferFunction),
    Curve(&'a Curve),
    Matrix3x3(&'a Matrix3x3),
    Matrix3x4(&'a Matrix3x4),
    A2B(&'a A2B),
    B2A(&'a B2A),
}

/// One op of a program, with its context.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stage<'a> {
    pub op: Op,
    pub arg: Arg<'a>,
}

// The four channels of the pixel being processed.
#[derive(Clone, Copy)]
struct Regs {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

// ~~~~ Helpers of Transform_inl.h ~~~~

// When we convert from float to fixed point, it's very common to want to round,
// and for some reason compilers generate better code when converting to int32_t.
// To serve both those ends, we use this function to_fixed() instead of direct cast().
// Port of: modules/skcms/src/Transform_inl.h#L137 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors (U32)cast<I32>(f + 0.5f)
fn to_fixed(f: f32) -> u32 {
    cvt_i32(f + 0.5) as u32
}

// Port of: modules/skcms/src/Transform_inl.h#L148-L162 (chrome/m156)
fn f_from_half(half: u16) -> f32 {
    if crate::cpu::hardware_half() {
        return skia_rust_simd::half::cvtph2ps(half);
    }
    let wide = u32::from(half);
    // A half is 1-5-10 sign-exponent-mantissa, with 15 exponent bias.
    let s = wide & 0x8000;
    let em = wide ^ s;

    // Constructing the float is easy if the half is not denormalized.
    let norm = f32::from_bits(
        (s << 16)
            .wrapping_add(em << 13)
            .wrapping_add((127 - 15) << 23),
    );

    // Simply flush all denorm half floats to zero.
    if em < 0x0400 { 0.0 } else { norm }
}

// Port of: modules/skcms/src/Transform_inl.h#L180-L197 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors cast<U16>(U32)
fn half_from_f(f: f32) -> u16 {
    if crate::cpu::hardware_half() {
        return skia_rust_simd::half::cvtps2ph(f);
    }
    // A float is 1-8-23 sign-exponent-mantissa, with 127 exponent bias.
    let sem = f.to_bits();
    let s = sem & 0x8000_0000;
    let em = sem ^ s;

    // For simplicity we flush denorm half floats (including all denorm floats) to zero.
    (if em < 0x3880_0000 {
        0
    } else {
        (s >> 16)
            .wrapping_add(em >> 13)
            .wrapping_sub((127 - 15) << 10)
    }) as u16
}

// Port of: modules/skcms/src/Transform_inl.h#L218-L219 (chrome/m156)
fn min_(x: f32, y: f32) -> f32 {
    if x > y { y } else { x }
}
fn max_(x: f32, y: f32) -> f32 {
    if x < y { y } else { x }
}

// Port of: modules/skcms/src/Transform_inl.h#L250-L262 (chrome/m156)
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)] // mirrors cast<F>(I32)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn approx_log2(x: f32) -> f32 {
    // The first approximation of log2(x) is its exponent 'e', minus 127.
    let bits = x.to_bits() as i32;

    let e = bits as f32 * (1.0f32 / (1 << 23) as f32);

    // If we use the mantissa too we can refine the error signficantly.
    let m = f32::from_bits(((bits & 0x007f_ffff) | 0x3f00_0000) as u32);

    e - 124.225_514_990f32 - 1.498_030_302f32 * m - 1.725_879_990f32 / (0.352_088_706_8f32 + m)
}

// Port of: modules/skcms/src/Transform_inl.h#L264-L267 (chrome/m156)
#[allow(clippy::approx_constant)] // Skia's float literals kept verbatim
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn approx_log(x: f32) -> f32 {
    let ln2 = 0.693_147_18f32;
    ln2 * approx_log2(x)
}

// Port of: modules/skcms/src/Transform_inl.h#L269-L278 (chrome/m156)
#[allow(clippy::cast_possible_wrap, clippy::cast_precision_loss)] // mirrors cast<I32>(F)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn approx_exp2(x: f32) -> f32 {
    let fract = x - floorf_(x);

    let fbits = (1.0f32 * (1 << 23) as f32)
        * (x + 121.274_057_500f32 - 1.490_129_070f32 * fract
            + 27.728_023_300f32 / (4.842_525_68f32 - fract));
    // FInfBits is the bit pattern of +Inf as a float, 0x7f800000 == 2139095040.
    let bits = cvt_i32(min_(max_(fbits, 0.0), 2_139_095_040.0));

    f32::from_bits(bits as u32)
}

// Port of: modules/skcms/src/Transform_inl.h#L280-L283 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons with 0 and 1
fn approx_pow(x: f32, y: f32) -> f32 {
    if x == 0.0 || x == 1.0 {
        x
    } else {
        approx_exp2(approx_log2(x) * y)
    }
}

// Port of: modules/skcms/src/Transform_inl.h#L285-L288 (chrome/m156)
#[allow(clippy::approx_constant)] // Skia's float literals kept verbatim
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn approx_exp(x: f32) -> f32 {
    let log2_e = 1.442_695_040_888_963_4f32;
    approx_exp2(log2_e * x)
}

// Port of: modules/skcms/src/Transform_inl.h#L290-L294 (chrome/m156)
fn strip_sign(x: f32) -> (f32, u32) {
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    (f32::from_bits(bits ^ sign), sign)
}

// Port of: modules/skcms/src/Transform_inl.h#L296-L298 (chrome/m156)
fn apply_sign(x: f32, sign: u32) -> f32 {
    f32::from_bits(sign | x.to_bits())
}

// Return tf(x).
// Port of: modules/skcms/src/Transform_inl.h#L301-L312 (chrome/m156)
fn apply_tf(tf: &TransferFunction, x: f32) -> f32 {
    // Peel off the sign bit and set x = |x|.
    let (x, sign) = strip_sign(x);

    // The transfer function has a linear part up to d, exponential at d and after.
    let v = if x < tf.d {
        tf.c * x + tf.f
    } else {
        approx_pow(tf.a * x + tf.b, tf.g) + tf.e
    };

    // Tack the sign bit back on.
    apply_sign(v, sign)
}

// Return the gamma function (|x|^G with the original sign re-applied to x).
// Port of: modules/skcms/src/Transform_inl.h#L315-L319 (chrome/m156)
fn apply_gamma(tf: &TransferFunction, x: f32) -> f32 {
    let (x, sign) = strip_sign(x);
    apply_sign(approx_pow(x, tf.g), sign)
}

// Port of: modules/skcms/src/Transform_inl.h#L321-L331 (chrome/m156)
fn apply_pq(tf: &TransferFunction, x: f32) -> f32 {
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let x = f32::from_bits(bits ^ sign);

    let v = approx_pow(
        max_(tf.a + tf.b * approx_pow(x, tf.c), 0.0) / (tf.d + tf.e * approx_pow(x, tf.c)),
        tf.f,
    );

    f32::from_bits(sign | v.to_bits())
}

// Port of: modules/skcms/src/Transform_inl.h#L333-L345 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn apply_hlg(tf: &TransferFunction, x: f32) -> f32 {
    let (r, g, a, b, c, k) = (tf.a, tf.b, tf.c, tf.d, tf.e, tf.f + 1.0);
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let x = f32::from_bits(bits ^ sign);

    let v = if x * r <= 1.0 {
        approx_pow(x * r, g)
    } else {
        approx_exp((x - c) * a) + b
    };

    k * f32::from_bits(sign | v.to_bits())
}

// Port of: modules/skcms/src/Transform_inl.h#L347-L360 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn apply_hlginv(tf: &TransferFunction, x: f32) -> f32 {
    let (r, g, a, b, c, k) = (tf.a, tf.b, tf.c, tf.d, tf.e, tf.f + 1.0);
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let mut x = f32::from_bits(bits ^ sign);
    x /= k;

    let v = if x <= 1.0 {
        r * approx_pow(x, g)
    } else {
        a * approx_log(x - b) + c
    };

    f32::from_bits(sign | v.to_bits())
}

// Compute the luminance Y used in the HLG OOTF. This is equivalent to computing the dot product
// with the vector [0.2627 0.678  0.0593] in Rec2020 primaries, but is performed in the XYZD50
// space to simplify the pipeline.
// Port of: modules/skcms/src/Transform_inl.h#L365-L369 (chrome/m156)
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn compute_y_in_xyzd50(x: f32, y: f32, z: f32) -> f32 {
    -0.028_316_55f32 * x + 1.009_954_52f32 * y + 0.021_023_82f32 * z
}

// Port of: modules/skcms/src/Transform_inl.h#L589-L591 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors cast<F>(U8)
fn f_from_u8(v: u8) -> f32 {
    f32::from(v) * (1.0f32 / 255.0f32)
}

// Port of: modules/skcms/src/Transform_inl.h#L606-L608 (chrome/m156)
fn minus_1_ulp(v: f32) -> f32 {
    f32::from_bits(v.to_bits().wrapping_sub(1))
}

// The byte at `index` of the table (0 past the end of the underlying buffer).
fn gather_8(table: &ByteView, ix: i32) -> u8 {
    // A negative index wraps to a huge usize and reads as 0, like an out-of-range gather.
    #[allow(clippy::cast_sign_loss)]
    table.byte(ix as usize)
}

// The big-endian 16-bit value at the byte offset `off` of the table.
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn gather_16_be(table: &ByteView, off: i64) -> u16 {
    #[allow(clippy::cast_sign_loss)] // negative offsets wrap and read as 0
    let off = off as usize;
    u16::from_be_bytes([table.byte(off), table.byte(off.wrapping_add(1))])
}

// Port of: modules/skcms/src/Transform_inl.h#L610-L632 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors (float)(table_entries - 1) and cast<F>(I32)
fn table(curve: &Curve, v: f32) -> f32 {
    let (entries, table_8, table_16) = match curve {
        Curve::Table8 { entries, table } => (*entries, Some(table), None),
        Curve::Table16 { entries, table } => (*entries, None, Some(table)),
        // The table_* ops make the assumption that the curve is a table.
        Curve::Parametric(_) => return v,
    };
    // Clamp the input to [0,1], then scale to a table index.
    let ix = max_(0.0, min_(v, 1.0)) * entries.wrapping_sub(1) as f32;

    // We'll look up (equal or adjacent) entries at lo and hi, then lerp by t between the two.
    let lo = cvt_i32(ix);
    let hi = cvt_i32(minus_1_ulp(ix + 1.0));
    let t = ix - lo as f32; // i.e. the fractional part of ix.

    let (l, h);
    if let Some(table_8) = table_8 {
        l = f_from_u8(gather_8(table_8, lo));
        h = f_from_u8(gather_8(table_8, hi));
    } else if let Some(table_16) = table_16 {
        l = f32::from(gather_16_be(table_16, 2 * i64::from(lo))) * (1.0f32 / 65535.0f32);
        h = f32::from(gather_16_be(table_16, 2 * i64::from(hi))) * (1.0f32 / 65535.0f32);
    } else {
        return v;
    }
    l + (h - l) * t
}

// Port of: modules/skcms/src/Transform_inl.h#L634-L650 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::cast_possible_wrap)] // mirrors the C++ casts of the ported arithmetic
fn sample_clut_8(grid_8: &ByteView, ix: i32, channels: usize) -> [f32; 4] {
    // 3 channels: gather_24 reads 3 bytes per entry (plus a junk byte that is masked off);
    // 4 channels: gather_32.
    let base = i64::from(ix) * channels as i64;
    let mut out = [0.0f32; 4];
    for (c, o) in out.iter_mut().enumerate().take(channels) {
        #[allow(clippy::cast_sign_loss)] // negative offsets wrap and read as 0
        let off = (base + c as i64) as usize;
        *o = f32::from(grid_8.byte(off)) * (1.0f32 / 255.0f32);
    }
    out
}

// Port of: modules/skcms/src/Transform_inl.h#L652-L676 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // mirrors the C++ casts of the ported arithmetic
fn sample_clut_16(grid_16: &ByteView, ix: i32, channels: usize) -> [f32; 4] {
    // The data is big-endian 16-bit, `channels` values per entry.
    let base = i64::from(ix) * channels as i64;
    let mut out = [0.0f32; 4];
    for (c, o) in out.iter_mut().enumerate().take(channels) {
        *o = f32::from(gather_16_be(grid_16, 2 * (base + c as i64))) * (1.0f32 / 65535.0f32);
    }
    out
}

// Port of: modules/skcms/src/Transform_inl.h#L678-L771 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::cast_precision_loss)] // mirrors cast<F>(I32) and (float)(grid_points[i] - 1)
#[allow(clippy::cast_possible_wrap)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn clut(
    input_channels: u32,
    output_channels: u32,
    grid_points: [u8; 4],
    grid_8: Option<&ByteView>,
    grid_16: Option<&ByteView>,
    r: &mut f32,
    g: &mut f32,
    b: &mut f32,
    a: &mut f32,
) {
    let dim = input_channels as i32;
    if dim <= 0 || dim > 4 {
        return;
    }
    debug_assert!(output_channels == 3 || output_channels == 4);

    // For each of these arrays, think foo[2*dim], but we use foo[8] since we know dim <= 4.
    // Index contribution by dimension, first low from 0, then high from 4.
    let mut index = [0i32; 8];
    // Weight for each contribution, again first low, then high.
    let mut weight = [0.0f32; 8];

    // O(dim) work first: calculate index,weight from r,g,b,a.
    let inputs = [*r, *g, *b, *a];
    let mut stride = 1i32;
    for i in (0..dim as usize).rev() {
        // x is where we logically want to sample the grid in the i-th dimension.
        // We MUST clamp to [0,1] here to avoid negative indices.
        let x = max_(0.0, min_(inputs[i], 1.0)) * (i32::from(grid_points[i]) - 1) as f32;

        // But we can't index at floats.  lo and hi are the two integer grid points surrounding x.
        // i.e. trunc(x) == floor(x) here.
        let lo = cvt_i32(x);
        let hi = cvt_i32(minus_1_ulp(x + 1.0));
        // Notice how we fold in the accumulated stride across previous dimensions here.
        index[i] = lo.wrapping_mul(stride);
        index[i + 4] = hi.wrapping_mul(stride);
        stride = stride.wrapping_mul(i32::from(grid_points[i]));

        // We'll interpolate between those two integer grid points by t.
        let t = x - lo as f32; // i.e. fract(x)
        weight[i] = 1.0 - t;
        weight[i + 4] = t;
    }

    *r = 0.0;
    *g = 0.0;
    *b = 0.0;
    if output_channels == 4 {
        *a = 0.0;
    }

    // We'll sample 2^dim == 1<<dim table entries per pixel,
    // in all combinations of low and high in each dimension.
    for combo in 0..(1usize << dim) {
        // This loop can be done in any order.

        // Each of these upcoming (combo&N)*K expressions here evaluates to 0 or 4,
        // where 0 selects the low index contribution and its weight 1-t,
        // or 4 the high index contribution and its weight t.

        // Since 0<dim≤4, we can always just start off with the 0-th channel,
        // then handle the others conditionally.
        let mut ix = index[(combo & 1) * 4];
        let mut w = weight[(combo & 1) * 4];

        // The C++ switch falls through from case 3 to 2 to 1.
        if dim >= 4 {
            ix = ix.wrapping_add(index[3 + (combo & 8) / 2]);
            w *= weight[3 + (combo & 8) / 2];
        }
        if dim >= 3 {
            ix = ix.wrapping_add(index[2 + (combo & 4)]);
            w *= weight[2 + (combo & 4)];
        }
        if dim >= 2 {
            ix = ix.wrapping_add(index[1 + (combo & 2) * 2]);
            w *= weight[1 + (combo & 2) * 2];
        }

        let channels = output_channels as usize;
        let sample = if let Some(grid_8) = grid_8 {
            sample_clut_8(grid_8, ix, channels)
        } else if let Some(grid_16) = grid_16 {
            sample_clut_16(grid_16, ix, channels)
        } else {
            [0.0; 4]
        };
        // `A` is zero when there are only 3 output channels.
        let (rr, gg, bb, aa) = (sample[0], sample[1], sample[2], sample[3]);
        *r += w * rr;
        *g += w * gg;
        *b += w * bb;
        *a += w * aa;
    }
}

// ~~~~ Loads and stores ~~~~

fn le_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn le_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn le_u64(buf: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(b)
}

fn le_f32(buf: &[u8], off: usize) -> f32 {
    f32::from_bits(le_u32(buf, off))
}

fn put_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(buf: &mut [u8], off: usize, v: f32) {
    put_u32(buf, off, v.to_bits());
}

// Port of: modules/skcms/src/Transform_inl.h#L836-L1076 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors cast<F>(U8/U16/U32/U64)
#[allow(clippy::identity_op)] // mirrors the C++ expression
#[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
fn load(op: Op, src: &[u8], i: usize, regs: &mut Regs) {
    let Regs { r, g, b, a } = regs;
    match op {
        Op::LoadA8 => {
            *a = f_from_u8(src[i]);
        }
        Op::LoadG8 => {
            let v = f_from_u8(src[i]);
            *r = v;
            *g = v;
            *b = v;
        }
        Op::LoadGa88 => {
            let u16 = le_u16(src, 2 * i);
            let v = f32::from(u16 & 0xff) * (1.0f32 / 255.0f32);
            *r = v;
            *g = v;
            *b = v;
            *a = f32::from((u16 >> 8) & 0xff) * (1.0f32 / 255.0f32);
        }
        Op::Load4444 => {
            let abgr = le_u16(src, 2 * i);

            *r = f32::from((abgr >> 12) & 0xf) * (1.0f32 / 15.0f32);
            *g = f32::from((abgr >> 8) & 0xf) * (1.0f32 / 15.0f32);
            *b = f32::from((abgr >> 4) & 0xf) * (1.0f32 / 15.0f32);
            *a = f32::from(abgr & 0xf) * (1.0f32 / 15.0f32);
        }
        Op::Load565 => {
            let rgb = le_u16(src, 2 * i);

            *r = f32::from(rgb & (31 << 0)) * (1.0f32 / (31 << 0) as f32);
            *g = f32::from(rgb & (63 << 5)) * (1.0f32 / (63 << 5) as f32);
            *b = f32::from(rgb & (31 << 11)) * (1.0f32 / (31 << 11) as f32);
        }
        Op::Load888 => {
            let o = 3 * i;
            *r = f32::from(src[o]) * (1.0f32 / 255.0f32);
            *g = f32::from(src[o + 1]) * (1.0f32 / 255.0f32);
            *b = f32::from(src[o + 2]) * (1.0f32 / 255.0f32);
        }
        Op::Load8888 => {
            let rgba = le_u32(src, 4 * i);

            *r = (rgba & 0xff) as f32 * (1.0f32 / 255.0f32);
            *g = ((rgba >> 8) & 0xff) as f32 * (1.0f32 / 255.0f32);
            *b = ((rgba >> 16) & 0xff) as f32 * (1.0f32 / 255.0f32);
            *a = ((rgba >> 24) & 0xff) as f32 * (1.0f32 / 255.0f32);
        }
        Op::Load1010102 => {
            let rgba = le_u32(src, 4 * i);

            *r = (rgba & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
            *g = ((rgba >> 10) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
            *b = ((rgba >> 20) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
            *a = ((rgba >> 30) & 0x3) as f32 * (1.0f32 / 3.0f32);
        }
        Op::Load101010xXr => {
            // The subtraction is unsigned (and wraps) in the C++.
            let rgba = le_u32(src, 4 * i);
            *r = (rgba & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
            *g = ((rgba >> 10) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
            *b = ((rgba >> 20) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
        }
        Op::Load10101010Xr => {
            let rgba = le_u64(src, 8 * i);
            // Each channel is 16 bits, where the 6 low bits are padding.
            *r = ((rgba >> 6) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
            *g = ((rgba >> (16 + 6)) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
            *b = ((rgba >> (32 + 6)) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
            *a = ((rgba >> (48 + 6)) & 0x3ff).wrapping_sub(384) as f32 / 510.0f32;
        }
        Op::Load161616Le => {
            let o = 6 * i;
            *r = f32::from(le_u16(src, o)) * (1.0f32 / 65535.0f32);
            *g = f32::from(le_u16(src, o + 2)) * (1.0f32 / 65535.0f32);
            *b = f32::from(le_u16(src, o + 4)) * (1.0f32 / 65535.0f32);
        }
        Op::Load16161616Le => {
            let px = le_u64(src, 8 * i);

            *r = (px & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *g = ((px >> 16) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *b = ((px >> 32) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *a = ((px >> 48) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
        }
        Op::Load161616Be => {
            let o = 6 * i;
            // R,G,B are big-endian 16-bit, so byte swap them before converting to float.
            *r = f32::from(le_u16(src, o).swap_bytes()) * (1.0f32 / 65535.0f32);
            *g = f32::from(le_u16(src, o + 2).swap_bytes()) * (1.0f32 / 65535.0f32);
            *b = f32::from(le_u16(src, o + 4).swap_bytes()) * (1.0f32 / 65535.0f32);
        }
        Op::Load16161616Be => {
            let px = swap_endian_16x4(le_u64(src, 8 * i));

            *r = (px & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *g = ((px >> 16) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *b = ((px >> 32) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
            *a = ((px >> 48) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
        }
        Op::LoadHhh => {
            let o = 6 * i;
            *r = f_from_half(le_u16(src, o));
            *g = f_from_half(le_u16(src, o + 2));
            *b = f_from_half(le_u16(src, o + 4));
        }
        Op::LoadHhhh => {
            let px = le_u64(src, 8 * i);
            #[allow(clippy::cast_possible_truncation)] // mirrors cast<U16>(U64 & 0xffff)
            {
                *r = f_from_half((px & 0xffff) as u16);
                *g = f_from_half(((px >> 16) & 0xffff) as u16);
                *b = f_from_half(((px >> 32) & 0xffff) as u16);
                *a = f_from_half(((px >> 48) & 0xffff) as u16);
            }
        }
        Op::LoadFff => {
            let o = 12 * i;
            *r = le_f32(src, o);
            *g = le_f32(src, o + 4);
            *b = le_f32(src, o + 8);
        }
        Op::LoadFfff => {
            let o = 16 * i;
            *r = le_f32(src, o);
            *g = le_f32(src, o + 4);
            *b = le_f32(src, o + 8);
            *a = le_f32(src, o + 12);
        }
        _ => unreachable!("not a load op"),
    }
}

// Swap high and low bytes of 16-bit lanes, converting between big-endian and little-endian.
// Port of: modules/skcms/src/Transform_inl.h#L206-L209 (chrome/m156)
fn swap_endian_16x4(rgba: u64) -> u64 {
    ((rgba & 0x00ff_00ff_00ff_00ff) << 8) | ((rgba & 0xff00_ff00_ff00_ff00) >> 8)
}

// Port of: modules/skcms/src/Transform_inl.h#L1291-L1535 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors cast<U8/U16>(U32)
#[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
fn store(op: Op, dst: &mut [u8], i: usize, regs: &Regs) {
    let Regs { r, g, b, a } = *regs;
    match op {
        Op::StoreA8 => {
            dst[i] = to_fixed(a * 255.0) as u8;
        }
        Op::StoreG8 => {
            // g should be holding luminance (Y) (r,g,b ~~~> X,Y,Z)
            dst[i] = to_fixed(g * 255.0) as u8;
        }
        Op::StoreGa88 => {
            // g should be holding luminance (Y) (r,g,b ~~~> X,Y,Z)
            let v = (to_fixed(g * 255.0) as u16) | ((to_fixed(a * 255.0) << 8) as u16);
            put_u16(dst, 2 * i, v);
        }
        Op::Store4444 => {
            let v = ((to_fixed(r * 15.0) << 12) as u16)
                | ((to_fixed(g * 15.0) << 8) as u16)
                | ((to_fixed(b * 15.0) << 4) as u16)
                | (to_fixed(a * 15.0) as u16);
            put_u16(dst, 2 * i, v);
        }
        Op::Store565 => {
            let v = (to_fixed(r * 31.0) as u16)
                | ((to_fixed(g * 63.0) << 5) as u16)
                | ((to_fixed(b * 31.0) << 11) as u16);
            put_u16(dst, 2 * i, v);
        }
        Op::Store888 => {
            let o = 3 * i;
            dst[o] = to_fixed(r * 255.0) as u8;
            dst[o + 1] = to_fixed(g * 255.0) as u8;
            dst[o + 2] = to_fixed(b * 255.0) as u8;
        }
        Op::Store8888 => {
            let v = to_fixed(r * 255.0)
                | (to_fixed(g * 255.0) << 8)
                | (to_fixed(b * 255.0) << 16)
                | (to_fixed(a * 255.0) << 24);
            put_u32(dst, 4 * i, v);
        }
        Op::Store101010xXr => {
            let v = to_fixed((r * 510.0) + 384.0)
                | (to_fixed((g * 510.0) + 384.0) << 10)
                | (to_fixed((b * 510.0) + 384.0) << 20);
            put_u32(dst, 4 * i, v);
        }
        Op::Store10101010Xr => {
            // Each channel is 16 bits, where the 6 low bits are padding.
            let v = (u64::from(to_fixed((r * 510.0) + 384.0)) << 6)
                | (u64::from(to_fixed((g * 510.0) + 384.0)) << (16 + 6))
                | (u64::from(to_fixed((b * 510.0) + 384.0)) << (32 + 6))
                | (u64::from(to_fixed((a * 510.0) + 384.0)) << (48 + 6));
            put_u64(dst, 8 * i, v);
        }
        Op::Store1010102 => {
            let v = to_fixed(r * 1023.0)
                | (to_fixed(g * 1023.0) << 10)
                | (to_fixed(b * 1023.0) << 20)
                | (to_fixed(a * 3.0) << 30);
            put_u32(dst, 4 * i, v);
        }
        Op::Store161616Le => {
            let o = 6 * i;
            put_u16(dst, o, u16_from_f(r));
            put_u16(dst, o + 2, u16_from_f(g));
            put_u16(dst, o + 4, u16_from_f(b));
        }
        Op::Store16161616Le => {
            let px = u64::from(to_fixed(r * 65535.0))
                | (u64::from(to_fixed(g * 65535.0)) << 16)
                | (u64::from(to_fixed(b * 65535.0)) << 32)
                | (u64::from(to_fixed(a * 65535.0)) << 48);
            put_u64(dst, 8 * i, px);
        }
        Op::Store161616Be => {
            let o = 6 * i;
            let rr = to_fixed(r * 65535.0);
            let gg = to_fixed(g * 65535.0);
            let bb = to_fixed(b * 65535.0);
            put_u16(dst, o, (((rr & 0x00ff) << 8) | ((rr & 0xff00) >> 8)) as u16);
            put_u16(
                dst,
                o + 2,
                (((gg & 0x00ff) << 8) | ((gg & 0xff00) >> 8)) as u16,
            );
            put_u16(
                dst,
                o + 4,
                (((bb & 0x00ff) << 8) | ((bb & 0xff00) >> 8)) as u16,
            );
        }
        Op::Store16161616Be => {
            let px = u64::from(to_fixed(r * 65535.0))
                | (u64::from(to_fixed(g * 65535.0)) << 16)
                | (u64::from(to_fixed(b * 65535.0)) << 32)
                | (u64::from(to_fixed(a * 65535.0)) << 48);
            put_u64(dst, 8 * i, swap_endian_16x4(px));
        }
        Op::StoreHhh => {
            let o = 6 * i;
            put_u16(dst, o, half_from_f(r));
            put_u16(dst, o + 2, half_from_f(g));
            put_u16(dst, o + 4, half_from_f(b));
        }
        Op::StoreHhhh => {
            let px = u64::from(half_from_f(r))
                | (u64::from(half_from_f(g)) << 16)
                | (u64::from(half_from_f(b)) << 32)
                | (u64::from(half_from_f(a)) << 48);
            put_u64(dst, 8 * i, px);
        }
        Op::StoreFff => {
            let o = 12 * i;
            put_f32(dst, o, r);
            put_f32(dst, o + 4, g);
            put_f32(dst, o + 8, b);
        }
        Op::StoreFfff => {
            let o = 16 * i;
            put_f32(dst, o, r);
            put_f32(dst, o + 4, g);
            put_f32(dst, o + 8, b);
            put_f32(dst, o + 12, a);
        }
        _ => unreachable!("not a store op"),
    }
}

// 65535 == inf in FP16, so promote to FP32 before converting.
// Port of: modules/skcms/src/Transform_inl.h#L601-L604 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors cast<U16>(F)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
fn u16_from_f(v: f32) -> u16 {
    cvt_i32(v * 65535.0 + 0.5) as u16
}

fn is_store(op: Op) -> bool {
    matches!(
        op,
        Op::StoreA8
            | Op::StoreG8
            | Op::StoreGa88
            | Op::Store4444
            | Op::Store565
            | Op::Store888
            | Op::Store8888
            | Op::Store1010102
            | Op::Store161616Le
            | Op::Store16161616Le
            | Op::Store161616Be
            | Op::Store16161616Be
            | Op::Store101010xXr
            | Op::Store10101010Xr
            | Op::StoreHhh
            | Op::StoreHhhh
            | Op::StoreFff
            | Op::StoreFfff
    )
}

// Applies `f` to the channel(s) that `op` names. `ops` is [r, g, b, a, rgb].
fn apply_channels(op: Op, ops: [Op; 5], regs: &mut Regs, f: impl Fn(f32) -> f32) -> bool {
    if op == ops[0] {
        regs.r = f(regs.r);
    } else if op == ops[1] {
        regs.g = f(regs.g);
    } else if op == ops[2] {
        regs.b = f(regs.b);
    } else if op == ops[3] {
        regs.a = f(regs.a);
    } else if op == ops[4] {
        regs.r = f(regs.r);
        regs.g = f(regs.g);
        regs.b = f(regs.b);
    } else {
        return false;
    }
    true
}

// Runs one non-store stage.
// Port of: modules/skcms/src/Transform_inl.h#L1078-L1287 (chrome/m156)
#[allow(clippy::too_many_lines)] // one arm per op, as the STAGE()s in C++
#[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
fn exec_work_stage(stage: &Stage<'_>, src: &[u8], i: usize, regs: &mut Regs) {
    let op = stage.op;
    match op {
        Op::LoadA8
        | Op::LoadG8
        | Op::LoadGa88
        | Op::Load4444
        | Op::Load565
        | Op::Load888
        | Op::Load8888
        | Op::Load1010102
        | Op::Load101010xXr
        | Op::Load10101010Xr
        | Op::Load161616Le
        | Op::Load16161616Le
        | Op::Load161616Be
        | Op::Load16161616Be
        | Op::LoadHhh
        | Op::LoadHhhh
        | Op::LoadFff
        | Op::LoadFfff => load(op, src, i, regs),

        Op::SwapRb => {
            std::mem::swap(&mut regs.r, &mut regs.b);
        }

        Op::Clamp => {
            regs.r = max_(0.0, min_(regs.r, 1.0));
            regs.g = max_(0.0, min_(regs.g, 1.0));
            regs.b = max_(0.0, min_(regs.b, 1.0));
            regs.a = max_(0.0, min_(regs.a, 1.0));
        }

        Op::Invert => {
            regs.r = 1.0 - regs.r;
            regs.g = 1.0 - regs.g;
            regs.b = 1.0 - regs.b;
            regs.a = 1.0 - regs.a;
        }

        Op::ForceOpaque => {
            regs.a = 1.0;
        }

        Op::Premul => {
            regs.r *= regs.a;
            regs.g *= regs.a;
            regs.b *= regs.a;
        }

        Op::Unpremul => {
            let scale = if 1.0 / regs.a < f32::INFINITY {
                1.0 / regs.a
            } else {
                0.0
            };
            regs.r *= scale;
            regs.g *= scale;
            regs.b *= scale;
        }

        Op::Matrix3x3 => {
            let Arg::Matrix3x3(matrix) = stage.arg else {
                unreachable!("matrix_3x3 stage without a matrix")
            };
            let (r, g, b) = (regs.r, regs.g, regs.b);
            let rr = matrix.vals[0][0] * r + matrix.vals[0][1] * g + matrix.vals[0][2] * b;
            let gg = matrix.vals[1][0] * r + matrix.vals[1][1] * g + matrix.vals[1][2] * b;
            let bb = matrix.vals[2][0] * r + matrix.vals[2][1] * g + matrix.vals[2][2] * b;

            regs.r = rr;
            regs.g = gg;
            regs.b = bb;
        }

        Op::Matrix3x4 => {
            let Arg::Matrix3x4(matrix) = stage.arg else {
                unreachable!("matrix_3x4 stage without a matrix")
            };
            let (r, g, b) = (regs.r, regs.g, regs.b);
            let rr = matrix.vals[0][0] * r
                + matrix.vals[0][1] * g
                + matrix.vals[0][2] * b
                + matrix.vals[0][3];
            let gg = matrix.vals[1][0] * r
                + matrix.vals[1][1] * g
                + matrix.vals[1][2] * b
                + matrix.vals[1][3];
            let bb = matrix.vals[2][0] * r
                + matrix.vals[2][1] * g
                + matrix.vals[2][2] * b
                + matrix.vals[2][3];

            regs.r = rr;
            regs.g = gg;
            regs.b = bb;
        }

        Op::LabToXyz => {
            // The L*a*b values are in r,g,b, but normalized to [0,1].  Reconstruct them:
            let l = regs.r * 100.0;
            let a = regs.g * 255.0 - 128.0;
            let b = regs.b * 255.0 - 128.0;

            // Convert to CIE XYZ.
            let mut y = (l + 16.0) * (1.0f32 / 116.0f32);
            let mut x = y + a * (1.0f32 / 500.0f32);
            let mut z = y - b * (1.0f32 / 200.0f32);

            x = if x * x * x > 0.008_856f32 {
                x * x * x
            } else {
                (x - (16.0f32 / 116.0f32)) * (1.0f32 / 7.787f32)
            };
            y = if y * y * y > 0.008_856f32 {
                y * y * y
            } else {
                (y - (16.0f32 / 116.0f32)) * (1.0f32 / 7.787f32)
            };
            z = if z * z * z > 0.008_856f32 {
                z * z * z
            } else {
                (z - (16.0f32 / 116.0f32)) * (1.0f32 / 7.787f32)
            };

            // Adjust to XYZD50 illuminant, and stuff back into r,g,b for the next op.
            regs.r = x * 0.9642f32;
            regs.g = y;
            regs.b = z * 0.8249f32;
        }

        // As above, in reverse.
        Op::XyzToLab => {
            let mut x = regs.r * (1.0f32 / 0.9642f32);
            let mut y = regs.g;
            let mut z = regs.b * (1.0f32 / 0.8249f32);

            x = if x > 0.008_856f32 {
                approx_pow(x, 1.0f32 / 3.0f32)
            } else {
                x * 7.787f32 + (16.0f32 / 116.0f32)
            };
            y = if y > 0.008_856f32 {
                approx_pow(y, 1.0f32 / 3.0f32)
            } else {
                y * 7.787f32 + (16.0f32 / 116.0f32)
            };
            z = if z > 0.008_856f32 {
                approx_pow(z, 1.0f32 / 3.0f32)
            } else {
                z * 7.787f32 + (16.0f32 / 116.0f32)
            };

            let l = y * 116.0 - 16.0;
            let a = (x - y) * 500.0;
            let b = (y - z) * 200.0;

            regs.r = l * (1.0f32 / 100.0f32);
            regs.g = (a + 128.0) * (1.0f32 / 255.0f32);
            regs.b = (b + 128.0) * (1.0f32 / 255.0f32);
        }

        Op::GammaR | Op::GammaG | Op::GammaB | Op::GammaA | Op::GammaRgb => {
            let Arg::Tf(tf) = stage.arg else {
                unreachable!("gamma stage without a transfer function")
            };
            apply_channels(
                op,
                [Op::GammaR, Op::GammaG, Op::GammaB, Op::GammaA, Op::GammaRgb],
                regs,
                |v| apply_gamma(tf, v),
            );
        }

        Op::TfR | Op::TfG | Op::TfB | Op::TfA | Op::TfRgb => {
            let Arg::Tf(tf) = stage.arg else {
                unreachable!("tf stage without a transfer function")
            };
            apply_channels(
                op,
                [Op::TfR, Op::TfG, Op::TfB, Op::TfA, Op::TfRgb],
                regs,
                |v| apply_tf(tf, v),
            );
        }

        Op::PqR | Op::PqG | Op::PqB | Op::PqA | Op::PqRgb => {
            let Arg::Tf(tf) = stage.arg else {
                unreachable!("pq stage without a transfer function")
            };
            apply_channels(
                op,
                [Op::PqR, Op::PqG, Op::PqB, Op::PqA, Op::PqRgb],
                regs,
                |v| apply_pq(tf, v),
            );
        }

        Op::HlgR | Op::HlgG | Op::HlgB | Op::HlgA | Op::HlgRgb => {
            let Arg::Tf(tf) = stage.arg else {
                unreachable!("hlg stage without a transfer function")
            };
            apply_channels(
                op,
                [Op::HlgR, Op::HlgG, Op::HlgB, Op::HlgA, Op::HlgRgb],
                regs,
                |v| apply_hlg(tf, v),
            );
        }

        // Apply the HLG Reference OOTF, as described in ITU-R BT.2100-3 Table 5.
        Op::HlgOotfScale => {
            // Compute Y in the XYZD50 primaries.
            let y = compute_y_in_xyzd50(regs.r, regs.g, regs.b);

            // Apply the gamma of 1.2.
            let gamma_minus_1 = 0.2f32;
            let (y, sign) = strip_sign(y);
            let y_to_gamma_minus1 = apply_sign(approx_pow(y, gamma_minus_1), sign);
            regs.r *= y_to_gamma_minus1;
            regs.g *= y_to_gamma_minus1;
            regs.b *= y_to_gamma_minus1;

            // Scale to the reference peak white (1000 nits) to get display luminance. Then divide
            // by the HDR reference white (203 nits), to get a value in relative linear color space.
            regs.r *= 1000.0f32 / 203.0f32;
            regs.g *= 1000.0f32 / 203.0f32;
            regs.b *= 1000.0f32 / 203.0f32;
        }

        Op::HlginvR | Op::HlginvG | Op::HlginvB | Op::HlginvA | Op::HlginvRgb => {
            let Arg::Tf(tf) = stage.arg else {
                unreachable!("hlginv stage without a transfer function")
            };
            apply_channels(
                op,
                [
                    Op::HlginvR,
                    Op::HlginvG,
                    Op::HlginvB,
                    Op::HlginvA,
                    Op::HlginvRgb,
                ],
                regs,
                |v| apply_hlginv(tf, v),
            );
        }

        // Perform the inverse of the operation in hlg_ootf_scale.
        Op::HlginvOotfScale => {
            regs.r *= 203.0f32 / 1000.0f32;
            regs.g *= 203.0f32 / 1000.0f32;
            regs.b *= 203.0f32 / 1000.0f32;

            let gamma_inv_minus_1 = 1.0f32 / 1.2f32 - 1.0f32;
            let y = compute_y_in_xyzd50(regs.r, regs.g, regs.b);
            let (y, sign) = strip_sign(y);
            let y_to_gamma_minus1 = apply_sign(approx_pow(y, gamma_inv_minus_1), sign);

            regs.r *= y_to_gamma_minus1;
            regs.g *= y_to_gamma_minus1;
            regs.b *= y_to_gamma_minus1;
        }

        Op::TableR => {
            let Arg::Curve(curve) = stage.arg else {
                unreachable!("table stage without a curve")
            };
            regs.r = table(curve, regs.r);
        }
        Op::TableG => {
            let Arg::Curve(curve) = stage.arg else {
                unreachable!("table stage without a curve")
            };
            regs.g = table(curve, regs.g);
        }
        Op::TableB => {
            let Arg::Curve(curve) = stage.arg else {
                unreachable!("table stage without a curve")
            };
            regs.b = table(curve, regs.b);
        }
        Op::TableA => {
            let Arg::Curve(curve) = stage.arg else {
                unreachable!("table stage without a curve")
            };
            regs.a = table(curve, regs.a);
        }

        Op::ClutA2B => {
            let Arg::A2B(a2b) = stage.arg else {
                unreachable!("clut_A2B stage without an A2B")
            };
            // The A2B CLUT takes `a` by value: it is not changed by the lookup.
            let mut a = regs.a;
            clut(
                a2b.input_channels,
                a2b.output_channels,
                a2b.grid_points,
                a2b.grid_8.as_ref(),
                a2b.grid_16.as_ref(),
                &mut regs.r,
                &mut regs.g,
                &mut regs.b,
                &mut a,
            );

            if a2b.input_channels == 4 {
                // CMYK is opaque.
                regs.a = 1.0;
            }
        }

        Op::ClutB2A => {
            let Arg::B2A(b2a) = stage.arg else {
                unreachable!("clut_B2A stage without a B2A")
            };
            clut(
                b2a.input_channels,
                b2a.output_channels,
                b2a.grid_points,
                b2a.grid_8.as_ref(),
                b2a.grid_16.as_ref(),
                &mut regs.r,
                &mut regs.g,
                &mut regs.b,
                &mut regs.a,
            );
        }

        _ => unreachable!("store ops are handled by the caller"),
    }
}

// Port of: modules/skcms/src/Transform_inl.h#L1539-L1560 (chrome/m156)
fn exec_stages(program: &[Stage<'_>], src: &[u8], dst: &mut [u8], i: usize) {
    let mut regs = Regs {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    for stage in program {
        if is_store(stage.op) {
            store(stage.op, dst, i, &regs);
            return;
        }
        exec_work_stage(stage, src, i, &mut regs);
    }
}

/// Runs `program` over `n` pixels. `src` must hold `n * src_bpp` bytes and `dst` `n * dst_bpp`.
// Port of: modules/skcms/src/Transform_inl.h#L1563-L1602 (chrome/m156)
pub(crate) fn run_program(program: &[Stage<'_>], src: &[u8], dst: &mut [u8], n: usize) {
    for i in 0..n {
        exec_stages(program, src, dst, i);
    }
}

// The table_* ops make this assumption.
const _: () = assert!(MAX_TABLE_ENTRIES == 1 << 24);
