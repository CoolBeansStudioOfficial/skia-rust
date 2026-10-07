// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Wide memory stages: the f16 family, f32, `16161616/a16/r16/rg1616`, `1010102/xr`, `10x6`,
//! `10101010_xr`, `load_src_rg`/`store_src_rg`.
//!
//! Owner: task B2 (`docs/design/raster-pipeline.md` §5).
//!
//! Skia's `load2`/`load4`/`store2`/`store4` and `gather_unaligned` are per-tier SIMD transposes
//! and gathers whose results are the plain interleaved reads and writes; here they are the lane
//! loops `load_u16s`/`store_u16s`/`load_f32s`/`store_f32s`/`gather_*`, all bounds-checked.

// Skia's stages name the color registers r, g, b, a (and dr, dg, db, da).
#![allow(clippy::many_single_char_names)]

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `F` register (`N` floats).
const F_BYTES: usize = 4 * N;

// The helpers below are private so they never collide with the same-named helpers other tasks'
// stage files export (a local item shadows a glob import).
si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2010-L2012 (chrome/m156)
    fn from_short_w(s: U16) -> F {
        let wide: U32 = s.cast(); // expand
        cast_f(wide) * (1.0 / 65535.0)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2037-L2042 (chrome/m156)
    fn from_1010102_w(rgba: U32) -> (F, F, F, F) {
        (
            cast_f(rgba & 0x3ff) * (1.0 / 1023.0),
            cast_f((rgba >> 10) & 0x3ff) * (1.0 / 1023.0),
            cast_f((rgba >> 20) & 0x3ff) * (1.0 / 1023.0),
            cast_f(rgba >> 30) * (1.0 / 3.0),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2043-L2051 (chrome/m156)
    fn from_1010102_xr_w(rgba: U32) -> (F, F, F, F) {
        // Match https://developer.apple.com/documentation/metal/mtlpixelformat/bgr10_xr?language=objc
        // i.e. "float = (xr10_value - 384) / 510.0f", but with the modification that we store 2
        // bits of alpha with a regular unorm encoding.
        (
            (cast_f(rgba & 0x3ff) - 384.0) * (1.0 / 510.0),
            (cast_f((rgba >> 10) & 0x3ff) - 384.0) * (1.0 / 510.0),
            (cast_f((rgba >> 20) & 0x3ff) - 384.0) * (1.0 / 510.0),
            cast_f(rgba >> 30) * (1.0 / 3.0), // A in 1010102_xr is *not* extended range
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2052-L2060 (chrome/m156)
    /// One channel of `from_10101010_xr`, from its 16 bits.
    fn from_10101010_xr_ch(h: U16) -> F {
        // The linear transformation is the same as 1010102_xr, except the integer encoding is
        // shifted to have 6 low bits of padding.
        let w: U32 = h.cast();
        (cast_f((w >> 6) & 0x3ff) - 384.0) * (1.0 / 510.0)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2061-L2066 (chrome/m156)
    /// One channel of `from_10x6`, from its 16 bits.
    fn from_10x6_ch(h: U16) -> F {
        let w: U32 = h.cast();
        cast_f((w >> 6) & 0x3ff) * (1.0 / 1023.0)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2067-L2070 (chrome/m156)
    fn from_1616_w(x: U32) -> (F, F) {
        (
            cast_f(x & 0xffff) * (1.0 / 65535.0),
            cast_f((x >> 16) & 0xffff) * (1.0 / 65535.0),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2269-L2272 (chrome/m156)
    /// `to_unorm(v, scale, bias, maxI)`: `v * scale + bias` clamped to `[0, max_i]` and rounded.
    fn to_unorm_w(v: F, scale: f32, bias: f32, max_i: f32) -> U32 {
        // Any time we use round() we probably want to use to_unorm().
        round(min_f(
            max_f(F::splat(0.0), mad(v, F::splat(scale), F::splat(bias))),
            F::splat(max_i),
        ))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2273-L2275 (chrome/m156)
    /// `to_unorm(v, int scale)`.
    fn to_unorm_scale(v: F, scale: f32) -> U32 {
        to_unorm_w(v, scale, 0.0, scale)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2090-L2095 (chrome/m156)
    /// `clamp_ex`: clamp `v` to `(0, limit)`.
    fn clamp_ex_w(v: F, limit: f32) -> F {
        let inclusive_z = F::splat(f32::MIN_POSITIVE);
        let limit_bits: U32 = F::splat(limit).bit_cast();
        let inclusive_l: F = (limit_bits - 1).bit_cast();
        min_f(max_f(inclusive_z, v), inclusive_l)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2246-L2256 (chrome/m156)
    /// `ix_and_ptr`: the pixel index (in pixels from the start of the gather memory) of each lane.
    fn ix_and_ptr_w(ctx: &GatherCtx<'_>, x: F, y: F) -> U32 {
        // We use exclusive clamp so that our min value is > 0 because ULP subtraction using U32
        // would produce a NaN if applied to +0.f.
        let x = clamp_ex_w(x, ctx.width);
        let y = clamp_ex_w(y, ctx.height);
        let round_down = u32::from(ctx.round_down_at_integer);
        let (xb, yb): (U32, U32) = (x.bit_cast(), y.bit_cast());
        let x: F = (xb - round_down).bit_cast();
        let y: F = (yb - round_down).bit_cast();
        // `stride` is an `int` that Skia converts to `U32`.
        #[allow(clippy::cast_sign_loss)]
        let stride = ctx.stride as u32;
        trunc_(y) * stride + trunc_(x)
    }

    /// The byte offset of 16-bit channel `k` of element `i` of `elem_bytes` bytes.
    fn gather_offset(i: u32, elem_bytes: usize, k: usize) -> usize {
        (i as usize)
            .checked_mul(elem_bytes)
            .and_then(|o| o.checked_add(2 * k))
            .expect("raster pipeline: gather index out of range")
    }

    /// `gather_unaligned` of 16-bit channel `k` of each lane's `elem_bytes`-byte element (Skia
    /// gathers the whole element; the channels are what its callers extract).
    fn gather_u16s(px: &[u8], ix: U32, elem_bytes: usize, k: usize) -> U16 {
        ix.map(|i| {
            let o = gather_offset(i, elem_bytes, k);
            u16::from_ne_bytes([px[o], px[o + 1]])
        })
    }

    /// `gather_unaligned` of 32-bit elements at element index `ix`.
    fn gather_u32(px: &[u8], ix: U32) -> U32 {
        ix.map(|i| {
            let o = gather_offset(i, 4, 0);
            u32::from_ne_bytes([px[o], px[o + 1], px[o + 2], px[o + 3]])
        })
    }

    /// Channel `k` (16 bits) of each lane's pixel of `bpp` bytes (`load2`/`load4`).
    fn load_u16s(ptr: &[u8], bpp: usize, k: usize) -> U16 {
        let mut lanes = [0u16; MAX_STRIDE_HIGHP];
        for (i, lane) in lanes.iter_mut().take(N).enumerate() {
            let o = i * bpp + 2 * k;
            *lane = u16::from_ne_bytes([ptr[o], ptr[o + 1]]);
        }
        U16::load(&lanes[..N])
    }

    /// Writes channel `k` (16 bits) of each lane's pixel of `bpp` bytes (`store2`/`store4`).
    fn store_u16s(ptr: &mut [u8], bpp: usize, k: usize, v: U16) {
        for i in 0..N {
            let o = i * bpp + 2 * k;
            ptr[o..o + 2].copy_from_slice(&v[i].to_ne_bytes());
        }
    }

    /// Channel `c` of each lane's float pixel (`load4`).
    fn load_f32s(ptr: &[u8], c: usize) -> F {
        let mut lanes = [0f32; MAX_STRIDE_HIGHP];
        for (i, lane) in lanes.iter_mut().take(N).enumerate() {
            let o = i * 16 + 4 * c;
            *lane = f32::from_ne_bytes([ptr[o], ptr[o + 1], ptr[o + 2], ptr[o + 3]]);
        }
        F::load(&lanes[..N])
    }

    /// Writes channel `c` of each lane's float pixel (`store4`).
    fn store_f32s(ptr: &mut [u8], c: usize, v: F) {
        for i in 0..N {
            let o = i * 16 + 4 * c;
            ptr[o..o + 4].copy_from_slice(&v[i].to_ne_bytes());
        }
    }

    /// `load4` of the 16-bit channels of 8-byte pixels.
    fn load4_u16(ptr: &[u8]) -> [U16; 4] {
        [
            load_u16s(ptr, 8, 0),
            load_u16s(ptr, 8, 1),
            load_u16s(ptr, 8, 2),
            load_u16s(ptr, 8, 3),
        ]
    }

    /// `store4` of the 16-bit channels of 8-byte pixels.
    fn store4_u16(ptr: &mut [u8], c: [U16; 4]) {
        store_u16s(ptr, 8, 0, c[0]);
        store_u16s(ptr, 8, 1, c[1]);
        store_u16s(ptr, 8, 2, c[2]);
        store_u16s(ptr, 8, 3, c[3]);
    }

    /// `gather_unaligned` of 8-byte pixels, as their four 16-bit channels.
    fn gather4_u16(px: &[u8], ix: U32) -> [U16; 4] {
        [
            gather_u16s(px, ix, 8, 0),
            gather_u16s(px, ix, 8, 1),
            gather_u16s(px, ix, 8, 2),
            gather_u16s(px, ix, 8, 3),
        ]
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2071-L2076 (chrome/m156)
    /// `from_16161616` of the four 16-bit channels.
    fn from_16161616_w(c: [U16; 4]) -> [F; 4] {
        [
            from_short_w(c[0]),
            from_short_w(c[1]),
            from_short_w(c[2]),
            from_short_w(c[3]),
        ]
    }

    /// `from_10x6` of the four 16-bit channels.
    fn from_10x6_w(c: [U16; 4]) -> [F; 4] {
        [
            from_10x6_ch(c[0]),
            from_10x6_ch(c[1]),
            from_10x6_ch(c[2]),
            from_10x6_ch(c[3]),
        ]
    }

    /// `from_10101010_xr` of the four 16-bit channels.
    fn from_10101010_xr_w(c: [U16; 4]) -> [F; 4] {
        [
            from_10101010_xr_ch(c[0]),
            from_10101010_xr_ch(c[1]),
            from_10101010_xr_ch(c[2]),
            from_10101010_xr_ch(c[3]),
        ]
    }

    /// `from_half` of the four 16-bit channels.
    fn from_half4(c: [U16; 4]) -> [F; 4] {
        [from_half(c[0]), from_half(c[1]), from_half(c[2]), from_half(c[3])]
    }

    /// `pack(to_unorm(v, scale, bias, maxI)) << 6`.
    fn pack_10x6(v: F, scale: f32, bias: f32) -> U16 {
        pack_u32(to_unorm_w(v, scale, bias, 1023.0)) << 6
    }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L3295-L3298 (chrome/m156)
    pub(super) fn load_16161616(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_16161616_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3299-L3302 (chrome/m156)
    pub(super) fn load_16161616_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_16161616_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3308-L3317 (chrome/m156)
    pub(super) fn store_16161616(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = [
            pack_u32(to_unorm_scale(p.r, 65535.0)),
            pack_u32(to_unorm_scale(p.g, 65535.0)),
            pack_u32(to_unorm_scale(p.b, 65535.0)),
            pack_u32(to_unorm_scale(p.a, 65535.0)),
        ];
        store4_u16(e.ptr_at_xy_mut(ctx, 8), c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3303-L3307 (chrome/m156)
    pub(super) fn gather_16161616(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let [r, g, b, a] = from_16161616_w(gather4_u16(ctx.pixels, ix));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3219-L3223 (chrome/m156)
    pub(super) fn load_a16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = from_short_w(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        let zero = F::splat(0.0);
        (p.r, p.g, p.b, p.a) = (zero, zero, zero, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3224-L3228 (chrome/m156)
    pub(super) fn load_a16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = from_short_w(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        let zero = F::splat(0.0);
        (p.dr, p.dg, p.db, p.da) = (zero, zero, zero, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3235-L3240 (chrome/m156)
    pub(super) fn store_a16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px = pack_u32(to_unorm_scale(p.a, 65535.0));
        px.store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3229-L3234 (chrome/m156)
    pub(super) fn gather_a16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let zero = F::splat(0.0);
        (p.r, p.g, p.b) = (zero, zero, zero);
        p.a = from_short_w(gather_u16s(ctx.pixels, ix, 2, 0));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3242-L3247 (chrome/m156)
    pub(super) fn load_r16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let zero = F::splat(0.0);
        (p.g, p.b) = (zero, zero);
        p.a = F::splat(1.0);
        p.r = from_short_w(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3248-L3253 (chrome/m156)
    pub(super) fn load_r16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let zero = F::splat(0.0);
        (p.dg, p.db) = (zero, zero);
        p.da = F::splat(1.0);
        p.dr = from_short_w(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3261-L3266 (chrome/m156)
    pub(super) fn store_r16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px = pack_u32(to_unorm_scale(p.r, 65535.0));
        px.store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3254-L3260 (chrome/m156)
    pub(super) fn gather_r16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let zero = F::splat(0.0);
        (p.g, p.b) = (zero, zero);
        p.a = F::splat(1.0);
        p.r = from_short_w(gather_u16s(ctx.pixels, ix, 2, 0));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3268-L3273 (chrome/m156)
    pub(super) fn load_rg1616(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
        (p.r, p.g) = from_1616_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3274-L3279 (chrome/m156)
    pub(super) fn load_rg1616_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg) = from_1616_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
        p.db = F::splat(0.0);
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3287-L3293 (chrome/m156)
    pub(super) fn store_rg1616(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U32 =
            to_unorm_scale(p.r, 65535.0) | (to_unorm_scale(p.g, 65535.0) << 16);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3280-L3286 (chrome/m156)
    pub(super) fn gather_rg1616(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        (p.r, p.g) = from_1616_w(gather_u32(ctx.pixels, ix));
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3413-L3422 (chrome/m156)
    pub(super) fn load_f16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_half4(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3423-L3432 (chrome/m156)
    pub(super) fn load_f16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_half4(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3445-L3451 (chrome/m156)
    pub(super) fn store_f16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = [to_half(p.r), to_half(p.g), to_half(p.b), to_half(p.a)];
        store4_u16(e.ptr_at_xy_mut(ctx, 8), c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3433-L3444 (chrome/m156)
    pub(super) fn gather_f16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let [r, g, b, a] = from_half4(gather4_u16(ctx.pixels, ix));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3453-L3461 (chrome/m156)
    pub(super) fn load_af16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = U16::load_bytes(e.ptr_at_xy(ctx, 2));
        let zero = F::splat(0.0);
        (p.r, p.g, p.b) = (zero, zero, zero);
        p.a = from_half(a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3462-L3468 (chrome/m156)
    pub(super) fn load_af16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let a = U16::load_bytes(e.ptr_at_xy(ctx, 2));
        let zero = F::splat(0.0);
        (p.dr, p.dg, p.db) = (zero, zero, zero);
        p.da = from_half(a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3475-L3478 (chrome/m156)
    pub(super) fn store_af16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        to_half(p.a).store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3469-L3474 (chrome/m156)
    pub(super) fn gather_af16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let zero = F::splat(0.0);
        (p.r, p.g, p.b) = (zero, zero, zero);
        p.a = from_half(gather_u16s(ctx.pixels, ix, 2, 0));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3480-L3487 (chrome/m156)
    pub(super) fn load_rf16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let r = U16::load_bytes(e.ptr_at_xy(ctx, 2));
        p.r = from_half(r);
        let zero = F::splat(0.0);
        (p.g, p.b) = (zero, zero);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3488-L3495 (chrome/m156)
    pub(super) fn load_rf16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let r = U16::load_bytes(e.ptr_at_xy(ctx, 2));
        p.dr = from_half(r);
        let zero = F::splat(0.0);
        (p.dg, p.db) = (zero, zero);
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3503-L3506 (chrome/m156)
    pub(super) fn store_rf16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        to_half(p.r).store_bytes(e.ptr_at_xy_mut(ctx, 2));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3496-L3502 (chrome/m156)
    pub(super) fn gather_rf16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        p.r = from_half(gather_u16s(ctx.pixels, ix, 2, 0));
        let zero = F::splat(0.0);
        (p.g, p.b) = (zero, zero);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3508-L3517 (chrome/m156)
    pub(super) fn load_rgf16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = e.ptr_at_xy(ctx, 4);
        let (r, g) = (load_u16s(ptr, 4, 0), load_u16s(ptr, 4, 1));
        p.r = from_half(r);
        p.g = from_half(g);
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3518-L3527 (chrome/m156)
    pub(super) fn load_rgf16_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = e.ptr_at_xy(ctx, 4);
        let (r, g) = (load_u16s(ptr, 4, 0), load_u16s(ptr, 4, 1));
        p.dr = from_half(r);
        p.dg = from_half(g);
        p.db = F::splat(0.0);
        p.da = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3540-L3544 (chrome/m156)
    pub(super) fn store_rgf16(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g) = (to_half(p.r), to_half(p.g));
        let ptr = e.ptr_at_xy_mut(ctx, 4);
        store_u16s(ptr, 4, 0, r);
        store_u16s(ptr, 4, 1, g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3528-L3539 (chrome/m156)
    pub(super) fn gather_rgf16(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let (r, g) = (
            gather_u16s(ctx.pixels, ix, 4, 0),
            gather_u16s(ctx.pixels, ix, 4, 1),
        );
        p.r = from_half(r);
        p.g = from_half(g);
        p.b = F::splat(0.0);
        p.a = F::splat(1.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3546-L3549 (chrome/m156)
    pub(super) fn load_f32(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = e.ptr_at_xy(ctx, 16);
        let (r, g, b, a) = (
            load_f32s(ptr, 0),
            load_f32s(ptr, 1),
            load_f32s(ptr, 2),
            load_f32s(ptr, 3),
        );
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3550-L3553 (chrome/m156)
    pub(super) fn load_f32_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = e.ptr_at_xy(ctx, 16);
        let (r, g, b, a) = (
            load_f32s(ptr, 0),
            load_f32s(ptr, 1),
            load_f32s(ptr, 2),
            load_f32s(ptr, 3),
        );
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3562-L3565 (chrome/m156)
    pub(super) fn store_f32(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g, b, a) = (p.r, p.g, p.b, p.a);
        let ptr = e.ptr_at_xy_mut(ctx, 16);
        store_f32s(ptr, 0, r);
        store_f32s(ptr, 1, g);
        store_f32s(ptr, 2, b);
        store_f32s(ptr, 3, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3554-L3561 (chrome/m156)
    pub(super) fn gather_f32(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        // `4*ix + c` indexes floats.
        p.r = gather_u32(ctx.pixels, ix * 4u32).bit_cast();
        p.g = gather_u32(ctx.pixels, ix * 4u32 + 1u32).bit_cast();
        p.b = gather_u32(ctx.pixels, ix * 4u32 + 2u32).bit_cast();
        p.a = gather_u32(ctx.pixels, ix * 4u32 + 3u32).bit_cast();
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3343-L3346 (chrome/m156)
    pub(super) fn load_1010102(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g, b, a) = from_1010102_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3347-L3350 (chrome/m156)
    pub(super) fn load_1010102_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g, b, a) = from_1010102_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3393-L3401 (chrome/m156)
    pub(super) fn store_1010102(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U32 = to_unorm_scale(p.r, 1023.0)
            | (to_unorm_scale(p.g, 1023.0) << 10)
            | (to_unorm_scale(p.b, 1023.0) << 20)
            | (to_unorm_scale(p.a, 3.0) << 30);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3359-L3363 (chrome/m156)
    pub(super) fn gather_1010102(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        (p.r, p.g, p.b, p.a) = from_1010102_w(gather_u32(ctx.pixels, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3351-L3354 (chrome/m156)
    pub(super) fn load_1010102_xr(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g, b, a) = from_1010102_xr_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3355-L3358 (chrome/m156)
    pub(super) fn load_1010102_xr_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (r, g, b, a) = from_1010102_xr_w(U32::load_bytes(e.ptr_at_xy(ctx, 4)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3402-L3411 (chrome/m156)
    pub(super) fn store_1010102_xr(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        // This is the inverse of from_1010102_xr, e.g. (v * 510 + 384)
        let px: U32 = to_unorm_w(p.r, 510.0, 384.0, 1023.0)
            | (to_unorm_w(p.g, 510.0, 384.0, 1023.0) << 10)
            | (to_unorm_w(p.b, 510.0, 384.0, 1023.0) << 20)
            | (to_unorm_scale(p.a, 3.0) << 30);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3364-L3368 (chrome/m156)
    pub(super) fn gather_1010102_xr(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        (p.r, p.g, p.b, p.a) = from_1010102_xr_w(gather_u32(ctx.pixels, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3319-L3322 (chrome/m156)
    pub(super) fn load_10x6(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_10x6_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3323-L3326 (chrome/m156)
    pub(super) fn load_10x6_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_10x6_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3332-L3341 (chrome/m156)
    pub(super) fn store_10x6(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = [
            pack_10x6(p.r, 1023.0, 0.0),
            pack_10x6(p.g, 1023.0, 0.0),
            pack_10x6(p.b, 1023.0, 0.0),
            pack_10x6(p.a, 1023.0, 0.0),
        ];
        store4_u16(e.ptr_at_xy_mut(ctx, 8), c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3327-L3331 (chrome/m156)
    pub(super) fn gather_10x6(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let [r, g, b, a] = from_10x6_w(gather4_u16(ctx.pixels, ix));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3369-L3373 (chrome/m156)
    pub(super) fn gather_10101010_xr(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = ix_and_ptr_w(ctx, p.r, p.g);
        let [r, g, b, a] = from_10101010_xr_w(gather4_u16(ctx.pixels, ix));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3374-L3377 (chrome/m156)
    pub(super) fn load_10101010_xr(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_10101010_xr_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.r, p.g, p.b, p.a) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3378-L3381 (chrome/m156)
    pub(super) fn load_10101010_xr_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let [r, g, b, a] = from_10101010_xr_w(load4_u16(e.ptr_at_xy(ctx, 8)));
        (p.dr, p.dg, p.db, p.da) = (r, g, b, a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3382-L3392 (chrome/m156)
    pub(super) fn store_10101010_xr(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        // This is the inverse of from_10101010_xr, e.g. (v * 510 + 384)
        let c = [
            pack_10x6(p.r, 510.0, 384.0),
            pack_10x6(p.g, 510.0, 384.0),
            pack_10x6(p.b, 510.0, 384.0),
            pack_10x6(p.a, 510.0, 384.0),
        ];
        store4_u16(e.ptr_at_xy_mut(ctx, 8), c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2397-L2400 (chrome/m156)
    /// Stores registers `r,g` into the context.
    pub(super) fn store_src_rg(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &mut e.ptr_mut(ctx)[..2 * F_BYTES];
        p.r.store_bytes(ptr);
        p.g.store_bytes(&mut ptr[F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2402-L2405 (chrome/m156)
    /// Loads registers `r,g` from the context.
    pub(super) fn load_src_rg(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &e.ptr(ctx)[..2 * F_BYTES];
        p.r = F::load_bytes(ptr);
        p.g = F::load_bytes(&ptr[F_BYTES..]);
    }
}
