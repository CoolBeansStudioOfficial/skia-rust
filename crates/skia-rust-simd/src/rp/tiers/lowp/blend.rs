// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Blend modes and coverage (`scale_*`, `lerp_*`), lowp.
//!
//! Owner: task B3 (`docs/design/raster-pipeline.md` §5). `srcover` was ported by A3.
//!
//! The blend modes are the `#else` branch of `SK_USE_INACCURATE_DIV255_IN_BLEND` (Skia's
//! default: the define is never set by the build).

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `U16` register (`N` lanes).
const U16_BYTES: usize = 2 * N;

// Port of: src/opts/SkRasterPipeline_opts.h#L6229-L6237 (chrome/m156)
/// The lowp `BLEND_MODE(name)` macro for modes that apply the same logic to all 4 channels.
macro_rules! blend_mode_all {
    ($name:ident, $channel:ident) => {
        si! {
            pub(super) fn $name(p: &mut Regs, _e: &mut Params<'_, '_>) {
                p.r = $channel(p.r, p.dr, p.a, p.da);
                p.g = $channel(p.g, p.dg, p.a, p.da);
                p.b = $channel(p.b, p.db, p.a, p.da);
                p.a = $channel(p.a, p.da, p.a, p.da);
            }
        }
    };
}

// Port of: src/opts/SkRasterPipeline_opts.h#L6273-L6281 (chrome/m156)
/// The second lowp `BLEND_MODE(name)` macro: the same logic applied to color, and srcover for
/// alpha.
macro_rules! blend_mode_color {
    ($name:ident, $channel:ident) => {
        si! {
            pub(super) fn $name(p: &mut Regs, _e: &mut Params<'_, '_>) {
                p.r = $channel(p.r, p.dr, p.a, p.da);
                p.g = $channel(p.g, p.dg, p.a, p.da);
                p.b = $channel(p.b, p.db, p.a, p.da);
                p.a = p.a + div255(p.da * inv(p.a));
            }
        }
    };
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5730-L5730 (chrome/m156)
    /// `inv(v)`: `255 - v`.
    fn inv(v: U16) -> U16 {
        U16::splat(255) - v
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6255-L6268 (chrome/m156)
    fn clear_channel(_s: U16, _d: U16, _sa: U16, _da: U16) -> U16 {
        U16::splat(0)
    }
    fn srcatop_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        div255(s * da + d * inv(sa))
    }
    fn dstatop_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        div255(d * sa + s * inv(da))
    }
    fn srcin_channel(s: U16, _d: U16, _sa: U16, da: U16) -> U16 {
        div255_accurate(s * da)
    }
    fn dstin_channel(_s: U16, d: U16, sa: U16, _da: U16) -> U16 {
        div255_accurate(d * sa)
    }
    fn srcout_channel(s: U16, _d: U16, _sa: U16, da: U16) -> U16 {
        div255_accurate(s * inv(da))
    }
    fn dstout_channel(_s: U16, d: U16, sa: U16, _da: U16) -> U16 {
        div255_accurate(d * inv(sa))
    }
    fn srcover_channel(s: U16, d: U16, sa: U16, _da: U16) -> U16 {
        s + div255_accurate(d * inv(sa))
    }
    fn dstover_channel(s: U16, d: U16, _sa: U16, da: U16) -> U16 {
        d + div255_accurate(s * inv(da))
    }
    fn modulate_channel(s: U16, d: U16, _sa: U16, _da: U16) -> U16 {
        div255_accurate(s * d)
    }
    fn multiply_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        div255(s * inv(da) + d * inv(sa) + s * d)
    }
    fn plus_channel(s: U16, d: U16, _sa: U16, _da: U16) -> U16 {
        min_u16(s + d, U16::splat(255))
    }
    fn screen_channel(s: U16, d: U16, _sa: U16, _da: U16) -> U16 {
        s + d - div255_accurate(s * d)
    }
    fn xor_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        div255(s * inv(da) + d * inv(sa))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6283-L6286 (chrome/m156)
    fn darken_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        s + d - div255(max_intr_u16(s * da, d * sa))
    }
    fn lighten_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        s + d - div255(min_intr_u16(s * da, d * sa))
    }
    fn difference_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        s + d - div255(min_intr_u16(s * da, d * sa)) * 2
    }
    fn exclusion_channel(s: U16, d: U16, _sa: U16, _da: U16) -> U16 {
        s + d - div255(s * d) * 2
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6288-L6291 (chrome/m156)
    fn hardlight_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        let c: I16 = (s * 2).le_mask(sa).bit_cast();
        div255(
            s * inv(da)
                + d * inv(sa)
                + if_then_else_u16(c, s * 2 * d, sa * da - (sa - s) * 2 * (da - d)),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6292-L6295 (chrome/m156)
    fn overlay_channel(s: U16, d: U16, sa: U16, da: U16) -> U16 {
        let c: I16 = (d * 2).le_mask(da).bit_cast();
        div255(
            s * inv(da)
                + d * inv(sa)
                + if_then_else_u16(c, s * 2 * d, sa * da - (sa - s) * 2 * (da - d)),
        )
    }
}

blend_mode_all!(clear, clear_channel);
blend_mode_all!(srcatop, srcatop_channel);
blend_mode_all!(dstatop, dstatop_channel);
blend_mode_all!(srcin, srcin_channel);
blend_mode_all!(dstin, dstin_channel);
blend_mode_all!(srcout, srcout_channel);
blend_mode_all!(dstout, dstout_channel);
blend_mode_all!(srcover, srcover_channel);
blend_mode_all!(dstover, dstover_channel);
blend_mode_all!(modulate, modulate_channel);
blend_mode_all!(multiply, multiply_channel);
blend_mode_all!(plus_, plus_channel);
blend_mode_all!(screen, screen_channel);
blend_mode_all!(xor_, xor_channel);

blend_mode_color!(darken, darken_channel);
blend_mode_color!(lighten, lighten_channel);
blend_mode_color!(difference, difference_channel);
blend_mode_color!(exclusion, exclusion_channel);
blend_mode_color!(hardlight, hardlight_channel);
blend_mode_color!(overlay, overlay_channel);

// ~~~ Coverage scales / lerps ~~~
si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5732-L5732 (chrome/m156)
    /// `from_float(f)`: `U16_(f * 255.0f + 0.5f)`.
    fn from_float(f: f32) -> U16 {
        // mirrors the C++ float -> uint16_t conversion
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let v = (f * 255.0 + 0.5) as u16;
        U16::splat(v)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5734-L5734 (chrome/m156)
    /// `lerp(from, to, t)`: `div255(from*inv(t) + to*t)`.
    fn lerp(from: U16, to: U16, t: U16) -> U16 {
        div255(from * inv(t) + to * t)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6740-L6742 (chrome/m156)
    /// `load_8(ptr)`: `cast<U16>(load<U8>(ptr))`.
    fn load_8(ptr: &[u8]) -> U16 {
        U8::load_bytes(ptr).cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6589-L6599 (chrome/m156)
    /// `load_565_(ptr, &r, &g, &b)`: 565 pixels as 8-bit `r, g, b` (via `from_565`).
    fn load_565_(ptr: &[u8]) -> (U16, U16, U16) {
        let rgb = U16::load_bytes(ptr);
        // Format for 565 buffers: 15|rrrrr gggggg bbbbb|0
        let r = (rgb >> 11) & 31;
        let g = (rgb >> 5) & 63;
        let b = rgb & 31;

        // These bit replications are the same as multiplying by 255/31 or 255/63 to scale to
        // 8-bit.
        ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6872-L6875 (chrome/m156)
    /// Derive alpha's coverage from rgb coverage and the values of src and dst alpha.
    fn alpha_coverage_from_rgb_coverage(a: U16, da: U16, cr: U16, cg: U16, cb: U16) -> U16 {
        let c: I16 = a.lt_mask(da).bit_cast();
        if_then_else_u16(
            c,
            min_intr_u16(cr, min_intr_u16(cg, cb)),
            max_intr_u16(cr, max_intr_u16(cg, cb)),
        )
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6826-L6832 (chrome/m156)
    pub(super) fn scale_1_float(ctx: &Cell<f32>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let c = from_float(ctx.get());
        p.r = div255(p.r * c);
        p.g = div255(p.g * c);
        p.b = div255(p.b * c);
        p.a = div255(p.a * c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6833-L6839 (chrome/m156)
    pub(super) fn lerp_1_float(ctx: &Cell<f32>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let c = from_float(ctx.get());
        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6840-L6846 (chrome/m156)
    pub(super) fn scale_native(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = U16::load_bytes(&e.ptr(ctx)[..U16_BYTES]);
        p.r = div255(p.r * c);
        p.g = div255(p.g * c);
        p.b = div255(p.b * c);
        p.a = div255(p.a * c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6848-L6854 (chrome/m156)
    pub(super) fn lerp_native(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = U16::load_bytes(&e.ptr(ctx)[..U16_BYTES]);
        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6856-L6862 (chrome/m156)
    pub(super) fn scale_u8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = load_8(e.ptr_at_xy(ctx, 1));
        p.r = div255(p.r * c);
        p.g = div255(p.g * c);
        p.b = div255(p.b * c);
        p.a = div255(p.a * c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6863-L6869 (chrome/m156)
    pub(super) fn lerp_u8(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let c = load_8(e.ptr_at_xy(ctx, 1));
        p.r = lerp(p.dr, p.r, c);
        p.g = lerp(p.dg, p.g, c);
        p.b = lerp(p.db, p.b, c);
        p.a = lerp(p.da, p.a, c);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6876-L6885 (chrome/m156)
    pub(super) fn scale_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (cr, cg, cb) = load_565_(e.ptr_at_xy(ctx, 2));
        let ca = alpha_coverage_from_rgb_coverage(p.a, p.da, cr, cg, cb);

        p.r = div255(p.r * cr);
        p.g = div255(p.g * cg);
        p.b = div255(p.b * cb);
        p.a = div255(p.a * ca);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6886-L6896 (chrome/m156)
    pub(super) fn lerp_565(ctx: MemoryCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let (cr, cg, cb) = load_565_(e.ptr_at_xy(ctx, 2));
        let ca = alpha_coverage_from_rgb_coverage(p.a, p.da, cr, cg, cb);

        p.r = lerp(p.dr, p.r, cr);
        p.g = lerp(p.dg, p.g, cg);
        p.b = lerp(p.db, p.b, cb);
        p.a = lerp(p.da, p.a, ca);
    }
}
