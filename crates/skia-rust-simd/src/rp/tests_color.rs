// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the B4 color stages (design §5): known answers on every selection this host can run
//! (Scalar, the native tiers and their models, also under Miri), and stage twins (native vs
//! `Model(Host)` vs `Model(AmdZen4)`, `Model(Arm)` for Neon) on random and special lanes.
//!
//! Like `tests.rs`'s `srcover` twin, the twins compare only NaN-ness where both results are NaN
//! (which NaN of two a commutative operation returns depends on operand order the compiler
//! picks); every other lane must match bit for bit.

// Test data is built from small indices (the casts are exact), expected values are Skia's
// constants and exact float results, and each test lists many stages.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    clippy::too_many_lines
)]

use super::contexts::{EmbossCtx, TablesCtx, TransferFunction, UniformColorCtx};
use super::lanes::test_support::{Rng, float_specials, thin_nans};
use super::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Program, Stage};
use crate::tier::{Backend, Estimates, Selection, Tier};

const SIMD: [Tier; 5] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4, Tier::Neon];

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

/// `selections()`, thinned under Miri to Scalar and the `Ml4` and `Neon` models (the two stride
/// classes; `Sse2` is covered by the memory tests), for the tests that cost one pipeline run per case.
fn sample_selections() -> Vec<Selection> {
    selections()
        .into_iter()
        .filter(|s| !cfg!(miri) || !matches!(s.tier, Tier::Sse2 | Tier::Sse41 | Tier::Ml3))
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

const IN0: MemPtr = MemPtr::new(MemSlot(0), 0);
const IN1: MemPtr = MemPtr::new(MemSlot(1), 0);
const OUT0: MemPtr = MemPtr::new(MemSlot(2), 0);
const OUT1: MemPtr = MemPtr::new(MemSlot(3), 0);
/// `emboss`'s two A8 contexts.
const EMBOSS: EmbossCtx = EmbossCtx {
    mul: MemoryCtx::new(MemSlot(4)),
    add: MemoryCtx::new(MemSlot(5)),
};

/// Bytes of four registers of the widest tier.
const REGS_BYTES: usize = 4 * 4 * 16;

/// What a run returns: `r,g,b,a` and `dr,dg,db,da` after the stage.
struct Out {
    src: [u8; REGS_BYTES],
    dst: [u8; REGS_BYTES],
}

/// Runs `load_src, load_dst, stage, store_src, store_dst` over `w` pixels at `at` with the
/// register images `src`/`dst` (planar, `N` lanes per register) and the A8 images `mul`/`add`
/// (the `emboss` contexts, read at `(0, 0)`) bound. Returns whether the program was lowp.
fn run_stage(
    stage: Stage<'_>,
    sel: Selection,
    force_highp: bool,
    at: (usize, usize),
    w: usize,
    (src, dst): (&[u8], &[u8]),
    (mul, add): (&[u8], &[u8]),
) -> (bool, Out) {
    let mut a = [0u8; REGS_BYTES];
    let mut b = [0u8; REGS_BYTES];
    a[..src.len()].copy_from_slice(src);
    b[..dst.len()].copy_from_slice(dst);
    let mut out = Out {
        src: [0; REGS_BYTES],
        dst: [0; REGS_BYTES],
    };
    let stages = [
        Stage::LoadSrc(IN0),
        Stage::LoadDst(IN1),
        stage,
        Stage::StoreSrc(OUT0),
        Stage::StoreDst(OUT1),
    ];
    let mut program = Program::new(&stages, sel, force_highp);
    let lowp = program.is_lowp();
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&a))
        .with(MemSlot(1), MemView::read(&b))
        .with(MemSlot(2), MemView::write(&mut out.src))
        .with(MemSlot(3), MemView::write(&mut out.dst))
        .with(MemSlot(4), MemView::read(mul))
        .with(MemSlot(5), MemView::read(add));
    program.run(at.0, at.1, w, 1, &mut mem);
    drop(mem);
    (lowp, out)
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

/// A register image with every lane of channel `c` set to `v[c]` (`n` lanes per channel).
fn splat_regs(n: usize, v: [f32; 4]) -> Vec<u8> {
    v.iter()
        .flat_map(|&x| std::iter::repeat_n(x, n))
        .flat_map(f32::to_ne_bytes)
        .collect()
}

/// A lowp register image with every lane of channel `c` set to `v[c]`.
fn splat_regs_lowp(n: usize, v: [u16; 4]) -> Vec<u8> {
    v.iter()
        .flat_map(|&x| std::iter::repeat_n(x, n))
        .flat_map(u16::to_ne_bytes)
        .collect()
}

/// Lane 0 of each of the four registers in a highp output image of `n` lanes.
fn lane0(out: &[u8], n: usize) -> [f32; 4] {
    let f = floats(&out[..16 * n]);
    [f[0], f[n], f[2 * n], f[3 * n]]
}

/// Lane 0 of each of the four registers in a lowp output image of `n` lanes.
fn lane0_lowp(out: &[u8], n: usize) -> [u16; 4] {
    let h = halves(&out[..8 * n]);
    [h[0], h[n], h[2 * n], h[3 * n]]
}

/// Runs a highp stage with every lane of `src` set to `src_color` (and `dst` to `dst_color`) on
/// every selection, returning each selection's lane 0 (`r,g,b,a`, then `dr,dg,db,da`).
fn highp_known(
    stage: Stage<'_>,
    src_color: [f32; 4],
    dst_color: [f32; 4],
) -> Vec<(Selection, [f32; 4], [f32; 4])> {
    sample_selections()
        .into_iter()
        .map(|sel| {
            let n = sel.tier.highp_stride();
            let (_, out) = run_stage(
                stage,
                sel,
                true,
                (0, 0),
                n,
                (&splat_regs(n, src_color), &splat_regs(n, dst_color)),
                (&[], &[]),
            );
            (sel, lane0(&out.src, n), lane0(&out.dst, n))
        })
        .collect()
}

#[test]
fn highp_known_answers() {
    // Bit patterns: results of exact operations on exactly representable values.
    for (sel, s, d) in highp_known(Stage::Premul, [1.0, 0.5, 0.25, 0.5], [0.0; 4]) {
        assert_eq!(s, [0.5, 0.25, 0.125, 0.5], "premul {sel}");
        assert_eq!(d, [0.0; 4], "premul {sel}");
    }
    for (sel, s, d) in highp_known(Stage::PremulDst, [0.0; 4], [1.0, 0.5, 0.25, 0.5]) {
        assert_eq!(s, [0.0; 4], "premul_dst {sel}");
        assert_eq!(d, [0.5, 0.25, 0.125, 0.5], "premul_dst {sel}");
    }
    for (sel, s, _) in highp_known(Stage::Unpremul, [0.5, 0.25, 0.125, 0.5], [0.0; 4]) {
        assert_eq!(s, [1.0, 0.5, 0.25, 0.5], "unpremul {sel}");
    }
    // a == 0: the scale is 0, not infinity.
    for (sel, s, _) in highp_known(Stage::Unpremul, [0.5, 0.25, 0.125, 0.0], [0.0; 4]) {
        assert_eq!(s, [0.0, 0.0, 0.0, 0.0], "unpremul a=0 {sel}");
    }
    for (sel, s, _) in highp_known(Stage::UnpremulPolar, [0.5, 0.25, 0.125, 0.5], [0.0; 4]) {
        assert_eq!(s, [0.5, 0.5, 0.25, 0.5], "unpremul_polar {sel}");
    }
    for (sel, s, _) in highp_known(Stage::Clamp01, [-1.0, 0.5, 2.0, 1.5], [0.0; 4]) {
        assert_eq!(s, [0.0, 0.5, 1.0, 1.0], "clamp_01 {sel}");
    }
    for (sel, s, _) in highp_known(Stage::ClampA01, [-1.0, 0.5, 2.0, 1.5], [0.0; 4]) {
        assert_eq!(s, [-1.0, 0.5, 2.0, 1.0], "clamp_a_01 {sel}");
    }
    for (sel, s, _) in highp_known(Stage::ClampGamut, [-1.0, 0.5, 2.0, 0.75], [0.0; 4]) {
        assert_eq!(s, [0.0, 0.5, 0.75, 0.75], "clamp_gamut {sel}");
    }
    for (sel, s, d) in highp_known(Stage::ForceOpaque, [0.1, 0.2, 0.3, 0.4], [0.5; 4]) {
        assert_eq!(
            (s, d),
            ([0.1, 0.2, 0.3, 1.0], [0.5; 4]),
            "force_opaque {sel}"
        );
    }
    for (sel, s, d) in highp_known(Stage::ForceOpaqueDst, [0.1, 0.2, 0.3, 0.4], [0.5; 4]) {
        assert_eq!(
            (s, d),
            ([0.1, 0.2, 0.3, 0.4], [0.5, 0.5, 0.5, 1.0]),
            "force_opaque_dst {sel}"
        );
    }
    let rgb = [0.25f32, 0.5, 2.0];
    for (stage, name) in [
        (Stage::SetRgb(&rgb), "set_rgb"),
        (Stage::UnboundedSetRgb(&rgb), "unbounded_set_rgb"),
    ] {
        for (sel, s, _) in highp_known(stage, [9.0; 4], [0.0; 4]) {
            assert_eq!(s, [0.25, 0.5, 2.0, 9.0], "{name} {sel}");
        }
    }
    let color = UniformColorCtx {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
        rgba: [26, 51, 77, 102],
    };
    for (stage, name) in [
        (Stage::UniformColor(&color), "uniform_color"),
        (
            Stage::UnboundedUniformColor(&color),
            "unbounded_uniform_color",
        ),
    ] {
        for (sel, s, d) in highp_known(stage, [9.0; 4], [7.0; 4]) {
            assert_eq!((s, d), ([0.1, 0.2, 0.3, 0.4], [7.0; 4]), "{name} {sel}");
        }
    }
    for (sel, s, d) in highp_known(Stage::UniformColorDst(&color), [9.0; 4], [7.0; 4]) {
        assert_eq!(
            (s, d),
            ([9.0; 4], [0.1, 0.2, 0.3, 0.4]),
            "uniform_color_dst {sel}"
        );
    }
    for (sel, s, _) in highp_known(Stage::BlackColor, [9.0; 4], [7.0; 4]) {
        assert_eq!(s, [0.0, 0.0, 0.0, 1.0], "black_color {sel}");
    }
    for (sel, s, _) in highp_known(Stage::WhiteColor, [9.0; 4], [7.0; 4]) {
        assert_eq!(s, [1.0; 4], "white_color {sel}");
    }
    for (sel, s, _) in highp_known(Stage::Swizzle(*b"bgr1"), [0.1, 0.2, 0.3, 0.4], [0.0; 4]) {
        assert_eq!(s, [0.3, 0.2, 0.1, 1.0], "swizzle bgr1 {sel}");
    }
    // 'x' leaves a channel alone; '0' zeroes it; 'a' reads the original alpha.
    for (sel, s, _) in highp_known(Stage::Swizzle(*b"0xag"), [0.1, 0.2, 0.3, 0.4], [0.0; 4]) {
        assert_eq!(s, [0.0, 0.2, 0.4, 0.2], "swizzle 0xag {sel}");
    }
    for (sel, s, _) in highp_known(
        Stage::Bt709LuminanceOrLumaToAlpha,
        [1.0, 1.0, 1.0, 0.5],
        [0.0; 4],
    ) {
        assert_eq!(s[..3], [0.0; 3], "bt709 to alpha {sel}");
        assert!((s[3] - 1.0).abs() < 1e-6, "bt709 to alpha {sel}: {}", s[3]);
    }
    for (sel, s, _) in highp_known(
        Stage::Bt709LuminanceOrLumaToRgb,
        [0.0, 1.0, 0.0, 0.5],
        [0.0; 4],
    ) {
        assert_eq!(s, [0.7152f32, 0.7152, 0.7152, 0.5], "bt709 to rgb {sel}");
    }
    // Matrices: identity and a permutation.
    let m3x3 = [0.0f32, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0];
    for (sel, s, _) in highp_known(Stage::Matrix3x3(&m3x3), [0.25, 0.5, 0.75, 1.0], [0.0; 4]) {
        // R = r*m0 + g*m3 + b*m6 = b; G = r*m1 + g*m4 + b*m7 = r; B = g.
        assert_eq!(s, [0.75, 0.25, 0.5, 1.0], "matrix_3x3 {sel}");
    }
    let m3x4 = [
        1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.5, 0.25, 0.125,
    ];
    for (sel, s, _) in highp_known(Stage::Matrix3x4(&m3x4), [0.25, 0.5, 0.75, 1.0], [0.0; 4]) {
        assert_eq!(s, [0.75, 0.75, 0.875, 1.0], "matrix_3x4 {sel}");
    }
    let mut m4x5 = [0.0f32; 20];
    m4x5[0] = 1.0;
    m4x5[4] = 0.25; // R = r + 0.25
    m4x5[6] = 2.0; // G = 2g
    m4x5[12] = 1.0; // B = b
    m4x5[19] = 0.5; // A = 0.5
    for (sel, s, _) in highp_known(Stage::Matrix4x5(&m4x5), [0.25, 0.5, 0.75, 1.0], [0.0; 4]) {
        assert_eq!(s, [0.5, 1.0, 0.75, 0.5], "matrix_4x5 {sel}");
    }
    let mut m4x3 = [0.0f32; 12];
    m4x3[0] = 1.0; // r' = x
    m4x3[5] = 1.0; // g' = y
    m4x3[10] = 0.5; // b' = 0.5
    m4x3[7] = 2.0; // a' = 2y + 1
    m4x3[11] = 1.0;
    for (sel, s, _) in highp_known(Stage::Matrix4x3(&m4x3), [0.25, 0.5, 0.75, 1.0], [0.0; 4]) {
        assert_eq!(s, [0.25, 0.5, 0.5, 2.0], "matrix_4x3 {sel}");
    }
    for (sel, s, _) in highp_known(Stage::RgbToHsl, [0.0, 0.0, 1.0, 0.5], [0.0; 4]) {
        // Pure blue: hue 4/6 (1/6 * 4), saturation 1, lightness 0.5.
        assert!((s[0] - 2.0 / 3.0).abs() < 1e-6, "rgb_to_hsl {sel}: {s:?}");
        assert_eq!((s[1], s[2], s[3]), (1.0, 0.5, 0.5), "rgb_to_hsl {sel}");
    }
    for (sel, s, _) in highp_known(Stage::HslToRgb, [0.0, 1.0, 0.5, 0.5], [0.0; 4]) {
        // Hue 0, full saturation, mid lightness: pure red.
        for (got, want) in s.into_iter().zip([1.0, 0.0, 0.0, 0.5]) {
            assert!((got - want).abs() < 1e-6, "hsl_to_rgb {sel}: {s:?}");
        }
    }
    for (sel, s, _) in highp_known(Stage::GaussAToRgba, [0.0, 0.0, 0.0, 0.0], [0.0; 4]) {
        assert_eq!(s, [0.00030726194381713867f32; 4], "gauss_a_to_rgba {sel}");
    }
    // The transfer functions: gamma 1 is the identity (powf(x, 1) goes through the
    // approximation, so only the fixed points 0 and 1, which `approx_powf` passes through).
    for (sel, s, _) in highp_known(Stage::Gamma(2.0), [0.0, 1.0, 0.0, 0.5], [0.0; 4]) {
        assert_eq!(s, [0.0, 1.0, 0.0, 0.5], "gamma_ {sel}");
    }
    let srgb = TransferFunction {
        g: 2.4,
        a: 1.0 / 1.055,
        b: 0.055 / 1.055,
        c: 1.0 / 12.92,
        d: 0.04045,
        e: 0.0,
        f: 0.0,
    };
    for (sel, s, _) in highp_known(Stage::Parametric(&srgb), [0.0, 1.0, 0.02, 0.5], [0.0; 4]) {
        assert_eq!(s[0], 0.0, "parametric {sel}");
        assert!((s[1] - 1.0).abs() < 1e-3, "parametric {sel}: {s:?}");
        assert!(
            (s[2] - 0.02 / 12.92).abs() < 1e-6,
            "parametric {sel}: {s:?}"
        );
        assert_eq!(s[3], 0.5, "parametric {sel}");
    }
    // A negative input is transformed as its magnitude and keeps its sign.
    for (sel, s, _) in highp_known(Stage::Parametric(&srgb), [-0.02, 0.0, 0.0, 0.5], [0.0; 4]) {
        assert!(
            (s[0] + 0.02 / 12.92).abs() < 1e-6,
            "parametric sign {sel}: {s:?}"
        );
    }
}

#[test]
fn highp_tables_and_emboss() {
    let mut r = [0u8; 256];
    let mut g = [0u8; 256];
    let mut b = [0u8; 256];
    let mut a = [0u8; 256];
    for i in 0..256 {
        r[i] = 255 - i as u8;
        g[i] = i as u8;
        b[i] = (i as u8) / 2;
        a[i] = 7;
    }
    let tables = TablesCtx { r, g, b, a };
    for (sel, s, _) in highp_known(Stage::ByteTables(&tables), [0.0, 1.0, 1.0, 0.5], [0.0; 4]) {
        assert_eq!(s[0], 255.0 * (1.0 / 255.0), "byte_tables {sel}");
        assert_eq!(s[1], 255.0 * (1.0 / 255.0), "byte_tables {sel}");
        assert_eq!(s[2], 127.0 * (1.0 / 255.0), "byte_tables {sel}");
        assert_eq!(s[3], 7.0 * (1.0 / 255.0), "byte_tables {sel}");
    }

    // emboss: r = r*mul + add, with mul/add bytes per pixel (including a tail run).
    for sel in selections() {
        let n = sel.tier.highp_stride();
        let mul: Vec<u8> = (0..n).map(|i| (255 - 16 * (i % 16)) as u8).collect();
        let add: Vec<u8> = (0..n).map(|i| (3 * i) as u8).collect();
        for w in [n, n.div_ceil(2)] {
            let (_, out) = run_stage(
                Stage::Emboss(EMBOSS),
                sel,
                true,
                (0, 0),
                w,
                (&splat_regs(n, [0.5, 1.0, 0.25, 1.0]), &[]),
                (&mul, &add),
            );
            let f = floats(&out.src[..16 * n]);
            for i in 0..w {
                let (m, ad) = (
                    f32::from(mul[i]) * (1.0 / 255.0),
                    f32::from(add[i]) * (1.0 / 255.0),
                );
                for (c, v) in [0.5f32, 1.0, 0.25].into_iter().enumerate() {
                    // Unfused on Scalar/Sse*, fused on Ml3/Ml4/Neon.
                    let want = if matches!(sel.tier, Tier::Scalar | Tier::Sse2 | Tier::Sse41) {
                        ad + v * m
                    } else {
                        v.mul_add(m, ad)
                    };
                    assert_eq!(
                        f[c * n + i],
                        want,
                        "emboss {sel} w={w} lane {i} channel {c}"
                    );
                }
                assert_eq!(f[3 * n + i], 1.0, "emboss {sel} alpha");
            }
        }
    }
}

/// Lowp known answers on every lowp selection.
#[test]
fn lowp_known_answers() {
    let run =
        |stage: Stage<'_>, src: [u16; 4], dst: [u16; 4]| -> Vec<(Selection, [u16; 4], [u16; 4])> {
            sample_selections()
                .into_iter()
                .filter(|s| s.tier.lowp_stride().is_some())
                .map(|sel| {
                    let n = sel.tier.lowp_stride().unwrap();
                    let (lowp, out) = run_stage(
                        stage,
                        sel,
                        false,
                        (0, 0),
                        n,
                        (&splat_regs_lowp(n, src), &splat_regs_lowp(n, dst)),
                        (&[], &[]),
                    );
                    assert!(lowp, "{sel}: a lowp stage ran in a highp program");
                    (sel, lane0_lowp(&out.src, n), lane0_lowp(&out.dst, n))
                })
                .collect()
        };
    for (sel, s, _) in run(Stage::Premul, [255, 128, 64, 128], [0; 4]) {
        // div255_accurate(255*128) = 128, (128*128) = 64, (64*128) = 32.
        assert_eq!(s, [128, 64, 32, 128], "premul {sel}");
    }
    for (sel, _, d) in run(Stage::PremulDst, [0; 4], [255, 128, 64, 128]) {
        assert_eq!(d, [128, 64, 32, 128], "premul_dst {sel}");
    }
    for (sel, s, _) in run(Stage::Clamp01, [0, 255, 300, 1000], [0; 4]) {
        assert_eq!(s, [0, 255, 255, 255], "clamp_01 {sel}");
    }
    for (sel, s, _) in run(Stage::ClampA01, [0, 255, 300, 1000], [0; 4]) {
        assert_eq!(s, [0, 255, 300, 255], "clamp_a_01 {sel}");
    }
    for (sel, s, _) in run(Stage::ClampGamut, [10, 255, 300, 200], [0; 4]) {
        assert_eq!(s, [10, 200, 200, 200], "clamp_gamut {sel}");
    }
    for (sel, s, d) in run(Stage::ForceOpaque, [1, 2, 3, 4], [5, 6, 7, 8]) {
        assert_eq!((s, d), ([1, 2, 3, 255], [5, 6, 7, 8]), "force_opaque {sel}");
    }
    for (sel, s, d) in run(Stage::ForceOpaqueDst, [1, 2, 3, 4], [5, 6, 7, 8]) {
        assert_eq!(
            (s, d),
            ([1, 2, 3, 4], [5, 6, 7, 255]),
            "force_opaque_dst {sel}"
        );
    }
    let color = UniformColorCtx {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
        rgba: [26, 51, 77, 102],
    };
    for (sel, s, d) in run(Stage::UniformColor(&color), [9; 4], [7; 4]) {
        assert_eq!((s, d), ([26, 51, 77, 102], [7; 4]), "uniform_color {sel}");
    }
    for (sel, s, d) in run(Stage::UniformColorDst(&color), [9; 4], [7; 4]) {
        assert_eq!(
            (s, d),
            ([9; 4], [26, 51, 77, 102]),
            "uniform_color_dst {sel}"
        );
    }
    for (sel, s, _) in run(Stage::BlackColor, [9; 4], [7; 4]) {
        assert_eq!(s, [0, 0, 0, 255], "black_color {sel}");
    }
    for (sel, s, _) in run(Stage::WhiteColor, [9; 4], [7; 4]) {
        assert_eq!(s, [255; 4], "white_color {sel}");
    }
    // `from_float(f) = (uint16_t)(f * 255 + 0.5)`.
    let rgb = [0.0f32, 0.5, 1.0];
    for (sel, s, _) in run(Stage::SetRgb(&rgb), [9; 4], [7; 4]) {
        assert_eq!(s, [0, 128, 255, 9], "set_rgb {sel}");
    }
    for (sel, s, _) in run(Stage::Swizzle(*b"bgr1"), [10, 20, 30, 40], [0; 4]) {
        assert_eq!(s, [30, 20, 10, 255], "swizzle {sel}");
    }
    for (sel, s, _) in run(Stage::Swizzle(*b"0xag"), [10, 20, 30, 40], [0; 4]) {
        assert_eq!(s, [0, 20, 40, 20], "swizzle {sel}");
    }
    // (r*54 + g*183 + b*19) / 256.
    for (sel, s, _) in run(Stage::Bt709LuminanceOrLumaToRgb, [255, 255, 255, 7], [0; 4]) {
        assert_eq!(s, [255, 255, 255, 7], "bt709 rgb {sel}");
    }
    for (sel, s, _) in run(
        Stage::Bt709LuminanceOrLumaToAlpha,
        [255, 255, 255, 7],
        [0; 4],
    ) {
        assert_eq!(s, [0, 0, 0, 255], "bt709 alpha {sel}");
    }
    // emboss: min(div255(r*mul) + add, a).
    for sel in selections()
        .into_iter()
        .filter(|s| s.tier.lowp_stride().is_some())
    {
        let n = sel.tier.lowp_stride().unwrap();
        let mul: Vec<u8> = (0..n).map(|i| (255 - 8 * i) as u8).collect();
        let add: Vec<u8> = (0..n).map(|i| (5 * i) as u8).collect();
        for w in [n, n.div_ceil(2)] {
            let (lowp, out) = run_stage(
                Stage::Emboss(EMBOSS),
                sel,
                false,
                (0, 0),
                w,
                (&splat_regs_lowp(n, [200, 100, 50, 220]), &[]),
                (&mul, &add),
            );
            assert!(lowp);
            let h = halves(&out.src[..8 * n]);
            for i in 0..w {
                for (c, v) in [200u16, 100, 50].into_iter().enumerate() {
                    let p = v * u16::from(mul[i]);
                    // div255: Neon's is exact, x86's is (v + 255) / 256.
                    let d = if sel.tier == Tier::Neon {
                        (p + (p + 128) / 256 + 128) / 256
                    } else {
                        p.div_ceil(256)
                    };
                    let want = (d + u16::from(add[i])).min(220);
                    assert_eq!(
                        h[c * n + i],
                        want,
                        "emboss {sel} w={w} lane {i} channel {c}"
                    );
                }
            }
        }
    }
}

/// Random lane bits: mostly special floats, some random patterns.
fn random_lanes(rng: &mut Rng, specials: &[u32], count: usize) -> Vec<u32> {
    (0..count)
        .map(|_| {
            if rng.below(4) == 0 {
                rng.next_u32()
            } else {
                rng.pick(specials)
            }
        })
        .collect()
}

/// Mostly colors in `[0, 1]`, then special floats: the interesting domain of color stages.
fn color_lanes(rng: &mut Rng, specials: &[u32], count: usize) -> Vec<u32> {
    (0..count)
        .map(|_| match rng.below(4) {
            0 => rng.pick(specials),
            1 => (rng.next_u32() as f32 / u32::MAX as f32 * 3.0 - 1.0).to_bits(),
            _ => (rng.next_u32() as f32 / u32::MAX as f32).to_bits(),
        })
        .collect()
}

fn bytes(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

/// Two NaNs, or the same bits.
fn same(a: u32, b: u32) -> bool {
    a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

#[test]
fn highp_color_stage_twins() {
    let specials = float_specials();
    let color = UniformColorCtx {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
        rgba: [26, 51, 77, 102],
    };
    let rgb = [0.25f32, 0.5, 2.0];
    let m3x3 = [0.3f32, -0.2, 0.9, 1.5, 0.1, -0.7, 0.25, 0.5, 2.0];
    let m12 = [
        0.3f32, -0.2, 0.9, 1.5, 0.1, -0.7, 0.25, 0.5, 2.0, 0.125, -0.5, 0.75,
    ];
    let m20: [f32; 20] = core::array::from_fn(|i| (i as f32 - 7.0) / 5.0);
    let srgb = TransferFunction {
        g: 2.4,
        a: 1.0 / 1.055,
        b: 0.055 / 1.055,
        c: 1.0 / 12.92,
        d: 0.04045,
        e: 0.1,
        f: 0.02,
    };
    let pq = TransferFunction {
        g: 0.0,
        a: -107.0 / 128.0,
        b: 1.0,
        c: 32.0 / 2523.0,
        d: 2413.0 / 128.0,
        e: -2392.0 / 128.0,
        f: 8192.0 / 1305.0,
    };
    let hlg = TransferFunction {
        g: 0.0,
        a: 2.0,
        b: 2.0,
        c: 1.0 / 0.17883277,
        d: 0.28466892,
        e: 0.55991073,
        f: 0.0,
    };
    let hlg_inv = TransferFunction {
        g: 0.0,
        a: 0.5,
        b: 0.5,
        c: 0.17883277,
        d: 0.28466892,
        e: 0.55991073,
        f: 0.0,
    };
    let ootf = [0.2627f32, 0.6780, 0.0593, 0.2];
    let (mut r, mut g, mut b, mut a) = ([0u8; 256], [0u8; 256], [0u8; 256], [0u8; 256]);
    for i in 0..256 {
        r[i] = (255 - i) as u8;
        g[i] = (i * 7) as u8;
        b[i] = (i / 3) as u8;
        a[i] = (i ^ 0x55) as u8;
    }
    let tables = TablesCtx { r, g, b, a };
    let stages: Vec<(&str, Stage<'_>)> = vec![
        ("premul", Stage::Premul),
        ("premul_dst", Stage::PremulDst),
        ("unpremul", Stage::Unpremul),
        ("unpremul_polar", Stage::UnpremulPolar),
        ("clamp_01", Stage::Clamp01),
        ("clamp_a_01", Stage::ClampA01),
        ("clamp_gamut", Stage::ClampGamut),
        ("force_opaque", Stage::ForceOpaque),
        ("force_opaque_dst", Stage::ForceOpaqueDst),
        ("set_rgb", Stage::SetRgb(&rgb)),
        ("unbounded_set_rgb", Stage::UnboundedSetRgb(&rgb)),
        ("black_color", Stage::BlackColor),
        ("white_color", Stage::WhiteColor),
        ("uniform_color", Stage::UniformColor(&color)),
        ("uniform_color_dst", Stage::UniformColorDst(&color)),
        (
            "unbounded_uniform_color",
            Stage::UnboundedUniformColor(&color),
        ),
        ("bt709_alpha", Stage::Bt709LuminanceOrLumaToAlpha),
        ("bt709_rgb", Stage::Bt709LuminanceOrLumaToRgb),
        ("swizzle", Stage::Swizzle(*b"a0rg")),
        ("swizzle2", Stage::Swizzle(*b"1xbr")),
        ("dither", Stage::Dither(1.0 / 255.0)),
        ("matrix_3x3", Stage::Matrix3x3(&m3x3)),
        ("matrix_3x4", Stage::Matrix3x4(&m12)),
        ("matrix_4x5", Stage::Matrix4x5(&m20)),
        ("matrix_4x3", Stage::Matrix4x3(&m12)),
        ("parametric", Stage::Parametric(&srgb)),
        ("gamma_", Stage::Gamma(2.2)),
        ("PQish", Stage::PQish(&pq)),
        ("HLGish", Stage::HLGish(&hlg)),
        ("HLGinvish", Stage::HLGinvish(&hlg_inv)),
        ("ootf", Stage::Ootf(&ootf)),
        ("rgb_to_hsl", Stage::RgbToHsl),
        ("hsl_to_rgb", Stage::HslToRgb),
        ("css_lab_to_xyz", Stage::CssLabToXyz),
        ("css_oklab_to_linear_srgb", Stage::CssOklabToLinearSrgb),
        (
            "css_oklab_gamut_map_to_linear_srgb",
            Stage::CssOklabGamutMapToLinearSrgb,
        ),
        ("css_hcl_to_lab", Stage::CssHclToLab),
        ("css_hsl_to_srgb", Stage::CssHslToSrgb),
        ("css_hwb_to_srgb", Stage::CssHwbToSrgb),
        ("gauss_a_to_rgba", Stage::GaussAToRgba),
        ("byte_tables", Stage::ByteTables(&tables)),
        ("emboss", Stage::Emboss(EMBOSS)),
    ];
    for (native, models) in twin_sets() {
        let n = native.tier.highp_stride();
        let mut rng = Rng::new(0x00b4_c010);
        for round in 0..200 {
            let (src, dst) = if round % 2 == 0 {
                (
                    color_lanes(&mut rng, &specials, 4 * n),
                    color_lanes(&mut rng, &specials, 4 * n),
                )
            } else {
                (
                    random_lanes(&mut rng, &specials, 4 * n),
                    random_lanes(&mut rng, &specials, 4 * n),
                )
            };
            let (mut src, mut dst) = (src, dst);
            thin_nans(&mut [&mut src, &mut dst], n, true);
            let (sb, db) = (bytes(&src), bytes(&dst));
            let mul: Vec<u8> = (0..n).map(|_| rng.next_u32() as u8).collect();
            let add: Vec<u8> = (0..n).map(|_| rng.next_u32() as u8).collect();
            let w = if round % 3 == 0 { n } else { 1 + rng.below(n) };
            // `dither` and the memory stages depend on the position: the same for every run.
            for (name, stage) in &stages {
                // `dither` depends on the position; the memory stage reads at (0, 0).
                let at = if *name == "emboss" {
                    (0, 0)
                } else {
                    (rng.below(1 << 20), rng.below(1 << 20))
                };
                let (_, want) = run_stage(*stage, native, true, at, w, (&sb, &db), (&mul, &add));
                for model in &models {
                    let (_, got) = run_stage(*stage, *model, true, at, w, (&sb, &db), (&mul, &add));
                    for (img_want, img_got, what) in
                        [(&want.src, &got.src, "src"), (&want.dst, &got.dst, "dst")]
                    {
                        let (a, b) = (words(&img_want[..16 * n]), words(&img_got[..16 * n]));
                        for i in 0..4 * n {
                            // Lanes past the tail hold whatever the registers did; compare all
                            // the same: both sides run the same code on the same inputs.
                            assert!(
                                same(a[i], b[i]),
                                "{name} {native} vs {model}, round {round}, w {w}, {what} word {i}: {:08x} vs {:08x}",
                                a[i],
                                b[i]
                            );
                        }
                    }
                }
            }
        }
    }
}

fn words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect()
}

#[test]
fn lowp_color_stage_twins() {
    let color = UniformColorCtx {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
        rgba: [26, 51, 77, 102],
    };
    let rgb = [0.0f32, 0.3, 1.0];
    let stages: Vec<(&str, Stage<'_>)> = vec![
        ("premul", Stage::Premul),
        ("premul_dst", Stage::PremulDst),
        ("clamp_01", Stage::Clamp01),
        ("clamp_a_01", Stage::ClampA01),
        ("clamp_gamut", Stage::ClampGamut),
        ("force_opaque", Stage::ForceOpaque),
        ("force_opaque_dst", Stage::ForceOpaqueDst),
        ("set_rgb", Stage::SetRgb(&rgb)),
        ("black_color", Stage::BlackColor),
        ("white_color", Stage::WhiteColor),
        ("uniform_color", Stage::UniformColor(&color)),
        ("uniform_color_dst", Stage::UniformColorDst(&color)),
        ("bt709_alpha", Stage::Bt709LuminanceOrLumaToAlpha),
        ("bt709_rgb", Stage::Bt709LuminanceOrLumaToRgb),
        ("swizzle", Stage::Swizzle(*b"a0rg")),
        ("swizzle2", Stage::Swizzle(*b"1xbr")),
        ("emboss", Stage::Emboss(EMBOSS)),
    ];
    for (native, models) in twin_sets() {
        let n = native.tier.lowp_stride().unwrap();
        let mut rng = Rng::new(0x10b4);
        for round in 0..200 {
            let lanes = |rng: &mut Rng| -> Vec<u8> {
                (0..4 * n)
                    .flat_map(|_| {
                        // Mostly [0, 255] (colors), sometimes any 16 bits.
                        let v = if rng.below(4) == 0 {
                            rng.next_u32() as u16
                        } else {
                            (rng.next_u32() & 0xFF) as u16
                        };
                        v.to_ne_bytes()
                    })
                    .collect()
            };
            let (sb, db) = (lanes(&mut rng), lanes(&mut rng));
            let mul: Vec<u8> = (0..n).map(|_| rng.next_u32() as u8).collect();
            let add: Vec<u8> = (0..n).map(|_| rng.next_u32() as u8).collect();
            let w = if round % 3 == 0 { n } else { 1 + rng.below(n) };
            for (name, stage) in &stages {
                let (lowp, want) =
                    run_stage(*stage, native, false, (0, 0), w, (&sb, &db), (&mul, &add));
                assert!(lowp, "{name} {native}");
                for model in &models {
                    let (_, got) =
                        run_stage(*stage, *model, false, (0, 0), w, (&sb, &db), (&mul, &add));
                    assert_eq!(
                        want.src[..8 * n],
                        got.src[..8 * n],
                        "{name} {native} vs {model}, round {round}, w {w}"
                    );
                    assert_eq!(
                        want.dst[..8 * n],
                        got.dst[..8 * n],
                        "{name} {native} vs {model}, round {round}, w {w}"
                    );
                }
            }
        }
    }
}
