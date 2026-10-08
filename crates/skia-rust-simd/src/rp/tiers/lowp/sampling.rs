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
    /// `clamp_01_`: `min(max(0, v), 1)`.
    fn clamp_01_(v: F) -> F {
        min_f(max_f(F::splat(0.0), v), F::splat(1.0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6943-L6954 (chrome/m156)
    /// `round_F_to_U16`: the colors in `[0, 1]` (alpha is assumed to be already) as 8-bit values
    /// in 16-bit lanes.
    fn round_f_to_u16(r: F, g: F, b: F, a: F) -> (U16, U16, U16, U16) {
        // The `uint16_t` conversion of a float is the tier's truncating float -> int conversion
        // followed by a truncation to 16 bits (see `from_float` in lowp `color`).
        let round_color = |x: F| -> U16 { to_i32(x * 255.0 + 0.5).cast() };
        (
            round_color(clamp_01_(r)),
            round_color(clamp_01_(g)),
            round_color(clamp_01_(b)),
            round_color(a), // we assume alpha is already in [0,1].
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6956-L7046 (chrome/m156)
    /// `gradient_lookup`: `t * factor[idx] + bias[idx]` as 8-bit colors. (Skia's AVX2 path
    /// permutes eight floats loaded from the tables instead of gathering; `idx < stopCount`
    /// always, so the results are the gather's.)
    fn gradient_lookup(c: &GradientCtx, idx: U32, t: F) -> (U16, U16, U16, U16) {
        let gather = |table: &[f32]| -> F { idx.map(|i| table[i as usize]) };
        let (fr, fg, fb, fa) = (
            gather(&c.factors[0]),
            gather(&c.factors[1]),
            gather(&c.factors[2]),
            gather(&c.factors[3]),
        );
        let (br, bg, bb, ba) = (
            gather(&c.biases[0]),
            gather(&c.biases[1]),
            gather(&c.biases[2]),
            gather(&c.biases[3]),
        );
        round_f_to_u16(mad(t, fr, br), mad(t, fg, bg), mad(t, fb, bb), mad(t, fa, ba))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7060-L7065 (chrome/m156)
    pub(super) fn evenly_spaced_gradient(ctx: &GradientCtx, x: F, _y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = x;
        // `static_cast<float>(c->stopCount-1)`
        #[allow(clippy::cast_precision_loss)]
        let idx = trunc_(t * ((ctx.stop_count - 1) as f32));
        (p.r, p.g, p.b, p.a) = gradient_lookup(ctx, idx, t);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7048-L7058 (chrome/m156)
    pub(super) fn gradient(ctx: &GradientCtx, x: F, _y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = x;
        let mut idx = U32::splat(0);

        // N.B. The loop starts at 1 because idx 0 is the color to use before the first stop.
        for i in 1..ctx.stop_count {
            let c: I32 = t.ge_mask(ctx.ts[i]).bit_cast();
            idx += if_then_else_u32(c, U32::splat(1), U32::splat(0));
        }

        (p.r, p.g, p.b, p.a) = gradient_lookup(ctx, idx, t);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7066-L7074 (chrome/m156)
    pub(super) fn evenly_spaced_2_stop_gradient(ctx: &EvenlySpaced2StopGradientCtx, x: F, _y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let t = x;
        let s = F::splat;
        (p.r, p.g, p.b, p.a) = round_f_to_u16(
            mad(t, s(ctx.factor[0]), s(ctx.bias[0])),
            mad(t, s(ctx.factor[1]), s(ctx.bias[1])),
            mad(t, s(ctx.factor[2]), s(ctx.bias[2])),
            mad(t, s(ctx.factor[3]), s(ctx.bias[3])),
        );
    }

    /// `abs_(F)`: clears the sign bit.
    fn abs_(x: F) -> F {
        let bits: I32 = x.bit_cast();
        (bits & 0x7fff_ffff).bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7200-L7222 (chrome/m156)
    pub(super) fn xy_to_unit_angle(x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        let (xv, yv) = (*x, *y);
        let xabs = abs_(xv);
        let yabs = abs_(yv);

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
        let lt: I32 = xv.lt_mask(0.0).bit_cast();
        phi = if_then_else_f(lt, F::splat(1.0f32 / 2.0f32) - phi, phi);
        let lt: I32 = yv.lt_mask(0.0).bit_cast();
        phi = if_then_else_f(lt, F::splat(1.0f32) - phi, phi);
        let nan: I32 = phi.ne_mask(phi).bit_cast();
        phi = if_then_else_f(nan, F::splat(0.0), phi); // Check for NaN.
        *x = phi;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7223-L7225 (chrome/m156)
    pub(super) fn xy_to_radius(x: &mut F, y: &mut F, _e: &mut Params<'_, '_>) {
        *x = sqrt_(*x * *x + *y * *y);
    }
}
