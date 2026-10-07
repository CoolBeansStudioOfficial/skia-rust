// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! 8-bit memory stages: load/store/gather of a8, 565, 4444, 8888, rg88, `store_r8`,
//! `srcover_rgba_8888`, `swap_rb`, `alpha_to_*`, `debug_*`.
//!
//! Owner: task B1 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn swap_rb(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swap_rb", "B1")
    }

    pub(super) fn swap_rb_dst(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swap_rb_dst", "B1")
    }

    pub(super) fn load_a8(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_a8", "B1")
    }

    pub(super) fn load_a8_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_a8_dst", "B1")
    }

    pub(super) fn store_a8(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_a8", "B1")
    }

    pub(super) fn gather_a8(_ctx: &GatherCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_a8", "B1")
    }

    pub(super) fn load_565(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_565", "B1")
    }

    pub(super) fn load_565_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_565_dst", "B1")
    }

    pub(super) fn store_565(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_565", "B1")
    }

    pub(super) fn gather_565(_ctx: &GatherCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_565", "B1")
    }

    pub(super) fn load_4444(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_4444", "B1")
    }

    pub(super) fn load_4444_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_4444_dst", "B1")
    }

    pub(super) fn store_4444(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_4444", "B1")
    }

    pub(super) fn gather_4444(_ctx: &GatherCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_4444", "B1")
    }

    pub(super) fn load_8888(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_8888", "B1")
    }

    pub(super) fn load_8888_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_8888_dst", "B1")
    }

    pub(super) fn store_8888(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_8888", "B1")
    }

    pub(super) fn gather_8888(_ctx: &GatherCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_8888", "B1")
    }

    pub(super) fn load_rg88(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rg88", "B1")
    }

    pub(super) fn load_rg88_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rg88_dst", "B1")
    }

    pub(super) fn store_rg88(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_rg88", "B1")
    }

    pub(super) fn gather_rg88(_ctx: &GatherCtx<'_>, _x: F, _y: F, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_rg88", "B1")
    }

    pub(super) fn store_r8(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_r8", "B1")
    }

    pub(super) fn alpha_to_gray(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alpha_to_gray", "B1")
    }

    pub(super) fn alpha_to_gray_dst(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alpha_to_gray_dst", "B1")
    }

    pub(super) fn alpha_to_red(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alpha_to_red", "B1")
    }

    pub(super) fn alpha_to_red_dst(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("alpha_to_red_dst", "B1")
    }

    pub(super) fn srcover_rgba_8888(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("srcover_rgba_8888", "B1")
    }

    pub(super) fn debug_x(_ctx: MemoryCtx, _x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        not_ported!("debug_x", "B1")
    }

    pub(super) fn debug_y(_ctx: MemoryCtx, _x: &mut F, _y: &mut F, _e: &mut Params<'_, '_>) {
        not_ported!("debug_y", "B1")
    }

    pub(super) fn debug_r(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_r", "B1")
    }

    pub(super) fn debug_g(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_g", "B1")
    }

    pub(super) fn debug_b(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_b", "B1")
    }

    pub(super) fn debug_a(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_a", "B1")
    }

    pub(super) fn debug_r_255(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_r_255", "B1")
    }

    pub(super) fn debug_g_255(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_g_255", "B1")
    }

    pub(super) fn debug_b_255(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_b_255", "B1")
    }

    pub(super) fn debug_a_255(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("debug_a_255", "B1")
    }
}
