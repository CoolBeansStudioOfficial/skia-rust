// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RandomTest.cpp (chrome/m156)

use skia_rust_core::libm;
use skia_rust_core::random::Random;

use crate::{Reporter, def_test, reporter_assert};

// Port of: include/private/SkMath.h#L37-L39 (chrome/m156)
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the C++ casts
fn sk_left_shift(value: i32, shift: i32) -> i32 {
    ((value as u32) << shift) as i32
}

// Port of: tests/RandomTest.cpp#L18-L38 (chrome/m156)
fn anderson_darling_test(p: &mut [f64; 32]) -> bool {
    // Min and max Anderson-Darling values allowable for k=32
    const K_AD_MIN32: f64 = 0.202; // p-value of ~0.1
    const K_AD_MAX32: f64 = 3.89; // p-value of ~0.99

    // sort p values
    // skia-rust: SkTQSort<double> replaced by the std sort (Rust uses std for sorting).
    p.sort_by(f64::total_cmp);

    // and compute Anderson-Darling statistic to ensure these are uniform
    let mut s = 0.0f64;
    for k in 0..32usize {
        let mut v = p[k] * (1.0 - p[31 - k]);
        if v < 1.0e-30 {
            v = 1.0e-30;
        }
        s += (2.0 * f64::from(u32::try_from(k + 1).unwrap()) - 1.0) * libm::log(v);
    }
    let a2 = -32.0 - 0.03125 * s;

    K_AD_MIN32 < a2 && a2 < K_AD_MAX32
}

// Port of: tests/RandomTest.cpp#L40-L53 (chrome/m156)
fn chi_square_test(bins: &[i32; 256], e: i32) -> bool {
    // Min and max chisquare values allowable
    const K_CHI_SQ_MIN256: f64 = 206.3179; // probability of chance = 0.99 with k=256
    const K_CHI_SQ_MAX256: f64 = 311.5603; // probability of chance = 0.01 with k=256

    // compute chi-square
    let mut chi2 = 0.0f64;
    for bin in bins {
        let delta = f64::from(bin - e);
        chi2 += delta * delta / f64::from(e);
    }

    K_CHI_SQ_MIN256 < chi2 && chi2 < K_CHI_SQ_MAX256
}

// Approximation to the normal distribution CDF
// From Waissi and Rossin, 1996
// Port of: tests/RandomTest.cpp#L55-L63 (chrome/m156)
fn normal_cdf(z: f64) -> f64 {
    // Note: the C++ source has `-0.0004406*z*z* + 0.0418198`, i.e. a multiplication by the
    // unary-plus constant; that is ported as written.
    let mut t = ((-0.000_440_6 * z * z * 0.041_819_8) * z * z + 0.9) * z;
    t *= -1.772_453_850_91; // -sqrt(PI)
    1.0 / (1.0 + libm::exp(t))
}

// Port of: tests/RandomTest.cpp#L65-L75 (chrome/m156)
fn test_random_byte(reporter: &mut Reporter, shift: u32) {
    let mut bins = [0i32; 256];

    let mut rand = Random::default();
    for _ in 0..256 * 10000 {
        bins[((rand.next_u() >> shift) & 0xff) as usize] += 1;
    }

    reporter_assert!(reporter, chi_square_test(&bins, 10000));
}

// Port of: tests/RandomTest.cpp#L77-L96 (chrome/m156)
#[allow(clippy::manual_range_contains)] // keeps the C++ assertion text verbatim
fn test_random_float(reporter: &mut Reporter) {
    let mut bins = [0i32; 256];

    let mut rand = Random::default();
    for _ in 0..256 * 10000 {
        let f = rand.next_f();
        reporter_assert!(reporter, 0.0f32 <= f && f < 1.0f32);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // mirrors (int)(f*256.f); f is in [0,1) so the value is in range
        {
            bins[(f * 256.0f32) as usize] += 1;
        }
    }
    reporter_assert!(reporter, chi_square_test(&bins, 10000));

    let mut p = [0.0f64; 32];
    for slot in &mut p {
        let f = rand.next_f();
        reporter_assert!(reporter, 0.0f32 <= f && f < 1.0f32);
        *slot = f64::from(f);
    }
    reporter_assert!(reporter, anderson_darling_test(&mut p));
}

// This is a test taken from tuftests by Marsaglia and Tsang. The idea here is that
// we are using the random bit generated from a single shift position to generate
// "strings" of 16 bits in length, shifting the string and adding a new bit with each
// iteration. We track the numbers generated. The ones that we don't generate will
// have a normal distribution with mean ~24108 and standard deviation ~127. By
// creating a z-score (# of deviations from the mean) for one iteration of this step
// we can determine its probability.
//
// The original test used 26 bit strings, but is somewhat slow. This version uses 16
// bits which is less rigorous but much faster to generate.
// Port of: tests/RandomTest.cpp#L98-L156 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ local names rand and rnd
fn test_single_gorilla(reporter: &mut Reporter, shift: u32) -> f64 {
    const K_WORD_WIDTH: i32 = 16;
    const K_MEAN: f64 = 24108.0;
    const K_STANDARD_DEVIATION: f64 = 127.0;
    const K_N: i32 = 1 << K_WORD_WIDTH;
    const K_NUM_ENTRIES: i32 = K_N >> 5; // dividing by 32
    let mut entries = [0u32; K_NUM_ENTRIES as usize];

    let mut rand = Random::default();
    // pre-seed our string value
    let mut value: i32 = 0;
    for _ in 0..K_WORD_WIDTH - 1 {
        value <<= 1;
        let rnd = rand.next_u();
        value |= i32::try_from((rnd >> shift) & 0x1).unwrap();
    }

    // now make some strings and track them
    for _ in 0..K_N {
        value = sk_left_shift(value, 1);
        let rnd = rand.next_u();
        value |= i32::try_from((rnd >> shift) & 0x1).unwrap();

        let index = value & (K_NUM_ENTRIES - 1);
        debug_assert!(index < K_NUM_ENTRIES);
        let entry_shift = (value >> (K_WORD_WIDTH - 5)) & 0x1f;
        entries[usize::try_from(index).unwrap()] |= 0x1 << entry_shift;
    }

    // count entries
    let mut total: i32 = 0;
    for &e in &entries {
        let mut entry = e;
        while entry != 0 {
            total += i32::try_from(entry & 0x1).unwrap();
            entry >>= 1;
        }
    }

    // convert counts to normal distribution z-score
    let z = (f64::from(K_N - total) - K_MEAN) / K_STANDARD_DEVIATION;

    // compute probability from normal distibution CDF
    let p = normal_cdf(z);

    reporter_assert!(reporter, 0.01 < p && p < 0.99);
    p
}

// Port of: tests/RandomTest.cpp#L158-L166 (chrome/m156)
fn test_gorilla(reporter: &mut Reporter) {
    let mut p = [0.0f64; 32];
    for (bit_position, slot) in p.iter_mut().enumerate() {
        *slot = test_single_gorilla(reporter, u32::try_from(bit_position).unwrap());
    }

    reporter_assert!(reporter, anderson_darling_test(&mut p));
}

// Port of: tests/RandomTest.cpp#L168-L184 (chrome/m156)
#[allow(clippy::manual_range_contains)] // keeps the C++ assertion text verbatim
fn test_range(reporter: &mut Reporter) {
    let mut rand = Random::default();

    // just to make sure we don't crash in this case
    let _ = rand.next_range_u(0, 0xffff_ffff);

    // check a case to see if it's uniform
    let mut bins = [0i32; 256];
    for _ in 0..256 * 10000 {
        let u = rand.next_range_u(17, 17 + 255);
        reporter_assert!(reporter, 17 <= u && u <= 17 + 255);
        bins[(u - 17) as usize] += 1;
    }

    reporter_assert!(reporter, chi_square_test(&bins, 10000));
}

// Port of: tests/RandomTest.cpp#L186-L198 (chrome/m156)
def_test!(Random, |reporter| {
    // check uniform distributions of each byte in 32-bit word
    test_random_byte(reporter, 0);
    test_random_byte(reporter, 8);
    test_random_byte(reporter, 16);
    test_random_byte(reporter, 24);

    test_random_float(reporter);

    test_gorilla(reporter);

    test_range(reporter);
});
