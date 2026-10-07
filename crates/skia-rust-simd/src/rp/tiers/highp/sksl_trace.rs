// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` trace ops and `callback`.
//!
//! Owner: task B6d (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn callback(_ctx: &CallbackCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("callback", "B6d")
    }

    pub(super) fn trace_line(_ctx: &TraceLineCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("trace_line", "B6d")
    }

    pub(super) fn trace_var(_ctx: &TraceVarCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("trace_var", "B6d")
    }

    pub(super) fn trace_enter(_ctx: &TraceFuncCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("trace_enter", "B6d")
    }

    pub(super) fn trace_exit(_ctx: &TraceFuncCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("trace_exit", "B6d")
    }

    pub(super) fn trace_scope(_ctx: &TraceScopeCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("trace_scope", "B6d")
    }
}
