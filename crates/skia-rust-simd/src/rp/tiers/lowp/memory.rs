// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! 8-bit memory stages (lowp): load/store/gather of a8, 565, 4444, 8888, rg88, `store_r8`,
//! `srcover_rgba_8888`, `swap_rb`, `alpha_to_*`, `debug_*`.
//!
//! Owner: task B1 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L6194-L6203 (chrome/m156)
    pub(super) fn swap_rb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.r, &mut p.b);
    }

    pub(super) fn swap_rb_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.dr, &mut p.db);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6305-L6321 (chrome/m156)
    /// `ix_and_ptr`: the vector of pixel indices for the gather at `(x, y)`.
    fn gather_ix(ctx: &GatherCtx<'_>, xs: F, ys: F) -> U32 {
        // Exclusive -> inclusive.
        let (wb, hb): (U32, U32) = (
            F::splat(ctx.width).bit_cast(),
            F::splat(ctx.height).bit_cast(),
        );
        let max_x: F = (wb - 1).bit_cast();
        let max_y: F = (hb - 1).bit_cast();

        let zero = F::splat(f32::MIN_POSITIVE);

        let xs = min_intr_f(max_intr_f(zero, xs), max_x);
        let ys = min_intr_f(max_intr_f(zero, ys), max_y);

        let down = u32::from(ctx.round_down_at_integer);
        let (xb, yb): (U32, U32) = (xs.bit_cast(), ys.bit_cast());
        let xs: F = (xb - down).bit_cast();
        let ys: F = (yb - down).bit_cast();

        // `trunc_(y)*ctx->stride`: the `int` stride converts to `uint32_t`.
        #[allow(clippy::cast_sign_loss)] // mirrors the int → U32 conversion
        let stride = ctx.stride as u32;
        trunc_(ys) * stride + trunc_(xs)
    }

    /// `gather<U8>(ptr, ix)`.
    fn gather_u8(ctx: &GatherCtx<'_>, ix: U32) -> U8 {
        let mut lanes = [0u8; MAX_STRIDE];
        for (i, lane) in lanes.iter_mut().enumerate().take(N) {
            *lane = ctx.pixels[ix[i] as usize];
        }
        U8::load(&lanes[..N])
    }

    /// `gather_unaligned<U16>(ptr, ix)`.
    fn gather_u16(ctx: &GatherCtx<'_>, ix: U32) -> U16 {
        let mut lanes = [0u16; MAX_STRIDE];
        for (i, lane) in lanes.iter_mut().enumerate().take(N) {
            let at = 2 * ix[i] as usize;
            *lane = u16::from_ne_bytes([ctx.pixels[at], ctx.pixels[at + 1]]);
        }
        U16::load(&lanes[..N])
    }

    /// `gather_unaligned<U32>(ptr, ix)`.
    fn gather_u32(ctx: &GatherCtx<'_>, ix: U32) -> U32 {
        let mut lanes = [0u32; MAX_STRIDE];
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

    // Port of: src/opts/SkRasterPipeline_opts.h#L6452-L6514 (chrome/m156)
    fn from_8888(rgba: U32) -> (U16, U16, U16, U16) {
        let lo: U16 = (rgba & 65535).cast();
        let hi: U16 = (rgba >> 16).cast();
        (lo & 255, lo >> 8, hi & 255, hi >> 8)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6516-L6526 (chrome/m156)
    fn load_8888_(ptr: &[u8]) -> (U16, U16, U16, U16) {
        from_8888(U32::load_bytes(ptr))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6527-L6570 (chrome/m156)
    fn store_8888_(ptr: &mut [u8], r: U16, g: U16, b: U16, a: U16) {
        let r = min_u16(r, U16::splat(255));
        let g = min_u16(g, U16::splat(255));
        let b = min_u16(b, U16::splat(255));
        let a = min_u16(a, U16::splat(255));

        let lo: U32 = (r | (g << 8)).cast();
        let hi: U32 = (b | (a << 8)).cast();
        let px: U32 = lo | (hi << 16);
        px.store_bytes(ptr);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6589-L6599 (chrome/m156)
    fn from_565(rgb: U16) -> (U16, U16, U16) {
        // Format for 565 buffers: 15|rrrrr gggggg bbbbb|0
        let r = (rgb >> 11) & 31;
        let g = (rgb >> 5) & 63;
        let b = rgb & 31;

        // These bit replications are the same as multiplying by 255/31 or 255/63 to scale to
        // 8-bit.
        ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6603-L6618 (chrome/m156)
    fn store_565_(ptr: &mut [u8], r: U16, g: U16, b: U16) {
        let r = min_u16(r, U16::splat(255));
        let g = min_u16(g, U16::splat(255));
        let b = min_u16(b, U16::splat(255));

        // Round from [0,255] to [0,31] or [0,63], as if x * (31/255.0f) + 0.5f.
        // (Don't feel like you need to find some fundamental truth in these...
        // they were brute-force searched.)
        let r5 = (r * 9 + 36) / 74; //  9/74 ≈ 31/255, plus 36/74, about half.
        let g6 = (g * 21 + 42) / 85; // 21/85 = 63/255 exactly.
        let b5 = (b * 9 + 36) / 74;
        // Pack them back into 15|rrrrr gggggg bbbbb|0.
        let px: U16 = (r5 << 11) | (g6 << 5) | b5;
        px.store_bytes(ptr);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6638-L6650 (chrome/m156)
    fn from_4444(rgba: U16) -> (U16, U16, U16, U16) {
        // Format for 4444 buffers: 15|rrrr gggg bbbb aaaa|0.
        let r = (rgba >> 12) & 15;
        let g = (rgba >> 8) & 15;
        let b = (rgba >> 4) & 15;
        let a = rgba & 15;

        // Scale [0,15] to [0,255].
        ((r << 4) | r, (g << 4) | g, (b << 4) | b, (a << 4) | a)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6654-L6670 (chrome/m156)
    fn store_4444_(ptr: &mut [u8], r: U16, g: U16, b: U16, a: U16) {
        let r = min_u16(r, U16::splat(255));
        let g = min_u16(g, U16::splat(255));
        let b = min_u16(b, U16::splat(255));
        let a = min_u16(a, U16::splat(255));

        // Round from [0,255] to [0,15], producing the same value as (x*(15/255.0f) + 0.5f).
        let r4 = (r + 8) / 17;
        let g4 = (g + 8) / 17;
        let b4 = (b + 8) / 17;
        let a4 = (a + 8) / 17;
        // Pack them back into 15|rrrr gggg bbbb aaaa|0.
        let px: U16 = (r4 << 12) | (g4 << 8) | (b4 << 4) | a4;
        px.store_bytes(ptr);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6687-L6690 (chrome/m156)
    fn from_88(rg: U16) -> (U16, U16) {
        (rg & 0xFF, rg >> 8)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6702-L6715 (chrome/m156)
    fn store_88_(ptr: &mut [u8], r: U16, g: U16) {
        let r = min_u16(r, U16::splat(255));
        let g = min_u16(g, U16::splat(255));

        let px: U16 = r | (g << 8);
        px.store_bytes(ptr);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6740-L6742 (chrome/m156)
    fn load_8(ptr: &[u8]) -> U16 {
        U8::load_bytes(ptr).cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6743-L6746 (chrome/m156)
    fn store_8(ptr: &mut [u8], v: U16) {
        let v = min_u16(v, U16::splat(255));
        let b: U8 = v.cast();
        b.store_bytes(ptr);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6572-L6574 (chrome/m156)
    pub(super) fn load_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b, p.a) = load_8888_(e.ptr_at_xy(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6575-L6577 (chrome/m156)
    pub(super) fn load_8888_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db, p.da) = load_8888_(e.ptr_at_xy(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6578-L6580 (chrome/m156)
    pub(super) fn store_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_8888_(e.ptr_at_xy_mut(ctx, 4), p.r, p.g, p.b, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6581-L6585 (chrome/m156)
    pub(super) fn gather_8888(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, x, y);
        (p.r, p.g, p.b, p.a) = from_8888(gather_u32(ctx, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6620-L6623 (chrome/m156)
    pub(super) fn load_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6624-L6627 (chrome/m156)
    pub(super) fn load_565_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db) = from_565(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.da = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6628-L6630 (chrome/m156)
    pub(super) fn store_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_565_(e.ptr_at_xy_mut(ctx, 2), p.r, p.g, p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6631-L6636 (chrome/m156)
    pub(super) fn gather_565(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, x, y);
        (p.r, p.g, p.b) = from_565(gather_u16(ctx, ix));
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6672-L6674 (chrome/m156)
    pub(super) fn load_4444(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g, p.b, p.a) = from_4444(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6675-L6677 (chrome/m156)
    pub(super) fn load_4444_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db, p.da) = from_4444(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6678-L6680 (chrome/m156)
    pub(super) fn store_4444(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_4444_(e.ptr_at_xy_mut(ctx, 2), p.r, p.g, p.b, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6681-L6685 (chrome/m156)
    pub(super) fn gather_4444(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, x, y);
        (p.r, p.g, p.b, p.a) = from_4444(gather_u16(ctx, ix));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6717-L6721 (chrome/m156)
    pub(super) fn load_rg88(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.r, p.g) = from_88(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.b = U16::splat(0);
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6722-L6726 (chrome/m156)
    pub(super) fn load_rg88_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg) = from_88(U16::load_bytes(e.ptr_at_xy(ctx, 2)));
        p.db = U16::splat(0);
        p.da = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6727-L6729 (chrome/m156)
    pub(super) fn store_rg88(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_88_(e.ptr_at_xy_mut(ctx, 2), p.r, p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6730-L6736 (chrome/m156)
    pub(super) fn gather_rg88(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, x, y);
        (p.r, p.g) = from_88(gather_u16(ctx, ix));
        p.b = U16::splat(0);
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6748-L6751 (chrome/m156)
    pub(super) fn load_a8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.r = U16::splat(0);
        p.g = U16::splat(0);
        p.b = U16::splat(0);
        p.a = load_8(e.ptr_at_xy(ctx, 1));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6752-L6755 (chrome/m156)
    pub(super) fn load_a8_dst(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.dr = U16::splat(0);
        p.dg = U16::splat(0);
        p.db = U16::splat(0);
        p.da = load_8(e.ptr_at_xy(ctx, 1));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6756-L6758 (chrome/m156)
    pub(super) fn store_a8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_8(e.ptr_at_xy_mut(ctx, 1), p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6759-L6764 (chrome/m156)
    pub(super) fn gather_a8(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let ix = gather_ix(ctx, x, y);
        p.r = U16::splat(0);
        p.g = U16::splat(0);
        p.b = U16::splat(0);
        p.a = gather_u8(ctx, ix).cast();
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6765-L6767 (chrome/m156)
    pub(super) fn store_r8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        store_8(e.ptr_at_xy_mut(ctx, 1), p.r);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6769-L6772 (chrome/m156)
    pub(super) fn alpha_to_gray(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.a;
        p.g = p.a;
        p.b = p.a;
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6773-L6776 (chrome/m156)
    pub(super) fn alpha_to_gray_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.da;
        p.dg = p.da;
        p.db = p.da;
        p.da = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6777-L6780 (chrome/m156)
    pub(super) fn alpha_to_red(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.a;
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6781-L6784 (chrome/m156)
    pub(super) fn alpha_to_red_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.da;
        p.da = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7229-L7238 (chrome/m156)
    pub(super) fn srcover_rgba_8888(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        (p.dr, p.dg, p.db, p.da) = load_8888_(e.ptr_at_xy(ctx, 4));
        let inv_a = U16::splat(255) - p.a; // inv(a)
        p.r += div255(p.dr * inv_a);
        p.g += div255(p.dg * inv_a);
        p.b += div255(p.db * inv_a);
        p.a += div255(p.da * inv_a);
        store_8888_(e.ptr_at_xy_mut(ctx, 4), p.r, p.g, p.b, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7274-L7280 (chrome/m156)
    pub(super) fn debug_r_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px: U32 = min_u16(p.r, U16::splat(255)).cast();
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7281-L7287 (chrome/m156)
    pub(super) fn debug_g_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px: U32 = min_u16(p.g, U16::splat(255)).cast::<u32>() << 8;
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7288-L7294 (chrome/m156)
    pub(super) fn debug_b_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mut px: U32 = min_u16(p.b, U16::splat(255)).cast::<u32>() << 16;
        px |= 0xFF00_0000; // make opaque
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7295-L7301 (chrome/m156)
    pub(super) fn debug_a_255(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let px: U32 = min_u16(p.a, U16::splat(255)).cast();
        // Render alpha as greyscale
        let out: U32 = px | px << 8 | px << 16 | px << 24;
        out.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    /// lowp `abs_(F)`: clears the sign bit.
    fn abs_(x: F) -> F {
        let bits: I32 = x.bit_cast();
        (bits & 0x7fff_ffff).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7303-L7313 (chrome/m156)
    fn lowp_fixed_point_f(ctx: MemoryCtx, e: &mut Params<'_, '_>, lane: F) {
        // `cast<U32>(F)`: a C float -> unsigned conversion (the inputs are non-negative).
        let r2: U32 = (abs_(lane) / 256.0).cast::<u32>() & 0xFF;
        let g2: U32 = abs_(lane).cast::<u32>() & 0xFF;
        let b2: U32 = (abs_(lane) * 256.0).cast::<u32>() & 0xFF;
        let a2: U32 = min_f(max_f(lane * -256.0 * 256.0, F::splat(0.0)), F::splat(255.0)).cast();
        let px: U32 = r2 | (g2 << 8) | (b2 << 16) | (a2 << 24);
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7315-L7317 (chrome/m156)
    pub(super) fn debug_x(ctx: MemoryCtx, x: &mut F, _y: &mut F, e: &mut Params<'_, '_>) {
        lowp_fixed_point_f(ctx, e, *x);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7318-L7320 (chrome/m156)
    pub(super) fn debug_y(ctx: MemoryCtx, _x: &mut F, y: &mut F, e: &mut Params<'_, '_>) {
        lowp_fixed_point_f(ctx, e, *y);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7322-L7331 (chrome/m156)
    // Note that there won't be negative numbers nor fractional points.
    fn lowp_fixed_point(ctx: MemoryCtx, e: &mut Params<'_, '_>, lane: U16) {
        // Flip the byte ordering so it aligns with the fixed point used above
        let px: U32 = (((lane & 0xFF00) >> 8) | ((lane & 0x00FF) << 8)).cast();
        px.store_bytes(e.ptr_at_xy_mut(ctx, 4));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7333-L7335 (chrome/m156)
    pub(super) fn debug_r(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        lowp_fixed_point(ctx, e, p.r);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7336-L7338 (chrome/m156)
    pub(super) fn debug_g(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        lowp_fixed_point(ctx, e, p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7339-L7341 (chrome/m156)
    pub(super) fn debug_b(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        lowp_fixed_point(ctx, e, p.b);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7342-L7344 (chrome/m156)
    pub(super) fn debug_a(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        lowp_fixed_point(ctx, e, p.a);
    }
}
