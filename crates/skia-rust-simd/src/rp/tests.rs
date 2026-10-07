// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the op table, programs, tail patching and the A3 stages:
//!
//! - the op list matches `SkRasterPipelineOpList.h` (order, counts, names, lowp set);
//! - lowp/highp choice and program layout (`buildLowpPipeline`/`buildHighpPipeline`);
//! - `MemoryCtx` registration (`uncheckedAppend`/`addMemoryContext`), tail patching and the
//!   persistence of stale scratch lanes across runs of a compiled program (design §1.7, R8);
//! - known answers for the A3 stages on every selection this host can run (Scalar, the native
//!   x86 tiers, and their models, also under Miri);
//! - stage twins: native vs `Model(Host)` vs `Model(AmdZen4)`, bit for bit, on random and
//!   special lanes (design §2.8).

// Under Miri only Scalar and the AmdZen4 models run, leaving some helpers unused.
#![cfg_attr(miri, allow(dead_code))]
// Test data is built from small indices; the casts are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::float_cmp
)]

use super::contexts::{
    BinaryOpCtx, BranchCtx, BranchIfEqualCtx, CaseOpCtx, ConstantCtx, CopyIndirectCtx,
    CopyIndirectUniformCtx, MemoryCtxInfo, ShuffleCtx, SwizzleCopyCtx, SwizzleCopyIndirectCtx,
    SwizzleCtx, UniformCtx,
};
use super::lanes::test_support::{Rng, float_specials, mad_nan_ambiguous};
use super::memory::{MemoryCtxPatch, patch_memory_contexts, restore_memory_contexts};
use super::{
    MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, NO_TAIL, NUM_HIGHP_OPS, NUM_LOWP_OPS, Op,
    Params, Program, Stage, memory_ctx_infos,
};
use crate::tier::{Backend, Estimates, Selection, Tier};

/// The SIMD tiers.
const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

/// A tier's model estimate sources: the host's, then the oracle's (`AmdZen4` on x86, `Arm`).
fn estimate_sources(t: Tier) -> [Estimates; 2] {
    if t == Tier::Neon {
        [Estimates::Host, Estimates::Arm]
    } else {
        [Estimates::Host, Estimates::AmdZen4]
    }
}

/// The models of `t` this host can run (`Model(Host)` needs the host's estimate instructions,
/// and is not run under Miri).
fn models(t: Tier) -> Vec<Selection> {
    estimate_sources(t)
        .into_iter()
        .map(|e| Selection::model(t, e))
        .filter(|s| {
            s.check().is_ok() && !(cfg!(miri) && s.backend == Backend::Model(Estimates::Host))
        })
        .collect()
}

/// Every selection this host can run.
fn selections() -> Vec<Selection> {
    let mut v = vec![Selection::native(Tier::Scalar)];
    for t in SIMD {
        if !cfg!(miri) && t.is_native() {
            v.push(Selection::native(t));
        }
        v.extend(models(t));
    }
    v
}

/// The SIMD tiers with a native backend on this host, each with its models.
fn twin_sets() -> Vec<(Selection, Vec<Selection>)> {
    if cfg!(miri) {
        return Vec::new();
    }
    SIMD.into_iter()
        .filter(|t| t.is_native())
        .map(|t| (Selection::native(t), models(t)))
        .collect()
}

const IN0: MemPtr = MemPtr::new(MemSlot(0), 0);
const IN1: MemPtr = MemPtr::new(MemSlot(1), 0);
const OUT0: MemPtr = MemPtr::new(MemSlot(2), 0);
const OUT1: MemPtr = MemPtr::new(MemSlot(3), 0);

/// Bytes of four registers of the widest tier.
const REGS_BYTES: usize = 4 * 4 * 16;

/// Runs `stages` once over `w` pixels at `(x, y)` with four 256-byte buffers bound to slots 0–3
/// (`in0`, `in1` are the inputs) and returns the two output buffers.
fn run4(
    stages: &[Stage<'_>],
    sel: Selection,
    force_highp: bool,
    at: (usize, usize, usize),
    in0: &[u8],
    in1: &[u8],
) -> (Program<'static>, [u8; REGS_BYTES], [u8; REGS_BYTES]) {
    let mut a = [0u8; REGS_BYTES];
    let mut b = [0u8; REGS_BYTES];
    a[..in0.len()].copy_from_slice(in0);
    b[..in1.len()].copy_from_slice(in1);
    let mut out0 = [0u8; REGS_BYTES];
    let mut out1 = [0u8; REGS_BYTES];
    // Contexts here are values, so the program does not borrow anything.
    let owned: Vec<Stage<'static>> = stages.iter().map(|s| to_static(*s)).collect();
    let mut program = Program::new(&owned, sel, force_highp);
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&a))
        .with(MemSlot(1), MemView::read(&b))
        .with(MemSlot(2), MemView::write(&mut out0))
        .with(MemSlot(3), MemView::write(&mut out1));
    program.run(at.0, at.1, at.2, 1, &mut mem);
    drop(mem);
    (program, out0, out1)
}

/// The by-value stages used in these tests, with the `'static` lifetime.
fn to_static(s: Stage<'_>) -> Stage<'static> {
    match s {
        Stage::LoadSrc(p) => Stage::LoadSrc(p),
        Stage::StoreSrc(p) => Stage::StoreSrc(p),
        Stage::LoadDst(p) => Stage::LoadDst(p),
        Stage::StoreDst(p) => Stage::StoreDst(p),
        Stage::StoreSrcA(p) => Stage::StoreSrcA(p),
        Stage::SeedShader => Stage::SeedShader,
        Stage::MoveSrcDst => Stage::MoveSrcDst,
        Stage::MoveDstSrc => Stage::MoveDstSrc,
        Stage::SwapSrcDst => Stage::SwapSrcDst,
        Stage::Srcover => Stage::Srcover,
        Stage::Jump(c) => Stage::Jump(c),
        Stage::BranchIfAnyLanesActive(c) => Stage::BranchIfAnyLanesActive(c),
        Stage::BranchIfNoLanesActive(c) => Stage::BranchIfNoLanesActive(c),
        Stage::BranchIfAllLanesActive(c) => Stage::BranchIfAllLanesActive(c),
        Stage::StackRewind => Stage::StackRewind,
        Stage::SetBasePointer(p) => Stage::SetBasePointer(p),
        other => panic!("not used by these tests: {other:?}"),
    }
}

/// `N` floats as bytes.
fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

/// Bytes as floats.
fn floats(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

/// Bytes as 32-bit words.
fn words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect()
}

/// Bytes as 16-bit words.
fn halves(b: &[u8]) -> Vec<u16> {
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes(*c))
        .collect()
}

/// The highp stride a program for `sel` runs with.
fn highp_n(sel: Selection) -> usize {
    sel.tier.highp_stride()
}

// ~~~ The op list ~~~

#[test]
fn op_list_matches_skia() {
    // SkRasterPipelineOpList.h at chrome/m156.
    assert_eq!(NUM_LOWP_OPS, 109);
    assert_eq!(NUM_HIGHP_OPS, 527);
    assert_eq!(Op::ALL.len(), NUM_HIGHP_OPS);
    for (i, op) in Op::ALL.iter().enumerate() {
        assert_eq!(*op as usize, i);
        assert_eq!(op.has_lowp(), i < NUM_LOWP_OPS, "{}", op.name());
    }
    let name = |i: usize| Op::ALL[i].name();
    assert_eq!(name(0), "move_src_dst");
    assert_eq!(name(NUM_LOWP_OPS - 1), "debug_a_255");
    assert_eq!(name(NUM_LOWP_OPS), "callback");
    assert_eq!(name(NUM_HIGHP_OPS - 1), "trace_scope");
    assert_eq!(Op::Srcover.name(), "srcover");
    assert_eq!(Op::PQish.name(), "PQish");
    assert_eq!(Op::Plus.name(), "plus_");
    assert_eq!(Op::InitLaneMasks.name(), "init_lane_masks");
    assert_eq!(Op::Srcover.task(), "A3");
    assert_eq!(Op::Load8888.task(), "B1");
    // Every name is unique.
    let mut names: Vec<_> = Op::ALL.iter().map(|o| o.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), NUM_HIGHP_OPS);
}

#[test]
fn stages_are_small() {
    // Two words: the interpreter walks a dense array of stages.
    assert_eq!(size_of::<Stage<'_>>(), 16);
    assert_eq!(size_of::<super::tiers::Instr<'_>>(), 16);
}

#[test]
fn stage_op_round_trips() {
    assert_eq!(Stage::Srcover.op(), Op::Srcover);
    assert_eq!(Stage::LoadSrc(IN0).op(), Op::LoadSrc);
    assert_eq!(Stage::Jump(BranchCtx { offset: 1 }).op(), Op::Jump);
}

// ~~~ Programs ~~~

#[test]
fn lowp_or_highp() {
    let lowp_stages = [Stage::SeedShader, Stage::Srcover, Stage::StoreSrc(OUT0)];
    let scalar = Selection::native(Tier::Scalar);
    for sel in selections() {
        let p = Program::new(&lowp_stages, sel, false);
        assert_eq!(p.is_lowp(), sel.tier != Tier::Scalar, "{sel}");
        assert!(!Program::new(&lowp_stages, sel, true).is_lowp(), "{sel}");
        // A highp-only op forces highp.
        let highp = [Stage::Srcover, Stage::Jump(BranchCtx { offset: 1 })];
        assert!(!Program::new(&highp, sel, false).is_lowp(), "{sel}");
    }
    assert!(!Program::new(&lowp_stages, scalar, false).is_lowp());
    assert!(super::has_lowp(Tier::Sse2, Op::Srcover));
    assert!(!super::has_lowp(Tier::Scalar, Op::Srcover));
    assert!(!super::has_lowp(Tier::Sse2, Op::Callback));
}

#[test]
fn stack_rewind_forces_highp_and_a_checkpoint() {
    for sel in selections() {
        let p = Program::new(&[Stage::Srcover], sel, false);
        assert_eq!(p.len(), 2); // srcover, just_return
        let p = Program::new(&[Stage::Srcover, Stage::StackRewind], sel, false);
        assert!(!p.is_lowp());
        assert_eq!(p.len(), 4); // stack_checkpoint, srcover, stack_rewind, just_return
    }
}

#[test]
fn memory_ctx_registration() {
    let dst = MemoryCtx::new(MemSlot(0));
    let src = MemoryCtx::new(MemSlot(1));
    let coverage = MemoryCtx::new(MemSlot(2));
    let stages = [
        Stage::Load8888(src),
        Stage::Load8888Dst(dst),
        Stage::ScaleU8(coverage),
        Stage::Srcover,
        Stage::Store8888(dst),
    ];
    let info = |context, bytes_per_pixel, load, store| MemoryCtxInfo {
        context,
        bytes_per_pixel,
        load,
        store,
    };
    assert_eq!(
        memory_ctx_infos(&stages),
        [
            info(src, 4, true, false),
            info(dst, 4, true, true),
            info(coverage, 1, true, false)
        ]
    );
    // Emboss registers `add` before `mul`.
    let emboss = [Stage::Emboss(super::contexts::EmbossCtx {
        mul: MemoryCtx::new(MemSlot(5)),
        add: MemoryCtx::new(MemSlot(6)),
    })];
    assert_eq!(
        memory_ctx_infos(&emboss),
        [
            info(MemoryCtx::new(MemSlot(6)), 1, true, false),
            info(MemoryCtx::new(MemSlot(5)), 1, true, false)
        ]
    );
    // Gathers and SkSL memory are not patched.
    assert_eq!(
        memory_ctx_infos(&[Stage::StoreSrc(OUT0), Stage::Srcover]),
        []
    );
}

// ~~~ Tail patching ~~~

#[test]
fn patching_copies_only_the_tail_and_keeps_stale_lanes() {
    let ctx = MemoryCtx::new(MemSlot(0));
    let info = MemoryCtxInfo {
        context: ctx,
        bytes_per_pixel: 4,
        load: true,
        store: true,
    };
    let mut patches = [MemoryCtxPatch::new(info)];
    // Two rows of 6 pixels, stride 6; pixel (x, y) holds bytes [10y + x; 4].
    let mut pixels: Vec<u8> = (0..2 * 6)
        .flat_map(|i: u8| [10 * (i / 6) + i % 6; 4])
        .collect();
    let mut views = [Some(MemView::write(&mut pixels).with_stride(6))];

    // Row 0: a tail of 3 pixels at x = 3.
    patch_memory_contexts(&views, &mut patches, 3, 0, 3);
    assert_eq!(
        &patches[0].scratch[..12],
        &[3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5]
    );
    assert!(patches[0].scratch[12..].iter().all(|&b| b == 0));
    {
        let mut e = Params {
            dx: 3,
            dy: 0,
            tail: 3,
            base: None,
            views: &mut views,
            patches: &mut patches,
        };
        // During the tail the context addresses the scratch buffer.
        assert_eq!(e.ptr_at_xy(ctx, 4)[..4], [3, 3, 3, 3]);
        // A stage stores N pixels; only the tail goes back to memory.
        e.ptr_at_xy_mut(ctx, 4)[..16].copy_from_slice(&[0xA0; 16]);
    }
    restore_memory_contexts(&mut views, &patches, 3, 0, 3);
    let bytes = views[0].as_ref().unwrap().bytes();
    assert_eq!(&bytes[..12], &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
    assert!(bytes[12..24].iter().all(|&b| b == 0xA0));
    assert_eq!(&bytes[24..28], &[10, 10, 10, 10]);

    // Row 1: a tail of 1 pixel at x = 5. Lanes past it keep the previous tail's bytes.
    patch_memory_contexts(&views, &mut patches, 5, 1, 1);
    assert_eq!(&patches[0].scratch[..4], &[15, 15, 15, 15]);
    assert!(patches[0].scratch[4..16].iter().all(|&b| b == 0xA0));
    restore_memory_contexts(&mut views, &patches, 5, 1, 1);

    // Outside the tail the context addresses memory.
    let e = Params {
        dx: 2,
        dy: 1,
        tail: NO_TAIL,
        base: None,
        views: &mut views,
        patches: &mut patches,
    };
    assert_eq!(e.ptr_at_xy(ctx, 4)[..4], [12, 12, 12, 12]);
}

#[test]
fn origin_and_stride_address_pixels_like_ptr_at_xy() {
    // A "fake base" one row and two pixels before the bytes (as Skia's sprite blitter builds).
    let bytes: Vec<u8> = (0..32).collect();
    let mut views = [Some(
        MemView::read(&bytes)
            .with_stride(4)
            .with_origin(-(4 + 2) * 2),
    )];
    let ctx = MemoryCtx::new(MemSlot(0));
    let e = Params {
        dx: 3,
        dy: 2,
        tail: NO_TAIL,
        base: None,
        views: &mut views,
        patches: &mut [],
    };
    // (3, 2) is pixel (1, 1) of the bytes: 2 * (4 + 1) = 10.
    assert_eq!(e.ptr_at_xy(ctx, 2)[0], 10);
    assert_eq!(e.ptr(MemPtr::new(MemSlot(0), 5))[0], 5);
}

#[test]
#[should_panic(expected = "lies before the bound memory")]
fn pixels_before_the_memory_panic() {
    let bytes = [0u8; 8];
    let mut views = [Some(MemView::read(&bytes).with_origin(-4))];
    let e = Params {
        dx: 0,
        dy: 0,
        tail: NO_TAIL,
        base: None,
        views: &mut views,
        patches: &mut [],
    };
    let _ = e.ptr_at_xy(MemoryCtx::new(MemSlot(0)), 1);
}

#[test]
#[should_panic(expected = "no memory bound")]
fn unbound_memory_panics() {
    let _ = run4(
        &[Stage::StoreSrc(MemPtr::new(MemSlot(9), 0))],
        Selection::native(Tier::Scalar),
        false,
        (0, 0, 1),
        &[],
        &[],
    );
}

/// A compiled program's scratch buffers persist across runs: the stale lanes past a shorter
/// tail still hold the previous tail's pixels (Skia zeroes them once per `compile()`).
#[test]
fn compiled_programs_keep_stale_scratch_lanes() {
    let ctx = MemoryCtx::new(MemSlot(0));
    // load_8888 is registered (and patched) but jumped over: no unported stage runs.
    let stages = [Stage::Jump(BranchCtx { offset: 2 }), Stage::Load8888(ctx)];
    for sel in selections() {
        let n = highp_n(sel); // highp: jump has no lowp version
        if n == 1 {
            continue; // no tails on Scalar
        }
        let mut program = Program::new(&stages, sel, false);
        let mut row1: Vec<u8> = (0..(n + 3) * 4).map(|i| (i / 4) as u8 + 1).collect();
        program.run(
            0,
            0,
            n + 3,
            1,
            &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut row1)),
        );
        let s = &program.patches[0].scratch;
        let expect1: Vec<u8> = (n..n + 3).flat_map(|i| [i as u8 + 1; 4]).collect();
        assert_eq!(&s[..12], &expect1[..], "{sel}");
        assert!(s[12..].iter().all(|&b| b == 0), "{sel}");

        let mut row2: Vec<u8> = vec![0xEE; (n + 1) * 4];
        program.run(
            0,
            0,
            n + 1,
            1,
            &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut row2)),
        );
        let s = &program.patches[0].scratch;
        assert_eq!(&s[..4], &[0xEE; 4], "{sel}");
        assert_eq!(&s[4..12], &expect1[4..12], "{sel}: stale lanes");

        // `run()` semantics: a fresh program starts from zeroed scratch.
        let mut fresh = Program::new(&stages, sel, false);
        fresh.run(
            0,
            0,
            n + 1,
            1,
            &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut row2)),
        );
        assert!(
            fresh.patches[0].scratch[4..].iter().all(|&b| b == 0),
            "{sel}"
        );
    }
}

/// A store-registered context writes its scratch back over the tail even when no stage stores
/// (as Skia's `restore_memory_contexts` does).
#[test]
fn store_contexts_write_back_the_tail() {
    let ctx = MemoryCtx::new(MemSlot(0));
    let stages = [Stage::Jump(BranchCtx { offset: 2 }), Stage::Store8888(ctx)];
    for sel in selections() {
        let n = highp_n(sel);
        let mut row = vec![7u8; (2 * n + 1) * 4];
        let mut program = Program::new(&stages, sel, false);
        program.run(
            0,
            0,
            2 * n + 1,
            1,
            &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut row)),
        );
        if n == 1 {
            assert!(row.iter().all(|&b| b == 7), "{sel}");
        } else {
            assert!(row[..2 * n * 4].iter().all(|&b| b == 7), "{sel}");
            assert!(row[2 * n * 4..].iter().all(|&b| b == 0), "{sel}");
        }
    }
}

// ~~~ Known answers ~~~

#[test]
fn seed_shader_highp() {
    for sel in selections() {
        let n = highp_n(sel);
        let (_, out, _) = run4(
            &[Stage::SeedShader, Stage::StoreSrc(OUT0)],
            sel,
            true,
            (5, 9, 1),
            &[],
            &[],
        );
        let f = floats(&out);
        for i in 0..n {
            assert_eq!(f[i], 5.5 + i as f32, "{sel} r[{i}]");
            assert_eq!(f[n + i], 9.5, "{sel} g[{i}]");
            assert_eq!(f[2 * n + i], 1.0, "{sel} b[{i}]");
            assert_eq!(f[3 * n + i].to_bits(), 0, "{sel} a[{i}]");
        }
    }
}

#[test]
fn seed_shader_lowp() {
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        let (p, out, _) = run4(
            &[Stage::SeedShader, Stage::StoreSrc(OUT0)],
            sel,
            false,
            (5, 9, 1),
            &[],
            &[],
        );
        assert!(p.is_lowp());
        // x is split into r,g and y into b,a: the stored bytes are x then y.
        let f = floats(&out[..8 * n]);
        for i in 0..n {
            assert_eq!(f[i], 5.5 + i as f32, "{sel} x[{i}]");
            assert_eq!(f[n + i], 9.5, "{sel} y[{i}]");
        }
    }
}

#[test]
fn load_store_and_moves_highp() {
    for sel in selections() {
        let n = highp_n(sel);
        let src: Vec<f32> = (0..4 * n).map(|i| i as f32 + 0.25).collect();
        let dst: Vec<f32> = (0..4 * n).map(|i| -(i as f32)).collect();
        let (s, d) = (f32_bytes(&src), f32_bytes(&dst));
        let w = 4 * 4 * n;
        let go = |stages: &[Stage<'_>]| {
            let (_, o0, o1) = run4(stages, sel, true, (0, 0, n), &s, &d);
            (o0[..w].to_vec(), o1[..w].to_vec())
        };
        let ld = [Stage::LoadSrc(IN0), Stage::LoadDst(IN1)];
        let st = [Stage::StoreSrc(OUT0), Stage::StoreDst(OUT1)];
        let with = |mid: &[Stage<'static>]| [&ld[..], mid, &st[..]].concat();
        assert_eq!(go(&with(&[])), (s.clone(), d.clone()), "{sel}");
        assert_eq!(
            go(&with(&[Stage::MoveSrcDst])),
            (s.clone(), s.clone()),
            "{sel}"
        );
        assert_eq!(
            go(&with(&[Stage::MoveDstSrc])),
            (d.clone(), d.clone()),
            "{sel}"
        );
        assert_eq!(
            go(&with(&[Stage::SwapSrcDst])),
            (d.clone(), s.clone()),
            "{sel}"
        );
        // store_src_a writes only a.
        let (_, o0, _) = run4(
            &[Stage::LoadSrc(IN0), Stage::StoreSrcA(OUT0)],
            sel,
            true,
            (0, 0, n),
            &s,
            &d,
        );
        assert_eq!(o0[..4 * n], s[3 * 4 * n..], "{sel}");
    }
}

#[test]
fn load_store_and_moves_lowp() {
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        let src: Vec<u8> = (0..4 * n)
            .flat_map(|i| (i as u16 * 3 + 1).to_ne_bytes())
            .collect();
        let dst: Vec<u8> = (0..4 * n)
            .flat_map(|i| (i as u16 * 5 + 2).to_ne_bytes())
            .collect();
        let w = 4 * 2 * n;
        let go = |mid: &[Stage<'static>]| {
            let stages = [
                &[Stage::LoadSrc(IN0), Stage::LoadDst(IN1)][..],
                mid,
                &[Stage::StoreSrc(OUT0), Stage::StoreDst(OUT1)],
            ]
            .concat();
            let (p, o0, o1) = run4(&stages, sel, false, (0, 0, n), &src, &dst);
            assert!(p.is_lowp());
            (o0[..w].to_vec(), o1[..w].to_vec())
        };
        assert_eq!(go(&[]), (src.clone(), dst.clone()), "{sel}");
        assert_eq!(
            go(&[Stage::MoveSrcDst]),
            (src.clone(), src.clone()),
            "{sel}"
        );
        assert_eq!(
            go(&[Stage::MoveDstSrc]),
            (dst.clone(), dst.clone()),
            "{sel}"
        );
        assert_eq!(
            go(&[Stage::SwapSrcDst]),
            (dst.clone(), src.clone()),
            "{sel}"
        );
    }
}

#[test]
fn srcover_highp() {
    for sel in selections() {
        let n = highp_n(sel);
        // src = (0.5, 0.25, 0, 0.5), dst = (1, 0.5, 0.25, 1) in every lane.
        let src: Vec<f32> = [0.5f32, 0.25, 0.0, 0.5]
            .iter()
            .flat_map(|&v| vec![v; n])
            .collect();
        let dst: Vec<f32> = [1.0f32, 0.5, 0.25, 1.0]
            .iter()
            .flat_map(|&v| vec![v; n])
            .collect();
        let stages = [
            Stage::LoadSrc(IN0),
            Stage::LoadDst(IN1),
            Stage::Srcover,
            Stage::StoreSrc(OUT0),
        ];
        let (_, out, _) = run4(
            &stages,
            sel,
            true,
            (0, 0, n),
            &f32_bytes(&src),
            &f32_bytes(&dst),
        );
        let f = floats(&out);
        // s + d*(1 - sa)
        for (c, want) in [1.0f32, 0.5, 0.125, 1.0].iter().enumerate() {
            assert!(
                f[c * n..(c + 1) * n].iter().all(|v| v == want),
                "{sel} channel {c}: {f:?}"
            );
        }
    }
}

#[test]
fn srcover_lowp() {
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        let reg = |vals: [u16; 4]| -> Vec<u8> {
            vals.iter()
                .flat_map(|&v| vec![v; n])
                .flat_map(u16::to_ne_bytes)
                .collect()
        };
        let stages = [
            Stage::LoadSrc(IN0),
            Stage::LoadDst(IN1),
            Stage::Srcover,
            Stage::StoreSrc(OUT0),
        ];
        let (p, out, _) = run4(
            &stages,
            sel,
            false,
            (0, 0, n),
            &reg([100, 7, 0, 128]),
            &reg([255, 30, 200, 255]),
        );
        assert!(p.is_lowp());
        let h = halves(&out);
        // s + div255_accurate(d * (255 - sa)), with div255_accurate(v) = (v+128 + (v+128)/256)/256
        let div = |v: u32| ((v + 128) + (v + 128) / 256) / 256;
        let want = |s: u32, d: u32| (s + div(d * 127)) as u16;
        for (c, w) in [want(100, 255), want(7, 30), want(0, 200), want(128, 255)]
            .iter()
            .enumerate()
        {
            assert!(
                h[c * n..(c + 1) * n].iter().all(|v| v == w),
                "{sel} channel {c}: {h:?}"
            );
        }
    }
}

/// `a` = the execution mask: lane `i` active iff `active[i]`.
fn mask_regs(n: usize, active: impl Fn(usize) -> bool) -> Vec<u8> {
    let mut regs = vec![0u32; 4 * n];
    for i in 0..n {
        regs[i] = 0x3f80_0000; // r = 1.0
        regs[3 * n + i] = if active(i) { !0 } else { 0 };
    }
    regs.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

/// Runs `load_src, <branch>, swap_src_dst, store_src` over `w` pixels and reports whether the
/// branch skipped the swap (the stored `r` is still 1.0).
fn branch_taken(sel: Selection, branch: Stage<'static>, regs: &[u8], w: usize) -> bool {
    let stages = [
        Stage::LoadSrc(IN0),
        branch,
        Stage::SwapSrcDst,
        Stage::StoreSrc(OUT0),
    ];
    let (_, out, _) = run4(&stages, sel, false, (0, 0, w), regs, &[]);
    let r = floats(&out)[0];
    assert!(r == 1.0 || r == 0.0);
    r == 1.0
}

#[test]
fn jump_and_branches() {
    let to = BranchCtx { offset: 2 };
    for sel in selections() {
        let n = highp_n(sel);
        let none = mask_regs(n, |_| false);
        let all = mask_regs(n, |_| true);
        let first = mask_regs(n, |i| i == 0);
        assert!(branch_taken(sel, Stage::Jump(to), &none, n), "{sel}");
        assert!(
            !branch_taken(sel, Stage::Jump(BranchCtx { offset: 1 }), &none, n),
            "{sel}"
        );

        let any = Stage::BranchIfAnyLanesActive(to);
        assert!(!branch_taken(sel, any, &none, n), "{sel}");
        assert!(branch_taken(sel, any, &all, n), "{sel}");
        assert!(branch_taken(sel, any, &first, n), "{sel}");

        let no = Stage::BranchIfNoLanesActive(to);
        assert!(branch_taken(sel, no, &none, n), "{sel}");
        assert!(!branch_taken(sel, no, &first, n), "{sel}");

        let all_active = Stage::BranchIfAllLanesActive(to);
        assert!(branch_taken(sel, all_active, &all, n), "{sel}");
        assert!(!branch_taken(sel, all_active, &none, n), "{sel}");
        // Full chunk: lanes 1.. are inactive.
        assert_eq!(branch_taken(sel, all_active, &first, n), n == 1, "{sel}");
        // Tail chunk of one pixel: lanes past the tail are excluded.
        assert!(branch_taken(sel, all_active, &first, 1), "{sel}");
    }
}

#[test]
fn branch_if_no_active_lanes_eq() {
    for sel in selections() {
        let n = highp_n(sel);
        // Lane values 0, 1, 2, …; branch unless an active lane equals `value`.
        let values: Vec<u8> = (0..n as i32).flat_map(i32::to_ne_bytes).collect();
        for (value, active_lane, taken) in [(0, 0, false), (0, usize::MAX, true), (5, 0, true)] {
            let regs = mask_regs(n, |i| i == active_lane);
            let ctx = BranchIfEqualCtx {
                offset: 2,
                value,
                ptr: IN1,
            };
            let stages = [
                Stage::LoadSrc(IN0),
                Stage::BranchIfNoActiveLanesEq(&ctx),
                Stage::SwapSrcDst,
                Stage::StoreSrc(OUT0),
            ];
            let mut out = [0u8; REGS_BYTES];
            let mut program = Program::new(&stages, sel, false);
            program.run(
                0,
                0,
                n,
                1,
                &mut MemoryBindings::new()
                    .with(MemSlot(0), MemView::read(&regs))
                    .with(MemSlot(1), MemView::read(&values))
                    .with(MemSlot(2), MemView::write(&mut out)),
            );
            assert_eq!(floats(&out)[0] == 1.0, taken, "{sel} value {value}");
        }
    }
}

#[test]
fn branches_loop_backwards() {
    // A two-iteration loop: jump back over the swap once, using the pipeline as a counter is not
    // possible with A3 stages, so check that a negative offset lands on the right stage: start
    // with a forward jump to the backward branch, which returns to the swap.
    let stages = [
        Stage::LoadSrc(IN0),
        Stage::Jump(BranchCtx { offset: 3 }),
        Stage::SwapSrcDst,
        Stage::Jump(BranchCtx { offset: 2 }),
        Stage::Jump(BranchCtx { offset: -2 }),
        Stage::StoreSrc(OUT0),
    ];
    for sel in selections() {
        let n = highp_n(sel);
        // load, jump → [4] jump -2 → [2] swap → [3] jump +2 → [5] store: r = dst's 0.
        let (_, out, _) = run4(&stages, sel, false, (0, 0, n), &mask_regs(n, |_| true), &[]);
        assert_eq!(floats(&out)[0], 0.0, "{sel}");
    }
}

#[test]
fn set_base_pointer_and_stack_ops_run() {
    for sel in selections() {
        let n = highp_n(sel);
        let stages = [
            Stage::SetBasePointer(IN1),
            Stage::LoadSrc(IN0),
            Stage::StackRewind,
            Stage::StoreSrc(OUT0),
        ];
        let regs = mask_regs(n, |_| true);
        let (p, out, _) = run4(&stages, sel, false, (0, 0, n), &regs, &[]);
        assert!(!p.is_lowp());
        assert_eq!(out[..regs.len()], regs[..], "{sel}");
    }
}

#[test]
fn rectangles_cover_every_pixel_once() {
    // seed_shader + store_src at every chunk position: the last chunk of each row wins, so the
    // stored x of lane 0 is the last chunk's dx + 0.5 and g is the last row.
    for sel in selections() {
        let n = highp_n(sel);
        for w in [1, n - 1 + usize::from(n == 1), n, n + 1, 3 * n + 2] {
            let mut out = [0u8; REGS_BYTES];
            let mut program = Program::new(&[Stage::SeedShader, Stage::StoreSrc(OUT0)], sel, true);
            program.run(
                2,
                3,
                w,
                2,
                &mut MemoryBindings::new().with(MemSlot(2), MemView::write(&mut out)),
            );
            let last_dx = 2 + (w - 1) / n * n;
            let f = floats(&out);
            assert_eq!(f[0], last_dx as f32 + 0.5, "{sel} w={w}");
            assert_eq!(f[n], 4.5, "{sel} w={w}");
        }
    }
}

// ~~~ Stage twins ~~~

/// Random lane bits: mostly special floats, some random patterns.
fn random_lanes(rng: &mut Rng, specials: &[u32], count: usize) -> Vec<u32> {
    (0..count)
        .map(|_| {
            if rng.below(4) == 0 {
                rng.next_u32()
            } else {
                rng.pick(specials)
            }
        })
        .collect()
}

#[test]
fn highp_stage_twins() {
    let specials = float_specials();
    let mid: [&[Stage<'static>]; 6] = [
        &[],
        &[Stage::Srcover],
        &[Stage::MoveSrcDst],
        &[Stage::MoveDstSrc],
        &[Stage::SwapSrcDst],
        &[Stage::SeedShader],
    ];
    for (native, models) in twin_sets() {
        let n = highp_n(native);
        let mut rng = Rng::new(0x005e_eda3);
        for round in 0..400 {
            let src = random_lanes(&mut rng, &specials, 4 * n);
            let dst = random_lanes(&mut rng, &specials, 4 * n);
            let (sb, db): (Vec<u8>, Vec<u8>) = (
                src.iter().flat_map(|w| w.to_ne_bytes()).collect(),
                dst.iter().flat_map(|w| w.to_ne_bytes()).collect(),
            );
            for m in mid {
                let stages = [
                    &[Stage::LoadSrc(IN0), Stage::LoadDst(IN1)][..],
                    m,
                    &[Stage::StoreSrc(OUT0), Stage::StoreDst(OUT1)],
                ]
                .concat();
                let at = (rng.below(1 << 20), rng.below(1 << 20), n);
                let (_, n0, n1) = run4(&stages, native, true, at, &sb, &db);
                for model in &models {
                    let (_, m0, m1) = run4(&stages, *model, true, at, &sb, &db);
                    let (nw, mw) = (words(&n0[..16 * n]), words(&m0[..16 * n]));
                    for i in 0..4 * n {
                        let lane = i % n;
                        // srcover: mad(d, 1 - sa, s); two NaNs meeting in a commutative op may
                        // come out in either order (design §2.4, "NaN payloads").
                        let ambiguous = matches!(m, [Stage::Srcover]) && {
                            let sa = f32::from_bits(src[3 * n + lane]);
                            let inv = (1.0 - sa).to_bits();
                            mad_nan_ambiguous(dst[i], inv, src[i], true)
                        };
                        if ambiguous {
                            assert_eq!(
                                f32::from_bits(nw[i]).is_nan(),
                                f32::from_bits(mw[i]).is_nan()
                            );
                        } else {
                            assert_eq!(
                                nw[i], mw[i],
                                "{native} vs {model}, {m:?}, round {round}, word {i}"
                            );
                        }
                    }
                    assert_eq!(n1, m1, "{native} vs {model}, {m:?}, round {round}");
                }
            }
        }
    }
}

#[test]
fn lowp_stage_twins() {
    for (native, models) in twin_sets() {
        let n = native.tier.lowp_stride().unwrap();
        let mut rng = Rng::new(0x10_a3);
        for round in 0..400 {
            let lanes = |rng: &mut Rng| -> Vec<u8> {
                (0..4 * n)
                    .flat_map(|_| {
                        // Mostly [0, 255] (colors), sometimes any 16 bits.
                        let v = if rng.below(4) == 0 {
                            rng.next_u32() as u16
                        } else {
                            (rng.next_u32() & 0xFF) as u16
                        };
                        v.to_ne_bytes()
                    })
                    .collect()
            };
            let (sb, db) = (lanes(&mut rng), lanes(&mut rng));
            for m in [
                &[][..],
                &[Stage::Srcover],
                &[Stage::SwapSrcDst],
                &[Stage::SeedShader],
            ] {
                let stages = [
                    &[Stage::LoadSrc(IN0), Stage::LoadDst(IN1)][..],
                    m,
                    &[Stage::StoreSrc(OUT0), Stage::StoreDst(OUT1)],
                ]
                .concat();
                let at = (rng.below(1 << 20), rng.below(1 << 20), n);
                let (p, n0, n1) = run4(&stages, native, false, at, &sb, &db);
                assert!(p.is_lowp());
                for model in &models {
                    let (_, m0, m1) = run4(&stages, *model, false, at, &sb, &db);
                    assert_eq!(
                        (n0, n1),
                        (m0, m1),
                        "{native} vs {model}, {m:?}, round {round}"
                    );
                }
            }
        }
    }
}

#[test]
fn model_backends_match_on_every_host() {
    // The two estimate sources agree on A3's estimate-free stages; run under Miri too.
    let host_ok = |t: Tier| Selection::model(t, Estimates::Host).check().is_ok() && !cfg!(miri);
    for t in [Tier::Sse2, Tier::Sse41] {
        let zen = Selection::model(t, Estimates::AmdZen4);
        let n = highp_n(zen);
        let src: Vec<f32> = (0..4 * n).map(|i| i as f32 / 7.0).collect();
        let dst: Vec<f32> = (0..4 * n).map(|i| 1.0 - i as f32 / 13.0).collect();
        let stages = [
            Stage::LoadSrc(IN0),
            Stage::LoadDst(IN1),
            Stage::Srcover,
            Stage::StoreSrc(OUT0),
        ];
        let (_, z, _) = run4(
            &stages,
            zen,
            true,
            (0, 0, n),
            &f32_bytes(&src),
            &f32_bytes(&dst),
        );
        let (_, s, _) = run4(
            &stages,
            Selection::native(Tier::Scalar),
            true,
            (0, 0, 1),
            &f32_bytes(&[src[0], src[n], src[2 * n], src[3 * n]]),
            &f32_bytes(&[dst[0], dst[n], dst[2 * n], dst[3 * n]]),
        );
        // Lane 0 of each channel equals Scalar's result (srcover has no tier differences on
        // ordinary values: mad is unfused on Scalar, Sse2 and Sse41).
        let (zf, sf) = (floats(&z), floats(&s));
        for c in 0..4 {
            assert_eq!(zf[c * n].to_bits(), sf[c].to_bits());
        }
        if host_ok(t) {
            let (_, h, _) = run4(
                &stages,
                Selection::model(t, Estimates::Host),
                true,
                (0, 0, n),
                &f32_bytes(&src),
                &f32_bytes(&dst),
            );
            assert_eq!(h, z);
        }
    }
}

// ~~~ B6a: SkSL masks, branches and copies ~~~

/// The contexts of the B6a stages that are not plain values, for a highp stride of `n`.
struct SkslCtxs {
    uniform: UniformCtx<'static>,
    from_indirect: CopyIndirectCtx,
    from_indirect_uniform: CopyIndirectUniformCtx<'static>,
    to_indirect: CopyIndirectCtx,
    swizzle_to_indirect: SwizzleCopyIndirectCtx,
    shuffle: ShuffleCtx,
    swizzle_copy: SwizzleCopyCtx,
}

/// Uniform data of `SkslCtxs`.
static SKSL_UNIFORMS: [i32; 8] = [0x7fbf_ffff, -1, 0, 1, 0x1234_5678, -77, i32::MIN, i32::MAX];

/// Read-only data (slot 0), the initial `SkSL` slots (slot 3) and the registers (slot 1) of the
/// B6a twin tests.
const SKSL_DATA_BYTES: usize = 2048;

impl SkslCtxs {
    fn new(n: usize) -> SkslCtxs {
        // One slot is `4 * n` bytes; `SkSL` slots live in slot 3, read-only data in slot 0.
        let sb = u32::try_from(4 * n).unwrap();
        let data = |offset: u32| MemPtr::new(MemSlot(0), offset);
        let slots = |offset: u32| MemPtr::new(MemSlot(3), offset);
        let sb16 = u16::try_from(sb).unwrap();
        SkslCtxs {
            uniform: UniformCtx {
                dst: slots(5 * sb),
                src: &SKSL_UNIFORMS[..4],
            },
            from_indirect: CopyIndirectCtx {
                dst: slots(20 * sb),
                src: slots(0),
                indirect_offset: data(0),
                indirect_limit: 4,
                slots: 3,
            },
            from_indirect_uniform: CopyIndirectUniformCtx {
                dst: slots(20 * sb),
                src: &SKSL_UNIFORMS,
                indirect_offset: data(0),
                indirect_limit: 4,
                slots: 3,
            },
            to_indirect: CopyIndirectCtx {
                dst: slots(8 * sb),
                src: slots(0),
                indirect_offset: data(0),
                indirect_limit: 4,
                slots: 3,
            },
            swizzle_to_indirect: SwizzleCopyIndirectCtx {
                copy: CopyIndirectCtx {
                    dst: slots(8 * sb),
                    src: data(3 * sb),
                    indirect_offset: data(0),
                    indirect_limit: 2,
                    slots: 3,
                },
                offsets: [2 * sb16, 0, sb16, 3 * sb16],
            },
            shuffle: ShuffleCtx {
                ptr: slots(4 * sb),
                count: 7,
                offsets: [
                    6 * sb16,
                    0,
                    3 * sb16,
                    3 * sb16,
                    sb16,
                    15 * sb16,
                    8 * sb16,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            },
            swizzle_copy: SwizzleCopyCtx {
                dst: slots(12 * sb),
                src: data(sb),
                offsets: [3 * sb16, 0, 2 * sb16, sb16],
            },
        }
    }
}

/// Every B6a stage (one list entry each) for a highp stride of `n`.
fn sksl_stages(n: usize, c: &SkslCtxs) -> Vec<Stage<'_>> {
    let sb = u32::try_from(4 * n).unwrap();
    let sb8 = u8::try_from(sb).unwrap();
    let data = |offset: u32| MemPtr::new(MemSlot(0), offset);
    let slots = |offset: u32| MemPtr::new(MemSlot(3), offset);
    let binary = BinaryOpCtx {
        dst: 9 * sb,
        src: 2 * sb,
    };
    let constant = ConstantCtx {
        value: 0x7fbf_ffff,
        dst: 6 * sb,
    };
    let swizzle = SwizzleCtx {
        dst: 3 * sb,
        offsets: [3 * sb8, 3 * sb8, sb8, 0],
    };
    vec![
        Stage::InitLaneMasks,
        Stage::StoreDeviceXy01(slots(10 * sb)),
        Stage::ExchangeSrc(slots(2 * sb)),
        Stage::LoadConditionMask(data(sb)),
        Stage::StoreConditionMask(slots(sb)),
        Stage::MergeConditionMask(data(2 * sb)),
        Stage::MergeInvConditionMask(data(2 * sb)),
        Stage::LoadLoopMask(data(sb)),
        Stage::StoreLoopMask(slots(sb)),
        Stage::MaskOffLoopMask,
        Stage::ReenableLoopMask(data(3 * sb)),
        Stage::MergeLoopMask(data(3 * sb)),
        Stage::CaseOp(CaseOpCtx {
            expected_value: SKSL_UNIFORMS[4],
            offset: 6 * sb,
        }),
        Stage::ContinueOp(slots(7 * sb)),
        Stage::LoadReturnMask(data(sb)),
        Stage::StoreReturnMask(slots(sb)),
        Stage::MaskOffReturnMask,
        Stage::CopyUniform(&c.uniform),
        Stage::Copy2Uniforms(&c.uniform),
        Stage::Copy3Uniforms(&c.uniform),
        Stage::Copy4Uniforms(&c.uniform),
        Stage::CopyConstant(constant),
        Stage::Splat2Constants(constant),
        Stage::Splat3Constants(constant),
        Stage::Splat4Constants(constant),
        Stage::CopySlotMasked(binary),
        Stage::Copy2SlotsMasked(binary),
        Stage::Copy3SlotsMasked(binary),
        Stage::Copy4SlotsMasked(binary),
        Stage::CopyFromIndirectUnmasked(&c.from_indirect),
        Stage::CopyFromIndirectUniformUnmasked(&c.from_indirect_uniform),
        Stage::CopyToIndirectMasked(&c.to_indirect),
        Stage::SwizzleCopyToIndirectMasked(&c.swizzle_to_indirect),
        Stage::CopySlotUnmasked(binary),
        Stage::Copy2SlotsUnmasked(binary),
        Stage::Copy3SlotsUnmasked(binary),
        Stage::Copy4SlotsUnmasked(binary),
        Stage::CopyImmutableUnmasked(binary),
        Stage::Copy2ImmutablesUnmasked(binary),
        Stage::Copy3ImmutablesUnmasked(binary),
        Stage::Copy4ImmutablesUnmasked(binary),
        Stage::SwizzleCopySlotMasked(&c.swizzle_copy),
        Stage::SwizzleCopy2SlotsMasked(&c.swizzle_copy),
        Stage::SwizzleCopy3SlotsMasked(&c.swizzle_copy),
        Stage::SwizzleCopy4SlotsMasked(&c.swizzle_copy),
        Stage::Swizzle1(swizzle),
        Stage::Swizzle2(swizzle),
        Stage::Swizzle3(swizzle),
        Stage::Swizzle4(swizzle),
        Stage::Shuffle(&c.shuffle),
    ]
}

/// Runs `LoadSrc(slot 1), SetBasePointer(slot 3), [InitLaneMasks], stage, StoreSrc(slot 2)` over
/// `w` pixels at `at` with read-only `data` in slot 0, the registers in slot 1 and the `SkSL`
/// slots (initially `data`) in slot 3; returns the stored registers and the slots.
fn run_sksl(
    stage: Stage<'_>,
    init_masks: bool,
    sel: Selection,
    at: (usize, usize, usize),
    regs: &[u8],
    data: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let mut stages = vec![
        Stage::LoadSrc(MemPtr::new(MemSlot(1), 0)),
        Stage::SetBasePointer(MemPtr::new(MemSlot(3), 0)),
    ];
    if init_masks {
        stages.push(Stage::InitLaneMasks);
    }
    stages.push(stage);
    stages.push(Stage::StoreSrc(OUT0));
    let mut out = vec![0u8; REGS_BYTES];
    let mut slots = data.to_vec();
    let mut program = Program::new(&stages, sel, true);
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(data))
        .with(MemSlot(1), MemView::read(regs))
        .with(MemSlot(2), MemView::write(&mut out))
        .with(MemSlot(3), MemView::write(&mut slots));
    program.run(at.0, at.1, at.2, 1, &mut mem);
    drop(mem);
    (out, slots)
}

/// Random lane words that are mostly all-ones/all-zero (masks) and signaling NaNs.
fn sksl_random_bytes(rng: &mut Rng, count: usize) -> Vec<u8> {
    random_lanes(rng, &[0, u32::MAX, 0x7fbf_ffff, 0xffbf_ffff], count)
        .iter()
        .flat_map(|w| w.to_ne_bytes())
        .collect()
}

#[test]
fn sksl_mask_and_copy_stages_run_on_every_selection() {
    // Under Miri this covers Scalar and the AmdZen4 models; each stage runs on one of them (and
    // without `init_lane_masks`) to keep the interpreted run short.
    let sels = selections();
    for (sel_index, sel) in sels.iter().copied().enumerate() {
        let n = highp_n(sel);
        let ctxs = SkslCtxs::new(n);
        let mut rng = Rng::new(0xb6a0);
        for (stage_index, stage) in sksl_stages(n, &ctxs).into_iter().enumerate() {
            if cfg!(miri) && stage_index % sels.len() != sel_index {
                continue;
            }
            let regs = sksl_random_bytes(&mut rng, REGS_BYTES / 4);
            let data = sksl_random_bytes(&mut rng, SKSL_DATA_BYTES / 4);
            let variants: &[bool] = if cfg!(miri) { &[false] } else { &[false, true] };
            for &init_masks in variants {
                let at = (rng.below(1 << 20), rng.below(1 << 20), 1 + rng.below(2 * n));
                let _ = run_sksl(stage, init_masks, sel, at, &regs, &data);
            }
        }
    }
}

#[test]
fn sksl_mask_and_copy_stage_twins() {
    for (native, models) in twin_sets() {
        let n = highp_n(native);
        let ctxs = SkslCtxs::new(n);
        let mut rng = Rng::new(0xb6a1);
        for round in 0..20 {
            for stage in sksl_stages(n, &ctxs) {
                let regs = sksl_random_bytes(&mut rng, REGS_BYTES / 4);
                let data = sksl_random_bytes(&mut rng, SKSL_DATA_BYTES / 4);
                for init_masks in [false, true] {
                    let at = (rng.below(1 << 20), rng.below(1 << 20), 1 + rng.below(2 * n));
                    let want = run_sksl(stage, init_masks, native, at, &regs, &data);
                    for model in &models {
                        let got = run_sksl(stage, init_masks, *model, at, &regs, &data);
                        assert_eq!(
                            want, got,
                            "{native} vs {model}, {stage:?}, round {round}, init_masks {init_masks}"
                        );
                    }
                }
            }
        }
    }
}
