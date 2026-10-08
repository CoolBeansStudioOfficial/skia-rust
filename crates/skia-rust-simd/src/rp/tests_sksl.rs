// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the `SkSL` math (B6c), trace and `callback` (B6d) stages:
//!
//! - known answers on every selection this host can run (Scalar, the native tiers and the
//!   models, also under Miri);
//! - stage twins: native vs `Model(Host)` (vs `Model(AmdZen4)` for the stages without
//!   estimates), bit for bit, on random and special lanes (design §2.8);
//! - the trace ops' masking and `callback`'s `store4` layout.
//!
//! The ports of Skia's own tests are in `tests/src/unit` (`SkRasterPipelineOptsTest`,
//! `SkRasterPipeline_Trace*`).

// Under Miri only Scalar and the AmdZen4 models run, leaving some helpers unused.
#![cfg_attr(miri, allow(dead_code))]
// Test data is built from small indices; the casts are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use super::contexts::{BinaryOpCtx, CallbackCtx, TraceFuncCtx, TraceHook, TraceLineCtx};
use super::contexts::{TraceScopeCtx, TraceVarCtx};
use super::lanes::test_support::{Rng, float_specials};
use super::{MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

/// The models of `t` this host can run (not `Model(Host)` under Miri).
fn models(t: Tier) -> Vec<(Selection, bool)> {
    let sources = if t == Tier::Neon {
        [Estimates::Host, Estimates::Arm]
    } else {
        [Estimates::Host, Estimates::AmdZen4]
    };
    sources
        .into_iter()
        .map(|e| (Selection::model(t, e), e == Estimates::Host))
        .filter(|(s, _)| {
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
        v.extend(models(t).into_iter().map(|(s, _)| s));
    }
    v
}

const REGS: usize = 16;
const BASE: MemPtr = MemPtr::new(MemSlot(0), 0);

/// Runs `stages` over one full chunk with `regs` (register `i`, lane `l` at `i * n + l`) bound
/// to slot 0, returning the registers afterwards.
fn run_regs(stages: &[Stage<'_>], sel: Selection, regs: &[f32]) -> Vec<f32> {
    let n = sel.tier.highp_stride();
    let mut bytes: Vec<u8> = regs.iter().flat_map(|f| f.to_ne_bytes()).collect();
    bytes.resize(REGS * 4 * n, 0);
    let mut program = Program::new(stages, sel, true);
    let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut bytes));
    program.run(0, 0, n, 1, &mut mem);
    drop(mem);
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

/// The unary 1-register stages: name, stage, and whether they use an estimate.
fn unary_stages() -> Vec<(&'static str, Stage<'static>, bool)> {
    let p = BASE;
    vec![
        ("sin", Stage::SinFloat(p), false),
        ("cos", Stage::CosFloat(p), false),
        ("tan", Stage::TanFloat(p), false),
        ("asin", Stage::AsinFloat(p), false),
        ("acos", Stage::AcosFloat(p), false),
        ("atan", Stage::AtanFloat(p), false),
        ("sqrt", Stage::SqrtFloat(p), false),
        ("exp", Stage::ExpFloat(p), false),
        ("exp2", Stage::Exp2Float(p), false),
        ("log", Stage::LogFloat(p), false),
        ("log2", Stage::Log2Float(p), false),
        ("invsqrt", Stage::InvsqrtFloat(p), true),
        ("invsqrt2", Stage::Invsqrt2Floats(p), true),
        ("invsqrt3", Stage::Invsqrt3Floats(p), true),
        ("invsqrt4", Stage::Invsqrt4Floats(p), true),
        ("inverse_mat2", Stage::InverseMat2(p), true),
        ("inverse_mat3", Stage::InverseMat3(p), true),
        ("inverse_mat4", Stage::InverseMat4(p), true),
    ]
}

/// Two registers of `atan2`/`pow` operands: `dst` at register 0, `src` at register 1.
fn binary_stages(n: usize) -> Vec<(&'static str, Vec<Stage<'static>>)> {
    let ctx = BinaryOpCtx {
        dst: 0,
        src: (4 * n) as u32,
    };
    vec![
        (
            "atan2",
            vec![Stage::SetBasePointer(BASE), Stage::Atan2NFloats(ctx)],
        ),
        (
            "pow",
            vec![Stage::SetBasePointer(BASE), Stage::PowNFloats(ctx)],
        ),
    ]
}

/// Two NaNs may differ in payload when they meet in a commutative operation (design §2.4).
fn same_bits(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn known_answers_on_every_selection() {
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let reg = |regs: &[f32], i: usize| regs[i * n..(i + 1) * n].to_vec();
        let splat = |vals: &[f32]| -> Vec<f32> {
            vals.iter()
                .flat_map(|v| std::iter::repeat_n(*v, n))
                .collect()
        };

        // sqrt is exact.
        let out = run_regs(&[Stage::SqrtFloat(BASE)], sel, &splat(&[4.0]));
        assert_eq!(reg(&out, 0), vec![2.0; n], "{sel}");

        // invsqrt of 4 is 1/2 to within the estimate.
        let out = run_regs(&[Stage::Invsqrt2Floats(BASE)], sel, &splat(&[4.0, 16.0]));
        for (i, want) in [0.5f32, 0.25].into_iter().enumerate() {
            for l in reg(&out, i) {
                assert!((l - want).abs() < want * 1e-3, "{sel}: {l} vs {want}");
            }
        }

        // exp2/log2 are exact at 0 and 1 to within the approximation.
        let out = run_regs(&[Stage::Exp2Float(BASE)], sel, &splat(&[0.0]));
        assert!(reg(&out, 0).iter().all(|v| (v - 1.0).abs() < 1e-3), "{sel}");
        let out = run_regs(&[Stage::Log2Float(BASE)], sel, &splat(&[1.0]));
        assert!(reg(&out, 0).iter().all(|v| v.abs() < 1e-3), "{sel}");

        // pow(0, y) = 0 and pow(1, y) = 1 exactly; pow(2, 3) = 8 approximately.
        let ctx = BinaryOpCtx {
            dst: 0,
            src: (4 * n) as u32,
        };
        let stages = [Stage::SetBasePointer(BASE), Stage::PowNFloats(ctx)];
        let out = run_regs(&stages, sel, &splat(&[0.0, 5.0]));
        assert_eq!(reg(&out, 0), vec![0.0; n], "{sel}");
        let out = run_regs(&stages, sel, &splat(&[1.0, 5.0]));
        assert_eq!(reg(&out, 0), vec![1.0; n], "{sel}");
        let out = run_regs(&stages, sel, &splat(&[2.0, 3.0]));
        assert!(reg(&out, 0).iter().all(|v| (v - 8.0).abs() < 0.05), "{sel}");

        // A two-register binary op processes both registers (atan2 over {y0, y1} / {x0, x1}).
        let ctx2 = BinaryOpCtx {
            dst: 0,
            src: (8 * n) as u32,
        };
        let stages = [Stage::SetBasePointer(BASE), Stage::Atan2NFloats(ctx2)];
        let out = run_regs(&stages, sel, &splat(&[1.0, -1.0, 1.0, 1.0]));
        let quarter = std::f32::consts::FRAC_PI_4;
        assert!(
            reg(&out, 0).iter().all(|v| (v - quarter).abs() < 2e-3),
            "{sel}"
        );
        assert!(
            reg(&out, 1).iter().all(|v| (v + quarter).abs() < 2e-3),
            "{sel}"
        );

        // Inverses of diagonal matrices.
        let diag = |k: usize| -> Vec<f32> {
            let mut vals = vec![0.0; k * k];
            for i in 0..k {
                vals[i * k + i] = 2.0;
            }
            splat(&vals)
        };
        for (stage, k) in [
            (Stage::InverseMat2(BASE), 2),
            (Stage::InverseMat3(BASE), 3),
            (Stage::InverseMat4(BASE), 4),
        ] {
            let out = run_regs(&[stage], sel, &diag(k));
            for i in 0..k * k {
                let want = if i % (k + 1) == 0 { 0.5 } else { 0.0 };
                for v in reg(&out, i) {
                    assert!((v - want).abs() < 1e-5, "{sel}: mat{k} element {i} = {v}");
                }
            }
        }
    }
}

#[test]
fn sksl_math_stage_twins() {
    if cfg!(miri) {
        return;
    }
    let specials = float_specials();
    for t in SIMD.into_iter().filter(|t| t.is_native()) {
        let native = Selection::native(t);
        let n = t.highp_stride();
        let mut rng = Rng::new(0x5c1_a7b);
        let random = |rng: &mut Rng| -> Vec<f32> {
            (0..REGS * n)
                .map(|_| {
                    f32::from_bits(match rng.below(4) {
                        0 => rng.next_u32(),
                        1 => rng.pick(&specials),
                        // Moderate values, where the approximations are interesting.
                        _ => ((rng.below(4001) as f32 - 2000.0) / 100.0).to_bits(),
                    })
                })
                .collect()
        };
        let check = |name: &str, stages: &[Stage<'static>], estimate: bool, regs: &[f32]| {
            let want = run_regs(stages, native, regs);
            for (model, host) in models(t) {
                if estimate && !host {
                    continue;
                }
                let got = run_regs(stages, model, regs);
                for (i, (w, g)) in want.iter().zip(&got).enumerate() {
                    assert!(
                        same_bits(*w, *g),
                        "{name}: {native} vs {model}, word {i}: {w} vs {g} (input {})",
                        regs[i]
                    );
                }
            }
        };
        for round in 0..300 {
            let regs = random(&mut rng);
            for (name, stage, estimate) in unary_stages() {
                check(name, &[stage], estimate, &regs);
            }
            for (name, stages) in binary_stages(n) {
                check(name, &stages, false, &regs);
            }
            let _ = round;
        }
    }
}

/// A `TraceHook` that records its events.
#[derive(Default)]
struct Recorder(Mutex<Vec<i32>>);

impl TraceHook for Recorder {
    fn var(&self, slot: i32, val: i32) {
        self.0.lock().unwrap().extend([-1, slot, val]);
    }
    fn line(&self, line_num: i32) {
        self.0.lock().unwrap().extend([-2, line_num]);
    }
    fn enter(&self, fn_idx: i32) {
        self.0.lock().unwrap().extend([-3, fn_idx]);
    }
    fn exit(&self, fn_idx: i32) {
        self.0.lock().unwrap().extend([-4, fn_idx]);
    }
    fn scope(&self, delta: i32) {
        self.0.lock().unwrap().extend([-5, delta]);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // one scenario, five ops, two execution masks
fn trace_ops_mask_per_lane() {
    for sel in selections() {
        let n = sel.tier.highp_stride();
        // Slot 0: r,g,b,a (`a` is the execution mask); slot 1: trace mask, data, indirect offsets.
        let exec = |lanes: &[bool]| -> Vec<u8> {
            let mut b = vec![0u8; 4 * 4 * n];
            for (l, on) in lanes.iter().enumerate() {
                let w: u32 = if *on { !0 } else { 0 };
                b[(3 * n + l) * 4..(3 * n + l + 1) * 4].copy_from_slice(&w.to_ne_bytes());
            }
            b
        };
        let trace_mask = MemPtr::new(MemSlot(1), 0);
        let data = MemPtr::new(MemSlot(1), (4 * n) as u32);
        let indirect = MemPtr::new(MemSlot(1), (4 * n * 3) as u32);
        let mut aux = vec![0u8; 4 * n * 8];
        let mut put = |reg: usize, lane: usize, v: u32| {
            aux[(reg * n + lane) * 4..(reg * n + lane + 1) * 4].copy_from_slice(&v.to_ne_bytes());
        };
        for l in 0..n {
            put(0, l, if l == n - 1 { !0 } else { 0 }); // trace mask: last lane only
            put(1, l, 100 + l as u32); // data[0]
            put(2, l, 200 + l as u32); // data[1]
            put(3, l, 1); // indirect offset
        }

        let hook = Arc::new(Recorder::default());
        let run = |stages: &[Stage<'_>], lanes: &[bool]| {
            let mut regs = exec(lanes);
            let mut aux = aux.clone();
            let mut program = Program::new(stages, sel, true);
            let mut mem = MemoryBindings::new()
                .with(MemSlot(0), MemView::write(&mut regs))
                .with(MemSlot(1), MemView::write(&mut aux));
            program.run(0, 0, n, 1, &mut mem);
        };
        let line = TraceLineCtx {
            trace_mask,
            trace_hook: hook.clone(),
            line_number: 7,
        };
        let enter = TraceFuncCtx {
            trace_mask,
            trace_hook: hook.clone(),
            func_idx: 3,
        };
        let scope = TraceScopeCtx {
            trace_mask,
            trace_hook: hook.clone(),
            delta: -2,
        };
        let var = TraceVarCtx {
            trace_mask,
            trace_hook: hook.clone(),
            slot_idx: 10,
            num_slots: 2,
            data,
            indirect_offset: None,
            indirect_limit: 0,
        };
        let var_indirect = TraceVarCtx {
            indirect_offset: Some(indirect),
            indirect_limit: 1,
            slot_idx: 20,
            num_slots: 1,
            ..var.clone()
        };
        let stages = [
            Stage::LoadSrc(MemPtr::new(MemSlot(0), 0)),
            Stage::TraceLine(&line),
            Stage::TraceEnter(&enter),
            Stage::TraceExit(&enter),
            Stage::TraceScope(&scope),
            Stage::TraceVar(&var),
            Stage::TraceVar(&var_indirect),
        ];

        // The traced lane is executing: every op reports once.
        let mut lanes = vec![false; n];
        lanes[n - 1] = true;
        run(&stages, &lanes);
        let last = (n - 1) as i32;
        assert_eq!(
            *hook.0.lock().unwrap(),
            vec![
                -2,
                7,
                -3,
                3,
                -4,
                3,
                -5,
                -2, // line, enter, exit, scope
                -1,
                10,
                100 + last,
                -1,
                11,
                200 + last, // var: both slots, lane n-1
                -1,
                21,
                200 + last, // indirect var: offset 1 register, slot 20 + 1
            ],
            "{sel}"
        );

        // The traced lane is not executing: only `trace_scope` (which ignores the execution
        // mask) reports.
        hook.0.lock().unwrap().clear();
        run(&stages, &vec![false; n]);
        let expected: &[i32] = &[-5, -2];
        assert_eq!(*hook.0.lock().unwrap(), expected, "{sel}");
    }
}

#[test]
fn callback_sees_interleaved_pixels() {
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let seen = RefCell::new((0usize, Vec::new()));
        let callback = |rgba: &mut [f32; 64], active: usize| {
            *seen.borrow_mut() = (active, rgba[..4 * active].to_vec());
            for (i, v) in rgba[..4 * active].iter_mut().enumerate() {
                *v += i as f32 * 0.5;
            }
        };
        let ctx = CallbackCtx {
            callback: &callback,
        };
        let src: Vec<f32> = (0..4 * n).map(|i| i as f32).collect();
        let out = run_regs(
            &[
                Stage::LoadSrc(BASE),
                Stage::Callback(&ctx),
                Stage::StoreSrc(BASE),
            ],
            sel,
            &src,
        );

        let (active, rgba) = seen.into_inner();
        assert_eq!(active, n, "{sel}");
        for lane in 0..n {
            for c in 0..4 {
                // Register c, lane `lane` is word c * n + lane; the callback sees pixel-major.
                assert_eq!(rgba[4 * lane + c], src[c * n + lane], "{sel}");
                let i = 4 * lane + c;
                assert_eq!(
                    out[c * n + lane],
                    src[c * n + lane] + i as f32 * 0.5,
                    "{sel}"
                );
            }
        }
    }
}
