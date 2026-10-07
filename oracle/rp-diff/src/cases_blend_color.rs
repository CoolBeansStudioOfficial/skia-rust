// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Cases of the blend-mode, coverage and color stages (B3, B4): [`generate`], called from
//! [`crate::cases::all`].
//!
//! Register-transform stages run through [`regs`], which extends
//! [`Cases::registers`](crate::cases::Cases::registers) with input kinds that make the stages'
//! branches fire (values on a coarse grid so `d == da`, `s == 0`, `s == sa` happen; premultiplied
//! values; lowp edge values) and with extra buffers for pointer contexts. Stages that read pixel
//! memory (coverage, `emboss`, `dither`) run through [`pixel`] over whole rects, so every pixel of
//! every tail is compared, not only the last chunk.

// Case lists are long tables of test data: float literals are written the way the reference
// formulas (skcms transfer functions, BT.2020) print them, hex words are test patterns, and the
// generators are flat lists of cases.
#![allow(
    clippy::too_many_lines,
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    clippy::enum_glob_use
)]

use skia_rust_simd::rp::Op;
use skia_rust_simd::rp::contexts::UniformColorCtx;

use crate::case::{Buffer, Case, Ctx, Rect, StageSpec};
use crate::cases::{
    Cases, FILL, Inputs, Precision, REG_BYTES, REGISTER_WIDTHS, Rng, TAIL_WIDTHS, float_specials,
    output,
};

/// Widths of context cases: a pure tail on every tier but Sse (7), and a full chunk plus a tail
/// on every tier (19).
const CTX_WIDTHS: &[usize] = &[7, 19];

/// Register input kinds on top of [`Inputs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Std(Inputs),
    /// Highp: random picks from a coarse grid of floats in `[0, 1]`.
    Grid,
    /// Highp: premultiplied (the words of the buffer ascend, so for every lane count `r, g, b`
    /// are `<= a` in every lane).
    Sorted,
    /// Lowp: random picks from byte edge values.
    Edge16,
    /// Lowp: ascending bytes (premultiplied for every lane count).
    Sorted16,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Std(i) => i.name(),
            Kind::Grid => "grid",
            Kind::Sorted => "sorted",
            Kind::Edge16 => "edge",
            Kind::Sorted16 => "sorted16",
        }
    }

    /// Whether the kind belongs to the base set (run at every width given).
    fn is_base(self) -> bool {
        matches!(self, Kind::Std(_))
    }

    fn for_precision(p: Precision) -> Vec<Kind> {
        let mut v: Vec<Kind> = Inputs::for_precision(p)
            .iter()
            .map(|&i| Kind::Std(i))
            .collect();
        match p {
            Precision::Highp => v.extend([Kind::Grid, Kind::Sorted]),
            Precision::Auto => v.extend([Kind::Edge16, Kind::Sorted16]),
        }
        v
    }

    /// Source and destination register buffers.
    fn pair(self, s1: u32, s2: u32) -> (Vec<u8>, Vec<u8>) {
        match self {
            Kind::Std(i) => i.pair(s1, s2),
            k => (extra_input(k, s1), extra_input(k, s2)),
        }
    }
}

fn f32_bytes(words: impl IntoIterator<Item = f32>) -> Vec<u8> {
    words.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn u16_bytes(words: impl IntoIterator<Item = u16>) -> Vec<u8> {
    words.into_iter().flat_map(u16::to_le_bytes).collect()
}

#[allow(clippy::cast_precision_loss)] // < 2^24, exact
fn unit(rng: &mut Rng) -> f32 {
    (rng.next_u32() >> 8) as f32 / 16_777_216.0
}

const GRID: [f32; 9] = [0.0, 0.25, 0.5, 0.75, 1.0, 0.125, 0.375, 0.625, 0.875];
const EDGES16: [u16; 12] = [0, 1, 2, 3, 127, 128, 129, 253, 254, 255, 85, 170];

fn extra_input(k: Kind, seed: u32) -> Vec<u8> {
    let mut rng = Rng::new(0x1234_5679 ^ seed.wrapping_mul(0x9e37_79b1));
    match k {
        Kind::Grid => f32_bytes((0..REG_BYTES / 4).map(|_| GRID[rng.below(9) as usize])),
        Kind::Sorted => {
            let mut prev = 0.0f32;
            let mut v: Vec<f32> = (0..REG_BYTES / 4)
                .map(|_| {
                    // Plateaus make `r == a` (and `0 == r`) happen.
                    if rng.below(3) != 0 {
                        prev = unit(&mut rng);
                    }
                    prev
                })
                .collect();
            v.sort_by(f32::total_cmp);
            f32_bytes(v)
        }
        Kind::Edge16 => u16_bytes((0..REG_BYTES / 2).map(|_| EDGES16[rng.below(12) as usize])),
        Kind::Sorted16 => {
            let mut prev = 0u16;
            let mut v: Vec<u16> = (0..REG_BYTES / 2)
                .map(|_| {
                    if rng.below(3) != 0 {
                        prev = u16::try_from(rng.below(256)).expect("byte");
                    }
                    prev
                })
                .collect();
            v.sort_unstable();
            u16_bytes(v)
        }
        Kind::Std(_) => unreachable!("standard kinds go through Inputs"),
    }
}

/// `load_src(0) load_dst(1) <stages> store_src(2) store_dst(3)` over one row at `(0, 0)`, for
/// every precision, input kind and width; `extra(precision)` adds buffers from slot 4 (for
/// pointer contexts). Base kinds run at `widths`, the others at [`CTX_WIDTHS`] (or at `widths`
/// too when `widths` is [`CTX_WIDTHS`]). Named `<group>/<precision>/<kind>/w<width>`.
fn regs(
    c: &mut Cases,
    group: &str,
    stages: &[StageSpec],
    precisions: &[Precision],
    widths: &[usize],
    extra: &dyn Fn(Precision) -> Vec<Buffer>,
) {
    regs_where(c, group, stages, precisions, widths, extra, &|_| true);
}

/// [`regs`] over the input kinds `keep` accepts.
fn regs_where(
    c: &mut Cases,
    group: &str,
    stages: &[StageSpec],
    precisions: &[Precision],
    widths: &[usize],
    extra: &dyn Fn(Precision) -> Vec<Buffer>,
    keep: &dyn Fn(Kind) -> bool,
) {
    for &p in precisions {
        for kind in Kind::for_precision(p).into_iter().filter(|&k| keep(k)) {
            let ws = if kind.is_base() { widths } else { CTX_WIDTHS };
            for &w in ws {
                let (s1, s2) = (c.next_seed(), c.next_seed());
                let (src, dst) = kind.pair(s1, s2);
                let mut all = vec![
                    StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                    StageSpec::with(Op::LoadDst, Ctx::Ptr { slot: 1, offset: 0 }),
                ];
                all.extend_from_slice(stages);
                all.push(StageSpec::with(
                    Op::StoreSrc,
                    Ctx::Ptr { slot: 2, offset: 0 },
                ));
                all.push(StageSpec::with(
                    Op::StoreDst,
                    Ctx::Ptr { slot: 3, offset: 0 },
                ));
                let mut buffers = vec![
                    Buffer::new(src),
                    Buffer::new(dst),
                    output(REG_BYTES),
                    output(REG_BYTES),
                ];
                buffers.extend(extra(p));
                c.push(Case {
                    name: format!("{group}/{}/{}/w{w}", p.name(), kind.name()),
                    force_highp: p == Precision::Highp,
                    compiled: false,
                    buffers,
                    stages: all,
                    runs: vec![Rect::new(0, 0, w, 1)],
                });
            }
        }
    }
}

fn no_extra(_: Precision) -> Vec<Buffer> {
    Vec::new()
}

/// A stage without a context, in the standard grid of cases.
fn plain(c: &mut Cases, op: Op, precisions: &[Precision]) {
    regs(
        c,
        op.name(),
        &[StageSpec::new(op)],
        precisions,
        REGISTER_WIDTHS,
        &no_extra,
    );
}

/// A stage with several contexts: `<op>/<tag>`.
fn with_ctxs(c: &mut Cases, op: Op, precisions: &[Precision], ctxs: &[(&str, Ctx)]) {
    for (tag, ctx) in ctxs {
        regs(
            c,
            &format!("{}/{tag}", op.name()),
            &[StageSpec::with(op, ctx.clone())],
            precisions,
            CTX_WIDTHS,
            &no_extra,
        );
    }
}

fn f32s(v: &[f32]) -> Ctx {
    Ctx::F32(v.to_vec())
}

// ---------------------------------------------------------------------------------------------
// Pixel-memory cases.

/// Pixel data kinds of [`pixel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pix {
    /// Highp: `f32` in `[0, 1]`; lowp: random bytes.
    Unit,
    /// Highp: special `f32` values (no NaN); lowp: byte edge values.
    Special,
    /// Premultiplied: `r, g, b <= a`.
    Premul,
}

impl Pix {
    fn name(self) -> &'static str {
        match self {
            Pix::Unit => "unit",
            Pix::Special => "special",
            Pix::Premul => "premul",
        }
    }
}

const PIX_KINDS: [Pix; 3] = [Pix::Unit, Pix::Special, Pix::Premul];

/// `pixels` pixels of `p` (highp: 4 x `f32`; lowp: `RGBA8888`).
fn pixel_data(kind: Pix, p: Precision, pixels: usize, seed: u32) -> Vec<u8> {
    let mut rng = Rng::new(0x7777_1357 ^ seed.wrapping_mul(0x85eb_ca6b));
    let specials = float_specials();
    let mut out = Vec::new();
    for _ in 0..pixels {
        match p {
            Precision::Highp => {
                let px: [f32; 4] = match kind {
                    Pix::Unit => std::array::from_fn(|_| unit(&mut rng)),
                    Pix::Special => {
                        std::array::from_fn(|_| f32::from_bits(specials[rng.below(44) as usize]))
                    }
                    Pix::Premul => {
                        let a = if rng.below(4) == 0 {
                            GRID[rng.below(9) as usize]
                        } else {
                            unit(&mut rng)
                        };
                        [
                            a * unit(&mut rng),
                            a * unit(&mut rng),
                            if rng.below(4) == 0 { a } else { 0.0 },
                            a,
                        ]
                    }
                };
                out.extend(f32_bytes(px));
            }
            Precision::Auto => {
                let edge = |rng: &mut Rng| u8::try_from(EDGES16[rng.below(12) as usize]).unwrap();
                let rand = |rng: &mut Rng| u8::try_from(rng.below(256)).unwrap();
                let px: [u8; 4] = match kind {
                    Pix::Unit => std::array::from_fn(|_| rand(&mut rng)),
                    Pix::Special => std::array::from_fn(|_| edge(&mut rng)),
                    Pix::Premul => {
                        let a = rand(&mut rng);
                        let below = |rng: &mut Rng| {
                            u8::try_from(rng.below(u32::from(a) + 1)).expect("byte")
                        };
                        [below(&mut rng), below(&mut rng), a, a]
                    }
                };
                out.extend(px);
            }
        }
    }
    out
}

fn pixel_bytes(p: Precision) -> usize {
    match p {
        Precision::Highp => 16,
        Precision::Auto => 4,
    }
}

/// A pixel-memory case: `load(0) load_dst(1) <mid> store(2) swap_src_dst store(3)` over `runs`
/// of buffers with row stride `stride` pixels and `rows` rows; `extra` buffers from slot 4.
#[allow(clippy::too_many_arguments)] // a case spec
fn pixel(
    c: &mut Cases,
    name: String,
    p: Precision,
    kind: Pix,
    mid: &[StageSpec],
    extra: Vec<Buffer>,
    stride: usize,
    rows: usize,
    compiled: bool,
    runs: Vec<Rect>,
) {
    let pixels = stride * rows;
    let layout = |b: Buffer| b.with_layout(isize::try_from(stride).unwrap(), 0);
    let (s1, s2) = (c.next_seed(), c.next_seed());
    let mem = |slot| Ctx::Mem { slot };
    let (load, load_dst, store) = match p {
        Precision::Highp => (Op::LoadF32, Op::LoadF32Dst, Op::StoreF32),
        Precision::Auto => (Op::Load8888, Op::Load8888Dst, Op::Store8888),
    };
    let mut stages = vec![
        StageSpec::with(load, mem(0)),
        StageSpec::with(load_dst, mem(1)),
    ];
    stages.extend_from_slice(mid);
    stages.extend([
        StageSpec::with(store, mem(2)),
        StageSpec::new(Op::SwapSrcDst),
        StageSpec::with(store, mem(3)),
    ]);
    let mut buffers = vec![
        layout(Buffer::new(pixel_data(kind, p, pixels, s1))),
        layout(Buffer::new(pixel_data(kind, p, pixels, s2))),
        layout(Buffer::new(vec![FILL; pixels * pixel_bytes(p)])),
        layout(Buffer::new(vec![FILL; pixels * pixel_bytes(p)])),
    ];
    buffers.extend(extra);
    c.push(Case {
        name,
        force_highp: p == Precision::Highp,
        compiled,
        buffers,
        stages,
        runs,
    });
}

/// Random bytes with edge values mixed in.
fn plane8(n: usize, seed: u32) -> Vec<u8> {
    let mut rng = Rng::new(0x4242_4243 ^ seed.wrapping_mul(0x9e37_79b1));
    (0..n)
        .map(|_| match rng.below(6) {
            0 => 0,
            1 => 255,
            2 => u8::try_from(EDGES16[rng.below(12) as usize]).unwrap(),
            _ => u8::try_from(rng.below(256)).unwrap(),
        })
        .collect()
}

/// Random `RGB565` words with edge values mixed in.
fn plane565(n: usize, seed: u32) -> Vec<u8> {
    let mut rng = Rng::new(0x2468_ace1 ^ seed.wrapping_mul(0x9e37_79b1));
    u16_bytes((0..n).map(|_| match rng.below(8) {
        0 => 0,
        1 => 0xffff,
        2 => 0xf800,
        3 => 0x07e0,
        4 => 0x001f,
        _ => u16::try_from(rng.next_u32() & 0xffff).unwrap(),
    }))
}

/// Runs `op` (with a coverage plane in slot 4 of `bytes_per_px`) over tail widths and over a
/// compiled pipeline with several rects.
fn coverage(c: &mut Cases, op: Op, bytes_per_px: usize, plane: fn(usize, u32) -> Vec<u8>) {
    for p in [Precision::Auto, Precision::Highp] {
        for kind in PIX_KINDS {
            for &w in TAIL_WIDTHS {
                let (x, y, h) = (3, 1, 2);
                let stride = x + w + 2;
                let rows = y + h + 1;
                let seed = c.next_seed();
                pixel(
                    c,
                    format!("{}/{}/{}/w{w}", op.name(), p.name(), kind.name()),
                    p,
                    kind,
                    &[StageSpec::with(op, Ctx::Mem { slot: 4 })],
                    vec![
                        Buffer::new(plane(stride * rows, seed))
                            .with_layout(isize::try_from(stride).unwrap(), 0),
                    ],
                    stride,
                    rows,
                    false,
                    vec![Rect::new(x, y, w, h)],
                );
            }
            // A compiled pipeline on several rects: tail scratch persists (design 1.7).
            let seed = c.next_seed();
            let (stride, rows) = (32, 4);
            pixel(
                c,
                format!("{}/{}/{}/compiled", op.name(), p.name(), kind.name()),
                p,
                kind,
                &[StageSpec::with(op, Ctx::Mem { slot: 4 })],
                vec![
                    Buffer::new(plane(stride * rows, seed))
                        .with_layout(isize::try_from(stride).unwrap(), 0),
                ],
                stride,
                rows,
                true,
                vec![
                    Rect::new(0, 0, 5, 2),
                    Rect::new(7, 1, 17, 2),
                    Rect::new(2, 0, 3, 1),
                    Rect::new(0, 3, 32, 1),
                ],
            );
        }
        let _ = bytes_per_px;
    }
}

// ---------------------------------------------------------------------------------------------
// Context sets.

/// Floats that are interesting in `[0, 1]` and for lowp's `from_float` (`round(f * 255)`).
fn rgb_sets() -> Vec<(&'static str, Vec<f32>)> {
    let half = |k: f32| (k + 0.5) / 255.0;
    vec![
        ("zero", vec![0.0, 0.0, 0.0]),
        ("one", vec![1.0, 1.0, 1.0]),
        ("mid", vec![0.5, 0.25, 0.75]),
        ("third", vec![1.0 / 3.0, 2.0 / 3.0, 0.999]),
        ("round0", vec![half(0.0), half(127.0), half(254.0)]),
        (
            "roundup",
            vec![
                f32::from_bits(half(0.0).to_bits() + 1),
                f32::from_bits(half(127.0).to_bits() + 1),
                f32::from_bits(half(254.0).to_bits() + 1),
            ],
        ),
        (
            "rounddown",
            vec![
                f32::from_bits(half(0.0).to_bits() - 1),
                f32::from_bits(half(127.0).to_bits() - 1),
                f32::from_bits(half(254.0).to_bits() - 1),
            ],
        ),
        ("range", vec![-0.25, 1.5, 2.5]),
        ("neg", vec![-1.0, -0.0, -255.5]),
        ("big", vec![255.5, 65535.5, 1e30]),
        ("tiny", vec![1e-39, f32::MIN_POSITIVE, 1e-30]),
    ]
}

fn uniform(rgba: [f32; 4]) -> Ctx {
    // Skia: `rgba[i] = (uint16_t)(f * 255 + 0.5)` for in-range colors (SkRasterPipeline::
    // appendConstantColor); out-of-range ones get an arbitrary raw value.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // in range or arbitrary
    let to16 = |f: f32| {
        if (0.0..=1.0).contains(&f) {
            (f * 255.0 + 0.5) as u16
        } else {
            0xABCD
        }
    };
    Ctx::UniformColor {
        rgba,
        rgba16: rgba.map(to16),
    }
}

fn uniform_sets() -> Vec<(&'static str, Ctx)> {
    let mut v = vec![
        ("clear", uniform([0.0, 0.0, 0.0, 0.0])),
        ("opaque", uniform([1.0, 0.5, 0.25, 1.0])),
        ("premul", uniform([0.125, 0.25, 0.5, 0.5])),
        (
            "round",
            uniform([0.5 / 255.0, 127.5 / 255.0, 254.5 / 255.0, 1.0]),
        ),
        ("range", uniform([-0.5, 2.0, 255.0, -3.0])),
        ("tiny", uniform([1e-39, 1e-30, 0.999, 0.001])),
    ];
    // Raw 16-bit lanes that disagree with the floats (lowp reads only `rgba16`).
    v.push((
        "raw16",
        Ctx::UniformColor {
            rgba: [0.1, 0.2, 0.3, 0.4],
            rgba16: [0, 255, 65535, 12345],
        },
    ));
    v
}

fn tf(v: [f32; 7]) -> Ctx {
    Ctx::F32(v.to_vec())
}

/// `skcms_TransferFunction`s as `parametric` sees them: `g, a, b, c, d, e, f`.
fn parametric_sets() -> Vec<(&'static str, Ctx)> {
    vec![
        (
            "srgb",
            tf([
                2.4,
                1.0 / 1.055,
                0.055 / 1.055,
                1.0 / 12.92,
                0.04045,
                0.0,
                0.0,
            ]),
        ),
        (
            "srgb_inv",
            tf([1.0 / 2.4, 1.137_119_5, 0.0, 12.92, 0.003_130_8, -0.055, 0.0]),
        ),
        ("gamma22", tf([2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0])),
        (
            "rec709",
            tf([
                1.0 / 0.45,
                1.0 / 1.099,
                0.099 / 1.099,
                1.0 / 4.5,
                0.081,
                0.0,
                0.0,
            ]),
        ),
        ("offsets", tf([1.8, 0.9, 0.1, 0.5, 0.3, 0.05, -0.02])),
        ("linear", tf([1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0])),
        ("d0", tf([3.0, 1.0, 0.0, 0.5, 0.0, 0.25, 0.125])),
        ("dbig", tf([2.0, 1.0, 0.0, 2.0, 1e30, 0.0, 1.0])),
        ("g0", tf([0.0, 1.0, 0.0, 1.0, 0.1, 0.0, 0.0])),
        ("gneg", tf([-1.0, 2.0, 0.5, 1.0, 0.1, 0.0, 0.0])),
        ("ghalf", tf([0.5, 1.0, 0.0, 1.0, 0.04, 0.0, 0.0])),
    ]
}

fn pq_sets() -> Vec<(&'static str, Ctx)> {
    vec![
        // skcms_PQ: {-2, -107/128, 1, 32/2523, 2413/128, -2392/128, 8192/1305}.
        (
            "pq",
            tf([
                -2.0,
                -107.0 / 128.0,
                1.0,
                32.0 / 2523.0,
                2413.0 / 128.0,
                -2392.0 / 128.0,
                8192.0 / 1305.0,
            ]),
        ),
        // Moderate parameters: nothing overflows (an overflow gives `inf / inf`, whose NaN the
        // x86 models only get right on x86 hosts; see `expected::output_matches`).
        ("pq_mid", tf([-5.0, 0.1, 2.0, 0.5, 0.25, 0.8, 0.45])),
        ("odd", tf([0.0, 0.1, 0.9, 0.3, 1.7, 0.6, 1.3])),
        ("zero", tf([0.0; 7])),
    ]
}

fn hlg_sets() -> Vec<(&'static str, Ctx)> {
    vec![
        // skcms_HLG: {-3, 2, 2, 1/0.17883277, 0.28466892, 0.55991073, 0}.
        (
            "hlg",
            tf([
                -3.0,
                2.0,
                2.0,
                1.0 / 0.178_832_77,
                0.284_668_92,
                0.559_910_73,
                0.0,
            ]),
        ),
        (
            "hlg_k",
            tf([-3.0, 2.0, 2.0, 5.591_816, 0.284_668_92, 0.559_910_73, 11.0]),
        ),
        // skcms_HLGinv: {-4, 2, 2, 0.17883277, 0.28466892, 0.55991073, 0}.
        (
            "hlginv",
            tf([
                -4.0,
                2.0,
                2.0,
                0.178_832_77,
                0.284_668_92,
                0.559_910_73,
                0.0,
            ]),
        ),
        (
            "hlginv_k",
            tf([
                -4.0,
                0.5,
                0.75,
                0.178_832_77,
                0.284_668_92,
                0.559_910_73,
                3.0,
            ]),
        ),
        ("odd", tf([0.0, 1.3, 0.7, 0.4, 0.1, 0.2, 0.5])),
    ]
}

fn random_floats(n: usize, seed: u32, lo: f32, hi: f32) -> Vec<f32> {
    let mut rng = Rng::new(0x5151_5153 ^ seed.wrapping_mul(0x85eb_ca6b));
    (0..n).map(|_| lo + (hi - lo) * unit(&mut rng)).collect()
}

/// Matrix contexts of `n` floats (`rows x cols`, with the last `cols - 3`... whatever Skia
/// reads): identity-like, luminance, random, sparse, extreme.
fn matrix_sets(c: &mut Cases, n: usize, ident: &[f32]) -> Vec<(String, Ctx)> {
    let mut v: Vec<(String, Ctx)> = vec![("identity".into(), f32s(ident))];
    let rand = |c: &mut Cases, lo, hi| random_floats(n, c.next_seed(), lo, hi);
    v.push(("unit".into(), Ctx::F32(rand(c, 0.0, 1.0))));
    v.push(("signed".into(), Ctx::F32(rand(c, -2.0, 2.0))));
    v.push(("small".into(), Ctx::F32(rand(c, -0.01, 0.01))));
    let mut sparse = rand(c, -1.0, 1.0);
    for (i, x) in sparse.iter_mut().enumerate() {
        if i % 2 == 0 {
            *x = 0.0;
        }
    }
    v.push(("sparse".into(), Ctx::F32(sparse)));
    let mut ext: Vec<f32> = (0..n)
        .map(|i| f32::from_bits(float_specials()[(i * 7 + 3) % 44]))
        .collect();
    for x in &mut ext {
        if x.is_infinite() {
            *x = 3.0e38;
        }
    }
    v.push(("extreme".into(), Ctx::F32(ext)));
    v
}

fn swizzles() -> Vec<[u8; 4]> {
    let mut v: Vec<[u8; 4]> = Vec::new();
    // The 24 permutations of rgba.
    let ch = *b"rgba";
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                for l in 0..4 {
                    let mut s = [i, j, k, l];
                    s.sort_unstable();
                    if s == [0, 1, 2, 3] {
                        v.push([ch[i], ch[j], ch[k], ch[l]]);
                    }
                }
            }
        }
    }
    v.extend([
        *b"aaaa", *b"rrrr", *b"bbba", *b"0001", *b"1111", *b"000a", *b"rgb1", *b"rgb0", *b"111a",
        *b"01rg", *b"r0g1", *b"a10b", *b"rxbx", *b"xxxx", *b"gxba",
    ]);
    v
}

// ---------------------------------------------------------------------------------------------

/// Adds the cases of the B3 and B4 stages.
pub fn generate(c: &mut Cases) {
    b3_blend(c);
    b3_coverage(c);
    b4_color(c);
    b4_contexts(c);
    pixel_ops(c);
}

/// Every stage again over pixel memory: the output does not depend on the tier's register
/// layout, so the stored hashes of the five paths can be compared with each other (which stages
/// the estimates, fused `mad`s and `floor_` separate; design §1.4).
fn pixel_ops(c: &mut Cases) {
    use Op::*;
    let plain_both = [
        Clear,
        Srcatop,
        Dstatop,
        Srcin,
        Dstin,
        Srcout,
        Dstout,
        Dstover,
        Modulate,
        Multiply,
        Plus,
        Screen,
        Xor,
        Darken,
        Lighten,
        Difference,
        Exclusion,
        Hardlight,
        Overlay,
        Clamp01,
        ClampA01,
        ClampGamut,
        Premul,
        PremulDst,
        ForceOpaque,
        ForceOpaqueDst,
        BlackColor,
        WhiteColor,
        Bt709LuminanceOrLumaToAlpha,
        Bt709LuminanceOrLumaToRgb,
    ];
    let plain_highp = [
        Colorburn,
        Colordodge,
        Softlight,
        Hue,
        Saturation,
        Color,
        Luminosity,
        Unpremul,
        UnpremulPolar,
        RgbToHsl,
        HslToRgb,
        CssLabToXyz,
        CssOklabToLinearSrgb,
        CssOklabGamutMapToLinearSrgb,
        CssHclToLab,
        CssHslToSrgb,
        CssHwbToSrgb,
        GaussAToRgba,
    ];
    let mut jobs: Vec<(String, Vec<Precision>, StageSpec)> = Vec::new();
    for op in plain_both {
        jobs.push((
            op.name().to_owned(),
            Precision::BOTH.to_vec(),
            StageSpec::new(op),
        ));
    }
    for op in plain_highp {
        jobs.push((
            op.name().to_owned(),
            Precision::HIGHP.to_vec(),
            StageSpec::new(op),
        ));
    }
    let with = |name: &str, both: bool, op: Op, ctx: Ctx| {
        (
            name.to_owned(),
            if both {
                Precision::BOTH
            } else {
                Precision::HIGHP
            }
            .to_vec(),
            StageSpec::with(op, ctx),
        )
    };
    let pc = |sets: Vec<(&'static str, Ctx)>, i: usize| sets[i].1.clone();
    jobs.extend([
        with("set_rgb", true, SetRgb, f32s(&[0.5 / 255.0, 0.3, 1.2])),
        with(
            "unbounded_set_rgb",
            false,
            UnboundedSetRgb,
            f32s(&[0.5 / 255.0, 0.3, 1.2]),
        ),
        with(
            "uniform_color",
            true,
            UniformColor,
            uniform([0.1, 0.5, 0.9, 0.7]),
        ),
        with(
            "uniform_color_dst",
            true,
            UniformColorDst,
            uniform([0.1, 0.5, 0.9, 0.7]),
        ),
        with(
            "unbounded_uniform_color",
            false,
            UnboundedUniformColor,
            uniform([-0.1, 1.5, 0.9, 0.7]),
        ),
        with("scale_1_float", true, Scale1Float, f32s(&[1.0 / 3.0])),
        with("lerp_1_float", true, Lerp1Float, f32s(&[1.0 / 3.0])),
        with("swizzle", true, Swizzle, Ctx::U8x4(*b"bgra")),
        with("dither", false, Dither, f32s(&[1.0 / 255.0])),
        with("gamma_", false, Gamma, f32s(&[2.2])),
        with("ootf", false, Ootf, f32s(&[0.2627, 0.678, 0.0593, 0.2])),
        with(
            "parametric_srgb",
            false,
            Parametric,
            pc(parametric_sets(), 0),
        ),
        with(
            "parametric_offsets",
            false,
            Parametric,
            pc(parametric_sets(), 4),
        ),
        with("pq", false, PQish, pc(pq_sets(), 0)),
        with("hlg", false, HLGish, pc(hlg_sets(), 0)),
        with("hlg_k", false, HLGish, pc(hlg_sets(), 1)),
        with("hlginv", false, HLGinvish, pc(hlg_sets(), 2)),
    ]);
    let id9 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let (m3, m4) = (matrix_sets(c, 9, &id9), matrix_sets(c, 20, &[0.0; 20]));
    jobs.push(with("matrix_3x3", false, Matrix3x3, m3[2].1.clone()));
    jobs.push(with("matrix_4x5", false, Matrix4x5, m4[2].1.clone()));
    let m12 = matrix_sets(c, 12, &[0.0; 12]);
    jobs.push(with("matrix_3x4", false, Matrix3x4, m12[2].1.clone()));
    jobs.push(with("matrix_4x3", false, Matrix4x3, m12[1].1.clone()));
    for (name, precisions, stage) in jobs {
        for &p in &precisions {
            for kind in PIX_KINDS {
                let (x, y, w, h) = (2, 1, 19, 2);
                let (stride, rows) = (x + w + 3, y + h + 1);
                pixel(
                    c,
                    format!("px/{name}/{}/{}", p.name(), kind.name()),
                    p,
                    kind,
                    std::slice::from_ref(&stage),
                    vec![],
                    stride,
                    rows,
                    false,
                    vec![Rect::new(x, y, w, h)],
                );
            }
        }
    }
    // Tables need no layout either.
    let mut rng = Rng::new(0x007a_b1e5);
    let t: Vec<u8> = (0..1024)
        .map(|_| u8::try_from(rng.below(256)).unwrap())
        .collect();
    for kind in PIX_KINDS {
        pixel(
            c,
            format!("px/byte_tables/highp/{}", kind.name()),
            Precision::Highp,
            kind,
            &[StageSpec::with(ByteTables, Ctx::Tables(t.clone()))],
            vec![],
            24,
            4,
            false,
            vec![Rect::new(2, 1, 19, 2)],
        );
    }
}

/// B3: Porter-Duff, separable and non-separable blend modes. `colorburn`, `colordodge` and the
/// non-separable modes use `rcp_fast` (the estimate paths), `softlight` a division and `sqrt`.
fn b3_blend(c: &mut Cases) {
    use Op::{
        Clear, Color, Colorburn, Colordodge, Darken, Difference, Dstatop, Dstin, Dstout, Dstover,
        Exclusion, Hardlight, Hue, Lighten, Luminosity, Modulate, Multiply, Overlay, Plus,
        Saturation, Screen, Softlight, Srcatop, Srcin, Srcout, Xor,
    };
    for op in [
        Clear, Srcatop, Dstatop, Srcin, Dstin, Srcout, Dstout, Dstover, Modulate, Multiply, Plus,
        Xor, Darken, Lighten, Difference, Exclusion, Hardlight, Overlay,
    ] {
        plain(c, op, Precision::BOTH);
    }
    // `screen` is `nmad(s, d, s + d)`: a NaN `s` (or `d`) is both the product's and the addend's
    // NaN, so which one the fused instruction returns depends on the `vfnmadd...` form the
    // compiler picked (Skia's clang and rustc differ on ml3/ml4, the models agree with Skia).
    // That is the NaN-meets-NaN rule of `Inputs`, so `screen` runs without NaN inputs.
    regs_where(
        c,
        Screen.name(),
        &[StageSpec::new(Screen)],
        Precision::BOTH,
        REGISTER_WIDTHS,
        &no_extra,
        &|k| !matches!(k, Kind::Std(Inputs::NanSrc | Inputs::NanDst)),
    );
    for op in [
        Colorburn, Colordodge, Softlight, Hue, Saturation, Color, Luminosity,
    ] {
        plain(c, op, Precision::HIGHP);
    }
    // Sequences: a blend after a premultiply, as the blitters build them.
    for op in [Srcatop, Multiply, Hardlight, Overlay, Difference] {
        regs(
            c,
            &format!("premul_then_{}", op.name()),
            &[
                StageSpec::new(Op::Premul),
                StageSpec::new(Op::PremulDst),
                StageSpec::new(op),
            ],
            Precision::BOTH,
            CTX_WIDTHS,
            &no_extra,
        );
    }
    for op in [
        Colorburn, Colordodge, Softlight, Hue, Saturation, Color, Luminosity,
    ] {
        regs(
            c,
            &format!("premul_then_{}", op.name()),
            &[
                StageSpec::new(Op::Premul),
                StageSpec::new(Op::PremulDst),
                StageSpec::new(op),
            ],
            Precision::HIGHP,
            CTX_WIDTHS,
            &no_extra,
        );
    }
}

/// B3: `scale_*` and `lerp_*`.
fn b3_coverage(c: &mut Cases) {
    // Memory coverage.
    coverage(c, Op::ScaleU8, 1, plane8);
    coverage(c, Op::LerpU8, 1, plane8);
    coverage(c, Op::Scale565, 2, plane565);
    coverage(c, Op::Lerp565, 2, plane565);

    // `*_1_float`: a constant coverage.
    let coverages: [(&str, f32); 16] = [
        ("zero", 0.0),
        ("one", 1.0),
        ("half", 0.5),
        ("third", 1.0 / 3.0),
        ("almost1", 0.999),
        ("round", 127.5 / 255.0),
        ("round_up", f32::from_bits((127.5f32 / 255.0).to_bits() + 1)),
        ("two", 2.0),
        ("neg", -0.5),
        ("big", 255.5),
        ("tiny", 1e-39),
        ("u8", 254.0 / 255.0),
        ("big16", 65535.5),
        ("huge", 1e30),
        ("neg_huge", -1e30),
        ("inf", f32::INFINITY),
    ];
    for op in [Op::Scale1Float, Op::Lerp1Float] {
        for (tag, v) in coverages {
            regs(
                c,
                &format!("{}/{tag}", op.name()),
                &[StageSpec::with(op, Ctx::F32(vec![v]))],
                Precision::BOTH,
                CTX_WIDTHS,
                &no_extra,
            );
        }
    }

    // `*_native`: a pointer to N per-lane coverages (highp: N floats; lowp: N 16-bit values).
    for op in [Op::ScaleNative, Op::LerpNative] {
        for (tag, kind) in [
            ("a", Inputs::Unit),
            ("b", Inputs::Bits),
            ("c", Inputs::Special),
        ] {
            for &p in Precision::BOTH {
                // The coverage values of lowp are bytes; of highp unit floats / specials.
                let seed = c.next_seed();
                let data = match (p, kind) {
                    (Precision::Auto, Inputs::Unit) => {
                        crate::cases::register_input(Inputs::Bytes, seed)
                    }
                    (Precision::Auto, Inputs::Special) => extra_input(Kind::Edge16, seed),
                    (_, k) => crate::cases::register_input(k, seed),
                };
                regs(
                    c,
                    &format!("{}/{tag}", op.name()),
                    &[StageSpec::with(op, Ctx::Ptr { slot: 4, offset: 0 })],
                    &[p],
                    CTX_WIDTHS,
                    &|_| vec![Buffer::new(data.clone())],
                );
            }
        }
    }
}

/// B4: stages without a context.
fn b4_color(c: &mut Cases) {
    for op in [
        Op::Clamp01,
        Op::ClampA01,
        Op::ClampGamut,
        Op::Premul,
        Op::PremulDst,
        Op::ForceOpaque,
        Op::ForceOpaqueDst,
        Op::BlackColor,
        Op::WhiteColor,
        Op::Bt709LuminanceOrLumaToAlpha,
        Op::Bt709LuminanceOrLumaToRgb,
    ] {
        plain(c, op, Precision::BOTH);
    }
    for op in [
        Op::Unpremul,
        Op::UnpremulPolar,
        Op::RgbToHsl,
        Op::HslToRgb,
        Op::CssLabToXyz,
        Op::CssOklabToLinearSrgb,
        Op::CssOklabGamutMapToLinearSrgb,
        Op::CssHclToLab,
        Op::CssHslToSrgb,
        Op::CssHwbToSrgb,
        Op::GaussAToRgba,
    ] {
        plain(c, op, Precision::HIGHP);
    }
    // Sequences the blitters and color filters build.
    regs(
        c,
        "premul_unpremul",
        &[StageSpec::new(Op::Premul), StageSpec::new(Op::Unpremul)],
        Precision::HIGHP,
        CTX_WIDTHS,
        &no_extra,
    );
    regs(
        c,
        "unpremul_clamp_premul",
        &[
            StageSpec::new(Op::Unpremul),
            StageSpec::new(Op::Clamp01),
            StageSpec::new(Op::Premul),
        ],
        Precision::HIGHP,
        CTX_WIDTHS,
        &no_extra,
    );
    regs(
        c,
        "rgb_to_hsl_to_rgb",
        &[StageSpec::new(Op::RgbToHsl), StageSpec::new(Op::HslToRgb)],
        Precision::HIGHP,
        CTX_WIDTHS,
        &no_extra,
    );
    regs(
        c,
        "luminance_clamp",
        &[
            StageSpec::new(Op::Bt709LuminanceOrLumaToRgb),
            StageSpec::new(Op::ClampGamut),
        ],
        Precision::BOTH,
        CTX_WIDTHS,
        &no_extra,
    );
}

/// B4: stages with a context.
fn b4_contexts(c: &mut Cases) {
    // set_rgb / unbounded_set_rgb.
    for (tag, v) in rgb_sets() {
        with_ctxs(
            c,
            Op::SetRgb,
            Precision::BOTH,
            &[(tag, Ctx::F32(v.clone()))],
        );
        with_ctxs(
            c,
            Op::UnboundedSetRgb,
            Precision::HIGHP,
            &[(tag, Ctx::F32(v))],
        );
    }
    // uniform_color and friends.
    for (tag, u) in uniform_sets() {
        for op in [Op::UniformColor, Op::UniformColorDst] {
            with_ctxs(c, op, Precision::BOTH, &[(tag, u.clone())]);
        }
        with_ctxs(c, Op::UnboundedUniformColor, Precision::HIGHP, &[(tag, u)]);
    }
    let _ = UniformColorCtx::default();

    // swizzle.
    for s in swizzles() {
        let tag = String::from_utf8(s.to_vec()).expect("ascii");
        let full = matches!(&s, b"bgra" | b"rgba" | b"aaaa" | b"0001" | b"rxbx");
        regs(
            c,
            &format!("swizzle/{tag}"),
            &[StageSpec::with(Op::Swizzle, Ctx::U8x4(s))],
            Precision::BOTH,
            if full { REGISTER_WIDTHS } else { &[7] },
            &no_extra,
        );
    }

    // dither: 8x8 ordered dither over rects at several offsets.
    for rate in [1.0f32 / 255.0, 0.5 / 255.0, 0.0, 1.0, -0.25, 1e-30] {
        let tag = format!("{:08x}", rate.to_bits());
        for kind in PIX_KINDS {
            for (i, r) in [
                Rect::new(0, 0, 19, 2),
                Rect::new(5, 3, 17, 3),
                Rect::new(13, 6, 9, 2),
                Rect::new(1, 7, 4, 1),
            ]
            .into_iter()
            .enumerate()
            {
                let (stride, rows) = (r.x + r.w + 1, r.y + r.h + 1);
                pixel(
                    c,
                    format!("dither/{tag}/{}/r{i}", kind.name()),
                    Precision::Highp,
                    kind,
                    &[StageSpec::with(Op::Dither, Ctx::F32(vec![rate]))],
                    vec![],
                    stride,
                    rows,
                    false,
                    vec![r],
                );
            }
            pixel(
                c,
                format!("dither/{tag}/{}/compiled", kind.name()),
                Precision::Highp,
                kind,
                &[StageSpec::with(Op::Dither, Ctx::F32(vec![rate]))],
                vec![],
                32,
                8,
                true,
                vec![
                    Rect::new(0, 0, 5, 2),
                    Rect::new(7, 3, 17, 2),
                    Rect::new(2, 6, 29, 2),
                ],
            );
        }
        // And the register view of the last chunk, at far coordinates.
        regs(
            c,
            &format!("dither_regs/{tag}"),
            &[StageSpec::with(Op::Dither, Ctx::F32(vec![rate]))],
            Precision::HIGHP,
            CTX_WIDTHS,
            &no_extra,
        );
    }

    // byte_tables.
    let table = |f: &dyn Fn(usize) -> u8| -> Vec<u8> { (0..256).map(f).collect() };
    let mut rng = Rng::new(0xb17e_7ab1);
    let random: Vec<u8> = (0..1024)
        .map(|_| u8::try_from(rng.below(256)).unwrap())
        .collect();
    let id = table(&|i| u8::try_from(i).unwrap());
    let inv = table(&|i| u8::try_from(255 - i).unwrap());
    let sets: Vec<(&str, Vec<u8>)> = vec![
        ("identity", [&id[..], &id, &id, &id].concat()),
        ("invert", [&inv[..], &inv, &inv, &inv].concat()),
        (
            "mixed",
            [&id[..], &inv, &random[..256], &random[256..512]].concat(),
        ),
        ("random", random),
    ];
    for (tag, t) in sets {
        regs(
            c,
            &format!("byte_tables/{tag}"),
            &[StageSpec::with(Op::ByteTables, Ctx::Tables(t))],
            Precision::HIGHP,
            CTX_WIDTHS,
            &no_extra,
        );
    }

    // emboss: two A8 planes.
    for p in [Precision::Auto, Precision::Highp] {
        for kind in PIX_KINDS {
            for &w in TAIL_WIDTHS {
                let (x, y, h) = (3, 1, 2);
                let stride = x + w + 2;
                let rows = y + h + 1;
                let (s1, s2) = (c.next_seed(), c.next_seed());
                let layout = |b: Buffer| b.with_layout(isize::try_from(stride).unwrap(), 0);
                pixel(
                    c,
                    format!("emboss/{}/{}/w{w}", p.name(), kind.name()),
                    p,
                    kind,
                    &[StageSpec::with(Op::Emboss, Ctx::Emboss { mul: 4, add: 5 })],
                    vec![
                        layout(Buffer::new(plane8(stride * rows, s1))),
                        layout(Buffer::new(plane8(stride * rows, s2))),
                    ],
                    stride,
                    rows,
                    false,
                    vec![Rect::new(x, y, w, h)],
                );
            }
        }
    }

    // matrices.
    let ident3x3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let ident3x4 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let ident4x3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let mut ident4x5 = [0.0; 20];
    for i in 0..4 {
        ident4x5[i * 5 + i] = 1.0;
    }
    let mut ident4x5_t = ident4x5.to_vec();
    // The layout Skia's color filter matrices use: rows of 4 plus a translation column.
    ident4x5_t[4] = 0.25;
    for (op, n, ident) in [
        (Op::Matrix3x3, 9, ident3x3.to_vec()),
        (Op::Matrix3x4, 12, ident3x4.to_vec()),
        (Op::Matrix4x3, 12, ident4x3.to_vec()),
        (Op::Matrix4x5, 20, ident4x5_t),
    ] {
        for (tag, ctx) in matrix_sets(c, n, &ident) {
            with_ctxs(c, op, Precision::HIGHP, &[(&tag, ctx)]);
        }
    }

    // Transfer functions.
    with_ctxs(c, Op::Parametric, Precision::HIGHP, &parametric_sets());
    with_ctxs(c, Op::PQish, Precision::HIGHP, &pq_sets());
    for op in [Op::HLGish, Op::HLGinvish] {
        with_ctxs(c, op, Precision::HIGHP, &hlg_sets());
    }
    for (tag, g) in [
        ("2_2", 2.2f32),
        ("inv2_2", 1.0 / 2.2),
        ("one", 1.0),
        ("zero", 0.0),
        ("neg", -1.0),
        ("half", 0.5),
        ("big", 8.0),
    ] {
        with_ctxs(c, Op::Gamma, Precision::HIGHP, &[(tag, f32s(&[g]))]);
    }
    for (tag, o) in [
        ("rec2020", [0.2627f32, 0.678, 0.0593, 0.2]),
        ("rec709", [0.2126, 0.7152, 0.0722, 0.1]),
        ("sum1", [0.5, 0.25, 0.25, 1.0]),
        ("neg_gamma", [0.3, 0.3, 0.4, -0.5]),
        ("zero_gamma", [0.3, 0.3, 0.4, 0.0]),
        ("big", [4.0, 4.0, 4.0, 3.0]),
    ] {
        with_ctxs(c, Op::Ootf, Precision::HIGHP, &[(tag, f32s(&o))]);
    }
}
