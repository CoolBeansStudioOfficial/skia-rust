// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` transcendental functions: `sin`…`atan2`, `pow`/`exp`/`log`, `sqrt`, `invsqrt`,
//! `inverse_mat2/3/4`.
//!
//! Owner: task B6c (`docs/design/raster-pipeline.md` §5). These are Skia's own polynomial
//! approximations, ported exactly (never libm). The helpers (`sin_`, `approx_pow2`, …) are
//! private to this file, like the stage-level functions of `SkRasterPipeline_opts.h` they port.

#![allow(
    // Ported constants keep the digits Skia's source has.
    clippy::excessive_precision,
    // `ln2` and `log2_e` are Skia's literals (they round to the std constants).
    // Ported polynomials name their coefficients c0, c1, ….
    clippy::approx_constant,
    clippy::similar_names,
    clippy::many_single_char_names
)]

#[allow(clippy::wildcard_imports)]
use super::*;

use core::f32::consts::PI;

/// Bytes of one `F` register (`N` floats).
const F_BYTES: usize = 4 * N;

si! {
    /// Register `index` of the `F` array at `ptr` (`F* dst; dst[index]`).
    fn load_f(e: &Params<'_, '_>, ptr: MemPtr, index: usize) -> F {
        F::load_bytes(&e.ptr(ptr)[index * F_BYTES..])
    }

    /// `dst[index] = v` for the `F` array at `ptr`.
    fn store_f(e: &mut Params<'_, '_>, ptr: MemPtr, index: usize, v: F) {
        v.store_bytes(&mut e.ptr_mut(ptr)[index * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1608 (chrome/m156)
    fn fract(v: F) -> F {
        v - floor_(v)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1610-L1619 (chrome/m156)
    // See http://www.machinedlearnings.com/2011/06/fast-approximate-logarithm-exponential.html
    fn approx_log2(x: F) -> F {
        // e - 127 is a fair approximation of log2(x) in its own right...
        let xu: U32 = x.bit_cast();
        let e = cast_f(xu) * (1.0 / 8_388_608.0); // 1.0f / (1<<23)

        // ... but using the mantissa to refine its error is _much_ better.
        let m: F = ((xu & 0x007f_ffff) | 0x3f00_0000).bit_cast();

        nmad(m, F::splat(1.498_030_302), e - 124.225_514_990)
            - F::splat(1.725_879_990) / (m + 0.352_088_706_8)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1621-L1624 (chrome/m156)
    fn approx_log(x: F) -> F {
        let ln2 = 0.693_147_18_f32;
        F::splat(ln2) * approx_log2(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1626-L1636 (chrome/m156)
    fn approx_pow2(x: F) -> F {
        let k_infinity_bits = 2_139_095_040.0_f32; // 0x7f800000

        let f = fract(x);
        let mut approx = nmad(f, F::splat(1.490_129_070), x + 121.274_057_500);
        approx += F::splat(27.728_023_300) / (F::splat(4.842_525_68) - f);
        approx *= 8_388_608.0; // 1.0f * (1<<23)
        // guard against underflow/overflow
        approx = min_f(max_f(approx, F::splat(0.0)), F::splat(k_infinity_bits));

        round(approx).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1638-L1641 (chrome/m156)
    fn approx_exp(x: F) -> F {
        let log2_e = 1.442_695_040_888_963_407_4_f32;
        approx_pow2(F::splat(log2_e) * x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1643-L1646 (chrome/m156)
    fn approx_powf(x: F, y: F) -> F {
        if_then_else_f(
            x.eq_mask(0.0) | x.eq_mask(1.0),
            x,
            approx_pow2(approx_log2(x) * y),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2097-L2108 (chrome/m156)
    // Polynomial approximation of degree 5 for sin(x * 2 * pi) in the range [-1/4, 1/4]
    // Adapted from https://github.com/google/swiftshader/blob/master/docs/Sin-Cos-Optimization.pdf
    fn sin5q_(x: F) -> F {
        // A * x + B * x^3 + C * x^5
        // Exact at x = 0, 1/12, 1/6, 1/4, and their negatives,
        // which correspond to x * 2 * pi = 0, pi/6, pi/3, pi/2
        let a = 6.282_308_58_f32;
        let b = -41.169_368_7_f32;
        let c = 74.438_888_5_f32;
        let x2 = x * x;
        x * mad(mad(x2, F::splat(c), F::splat(b)), x2, F::splat(a))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2110-L2115 (chrome/m156)
    fn sin_(x: F) -> F {
        let one_over_pi2 = 1.0 / (2.0 * PI);
        let x = mad(x, F::splat(-one_over_pi2), F::splat(0.25));
        let x = F::splat(0.25) - abs_f(x - floor_(x + 0.5));
        sin5q_(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2117-L2122 (chrome/m156)
    fn cos_(x: F) -> F {
        let one_over_pi2 = 1.0 / (2.0 * PI);
        let x = x * one_over_pi2;
        let x = F::splat(0.25) - abs_f(x - floor_(x + 0.5));
        sin5q_(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2124-L2169 (chrome/m156)
    // "GENERATING ACCURATE VALUES FOR THE TANGENT FUNCTION"
    // https://mae.ufl.edu/~uhk/ACCURATE-TANGENT.pdf
    //
    // approx = x + (1/3)x^3 + (2/15)x^5 + (17/315)x^7 + (62/2835)x^9
    //
    // Some simplifications:
    // 1. tan(x) is periodic, -PI/2 < x < PI/2
    // 2. tan(x) is odd, so tan(-x) = -tan(x)
    // 3. Our polynomial approximation is best near zero, so we use the identity
    //    tan(x + y) = (tan(x) + tan(y)) / (1 - tan(x)*tan(y)), tan(PI/4) = 1.
    //    So for x > PI/8, we do the following refactor: x' = x - PI/4,
    //    tan(x) = (1 + tan(x')) / (1 - tan(x'))
    fn tan_(x: F) -> F {
        // periodic between -pi/2 ... pi/2
        // shift to 0...Pi, scale 1/Pi to get into 0...1, then fract, scale-up, shift-back
        let mut x = mad(
            fract(mad(x, F::splat(1.0 / PI), F::splat(0.5))),
            F::splat(PI),
            F::splat(-PI / 2.0),
        );

        let neg = x.lt_mask(0.0);
        x = if_then_else_f(neg, -x, x);

        // minimize total error by shifting if x > pi/8
        let use_quotient = x.gt_mask(PI / 8.0);
        x = if_then_else_f(use_quotient, x - (PI / 4.0), x);

        // 9th order poly = 4th order(x^2) * x
        let c4 = 62.0 / 2835.0_f32;
        let c3 = 17.0 / 315.0_f32;
        let c2 = 2.0 / 15.0_f32;
        let c1 = 1.0 / 3.0_f32;
        let c0 = 1.0_f32;
        let x2 = x * x;
        x *= mad(
            x2,
            mad(
                x2,
                mad(x2, mad(x2, F::splat(c4), F::splat(c3)), F::splat(c2)),
                F::splat(c1),
            ),
            F::splat(c0),
        );
        x = if_then_else_f(
            use_quotient,
            (F::splat(1.0) + x) / (F::splat(1.0) - x),
            x,
        );
        x = if_then_else_f(neg, -x, x);
        x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2171-L2187 (chrome/m156)
    // Use 4th order polynomial approximation from https://arachnoid.com/polysolve/
    // with 129 values of x,atan(x) for x:[0...1]. This only works for 0 <= x <= 1.
    fn approx_atan_unit(x: F) -> F {
        // y =   0.14130025741326729 x⁴
        //     - 0.34312835980675116 x³
        //     - 0.016172900528248768 x²
        //     + 1.00376969762003850 x
        //     - 0.00014758242182738969
        let c4 = 0.141_300_257_413_267_29_f32;
        let c3 = -0.343_128_359_806_751_16_f32;
        let c2 = -0.016_172_900_528_248_768_f32;
        let c1 = 1.003_769_697_620_038_5_f32;
        let c0 = -0.000_147_582_421_827_389_69_f32;
        mad(
            x,
            mad(
                x,
                mad(x, mad(x, F::splat(c4), F::splat(c3)), F::splat(c2)),
                F::splat(c1),
            ),
            F::splat(c0),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2189-L2199 (chrome/m156)
    // Use identity atan(x) = pi/2 - atan(1/x) for x > 1
    fn atan_(x: F) -> F {
        let neg = x.lt_mask(0.0);
        let mut x = if_then_else_f(neg, -x, x);
        let flip = x.gt_mask(1.0);
        x = if_then_else_f(flip, F::splat(1.0) / x, x);
        x = approx_atan_unit(x);
        x = if_then_else_f(flip, F::splat(PI / 2.0) - x, x);
        x = if_then_else_f(neg, -x, x);
        x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2201-L2215 (chrome/m156)
    // Handbook of Mathematical Functions, by Milton Abramowitz and Irene Stegun.
    fn asin_(x: F) -> F {
        let neg = x.lt_mask(0.0);
        let mut x = if_then_else_f(neg, -x, x);
        let c3 = -0.018_729_3_f32;
        let c2 = 0.074_261_0_f32;
        let c1 = -0.212_114_4_f32;
        let c0 = 1.570_728_8_f32;
        let poly = mad(
            x,
            mad(x, mad(x, F::splat(c3), F::splat(c2)), F::splat(c1)),
            F::splat(c0),
        );
        x = nmad(sqrt_(F::splat(1.0) - x), poly, F::splat(PI / 2.0));
        x = if_then_else_f(neg, -x, x);
        x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2217-L2219 (chrome/m156)
    fn acos_(x: F) -> F {
        F::splat(PI / 2.0) - asin_(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2221-L2243 (chrome/m156)
    // Use identity atan(x) = pi/2 - atan(1/x) for x > 1
    // By swapping y,x to ensure the ratio is <= 1, we can safely call atan_unit()
    // which avoids a 2nd divide instruction if we had instead called atan().
    fn atan2_(y0: F, x0: F) -> F {
        let flip = abs_f(y0).gt_mask(abs_f(x0));
        let y = if_then_else_f(flip, x0, y0);
        let x = if_then_else_f(flip, y0, x0);
        let mut arg = y / x;

        let neg = arg.lt_mask(0.0);
        arg = if_then_else_f(neg, -arg, arg);

        let mut r = approx_atan_unit(arg);
        r = if_then_else_f(flip, F::splat(PI / 2.0) - r, r);
        r = if_then_else_f(neg, -r, r);

        // handle quadrant distinctions
        r = if_then_else_f(y0.ge_mask(0.0) & x0.lt_mask(0.0), r + PI, r);
        r = if_then_else_f(y0.lt_mask(0.0) & x0.le_mask(0.0), r - PI, r);
        // Note: we don't try to handle 0,0 or infinities
        r
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4726-L4732, L4770-L4772 (chrome/m156)
    /// `apply_adjacent_unary` of `invsqrt_fn` over the `n` registers at `dst`.
    fn invsqrt_n(dst: MemPtr, e: &mut Params<'_, '_>, n: usize) {
        for i in 0..n {
            let v = rsqrt(load_f(e, dst, i));
            store_f(e, dst, i, v);
        }
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4804-L4815 (chrome/m156)
/// `*dst = $f(*dst)` for the register at `$dst` (the 1-slot complex unary stages). A macro, not
/// a function taking `$f`, because `#[target_feature]` functions cannot be passed as `Fn`.
macro_rules! apply_unary {
    ($dst:expr, $e:expr, $f:ident) => {{
        let v = $f(load_f($e, $dst, 0));
        store_f($e, $dst, 0, v);
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4897-L4915 (chrome/m156)
/// `apply_adjacent_binary_packed`: `dst[i] = $f(dst[i], src[i])` for the registers from
/// `base + ctx.dst` up to `base + ctx.src`.
macro_rules! apply_adjacent_binary_packed {
    ($ctx:expr, $e:expr, $f:ident) => {{
        let ctx = $ctx;
        let dst = $e.base_ptr(ctx.dst);
        let src = $e.base_ptr(ctx.src);
        let bytes = ctx
            .src
            .checked_sub(ctx.dst)
            .expect("raster pipeline: binary op source before destination") as usize;
        // `do { ... } while (dst != end)`: at least one register.
        for i in 0..(bytes / F_BYTES).max(1) {
            let d = load_f($e, dst, i);
            let s = load_f($e, src, i);
            store_f($e, dst, i, $f(d, s));
        }
    }};
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L4774-L4778, L4797 (chrome/m156)
    pub(super) fn invsqrt_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        invsqrt_n(ctx, e, 1);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4774-L4778, L4797 (chrome/m156)
    pub(super) fn invsqrt_2_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        invsqrt_n(ctx, e, 2);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4774-L4778, L4797 (chrome/m156)
    pub(super) fn invsqrt_3_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        invsqrt_n(ctx, e, 3);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4774-L4778, L4797 (chrome/m156)
    pub(super) fn invsqrt_4_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        invsqrt_n(ctx, e, 4);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4817-L4826 (chrome/m156)
    pub(super) fn inverse_mat2(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let (a00, a01) = (load_f(e, ctx, 0), load_f(e, ctx, 1));
        let (a10, a11) = (load_f(e, ctx, 2), load_f(e, ctx, 3));
        let det = nmad(a01, a10, a00 * a11);
        let invdet = rcp_precise(det);
        store_f(e, ctx, 0, invdet * a11);
        store_f(e, ctx, 1, -invdet * a01);
        store_f(e, ctx, 2, -invdet * a10);
        store_f(e, ctx, 3, invdet * a00);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4828-L4846 (chrome/m156)
    pub(super) fn inverse_mat3(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let (a00, a01, a02) = (load_f(e, ctx, 0), load_f(e, ctx, 1), load_f(e, ctx, 2));
        let (a10, a11, a12) = (load_f(e, ctx, 3), load_f(e, ctx, 4), load_f(e, ctx, 5));
        let (a20, a21, a22) = (load_f(e, ctx, 6), load_f(e, ctx, 7), load_f(e, ctx, 8));
        let b01 = nmad(a12, a21, a22 * a11);
        let b11 = nmad(a22, a10, a12 * a20);
        let b21 = nmad(a11, a20, a21 * a10);
        let det = mad(a00, b01, mad(a01, b11, a02 * b21));
        let invdet = rcp_precise(det);
        store_f(e, ctx, 0, invdet * b01);
        store_f(e, ctx, 1, invdet * nmad(a22, a01, a02 * a21));
        store_f(e, ctx, 2, invdet * nmad(a02, a11, a12 * a01));
        store_f(e, ctx, 3, invdet * b11);
        store_f(e, ctx, 4, invdet * nmad(a02, a20, a22 * a00));
        store_f(e, ctx, 5, invdet * nmad(a12, a00, a02 * a10));
        store_f(e, ctx, 6, invdet * b21);
        store_f(e, ctx, 7, invdet * nmad(a21, a00, a01 * a20));
        store_f(e, ctx, 8, invdet * nmad(a01, a10, a11 * a00));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4848-L4895 (chrome/m156)
    pub(super) fn inverse_mat4(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let (a00, a01, a02, a03) = (
            load_f(e, ctx, 0),
            load_f(e, ctx, 1),
            load_f(e, ctx, 2),
            load_f(e, ctx, 3),
        );
        let (a10, a11, a12, a13) = (
            load_f(e, ctx, 4),
            load_f(e, ctx, 5),
            load_f(e, ctx, 6),
            load_f(e, ctx, 7),
        );
        let (a20, a21, a22, a23) = (
            load_f(e, ctx, 8),
            load_f(e, ctx, 9),
            load_f(e, ctx, 10),
            load_f(e, ctx, 11),
        );
        let (a30, a31, a32, a33) = (
            load_f(e, ctx, 12),
            load_f(e, ctx, 13),
            load_f(e, ctx, 14),
            load_f(e, ctx, 15),
        );
        let mut b00 = nmad(a01, a10, a00 * a11);
        let mut b01 = nmad(a02, a10, a00 * a12);
        let mut b02 = nmad(a03, a10, a00 * a13);
        let mut b03 = nmad(a02, a11, a01 * a12);
        let mut b04 = nmad(a03, a11, a01 * a13);
        let mut b05 = nmad(a03, a12, a02 * a13);
        let mut b06 = nmad(a21, a30, a20 * a31);
        let mut b07 = nmad(a22, a30, a20 * a32);
        let mut b08 = nmad(a23, a30, a20 * a33);
        let mut b09 = nmad(a22, a31, a21 * a32);
        let mut b10 = nmad(a23, a31, a21 * a33);
        let mut b11 = nmad(a23, a32, a22 * a33);
        let det = mad(b00, b11, b05 * b06) + mad(b02, b09, b03 * b08) - mad(b01, b10, b04 * b07);
        let invdet = rcp_precise(det);
        b00 *= invdet;
        b01 *= invdet;
        b02 *= invdet;
        b03 *= invdet;
        b04 *= invdet;
        b05 *= invdet;
        b06 *= invdet;
        b07 *= invdet;
        b08 *= invdet;
        b09 *= invdet;
        b10 *= invdet;
        b11 *= invdet;
        store_f(e, ctx, 0, mad(a13, b09, nmad(a12, b10, a11 * b11)));
        store_f(e, ctx, 1, nmad(a03, b09, nmad(a01, b11, a02 * b10)));
        store_f(e, ctx, 2, mad(a33, b03, nmad(a32, b04, a31 * b05)));
        store_f(e, ctx, 3, nmad(a23, b03, nmad(a21, b05, a22 * b04)));
        store_f(e, ctx, 4, nmad(a13, b07, nmad(a10, b11, a12 * b08)));
        store_f(e, ctx, 5, mad(a03, b07, nmad(a02, b08, a00 * b11)));
        store_f(e, ctx, 6, nmad(a33, b01, nmad(a30, b05, a32 * b02)));
        store_f(e, ctx, 7, mad(a23, b01, nmad(a22, b02, a20 * b05)));
        store_f(e, ctx, 8, mad(a13, b06, nmad(a11, b08, a10 * b10)));
        store_f(e, ctx, 9, nmad(a03, b06, nmad(a00, b10, a01 * b08)));
        store_f(e, ctx, 10, mad(a33, b00, nmad(a31, b02, a30 * b04)));
        store_f(e, ctx, 11, nmad(a23, b00, nmad(a20, b04, a21 * b02)));
        store_f(e, ctx, 12, nmad(a12, b06, nmad(a10, b09, a11 * b07)));
        store_f(e, ctx, 13, mad(a02, b06, nmad(a01, b07, a00 * b09)));
        store_f(e, ctx, 14, nmad(a32, b00, nmad(a30, b03, a31 * b01)));
        store_f(e, ctx, 15, mad(a22, b00, nmad(a21, b01, a20 * b03)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4805 (chrome/m156)
    pub(super) fn sin_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, sin_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4806 (chrome/m156)
    pub(super) fn cos_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, cos_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4807 (chrome/m156)
    pub(super) fn tan_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, tan_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4808 (chrome/m156)
    pub(super) fn asin_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, asin_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4809 (chrome/m156)
    pub(super) fn acos_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, acos_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4810 (chrome/m156)
    pub(super) fn atan_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, atan_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5025-L5027, L5037-L5040, L5092 (chrome/m156)
    pub(super) fn atan2_n_floats(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_adjacent_binary_packed!(ctx, e, atan2_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4811 (chrome/m156)
    pub(super) fn sqrt_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, sqrt_);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5029-L5031, L5037-L5040, L5093 (chrome/m156)
    pub(super) fn pow_n_floats(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_adjacent_binary_packed!(ctx, e, approx_powf);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4812 (chrome/m156)
    pub(super) fn exp_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, approx_exp);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4813 (chrome/m156)
    pub(super) fn exp2_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, approx_pow2);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4814 (chrome/m156)
    pub(super) fn log_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, approx_log);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4815 (chrome/m156)
    pub(super) fn log2_float(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        apply_unary!(ctx, e, approx_log2);
    }
}
