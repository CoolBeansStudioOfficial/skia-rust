// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Blend modes and coverage (`scale_*`, `lerp_*`). `srcover` is ported (A3).
//!
//! Owner: task B3 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn scale_u8(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("scale_u8", "B3")
    }

    pub(super) fn scale_565(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("scale_565", "B3")
    }

    pub(super) fn scale_1_float(_ctx: &Cell<f32>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("scale_1_float", "B3")
    }

    pub(super) fn scale_native(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("scale_native", "B3")
    }

    pub(super) fn lerp_u8(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("lerp_u8", "B3")
    }

    pub(super) fn lerp_565(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("lerp_565", "B3")
    }

    pub(super) fn lerp_1_float(_ctx: &Cell<f32>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("lerp_1_float", "B3")
    }

    pub(super) fn lerp_native(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("lerp_native", "B3")
    }

    pub(super) fn dstatop(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dstatop", "B3")
    }

    pub(super) fn dstin(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dstin", "B3")
    }

    pub(super) fn dstout(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dstout", "B3")
    }

    pub(super) fn dstover(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dstover", "B3")
    }

    pub(super) fn srcatop(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("srcatop", "B3")
    }

    pub(super) fn srcin(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("srcin", "B3")
    }

    pub(super) fn srcout(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("srcout", "B3")
    }

    pub(super) fn clear(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("clear", "B3")
    }

    pub(super) fn modulate(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("modulate", "B3")
    }

    pub(super) fn multiply(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("multiply", "B3")
    }

    pub(super) fn plus_(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("plus_", "B3")
    }

    pub(super) fn screen(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("screen", "B3")
    }

    pub(super) fn xor_(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("xor_", "B3")
    }

    pub(super) fn darken(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("darken", "B3")
    }

    pub(super) fn difference(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("difference", "B3")
    }

    pub(super) fn exclusion(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("exclusion", "B3")
    }

    pub(super) fn hardlight(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("hardlight", "B3")
    }

    pub(super) fn lighten(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("lighten", "B3")
    }

    pub(super) fn overlay(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("overlay", "B3")
    }

    pub(super) fn colorburn(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("colorburn", "B3")
    }

    pub(super) fn colordodge(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("colordodge", "B3")
    }

    pub(super) fn softlight(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("softlight", "B3")
    }

    pub(super) fn hue(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("hue", "B3")
    }

    pub(super) fn saturation(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("saturation", "B3")
    }

    pub(super) fn color(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("color", "B3")
    }

    pub(super) fn luminosity(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("luminosity", "B3")
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L2427-L2439 (chrome/m156)
// The highp `BLEND_MODE(name)` macro: `name_channel` applied to each channel, alpha last.
si! {
    /// `inv(x)`: `1.0f - x`.
    fn inv(x: F) -> F {
        F::splat(1.0) - x
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2448-L2448 (chrome/m156)
    fn srcover_channel(s: F, d: F, sa: F, _da: F) -> F {
        mad(d, inv(sa), s)
    }

    pub(super) fn srcover(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = srcover_channel(p.r, p.dr, p.a, p.da);
        p.g = srcover_channel(p.g, p.dg, p.a, p.da);
        p.b = srcover_channel(p.b, p.db, p.a, p.da);
        p.a = srcover_channel(p.a, p.da, p.a, p.da);
    }
}
