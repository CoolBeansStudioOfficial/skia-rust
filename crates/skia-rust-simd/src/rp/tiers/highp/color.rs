// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Color stages: premul/unpremul, clamps, uniform colors, `dither`, `byte_tables`, color matrices,
//! `swizzle`, `emboss`, HSL/CSS conversions, transfer functions.
//!
//! Owner: task B4 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn clamp_01(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clamp_01", "B4")
    }

    pub(super) fn clamp_a_01(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clamp_a_01", "B4")
    }

    pub(super) fn clamp_gamut(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clamp_gamut", "B4")
    }

    pub(super) fn premul(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("premul", "B4")
    }

    pub(super) fn premul_dst(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("premul_dst", "B4")
    }

    pub(super) fn force_opaque(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("force_opaque", "B4")
    }

    pub(super) fn force_opaque_dst(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("force_opaque_dst", "B4")
    }

    pub(super) fn set_rgb(_ctx: &[f32; 3], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("set_rgb", "B4")
    }

    pub(super) fn black_color(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("black_color", "B4")
    }

    pub(super) fn white_color(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("white_color", "B4")
    }

    pub(super) fn uniform_color(_ctx: &UniformColorCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("uniform_color", "B4")
    }

    pub(super) fn uniform_color_dst(_ctx: &UniformColorCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("uniform_color_dst", "B4")
    }

    pub(super) fn bt709_luminance_or_luma_to_alpha(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bt709_luminance_or_luma_to_alpha", "B4")
    }

    pub(super) fn bt709_luminance_or_luma_to_rgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bt709_luminance_or_luma_to_rgb", "B4")
    }

    pub(super) fn emboss(_ctx: EmbossCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("emboss", "B4")
    }

    pub(super) fn swizzle(_ctx: [u8; 4], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle", "B4")
    }

    pub(super) fn unbounded_set_rgb(_ctx: &[f32; 3], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("unbounded_set_rgb", "B4")
    }

    pub(super) fn unbounded_uniform_color(_ctx: &UniformColorCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("unbounded_uniform_color", "B4")
    }

    pub(super) fn unpremul(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("unpremul", "B4")
    }

    pub(super) fn unpremul_polar(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("unpremul_polar", "B4")
    }

    pub(super) fn dither(_ctx: f32, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dither", "B4")
    }

    pub(super) fn byte_tables(_ctx: &TablesCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("byte_tables", "B4")
    }

    pub(super) fn matrix_3x3(_ctx: &[f32; 9], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_3x3", "B4")
    }

    pub(super) fn matrix_3x4(_ctx: &[f32; 12], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_3x4", "B4")
    }

    pub(super) fn matrix_4x5(_ctx: &[f32; 20], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_4x5", "B4")
    }

    pub(super) fn matrix_4x3(_ctx: &[f32; 12], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_4x3", "B4")
    }

    pub(super) fn parametric(_ctx: &TransferFunction, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("parametric", "B4")
    }

    pub(super) fn gamma_(_ctx: f32, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gamma_", "B4")
    }

    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn PQish(_ctx: &TransferFunction, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("PQish", "B4")
    }

    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn HLGish(_ctx: &TransferFunction, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("HLGish", "B4")
    }

    #[allow(non_snake_case)] // Skia's op name
    pub(super) fn HLGinvish(_ctx: &TransferFunction, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("HLGinvish", "B4")
    }

    pub(super) fn ootf(_ctx: &[f32; 4], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("ootf", "B4")
    }

    pub(super) fn rgb_to_hsl(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("rgb_to_hsl", "B4")
    }

    pub(super) fn hsl_to_rgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("hsl_to_rgb", "B4")
    }

    pub(super) fn css_lab_to_xyz(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_lab_to_xyz", "B4")
    }

    pub(super) fn css_oklab_to_linear_srgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_oklab_to_linear_srgb", "B4")
    }

    pub(super) fn css_oklab_gamut_map_to_linear_srgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_oklab_gamut_map_to_linear_srgb", "B4")
    }

    pub(super) fn css_hcl_to_lab(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_hcl_to_lab", "B4")
    }

    pub(super) fn css_hsl_to_srgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_hsl_to_srgb", "B4")
    }

    pub(super) fn css_hwb_to_srgb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("css_hwb_to_srgb", "B4")
    }

    pub(super) fn gauss_a_to_rgba(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gauss_a_to_rgba", "B4")
    }
}
