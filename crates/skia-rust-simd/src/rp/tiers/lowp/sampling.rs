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
    pub(super) fn evenly_spaced_gradient(_ctx: &GradientCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("evenly_spaced_gradient", "P3")
    }

    pub(super) fn gradient(_ctx: &GradientCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gradient", "P3")
    }

    pub(super) fn evenly_spaced_2_stop_gradient(_ctx: &EvenlySpaced2StopGradientCtx, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("evenly_spaced_2_stop_gradient", "P3")
    }

    pub(super) fn xy_to_unit_angle(_x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_unit_angle", "P3")
    }

    pub(super) fn xy_to_radius(_x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        not_ported!("xy_to_radius", "P3")
    }
}
