// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Geometry and tiling: matrices, `repeat`/`mirror`/`clamp`/`decal`.
//!
//! Owner: task B5 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn matrix_translate(_ctx: [f32; 2], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_translate", "B5")
    }

    pub(super) fn matrix_scale_translate(_ctx: &[f32; 4], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_scale_translate", "B5")
    }

    pub(super) fn matrix_2x3(_ctx: &[f32; 6], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_2x3", "B5")
    }

    pub(super) fn matrix_perspective(_ctx: &[f32; 9], _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_perspective", "B5")
    }

    pub(super) fn decal_x(_ctx: &DecalTileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("decal_x", "B5")
    }

    pub(super) fn decal_y(_ctx: &DecalTileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("decal_y", "B5")
    }

    pub(super) fn decal_x_and_y(_ctx: &DecalTileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("decal_x_and_y", "B5")
    }

    pub(super) fn check_decal_mask(_ctx: &DecalTileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("check_decal_mask", "B5")
    }

    pub(super) fn clamp_x_1(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clamp_x_1", "B5")
    }

    pub(super) fn mirror_x_1(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mirror_x_1", "B5")
    }

    pub(super) fn repeat_x_1(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("repeat_x_1", "B5")
    }

    pub(super) fn clamp_x_and_y(_ctx: &CoordClampCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clamp_x_and_y", "B5")
    }

    pub(super) fn mirror_x(_ctx: &TileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mirror_x", "B5")
    }

    pub(super) fn repeat_x(_ctx: &TileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("repeat_x", "B5")
    }

    pub(super) fn mirror_y(_ctx: &TileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mirror_y", "B5")
    }

    pub(super) fn repeat_y(_ctx: &TileCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("repeat_y", "B5")
    }
}
