// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Color stages: premul, clamps, uniform colors, `emboss`, `swizzle`, luminance.
//!
//! Owner: task B4 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5732-L5732 (chrome/m156)
    fn from_float(f: f32) -> U16 {
        // `U16_(f * 255.0f + 0.5f)`: the float converts to `uint16_t` (a truncating conversion;
        // x86 and Arm agree on in-range values).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors the C conversion
        let v = (f * 255.0f32 + 0.5f32) as i32 as u16;
        U16::splat(v)
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6740-L6742 (chrome/m156)
    fn load_8(ptr: &[u8]) -> U16 {
        U8::load_bytes(ptr).cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6140-L6145 (chrome/m156)
    pub(super) fn uniform_color(c: &UniformColorCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = U16::splat(c.rgba[0]);
        p.g = U16::splat(c.rgba[1]);
        p.b = U16::splat(c.rgba[2]);
        p.a = U16::splat(c.rgba[3]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6146-L6151 (chrome/m156)
    pub(super) fn uniform_color_dst(c: &UniformColorCtx, p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = U16::splat(c.rgba[0]);
        p.dg = U16::splat(c.rgba[1]);
        p.db = U16::splat(c.rgba[2]);
        p.da = U16::splat(c.rgba[3]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6152-L6153 (chrome/m156)
    pub(super) fn black_color(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = U16::splat(0);
        p.g = U16::splat(0);
        p.b = U16::splat(0);
        p.a = U16::splat(255);
    }

    pub(super) fn white_color(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = U16::splat(255);
        p.g = U16::splat(255);
        p.b = U16::splat(255);
        p.a = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6155-L6159 (chrome/m156)
    pub(super) fn set_rgb(rgb: &[f32; 3], p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = from_float(rgb[0]);
        p.g = from_float(rgb[1]);
        p.b = from_float(rgb[2]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6161-L6167 (chrome/m156)
    // No need to clamp against 0 here (values are unsigned)
    pub(super) fn clamp_01(p: &mut Regs, _e: &mut Params<'_, '_>) {
        let max = U16::splat(255);
        p.r = min_u16(p.r, max);
        p.g = min_u16(p.g, max);
        p.b = min_u16(p.b, max);
        p.a = min_u16(p.a, max);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6169-L6171 (chrome/m156)
    pub(super) fn clamp_a_01(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = min_u16(p.a, U16::splat(255));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6173-L6178 (chrome/m156)
    pub(super) fn clamp_gamut(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = min_u16(p.a, U16::splat(255));
        p.r = min_u16(p.r, p.a);
        p.g = min_u16(p.g, p.a);
        p.b = min_u16(p.b, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6180-L6184 (chrome/m156)
    pub(super) fn premul(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = div255_accurate(p.r * p.a);
        p.g = div255_accurate(p.g * p.a);
        p.b = div255_accurate(p.b * p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6185-L6189 (chrome/m156)
    pub(super) fn premul_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = div255_accurate(p.dr * p.da);
        p.dg = div255_accurate(p.dg * p.da);
        p.db = div255_accurate(p.db * p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6191-L6192 (chrome/m156)
    pub(super) fn force_opaque(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.a = U16::splat(255);
    }

    pub(super) fn force_opaque_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.da = U16::splat(255);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6786-L6789 (chrome/m156)
    pub(super) fn bt709_luminance_or_luma_to_alpha(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // 0.2126, 0.7152, 0.0722 with 256 denominator.
        p.a = (p.r * 54 + p.g * 183 + p.b * 19) / 256;
        p.r = U16::splat(0);
        p.g = U16::splat(0);
        p.b = U16::splat(0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6790-L6792 (chrome/m156)
    pub(super) fn bt709_luminance_or_luma_to_rgb(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // 0.2126, 0.7152, 0.0722 with 256 denominator.
        let y = (p.r * 54 + p.g * 183 + p.b * 19) / 256;
        p.r = y;
        p.g = y;
        p.b = y;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6897-L6904 (chrome/m156)
    pub(super) fn emboss(ctx: EmbossCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let mul = load_8(e.ptr_at_xy(ctx.mul, 1));
        let add = load_8(e.ptr_at_xy(ctx.add, 1));

        p.r = min_u16(div255(p.r * mul) + add, p.a);
        p.g = min_u16(div255(p.g * mul) + add, p.a);
        p.b = min_u16(div255(p.b * mul) + add, p.a);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L7242-L7259 (chrome/m156)
    // ~~~~~~ skgpu::Swizzle stage ~~~~~~ //
    pub(super) fn swizzle(swiz: [u8; 4], p: &mut Regs, _e: &mut Params<'_, '_>) {
        let (ir, ig, ib, ia) = (p.r, p.g, p.b, p.a);
        let o = [&mut p.r, &mut p.g, &mut p.b, &mut p.a];

        for (o, c) in o.into_iter().zip(swiz) {
            match c {
                b'r' => *o = ir,
                b'g' => *o = ig,
                b'b' => *o = ib,
                b'a' => *o = ia,
                b'0' => *o = U16::splat(0),
                b'1' => *o = U16::splat(255),
                _ => {}
            }
        }
    }
}
