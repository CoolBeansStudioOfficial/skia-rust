// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of B3's stages (blend modes and coverage), highp and lowp:
//!
//! - known answers: every stage against an independent scalar transcription of Skia's formulas
//!   (`Scalar` exactly, and the other tiers and models wherever their lane primitives agree with
//!   the reference: the same `mad` fusing, no `rcp` estimates);
//! - stage twins: native tier vs `Model(Host)` (and `Model(AmdZen4)`/`Model(Arm)` for stages
//!   without estimates), bit for bit, on random lanes;
//! - the coverage stages with `MemoryCtx` memory, including tail chunks.

// Test data is built from small indices; the casts are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use core::cell::Cell;
use core::num::Wrapping;

use super::lanes::test_support::Rng;
use super::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

const SRC: MemSlot = MemSlot(0);
const DST: MemSlot = MemSlot(1);
const OUT: MemSlot = MemSlot(2);
const COVER: MemSlot = MemSlot(4);

/// Rounds of random lanes per test.
fn rounds() -> usize {
    // Two rounds under Miri: a full chunk and a tail chunk (the sweeps run natively).
    if cfg!(miri) { 2 } else { 150 }
}

/// Under Miri, every third stage of a family (the per-stage sweeps run natively); all of them
/// otherwise.
fn sample<T>(v: Vec<T>) -> Vec<T> {
    if cfg!(miri) {
        v.into_iter().step_by(3).collect()
    } else {
        v
    }
}

/// Every selection this host can run (Scalar, native tiers, models; only Scalar and the
/// `AmdZen4`/`Arm` models under Miri).
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

/// Whether `mad`/`nmad` are fused on the tier's highp.
fn fused(t: Tier) -> bool {
    matches!(t, Tier::Ml3 | Tier::Ml4 | Tier::Neon)
}

// ~~~ Running one stage ~~~

/// Runs `LoadSrc, LoadDst, stages…, StoreSrc` over `w` pixels at `x0` and returns whether the
/// program was lowp and the stored source registers (4 registers of `n` lanes, as bytes).
fn run(
    stages: &[Stage<'_>],
    sel: Selection,
    force_highp: bool,
    (x0, w): (usize, usize),
    (src, dst): (&[u8], &[u8]),
    cover: &[u8],
) -> (bool, Vec<u8>) {
    let all = [
        &[
            Stage::LoadSrc(MemPtr::new(SRC, 0)),
            Stage::LoadDst(MemPtr::new(DST, 0)),
        ][..],
        stages,
        &[Stage::StoreSrc(MemPtr::new(OUT, 0))],
    ]
    .concat();
    let mut out = vec![0u8; 256];
    let mut program = Program::new(&all, sel, force_highp);
    let mut mem = MemoryBindings::new()
        .with(SRC, MemView::read(src))
        .with(DST, MemView::read(dst))
        .with(OUT, MemView::write(&mut out))
        .with(COVER, MemView::read(cover));
    program.run(x0, 0, w, 1, &mut mem);
    let lowp = program.is_lowp();
    drop(mem);
    (lowp, out)
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn u16_bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

fn halves(b: &[u8]) -> Vec<u16> {
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes(*c))
        .collect()
}

// ~~~ Random lanes ~~~

/// One highp lane: `r, g, b, a, dr, dg, db, da`.
type Px = [f32; 8];

fn unit(rng: &mut Rng) -> f32 {
    (rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32
}

fn gen_alpha(rng: &mut Rng) -> f32 {
    match rng.below(6) {
        0 => 0.0,
        1 => 1.0,
        _ => unit(rng),
    }
}

/// A color channel: often related to `a` so the equality branches (`s == 0`, `s == sa`,
/// `d == da`, `sat == 0`) are taken, sometimes out of gamut.
fn gen_color(rng: &mut Rng, a: f32) -> f32 {
    match rng.below(9) {
        0 => 0.0,
        1 | 2 => a,
        3 => 1.0,
        4 => -0.5 * unit(rng),
        5 => 2.0 * unit(rng),
        _ => unit(rng) * a,
    }
}

fn gen_px(rng: &mut Rng) -> Px {
    let (a, da) = (gen_alpha(rng), gen_alpha(rng));
    let gray = rng.below(8) == 0; // equal channels: `sat == 0`
    let (g0, g1) = (gen_color(rng, a), gen_color(rng, da));
    let c = |rng: &mut Rng, alpha: f32, g: f32| if gray { g } else { gen_color(rng, alpha) };
    [
        c(rng, a, g0),
        c(rng, a, g0),
        c(rng, a, g0),
        a,
        c(rng, da, g1),
        c(rng, da, g1),
        c(rng, da, g1),
        da,
    ]
}

/// The register files for `lanes` (`n` lanes): `(src, dst)` as 4 registers each.
fn highp_regs(lanes: &[Px]) -> (Vec<u8>, Vec<u8>) {
    let reg = |range: core::ops::Range<usize>| -> Vec<u8> {
        range
            .flat_map(|c| lanes.iter().map(move |l| l[c]))
            .flat_map(f32::to_ne_bytes)
            .collect()
    };
    (reg(0..4), reg(4..8))
}

// ~~~ The highp reference ~~~

/// Float arithmetic as the tier does it: fused or unfused `mad`, and the reciprocal estimate.
#[derive(Clone, Copy)]
struct Hf {
    fused: bool,
}

impl Hf {
    /// `a + f*m`.
    fn mad(self, f: f32, m: f32, a: f32) -> f32 {
        if self.fused {
            f.mul_add(m, a)
        } else {
            a + f * m
        }
    }
    /// `a - f*m`.
    fn nmad(self, f: f32, m: f32, a: f32) -> f32 {
        if self.fused {
            (-f).mul_add(m, a)
        } else {
            a - f * m
        }
    }
}

/// x86 `min`/`max` (second operand on ties and NaN).
fn fmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
fn fmax(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}
fn inv(x: f32) -> f32 {
    1.0 - x
}
fn two(x: f32) -> f32 {
    x + x
}
fn sel(c: bool, t: f32, e: f32) -> f32 {
    if c { t } else { e }
}

/// Stages with the `BLEND_MODE` shape "channel function on all four channels".
fn separable_all(st: &Stage<'_>) -> bool {
    matches!(
        st,
        Stage::Clear
            | Stage::Srcatop
            | Stage::Dstatop
            | Stage::Srcin
            | Stage::Dstin
            | Stage::Srcout
            | Stage::Dstout
            | Stage::Srcover
            | Stage::Dstover
            | Stage::Modulate
            | Stage::Multiply
            | Stage::Plus
            | Stage::Screen
            | Stage::Xor
    )
}

/// The `name_channel` function of the separable modes (`rcp` is `rcp_fast`).
fn ref_channel(
    st: &Stage<'_>,
    h: Hf,
    rcp: fn(f32) -> f32,
    s: f32,
    d: f32,
    sa: f32,
    da: f32,
) -> f32 {
    match st {
        Stage::Clear => 0.0,
        Stage::Srcatop => h.mad(s, da, d * inv(sa)),
        Stage::Dstatop => h.mad(d, sa, s * inv(da)),
        Stage::Srcin => s * da,
        Stage::Dstin => d * sa,
        Stage::Srcout => s * inv(da),
        Stage::Dstout => d * inv(sa),
        Stage::Srcover => h.mad(d, inv(sa), s),
        Stage::Dstover => h.mad(s, inv(da), d),
        Stage::Modulate => s * d,
        Stage::Multiply => h.mad(s, d, h.mad(s, inv(da), d * inv(sa))),
        Stage::Plus => fmin(s + d, 1.0),
        Stage::Screen => h.nmad(s, d, s + d),
        Stage::Xor => h.mad(s, inv(da), d * inv(sa)),
        Stage::Darken => s + d - fmax(s * da, d * sa),
        Stage::Lighten => s + d - fmin(s * da, d * sa),
        Stage::Difference => s + d - two(fmin(s * da, d * sa)),
        Stage::Exclusion => s + d - two(s * d),
        Stage::Colorburn => sel(
            d == da,
            d + s * inv(da),
            sel(
                s == 0.0,
                d * inv(sa),
                sa * (da - fmin(da, (da - d) * sa * rcp(s))) + s * inv(da) + d * inv(sa),
            ),
        ),
        Stage::Colordodge => sel(
            d == 0.0,
            s * inv(da),
            sel(
                s == sa,
                s + d * inv(sa),
                sa * fmin(da, (d * sa) * rcp(sa - s)) + s * inv(da) + d * inv(sa),
            ),
        ),
        Stage::Hardlight => {
            s * inv(da)
                + d * inv(sa)
                + sel(two(s) <= sa, two(s * d), sa * da - two((da - d) * (sa - s)))
        }
        Stage::Overlay => {
            s * inv(da)
                + d * inv(sa)
                + sel(two(d) <= da, two(s * d), sa * da - two((da - d) * (sa - s)))
        }
        Stage::Softlight => {
            let m = sel(da > 0.0, d / da, 0.0);
            let s2 = two(s);
            let m4 = two(two(m));
            let dark_src = d * (sa + (s2 - sa) * (1.0 - m));
            let dark_dst = (m4 * m4 + m4) * (m - 1.0) + 7.0 * m;
            let lite_dst = m.sqrt() - m;
            let lite_src = d * sa + da * (s2 - sa) * sel(two(two(d)) <= da, dark_dst, lite_dst);
            s * inv(da) + d * inv(sa) + sel(s2 <= sa, dark_src, lite_src)
        }
        other => panic!("not a separable mode: {other:?}"),
    }
}

fn lum(h: Hf, c: [f32; 3]) -> f32 {
    h.mad(c[0], 0.30, h.mad(c[1], 0.59, c[2] * 0.11))
}

fn sat(c: [f32; 3]) -> f32 {
    fmax(c[0], fmax(c[1], c[2])) - fmin(c[0], fmin(c[1], c[2]))
}

fn set_sat(rcp: fn(f32) -> f32, c: [f32; 3], s: f32) -> [f32; 3] {
    let mn = fmin(c[0], fmin(c[1], c[2]));
    let mx = fmax(c[0], fmax(c[1], c[2]));
    let sat = mx - mn;
    let s = sel(sat == 0.0, 0.0, s * rcp(sat));
    c.map(|v| (v - mn) * s)
}

fn set_lum(h: Hf, c: [f32; 3], l: f32) -> [f32; 3] {
    let diff = l - lum(h, c);
    c.map(|v| v + diff)
}

fn clip_color(h: Hf, rcp: fn(f32) -> f32, c: [f32; 3], a: f32) -> [f32; 3] {
    let mn = fmin(c[0], fmin(c[1], c[2]));
    let mx = fmax(c[0], fmax(c[1], c[2]));
    let l = lum(h, c);
    let mn_scale = l * rcp(l - mn);
    let mx_scale = (a - l) * rcp(mx - l);
    let clip_low = mn < 0.0 && l != mn;
    let clip_high = mx > a && l != mx;
    c.map(|v| {
        let v = sel(clip_low, h.mad(mn_scale, v - l, l), v);
        let v = sel(clip_high, h.mad(mx_scale, v - l, l), v);
        fmax(v, 0.0)
    })
}

/// A stage on one lane: `[r, g, b, a]` out.
fn ref_highp(st: &Stage<'_>, h: Hf, rcp: fn(f32) -> f32, px: Px) -> [f32; 4] {
    let [r, g, b, a, dr, dg, db, da] = px;
    if separable_all(st) {
        let f = |s, d, sa, da| ref_channel(st, h, rcp, s, d, sa, da);
        return [
            f(r, dr, a, da),
            f(g, dg, a, da),
            f(b, db, a, da),
            f(a, da, a, da),
        ];
    }
    let rgb_then_alpha =
        |rgb: [f32; 3]| -> [f32; 4] { [rgb[0], rgb[1], rgb[2], a + h.nmad(a, da, da)] };
    let combine = |c: [f32; 3]| -> [f32; 4] {
        let f = |s, d, c| h.mad(s, inv(da), h.mad(d, inv(a), c));
        rgb_then_alpha([f(r, dr, c[0]), f(g, dg, c[1]), f(b, db, c[2])])
    };
    match st {
        Stage::Hue => {
            let c = set_sat(rcp, [r * a, g * a, b * a], sat([dr, dg, db]) * a);
            let c = set_lum(h, c, lum(h, [dr, dg, db]) * a);
            combine(clip_color(h, rcp, c, a * da))
        }
        Stage::Saturation => {
            let c = set_sat(rcp, [dr * a, dg * a, db * a], sat([r, g, b]) * da);
            let c = set_lum(h, c, lum(h, [dr, dg, db]) * a);
            combine(clip_color(h, rcp, c, a * da))
        }
        Stage::Color => {
            let c = set_lum(h, [r * da, g * da, b * da], lum(h, [dr, dg, db]) * a);
            combine(clip_color(h, rcp, c, a * da))
        }
        Stage::Luminosity => {
            let c = set_lum(h, [dr * a, dg * a, db * a], lum(h, [r, g, b]) * da);
            combine(clip_color(h, rcp, c, a * da))
        }
        _ => {
            // Separable modes that srcover alpha.
            let f = |s, d| ref_channel(st, h, rcp, s, d, a, da);
            [f(r, dr), f(g, dg), f(b, db), h.mad(da, inv(a), a)]
        }
    }
}

/// Every highp blend stage and whether it uses `rcp_fast`.
fn highp_blends() -> Vec<(Stage<'static>, bool)> {
    use Stage::*;
    vec![
        (Clear, false),
        (Srcatop, false),
        (Dstatop, false),
        (Srcin, false),
        (Dstin, false),
        (Srcout, false),
        (Dstout, false),
        (Srcover, false),
        (Dstover, false),
        (Modulate, false),
        (Multiply, false),
        (Plus, false),
        (Screen, false),
        (Xor, false),
        (Darken, false),
        (Lighten, false),
        (Difference, false),
        (Exclusion, false),
        (Hardlight, false),
        (Overlay, false),
        (Softlight, false),
        (Colorburn, true),
        (Colordodge, true),
        (Hue, true),
        (Saturation, true),
        (Color, true),
        (Luminosity, true),
    ]
}

/// Equal as numbers (`-0 == +0`), or both NaN.
fn same(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

#[test]
fn highp_blends_match_the_reference() {
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let h = Hf {
            fused: fused(sel.tier),
        };
        let mut rng = Rng::new(0xb3_0001);
        for (st, rcp) in sample(highp_blends()) {
            // `rcp_fast` is 1/x on Scalar only (the others use estimates, covered by the twins).
            if rcp && sel.tier != Tier::Scalar {
                continue;
            }
            for round in 0..rounds() {
                let lanes: Vec<Px> = (0..n).map(|_| gen_px(&mut rng)).collect();
                let (src, dst) = highp_regs(&lanes);
                let (lowp, out) = run(&[st], sel, true, (0, n), (&src, &dst), &[]);
                assert!(!lowp);
                let got = floats(&out[..4 * 4 * n]);
                for (i, lane) in lanes.iter().enumerate() {
                    let want = ref_highp(&st, h, |x| 1.0 / x, *lane);
                    for c in 0..4 {
                        assert!(
                            same(got[c * n + i], want[c]),
                            "{sel} {st:?} round {round} lane {i} channel {c}: {lane:?} -> {} != {}",
                            got[c * n + i],
                            want[c]
                        );
                    }
                }
            }
        }
    }
}

// ~~~ The lowp reference ~~~

type W = Wrapping<u16>;

fn w(v: u16) -> W {
    Wrapping(v)
}

/// `div255` on the tier (x86: `(v+255)/256`; Neon: exact).
fn div255(t: Tier, v: W) -> W {
    if t == Tier::Neon {
        // vrshrq_n_u16(vrsraq_n_u16(v, v, 8), 8)
        let t = v.0.wrapping_add(((u32::from(v.0) + 128) >> 8) as u16);
        w(((u32::from(t) + 128) >> 8) as u16)
    } else {
        (v + w(255)) / w(256)
    }
}

/// `div255_accurate` on the tier.
fn div255_accurate(t: Tier, v: W) -> W {
    if t == Tier::Neon {
        div255(t, v)
    } else {
        let v = v + w(128);
        (v + v / w(256)) / w(256)
    }
}

fn linv(v: W) -> W {
    w(255) - v
}

fn lmin(a: W, b: W) -> W {
    if a < b { a } else { b }
}

fn lmax(a: W, b: W) -> W {
    if a > b { a } else { b }
}

fn lsel(c: bool, t: W, e: W) -> W {
    if c { t } else { e }
}

/// The lowp stages: their `name_channel`, and whether alpha uses the same (`true`) or srcover.
fn ref_lowp_channel(st: &Stage<'_>, t: Tier, s: W, d: W, sa: W, da: W) -> Option<(W, bool)> {
    let d255 = |v| div255(t, v);
    let acc = |v| div255_accurate(t, v);
    Some(match st {
        Stage::Clear => (w(0), true),
        Stage::Srcatop => (d255(s * da + d * linv(sa)), true),
        Stage::Dstatop => (d255(d * sa + s * linv(da)), true),
        Stage::Srcin => (acc(s * da), true),
        Stage::Dstin => (acc(d * sa), true),
        Stage::Srcout => (acc(s * linv(da)), true),
        Stage::Dstout => (acc(d * linv(sa)), true),
        Stage::Srcover => (s + acc(d * linv(sa)), true),
        Stage::Dstover => (d + acc(s * linv(da)), true),
        Stage::Modulate => (acc(s * d), true),
        Stage::Multiply => (d255(s * linv(da) + d * linv(sa) + s * d), true),
        Stage::Plus => (lmin(s + d, w(255)), true),
        Stage::Screen => (s + d - acc(s * d), true),
        Stage::Xor => (d255(s * linv(da) + d * linv(sa)), true),
        Stage::Darken => (s + d - d255(lmax(s * da, d * sa)), false),
        Stage::Lighten => (s + d - d255(lmin(s * da, d * sa)), false),
        Stage::Difference => (s + d - w(2) * d255(lmin(s * da, d * sa)), false),
        Stage::Exclusion => (s + d - w(2) * d255(s * d), false),
        Stage::Hardlight => (
            d255(
                s * linv(da)
                    + d * linv(sa)
                    + lsel(
                        w(2) * s <= sa,
                        w(2) * s * d,
                        sa * da - w(2) * (sa - s) * (da - d),
                    ),
            ),
            false,
        ),
        Stage::Overlay => (
            d255(
                s * linv(da)
                    + d * linv(sa)
                    + lsel(
                        w(2) * d <= da,
                        w(2) * s * d,
                        sa * da - w(2) * (sa - s) * (da - d),
                    ),
            ),
            false,
        ),
        _ => return None,
    })
}

fn ref_lowp(st: &Stage<'_>, t: Tier, px: [u16; 8]) -> [u16; 4] {
    let [r, g, b, a, dr, dg, db, da] = px.map(w);
    let ch = |s, d, sa, da| ref_lowp_channel(st, t, s, d, sa, da).unwrap();
    let (rr, _) = ch(r, dr, a, da);
    let (gg, _) = ch(g, dg, a, da);
    let (bb, _) = ch(b, db, a, da);
    let (aa, same_alpha) = ch(a, da, a, da);
    let alpha = if same_alpha {
        aa
    } else {
        a + div255(t, da * linv(a))
    };
    [rr.0, gg.0, bb.0, alpha.0]
}

fn lowp_blends() -> Vec<Stage<'static>> {
    use Stage::*;
    vec![
        Clear, Srcatop, Dstatop, Srcin, Dstin, Srcout, Dstout, Srcover, Dstover, Modulate,
        Multiply, Plus, Screen, Xor, Darken, Lighten, Difference, Exclusion, Hardlight, Overlay,
    ]
}

/// A lowp lane: mostly `[0, 255]` premul colors, sometimes any 16 bits.
fn gen_lowp_px(rng: &mut Rng) -> [u16; 8] {
    let mut px = [0u16; 8];
    for half in 0..2 {
        let a = rng.below(256) as u16;
        let a = match rng.below(6) {
            0 => 0,
            1 => 255,
            _ => a,
        };
        px[3 + 4 * half] = a;
        for c in 0..3 {
            px[c + 4 * half] = match rng.below(8) {
                0 => 0,
                1 => a,
                2 => 255,
                3 => rng.next_u32() as u16,
                _ => rng.below(usize::from(a) + 1) as u16,
            };
        }
    }
    px
}

fn lowp_regs(lanes: &[[u16; 8]]) -> (Vec<u8>, Vec<u8>) {
    let reg = |range: core::ops::Range<usize>| -> Vec<u8> {
        u16_bytes(
            &range
                .flat_map(|c| lanes.iter().map(move |l| l[c]))
                .collect::<Vec<_>>(),
        )
    };
    (reg(0..4), reg(4..8))
}

#[test]
fn lowp_blends_match_the_reference() {
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        let mut rng = Rng::new(0xb3_0002);
        for st in sample(lowp_blends()) {
            for round in 0..rounds() {
                let lanes: Vec<[u16; 8]> = (0..n).map(|_| gen_lowp_px(&mut rng)).collect();
                let (src, dst) = lowp_regs(&lanes);
                let (lowp, out) = run(&[st], sel, false, (0, n), (&src, &dst), &[]);
                assert!(lowp, "{st:?} is lowp");
                let got = halves(&out[..4 * 2 * n]);
                for (i, lane) in lanes.iter().enumerate() {
                    let want = ref_lowp(&st, sel.tier, *lane);
                    for c in 0..4 {
                        assert_eq!(
                            got[c * n + i],
                            want[c],
                            "{sel} {st:?} round {round} lane {i} channel {c}: {lane:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn highp_only_blends_are_not_lowp() {
    for sel in selections() {
        for st in [
            Stage::Colorburn,
            Stage::Colordodge,
            Stage::Softlight,
            Stage::Hue,
            Stage::Saturation,
            Stage::Color,
            Stage::Luminosity,
        ] {
            assert!(!Program::new(&[st], sel, false).is_lowp(), "{sel} {st:?}");
        }
    }
}

// ~~~ Known answers ~~~

#[test]
fn multiply_known_answers() {
    for sel in selections() {
        // highp: src = (0.5, 0.25, 0, 0.5), dst = (1, 0.5, 0.25, 1)
        let n = sel.tier.highp_stride();
        let lane: Px = [0.5, 0.25, 0.0, 0.5, 1.0, 0.5, 0.25, 1.0];
        let (src, dst) = highp_regs(&vec![lane; n]);
        let (_, out) = run(&[Stage::Multiply], sel, true, (0, n), (&src, &dst), &[]);
        let f = floats(&out[..16 * n]);
        // s*d + s*(1-da) + d*(1-sa)
        for (c, want) in [1.0f32, 0.375, 0.125, 1.0].iter().enumerate() {
            assert!(
                f[c * n..(c + 1) * n].iter().all(|v| v == want),
                "{sel} {c}: {f:?}"
            );
        }
        // lowp: opaque black times opaque white stays black, 50% over opaque is the product.
        if let Some(n) = sel.tier.lowp_stride() {
            let lane = [128u16, 255, 0, 255, 255, 128, 64, 255];
            let (src, dst) = lowp_regs(&vec![lane; n]);
            let (lowp, out) = run(&[Stage::Multiply], sel, false, (0, n), (&src, &dst), &[]);
            assert!(lowp);
            let h = halves(&out[..8 * n]);
            // da = sa = 255: div255(s*d) for each channel, alpha 255.
            let want = |s: u32, d: u32| (s * d).div_ceil(256) as u16;
            let want = [want(128, 255), want(255, 128), 0, 255];
            if sel.tier != Tier::Neon {
                for (c, w) in want.iter().enumerate() {
                    assert!(
                        h[c * n..(c + 1) * n].iter().all(|v| v == w),
                        "{sel} {c}: {h:?}"
                    );
                }
            }
        }
    }
}

// ~~~ Stage twins ~~~

/// The native tiers of this host with their models.
fn twin_sets() -> Vec<(Selection, Vec<Selection>)> {
    if cfg!(miri) {
        return Vec::new();
    }
    SIMD.into_iter()
        .filter(|t| t.is_native())
        .map(|t| (Selection::native(t), models(t)))
        .collect()
}

#[test]
fn highp_blend_twins() {
    for (native, models) in twin_sets() {
        let n = native.tier.highp_stride();
        let mut rng = Rng::new(0xb3_0003);
        for (st, estimates) in highp_blends() {
            for round in 0..rounds() {
                let lanes: Vec<Px> = (0..n).map(|_| gen_px(&mut rng)).collect();
                let (src, dst) = highp_regs(&lanes);
                let (_, want) = run(&[st], native, true, (0, n), (&src, &dst), &[]);
                for model in &models {
                    // `rcp_fast` differs between estimate sources: only the host's model is the
                    // native tier's twin.
                    if estimates && model.backend != Backend::Model(Estimates::Host) {
                        continue;
                    }
                    let (_, got) = run(&[st], *model, true, (0, n), (&src, &dst), &[]);
                    assert_eq!(
                        got[..16 * n],
                        want[..16 * n],
                        "{native} vs {model}, {st:?}, round {round}"
                    );
                }
            }
        }
    }
}

#[test]
fn lowp_blend_twins() {
    for (native, models) in twin_sets() {
        let n = native.tier.lowp_stride().unwrap();
        let mut rng = Rng::new(0xb3_0004);
        for st in lowp_blends() {
            for round in 0..rounds() {
                let lanes: Vec<[u16; 8]> = (0..n).map(|_| gen_lowp_px(&mut rng)).collect();
                let (src, dst) = lowp_regs(&lanes);
                let (lowp, want) = run(&[st], native, false, (0, n), (&src, &dst), &[]);
                assert!(lowp);
                for model in &models {
                    let (_, got) = run(&[st], *model, false, (0, n), (&src, &dst), &[]);
                    assert_eq!(
                        got[..8 * n],
                        want[..8 * n],
                        "{native} vs {model}, {st:?}, round {round}"
                    );
                }
            }
        }
    }
}

// ~~~ Coverage stages ~~~

/// The coverage stages under test.
#[derive(Clone, Copy, Debug)]
enum Cover {
    Scale1Float(f32),
    Lerp1Float(f32),
    ScaleU8,
    LerpU8,
    Scale565,
    Lerp565,
    ScaleNative,
    LerpNative,
}

impl Cover {
    const ALL: [Cover; 8] = [
        Cover::Scale1Float(0.0),
        Cover::Lerp1Float(0.0),
        Cover::ScaleU8,
        Cover::LerpU8,
        Cover::Scale565,
        Cover::Lerp565,
        Cover::ScaleNative,
        Cover::LerpNative,
    ];

    fn is_lerp(self) -> bool {
        matches!(
            self,
            Cover::Lerp1Float(_) | Cover::LerpU8 | Cover::Lerp565 | Cover::LerpNative
        )
    }
}

/// Runs a coverage stage (the `Cell` lives here so the stage can borrow it).
fn run_cover(
    cover: Cover,
    sel: Selection,
    force_highp: bool,
    at: (usize, usize),
    regs: (&[u8], &[u8]),
    mem: &[u8],
) -> (bool, Vec<u8>) {
    let coverage = Cell::new(match cover {
        Cover::Scale1Float(c) | Cover::Lerp1Float(c) => c,
        _ => 0.0,
    });
    let ctx = MemoryCtx::new(COVER);
    let native = MemPtr::new(COVER, 0);
    let stage = match cover {
        Cover::Scale1Float(_) => Stage::Scale1Float(&coverage),
        Cover::Lerp1Float(_) => Stage::Lerp1Float(&coverage),
        Cover::ScaleU8 => Stage::ScaleU8(ctx),
        Cover::LerpU8 => Stage::LerpU8(ctx),
        Cover::Scale565 => Stage::Scale565(ctx),
        Cover::Lerp565 => Stage::Lerp565(ctx),
        Cover::ScaleNative => Stage::ScaleNative(native),
        Cover::LerpNative => Stage::LerpNative(native),
    };
    run(&[stage], sel, force_highp, at, regs, mem)
}

/// 565 channels as floats, as `from_565` (highp).
fn from_565_f(p: u16) -> [f32; 3] {
    let wide = u32::from(p);
    [
        (wide & (31 << 11)) as f32 * (1.0 / (31 << 11) as f32),
        (wide & (63 << 5)) as f32 * (1.0 / (63 << 5) as f32),
        (wide & 31) as f32 * (1.0 / 31.0),
    ]
}

/// 565 channels as bytes, as `from_565` (lowp).
fn from_565_b(p: u16) -> [u16; 3] {
    let (r, g, b) = ((p >> 11) & 31, (p >> 5) & 63, p & 31);
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

#[test]
fn highp_coverage_matches_the_reference() {
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let h = Hf {
            fused: fused(sel.tier),
        };
        let mut rng = Rng::new(0xb3_0005);
        for cover in Cover::ALL
            .into_iter()
            .step_by(if cfg!(miri) { 3 } else { 1 })
        {
            for round in 0..rounds() {
                // Tail chunks (w < n) and full chunks, at an offset into the coverage memory.
                let w = if round % 2 == 0 { n } else { (n - 1).max(1) };
                let x0 = rng.below(4);
                let cover = match cover {
                    Cover::Scale1Float(_) => Cover::Scale1Float(unit(&mut rng)),
                    Cover::Lerp1Float(_) => Cover::Lerp1Float(unit(&mut rng)),
                    c => c,
                };
                let lanes: Vec<Px> = (0..n).map(|_| gen_px(&mut rng)).collect();
                let (src, dst) = highp_regs(&lanes);
                let bytes: Vec<u8> = (0..64).map(|_| rng.next_u32() as u8).collect();
                let floats_mem: Vec<f32> = (0..n).map(|_| unit(&mut rng)).collect();
                let mem = match cover {
                    Cover::ScaleNative | Cover::LerpNative => f32_bytes(&floats_mem),
                    _ => bytes.clone(),
                };
                let (lowp, out) = run_cover(cover, sel, true, (x0, w), (&src, &dst), &mem);
                assert!(!lowp);
                let got = floats(&out[..16 * n]);
                for (i, l) in lanes.iter().enumerate().take(w) {
                    let [r, g, b, a, dr, dg, db, da] = *l;
                    let lerp = |from: f32, to: f32, t: f32| h.mad(to - from, t, from);
                    let (cr, cg, cb, ca) = match cover {
                        Cover::Scale1Float(c) | Cover::Lerp1Float(c) => (c, c, c, c),
                        Cover::ScaleU8 | Cover::LerpU8 => {
                            let c = f32::from(bytes[x0 + i]) * (1.0 / 255.0);
                            (c, c, c, c)
                        }
                        Cover::ScaleNative | Cover::LerpNative => {
                            let c = floats_mem[i];
                            (c, c, c, c)
                        }
                        Cover::Scale565 | Cover::Lerp565 => {
                            let p =
                                u16::from_ne_bytes([bytes[2 * (x0 + i)], bytes[2 * (x0 + i) + 1]]);
                            let [cr, cg, cb] = from_565_f(p);
                            let ca = if a < da {
                                fmin(cr, fmin(cg, cb))
                            } else {
                                fmax(cr, fmax(cg, cb))
                            };
                            (cr, cg, cb, ca)
                        }
                    };
                    let want = if cover.is_lerp() {
                        [
                            lerp(dr, r, cr),
                            lerp(dg, g, cg),
                            lerp(db, b, cb),
                            lerp(da, a, ca),
                        ]
                    } else {
                        [r * cr, g * cg, b * cb, a * ca]
                    };
                    for c in 0..4 {
                        assert!(
                            same(got[c * n + i], want[c]),
                            "{sel} {cover:?} round {round} lane {i} channel {c}: {l:?}: {} != {}",
                            got[c * n + i],
                            want[c]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn lowp_coverage_matches_the_reference() {
    for sel in selections() {
        let Some(n) = sel.tier.lowp_stride() else {
            continue;
        };
        let t = sel.tier;
        let mut rng = Rng::new(0xb3_0006);
        for cover in Cover::ALL
            .into_iter()
            .step_by(if cfg!(miri) { 3 } else { 1 })
        {
            for round in 0..rounds() {
                let w = if round % 2 == 0 { n } else { (n - 1).max(1) };
                let x0 = rng.below(4);
                let cover = match cover {
                    Cover::Scale1Float(_) => Cover::Scale1Float(unit(&mut rng)),
                    Cover::Lerp1Float(_) => Cover::Lerp1Float(unit(&mut rng)),
                    c => c,
                };
                let lanes: Vec<[u16; 8]> = (0..n).map(|_| gen_lowp_px(&mut rng)).collect();
                let (src, dst) = lowp_regs(&lanes);
                let bytes: Vec<u8> = (0..64).map(|_| rng.next_u32() as u8).collect();
                // `scale_native`/`lerp_native` read `N` 16-bit coverages.
                let native_mem: Vec<u16> = (0..n).map(|_| rng.below(256) as u16).collect();
                let mem = match cover {
                    Cover::ScaleNative | Cover::LerpNative => u16_bytes(&native_mem),
                    _ => bytes.clone(),
                };
                let (lowp, out) = run_cover(cover, sel, false, (x0, w), (&src, &dst), &mem);
                assert!(lowp);
                let got = halves(&out[..8 * n]);
                for (i, l) in lanes.iter().enumerate().take(w) {
                    let [r, g, b, a, dr, dg, db, da] = l.map(w_);
                    let d255 = |v| div255(t, v);
                    let lerp = |from: W, to: W, c: W| d255(from * linv(c) + to * c);
                    let (cr, cg, cb, ca) = match cover {
                        Cover::Scale1Float(c) | Cover::Lerp1Float(c) => {
                            let c = w_((c * 255.0 + 0.5) as u16);
                            (c, c, c, c)
                        }
                        Cover::ScaleU8 | Cover::LerpU8 => {
                            let c = w_(u16::from(bytes[x0 + i]));
                            (c, c, c, c)
                        }
                        Cover::ScaleNative | Cover::LerpNative => {
                            let c = w_(native_mem[i]);
                            (c, c, c, c)
                        }
                        Cover::Scale565 | Cover::Lerp565 => {
                            let p =
                                u16::from_ne_bytes([bytes[2 * (x0 + i)], bytes[2 * (x0 + i) + 1]]);
                            let [cr, cg, cb] = from_565_b(p).map(w_);
                            let ca = if a < da {
                                lmin(cr, lmin(cg, cb))
                            } else {
                                lmax(cr, lmax(cg, cb))
                            };
                            (cr, cg, cb, ca)
                        }
                    };
                    let want = if cover.is_lerp() {
                        [
                            lerp(dr, r, cr),
                            lerp(dg, g, cg),
                            lerp(db, b, cb),
                            lerp(da, a, ca),
                        ]
                    } else {
                        [d255(r * cr), d255(g * cg), d255(b * cb), d255(a * ca)]
                    };
                    for c in 0..4 {
                        assert_eq!(
                            got[c * n + i],
                            want[c].0,
                            "{sel} {cover:?} round {round} lane {i} channel {c}: {l:?}"
                        );
                    }
                }
            }
        }
    }
}

fn w_(v: u16) -> W {
    Wrapping(v)
}

#[test]
fn coverage_twins() {
    for (native, models) in twin_sets() {
        let mut rng = Rng::new(0xb3_0007);
        for lowp in [false, true] {
            let n = if lowp {
                native.tier.lowp_stride().unwrap()
            } else {
                native.tier.highp_stride()
            };
            for cover in Cover::ALL {
                for round in 0..rounds() {
                    let w = if round % 2 == 0 { n } else { (n - 1).max(1) };
                    let x0 = rng.below(4);
                    let cover = match cover {
                        Cover::Scale1Float(_) => Cover::Scale1Float(unit(&mut rng)),
                        Cover::Lerp1Float(_) => Cover::Lerp1Float(unit(&mut rng)),
                        c => c,
                    };
                    let (src, dst) = if lowp {
                        let lanes: Vec<_> = (0..n).map(|_| gen_lowp_px(&mut rng)).collect();
                        lowp_regs(&lanes)
                    } else {
                        let lanes: Vec<_> = (0..n).map(|_| gen_px(&mut rng)).collect();
                        highp_regs(&lanes)
                    };
                    let mem: Vec<u8> = (0..64).map(|_| rng.next_u32() as u8).collect();
                    // Native coverage is `n` floats / 16-bit values: keep floats finite.
                    let mem = if !lowp && matches!(cover, Cover::ScaleNative | Cover::LerpNative) {
                        f32_bytes(&(0..n).map(|_| unit(&mut rng)).collect::<Vec<_>>())
                    } else {
                        mem
                    };
                    let (is_lowp, want) =
                        run_cover(cover, native, !lowp, (x0, w), (&src, &dst), &mem);
                    assert_eq!(is_lowp, lowp);
                    for model in &models {
                        let (_, got) = run_cover(cover, *model, !lowp, (x0, w), (&src, &dst), &mem);
                        let len = if lowp { 8 * n } else { 16 * n };
                        assert_eq!(
                            got[..len],
                            want[..len],
                            "{native} vs {model}, {cover:?}, lowp {lowp}, round {round}"
                        );
                    }
                }
            }
        }
    }
}
