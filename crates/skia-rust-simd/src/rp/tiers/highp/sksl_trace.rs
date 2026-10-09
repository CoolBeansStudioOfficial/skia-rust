// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` trace ops and `callback`.
//!
//! Owner: task B6d (`docs/design/raster-pipeline.md` §5). The trace ops report to a
//! [`TraceHook`](crate::rp::contexts::TraceHook), the temporary stand-in for `SkSL::TraceHook`
//! until an `SkSL` crate exists.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `F`/`I32` register (`N` lanes of 32 bits).
const REG_BYTES: usize = 4 * N;

si! {
    /// `*(const I32*)ctx->traceMask`.
    fn load_trace_mask(e: &Params<'_, '_>, trace_mask: MemPtr) -> I32 {
        I32::load_bytes(e.ptr(trace_mask))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4204 (chrome/m156)
    /// `execution_mask()`: the `a` register as an `I32`.
    fn execution_mask(p: &Regs) -> I32 {
        p.a.bit_cast()
    }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L4188-L4192 (chrome/m156)
    /// `callback`: hands the active pixels' `r,g,b,a` to the callback (`store4`'s layout:
    /// interleaved per pixel, on every tier) and loads back whatever it leaves in the array.
    pub(super) fn callback(ctx: &CallbackCtx<'_>, p: &mut Regs, _e: &mut Params<'_, '_>) {
        let mut rgba = [0.0f32; 4 * MAX_STRIDE_HIGHP];
        // store4(c->rgba, r,g,b,a)
        for lane in 0..N {
            rgba[4 * lane] = p.r[lane];
            rgba[4 * lane + 1] = p.g[lane];
            rgba[4 * lane + 2] = p.b[lane];
            rgba[4 * lane + 3] = p.a[lane];
        }
        // c->fn(c, N)
        (ctx.callback)(&mut rgba, N);
        // load4(c->read_from, &r,&g,&b,&a)
        for lane in 0..N {
            p.r[lane] = rgba[4 * lane];
            p.g[lane] = rgba[4 * lane + 1];
            p.b[lane] = rgba[4 * lane + 2];
            p.a[lane] = rgba[4 * lane + 3];
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4363-L4368 (chrome/m156)
    pub(super) fn trace_line(ctx: &TraceLineCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let trace_mask = load_trace_mask(e, ctx.trace_mask);
        if any(execution_mask(p) & trace_mask) {
            ctx.trace_hook.line(ctx.line_number);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4394-L4418 (chrome/m156)
    pub(super) fn trace_var(ctx: &TraceVarCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let trace_mask = load_trace_mask(e, ctx.trace_mask);
        let mask = execution_mask(p) & trace_mask;
        if any(mask) {
            for lane in 0..N {
                if mask[lane] != 0 {
                    let mut data = ctx.data;
                    let mut slot_idx = ctx.slot_idx;
                    if let Some(indirect) = ctx.indirect_offset {
                        // If this was an indirect store, apply the indirect-offset to the data
                        // pointer.
                        let offsets = U32::load_bytes(e.ptr(indirect));
                        let indirect_offset = offsets[lane].min(ctx.indirect_limit);
                        // `data += indirectOffset` advances by whole `I32` registers.
                        #[allow(clippy::cast_possible_truncation)] // REG_BYTES <= 64
                        let bytes = indirect_offset
                            .checked_mul(REG_BYTES as u32)
                            .expect("raster pipeline: trace_var indirect offset overflow");
                        data = data.add(bytes);
                        // `slotIdx += indirectOffset` (int += uint32_t wraps).
                        #[allow(clippy::cast_possible_wrap)] // mirrors the C++ conversion
                        let delta = indirect_offset as i32;
                        slot_idx = slot_idx.wrapping_add(delta);
                    }
                    for i in 0..usize::try_from(ctx.num_slots).unwrap_or(0) {
                        // `select_lane(*data, lane)`
                        let value = I32::load_bytes(&e.ptr(data)[i * REG_BYTES..])[lane];
                        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                        // mirrors `++slotIdx`; slot counts are tiny
                        let slot = slot_idx.wrapping_add(i as i32);
                        ctx.trace_hook.var(slot, value);
                    }
                    break;
                }
            }
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4370-L4375 (chrome/m156)
    pub(super) fn trace_enter(ctx: &TraceFuncCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let trace_mask = load_trace_mask(e, ctx.trace_mask);
        if any(execution_mask(p) & trace_mask) {
            ctx.trace_hook.enter(ctx.func_idx);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4377-L4382 (chrome/m156)
    pub(super) fn trace_exit(ctx: &TraceFuncCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let trace_mask = load_trace_mask(e, ctx.trace_mask);
        if any(execution_mask(p) & trace_mask) {
            ctx.trace_hook.exit(ctx.func_idx);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4384-L4392 (chrome/m156)
    pub(super) fn trace_scope(ctx: &TraceScopeCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        // Note that trace_scope intentionally does not incorporate the execution mask. Otherwise,
        // the scopes would become unbalanced if the execution mask changed in the middle of a
        // block. The caller is responsible for providing a combined trace- and execution-mask.
        let trace_mask = load_trace_mask(e, ctx.trace_mask);
        if any(trace_mask) {
            ctx.trace_hook.scope(ctx.delta);
        }
    }
}
