// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Image sampling (highp): the fused `bilerp_clamp_8888`/`bicubic_clamp_8888` stages, the
//! separable `bilinear_*`/`bicubic_*` samplers with `accumulate`, and `mipmap_linear_*`.
//!
//! Used by `SkImageShader::appendStages` (task P3, `docs/design/raster-pipeline.md` §5).
//! `rp-diff` cases for these stages need an oracle run (not available); they are covered by the
//! image shader GMs against the published goldens instead.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    /// `sk_unaligned_load<F>(ctx->field)`.
    fn load_lanes(cell: &Cell<[f32; MAX_STRIDE_HIGHP]>) -> F {
        F::load(&cell.get()[..N])
    }

    /// `sk_unaligned_store(ctx->field, v)`.
    fn store_lanes(cell: &Cell<[f32; MAX_STRIDE_HIGHP]>, v: F) {
        let mut lanes = cell.get();
        v.store(&mut lanes[..N]);
        cell.set(lanes);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5342-L5382 (chrome/m156)
    fn bilerp_clamp_large(ctx: &GatherCtx<'_>, p: &mut Regs) {
        // (cx,cy) are the center of our sample.
        let cx = p.r;
        let cy = p.g;

        // All sample points are at the same fractional offset (fx,fy).
        // They're the 4 corners of a logical 1x1 pixel surrounding (x,y) at (0.5,0.5) offsets.
        let fx = fract(cx + 0.5);
        let fy = fract(cy + 0.5);

        // We'll accumulate the color of all four samples into {r,g,b,a} directly.
        p.r = F::splat(0.0);
        p.g = F::splat(0.0);
        p.b = F::splat(0.0);
        p.a = F::splat(0.0);

        // for (float py = -0.5f; py <= +0.5f; py += 1.0f)
        // for (float px = -0.5f; px <= +0.5f; px += 1.0f)
        for py in [-0.5f32, 0.5] {
            for px in [-0.5f32, 0.5] {
                // (x,y) are the coordinates of this sample point.
                let x = cx + px;
                let y = cy + py;

                // ix_and_ptr() will clamp to the image's bounds for us.
                let ix = gather_ix(ctx, x, y);

                let (sr, sg, sb, sa) = from_8888(gather_u32(ctx, ix));

                // In bilinear interpolation, the 4 pixels at +/- 0.5 offsets from the sample
                // pixel center are combined in direct proportion to their area overlapping that
                // logical query pixel. At positive offsets, the x-axis contribution to that
                // rectangle is fx, or (1-fx) at negative x. Same deal for y.
                let sx = if px > 0.0 { fx } else { F::splat(1.0) - fx };
                let sy = if py > 0.0 { fy } else { F::splat(1.0) - fy };
                let area = sx * sy;

                p.r += sr * area;
                p.g += sg * area;
                p.b += sb * area;
                p.a += sa * area;
            }
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5384-L5387 (chrome/m156)
    /// A specialized fused image shader for clamp-x, clamp-y, non-sRGB sampling.
    pub(super) fn bilerp_clamp_8888(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilerp_clamp_large(ctx, p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5389-L5393 (chrome/m156)
    /// The same, for shaders that force high precision.
    pub(super) fn bilerp_clamp_8888_force_highp(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilerp_clamp_large(ctx, p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3990-L3992 (chrome/m156)
    /// The total weight along one axis, given one column of the sampling matrix and a
    /// fractional pixel offset.
    #[allow(clippy::many_single_char_names)] // Skia's names (`t`, `a`..`d`, `s`)
    fn bicubic_wts(t: F, a: f32, b: f32, c: f32, d: f32) -> F {
        let s = F::splat;
        mad(t, mad(t, mad(t, s(d), s(c)), s(b)), s(a))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5395-L5441 (chrome/m156)
    /// A specialized fused image shader for clamp-x, clamp-y, non-sRGB sampling.
    #[allow(clippy::similar_names)] // Skia's `scalex`/`scaley`
    pub(super) fn bicubic_clamp_8888(ctx: &GatherCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        // (cx,cy) are the center of our sample.
        let cx = p.r;
        let cy = p.g;

        // All sample points are at the same fractional offset (fx,fy).
        // They're the 4 corners of a logical 1x1 pixel surrounding (x,y) at (0.5,0.5) offsets.
        let fx = fract(cx + 0.5);
        let fy = fract(cy + 0.5);

        // We'll accumulate the color of all four samples into {r,g,b,a} directly.
        p.r = F::splat(0.0);
        p.g = F::splat(0.0);
        p.b = F::splat(0.0);
        p.a = F::splat(0.0);

        let w = &ctx.weights;
        let scaley = [
            bicubic_wts(fy, w[0], w[4], w[8], w[12]),
            bicubic_wts(fy, w[1], w[5], w[9], w[13]),
            bicubic_wts(fy, w[2], w[6], w[10], w[14]),
            bicubic_wts(fy, w[3], w[7], w[11], w[15]),
        ];
        let scalex = [
            bicubic_wts(fx, w[0], w[4], w[8], w[12]),
            bicubic_wts(fx, w[1], w[5], w[9], w[13]),
            bicubic_wts(fx, w[2], w[6], w[10], w[14]),
            bicubic_wts(fx, w[3], w[7], w[11], w[15]),
        ];

        let mut sample_y = cy - 1.5;
        for sy in scaley {
            let mut sample_x = cx - 1.5;
            for sx in scalex {
                let scale = sx * sy;

                // ix_and_ptr() will clamp to the image's bounds for us.
                let ix = gather_ix(ctx, sample_x, sample_y);

                let (sr, sg, sb, sa) = from_8888(gather_u32(ctx, ix));

                p.r = mad(scale, sr, p.r);
                p.g = mad(scale, sg, p.g);
                p.b = mad(scale, sb, p.b);
                p.a = mad(scale, sa, p.a);

                sample_x += 1.0;
            }
            sample_y += 1.0;
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3922-L3934 (chrome/m156)
    fn save_xy(p: &Regs, c: &SamplerCtx) {
        // Whether bilinear or bicubic, all sample points are at the same fractional offset
        // (fx,fy). They're either the 4 corners of a logical 1x1 pixel or the 16 corners of a
        // 3x3 grid surrounding (x,y) at (0.5,0.5) off-center.
        let fx = fract(p.r + 0.5);
        let fy = fract(p.g + 0.5);

        // Samplers will need to load x and fx, or y and fy.
        store_lanes(&c.x, p.r);
        store_lanes(&c.y, p.g);
        store_lanes(&c.fx, fx);
        store_lanes(&c.fy, fy);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3936-L3945 (chrome/m156)
    pub(super) fn accumulate(c: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        // Bilinear and bicubic filters are both separable, so we produce independent
        // contributions from x and y, multiplying them together here to get each pixel's total
        // scale factor.
        let scale = load_lanes(&c.scalex) * load_lanes(&c.scaley);
        p.dr = mad(scale, p.r, p.dr);
        p.dg = mad(scale, p.g, p.dg);
        p.db = mad(scale, p.b, p.db);
        p.da = mad(scale, p.a, p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3952-L3961 (chrome/m156)
    // In bilinear interpolation, the 4 pixels at +/- 0.5 offsets from the sample pixel center
    // are combined in direct proportion to their area overlapping that logical query pixel.
    // At positive offsets, the x-axis contribution to that rectangle is fx, or (1-fx) at
    // negative x. The y-axis is symmetric.
    fn bilinear_x(k_scale: i32, ctx: &SamplerCtx, x: &mut F) {
        #[allow(clippy::cast_precision_loss)] // kScale is -1 or +1
        let offset = k_scale as f32 * 0.5;
        *x = load_lanes(&ctx.x) + offset;
        let fx = load_lanes(&ctx.fx);

        let scalex = if k_scale == -1 {
            F::splat(1.0) - fx
        } else {
            fx
        };
        store_lanes(&ctx.scalex, scalex);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3962-L3971 (chrome/m156)
    fn bilinear_y(k_scale: i32, ctx: &SamplerCtx, y: &mut F) {
        #[allow(clippy::cast_precision_loss)] // kScale is -1 or +1
        let offset = k_scale as f32 * 0.5;
        *y = load_lanes(&ctx.y) + offset;
        let fy = load_lanes(&ctx.fy);

        let scaley = if k_scale == -1 {
            F::splat(1.0) - fy
        } else {
            fy
        };
        store_lanes(&ctx.scaley, scaley);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3973-L3977 (chrome/m156)
    pub(super) fn bilinear_setup(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        save_xy(p, ctx);
        // Init for accumulate
        p.dr = F::splat(0.0);
        p.dg = F::splat(0.0);
        p.db = F::splat(0.0);
        p.da = F::splat(0.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3979-L3982 (chrome/m156)
    pub(super) fn bilinear_nx(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilinear_x(-1, ctx, &mut p.r);
    }

    pub(super) fn bilinear_px(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilinear_x(1, ctx, &mut p.r);
    }

    pub(super) fn bilinear_ny(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilinear_y(-1, ctx, &mut p.g);
    }

    pub(super) fn bilinear_py(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bilinear_y(1, ctx, &mut p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L3994-L4004 (chrome/m156)
    // In bicubic interpolation, the 16 pixels and +/- 0.5 and +/- 1.5 offsets from the sample
    // pixel center are combined with a non-uniform cubic filter, with higher values near the
    // center.
    fn bicubic_x(k_scale: i32, ctx: &SamplerCtx, x: &mut F) {
        #[allow(clippy::cast_precision_loss)] // kScale is -3, -1, +1 or +3
        let offset = k_scale as f32 * 0.5;
        *x = load_lanes(&ctx.x) + offset;

        let wx = ctx.wx.get();
        let which = match k_scale {
            -3 => 0,
            -1 => 1,
            1 => 2,
            _ => 3,
        };
        store_lanes(&ctx.scalex, F::load(&wx[which][..N]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4005-L4015 (chrome/m156)
    fn bicubic_y(k_scale: i32, ctx: &SamplerCtx, y: &mut F) {
        #[allow(clippy::cast_precision_loss)] // kScale is -3, -1, +1 or +3
        let offset = k_scale as f32 * 0.5;
        *y = load_lanes(&ctx.y) + offset;

        let wy = ctx.wy.get();
        let which = match k_scale {
            -3 => 0,
            -1 => 1,
            1 => 2,
            _ => 3,
        };
        store_lanes(&ctx.scaley, F::load(&wy[which][..N]));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4017-L4036 (chrome/m156)
    pub(super) fn bicubic_setup(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        save_xy(p, ctx);

        let w = &ctx.weights;

        let fx = load_lanes(&ctx.fx);
        let mut wx = ctx.wx.get();
        bicubic_wts(fx, w[0], w[4], w[8], w[12]).store(&mut wx[0][..N]);
        bicubic_wts(fx, w[1], w[5], w[9], w[13]).store(&mut wx[1][..N]);
        bicubic_wts(fx, w[2], w[6], w[10], w[14]).store(&mut wx[2][..N]);
        bicubic_wts(fx, w[3], w[7], w[11], w[15]).store(&mut wx[3][..N]);
        ctx.wx.set(wx);

        let fy = load_lanes(&ctx.fy);
        let mut wy = ctx.wy.get();
        bicubic_wts(fy, w[0], w[4], w[8], w[12]).store(&mut wy[0][..N]);
        bicubic_wts(fy, w[1], w[5], w[9], w[13]).store(&mut wy[1][..N]);
        bicubic_wts(fy, w[2], w[6], w[10], w[14]).store(&mut wy[2][..N]);
        bicubic_wts(fy, w[3], w[7], w[11], w[15]).store(&mut wy[3][..N]);
        ctx.wy.set(wy);

        // Init for accumulate
        p.dr = F::splat(0.0);
        p.dg = F::splat(0.0);
        p.db = F::splat(0.0);
        p.da = F::splat(0.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4038-L4046 (chrome/m156)
    pub(super) fn bicubic_n3x(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_x(-3, ctx, &mut p.r);
    }

    pub(super) fn bicubic_n1x(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_x(-1, ctx, &mut p.r);
    }

    pub(super) fn bicubic_p1x(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_x(1, ctx, &mut p.r);
    }

    pub(super) fn bicubic_p3x(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_x(3, ctx, &mut p.r);
    }

    pub(super) fn bicubic_n3y(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_y(-3, ctx, &mut p.g);
    }

    pub(super) fn bicubic_n1y(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_y(-1, ctx, &mut p.g);
    }

    pub(super) fn bicubic_p1y(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_y(1, ctx, &mut p.g);
    }

    pub(super) fn bicubic_p3y(ctx: &SamplerCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        bicubic_y(3, ctx, &mut p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4166-L4169 (chrome/m156)
    pub(super) fn mipmap_linear_init(ctx: &MipmapCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        store_lanes(&ctx.x, p.r);
        store_lanes(&ctx.y, p.g);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4171-L4179 (chrome/m156)
    pub(super) fn mipmap_linear_update(ctx: &MipmapCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        store_lanes(&ctx.r, p.r);
        store_lanes(&ctx.g, p.g);
        store_lanes(&ctx.b, p.b);
        store_lanes(&ctx.a, p.a);

        p.r = load_lanes(&ctx.x) * ctx.scale_x;
        p.g = load_lanes(&ctx.y) * ctx.scale_y;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4181-L4186 (chrome/m156)
    pub(super) fn mipmap_linear_finish(ctx: &MipmapCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let weight = F::splat(ctx.lower_weight);
        p.r = lerp(load_lanes(&ctx.r), p.r, weight);
        p.g = lerp(load_lanes(&ctx.g), p.g, weight);
        p.b = lerp(load_lanes(&ctx.b), p.b, weight);
        p.a = lerp(load_lanes(&ctx.a), p.a, weight);
    }
}
