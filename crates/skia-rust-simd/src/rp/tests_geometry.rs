// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the B5 stages (`rp/tiers/{highp,lowp}/geometry.rs`): matrices, `repeat`/`mirror`/
//! `clamp`/`decal`.
//!
//! - known answers worked out by hand from the C++ formulas (`SkRasterPipeline_opts.h`), run on
//!   every selection this host can run (Scalar, the native x86 tiers and the models; the
//!   Scalar tier and the `AmdZen4`/`Arm` models also under Miri);
//! - scalar references of `exclusive_repeat`/`exclusive_mirror`, bit for bit on many inputs;
//! - stage twins: native vs `Model(Host)` (and `Model(AmdZen4)`/`Model(Arm)` where no estimate
//!   instruction is involved) on random and special lanes.
//!
//! highp programs load `r,g,b,a` (planar, `N` lanes each); lowp `gg` stages see `x` = the bytes
//! of `r,g` and `y` = the bytes of `b,a`, so a lowp program's input and output bytes are `x`
//! (`N` floats) then `y`.

// Under Miri only Scalar and the AmdZen4/Arm models run, leaving some helpers unused.
#![cfg_attr(miri, allow(dead_code))]
// Test data is built from small indices; the casts are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

use super::lanes::test_support::{Prim, Rng, float_specials};
use super::lanes::tests::run_highp;
use super::{
    MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage,
    contexts::{CoordClampCtx, DecalTileCtx, TileCtx},
};
use crate::tier::{Backend, Estimates, Selection, Tier};

/// The SIMD tiers.
const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

/// Bytes of four registers of the widest tier.
const REGS_BYTES: usize = 4 * 4 * 16;

const IN0: MemPtr = MemPtr::new(MemSlot(0), 0);
const OUT0: MemPtr = MemPtr::new(MemSlot(1), 0);

/// The models of `t` this host can run (`Model(Host)` needs the host's estimate instructions
/// and is not run under Miri).
fn models(t: Tier) -> Vec<Selection> {
    let sources = if t == Tier::Neon {
        [Estimates::Host, Estimates::Arm]
    } else {
        [Estimates::Host, Estimates::AmdZen4]
    };
    sources
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

/// Runs `LoadSrc, stages, StoreSrc` over `w` pixels at `(0, 0)` and returns whether the program
/// is lowp and the stored bytes.
fn run(
    stages: &[Stage<'_>],
    sel: Selection,
    force_highp: bool,
    w: usize,
    input: &[u8],
) -> (bool, [u8; REGS_BYTES]) {
    let all = [&[Stage::LoadSrc(IN0)][..], stages, &[Stage::StoreSrc(OUT0)]].concat();
    let mut program = Program::new(&all, sel, force_highp);
    let mut src = [0u8; REGS_BYTES];
    src[..input.len()].copy_from_slice(input);
    let mut out = [0u8; REGS_BYTES];
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&src))
        .with(MemSlot(1), MemView::write(&mut out));
    program.run(0, 0, w, 1, &mut mem);
    drop(mem);
    (program.is_lowp(), out)
}

/// Floats as bytes.
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

/// A highp register file: `r` and `g` (`n` lanes each, cycling through the given values), then
/// `b` and `a`.
fn cycled(vals: &[f32], n: usize) -> Vec<f32> {
    (0..n).map(|i| vals[i % vals.len()]).collect()
}

/// Runs `stages` as a highp program on `sel` over `n` lanes with `r`, `g` (cycled) and constant
/// `b`, `a`; returns `(r, g, b, a)`.
fn highp4(
    stages: &[Stage<'_>],
    sel: Selection,
    r: &[f32],
    g: &[f32],
) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = sel.tier.highp_stride();
    let input = [
        cycled(r, n),
        cycled(g, n),
        cycled(&[0.25], n),
        cycled(&[0.75], n),
    ]
    .concat();
    let (lowp, out) = run(stages, sel, true, n, &f32_bytes(&input));
    assert!(!lowp);
    let f = floats(&out[..16 * n]);
    (
        f[..n].to_vec(),
        f[n..2 * n].to_vec(),
        f[2 * n..3 * n].to_vec(),
        f[3 * n..].to_vec(),
    )
}

/// Runs `stages` as a lowp program on `sel` (which must have a lowp tier) over `n` lanes with
/// `x`, `y` cycled; returns `(x, y)`, or `None` on Scalar.
fn lowp2(
    stages: &[Stage<'_>],
    sel: Selection,
    x: &[f32],
    y: &[f32],
) -> Option<(Vec<f32>, Vec<f32>)> {
    let n = sel.tier.lowp_stride()?;
    let input = [cycled(x, n), cycled(y, n)].concat();
    let (lowp, out) = run(stages, sel, false, n, &f32_bytes(&input));
    assert!(lowp, "{sel}: expected a lowp program");
    let f = floats(&out[..8 * n]);
    Some((f[..n].to_vec(), f[n..].to_vec()))
}

/// Asserts `got[i] == want[i % want.len()]` bit for bit.
fn assert_lanes(what: &str, sel: Selection, got: &[f32], want: &[f32]) {
    for (i, g) in got.iter().enumerate() {
        let w = want[i % want.len()];
        assert_eq!(
            g.to_bits(),
            w.to_bits(),
            "{what} on {sel}, lane {i}: got {g} ({:#x}), want {w} ({:#x})",
            g.to_bits(),
            w.to_bits()
        );
    }
}

// ~~~ Matrices ~~~

#[test]
fn matrix_translate_known_answers() {
    // r += m[0]; g += m[1] (exact in binary).
    let (xs, ys) = ([-3.0, 0.5, 100.25, 7.0], [2.0, -0.5, 8.0, 0.0]);
    let wx = [7.5, 11.0, 110.75, 17.5];
    let wy = [-4.0, -6.5, 2.0, -6.0];
    let m = [10.5, -6.0];
    for sel in selections() {
        let (r, g, b, a) = highp4(&[Stage::MatrixTranslate(m)], sel, &xs, &ys);
        assert_lanes("highp matrix_translate r", sel, &r, &wx);
        assert_lanes("highp matrix_translate g", sel, &g, &wy);
        // b and a pass through.
        assert_lanes("highp b", sel, &b, &[0.25]);
        assert_lanes("highp a", sel, &a, &[0.75]);
        if let Some((x, y)) = lowp2(&[Stage::MatrixTranslate(m)], sel, &xs, &ys) {
            assert_lanes("lowp matrix_translate x", sel, &x, &wx);
            assert_lanes("lowp matrix_translate y", sel, &y, &wy);
        }
    }
}

#[test]
fn matrix_scale_translate_known_answers() {
    // r = r*m0 + m2; g = g*m1 + m3.
    let m = [2.0, -4.0, 1.0, 0.5];
    let (xs, ys) = ([0.0, 1.5, -3.0, 10.0], [1.0, 0.25, -2.0, 0.0]);
    let wx = [1.0, 4.0, -5.0, 21.0];
    let wy = [-3.5, -0.5, 8.5, 0.5];
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::MatrixScaleTranslate(&m)], sel, &xs, &ys);
        assert_lanes("highp matrix_scale_translate r", sel, &r, &wx);
        assert_lanes("highp matrix_scale_translate g", sel, &g, &wy);
        if let Some((x, y)) = lowp2(&[Stage::MatrixScaleTranslate(&m)], sel, &xs, &ys) {
            assert_lanes("lowp matrix_scale_translate x", sel, &x, &wx);
            assert_lanes("lowp matrix_scale_translate y", sel, &y, &wy);
        }
    }
}

#[test]
fn matrix_2x3_known_answers() {
    // R = r*m0 + (g*m1 + m2); G = r*m3 + (g*m4 + m5); both use the *old* r and g.
    let m = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let (xs, ys) = ([0.0, 1.0, -1.0, 2.5], [0.0, 1.0, 2.0, -0.5]);
    // (0,0) -> (3,6); (1,1) -> (6,15); (-1,2) -> (6,12); (2.5,-0.5) -> (4.5,13.5)
    let wx = [3.0, 6.0, 6.0, 4.5];
    let wy = [6.0, 15.0, 12.0, 13.5];
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::Matrix2x3(&m)], sel, &xs, &ys);
        assert_lanes("highp matrix_2x3 r", sel, &r, &wx);
        assert_lanes("highp matrix_2x3 g", sel, &g, &wy);
        if let Some((x, y)) = lowp2(&[Stage::Matrix2x3(&m)], sel, &xs, &ys) {
            assert_lanes("lowp matrix_2x3 x", sel, &x, &wx);
            assert_lanes("lowp matrix_2x3 y", sel, &y, &wy);
        }
    }
}

#[test]
fn matrix_perspective_known_answers() {
    // Row-major: X = r*m0 + (g*m1 + m2); Y = r*m3 + (g*m4 + m5); Z = r*m6 + (g*m7 + m8);
    // r = X * rcp_precise(Z); g = Y * rcp_precise(Z), with the selection's own rcp_precise.
    let m = [1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 0.5, 0.25, 1.0];
    let (xs, ys) = ([0.0, 2.0, 4.0, -1.0], [0.0, 4.0, 8.0, 3.0]);
    // X = x, Y = 2y + 1 = 1, 9, 17, 7 and Z = 0.5x + 0.25y + 1 = 1, 3, 5, 1.25: all exact.
    let (wx, wy) = ([0.0f32, 2.0, 4.0, -1.0], [1.0f32, 9.0, 17.0, 7.0]);
    let z = [1.0f32, 3.0, 5.0, 1.25];
    for sel in selections() {
        // rcp_precise(Z) from the lane module that executes `sel`, on 16 lanes (a multiple of
        // every stride).
        let zbits: Vec<u32> = (0..16).map(|i| z[i % 4].to_bits()).collect();
        let zero = vec![0u32; 16];
        let rcp: Vec<f32> = run_highp(sel, Prim::RcpPrecise, &zbits, &zero, &zero)
            .into_iter()
            .map(f32::from_bits)
            .collect();
        let want_x: Vec<f32> = (0..16).map(|i| wx[i % 4] * rcp[i]).collect();
        let want_y: Vec<f32> = (0..16).map(|i| wy[i % 4] * rcp[i]).collect();
        let (r, g, ..) = highp4(&[Stage::MatrixPerspective(&m)], sel, &xs, &ys);
        assert_lanes("highp matrix_perspective r", sel, &r, &want_x);
        assert_lanes("highp matrix_perspective g", sel, &g, &want_y);
        let lowp = lowp2(&[Stage::MatrixPerspective(&m)], sel, &xs, &ys);
        assert_eq!(lowp.is_none(), sel.tier == Tier::Scalar);
        if let Some((x, y)) = lowp {
            assert_lanes("lowp matrix_perspective x", sel, &x, &want_x);
            assert_lanes("lowp matrix_perspective y", sel, &y, &want_y);
        }
        if sel.tier == Tier::Scalar {
            // Scalar's rcp_precise is exactly 1/v.
            for (i, v) in rcp.iter().enumerate() {
                assert_eq!(v.to_bits(), (1.0 / z[i % 4]).to_bits());
            }
        }
    }
}

#[test]
fn matrix_perspective_matches_the_formula_with_the_tier_rcp() {
    // For every tier the result is exactly X * rcp_precise(Z): check against the lane module's
    // own rcp_precise through a pipeline that computes Z alone: matrix_perspective with
    // X = Y = 1 gives r = rcp_precise(Z) and g = rcp_precise(Z), so r == g bitwise, and
    // X = 3 gives r == 3 * g.
    let m1 = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 3.0];
    let m3 = [0.0, 0.0, 3.0, 0.0, 0.0, 1.0, 1.0, 0.0, 3.0];
    for sel in selections() {
        let xs = [0.0, 0.5, 1.0, 4.0, 10.0, -2.0, -0.25, 7.0];
        let (r1, g1, ..) = highp4(&[Stage::MatrixPerspective(&m1)], sel, &xs, &xs);
        let (r3, ..) = highp4(&[Stage::MatrixPerspective(&m3)], sel, &xs, &xs);
        for i in 0..r1.len() {
            // Z = x + 3 (r*1 + (g*0 + 3) ... with m6 = 1: Z = x + 3).
            assert_eq!(r1[i].to_bits(), g1[i].to_bits(), "{sel} lane {i}");
            assert_eq!(r3[i].to_bits(), (3.0 * r1[i]).to_bits(), "{sel} lane {i}");
        }
    }
}

// ~~~ Gradient tiling ~~~

/// Inputs for the `_1` stages and their exact outputs, worked out by hand:
/// `clamp_x_1`: `min(max(0, v), 1)`; `repeat_x_1`: `clamp(v - floor(v))`;
/// `mirror_x_1`: `clamp(abs((v-1) - 2*floor((v-1)/2) - 1))`.
const T1_IN: [f32; 8] = [-0.5, 0.0, 0.25, 1.0, 1.25, 1.75, 2.0, 3.0];
const T1_CLAMP: [f32; 8] = [0.0, 0.0, 0.25, 1.0, 1.0, 1.0, 1.0, 1.0];
const T1_REPEAT: [f32; 8] = [0.5, 0.0, 0.25, 0.0, 0.25, 0.75, 0.0, 0.0];
const T1_MIRROR: [f32; 8] = [0.5, 0.0, 0.25, 1.0, 0.75, 0.25, 0.0, 1.0];

#[test]
fn x_1_stages_known_answers() {
    for sel in selections() {
        for (stage, want, name) in [
            (Stage::ClampX1, T1_CLAMP, "clamp_x_1"),
            (Stage::RepeatX1, T1_REPEAT, "repeat_x_1"),
            (Stage::MirrorX1, T1_MIRROR, "mirror_x_1"),
        ] {
            let (r, g, ..) = highp4(&[stage], sel, &T1_IN, &[9.0]);
            assert_lanes(&format!("highp {name}"), sel, &r, &want);
            assert_lanes(&format!("highp {name} g"), sel, &g, &[9.0]);
            if let Some((x, y)) = lowp2(&[stage], sel, &T1_IN, &[9.0]) {
                assert_lanes(&format!("lowp {name}"), sel, &x, &want);
                assert_lanes(&format!("lowp {name} y"), sel, &y, &[9.0]);
            }
        }
    }
}

#[test]
fn x_1_stages_clamp_huge_inputs() {
    // "Even repeat and mirror funnel through a clamp to handle bad inputs": huge finite values
    // end up in [0, 1] (NaN inputs are tier dependent: x86 min/max drop them, Neon keeps them;
    // the twins cover those).
    let bad = [1.0e30, -1.0e30, 3.0e38, -3.0e38, 8_388_609.0, -8_388_609.5];
    for sel in selections() {
        for stage in [Stage::ClampX1, Stage::RepeatX1, Stage::MirrorX1] {
            let (r, ..) = highp4(&[stage], sel, &bad, &[0.0]);
            for (i, v) in r.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(v),
                    "highp {stage:?} on {sel}, lane {i}: {v}"
                );
            }
        }
    }
    // clamp_x_1 alone also handles the infinities.
    for sel in selections() {
        let (r, ..) = highp4(
            &[Stage::ClampX1],
            sel,
            &[f32::INFINITY, f32::NEG_INFINITY],
            &[0.0],
        );
        assert_lanes("clamp_x_1 inf", sel, &r, &[1.0, 0.0]);
    }
}

#[test]
fn clamp_x_and_y_known_answers() {
    let ctx = CoordClampCtx {
        min_x: 1.0,
        min_y: -2.0,
        max_x: 3.0,
        max_y: 0.0,
    };
    let xs = [0.0, 1.0, 2.0, 3.0, 4.0, -7.0, 2.5, 3.0001];
    let ys = [-3.0, -2.0, -1.0, 0.0, 1.0, 9.0, -0.5, 0.25];
    let wx = [1.0, 1.0, 2.0, 3.0, 3.0, 1.0, 2.5, 3.0];
    let wy = [-2.0, -2.0, -1.0, 0.0, 0.0, 0.0, -0.5, 0.0];
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::ClampXAndY(&ctx)], sel, &xs, &ys);
        assert_lanes("highp clamp_x_and_y r", sel, &r, &wx);
        assert_lanes("highp clamp_x_and_y g", sel, &g, &wy);
        if let Some((x, y)) = lowp2(&[Stage::ClampXAndY(&ctx)], sel, &xs, &ys) {
            assert_lanes("lowp clamp_x_and_y x", sel, &x, &wx);
            assert_lanes("lowp clamp_x_and_y y", sel, &y, &wy);
        }
    }
}

// ~~~ repeat / mirror ~~~

/// `exclusive_repeat`, written as plain `f32` code.
fn ref_repeat(v: f32, ctx: &TileCtx) -> f32 {
    v - (v * ctx.inv_scale).floor() * ctx.scale
}

/// `exclusive_mirror`, written as plain `f32` code (finite, moderate inputs).
fn ref_mirror(v: f32, ctx: &TileCtx) -> f32 {
    let (limit, inv_limit) = (ctx.scale, ctx.inv_scale);
    let u = v - (v * inv_limit * 0.5).floor() * 2.0 * limit;
    let s = (u * inv_limit).floor();
    let m = u - s * 2.0 * (u - limit);
    let bias = (ctx.mirror_bias_dir as u32).wrapping_mul(s as i32 as u32);
    f32::from_bits(m.to_bits().wrapping_add(bias))
}

fn tile(scale: f32, dir: i32) -> TileCtx {
    TileCtx {
        scale,
        inv_scale: 1.0 / scale,
        mirror_bias_dir: dir,
    }
}

#[test]
fn repeat_known_answers() {
    // scale 4, invScale 0.25: v - floor(v/4)*4.
    let ctx = tile(4.0, 1);
    let xs = [5.5, -1.0, 4.0, 9.25, 0.0, 3.5, -4.0, 100.0];
    let wx = [1.5, 3.0, 0.0, 1.25, 0.0, 3.5, 0.0, 0.0];
    let ys = [-0.5, 8.0, 7.75, 2.0, -9.0, 4.5, 0.25, -100.5];
    let wy = [3.5, 0.0, 3.75, 2.0, 3.0, 0.5, 0.25, 3.5];
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::RepeatX(&ctx)], sel, &xs, &ys);
        assert_lanes("repeat_x r", sel, &r, &wx);
        assert_lanes("repeat_x leaves g", sel, &g, &ys);
        let (r, g, ..) = highp4(&[Stage::RepeatY(&ctx)], sel, &xs, &ys);
        assert_lanes("repeat_y leaves r", sel, &r, &xs);
        assert_lanes("repeat_y g", sel, &g, &wy);
    }
}

#[test]
fn mirror_known_answers() {
    // scale (limit) 4, invScale 0.25. For v = 5.5: u = 5.5, s = floor(1.375) = 1,
    // m = 5.5 - 2*1*(5.5 - 4) = 2.5, bias = dir*1 ulp. For v = 2.5: u = 2.5, s = 0, m = 2.5.
    // For v = -1: u = -1 + 8 = 7, s = 1, m = 7 - 2*3 = 1, bias dir*1 ulp.
    // For v = 4: u = 4, s = 1, m = 4 - 0 = 4, bias dir*1 ulp. For v = 8: u = 0, m = 0.
    let ulp = |f: f32, d: i32| f32::from_bits(f.to_bits().wrapping_add(d as u32));
    let xs = [5.5, 2.5, -1.0, 4.0, 8.0, 0.0, 6.0, 3.0];
    for dir in [1, -1] {
        let ctx = tile(4.0, dir);
        // v = 6: u = 6, s = 1, m = 6 - 2*2 = 2 (+dir ulp). v = 3: m = 3 (s = 0).
        let want = [
            ulp(2.5, dir),
            2.5,
            ulp(1.0, dir),
            ulp(4.0, dir),
            0.0,
            0.0,
            ulp(2.0, dir),
            3.0,
        ];
        for sel in selections() {
            let (r, g, ..) = highp4(&[Stage::MirrorX(&ctx)], sel, &xs, &xs);
            assert_lanes(&format!("mirror_x dir {dir}"), sel, &r, &want);
            assert_lanes("mirror_x leaves g", sel, &g, &xs);
            let (r, g, ..) = highp4(&[Stage::MirrorY(&ctx)], sel, &xs, &xs);
            assert_lanes("mirror_y leaves r", sel, &r, &xs);
            assert_lanes(&format!("mirror_y dir {dir}"), sel, &g, &want);
        }
    }
}

#[test]
fn repeat_and_mirror_match_the_scalar_references() {
    let mut rng = Rng::new(0x0071_17e5);
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let scales: &[f32] = if cfg!(miri) {
            &[1.0, 7.5, 0.375]
        } else {
            &[1.0, 3.0, 4.0, 7.5, 0.375]
        };
        for &scale in scales {
            for dir in [1, -1] {
                let ctx = tile(scale, dir);
                for _ in 0..if cfg!(miri) { 2 } else { 100 } {
                    let xs: Vec<f32> = (0..n)
                        .map(|_| (rng.below(200_000) as f32 - 100_000.0) / 256.0)
                        .collect();
                    let ys: Vec<f32> = (0..n)
                        .map(|_| (rng.below(200_000) as f32 - 100_000.0) / 256.0)
                        .collect();
                    let (r, g, ..) =
                        highp4(&[Stage::RepeatX(&ctx), Stage::RepeatY(&ctx)], sel, &xs, &ys);
                    let (wr, wg): (Vec<f32>, Vec<f32>) = (
                        xs.iter().map(|v| ref_repeat(*v, &ctx)).collect(),
                        ys.iter().map(|v| ref_repeat(*v, &ctx)).collect(),
                    );
                    assert_lanes("repeat_x", sel, &r, &wr);
                    assert_lanes("repeat_y", sel, &g, &wg);
                    let (r, g, ..) =
                        highp4(&[Stage::MirrorX(&ctx), Stage::MirrorY(&ctx)], sel, &xs, &ys);
                    let (wr, wg): (Vec<f32>, Vec<f32>) = (
                        xs.iter().map(|v| ref_mirror(*v, &ctx)).collect(),
                        ys.iter().map(|v| ref_mirror(*v, &ctx)).collect(),
                    );
                    assert_lanes("mirror_x", sel, &r, &wr);
                    assert_lanes("mirror_y", sel, &g, &wg);
                }
            }
        }
    }
}

// ~~~ decal ~~~

/// A decal context: `limit_x = 4`, `limit_y = 2`, and the inclusive edges for
/// `round_down_at_integer` (`limit`) or not (`0`).
fn decal_ctx(round_down: bool) -> DecalTileCtx {
    DecalTileCtx {
        limit_x: 4.0,
        limit_y: 2.0,
        inclusive_edge_x: if round_down { 4.0 } else { 0.0 },
        inclusive_edge_y: if round_down { 2.0 } else { 0.0 },
        ..DecalTileCtx::default()
    }
}

/// highp: `((0 < v) & (v < limit)) | (v == edge)`.
fn highp_in(v: f32, limit: f32, edge: f32) -> bool {
    (0.0 < v && v < limit) || v == edge
}

#[test]
fn decal_highp_known_answers() {
    let xs = [-1.0, 0.0, 0.5, 3.99, 4.0, 4.5, f32::NAN, f32::INFINITY];
    let ys = [0.5, 1.0, 2.0, -0.5, 0.0, 1.99, 2.5, 1.0];
    for round_down in [true, false] {
        // Hand-checked: x in (0,4) or x == edge_x (4 or 0); y in (0,2) or y == edge_y (2 or 0).
        let in_x = if round_down {
            [false, false, true, true, true, false, false, false]
        } else {
            [false, true, true, true, false, false, false, false]
        };
        let in_y = if round_down {
            [true, true, true, false, false, true, false, true]
        } else {
            [true, true, false, false, true, true, false, true]
        };
        for i in 0..8 {
            assert_eq!(
                highp_in(xs[i], 4.0, if round_down { 4.0 } else { 0.0 }),
                in_x[i]
            );
            assert_eq!(
                highp_in(ys[i], 2.0, if round_down { 2.0 } else { 0.0 }),
                in_y[i]
            );
        }
        for sel in selections() {
            let n = sel.tier.highp_stride();
            for (name, which) in [("decal_x", 0), ("decal_y", 1), ("decal_x_and_y", 2)] {
                let ctx = decal_ctx(round_down);
                let stage = [
                    Stage::DecalX(&ctx),
                    Stage::DecalY(&ctx),
                    Stage::DecalXAndY(&ctx),
                ][which];
                let (r, g, b, a) = highp4(&[stage, Stage::CheckDecalMask(&ctx)], sel, &xs, &ys);
                for i in 0..n {
                    let keep = match which {
                        0 => in_x[i % 8],
                        1 => in_y[i % 8],
                        _ => in_x[i % 8] && in_y[i % 8],
                    };
                    let (wr, wg, wb, wa) = if keep {
                        (xs[i % 8], ys[i % 8], 0.25, 0.75)
                    } else {
                        (0.0, 0.0, 0.0, 0.0)
                    };
                    for (got, want, reg) in [
                        (r[i], wr, "r"),
                        (g[i], wg, "g"),
                        (b[i], wb, "b"),
                        (a[i], wa, "a"),
                    ] {
                        assert_eq!(
                            got.to_bits(),
                            want.to_bits(),
                            "{name} (round_down {round_down}) on {sel}, {reg}[{i}]"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn decal_lowp_known_answers() {
    // lowp: (0 <= v) & (v < limit), no inclusive edge; the mask is 16 bits per lane and
    // check_decal_mask ANDs it with the 16-bit r,g,b,a registers (here: the halves of x, y).
    let xs = [-1.0, 0.0, 0.5, 3.99, 4.0, 4.5, f32::NAN, 2.0];
    let ys = [0.5, 1.0, 2.0, -0.5, 0.0, 1.99, 2.5, 1.0];
    let in_x = [false, true, true, true, false, false, false, true];
    let in_y = [true, true, false, false, true, true, false, true];
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        for (name, which) in [("decal_x", 0), ("decal_y", 1), ("decal_x_and_y", 2)] {
            let ctx = decal_ctx(true);
            let stage = [
                Stage::DecalX(&ctx),
                Stage::DecalY(&ctx),
                Stage::DecalXAndY(&ctx),
            ][which];
            let input = f32_bytes(&[cycled(&xs, n), cycled(&ys, n)].concat());
            let (lowp, out) = run(&[stage, Stage::CheckDecalMask(&ctx)], sel, false, n, &input);
            assert!(lowp);
            let (inw, outw) = (halves(&input), halves(&out[..8 * n]));
            for reg in 0..4 {
                for j in 0..n {
                    let keep = match which {
                        0 => in_x[j % 8],
                        1 => in_y[j % 8],
                        _ => in_x[j % 8] && in_y[j % 8],
                    };
                    let want = if keep { inw[reg * n + j] } else { 0 };
                    assert_eq!(
                        outw[reg * n + j],
                        want,
                        "{name} on {sel}, reg {reg}, lane {j}"
                    );
                }
            }
            // The stage does not change x and y: running it alone is the identity.
            let (lowp, out) = run(&[stage], sel, false, n, &input);
            assert!(lowp);
            assert_eq!(&out[..8 * n], &input[..], "{name} on {sel}");
        }
    }
}

// ~~~ Stage twins ~~~

fn random_lanes(rng: &mut Rng, specials: &[u32], count: usize) -> Vec<u32> {
    (0..count)
        .map(|_| match rng.below(4) {
            0 => rng.next_u32(),
            1 => {
                // A moderate value, so the tiling stages see normal data too.
                ((rng.below(8192) as f32 - 4096.0) / 64.0).to_bits()
            }
            _ => rng.pick(specials),
        })
        .collect()
}

fn word_bytes(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

/// Two words are equal, or both are NaNs (two NaNs meeting in a `mad` may come out in either
/// order, design §2.4 "NaN payloads").
fn same_or_both_nan(a: u32, b: u32) -> bool {
    a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

/// The contexts of one twin round.
struct Ctxs {
    m2: [f32; 2],
    m4: [f32; 4],
    m6: [f32; 6],
    m9: [f32; 9],
    tile: TileCtx,
    clamp: CoordClampCtx,
    decal: DecalTileCtx,
}

fn random_ctxs(rng: &mut Rng, specials: &[u32]) -> Ctxs {
    let v: Vec<f32> = random_lanes(rng, specials, 40)
        .into_iter()
        .map(f32::from_bits)
        .collect();
    let scale = v[20];
    let tile = TileCtx {
        scale,
        inv_scale: if rng.below(2) == 0 {
            1.0 / scale
        } else {
            v[21]
        },
        mirror_bias_dir: if rng.below(2) == 0 { 1 } else { -1 },
    };
    let clamp = CoordClampCtx {
        min_x: v[22],
        min_y: v[23],
        max_x: v[24],
        max_y: v[25],
    };
    let decal = DecalTileCtx {
        limit_x: v[26],
        limit_y: v[27],
        inclusive_edge_x: v[28],
        inclusive_edge_y: v[29],
        ..DecalTileCtx::default()
    };
    Ctxs {
        m2: [v[0], v[1]],
        m4: [v[2], v[3], v[4], v[5]],
        m6: [v[6], v[7], v[8], v[9], v[10], v[11]],
        m9: [
            v[12], v[13], v[14], v[15], v[16], v[17], v[18], v[19], v[30],
        ],
        tile,
        clamp,
        decal,
    }
}

/// Every geometry stage, as `(name, stages, lowp-capable, uses an estimate, mad-like)`.
fn all_stage_programs(c: &Ctxs) -> Vec<(&'static str, Vec<Stage<'_>>, bool, bool, bool)> {
    vec![
        (
            "matrix_translate",
            vec![Stage::MatrixTranslate(c.m2)],
            true,
            false,
            false,
        ),
        (
            "matrix_scale_translate",
            vec![Stage::MatrixScaleTranslate(&c.m4)],
            true,
            false,
            true,
        ),
        (
            "matrix_2x3",
            vec![Stage::Matrix2x3(&c.m6)],
            true,
            false,
            true,
        ),
        (
            "matrix_perspective",
            vec![Stage::MatrixPerspective(&c.m9)],
            true,
            true,
            true,
        ),
        (
            "decal_x",
            vec![Stage::DecalX(&c.decal), Stage::CheckDecalMask(&c.decal)],
            true,
            false,
            false,
        ),
        (
            "decal_y",
            vec![Stage::DecalY(&c.decal), Stage::CheckDecalMask(&c.decal)],
            true,
            false,
            false,
        ),
        (
            "decal_x_and_y",
            vec![Stage::DecalXAndY(&c.decal), Stage::CheckDecalMask(&c.decal)],
            true,
            false,
            false,
        ),
        ("clamp_x_1", vec![Stage::ClampX1], true, false, false),
        ("mirror_x_1", vec![Stage::MirrorX1], true, false, false),
        ("repeat_x_1", vec![Stage::RepeatX1], true, false, false),
        (
            "clamp_x_and_y",
            vec![Stage::ClampXAndY(&c.clamp)],
            true,
            false,
            false,
        ),
        (
            "repeat_x",
            vec![Stage::RepeatX(&c.tile)],
            false,
            false,
            false,
        ),
        (
            "repeat_y",
            vec![Stage::RepeatY(&c.tile)],
            false,
            false,
            false,
        ),
        (
            "mirror_x",
            vec![Stage::MirrorX(&c.tile)],
            false,
            false,
            false,
        ),
        (
            "mirror_y",
            vec![Stage::MirrorY(&c.tile)],
            false,
            false,
            false,
        ),
    ]
}

#[test]
fn geometry_stage_twins() {
    let specials = float_specials();
    for (native, models) in twin_sets() {
        let n = native.tier.highp_stride();
        let lowp_n = native.tier.lowp_stride().unwrap();
        let mut rng = Rng::new(0x6e0_5e7);
        for round in 0..300 {
            let ctxs = random_ctxs(&mut rng, &specials);
            let high_in = word_bytes(&random_lanes(&mut rng, &specials, 4 * n));
            let low_in = word_bytes(&random_lanes(&mut rng, &specials, 2 * lowp_n));
            for (name, stages, lowp_ok, estimate, mad_like) in all_stage_programs(&ctxs) {
                let (_, hn) = run(&stages, native, true, n, &high_in);
                let lowp_native = lowp_ok.then(|| run(&stages, native, false, lowp_n, &low_in));
                if let Some((is_lowp, _)) = &lowp_native {
                    assert!(is_lowp, "{name}");
                }
                for model in &models {
                    // The models of the two estimate sources differ on rcp's low bits, which
                    // matrix_perspective passes through a Newton-Raphson step.
                    if estimate && model.backend != Backend::Model(Estimates::Host) {
                        continue;
                    }
                    let same = |a: u32, b: u32| {
                        if mad_like {
                            same_or_both_nan(a, b)
                        } else {
                            a == b
                        }
                    };
                    let (_, hm) = run(&stages, *model, true, n, &high_in);
                    for (i, (a, b)) in words(&hn[..16 * n])
                        .iter()
                        .zip(words(&hm[..16 * n]))
                        .enumerate()
                    {
                        assert!(
                            same(*a, b),
                            "highp {name}: {native} vs {model}, round {round}, word {i}: {a:#x} vs {b:#x}"
                        );
                    }
                    if let Some((_, ln)) = &lowp_native {
                        let (_, lm) = run(&stages, *model, false, lowp_n, &low_in);
                        if mad_like {
                            for (i, (a, b)) in words(&ln[..8 * lowp_n])
                                .iter()
                                .zip(words(&lm[..8 * lowp_n]))
                                .enumerate()
                            {
                                assert!(
                                    same(*a, b),
                                    "lowp {name}: {native} vs {model}, round {round}, word {i}: {a:#x} vs {b:#x}"
                                );
                            }
                        } else {
                            assert_eq!(
                                ln[..8 * lowp_n],
                                lm[..8 * lowp_n],
                                "lowp {name}: {native} vs {model}, round {round}"
                            );
                        }
                    }
                }
            }
        }
    }
}
