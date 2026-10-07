// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `rcpps`/`rsqrtps` as data: per-vendor tables plus the rule that extends them to every `f32`
//! (design §1.4, §2.8).
//!
//! On the oracle host (AMD Zen 4) both instructions are exact functions of the sign, the
//! exponent and the **top 12 mantissa bits** of the input. A table holds the outputs for the 4096
//! inputs `1 + i/4096` (`rcpps`, `rsqrtps` with even exponent) and `2 + i/2048` (`rsqrtps` with
//! odd exponent); every other input follows from the rules documented on
//! [`SseTables::rcp_approx`] and [`SseTables::rsqrt_approx`], which were checked against the
//! host over all 2³² inputs (`exhaustive_amd_zen4_vs_host`).

/// Decodes `4 * N` little-endian bytes into `N` words (at compile time).
const fn le_words<const N: usize>(bytes: &[u8]) -> [u32; N] {
    assert!(bytes.len() == 4 * N, "table has the wrong size");
    let mut out = [0u32; N];
    let mut i = 0;
    while i < N {
        out[i] = u32::from_le_bytes([
            bytes[4 * i],
            bytes[4 * i + 1],
            bytes[4 * i + 2],
            bytes[4 * i + 3],
        ]);
        i += 1;
    }
    out
}

use super::TABLE_LEN;

/// A vendor's `rcpps`/`rsqrtps` tables, in the format `cargo xtask cpu-probe --dump-tables`
/// writes (see [`super::EstimateTables`]).
#[derive(Clone, Copy, Debug)]
pub struct SseTables {
    /// `rcpps[i]` = output bits for input bits `0x3f80_0000 | i << 11` (`1 + i/4096`).
    pub rcpps: &'static [u32; TABLE_LEN],
    /// `rsqrtps[i]` = output bits for `0x3f80_0000 | i << 11`, `rsqrtps[4096 + i]` for
    /// `0x4000_0000 | i << 11` (`2 + i/2048`).
    pub rsqrtps: &'static [u32; 2 * TABLE_LEN],
}

static AMD_ZEN4_RCPPS: [u32; TABLE_LEN] = le_words(include_bytes!("amd_zen4/rcpps.bin"));
static AMD_ZEN4_RSQRTPS: [u32; 2 * TABLE_LEN] = le_words(include_bytes!("amd_zen4/rsqrtps.bin"));

/// The oracle host's tables (AMD Ryzen 7 7800X3D, Zen 4), generated with
/// `cargo xtask cpu-probe --dump-tables` (`amd_zen4/estimates.txt` records the host and its
/// fingerprints, which equal [`super::AMD_ZEN4`]).
pub static AMD_ZEN4: SseTables = SseTables {
    rcpps: &AMD_ZEN4_RCPPS,
    rsqrtps: &AMD_ZEN4_RSQRTPS,
};

const SIGN: u32 = 0x8000_0000;
const EXP_MASK: u32 = 0x7f80_0000;
const MANT_MASK: u32 = 0x007f_ffff;
const QNAN_BIT: u32 = 0x0040_0000;
/// The x86 "real indefinite" `QNaN` (`-nan`), returned for invalid operations.
const DEFAULT_NAN: u32 = 0xffc0_0000;

impl SseTables {
    /// `rcpps` (and `rcpss`, `vrcpps`) on one lane.
    ///
    /// - NaN: the input, quieted (payload and sign kept).
    /// - ±∞ → ±0; ±0 and every denormal input → ±∞ (denormals are treated as zero).
    /// - Normal `x = s·1.m·2ᴱ`: `rcpps[m >> 11]` with its exponent decreased by `E` and the sign
    ///   of `x`; a result below the normal range (`E ≥ 126`) is flushed to ±0 (no denormal
    ///   results).
    #[must_use]
    pub fn rcp_approx(&self, x: f32) -> f32 {
        let bits = x.to_bits();
        let sign = bits & SIGN;
        let exp = (bits & EXP_MASK) >> 23;
        let mant = bits & MANT_MASK;
        let out = if exp == 0xff {
            if mant == 0 { sign } else { bits | QNAN_BIT }
        } else if exp == 0 {
            sign | EXP_MASK
        } else {
            let t = self.rcpps[(mant >> 11) as usize];
            // The entry is the estimate for exponent 127; scale it by 2^-(exp - 127).
            #[allow(clippy::cast_possible_wrap)] // 8-bit exponent fields
            let e = ((t & EXP_MASK) >> 23) as i32 - (exp as i32 - 127);
            if e <= 0 {
                sign
            } else {
                #[allow(clippy::cast_sign_loss)] // e is in 1..=253
                let e = e as u32;
                sign | e << 23 | (t & MANT_MASK)
            }
        };
        f32::from_bits(out)
    }

    /// `rsqrtps` (and `rsqrtss`, `vrsqrtps`) on one lane.
    ///
    /// - NaN: the input, quieted. `+∞` → `+0`.
    /// - ±0 and every denormal input → ±∞ (denormals are treated as zero, keeping the sign).
    /// - Any other negative input (normal or `-∞`) → the default NaN `0xffc0_0000`.
    /// - Positive normal `x = 1.m·2ᴱ`: for even `E`, `rsqrtps[m >> 11]` scaled by `2^(-E/2)`;
    ///   for odd `E`, `rsqrtps[4096 + (m >> 11)]` scaled by `2^(-(E-1)/2)` (the input is read as
    ///   `2·1.m · 2^(E-1)`). The result is always normal.
    #[must_use]
    pub fn rsqrt_approx(&self, x: f32) -> f32 {
        let bits = x.to_bits();
        let sign = bits & SIGN;
        let exp = (bits & EXP_MASK) >> 23;
        let mant = bits & MANT_MASK;
        let out = if exp == 0xff && mant != 0 {
            bits | QNAN_BIT
        } else if exp == 0 {
            sign | EXP_MASK
        } else if sign != 0 {
            DEFAULT_NAN
        } else if exp == 0xff {
            0
        } else {
            #[allow(clippy::cast_possible_wrap)] // 8-bit exponent field
            let e = exp as i32 - 127;
            let odd = e & 1;
            let t = self.rsqrtps[usize::from(odd != 0) * TABLE_LEN + (mant >> 11) as usize];
            // `(e - odd) / 2` is exact; the table entry is for exponent 0 (even) or 1 (odd).
            #[allow(clippy::cast_sign_loss)] // two's-complement exponent arithmetic
            let delta = (((e - odd) / 2) << 23) as u32;
            t.wrapping_sub(delta)
        };
        f32::from_bits(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tier;
    use crate::estimates::{EstimateTables, Fingerprints};

    /// The committed data decodes as `cpu-probe` wrote it (runs under Miri).
    #[test]
    fn amd_zen4_tables_decode() {
        assert_eq!(AMD_ZEN4.rcpps[0], 0x3f7f_f000); // rcpps(1.0)
        assert_eq!(AMD_ZEN4.rsqrtps[0], 0x3f7f_f800); // rsqrtps(1.0)
        assert_eq!(AMD_ZEN4.rsqrtps[TABLE_LEN], 0x3f35_0000); // rsqrtps(2.0)
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "walks all 12288 entries; slow under Miri and data-only"
    )]
    fn amd_zen4_tables_shape() {
        // Estimates of 1 + i/4096 decrease with i and carry no bits below the top 12.
        for t in [
            &AMD_ZEN4.rcpps[..],
            &AMD_ZEN4.rsqrtps[..TABLE_LEN],
            &AMD_ZEN4.rsqrtps[TABLE_LEN..],
        ] {
            assert!(t.windows(2).all(|w| w[0] >= w[1]));
            assert!(t.iter().all(|w| w.trailing_zeros() >= 11));
        }
    }

    /// On the oracle host the committed tables are the host's.
    #[test]
    fn amd_zen4_tables_are_the_hosts() {
        if !crate::estimates::EstimateOp::Rcpps.is_available()
            || !Fingerprints::host().matches_for(&crate::estimates::AMD_ZEN4, Tier::Sse2)
        {
            eprintln!("skipping amd_zen4_tables_are_the_hosts: host is not AMD Zen 4");
            return;
        }
        let host = EstimateTables::host().unwrap();
        assert_eq!(&host.rcpps[..], &AMD_ZEN4.rcpps[..]);
        assert_eq!(&host.rsqrtps[..], &AMD_ZEN4.rsqrtps[..]);
    }
}
