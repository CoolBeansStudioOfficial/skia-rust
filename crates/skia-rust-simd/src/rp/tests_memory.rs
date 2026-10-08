// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of task B1's 8-bit memory stages (`load`/`store`/`gather` of a8, 565, 4444, 8888, rg88,
//! `store_r8`, `srcover_rgba_8888`, `swap_rb`, `alpha_to_*`, `debug_*`), highp and lowp:
//!
//! - format round trips (every 16-bit value of the 16-bit formats) with tails, and untouched
//!   neighbours;
//! - known answers (`swap_rb`, `alpha_to_*`, the `_dst` loads, `srcover_rgba_8888`, `debug_*`);
//! - highp and lowp agree where Skia's formulas are meant to agree (565/4444 expansion, 8-bit to
//!   565 rounding);
//! - gathers equal loads at the clamped coordinates;
//! - stage twins: native vs `Model(Host)` vs `Model(AmdZen4)` (`Model(Arm)` for Neon), bit for
//!   bit, on random and special lanes (design §2.8).

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

use super::contexts::GatherCtx;
use super::lanes::test_support::{Rng, float_specials};
use super::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

/// The SIMD tiers.
const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

/// The models of `t` this host can run (`Model(Host)` needs the host's estimate instructions,
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

/// Pixel memory read by loads (slot 0), and written/read by stores (slot 1).
const SRC: MemoryCtx = MemoryCtx::new(MemSlot(0));
const DST: MemoryCtx = MemoryCtx::new(MemSlot(1));
/// Register dumps: `load_src` reads `REG_IN`, `store_src` writes `REG_OUT`.
const REG_IN: MemPtr = MemPtr::new(MemSlot(2), 0);
const REG_OUT: MemPtr = MemPtr::new(MemSlot(3), 0);

/// Runs `stages` over the `w` pixels of row 0 from `x` with four writable buffers bound to
/// slots 0–3, and returns whether the program is lowp and the buffers.
fn exec(
    stages: &[Stage<'_>],
    sel: Selection,
    force_highp: bool,
    (x, w): (usize, usize),
    mut bufs: [Vec<u8>; 4],
) -> (bool, [Vec<u8>; 4]) {
    let mut program = Program::new(stages, sel, force_highp);
    let [b0, b1, b2, b3] = &mut bufs;
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::write(b0))
        .with(MemSlot(1), MemView::write(b1))
        .with(MemSlot(2), MemView::write(b2))
        .with(MemSlot(3), MemView::write(b3));
    program.run(x, 0, w, 1, &mut mem);
    drop(mem);
    (program.is_lowp(), bufs)
}

/// `exec` with only the pixel buffers (`src`, `dst`); returns the destination bytes.
fn run_px(
    stages: &[Stage<'_>],
    sel: Selection,
    force_highp: bool,
    at: (usize, usize),
    src: &[u8],
    dst: &[u8],
) -> Vec<u8> {
    let bufs = [src.to_vec(), dst.to_vec(), vec![0; 1024], vec![0; 1024]];
    let (lowp, [_, out, _, _]) = exec(stages, sel, force_highp, at, bufs);
    assert_eq!(lowp, !force_highp && sel.tier != Tier::Scalar, "{sel}");
    out
}

/// A pixel format with its load and store stages.
#[derive(Clone, Copy, Debug)]
struct Fmt {
    name: &'static str,
    bpp: usize,
    load: Stage<'static>,
    load_dst: Stage<'static>,
    /// The stages that store the registers `load` produced.
    store: &'static [Stage<'static>],
    /// `load` must go through `alpha_to_red` before the store (`r8`).
    pre_store: &'static [Stage<'static>],
}

const FORMATS: [Fmt; 6] = [
    Fmt {
        name: "a8",
        bpp: 1,
        load: Stage::LoadA8(SRC),
        load_dst: Stage::LoadA8Dst(SRC),
        store: &[Stage::StoreA8(DST)],
        pre_store: &[],
    },
    Fmt {
        name: "565",
        bpp: 2,
        load: Stage::Load565(SRC),
        load_dst: Stage::Load565Dst(SRC),
        store: &[Stage::Store565(DST)],
        pre_store: &[],
    },
    Fmt {
        name: "4444",
        bpp: 2,
        load: Stage::Load4444(SRC),
        load_dst: Stage::Load4444Dst(SRC),
        store: &[Stage::Store4444(DST)],
        pre_store: &[],
    },
    Fmt {
        name: "8888",
        bpp: 4,
        load: Stage::Load8888(SRC),
        load_dst: Stage::Load8888Dst(SRC),
        store: &[Stage::Store8888(DST)],
        pre_store: &[],
    },
    Fmt {
        name: "rg88",
        bpp: 2,
        load: Stage::LoadRg88(SRC),
        load_dst: Stage::LoadRg88Dst(SRC),
        store: &[Stage::StoreRg88(DST)],
        pre_store: &[],
    },
    // r8 has no load; it stores the red channel (`appendLoad` uses load_a8 + alpha_to_red).
    Fmt {
        name: "r8",
        bpp: 1,
        load: Stage::LoadA8(SRC),
        load_dst: Stage::LoadA8Dst(SRC),
        store: &[Stage::StoreR8(DST)],
        pre_store: &[Stage::AlphaToRed],
    },
];

/// Whether the selection runs lowp when asked to (`Scalar` has none).
fn precisions(sel: Selection) -> Vec<bool> {
    if sel.tier == Tier::Scalar {
        vec![true]
    } else {
        vec![true, false]
    }
}

/// The widths to run: around every stride, plus a long one.
fn widths(sel: Selection) -> Vec<usize> {
    let n = sel.tier.highp_stride();
    let m = sel.tier.lowp_stride().unwrap_or(1);
    let mut v = vec![1, 2, 3, n, n + 1, m, m + 1, 2 * m + 3, 41];
    v.sort_unstable();
    v.dedup();
    if cfg!(miri) {
        // Miri is slow: the first, a stride-crossing and the longest width.
        v = vec![1, n + 1, 41];
    }
    v
}

fn random_bytes(rng: &mut Rng, n: usize) -> Vec<u8> {
    (0..n).map(|_| rng.next_u32() as u8).collect()
}

/// The pipeline `load; [pre_store]; store`.
fn round_trip_stages(f: Fmt) -> Vec<Stage<'static>> {
    [&[f.load][..], f.pre_store, f.store].concat()
}

#[test]
fn formats_round_trip() {
    let mut rng = Rng::new(0xb1_0001);
    for sel in sample_selections() {
        for force_highp in precisions(sel) {
            for f in FORMATS {
                // 565 and 4444 have their own test (every 16-bit value).
                if f.name == "565" || f.name == "4444" {
                    continue;
                }
                let stages = round_trip_stages(f);
                for w in widths(sel) {
                    for x in if cfg!(miri) { vec![3] } else { vec![0, 3, 17] } {
                        let src = random_bytes(&mut rng, f.bpp * (x + w) + 64);
                        let dst = vec![0xab; f.bpp * (x + w) + 64];
                        let out = run_px(&stages, sel, force_highp, (x, w), &src, &dst);
                        let (lo, hi) = (f.bpp * x, f.bpp * (x + w));
                        assert_eq!(
                            out[lo..hi],
                            src[lo..hi],
                            "{sel} highp={force_highp} {} x={x} w={w}",
                            f.name
                        );
                        assert!(out[..lo].iter().all(|&b| b == 0xab), "{sel} before");
                        assert!(out[hi..].iter().all(|&b| b == 0xab), "{sel} after");
                    }
                }
            }
        }
    }
}

/// The 16-bit pixel values to run: all of them natively; under Miri the channel extremes of 565
/// and 4444 plus a spread of 37 values (a full chunk and a tail on every tier).
fn sixteen_bit_values() -> Vec<u16> {
    if cfg!(miri) {
        let mut v = vec![0, 0xFFFF, 0xF800, 0x07E0, 0x001F, 0xF0F0, 0x1234, 0x8410];
        v.extend((0..29u32).map(|i| (i * 2237 + 11) as u16));
        v
    } else {
        (0..=u16::MAX).collect()
    }
}

#[test]
fn sixteen_bit_formats_round_trip_for_every_value() {
    // Every 16-bit value of 565 and 4444 survives load + store (highp and lowp).
    let values = sixteen_bit_values();
    let count = values.len();
    let src: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
    for sel in selections() {
        for force_highp in precisions(sel) {
            for f in &FORMATS[1..=2] {
                let dst = vec![0; src.len()];
                let out = run_px(
                    &round_trip_stages(*f),
                    sel,
                    force_highp,
                    (0, count),
                    &src,
                    &dst,
                );
                let got: Vec<u16> = out
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                // 565/4444 have no unused bits, so every value is a fixed point.
                assert_eq!(got, values, "{sel} highp={force_highp} {}", f.name);
            }
        }
    }
}

/// `load; store_8888` of `src` and the 8888 pixels it gives.
fn widen_to_8888(f: Fmt, sel: Selection, force_highp: bool, src: &[u8], count: usize) -> Vec<u32> {
    let stages = [&[f.load][..], &[Stage::Store8888(DST)]].concat();
    let out = run_px(
        &stages,
        sel,
        force_highp,
        (0, count),
        src,
        &vec![0; 4 * count],
    );
    out.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn expanding_16_bit_formats_to_8888() {
    // 4444: both precisions give nibble * 17. 565: lowp replicates bits (`R << 3 | R >> 2`), highp
    // scales and rounds (`round(R / 31 * 255)`); the two differ for some values, as in Skia.
    let values = sixteen_bit_values();
    let count = values.len();
    let src: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let pack = |r: u32, g: u32, b: u32, a: u32| r | (g << 8) | (b << 16) | (a << 24);
    let scale = |v: u32, max: u32| (v as f32 * (1.0 / max as f32) * 255.0).round_ties_even() as u32;
    for sel in selections() {
        for h in precisions(sel) {
            let want_4444: Vec<u32> = values
                .iter()
                .map(|&v| {
                    let n = |s: u32| u32::from((v >> s) & 15) * 17;
                    pack(n(12), n(8), n(4), n(0))
                })
                .collect();
            assert_eq!(
                widen_to_8888(FORMATS[2], sel, h, &src, count),
                want_4444,
                "{sel} highp={h} 4444"
            );
            let want_565: Vec<u32> = values
                .iter()
                .map(|&v| {
                    let (r, g, b) = (
                        u32::from(v >> 11) & 31,
                        u32::from(v >> 5) & 63,
                        u32::from(v) & 31,
                    );
                    if h {
                        pack(scale(r, 31), scale(g, 63), scale(b, 31), 255)
                    } else {
                        let rep = |c: u32, bits: u32| (c << (8 - bits)) | (c >> (2 * bits - 8));
                        pack(rep(r, 5), rep(g, 6), rep(b, 5), 255)
                    }
                })
                .collect();
            assert_eq!(
                widen_to_8888(FORMATS[1], sel, h, &src, count),
                want_565,
                "{sel} highp={h} 565"
            );
        }
    }
}

#[test]
fn highp_and_lowp_round_to_565_and_4444_alike() {
    // 8888 to 565/4444: lowp's brute-force searched integer rounding equals highp's
    // `to_unorm`, for every value of each channel.
    // (Miri: every 15th value, 255 included, so a full chunk and a tail.)
    let px: Vec<u32> = (0..256u32)
        .step_by(if cfg!(miri) { 15 } else { 1 })
        .map(|v| v | (v << 8) | (v << 16) | (((255 - v) & 0xff) << 24))
        .collect();
    let src: Vec<u8> = px.iter().flat_map(|v| v.to_ne_bytes()).collect();
    for sel in selections() {
        if sel.tier == Tier::Scalar {
            continue;
        }
        for store in [Stage::Store565(DST), Stage::Store4444(DST)] {
            let stages = [Stage::Load8888(SRC), store];
            let dst = vec![0; 2 * px.len()];
            let hi = run_px(&stages, sel, true, (0, px.len()), &src, &dst);
            let lo = run_px(&stages, sel, false, (0, px.len()), &src, &dst);
            assert_eq!(hi, lo, "{sel} {store:?}");
        }
    }
}

// Port of: tests/SkRasterPipelineTest.cpp#L3244-L3270 (chrome/m156) (SkRasterPipeline_lowp, on
// the pipeline interpreter directly: `load_8888, swap_rb, store_8888`, in place).
#[test]
fn swap_rb_in_place() {
    for sel in selections() {
        for force_highp in precisions(sel) {
            let rgba: Vec<u32> = (0..64u32)
                .map(|i| (4 * i) | ((4 * i + 1) << 8) | ((4 * i + 2) << 16) | ((4 * i + 3) << 24))
                .collect();
            let bytes: Vec<u8> = rgba.iter().flat_map(|v| v.to_ne_bytes()).collect();
            // load and store through the same memory.
            let ctx = MemoryCtx::new(MemSlot(0));
            let stages = [Stage::Load8888(ctx), Stage::SwapRb, Stage::Store8888(ctx)];
            let mut buf = bytes.clone();
            let mut program = Program::new(&stages, sel, force_highp);
            let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut buf));
            program.run(0, 0, 64, 1, &mut mem);
            drop(mem);
            for (i, c) in buf.as_chunks::<4>().0.iter().enumerate() {
                let i = i as u32;
                let want = ((4 * i) << 16) | ((4 * i + 1) << 8) | (4 * i + 2) | ((4 * i + 3) << 24);
                let got = u32::from_ne_bytes([c[0], c[1], c[2], c[3]]);
                assert_eq!(got, want, "{sel} highp={force_highp} pixel {i}");
            }
        }
    }
}

/// 8888 pixels `[r, g, b, a]` as bytes.
fn px_bytes(px: &[[u8; 4]]) -> Vec<u8> {
    px.iter().flatten().copied().collect()
}

#[test]
fn channel_shuffles_and_dst_loads() {
    let src: Vec<[u8; 4]> = (0..37u32)
        .map(|i| {
            [
                (i * 7) as u8,
                (i * 13 + 1) as u8,
                (i * 29 + 2) as u8,
                (i * 31 + 3) as u8,
            ]
        })
        .collect();
    let bytes = px_bytes(&src);
    let go = |sel: Selection, h: bool, mid: &[Stage<'static>], w: usize| {
        let stages = [&[Stage::Load8888(SRC)][..], mid, &[Stage::Store8888(DST)]].concat();
        let out = run_px(&stages, sel, h, (0, w), &bytes, &vec![0; 4 * w]);
        out.as_chunks::<4>()
            .0
            .iter()
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect::<Vec<_>>()
    };
    for sel in selections() {
        for h in precisions(sel) {
            for w in if cfg!(miri) { vec![9] } else { vec![1, 9, 37] } {
                let m =
                    |f: fn([u8; 4]) -> [u8; 4]| src[..w].iter().map(|p| f(*p)).collect::<Vec<_>>();
                let tag = format!("{sel} highp={h} w={w}");
                assert_eq!(go(sel, h, &[], w), m(|p| p), "{tag}");
                assert_eq!(
                    go(sel, h, &[Stage::SwapRb], w),
                    m(|p| [p[2], p[1], p[0], p[3]]),
                    "{tag}"
                );
                assert_eq!(
                    go(sel, h, &[Stage::AlphaToGray], w),
                    m(|p| [p[3], p[3], p[3], 255]),
                    "{tag}"
                );
                assert_eq!(
                    go(sel, h, &[Stage::AlphaToRed], w),
                    m(|p| [p[3], p[1], p[2], 255]),
                    "{tag}"
                );
                // The `_dst` forms: load dst, shuffle dst, move it to src.
                let dst = |mid: &[Stage<'static>]| {
                    let stages = [
                        &[Stage::Load8888Dst(SRC)][..],
                        mid,
                        &[Stage::MoveDstSrc, Stage::Store8888(DST)],
                    ]
                    .concat();
                    run_px(&stages, sel, h, (0, w), &bytes, &vec![0; 4 * w])
                };
                assert_eq!(dst(&[]), bytes[..4 * w], "{tag}");
                let flat = |v: Vec<[u8; 4]>| px_bytes(&v);
                assert_eq!(
                    dst(&[Stage::SwapRbDst]),
                    flat(m(|p| [p[2], p[1], p[0], p[3]])),
                    "{tag}"
                );
                assert_eq!(
                    dst(&[Stage::AlphaToGrayDst]),
                    flat(m(|p| [p[3], p[3], p[3], 255])),
                    "{tag}"
                );
                assert_eq!(
                    dst(&[Stage::AlphaToRedDst]),
                    flat(m(|p| [p[3], p[1], p[2], 255])),
                    "{tag}"
                );
            }
        }
    }
}

#[test]
fn dst_loads_equal_src_loads() {
    let mut rng = Rng::new(0xb1_0002);
    for sel in selections() {
        for h in precisions(sel) {
            for f in FORMATS {
                let src = random_bytes(&mut rng, f.bpp * 50);
                let via_src = round_trip_stages(f);
                let via_dst = [
                    &[f.load_dst][..],
                    &[Stage::MoveDstSrc],
                    f.pre_store,
                    f.store,
                ]
                .concat();
                for w in if cfg!(miri) {
                    vec![13]
                } else {
                    vec![1, 13, 50]
                } {
                    let a = run_px(&via_src, sel, h, (0, w), &src, &vec![0; src.len()]);
                    let b = run_px(&via_dst, sel, h, (0, w), &src, &vec![0; src.len()]);
                    assert_eq!(a, b, "{sel} highp={h} {} w={w}", f.name);
                }
            }
        }
    }
}

#[test]
fn load_known_answers() {
    // a8 → (0,0,0,a); 565 → opaque; rg88 → (r,g,0,255); 4444 nibbles replicate.
    let go = |sel: Selection, h: bool, load: Stage<'static>, bpp: usize, src: &[u8]| {
        let w = src.len() / bpp;
        let stages = [load, Stage::Store8888(DST)];
        run_px(&stages, sel, h, (0, w), src, &vec![0; 4 * w])
    };
    for sel in selections() {
        for h in precisions(sel) {
            // (Miri: 19 values, a full chunk and a tail.)
            let a8: Vec<u8> = if cfg!(miri) {
                (0..19u8).map(|i| i.wrapping_mul(15)).collect()
            } else {
                (0..=255).collect()
            };
            let got = go(sel, h, Stage::LoadA8(SRC), 1, &a8);
            for (c, &v) in got.as_chunks::<4>().0.iter().zip(&a8) {
                assert_eq!(*c, [0, 0, 0, v], "{sel} a8 {v}");
            }
            // 565: 0xF800 red, 0x07E0 green, 0x001F blue.
            let p565: Vec<u8> = [0xF800u16, 0x07E0, 0x001F, 0xFFFF, 0]
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect();
            let got = go(sel, h, Stage::Load565(SRC), 2, &p565);
            assert_eq!(
                got,
                px_bytes(&[
                    [255, 0, 0, 255],
                    [0, 255, 0, 255],
                    [0, 0, 255, 255],
                    [255, 255, 255, 255],
                    [0, 0, 0, 255]
                ]),
                "{sel} 565"
            );
            let p4444: Vec<u8> = [0xF0F0u16, 0x1234, 0x000F]
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect();
            let got = go(sel, h, Stage::Load4444(SRC), 2, &p4444);
            assert_eq!(
                got,
                px_bytes(&[[255, 0, 255, 0], [0x11, 0x22, 0x33, 0x44], [0, 0, 0, 255]]),
                "{sel} 4444"
            );
            let rg = [10u8, 200, 0, 255, 255, 1];
            let got = go(sel, h, Stage::LoadRg88(SRC), 2, &rg);
            assert_eq!(
                got,
                px_bytes(&[[10, 200, 0, 255], [0, 255, 0, 255], [255, 1, 0, 255]]),
                "{sel} rg88"
            );
        }
    }
}

/// The `div255` of a tier's lowp: `(v + 255) / 256` on x86, the exact one on Neon.
fn div255(sel: Selection, v: u16) -> u16 {
    if sel.tier == Tier::Neon {
        let v = v.wrapping_add(128);
        (v.wrapping_add(v >> 8)) >> 8
    } else {
        v.wrapping_add(255) / 256
    }
}

#[test]
#[allow(clippy::many_single_char_names)] // s, d, o: source, destination, output
fn srcover_rgba_8888_known_answers() {
    let mut rng = Rng::new(0xb1_0003);
    for sel in selections() {
        for h in precisions(sel) {
            let n = 29;
            let src = px_bytes(
                &(0..n)
                    .map(|i| match i % 3 {
                        0 => [10, 20, 30, 255], // opaque: replaces dst
                        1 => [0, 0, 0, 0],      // transparent black: keeps dst
                        _ => {
                            let a = rng.next_u32() as u8;
                            // premultiplied
                            let c = |rng: &mut Rng| (rng.next_u32() as u8).min(a);
                            [c(&mut rng), c(&mut rng), c(&mut rng), a]
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            let dst = random_bytes(&mut rng, 4 * n);
            for w in [1, 5, n] {
                let stages = [Stage::Load8888(SRC), Stage::SrcoverRgba8888(DST)];
                let out = run_px(&stages, sel, h, (0, w), &src, &dst);
                for i in 0..w {
                    let (s, d, o) = (
                        &src[4 * i..4 * i + 4],
                        &dst[4 * i..4 * i + 4],
                        &out[4 * i..4 * i + 4],
                    );
                    match i % 3 {
                        0 => assert_eq!(o, s, "{sel} highp={h} opaque {i}"),
                        1 => assert_eq!(o, d, "{sel} highp={h} transparent {i}"),
                        _ if !h => {
                            // lowp: s + div255(d * (255 - sa)), clamped to 255.
                            for c in 0..4 {
                                let v = u16::from(s[c])
                                    + div255(sel, u16::from(d[c]) * (255 - u16::from(s[3])));
                                assert_eq!(u16::from(o[c]), v.min(255), "{sel} lowp {i} {c}");
                            }
                        }
                        _ => {
                            // highp: s*255 + d*(1-a), rounded and clamped.
                            let a = f32::from(s[3]) * (1.0 / 255.0);
                            for c in 0..4 {
                                let sf = f32::from(s[c]) * (1.0 / 255.0);
                                let v = f32::from(d[c]) * (1.0 - a) + sf * 255.0;
                                let want = v.clamp(0.0, 255.0);
                                assert!(
                                    (f32::from(o[c]) - want).abs() <= 1.0,
                                    "{sel} highp {i} {c}: {} vs {want}",
                                    o[c]
                                );
                            }
                        }
                    }
                }
                assert_eq!(out[4 * w..], dst[4 * w..], "{sel} past the run");
            }
        }
    }
}

#[test]
fn debug_stages_known_answers() {
    let src = px_bytes(&[[200, 100, 50, 25], [255, 0, 255, 0], [1, 2, 3, 4]]);
    for sel in selections() {
        for h in precisions(sel) {
            let go = |stage: Stage<'static>| {
                let stages = [Stage::Load8888(SRC), stage];
                // The debug stages store to DST.
                let out = run_px(&stages, sel, h, (0, 3), &src, &[0; 12]);
                out.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect::<Vec<_>>()
            };
            let col =
                |k: usize| -> Vec<u8> { src.as_chunks::<4>().0.iter().map(|c| c[k]).collect() };
            let opaque = |k: usize| -> Vec<[u8; 4]> {
                col(k)
                    .iter()
                    .map(|&v| {
                        let mut p = [0, 0, 0, 255];
                        p[k] = v;
                        p
                    })
                    .collect()
            };
            // debug_?_255: the channel in its own color, opaque (alpha: grey).
            assert_eq!(go(Stage::DebugR255(DST)), opaque(0), "{sel} highp={h}");
            assert_eq!(go(Stage::DebugG255(DST)), opaque(1), "{sel} highp={h}");
            assert_eq!(go(Stage::DebugB255(DST)), opaque(2), "{sel} highp={h}");
            assert_eq!(
                go(Stage::DebugA255(DST)),
                col(3).iter().map(|&v| [v; 4]).collect::<Vec<_>>(),
                "{sel} highp={h}"
            );
            if h {
                // highp: 12.8 fixed point of the float channel.
                let fixed = |v: u8| -> [u8; 4] {
                    let lane = f32::from(v) * (1.0 / 255.0);
                    [
                        ((lane.abs() / 256.0) as u32 & 0xFF) as u8,
                        (lane.abs() as u32 & 0xFF) as u8,
                        ((lane.abs() * 256.0) as u32 & 0xFF) as u8,
                        (lane * -256.0 * 256.0).clamp(0.0, 255.0).round_ties_even() as u8,
                    ]
                };
                for (k, stage) in [
                    Stage::DebugR(DST),
                    Stage::DebugG(DST),
                    Stage::DebugB(DST),
                    Stage::DebugA(DST),
                ]
                .into_iter()
                .enumerate()
                {
                    let want: Vec<[u8; 4]> = col(k).iter().map(|&v| fixed(v)).collect();
                    assert_eq!(go(stage), want, "{sel} highp debug {k}");
                }
            } else {
                // lowp: the 8-bit value, byte-swapped into a 16-bit fixed-point lane.
                for (k, stage) in [
                    Stage::DebugR(DST),
                    Stage::DebugG(DST),
                    Stage::DebugB(DST),
                    Stage::DebugA(DST),
                ]
                .into_iter()
                .enumerate()
                {
                    let want: Vec<[u8; 4]> = col(k).iter().map(|&v| [0, v, 0, 0]).collect();
                    assert_eq!(go(stage), want, "{sel} lowp debug {k}");
                }
            }
        }
    }
}

#[test]
fn debug_x_and_y_show_the_device_coordinates() {
    // seed_shader gives x = dx + 0.5, y = dy + 0.5; debug_x/debug_y show them in 12.8 fixed
    // point (highp treats x and y as r and g, which seed_shader sets the same way).
    for sel in sample_selections() {
        for h in precisions(sel) {
            for stage in [Stage::DebugX(DST), Stage::DebugY(DST)] {
                let is_x = matches!(stage, Stage::DebugX(_));
                for (x0, y0) in [(0usize, 0usize), (3, 5), (300, 70_000)] {
                    let w = 19;
                    let stages = [Stage::SeedShader, stage];
                    // The pixel at x lies at byte 4 * x of the (stride 0) destination.
                    let mut out = vec![0u8; 4 * (x0 + w)];
                    let mut unused = vec![0u8; 8];
                    let mut program = Program::new(&stages, sel, h);
                    let mut mem = MemoryBindings::new()
                        .with(MemSlot(0), MemView::write(&mut unused))
                        .with(MemSlot(1), MemView::write(&mut out));
                    program.run(x0, y0, w, 1, &mut mem);
                    drop(mem);
                    let out = &out[4 * x0..];
                    for i in 0..w {
                        let lane = if is_x {
                            (x0 + i) as f32 + 0.5
                        } else {
                            y0 as f32 + 0.5
                        };
                        let want = [
                            ((lane / 256.0) as u32 & 0xFF) as u8,
                            (lane as u32 & 0xFF) as u8,
                            ((lane * 256.0) as u32 & 0xFF) as u8,
                            0,
                        ];
                        assert_eq!(out[4 * i..4 * i + 4], want, "{sel} highp={h} {x0},{y0} {i}");
                    }
                }
            }
        }
    }
}

/// A `width` × `height` texture of `fmt` pixels with `stride` pixels per row; padding is
/// distinct from the pixels.
fn texture(rng: &mut Rng, f: Fmt, width: usize, height: usize, stride: usize) -> Vec<u8> {
    assert!(stride >= width);
    random_bytes(rng, f.bpp * stride * height)
}

#[test]
fn gather_equals_load_at_the_clamped_pixel() {
    let mut rng = Rng::new(0xb1_0004);
    for sel in sample_selections() {
        for h in precisions(sel) {
            for f in FORMATS.iter().filter(|f| f.name != "r8") {
                // (Miri: only the 9 x 3 texture, whose clamp is hit by the 17 pixels.)
                for (width, height, stride) in [(70usize, 1usize, 70usize), (9, 3, 12), (1, 2, 5)]
                    .into_iter()
                    .filter(|g| !cfg!(miri) || g.0 == 9)
                {
                    let tex = texture(&mut rng, *f, width, height, stride);
                    let ctx = GatherCtx {
                        pixels: (&tex).into(),
                        stride: stride as i32,
                        width: width as f32,
                        height: height as f32,
                        weights: [0.0; 16],
                        round_down_at_integer: false,
                    };
                    let gather = match f.name {
                        "a8" => Stage::GatherA8(&ctx),
                        "565" => Stage::Gather565(&ctx),
                        "4444" => Stage::Gather4444(&ctx),
                        "8888" => Stage::Gather8888(&ctx),
                        _ => Stage::GatherRg88(&ctx),
                    };
                    // Miri: the last row (which also tests the stride), and 17 pixels (a lowp chunk and a tail).
                    for row in (0..height).filter(|r| !cfg!(miri) || *r + 1 == height) {
                        let w = if cfg!(miri) { 17 } else { 41 };
                        // The pixels a clamped gather at (x + 0.5, row + 0.5) must read.
                        let want_src: Vec<u8> = (0..w)
                            .flat_map(|x| {
                                let at = f.bpp * (row.min(height - 1) * stride + x.min(width - 1));
                                tex[at..at + f.bpp].to_vec()
                            })
                            .collect();
                        let store = f.store;
                        let via_load = [&[f.load][..], f.pre_store, store].concat();
                        let via_gather =
                            [&[Stage::SeedShader, gather][..], f.pre_store, store].concat();
                        let dst = vec![0; f.bpp * w];
                        let a = run_px(&via_load, sel, h, (0, w), &want_src, &dst);
                        // The gather run is on row `row` (the destination has stride 0).
                        let mut out = dst.clone();
                        let mut program = Program::new(&via_gather, sel, h);
                        let mut z0 = vec![0u8; 8];
                        let mut mem = MemoryBindings::new()
                            .with(MemSlot(0), MemView::write(&mut z0))
                            .with(MemSlot(1), MemView::write(&mut out));
                        program.run(0, row, w, 1, &mut mem);
                        drop(mem);
                        assert_eq!(
                            out, a,
                            "{sel} highp={h} {} {width}x{height}/{stride} row {row}",
                            f.name
                        );
                    }
                }
            }
        }
    }
}

// ~~~ Stage twins ~~~

/// Random float lane bits: mostly special floats, some random patterns.
fn random_floats(rng: &mut Rng, specials: &[u32], count: usize, no_nan: bool) -> Vec<u8> {
    (0..count)
        .flat_map(|_| {
            let mut bits = if rng.below(4) == 0 {
                rng.next_u32()
            } else {
                rng.pick(specials)
            };
            if no_nan && f32::from_bits(bits).is_nan() {
                bits = 0;
            }
            bits.to_ne_bytes()
        })
        .collect()
}

/// Random lowp lanes: mostly colors in `[0, 255]`, sometimes any 16 bits.
fn random_u16s(rng: &mut Rng, count: usize) -> Vec<u8> {
    (0..count)
        .flat_map(|_| {
            let v = if rng.below(4) == 0 {
                rng.next_u32() as u16
            } else {
                (rng.next_u32() & 0xFF) as u16
            };
            v.to_ne_bytes()
        })
        .collect()
}

/// Pads `v` with zeros to `len`.
fn padded(mut v: Vec<u8>, len: usize) -> Vec<u8> {
    v.resize(len, 0);
    v
}

#[test]
#[allow(clippy::too_many_lines)] // one table of stages, one loop over tiers
fn memory_stage_twins() {
    let specials = float_specials();
    let mut tex_rng = Rng::new(0xb1_0005);
    // Textures for the gathers (a8, 565, 4444, 8888, rg88), 11 x 7 with a row stride of 13.
    let tex: Vec<Vec<u8>> = [1usize, 2, 2, 4, 2]
        .iter()
        .map(|bpp| random_bytes(&mut tex_rng, bpp * 13 * 7))
        .collect();
    let ctx = |i: usize| GatherCtx {
        pixels: (&tex[i]).into(),
        stride: 13,
        width: 11.0,
        height: 7.0,
        weights: [0.0; 16],
        round_down_at_integer: i % 2 == 1,
    };
    let ctxs = [ctx(0), ctx(1), ctx(2), ctx(3), ctx(4)];
    let gathers = [
        Stage::GatherA8(&ctxs[0]),
        Stage::Gather565(&ctxs[1]),
        Stage::Gather4444(&ctxs[2]),
        Stage::Gather8888(&ctxs[3]),
        Stage::GatherRg88(&ctxs[4]),
    ];
    let loads = [
        Stage::LoadA8(SRC),
        Stage::Load565(SRC),
        Stage::Load4444(SRC),
        Stage::Load8888(SRC),
        Stage::LoadRg88(SRC),
        Stage::LoadA8Dst(SRC),
        Stage::Load565Dst(SRC),
        Stage::Load4444Dst(SRC),
        Stage::Load8888Dst(SRC),
        Stage::LoadRg88Dst(SRC),
    ];
    let stores = [
        Stage::StoreA8(DST),
        Stage::StoreR8(DST),
        Stage::Store565(DST),
        Stage::Store4444(DST),
        Stage::Store8888(DST),
        Stage::StoreRg88(DST),
        Stage::SrcoverRgba8888(DST),
        Stage::DebugR(DST),
        Stage::DebugG(DST),
        Stage::DebugB(DST),
        Stage::DebugA(DST),
        Stage::DebugR255(DST),
        Stage::DebugG255(DST),
        Stage::DebugB255(DST),
        Stage::DebugA255(DST),
        Stage::DebugX(DST),
        Stage::DebugY(DST),
    ];
    let shuffles = [
        Stage::SwapRb,
        Stage::SwapRbDst,
        Stage::AlphaToGray,
        Stage::AlphaToGrayDst,
        Stage::AlphaToRed,
        Stage::AlphaToRedDst,
    ];

    for (native, models) in twin_sets() {
        for force_highp in [true, false] {
            let n = if force_highp {
                native.tier.highp_stride()
            } else {
                native.tier.lowp_stride().unwrap()
            };
            let mut rng = Rng::new(0xb1_0006);
            // register bytes: 4 registers of n lanes (floats on highp, u16 on lowp)
            let reg_bytes = 4 * n * if force_highp { 4 } else { 2 };
            let pixel_bytes = 16 * 4 * 3;
            for round in 0..if cfg!(miri) { 1 } else { 60 } {
                let regs = if force_highp {
                    random_floats(&mut rng, &specials, 4 * n, false)
                } else {
                    random_u16s(&mut rng, 4 * n)
                };
                // Coordinates for the gathers: 2n non-NaN floats first (x lanes, y lanes).
                let coords = if round % 2 == 0 {
                    padded(random_floats(&mut rng, &specials, 2 * n, true), reg_bytes)
                } else {
                    // Mostly in-range coordinates.
                    padded(
                        (0..2 * n)
                            .flat_map(|_| (rng.below(130) as f32 / 10.0 - 1.0).to_ne_bytes())
                            .collect(),
                        reg_bytes,
                    )
                };
                let src = random_bytes(&mut rng, pixel_bytes);
                let dst = random_bytes(&mut rng, pixel_bytes);
                let w = 1 + rng.below(2 * n + 3);
                let x = rng.below(5);

                let mut pipelines: Vec<(Vec<Stage<'_>>, bool)> = Vec::new();
                // regs -> store; seeded coordinates -> debug_x/y
                for st in stores {
                    pipelines.push((vec![Stage::LoadSrc(REG_IN), st], false));
                    pipelines.push((vec![Stage::SeedShader, st], false));
                }
                // load -> regs
                for l in loads {
                    pipelines.push((
                        vec![
                            l,
                            Stage::StoreSrc(REG_OUT),
                            Stage::StoreDst(MemPtr::new(MemSlot(3), reg_bytes as u32)),
                        ],
                        false,
                    ));
                }
                // shuffles on random regs, observed through the register dump
                for s in shuffles {
                    pipelines.push((
                        vec![
                            Stage::LoadSrc(REG_IN),
                            Stage::MoveSrcDst,
                            s,
                            Stage::StoreSrc(REG_OUT),
                            Stage::StoreDst(MemPtr::new(MemSlot(3), reg_bytes as u32)),
                        ],
                        false,
                    ));
                }
                // gathers at random coordinates
                for g in gathers {
                    pipelines.push((
                        vec![Stage::LoadSrc(REG_IN), g, Stage::StoreSrc(REG_OUT)],
                        true,
                    ));
                }

                for (stages, coords_in) in &pipelines {
                    let reg_in = if *coords_in {
                        coords.clone()
                    } else {
                        regs.clone()
                    };
                    let bufs = || {
                        [
                            padded(src.clone(), 4096),
                            padded(dst.clone(), 4096),
                            padded(reg_in.clone(), 4096),
                            vec![0; 4096],
                        ]
                    };
                    let (lowp, want) = exec(stages, native, force_highp, (x, w), bufs());
                    assert_eq!(lowp, !force_highp);
                    for model in &models {
                        let (_, got) = exec(stages, *model, force_highp, (x, w), bufs());
                        for (slot, (a, b)) in want.iter().zip(&got).enumerate() {
                            assert_eq!(
                                a, b,
                                "{native} vs {model}, highp={force_highp}, {stages:?}, round {round}, slot {slot}"
                            );
                        }
                    }
                }
            }
        }
    }
}
