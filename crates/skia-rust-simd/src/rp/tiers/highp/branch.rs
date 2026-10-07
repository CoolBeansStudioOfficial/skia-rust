// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Branch stages (`HIGHP_BRANCH_STAGE`): `branch_if_*` and `jump`. Each returns the offset to
//! add to the program counter (`1` continues with the next stage).
//!
//! The `SkSL` control-flow stages keep their masks in the common registers: `r` is the
//! condition mask, `g` the loop mask, `b` the return mask and `a` the execution mask (the
//! intersection of all three).
//!
//! Owner: task A3 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

tier_fn! {
    /// `cond_to_mask(*ctx->tail <= iota)`: the lanes past the tail (all lanes outside the tail
    /// chunk, where the tail byte is `0xFF`).
    ///
    /// Out of line: it depends only on the tail, so LLVM would otherwise hoist it into every
    /// chunk's entry, for every program.
    #[inline(never)]
    fn tail_lanes(tail: u8) -> I32 {
        let tail: U32 = U32::splat(u32::from(tail));
        cond_to_mask(tail.le_mask(U32::load(&IOTA_U32[..N])).bit_cast())
    }
}

si! {
    /// `execution_mask()`: `sk_bit_cast<I32>(a)`.
    fn execution_mask(p: &Regs) -> I32 {
        p.a.bit_cast()
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4336-L4342 (chrome/m156)
    /// Branches if every lane that exists in this chunk is active: lanes past the tail are
    /// _never_ active, so they are excluded.
    pub(super) fn branch_if_all_lanes_active(
        ctx: BranchCtx,
        p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) -> i32 {
        if all(execution_mask(p) | tail_lanes(e.tail)) { ctx.offset } else { 1 }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4344-L4346 (chrome/m156)
    pub(super) fn branch_if_any_lanes_active(
        ctx: BranchCtx,
        p: &mut Regs,
        _e: &mut Params<'_, '_>,
    ) -> i32 {
        if any(execution_mask(p)) { ctx.offset } else { 1 }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4348-L4350 (chrome/m156)
    pub(super) fn branch_if_no_lanes_active(
        ctx: BranchCtx,
        p: &mut Regs,
        _e: &mut Params<'_, '_>,
    ) -> i32 {
        if any(execution_mask(p)) { 1 } else { ctx.offset }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4352-L4352 (chrome/m156)
    pub(super) fn jump(ctx: BranchCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) -> i32 {
        ctx.offset
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4354-L4361 (chrome/m156)
    pub(super) fn branch_if_no_active_lanes_eq(
        ctx: &BranchIfEqualCtx,
        p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) -> i32 {
        // Compare each lane against the expected value...
        let mut matches = cond_to_mask(I32::load_bytes(e.ptr(ctx.ptr)).eq_mask(ctx.value));
        // ... but mask off lanes that aren't executing.
        matches &= execution_mask(p);
        // If any lanes matched, don't take the branch.
        if any(matches) { 1 } else { ctx.offset }
    }
}
