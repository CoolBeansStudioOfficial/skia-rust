// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Image sampling (lowp): the fused `bilerp_clamp_8888`, used by `SkImageShader::appendStages`
//! for clamp-x, clamp-y bilinear sampling of 8888 images.
//!
//! `rp-diff` cases for it need an oracle run (not available); it is covered by the image shader
//! GMs against the published goldens instead.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L6323-L6335 (chrome/m156)
    /// `ix_and_ptr` for integer coordinates: the vector of pixel indices for the gather at
    /// `(x, y)`, clamped to the image's bounds.
    fn gather_ix_i32(ctx: &GatherCtx<'_>, x: I32, y: I32) -> U32 {
        // This flag doesn't make sense when the coords are integers.
        debug_assert!(!ctx.round_down_at_integer);
        // Exclusive -> inclusive.
        #[allow(clippy::cast_possible_truncation)] // the context holds an integer width/height
        let w = I32::splat(ctx.width as i32 - 1);
        #[allow(clippy::cast_possible_truncation)]
        let h = I32::splat(ctx.height as i32 - 1);

        let ax: U32 = min_intr_i(max_intr_i(I32::splat(0), x), w).cast();
        let ay: U32 = min_intr_i(max_intr_i(I32::splat(0), y), h).cast();

        // `ay * ctx->stride`: the `int` stride converts to `uint32_t`.
        #[allow(clippy::cast_sign_loss)] // mirrors the int → U32 conversion
        let stride = ctx.stride as u32;
        ay * stride + ax
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6066-L6081 (chrome/m156)
    /// This sum is to support lerp where the result will always be a positive number. In
    /// general, a sum like this would require an additional bit, but because we know the range
    /// of the result we know that the extra bit will always be zero.
    fn constrained_add(a: I16, b: U16) -> U16 {
        let a: U16 = a.bit_cast();
        b + a
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7076-L7198 (chrome/m156)
    pub(super) fn bilerp_clamp_8888(ctx: &GatherCtx<'_>, x: F, y: F, p: &mut Regs, _e: &mut Params<'_, '_>) {
        // Quantize sample point and transform into lerp coordinates converting them to 16.16
        // fixed point number.
        let qx: I32 = to_i32(floor_(x * 65536.0 + 0.5)) - 32768;
        let qy: I32 = to_i32(floor_(y * 65536.0 + 0.5)) - 32768;

        // Calculate screen coordinates sx & sy by flooring qx and qy.
        let sx: I32 = qx >> 16;
        let sy: I32 = qy >> 16;

        // We are going to perform a change of parameters for qx on [0, 1) to tx on [-1, 1).
        // This will put tx in Q15 format for use with q_mult.
        // Calculate tx and ty on the interval of [-1, 1). Give {qx} and {qy} are on the interval
        // [0, 1), where {v} is fract(v), we can transform to tx in the following manner ty
        // follows the same math:
        //     tx = 2 * {qx} - 1, so
        //     {qx} = (tx + 1) / 2.
        // Calculate {qx} - 1 and {qy} - 1 where the {} operation is handled by the cast, and
        // the - 1 is handled by the ^ 0x8000, dividing by 2 is deferred and handled in lerpX
        // and lerpY in order to use the full 16-bit resolution.
        let tx: I16 = (qx ^ 0x8000).cast();
        let ty: I16 = (qy ^ 0x8000).cast();

        // Substituting the {qx} by the equation for tx from above into the lerp equation where
        // v is the lerped value:
        //         v = {qx}*(R - L) + L,
        //         v = 1/2*(tx + 1)*(R - L) + L
        //     2 * v = (tx + 1)*(R - L) + 2*L
        //           = tx*R - tx*L + R - L + 2*L
        //           = tx*(R - L) + (R + L).
        // Since R and L are on [0, 255] we need them on the interval [0, 1/2] to get them into
        // form for Q15_mult. If L and R where in 16.16 format, this would be done by dividing by
        // 2^9. In code, we can multiply by 2^7 to get the value directly.
        //            2 * v = tx*(R - L) + (R + L)
        //     2^-9 * 2 * v = tx*(R - L)*2^-9 + (R + L)*2^-9
        //         2^-8 * v = 2^-9 * (tx*(R - L) + (R + L))
        //                v = 1/2 * (tx*(R - L) + (R + L))
        let lerp_x = |left: U16, right: U16| -> U16 {
            let diff: I16 = (right - left).bit_cast();
            let width: I16 = diff << 7;
            let middle: U16 = (right + left) << 7;
            // The constrained_add is the most subtle part of lerp. The first term is on the
            // interval [-1, 1), and the second term is on the interval is on the interval [0, 1)
            // because both terms are too high by a factor of 2 which will be handled below.
            // (Both R and L are on [0, 1/2), but the sum R + L is on the interval [0, 1).)
            // Generally, the sum below should overflow, but because we know that sum produces an
            // output on the interval [0, 1) we know that the extra bit that would be needed will
            // always be 0. So we need to be careful to treat this sum as an unsigned positive
            // number in the divide by 2 below. Add +1 for rounding.
            let v2: U16 = constrained_add(scaled_mult(tx, width), middle) + 1;
            // Divide by 2 to calculate v and at the same time bring the intermediate value onto
            // the interval [0, 1/2] to set up for the lerpY.
            v2 >> 1
        };

        let ix = gather_ix_i32(ctx, sx, sy);
        let (left_r, left_g, left_b, left_a) = from_8888(gather_u32(ctx, ix));

        let ix = gather_ix_i32(ctx, sx + 1, sy);
        let (right_r, right_g, right_b, right_a) = from_8888(gather_u32(ctx, ix));

        let top_r = lerp_x(left_r, right_r);
        let top_g = lerp_x(left_g, right_g);
        let top_b = lerp_x(left_b, right_b);
        let top_a = lerp_x(left_a, right_a);

        let ix = gather_ix_i32(ctx, sx, sy + 1);
        let (left_r, left_g, left_b, left_a) = from_8888(gather_u32(ctx, ix));

        let ix = gather_ix_i32(ctx, sx + 1, sy + 1);
        let (right_r, right_g, right_b, right_a) = from_8888(gather_u32(ctx, ix));

        let bottom_r = lerp_x(left_r, right_r);
        let bottom_g = lerp_x(left_g, right_g);
        let bottom_b = lerp_x(left_b, right_b);
        let bottom_a = lerp_x(left_a, right_a);

        // lerpY plays the same mathematical tricks as lerpX, but the final divide is by 256
        // resulting in a value on [0, 255].
        let lerp_y = |top: U16, bottom: U16| -> U16 {
            let (bottom_i, top_i): (I16, I16) = (bottom.bit_cast(), top.bit_cast());
            let width: I16 = bottom_i - top_i;
            let middle: U16 = bottom + top;
            // Add + 0x80 for rounding.
            let blend: U16 = constrained_add(scaled_mult(ty, width), middle) + 0x80;

            blend >> 8
        };

        p.r = lerp_y(top_r, bottom_r);
        p.g = lerp_y(top_g, bottom_g);
        p.b = lerp_y(top_b, bottom_b);
        p.a = lerp_y(top_a, bottom_a);
    }
}
