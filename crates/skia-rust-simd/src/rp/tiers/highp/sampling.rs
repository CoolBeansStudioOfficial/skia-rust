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
    pub(super) fn bilerp_clamp_8888(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilerp_clamp_8888", "P3")
    }

    pub(super) fn evenly_spaced_gradient(_ctx: &GradientCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("evenly_spaced_gradient", "P3")
    }

    pub(super) fn gradient(_ctx: &GradientCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gradient", "P3")
    }

    pub(super) fn evenly_spaced_2_stop_gradient(_ctx: &EvenlySpaced2StopGradientCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("evenly_spaced_2_stop_gradient", "P3")
    }

    pub(super) fn xy_to_unit_angle(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_unit_angle", "P3")
    }

    pub(super) fn xy_to_radius(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_radius", "P3")
    }

    pub(super) fn negate_x(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("negate_x", "P3")
    }

    pub(super) fn bilerp_clamp_8888_force_highp(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilerp_clamp_8888_force_highp", "P3")
    }

    pub(super) fn bicubic_clamp_8888(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_clamp_8888", "P3")
    }

    pub(super) fn bilinear_setup(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilinear_setup", "P3")
    }

    pub(super) fn bilinear_nx(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilinear_nx", "P3")
    }

    pub(super) fn bilinear_px(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilinear_px", "P3")
    }

    pub(super) fn bilinear_ny(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilinear_ny", "P3")
    }

    pub(super) fn bilinear_py(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bilinear_py", "P3")
    }

    pub(super) fn bicubic_setup(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_setup", "P3")
    }

    pub(super) fn bicubic_n3x(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_n3x", "P3")
    }

    pub(super) fn bicubic_n1x(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_n1x", "P3")
    }

    pub(super) fn bicubic_p1x(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_p1x", "P3")
    }

    pub(super) fn bicubic_p3x(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_p3x", "P3")
    }

    pub(super) fn bicubic_n3y(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_n3y", "P3")
    }

    pub(super) fn bicubic_n1y(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_n1y", "P3")
    }

    pub(super) fn bicubic_p1y(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_p1y", "P3")
    }

    pub(super) fn bicubic_p3y(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bicubic_p3y", "P3")
    }

    pub(super) fn accumulate(_ctx: &SamplerCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("accumulate", "P3")
    }

    pub(super) fn perlin_noise(_ctx: &PerlinNoiseCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("perlin_noise", "P3")
    }

    pub(super) fn mipmap_linear_init(_ctx: &MipmapCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mipmap_linear_init", "P3")
    }

    pub(super) fn mipmap_linear_update(_ctx: &MipmapCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mipmap_linear_update", "P3")
    }

    pub(super) fn mipmap_linear_finish(_ctx: &MipmapCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mipmap_linear_finish", "P3")
    }

    pub(super) fn xy_to_2pt_conical_strip(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_2pt_conical_strip", "P3")
    }

    pub(super) fn xy_to_2pt_conical_focal_on_circle(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_2pt_conical_focal_on_circle", "P3")
    }

    pub(super) fn xy_to_2pt_conical_well_behaved(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_2pt_conical_well_behaved", "P3")
    }

    pub(super) fn xy_to_2pt_conical_smaller(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_2pt_conical_smaller", "P3")
    }

    pub(super) fn xy_to_2pt_conical_greater(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_2pt_conical_greater", "P3")
    }

    pub(super) fn alter_2pt_conical_compensate_focal(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alter_2pt_conical_compensate_focal", "P3")
    }

    pub(super) fn alter_2pt_conical_unswap(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alter_2pt_conical_unswap", "P3")
    }

    pub(super) fn mask_2pt_conical_nan(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mask_2pt_conical_nan", "P3")
    }

    pub(super) fn mask_2pt_conical_degenerates(_ctx: &Conical2PtCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mask_2pt_conical_degenerates", "P3")
    }

    pub(super) fn apply_vector_mask(_ctx: &Cell<[u32; MAX_STRIDE_HIGHP]>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("apply_vector_mask", "P3")
    }
}
