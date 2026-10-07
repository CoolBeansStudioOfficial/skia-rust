// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Basic lowp stages: `seed_shader`, `load_src`/`store_src`/`load_dst`/`store_dst` and the
//! register moves.
//!
//! Owner: task A3 (`docs/design/raster-pipeline.md` §5).

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one `U16` register (`N` lanes).
const U16_BYTES: usize = 2 * N;

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L6088-L6115 (chrome/m156) (non-LSX branch)
    pub(super) fn seed_shader(x: &mut F, y: &mut F, e: &mut Params<'_, '_>) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // I32_(dx)
        let (dx, dy) = (e.dx as i32, e.dy as i32);
        // `cast<F>(I32_(dx))` converts a splat: the same as splatting the converted scalar.
        #[allow(clippy::cast_precision_loss)] // mirrors cast<F>(I32): int → float
        let (fx, fy) = (dx as f32, dy as f32);
        *x = F::splat(fx) + F::load(&IOTA_F[..N]);
        *y = F::splat(fy) + 0.5;
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6205-L6224 (chrome/m156)
    pub(super) fn move_src_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.dr = p.r;
        p.dg = p.g;
        p.db = p.b;
        p.da = p.a;
    }

    pub(super) fn move_dst_src(p: &mut Regs, _e: &mut Params<'_, '_>) {
        p.r = p.dr;
        p.g = p.dg;
        p.b = p.db;
        p.a = p.da;
    }

    pub(super) fn swap_src_dst(p: &mut Regs, _e: &mut Params<'_, '_>) {
        core::mem::swap(&mut p.r, &mut p.dr);
        core::mem::swap(&mut p.g, &mut p.dg);
        core::mem::swap(&mut p.b, &mut p.db);
        core::mem::swap(&mut p.a, &mut p.da);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6796-L6801 (chrome/m156)
    pub(super) fn load_src(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &e.ptr(ctx)[..4 * U16_BYTES];
        p.r = U16::load_bytes(ptr);
        p.g = U16::load_bytes(&ptr[U16_BYTES..]);
        p.b = U16::load_bytes(&ptr[2 * U16_BYTES..]);
        p.a = U16::load_bytes(&ptr[3 * U16_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6802-L6807 (chrome/m156)
    pub(super) fn store_src(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &mut e.ptr_mut(ctx)[..4 * U16_BYTES];
        p.r.store_bytes(ptr);
        p.g.store_bytes(&mut ptr[U16_BYTES..]);
        p.b.store_bytes(&mut ptr[2 * U16_BYTES..]);
        p.a.store_bytes(&mut ptr[3 * U16_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6808-L6810 (chrome/m156)
    pub(super) fn store_src_a(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        p.a.store_bytes(e.ptr_mut(ctx));
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6811-L6816 (chrome/m156)
    pub(super) fn load_dst(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &e.ptr(ctx)[..4 * U16_BYTES];
        p.dr = U16::load_bytes(ptr);
        p.dg = U16::load_bytes(&ptr[U16_BYTES..]);
        p.db = U16::load_bytes(&ptr[2 * U16_BYTES..]);
        p.da = U16::load_bytes(&ptr[3 * U16_BYTES..]);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L6817-L6822 (chrome/m156)
    pub(super) fn store_dst(ctx: MemPtr, p: &mut Regs, e: &mut Params<'_, '_>) {
        let ptr = &mut e.ptr_mut(ctx)[..4 * U16_BYTES];
        p.dr.store_bytes(ptr);
        p.dg.store_bytes(&mut ptr[U16_BYTES..]);
        p.db.store_bytes(&mut ptr[2 * U16_BYTES..]);
        p.da.store_bytes(&mut ptr[3 * U16_BYTES..]);
    }
}
