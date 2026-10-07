// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Basic stages: `seed_shader`, `load_src`/`store_src`/`load_dst`/`store_dst`, register moves,
//! the stack ops and `set_base_pointer`.
//!
//! Owner: task A3 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `F` register (`N` floats).
const F_BYTES: usize = 4 * N;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2300-L2314 (chrome/m156)
    pub(super) fn seed_shader(p: &mut Regs, e: &mut Params<'_, '_>) {
        // It's important for speed to explicitly cast(dx) and cast(dy),
        // which has the effect of splatting them to vectors before converting to floats.
        // On Intel this breaks a data dependency on previous loop iterations' registers.
        #[allow(clippy::cast_possible_truncation)] // mirrors U32_(dx): size_t → uint32_t
        let (dx, dy) = (e.dx as u32, e.dy as u32);
        p.r = cast_f(U32::splat(dx)) + F::load(&IOTA_F[..N]);
        p.g = cast_f(U32::splat(dy)) + 0.5;
        p.b = F::splat(1.0); // This is w=1 for matrix multiplies by the device coords.
        p.a = F::splat(0.0);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2381-L2388 (chrome/m156)
    /// Loads registers `r,g,b,a` from the context (mirrors `store_src`).
    pub(super) fn load_src(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &e.ptr(ctx)[..4 * F_BYTES];
        p.r = F::load_bytes(ptr);
        p.g = F::load_bytes(&ptr[F_BYTES..]);
        p.b = F::load_bytes(&ptr[2 * F_BYTES..]);
        p.a = F::load_bytes(&ptr[3 * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2389-L2395 (chrome/m156)
    /// Stores registers `r,g,b,a` into the context (mirrors `load_src`).
    pub(super) fn store_src(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &mut e.ptr_mut(ctx)[..4 * F_BYTES];
        p.r.store_bytes(ptr);
        p.g.store_bytes(&mut ptr[F_BYTES..]);
        p.b.store_bytes(&mut ptr[2 * F_BYTES..]);
        p.a.store_bytes(&mut ptr[3 * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2406-L2410 (chrome/m156)
    /// Stores register `a` into the context.
    pub(super) fn store_src_a(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.a.store_bytes(e.ptr_mut(ctx));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2411-L2418 (chrome/m156)
    /// Loads registers `dr,dg,db,da` from the context (mirrors `store_dst`).
    pub(super) fn load_dst(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &e.ptr(ctx)[..4 * F_BYTES];
        p.dr = F::load_bytes(ptr);
        p.dg = F::load_bytes(&ptr[F_BYTES..]);
        p.db = F::load_bytes(&ptr[2 * F_BYTES..]);
        p.da = F::load_bytes(&ptr[3 * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2419-L2425 (chrome/m156)
    /// Stores registers `dr,dg,db,da` into the context (mirrors `load_dst`).
    pub(super) fn store_dst(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &mut e.ptr_mut(ctx)[..4 * F_BYTES];
        p.dr.store_bytes(ptr);
        p.dg.store_bytes(&mut ptr[F_BYTES..]);
        p.db.store_bytes(&mut ptr[2 * F_BYTES..]);
        p.da.store_bytes(&mut ptr[3 * F_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2681-L2686 (chrome/m156)
    pub(super) fn move_src_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.r;
        p.dg = p.g;
        p.db = p.b;
        p.da = p.a;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2687-L2692 (chrome/m156)
    pub(super) fn move_dst_src(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.dr;
        p.g = p.dg;
        p.b = p.db;
        p.a = p.da;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2693-L2698 (chrome/m156)
    pub(super) fn swap_src_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.r, &mut p.dr);
        core::mem::swap(&mut p.g, &mut p.dg);
        core::mem::swap(&mut p.b, &mut p.db);
        core::mem::swap(&mut p.a, &mut p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L1897-L1990 (chrome/m156)
    /// `stack_checkpoint`: a no-op. In Skia it re-enters the stage chain after each
    /// `stack_rewind` to reclaim the C++ stack of long `SkSL` programs (every stage tail-calls the
    /// next); the interpreter's loop uses no stack per stage, so it only continues with the next
    /// stage, as Skia does after the rewind.
    pub(super) fn stack_checkpoint(_p: &mut Regs, _e: &mut Params<'_, '_>) {}

    /// `stack_rewind`: a no-op (see `stack_checkpoint`): Skia saves the registers and the
    /// program counter, unwinds to the checkpoint, restores them and continues with the stage
    /// after the rewind, which is what falling through does.
    pub(super) fn stack_rewind(_p: &mut Regs, _e: &mut Params<'_, '_>) {}

    // Port of: src/opts/SkRasterPipeline_opts.h#L4194-L4196 (chrome/m156)
    pub(super) fn set_base_pointer(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        e.base = Some(ctx);
    }
}
