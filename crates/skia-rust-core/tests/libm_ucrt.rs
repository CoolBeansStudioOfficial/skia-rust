// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `skia_rust_core::libm` against the Universal CRT, the C library of the oracle host.
//!
//! On Windows x64 the platform functions behind `f32::sin` & co. *are* the UCRT, so there the test
//! compares every function bit for bit on a large deterministic input set (including the exact
//! arguments of the GMs that exposed the host dependence). On every host it also checks a digest of
//! the same results, so a host that computes anything differently (a miscompiled `mul_add`, a
//! changed table) fails too. See `docs/design/math.md`.

// The point of this file is to call the platform libm.
#![allow(clippy::disallowed_methods)]

use skia_rust_core::libm;

/// Deterministic xorshift64 inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    #[allow(clippy::cast_possible_truncation)] // the low 32 bits
    fn bits32(&mut self) -> u32 {
        self.next() as u32
    }

    /// Uniform in `[lo, hi)`.
    #[allow(clippy::cast_precision_loss)] // 24 random bits fit a float
    fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        let u = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * u
    }

    /// Uniform in `[lo, hi)` as a double.
    #[allow(clippy::cast_precision_loss)] // 53 random bits fit a double
    fn uniform64(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * u
    }
}

/// How many inputs of each kind: release builds (CI runs those too) check many more.
const COUNT: usize = if cfg!(debug_assertions) {
    1 << 14
} else {
    1 << 20
};

const SPECIALS_F32: [f32; 20] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    -0.5,
    2.0,
    f32::MIN_POSITIVE,
    1.0e-45,
    -1.0e-45,
    f32::MAX,
    f32::MIN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    std::f32::consts::PI,
    std::f32::consts::FRAC_PI_2,
    std::f32::consts::FRAC_PI_4,
    1.0e10,
    -3.0e38,
];

/// The float inputs: specials, random bit patterns, and values in the ranges geometry uses.
fn inputs_f32(seed: u64) -> Vec<f32> {
    let mut rng = Rng(seed);
    let mut v = SPECIALS_F32.to_vec();
    for _ in 0..COUNT {
        v.push(f32::from_bits(rng.bits32()));
        v.push(rng.uniform(-8.0, 8.0));
        v.push(rng.uniform(-1.0, 1.0));
        v.push(rng.uniform(-1000.0, 1000.0));
        v.push(rng.uniform(0.0, 0.1));
    }
    v
}

/// The double inputs.
fn inputs_f64(seed: u64) -> Vec<f64> {
    let mut rng = Rng(seed);
    let mut v: Vec<f64> = SPECIALS_F32.iter().map(|&x| f64::from(x)).collect();
    v.extend([
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        1e300,
        -1e-300,
        710.0,
        -745.5,
        1024.5,
    ]);
    for _ in 0..COUNT {
        v.push(f64::from_bits(rng.next()));
        v.push(rng.uniform64(-8.0, 8.0));
        v.push(rng.uniform64(-1.0, 1.0));
        v.push(rng.uniform64(-1100.0, 1100.0));
        // `a + 3.141592 / 3` as in longpathdash.
        v.push(f64::from(rng.uniform(-7.0, 7.0)) + f64::from_bits(0x3ff0_c151_fdb2_0051));
    }
    v
}

/// The arguments the arcto, parsedpaths and longpathdash GMs pass.
fn gm_args() -> Vec<(String, u64, u64)> {
    include_str!("data/libm_gm_args.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next().unwrap().to_string();
            let a = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let b = it.next().map_or(0, |s| u64::from_str_radix(s, 16).unwrap());
            (name, a, b)
        })
        .collect()
}

type F32Fn = fn(f32) -> f32;
type F64Fn = fn(f64) -> f64;

const F32_FNS: [(&str, F32Fn, F32Fn); 10] = [
    ("sinf", libm::sinf, f32::sin),
    ("cosf", libm::cosf, f32::cos),
    ("tanf", libm::tanf, f32::tan),
    ("asinf", libm::asinf, f32::asin),
    ("acosf", libm::acosf, f32::acos),
    ("atanf", libm::atanf, f32::atan),
    ("expf", libm::expf, f32::exp),
    ("logf", libm::logf, f32::ln),
    ("log2f", libm::log2f, f32::log2),
    ("cbrtf", libm::cbrtf, f32::cbrt),
];

const F64_FNS: [(&str, F64Fn, F64Fn); 7] = [
    ("sin", libm::sin, f64::sin),
    ("cos", libm::cos, f64::cos),
    ("acos", libm::acos, f64::acos),
    ("cbrt", libm::cbrt, f64::cbrt),
    ("exp", libm::exp, f64::exp),
    ("log", libm::log, f64::ln),
    ("exp2", libm::exp2, f64::exp2),
];

/// FNV-1a over result bits; NaNs hash as one value (payloads are not part of the contract).
#[derive(Default)]
struct Digest(u64);

impl Digest {
    fn add(&mut self, bits: u64) {
        let mut h = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for b in bits.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }
    fn add32(&mut self, v: f32) {
        self.add(if v.is_nan() {
            u64::MAX
        } else {
            u64::from(v.to_bits())
        });
    }
    fn add64(&mut self, v: f64) {
        self.add(if v.is_nan() { u64::MAX } else { v.to_bits() });
    }
}

/// Runs every function over the inputs; `check` sees (name, input, ours, platform) for each result.
fn run(mut check: impl FnMut(&str, String, u64, u64)) -> u64 {
    let mut digest = Digest::default();
    let xs = inputs_f32(0x9e37_79b9_7f4a_7c15);
    for (name, ours, platform) in F32_FNS {
        for &x in &xs {
            let a = ours(x);
            digest.add32(a);
            let b = platform(std::hint::black_box(x));
            check(
                name,
                format!("{:#010x}", x.to_bits()),
                u64::from(a.to_bits()),
                u64::from(b.to_bits()),
            );
        }
    }
    let ys = inputs_f32(0x0123_4567_89ab_cdef);
    for (&y, &x) in ys.iter().zip(&xs) {
        let a = libm::atan2f(y, x);
        digest.add32(a);
        let b = std::hint::black_box(y).atan2(std::hint::black_box(x));
        let args = format!("{:#010x}, {:#010x}", y.to_bits(), x.to_bits());
        check(
            "atan2f",
            args,
            u64::from(a.to_bits()),
            u64::from(b.to_bits()),
        );
        let a = libm::powf(x.abs(), y);
        digest.add32(a);
        let b = std::hint::black_box(x.abs()).powf(std::hint::black_box(y));
        let args = format!("{:#010x}, {:#010x}", x.abs().to_bits(), y.to_bits());
        check("powf", args, u64::from(a.to_bits()), u64::from(b.to_bits()));
        let a = libm::powf(x, y.round());
        digest.add32(a);
        let b = std::hint::black_box(x).powf(std::hint::black_box(y.round()));
        let args = format!("{:#010x}, {:#010x}", x.to_bits(), y.round().to_bits());
        check("powf", args, u64::from(a.to_bits()), u64::from(b.to_bits()));
    }
    let xs = inputs_f64(0x5851_f42d_4c95_7f2d);
    for (name, ours, platform) in F64_FNS {
        for &x in &xs {
            let a = ours(x);
            digest.add64(a);
            let b = platform(std::hint::black_box(x));
            check(
                name,
                format!("{:#018x}", x.to_bits()),
                a.to_bits(),
                b.to_bits(),
            );
        }
    }
    let ys = inputs_f64(0x2545_f491_4f6c_dd1d);
    for (&y, &x) in ys.iter().zip(&xs) {
        let a = libm::pow(x.abs(), y);
        digest.add64(a);
        let b = std::hint::black_box(x.abs()).powf(std::hint::black_box(y));
        let args = format!("{:#018x}, {:#018x}", x.abs().to_bits(), y.to_bits());
        check("pow", args, a.to_bits(), b.to_bits());
    }
    for (name, a, b) in gm_args() {
        #[allow(clippy::cast_possible_truncation)] // float arguments are stored as 32-bit patterns
        let (fa, fb) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
        let (ours, platform) = match name.as_str() {
            "sinf" => (
                u64::from(libm::sinf(fa).to_bits()),
                u64::from(fa.sin().to_bits()),
            ),
            "cosf" => (
                u64::from(libm::cosf(fa).to_bits()),
                u64::from(fa.cos().to_bits()),
            ),
            "tanf" => (
                u64::from(libm::tanf(fa).to_bits()),
                u64::from(fa.tan().to_bits()),
            ),
            "atan2f" => (
                u64::from(libm::atan2f(fa, fb).to_bits()),
                u64::from(std::hint::black_box(fa).atan2(fb).to_bits()),
            ),
            "sin" => {
                let x = f64::from_bits(a);
                (
                    libm::sin(x).to_bits(),
                    std::hint::black_box(x).sin().to_bits(),
                )
            }
            "cos" => {
                let x = f64::from_bits(a);
                (
                    libm::cos(x).to_bits(),
                    std::hint::black_box(x).cos().to_bits(),
                )
            }
            _ => panic!("unknown function {name} in libm_gm_args.txt"),
        };
        digest.add(ours);
        check(&name, format!("{a:#x}, {b:#x}"), ours, platform);
    }
    digest.0
}

/// NaN results agree when both are NaN (payloads are not compared).
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn same(a: u64, b: u64) -> bool {
    if a == b {
        return true;
    }
    let nan32 =
        |v: u64| v <= u64::from(u32::MAX) && f32::from_bits(u32::try_from(v).unwrap()).is_nan();
    (nan32(a) && nan32(b)) || (f64::from_bits(a).is_nan() && f64::from_bits(b).is_nan())
}

/// The digest of [`run`] as the UCRT computes it (see `ucrt_agreement`, which proves it on Windows).
const EXPECTED_DIGEST: [u64; 2] = [0xb8ea_f11d_3e4e_42ee, 0xd0fa_892e_1a14_2260];

#[test]
fn digest_matches_ucrt() {
    let digest = run(|_, _, _, _| {});
    let expected = EXPECTED_DIGEST[usize::from(!cfg!(debug_assertions))];
    assert_eq!(
        digest, expected,
        "libm results differ from the UCRT's: {digest:#018x}"
    );
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[test]
fn ucrt_agreement() {
    let mut mismatches = Vec::new();
    let mut total = 0usize;
    run(|name, args, ours, platform| {
        total += 1;
        if !same(ours, platform) && mismatches.len() < 50 {
            mismatches.push(format!(
                "{name}({args}): ours {ours:#x}, UCRT {platform:#x}"
            ));
        }
    });
    assert!(
        mismatches.is_empty(),
        "{} of {total} differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
