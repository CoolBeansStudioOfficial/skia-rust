// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! B6 cases: the `SkSL` stages (B6a masks, branches, copies, swizzles; B6b arithmetic; B6c math).
//! The trace ops (B6d, `trace_*`) and `callback` have no cases: they report to a host-side hook
//! (`SkSL::TraceHook`, a function pointer), which the text format cannot express and the driver
//! has no `SkSL` library to build; they are covered by Skia's own tests (ported in `tests/`).
//!
//! # Layout
//! `SkSL` programs keep their variables in *slots*, vectors of `N` lanes (`N` = the tier's highp
//! stride) addressed from a base pointer, so slot `s` is bytes `[4Ns, 4N(s+1))` of the slot
//! buffer. Every case here is
//! `set_base_pointer(buf 0) load_src(buf 1) <stages> store_src(buf 2)`: buffer 0 holds the slots,
//! buffer 1 the `r, g, b, a` registers (the masks of the control-flow stages), buffer 2 receives
//! the registers. The output compared is all three buffers, so every lane of every slot is
//! checked, and a case's text is the same for all tiers: contexts name slots by index
//! ([`Ctx::SkslPtr`] and friends), each side turning them into the tier's byte offsets.
//!
//! The bytes of the slot buffer are generated per word ([`Data`]); which lane of which slot a
//! word is depends on `N`, so a word's role changes between tiers while every tier still sees a
//! full, varied set of lanes.
//!
//! # NaNs
//! As for the register cases ([`crate::cases::Inputs`]): two NaNs must not meet in one operation.
//! Float data therefore has at most one NaN word ([`Data::Nan`]) and no other NaN source.
//!
//! # Scalar and R5
//! Cases whose inputs reach a float → integer conversion out of range, a negative float → `U32`
//! conversion or a `±0` tie in `fminf`/`fmaxf` carry a `/r5/` name segment (see
//! [`Case::scalar_proxy_differs`](crate::case::Case::scalar_proxy_differs)): the `Scalar` tier
//! follows wasm there, Skia's x64 scalar proxy follows x86.

// Test data is built from small indices and bit patterns; the casts are exact or intended. The
// generators are tables of cases, one block per op family.
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_simd::rp::Op;

use crate::case::{Buffer, Case, Ctx, Rect, StageSpec};
use crate::cases::{Cases, Inputs, REG_BYTES, Rng, TAIL_WIDTHS, float_specials, output};

/// Words reserved per slot in the slot buffer (the largest `N`).
const SLOT_WORDS: usize = 16;

/// The op with Skia's name `name`.
///
/// # Panics
/// If there is none.
fn op(name: &str) -> Op {
    *Op::ALL
        .iter()
        .find(|o| o.name() == name)
        .unwrap_or_else(|| panic!("no raster pipeline op named {name}"))
}

/// Slot data: how the words of the slot buffer are generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Data {
    /// Special non-NaN floats (±0, denormals, 0.5±ulp, 255.5, 2³¹, `FLT_MAX`, ±inf, ...); `k`
    /// varies the rotation.
    Special(u32),
    /// Tame floats (magnitude in `[0.25, 2)`) with one NaN at this word.
    Nan(usize),
    /// Floats in `[0, 1]`.
    Unit,
    /// Floats in `[-1, 1]` (never `-0`).
    Signed,
    /// Floats of magnitude `2^-10 .. 2^10`, random sign.
    Wide,
    /// Floats of magnitude in `[0.25, 2)`, random sign.
    Tame,
    /// Floats from {0, 0, 0, 1, -1, 0.5, -0.5}: singular matrices, identities, inside the domain
    /// of `asin`, and no internal NaNs.
    Sparse,
    /// Floats with random bits (NaNs made finite).
    Bits,
    /// Floats in `±2^26` with fractions: in range for conversions to `int`.
    Range,
    /// As `Range`, non-negative: in range for conversions to `uint`.
    URange,
    /// Interesting `int`s: 0, ±1, powers, `INT_MIN`, `INT_MAX`, ... (`k` varies the pairing).
    Ints(u32),
    /// Interesting `uint`s.
    Uints(u32),
    /// Dividend/divisor edge pairs for `int` division: even words are the dividend and odd words
    /// the divisor of pair `i` (`INT_MIN / -1`, `x / 0`, ...): on the `Scalar` tier (one lane
    /// per slot) word 0 and word 1 are exactly the pair; wider tiers see them interleaved.
    IntPair(u32),
    /// As `IntPair` for `uint` division.
    UintPair(u32),
    /// `int`s in `-8 ..= 8`.
    Small,
    /// Lane masks: 0, all ones, sign bit only, 1, random.
    Mask(u32),
    /// Indices `0 .. limit + 4` plus a few huge values (for the indirect copies).
    Index(u32),
}

impl Data {
    fn name(self) -> String {
        match self {
            Data::Special(k) => format!("special{k}"),
            Data::Nan(p) => format!("nan{p}"),
            Data::Unit => "unit".into(),
            Data::Signed => "signed".into(),
            Data::Wide => "wide".into(),
            Data::Tame => "tame".into(),
            Data::Sparse => "sparse".into(),
            Data::Bits => "bits".into(),
            Data::Range => "range".into(),
            Data::URange => "urange".into(),
            Data::Ints(k) => format!("ints{k}"),
            Data::Uints(k) => format!("uints{k}"),
            Data::IntPair(i) => format!("ipair{i}"),
            Data::UintPair(i) => format!("upair{i}"),
            Data::Small => "small".into(),
            Data::Mask(k) => format!("mask{k}"),
            Data::Index(l) => format!("idx{l}"),
        }
    }
}

const NANS: [u32; 8] = [
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc1_2345,
    0xffc1_2345,
    0x7f80_0001,
    0xff80_0001,
    0x7fa5_a5a5,
    0xffbf_ffff,
];

const INTS: [i32; 25] = [
    0,
    1,
    -1,
    2,
    -2,
    3,
    7,
    -7,
    10,
    100,
    -100,
    255,
    256,
    65535,
    65536,
    -65536,
    i32::MAX,
    i32::MIN,
    i32::MAX - 1,
    i32::MIN + 1,
    0x1234_5678,
    -0x1234_5678,
    0x5555_5555,
    0xAAAA_AAAAu32 as i32,
    1 << 30,
];

/// (dividend, divisor) pairs of `int` division: overflow, division by zero, signs, exact and
/// inexact quotients, magnitudes beyond 24 bits.
const INT_PAIRS: [(i32, i32); 16] = [
    (i32::MIN, -1),
    (i32::MAX, -1),
    (7, 0),
    (-7, 0),
    (i32::MIN, 0),
    (0, 0),
    (1, -1),
    (i32::MIN, 1),
    (i32::MIN, i32::MIN),
    (i32::MAX, i32::MAX),
    (-7, 2),
    (7, -2),
    (100, 3),
    (-100, 3),
    (0x7fff_ffff, 3),
    (-0x1234_5679, 0x1000),
];

/// (dividend, divisor) pairs of `uint` division.
const UINT_PAIRS: [(u32, u32); 12] = [
    (u32::MAX, 0),
    (7, 0),
    (0, 0),
    (u32::MAX, 1),
    (0x8000_0000, u32::MAX),
    (u32::MAX, u32::MAX),
    (u32::MAX, 3),
    (0x8000_0000, 7),
    (100, 3),
    (0xdead_beef, 0x1_0001),
    (1, 2),
    (0xffff_fffe, 0x7fff_ffff),
];

const UINTS: [u32; 19] = [
    0,
    1,
    2,
    3,
    7,
    10,
    100,
    255,
    256,
    65535,
    65536,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xffff_fffe,
    0xffff_ffff,
    0x1234_5678,
    0xdead_beef,
    1 << 30,
];

/// `f32` with a random sign.
fn signed(rng: &mut Rng, magnitude: f32) -> u32 {
    magnitude.to_bits() | (rng.next_u32() & 0x8000_0000)
}

/// `count` words of `data`.
fn data_words(data: Data, count: usize, seed: u32) -> Vec<u32> {
    let mut rng = Rng::new((seed.wrapping_mul(0x2545_f491) ^ 0x1234_5679) | 1);
    let count32 = u32::try_from(count).expect("small");
    match data {
        Data::Special(k) => {
            let s = float_specials();
            let n = u32::try_from(s.len()).expect("few");
            let start = (k.wrapping_mul(13) + rng.below(n)) % n;
            // Coprime with 44.
            let step = [5, 7, 9, 13][((k + rng.below(4)) % 4) as usize];
            (0..count32)
                .map(|j| s[((start + j * step) % n) as usize])
                .collect()
        }
        Data::Nan(p) => {
            let mut w: Vec<u32> = (0..count)
                .map(|_| (rng.next_u32() & 0x8000_0000) | (0x3e80_0000 + rng.below(3 << 23)))
                .collect();
            if p < w.len() {
                w[p] = NANS[rng.below(8) as usize];
            }
            w
        }
        Data::Unit => (0..count32)
            .map(|j| match j % 29 {
                0 => 0,
                1 => 1.0f32.to_bits(),
                _ => ((rng.next_u32() >> 8) as f32 / 16_777_216.0).to_bits(),
            })
            .collect(),
        Data::Signed => (0..count32)
            .map(|j| match j % 29 {
                0 => 0,
                1 => (-1.0f32).to_bits(),
                2 => 1.0f32.to_bits(),
                _ => {
                    let m = (rng.next_u32() >> 8) as f32 / 16_777_216.0;
                    if m == 0.0 { 0 } else { signed(&mut rng, m) }
                }
            })
            .collect(),
        Data::Wide => (0..count)
            .map(|_| {
                let e = 127 - 10 + rng.below(21);
                (rng.next_u32() & 0x8000_0000) | (e << 23) | (rng.next_u32() & 0x007f_ffff)
            })
            .collect(),
        Data::Sparse => {
            const SPARSE: [f32; 7] = [0.0, 0.0, 0.0, 1.0, -1.0, 0.5, -0.5];
            (0..count)
                .map(|_| SPARSE[rng.below(7) as usize].to_bits())
                .collect()
        }
        Data::Tame => (0..count)
            .map(|_| (rng.next_u32() & 0x8000_0000) | (0x3e80_0000 + rng.below(3 << 23)))
            .collect(),
        Data::Bits => (0..count)
            .map(|_| {
                let w = rng.next_u32();
                if f32::from_bits(w).is_nan() {
                    w & !0x4000_0000
                } else {
                    w
                }
            })
            .collect(),
        Data::Range | Data::URange => (0..count)
            .map(|j| {
                let m = rng.below(1 << 24) as f32 / 16.0 * (1u32 << rng.below(7)) as f32;
                let m = match j % 11 {
                    0 => 0.0,
                    1 => 0.5,
                    2 => 1.5,
                    3 => 2.5,
                    _ => m,
                };
                if data == Data::Range && m != 0.0 {
                    signed(&mut rng, m)
                } else {
                    m.to_bits()
                }
            })
            .collect(),
        Data::Ints(k) => {
            let n = u32::try_from(INTS.len()).expect("few");
            let start = (k * 7 + rng.below(n)) % n;
            let step = [1, 3, 5, 7, 9, 11, 13, 17][(k % 8) as usize];
            (0..count32)
                .map(|j| INTS[((start + j * step) % n) as usize] as u32)
                .collect()
        }
        Data::Uints(k) => {
            let n = u32::try_from(UINTS.len()).expect("few");
            let start = (k * 5 + rng.below(n)) % n;
            let step = [1, 2, 4, 5, 7, 8, 10, 11][(k % 8) as usize];
            (0..count32)
                .map(|j| UINTS[((start + j * step) % n) as usize])
                .collect()
        }
        Data::IntPair(i) => {
            let (a, b) = INT_PAIRS[i as usize % INT_PAIRS.len()];
            (0..count)
                .map(|j| if j % 2 == 0 { a as u32 } else { b as u32 })
                .collect()
        }
        Data::UintPair(i) => {
            let (a, b) = UINT_PAIRS[i as usize % UINT_PAIRS.len()];
            (0..count).map(|j| if j % 2 == 0 { a } else { b }).collect()
        }
        Data::Small => (0..count)
            .map(|_| (rng.below(17) as i32 - 8) as u32)
            .collect(),
        Data::Mask(_) => (0..count)
            .map(|_| match rng.below(5) {
                0 => 0,
                1 => !0,
                2 => 0x8000_0000,
                3 => 1,
                _ => rng.next_u32(),
            })
            .collect(),
        Data::Index(limit) => (0..count)
            .map(|_| match rng.below(12) {
                0 => 0xffff_ffff,
                1 => 0x7fff_ffff,
                2 => 0x8000_0000,
                3 => limit,
                _ => rng.below(limit + 4),
            })
            .collect(),
    }
}

/// The slot buffer: `nslots` slots of `base` data; the words that are lanes of the slots in
/// `overrides` (for every possible `N`) are replaced by the given data.
fn slot_buffer(nslots: u32, base: Data, overrides: &[(u32, Data)], seed: u32) -> Vec<u8> {
    let total = nslots as usize * SLOT_WORDS;
    let mut words = data_words(base, total, seed);
    for &(slot, d) in overrides {
        let patch = data_words(d, total, seed ^ 0x77);
        let slot = slot as usize;
        for n in [1usize, 4, 8, 16] {
            for i in slot * n..(slot + 1) * n {
                if i < total {
                    words[i] = patch[i];
                }
            }
        }
    }
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Lane-dependent mask patterns for the `a` register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskPat {
    First,
    Last,
    Even,
    Sign,
    Low,
}

impl MaskPat {
    fn name(self) -> &'static str {
        match self {
            MaskPat::First => "first",
            MaskPat::Last => "last",
            MaskPat::Even => "even",
            MaskPat::Sign => "sign",
            MaskPat::Low => "low",
        }
    }

    /// Lane `i` of `n`.
    fn lane(self, i: usize, n: usize) -> u32 {
        match self {
            MaskPat::First => {
                if i == 0 {
                    !0
                } else {
                    0
                }
            }
            MaskPat::Last => {
                if i + 1 == n {
                    !0
                } else {
                    0
                }
            }
            MaskPat::Even => {
                if i.is_multiple_of(2) {
                    !0
                } else {
                    0
                }
            }
            MaskPat::Sign => 0x8000_0000,
            MaskPat::Low => 1,
        }
    }
}

/// The `r, g, b, a` registers the control-flow stages start from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Regs {
    /// Random bits.
    Bits,
    /// All lanes on in every register.
    On,
    /// All lanes off.
    Off,
    /// `r, g, b` on, `a` with a lane pattern.
    A(MaskPat),
    /// Random masks (0, all ones, sign bit, 1, random).
    Mix,
}

impl Regs {
    fn name(self) -> String {
        match self {
            Regs::Bits => "regs_bits".into(),
            Regs::On => "regs_on".into(),
            Regs::Off => "regs_off".into(),
            Regs::A(p) => format!("regs_a_{}", p.name()),
            Regs::Mix => "regs_mix".into(),
        }
    }

    fn bytes(self, seed: u32) -> Vec<u8> {
        let mut rng = Rng::new((seed.wrapping_mul(0x9e37_79b1) ^ 0x5555) | 1);
        let words = REG_BYTES / 4;
        let w: Vec<u32> = match self {
            Regs::Bits => {
                return crate::cases::register_input(Inputs::Bits, seed);
            }
            Regs::On => vec![!0; words],
            Regs::Off => vec![0; words],
            Regs::Mix => (0..words)
                .map(|_| match rng.below(5) {
                    0 => 0,
                    1 => !0,
                    2 => 0x8000_0000,
                    3 => 1,
                    _ => rng.next_u32(),
                })
                .collect(),
            Regs::A(pat) => {
                let mut w = vec![!0u32; words];
                // `a` is words `3N..4N` for every N; the ranges do not overlap.
                for n in [1usize, 4, 8, 16] {
                    for i in 0..n {
                        w[3 * n + i] = pat.lane(i, n);
                    }
                }
                w
            }
        };
        w.iter().flat_map(|x| x.to_le_bytes()).collect()
    }
}

/// One `SkSL` case being described.
struct Spec {
    name: String,
    nslots: u32,
    stages: Vec<StageSpec>,
    data: Data,
    overrides: Vec<(u32, Data)>,
    regs: Regs,
    rects: Vec<Rect>,
}

impl Spec {
    fn new(name: impl Into<String>, nslots: u32, stages: Vec<StageSpec>, data: Data) -> Spec {
        Spec {
            name: name.into(),
            nslots,
            stages,
            data,
            overrides: Vec::new(),
            regs: Regs::Bits,
            rects: vec![Rect::new(0, 0, 1, 1)],
        }
    }

    fn over(mut self, slot: u32, d: Data) -> Spec {
        self.overrides.push((slot, d));
        self
    }

    fn regs(mut self, r: Regs) -> Spec {
        self.regs = r;
        self
    }

    fn width(mut self, w: usize) -> Spec {
        self.rects = vec![Rect::new(0, 0, w, 1)];
        self
    }

    fn rects(mut self, r: Vec<Rect>) -> Spec {
        self.rects = r;
        self
    }

    fn emit(self, c: &mut Cases) {
        let (s1, s2) = (c.next_seed(), c.next_seed());
        let mut stages = vec![
            StageSpec::with(Op::SetBasePointer, Ctx::Ptr { slot: 0, offset: 0 }),
            StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 1, offset: 0 }),
        ];
        stages.extend(self.stages);
        stages.push(StageSpec::with(
            Op::StoreSrc,
            Ctx::Ptr { slot: 2, offset: 0 },
        ));
        c.push(Case {
            name: self.name,
            force_highp: true,
            compiled: false,
            buffers: vec![
                Buffer::new(slot_buffer(self.nslots, self.data, &self.overrides, s1)),
                Buffer::new(self.regs.bytes(s2)),
                output(REG_BYTES),
            ],
            stages,
            runs: self.rects,
        });
    }
}

/// `SkslPtr` into buffer 0.
fn ptr(slot: u32) -> Ctx {
    Ctx::SkslPtr { buf: 0, slot }
}

fn ptr_stage(name: &str, slot: u32) -> StageSpec {
    StageSpec::with(op(name), ptr(slot))
}

/// `<op>/<variant>/<data>/w1`, with a `r5` segment after the op when `r5`.
fn case_name(op_name: &str, variant: &str, r5: bool, data: Data) -> String {
    let r5 = if r5 { "/r5" } else { "" };
    let variant = if variant.is_empty() {
        String::new()
    } else {
        format!("/{variant}")
    };
    format!("{op_name}{r5}{variant}/{}/w1", data.name())
}

/// Scalar-proxy differences (R5) by family: whether this data reaches them. Measured: with
/// the `/r5/` segments ignored, exactly these cases (and the tile/clamp ones of
/// [`crate::geometry`]) differ on `Scalar`; `fminf`/`fmaxf` with `±0` in the `SkSL` min/max
/// ops, for instance, agree.
fn is_r5(family: &str, d: Data) -> bool {
    match family {
        // Float → int conversion out of range or NaN: wasm saturates (NaN → 0), x86 gives
        // `0x80000000`.
        "to_int" => !matches!(d, Data::Unit | Data::Signed | Data::Range | Data::URange),
        // Float → uint: also negative values (wasm: 0; x64: the low half of the 64-bit result).
        "to_uint" => !matches!(d, Data::Unit | Data::URange),
        // `floorf`/`ceilf` of a signaling NaN: whether the result is quieted depends on the
        // host's libm (the Scalar tier calls it; x86 `roundps` always quiets).
        "libm" => matches!(d, Data::Nan(_)),
        _ => false,
    }
}

// ----------------------------------------------------------------------------------------------
// Families
// ----------------------------------------------------------------------------------------------

/// Element type of the arithmetic ops.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ty {
    Float,
    Int,
    Uint,
}

impl Ty {
    fn one(self) -> &'static str {
        match self {
            Ty::Float => "float",
            Ty::Int => "int",
            Ty::Uint => "uint",
        }
    }

    fn many(self) -> &'static str {
        match self {
            Ty::Float => "floats",
            Ty::Int => "ints",
            Ty::Uint => "uints",
        }
    }
}

const FLOAT_ONE: &[Data] = &[
    Data::Special(0),
    Data::Special(1),
    Data::Nan(0),
    Data::Nan(1),
    Data::Nan(4),
    Data::Unit,
    Data::Signed,
    Data::Bits,
];
const FLOAT_FEW: &[Data] = &[Data::Special(0), Data::Bits];
const FLOAT_N1: &[Data] = &[Data::Special(1), Data::Nan(1), Data::Unit];
const INT_ONE: &[Data] = &[Data::Ints(0), Data::Ints(1), Data::Bits, Data::Small];
const INT_FEW: &[Data] = &[Data::Ints(2), Data::Bits];
const UINT_ONE: &[Data] = &[Data::Uints(0), Data::Uints(1), Data::Bits, Data::Small];
const UINT_FEW: &[Data] = &[Data::Uints(2), Data::Bits];

/// Binary ops `<base>_<one>`, `<base>_{2,3,4}_<many>` (a `MemPtr` to `dst`, `src` follows) and
/// `<base>_n_<many>` (a `BinaryOpCtx`).
fn binary(c: &mut Cases, base: &str, ty: Ty, family: &str) {
    let (one, few, n1) = match ty {
        Ty::Float => (FLOAT_ONE, FLOAT_FEW, FLOAT_N1),
        Ty::Int => (INT_ONE, INT_FEW, INT_FEW),
        Ty::Uint => (UINT_ONE, UINT_FEW, UINT_FEW),
    };
    // Integer division gets more pairings of dividends and divisors.
    let one: Vec<Data> = if base == "div" && ty != Ty::Float {
        if ty == Ty::Int {
            (0..8)
                .map(Data::Ints)
                .chain((0..16).map(Data::IntPair))
                .collect()
        } else {
            (0..8)
                .map(Data::Uints)
                .chain((0..12).map(Data::UintPair))
                .collect()
        }
    } else {
        one.to_vec()
    };
    let name1 = format!("{base}_{}", ty.one());
    for &d in &one {
        Spec::new(
            case_name(&name1, "", is_r5(family, d), d),
            2,
            vec![ptr_stage(&name1, 0)],
            d,
        )
        .emit(c);
    }
    for k in 2..=4u32 {
        let name = format!("{base}_{k}_{}", ty.many());
        for &d in few {
            Spec::new(
                case_name(&name, "", is_r5(family, d), d),
                2 * k,
                vec![ptr_stage(&name, 0)],
                d,
            )
            .emit(c);
        }
    }
    let name = format!("{base}_n_{}", ty.many());
    for (n, datas) in [(1u32, n1), (3, few), (5, few)] {
        for &d in datas {
            Spec::new(
                case_name(&name, &format!("n{n}"), is_r5(family, d), d),
                1 + 2 * n,
                vec![StageSpec::with(
                    op(&name),
                    Ctx::SkslBinary { dst: 1, src: 1 + n },
                )],
                d,
            )
            .emit(c);
        }
    }
}

/// `<base>_imm_<ty>` ops (`ConstantCtx`, destination slot 1) with each value; `datas` rotate
/// across the values.
fn immediate(c: &mut Cases, name: &str, values: &[i32], datas: &[Data], family: &str) {
    for (i, &value) in values.iter().enumerate() {
        let d = datas[i % datas.len()];
        // A NaN immediate must not meet a NaN in the data.
        let d = if f32::from_bits(value as u32).is_nan() && matches!(d, Data::Nan(_)) {
            Data::Unit
        } else {
            d
        };
        Spec::new(
            case_name(name, &format!("v{i}"), is_r5(family, d), d),
            6,
            vec![StageSpec::with(
                op(name),
                Ctx::SkslConstant { value, dst: 1 },
            )],
            d,
        )
        .emit(c);
    }
}

fn f(v: f32) -> i32 {
    v.to_bits() as i32
}

const FLOAT_IMMS: &[f32] = &[
    1.0,
    -0.5,
    0.0,
    -0.0,
    255.5,
    f32::INFINITY,
    f32::from_bits(1),
    3.0e38,
    f32::from_bits(0x7fc1_2345),
];

const INT_IMMS: &[i32] = &[0, 1, -1, 7, i32::MAX, i32::MIN, 0x5555_5555, 255];

const UINT_IMMS: &[i32] = &[0, 1, i32::MIN, -1, 7, 0x7fff_ffff];

/// The `1..=4`-slot unary ops `<prefix>_<one>` and `<prefix>_{2,3,4}_<many>`.
fn unary(c: &mut Cases, prefix: &str, one: &str, many: &str, datas: &[Data], family: &str) {
    let name1 = format!("{prefix}_{one}");
    for &d in datas {
        Spec::new(
            case_name(&name1, "", is_r5(family, d), d),
            1,
            vec![ptr_stage(&name1, 0)],
            d,
        )
        .emit(c);
    }
    for k in 2..=4u32 {
        let name = format!("{prefix}_{k}_{many}");
        for &d in datas.iter().take(3) {
            Spec::new(
                case_name(&name, "", is_r5(family, d), d),
                k,
                vec![ptr_stage(&name, 0)],
                d,
            )
            .emit(c);
        }
    }
}

/// A one-slot math op (`sin_float`, ...).
fn math1(c: &mut Cases, name: &str, datas: &[Data], family: &str) {
    for &d in datas {
        Spec::new(
            case_name(name, "", is_r5(family, d), d),
            1,
            vec![ptr_stage(name, 0)],
            d,
        )
        .emit(c);
    }
}

/// All B6 cases.
pub fn b6_sksl(c: &mut Cases) {
    b6a_masks(c);
    b6a_copies(c);
    b6a_swizzles(c);
    b6b_arithmetic(c);
    b6b_casts_and_unary(c);
    b6b_vector(c);
    b6c_math(c);
}

// ----------------------------------------------------------------------------------------------
// B6a: masks, control flow, copies, swizzles
// ----------------------------------------------------------------------------------------------

const REGS_MASKS: &[Regs] = &[
    Regs::On,
    Regs::Off,
    Regs::A(MaskPat::First),
    Regs::A(MaskPat::Last),
    Regs::A(MaskPat::Even),
    Regs::A(MaskPat::Sign),
    Regs::A(MaskPat::Low),
    Regs::Mix,
    Regs::Bits,
];

fn b6a_masks(c: &mut Cases) {
    // init_lane_masks: the tail decides the lanes (all widths around every stride).
    for &w in TAIL_WIDTHS {
        Spec::new(
            format!("init_lane_masks/w{w}"),
            1,
            vec![StageSpec::new(Op::InitLaneMasks)],
            Data::Bits,
        )
        .width(w)
        .emit(c);
    }
    // The mask stages: load/store/merge a mask slot, from every register state.
    for name in [
        "load_condition_mask",
        "store_condition_mask",
        "merge_condition_mask",
        "merge_inv_condition_mask",
        "load_loop_mask",
        "store_loop_mask",
        "reenable_loop_mask",
        "merge_loop_mask",
        "continue_op",
        "load_return_mask",
        "store_return_mask",
    ] {
        for &regs in REGS_MASKS {
            for data in [Data::Mask(0), Data::Bits] {
                Spec::new(
                    format!("{name}/{}/{}/w1", regs.name(), data.name()),
                    2,
                    vec![ptr_stage(name, 0)],
                    data,
                )
                .regs(regs)
                .emit(c);
            }
        }
    }
    for name in ["mask_off_loop_mask", "mask_off_return_mask"] {
        for &regs in REGS_MASKS {
            Spec::new(
                format!("{name}/{}/w1", regs.name()),
                1,
                vec![StageSpec::new(op(name))],
                Data::Bits,
            )
            .regs(regs)
            .emit(c);
        }
    }
    // More than one chunk per row (the stages run again on the same slots and registers).
    for &w in &[5usize, 17] {
        for name in ["merge_condition_mask", "store_loop_mask", "continue_op"] {
            Spec::new(
                format!("{name}/chunks/w{w}"),
                2,
                vec![ptr_stage(name, 0)],
                Data::Mask(7),
            )
            .regs(Regs::Mix)
            .width(w)
            .emit(c);
        }
        Spec::new(
            format!("copy_4_slots_masked/chunks/w{w}"),
            10,
            vec![StageSpec::with(
                Op::Copy4SlotsMasked,
                Ctx::SkslBinary { dst: 1, src: 5 },
            )],
            Data::Bits,
        )
        .regs(Regs::A(MaskPat::Even))
        .width(w)
        .emit(c);
    }
    // case_op: {actual value, default mask} at slot 0; matches in some lanes.
    for expected in [0, 1, -1, 7] {
        for &regs in &[Regs::On, Regs::A(MaskPat::Even), Regs::Mix] {
            for data in [Data::Small, Data::Ints(0), Data::Mask(1)] {
                Spec::new(
                    format!("case_op/e{expected}/{}/{}/w1", regs.name(), data.name()),
                    2,
                    vec![StageSpec::with(
                        Op::CaseOp,
                        Ctx::SkslCase { expected, slot: 0 },
                    )],
                    data,
                )
                .regs(regs)
                .emit(c);
            }
        }
    }
    // Real sequences: the masks of a function, a branch on them and a store that the branch may
    // skip, at every width around every stride (branch_if_all_lanes_active looks at the tail).
    for branch in [
        Op::BranchIfAllLanesActive,
        Op::BranchIfAnyLanesActive,
        Op::BranchIfNoLanesActive,
    ] {
        for (mi, merge) in ["merge_condition_mask", "load_condition_mask"]
            .into_iter()
            .enumerate()
        {
            for (di, data) in [Data::Mask(2), Data::Mask(3), Data::Mask(4)]
                .into_iter()
                .enumerate()
            {
                for &w in TAIL_WIDTHS {
                    Spec::new(
                        format!("seq_{}/{}/m{di}/w{w}", branch.name(), merge),
                        8,
                        vec![
                            StageSpec::new(Op::InitLaneMasks),
                            ptr_stage(merge, 0),
                            StageSpec::with(branch, Ctx::Branch { offset: 2 }),
                            StageSpec::with(
                                Op::Splat4Constants,
                                Ctx::SkslConstant {
                                    value: 0x1234_5678,
                                    dst: 2,
                                },
                            ),
                            ptr_stage("store_condition_mask", 0),
                        ],
                        data,
                    )
                    .width(w)
                    .emit(c);
                    let _ = mi;
                }
            }
        }
    }
    // A loop: break (mask_off_loop_mask), continue, reenable and merge, with the execution mask
    // recomputed after each.
    for &w in &[1, 5, 17] {
        for data in [Data::Mask(5), Data::Mask(6)] {
            Spec::new(
                format!("seq_loop/{}/w{w}", data.name()),
                4,
                vec![
                    StageSpec::new(Op::InitLaneMasks),
                    ptr_stage("merge_condition_mask", 0),
                    ptr_stage("store_loop_mask", 2),
                    StageSpec::new(Op::MaskOffLoopMask),
                    ptr_stage("continue_op", 3),
                    ptr_stage("reenable_loop_mask", 0),
                    ptr_stage("merge_loop_mask", 1),
                    StageSpec::new(Op::MaskOffReturnMask),
                    ptr_stage("store_return_mask", 3),
                ],
                data,
            )
            .width(w)
            .emit(c);
        }
    }
}

fn b6a_copies(c: &mut Cases) {
    // Constants: copy_constant and splat_N_constants (ConstantCtx).
    for (name, slots) in [
        ("copy_constant", 1u32),
        ("splat_2_constants", 2),
        ("splat_3_constants", 3),
        ("splat_4_constants", 4),
    ] {
        for (i, value) in [0, 1, -1, f(1.0), f(f32::NAN), i32::MIN, 0x1234_5678]
            .into_iter()
            .enumerate()
        {
            Spec::new(
                format!("{name}/v{i}/w1"),
                slots + 2,
                vec![StageSpec::with(
                    op(name),
                    Ctx::SkslConstant { value, dst: 1 },
                )],
                Data::Bits,
            )
            .emit(c);
        }
    }
    // Uniforms: copy_N_uniforms (UniformCtx).
    for (n, name) in [
        (1usize, "copy_uniform"),
        (2, "copy_2_uniforms"),
        (3, "copy_3_uniforms"),
        (4, "copy_4_uniforms"),
    ] {
        for set in 0..2 {
            let values: Vec<i32> = (0..n)
                .map(|i| match set {
                    0 => [f(1.5), f(-2.25), f(1.0e10), f(0.0)][i],
                    _ => [i32::MIN, -1, 0x7fff_ffff, 12345][i],
                })
                .collect();
            Spec::new(
                format!("{name}/set{set}/w1"),
                n as u32 + 2,
                vec![StageSpec::with(
                    op(name),
                    Ctx::SkslUniform {
                        buf: 0,
                        dst: 1,
                        values,
                    },
                )],
                Data::Bits,
            )
            .emit(c);
        }
    }
    // Unmasked slot-to-slot copies, and immutables (scalars broadcast into slots).
    for k in 1..=4u32 {
        let slots = if k == 1 {
            "copy_slot_unmasked".to_owned()
        } else {
            format!("copy_{k}_slots_unmasked")
        };
        let imm = if k == 1 {
            "copy_immutable_unmasked".to_owned()
        } else {
            format!("copy_{k}_immutables_unmasked")
        };
        for name in [&slots, &imm] {
            for data in [Data::Bits, Data::Special(0)] {
                Spec::new(
                    format!("{name}/{}/w1", data.name()),
                    2 * k + 2,
                    vec![StageSpec::with(
                        op(name),
                        Ctx::SkslBinary { dst: 1, src: 1 + k },
                    )],
                    data,
                )
                .emit(c);
            }
        }
    }
    // Masked copies.
    for k in 1..=4u32 {
        let name = if k == 1 {
            "copy_slot_masked".to_owned()
        } else {
            format!("copy_{k}_slots_masked")
        };
        for &regs in REGS_MASKS {
            Spec::new(
                format!("{name}/{}/w1", regs.name()),
                2 * k + 2,
                vec![StageSpec::with(
                    op(&name),
                    Ctx::SkslBinary { dst: 1, src: 1 + k },
                )],
                Data::Bits,
            )
            .regs(regs)
            .emit(c);
        }
    }
    // Swizzled masked copies: src slots 0..k scatter to dst slots (component indices).
    let combos: [[u16; 4]; 4] = [[0, 1, 2, 3], [3, 2, 1, 0], [1, 1, 0, 2], [2, 0, 3, 3]];
    for k in 1..=4u32 {
        let name = if k == 1 {
            "swizzle_copy_slot_masked".to_owned()
        } else {
            format!("swizzle_copy_{k}_slots_masked")
        };
        for (ci, comps) in combos.iter().enumerate() {
            for &regs in &[Regs::On, Regs::A(MaskPat::Even), Regs::Mix] {
                Spec::new(
                    format!("{name}/c{ci}/{}/w1", regs.name()),
                    8,
                    vec![StageSpec::with(
                        op(&name),
                        Ctx::SkslSwizzleCopy {
                            buf: 0,
                            dst: 0,
                            src: 4,
                            comps: *comps,
                        },
                    )],
                    Data::Bits,
                )
                .regs(regs)
                .emit(c);
            }
        }
    }
    // Indirect copies: per-lane indices into an array of slots. Layout (slots): the array at
    // 0..E (E = limit + 4 + slots: room for the largest index plus the swizzle components), the
    // direct operand at E..E+slots, the indices at E+slots.
    for limit in [0u32, 3, 7] {
        for slots in [1u32, 2, 4] {
            let e = limit + 4 + slots;
            let indirect = e + slots;
            let nslots = indirect + 1;
            let ctx = |dst: u32, src: u32, uniform: Vec<i32>| Ctx::SkslIndirect {
                buf: 0,
                dst,
                src,
                indirect,
                limit,
                slots,
                comps: [3, 1, 2, 0],
                uniform,
            };
            let variants: [(&str, Ctx, &[Regs]); 3] = [
                (
                    "copy_from_indirect_unmasked",
                    ctx(e, 0, Vec::new()),
                    &[Regs::On],
                ),
                (
                    "copy_to_indirect_masked",
                    ctx(0, e, Vec::new()),
                    &[Regs::On, Regs::A(MaskPat::Even), Regs::Mix],
                ),
                (
                    "swizzle_copy_to_indirect_masked",
                    ctx(0, e, Vec::new()),
                    &[Regs::On, Regs::A(MaskPat::Even), Regs::Mix],
                ),
            ];
            for (name, ctx, regs_list) in variants {
                for &regs in regs_list {
                    Spec::new(
                        format!("{name}/l{limit}/s{slots}/{}/w1", regs.name()),
                        nslots,
                        vec![StageSpec::with(op(name), ctx.clone())],
                        Data::Bits,
                    )
                    .over(indirect, Data::Index(limit))
                    .regs(regs)
                    .emit(c);
                }
            }
            // From a uniform array.
            let uniform: Vec<i32> = (0..(limit + slots + 1) as i32)
                .map(|i| match i % 3 {
                    0 => f(1.5) + i,
                    1 => -i * 1000,
                    _ => 0x1234_0000 + i,
                })
                .collect();
            Spec::new(
                format!("copy_from_indirect_uniform_unmasked/l{limit}/s{slots}/w1"),
                nslots,
                vec![StageSpec::with(
                    Op::CopyFromIndirectUniformUnmasked,
                    ctx(e, 0, uniform),
                )],
                Data::Bits,
            )
            .over(indirect, Data::Index(limit))
            .emit(c);
        }
    }
    // exchange_src: swaps r, g, b, a with four slots.
    for data in [Data::Bits, Data::Special(0), Data::Nan(2)] {
        Spec::new(
            format!("exchange_src/{}/w1", data.name()),
            4,
            vec![ptr_stage("exchange_src", 0)],
            data,
        )
        .emit(c);
    }
    // store_device_xy01: dx + iota, dy + 0.5, 0, 1 at several origins.
    let rects = [
        Rect::new(0, 0, 1, 1),
        Rect::new(3, 7, 5, 1),
        Rect::new(0, 0, 19, 2),
        Rect::new(1_000_003, 5, 9, 1),
        Rect::new(16_777_213, 16_777_217, 6, 1),
        Rect::new(2_147_483_000, 2_147_483_646, 17, 1),
    ];
    for r in rects {
        Spec::new(
            format!("store_device_xy01/x{}_y{}_w{}_h{}", r.x, r.y, r.w, r.h),
            4,
            vec![ptr_stage("store_device_xy01", 0)],
            Data::Bits,
        )
        .rects(vec![r])
        .emit(c);
    }
}

fn b6a_swizzles(c: &mut Cases) {
    // swizzle_N: the first N slots (from dst) are replaced by slots picked by component.
    let combos: [[u8; 4]; 5] = [
        [0, 1, 2, 3],
        [3, 2, 1, 0],
        [0, 0, 0, 0],
        [1, 2, 3, 0],
        [2, 2, 1, 3],
    ];
    for n in 1..=4u32 {
        let name = format!("swizzle_{n}");
        for (ci, comps) in combos.iter().enumerate() {
            for data in [Data::Bits, Data::Special(0)] {
                Spec::new(
                    format!("{name}/c{ci}/{}/w1", data.name()),
                    4,
                    vec![StageSpec::with(
                        op(&name),
                        Ctx::SkslSwizzle {
                            dst: 0,
                            comps: *comps,
                        },
                    )],
                    data,
                )
                .emit(c);
            }
        }
    }
    // shuffle: `count` slots of 16 picked by up to 16 component indices.
    let identity: Vec<u16> = (0..16).collect();
    let reverse: Vec<u16> = (0..16).rev().collect();
    let strided: Vec<u16> = (0..16).map(|i| (i * 5 + 3) % 16).collect();
    let dup: Vec<u16> = (0..16).map(|i| i / 2).collect();
    for (pi, comps) in [identity, reverse, strided, dup].into_iter().enumerate() {
        for count in [1, 4, 9, 16] {
            Spec::new(
                format!("shuffle/p{pi}/n{count}/w1"),
                16,
                vec![StageSpec::with(
                    Op::Shuffle,
                    Ctx::SkslShuffle {
                        buf: 0,
                        slot: 0,
                        count,
                        comps: comps.clone(),
                    },
                )],
                Data::Bits,
            )
            .emit(c);
        }
    }
}

// ----------------------------------------------------------------------------------------------
// B6b: arithmetic
// ----------------------------------------------------------------------------------------------

fn b6b_arithmetic(c: &mut Cases) {
    for base in [
        "add", "sub", "mul", "div", "mod", "min", "max", "cmplt", "cmple", "cmpeq", "cmpne",
    ] {
        let family = match base {
            "min" => "min_float",
            "max" => "max_float",
            _ => "arith",
        };
        binary(c, base, Ty::Float, family);
    }
    for base in [
        "add",
        "sub",
        "mul",
        "div",
        "min",
        "max",
        "cmplt",
        "cmple",
        "cmpeq",
        "cmpne",
        "bitwise_and",
        "bitwise_or",
        "bitwise_xor",
    ] {
        binary(c, base, Ty::Int, "int");
    }
    for base in ["div", "min", "max", "cmplt", "cmple"] {
        binary(c, base, Ty::Uint, "uint");
    }
    // Immediates.
    let floats: Vec<i32> = FLOAT_IMMS.iter().map(|&v| f(v)).collect();
    for base in [
        "add", "mul", "max", "min", "cmplt", "cmple", "cmpeq", "cmpne",
    ] {
        let family = match base {
            "min" => "min_float",
            "max" => "max_float",
            _ => "arith",
        };
        immediate(
            c,
            &format!("{base}_imm_float"),
            &floats,
            &[Data::Special(0), Data::Unit, Data::Nan(1), Data::Signed],
            family,
        );
    }
    for base in [
        "add",
        "mul",
        "bitwise_xor",
        "cmplt",
        "cmple",
        "cmpeq",
        "cmpne",
    ] {
        immediate(
            c,
            &format!("{base}_imm_int"),
            INT_IMMS,
            &[Data::Ints(0), Data::Bits, Data::Small],
            "int",
        );
    }
    for base in ["cmplt", "cmple"] {
        immediate(
            c,
            &format!("{base}_imm_uint"),
            UINT_IMMS,
            &[Data::Uints(0), Data::Bits],
            "uint",
        );
    }
    for name in [
        "bitwise_and_imm_int",
        "bitwise_and_imm_2_ints",
        "bitwise_and_imm_3_ints",
        "bitwise_and_imm_4_ints",
    ] {
        immediate(c, name, INT_IMMS, &[Data::Ints(1), Data::Bits], "int");
    }
}

fn b6b_casts_and_unary(c: &mut Cases) {
    let float_unary = [
        Data::Special(0),
        Data::Special(1),
        Data::Nan(0),
        Data::Unit,
        Data::Signed,
        Data::Wide,
        Data::Bits,
    ];
    unary(c, "floor", "float", "floats", &float_unary, "libm");
    unary(c, "ceil", "float", "floats", &float_unary, "libm");
    unary(
        c,
        "cast_to_int_from",
        "float",
        "floats",
        &[
            Data::Range,
            Data::Special(0),
            Data::Special(1),
            Data::Nan(0),
            Data::Signed,
            Data::Bits,
        ],
        "to_int",
    );
    unary(
        c,
        "cast_to_uint_from",
        "float",
        "floats",
        &[
            Data::URange,
            Data::Range,
            Data::Special(0),
            Data::Special(1),
            Data::Nan(0),
            Data::Unit,
            Data::Bits,
        ],
        "to_uint",
    );
    unary(
        c,
        "cast_to_float_from",
        "int",
        "ints",
        &[Data::Ints(0), Data::Ints(3), Data::Small, Data::Bits],
        "int",
    );
    unary(
        c,
        "cast_to_float_from",
        "uint",
        "uints",
        &[Data::Uints(0), Data::Uints(3), Data::Small, Data::Bits],
        "uint",
    );
    unary(
        c,
        "abs",
        "int",
        "ints",
        &[Data::Ints(0), Data::Ints(3), Data::Small, Data::Bits],
        "int",
    );
}

fn b6b_vector(c: &mut Cases) {
    // dot_N_floats: 2N slots.
    let datas = [
        Data::Special(0),
        Data::Special(1),
        Data::Nan(0),
        Data::Nan(1),
        Data::Unit,
        Data::Signed,
        Data::Bits,
    ];
    for k in 2..=4u32 {
        let name = format!("dot_{k}_floats");
        for &d in &datas {
            Spec::new(
                case_name(&name, "", false, d),
                2 * k,
                vec![ptr_stage(&name, 0)],
                d,
            )
            .emit(c);
        }
    }
    // matrix_multiply_N: result (rc x lr), left (N x lr), right (rc x N).
    let pairs = [
        (1u8, 1u8),
        (1, 4),
        (4, 1),
        (2, 2),
        (3, 3),
        (4, 4),
        (2, 3),
        (3, 2),
    ];
    for n in 2..=4u8 {
        let name = format!("matrix_multiply_{n}");
        for &(lr, rc) in &pairs {
            let slots = u32::from(rc) * u32::from(lr)
                + u32::from(n) * u32::from(lr)
                + u32::from(rc) * u32::from(n);
            for d in [
                Data::Unit,
                Data::Signed,
                Data::Special(0),
                Data::Nan(2),
                Data::Bits,
            ] {
                Spec::new(
                    format!("{name}/l{n}x{lr}_r{rc}x{n}/{}/w1", d.name()),
                    slots,
                    vec![StageSpec::with(
                        op(&name),
                        Ctx::SkslMatmul {
                            dst: 0,
                            dims: [n, lr, rc, n],
                        },
                    )],
                    d,
                )
                .emit(c);
            }
        }
    }
    // refract_4_floats: incident (4), normal (4), eta (1).
    for d in [
        Data::Special(0),
        Data::Special(1),
        Data::Nan(0),
        Data::Unit,
        Data::Signed,
        Data::Tame,
        Data::Bits,
    ] {
        Spec::new(
            case_name("refract_4_floats", "", false, d),
            9,
            vec![ptr_stage("refract_4_floats", 0)],
            d,
        )
        .emit(c);
    }
    // The Scalar tier evaluates refract's `k` and smoothstep's polynomial in double precision
    // (C++ double literals); one lane per case there, so a few more seeds of ordinary values.
    for i in 0..12 {
        for d in [Data::Unit, Data::Signed] {
            Spec::new(
                format!("refract_4_floats/more{i}/{}/w1", d.name()),
                9,
                vec![ptr_stage("refract_4_floats", 0)],
                d,
            )
            .emit(c);
            Spec::new(
                format!("smoothstep_n_floats/more{i}/{}/w1", d.name()),
                3,
                vec![StageSpec::with(
                    Op::SmoothstepNFloats,
                    Ctx::SkslTernary { dst: 0, delta: 1 },
                )],
                d,
            )
            .emit(c);
        }
    }
    // smoothstep_n_floats: edge0, edge1, x with `n` slots each (TernaryOpCtx).
    for n in [1u32, 3] {
        for d in [
            Data::Special(0),
            Data::Nan(1),
            Data::Unit,
            Data::Signed,
            Data::Wide,
            Data::Bits,
        ] {
            Spec::new(
                case_name("smoothstep_n_floats", &format!("n{n}"), false, d),
                3 * n,
                vec![StageSpec::with(
                    Op::SmoothstepNFloats,
                    Ctx::SkslTernary { dst: 0, delta: n },
                )],
                d,
            )
            .emit(c);
        }
    }
    // mix: floats and ints, 1-4 slots (MemPtr) and n-way (TernaryOpCtx).
    for (ty, datas) in [
        (
            Ty::Float,
            &[Data::Special(0), Data::Nan(1), Data::Unit, Data::Bits][..],
        ),
        (Ty::Int, &[Data::Ints(0), Data::Mask(0), Data::Bits][..]),
    ] {
        let name1 = format!("mix_{}", ty.one());
        for &d in datas {
            Spec::new(
                case_name(&name1, "", false, d),
                3,
                vec![ptr_stage(&name1, 0)],
                d,
            )
            .emit(c);
        }
        for k in 2..=4u32 {
            let name = format!("mix_{k}_{}", ty.many());
            for &d in datas.iter().take(2) {
                Spec::new(
                    case_name(&name, "", false, d),
                    3 * k,
                    vec![ptr_stage(&name, 0)],
                    d,
                )
                .emit(c);
            }
        }
        let name = format!("mix_n_{}", ty.many());
        for n in [1u32, 3] {
            for &d in datas {
                Spec::new(
                    case_name(&name, &format!("n{n}"), false, d),
                    3 * n,
                    vec![StageSpec::with(
                        op(&name),
                        Ctx::SkslTernary { dst: 0, delta: n },
                    )],
                    d,
                )
                .emit(c);
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------
// B6c: math
// ----------------------------------------------------------------------------------------------

fn b6c_math(c: &mut Cases) {
    let general = [
        Data::Special(0),
        Data::Special(1),
        Data::Nan(0),
        Data::Unit,
        Data::Signed,
        Data::Wide,
        Data::Bits,
    ];
    for name in [
        "sin_float",
        "cos_float",
        "tan_float",
        "atan_float",
        "sqrt_float",
    ] {
        math1(c, name, &general, "math");
    }
    // asin/acos negate `nmad(sqrt(1 - x), poly, pi/2)`: outside [-1, 1] (and for a NaN input)
    // the result is a NaN whose *sign* depends on whether the compiler folds the negation into
    // the fused multiply-add (rustc does where Skia's clang does not): like two NaNs meeting in
    // a fused mad, not specified by Skia's source. So only the domain.
    for name in ["asin_float", "acos_float"] {
        math1(c, name, &[Data::Unit, Data::Signed, Data::Sparse], "math");
    }
    for name in ["exp_float", "exp2_float", "log_float", "log2_float"] {
        math1(c, name, &general, "approx");
    }
    // invsqrt: rsqrt estimates (tier-dependent), 1-4 slots.
    unary(c, "invsqrt", "float", "floats", &general, "arith");
    // atan2, pow: n-way binary.
    for name in ["atan2_n_floats", "pow_n_floats"] {
        let family = if name.starts_with("pow") {
            "approx"
        } else {
            "math"
        };
        for n in [1u32, 3] {
            let datas: &[Data] = if n == 1 { FLOAT_ONE } else { FLOAT_FEW };
            for &d in datas {
                Spec::new(
                    case_name(name, &format!("n{n}"), is_r5(family, d), d),
                    1 + 2 * n,
                    vec![StageSpec::with(
                        op(name),
                        Ctx::SkslBinary { dst: 1, src: 1 + n },
                    )],
                    d,
                )
                .emit(c);
            }
        }
    }
    // inverse_mat2/3/4: rcp_precise (estimates, fused ops). A NaN (from an infinity, or a zero
    // determinant's `inf * 0`) multiplies the matrix elements, one of which may itself be the
    // NaN: two NaNs meet in a `mulps` or a fused mad, so no infinities and no NaN input; the
    // singular matrices (`Sparse`) cover the zero determinant (`inf * 0` is one NaN).
    for (name, slots) in [
        ("inverse_mat2", 4u32),
        ("inverse_mat3", 9),
        ("inverse_mat4", 16),
    ] {
        for d in [
            Data::Unit,
            Data::Signed,
            Data::Tame,
            Data::Sparse,
            Data::Wide,
        ] {
            Spec::new(
                case_name(name, "", false, d),
                slots,
                vec![ptr_stage(name, 0)],
                d,
            )
            .emit(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Every B5/B6a/B6b/B6c op has a case (B6d, the trace ops, and `callback` need a host hook).
    #[test]
    fn every_geometry_and_sksl_op_has_cases() {
        let used: BTreeSet<&str> = crate::cases::all()
            .iter()
            .flat_map(|c| c.stages.iter().map(|s| s.op.name()))
            .collect();
        let missing: Vec<&str> = Op::ALL
            .iter()
            .filter(|o| ["B5", "B6a", "B6b", "B6c"].contains(&o.task()))
            .map(|o| o.name())
            .filter(|n| !used.contains(n))
            .collect();
        assert!(missing.is_empty(), "ops without rp-diff cases: {missing:?}");
    }

    #[test]
    fn slot_data_is_deterministic_and_nan_free_where_promised() {
        for d in [
            Data::Special(0),
            Data::Unit,
            Data::Signed,
            Data::Wide,
            Data::Bits,
        ] {
            let w = data_words(d, 256, 3);
            assert!(w.iter().all(|&x| !f32::from_bits(x).is_nan()), "{d:?}");
            assert_eq!(w, data_words(d, 256, 3));
        }
        let nans = data_words(Data::Nan(5), 256, 3)
            .into_iter()
            .filter(|&x| f32::from_bits(x).is_nan())
            .count();
        assert_eq!(nans, 1);
    }
}
