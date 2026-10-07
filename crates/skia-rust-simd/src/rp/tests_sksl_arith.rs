// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the B6b `SkSL` arithmetic stages (`tiers/highp/sksl_arith.rs`):
//!
//! - every op against a plain-Rust reference, on every selection this host can run (Scalar, the
//!   native x86 tiers, and their models, also under Miri), over inputs where the tiers agree
//!   with IEEE/two's-complement arithmetic;
//! - stage twins: native vs `Model(Host)` vs `Model(AmdZen4)` (`Model(Arm)` for Neon), bit for
//!   bit, on random and special lanes (design §2.8), including the ops that use fused `mad`
//!   (compared up to NaN-ness where operands of a fused op are NaN).
//!
//! Skia's own tests of these stages (`SkRasterPipelineTest.cpp`) are ported in
//! `tests/src/unit/sk_raster_pipeline_test.rs`.

#![cfg_attr(miri, allow(dead_code))]
// Test data is built from small indices; the casts are exact.
#![allow(
    // One table of every op; closures mirror the stage's expression.
    clippy::too_many_lines,
    clippy::redundant_closure_for_method_calls,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use super::contexts::{BinaryOpCtx, ConstantCtx, MatrixMultiplyCtx, TernaryOpCtx};
use super::lanes::test_support::{Rng, float_specials, int_specials};
use super::{MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

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

/// Under Miri (docs/UNSAFE.md): Scalar and one x86 and the Arm model; natively: everything.
fn miri_selections() -> Vec<Selection> {
    if !cfg!(miri) {
        return selections();
    }
    selections()
        .into_iter()
        .filter(|s| matches!(s.tier, Tier::Scalar | Tier::Ml3 | Tier::Neon))
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

/// Slots of memory every run gets (the 4x4 matrix multiply uses 48).
const SLOTS: usize = 48;

/// Runs `stage` (after `set_base_pointer`) over one full chunk with `init` as the slots' words
/// (`slot * n + lane`) and returns the slots' words afterwards.
fn run(sel: Selection, stage: Stage<'static>, init: &[u32]) -> Vec<u32> {
    let n = sel.tier.highp_stride();
    let mut bytes: Vec<u8> = init.iter().flat_map(|w| w.to_ne_bytes()).collect();
    let stages = [Stage::SetBasePointer(MemPtr::new(MemSlot(0), 0)), stage];
    let mut program = Program::new(&stages, sel, true);
    let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut bytes));
    program.run(0, 0, n, 1, &mut mem);
    drop(mem);
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect()
}

/// The domain of a case's operands.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dom {
    /// Floats where IEEE arithmetic is unambiguous: `k/4` for `|k| <= 16` (ties are frequent).
    Float,
    /// As `Float`, never zero (divisors).
    FloatNz,
    /// As `Float`, never negative (`cast_to_uint_from_float`).
    FloatPos,
    /// Non-integral floats `(2k+1)/8` (`floor`/`ceil` of these have a unique answer).
    FloatFrac,
    /// Small ints in `-3..=3`.
    Int,
    /// Non-zero small ints (divisors).
    IntNz,
    /// Any int.
    IntAny,
    /// Any uint.
    UintAny,
    /// Uints below `2^31`, never zero (divisors: the x86 tiers divide via `f64`).
    UintNz,
    /// `0` or `!0` (`mix` selectors, first operand only).
    Mask,
}

impl Dom {
    fn word(self, rng: &mut Rng) -> u32 {
        let k = rng.below(33) as i32 - 16;
        match self {
            Dom::Float => (k as f32 / 4.0).to_bits(),
            Dom::FloatNz => ((if k == 0 { 5 } else { k }) as f32 / 4.0).to_bits(),
            Dom::FloatPos => (k.unsigned_abs() as f32 / 4.0).to_bits(),
            Dom::FloatFrac => ((2 * k + 1) as f32 / 8.0).to_bits(),
            Dom::Int => (rng.below(7) as i32 - 3) as u32,
            Dom::IntNz => {
                let v = rng.below(6) as i32 - 3;
                (if v >= 0 { v + 1 } else { v }) as u32 // -3..=3 without 0
            }
            Dom::IntAny | Dom::UintAny => rng.next_u32(),
            Dom::UintNz => rng.next_u32() % 0x7fff_ffff + 1,
            Dom::Mask => {
                if rng.below(2) == 0 {
                    0
                } else {
                    !0
                }
            }
        }
    }
}

/// How a case's lanes are computed: `slot j` of the result is `f(in[j], in[k + j], in[2k + j])`
/// (fewer operands for lower arities), or `f(in[j], imm)` for immediate ops.
#[derive(Clone, Copy)]
enum Elem {
    U1(fn(u32) -> u32),
    U2(fn(u32, u32) -> u32),
    U3(fn(u32, u32, u32) -> u32),
    /// Twin-tested only (no reference).
    None,
}

struct Case {
    name: &'static str,
    /// The slot counts to run (`k`).
    ks: &'static [usize],
    /// The stage for `k` slots on a tier with `n` lanes.
    stage: Box<dyn Fn(usize, usize) -> Stage<'static>>,
    elem: Elem,
    dom: Dom,
    /// `Some(bits)` for an immediate op (the operand is the constant).
    imm: Option<u32>,
    /// Twin comparison is exact (otherwise: NaN-ness where both are NaN, for fused `mad`).
    exact_twin: bool,
}

type Mp = fn(MemPtr) -> Stage<'static>;

fn mp(slot_stage: Mp) -> impl Fn(usize, usize) -> Stage<'static> {
    move |_, _| slot_stage(MemPtr::new(MemSlot(0), 0))
}

/// `[1-slot, 2-slot, 3-slot, 4-slot]` hard-coded stages plus the n-way one, as one closure.
fn fixed_and_n<C: 'static>(
    fixed: [Mp; 4],
    n_way: impl Fn(u32, u32) -> C + 'static,
    wrap: fn(C) -> Stage<'static>,
) -> Box<dyn Fn(usize, usize) -> Stage<'static>> {
    Box::new(move |k, lanes| {
        // k = 1..=4: the hard-coded stage; k = 5: the n-way stage over 5 slots.
        if k <= 4 {
            fixed[k - 1](MemPtr::new(MemSlot(0), 0))
        } else {
            wrap(n_way(0, (k * lanes * 4) as u32))
        }
    })
}

fn binary_stage(
    fixed: [Mp; 4],
    n_way: fn(BinaryOpCtx) -> Stage<'static>,
) -> Box<dyn Fn(usize, usize) -> Stage<'static>> {
    fixed_and_n(fixed, |dst, src| BinaryOpCtx { dst, src }, n_way)
}

fn ternary_stage(
    fixed: [Mp; 4],
    n_way: fn(TernaryOpCtx) -> Stage<'static>,
) -> Box<dyn Fn(usize, usize) -> Stage<'static>> {
    fixed_and_n(fixed, |dst, delta| TernaryOpCtx { dst, delta }, n_way)
}

fn case(
    name: &'static str,
    ks: &'static [usize],
    stage: Box<dyn Fn(usize, usize) -> Stage<'static>>,
    elem: Elem,
    dom: Dom,
) -> Case {
    Case {
        name,
        ks,
        stage,
        elem,
        dom,
        imm: None,
        exact_twin: true,
    }
}

const K1_4: &[usize] = &[1, 2, 3, 4];
const K1_5: &[usize] = &[1, 2, 3, 4, 5];

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}
fn i(w: u32) -> i32 {
    w as i32
}
fn mask(b: bool) -> u32 {
    if b { !0 } else { 0 }
}

macro_rules! unary {
    ($out:ident, $name:literal, $dom:expr, $f:expr, $a:ident $b:ident $c:ident $d:ident) => {
        $out.push(case(
            $name,
            K1_4,
            Box::new(|k, _| {
                let ctors: [Mp; 4] = [Stage::$a, Stage::$b, Stage::$c, Stage::$d];
                ctors[k - 1](MemPtr::new(MemSlot(0), 0))
            }),
            Elem::U1($f),
            $dom,
        ));
    };
}

macro_rules! binary {
    ($out:ident, $name:literal, $dom:expr, $f:expr, $a:ident $b:ident $c:ident $d:ident $n:ident) => {
        $out.push(case(
            $name,
            K1_5,
            binary_stage([Stage::$a, Stage::$b, Stage::$c, Stage::$d], Stage::$n),
            Elem::U2($f),
            $dom,
        ));
    };
}

macro_rules! imm {
    ($out:ident, $name:literal, $dom:expr, $imm:expr, $f:expr, $k:expr, $ctor:ident) => {{
        let imm: u32 = $imm;
        let mut c = case(
            $name,
            $k,
            Box::new(move |_, _| {
                Stage::$ctor(ConstantCtx {
                    value: imm as i32,
                    dst: 0,
                })
            }),
            Elem::U2($f),
            $dom,
        );
        c.imm = Some(imm);
        $out.push(c);
    }};
}

fn cases() -> Vec<Case> {
    let mut c = Vec::new();

    // Casts and unary ops.
    unary!(c, "cast_to_float_from_int", Dom::IntAny, |a| (i(a) as f32).to_bits(),
        CastToFloatFromInt CastToFloatFrom2Ints CastToFloatFrom3Ints CastToFloatFrom4Ints);
    unary!(c, "cast_to_float_from_uint", Dom::UintAny, |a| (a as f32).to_bits(),
        CastToFloatFromUint CastToFloatFrom2Uints CastToFloatFrom3Uints CastToFloatFrom4Uints);
    unary!(c, "cast_to_int_from_float", Dom::FloatFrac, |a| (f(a) as i32) as u32,
        CastToIntFromFloat CastToIntFrom2Floats CastToIntFrom3Floats CastToIntFrom4Floats);
    unary!(c, "cast_to_uint_from_float", Dom::FloatPos, |a| f(a) as u32,
        CastToUintFromFloat CastToUintFrom2Floats CastToUintFrom3Floats CastToUintFrom4Floats);
    unary!(c, "abs_int", Dom::Int, |a| i(a).wrapping_abs() as u32,
        AbsInt Abs2Ints Abs3Ints Abs4Ints);
    unary!(c, "floor_float", Dom::FloatFrac, |a| f(a).floor().to_bits(),
        FloorFloat Floor2Floats Floor3Floats Floor4Floats);
    unary!(c, "ceil_float", Dom::FloatFrac, |a| f(a).ceil().to_bits(),
        CeilFloat Ceil2Floats Ceil3Floats Ceil4Floats);

    // Float arithmetic.
    binary!(c, "add_float", Dom::Float, |a, b| (f(a) + f(b)).to_bits(),
        AddFloat Add2Floats Add3Floats Add4Floats AddNFloats);
    binary!(c, "sub_float", Dom::Float, |a, b| (f(a) - f(b)).to_bits(),
        SubFloat Sub2Floats Sub3Floats Sub4Floats SubNFloats);
    binary!(c, "mul_float", Dom::Float, |a, b| (f(a) * f(b)).to_bits(),
        MulFloat Mul2Floats Mul3Floats Mul4Floats MulNFloats);
    binary!(c, "div_float", Dom::FloatNz, |a, b| (f(a) / f(b)).to_bits(),
        DivFloat Div2Floats Div3Floats Div4Floats DivNFloats);
    binary!(c, "max_float", Dom::Float, |a, b| if f(a) > f(b) { a } else { b },
        MaxFloat Max2Floats Max3Floats Max4Floats MaxNFloats);
    binary!(c, "min_float", Dom::Float, |a, b| if f(a) < f(b) { a } else { b },
        MinFloat Min2Floats Min3Floats Min4Floats MinNFloats);

    // Int and uint arithmetic.
    binary!(c, "add_int", Dom::IntAny, |a, b| a.wrapping_add(b),
        AddInt Add2Ints Add3Ints Add4Ints AddNInts);
    binary!(c, "sub_int", Dom::IntAny, |a, b| a.wrapping_sub(b),
        SubInt Sub2Ints Sub3Ints Sub4Ints SubNInts);
    binary!(c, "mul_int", Dom::IntAny, |a, b| a.wrapping_mul(b),
        MulInt Mul2Ints Mul3Ints Mul4Ints MulNInts);
    binary!(c, "div_int", Dom::IntNz, |a, b| i(a).wrapping_div(i(b)) as u32,
        DivInt Div2Ints Div3Ints Div4Ints DivNInts);
    binary!(c, "div_uint", Dom::UintNz, |a, b| a / b,
        DivUint Div2Uints Div3Uints Div4Uints DivNUints);
    binary!(c, "bitwise_and_int", Dom::IntAny, |a, b| a & b,
        BitwiseAndInt BitwiseAnd2Ints BitwiseAnd3Ints BitwiseAnd4Ints BitwiseAndNInts);
    binary!(c, "bitwise_or_int", Dom::IntAny, |a, b| a | b,
        BitwiseOrInt BitwiseOr2Ints BitwiseOr3Ints BitwiseOr4Ints BitwiseOrNInts);
    binary!(c, "bitwise_xor_int", Dom::IntAny, |a, b| a ^ b,
        BitwiseXorInt BitwiseXor2Ints BitwiseXor3Ints BitwiseXor4Ints BitwiseXorNInts);
    binary!(c, "max_int", Dom::IntAny, |a, b| if i(a) > i(b) { a } else { b },
        MaxInt Max2Ints Max3Ints Max4Ints MaxNInts);
    binary!(c, "max_uint", Dom::UintAny, |a, b| if a > b { a } else { b },
        MaxUint Max2Uints Max3Uints Max4Uints MaxNUints);
    binary!(c, "min_int", Dom::IntAny, |a, b| if i(a) < i(b) { a } else { b },
        MinInt Min2Ints Min3Ints Min4Ints MinNInts);
    binary!(c, "min_uint", Dom::UintAny, |a, b| if a < b { a } else { b },
        MinUint Min2Uints Min3Uints Min4Uints MinNUints);

    // Comparisons (all-ones/all-zeros masks).
    binary!(c, "cmplt_float", Dom::Float, |a, b| mask(f(a) < f(b)),
        CmpltFloat Cmplt2Floats Cmplt3Floats Cmplt4Floats CmpltNFloats);
    binary!(c, "cmple_float", Dom::Float, |a, b| mask(f(a) <= f(b)),
        CmpleFloat Cmple2Floats Cmple3Floats Cmple4Floats CmpleNFloats);
    binary!(c, "cmpeq_float", Dom::Float, |a, b| mask(f(a) == f(b)),
        CmpeqFloat Cmpeq2Floats Cmpeq3Floats Cmpeq4Floats CmpeqNFloats);
    binary!(c, "cmpne_float", Dom::Float, |a, b| mask(f(a) != f(b)),
        CmpneFloat Cmpne2Floats Cmpne3Floats Cmpne4Floats CmpneNFloats);
    binary!(c, "cmplt_int", Dom::Int, |a, b| mask(i(a) < i(b)),
        CmpltInt Cmplt2Ints Cmplt3Ints Cmplt4Ints CmpltNInts);
    binary!(c, "cmple_int", Dom::Int, |a, b| mask(i(a) <= i(b)),
        CmpleInt Cmple2Ints Cmple3Ints Cmple4Ints CmpleNInts);
    binary!(c, "cmpeq_int", Dom::Int, |a, b| mask(a == b),
        CmpeqInt Cmpeq2Ints Cmpeq3Ints Cmpeq4Ints CmpeqNInts);
    binary!(c, "cmpne_int", Dom::Int, |a, b| mask(a != b),
        CmpneInt Cmpne2Ints Cmpne3Ints Cmpne4Ints CmpneNInts);
    binary!(c, "cmplt_uint", Dom::UintAny, |a, b| mask(a < b),
        CmpltUint Cmplt2Uints Cmplt3Uints Cmplt4Uints CmpltNUints);
    binary!(c, "cmple_uint", Dom::UintAny, |a, b| mask(a <= b),
        CmpleUint Cmple2Uints Cmple3Uints Cmple4Uints CmpleNUints);

    // Immediate forms (the constant is the right operand of every affected slot).
    imm!(
        c,
        "add_imm_float",
        Dom::Float,
        0.75f32.to_bits(),
        |a, b| (f(a) + f(b)).to_bits(),
        &[1],
        AddImmFloat
    );
    imm!(
        c,
        "mul_imm_float",
        Dom::Float,
        (-1.5f32).to_bits(),
        |a, b| (f(a) * f(b)).to_bits(),
        &[1],
        MulImmFloat
    );
    imm!(
        c,
        "max_imm_float",
        Dom::Float,
        0.5f32.to_bits(),
        |a, b| if f(a) > f(b) { a } else { b },
        &[1],
        MaxImmFloat
    );
    imm!(
        c,
        "min_imm_float",
        Dom::Float,
        0.5f32.to_bits(),
        |a, b| if f(a) < f(b) { a } else { b },
        &[1],
        MinImmFloat
    );
    imm!(
        c,
        "add_imm_int",
        Dom::IntAny,
        0x1234_5678,
        |a, b| a.wrapping_add(b),
        &[1],
        AddImmInt
    );
    imm!(
        c,
        "mul_imm_int",
        Dom::IntAny,
        0xfffd_0003,
        |a, b| a.wrapping_mul(b),
        &[1],
        MulImmInt
    );
    imm!(
        c,
        "bitwise_xor_imm_int",
        Dom::IntAny,
        0x00ff_ff00,
        |a, b| a ^ b,
        &[1],
        BitwiseXorImmInt
    );
    imm!(
        c,
        "bitwise_and_imm_int",
        Dom::IntAny,
        0x0ff0_0ff0,
        |a, b| a & b,
        &[1],
        BitwiseAndImmInt
    );
    imm!(
        c,
        "bitwise_and_imm_2_ints",
        Dom::IntAny,
        0x0ff0_0ff0,
        |a, b| a & b,
        &[2],
        BitwiseAndImm2Ints
    );
    imm!(
        c,
        "bitwise_and_imm_3_ints",
        Dom::IntAny,
        0x0ff0_0ff0,
        |a, b| a & b,
        &[3],
        BitwiseAndImm3Ints
    );
    imm!(
        c,
        "bitwise_and_imm_4_ints",
        Dom::IntAny,
        0x0ff0_0ff0,
        |a, b| a & b,
        &[4],
        BitwiseAndImm4Ints
    );
    imm!(
        c,
        "cmplt_imm_float",
        Dom::Float,
        0.25f32.to_bits(),
        |a, b| mask(f(a) < f(b)),
        &[1],
        CmpltImmFloat
    );
    imm!(
        c,
        "cmple_imm_float",
        Dom::Float,
        0.25f32.to_bits(),
        |a, b| mask(f(a) <= f(b)),
        &[1],
        CmpleImmFloat
    );
    imm!(
        c,
        "cmpeq_imm_float",
        Dom::Float,
        0.25f32.to_bits(),
        |a, b| mask(f(a) == f(b)),
        &[1],
        CmpeqImmFloat
    );
    imm!(
        c,
        "cmpne_imm_float",
        Dom::Float,
        0.25f32.to_bits(),
        |a, b| mask(f(a) != f(b)),
        &[1],
        CmpneImmFloat
    );
    imm!(
        c,
        "cmplt_imm_int",
        Dom::Int,
        1,
        |a, b| mask(i(a) < i(b)),
        &[1],
        CmpltImmInt
    );
    imm!(
        c,
        "cmple_imm_int",
        Dom::Int,
        1,
        |a, b| mask(i(a) <= i(b)),
        &[1],
        CmpleImmInt
    );
    imm!(
        c,
        "cmpeq_imm_int",
        Dom::Int,
        1,
        |a, b| mask(a == b),
        &[1],
        CmpeqImmInt
    );
    imm!(
        c,
        "cmpne_imm_int",
        Dom::Int,
        1,
        |a, b| mask(a != b),
        &[1],
        CmpneImmInt
    );
    imm!(
        c,
        "cmplt_imm_uint",
        Dom::UintAny,
        0x8000_0000,
        |a, b| mask(a < b),
        &[1],
        CmpltImmUint
    );
    imm!(
        c,
        "cmple_imm_uint",
        Dom::UintAny,
        0x8000_0000,
        |a, b| mask(a <= b),
        &[1],
        CmpleImmUint
    );

    // Ternary ops: `mix` on ints selects with a mask; the float ops, `mod` and `smoothstep` have
    // no exact reference (fused `mad`, `floor`), so they are twin-tested only.
    c.push(case(
        "mix_int",
        K1_5,
        ternary_stage(
            [
                Stage::MixInt,
                Stage::Mix2Ints,
                Stage::Mix3Ints,
                Stage::Mix4Ints,
            ],
            Stage::MixNInts,
        ),
        Elem::U3(|a, x, y| if a == 0 { x } else { y }),
        Dom::Mask,
    ));
    c.push(Case {
        exact_twin: false,
        ..case(
            "mix_float",
            K1_5,
            ternary_stage(
                [
                    Stage::MixFloat,
                    Stage::Mix2Floats,
                    Stage::Mix3Floats,
                    Stage::Mix4Floats,
                ],
                Stage::MixNFloats,
            ),
            Elem::None,
            Dom::Float,
        )
    });
    c.push(Case {
        exact_twin: false,
        ..case(
            "mod_float",
            K1_5,
            binary_stage(
                [
                    Stage::ModFloat,
                    Stage::Mod2Floats,
                    Stage::Mod3Floats,
                    Stage::Mod4Floats,
                ],
                Stage::ModNFloats,
            ),
            Elem::None,
            Dom::FloatNz,
        )
    });
    c.push(Case {
        exact_twin: false,
        ..case(
            "smoothstep_n_floats",
            &[1, 2, 3, 4, 5],
            Box::new(|k, lanes| {
                Stage::SmoothstepNFloats(TernaryOpCtx {
                    dst: 0,
                    delta: (k * lanes * 4) as u32,
                })
            }),
            Elem::None,
            Dom::Float,
        )
    });

    // Dot products, refract and matrix multiplies: twin-tested only (their ported tests compare
    // against a reference).
    for (name, stage) in [
        ("dot_2_floats", mp(Stage::Dot2Floats)),
        ("dot_3_floats", mp(Stage::Dot3Floats)),
        ("dot_4_floats", mp(Stage::Dot4Floats)),
        ("refract_4_floats", mp(Stage::Refract4Floats)),
    ] {
        c.push(Case {
            exact_twin: false,
            ..case(name, &[1], Box::new(stage), Elem::None, Dom::Float)
        });
    }
    c.push(Case {
        exact_twin: false,
        ..case(
            "matrix_multiply",
            &[2, 3, 4],
            Box::new(|k, _| {
                let ctx = MatrixMultiplyCtx {
                    dst: 0,
                    left_columns: k as u8,
                    left_rows: k as u8,
                    right_columns: k as u8,
                    right_rows: k as u8,
                };
                match k {
                    2 => Stage::MatrixMultiply2(ctx),
                    3 => Stage::MatrixMultiply3(ctx),
                    _ => Stage::MatrixMultiply4(ctx),
                }
            }),
            Elem::None,
            Dom::Float,
        )
    });
    c
}

/// The case's input words for `n` lanes (all `SLOTS` slots).
fn inputs(case: &Case, dom_override: Option<Dom>, rng: &mut Rng, n: usize) -> Vec<u32> {
    let dom = dom_override.unwrap_or(case.dom);
    (0..SLOTS * n)
        .map(|w| {
            // `Mask` only applies to the selector (the first operand's slots).
            let d = if dom == Dom::Mask && w >= 5 * n {
                Dom::IntAny
            } else {
                dom
            };
            d.word(rng)
        })
        .collect()
}

/// Wild words for the twin comparison: specials and random bits (including NaN, zero divisors).
fn wild_inputs(case: &Case, rng: &mut Rng, n: usize, specials: &[u32], int_sp: &[u32]) -> Vec<u32> {
    let float_like = case.name.contains("float")
        || case.name.contains("smoothstep")
        || case.name.contains("dot")
        || case.name.contains("refract")
        || case.name.contains("matrix")
        || case.name.contains("mod");
    (0..SLOTS * n)
        .map(|_| match rng.below(4) {
            0 => rng.next_u32(),
            1 if float_like => rng.pick(specials),
            1 => rng.pick(int_sp),
            _ => Dom::Float.word(rng),
        })
        .collect()
}

fn expected(case: &Case, k: usize, n: usize, init: &[u32]) -> Vec<u32> {
    let mut out = init.to_vec();
    let at = |slot: usize, lane: usize| init[slot * n + lane];
    for j in 0..k {
        for lane in 0..n {
            out[j * n + lane] = match (case.elem, case.imm) {
                (Elem::U1(g), _) => g(at(j, lane)),
                (Elem::U2(g), Some(imm)) => g(at(j, lane), imm),
                (Elem::U2(g), None) => g(at(j, lane), at(k + j, lane)),
                (Elem::U3(g), _) => g(at(j, lane), at(k + j, lane), at(2 * k + j, lane)),
                (Elem::None, _) => unreachable!(),
            };
        }
    }
    out
}

fn same(a: u32, b: u32, by_value: bool) -> bool {
    a == b || (by_value && (f(a) == f(b) || (f(a).is_nan() && f(b).is_nan())))
}

#[test]
fn sksl_arith_matches_reference() {
    let rounds = if cfg!(miri) { 1 } else { 24 };
    for sel in miri_selections() {
        let n = sel.tier.highp_stride();
        let mut rng = Rng::new(0x00b6_b001);
        for (index, case) in cases()
            .iter()
            .filter(|c| !matches!(c.elem, Elem::None))
            .enumerate()
        {
            // Miri policy (docs/UNSAFE.md): a representative sample of the ops.
            if cfg!(miri) && index % 6 != 0 {
                continue;
            }
            let by_value = case.name.starts_with("floor") || case.name.starts_with("ceil");
            for &k in case.ks {
                // Miri is slow: only the first and last slot counts.
                if cfg!(miri) && k != case.ks[0] && k != case.ks[case.ks.len() - 1] {
                    continue;
                }
                for _ in 0..rounds {
                    let init = inputs(case, None, &mut rng, n);
                    let got = run(sel, (case.stage)(k, n), &init);
                    let want = expected(case, k, n, &init);
                    for (w, (g, e)) in got.iter().zip(&want).enumerate() {
                        assert!(
                            same(*g, *e, by_value),
                            "{sel}: {} (k = {k}) word {w}: got {g:#010x}, want {e:#010x}",
                            case.name
                        );
                    }
                }
            }
        }
    }
}

/// Division by zero and `INT_MIN / -1` do not crash on any selection (design §1: integer
/// division per tier; the twin test compares each tier with its models on such lanes).
#[test]
fn int_division_edge_cases_run() {
    for sel in miri_selections() {
        let n = sel.tier.highp_stride();
        let init: Vec<u32> = (0..SLOTS * n)
            .map(|w| match (w / n) % 2 {
                0 => [i32::MIN as u32, 7, 0x8000_0001, 0][w % 4],
                _ => [0, 0, (-1i32) as u32, 0xffff_ffff][w % 4],
            })
            .collect();
        for stage in [
            Stage::DivInt(MemPtr::new(MemSlot(0), 0)),
            Stage::DivUint(MemPtr::new(MemSlot(0), 0)),
        ] {
            let _ = run(sel, stage, &init);
        }
    }
}

#[test]
fn sksl_arith_twins() {
    let specials = float_specials();
    let int_sp = int_specials();
    let rounds = 60;
    for (native, models) in twin_sets() {
        let n = native.tier.highp_stride();
        let mut rng = Rng::new(0x00b6_b002);
        for case in cases() {
            for &k in case.ks {
                for round in 0..rounds {
                    let init = wild_inputs(&case, &mut rng, n, &specials, &int_sp);
                    let stage = (case.stage)(k, n);
                    let want = run(native, stage, &init);
                    for model in &models {
                        let got = run(*model, stage, &init);
                        for (w, (g, e)) in got.iter().zip(&want).enumerate() {
                            let ok =
                                *g == *e || (!case.exact_twin && f(*g).is_nan() && f(*e).is_nan());
                            assert!(
                                ok,
                                "{native} vs {model}: {} (k = {k}) round {round} word {w}: \
                                 model {g:#010x}, native {e:#010x}",
                                case.name
                            );
                        }
                    }
                }
            }
        }
    }
}

/// `cast_to_uint_from_float` is `__builtin_convertvector(F, U32)`: on `Ml4` an unsigned
/// conversion (`vcvttps2udq`: below `-1`, NaN and `2^32` and more give `0xFFFFFFFF`), on the
/// other x86 tiers the signed conversion's bits, on `Scalar` wasm's saturating `trunc_sat` (found
/// by `oracle/rp-diff`: Ml4 used the signed conversion).
#[test]
fn cast_to_uint_from_float_semantics() {
    let inputs = [
        -300.0f32,
        -0.5,
        f32::NAN,
        2_147_483_648.0,
        4_294_967_040.0,
        4_294_967_296.0,
        1.0e10,
        7.9,
    ];
    for sel in miri_selections() {
        let n = sel.tier.highp_stride();
        let init: Vec<u32> = (0..n * 4).map(|w| inputs[w % 8].to_bits()).collect();
        let out = run(
            sel,
            Stage::CastToUintFromFloat(MemPtr::new(MemSlot(0), 0)),
            &init,
        );
        let want: [u32; 8] = match sel.tier {
            Tier::Ml4 => [
                0xffff_ffff,
                0,
                0xffff_ffff,
                0x8000_0000,
                0xffff_ff00,
                0xffff_ffff,
                0xffff_ffff,
                7,
            ],
            Tier::Scalar => [
                0,
                0,
                0,
                0x8000_0000,
                0xffff_ff00,
                0xffff_ffff,
                0xffff_ffff,
                7,
            ],
            // Neon has no oracle yet (Skia's would be `FCVTZU`).
            Tier::Neon => continue,
            _ => [
                0xffff_fed4,
                0,
                0x8000_0000,
                0x8000_0000,
                0x8000_0000,
                0x8000_0000,
                0x8000_0000,
                7,
            ],
        };
        for w in 0..n {
            assert_eq!(out[w], want[w % 8], "{sel}: lane {w}");
        }
    }
}

/// On the `Scalar` tier `F` is a C `float` and `smoothstep`'s `3.0 - 2.0 * t` and `refract`'s
/// `1.0 - eta * eta * (1.0 - dotNI * dotNI)` are evaluated in double precision, narrowed on
/// assignment (found by `oracle/rp-diff`); the vector tiers' `F` converts the literals to float.
#[test]
fn scalar_double_precision_literals() {
    let sel = Selection::native(Tier::Scalar);
    let mut rng = Rng::new(0x00b6_b003);
    let unit = |rng: &mut Rng| (rng.below(1 << 24) as f32) / 16_777_216.0;
    let mut differ = 0;
    for _ in 0..400 {
        // smoothstep(0, 1, x) = t * t * (3.0 - 2.0 * t), t = x.
        let x = unit(&mut rng);
        let init = [0.0f32.to_bits(), 1.0f32.to_bits(), x.to_bits()];
        let stage = Stage::SmoothstepNFloats(TernaryOpCtx { dst: 0, delta: 4 });
        let got = f32::from_bits(run(sel, stage, &init)[0]);
        let want = (f64::from(x * x) * (3.0 - 2.0 * f64::from(x))) as f32;
        assert_eq!(got.to_bits(), want.to_bits(), "smoothstep({x})");
        differ += usize::from(want != x * x * (3.0f32 - 2.0f32 * x));

        // refract: incident (4), normal (4), eta (1).
        let v: Vec<f32> = (0..9).map(|_| unit(&mut rng) * 2.0 - 1.0).collect();
        let init: Vec<u32> = v.iter().map(|x| x.to_bits()).collect();
        let out = run(
            sel,
            Stage::Refract4Floats(MemPtr::new(MemSlot(0), 0)),
            &init,
        );
        let (inc, nor, eta) = (&v[0..4], &v[4..8], v[8]);
        let dot = nor[0] * inc[0] + (nor[1] * inc[1] + (nor[2] * inc[2] + nor[3] * inc[3]));
        let k = (1.0 - f64::from(eta * eta) * (1.0 - f64::from(dot * dot))) as f32;
        let sqrt_k = k.sqrt();
        for idx in 0..4 {
            let want = if k >= 0.0 {
                eta * inc[idx] - (eta * dot + sqrt_k) * nor[idx]
            } else {
                0.0
            };
            assert_eq!(out[idx], want.to_bits(), "refract {v:?} component {idx}");
        }
    }
    assert!(
        differ > 0,
        "the test must be able to tell double from float"
    );
}
