// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` control flow and copies: lane/condition/loop/return masks, `case_op`,
//! slot/uniform/constant/immutable/indirect copies, swizzles, `shuffle`, `exchange_src`,
//! `store_device_xy01`.
//!
//! Owner: task B6a (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L4209-L4215 (chrome/m156)
    // (Minimal version, needed by the B6d trace tests; B6a owns this file.)
    pub(super) fn init_lane_masks(p: &mut Regs, e: &mut Params<'_, '_>) {
        let tail = U32::splat(u32::from(e.tail));
        let mask: I32 = cond_to_mask(U32::load(&IOTA_U32[..N]).lt_mask(tail).bit_cast());
        let mask: F = mask.bit_cast();
        p.r = mask;
        p.g = mask;
        p.b = mask;
        p.a = mask;
    }

    pub(super) fn store_device_xy01(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_device_xy01", "B6a")
    }

    pub(super) fn exchange_src(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("exchange_src", "B6a")
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4245-L4248 (chrome/m156)
    // (Minimal version, needed by the B6d trace tests; B6a owns this file.)
    pub(super) fn load_condition_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.r = F::load_bytes(e.ptr(ctx));
        // update_execution_mask()
        let (cond, lp, ret): (I32, I32, I32) = (p.r.bit_cast(), p.g.bit_cast(), p.b.bit_cast());
        p.a = (cond & lp & ret).bit_cast();
    }

    pub(super) fn store_condition_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_condition_mask", "B6a")
    }

    pub(super) fn merge_condition_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("merge_condition_mask", "B6a")
    }

    pub(super) fn merge_inv_condition_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("merge_inv_condition_mask", "B6a")
    }

    pub(super) fn load_loop_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_loop_mask", "B6a")
    }

    pub(super) fn store_loop_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_loop_mask", "B6a")
    }

    pub(super) fn mask_off_loop_mask(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mask_off_loop_mask", "B6a")
    }

    pub(super) fn reenable_loop_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("reenable_loop_mask", "B6a")
    }

    pub(super) fn merge_loop_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("merge_loop_mask", "B6a")
    }

    pub(super) fn case_op(_ctx: CaseOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("case_op", "B6a")
    }

    pub(super) fn continue_op(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("continue_op", "B6a")
    }

    pub(super) fn load_return_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("load_return_mask", "B6a")
    }

    pub(super) fn store_return_mask(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("store_return_mask", "B6a")
    }

    pub(super) fn mask_off_return_mask(_p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mask_off_return_mask", "B6a")
    }

    pub(super) fn copy_uniform(_ctx: &UniformCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_uniform", "B6a")
    }

    pub(super) fn copy_2_uniforms(_ctx: &UniformCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_2_uniforms", "B6a")
    }

    pub(super) fn copy_3_uniforms(_ctx: &UniformCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_3_uniforms", "B6a")
    }

    pub(super) fn copy_4_uniforms(_ctx: &UniformCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_4_uniforms", "B6a")
    }

    pub(super) fn copy_constant(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_constant", "B6a")
    }

    pub(super) fn splat_2_constants(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("splat_2_constants", "B6a")
    }

    pub(super) fn splat_3_constants(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("splat_3_constants", "B6a")
    }

    pub(super) fn splat_4_constants(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("splat_4_constants", "B6a")
    }

    pub(super) fn copy_slot_masked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_slot_masked", "B6a")
    }

    pub(super) fn copy_2_slots_masked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_2_slots_masked", "B6a")
    }

    pub(super) fn copy_3_slots_masked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_3_slots_masked", "B6a")
    }

    pub(super) fn copy_4_slots_masked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_4_slots_masked", "B6a")
    }

    pub(super) fn copy_from_indirect_unmasked(_ctx: &CopyIndirectCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_from_indirect_unmasked", "B6a")
    }

    pub(super) fn copy_from_indirect_uniform_unmasked(_ctx: &CopyIndirectUniformCtx<'_>, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_from_indirect_uniform_unmasked", "B6a")
    }

    pub(super) fn copy_to_indirect_masked(_ctx: &CopyIndirectCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_to_indirect_masked", "B6a")
    }

    pub(super) fn swizzle_copy_to_indirect_masked(_ctx: &SwizzleCopyIndirectCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_copy_to_indirect_masked", "B6a")
    }

    pub(super) fn copy_slot_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_slot_unmasked", "B6a")
    }

    pub(super) fn copy_2_slots_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_2_slots_unmasked", "B6a")
    }

    pub(super) fn copy_3_slots_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_3_slots_unmasked", "B6a")
    }

    pub(super) fn copy_4_slots_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_4_slots_unmasked", "B6a")
    }

    pub(super) fn copy_immutable_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_immutable_unmasked", "B6a")
    }

    pub(super) fn copy_2_immutables_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_2_immutables_unmasked", "B6a")
    }

    pub(super) fn copy_3_immutables_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_3_immutables_unmasked", "B6a")
    }

    pub(super) fn copy_4_immutables_unmasked(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("copy_4_immutables_unmasked", "B6a")
    }

    pub(super) fn swizzle_copy_slot_masked(_ctx: &SwizzleCopyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_copy_slot_masked", "B6a")
    }

    pub(super) fn swizzle_copy_2_slots_masked(_ctx: &SwizzleCopyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_copy_2_slots_masked", "B6a")
    }

    pub(super) fn swizzle_copy_3_slots_masked(_ctx: &SwizzleCopyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_copy_3_slots_masked", "B6a")
    }

    pub(super) fn swizzle_copy_4_slots_masked(_ctx: &SwizzleCopyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_copy_4_slots_masked", "B6a")
    }

    pub(super) fn swizzle_1(_ctx: SwizzleCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_1", "B6a")
    }

    pub(super) fn swizzle_2(_ctx: SwizzleCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_2", "B6a")
    }

    pub(super) fn swizzle_3(_ctx: SwizzleCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_3", "B6a")
    }

    pub(super) fn swizzle_4(_ctx: SwizzleCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("swizzle_4", "B6a")
    }

    pub(super) fn shuffle(_ctx: &ShuffleCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("shuffle", "B6a")
    }
}
