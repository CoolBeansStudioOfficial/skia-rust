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
}
