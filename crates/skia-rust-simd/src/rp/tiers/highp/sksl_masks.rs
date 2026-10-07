// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` control flow and copies: lane/condition/loop/return masks, `case_op`,
//! slot/uniform/constant/immutable/indirect copies, swizzles, `shuffle`, `exchange_src`,
//! `store_device_xy01`.
//!
//! Owner: task B6a (`docs/design/raster-pipeline.md` §5).
//!
//! `SkSL` slots are `F`-strided arrays of `N` lanes each in the memory behind the base pointer
//! (`set_base_pointer`); `SkRPOffset`s are byte offsets from it. Plain copies move bytes (Skia
//! moves `F`/`I32` registers or `memcpy`s, which is bit-exact for every value, including
//! signaling NaNs); the gathers and scatters work on `int`s, as in Skia.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `F`/`I32` register (`N` 32-bit lanes): the stride of `SkSL` slots.
const F_BYTES: usize = 4 * N;

tier_fn! {
    /// `cond_to_mask(iota < *ctx->tail)`: the lanes inside the tail (all lanes outside the tail
    /// chunk, where the tail byte is `0xFF`).
    ///
    /// Out of line for the same reason as `branch_if_all_lanes_active`'s `tail_lanes`: it
    /// depends only on the tail, so LLVM would otherwise hoist it into every chunk's entry.
    #[inline(never)]
    fn lanes_in_tail(tail: u8) -> I32 {
        let tail: U32 = U32::splat(u32::from(tail));
        cond_to_mask(U32::load(&IOTA_U32[..N]).lt_mask(tail).bit_cast())
    }
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L4204-L4207 (chrome/m156)
    /// `execution_mask()`: `sk_bit_cast<I32>(a)`.
    fn execution_mask(p: &Regs) -> I32 {
        p.a.bit_cast()
    }

    /// `update_execution_mask()`: `a = r & g & b` (after updating `r`, `g` or `b`).
    fn update_execution_mask(p: &mut Regs) {
        let r: I32 = p.r.bit_cast();
        let g: I32 = p.g.bit_cast();
        let b: I32 = p.b.bit_cast();
        p.a = (r & g & b).bit_cast();
    }

    /// The `I32` register at byte `offset` of `buf` (`sk_unaligned_load<I32>`).
    fn load_i32(buf: &[u8], offset: usize) -> I32 {
        I32::load_bytes(&buf[offset..offset + F_BYTES])
    }

    /// Stores `v` at byte `offset` of `buf` (`sk_unaligned_store`).
    fn store_i32(v: I32, buf: &mut [u8], offset: usize) {
        v.store_bytes(&mut buf[offset..offset + F_BYTES]);
    }

    /// The `U32` register at byte `offset` of `buf`.
    fn load_u32(buf: &[u8], offset: usize) -> U32 {
        U32::load_bytes(&buf[offset..offset + F_BYTES])
    }

    /// `gather(const int* p, U32 ix)`: lane `i` reads the `int` at index `ix[i]` of `buf`.
    fn gather_i32(buf: &[u8], ix: U32) -> I32 {
        let mut out = I32::splat(0);
        for i in 0..N {
            let o = 4 * ix[i] as usize;
            out[i] = i32::from_ne_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        }
        out
    }

    /// `scatter_masked(I32 src, int* dst, U32 ix, I32 mask)` (the SIMD paths': gather the old
    /// values, select, then write every lane back in lane order).
    fn scatter_masked(src: I32, buf: &mut [u8], ix: U32, mask: I32) {
        let before = gather_i32(buf, ix);
        let after = if_then_else_i(mask, src, before);
        for i in 0..N {
            let o = 4 * ix[i] as usize;
            buf[o..o + 4].copy_from_slice(&after[i].to_ne_bytes());
        }
    }

    /// The clamped, lane-adjusted offsets of the indirect stores/loads:
    /// `min(offsets, limit) * N + iota`.
    fn indirect_lane_offsets(offsets: U32, limit: u32) -> U32 {
        // Clamp the indirect offsets to stay within the limit.
        let mut offsets = min_u(offsets, U32::splat(limit));

        // Scale up the offsets to account for the N lanes per value.
        // (`N` <= 16: the cast is exact.)
        #[allow(clippy::cast_possible_truncation)]
        let n = N as u32;
        offsets *= n;

        // Adjust the offsets forward so that they fetch from or store into the correct lane.
        offsets += U32::load(&IOTA_U32[..N]);
        offsets
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4209-L4215 (chrome/m156)
    pub(super) fn init_lane_masks(p: &mut Regs, e: &mut Params<'_, '_>) {
        let mask: F = lanes_in_tail(e.tail).bit_cast();
        p.r = mask;
        p.g = mask;
        p.b = mask;
        p.a = mask;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4217-L4230 (chrome/m156)
    pub(super) fn store_device_xy01(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        // This is very similar to `seed_shader + store_src`, but b/a are backwards.
        // (sk_FragCoord actually puts w=1 in the w slot.)
        #[allow(clippy::cast_possible_truncation)] // mirrors U32_(dx): size_t → uint32_t
        let (dx, dy) = (e.dx as u32, e.dy as u32);
        let x = cast_f(U32::splat(dx)) + F::load(&IOTA_F[..N]);
        let y = cast_f(U32::splat(dy)) + 0.5;
        let dst = &mut e.ptr_mut(ctx)[..4 * F_BYTES];
        x.store_bytes(dst);
        y.store_bytes(&mut dst[F_BYTES..]);
        F::splat(0.0).store_bytes(&mut dst[2 * F_BYTES..]);
        F::splat(1.0).store_bytes(&mut dst[3 * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4232-L4243 (chrome/m156)
    pub(super) fn exchange_src(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Swaps r,g,b,a registers with the values at `rgba`.
        let rgba = &mut e.ptr_mut(ctx)[..4 * F_BYTES];
        let temp = [p.r, p.g, p.b, p.a];
        p.r = F::load_bytes(rgba);
        p.g = F::load_bytes(&rgba[F_BYTES..]);
        p.b = F::load_bytes(&rgba[2 * F_BYTES..]);
        p.a = F::load_bytes(&rgba[3 * F_BYTES..]);
        for (i, t) in temp.iter().enumerate() {
            t.store_bytes(&mut rgba[i * F_BYTES..]);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4245-L4248 (chrome/m156)
    pub(super) fn load_condition_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.r = F::load_bytes(&e.ptr(ctx)[..F_BYTES]);
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4250-L4252 (chrome/m156)
    pub(super) fn store_condition_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.r.store_bytes(&mut e.ptr_mut(ctx)[..F_BYTES]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4254-L4258 (chrome/m156)
    pub(super) fn merge_condition_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Set the condition-mask to the intersection of two adjacent masks at the pointer.
        let ptr = e.ptr(ctx);
        p.r = (load_i32(ptr, 0) & load_i32(ptr, F_BYTES)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4260-L4264 (chrome/m156)
    pub(super) fn merge_inv_condition_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Set the condition-mask to the intersection of the first mask and the inverse of the
        // second.
        let ptr = e.ptr(ctx);
        p.r = (load_i32(ptr, 0) & !load_i32(ptr, F_BYTES)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4266-L4269 (chrome/m156)
    pub(super) fn load_loop_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.g = F::load_bytes(&e.ptr(ctx)[..F_BYTES]);
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4271-L4273 (chrome/m156)
    pub(super) fn store_loop_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.g.store_bytes(&mut e.ptr_mut(ctx)[..F_BYTES]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4275-L4280 (chrome/m156)
    pub(super) fn mask_off_loop_mask(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // We encountered a break statement. If a lane was active, it should be masked off now,
        // and stay masked-off until the termination of the loop.
        let g: I32 = p.g.bit_cast();
        p.g = (g & !execution_mask(p)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4282-L4286 (chrome/m156)
    pub(super) fn reenable_loop_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Set the loop-mask to the union of the current loop-mask with the mask at the pointer.
        let g: I32 = p.g.bit_cast();
        p.g = (g | load_i32(e.ptr(ctx), 0)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4288-L4293 (chrome/m156)
    pub(super) fn merge_loop_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Set the loop-mask to the intersection of the current loop-mask with the mask at the
        // pointer. (Note: this behavior subtly differs from merge_condition_mask!)
        let g: I32 = p.g.bit_cast();
        p.g = (g & load_i32(e.ptr(ctx), 0)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4295-L4302 (chrome/m156)
    pub(super) fn continue_op(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Set any currently-executing lanes in the continue-mask to true.
        let continue_mask = load_i32(e.ptr(ctx), 0) | execution_mask(p);
        store_i32(continue_mask, e.ptr_mut(ctx), 0);

        // Disable any currently-executing lanes from the loop mask. (Just like
        // `mask_off_loop_mask`.)
        let g: I32 = p.g.bit_cast();
        p.g = (g & !execution_mask(p)).bit_cast();
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4304-L4318 (chrome/m156)
    pub(super) fn case_op(ctx: CaseOpCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        // Check each lane to see if the case value matches the expectation.
        let actual_value = e.base_ptr(ctx.offset);
        let case_matches =
            cond_to_mask(load_i32(e.ptr(actual_value), 0).eq_mask(ctx.expected_value));

        // In lanes where we found a match, enable the loop mask...
        let g: I32 = p.g.bit_cast();
        p.g = (g | case_matches).bit_cast();
        update_execution_mask(p);

        // ... and clear the default-case mask.
        let default_mask = e.ptr_mut(actual_value);
        let cleared = load_i32(default_mask, F_BYTES) & !case_matches;
        store_i32(cleared, default_mask, F_BYTES);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4320-L4323 (chrome/m156)
    pub(super) fn load_return_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.b = F::load_bytes(&e.ptr(ctx)[..F_BYTES]);
        update_execution_mask(p);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4325-L4327 (chrome/m156)
    pub(super) fn store_return_mask(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.b.store_bytes(&mut e.ptr_mut(ctx)[..F_BYTES]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4329-L4334 (chrome/m156)
    pub(super) fn mask_off_return_mask(p: &mut Regs, _e: &mut Params<'_, '_>) {
        // We encountered a return statement. If a lane was active, it should be masked off now,
        // and stay masked-off until the end of the function.
        let b: I32 = p.b.bit_cast();
        p.b = (b & !execution_mask(p)).bit_cast();
        update_execution_mask(p);
    }

    /// Splats `K` uniforms into `K` consecutive slots at `ctx.dst` (`copy_uniform` and its
    /// variants).
    fn copy_n_uniforms<const K: usize>(ctx: &UniformCtx<'_>, e: &mut Params<'_, '_>) {
        let dst = &mut e.ptr_mut(ctx.dst)[..K * F_BYTES];
        for k in 0..K {
            store_i32(I32::splat(ctx.src[k]), dst, k * F_BYTES);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4420-L4424 (chrome/m156)
    pub(super) fn copy_uniform(ctx: &UniformCtx<'_>, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_uniforms::<1>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4425-L4430 (chrome/m156)
    pub(super) fn copy_2_uniforms(ctx: &UniformCtx<'_>, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_uniforms::<2>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4431-L4437 (chrome/m156)
    pub(super) fn copy_3_uniforms(ctx: &UniformCtx<'_>, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_uniforms::<3>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4438-L4445 (chrome/m156)
    pub(super) fn copy_4_uniforms(ctx: &UniformCtx<'_>, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_uniforms::<4>(ctx, e);
    }

    /// Splats `ctx.value` into `K` consecutive slots at `base + ctx.dst`.
    fn splat_n_constants<const K: usize>(ctx: ConstantCtx, e: &mut Params<'_, '_>) {
        let dst = e.base_ptr(ctx.dst);
        let dst = &mut e.ptr_mut(dst)[..K * F_BYTES];
        let value = I32::splat(ctx.value);
        for k in 0..K {
            store_i32(value, dst, k * F_BYTES);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4447-L4452 (chrome/m156)
    pub(super) fn copy_constant(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        splat_n_constants::<1>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4453-L4458 (chrome/m156)
    pub(super) fn splat_2_constants(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        splat_n_constants::<2>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4459-L4464 (chrome/m156)
    pub(super) fn splat_3_constants(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        splat_n_constants::<3>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4465-L4470 (chrome/m156)
    pub(super) fn splat_4_constants(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        splat_n_constants::<4>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4524-L4536 (chrome/m156)
    /// `copy_n_slots_masked_fn`.
    fn copy_n_slots_masked<const K: usize>(ctx: BinaryOpCtx, mask: I32, e: &mut Params<'_, '_>) {
        let base = e.base_ptr(0);
        let buf = e.ptr_mut(base);
        for k in 0..K {
            let dst = ctx.dst as usize + k * F_BYTES;
            let src = ctx.src as usize + k * F_BYTES;
            let v = if_then_else_i(mask, load_i32(buf, src), load_i32(buf, dst));
            store_i32(v, buf, dst);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4538-L4540 (chrome/m156)
    pub(super) fn copy_slot_masked(ctx: BinaryOpCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_masked::<1>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4541-L4543 (chrome/m156)
    pub(super) fn copy_2_slots_masked(ctx: BinaryOpCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_masked::<2>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4544-L4546 (chrome/m156)
    pub(super) fn copy_3_slots_masked(ctx: BinaryOpCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_masked::<3>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4547-L4549 (chrome/m156)
    pub(super) fn copy_4_slots_masked(ctx: BinaryOpCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_masked::<4>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4629-L4651 (chrome/m156)
    pub(super) fn copy_from_indirect_unmasked(ctx: &CopyIndirectCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let offsets = indirect_lane_offsets(load_u32(e.ptr(ctx.indirect_offset), 0), ctx.indirect_limit);

        // Use gather to perform indirect lookups; write the results into `dst`.
        for k in 0..ctx.slots as usize {
            let v = gather_i32(&e.ptr(ctx.src)[k * F_BYTES..], offsets);
            store_i32(v, e.ptr_mut(ctx.dst), k * F_BYTES);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4653-L4668 (chrome/m156)
    pub(super) fn copy_from_indirect_uniform_unmasked(ctx: &CopyIndirectUniformCtx<'_>, _p: &mut Regs, e: &mut Params<'_, '_>) {
        // Clamp the indirect offsets to stay within the limit.
        let offsets = min_u(load_u32(e.ptr(ctx.indirect_offset), 0), U32::splat(ctx.indirect_limit));

        // Use gather to perform indirect lookups; write the results into `dst`.
        for k in 0..ctx.slots as usize {
            let mut v = I32::splat(0);
            for i in 0..N {
                v[i] = ctx.src[k + offsets[i] as usize];
            }
            store_i32(v, e.ptr_mut(ctx.dst), k * F_BYTES);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4670-L4693 (chrome/m156)
    pub(super) fn copy_to_indirect_masked(ctx: &CopyIndirectCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let offsets = indirect_lane_offsets(load_u32(e.ptr(ctx.indirect_offset), 0), ctx.indirect_limit);

        // Perform indirect, masked writes into `dst`.
        let mask = execution_mask(p);
        for k in 0..ctx.slots as usize {
            let src = load_i32(e.ptr(ctx.src), k * F_BYTES);
            scatter_masked(src, &mut e.ptr_mut(ctx.dst)[k * F_BYTES..], offsets, mask);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4695-L4721 (chrome/m156)
    pub(super) fn swizzle_copy_to_indirect_masked(ctx: &SwizzleCopyIndirectCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        let copy = &ctx.copy;
        let offsets = indirect_lane_offsets(load_u32(e.ptr(copy.indirect_offset), 0), copy.indirect_limit);

        // Perform indirect, masked, swizzled writes into `dst`.
        let mask = execution_mask(p);
        for k in 0..copy.slots as usize {
            let src = load_i32(e.ptr(copy.src), k * F_BYTES);
            let swizzle = usize::from(ctx.offsets[k]);
            scatter_masked(src, &mut e.ptr_mut(copy.dst)[swizzle..], offsets, mask);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4472-L4478 (chrome/m156)
    /// `copy_n_slots_unmasked_fn`.
    fn copy_n_slots_unmasked<const K: usize>(ctx: BinaryOpCtx, e: &mut Params<'_, '_>) {
        let base = e.base_ptr(0);
        let src = ctx.src as usize;
        e.ptr_mut(base).copy_within(src..src + K * F_BYTES, ctx.dst as usize);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4480-L4482 (chrome/m156)
    pub(super) fn copy_slot_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_unmasked::<1>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4483-L4485 (chrome/m156)
    pub(super) fn copy_2_slots_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_unmasked::<2>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4486-L4488 (chrome/m156)
    pub(super) fn copy_3_slots_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_unmasked::<3>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4489-L4491 (chrome/m156)
    pub(super) fn copy_4_slots_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_slots_unmasked::<4>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4493-L4509 (chrome/m156)
    /// `copy_n_immutable_unmasked_fn`: broadcasts `K` consecutive scalars into `K` slots.
    fn copy_n_immutables_unmasked<const K: usize>(ctx: BinaryOpCtx, e: &mut Params<'_, '_>) {
        let base = e.base_ptr(0);
        let buf = e.ptr_mut(base);
        // Load the scalar values.
        let mut values = [0_i32; K];
        for (index, value) in values.iter_mut().enumerate() {
            let o = ctx.src as usize + 4 * index;
            *value = i32::from_ne_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        }
        // Broadcast the scalars into the destination.
        for (index, value) in values.iter().enumerate() {
            store_i32(I32::splat(*value), buf, ctx.dst as usize + index * F_BYTES);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4511-L4513 (chrome/m156)
    pub(super) fn copy_immutable_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_immutables_unmasked::<1>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4514-L4516 (chrome/m156)
    pub(super) fn copy_2_immutables_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_immutables_unmasked::<2>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4517-L4519 (chrome/m156)
    pub(super) fn copy_3_immutables_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_immutables_unmasked::<3>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4520-L4522 (chrome/m156)
    pub(super) fn copy_4_immutables_unmasked(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        copy_n_immutables_unmasked::<4>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4605-L4614 (chrome/m156)
    /// `swizzle_copy_masked_fn`.
    fn swizzle_copy_masked<const K: usize>(ctx: &SwizzleCopyCtx, mask: I32, e: &mut Params<'_, '_>) {
        for k in 0..K {
            let src = load_i32(e.ptr(ctx.src), k * F_BYTES);
            let dst = e.ptr_mut(ctx.dst);
            let o = usize::from(ctx.offsets[k]);
            let v = if_then_else_i(mask, src, load_i32(dst, o));
            store_i32(v, dst, o);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4616-L4618 (chrome/m156)
    pub(super) fn swizzle_copy_slot_masked(ctx: &SwizzleCopyCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        swizzle_copy_masked::<1>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4619-L4621 (chrome/m156)
    pub(super) fn swizzle_copy_2_slots_masked(ctx: &SwizzleCopyCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        swizzle_copy_masked::<2>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4622-L4624 (chrome/m156)
    pub(super) fn swizzle_copy_3_slots_masked(ctx: &SwizzleCopyCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        swizzle_copy_masked::<3>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4625-L4627 (chrome/m156)
    pub(super) fn swizzle_copy_4_slots_masked(ctx: &SwizzleCopyCtx, p: &mut Regs, e: &mut Params<'_, '_>) {
        swizzle_copy_masked::<4>(ctx, execution_mask(p), e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4551-L4581 (chrome/m156)
    /// `shuffle_fn`: slot `k` of `buf` becomes the slot at byte `offsets[k]` (read before any
    /// write), for `k < offsets.len()`. (Skia always loads `LoopCount` offsets and stores
    /// `numSlots` of them; the extra loads have no effect.)
    fn shuffle_fn(buf: &mut [u8], offsets: &[usize]) {
        let mut scratch = [[0_u8; F_BYTES]; 16];
        for (slot, &offset) in scratch.iter_mut().zip(offsets) {
            slot.copy_from_slice(&buf[offset..offset + F_BYTES]);
        }
        for (k, slot) in scratch.iter().take(offsets.len()).enumerate() {
            buf[k * F_BYTES..(k + 1) * F_BYTES].copy_from_slice(slot);
        }
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4583-L4587 (chrome/m156)
    /// `small_swizzle_fn`.
    fn small_swizzle<const K: usize>(ctx: SwizzleCtx, e: &mut Params<'_, '_>) {
        let dst = e.base_ptr(ctx.dst);
        let mut offsets = [0_usize; K];
        for (offset, &o) in offsets.iter_mut().zip(&ctx.offsets) {
            *offset = usize::from(o);
        }
        shuffle_fn(e.ptr_mut(dst), &offsets);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4589-L4591 (chrome/m156)
    pub(super) fn swizzle_1(ctx: SwizzleCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        small_swizzle::<1>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4592-L4594 (chrome/m156)
    pub(super) fn swizzle_2(ctx: SwizzleCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        small_swizzle::<2>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4595-L4597 (chrome/m156)
    pub(super) fn swizzle_3(ctx: SwizzleCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        small_swizzle::<3>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4598-L4600 (chrome/m156)
    pub(super) fn swizzle_4(ctx: SwizzleCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        small_swizzle::<4>(ctx, e);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4601-L4603 (chrome/m156)
    pub(super) fn shuffle(ctx: &ShuffleCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let count = usize::try_from(ctx.count).expect("shuffle: negative count").min(16);
        let mut offsets = [0_usize; 16];
        for (offset, &o) in offsets.iter_mut().zip(&ctx.offsets) {
            *offset = usize::from(o);
        }
        shuffle_fn(e.ptr_mut(ctx.ptr), &offsets[..count]);
    }
}
