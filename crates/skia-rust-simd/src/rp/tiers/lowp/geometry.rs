// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Geometry and tiling: matrices, `repeat`/`mirror`/`clamp`/`decal`.
//!
//! Owner: task B5 (`docs/design/raster-pipeline.md` §5). The lowp decal stages store `N` 16-bit
//! mask lanes into the context's mask words (two lanes per word, lane `2i` in the low half),
//! which lowp `check_decal_mask` reads back in the same chunk.

// Skia's stages name their registers `r, g, b, a` / `x, y`.
#![allow(clippy::many_single_char_names)]

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L6117-L6120 (chrome/m156)
    pub(super) fn matrix_translate(ctx: [f32; 2], x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        *x += ctx[0];
        *y += ctx[1];
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6121-L6124 (chrome/m156)
    pub(super) fn matrix_scale_translate(ctx: &[f32; 4], x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        *x = mad(*x, F::splat(ctx[0]), F::splat(ctx[2]));
        *y = mad(*y, F::splat(ctx[1]), F::splat(ctx[3]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6125-L6130 (chrome/m156)
    pub(super) fn matrix_2x3(m: &[f32; 6], x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        let (xx, yy) = (*x, *y);
        *x = mad(xx, s(m[0]), mad(yy, s(m[1]), s(m[2])));
        *y = mad(xx, s(m[3]), mad(yy, s(m[4]), s(m[5])));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6131-L6139 (chrome/m156)
    /// N.B. Unlike the other `matrix_*` stages, this matrix is row-major.
    pub(super) fn matrix_perspective(m: &[f32; 9], x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        let (xx, yy) = (*x, *y);
        let nx = mad(xx, s(m[0]), mad(yy, s(m[1]), s(m[2])));
        let ny = mad(xx, s(m[3]), mad(yy, s(m[4]), s(m[5])));
        let z = mad(xx, s(m[6]), mad(yy, s(m[7]), s(m[8])));
        *x = nx * rcp_precise(z);
        *y = ny * rcp_precise(z);
    }

    /// `sk_unaligned_store(ctx->mask, cond_to_mask_16(cond))`: `cast<I16>(cond)`, stored as `N`
    /// 16-bit lanes (two per mask word, lane `2i` in the low half).
    fn store_decal_mask(ctx: &DecalTileCtx, cond: I32) {
        let mut halves = [0u16; MAX_STRIDE];
        let cond16: I16 = cond.cast();
        let bits: U16 = cond16.bit_cast();
        bits.store(&mut halves[..N]);
        let mut mask = ctx.mask.get();
        for (w, h) in mask.iter_mut().zip(halves.as_chunks::<2>().0.iter()) {
            *w = u32::from(h[0]) | (u32::from(h[1]) << 16);
        }
        ctx.mask.set(mask);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6913-L6925 (chrome/m156)
    pub(super) fn decal_x(ctx: &DecalTileCtx, x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        let w = ctx.limit_x;
        store_decal_mask(ctx, (x.ge_mask(0.0) & x.lt_mask(w)).bit_cast());
    }

    pub(super) fn decal_y(ctx: &DecalTileCtx, _x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let h = ctx.limit_y;
        store_decal_mask(ctx, (y.ge_mask(0.0) & y.lt_mask(h)).bit_cast());
    }

    pub(super) fn decal_x_and_y(ctx: &DecalTileCtx, x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let (w, h) = (ctx.limit_x, ctx.limit_y);
        let cond = x.ge_mask(0.0) & x.lt_mask(w) & y.ge_mask(0.0) & y.lt_mask(h);
        store_decal_mask(ctx, cond.bit_cast());
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6951-L6960 (chrome/m156)
    pub(super) fn check_decal_mask(ctx: &DecalTileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let words = ctx.mask.get();
        let mut halves = [0u16; MAX_STRIDE];
        for (h, w) in halves.as_chunks_mut::<2>().0.iter_mut().zip(words) {
            h[0] = (w & 0xFFFF) as u16;
            h[1] = (w >> 16) as u16;
        }
        let mask = U16::load(&halves[..N]);
        p.r &= mask;
        p.g &= mask;
        p.b &= mask;
        p.a &= mask;
    }

    /// `clamp_01_`: `min(max(0, v), 1)`.
    fn clamp_01_(v: F) -> F {
        min_f(max_f(F::splat(0.0), v), F::splat(1.0))
    }

    /// `fast_clamp_01_`: `min_intr(max_intr(0, v), 1)`.
    fn fast_clamp_01_(v: F) -> F {
        min_intr_f(max_intr_f(F::splat(0.0), v), F::splat(1.0))
    }

    /// `abs_(F)`: clears the sign bit.
    fn abs_(x: F) -> F {
        let bits: I32 = x.bit_cast();
        (bits & 0x7fff_ffff).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6897-L6906 (chrome/m156)
    pub(super) fn clamp_x_1(x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        *x = fast_clamp_01_(*x);
    }

    pub(super) fn repeat_x_1(x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        *x = fast_clamp_01_(*x - floor_(*x));
    }

    pub(super) fn mirror_x_1(x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        let two = |v: F| v + v;
        let v = *x;
        *x = clamp_01_(abs_((v - 1.0) - two(floor_((v - 1.0) * 0.5)) - 1.0));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6940-L6944 (chrome/m156)
    pub(super) fn clamp_x_and_y(ctx: &CoordClampCtx, x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        *x = min_intr_f(s(ctx.max_x), max_intr_f(s(ctx.min_x), *x));
        *y = min_intr_f(s(ctx.max_y), max_intr_f(s(ctx.min_y), *y));
    }
}
