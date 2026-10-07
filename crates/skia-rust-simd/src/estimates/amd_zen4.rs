// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The oracle host's estimate instructions as pure functions: what model tiers use for
//! [`Estimates::AmdZen4`](crate::Estimates::AmdZen4).
//!
//! Each function reproduces one lane of the AMD Zen 4 instruction bit for bit for **every** `f32`
//! input (checked exhaustively on the oracle host: `exhaustive_amd_zen4_vs_host`). They run
//! anywhere, including under Miri.

/// `rcpps`/`rcpss`/`vrcpps` (Sse2, Sse41, Ml3 `rcp_approx`).
#[must_use]
#[inline]
pub fn rcp(x: f32) -> f32 {
    super::tables::AMD_ZEN4.rcp_approx(x)
}

/// `rsqrtps`/`rsqrtss`/`vrsqrtps` (Sse2, Sse41, Ml3 `rsqrt_approx`).
#[must_use]
#[inline]
pub fn rsqrt(x: f32) -> f32 {
    super::tables::AMD_ZEN4.rsqrt_approx(x)
}

/// `vrcp14ps`/`vrcp14ss` (Ml4 `rcp_approx`). Zen 4 implements Intel's reference algorithm;
/// see [`super::recip14`].
#[must_use]
#[inline]
pub fn rcp14(x: f32) -> f32 {
    super::recip14::rcp14(x)
}

/// `vrsqrt14ps`/`vrsqrt14ss` (Ml4 `rsqrt_approx`). See [`super::recip14`].
#[must_use]
#[inline]
pub fn rsqrt14(x: f32) -> f32 {
    super::recip14::rsqrt14(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tier;
    use crate::estimates::{AMD_ZEN4, EstimateOp, Fingerprints, host_estimates};

    fn model(op: EstimateOp) -> fn(f32) -> f32 {
        match op {
            EstimateOp::Rcpps => rcp,
            EstimateOp::Rsqrtps => rsqrt,
            EstimateOp::Rcp14 => rcp14,
            EstimateOp::Rsqrt14 => rsqrt14,
        }
    }

    /// Whether this host's `op` is the oracle host's (fingerprints, design §1.4).
    fn host_is_amd_zen4(host: &Fingerprints, op: EstimateOp) -> bool {
        let tier = match op {
            EstimateOp::Rcpps | EstimateOp::Rsqrtps => Tier::Sse2,
            EstimateOp::Rcp14 | EstimateOp::Rsqrt14 => Tier::Ml4,
        };
        host.matches_for(&AMD_ZEN4, tier)
    }

    type Mismatch = (u32, u32, u32);

    /// Compares the model with the host on `inputs` (bits); returns the mismatch count and up
    /// to 16 `(input, host, model)` examples.
    fn compare(op: EstimateOp, inputs: &[u32]) -> (u64, Vec<Mismatch>) {
        let xs: Vec<f32> = inputs.iter().copied().map(f32::from_bits).collect();
        let mut out = vec![0.0f32; xs.len()];
        host_estimates(op, &xs, &mut out).unwrap();
        let f = model(op);
        let mut bad = 0;
        let mut examples = Vec::new();
        for (&x, &h) in xs.iter().zip(&out) {
            let m = f(x);
            if m.to_bits() != h.to_bits() {
                bad += 1;
                if examples.len() < 16 {
                    examples.push((x.to_bits(), h.to_bits(), m.to_bits()));
                }
            }
        }
        (bad, examples)
    }

    /// All 2³² inputs, split across every available thread.
    fn compare_exhaustive(op: EstimateOp) -> (u64, Vec<Mismatch>) {
        const CHUNK: u64 = 1 << 16;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get() as u64);
        let per = (1u64 << 32).div_ceil(threads);
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || {
                        let hi = ((t + 1) * per).min(1 << 32);
                        let mut bad = 0;
                        let mut examples = Vec::new();
                        let mut start = t * per;
                        while start < hi {
                            let end = (start + CHUNK).min(hi);
                            #[allow(clippy::cast_possible_truncation)] // < 2^32
                            let inputs: Vec<u32> = (start..end).map(|b| b as u32).collect();
                            let (b, e) = compare(op, &inputs);
                            bad += b;
                            let room = 16 - examples.len();
                            examples.extend(e.into_iter().take(room));
                            start = end;
                        }
                        (bad, examples)
                    })
                })
                .collect();
            let mut bad = 0;
            let mut examples = Vec::new();
            for h in handles {
                let (b, e) = h.join().unwrap();
                bad += b;
                examples.extend(e);
            }
            examples.truncate(16);
            (bad, examples)
        })
    }

    /// Inputs where the special-value rules matter: signed zeros, denormals (including the
    /// `rcp14` overflow threshold `0x0020_0000`), binade edges, the top of the range, ±∞, NaNs.
    const SPECIALS: [u32; 24] = [
        0x0000_0000,
        0x0000_0001,
        0x0000_0800,
        0x001f_ffff,
        0x0020_0000,
        0x0020_0001,
        0x0040_0000,
        0x007f_ffff,
        0x0080_0000,
        0x0080_0800,
        0x3f80_0000,
        0x3f80_0001,
        0x3f81_2345,
        0x4000_0000,
        0x7e00_0000,
        0x7e7f_f800,
        0x7e7f_ffff,
        0x7e80_0000,
        0x7e80_0001,
        0x7f00_0000,
        0x7f7f_ffff,
        0x7f80_0000,
        0x7f80_0001,
        0x7fc0_0000,
    ];

    /// `(input, rcp, rsqrt, rcp14, rsqrt14)` output bits measured on the oracle host.
    #[rustfmt::skip]
    const KNOWN: [(u32, u32, u32, u32, u32); 16] = [
        (0x0000_0000, 0x7f80_0000, 0x7f80_0000, 0x7f80_0000, 0x7f80_0000),
        (0x8000_0000, 0xff80_0000, 0xff80_0000, 0xff80_0000, 0xff80_0000),
        (0x0000_0001, 0x7f80_0000, 0x7f80_0000, 0x7f80_0000, 0x64b5_0280),
        (0x8000_0001, 0xff80_0000, 0xff80_0000, 0xff80_0000, 0xffc0_0000),
        (0x0020_0000, 0x7f80_0000, 0x7f80_0000, 0x7f80_0000, 0x5f80_0000),
        (0x0040_0000, 0x7f80_0000, 0x7f80_0000, 0x7f00_0000, 0x5f35_0280),
        (0x0080_0000, 0x7e7f_f000, 0x5eff_f800, 0x7e80_0000, 0x5f00_0000),
        (0x8080_0000, 0xfe7f_f000, 0xffc0_0000, 0xfe80_0000, 0xffc0_0000),
        (0x3f80_0000, 0x3f7f_f000, 0x3f7f_f800, 0x3f80_0000, 0x3f80_0000),
        (0x3f80_0001, 0x3f7f_f000, 0x3f7f_f800, 0x3f7f_fe00, 0x3f7f_fd00),
        (0xbf81_2345, 0xbf7d_c000, 0xffc0_0000, 0xbf7d_c080, 0xffc0_0000),
        (0x4000_0000, 0x3eff_f000, 0x3f35_0000, 0x3f00_0000, 0x3f35_0280),
        (0x7e00_0000, 0x00ff_f000, 0x2035_0000, 0x0100_0000, 0x2035_0280),
        (0x7e80_0001, 0x0000_0000, 0x1fff_f800, 0x007f_ff00, 0x1fff_fd00),
        (0x7f7f_ffff, 0x0000_0000, 0x1f80_0000, 0x0020_0000, 0x1f80_0000),
        (0xff81_2345, 0xffc1_2345, 0xffc1_2345, 0xffc1_2345, 0xffc1_2345),
    ];

    /// Host-independent (and run under Miri): values measured on the oracle host.
    #[test]
    fn known_values() {
        for (x, r, rs, r14, rs14) in KNOWN {
            let x = f32::from_bits(x);
            assert_eq!(rcp(x).to_bits(), r, "rcp {x:e}");
            assert_eq!(rsqrt(x).to_bits(), rs, "rsqrt {x:e}");
            assert_eq!(rcp14(x).to_bits(), r14, "rcp14 {x:e}");
            assert_eq!(rsqrt14(x).to_bits(), rs14, "rsqrt14 {x:e}");
        }
        assert_eq!(rcp(f32::INFINITY).to_bits(), 0);
        assert_eq!(rcp(f32::NEG_INFINITY).to_bits(), 0x8000_0000);
        assert_eq!(rsqrt(f32::INFINITY).to_bits(), 0);
        assert_eq!(rsqrt(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(rcp14(f32::NEG_INFINITY).to_bits(), 0x8000_0000);
        assert_eq!(rsqrt14(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
    }

    /// Host-independent: the models reproduce the oracle host's fingerprints (design §1.4),
    /// i.e. all 2²³ outputs of each fingerprinted binade.
    #[test]
    #[cfg_attr(miri, ignore = "5 x 2^23 evaluations; too slow under Miri")]
    fn models_reproduce_amd_zen4_fingerprints() {
        use crate::estimates::{Binade, fnv1a_words};
        let fp = |f: fn(f32) -> f32, binade: Binade| {
            fnv1a_words(
                (0..1u32 << 23).map(|m| f(f32::from_bits(binade.base_bits() | m)).to_bits()),
            )
        };
        assert_eq!(Some(fp(rcp, Binade::One)), AMD_ZEN4.rcpps_1_2);
        assert_eq!(Some(fp(rsqrt, Binade::One)), AMD_ZEN4.rsqrtps_1_2);
        assert_eq!(Some(fp(rsqrt, Binade::Two)), AMD_ZEN4.rsqrtps_2_4);
        assert_eq!(Some(fp(rcp14, Binade::One)), AMD_ZEN4.rcp14_1_2);
        assert_eq!(Some(fp(rsqrt14, Binade::One)), AMD_ZEN4.rsqrt14_1_2);
    }

    /// Fast version of the exhaustive check: every 4093rd bit pattern (about a million inputs,
    /// all exponents and signs) plus [`SPECIALS`] and their negations. Runs on any host whose
    /// estimates are the oracle host's; skipped elsewhere.
    #[test]
    fn sampled_amd_zen4_vs_host() {
        if !EstimateOp::Rcpps.is_available() {
            eprintln!("skipping sampled_amd_zen4_vs_host: no x86 estimate instructions");
            return;
        }
        let host = Fingerprints::host();
        let mut inputs: Vec<u32> = Vec::new();
        let mut ran = false;
        for op in EstimateOp::ALL {
            if !host_is_amd_zen4(&host, op) {
                eprintln!(
                    "skipping {}: this host's estimates are not the oracle host's",
                    op.name()
                );
                continue;
            }
            if inputs.is_empty() {
                inputs = (0..=u32::MAX).step_by(4093).collect();
                inputs.extend(SPECIALS);
                inputs.extend(SPECIALS.map(|b| b | 0x8000_0000));
            }
            ran = true;
            let (bad, examples) = compare(op, &inputs);
            assert_eq!(
                bad,
                0,
                "{}: (input, host, model) {examples:08x?}",
                op.name()
            );
        }
        if !ran {
            eprintln!("skipping sampled_amd_zen4_vs_host: host is not AMD Zen 4");
        }
    }

    /// Every `f32` input through the host instruction and the model: 0 mismatches. Needs the
    /// oracle host (AMD Zen 4 with AVX-512); fails, rather than passing vacuously, elsewhere.
    #[test]
    #[ignore = "exhaustive over 2^32 inputs per op (about 10 s in release on 16 threads) and needs the \
                oracle host (AMD Zen 4). Run: \
                cargo test -p skia-rust-simd --release -- --ignored exhaustive_amd_zen4"]
    fn exhaustive_amd_zen4_vs_host() {
        let host = Fingerprints::host();
        let mut failed = false;
        for op in EstimateOp::ALL {
            assert!(
                host_is_amd_zen4(&host, op),
                "{} on this host is not the oracle host's (fingerprints {host:x?})",
                op.name()
            );
            let t = std::time::Instant::now();
            let (bad, examples) = compare_exhaustive(op);
            eprintln!(
                "{}: {bad} mismatches over 2^32 inputs ({:.1?})",
                op.name(),
                t.elapsed()
            );
            for (x, h, m) in examples {
                eprintln!("  x={x:08x} host={h:08x} model={m:08x}");
            }
            failed |= bad != 0;
        }
        assert!(!failed, "model differs from the host");
    }
}
