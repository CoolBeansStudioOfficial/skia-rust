// Port of: libjpeg-turbo src/jidctint.c#L172-L901 (islow 8x8, 7x7, 6x6, 5x5, 3x3) and
//          src/jidctred.c#L120-L407 (4x4, 2x2, 1x1) (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1996, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2015-2016,
// 2018, 2021, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! Inverse DCT methods used for the sizes Skia reaches: `JDCT_ISLOW` at 8x8 and the scaled
//! sizes 1x1 .. 7x7 (`scale_num / 8`).
//!
//! Each method takes the dequantization table (`dct_table`, natural order, `ISLOW_MULT_TYPE`
//! = int) and a block of `JCOEF` values, and writes its `N x N` output as level-shifted sample
//! values through the range-limit table. The output is returned in a small array, so the caller
//! copies it into the sample rows (the C version writes through `output_buf`).
//!
//! `JLONG` is 32 bits (the Windows oracle), so every C multiplication and addition is done with
//! wrapping arithmetic. Corrupt coefficients can overflow in C, and the wrap matches that.

use crate::tables::DCTSIZE2;

/// `CONST_BITS` for the islow, 7x7, 6x6, 5x5, 3x3 and jidctred methods.
const CONST_BITS: i32 = 13;
/// `PASS1_BITS` for 8-bit samples (`jidctint.c` and `jidctred.c` use 2; 1 is for 12-bit).
const PASS1_BITS: i32 = 2;
/// `RANGE_MASK`: `MAXJSAMPLE * 4 + 3`.
const RANGE_MASK: i32 = 255 * 4 + 3;

/// `FIX(x)` at `CONST_BITS` = 13 (`(JLONG)((x) * CONST_SCALE + 0.5)`).
#[inline]
const fn fix(x: f64) -> i32 {
    (x * 8192.0 + 0.5) as i32
}

/// `MULTIPLY16C16` is not used: `MULTIPLY` is `((var) * (const))` in JLONG arithmetic.
#[inline]
fn mul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

/// `LEFT_SHIFT(x, n)`.
#[inline]
fn lshift(a: i32, n: i32) -> i32 {
    (a as u32).wrapping_shl(n as u32) as i32
}

/// `RIGHT_SHIFT(x, n)`: arithmetic shift.
#[inline]
fn rshift(a: i32, n: i32) -> i32 {
    a >> n
}

/// `DESCALE(x, n)`: `RIGHT_SHIFT(x + (ONE << (n - 1)), n)`.
#[inline]
fn descale(x: i32, n: i32) -> i32 {
    rshift(x.wrapping_add(1i32 << (n - 1)), n)
}

/// `DEQUANTIZE(coef, quantval)` = `((ISLOW_MULT_TYPE)(coef)) * (quantval)`.
#[inline]
fn dequantize(coef: i16, quant: i32) -> i32 {
    mul(i32::from(coef), quant)
}

/// The `sample_range_limit` table of `jdmaster.c` (`prepare_range_limit_table`), 8-bit case.
///
/// `at(k)` is `sample_range_limit[k]` for `k` in `-256..=1151`, as the IDCT and colour
/// converters index it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RangeLimit {
    table: Vec<u8>,
}

/// Offset of `sample_range_limit[0]` inside [`RangeLimit::table`].
const RL_BASE: usize = 256;

impl RangeLimit {
    /// Builds the table exactly as `prepare_range_limit_table` does for `data_precision` 8.
    pub(crate) fn new() -> Self {
        // (5 * (MAXJSAMPLE + 1) + CENTERJSAMPLE) bytes, with sample_range_limit at +256.
        let mut t = vec![0u8; 5 * 256 + 128];
        // memset(table - 256, 0, 256): already zero.
        for i in 0..=255usize {
            t[RL_BASE + i] = i as u8;
        }
        // table += CENTERJSAMPLE; for i in [128, 512): table[i] = 255.
        let base2 = RL_BASE + 128;
        for i in 128..512usize {
            t[base2 + i] = 255;
        }
        // memset(table + 512, 0, 384): already zero.
        // memcpy(table + 1024 - 128, sample_range_limit, 128): copies entries 0..127.
        for i in 0..128usize {
            t[base2 + 1024 - 128 + i] = t[RL_BASE + i];
        }
        RangeLimit { table: t }
    }

    /// `sample_range_limit[k]`.
    #[inline]
    pub(crate) fn at(&self, k: i32) -> u8 {
        self.table[(RL_BASE as i32 + k) as usize]
    }

    /// `range_limit[x & RANGE_MASK]` where `range_limit = sample_range_limit + CENTERJSAMPLE`.
    #[inline]
    pub(crate) fn idct(&self, x: i32) -> u8 {
        self.at(128 + (x & RANGE_MASK))
    }
}

/// Output of one inverse DCT: `out[row][col]` for `row, col < size`.
pub(crate) type IdctOut = [[u8; 8]; 8];

/// `_jpeg_idct_islow`: accurate integer 8x8 IDCT.
pub(crate) fn idct_islow(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; DCTSIZE2];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8 {
        // Due to quantization, we will usually find that many of the input coefficients are
        // zero, especially the AC terms.  We can exploit this by short-circuiting the IDCT
        // calculation for any column in which all the AC terms are zero.
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        if (1..8).all(|r| c(r) == 0) {
            // AC terms all zero.
            let dcval = lshift(dequantize(c(0), q(0)), PASS1_BITS);
            for r in 0..8 {
                workspace[ctr + 8 * r] = dcval;
            }
            continue;
        }
        // Even part: reverse the even part of the forward DCT.
        let mut z2 = dequantize(c(2), q(2));
        let mut z3 = dequantize(c(6), q(6));
        let z1 = mul(z2.wrapping_add(z3), fix(0.541196100));
        let tmp2 = z1.wrapping_add(mul(z3, -fix(1.847759065)));
        let tmp3 = z1.wrapping_add(mul(z2, fix(0.765366865)));
        z2 = dequantize(c(0), q(0));
        z3 = dequantize(c(4), q(4));
        let tmp0 = lshift(z2.wrapping_add(z3), CONST_BITS);
        let tmp1 = lshift(z2.wrapping_sub(z3), CONST_BITS);
        let tmp10 = tmp0.wrapping_add(tmp3);
        let tmp13 = tmp0.wrapping_sub(tmp3);
        let tmp11 = tmp1.wrapping_add(tmp2);
        let tmp12 = tmp1.wrapping_sub(tmp2);
        // Odd part.
        let mut tmp0 = dequantize(c(7), q(7));
        let mut tmp1 = dequantize(c(5), q(5));
        let mut tmp2 = dequantize(c(3), q(3));
        let mut tmp3 = dequantize(c(1), q(1));
        let mut z1 = tmp0.wrapping_add(tmp3);
        let mut z2 = tmp1.wrapping_add(tmp2);
        let mut z3 = tmp0.wrapping_add(tmp2);
        let mut z4 = tmp1.wrapping_add(tmp3);
        let z5 = mul(z3.wrapping_add(z4), fix(1.175875602));
        tmp0 = mul(tmp0, fix(0.298631336));
        tmp1 = mul(tmp1, fix(2.053119869));
        tmp2 = mul(tmp2, fix(3.072711026));
        tmp3 = mul(tmp3, fix(1.501321110));
        z1 = mul(z1, -fix(0.899976223));
        z2 = mul(z2, -fix(2.562915447));
        z3 = mul(z3, -fix(1.961570560));
        z4 = mul(z4, -fix(0.390180644));
        z3 = z3.wrapping_add(z5);
        z4 = z4.wrapping_add(z5);
        tmp0 = tmp0.wrapping_add(z1.wrapping_add(z3));
        tmp1 = tmp1.wrapping_add(z2.wrapping_add(z4));
        tmp2 = tmp2.wrapping_add(z2.wrapping_add(z3));
        tmp3 = tmp3.wrapping_add(z1.wrapping_add(z4));
        // Final output stage: inputs are tmp10..tmp13, tmp0..tmp3.
        let sh = CONST_BITS - PASS1_BITS;
        workspace[ctr] = descale(tmp10.wrapping_add(tmp3), sh);
        workspace[ctr + 8 * 7] = descale(tmp10.wrapping_sub(tmp3), sh);
        workspace[ctr + 8] = descale(tmp11.wrapping_add(tmp2), sh);
        workspace[ctr + 8 * 6] = descale(tmp11.wrapping_sub(tmp2), sh);
        workspace[ctr + 8 * 2] = descale(tmp12.wrapping_add(tmp1), sh);
        workspace[ctr + 8 * 5] = descale(tmp12.wrapping_sub(tmp1), sh);
        workspace[ctr + 8 * 3] = descale(tmp13.wrapping_add(tmp0), sh);
        workspace[ctr + 8 * 4] = descale(tmp13.wrapping_sub(tmp0), sh);
    }
    // Pass 2: process rows from work array, store into output array.
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..8 {
        let w = |c: usize| workspace[8 * ctr + c];
        // Rows of zeros can be exploited in the same way as we did with columns.
        if (1..8).all(|c| w(c) == 0) {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr] = [dcval; 8];
            continue;
        }
        // Even part.
        let z2 = w(2);
        let z3 = w(6);
        let z1 = mul(z2.wrapping_add(z3), fix(0.541196100));
        let tmp2 = z1.wrapping_add(mul(z3, -fix(1.847759065)));
        let tmp3 = z1.wrapping_add(mul(z2, fix(0.765366865)));
        let tmp0 = lshift(w(0).wrapping_add(w(4)), CONST_BITS);
        let tmp1 = lshift(w(0).wrapping_sub(w(4)), CONST_BITS);
        let tmp10 = tmp0.wrapping_add(tmp3);
        let tmp13 = tmp0.wrapping_sub(tmp3);
        let tmp11 = tmp1.wrapping_add(tmp2);
        let tmp12 = tmp1.wrapping_sub(tmp2);
        // Odd part.
        let mut tmp0 = w(7);
        let mut tmp1 = w(5);
        let mut tmp2 = w(3);
        let mut tmp3 = w(1);
        let mut z1 = tmp0.wrapping_add(tmp3);
        let mut z2b = tmp1.wrapping_add(tmp2);
        let mut z3b = tmp0.wrapping_add(tmp2);
        let mut z4 = tmp1.wrapping_add(tmp3);
        let z5 = mul(z3b.wrapping_add(z4), fix(1.175875602));
        tmp0 = mul(tmp0, fix(0.298631336));
        tmp1 = mul(tmp1, fix(2.053119869));
        tmp2 = mul(tmp2, fix(3.072711026));
        tmp3 = mul(tmp3, fix(1.501321110));
        z1 = mul(z1, -fix(0.899976223));
        z2b = mul(z2b, -fix(2.562915447));
        z3b = mul(z3b, -fix(1.961570560));
        z4 = mul(z4, -fix(0.390180644));
        z3b = z3b.wrapping_add(z5);
        z4 = z4.wrapping_add(z5);
        tmp0 = tmp0.wrapping_add(z1.wrapping_add(z3b));
        tmp1 = tmp1.wrapping_add(z2b.wrapping_add(z4));
        tmp2 = tmp2.wrapping_add(z2b.wrapping_add(z3b));
        tmp3 = tmp3.wrapping_add(z1.wrapping_add(z4));
        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(descale(tmp10.wrapping_add(tmp3), sh));
        out[ctr][7] = rl.idct(descale(tmp10.wrapping_sub(tmp3), sh));
        out[ctr][1] = rl.idct(descale(tmp11.wrapping_add(tmp2), sh));
        out[ctr][6] = rl.idct(descale(tmp11.wrapping_sub(tmp2), sh));
        out[ctr][2] = rl.idct(descale(tmp12.wrapping_add(tmp1), sh));
        out[ctr][5] = rl.idct(descale(tmp12.wrapping_sub(tmp1), sh));
        out[ctr][3] = rl.idct(descale(tmp13.wrapping_add(tmp0), sh));
        out[ctr][4] = rl.idct(descale(tmp13.wrapping_sub(tmp0), sh));
    }
    out
}

/// `_jpeg_idct_7x7`.
pub(crate) fn idct_7x7(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; 7 * 7];
    for ctr in 0..7 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp13 = dequantize(c(0), q(0));
        tmp13 = lshift(tmp13, CONST_BITS);
        // Add fudge factor here for final descale.
        tmp13 = tmp13.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let z1 = dequantize(c(2), q(2));
        let mut z2 = dequantize(c(4), q(4));
        let z3 = dequantize(c(6), q(6));
        let mut tmp10 = mul(z2.wrapping_sub(z3), fix(0.881747734));
        let mut tmp12 = mul(z1.wrapping_sub(z2), fix(0.314692123));
        let tmp11 = tmp10.wrapping_add(tmp12).wrapping_add(tmp13).wrapping_sub(mul(z2, fix(1.841218003)));
        let mut tmp0 = z1.wrapping_add(z3);
        z2 = z2.wrapping_sub(tmp0);
        tmp0 = mul(tmp0, fix(1.274162392)).wrapping_add(tmp13);
        tmp10 = tmp10.wrapping_add(tmp0.wrapping_sub(mul(z3, fix(0.077722536))));
        tmp12 = tmp12.wrapping_add(tmp0.wrapping_sub(mul(z1, fix(2.470602249))));
        tmp13 = tmp13.wrapping_add(mul(z2, fix(1.414213562)));

        let z1 = dequantize(c(1), q(1));
        let z2 = dequantize(c(3), q(3));
        let z3 = dequantize(c(5), q(5));
        let mut tmp1 = mul(z1.wrapping_add(z2), fix(0.935414347));
        let mut tmp2 = mul(z1.wrapping_sub(z2), fix(0.170262339));
        tmp0 = tmp1.wrapping_sub(tmp2);
        tmp1 = tmp1.wrapping_add(tmp2);
        tmp2 = mul(z2.wrapping_add(z3), -fix(1.378756276));
        tmp1 = tmp1.wrapping_add(tmp2);
        let z2b = mul(z1.wrapping_add(z3), fix(0.613604268));
        tmp0 = tmp0.wrapping_add(z2b);
        tmp2 = tmp2.wrapping_add(z2b.wrapping_add(mul(z3, fix(1.870828693))));

        let sh = CONST_BITS - PASS1_BITS;
        workspace[ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[ctr + 7 * 6] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[ctr + 7] = rshift(tmp11.wrapping_add(tmp1), sh);
        workspace[ctr + 7 * 5] = rshift(tmp11.wrapping_sub(tmp1), sh);
        workspace[ctr + 7 * 2] = rshift(tmp12.wrapping_add(tmp2), sh);
        workspace[ctr + 7 * 4] = rshift(tmp12.wrapping_sub(tmp2), sh);
        workspace[ctr + 7 * 3] = rshift(tmp13, sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..7 {
        let w = |c: usize| workspace[7 * ctr + c];
        let mut tmp13 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp13 = lshift(tmp13, CONST_BITS);
        let z1 = w(2);
        let mut z2 = w(4);
        let z3 = w(6);
        let mut tmp10 = mul(z2.wrapping_sub(z3), fix(0.881747734));
        let mut tmp12 = mul(z1.wrapping_sub(z2), fix(0.314692123));
        let tmp11 = tmp10.wrapping_add(tmp12).wrapping_add(tmp13).wrapping_sub(mul(z2, fix(1.841218003)));
        let mut tmp0 = z1.wrapping_add(z3);
        z2 = z2.wrapping_sub(tmp0);
        tmp0 = mul(tmp0, fix(1.274162392)).wrapping_add(tmp13);
        tmp10 = tmp10.wrapping_add(tmp0.wrapping_sub(mul(z3, fix(0.077722536))));
        tmp12 = tmp12.wrapping_add(tmp0.wrapping_sub(mul(z1, fix(2.470602249))));
        tmp13 = tmp13.wrapping_add(mul(z2, fix(1.414213562)));

        let z1 = w(1);
        let z2 = w(3);
        let z3 = w(5);
        let mut tmp1 = mul(z1.wrapping_add(z2), fix(0.935414347));
        let mut tmp2 = mul(z1.wrapping_sub(z2), fix(0.170262339));
        tmp0 = tmp1.wrapping_sub(tmp2);
        tmp1 = tmp1.wrapping_add(tmp2);
        tmp2 = mul(z2.wrapping_add(z3), -fix(1.378756276));
        tmp1 = tmp1.wrapping_add(tmp2);
        let z2b = mul(z1.wrapping_add(z3), fix(0.613604268));
        tmp0 = tmp0.wrapping_add(z2b);
        tmp2 = tmp2.wrapping_add(z2b.wrapping_add(mul(z3, fix(1.870828693))));

        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(rshift(tmp10.wrapping_add(tmp0), sh));
        out[ctr][6] = rl.idct(rshift(tmp10.wrapping_sub(tmp0), sh));
        out[ctr][1] = rl.idct(rshift(tmp11.wrapping_add(tmp1), sh));
        out[ctr][5] = rl.idct(rshift(tmp11.wrapping_sub(tmp1), sh));
        out[ctr][2] = rl.idct(rshift(tmp12.wrapping_add(tmp2), sh));
        out[ctr][4] = rl.idct(rshift(tmp12.wrapping_sub(tmp2), sh));
        out[ctr][3] = rl.idct(rshift(tmp13, sh));
    }
    out
}

/// `_jpeg_idct_6x6`.
pub(crate) fn idct_6x6(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; 6 * 6];
    for ctr in 0..6 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp0 = dequantize(c(0), q(0));
        tmp0 = lshift(tmp0, CONST_BITS);
        tmp0 = tmp0.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp2 = dequantize(c(4), q(4));
        let tmp10 = mul(tmp2, fix(0.707106781));
        let mut tmp1 = tmp0.wrapping_add(tmp10);
        let tmp11 = rshift(tmp0.wrapping_sub(tmp10).wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        let tmp10b = dequantize(c(2), q(2));
        let tmp0b = mul(tmp10b, fix(1.224744871));
        let tmp10 = tmp1.wrapping_add(tmp0b);
        let tmp12 = tmp1.wrapping_sub(tmp0b);
        let z1 = dequantize(c(1), q(1));
        let z2 = dequantize(c(3), q(3));
        let z3 = dequantize(c(5), q(5));
        tmp1 = mul(z1.wrapping_add(z3), fix(0.366025404));
        let tmp0 = tmp1.wrapping_add(lshift(z1.wrapping_add(z2), CONST_BITS));
        let tmp2 = tmp1.wrapping_add(lshift(z3.wrapping_sub(z2), CONST_BITS));
        let tmp1 = lshift(z1.wrapping_sub(z2).wrapping_sub(z3), PASS1_BITS);

        let sh = CONST_BITS - PASS1_BITS;
        workspace[6 * 0 + ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[6 * 5 + ctr] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[6 * 1 + ctr] = tmp11.wrapping_add(tmp1);
        workspace[6 * 4 + ctr] = tmp11.wrapping_sub(tmp1);
        workspace[6 * 2 + ctr] = rshift(tmp12.wrapping_add(tmp2), sh);
        workspace[6 * 3 + ctr] = rshift(tmp12.wrapping_sub(tmp2), sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..6 {
        let w = |c: usize| workspace[6 * ctr + c];
        let mut tmp0 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp0 = lshift(tmp0, CONST_BITS);
        let tmp2 = w(4);
        let tmp10 = mul(tmp2, fix(0.707106781));
        let mut tmp1 = tmp0.wrapping_add(tmp10);
        let tmp11 = tmp0.wrapping_sub(tmp10).wrapping_sub(tmp10);
        let tmp10b = w(2);
        let tmp0b = mul(tmp10b, fix(1.224744871));
        let tmp10 = tmp1.wrapping_add(tmp0b);
        let tmp12 = tmp1.wrapping_sub(tmp0b);
        let z1 = w(1);
        let z2 = w(3);
        let z3 = w(5);
        tmp1 = mul(z1.wrapping_add(z3), fix(0.366025404));
        let tmp0 = tmp1.wrapping_add(lshift(z1.wrapping_add(z2), CONST_BITS));
        let tmp2 = tmp1.wrapping_add(lshift(z3.wrapping_sub(z2), CONST_BITS));
        let tmp1 = lshift(z1.wrapping_sub(z2).wrapping_sub(z3), CONST_BITS);

        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(rshift(tmp10.wrapping_add(tmp0), sh));
        out[ctr][5] = rl.idct(rshift(tmp10.wrapping_sub(tmp0), sh));
        out[ctr][1] = rl.idct(rshift(tmp11.wrapping_add(tmp1), sh));
        out[ctr][4] = rl.idct(rshift(tmp11.wrapping_sub(tmp1), sh));
        out[ctr][2] = rl.idct(rshift(tmp12.wrapping_add(tmp2), sh));
        out[ctr][3] = rl.idct(rshift(tmp12.wrapping_sub(tmp2), sh));
    }
    out
}

/// `_jpeg_idct_5x5`.
pub(crate) fn idct_5x5(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; 5 * 5];
    for ctr in 0..5 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp12 = dequantize(c(0), q(0));
        tmp12 = lshift(tmp12, CONST_BITS);
        tmp12 = tmp12.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp0 = dequantize(c(2), q(2));
        let tmp1 = dequantize(c(4), q(4));
        let z1 = mul(tmp0.wrapping_add(tmp1), fix(0.790569415));
        let z2 = mul(tmp0.wrapping_sub(tmp1), fix(0.353553391));
        let z3 = tmp12.wrapping_add(z2);
        let tmp10 = z3.wrapping_add(z1);
        let tmp11 = z3.wrapping_sub(z1);
        tmp12 = tmp12.wrapping_sub(lshift(z2, 2));
        let z2 = dequantize(c(1), q(1));
        let z3 = dequantize(c(3), q(3));
        let z1 = mul(z2.wrapping_add(z3), fix(0.831253876));
        let tmp0 = z1.wrapping_add(mul(z2, fix(0.513743148)));
        let tmp1 = z1.wrapping_sub(mul(z3, fix(2.176250899)));
        let sh = CONST_BITS - PASS1_BITS;
        workspace[5 * 0 + ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[5 * 4 + ctr] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[5 * 1 + ctr] = rshift(tmp11.wrapping_add(tmp1), sh);
        workspace[5 * 3 + ctr] = rshift(tmp11.wrapping_sub(tmp1), sh);
        workspace[5 * 2 + ctr] = rshift(tmp12, sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..5 {
        let w = |c: usize| workspace[5 * ctr + c];
        let mut tmp12 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp12 = lshift(tmp12, CONST_BITS);
        let tmp0 = w(2);
        let tmp1 = w(4);
        let z1 = mul(tmp0.wrapping_add(tmp1), fix(0.790569415));
        let z2 = mul(tmp0.wrapping_sub(tmp1), fix(0.353553391));
        let z3 = tmp12.wrapping_add(z2);
        let tmp10 = z3.wrapping_add(z1);
        let tmp11 = z3.wrapping_sub(z1);
        tmp12 = tmp12.wrapping_sub(lshift(z2, 2));
        let z2 = w(1);
        let z3 = w(3);
        let z1 = mul(z2.wrapping_add(z3), fix(0.831253876));
        let tmp0 = z1.wrapping_add(mul(z2, fix(0.513743148)));
        let tmp1 = z1.wrapping_sub(mul(z3, fix(2.176250899)));
        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(rshift(tmp10.wrapping_add(tmp0), sh));
        out[ctr][4] = rl.idct(rshift(tmp10.wrapping_sub(tmp0), sh));
        out[ctr][1] = rl.idct(rshift(tmp11.wrapping_add(tmp1), sh));
        out[ctr][3] = rl.idct(rshift(tmp11.wrapping_sub(tmp1), sh));
        out[ctr][2] = rl.idct(rshift(tmp12, sh));
    }
    out
}

/// `_jpeg_idct_3x3`.
pub(crate) fn idct_3x3(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; 3 * 3];
    for ctr in 0..3 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp0 = dequantize(c(0), q(0));
        tmp0 = lshift(tmp0, CONST_BITS);
        tmp0 = tmp0.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp2 = dequantize(c(2), q(2));
        let tmp12 = mul(tmp2, fix(0.707106781));
        let tmp10 = tmp0.wrapping_add(tmp12);
        let tmp2 = tmp0.wrapping_sub(tmp12).wrapping_sub(tmp12);
        let tmp12 = dequantize(c(1), q(1));
        let tmp0 = mul(tmp12, fix(1.224744871));
        let sh = CONST_BITS - PASS1_BITS;
        workspace[3 * 0 + ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[3 * 2 + ctr] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[3 * 1 + ctr] = rshift(tmp2, sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..3 {
        let w = |c: usize| workspace[3 * ctr + c];
        let mut tmp0 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp0 = lshift(tmp0, CONST_BITS);
        let tmp2 = w(2);
        let tmp12 = mul(tmp2, fix(0.707106781));
        let tmp10 = tmp0.wrapping_add(tmp12);
        let tmp2 = tmp0.wrapping_sub(tmp12).wrapping_sub(tmp12);
        let tmp12 = w(1);
        let tmp0 = mul(tmp12, fix(1.224744871));
        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(rshift(tmp10.wrapping_add(tmp0), sh));
        out[ctr][2] = rl.idct(rshift(tmp10.wrapping_sub(tmp0), sh));
        out[ctr][1] = rl.idct(rshift(tmp2, sh));
    }
    out
}

/// `_jpeg_idct_4x4` (jidctred.c): 4x4 output from the top-left 4x4 coefficients.
pub(crate) fn idct_4x4(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    // CONST_BITS 13, PASS1_BITS 1 for the reduced IDCTs (jidctred.c).
    let mut workspace = [0i32; 8 * 4];
    for ctr in 0..8usize {
        // Skip the unused row (ctr == DCTSIZE - 4).
        if ctr == 8 - 4 {
            continue;
        }
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        if c(1) == 0 && c(2) == 0 && c(3) == 0 && c(5) == 0 && c(6) == 0 && c(7) == 0 {
            // AC terms all zero.
            let dcval = lshift(dequantize(c(0), q(0)), PASS1_BITS);
            workspace[ctr] = dcval;
            workspace[ctr + 8] = dcval;
            workspace[ctr + 16] = dcval;
            workspace[ctr + 24] = dcval;
            continue;
        }
        // Even part.
        let mut tmp0 = dequantize(c(0), q(0));
        tmp0 = lshift(tmp0, CONST_BITS + 1);
        let z2 = dequantize(c(2), q(2));
        let z3 = dequantize(c(6), q(6));
        let tmp2 = mul(z2, fix(1.847759065)).wrapping_add(mul(z3, -fix(0.765366865)));
        let tmp10 = tmp0.wrapping_add(tmp2);
        let tmp12 = tmp0.wrapping_sub(tmp2);
        // Odd part.
        let z1 = dequantize(c(7), q(7));
        let z2 = dequantize(c(5), q(5));
        let z3 = dequantize(c(3), q(3));
        let z4 = dequantize(c(1), q(1));
        let tmp0 = mul(z1, -fix(0.211164243))
            .wrapping_add(mul(z2, fix(1.451774981)))
            .wrapping_add(mul(z3, -fix(2.172734803)))
            .wrapping_add(mul(z4, fix(1.061594337)));
        let tmp2 = mul(z1, -fix(0.509795579))
            .wrapping_add(mul(z2, -fix(0.601344887)))
            .wrapping_add(mul(z3, fix(0.899976223)))
            .wrapping_add(mul(z4, fix(2.562915447)));
        let sh = CONST_BITS - PASS1_BITS + 1;
        workspace[ctr] = descale(tmp10.wrapping_add(tmp2), sh);
        workspace[ctr + 8 * 3] = descale(tmp10.wrapping_sub(tmp2), sh);
        workspace[ctr + 8] = descale(tmp12.wrapping_add(tmp0), sh);
        workspace[ctr + 8 * 2] = descale(tmp12.wrapping_sub(tmp0), sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..4 {
        let w = |c: usize| workspace[8 * ctr + c];
        if w(1) == 0 && w(2) == 0 && w(3) == 0 && w(5) == 0 && w(6) == 0 && w(7) == 0 {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr][..4].fill(dcval);
            continue;
        }
        let mut tmp0 = lshift(w(0), CONST_BITS + 1);
        let tmp2 = mul(w(2), fix(1.847759065)).wrapping_add(mul(w(6), -fix(0.765366865)));
        let tmp10 = tmp0.wrapping_add(tmp2);
        let tmp12 = tmp0.wrapping_sub(tmp2);
        let z1 = w(7);
        let z2 = w(5);
        let z3 = w(3);
        let z4 = w(1);
        tmp0 = mul(z1, -fix(0.211164243))
            .wrapping_add(mul(z2, fix(1.451774981)))
            .wrapping_add(mul(z3, -fix(2.172734803)))
            .wrapping_add(mul(z4, fix(1.061594337)));
        let tmp2 = mul(z1, -fix(0.509795579))
            .wrapping_add(mul(z2, -fix(0.601344887)))
            .wrapping_add(mul(z3, fix(0.899976223)))
            .wrapping_add(mul(z4, fix(2.562915447)));
        let sh = CONST_BITS + PASS1_BITS + 3 + 1;
        out[ctr][0] = rl.idct(descale(tmp10.wrapping_add(tmp2), sh));
        out[ctr][3] = rl.idct(descale(tmp10.wrapping_sub(tmp2), sh));
        out[ctr][1] = rl.idct(descale(tmp12.wrapping_add(tmp0), sh));
        out[ctr][2] = rl.idct(descale(tmp12.wrapping_sub(tmp0), sh));
    }
    out
}

/// `_jpeg_idct_2x2` (jidctred.c).
pub(crate) fn idct_2x2(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let mut workspace = [0i32; 8 * 2];
    for ctr in 0..8usize {
        if ctr == 8 - 2 || ctr == 8 - 4 || ctr == 8 - 6 {
            continue;
        }
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        if c(1) == 0 && c(3) == 0 && c(5) == 0 && c(7) == 0 {
            let dcval = lshift(dequantize(c(0), q(0)), PASS1_BITS);
            workspace[ctr] = dcval;
            workspace[ctr + 8] = dcval;
            continue;
        }
        let z1 = dequantize(c(0), q(0));
        let tmp10 = lshift(z1, CONST_BITS + 2);
        let z1 = dequantize(c(7), q(7));
        let mut tmp0 = mul(z1, -fix(0.720959822));
        let z1 = dequantize(c(5), q(5));
        tmp0 = tmp0.wrapping_add(mul(z1, fix(0.850430095)));
        let z1 = dequantize(c(3), q(3));
        tmp0 = tmp0.wrapping_add(mul(z1, -fix(1.272758580)));
        let z1 = dequantize(c(1), q(1));
        tmp0 = tmp0.wrapping_add(mul(z1, fix(3.624509785)));
        let sh = CONST_BITS - PASS1_BITS + 2;
        workspace[ctr] = descale(tmp10.wrapping_add(tmp0), sh);
        workspace[ctr + 8] = descale(tmp10.wrapping_sub(tmp0), sh);
    }
    let mut out = [[0u8; 8]; 8];
    for ctr in 0..2 {
        let w = |c: usize| workspace[8 * ctr + c];
        if w(1) == 0 && w(3) == 0 && w(5) == 0 && w(7) == 0 {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr][0] = dcval;
            out[ctr][1] = dcval;
            continue;
        }
        let tmp10 = lshift(w(0), CONST_BITS + 2);
        let tmp0 = mul(w(7), -fix(0.720959822))
            .wrapping_add(mul(w(5), fix(0.850430095)))
            .wrapping_add(mul(w(3), -fix(1.272758580)))
            .wrapping_add(mul(w(1), fix(3.624509785)));
        let sh = CONST_BITS + PASS1_BITS + 3 + 2;
        out[ctr][0] = rl.idct(descale(tmp10.wrapping_add(tmp0), sh));
        out[ctr][1] = rl.idct(descale(tmp10.wrapping_sub(tmp0), sh));
    }
    out
}

/// `_jpeg_idct_1x1` (jidctred.c).
pub(crate) fn idct_1x1(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], rl: &RangeLimit) -> IdctOut {
    let dcval = dequantize(coef[0], quant[0]);
    let dcval = descale(dcval, 3);
    let mut out = [[0u8; 8]; 8];
    out[0][0] = rl.idct(dcval);
    out
}

/// Dispatches to the method for `dct_scaled_size` (1..=8), as `jddctmgr.c` does.
pub(crate) fn idct_for_size(
    size: i32,
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> Option<IdctOut> {
    Some(match size {
        1 => idct_1x1(coef, quant, rl),
        2 => idct_2x2(coef, quant, rl),
        3 => idct_3x3(coef, quant, rl),
        4 => idct_4x4(coef, quant, rl),
        5 => idct_5x5(coef, quant, rl),
        6 => idct_6x6(coef, quant, rl),
        7 => idct_7x7(coef, quant, rl),
        8 => idct_islow(coef, quant, rl),
        _ => return None,
    })
}
