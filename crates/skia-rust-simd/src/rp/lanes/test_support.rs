// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Test support for the lane primitives: the primitive lists driven through each tier's stamped
//! harness, and input generation (special values, sweeps, random lanes).

// Under Miri the native-vs-model tests are compiled out, leaving parts of this module unused.
#![cfg_attr(miri, allow(dead_code))]

/// A highp primitive, as dispatched by `harness_highp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prim {
    MinF,
    MaxF,
    Mad,
    Nmad,
    AbsF,
    Floor,
    Ceil,
    Sqrt,
    RcpApprox,
    RsqrtApprox,
    RcpPrecise,
    RcpFast,
    Rsqrt,
    Iround,
    Round,
    Trunc,
    ToI32,
    CastF,
    PackU32,
    PackU16,
    IfThenElseF,
    IfThenElseI,
    Any,
    All,
    CondToMask,
    FromHalf,
    ToHalf,
    DivI32,
    DivU32,
    MinI,
    MaxI,
    MinU,
    MaxU,
    AbsI,
}

/// A lowp primitive, as dispatched by `harness_lowp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LowpPrim {
    Div255,
    Div255Accurate,
    MinF,
    MaxF,
    MinI,
    MaxI,
    MinU16,
    MaxU16,
    MinIntrF,
    MaxIntrF,
    MinIntrI,
    MaxIntrI,
    MinIntrU16,
    MaxIntrU16,
    IfThenElseF,
    IfThenElseI,
    IfThenElseU16,
    IfThenElseU32,
    Mad,
    Nmad,
    Trunc,
    ToI32,
    RcpPrecise,
    Sqrt,
    Floor,
    ScaledMult,
}

/// What a primitive's inputs are, which decides how they are generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// One float.
    UnaryF,
    /// Two floats.
    BinaryF,
    /// Three floats (`mad`).
    TernaryF,
    /// One 32-bit integer.
    UnaryInt,
    /// Two 32-bit integers.
    BinaryInt,
    /// One 16-bit integer (every value is tested).
    U16,
    /// Two 16-bit integers.
    BinaryU16,
    /// A (possibly non-canonical) mask and two values.
    Select,
    /// A (possibly non-canonical) mask.
    Mask,
}

impl Prim {
    pub(crate) const ALL: [Prim; 34] = [
        Prim::MinF,
        Prim::MaxF,
        Prim::Mad,
        Prim::Nmad,
        Prim::AbsF,
        Prim::Floor,
        Prim::Ceil,
        Prim::Sqrt,
        Prim::RcpApprox,
        Prim::RsqrtApprox,
        Prim::RcpPrecise,
        Prim::RcpFast,
        Prim::Rsqrt,
        Prim::Iround,
        Prim::Round,
        Prim::Trunc,
        Prim::ToI32,
        Prim::CastF,
        Prim::PackU32,
        Prim::PackU16,
        Prim::IfThenElseF,
        Prim::IfThenElseI,
        Prim::Any,
        Prim::All,
        Prim::CondToMask,
        Prim::FromHalf,
        Prim::ToHalf,
        Prim::DivI32,
        Prim::DivU32,
        Prim::MinI,
        Prim::MaxI,
        Prim::MinU,
        Prim::MaxU,
        Prim::AbsI,
    ];

    pub(crate) fn kind(self) -> Kind {
        match self {
            Prim::MinF | Prim::MaxF => Kind::BinaryF,
            Prim::Mad | Prim::Nmad => Kind::TernaryF,
            Prim::AbsF
            | Prim::Floor
            | Prim::Ceil
            | Prim::Sqrt
            | Prim::RcpApprox
            | Prim::RsqrtApprox
            | Prim::RcpPrecise
            | Prim::RcpFast
            | Prim::Rsqrt
            | Prim::Iround
            | Prim::Round
            | Prim::Trunc
            | Prim::ToI32
            | Prim::ToHalf => Kind::UnaryF,
            Prim::CastF | Prim::PackU32 | Prim::AbsI => Kind::UnaryInt,
            Prim::PackU16 | Prim::FromHalf => Kind::U16,
            Prim::IfThenElseF | Prim::IfThenElseI => Kind::Select,
            Prim::Any | Prim::All | Prim::CondToMask => Kind::Mask,
            Prim::DivI32 | Prim::DivU32 | Prim::MinI | Prim::MaxI | Prim::MinU | Prim::MaxU => {
                Kind::BinaryInt
            }
        }
    }

    /// Whether the x86 tiers' result depends on `rcpps`/`rsqrtps` (`rcp14`/`rsqrt14` on Ml4).
    pub(crate) fn uses_estimates(self) -> bool {
        matches!(
            self,
            Prim::RcpApprox | Prim::RsqrtApprox | Prim::RcpPrecise | Prim::RcpFast | Prim::Rsqrt
        )
    }
}

impl LowpPrim {
    pub(crate) const ALL: [LowpPrim; 26] = [
        LowpPrim::Div255,
        LowpPrim::Div255Accurate,
        LowpPrim::MinF,
        LowpPrim::MaxF,
        LowpPrim::MinI,
        LowpPrim::MaxI,
        LowpPrim::MinU16,
        LowpPrim::MaxU16,
        LowpPrim::MinIntrF,
        LowpPrim::MaxIntrF,
        LowpPrim::MinIntrI,
        LowpPrim::MaxIntrI,
        LowpPrim::MinIntrU16,
        LowpPrim::MaxIntrU16,
        LowpPrim::IfThenElseF,
        LowpPrim::IfThenElseI,
        LowpPrim::IfThenElseU16,
        LowpPrim::IfThenElseU32,
        LowpPrim::Mad,
        LowpPrim::Nmad,
        LowpPrim::Trunc,
        LowpPrim::ToI32,
        LowpPrim::RcpPrecise,
        LowpPrim::Sqrt,
        LowpPrim::Floor,
        LowpPrim::ScaledMult,
    ];

    pub(crate) fn kind(self) -> Kind {
        match self {
            LowpPrim::Div255 | LowpPrim::Div255Accurate => Kind::U16,
            LowpPrim::MinF | LowpPrim::MaxF | LowpPrim::MinIntrF | LowpPrim::MaxIntrF => {
                Kind::BinaryF
            }
            LowpPrim::MinI | LowpPrim::MaxI | LowpPrim::MinIntrI | LowpPrim::MaxIntrI => {
                Kind::BinaryInt
            }
            LowpPrim::MinU16
            | LowpPrim::MaxU16
            | LowpPrim::MinIntrU16
            | LowpPrim::MaxIntrU16
            | LowpPrim::ScaledMult => Kind::BinaryU16,
            LowpPrim::IfThenElseF
            | LowpPrim::IfThenElseI
            | LowpPrim::IfThenElseU16
            | LowpPrim::IfThenElseU32 => Kind::Select,
            LowpPrim::Mad | LowpPrim::Nmad => Kind::TernaryF,
            LowpPrim::Trunc
            | LowpPrim::ToI32
            | LowpPrim::RcpPrecise
            | LowpPrim::Sqrt
            | LowpPrim::Floor => Kind::UnaryF,
        }
    }

    /// Whether the x86 tiers' result depends on `rcpps`.
    pub(crate) fn uses_estimates(self) -> bool {
        self == LowpPrim::RcpPrecise
    }
}

/// `SplitMix64`: a tiny deterministic generator (no dependencies).
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A value in `0..n`.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        #[allow(clippy::cast_possible_truncation)] // the remainder is below n, a usize
        let r = (self.next_u64() % n as u64) as usize;
        r
    }

    /// One of `xs`.
    pub(crate) fn pick(&mut self, xs: &[u32]) -> u32 {
        xs[self.below(xs.len())]
    }
}

/// Make sure no two NaNs meet in one lane of a twin-test input (`lanes` lanes per register, so
/// word `i` is in lane `i % lanes`): in each lane only the first NaN of the `regs` slices is kept
/// (none with `keep_one == false`); the others become `1.5`.
///
/// Which of two NaN operands a commutative `addps`/`mulps` returns is up to the compiler, for
/// Skia's clang as for rustc: LLVM commutes the operands differently in debug and release, so the
/// native tier and its model can legitimately disagree on the NaN's sign and payload (design §2.4
/// "NaN payloads"). The twin tests therefore never feed a stage two NaNs in one lane.
pub(crate) fn thin_nans(regs: &mut [&mut [u32]], lanes: usize, keep_one: bool) {
    let mut seen = std::vec![!keep_one; lanes];
    for reg in regs.iter_mut() {
        for (i, w) in reg.iter_mut().enumerate() {
            if f32::from_bits(*w).is_nan() {
                if seen[i % lanes] {
                    *w = 1.5f32.to_bits();
                }
                seen[i % lanes] = true;
            }
        }
    }
}

/// Special float bit patterns (design §4.1): zeros, denormals, `FLT_MIN`, ±1, values around 0.5
/// and rounding ties, the `i32`/`u32` limits, half-float limits, `FLT_MAX`, infinities, and
/// quiet/signalling NaNs with both signs and several payloads.
pub(crate) fn float_specials() -> std::vec::Vec<u32> {
    let mut v: std::vec::Vec<u32> = [
        0.0f32,
        1.0,
        0.5,
        1.5,
        2.5,
        3.5,
        0.25,
        0.75,
        2.0,
        3.0,
        255.0,
        255.5,
        256.0,
        65535.0,
        65535.5,
        65504.0,        // largest half
        65520.0,        // rounds to half inf
        65536.0,        // software half inf
        6.103_515_6e-5, // 2^-14, smallest normal half
        5.960_464_5e-8, // 2^-24, smallest denormal half
        1e6,
        3e9,
        2_147_483_648.0, // 2^31
        2_147_483_520.0, // largest f32 below 2^31
        4_294_967_296.0, // 2^32
        8_388_608.0,     // 2^23
        8_388_607.5,
        16_777_216.0, // 2^24
        1e-3,
        0.1,
        1.0e30,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::EPSILON,
        f32::INFINITY,
        std::f32::consts::PI,
    ]
    .iter()
    .flat_map(|&x: &f32| [x.to_bits(), (-x).to_bits()])
    .collect();
    v.extend([
        1,           // smallest denormal
        0x007f_ffff, // largest denormal
        0x8000_0001, // -smallest denormal
        0x807f_ffff, // -largest denormal
        0x3eff_ffff, // 0.5 - ulp
        0x3f00_0001, // 0.5 + ulp
        0x3fff_ffff, // 2 - ulp
        0x7fc0_0000, // qNaN
        0xffc0_0000, // -qNaN (x86 indefinite)
        0x7fc0_1234, // qNaN with payload
        0xffc0_0001, // -qNaN with payload
        0x7f80_0001, // sNaN
        0xff80_0001, // -sNaN
        0x7fbf_ffff, // sNaN, max payload
        0xffff_ffff, // -qNaN, all payload bits
        0x3880_0000, // to_half's denorm boundary
        0x387f_ffff, // just below it
        0x4f00_0001, // 2^31 + ulp
        0xcf00_0001, // -2^31 - ulp
    ]);
    v
}

/// Special 32-bit integers.
pub(crate) fn int_specials() -> std::vec::Vec<u32> {
    let mut v: std::vec::Vec<u32> = [
        0i32,
        1,
        2,
        3,
        7,
        255,
        256,
        32767,
        32768,
        65535,
        65536,
        0x0001_2345,
        0x7fff,
        i32::MAX,
        i32::MAX - 1,
        i32::MIN,
        i32::MIN + 1,
        1 << 24,
        (1 << 24) + 1,
    ]
    .iter()
    .flat_map(|&x| [x.cast_unsigned(), x.wrapping_neg().cast_unsigned()])
    .collect();
    v.extend([
        0x8000_0000,
        0xffff_fffe,
        0xffff_ffff,
        0x8000_ffff,
        0x0000_8000,
    ]);
    v
}

/// Masks: canonical, and with only some bits set (sign bit only, low bits only, …).
pub(crate) fn mask_specials() -> std::vec::Vec<u32> {
    std::vec![
        0,
        0xffff_ffff,
        0x8000_0000,
        0x7fff_ffff,
        1,
        0x0000_ffff,
        0xffff_0000,
        0x8000_0001,
        0x00ff_00ff,
    ]
}

/// The sizes of the generated input sets.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    /// Stride of the sweep over all 2^32 float bit patterns (unary ops).
    pub sweep_step: u32,
    /// Number of random lanes for multi-input ops.
    pub random: usize,
    /// Whether the special-value cross products are included.
    pub cross: bool,
    /// Stride of the sweep over all 16-bit values (1: exhaustive).
    pub u16_step: u32,
}

impl Budget {
    /// The budget for this build: small under Miri, moderate in debug builds, larger otherwise.
    pub(crate) fn current() -> Budget {
        if cfg!(miri) {
            Budget {
                sweep_step: 0x1000_0001,
                random: 64,
                cross: false,
                u16_step: 257,
            }
        } else if cfg!(debug_assertions) {
            Budget {
                sweep_step: 16411,
                random: 1 << 15,
                cross: true,
                u16_step: 1,
            }
        } else {
            Budget {
                sweep_step: 257,
                random: 1 << 21,
                cross: true,
                u16_step: 1,
            }
        }
    }
}

/// A random float lane: uniform bits, a special, a small multiple of 1/4 or a nearby pair.
fn random_float(rng: &mut Rng, specials: &[u32]) -> u32 {
    match rng.below(4) {
        0 => rng.next_u32(),
        1 => rng.pick(specials),
        2 => {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
            let x = (rng.next_u32() as i32 >> 20) as f32 * 0.25;
            x.to_bits()
        }
        _ => rng.pick(specials) ^ (rng.next_u32() & 0xf),
    }
}

/// Pads `v` to a multiple of 16 lanes (every stride divides 16).
fn pad(v: &mut std::vec::Vec<u32>, fill: u32) {
    while !v.len().is_multiple_of(16) {
        v.push(fill);
    }
}

/// Inputs `(a, b, c)` for a primitive of `kind` (equal lengths, a multiple of 16 lanes).
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // one arm per input kind; a b c = lanes
pub(crate) fn inputs(kind: Kind, seed: u64, budget: Budget) -> [std::vec::Vec<u32>; 3] {
    let mut rng = Rng::new(seed);
    let fs = float_specials();
    let is = int_specials();
    let ms = mask_specials();
    let (mut a, mut b, mut c) = (
        std::vec::Vec::new(),
        std::vec::Vec::new(),
        std::vec::Vec::new(),
    );
    match kind {
        Kind::UnaryF | Kind::UnaryInt => {
            let specials = if kind == Kind::UnaryF { &fs } else { &is };
            a.extend_from_slice(specials);
            a.extend((0..=u32::MAX).step_by(budget.sweep_step as usize));
            // Every lane position sees every special (lane-order bugs).
            // (One rotation under Miri.)
            for i in 0..if cfg!(miri) { 1 } else { 4 } {
                a.extend(specials.iter().skip(i));
            }
        }
        Kind::U16 => a.extend((0..=u32::from(u16::MAX)).step_by(budget.u16_step as usize)),
        Kind::BinaryF | Kind::TernaryF | Kind::BinaryInt | Kind::BinaryU16 => {
            let specials: &[u32] = match kind {
                Kind::BinaryInt => &is,
                Kind::BinaryU16 => &[
                    0, 1, 127, 128, 255, 256, 0x4000, 0x7fff, 0x8000, 0x8001, 0xffff,
                ],
                _ => &fs,
            };
            let ternary = kind == Kind::TernaryF;
            if budget.cross {
                for &x in specials {
                    for &y in specials {
                        if ternary {
                            for z in [
                                0.0f32.to_bits(),
                                1.0f32.to_bits(),
                                0x7fc0_0000,
                                0xff80_0001,
                                (-2.5f32).to_bits(),
                            ] {
                                a.push(x);
                                b.push(y);
                                c.push(z);
                            }
                        } else {
                            a.push(x);
                            b.push(y);
                        }
                    }
                }
                if ternary {
                    for &z in specials {
                        a.push(f32::INFINITY.to_bits());
                        b.push(0);
                        c.push(z);
                    }
                }
            }
            for _ in 0..budget.random {
                let (x, y, z) = match kind {
                    Kind::BinaryInt => {
                        let r = |rng: &mut Rng| {
                            if rng.below(2) == 0 {
                                rng.next_u32()
                            } else {
                                rng.pick(&is)
                            }
                        };
                        (r(&mut rng), r(&mut rng), 0)
                    }
                    Kind::BinaryU16 => (rng.next_u32() & 0xffff, rng.next_u32() & 0xffff, 0),
                    _ => (
                        random_float(&mut rng, &fs),
                        random_float(&mut rng, &fs),
                        random_float(&mut rng, &fs),
                    ),
                };
                a.push(x);
                b.push(y);
                c.push(z);
            }
        }
        Kind::Select | Kind::Mask => {
            for _ in 0..budget.random {
                a.push(if rng.below(2) == 0 {
                    rng.pick(&ms)
                } else {
                    rng.next_u32()
                });
                b.push(random_float(&mut rng, &fs));
                c.push(random_float(&mut rng, &fs));
            }
            // Whole chunks of canonical masks (any/all on true masks).
            for pattern in 0..16u32 {
                for lane in 0..16 {
                    a.push(if pattern >> (lane % 4) & 1 == 1 {
                        0xffff_ffff
                    } else {
                        0
                    });
                    b.push(1.0f32.to_bits());
                    c.push(2.0f32.to_bits());
                }
            }
        }
    }
    let n = a.len();
    b.resize(n, 0);
    c.resize(n, 0);
    pad(&mut a, 0);
    pad(&mut b, 0);
    pad(&mut c, 0);
    [a, b, c]
}

/// Whether two NaNs meet in one commutative IEEE operation of `mad(f, m, a)` (`a + f*m`) or
/// `nmad` (`a - f*m`), so that which NaN comes out is not specified: LLVM (for us and for Skia's
/// clang) may commute the operands of `fmul`/`fadd`. Only NaN-ness is then comparable.
pub(crate) fn mad_nan_ambiguous(f: u32, m: u32, a: u32, outer_commutes: bool) -> bool {
    use super::x86_model;
    let (f, m, a) = (f32::from_bits(f), f32::from_bits(m), f32::from_bits(a));
    (f.is_nan() && m.is_nan()) || (outer_commutes && a.is_nan() && x86_model::mul(f, m).is_nan())
}

/// Whether several operands of a fused `mad(f, m, a)`/`nmad` (one `vfmadd`/`vfnmadd`) are NaN,
/// so that which NaN comes out is not specified: it depends on the instruction form
/// (`…132`/`…213`/`…231`) the compiler picks, for us and for Skia's clang. Only NaN-ness is
/// then comparable. (One NaN operand, or a NaN from an invalid operation, is fully specified.)
pub(crate) fn fma_nan_ambiguous(f: u32, m: u32, a: u32) -> bool {
    [f, m, a]
        .into_iter()
        .filter(|&x| f32::from_bits(x).is_nan())
        .count()
        >= 2
}
