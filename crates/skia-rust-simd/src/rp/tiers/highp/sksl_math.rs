// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` transcendental functions: `sin`…`atan2`, `pow`/`exp`/`log`, `sqrt`, `invsqrt`,
//! `inverse_mat2/3/4`.
//!
//! Owner: task B6c (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn invsqrt_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("invsqrt_float", "B6c")
    }

    pub(super) fn invsqrt_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("invsqrt_2_floats", "B6c")
    }

    pub(super) fn invsqrt_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("invsqrt_3_floats", "B6c")
    }

    pub(super) fn invsqrt_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("invsqrt_4_floats", "B6c")
    }

    pub(super) fn inverse_mat2(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("inverse_mat2", "B6c")
    }

    pub(super) fn inverse_mat3(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("inverse_mat3", "B6c")
    }

    pub(super) fn inverse_mat4(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("inverse_mat4", "B6c")
    }

    pub(super) fn sin_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sin_float", "B6c")
    }

    pub(super) fn cos_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cos_float", "B6c")
    }

    pub(super) fn tan_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("tan_float", "B6c")
    }

    pub(super) fn asin_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("asin_float", "B6c")
    }

    pub(super) fn acos_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("acos_float", "B6c")
    }

    pub(super) fn atan_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("atan_float", "B6c")
    }

    pub(super) fn atan2_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("atan2_n_floats", "B6c")
    }

    pub(super) fn sqrt_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sqrt_float", "B6c")
    }

    pub(super) fn pow_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("pow_n_floats", "B6c")
    }

    pub(super) fn exp_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("exp_float", "B6c")
    }

    pub(super) fn exp2_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("exp2_float", "B6c")
    }

    pub(super) fn log_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("log_float", "B6c")
    }

    pub(super) fn log2_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("log2_float", "B6c")
    }
}
