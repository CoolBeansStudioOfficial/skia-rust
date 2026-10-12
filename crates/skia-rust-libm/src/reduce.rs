//! Argument reduction modulo pi/2 with a fixed-point expansion of 2/pi (Payne–Hanek style; the
//! method is from M. Payne and R. Hanek, "Radian reduction for trigonometric functions", 1983,
//! and Muller, "Elementary Functions" ch. 11). The 2/pi bits were generated with an arbitrary
//! precision evaluation of 2/pi (see `docs/design/libm-cleanroom.md`).
//!
//! The result is the remainder `r = x - n*pi/2` correctly rounded to `f64`, with `n` the nearest
//! integer to `x*2/pi`, for any finite `f64` with `|x| > pi/4`.

/// `2/pi = 0.b1b2b3...` in binary; word 0 holds `b1..b64` (most significant first).
const TWO_OVER_PI: [u64; 22] = [
    0xA2F9_836E_4E44_1529,
    0xFC27_57D1_F534_DDC0,
    0xDB62_9599_3C43_9041,
    0xFE51_63AB_DEBB_C561,
    0xB724_6E3A_424D_D2E0,
    0x0649_2EEA_09D1_921C,
    0xFE1D_EB1C_B129_A73E,
    0xE882_35F5_2EBB_4484,
    0xE99C_7026_B45F_7E41,
    0x3991_D639_8353_39F4,
    0x9C84_5F8B_BDF9_283B,
    0x1FF8_97FF_DE05_980F,
    0xEF2F_118B_5A0A_6D1F,
    0x6D36_7ECF_27CB_09B7,
    0x4F46_3F66_9E5F_EA2D,
    0x7527_BAC7_EBE5_F17B,
    0x3D07_39F7_8A52_92EA,
    0x6BFB_5FB1_1F8D_5D08,
    0x5603_3046_FC7B_6BAB,
    0xF0CF_BC20_9AF4_361D,
    0xA9E3_9161_5EE6_1B08,
    0x6599_855F_14A0_6840,
];

/// `pi/2` as an unevaluated sum of two doubles.
const PIO2_HI: f64 = core::f64::consts::FRAC_PI_2;
const PIO2_LO: f64 = 6.123_233_995_736_766e-17;

/// Window length in bits of the 2/pi expansion used per reduction (5 words).
const L: i64 = 320;

/// Bits `[start, start + 64)` of the 2/pi bit string (`start` counted from 0).
fn stream_word(start: usize) -> u64 {
    let (a, sh) = (start / 64, start % 64);
    if sh == 0 {
        TWO_OVER_PI[a]
    } else {
        (TWO_OVER_PI[a] << sh) | (TWO_OVER_PI[a + 1] >> (64 - sh))
    }
}

/// `2^k` for `k` in the normal exponent range.
fn pow2(k: i64) -> f64 {
    debug_assert!((-1022..=1023).contains(&k));
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // k + 1023 is in 1..=2046
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// Error-free `a + b` (Knuth).
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// Error-free `a * b` (Dekker/Veltkamp splitting, no FMA).
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    const SPLIT: f64 = 134_217_729.0; // 2^27 + 1
    let p = a * b;
    let ca = SPLIT * a;
    let ah = ca - (ca - a);
    let al = a - ah;
    let cb = SPLIT * b;
    let bh = cb - (cb - b);
    let bl = b - bh;
    (p, ((ah * bh - p) + ah * bl + al * bh) + al * bl)
}

/// Reduces `x` (finite, `|x| > pi/4`): returns `(n mod 4, r)`.
#[allow(clippy::many_single_char_names, clippy::cast_possible_truncation)]
pub(crate) fn reduce_pio2(x: f64) -> (u32, f64) {
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let m = (bits & ((1 << 52) - 1)) | (1 << 52);
    #[allow(clippy::cast_possible_wrap)] // the exponent field is 11 bits
    let e = ((bits >> 52) & 0x7ff) as i64 - 1075;

    // x*(2/pi) = m * 2^e * sum(b_i 2^-i). Terms with i <= e-2 are multiples of 4 and do not
    // affect the quadrant or the fraction; keep the next L bits.
    let i0 = (e - 2).max(0);
    #[allow(clippy::cast_sign_loss)] // i0 >= 0
    let i0u = i0 as usize;
    let mut w = [0u64; 5]; // little-endian words of the L-bit window
    for (j, word) in w.iter_mut().enumerate() {
        *word = stream_word(i0u + 64 * (4 - j));
    }
    // P = m * W, little-endian, 6 words.
    let mut p = [0u64; 6];
    let mut carry = 0u128;
    for j in 0..5 {
        let t = u128::from(m) * u128::from(w[j]) + carry;
        p[j] = t as u64;
        carry = t >> 64;
    }
    p[5] = carry as u64;
    // x*(2/pi) = P / 2^s (mod 4), s = L + i0 - e.
    let s = L + i0 - e;
    #[allow(clippy::cast_sign_loss)] // s > 0
    let su = s as usize;
    let bit = |k: usize| (p[k / 64] >> (k % 64)) & 1;
    let q = (bit(su) | (bit(su + 1) << 1)) as u32;
    let round_up = bit(su - 1) == 1;

    // G = P mod 2^s, or 2^s - G when rounding to the next integer.
    let mut g = p;
    for (k, word) in g.iter_mut().enumerate() {
        let lo = k * 64;
        if lo >= su {
            *word = 0;
        } else if lo + 64 > su {
            *word &= (1u64 << (su - lo)) - 1;
        }
    }
    if round_up {
        // two's complement negate within s bits
        let mut c = 1u128;
        for (k, word) in g.iter_mut().enumerate() {
            let lo = k * 64;
            let t = u128::from(!*word) + c;
            *word = t as u64;
            c = t >> 64;
            if lo >= su {
                *word = 0;
            } else if lo + 64 > su {
                *word &= (1u64 << (su - lo)) - 1;
            }
        }
    }
    let n = (q + u32::from(round_up)) & 3;

    // Highest set bit of G.
    let mut h = 0usize;
    for k in (0..6).rev() {
        if g[k] != 0 {
            h = k * 64 + 63 - g[k].leading_zeros() as usize;
            break;
        }
    }
    // Top 128 bits of G, aligned so that bit h lands on bit 127.
    let get = |k: i64| -> u64 {
        if (0..6).contains(&k) {
            g[k as usize]
        } else {
            0
        }
    };
    let (wi, wb) = (h / 64, h % 64);
    // 192-bit window words [wi, wi-1, wi-2] shifted left so bit h is the top of the first word.
    let (w0, w1, w2) = (get(wi as i64), get(wi as i64 - 1), get(wi as i64 - 2));
    let sh = 63 - wb;
    let (c0, c1) = if sh == 0 {
        (w0, w1)
    } else {
        (
            (w0 << sh) | (w1 >> (64 - sh)),
            (w1 << sh) | (w2 >> (64 - sh)),
        )
    };
    let c = (u128::from(c0) << 64) | u128::from(c1);

    // |F| = c * 2^(h-127-s) as an (almost non-overlapping) double-double.
    let hi_int = (c >> 75) as u64;
    let lo_int = (c & ((1u128 << 75) - 1)) as f64;
    #[allow(clippy::cast_possible_wrap)] // h and s are below 1024
    let (hs, ss) = (h as i64, s);
    let f_hi = (hi_int as f64) * pow2(hs - 52 - ss);
    let f_lo = lo_int * pow2(hs - 127 - ss);
    let (f_hi, f_lo) = two_sum(f_hi, f_lo);

    // r = F * (pi/2) in double-double, then rounded.
    let (p0, p1) = two_prod(f_hi, PIO2_HI);
    let p1 = p1 + (f_hi * PIO2_LO + f_lo * PIO2_HI);
    let (r, _) = two_sum(p0, p1);
    let r = if round_up { -r } else { r };

    if neg { ((4 - n) & 3, -r) } else { (n, r) }
}

#[cfg(test)]
mod tests {
    use super::reduce_pio2;

    #[test]
    fn small_multiples() {
        let (n, r) = reduce_pio2(1.0);
        assert_eq!(n, 1);
        assert!((r - (1.0 - core::f64::consts::FRAC_PI_2)).abs() < 1e-15);
        let (n, r) = reduce_pio2(-4.0);
        // -4 = -3*pi/2 + 0.712...: nearest multiple of pi/2 is -3 (n mod 4 = 1)
        assert_eq!(n, 1);
        assert!((r - (-4.0 + 3.0 * core::f64::consts::FRAC_PI_2)).abs() < 1e-15);
    }

    #[test]
    fn huge_argument_in_range() {
        let (_, r) = reduce_pio2(1.0e300);
        assert!(r.abs() <= 0.785_398_163_397_448_4);
        let (_, r) = reduce_pio2(f64::from(f32::MAX));
        assert!(r.abs() <= 0.785_398_163_397_448_4);
    }
}
