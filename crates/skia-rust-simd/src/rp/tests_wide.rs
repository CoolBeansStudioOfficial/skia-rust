// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the wide memory stages (task B2): known answers against reference decoders on every
//! selection this host can run, store/load round trips (including the tail chunk), gathers
//! against loads, and stage twins (native vs the models, bit for bit, on random bytes and
//! special floats).

// Test data is built from small indices; the casts are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use super::contexts::GatherCtx;
use super::lanes::test_support::{Rng, float_specials};
use super::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

/// Memory slots: pixels in, pixels out, registers in, registers out.
const PIX_IN: MemSlot = MemSlot(0);
const PIX_OUT: MemSlot = MemSlot(1);
const REGS_IN: MemPtr = MemPtr::new(MemSlot(2), 0);
const REGS_OUT: MemPtr = MemPtr::new(MemSlot(3), 0);
/// Bytes of four registers of the widest tier.
const REGS_BYTES: usize = 4 * 4 * 16;

/// Every selection this host can run (under Miri: Scalar and the `AmdZen4`/`Arm` models).
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

/// The models of `t` this host can run.
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

/// `selections()`, thinned under Miri to Scalar and the `Sse2`, `Ml4` and `Neon` models (one per
/// stride class), for the tests that cost one pipeline run per case.
fn sample_selections() -> Vec<Selection> {
    selections()
        .into_iter()
        .filter(|s| !cfg!(miri) || !matches!(s.tier, Tier::Sse41 | Tier::Ml3))
        .collect()
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

fn lanes(sel: Selection) -> usize {
    sel.tier.highp_stride()
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

/// What a run leaves behind.
#[derive(Debug, PartialEq)]
struct Out {
    pixels: Vec<u8>,
    regs: Vec<u8>,
}

/// Runs `stages` over `w` pixels of row 0 with `pix_in` bound (read-only) and `regs_in` bound as
/// the input registers; returns the pixel output (`pix_out_len` bytes) and the output registers.
fn run(
    stages: &[Stage<'_>],
    sel: Selection,
    w: usize,
    pix_in: &[u8],
    pix_out_len: usize,
    regs_in: &[u8],
) -> Out {
    let mut rin = [0u8; REGS_BYTES];
    rin[..regs_in.len()].copy_from_slice(regs_in);
    let mut rout = [0u8; REGS_BYTES];
    let mut pout = vec![0u8; pix_out_len];
    let mut program = Program::new(stages, sel, true);
    let mut mem = MemoryBindings::new()
        .with(PIX_IN, MemView::read(pix_in))
        .with(PIX_OUT, MemView::write(&mut pout))
        .with(MemSlot(2), MemView::read(&rin))
        .with(MemSlot(3), MemView::write(&mut rout));
    program.run(0, 0, w, 1, &mut mem);
    drop(mem);
    Out {
        pixels: pout,
        regs: rout.to_vec(),
    }
}

// ~~~ Reference decoders ~~~

fn u16_at(px: &[u8], k: usize) -> u16 {
    u16::from_ne_bytes([px[2 * k], px[2 * k + 1]])
}

fn u32_at(px: &[u8]) -> u32 {
    u32::from_ne_bytes([px[0], px[1], px[2], px[3]])
}

fn unorm16(px: &[u8], k: usize) -> f32 {
    f32::from(u16_at(px, k)) * (1.0 / 65535.0)
}

/// A half with a normal exponent (no zero, denormal, infinity or NaN: `from_half` flushes
/// denormals and `to_half` truncates).
fn half_to_f32(h: u16) -> f32 {
    let s = u32::from(h & 0x8000) << 16;
    let em = u32::from(h & 0x7fff);
    f32::from_bits(s | ((em << 13) + ((127 - 15) << 23)))
}

fn xr10(v: u32) -> f32 {
    (v as f32 - 384.0) * (1.0 / 510.0)
}

fn dec_16161616(px: &[u8]) -> [f32; 4] {
    [
        unorm16(px, 0),
        unorm16(px, 1),
        unorm16(px, 2),
        unorm16(px, 3),
    ]
}
fn dec_a16(px: &[u8]) -> [f32; 4] {
    [0.0, 0.0, 0.0, unorm16(px, 0)]
}
fn dec_r16(px: &[u8]) -> [f32; 4] {
    [unorm16(px, 0), 0.0, 0.0, 1.0]
}
fn dec_rg1616(px: &[u8]) -> [f32; 4] {
    [unorm16(px, 0), unorm16(px, 1), 0.0, 1.0]
}
fn dec_f16(px: &[u8]) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| half_to_f32(u16_at(px, k)))
}
fn dec_af16(px: &[u8]) -> [f32; 4] {
    [0.0, 0.0, 0.0, half_to_f32(u16_at(px, 0))]
}
fn dec_rf16(px: &[u8]) -> [f32; 4] {
    [half_to_f32(u16_at(px, 0)), 0.0, 0.0, 1.0]
}
fn dec_rgf16(px: &[u8]) -> [f32; 4] {
    [
        half_to_f32(u16_at(px, 0)),
        half_to_f32(u16_at(px, 1)),
        0.0,
        1.0,
    ]
}
fn dec_f32(px: &[u8]) -> [f32; 4] {
    [0, 1, 2, 3].map(|c| f32::from_ne_bytes(px[4 * c..4 * c + 4].try_into().unwrap()))
}
fn dec_1010102(px: &[u8]) -> [f32; 4] {
    let v = u32_at(px);
    [
        (v & 0x3ff) as f32 * (1.0 / 1023.0),
        ((v >> 10) & 0x3ff) as f32 * (1.0 / 1023.0),
        ((v >> 20) & 0x3ff) as f32 * (1.0 / 1023.0),
        (v >> 30) as f32 * (1.0 / 3.0),
    ]
}
fn dec_1010102_xr(px: &[u8]) -> [f32; 4] {
    let v = u32_at(px);
    [
        xr10(v & 0x3ff),
        xr10((v >> 10) & 0x3ff),
        xr10((v >> 20) & 0x3ff),
        (v >> 30) as f32 * (1.0 / 3.0),
    ]
}
fn dec_10x6(px: &[u8]) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| f32::from((u16_at(px, k) >> 6) & 0x3ff) * (1.0 / 1023.0))
}
fn dec_10101010_xr(px: &[u8]) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| xr10(u32::from((u16_at(px, k) >> 6) & 0x3ff)))
}

// ~~~ Pixel generators (valid encodings that round trip) ~~~

fn gen_u16s(rng: &mut Rng, count: usize, f: impl Fn(u16) -> u16) -> Vec<u8> {
    (0..count)
        .flat_map(|_| f(rng.next_u32() as u16).to_ne_bytes())
        .collect()
}

/// Any bits.
fn gen_any(rng: &mut Rng, count: usize) -> Vec<u8> {
    gen_u16s(rng, count, |h| h)
}

/// Halves with a normal exponent (1..=30).
fn gen_halfs(rng: &mut Rng, count: usize) -> Vec<u8> {
    gen_u16s(rng, count, |h| {
        let exp = (h >> 10) & 0x1f;
        let exp = 1 + exp % 30;
        (h & 0x8000) | (exp << 10) | (h & 0x3ff)
    })
}

/// 10-bit channels with 6 bits of padding below.
fn gen_10x6(rng: &mut Rng, count: usize) -> Vec<u8> {
    gen_u16s(rng, count, |h| h & 0xffc0)
}

/// Normal floats.
fn gen_f32(rng: &mut Rng, count: usize) -> Vec<u8> {
    (0..count)
        .flat_map(|_| {
            let v = (rng.next_u32() as i32 % 2000 - 1000) as f32 / 8.0;
            v.to_ne_bytes()
        })
        .collect()
}

/// One format's stages and reference.
struct Fmt {
    name: &'static str,
    bpp: usize,
    load: fn(MemoryCtx) -> Stage<'static>,
    load_dst: fn(MemoryCtx) -> Stage<'static>,
    store: fn(MemoryCtx) -> Stage<'static>,
    gather: for<'a> fn(&'a GatherCtx<'a>) -> Stage<'a>,
    /// Random valid pixels: `count` of them.
    generate: fn(&mut Rng, usize) -> Vec<u8>,
    decode: fn(&[u8]) -> [f32; 4],
}

macro_rules! fmt {
    ($name:literal, $bpp:expr, $load:ident, $dst:ident, $store:ident, $gather:ident, $gen:expr, $dec:expr) => {
        Fmt {
            name: $name,
            bpp: $bpp,
            load: Stage::$load,
            load_dst: Stage::$dst,
            store: Stage::$store,
            gather: |c| Stage::$gather(c),
            generate: $gen,
            decode: $dec,
        }
    };
}

#[allow(clippy::too_many_lines)] // one row per format
fn formats() -> Vec<Fmt> {
    vec![
        fmt!(
            "16161616",
            8,
            Load16161616,
            Load16161616Dst,
            Store16161616,
            Gather16161616,
            |r, n| gen_any(r, 4 * n),
            dec_16161616
        ),
        fmt!(
            "a16",
            2,
            LoadA16,
            LoadA16Dst,
            StoreA16,
            GatherA16,
            |r, n| gen_any(r, n),
            dec_a16
        ),
        fmt!(
            "r16",
            2,
            LoadR16,
            LoadR16Dst,
            StoreR16,
            GatherR16,
            |r, n| gen_any(r, n),
            dec_r16
        ),
        fmt!(
            "rg1616",
            4,
            LoadRg1616,
            LoadRg1616Dst,
            StoreRg1616,
            GatherRg1616,
            |r, n| gen_any(r, 2 * n),
            dec_rg1616
        ),
        fmt!(
            "f16",
            8,
            LoadF16,
            LoadF16Dst,
            StoreF16,
            GatherF16,
            |r, n| gen_halfs(r, 4 * n),
            dec_f16
        ),
        fmt!(
            "af16",
            2,
            LoadAf16,
            LoadAf16Dst,
            StoreAf16,
            GatherAf16,
            |r, n| gen_halfs(r, n),
            dec_af16
        ),
        fmt!(
            "rf16",
            2,
            LoadRf16,
            LoadRf16Dst,
            StoreRf16,
            GatherRf16,
            |r, n| gen_halfs(r, n),
            dec_rf16
        ),
        fmt!(
            "rgf16",
            4,
            LoadRgf16,
            LoadRgf16Dst,
            StoreRgf16,
            GatherRgf16,
            |r, n| gen_halfs(r, 2 * n),
            dec_rgf16
        ),
        fmt!(
            "f32",
            16,
            LoadF32,
            LoadF32Dst,
            StoreF32,
            GatherF32,
            |r, n| gen_f32(r, 4 * n),
            dec_f32
        ),
        fmt!(
            "1010102",
            4,
            Load1010102,
            Load1010102Dst,
            Store1010102,
            Gather1010102,
            |r, n| gen_any(r, 2 * n),
            dec_1010102
        ),
        fmt!(
            "1010102_xr",
            4,
            Load1010102Xr,
            Load1010102XrDst,
            Store1010102Xr,
            Gather1010102Xr,
            |r, n| gen_any(r, 2 * n),
            dec_1010102_xr
        ),
        fmt!(
            "10x6",
            8,
            Load10x6,
            Load10x6Dst,
            Store10x6,
            Gather10x6,
            |r, n| gen_10x6(r, 4 * n),
            dec_10x6
        ),
        fmt!(
            "10101010_xr",
            8,
            Load10101010Xr,
            Load10101010XrDst,
            Store10101010Xr,
            Gather10101010Xr,
            |r, n| gen_10x6(r, 4 * n),
            dec_10101010_xr
        ),
    ]
}

const SRC: MemoryCtx = MemoryCtx::new(PIX_IN);
const DST: MemoryCtx = MemoryCtx::new(PIX_OUT);

#[test]
fn loads_decode_like_the_reference() {
    for sel in selections() {
        let n = lanes(sel);
        let mut rng = Rng::new(0xb2_0001);
        for f in formats() {
            let w = n; // one full chunk
            let px = (f.generate)(&mut rng, w);
            for dst in [false, true] {
                let stages = if dst {
                    [(f.load_dst)(SRC), Stage::StoreDst(REGS_OUT)]
                } else {
                    [(f.load)(SRC), Stage::StoreSrc(REGS_OUT)]
                };
                let out = run(&stages, sel, w, &px, 0, &[]);
                let regs = floats(&out.regs);
                for i in 0..w {
                    let want = (f.decode)(&px[i * f.bpp..(i + 1) * f.bpp]);
                    for c in 0..4 {
                        assert_eq!(
                            regs[c * n + i].to_bits(),
                            want[c].to_bits(),
                            "{sel} {} dst={dst} pixel {i} channel {c}",
                            f.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn stores_round_trip_through_loads_with_a_tail() {
    for sel in sample_selections() {
        let n = lanes(sel);
        let mut rng = Rng::new(0xb2_0002);
        for f in formats() {
            // Full chunks plus a tail chunk (every lane count from 1 to n).
            let widths = if cfg!(miri) {
                vec![n, 2 * n + 1]
            } else {
                vec![n, 2 * n, 2 * n + 1, 3 * n - n / 2]
            };
            for w in widths {
                let px = (f.generate)(&mut rng, w);
                let out = run(
                    &[(f.load)(SRC), (f.store)(DST)],
                    sel,
                    w,
                    &px,
                    w * f.bpp,
                    &[],
                );
                assert_eq!(out.pixels, px, "{sel} {} w={w}", f.name);
            }
        }
    }
}

#[test]
fn gathers_match_loads() {
    const W: usize = 5;
    const H: usize = 3;
    for sel in sample_selections() {
        let n = lanes(sel);
        let mut rng = Rng::new(0xb2_0003);
        for f in formats() {
            let image = (f.generate)(&mut rng, W * H);
            for round_down in [false, true] {
                let ctx = GatherCtx {
                    pixels: (&image).into(),
                    stride: W as i32,
                    width: W as f32,
                    height: H as f32,
                    weights: [0.0; 16],
                    round_down_at_integer: round_down,
                };
                // Pixel centers (every lane a different pixel), then edge cases: a coordinate
                // below 0, past the edge, and exactly on a pixel boundary.
                let mut xs = Vec::new();
                let mut ys = Vec::new();
                let mut want = Vec::new();
                for i in 0..n {
                    let (x, y) = (i % W, (i / W) % H);
                    xs.push(x as f32 + 0.5);
                    ys.push(y as f32 + 0.5);
                    want.push((x, y));
                }
                let edge = [
                    (-3.0, 1.5, (0, 1)),
                    (W as f32 + 10.0, 2.5, (W - 1, 2)),
                    (2.0, 0.5, (if round_down { 1 } else { 2 }, 0)),
                    (2.5, -0.25, (2, 0)),
                    (1e30, 1e30, (W - 1, H - 1)),
                ];
                for (i, (x, y, p)) in edge.into_iter().enumerate().take(n) {
                    xs[i] = x;
                    ys[i] = y;
                    want[i] = p;
                }
                let regs: Vec<f32> = xs
                    .iter()
                    .copied()
                    .chain(ys.iter().copied())
                    .chain(vec![0.0; 2 * n])
                    .collect();
                let out = run(
                    &[
                        Stage::LoadSrc(REGS_IN),
                        (f.gather)(&ctx),
                        Stage::StoreSrc(REGS_OUT),
                    ],
                    sel,
                    n,
                    &[],
                    0,
                    &f32_bytes(&regs),
                );
                let got = floats(&out.regs);
                for (i, (x, y)) in want.into_iter().enumerate() {
                    let px = &image[(y * W + x) * f.bpp..(y * W + x + 1) * f.bpp];
                    let expect = (f.decode)(px);
                    for c in 0..4 {
                        assert_eq!(
                            got[c * n + i].to_bits(),
                            expect[c].to_bits(),
                            "{sel} {} round_down={round_down} lane {i} channel {c}",
                            f.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn src_rg_round_trip() {
    for sel in selections() {
        let n = lanes(sel);
        let src: Vec<f32> = (0..4 * n).map(|i| i as f32 * 0.5 - 3.0).collect();
        // store_src_rg writes r then g.
        let out = run(
            &[Stage::LoadSrc(REGS_IN), Stage::StoreSrcRg(REGS_OUT)],
            sel,
            n,
            &[],
            0,
            &f32_bytes(&src),
        );
        let got = floats(&out.regs);
        assert_eq!(&got[..2 * n], &src[..2 * n], "{sel}");
        assert!(got[2 * n..].iter().all(|&v| v == 0.0), "{sel}");
        // load_src_rg reads r,g and leaves the other registers alone (zero).
        let back = run(
            &[Stage::LoadSrcRg(REGS_IN), Stage::StoreSrc(REGS_OUT)],
            sel,
            n,
            &[],
            0,
            &f32_bytes(&src),
        );
        let got = floats(&back.regs);
        assert_eq!(&got[..2 * n], &src[..2 * n], "{sel}");
        assert!(got[2 * n..4 * n].iter().all(|&v| v == 0.0), "{sel}");
    }
}

// ~~~ Stage twins ~~~

/// Random bytes, with some lanes drawn from the special floats.
fn random_bytes(rng: &mut Rng, specials: &[u32], len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        let w = if rng.below(2) == 0 {
            rng.next_u32()
        } else {
            rng.pick(specials)
        };
        v.extend_from_slice(&w.to_ne_bytes());
    }
    v.truncate(len);
    v
}

#[test]
fn wide_stage_twins() {
    let specials = float_specials();
    for (native, models) in twin_sets() {
        let n = lanes(native);
        let mut rng = Rng::new(0xb2_0004);
        for f in formats() {
            for round in 0..60 {
                // A tail chunk on every other round.
                let w = if round % 2 == 0 { 2 * n } else { 2 * n + 1 };
                let px = random_bytes(&mut rng, &specials, w * f.bpp);
                let regs = random_bytes(&mut rng, &specials, 4 * 4 * n);
                let image = random_bytes(&mut rng, &specials, 12 * f.bpp);
                let ctx = GatherCtx {
                    pixels: (&image).into(),
                    stride: 4,
                    width: 4.0,
                    height: 3.0,
                    weights: [0.0; 16],
                    round_down_at_integer: round % 4 < 2,
                };
                let pipelines: [Vec<Stage<'_>>; 5] = [
                    vec![(f.load)(SRC), Stage::StoreSrc(REGS_OUT)],
                    vec![(f.load_dst)(SRC), Stage::StoreDst(REGS_OUT)],
                    vec![Stage::LoadSrc(REGS_IN), (f.store)(DST)],
                    vec![
                        Stage::LoadSrc(REGS_IN),
                        (f.gather)(&ctx),
                        Stage::StoreSrc(REGS_OUT),
                    ],
                    vec![
                        Stage::LoadSrc(REGS_IN),
                        Stage::StoreSrcRg(MemPtr::new(MemSlot(3), 0)),
                        Stage::LoadSrcRg(MemPtr::new(MemSlot(3), 0)),
                        Stage::StoreSrc(MemPtr::new(MemSlot(1), 0)),
                    ],
                ];
                for (k, stages) in pipelines.iter().enumerate() {
                    // The last pipeline writes registers into the pixel buffer.
                    let out_len = if k == 4 { 4 * 4 * n } else { w * f.bpp };
                    let a = run(stages, native, w, &px, out_len, &regs);
                    for model in &models {
                        let b = run(stages, *model, w, &px, out_len, &regs);
                        assert_eq!(
                            a, b,
                            "{native} vs {model}, {} pipeline {k}, round {round}",
                            f.name
                        );
                    }
                }
            }
        }
    }
}
