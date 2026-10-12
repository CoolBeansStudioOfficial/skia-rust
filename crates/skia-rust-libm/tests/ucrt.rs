//! Cross-host digests and the Windows-x64 bit-for-bit comparison against the platform UCRT.
//!
//! The digests pin the module's outputs on every host (the module is pure Rust, so they do not
//! depend on the host libm). On Windows x64 the same functions are also compared bit for bit with
//! the platform functions, which is what ties the digests to the UCRT; the exhaustive 2^32 version
//! of that comparison is the `libm-cleanroom-probe` workflow (docs/design/libm-cleanroom.md).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names
)]

use skia_rust_libm::{atan2f, atanf, cosf, sinf, tanf};

/// splitmix64, so the input set is identical everywhere.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

const N: usize = 1 << 18;

/// Inputs: arbitrary bit patterns (finite or not), log-uniform magnitudes of both signs, and a
/// sweep of the small range where the approximations change shape.
fn unary_inputs() -> Vec<f32> {
    let mut r = Rng(0x5EED_0001);
    let mut v = Vec::with_capacity(3 * N);
    for _ in 0..N {
        v.push(f32::from_bits(r.next() as u32));
    }
    for _ in 0..N {
        let e = (r.next() % 80) as i32 - 40;
        let m = 1.0 + (r.next() >> 40) as f32 / (1u64 << 24) as f32;
        let s = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
        v.push(s * m * (e as f32).exp2());
    }
    for i in 0..N {
        v.push((i as f32 - (N / 2) as f32) * (1.0 / 4096.0));
    }
    v
}

fn binary_inputs() -> Vec<(f32, f32)> {
    let mut r = Rng(0x5EED_0002);
    let mut v = Vec::with_capacity(2 * N);
    for _ in 0..N {
        v.push((
            f32::from_bits(r.next() as u32),
            f32::from_bits(r.next() as u32),
        ));
    }
    for _ in 0..N {
        let f = |r: &mut Rng| {
            let e = (r.next() % 30) as i32 - 15;
            let m = 1.0 + (r.next() >> 40) as f32 / (1u64 << 24) as f32;
            let s = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
            s * m * (e as f32).exp2()
        };
        v.push((f(&mut r), f(&mut r)));
    }
    v
}

/// FNV-1a over the result bit patterns.
fn fnv(bits: impl Iterator<Item = u32>) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for b in bits {
        for k in 0..4 {
            h ^= u64::from((b >> (8 * k)) & 0xFF);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

fn unary_digest(f: fn(f32) -> f32) -> u64 {
    fnv(unary_inputs().into_iter().map(|x| f(x).to_bits()))
}

fn atan2_digest() -> u64 {
    fnv(binary_inputs()
        .into_iter()
        .map(|(y, x)| atan2f(y, x).to_bits()))
}

#[test]
fn digests_are_stable_on_every_host() {
    assert_eq!(unary_digest(sinf), SINF_DIGEST, "sinf");
    assert_eq!(unary_digest(cosf), COSF_DIGEST, "cosf");
    assert_eq!(unary_digest(tanf), TANF_DIGEST, "tanf");
    assert_eq!(unary_digest(atanf), ATANF_DIGEST, "atanf");
    assert_eq!(atan2_digest(), ATAN2F_DIGEST, "atan2f");
}

const SINF_DIGEST: u64 = 0x7ff8_cb20_4783_d104;
const COSF_DIGEST: u64 = 0x6a0f_74a4_68e2_1b02;
const TANF_DIGEST: u64 = 0x6348_c129_c117_df4f;
const ATANF_DIGEST: u64 = 0x7ae5_1753_a4a4_74d4;
const ATAN2F_DIGEST: u64 = 0xbe9c_de75_a524_f51a;

#[test]
#[ignore = "prints the digests to paste into the constants above"]
fn print_digests() {
    println!("SINF {:#018x}", unary_digest(sinf));
    println!("COSF {:#018x}", unary_digest(cosf));
    println!("TANF {:#018x}", unary_digest(tanf));
    println!("ATANF {:#018x}", unary_digest(atanf));
    println!("ATAN2F {:#018x}", atan2_digest());
}

/// On Windows x64 `std` calls the UCRT: the module must agree with it bit for bit.
#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
#[allow(clippy::disallowed_methods)] // the platform functions are the reference being matched
fn bit_identical_to_the_platform_ucrt() {
    let check = |name: &str, ours: fn(f32) -> f32, platform: fn(f32) -> f32| {
        for x in unary_inputs() {
            let (a, b) = (ours(x), platform(x));
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "{name}({x:e}) = {a:e} but the UCRT gives {b:e}"
            );
        }
    };
    check("sinf", sinf, f32::sin);
    check("cosf", cosf, f32::cos);
    check("tanf", tanf, f32::tan);
    check("atanf", atanf, f32::atan);
    // atan2f is the one function that is *not* exact (see the provenance log): measured at about
    // 3e-6 of random operand pairs. Guard against regressions of that rate.
    let pairs = binary_inputs();
    let wrong = pairs
        .iter()
        .filter(|&&(y, x)| {
            atan2f(y, x).to_bits() != y.atan2(x).to_bits() && !(y.is_nan() || x.is_nan())
        })
        .count();
    assert!(
        wrong * 100_000 < pairs.len(),
        "atan2f differs from the UCRT on {wrong} of {} pairs",
        pairs.len()
    );
}
