// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Wide memory stages: the f16 family, f32, `16161616/a16/r16/rg1616`, `1010102/xr`, `10x6`, `10101010_xr`,
//! `load_src_rg`/`store_src_rg`.
//!
//! Owner: task B2 (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn load_16161616(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_16161616", "B2")
    }

    pub(super) fn load_16161616_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_16161616_dst", "B2")
    }

    pub(super) fn store_16161616(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_16161616", "B2")
    }

    pub(super) fn gather_16161616(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_16161616", "B2")
    }

    pub(super) fn load_a16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_a16", "B2")
    }

    pub(super) fn load_a16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_a16_dst", "B2")
    }

    pub(super) fn store_a16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_a16", "B2")
    }

    pub(super) fn gather_a16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_a16", "B2")
    }

    pub(super) fn load_r16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_r16", "B2")
    }

    pub(super) fn load_r16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_r16_dst", "B2")
    }

    pub(super) fn store_r16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_r16", "B2")
    }

    pub(super) fn gather_r16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_r16", "B2")
    }

    pub(super) fn load_rg1616(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rg1616", "B2")
    }

    pub(super) fn load_rg1616_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rg1616_dst", "B2")
    }

    pub(super) fn store_rg1616(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_rg1616", "B2")
    }

    pub(super) fn gather_rg1616(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_rg1616", "B2")
    }

    pub(super) fn load_f16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_f16", "B2")
    }

    pub(super) fn load_f16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_f16_dst", "B2")
    }

    pub(super) fn store_f16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_f16", "B2")
    }

    pub(super) fn gather_f16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_f16", "B2")
    }

    pub(super) fn load_af16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_af16", "B2")
    }

    pub(super) fn load_af16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_af16_dst", "B2")
    }

    pub(super) fn store_af16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_af16", "B2")
    }

    pub(super) fn gather_af16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_af16", "B2")
    }

    pub(super) fn load_rf16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rf16", "B2")
    }

    pub(super) fn load_rf16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rf16_dst", "B2")
    }

    pub(super) fn store_rf16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_rf16", "B2")
    }

    pub(super) fn gather_rf16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_rf16", "B2")
    }

    pub(super) fn load_rgf16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rgf16", "B2")
    }

    pub(super) fn load_rgf16_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_rgf16_dst", "B2")
    }

    pub(super) fn store_rgf16(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_rgf16", "B2")
    }

    pub(super) fn gather_rgf16(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_rgf16", "B2")
    }

    pub(super) fn load_f32(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_f32", "B2")
    }

    pub(super) fn load_f32_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_f32_dst", "B2")
    }

    pub(super) fn store_f32(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_f32", "B2")
    }

    pub(super) fn gather_f32(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_f32", "B2")
    }

    pub(super) fn load_1010102(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_1010102", "B2")
    }

    pub(super) fn load_1010102_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_1010102_dst", "B2")
    }

    pub(super) fn store_1010102(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_1010102", "B2")
    }

    pub(super) fn gather_1010102(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_1010102", "B2")
    }

    pub(super) fn load_1010102_xr(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_1010102_xr", "B2")
    }

    pub(super) fn load_1010102_xr_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_1010102_xr_dst", "B2")
    }

    pub(super) fn store_1010102_xr(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_1010102_xr", "B2")
    }

    pub(super) fn gather_1010102_xr(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_1010102_xr", "B2")
    }

    pub(super) fn load_10x6(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_10x6", "B2")
    }

    pub(super) fn load_10x6_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_10x6_dst", "B2")
    }

    pub(super) fn store_10x6(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_10x6", "B2")
    }

    pub(super) fn gather_10x6(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_10x6", "B2")
    }

    pub(super) fn gather_10101010_xr(_ctx: &GatherCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("gather_10101010_xr", "B2")
    }

    pub(super) fn load_10101010_xr(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_10101010_xr", "B2")
    }

    pub(super) fn load_10101010_xr_dst(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_10101010_xr_dst", "B2")
    }

    pub(super) fn store_10101010_xr(_ctx: MemoryCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_10101010_xr", "B2")
    }

    pub(super) fn store_src_rg(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_src_rg", "B2")
    }

    pub(super) fn load_src_rg(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_src_rg", "B2")
    }
}
