// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Color stages: premul/unpremul, clamps, uniform colors, `dither`, `byte_tables`, color matrices,
//! `swizzle`, `emboss`, HSL/CSS conversions, transfer functions.
//!
//! Owner: task B4 (`docs/design/raster-pipeline.md` §5). The helpers here (`approx_*`, `sin_`,
//! `cos_`, `from_byte`, `to_unorm`, …) are private copies: other stage files that need them
//! (the `SkSL` math stages) carry their own.

// The float literals and the one-letter names are Skia's, digit for digit and letter for letter.
#![allow(
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    clippy::similar_names
)]

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    /// `F_(x)`: a splatted float constant.
    fn f_(x: f32) -> F {
        F::splat(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1608-L1608 (chrome/m156)
    pub(super) fn fract(v: F) -> F {
        v - floor_(v)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1610-L1619 (chrome/m156)
    // See http://www.machinedlearnings.com/2011/06/fast-approximate-logarithm-exponential.html
    fn approx_log2(x: F) -> F {
        // e - 127 is a fair approximation of log2(x) in its own right...
        let bits: U32 = x.bit_cast();
        let e = cast_f(bits) * (1.0f32 / 8_388_608.0);

        // ... but using the mantissa to refine its error is _much_ better.
        let m: F = ((bits & 0x007f_ffff) | 0x3f00_0000).bit_cast();

        nmad(m, f_(1.498030302), e - 124.225514990) - f_(1.725879990) / (f_(0.3520887068) + m)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1621-L1624 (chrome/m156)
    fn approx_log(x: F) -> F {
        let ln2 = core::f32::consts::LN_2; // 0.69314718f
        f_(ln2) * approx_log2(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1626-L1636 (chrome/m156)
    fn approx_pow2(x: F) -> F {
        // 0x7f800000 as a float (exactly representable).
        let k_infinity_bits = 2_139_095_040.0f32;

        let f = fract(x);
        let mut approx = nmad(f, f_(1.490129070), x + 121.274057500);
        approx += f_(27.728023300) / (f_(4.84252568) - f);
        approx *= 8_388_608.0f32; // 1.0f * (1<<23)
        // guard against underflow/overflow
        approx = min_f(max_f(approx, f_(0.0)), f_(k_infinity_bits));

        round(approx).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1638-L1641 (chrome/m156)
    fn approx_exp(x: F) -> F {
        let log2_e = core::f32::consts::LOG2_E; // 1.4426950408889634074f
        approx_pow2(f_(log2_e) * x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1643-L1649 (chrome/m156)
    fn approx_powf(x: F, y: F) -> F {
        if_then_else_f(
            x.eq_mask(0.0) | x.eq_mask(1.0),
            x,
            approx_pow2(approx_log2(x) * y),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2638-L2638 (chrome/m156)
    fn clamp_01_(v: F) -> F {
        min_f(max_f(f_(0.0), v), f_(1.0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2097-L2108 (chrome/m156)
    // Polynomial approximation of degree 5 for sin(x * 2 * pi) in the range [-1/4, 1/4]
    // Adapted from https://github.com/google/swiftshader/blob/master/docs/Sin-Cos-Optimization.pdf
    fn sin5q_(x: F) -> F {
        // A * x + B * x^3 + C * x^5
        // Exact at x = 0, 1/12, 1/6, 1/4, and their negatives,
        // which correspond to x * 2 * pi = 0, pi/6, pi/3, pi/2
        let a = 6.28230858f32;
        let b = -41.1693687f32;
        let c = 74.4388885f32;
        let x2 = x * x;
        x * mad(mad(x2, f_(c), f_(b)), x2, f_(a))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2110-L2115 (chrome/m156)
    fn sin_(x: F) -> F {
        let one_over_pi2 = 1.0f32 / (2.0f32 * core::f32::consts::PI);
        let x = mad(x, f_(-one_over_pi2), f_(0.25));
        let x = f_(0.25) - abs_f(x - floor_(x + 0.5));
        sin5q_(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2117-L2122 (chrome/m156)
    fn cos_(x: F) -> F {
        let one_over_pi2 = 1.0f32 / (2.0f32 * core::f32::consts::PI);
        let x = x * one_over_pi2;
        let x = f_(0.25) - abs_f(x - floor_(x + 0.5));
        sin5q_(x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2007-L2009 (chrome/m156)
    fn from_byte(b: U8) -> F {
        let wide: U32 = b.cast();
        cast_f(wide) * (1.0f32 / 255.0f32)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2269-L2275 (chrome/m156)
    // Any time we use round() we probably want to use to_unorm().
    fn to_unorm(v: F, scale: f32) -> U32 {
        round(min_f(max_f(f_(0.0), mad(v, f_(scale), f_(0.0))), f_(scale)))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2990-L2998 (chrome/m156)
    fn strip_sign(x: F) -> (F, U32) {
        let bits: U32 = x.bit_cast();
        let sign = bits & 0x8000_0000;
        ((bits ^ sign).bit_cast(), sign)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2996-L2998 (chrome/m156)
    fn apply_sign(x: F, sign: U32) -> F {
        let bits: U32 = x.bit_cast();
        (sign | bits).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2316-L2348 (chrome/m156)
    pub(super) fn dither(rate: f32, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Get [(dx,dy), (dx+1,dy), (dx+2,dy), ...] loaded up in integer vectors.
        #[allow(clippy::cast_possible_truncation)] // mirrors U32_(dx): size_t → uint32_t
        let (dx, dy) = (e.dx as u32, e.dy as u32);
        let x = U32::splat(dx) + U32::load(&IOTA_U32[..N]);
        let mut y = U32::splat(dy);

        // We're doing 8x8 ordered dithering, see https://en.wikipedia.org/wiki/Ordered_dithering.
        // In this case n=8 and we're using the matrix that looks like 1/64 x [ 0 48 12 60 ... ].

        // We only need X and X^Y from here on, so it's easier to just think of that as "Y".
        y ^= x;

        // We'll mix the bottom 3 bits of each of X and Y to make 6 bits,
        // for 2^6 == 64 == 8x8 matrix values.  If X=abc and Y=def, we make fcebda.
        let m = (y & 1) << 5
            | (x & 1) << 4
            | (y & 2) << 2
            | (x & 2) << 1
            | (y & 4) >> 1
            | (x & 4) >> 2;

        // Scale that dither to [0,1), then (-0.5,+0.5), here using 63/128 = 0.4921875 as 0.5-epsilon.
        // We want to make sure our dither is less than 0.5 in either direction to keep exact values
        // like 0 and 1 unchanged after rounding.
        let dither = mad(cast_f(m), f_(2.0 / 128.0), f_(-63.0 / 128.0));

        p.r = mad(dither, f_(rate), p.r);
        p.g = mad(dither, f_(rate), p.g);
        p.b = mad(dither, f_(rate), p.b);

        p.r = max_f(f_(0.0), min_f(p.r, p.a));
        p.g = max_f(f_(0.0), min_f(p.g, p.a));
        p.b = max_f(f_(0.0), min_f(p.b, p.a));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2351-L2356 (chrome/m156)
    /// load 4 floats from memory, and splat them into r,g,b,a
    pub(super) fn uniform_color(c: &UniformColorCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = f_(c.r);
        p.g = f_(c.g);
        p.b = f_(c.b);
        p.a = f_(c.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2357-L2362 (chrome/m156)
    pub(super) fn unbounded_uniform_color(
        c: &UniformColorCtx,
        p: &mut Regs,
        _e: &mut Params<'_, '_>,
    ) {
        p.r = f_(c.r);
        p.g = f_(c.g);
        p.b = f_(c.b);
        p.a = f_(c.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2363-L2369 (chrome/m156)
    /// load 4 floats from memory, and splat them into dr,dg,db,da
    pub(super) fn uniform_color_dst(c: &UniformColorCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = f_(c.r);
        p.dg = f_(c.g);
        p.db = f_(c.b);
        p.da = f_(c.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2371-L2375 (chrome/m156)
    /// splats opaque-black into r,g,b,a
    pub(super) fn black_color(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = f_(0.0);
        p.g = f_(0.0);
        p.b = f_(0.0);
        p.a = f_(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2377-L2379 (chrome/m156)
    pub(super) fn white_color(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = f_(1.0);
        p.g = f_(1.0);
        p.b = f_(1.0);
        p.a = f_(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2640-L2645 (chrome/m156)
    pub(super) fn clamp_01(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = clamp_01_(p.r);
        p.g = clamp_01_(p.g);
        p.b = clamp_01_(p.b);
        p.a = clamp_01_(p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2647-L2649 (chrome/m156)
    pub(super) fn clamp_a_01(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = clamp_01_(p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2651-L2656 (chrome/m156)
    pub(super) fn clamp_gamut(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = min_f(max_f(p.a, f_(0.0)), f_(1.0));
        p.r = min_f(max_f(p.r, f_(0.0)), p.a);
        p.g = min_f(max_f(p.g, f_(0.0)), p.a);
        p.b = min_f(max_f(p.b, f_(0.0)), p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2658-L2662 (chrome/m156)
    pub(super) fn set_rgb(rgb: &[f32; 3], p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = f_(rgb[0]);
        p.g = f_(rgb[1]);
        p.b = f_(rgb[2]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2664-L2668 (chrome/m156)
    pub(super) fn unbounded_set_rgb(rgb: &[f32; 3], p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = f_(rgb[0]);
        p.g = f_(rgb[1]);
        p.b = f_(rgb[2]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2700-L2704 (chrome/m156)
    pub(super) fn premul(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r *= p.a;
        p.g *= p.a;
        p.b *= p.a;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2705-L2709 (chrome/m156)
    pub(super) fn premul_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr *= p.da;
        p.dg *= p.da;
        p.db *= p.da;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2710-L2716 (chrome/m156)
    pub(super) fn unpremul(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let inf = f32::from_bits(0x7f80_0000);
        let rcp = f_(1.0) / p.a;
        let scale = if_then_else_f(rcp.lt_mask(inf), rcp, f_(0.0));
        p.r *= scale;
        p.g *= scale;
        p.b *= scale;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2717-L2722 (chrome/m156)
    pub(super) fn unpremul_polar(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let inf = f32::from_bits(0x7f80_0000);
        let rcp = f_(1.0) / p.a;
        let scale = if_then_else_f(rcp.lt_mask(inf), rcp, f_(0.0));
        p.g *= scale;
        p.b *= scale;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2724-L2725 (chrome/m156)
    pub(super) fn force_opaque(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = f_(1.0);
    }

    pub(super) fn force_opaque_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.da = f_(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2727-L2746 (chrome/m156)
    pub(super) fn rgb_to_hsl(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b) = (p.r, p.g, p.b);
        let mx = max_f(r, max_f(g, b));
        let mn = min_f(r, min_f(g, b));
        let d = mx - mn;
        let d_rcp = f_(1.0) / d;

        let h = f_(1.0 / 6.0)
            * if_then_else_f(
                mx.eq_mask(mn),
                f_(0.0),
                if_then_else_f(
                    mx.eq_mask(r),
                    (g - b) * d_rcp + if_then_else_f(g.lt_mask(b), f_(6.0), f_(0.0)),
                    if_then_else_f(
                        mx.eq_mask(g),
                        (b - r) * d_rcp + f_(2.0),
                        (r - g) * d_rcp + f_(4.0),
                    ),
                ),
            );

        let l = (mx + mn) * 0.5;
        let s = if_then_else_f(
            mx.eq_mask(mn),
            f_(0.0),
            d / if_then_else_f(l.gt_mask(0.5), f_(2.0) - mx - mn, mx + mn),
        );

        p.r = h;
        p.g = s;
        p.b = l;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2747-L2763 (chrome/m156)
    pub(super) fn hsl_to_rgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // See GrRGBToHSLFilterEffect.fp

        let (h, s, l) = (p.r, p.g, p.b);
        let c = (f_(1.0) - abs_f(f_(2.0) * l - 1.0)) * s;

        let hue_to_rgb = |hue: F| {
            let q = clamp_01_(abs_f(fract(hue) * 6.0 - 3.0) - 1.0);
            (q - 0.5) * c + l
        };

        p.r = hue_to_rgb(h + 0.0f32 / 3.0);
        p.g = hue_to_rgb(h + 2.0f32 / 3.0);
        p.b = hue_to_rgb(h + 1.0f32 / 3.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2767-L2788 (chrome/m156)
    // Color conversion functions used in gradient interpolation, based on
    // https://www.w3.org/TR/css-color-4/#color-conversion-code
    pub(super) fn css_lab_to_xyz(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let k = 24389.0f32 / 27.0;
        let e = 216.0f32 / 24389.0;

        let f1 = (p.r + 16.0) * (1.0f32 / 116.0);
        let f0 = (p.g * (1.0f32 / 500.0)) + f1;
        let f2 = f1 - (p.b * (1.0f32 / 200.0));

        let f_cubed = [f0 * f0 * f0, f1 * f1 * f1, f2 * f2 * f2];

        let xyz = [
            if_then_else_f(
                f_cubed[0].gt_mask(e),
                f_cubed[0],
                (f0 * 116.0 - 16.0) * (1.0f32 / k),
            ),
            if_then_else_f(p.r.gt_mask(k * e), f_cubed[1], p.r * (1.0f32 / k)),
            if_then_else_f(
                f_cubed[2].gt_mask(e),
                f_cubed[2],
                (f2 * 116.0 - 16.0) * (1.0f32 / k),
            ),
        ];

        let d50 = [0.3457f32 / 0.3585, 1.0, (1.0f32 - 0.3457 - 0.3585) / 0.3585];
        p.r = xyz[0] * d50[0];
        p.g = xyz[1] * d50[1];
        p.b = xyz[2] * d50[2];
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2790-L2802 (chrome/m156)
    pub(super) fn css_oklab_to_linear_srgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b) = (p.r, p.g, p.b);
        let l_ = r + f_(0.3963377774) * g + f_(0.2158037573) * b;
        let m_ = r - f_(0.1055613458) * g - f_(0.0638541728) * b;
        let s_ = r - f_(0.0894841775) * g - f_(1.2914855480) * b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        p.r = f_(4.0767416621) * l - f_(3.3077115913) * m + f_(0.2309699292) * s;
        p.g = f_(-1.2684380046) * l + f_(2.6097574011) * m - f_(0.3413193965) * s;
        p.b = f_(-0.0041960863) * l - f_(0.7034186147) * m + f_(1.7076147010) * s;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2804-L2818 (chrome/m156)
    pub(super) fn css_oklab_gamut_map_to_linear_srgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // TODO(https://crbug.com/1508329): Add support for gamut mapping.
        // Return a greyscale value, so that accidental use is obvious.
        let (l_, m_, s_) = (p.r, p.r, p.r);

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        p.r = f_(4.0767416621) * l - f_(3.3077115913) * m + f_(0.2309699292) * s;
        p.g = f_(-1.2684380046) * l + f_(2.6097574011) * m - f_(0.3413193965) * s;
        p.b = f_(-0.0041960863) * l - f_(0.7034186147) * m + f_(1.7076147010) * s;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2820-L2833 (chrome/m156)
    // Skia stores all polar colors with hue in the first component, so this "LCH -> Lab" transform
    // actually takes "HCL". This is also used to do the same polar transform for OkHCL to OkLAB.
    // See similar comments & logic in SkGradientBaseShader.cpp.
    pub(super) fn css_hcl_to_lab(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (h, c, l) = (p.r, p.g, p.b);

        // SK_FloatPI = 3.14159265f, which is f32::consts::PI.
        let hue_radians = h * (core::f32::consts::PI / 180.0);

        p.r = l;
        p.g = c * cos_(hue_radians);
        p.b = c * sin_(hue_radians);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2835-L2837 (chrome/m156)
    fn mod_(x: F, y: f32) -> F {
        nmad(f_(y), floor_(x * (1.0f32 / y)), x)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2839-L2858 (chrome/m156)
    fn css_hsl_to_srgb_(h: F, s: F, l: F) -> (F, F, F) {
        let h = mod_(h, 360.0);

        let s = s * 0.01;
        let l = l * 0.01;

        let k = [
            mod_(f_(0.0) + h * (1.0f32 / 30.0), 12.0),
            mod_(f_(8.0) + h * (1.0f32 / 30.0), 12.0),
            mod_(f_(4.0) + h * (1.0f32 / 30.0), 12.0),
        ];
        let a = s * min_f(l, f_(1.0) - l);
        let ch = |k: F| l - a * max_f(f_(-1.0), min_f(min_f(k - 3.0, f_(9.0) - k), f_(1.0)));
        (ch(k[0]), ch(k[1]), ch(k[2]))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2860-L2865 (chrome/m156)
    pub(super) fn css_hsl_to_srgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b) = css_hsl_to_srgb_(p.r, p.g, p.b);
        p.r = r;
        p.g = g;
        p.b = b;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2867-L2883 (chrome/m156)
    pub(super) fn css_hwb_to_srgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.g *= 0.01;
        p.b *= 0.01;

        let gray = p.g / (p.g + p.b);

        let (mut r, mut g, mut b) = css_hsl_to_srgb_(p.r, f_(100.0), f_(50.0));
        r = r * (f_(1.0) - p.g - p.b) + p.g;
        g = g * (f_(1.0) - p.g - p.b) + p.g;
        b = b * (f_(1.0) - p.g - p.b) + p.g;

        let is_gray = (p.g + p.b).ge_mask(1.0);

        p.r = if_then_else_f(is_gray, gray, r);
        p.g = if_then_else_f(is_gray, gray, g);
        p.b = if_then_else_f(is_gray, gray, b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2971-L2981 (chrome/m156)
    pub(super) fn emboss(ctx: EmbossCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mul = from_byte(U8::load_bytes(e.ptr_at_xy(ctx.mul, 1)));
        let add = from_byte(U8::load_bytes(e.ptr_at_xy(ctx.add, 1)));

        p.r = mad(p.r, mul, add);
        p.g = mad(p.g, mul, add);
        p.b = mad(p.b, mul, add);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2983-L2988 (chrome/m156)
    pub(super) fn byte_tables(tables: &TablesCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        // `gather(table, ix)`: `to_unorm(_, 255)` keeps every index in [0,255].
        p.r = from_byte(to_unorm(p.r, 255.0).map(|i| tables.r[i as usize]));
        p.g = from_byte(to_unorm(p.g, 255.0).map(|i| tables.g[i as usize]));
        p.b = from_byte(to_unorm(p.b, 255.0).map(|i| tables.b[i as usize]));
        p.a = from_byte(to_unorm(p.a, 255.0).map(|i| tables.a[i as usize]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3000-L3012 (chrome/m156)
    pub(super) fn parametric(ctx: &TransferFunction, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let f = |v: F| {
            let (v, sign) = strip_sign(v);

            let r = if_then_else_f(
                v.le_mask(ctx.d),
                mad(f_(ctx.c), v, f_(ctx.f)),
                approx_powf(mad(f_(ctx.a), v, f_(ctx.b)), f_(ctx.g)) + ctx.e,
            );
            apply_sign(r, sign)
        };
        p.r = f(p.r);
        p.g = f(p.g);
        p.b = f(p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3014-L3023 (chrome/m156)
    pub(super) fn gamma_(g: f32, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let f = |v: F| {
            let (v, sign) = strip_sign(v);
            apply_sign(approx_powf(v, f_(g)), sign)
        };
        p.r = f(p.r);
        p.g = f(p.g);
        p.b = f(p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3025-L3039 (chrome/m156)
    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn PQish(ctx: &TransferFunction, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let f = |v: F| {
            let (v, sign) = strip_sign(v);

            let r = approx_powf(
                max_f(mad(f_(ctx.b), approx_powf(v, f_(ctx.c)), f_(ctx.a)), f_(0.0))
                    / (mad(f_(ctx.e), approx_powf(v, f_(ctx.c)), f_(ctx.d))),
                f_(ctx.f),
            );

            apply_sign(r, sign)
        };
        p.r = f(p.r);
        p.g = f(p.g);
        p.b = f(p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3041-L3058 (chrome/m156)
    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn HLGish(ctx: &TransferFunction, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let f = |v: F| {
            let (v, sign) = strip_sign(v);

            let (rr, gg, a, b, c, k) = (ctx.a, ctx.b, ctx.c, ctx.d, ctx.e, ctx.f + 1.0f32);

            let r = if_then_else_f(
                (v * rr).le_mask(1.0),
                approx_powf(v * rr, f_(gg)),
                approx_exp((v - c) * a) + b,
            );

            f_(k) * apply_sign(r, sign)
        };
        p.r = f(p.r);
        p.g = f(p.g);
        p.b = f(p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3060-L3078 (chrome/m156)
    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn HLGinvish(ctx: &TransferFunction, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let f = |v: F| {
            let (v, sign) = strip_sign(v);

            let (rr, gg, a, b, c, k) = (ctx.a, ctx.b, ctx.c, ctx.d, ctx.e, ctx.f + 1.0f32);

            let v = v / k;
            let r = if_then_else_f(
                v.le_mask(1.0),
                f_(rr) * approx_powf(v, f_(gg)),
                f_(a) * approx_log(v - b) + c,
            );

            apply_sign(r, sign)
        };
        p.r = f(p.r);
        p.g = f(p.g);
        p.b = f(p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3080-L3089 (chrome/m156)
    pub(super) fn ootf(ctx: &[f32; 4], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let y = f_(ctx[0]) * p.r + f_(ctx[1]) * p.g + f_(ctx[2]) * p.b;

        let (y, sign) = strip_sign(y);
        let y_to_gamma_minus_one = apply_sign(approx_powf(y, f_(ctx[3])), sign);
        p.r *= y_to_gamma_minus_one;
        p.g *= y_to_gamma_minus_one;
        p.b *= y_to_gamma_minus_one;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3664-L3667 (chrome/m156)
    pub(super) fn bt709_luminance_or_luma_to_alpha(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = p.r * 0.2126 + p.g * 0.7152 + p.b * 0.0722;
        p.r = f_(0.0);
        p.g = f_(0.0);
        p.b = f_(0.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3668-L3670 (chrome/m156)
    pub(super) fn bt709_luminance_or_luma_to_rgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let y = p.r * 0.2126 + p.g * 0.7152 + p.b * 0.0722;
        p.r = y;
        p.g = y;
        p.b = y;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3686-L3693 (chrome/m156)
    pub(super) fn matrix_3x3(m: &[f32; 9], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b) = (p.r, p.g, p.b);
        let rr = mad(r, f_(m[0]), mad(g, f_(m[3]), b * m[6]));
        let gg = mad(r, f_(m[1]), mad(g, f_(m[4]), b * m[7]));
        let bb = mad(r, f_(m[2]), mad(g, f_(m[5]), b * m[8]));
        p.r = rr;
        p.g = gg;
        p.b = bb;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3694-L3701 (chrome/m156)
    pub(super) fn matrix_3x4(m: &[f32; 12], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b) = (p.r, p.g, p.b);
        let rr = mad(r, f_(m[0]), mad(g, f_(m[3]), mad(b, f_(m[6]), f_(m[9]))));
        let gg = mad(r, f_(m[1]), mad(g, f_(m[4]), mad(b, f_(m[7]), f_(m[10]))));
        let bb = mad(r, f_(m[2]), mad(g, f_(m[5]), mad(b, f_(m[8]), f_(m[11]))));
        p.r = rr;
        p.g = gg;
        p.b = bb;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3702-L3711 (chrome/m156)
    pub(super) fn matrix_4x5(m: &[f32; 20], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (r, g, b, a) = (p.r, p.g, p.b, p.a);
        let row = |i: usize| {
            mad(
                r,
                f_(m[i]),
                mad(
                    g,
                    f_(m[i + 1]),
                    mad(b, f_(m[i + 2]), mad(a, f_(m[i + 3]), f_(m[i + 4]))),
                ),
            )
        };
        let (rr, gg, bb, aa) = (row(0), row(5), row(10), row(15));
        p.r = rr;
        p.g = gg;
        p.b = bb;
        p.a = aa;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3712-L3720 (chrome/m156)
    pub(super) fn matrix_4x3(m: &[f32; 12], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);

        p.r = mad(x, f_(m[0]), mad(y, f_(m[4]), f_(m[8])));
        p.g = mad(x, f_(m[1]), mad(y, f_(m[5]), f_(m[9])));
        p.b = mad(x, f_(m[2]), mad(y, f_(m[6]), f_(m[10])));
        p.a = mad(x, f_(m[3]), mad(y, f_(m[7]), f_(m[11])));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5326-L5340 (chrome/m156)
    pub(super) fn gauss_a_to_rgba(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // x = 1 - x;
        // exp(-x * x * 4) - 0.018f;
        // ... now approximate with quartic
        //
        let c4 = -2.26661229133605957031f32;
        let c3 = 2.89795351028442382812f32;
        let c2 = 0.21345567703247070312f32;
        let c1 = 0.15489584207534790039f32;
        let c0 = 0.00030726194381713867f32;
        let a = p.a;
        let a = mad(a, mad(a, mad(a, mad(a, f_(c4), f_(c3)), f_(c2)), f_(c1)), f_(c0));
        p.a = a;
        p.r = a;
        p.g = a;
        p.b = a;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5445-L5462 (chrome/m156)
    // ~~~~~~ skgpu::Swizzle stage ~~~~~~ //
    pub(super) fn swizzle(swiz: [u8; 4], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (ir, ig, ib, ia) = (p.r, p.g, p.b, p.a);
        let o = [&mut p.r, &mut p.g, &mut p.b, &mut p.a];

        for (o, c) in o.into_iter().zip(swiz) {
            match c {
                b'r' => *o = ir,
                b'g' => *o = ig,
                b'b' => *o = ib,
                b'a' => *o = ia,
                b'0' => *o = f_(0.0),
                b'1' => *o = f_(1.0),
                _ => {}
            }
        }
    }
}
