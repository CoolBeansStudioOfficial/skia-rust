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

    pub(super) fn perlin_noise(_ctx: &PerlinNoiseCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("perlin_noise", "P3")
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
