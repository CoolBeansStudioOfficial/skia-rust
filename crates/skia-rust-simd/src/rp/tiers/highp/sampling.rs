// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Sampling, gradients and 2-point conical stages (Phase 3).
//!
//! Owner: task P3 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L3730-L3799 (chrome/m156)
    /// `gradient_lookup`: `(r, g, b, a) = t * factor[idx] + bias[idx]`. (Skia's AVX2 path
    /// permutes eight floats loaded from the tables instead of gathering; `idx < stopCount`
    /// always, so the results are the gather's.)
    fn gradient_lookup(c: &GradientCtx, idx: U32, t: F) -> (F, F, F, F) {
        let gather = |table: &[f32]| -> F { idx.map(|i| table[i as usize]) };
        let (fr, br) = (gather(&c.factors[0]), gather(&c.biases[0]));
        let (fg, bg) = (gather(&c.factors[1]), gather(&c.biases[1]));
        let (fb, bb) = (gather(&c.factors[2]), gather(&c.biases[2]));
        let (fa, ba) = (gather(&c.factors[3]), gather(&c.biases[3]));
        (mad(t, fr, br), mad(t, fg, bg), mad(t, fb, bb), mad(t, fa, ba))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3801-L3806 (chrome/m156)
    pub(super) fn evenly_spaced_gradient(ctx: &GradientCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = p.r;
        // `static_cast<float>(c->stopCount-1)`
        #[allow(clippy::cast_precision_loss)]
        let idx = trunc_(t * ((ctx.stop_count - 1) as f32));
        (p.r, p.g, p.b, p.a) = gradient_lookup(ctx, idx, t);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3807-L3817 (chrome/m156)
    pub(super) fn gradient(ctx: &GradientCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = p.r;
        let mut idx = U32::splat(0);

        // N.B. The loop starts at 1 because idx 0 is the color to use before the first stop.
        for i in 1..ctx.stop_count {
            let c: I32 = t.ge_mask(ctx.ts[i]).bit_cast();
            let one: U32 = if_then_else_i(c, I32::splat(1), I32::splat(0)).bit_cast();
            idx += one;
        }

        (p.r, p.g, p.b, p.a) = gradient_lookup(ctx, idx, t);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3819-L3826 (chrome/m156)
    pub(super) fn evenly_spaced_2_stop_gradient(ctx: &EvenlySpaced2StopGradientCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = p.r;
        let s = F::splat;
        p.r = mad(t, s(ctx.factor[0]), s(ctx.bias[0]));
        p.g = mad(t, s(ctx.factor[1]), s(ctx.bias[1]));
        p.b = mad(t, s(ctx.factor[2]), s(ctx.bias[2]));
        p.a = mad(t, s(ctx.factor[3]), s(ctx.bias[3]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3828-L3852 (chrome/m156)
    pub(super) fn xy_to_unit_angle(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        let xabs = abs_f(x);
        let yabs = abs_f(y);

        let slope = min_f(xabs, yabs) / max_f(xabs, yabs);
        let s = slope * slope;

        // Use a 7th degree polynomial to approximate atan.
        // This was generated using sollya.gforge.inria.fr.
        // A float optimized polynomial was generated using the following command.
        // P1 = fpminimax((1/(2*Pi))*atan(x),[|1,3,5,7|],[|24...|],[2^(-40),1],relative);
        #[allow(clippy::excessive_precision)] // Skia's literals kept verbatim
        let mut phi = slope
            * (F::splat(0.159_121_170_639_991_760_253_906_25_f32)
                + s * (F::splat(-5.185_396_969_318_389_892_578_125e-2_f32)
                    + s * (F::splat(2.476_101_927_459_239_959_716_796_875e-2_f32)
                        + s * (-7.054_738_234_728_574_752_807_617_187_5e-3_f32))));

        let lt: I32 = xabs.lt_mask(yabs).bit_cast();
        phi = if_then_else_f(lt, F::splat(1.0f32 / 4.0f32) - phi, phi);
        let lt: I32 = x.lt_mask(0.0).bit_cast();
        phi = if_then_else_f(lt, F::splat(1.0f32 / 2.0f32) - phi, phi);
        let lt: I32 = y.lt_mask(0.0).bit_cast();
        phi = if_then_else_f(lt, F::splat(1.0f32) - phi, phi);
        let nan: I32 = phi.ne_mask(phi).bit_cast();
        phi = if_then_else_f(nan, F::splat(0.0), phi); // Check for NaN.
        p.r = phi;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3854-L3858 (chrome/m156)
    pub(super) fn xy_to_radius(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let x2 = p.r * p.r;
        let y2 = p.g * p.g;
        p.r = sqrt_(x2 + y2);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3862-L3862 (chrome/m156)
    pub(super) fn negate_x(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = -p.r;
    }

    pub(super) fn perlin_noise(_ctx: &PerlinNoiseCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("perlin_noise", "P3")
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3864-L3867 (chrome/m156)
    pub(super) fn xy_to_2pt_conical_strip(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        p.r = x + sqrt_(F::splat(ctx.p0) - y * y); // ctx->fP0 = r0 * r0
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3869-L3872 (chrome/m156)
    pub(super) fn xy_to_2pt_conical_focal_on_circle(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        p.r = x + y * y / x; // (x^2 + y^2) / x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3874-L3877 (chrome/m156)
    pub(super) fn xy_to_2pt_conical_well_behaved(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        p.r = sqrt_(x * x + y * y) - x * ctx.p0; // ctx->fP0 = 1/r1
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3884-L3887 (chrome/m156)
    pub(super) fn xy_to_2pt_conical_smaller(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        p.r = -sqrt_(x * x - y * y) - x * ctx.p0; // ctx->fP0 = 1/r1
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3879-L3882 (chrome/m156)
    pub(super) fn xy_to_2pt_conical_greater(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (x, y) = (p.r, p.g);
        p.r = sqrt_(x * x - y * y) - x * ctx.p0; // ctx->fP0 = 1/r1
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3889-L3893 (chrome/m156)
    pub(super) fn alter_2pt_conical_compensate_focal(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r += ctx.p1; // ctx->fP1 = f
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3895-L3898 (chrome/m156)
    pub(super) fn alter_2pt_conical_unswap(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = F::splat(1.0) - p.r;
    }

    /// `sk_unaligned_store(&c->fMask, cond_to_mask(cond))`.
    fn store_conical_mask(ctx: &Conical2PtCtx, cond: I32) {
        let mut mask = ctx.mask.get();
        let bits: U32 = cond_to_mask(cond).bit_cast();
        bits.store(&mut mask[..N]);
        ctx.mask.set(mask);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3900-L3905 (chrome/m156)
    pub(super) fn mask_2pt_conical_nan(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = p.r;
        let is_degenerate: I32 = t.ne_mask(t).bit_cast(); // NaN
        p.r = if_then_else_f(is_degenerate, F::splat(0.0), t);
        store_conical_mask(ctx, is_degenerate.eq_mask(0).bit_cast()); // !is_degenerate
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3907-L3912 (chrome/m156)
    pub(super) fn mask_2pt_conical_degenerates(ctx: &Conical2PtCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = p.r;
        let le: I32 = t.le_mask(0.0).bit_cast();
        let nan: I32 = t.ne_mask(t).bit_cast();
        let is_degenerate = le | nan;
        p.r = if_then_else_f(is_degenerate, F::splat(0.0), t);
        store_conical_mask(ctx, is_degenerate.eq_mask(0).bit_cast()); // !is_degenerate
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3914-L3920 (chrome/m156)
    pub(super) fn apply_vector_mask(ctx: &Cell<[u32; MAX_STRIDE_HIGHP]>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mask = U32::load(&ctx.get()[..N]);
        let and = |v: F| -> F {
            let bits: U32 = v.bit_cast();
            (bits & mask).bit_cast()
        };
        p.r = and(p.r);
        p.g = and(p.g);
        p.b = and(p.b);
        p.a = and(p.a);
    }
}
