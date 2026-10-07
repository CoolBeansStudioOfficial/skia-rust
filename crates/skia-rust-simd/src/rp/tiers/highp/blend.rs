// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Blend modes and coverage (`scale_*`, `lerp_*`), highp.
//!
//! Owner: task B3 (`docs/design/raster-pipeline.md` §5). `srcover` was ported by A3.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `F` register (`N` floats).
const F_BYTES: usize = 4 * N;

// Port of: src/opts/SkRasterPipeline_opts.h#L2428-L2439 (chrome/m156)
/// The highp `BLEND_MODE(name)` macro for modes that apply the same logic to every channel:
/// `$channel` applied to `r, g, b`, then to alpha.
macro_rules! blend_mode_all {
    ($name:ident, $channel:ident) => {
        si! {
            pub(super) fn $name(p: &mut Regs, _e: &mut Params<'_, '_>) {
                p.r = $channel(p.r, p.dr, p.a, p.da);
                p.g = $channel(p.g, p.dg, p.a, p.da);
                p.b = $channel(p.b, p.db, p.a, p.da);
                p.a = $channel(p.a, p.da, p.a, p.da);
            }
        }
    };
}

// Port of: src/opts/SkRasterPipeline_opts.h#L2459-L2467 (chrome/m156)
/// The second `BLEND_MODE(name)` macro: `$channel` on the colors, srcover on alpha.
macro_rules! blend_mode_color {
    ($name:ident, $channel:ident) => {
        si! {
            pub(super) fn $name(p: &mut Regs, _e: &mut Params<'_, '_>) {
                p.r = $channel(p.r, p.dr, p.a, p.da);
                p.g = $channel(p.g, p.dg, p.a, p.da);
                p.b = $channel(p.b, p.db, p.a, p.da);
                p.a = mad(p.da, inv(p.a), p.a);
            }
        }
    };
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2438-L2439 (chrome/m156)
    /// `inv(x)`: `1.0f - x`.
    fn inv(x: F) -> F {
        F::splat(1.0) - x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2439-L2439 (chrome/m156)
    /// `two(x)`: `x + x`.
    fn two(x: F) -> F {
        x + x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2441-L2455 (chrome/m156)
    fn clear_channel(_s: F, _d: F, _sa: F, _da: F) -> F {
        F::splat(0.0)
    }
    fn srcatop_channel(s: F, d: F, sa: F, da: F) -> F {
        mad(s, da, d * inv(sa))
    }
    fn dstatop_channel(s: F, d: F, sa: F, da: F) -> F {
        mad(d, sa, s * inv(da))
    }
    fn srcin_channel(s: F, _d: F, _sa: F, da: F) -> F {
        s * da
    }
    fn dstin_channel(_s: F, d: F, sa: F, _da: F) -> F {
        d * sa
    }
    fn srcout_channel(s: F, _d: F, _sa: F, da: F) -> F {
        s * inv(da)
    }
    fn dstout_channel(_s: F, d: F, sa: F, _da: F) -> F {
        d * inv(sa)
    }
    // (srcover_channel is below, next to A3's `srcover`.)
    fn dstover_channel(s: F, d: F, _sa: F, da: F) -> F {
        mad(s, inv(da), d)
    }
    fn modulate_channel(s: F, d: F, _sa: F, _da: F) -> F {
        s * d
    }
    fn multiply_channel(s: F, d: F, sa: F, da: F) -> F {
        mad(s, d, mad(s, inv(da), d * inv(sa)))
    }
    fn plus_channel(s: F, d: F, _sa: F, _da: F) -> F {
        min_f(s + d, F::splat(1.0)) // We can clamp to either 1 or sa.
    }
    fn screen_channel(s: F, d: F, _sa: F, _da: F) -> F {
        nmad(s, d, s + d)
    }
    fn xor_channel(s: F, d: F, sa: F, da: F) -> F {
        mad(s, inv(da), d * inv(sa))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2469-L2472 (chrome/m156)
    fn darken_channel(s: F, d: F, sa: F, da: F) -> F {
        s + d - max_f(s * da, d * sa)
    }
    fn lighten_channel(s: F, d: F, sa: F, da: F) -> F {
        s + d - min_f(s * da, d * sa)
    }
    fn difference_channel(s: F, d: F, sa: F, da: F) -> F {
        s + d - two(min_f(s * da, d * sa))
    }
    fn exclusion_channel(s: F, d: F, _sa: F, _da: F) -> F {
        s + d - two(s * d)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2474-L2478 (chrome/m156)
    fn colorburn_channel(s: F, d: F, sa: F, da: F) -> F {
        if_then_else_f(
            d.eq_mask(da),
            d + s * inv(da),
            if_then_else_f(
                s.eq_mask(0.0),
                /* s + */ d * inv(sa),
                sa * (da - min_f(da, (da - d) * sa * rcp_fast(s))) + s * inv(da) + d * inv(sa),
            ),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2479-L2483 (chrome/m156)
    fn colordodge_channel(s: F, d: F, sa: F, da: F) -> F {
        if_then_else_f(
            d.eq_mask(0.0),
            /* d + */ s * inv(da),
            if_then_else_f(
                s.eq_mask(sa),
                s + d * inv(sa),
                sa * min_f(da, (d * sa) * rcp_fast(sa - s)) + s * inv(da) + d * inv(sa),
            ),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2484-L2487 (chrome/m156)
    fn hardlight_channel(s: F, d: F, sa: F, da: F) -> F {
        s * inv(da)
            + d * inv(sa)
            + if_then_else_f(
                two(s).le_mask(sa),
                two(s * d),
                sa * da - two((da - d) * (sa - s)),
            )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2488-L2492 (chrome/m156)
    fn overlay_channel(s: F, d: F, sa: F, da: F) -> F {
        s * inv(da)
            + d * inv(sa)
            + if_then_else_f(
                two(d).le_mask(da),
                two(s * d),
                sa * da - two((da - d) * (sa - s)),
            )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2493-L2507 (chrome/m156)
    fn softlight_channel(s: F, d: F, sa: F, da: F) -> F {
        let m = if_then_else_f(da.gt_mask(0.0), d / da, F::splat(0.0));
        let s2 = two(s);
        let m4 = two(two(m));

        // The logic forks three ways:
        //    1. dark src?
        //    2. light src, dark dst?
        //    3. light src, light dst?
        let dark_src = d * (sa + (s2 - sa) * (F::splat(1.0) - m)); // Used in case 1.
        let dark_dst = (m4 * m4 + m4) * (m - 1.0) + F::splat(7.0) * m; // Used in case 2.
        let lite_dst = sqrt_(m) - m;
        let lite_src = d * sa
            + da * (s2 - sa) * if_then_else_f(two(two(d)).le_mask(da), dark_dst, lite_dst); // 2 or 3?
        s * inv(da) + d * inv(sa) + if_then_else_f(s2.le_mask(sa), dark_src, lite_src) // 1 or (2 or 3)?
    }
}

blend_mode_all!(clear, clear_channel);
blend_mode_all!(srcatop, srcatop_channel);
blend_mode_all!(dstatop, dstatop_channel);
blend_mode_all!(srcin, srcin_channel);
blend_mode_all!(dstin, dstin_channel);
blend_mode_all!(srcout, srcout_channel);
blend_mode_all!(dstout, dstout_channel);
blend_mode_all!(dstover, dstover_channel);
blend_mode_all!(modulate, modulate_channel);
blend_mode_all!(multiply, multiply_channel);
blend_mode_all!(plus_, plus_channel);
blend_mode_all!(screen, screen_channel);
blend_mode_all!(xor_, xor_channel);

blend_mode_color!(darken, darken_channel);
blend_mode_color!(lighten, lighten_channel);
blend_mode_color!(difference, difference_channel);
blend_mode_color!(exclusion, exclusion_channel);
blend_mode_color!(colorburn, colorburn_channel);
blend_mode_color!(colordodge, colordodge_channel);
blend_mode_color!(hardlight, hardlight_channel);
blend_mode_color!(overlay, overlay_channel);
blend_mode_color!(softlight, softlight_channel);

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2448-L2448 (chrome/m156)
    fn srcover_channel(s: F, d: F, sa: F, _da: F) -> F {
        mad(d, inv(sa), s)
    }

    pub(super) fn srcover(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = srcover_channel(p.r, p.dr, p.a, p.da);
        p.g = srcover_channel(p.g, p.dg, p.a, p.da);
        p.b = srcover_channel(p.b, p.db, p.a, p.da);
        p.a = srcover_channel(p.a, p.da, p.a, p.da);
    }
}

// Non-separable blend modes, based on https://www.w3.org/TR/compositing-1/#blendingnonseparable
// and the OpenGL ES 3.2 spec (equivalent, but ES' math has been better simplified), with extra
// work to make the math work with premul inputs.
si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2518-L2518 (chrome/m156)
    fn sat(r: F, g: F, b: F) -> F {
        max_f(r, max_f(g, b)) - min_f(r, min_f(g, b))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2519-L2519 (chrome/m156)
    fn lum(r: F, g: F, b: F) -> F {
        mad(r, F::splat(0.30), mad(g, F::splat(0.59), b * 0.11))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2521-L2531 (chrome/m156)
    fn set_sat(r: &mut F, g: &mut F, b: &mut F, s: F) {
        let mn = min_f(*r, min_f(*g, *b));
        let mx = max_f(*r, max_f(*g, *b));
        let sat = mx - mn;

        // Map min channel to 0, max channel to s, and scale the middle proportionally.
        let s = if_then_else_f(sat.eq_mask(0.0), F::splat(0.0), s * rcp_fast(sat));
        *r = (*r - mn) * s;
        *g = (*g - mn) * s;
        *b = (*b - mn) * s;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2532-L2537 (chrome/m156)
    fn set_lum(r: &mut F, g: &mut F, b: &mut F, l: F) {
        let diff = l - lum(*r, *g, *b);
        *r += diff;
        *g += diff;
        *b += diff;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2538-L2543 (chrome/m156)
    #[allow(clippy::similar_names)] // Skia's names (`mn_scale`, `mx_scale`)
    fn clip_channel(c: F, l: F, clip_low: I32, clip_high: I32, mn_scale: F, mx_scale: F) -> F {
        let c = if_then_else_f(clip_low, mad(mn_scale, c - l, l), c);
        let c = if_then_else_f(clip_high, mad(mx_scale, c - l, l), c);
        max_f(c, F::splat(0.0)) // Sometimes without this we may dip just a little negative.
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2544-L2557 (chrome/m156)
    // Skia's names (`r, g, b, a`, `mn_scale`, `mx_scale`)
    #[allow(clippy::similar_names, clippy::many_single_char_names)]
    fn clip_color(r: &mut F, g: &mut F, b: &mut F, a: F) {
        let mn = min_f(*r, min_f(*g, *b));
        let mx = max_f(*r, max_f(*g, *b));
        let l = lum(*r, *g, *b);
        let mn_scale = (l) * rcp_fast(l - mn);
        let mx_scale = (a - l) * rcp_fast(mx - l);
        let clip_low = cond_to_mask(mn.lt_mask(0.0) & l.ne_mask(mn));
        let clip_high = cond_to_mask(mx.gt_mask(a) & l.ne_mask(mx));

        *r = clip_channel(*r, l, clip_low, clip_high, mn_scale, mx_scale);
        *g = clip_channel(*g, l, clip_low, clip_high, mn_scale, mx_scale);
        *b = clip_channel(*b, l, clip_low, clip_high, mn_scale, mx_scale);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2558-L2571 (chrome/m156)
    pub(super) fn hue(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut rr = p.r * p.a;
        let mut gg = p.g * p.a;
        let mut bb = p.b * p.a;

        set_sat(&mut rr, &mut gg, &mut bb, sat(p.dr, p.dg, p.db) * p.a);
        set_lum(&mut rr, &mut gg, &mut bb, lum(p.dr, p.dg, p.db) * p.a);
        clip_color(&mut rr, &mut gg, &mut bb, p.a * p.da);

        p.r = mad(p.r, inv(p.da), mad(p.dr, inv(p.a), rr));
        p.g = mad(p.g, inv(p.da), mad(p.dg, inv(p.a), gg));
        p.b = mad(p.b, inv(p.da), mad(p.db, inv(p.a), bb));
        p.a = p.a + nmad(p.a, p.da, p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2572-L2585 (chrome/m156)
    pub(super) fn saturation(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut rr = p.dr * p.a;
        let mut gg = p.dg * p.a;
        let mut bb = p.db * p.a;

        set_sat(&mut rr, &mut gg, &mut bb, sat(p.r, p.g, p.b) * p.da);
        set_lum(&mut rr, &mut gg, &mut bb, lum(p.dr, p.dg, p.db) * p.a); // (This is not redundant.)
        clip_color(&mut rr, &mut gg, &mut bb, p.a * p.da);

        p.r = mad(p.r, inv(p.da), mad(p.dr, inv(p.a), rr));
        p.g = mad(p.g, inv(p.da), mad(p.dg, inv(p.a), gg));
        p.b = mad(p.b, inv(p.da), mad(p.db, inv(p.a), bb));
        p.a = p.a + nmad(p.a, p.da, p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2586-L2598 (chrome/m156)
    pub(super) fn color(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut rr = p.r * p.da;
        let mut gg = p.g * p.da;
        let mut bb = p.b * p.da;

        set_lum(&mut rr, &mut gg, &mut bb, lum(p.dr, p.dg, p.db) * p.a);
        clip_color(&mut rr, &mut gg, &mut bb, p.a * p.da);

        p.r = mad(p.r, inv(p.da), mad(p.dr, inv(p.a), rr));
        p.g = mad(p.g, inv(p.da), mad(p.dg, inv(p.a), gg));
        p.b = mad(p.b, inv(p.da), mad(p.db, inv(p.a), bb));
        p.a = p.a + nmad(p.a, p.da, p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2599-L2611 (chrome/m156)
    pub(super) fn luminosity(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut rr = p.dr * p.a;
        let mut gg = p.dg * p.a;
        let mut bb = p.db * p.a;

        set_lum(&mut rr, &mut gg, &mut bb, lum(p.r, p.g, p.b) * p.da);
        clip_color(&mut rr, &mut gg, &mut bb, p.a * p.da);

        p.r = mad(p.r, inv(p.da), mad(p.dr, inv(p.a), rr));
        p.g = mad(p.g, inv(p.da), mad(p.dg, inv(p.a), gg));
        p.b = mad(p.b, inv(p.da), mad(p.db, inv(p.a), bb));
        p.a = p.a + nmad(p.a, p.da, p.da);
    }
}

// ~~~ Coverage scales / lerps ~~~
si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2007-L2009 (chrome/m156)
    /// `from_byte(U8)`: `cast(expand(b)) * (1/255.0f)`.
    fn from_byte(b: U8) -> F {
        let wide: U32 = b.cast();
        cast_f(wide) * (1.0 / 255.0)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2013-L2018 (chrome/m156)
    /// `from_565`: the 565 pixels as `r, g, b` in `[0, 1]`.
    fn from_565(v: U16) -> (F, F, F) {
        let wide: U32 = v.cast();
        let r = cast_f(wide & (31 << 11)) * (1.0 / f32::from(31u16 << 11));
        let g = cast_f(wide & (63 << 5)) * (1.0 / f32::from(63u16 << 5));
        let b = cast_f(wide & 31) * (1.0 / 31.0);
        (r, g, b)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2885-L2889 (chrome/m156)
    /// Derive alpha's coverage from rgb coverage and the values of src and dst alpha.
    fn alpha_coverage_from_rgb_coverage(a: F, da: F, cr: F, cg: F, cb: F) -> F {
        if_then_else_f(
            a.lt_mask(da),
            min_f(cr, min_f(cg, cb)),
            max_f(cr, max_f(cg, cb)),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2922-L2924 (chrome/m156)
    /// `lerp(from, to, t)`: `mad(to - from, t, from)`.
    fn lerp(from: F, to: F, t: F) -> F {
        mad(to - from, t, from)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2891-L2896 (chrome/m156)
    pub(super) fn scale_1_float(ctx: &Cell<f32>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let c = ctx.get();
        p.r *= c;
        p.g *= c;
        p.b *= c;
        p.a *= c;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2897-L2906 (chrome/m156)
    pub(super) fn scale_u8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = from_byte(U8::load_bytes(e.ptr_at_xy(ctx, 1)));

        p.r *= c;
        p.g *= c;
        p.b *= c;
        p.a *= c;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2908-L2920 (chrome/m156)
    pub(super) fn scale_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (cr, cg, cb) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));

        let ca = alpha_coverage_from_rgb_coverage(p.a, p.da, cr, cg, cb);

        p.r *= cr;
        p.g *= cg;
        p.b *= cb;
        p.a *= ca;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2926-L2931 (chrome/m156)
    pub(super) fn lerp_1_float(ctx: &Cell<f32>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let c = F::splat(ctx.get());
        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2932-L2938 (chrome/m156)
    pub(super) fn scale_native(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = F::load_bytes(&e.ptr(ctx)[..F_BYTES]);
        p.r *= c;
        p.g *= c;
        p.b *= c;
        p.a *= c;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2939-L2945 (chrome/m156)
    pub(super) fn lerp_native(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = F::load_bytes(&e.ptr(ctx)[..F_BYTES]);
        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2946-L2955 (chrome/m156)
    pub(super) fn lerp_u8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = from_byte(U8::load_bytes(e.ptr_at_xy(ctx, 1)));

        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2956-L2970 (chrome/m156)
    pub(super) fn lerp_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (cr, cg, cb) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));

        let ca = alpha_coverage_from_rgb_coverage(p.a, p.da, cr, cg, cb);

        p.r = lerp(p.dr, p.r, cr);
        p.g = lerp(p.dg, p.g, cg);
        p.b = lerp(p.db, p.b, cb);
        p.a = lerp(p.da, p.a, ca);
    }
}
