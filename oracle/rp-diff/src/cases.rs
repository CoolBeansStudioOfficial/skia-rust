// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The case list: [`all`]. Every implemented stage has cases here (design §4.2, §5: each Wave B
//! task adds the cases of its stages).
//!
//! # Adding cases for a new stage
//! 1. Write a generator `fn my_stages(c: &mut Cases)` below and call it from [`all`]. For an op
//!    that only transforms the registers, [`Cases::registers`] is all it takes:
//!    `c.registers("srcatop", &[StageSpec::new(Op::Srcatop)], Precision::BOTH, REGISTER_WIDTHS)`
//!    feeds `load_src`/`load_dst` from special, random and unit-range inputs, runs the stages and
//!    reads every register back with `store_src`/`store_dst`, in lowp and highp, at several widths
//!    (tails). For memory stages (`load_8888`, `store_f16`, gathers, ...) build [`Case`]s
//!    directly with [`Cases::push`], using [`Ctx::Mem`] buffers with a row stride, heights ≥ 2,
//!    widths around every tier's stride, and `compiled` cases with several rects (stale tail
//!    scratch, design §1.7).
//! 2. If a context type is new to rp-diff, implement `FromCtx` for it in `replay.rs` (and a
//!    [`Ctx`] variant plus its C++ in `cpp/driver.cpp` if no existing kind fits).
//! 3. On the oracle host: `cargo xtask oracle rp-diff --update` runs the cases through Skia for
//!    every x86 tier, compares them with skia-rust, and rewrites `expected/<tier>.txt`; commit
//!    those files. `cargo test -p skia-rust-rp-diff` then replays them anywhere.
//!
//! Case names are `<group>/<details>`; `--case-glob` filters on them. Generation is
//! deterministic (a fixed-seed xorshift), so the same list is produced on every host.

use skia_rust_simd::rp::Op;

use crate::case::{Buffer, Case, Ctx, Rect, StageSpec};

/// Bytes of a register buffer: 4 registers × 16 lanes × 4 bytes, enough for `load_src`/
/// `store_src` on every tier (highp 16 × `f32` on Ml4, lowp 16 × `u16`).
pub const REG_BYTES: usize = 256;

/// Byte that output buffers start as (unwritten lanes stay visible).
pub const FILL: u8 = 0xCD;

/// Widths for register-only cases: 1, a short tail, every highp/lowp stride, one past it.
pub const REGISTER_WIDTHS: &[usize] = &[1, 7, 16, 19];

/// Widths around every stride (1, 4, 8, 16): for stages whose result depends on the tail.
pub const TAIL_WIDTHS: &[usize] = &[1, 3, 4, 5, 8, 15, 16, 17];

/// Pipeline precision of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    /// lowp when every stage has a lowp version on the tier, else highp.
    Auto,
    /// highp (`gForceHighPrecisionRasterPipeline`).
    Highp,
}

impl Precision {
    /// Both precisions.
    pub const BOTH: &'static [Precision] = &[Precision::Auto, Precision::Highp];
    /// highp only (for highp-only ops `Auto` is the same pipeline).
    pub const HIGHP: &'static [Precision] = &[Precision::Highp];

    /// `auto` / `highp`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Precision::Auto => "auto",
            Precision::Highp => "highp",
        }
    }
}

/// Register input data, see [`register_input`] and [`Inputs::pair`].
///
/// # NaNs
/// When two NaNs meet in one x86 instruction, the result is the first NaN operand in
/// *encoding* order, which depends on the instruction form the compiler picks (`vfmadd132`/
/// `213`/`231`, commuted `addps` operands), for Skia's clang as for rustc: Skia's result is not
/// determined by its source there. So inputs never let two input NaNs meet: [`Inputs::Special`]
/// and [`Inputs::Bits`] contain no NaN (invalid operations on their infinities and zeros all
/// produce the same indefinite NaN, `0xFFC00000`), and NaN propagation is tested by
/// [`Inputs::NanSrc`]/[`Inputs::NanDst`], which put at most one NaN in each lane of each tier
/// ([`NAN_WORDS`]) and only tame values (finite, nonzero, magnitude in `[0.25, 2)`) beside them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inputs {
    /// Special non-NaN `f32` values (design §4.1: ±0, denormals, `FLT_MIN`, 0.5±ulp, 2.5, 255.5,
    /// 65535.5, ±2³¹, 2³², `FLT_MAX`, ±inf, ...), rotated so every lane position sees each.
    Special,
    /// NaNs (quiet and signalling, both signs, with payloads) in the source registers at
    /// [`NAN_WORDS`], tame values everywhere else and in the destination registers.
    NanSrc,
    /// As [`Inputs::NanSrc`] with the NaNs in the destination registers.
    NanDst,
    /// Random `f32` in `[0, 1]` (24-bit steps, including 0 and 1).
    Unit,
    /// Random bits, except that NaNs are made finite (bit 30 cleared).
    Bits,
    /// Random `u16` in `[0, 255]` (lowp's unorm values).
    Bytes,
}

impl Inputs {
    /// The inputs used for each precision by [`Cases::registers`].
    #[must_use]
    pub const fn for_precision(p: Precision) -> &'static [Inputs] {
        match p {
            Precision::Highp => &[
                Inputs::Special,
                Inputs::NanSrc,
                Inputs::NanDst,
                Inputs::Unit,
                Inputs::Bits,
            ],
            Precision::Auto => &[Inputs::Bytes, Inputs::Bits, Inputs::Unit],
        }
    }

    /// Lower-case name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Inputs::Special => "special",
            Inputs::NanSrc => "nan_src",
            Inputs::NanDst => "nan_dst",
            Inputs::Unit => "unit",
            Inputs::Bits => "bits",
            Inputs::Bytes => "bytes",
        }
    }

    /// Source and destination register buffers (`load_src`, `load_dst`) for one case.
    #[must_use]
    pub fn pair(self, seed_src: u32, seed_dst: u32) -> (Vec<u8>, Vec<u8>) {
        match self {
            Inputs::NanSrc => (nan_input(seed_src), tame_input(seed_dst)),
            Inputs::NanDst => (tame_input(seed_src), nan_input(seed_dst)),
            k => (register_input(k, seed_src), register_input(k, seed_dst)),
        }
    }
}

/// The words of a register buffer that hold NaNs in [`Inputs::NanSrc`]/[`Inputs::NanDst`].
/// Register `k`, lane `i` of a tier with `N` lanes is word `k*N + i`; for every `N` (1, 4, 8,
/// 16) these words fall into distinct lanes, so no lane of any tier holds two of them.
pub const NAN_WORDS: [usize; 16] = [0, 5, 10, 15, 17, 19, 20, 22, 34, 39, 40, 41, 43, 44, 45, 46];

/// The NaNs of [`Inputs::NanSrc`]: quiet and signalling, both signs, with payloads.
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

fn seeded(seed: u32) -> Rng {
    Rng::new(0x9e37_79b9 ^ seed.wrapping_mul(0x85eb_ca6b))
}

/// A finite nonzero `f32` of magnitude in `[0.25, 2)` with a random sign.
fn tame(rng: &mut Rng) -> u32 {
    (rng.next_u32() & 0x8000_0000) | (0x3e80_0000 + rng.below(3 << 23))
}

fn words_to_bytes(words: impl IntoIterator<Item = u32>) -> Vec<u8> {
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Tame values only.
fn tame_input(seed: u32) -> Vec<u8> {
    let mut rng = seeded(seed);
    words_to_bytes((0..REG_BYTES / 4).map(|_| tame(&mut rng)))
}

/// NaNs at [`NAN_WORDS`], tame values elsewhere.
fn nan_input(seed: u32) -> Vec<u8> {
    let mut rng = seeded(seed);
    let mut words: Vec<u32> = (0..REG_BYTES / 4).map(|_| tame(&mut rng)).collect();
    let start = rng.below(8) as usize;
    for (j, &w) in NAN_WORDS.iter().enumerate() {
        words[w] = NANS[(start + j) % NANS.len()];
    }
    words_to_bytes(words)
}

/// xorshift32 (as `oracle/skcms-diff`).
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    /// A generator from a nonzero seed.
    #[must_use]
    pub fn new(seed: u32) -> Rng {
        Rng(seed.max(1))
    }

    /// The next value.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// A value below `n`.
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u32() % n
    }
}

/// The bits of the special non-NaN `f32` values of design §4.1, both signs.
#[must_use]
pub fn float_specials() -> Vec<u32> {
    let positive: [u32; 22] = [
        0x0000_0000, // 0
        0x0000_0001, // smallest denormal
        0x007f_ffff, // largest denormal
        0x0080_0000, // FLT_MIN
        0x3e80_0000, // 0.25
        0x3eff_ffff, // 0.5 - ulp
        0x3f00_0000, // 0.5
        0x3f00_0001, // 0.5 + ulp
        0x3f80_0000, // 1
        0x3fc0_0000, // 1.5
        0x4020_0000, // 2.5
        0x437e_8000, // 254.5
        0x437f_8000, // 255.5
        0x4380_0000, // 256
        0x477f_ff80, // 65535.5
        0x477f_e000, // 65504 (largest half)
        0x4b00_0000, // 2^23
        0x4f00_0000, // 2^31
        0x4eff_ffff, // largest below 2^31
        0x4f80_0000, // 2^32
        0x7f7f_ffff, // FLT_MAX
        0x7f80_0000, // inf
    ];
    positive
        .iter()
        .flat_map(|&b| [b, b | 0x8000_0000])
        .collect()
}

/// `REG_BYTES` bytes of register input of kind `inputs` for one buffer ([`Inputs::NanDst`] gives
/// tame values only; [`Inputs::pair`] gives both buffers); `seed` varies the data.
///
/// # Panics
/// Never (the conversions are of small constants).
#[must_use]
pub fn register_input(inputs: Inputs, seed: u32) -> Vec<u8> {
    let mut rng = seeded(seed);
    let words = REG_BYTES / 4;
    match inputs {
        Inputs::Special => {
            let s = float_specials();
            let n = u32::try_from(s.len()).expect("few specials");
            let start = rng.below(n);
            // Coprime with the count (44): consecutive words step through all specials.
            let step = [5, 7, 9, 13][rng.below(4) as usize];
            words_to_bytes((0..words).map(|j| {
                let j = u32::try_from(j).expect("small");
                s[((start + j * step) % n) as usize]
            }))
        }
        Inputs::NanSrc => nan_input(seed),
        Inputs::NanDst => tame_input(seed),
        Inputs::Unit => words_to_bytes((0..words).map(|j| match j % 29 {
            0 => 0,
            1 => 1.0f32.to_bits(),
            // 24-bit steps in [0, 1]: exact in f32.
            #[allow(clippy::cast_precision_loss)] // < 2^24, exact
            _ => ((rng.next_u32() >> 8) as f32 / 16_777_216.0).to_bits(),
        })),
        Inputs::Bits => words_to_bytes((0..words).map(|_| {
            let w = rng.next_u32();
            if f32::from_bits(w).is_nan() {
                w & !0x4000_0000
            } else {
                w
            }
        })),
        Inputs::Bytes => (0..REG_BYTES / 2)
            .flat_map(|_| {
                u16::try_from(rng.below(256))
                    .expect("below 256")
                    .to_le_bytes()
            })
            .collect(),
    }
}

/// An output buffer of `len` bytes, filled with [`FILL`].
#[must_use]
pub fn output(len: usize) -> Buffer {
    Buffer::new(vec![FILL; len])
}

/// The case list being built.
#[derive(Debug, Default)]
pub struct Cases {
    cases: Vec<Case>,
    seed: u32,
}

impl Cases {
    /// Adds a case.
    ///
    /// # Panics
    /// If the name is already taken.
    pub fn push(&mut self, case: Case) {
        assert!(
            self.cases.iter().all(|c| c.name != case.name),
            "duplicate rp-diff case {}",
            case.name
        );
        self.cases.push(case);
    }

    /// A fresh seed for input data (deterministic: the n-th call returns the same value on every
    /// host).
    pub fn next_seed(&mut self) -> u32 {
        self.seed += 1;
        self.seed
    }

    /// Register-transform cases: for each precision, input kind and width, the pipeline
    /// `load_src(0) load_dst(1) <stages> store_src(2) store_dst(3)` over one row at `(0, 0)`.
    /// Slots 0 and 1 hold inputs ([`register_input`], different data); 2 and 3 start as
    /// [`FILL`]. Named `<group>/<precision>/<inputs>/w<width>`.
    pub fn registers(
        &mut self,
        group: &str,
        stages: &[StageSpec],
        precisions: &[Precision],
        widths: &[usize],
    ) {
        for &p in precisions {
            for &inputs in Inputs::for_precision(p) {
                for &w in widths {
                    let (s1, s2) = (self.next_seed(), self.next_seed());
                    let (src, dst) = inputs.pair(s1, s2);
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
                    self.push(Case {
                        name: format!("{group}/{}/{}/w{w}", p.name(), inputs.name()),
                        force_highp: p == Precision::Highp,
                        compiled: false,
                        buffers: vec![
                            Buffer::new(src),
                            Buffer::new(dst),
                            output(REG_BYTES),
                            output(REG_BYTES),
                        ],
                        stages: all,
                        runs: vec![Rect::new(0, 0, w, 1)],
                    });
                }
            }
        }
    }

    /// The cases, in order.
    #[must_use]
    pub fn finish(self) -> Vec<Case> {
        self.cases
    }
}

/// Every rp-diff case.
#[must_use]
pub fn all() -> Vec<Case> {
    let mut c = Cases::default();
    a3_registers(&mut c);
    a3_store_src_a(&mut c);
    a3_seed_shader(&mut c);
    a3_branches(&mut c);
    a3_stack_and_base_pointer(&mut c);
    // Wave B: call your generators here.
    c.finish()
}

/// A3: `move_src_dst`, `move_dst_src`, `swap_src_dst`, `srcover`; `load_src`, `load_dst`,
/// `store_src`, `store_dst` are in every register case.
fn a3_registers(c: &mut Cases) {
    for op in [Op::MoveSrcDst, Op::MoveDstSrc, Op::SwapSrcDst, Op::Srcover] {
        c.registers(
            op.name(),
            &[StageSpec::new(op)],
            Precision::BOTH,
            REGISTER_WIDTHS,
        );
    }
    // Only the loads and stores.
    c.registers("load_store", &[], Precision::BOTH, REGISTER_WIDTHS);
}

/// A3: `store_src_a` (`load_src(0) store_src_a(1)`).
fn a3_store_src_a(c: &mut Cases) {
    for &p in Precision::BOTH {
        for &inputs in Inputs::for_precision(p)
            .iter()
            .filter(|&&i| i != Inputs::NanDst)
        {
            for &w in REGISTER_WIDTHS {
                let seed = c.next_seed();
                c.push(Case {
                    name: format!("store_src_a/{}/{}/w{w}", p.name(), inputs.name()),
                    force_highp: p == Precision::Highp,
                    compiled: false,
                    buffers: vec![Buffer::new(register_input(inputs, seed)), output(REG_BYTES)],
                    stages: vec![
                        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                        StageSpec::with(Op::StoreSrcA, Ctx::Ptr { slot: 1, offset: 0 }),
                    ],
                    runs: vec![Rect::new(0, 0, w, 1)],
                });
            }
        }
    }
}

/// A3: `seed_shader` (`seed_shader store_src(0)`) at small, large and 2³¹-adjacent
/// coordinates; the last row's last chunk is what `store_src` keeps.
fn a3_seed_shader(c: &mut Cases) {
    let rects = [
        Rect::new(0, 0, 1, 1),
        Rect::new(3, 7, 5, 1),
        Rect::new(0, 0, 19, 2),
        Rect::new(1_000_003, 5, 9, 1),
        Rect::new(16_777_213, 16_777_217, 6, 1),
        Rect::new(2_147_483_000, 2_147_483_646, 17, 1),
    ];
    for &p in Precision::BOTH {
        for r in rects {
            c.push(Case {
                name: format!(
                    "seed_shader/{}/x{}_y{}_w{}_h{}",
                    p.name(),
                    r.x,
                    r.y,
                    r.w,
                    r.h
                ),
                force_highp: p == Precision::Highp,
                compiled: false,
                buffers: vec![output(REG_BYTES)],
                stages: vec![
                    StageSpec::new(Op::SeedShader),
                    StageSpec::with(Op::StoreSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                ],
                runs: vec![r],
            });
        }
        // A compiled pipeline run on several rects.
        c.push(Case {
            name: format!("seed_shader/{}/compiled", p.name()),
            force_highp: p == Precision::Highp,
            compiled: true,
            buffers: vec![output(REG_BYTES)],
            stages: vec![
                StageSpec::new(Op::SeedShader),
                StageSpec::with(Op::StoreSrc, Ctx::Ptr { slot: 0, offset: 0 }),
            ],
            runs: vec![Rect::new(0, 0, 5, 1), Rect::new(7, 3, 3, 2)],
        });
    }
}

/// Int patterns for `branch_if_no_active_lanes_eq`: `(name, value of lane i)`.
type IntPattern = (&'static str, fn(usize) -> i32);

/// Lane-mask patterns for the `a` register of branch cases: `(name, lane value for (lane, N))`.
type MaskPattern = (&'static str, fn(usize, usize, &mut Rng) -> u32);

const MASKS: &[MaskPattern] = &[
    ("on", |_, _, _| !0),
    ("off", |_, _, _| 0),
    // Only the sign bit: x86 SSE tests sign bits (movmskps), others any bit.
    ("sign", |_, _, _| 0x8000_0000),
    // Nonzero without the sign bit.
    ("low", |_, _, _| 1),
    ("first", |i, _, _| if i == 0 { !0 } else { 0 }),
    ("last", |i, n, _| if i + 1 == n { !0 } else { 0 }),
    ("even", |i, _, _| if i % 2 == 0 { !0 } else { 0 }),
    ("mixed", |_, _, rng| match rng.below(5) {
        0 => 0,
        1 => !0,
        2 => 0x8000_0000,
        3 => 1,
        _ => rng.next_u32(),
    }),
];

/// A `load_src` buffer whose `a` register holds `pattern` on every tier: `a` is words
/// `3N..4N` for `N` = 1, 4, 8, 16, which do not overlap; the other words are random.
fn mask_input(pattern: fn(usize, usize, &mut Rng) -> u32, seed: u32) -> Vec<u8> {
    let mut rng = Rng::new(seed.wrapping_mul(0x2545_f491));
    let mut words: Vec<u32> = (0..REG_BYTES / 4).map(|_| rng.next_u32()).collect();
    for n in [1, 4, 8, 16] {
        for i in 0..n {
            words[3 * n + i] = pattern(i, n, &mut rng);
        }
    }
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// A3: `branch_if_all_lanes_active`, `branch_if_any_lanes_active`, `branch_if_no_lanes_active`,
/// `jump`, `branch_if_no_active_lanes_eq`. Program: `load_src(0) load_dst(1) <branch +2>
/// swap_src_dst store_src(2) store_dst(3)`: the swap runs only if the branch is not taken.
fn a3_branches(c: &mut Cases) {
    let branch_case = |c: &mut Cases, name: String, branch: StageSpec, src: Vec<u8>, w, extra| {
        let seed = c.next_seed();
        let mut buffers = vec![
            Buffer::new(src),
            Buffer::new(register_input(Inputs::Bits, seed)),
            output(REG_BYTES),
            output(REG_BYTES),
        ];
        buffers.extend(extra);
        c.push(Case {
            name,
            force_highp: true,
            compiled: false,
            buffers,
            stages: vec![
                StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                StageSpec::with(Op::LoadDst, Ctx::Ptr { slot: 1, offset: 0 }),
                branch,
                StageSpec::new(Op::SwapSrcDst),
                StageSpec::with(Op::StoreSrc, Ctx::Ptr { slot: 2, offset: 0 }),
                StageSpec::with(Op::StoreDst, Ctx::Ptr { slot: 3, offset: 0 }),
            ],
            runs: vec![Rect::new(0, 0, w, 1)],
        });
    };

    for op in [
        Op::BranchIfAllLanesActive,
        Op::BranchIfAnyLanesActive,
        Op::BranchIfNoLanesActive,
    ] {
        for &(mask, pattern) in MASKS {
            for &w in TAIL_WIDTHS {
                let seed = c.next_seed();
                branch_case(
                    c,
                    format!("{}/{mask}/w{w}", op.name()),
                    StageSpec::with(op, Ctx::Branch { offset: 2 }),
                    mask_input(pattern, seed),
                    w,
                    None,
                );
            }
        }
    }
    for offset in [1, 2] {
        for &w in REGISTER_WIDTHS {
            let seed = c.next_seed();
            branch_case(
                c,
                format!("jump/{offset}/w{w}"),
                StageSpec::with(Op::Jump, Ctx::Branch { offset }),
                register_input(Inputs::Bits, seed),
                w,
                None,
            );
        }
    }

    // branch_if_no_active_lanes_eq: the ints (slot 4) against 7 where the mask is on.
    let ints: &[IntPattern] = &[
        ("all7", |_| 7),
        ("none7", |i| i32::try_from(i).expect("small") - 100),
        ("first7", |i| if i == 0 { 7 } else { 0 }),
        ("odd7", |i| if i % 2 == 1 { 7 } else { -7 }),
    ];
    for &(mask, pattern) in MASKS {
        for &(values, value_at) in ints {
            for &w in &[1, 4, 17] {
                let seed = c.next_seed();
                let words: Vec<u32> = (0..16).map(|i| value_at(i).cast_unsigned()).collect();
                branch_case(
                    c,
                    format!("branch_if_no_active_lanes_eq/{mask}/{values}/w{w}"),
                    StageSpec::with(
                        Op::BranchIfNoActiveLanesEq,
                        Ctx::BranchEq {
                            offset: 2,
                            value: 7,
                            slot: 4,
                            byte_offset: 0,
                        },
                    ),
                    mask_input(pattern, seed),
                    w,
                    Some(Buffer::from_u32s(&words)),
                );
            }
        }
    }
}

/// A3: `stack_rewind` (the builder adds `stack_checkpoint` and goes highp) and
/// `set_base_pointer`, around `srcover`.
fn a3_stack_and_base_pointer(c: &mut Cases) {
    c.registers(
        "stack_rewind",
        &[StageSpec::new(Op::StackRewind), StageSpec::new(Op::Srcover)],
        Precision::BOTH,
        &[1, 17],
    );
    for &w in &[1, 17] {
        let (s1, s2) = (c.next_seed(), c.next_seed());
        c.push(Case {
            name: format!("set_base_pointer/w{w}"),
            force_highp: true,
            compiled: false,
            buffers: vec![
                Buffer::new(register_input(Inputs::Unit, s1)),
                Buffer::new(register_input(Inputs::Unit, s2)),
                output(REG_BYTES),
                output(REG_BYTES),
                Buffer::zeroed(64),
            ],
            stages: vec![
                StageSpec::with(Op::SetBasePointer, Ctx::Ptr { slot: 4, offset: 0 }),
                StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                StageSpec::with(Op::LoadDst, Ctx::Ptr { slot: 1, offset: 0 }),
                StageSpec::new(Op::Srcover),
                StageSpec::with(Op::StoreSrc, Ctx::Ptr { slot: 2, offset: 0 }),
                StageSpec::with(Op::StoreDst, Ctx::Ptr { slot: 3, offset: 0 }),
            ],
            runs: vec![Rect::new(0, 0, w, 1)],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::build_stages;

    #[test]
    fn names_are_unique_and_every_case_builds() {
        let cases = all();
        assert!(cases.len() > 100);
        for c in &cases {
            build_stages(&c.stages).unwrap_or_else(|e| panic!("{}: {e}", c.name));
            assert!(!c.runs.is_empty(), "{}", c.name);
            let _ = c.to_text();
        }
    }

    #[test]
    fn nan_words_never_share_a_lane() {
        for n in [1, 4, 8, 16] {
            let mut lanes: Vec<usize> = NAN_WORDS
                .iter()
                .filter(|&&w| w < 4 * n)
                .map(|&w| w % n)
                .collect();
            let count = lanes.len();
            lanes.sort_unstable();
            lanes.dedup();
            assert_eq!(lanes.len(), count, "N = {n}");
        }
        let (src, dst) = Inputs::NanSrc.pair(1, 2);
        let nans = |b: &[u8]| {
            b.chunks(4)
                .filter(|w| f32::from_le_bytes([w[0], w[1], w[2], w[3]]).is_nan())
                .count()
        };
        assert_eq!((nans(&src), nans(&dst)), (16, 0));
        for k in [Inputs::Special, Inputs::Bits, Inputs::Unit] {
            assert_eq!(nans(&register_input(k, 3)), 0, "{k:?}");
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a: Vec<u64> = all().iter().map(Case::hash).collect();
        let b: Vec<u64> = all().iter().map(Case::hash).collect();
        assert_eq!(a, b);
    }
}
