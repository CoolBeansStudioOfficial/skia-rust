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

    /// `compute_perlin_vector`: the dot product of `(x, y)` with the gradient stored for each
    /// lane's lattice index `ix` (a `uint32` of two `uint16`s: `sampleLo` is the first, as on
    /// little-endian targets, which every Skia tier we port is).
    // Port of: src/opts/SkRasterPipeline_opts.h#L4048-L4065 (chrome/m156)
    fn compute_perlin_vector(noise: &[u16], channel: usize, ix: U32, x: F, y: F) -> F {
        let lo: F = ix.map(|i| f32::from(noise[2 * (channel + i as usize)]));
        let hi: F = ix.map(|i| f32::from(noise[2 * (channel + i as usize) + 1]));

        // Convert 32-bit sample value into two floats in the [-1..1] range.
        let scale = F::splat(2.0f32 / 65535.0f32);
        let vec_x = mad(lo, scale, F::splat(-1.0f32));
        let vec_y = mad(hi, scale, F::splat(-1.0f32));

        // Return the dot of the sample and the passed-in vector.
        mad(vec_x, x, vec_y * y)
    }

    /// The stitching wrap of `floorVal`/`ceilVal`: `stitch` when `v >= stitch`, else `+0.0`
    /// (`sk_bit_cast<F>(cond_to_mask(v >= stitch) & sk_bit_cast<I32>(stitch))`).
    fn stitch_wrap(v: F, stitch: F) -> F {
        let mask: I32 = cond_to_mask(v.ge_mask(stitch).bit_cast());
        let stitch_bits: I32 = stitch.bit_cast();
        let bits: I32 = mask & stitch_bits;
        bits.bit_cast()
    }

    /// `clamp_01_`: `min(max(0.0f, v), 1.0f)` (the `color` module's helper is private to it).
    fn clamp_unit(v: F) -> F {
        min_f(max_f(F::splat(0.0), v), F::splat(1.0))
    }

    /// `(U32)(iround(v)) & 0xFF`: a lattice index.
    fn lattice_index(v: F) -> U32 {
        let i: U32 = iround(v).bit_cast();
        i & 0xFF
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4067-L4158 (chrome/m156)
    pub(super) fn perlin_noise(ctx: &PerlinNoiseCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut noise_vec_x = (p.r + F::splat(0.5)) * F::splat(ctx.base_frequency_x);
        let mut noise_vec_y = (p.g + F::splat(0.5)) * F::splat(ctx.base_frequency_y);
        let zero = F::splat(0.0);
        // The accumulated `r, g, b, a`.
        let mut acc = [zero; 4];
        let mut stitch_data_x = F::splat(ctx.stitch_data_in_x);
        let mut stitch_data_y = F::splat(ctx.stitch_data_in_y);
        let mut ratio = F::splat(1.0);
        let noise = &ctx.noise_data[..];
        let lattice = &ctx.lattice_selector;

        for _octave in 0..ctx.num_octaves {
            // Calculate noise coordinates. (Roughly $noise_helper in Graphite)
            let mut floor_val_x = floor_(noise_vec_x);
            let mut floor_val_y = floor_(noise_vec_y);
            let mut ceil_val_x = floor_val_x + F::splat(1.0);
            let mut ceil_val_y = floor_val_y + F::splat(1.0);
            let fract_val_x = noise_vec_x - floor_val_x;
            let fract_val_y = noise_vec_y - floor_val_y;

            if ctx.stitching {
                // If we are stitching, wrap the coordinates to the stitch position.
                floor_val_x -= stitch_wrap(floor_val_x, stitch_data_x);
                floor_val_y -= stitch_wrap(floor_val_y, stitch_data_y);
                ceil_val_x -= stitch_wrap(ceil_val_x, stitch_data_x);
                ceil_val_y -= stitch_wrap(ceil_val_y, stitch_data_y);
            }

            let lattice_idx_x = lattice_gather(lattice, lattice_index(floor_val_x));
            let lattice_idx_y = lattice_gather(lattice, lattice_index(ceil_val_x));

            let b00 = lattice_index(lattice_idx_x + floor_val_y);
            let b10 = lattice_index(lattice_idx_y + floor_val_y);
            let b01 = lattice_index(lattice_idx_x + ceil_val_y);
            let b11 = lattice_index(lattice_idx_y + ceil_val_y);

            // Calculate noise colors. (Roughly $noise_function in Graphite)
            // Apply Hermite interpolation to the fractional value.
            let smooth_x = fract_val_x * fract_val_x * (F::splat(3.0) - F::splat(2.0) * fract_val_x);
            let smooth_y = fract_val_y * fract_val_y * (F::splat(3.0) - F::splat(2.0) * fract_val_y);

            let mut color = [zero; 4];
            for (channel, c) in color.iter_mut().enumerate() {
                // Each channel is 256 `uint32`s (512 `uint16`s) further on.
                let base = channel * 256;
                let left = compute_perlin_vector(noise, base, b00, fract_val_x, fract_val_y);
                let right = compute_perlin_vector(
                    noise,
                    base,
                    b10,
                    fract_val_x - F::splat(1.0),
                    fract_val_y,
                );
                let upper = lerp(left, right, smooth_x);

                let left = compute_perlin_vector(
                    noise,
                    base,
                    b01,
                    fract_val_x,
                    fract_val_y - F::splat(1.0),
                );
                let right = compute_perlin_vector(
                    noise,
                    base,
                    b11,
                    fract_val_x - F::splat(1.0),
                    fract_val_y - F::splat(1.0),
                );
                let lower = lerp(left, right, smooth_x);

                *c = lerp(upper, lower, smooth_y);
            }

            if ctx.noise_type != PerlinNoiseShaderType::FractalNoise {
                // For kTurbulence the result is: abs(noise[-1,1])
                for c in &mut color {
                    *c = abs_f(*c);
                }
            }

            for (acc_c, color_c) in acc.iter_mut().zip(color) {
                *acc_c = mad(color_c, ratio, *acc_c);
            }

            // Scale inputs for the next round.
            noise_vec_x *= F::splat(2.0);
            noise_vec_y *= F::splat(2.0);
            stitch_data_x *= F::splat(2.0);
            stitch_data_y *= F::splat(2.0);
            ratio *= F::splat(0.5);
        }

        if ctx.noise_type == PerlinNoiseShaderType::FractalNoise {
            // For kFractalNoise the result is: noise[-1,1] * 0.5 + 0.5
            for acc_c in &mut acc {
                *acc_c = mad(*acc_c, F::splat(0.5), F::splat(0.5));
            }
        }

        // Premultiply by the unclamped alpha, then clamp alpha.
        let alpha = acc[3];
        p.r = clamp_unit(acc[0]) * alpha;
        p.g = clamp_unit(acc[1]) * alpha;
        p.b = clamp_unit(acc[2]) * alpha;
        p.a = clamp_unit(alpha);
    }

    /// `expand(gather(ctx->latticeSelector, ix))` converted to float.
    fn lattice_gather(table: &[u8; 256], ix: U32) -> F {
        ix.map(|i| f32::from(table[i as usize]))
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
