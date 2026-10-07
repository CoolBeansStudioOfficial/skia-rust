// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlendModePriv.h, src/core/SkBlendMode.cpp

//! `SkBlendModePriv`: the private blend-mode helpers (raster pipeline stages, coverage,
//! constant-color blending, draw fast paths).

use crate::blend_mode::BlendMode;
use crate::color::PMColor4f;
use crate::paint::Paint;
use crate::raster_pipeline::{MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage};

/// Sentinel value for [`BlendMode`]: never a valid mode, but storable in a byte
/// (`kCustom_SkBlendMode`).
// Port of: src/core/SkBlendModePriv.h#L24 (chrome/m156)
#[doc(alias = "kCustom_SkBlendMode")]
pub const CUSTOM_BLEND_MODE: u8 = 0xFF;

/// True if `mode` can be applied with the coverage pre-multiplied into the source
/// (`SkBlendMode_ShouldPreScaleCoverage`); `rgb_coverage` is true for per-channel (LCD)
/// coverage.
// Port of: src/core/SkBlendMode.cpp#L22-L51 (chrome/m156)
#[doc(alias = "SkBlendMode_ShouldPreScaleCoverage")]
#[must_use]
pub fn should_pre_scale_coverage(mode: BlendMode, rgb_coverage: bool) -> bool {
    // The most important things we do here are:
    //   1) never pre-scale with rgb coverage if the blend mode involves a source-alpha term;
    //   2) always pre-scale Plus.
    //
    // When we pre-scale with rgb coverage, we scale each of source r,g,b, with a distinct value,
    // and source alpha with one of those three values.  This process destructively updates the
    // source-alpha term, so we can't evaluate blend modes that need its original value.
    //
    // Plus always requires pre-scaling as a specific quirk of its implementation in
    // SkRasterPipeline.  This lets us put the clamp inside the blend mode itself rather
    // than as a separate stage that'd come after the lerp.
    //
    // This function is a finer-grained breakdown of SkBlendMode_SupportsCoverageAsAlpha().
    match mode {
        BlendMode::Dst       // d              --> no sa term, ok!
        | BlendMode::DstOver // d + s*inv(da)  --> no sa term, ok!
        | BlendMode::Plus    // clamp(s+d)     --> no sa term, ok!
        => true,

        BlendMode::DstOut    // d * inv(sa)
        | BlendMode::SrcATop // s*da + d*inv(sa)
        | BlendMode::SrcOver // s + d*inv(sa)
        | BlendMode::Xor     // s*inv(da) + d*inv(sa)
        => !rgb_coverage,

        _ => false,
    }
}

/// True if coverage can be treated as alpha for `mode`
/// (`SkBlendMode_SupportsCoverageAsAlpha`).
// Port of: src/core/SkBlendMode.cpp#L53-L56 (chrome/m156)
#[doc(alias = "SkBlendMode_SupportsCoverageAsAlpha")]
#[must_use]
pub fn supports_coverage_as_alpha(mode: BlendMode) -> bool {
    should_pre_scale_coverage(mode, false)
}

/// True if the result of `mode` depends on the order of the color channels
/// (`SkBlendMode_CaresAboutRBOrder`): the non-separable modes.
// Port of: src/core/SkBlendModePriv.h#L28-L30 (chrome/m156)
#[doc(alias = "SkBlendMode_CaresAboutRBOrder")]
#[must_use]
pub fn cares_about_rb_order(mode: BlendMode) -> bool {
    mode > BlendMode::LAST_SEPARABLE_MODE
}

/// Appends the blend stage of `mode` to `p` (`SkBlendMode_AppendStages`): nothing for
/// [`BlendMode::Src`].
// Port of: src/core/SkBlendMode.cpp#L98-L134 (chrome/m156)
#[doc(alias = "SkBlendMode_AppendStages")]
pub fn append_stages(mode: BlendMode, p: &mut RasterPipeline<'_>) {
    let stage = match mode {
        BlendMode::Clear => Stage::Clear,
        BlendMode::Src => return, // This stage is a no-op.
        BlendMode::Dst => Stage::MoveDstSrc,
        BlendMode::SrcOver => Stage::Srcover,
        BlendMode::DstOver => Stage::Dstover,
        BlendMode::SrcIn => Stage::Srcin,
        BlendMode::DstIn => Stage::Dstin,
        BlendMode::SrcOut => Stage::Srcout,
        BlendMode::DstOut => Stage::Dstout,
        BlendMode::SrcATop => Stage::Srcatop,
        BlendMode::DstATop => Stage::Dstatop,
        BlendMode::Xor => Stage::Xor,
        BlendMode::Plus => Stage::Plus,
        BlendMode::Modulate => Stage::Modulate,

        BlendMode::Screen => Stage::Screen,
        BlendMode::Overlay => Stage::Overlay,
        BlendMode::Darken => Stage::Darken,
        BlendMode::Lighten => Stage::Lighten,
        BlendMode::ColorDodge => Stage::Colordodge,
        BlendMode::ColorBurn => Stage::Colorburn,
        BlendMode::HardLight => Stage::Hardlight,
        BlendMode::SoftLight => Stage::Softlight,
        BlendMode::Difference => Stage::Difference,
        BlendMode::Exclusion => Stage::Exclusion,
        BlendMode::Multiply => Stage::Multiply,

        BlendMode::Hue => Stage::Hue,
        BlendMode::Saturation => Stage::Saturation,
        BlendMode::Color => Stage::Color,
        BlendMode::Luminosity => Stage::Luminosity,
        // (Skia's `default: srcover` is unreachable: every mode is listed.)
    };
    p.append(stage);
}

/// Blends `src` over `dst` with `mode` (`SkBlendMode_Apply`): the simple modes directly, the
/// others through a one-pixel highp raster pipeline (`load_f32` / `store_f32`).
// Port of: src/core/SkBlendMode.cpp#L136-L165 (chrome/m156)
#[doc(alias = "SkBlendMode_Apply")]
#[must_use]
pub fn apply(mode: BlendMode, src: &PMColor4f, dst: &PMColor4f) -> PMColor4f {
    // special-case simple/common modes...
    match mode {
        BlendMode::Clear => return PMColor4f::new(0.0, 0.0, 0.0, 0.0),
        BlendMode::Src => return *src,
        BlendMode::Dst => return *dst,
        BlendMode::SrcOver => {
            // skvx: float4(src) + float4(dst) * (1 - src.fA), lane by lane.
            let inv_sa = 1.0 - src.a;
            return PMColor4f::new(
                src.r + dst.r * inv_sa,
                src.g + dst.g * inv_sa,
                src.b + dst.b * inv_sa,
                src.a + dst.a * inv_sa,
            );
        }
        _ => {}
    }

    let src_ctx = MemoryCtx::new(MemSlot(0));
    let dst_ctx = MemoryCtx::new(MemSlot(1));
    let res_ctx = MemoryCtx::new(MemSlot(2));

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadF32(dst_ctx));
    p.append(Stage::MoveSrcDst);
    p.append(Stage::LoadF32(src_ctx));
    append_stages(mode, &mut p);
    p.append(Stage::StoreF32(res_ctx));

    let src_storage = color_to_bytes(src.as_array());
    let dst_storage = color_to_bytes(dst.as_array());
    let mut res_storage = [0u8; 16];
    let mut mem = MemoryBindings::new();
    mem.bind(src_ctx.slot, MemView::read(&src_storage));
    mem.bind(dst_ctx.slot, MemView::read(&dst_storage));
    mem.bind(res_ctx.slot, MemView::write(&mut res_storage));
    p.run(0, 0, 1, 1, &mut mem);
    let res = color_from_bytes(&res_storage);
    PMColor4f::new(res[0], res[1], res[2], res[3])
}

/// The native-endian bytes of four floats (an `SkPMColor4f` in memory).
pub(crate) fn color_to_bytes(c: [f32; 4]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (chunk, v) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(c) {
        *chunk = v.to_ne_bytes();
    }
    bytes
}

/// Four floats from their native-endian bytes.
pub(crate) fn color_from_bytes(bytes: &[u8; 16]) -> [f32; 4] {
    core::array::from_fn(|i| {
        f32::from_ne_bytes([
            bytes[4 * i],
            bytes[4 * i + 1],
            bytes[4 * i + 2],
            bytes[4 * i + 3],
        ])
    })
}

/// What a draw with a paint can be reduced to (`SkBlendFastPath`).
// Port of: src/core/SkBlendModePriv.h#L37-L41 (chrome/m156)
#[doc(alias = "SkBlendFastPath")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlendFastPath {
    /// Draw normally (`kNormal`).
    Normal,
    /// Draw as if in src-over mode (`kSrcOver`).
    SrcOver,
    /// Draw nothing (`kSkipDrawing`).
    SkipDrawing,
}

// Port of: src/core/SkBlendMode.cpp#L204-L206 (chrome/m156)
fn just_solid_color(p: &Paint) -> bool {
    p.alpha() == 0xFF && p.color_filter().is_none() && p.shader().is_none()
}

/// Whether the paint's blend mode can be replaced with src-over or not drawn at all
/// (`CheckFastPath`).
// Port of: src/core/SkBlendMode.cpp#L208-L241 (chrome/m156)
#[doc(alias = "CheckFastPath")]
#[must_use]
pub fn check_fast_path(paint: &Paint, dst_is_opaque: bool) -> BlendFastPath {
    let Some(bm) = paint.as_blend_mode() else {
        return BlendFastPath::Normal;
    };
    match bm {
        BlendMode::SrcOver => BlendFastPath::SrcOver,
        BlendMode::Src => {
            if just_solid_color(paint) {
                return BlendFastPath::SrcOver;
            }
            BlendFastPath::Normal
        }
        BlendMode::Dst => BlendFastPath::SkipDrawing,
        BlendMode::DstOver => {
            if dst_is_opaque {
                return BlendFastPath::SkipDrawing;
            }
            BlendFastPath::Normal
        }
        BlendMode::SrcIn => {
            if dst_is_opaque && just_solid_color(paint) {
                return BlendFastPath::SrcOver;
            }
            BlendFastPath::Normal
        }
        BlendMode::DstIn => {
            if just_solid_color(paint) {
                return BlendFastPath::SkipDrawing;
            }
            BlendFastPath::Normal
        }
        _ => BlendFastPath::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::shaders;

    #[test]
    fn coverage() {
        for mode in BlendMode::VALUES {
            let always = matches!(mode, BlendMode::Dst | BlendMode::DstOver | BlendMode::Plus);
            let alpha_only = matches!(
                mode,
                BlendMode::DstOut | BlendMode::SrcATop | BlendMode::SrcOver | BlendMode::Xor
            );
            assert_eq!(should_pre_scale_coverage(mode, true), always, "{mode:?}");
            assert_eq!(
                should_pre_scale_coverage(mode, false),
                always || alpha_only,
                "{mode:?}"
            );
            assert_eq!(
                supports_coverage_as_alpha(mode),
                should_pre_scale_coverage(mode, false)
            );
            assert_eq!(
                cares_about_rb_order(mode),
                mode >= BlendMode::Hue,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn append_stages_of_src_is_nothing() {
        let mut p = RasterPipeline::new();
        append_stages(BlendMode::Src, &mut p);
        assert!(p.empty());
        append_stages(BlendMode::Xor, &mut p);
        assert_eq!(p.to_string(), "SkRasterPipeline, 1 stages\n\txor_\n\n");
    }

    #[test]
    fn apply_blends_one_color() {
        // Dyadic values: every tier computes them exactly.
        let src = PMColor4f::new(0.25, 0.5, 0.125, 0.5);
        let dst = PMColor4f::new(0.5, 0.25, 0.75, 1.0);
        assert_eq!(
            apply(BlendMode::Clear, &src, &dst),
            PMColor4f::new(0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(apply(BlendMode::Src, &src, &dst), src);
        assert_eq!(apply(BlendMode::Dst, &src, &dst), dst);
        assert_eq!(
            apply(BlendMode::SrcOver, &src, &dst),
            PMColor4f::new(0.5, 0.625, 0.5, 1.0)
        );
        // Through the pipeline.
        assert_eq!(
            apply(BlendMode::Modulate, &src, &dst),
            PMColor4f::new(0.125, 0.125, 0.093_75, 0.5)
        );
        assert_eq!(
            apply(BlendMode::DstIn, &src, &dst),
            PMColor4f::new(0.25, 0.125, 0.375, 0.5)
        );
        assert_eq!(
            apply(BlendMode::Plus, &src, &dst),
            PMColor4f::new(0.75, 0.75, 0.875, 1.0)
        );
    }

    #[test]
    fn fast_paths() {
        let mut p = Paint::default();
        assert_eq!(check_fast_path(&p, false), BlendFastPath::SrcOver);
        p.set_blend_mode(BlendMode::Src);
        assert_eq!(check_fast_path(&p, false), BlendFastPath::SrcOver);
        p.set_blend_mode(BlendMode::Dst);
        assert_eq!(check_fast_path(&p, false), BlendFastPath::SkipDrawing);
        p.set_blend_mode(BlendMode::DstOver);
        assert_eq!(check_fast_path(&p, false), BlendFastPath::Normal);
        assert_eq!(check_fast_path(&p, true), BlendFastPath::SkipDrawing);
        p.set_blend_mode(BlendMode::SrcIn);
        assert_eq!(check_fast_path(&p, false), BlendFastPath::Normal);
        assert_eq!(check_fast_path(&p, true), BlendFastPath::SrcOver);
        p.set_blend_mode(BlendMode::DstIn);
        assert_eq!(check_fast_path(&p, false), BlendFastPath::SkipDrawing);
        p.set_blend_mode(BlendMode::Multiply);
        assert_eq!(check_fast_path(&p, true), BlendFastPath::Normal);

        // Not a solid color: translucent, or with a shader.
        p.set_blend_mode(BlendMode::Src);
        p.set_alpha(0xFE);
        assert_eq!(check_fast_path(&p, true), BlendFastPath::Normal);
        p.set_alpha(0xFF);
        p.set_shader(shaders::color(Color::RED));
        assert_eq!(check_fast_path(&p, true), BlendFastPath::Normal);
        p.set_blend_mode(BlendMode::DstIn);
        assert_eq!(check_fast_path(&p, true), BlendFastPath::Normal);
    }
}
