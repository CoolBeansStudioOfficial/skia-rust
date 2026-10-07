// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! 8-bit memory stages: load/store/gather of a8, 565, 4444, 8888, rg88, `store_r8`,
//! `srcover_rgba_8888`, `swap_rb`, `alpha_to_*`, `debug_*`.
//!
//! Owner: task B1 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

// `1/255.0f`: a float division of exactly representable integers.
const INV_255: f32 = 1.0 / 255.0;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2007-L2009 (chrome/m156)
    fn from_byte(b: U8) -> F {
        cast_f(b.cast::<u32>()) * INV_255
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2013-L2018 (chrome/m156)
    fn from_565(v: U16) -> (F, F, F) {
        let wide: U32 = v.cast();
        (
            cast_f(wide & (31 << 11)) * (1.0 / 63488.0),
            cast_f(wide & (63 << 5)) * (1.0 / 2016.0),
            cast_f(wide & 31) * (1.0 / 31.0),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2019-L2025 (chrome/m156)
    fn from_4444(v: U16) -> (F, F, F, F) {
        let wide: U32 = v.cast();
        (
            cast_f(wide & (15 << 12)) * (1.0 / 61440.0),
            cast_f(wide & (15 << 8)) * (1.0 / 3840.0),
            cast_f(wide & (15 << 4)) * (1.0 / 240.0),
            cast_f(wide & 15) * (1.0 / 15.0),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2026-L2031 (chrome/m156)
    fn from_8888(v: U32) -> (F, F, F, F) {
        (
            cast_f(v & 0xff) * INV_255,
            cast_f((v >> 8) & 0xff) * INV_255,
            cast_f((v >> 16) & 0xff) * INV_255,
            cast_f(v >> 24) * INV_255,
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2032-L2036 (chrome/m156)
    fn from_88(v: U16) -> (F, F) {
        let wide: U32 = v.cast();
        (
            cast_f(wide & 0xff) * INV_255,
            cast_f((wide >> 8) & 0xff) * INV_255,
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2269-L2272 (chrome/m156)
    fn to_unorm_full(v: F, scale: f32, bias: f32, max_i: i32) -> U32 {
        // Any time we use round() we probably want to use to_unorm().
        #[allow(clippy::cast_precision_loss)] // mirrors `(float) maxI`
        let max = max_i as f32;
        round(min_f(
            max_f(F::splat(0.0), mad(v, F::splat(scale), F::splat(bias))),
            F::splat(max),
        ))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2273-L2275 (chrome/m156)
    fn to_unorm(v: F, scale: i32) -> U32 {
        #[allow(clippy::cast_precision_loss)] // mirrors `(float) scale`
        let s = scale as f32;
        to_unorm_full(v, s, 0.0, scale)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2090-L2095 (chrome/m156)
    /// `clamp_ex(v, limit)`: clamp to `(0, limit)`.
    fn clamp_ex(v: F, limit: f32) -> F {
        let inclusive_z = F::splat(f32::MIN_POSITIVE);
        let limit_bits: U32 = F::splat(limit).bit_cast();
        let inclusive_l: F = (limit_bits - 1).bit_cast();
        min_f(max_f(inclusive_z, v), inclusive_l)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2245-L2256 (chrome/m156)
    /// `ix_and_ptr`: the vector of pixel indices for the gather at `(x, y)`.
    fn gather_ix(ctx: &GatherCtx<'_>, x: F, y: F) -> U32 {
        // We use exclusive clamp so that our min value is > 0 because ULP subtraction using U32
        // would produce a NaN if applied to +0.f.
        let x = clamp_ex(x, ctx.width);
        let y = clamp_ex(y, ctx.height);
        let down = u32::from(ctx.round_down_at_integer);
        let (xb, yb): (U32, U32) = (x.bit_cast(), y.bit_cast());
        let x: F = (xb - down).bit_cast();
        let y: F = (yb - down).bit_cast();
        // `trunc_(y)*ctx->stride`: the `int` stride converts to `uint32_t`.
        #[allow(clippy::cast_sign_loss)] // mirrors the int → U32 conversion
        let stride = ctx.stride as u32;
        trunc_(y) * stride + trunc_(x)
    }

    /// `gather<U8>(ptr, ix)`.
    fn gather_u8(ctx: &GatherCtx<'_>, ix: U32) -> U8 {
        let mut lanes = [0u8; MAX_STRIDE_HIGHP];
        for (i, lane) in lanes.iter_mut().enumerate().take(N) {
            *lane = ctx.pixels[ix[i] as usize];
        }
        U8::load(&lanes[..N])
    }

    /// `gather_unaligned<U16>(ptr, ix)`.
    fn gather_u16(ctx: &GatherCtx<'_>, ix: U32) -> U16 {
        let mut lanes = [0u16; MAX_STRIDE_HIGHP];
        for (i, lane) in lanes.iter_mut().enumerate().take(N) {
            let at = 2 * ix[i] as usize;
            *lane = u16::from_ne_bytes([ctx.pixels[at], ctx.pixels[at + 1]]);
        }
        U16::load(&lanes[..N])
    }

    /// `gather_unaligned<U32>(ptr, ix)`.
    fn gather_u32(ctx: &GatherCtx<'_>, ix: U32) -> U32 {
        let mut lanes = [0u32; MAX_STRIDE_HIGHP];
        for (i, lane) in lanes.iter_mut().enumerate().take(N) {
            let at = 4 * ix[i] as usize;
            *lane = u32::from_ne_bytes([
                ctx.pixels[at],
                ctx.pixels[at + 1],
                ctx.pixels[at + 2],
                ctx.pixels[at + 3],
            ]);
        }
        U32::load(&lanes[..N])
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2670-L2674 (chrome/m156)
    pub(super) fn swap_rb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.r, &mut p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2675-L2679 (chrome/m156)
    pub(super) fn swap_rb_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.dr, &mut p.db);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3091-L3096 (chrome/m156)
    pub(super) fn load_a8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = from_byte(U8::load_bytes(e.ptr_at_xy(ctx, 1)));
        p.r = F::splat(0.0);
        p.g = F::splat(0.0);
        p.b = F::splat(0.0);
        p.a = a;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3097-L3102 (chrome/m156)
    pub(super) fn load_a8_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = from_byte(U8::load_bytes(e.ptr_at_xy(ctx, 1)));
        p.dr = F::splat(0.0);
        p.dg = F::splat(0.0);
        p.db = F::splat(0.0);
        p.da = a;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3109-L3114 (chrome/m156)
    pub(super) fn store_a8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let packed: U8 = pack_u16(pack_u32(to_unorm(p.a, 255)));
        packed.store_bytes(e.ptr_at_xy_mut(ctx, 1));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3103-L3108 (chrome/m156)
    pub(super) fn gather_a8(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, p.r, p.g);
        p.r = F::splat(0.0);
        p.g = F::splat(0.0);
        p.b = F::splat(0.0);
        p.a = from_byte(gather_u8(ctx, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3122-L3127 (chrome/m156)
    pub(super) fn load_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3128-L3133 (chrome/m156)
    pub(super) fn load_565_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3140-L3147 (chrome/m156)
    pub(super) fn store_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U16 = pack_u32(
            to_unorm(p.r, 31) << 11 | to_unorm(p.g, 63) << 5 | to_unorm(p.b, 31),
        );
        px.store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3134-L3139 (chrome/m156)
    pub(super) fn gather_565(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, p.r, p.g);
        (p.r, p.g, p.b) = from_565(gather_u16(ctx, ix));
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3149-L3152 (chrome/m156)
    pub(super) fn load_4444(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b, p.a) = from_4444(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3153-L3156 (chrome/m156)
    pub(super) fn load_4444_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db, p.da) = from_4444(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3162-L3169 (chrome/m156)
    pub(super) fn store_4444(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U16 = pack_u32(
            to_unorm(p.r, 15) << 12
                | to_unorm(p.g, 15) << 8
                | to_unorm(p.b, 15) << 4
                | to_unorm(p.a, 15),
        );
        px.store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3157-L3161 (chrome/m156)
    pub(super) fn gather_4444(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, p.r, p.g);
        (p.r, p.g, p.b, p.a) = from_4444(gather_u16(ctx, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3171-L3174 (chrome/m156)
    pub(super) fn load_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b, p.a) = from_8888(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3175-L3178 (chrome/m156)
    pub(super) fn load_8888_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db, p.da) = from_8888(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3184-L3192 (chrome/m156)
    pub(super) fn store_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U32 = to_unorm(p.r, 255)
            | to_unorm(p.g, 255) << 8
            | to_unorm(p.b, 255) << 16
            | to_unorm(p.a, 255) << 24;
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3179-L3183 (chrome/m156)
    pub(super) fn gather_8888(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, p.r, p.g);
        (p.r, p.g, p.b, p.a) = from_8888(gather_u32(ctx, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3194-L3199 (chrome/m156)
    pub(super) fn load_rg88(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g) = from_88(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3200-L3205 (chrome/m156)
    pub(super) fn load_rg88_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg) = from_88(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.db = F::splat(0.0);
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3213-L3217 (chrome/m156)
    pub(super) fn store_rg88(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U16 = pack_u32(to_unorm(p.r, 255) | to_unorm(p.g, 255) << 8);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3206-L3212 (chrome/m156)
    pub(super) fn gather_rg88(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, p.r, p.g);
        (p.r, p.g) = from_88(gather_u16(ctx, ix));
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3115-L3120 (chrome/m156)
    pub(super) fn store_r8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let packed: U8 = pack_u16(pack_u32(to_unorm(p.r, 255)));
        packed.store_bytes(e.ptr_at_xy_mut(ctx, 1));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3647-L3650 (chrome/m156)
    pub(super) fn alpha_to_gray(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.a;
        p.g = p.a;
        p.b = p.a;
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3651-L3654 (chrome/m156)
    pub(super) fn alpha_to_gray_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.da;
        p.dg = p.da;
        p.db = p.da;
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3655-L3658 (chrome/m156)
    pub(super) fn alpha_to_red(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.a;
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3659-L3662 (chrome/m156)
    pub(super) fn alpha_to_red_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.da;
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2613-L2636 (chrome/m156)
    pub(super) fn srcover_rgba_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut dst = U32::load_bytes(e.ptr_at_xy(ctx, 4));
        p.dr = cast_f(dst & 0xff);
        p.dg = cast_f((dst >> 8) & 0xff);
        p.db = cast_f((dst >> 16) & 0xff);
        p.da = cast_f(dst >> 24);
        // {dr,dg,db,da} are in [0,255]
        // { r, g, b, a} are in [0,  1] (but may be out of gamut)

        let inv_a = F::splat(1.0) - p.a; // inv(a)
        p.r = mad(p.dr, inv_a, p.r * 255.0);
        p.g = mad(p.dg, inv_a, p.g * 255.0);
        p.b = mad(p.db, inv_a, p.b * 255.0);
        p.a = mad(p.da, inv_a, p.a * 255.0);
        // { r, g, b, a} are now in [0,255]  (but may be out of gamut)

        // to_unorm() clamps back to gamut.  Scaling by 1 since we're already 255-based.
        dst = to_unorm_full(p.r, 1.0, 0.0, 255)
            | to_unorm_full(p.g, 1.0, 0.0, 255) << 8
            | to_unorm_full(p.b, 1.0, 0.0, 255) << 16
            | to_unorm_full(p.a, 1.0, 0.0, 255) << 24;
        dst.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7378-L7388 (chrome/m156)
    fn highp_fixed_point(ctx: MemoryCtx, e: &mut Params<'_, '_>, lane: F) {
        let r2 = trunc_(abs_f(lane) / 256.0) & 0xFF;
        let g2 = trunc_(abs_f(lane)) & 0xFF;
        let b2 = trunc_(abs_f(lane) * 256.0) & 0xFF;
        let a2 = to_unorm(lane * -256.0 * 256.0, 255);
        let px: U32 = r2 | (g2 << 8) | (b2 << 16) | (a2 << 24);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7406-L7408 (chrome/m156)
    // The special handling of x and y only make sense in lowp mode. If they are called in highp
    // mode, Skia treats x and y as r and g respectively.
    pub(super) fn debug_x(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.r);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7409-L7411 (chrome/m156)
    pub(super) fn debug_y(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7390-L7392 (chrome/m156)
    pub(super) fn debug_r(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.r);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7393-L7395 (chrome/m156)
    pub(super) fn debug_g(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7396-L7398 (chrome/m156)
    pub(super) fn debug_b(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7399-L7401 (chrome/m156)
    pub(super) fn debug_a(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        highp_fixed_point(ctx, e, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7349-L7355 (chrome/m156)
    pub(super) fn debug_r_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px = to_unorm(p.r, 255);
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7356-L7362 (chrome/m156)
    pub(super) fn debug_g_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px = to_unorm(p.g, 255) << 8;
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7363-L7369 (chrome/m156)
    pub(super) fn debug_b_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px = to_unorm(p.b, 255) << 16;
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7370-L7376 (chrome/m156)
    pub(super) fn debug_a_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px = to_unorm(p.a, 255);
        // Render alpha as greyscale
        let out: U32 = px | px << 8 | px << 16 | px << 24;
        out.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }
}
