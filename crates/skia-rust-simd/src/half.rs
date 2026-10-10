// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Scalar models of the F16C half-float conversions (`vcvtph2ps`, `vcvtps2ph`), bit-identical
//! to the instructions. Shared by the raster pipeline's x86 model tiers and by skcms, whose
//! HSW/SKX kernels use the same instructions (`Half_from_F`, `F_from_Half`).

/// `vcvtph2ps` (F16C, one lane): exact half → float, half denormals kept, a NaN quieted
/// (payload shifted into the top mantissa bits).
#[inline]
#[must_use]
pub fn cvtph2ps(h: u16) -> f32 {
    let h = u32::from(h);
    let sign = (h & 0x8000) << 16;
    let exp = (h >> 10) & 0x1f;
    let mant = h & 0x3ff;
    let bits = match (exp, mant) {
        (0, 0) => sign,
        // Denormal: mant * 2^-24, exact in f32 (normalized: shift the leading one to bit 10).
        (0, _) => {
            let shift = mant.leading_zeros() - 21;
            let m = (mant << shift) & 0x3ff;
            sign | ((127 - 15 + 1 - shift) << 23) | (m << 13)
        }
        (31, 0) => sign | 0x7f80_0000,
        (31, _) => sign | 0x7fc0_0000 | (mant << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (mant << 13),
    };
    f32::from_bits(bits)
}

/// `vcvtps2ph` (F16C, one lane) with `_MM_FROUND_CUR_DIRECTION` under the default MXCSR: round
/// to nearest even, half denormals produced (float denormals round to `±0`), overflow to `±inf`,
/// a NaN quieted with the top 10 payload bits kept.
#[inline]
#[must_use]
pub fn cvtps2ph(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = (bits >> 16) & 0x8000;
    let abs = bits & 0x7fff_ffff;
    let h = if abs > 0x7f80_0000 {
        0x7e00 | ((abs >> 13) & 0x3ff) // NaN
    } else if abs >= 0x477f_f000 {
        0x7c00 // >= 65520 (incl. inf) rounds to inf
    } else if abs < 0x3880_0000 {
        // Below 2^-14: a half denormal, |f| / 2^-24 rounded to nearest even (exact in f64).
        let scaled = (f64::from(f32::from_bits(abs)) * 16_777_216.0).round_ties_even();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // in [0, 1024]
        let m = scaled as u32;
        m
    } else {
        let e = (abs >> 23) - (127 - 15);
        let mant = abs & 0x007f_ffff;
        let h = (e << 10) | (mant >> 13);
        let rem = mant & 0x1fff;
        // Ties to even; a carry into the exponent is the correct next half.
        if rem > 0x1000 || (rem == 0x1000 && h & 1 == 1) {
            h + 1
        } else {
            h
        }
    };
    #[allow(clippy::cast_possible_truncation)] // h < 0x8000, sign is 0 or 0x8000
    let r = (sign | h) as u16;
    r
}
