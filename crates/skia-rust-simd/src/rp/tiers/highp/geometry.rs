// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Geometry and tiling: matrices, `repeat`/`mirror`/`clamp`/`decal`.
//!
//! Owner: task B5 (`docs/design/raster-pipeline.md` §5). The decal stages write their mask
//! into the context (`DecalTileCtx::mask`), which `check_decal_mask` reads in the same chunk.

// Skia's stages name their registers `r, g, b, a` / `x, y`.
#![allow(clippy::many_single_char_names)]

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L6913-L6916 (chrome/m156)
    pub(super) fn matrix_translate(ctx: [f32; 2], p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r += ctx[0];
        p.g += ctx[1];
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6917-L6920 (chrome/m156)
    pub(super) fn matrix_scale_translate(ctx: &[f32; 4], p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = mad(p.r, F::splat(ctx[0]), F::splat(ctx[2]));
        p.g = mad(p.g, F::splat(ctx[1]), F::splat(ctx[3]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6921-L6926 (chrome/m156)
    pub(super) fn matrix_2x3(m: &[f32; 6], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        let (r, g) = (p.r, p.g);
        p.r = mad(r, s(m[0]), mad(g, s(m[1]), s(m[2])));
        p.g = mad(r, s(m[3]), mad(g, s(m[4]), s(m[5])));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6954-L6960 (chrome/m156)
    /// N.B. Unlike the other `matrix_*` stages, this matrix is row-major.
    pub(super) fn matrix_perspective(m: &[f32; 9], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        let (r, g) = (p.r, p.g);
        let rr = mad(r, s(m[0]), mad(g, s(m[1]), s(m[2])));
        let gg = mad(r, s(m[3]), mad(g, s(m[4]), s(m[5])));
        let z = mad(r, s(m[6]), mad(g, s(m[7]), s(m[8])));
        p.r = rr * rcp_precise(z);
        p.g = gg * rcp_precise(z);
    }

    /// `((0 < v) & (v < limit)) | (v == edge)` as an `I32` condition.
    fn decal_cond(v: F, limit: f32, edge: f32) -> I32 {
        let (gt, lt, eq): (I32, I32, I32) = (
            v.gt_mask(0.0).bit_cast(),
            v.lt_mask(limit).bit_cast(),
            v.eq_mask(edge).bit_cast(),
        );
        (gt & lt) | eq
    }

    /// `sk_unaligned_store(ctx->mask, cond_to_mask(cond))`.
    fn store_decal_mask(ctx: &DecalTileCtx, cond: I32) {
        let mut mask = ctx.mask.get();
        let bits: U32 = cond_to_mask(cond).bit_cast();
        bits.store(&mut mask[..N]);
        ctx.mask.set(mask);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3611-L3621 (chrome/m156)
    pub(super) fn decal_x(ctx: &DecalTileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        store_decal_mask(ctx, decal_cond(p.r, ctx.limit_x, ctx.inclusive_edge_x));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3622-L3627 (chrome/m156)
    pub(super) fn decal_y(ctx: &DecalTileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        store_decal_mask(ctx, decal_cond(p.g, ctx.limit_y, ctx.inclusive_edge_y));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3628-L3638 (chrome/m156)
    pub(super) fn decal_x_and_y(ctx: &DecalTileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let cond = decal_cond(p.r, ctx.limit_x, ctx.inclusive_edge_x)
            & decal_cond(p.g, ctx.limit_y, ctx.inclusive_edge_y);
        store_decal_mask(ctx, cond);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3639-L3646 (chrome/m156)
    pub(super) fn check_decal_mask(ctx: &DecalTileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mask = U32::load(&ctx.mask.get()[..N]);
        let and = |v: F| -> F {
            let bits: U32 = v.bit_cast();
            (bits & mask).bit_cast()
        };
        p.r = and(p.r);
        p.g = and(p.g);
        p.b = and(p.b);
        p.a = and(p.a);
    }

    /// `clamp_01_`: `min(max(0, v), 1)`.
    fn clamp_01_(v: F) -> F {
        min_f(max_f(F::splat(0.0), v), F::splat(1.0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3603-L3605 (chrome/m156)
    pub(super) fn clamp_x_1(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = clamp_01_(p.r);
    }

    pub(super) fn mirror_x_1(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let r = p.r;
        let two = |x: F| x + x;
        p.r = clamp_01_(abs_f((r - 1.0) - two(floor_((r - 1.0) * 0.5)) - 1.0));
    }

    pub(super) fn repeat_x_1(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = clamp_01_(p.r - floor_(p.r));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3607-L3610 (chrome/m156)
    pub(super) fn clamp_x_and_y(ctx: &CoordClampCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let s = F::splat;
        p.r = min_f(s(ctx.max_x), max_f(s(ctx.min_x), p.r));
        p.g = min_f(s(ctx.max_y), max_f(s(ctx.min_y), p.g));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3565-L3567 (chrome/m156)
    fn exclusive_repeat(v: F, ctx: &TileCtx) -> F {
        v - floor_(v * ctx.inv_scale) * ctx.scale
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3568-L3587 (chrome/m156)
    fn exclusive_mirror(v: F, ctx: &TileCtx) -> F {
        let limit = ctx.scale;
        let inv_limit = ctx.inv_scale;

        // This is "repeat" over the range 0..2*limit
        let u = v - floor_(v * inv_limit * 0.5) * 2.0 * limit;
        // s will be 0 when moving forward (e.g. [0, limit)) and 1 when moving backward (e.g.
        // [limit, 2*limit)).
        let s = floor_(u * inv_limit);
        // This is the mirror result.
        let m = u - s * 2.0 * (u - limit);
        // Apply a bias to m if moving backwards so that we snap consistently at exact integer
        // coords in the logical infinite image. This is tested by mirror_tile GM. Note that all
        // values that have a non-zero bias applied are > 0.
        let bias_in_ulps = trunc_(s);
        // `ctx->mirrorBiasDir*biasInUlps`: the int converts to U32 and the math wraps.
        #[allow(clippy::cast_sign_loss)]
        let bias = U32::splat(ctx.mirror_bias_dir as u32) * bias_in_ulps;
        let m_bits: U32 = m.bit_cast();
        (m_bits + bias).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3592-L3602 (chrome/m156)
    pub(super) fn repeat_x(ctx: &TileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = exclusive_repeat(p.r, ctx);
    }

    pub(super) fn repeat_y(ctx: &TileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.g = exclusive_repeat(p.g, ctx);
    }

    pub(super) fn mirror_x(ctx: &TileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = exclusive_mirror(p.r, ctx);
    }

    pub(super) fn mirror_y(ctx: &TileCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.g = exclusive_mirror(p.g, ctx);
    }
}
