// Port of: libjpeg-turbo src/jfdctint.c#L80-L285 (`_jpeg_fdct_islow`, libjpeg_turbo@e14cbfaa,
// 3.1.0 source), src/jcdctmgr.c#L60-L500 (`flss`, `compute_reciprocal`, the ISLOW branch of
// `start_pass_fdctmgr`, `convsamp`, `quantize`, `forward_DCT`) and the constants of jdct.h.
//
// Copyright (C) 1991-1996, Thomas G. Lane. Modified 2003-2011 by Guido Vollbeding.
// Copyright (C) 2015, 2020, 2025 D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.
//
// The non-SIMD x64 build of libjpeg-turbo has `DCTELEM` = `int` and `UDCTELEM` = `unsigned int`
// (jdct.h, `#ifndef WITH_SIMD`), so the integer types here are 32 bits and the reciprocal product
// is 64 bits (`UDCTELEM2`).

use super::Block;
use crate::tables::{DCTSIZE2, JQuantTbl};

/// `CONST_BITS` and `PASS1_BITS` for `BITS_IN_JSAMPLE == 8` (jfdctint.c).
const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

// FIX(x) constants for CONST_BITS == 13 (jfdctint.c).
const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

/// `DESCALE(x, n)` (jdct.h): a rounding right shift.
fn descale(x: i32, n: i32) -> i32 {
    (x + (1 << (n - 1))) >> n
}

/// `_jpeg_fdct_islow`: the accurate integer forward DCT, in place on 64 values in natural order.
/// The first pass is over the rows, the second over the columns.
pub(super) fn fdct_islow(data: &mut [i32; DCTSIZE2]) {
    for row in 0..8 {
        let d = &mut data[row * 8..row * 8 + 8];
        let tmp0 = d[0] + d[7];
        let tmp7 = d[0] - d[7];
        let tmp1 = d[1] + d[6];
        let tmp6 = d[1] - d[6];
        let tmp2 = d[2] + d[5];
        let tmp5 = d[2] - d[5];
        let tmp3 = d[3] + d[4];
        let tmp4 = d[3] - d[4];

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        d[0] = (tmp10 + tmp11) << PASS1_BITS;
        d[4] = (tmp10 - tmp11) << PASS1_BITS;

        let mut z1 = (tmp12 + tmp13) * FIX_0_541196100;
        d[2] = descale(z1 + tmp13 * FIX_0_765366865, CONST_BITS - PASS1_BITS);
        d[6] = descale(z1 + tmp12 * -FIX_1_847759065, CONST_BITS - PASS1_BITS);

        z1 = tmp4 + tmp7;
        let mut z2 = tmp5 + tmp6;
        let mut z3 = tmp4 + tmp6;
        let mut z4 = tmp5 + tmp7;
        let z5 = (z3 + z4) * FIX_1_175875602;

        let tmp4 = tmp4 * FIX_0_298631336;
        let tmp5 = tmp5 * FIX_2_053119869;
        let tmp6 = tmp6 * FIX_3_072711026;
        let tmp7 = tmp7 * FIX_1_501321110;
        z1 *= -FIX_0_899976223;
        z2 *= -FIX_2_562915447;
        z3 *= -FIX_1_961570560;
        z4 *= -FIX_0_390180644;
        z3 += z5;
        z4 += z5;

        d[7] = descale(tmp4 + z1 + z3, CONST_BITS - PASS1_BITS);
        d[5] = descale(tmp5 + z2 + z4, CONST_BITS - PASS1_BITS);
        d[3] = descale(tmp6 + z2 + z3, CONST_BITS - PASS1_BITS);
        d[1] = descale(tmp7 + z1 + z4, CONST_BITS - PASS1_BITS);
    }
    for col in 0..8 {
        let at = |k: usize| k * 8 + col;
        let tmp0 = data[at(0)] + data[at(7)];
        let tmp7 = data[at(0)] - data[at(7)];
        let tmp1 = data[at(1)] + data[at(6)];
        let tmp6 = data[at(1)] - data[at(6)];
        let tmp2 = data[at(2)] + data[at(5)];
        let tmp5 = data[at(2)] - data[at(5)];
        let tmp3 = data[at(3)] + data[at(4)];
        let tmp4 = data[at(3)] - data[at(4)];

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        data[at(0)] = descale(tmp10 + tmp11, PASS1_BITS);
        data[at(4)] = descale(tmp10 - tmp11, PASS1_BITS);

        let mut z1 = (tmp12 + tmp13) * FIX_0_541196100;
        data[at(2)] = descale(z1 + tmp13 * FIX_0_765366865, CONST_BITS + PASS1_BITS);
        data[at(6)] = descale(z1 + tmp12 * -FIX_1_847759065, CONST_BITS + PASS1_BITS);

        z1 = tmp4 + tmp7;
        let mut z2 = tmp5 + tmp6;
        let mut z3 = tmp4 + tmp6;
        let mut z4 = tmp5 + tmp7;
        let z5 = (z3 + z4) * FIX_1_175875602;

        let tmp4 = tmp4 * FIX_0_298631336;
        let tmp5 = tmp5 * FIX_2_053119869;
        let tmp6 = tmp6 * FIX_3_072711026;
        let tmp7 = tmp7 * FIX_1_501321110;
        z1 *= -FIX_0_899976223;
        z2 *= -FIX_2_562915447;
        z3 *= -FIX_1_961570560;
        z4 *= -FIX_0_390180644;
        z3 += z5;
        z4 += z5;

        data[at(7)] = descale(tmp4 + z1 + z3, CONST_BITS + PASS1_BITS);
        data[at(5)] = descale(tmp5 + z2 + z4, CONST_BITS + PASS1_BITS);
        data[at(3)] = descale(tmp6 + z2 + z3, CONST_BITS + PASS1_BITS);
        data[at(1)] = descale(tmp7 + z1 + z4, CONST_BITS + PASS1_BITS);
    }
}

/// `flss` (jcdctmgr.c): one more than the index of the highest set bit of a 16-bit value, 0 for 0.
fn flss(val: u16) -> i32 {
    let mut v = u32::from(val) << 16;
    if val == 0 {
        return 0;
    }
    let mut bit = 16;
    if v & 0xff00_0000 == 0 {
        bit -= 8;
        v <<= 8;
    }
    if v & 0xf000_0000 == 0 {
        bit -= 4;
        v <<= 4;
    }
    if v & 0xc000_0000 == 0 {
        bit -= 2;
        v <<= 2;
    }
    if v & 0x8000_0000 == 0 {
        bit -= 1;
    }
    bit
}

/// One quantizer's divisors for the non-SIMD `quantize`: `dtbl[i]`, `dtbl[i + 64]` and
/// `dtbl[i + 192]` of the C table (the reciprocal, the rounding correction and the shift).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Divisor {
    /// `UDCTELEM` reciprocal of the divisor.
    recip: u32,
    /// `UDCTELEM` correction: half the divisor, plus one where the reciprocal rounds down.
    corr: u32,
    /// `DCTELEM` shift, `r - 32`.
    shift: i32,
}

/// `compute_reciprocal` (jcdctmgr.c), for the non-SIMD build. `divisor` is `UINT16`.
fn compute_reciprocal(divisor: u16) -> Divisor {
    if divisor == 1 {
        // Unquantized: the reciprocal, correction and shift give the identity.
        return Divisor {
            recip: 1,
            corr: 0,
            shift: -32,
        };
    }
    let b = flss(divisor) - 1;
    let mut r = 32 + b;
    let div = u64::from(divisor);
    let mut fq = (1u64 << r) / div;
    let fr = (1u64 << r) % div;
    let mut c = u32::from(divisor) / 2;
    if fr == 0 {
        // The divisor is a power of two: fq is one bit too large, so adjust.
        fq >>= 1;
        r -= 1;
    } else if fr <= u64::from(u32::from(divisor) / 2) {
        c += 1;
    } else {
        fq += 1;
    }
    Divisor {
        recip: fq as u32,
        corr: c,
        shift: r - 32,
    }
}

/// The divisors of one quantization table (`start_pass_fdctmgr`, `JDCT_ISLOW`, 8-bit).
pub(super) fn make_divisors(qtbl: &JQuantTbl) -> [Divisor; DCTSIZE2] {
    let mut dtbl = [Divisor::default(); DCTSIZE2];
    for (i, d) in dtbl.iter_mut().enumerate() {
        // `qtbl->quantval[i] << 3` is an `int` passed as `UINT16`.
        let value = ((i32::from(qtbl.quantval[i])) << 3) as u16;
        *d = compute_reciprocal(value);
    }
    dtbl
}

/// `convsamp` and `jpeg_fdct_islow` and `quantize` for one block: `samples` are the eight rows
/// of eight samples starting at `(row0, col0)` in a plane of `stride` samples per row.
pub(super) fn forward_dct_block(
    plane: &[u8],
    stride: usize,
    row0: usize,
    col0: usize,
    divisors: &[Divisor; DCTSIZE2],
) -> Block {
    let mut workspace = [0i32; DCTSIZE2];
    for r in 0..8 {
        let base = (row0 + r) * stride + col0;
        for c in 0..8 {
            // `(*elemptr++) - _CENTERJSAMPLE`
            workspace[r * 8 + c] = i32::from(plane[base + c]) - 128;
        }
    }
    fdct_islow(&mut workspace);
    quantize(&workspace, divisors)
}

/// `quantize` (jcdctmgr.c), the `BITS_IN_JSAMPLE == 8` variant: a reciprocal multiply, a rounding
/// correction and a shift, with the sign handled outside the unsigned arithmetic.
pub(super) fn quantize(workspace: &[i32; DCTSIZE2], divisors: &[Divisor; DCTSIZE2]) -> Block {
    let mut out = [0i16; DCTSIZE2];
    for i in 0..DCTSIZE2 {
        let mut temp = workspace[i];
        let d = divisors[i];
        let negative = temp < 0;
        if negative {
            temp = -temp;
        }
        // `product = (UDCTELEM2)(temp + corr) * recip`: the sum is an unsigned int.
        let sum = (temp as u32).wrapping_add(d.corr);
        let product = u64::from(sum) * u64::from(d.recip);
        // `product >>= shift + sizeof(DCTELEM) * 8`
        let shifted = product >> ((d.shift + 32) as u32);
        let mut t = shifted as u32 as i32;
        if negative {
            t = -t;
        }
        out[i] = t as i16;
    }
    out
}
