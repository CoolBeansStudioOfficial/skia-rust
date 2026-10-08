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

// Clippy (pedantic) allows, for this module. Each one fires on the C arithmetic and naming this
// module mirrors, and the code is kept as the C writes it so it can be checked line by line:
// JLONG/int/JDIMENSION casts (sign, truncation and wrap), C operator precedence and identity
// terms that come out of macros (`x * 1`, `0 * n`), C loop shapes (`needless_range_loop`,
// `explicit_counter_loop`, `collapsible_if`, `match_same_arms`), the C variable names
// (`similar_names`, `struct_field_names`), libjpeg's constants written as in jdct.h
// (`approx_constant`, `unreadable_literal`), functions whose C form returns a status that
// this path never sets (`unnecessary_wraps`), and the long C routines (`too_many_lines`,
// `too_many_arguments`). Error docs point at the `Error` variants, which name the C codes.
#![allow(
    clippy::approx_constant,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::erasing_op,
    clippy::explicit_counter_loop,
    clippy::identity_op,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_range_loop,
    clippy::precedence,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self
)]

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
pub(crate) type IdctOut = [[u8; 16]; 16];

/// `_jpeg_idct_islow`: accurate integer 8x8 IDCT.
pub(crate) fn idct_islow(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
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
        let z1 = mul(z2.wrapping_add(z3), fix(0.541_196_100));
        let tmp2 = z1.wrapping_add(mul(z3, -fix(1.847_759_065)));
        let tmp3 = z1.wrapping_add(mul(z2, fix(0.765_366_865)));
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
        let z5 = mul(z3.wrapping_add(z4), fix(1.175_875_602));
        tmp0 = mul(tmp0, fix(0.298_631_336));
        tmp1 = mul(tmp1, fix(2.053_119_869));
        tmp2 = mul(tmp2, fix(3.072_711_026));
        tmp3 = mul(tmp3, fix(1.501_321_110));
        z1 = mul(z1, -fix(0.899_976_223));
        z2 = mul(z2, -fix(2.562_915_447));
        z3 = mul(z3, -fix(1.961_570_560));
        z4 = mul(z4, -fix(0.390_180_644));
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
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..8 {
        let w = |c: usize| workspace[8 * ctr + c];
        // Rows of zeros can be exploited in the same way as we did with columns.
        if (1..8).all(|c| w(c) == 0) {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr][..8].fill(dcval);
            continue;
        }
        // Even part.
        let z2 = w(2);
        let z3 = w(6);
        let z1 = mul(z2.wrapping_add(z3), fix(0.541_196_100));
        let tmp2 = z1.wrapping_add(mul(z3, -fix(1.847_759_065)));
        let tmp3 = z1.wrapping_add(mul(z2, fix(0.765_366_865)));
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
        let z5 = mul(z3b.wrapping_add(z4), fix(1.175_875_602));
        tmp0 = mul(tmp0, fix(0.298_631_336));
        tmp1 = mul(tmp1, fix(2.053_119_869));
        tmp2 = mul(tmp2, fix(3.072_711_026));
        tmp3 = mul(tmp3, fix(1.501_321_110));
        z1 = mul(z1, -fix(0.899_976_223));
        z2b = mul(z2b, -fix(2.562_915_447));
        z3b = mul(z3b, -fix(1.961_570_560));
        z4 = mul(z4, -fix(0.390_180_644));
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
pub(crate) fn idct_7x7(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
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
        let mut tmp10 = mul(z2.wrapping_sub(z3), fix(0.881_747_734));
        let mut tmp12 = mul(z1.wrapping_sub(z2), fix(0.314_692_123));
        let tmp11 = tmp10
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z2, fix(1.841_218_003)));
        let mut tmp0 = z1.wrapping_add(z3);
        z2 = z2.wrapping_sub(tmp0);
        tmp0 = mul(tmp0, fix(1.274_162_392)).wrapping_add(tmp13);
        tmp10 = tmp10.wrapping_add(tmp0.wrapping_sub(mul(z3, fix(0.077_722_536))));
        tmp12 = tmp12.wrapping_add(tmp0.wrapping_sub(mul(z1, fix(2.470_602_249))));
        tmp13 = tmp13.wrapping_add(mul(z2, fix(1.414_213_562)));

        let z1 = dequantize(c(1), q(1));
        let z2 = dequantize(c(3), q(3));
        let z3 = dequantize(c(5), q(5));
        let mut tmp1 = mul(z1.wrapping_add(z2), fix(0.935_414_347));
        let mut tmp2 = mul(z1.wrapping_sub(z2), fix(0.170_262_339));
        tmp0 = tmp1.wrapping_sub(tmp2);
        tmp1 = tmp1.wrapping_add(tmp2);
        tmp2 = mul(z2.wrapping_add(z3), -fix(1.378_756_276));
        tmp1 = tmp1.wrapping_add(tmp2);
        let z2b = mul(z1.wrapping_add(z3), fix(0.613_604_268));
        tmp0 = tmp0.wrapping_add(z2b);
        tmp2 = tmp2.wrapping_add(z2b.wrapping_add(mul(z3, fix(1.870_828_693))));

        let sh = CONST_BITS - PASS1_BITS;
        workspace[ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[ctr + 7 * 6] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[ctr + 7] = rshift(tmp11.wrapping_add(tmp1), sh);
        workspace[ctr + 7 * 5] = rshift(tmp11.wrapping_sub(tmp1), sh);
        workspace[ctr + 7 * 2] = rshift(tmp12.wrapping_add(tmp2), sh);
        workspace[ctr + 7 * 4] = rshift(tmp12.wrapping_sub(tmp2), sh);
        workspace[ctr + 7 * 3] = rshift(tmp13, sh);
    }
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..7 {
        let w = |c: usize| workspace[7 * ctr + c];
        let mut tmp13 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp13 = lshift(tmp13, CONST_BITS);
        let z1 = w(2);
        let mut z2 = w(4);
        let z3 = w(6);
        let mut tmp10 = mul(z2.wrapping_sub(z3), fix(0.881_747_734));
        let mut tmp12 = mul(z1.wrapping_sub(z2), fix(0.314_692_123));
        let tmp11 = tmp10
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z2, fix(1.841_218_003)));
        let mut tmp0 = z1.wrapping_add(z3);
        z2 = z2.wrapping_sub(tmp0);
        tmp0 = mul(tmp0, fix(1.274_162_392)).wrapping_add(tmp13);
        tmp10 = tmp10.wrapping_add(tmp0.wrapping_sub(mul(z3, fix(0.077_722_536))));
        tmp12 = tmp12.wrapping_add(tmp0.wrapping_sub(mul(z1, fix(2.470_602_249))));
        tmp13 = tmp13.wrapping_add(mul(z2, fix(1.414_213_562)));

        let z1 = w(1);
        let z2 = w(3);
        let z3 = w(5);
        let mut tmp1 = mul(z1.wrapping_add(z2), fix(0.935_414_347));
        let mut tmp2 = mul(z1.wrapping_sub(z2), fix(0.170_262_339));
        tmp0 = tmp1.wrapping_sub(tmp2);
        tmp1 = tmp1.wrapping_add(tmp2);
        tmp2 = mul(z2.wrapping_add(z3), -fix(1.378_756_276));
        tmp1 = tmp1.wrapping_add(tmp2);
        let z2b = mul(z1.wrapping_add(z3), fix(0.613_604_268));
        tmp0 = tmp0.wrapping_add(z2b);
        tmp2 = tmp2.wrapping_add(z2b.wrapping_add(mul(z3, fix(1.870_828_693))));

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
pub(crate) fn idct_6x6(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 6 * 6];
    for ctr in 0..6 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp0 = dequantize(c(0), q(0));
        tmp0 = lshift(tmp0, CONST_BITS);
        tmp0 = tmp0.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp2 = dequantize(c(4), q(4));
        let tmp10 = mul(tmp2, fix(0.707_106_781));
        let mut tmp1 = tmp0.wrapping_add(tmp10);
        let tmp11 = rshift(
            tmp0.wrapping_sub(tmp10).wrapping_sub(tmp10),
            CONST_BITS - PASS1_BITS,
        );
        let tmp10b = dequantize(c(2), q(2));
        let tmp0b = mul(tmp10b, fix(1.224_744_871));
        let tmp10 = tmp1.wrapping_add(tmp0b);
        let tmp12 = tmp1.wrapping_sub(tmp0b);
        let z1 = dequantize(c(1), q(1));
        let z2 = dequantize(c(3), q(3));
        let z3 = dequantize(c(5), q(5));
        tmp1 = mul(z1.wrapping_add(z3), fix(0.366_025_404));
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
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..6 {
        let w = |c: usize| workspace[6 * ctr + c];
        let mut tmp0 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp0 = lshift(tmp0, CONST_BITS);
        let tmp2 = w(4);
        let tmp10 = mul(tmp2, fix(0.707_106_781));
        let mut tmp1 = tmp0.wrapping_add(tmp10);
        let tmp11 = tmp0.wrapping_sub(tmp10).wrapping_sub(tmp10);
        let tmp10b = w(2);
        let tmp0b = mul(tmp10b, fix(1.224_744_871));
        let tmp10 = tmp1.wrapping_add(tmp0b);
        let tmp12 = tmp1.wrapping_sub(tmp0b);
        let z1 = w(1);
        let z2 = w(3);
        let z3 = w(5);
        tmp1 = mul(z1.wrapping_add(z3), fix(0.366_025_404));
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
pub(crate) fn idct_5x5(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 5 * 5];
    for ctr in 0..5 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp12 = dequantize(c(0), q(0));
        tmp12 = lshift(tmp12, CONST_BITS);
        tmp12 = tmp12.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp0 = dequantize(c(2), q(2));
        let tmp1 = dequantize(c(4), q(4));
        let z1 = mul(tmp0.wrapping_add(tmp1), fix(0.790_569_415));
        let z2 = mul(tmp0.wrapping_sub(tmp1), fix(0.353_553_391));
        let z3 = tmp12.wrapping_add(z2);
        let tmp10 = z3.wrapping_add(z1);
        let tmp11 = z3.wrapping_sub(z1);
        tmp12 = tmp12.wrapping_sub(lshift(z2, 2));
        let z2 = dequantize(c(1), q(1));
        let z3 = dequantize(c(3), q(3));
        let z1 = mul(z2.wrapping_add(z3), fix(0.831_253_876));
        let tmp0 = z1.wrapping_add(mul(z2, fix(0.513_743_148)));
        let tmp1 = z1.wrapping_sub(mul(z3, fix(2.176_250_899)));
        let sh = CONST_BITS - PASS1_BITS;
        workspace[5 * 0 + ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[5 * 4 + ctr] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[5 * 1 + ctr] = rshift(tmp11.wrapping_add(tmp1), sh);
        workspace[5 * 3 + ctr] = rshift(tmp11.wrapping_sub(tmp1), sh);
        workspace[5 * 2 + ctr] = rshift(tmp12, sh);
    }
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..5 {
        let w = |c: usize| workspace[5 * ctr + c];
        let mut tmp12 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp12 = lshift(tmp12, CONST_BITS);
        let tmp0 = w(2);
        let tmp1 = w(4);
        let z1 = mul(tmp0.wrapping_add(tmp1), fix(0.790_569_415));
        let z2 = mul(tmp0.wrapping_sub(tmp1), fix(0.353_553_391));
        let z3 = tmp12.wrapping_add(z2);
        let tmp10 = z3.wrapping_add(z1);
        let tmp11 = z3.wrapping_sub(z1);
        tmp12 = tmp12.wrapping_sub(lshift(z2, 2));
        let z2 = w(1);
        let z3 = w(3);
        let z1 = mul(z2.wrapping_add(z3), fix(0.831_253_876));
        let tmp0 = z1.wrapping_add(mul(z2, fix(0.513_743_148)));
        let tmp1 = z1.wrapping_sub(mul(z3, fix(2.176_250_899)));
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
pub(crate) fn idct_3x3(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 3 * 3];
    for ctr in 0..3 {
        let c = |r: usize| coef[ctr + 8 * r];
        let q = |r: usize| quant[ctr + 8 * r];
        let mut tmp0 = dequantize(c(0), q(0));
        tmp0 = lshift(tmp0, CONST_BITS);
        tmp0 = tmp0.wrapping_add(1i32 << (CONST_BITS - PASS1_BITS - 1));
        let tmp2 = dequantize(c(2), q(2));
        let tmp12 = mul(tmp2, fix(0.707_106_781));
        let tmp10 = tmp0.wrapping_add(tmp12);
        let tmp2 = tmp0.wrapping_sub(tmp12).wrapping_sub(tmp12);
        let tmp12 = dequantize(c(1), q(1));
        let tmp0 = mul(tmp12, fix(1.224_744_871));
        let sh = CONST_BITS - PASS1_BITS;
        workspace[3 * 0 + ctr] = rshift(tmp10.wrapping_add(tmp0), sh);
        workspace[3 * 2 + ctr] = rshift(tmp10.wrapping_sub(tmp0), sh);
        workspace[3 * 1 + ctr] = rshift(tmp2, sh);
    }
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..3 {
        let w = |c: usize| workspace[3 * ctr + c];
        let mut tmp0 = w(0).wrapping_add(1i32 << (PASS1_BITS + 2));
        tmp0 = lshift(tmp0, CONST_BITS);
        let tmp2 = w(2);
        let tmp12 = mul(tmp2, fix(0.707_106_781));
        let tmp10 = tmp0.wrapping_add(tmp12);
        let tmp2 = tmp0.wrapping_sub(tmp12).wrapping_sub(tmp12);
        let tmp12 = w(1);
        let tmp0 = mul(tmp12, fix(1.224_744_871));
        let sh = CONST_BITS + PASS1_BITS + 3;
        out[ctr][0] = rl.idct(rshift(tmp10.wrapping_add(tmp0), sh));
        out[ctr][2] = rl.idct(rshift(tmp10.wrapping_sub(tmp0), sh));
        out[ctr][1] = rl.idct(rshift(tmp2, sh));
    }
    out
}

/// `_jpeg_idct_4x4` (jidctred.c): 4x4 output from the top-left 4x4 coefficients.
pub(crate) fn idct_4x4(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
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
        let tmp2 = mul(z2, fix(1.847_759_065)).wrapping_add(mul(z3, -fix(0.765_366_865)));
        let tmp10 = tmp0.wrapping_add(tmp2);
        let tmp12 = tmp0.wrapping_sub(tmp2);
        // Odd part.
        let z1 = dequantize(c(7), q(7));
        let z2 = dequantize(c(5), q(5));
        let z3 = dequantize(c(3), q(3));
        let z4 = dequantize(c(1), q(1));
        let tmp0 = mul(z1, -fix(0.211_164_243))
            .wrapping_add(mul(z2, fix(1.451_774_981)))
            .wrapping_add(mul(z3, -fix(2.172_734_803)))
            .wrapping_add(mul(z4, fix(1.061_594_337)));
        let tmp2 = mul(z1, -fix(0.509_795_579))
            .wrapping_add(mul(z2, -fix(0.601_344_887)))
            .wrapping_add(mul(z3, fix(0.899_976_223)))
            .wrapping_add(mul(z4, fix(2.562_915_447)));
        let sh = CONST_BITS - PASS1_BITS + 1;
        workspace[ctr] = descale(tmp10.wrapping_add(tmp2), sh);
        workspace[ctr + 8 * 3] = descale(tmp10.wrapping_sub(tmp2), sh);
        workspace[ctr + 8] = descale(tmp12.wrapping_add(tmp0), sh);
        workspace[ctr + 8 * 2] = descale(tmp12.wrapping_sub(tmp0), sh);
    }
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..4 {
        let w = |c: usize| workspace[8 * ctr + c];
        if w(1) == 0 && w(2) == 0 && w(3) == 0 && w(5) == 0 && w(6) == 0 && w(7) == 0 {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr][..4].fill(dcval);
            continue;
        }
        let mut tmp0 = lshift(w(0), CONST_BITS + 1);
        let tmp2 = mul(w(2), fix(1.847_759_065)).wrapping_add(mul(w(6), -fix(0.765_366_865)));
        let tmp10 = tmp0.wrapping_add(tmp2);
        let tmp12 = tmp0.wrapping_sub(tmp2);
        let z1 = w(7);
        let z2 = w(5);
        let z3 = w(3);
        let z4 = w(1);
        tmp0 = mul(z1, -fix(0.211_164_243))
            .wrapping_add(mul(z2, fix(1.451_774_981)))
            .wrapping_add(mul(z3, -fix(2.172_734_803)))
            .wrapping_add(mul(z4, fix(1.061_594_337)));
        let tmp2 = mul(z1, -fix(0.509_795_579))
            .wrapping_add(mul(z2, -fix(0.601_344_887)))
            .wrapping_add(mul(z3, fix(0.899_976_223)))
            .wrapping_add(mul(z4, fix(2.562_915_447)));
        let sh = CONST_BITS + PASS1_BITS + 3 + 1;
        out[ctr][0] = rl.idct(descale(tmp10.wrapping_add(tmp2), sh));
        out[ctr][3] = rl.idct(descale(tmp10.wrapping_sub(tmp2), sh));
        out[ctr][1] = rl.idct(descale(tmp12.wrapping_add(tmp0), sh));
        out[ctr][2] = rl.idct(descale(tmp12.wrapping_sub(tmp0), sh));
    }
    out
}

/// `_jpeg_idct_2x2` (jidctred.c).
pub(crate) fn idct_2x2(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
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
        let mut tmp0 = mul(z1, -fix(0.720_959_822));
        let z1 = dequantize(c(5), q(5));
        tmp0 = tmp0.wrapping_add(mul(z1, fix(0.850_430_095)));
        let z1 = dequantize(c(3), q(3));
        tmp0 = tmp0.wrapping_add(mul(z1, -fix(1.272_758_580)));
        let z1 = dequantize(c(1), q(1));
        tmp0 = tmp0.wrapping_add(mul(z1, fix(3.624_509_785)));
        let sh = CONST_BITS - PASS1_BITS + 2;
        workspace[ctr] = descale(tmp10.wrapping_add(tmp0), sh);
        workspace[ctr + 8] = descale(tmp10.wrapping_sub(tmp0), sh);
    }
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..2 {
        let w = |c: usize| workspace[8 * ctr + c];
        if w(1) == 0 && w(3) == 0 && w(5) == 0 && w(7) == 0 {
            let dcval = rl.idct(descale(w(0), PASS1_BITS + 3));
            out[ctr][0] = dcval;
            out[ctr][1] = dcval;
            continue;
        }
        let tmp10 = lshift(w(0), CONST_BITS + 2);
        let tmp0 = mul(w(7), -fix(0.720_959_822))
            .wrapping_add(mul(w(5), fix(0.850_430_095)))
            .wrapping_add(mul(w(3), -fix(1.272_758_580)))
            .wrapping_add(mul(w(1), fix(3.624_509_785)));
        let sh = CONST_BITS + PASS1_BITS + 3 + 2;
        out[ctr][0] = rl.idct(descale(tmp10.wrapping_add(tmp0), sh));
        out[ctr][1] = rl.idct(descale(tmp10.wrapping_sub(tmp0), sh));
    }
    out
}

/// `_jpeg_idct_1x1` (jidctred.c).
pub(crate) fn idct_1x1(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let dcval = dequantize(coef[0], quant[0]);
    let dcval = descale(dcval, 3);
    let mut out = [[0u8; 16]; 16];
    out[0][0] = rl.idct(dcval);
    out
}

/// `_jpeg_idct_9x9`: 9x9 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L902-L1061 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_9x9(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 72];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut tmp0 = dequantize(coef[ctr], quant[ctr]);
        tmp0 = lshift(tmp0, CONST_BITS);
        // Add fudge factor here for final descale.
        tmp0 = tmp0.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z1 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        let mut z2 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z3 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        let mut tmp3 = mul(z3, fix(0.707_106_781));
        let mut tmp1 = tmp0.wrapping_add(tmp3);
        let mut tmp2 = tmp0.wrapping_sub(tmp3).wrapping_sub(tmp3);
        tmp0 = mul(z1.wrapping_sub(z2), fix(0.707_106_781));
        let tmp11 = tmp2.wrapping_add(tmp0);
        let tmp14 = tmp2.wrapping_sub(tmp0).wrapping_sub(tmp0);
        tmp0 = mul(z1.wrapping_add(z2), fix(1.328_926_049));
        tmp2 = mul(z1, fix(1.083_350_441));
        tmp3 = mul(z2, fix(0.245_575_608));
        let tmp10 = tmp1.wrapping_add(tmp0).wrapping_sub(tmp3);
        let tmp12 = tmp1.wrapping_sub(tmp0).wrapping_add(tmp2);
        let tmp13 = tmp1.wrapping_sub(tmp2).wrapping_add(tmp3);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        let z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        z2 = mul(z2, -fix(1.224_744_871));
        tmp2 = mul(z1.wrapping_add(z3), fix(0.909_038_955));
        tmp3 = mul(z1.wrapping_add(z4), fix(0.483_689_525));
        tmp0 = tmp2.wrapping_add(tmp3).wrapping_sub(z2);
        tmp1 = mul(z3.wrapping_sub(z4), fix(1.392_728_481));
        tmp2 = tmp2.wrapping_add(z2.wrapping_sub(tmp1));
        tmp3 = tmp3.wrapping_add(z2.wrapping_add(tmp1));
        tmp1 = mul(z1.wrapping_sub(z3).wrapping_sub(z4), fix(1.224_744_871));
        // Final output stage
        workspace[ctr] = rshift(tmp10.wrapping_add(tmp0), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp10.wrapping_sub(tmp0), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp11.wrapping_add(tmp1), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp11.wrapping_sub(tmp1), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp12.wrapping_add(tmp2), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp12.wrapping_sub(tmp2), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp13.wrapping_add(tmp3), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp13.wrapping_sub(tmp3), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp14, CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 9 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..9usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut tmp0 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        tmp0 = lshift(tmp0, CONST_BITS);
        let mut z1 = workspace[8 * ctr + 2];
        let mut z2 = workspace[8 * ctr + 4];
        let mut z3 = workspace[8 * ctr + 6];
        let mut tmp3 = mul(z3, fix(0.707_106_781));
        let mut tmp1 = tmp0.wrapping_add(tmp3);
        let mut tmp2 = tmp0.wrapping_sub(tmp3).wrapping_sub(tmp3);
        tmp0 = mul(z1.wrapping_sub(z2), fix(0.707_106_781));
        let tmp11 = tmp2.wrapping_add(tmp0);
        let tmp14 = tmp2.wrapping_sub(tmp0).wrapping_sub(tmp0);
        tmp0 = mul(z1.wrapping_add(z2), fix(1.328_926_049));
        tmp2 = mul(z1, fix(1.083_350_441));
        tmp3 = mul(z2, fix(0.245_575_608));
        let tmp10 = tmp1.wrapping_add(tmp0).wrapping_sub(tmp3);
        let tmp12 = tmp1.wrapping_sub(tmp0).wrapping_add(tmp2);
        let tmp13 = tmp1.wrapping_sub(tmp2).wrapping_add(tmp3);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        let z4 = workspace[8 * ctr + 7];
        z2 = mul(z2, -fix(1.224_744_871));
        tmp2 = mul(z1.wrapping_add(z3), fix(0.909_038_955));
        tmp3 = mul(z1.wrapping_add(z4), fix(0.483_689_525));
        tmp0 = tmp2.wrapping_add(tmp3).wrapping_sub(z2);
        tmp1 = mul(z3.wrapping_sub(z4), fix(1.392_728_481));
        tmp2 = tmp2.wrapping_add(z2.wrapping_sub(tmp1));
        tmp3 = tmp3.wrapping_add(z2.wrapping_add(tmp1));
        tmp1 = mul(z1.wrapping_sub(z3).wrapping_sub(z4), fix(1.224_744_871));
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp10.wrapping_add(tmp0),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp10.wrapping_sub(tmp0),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp11.wrapping_add(tmp1),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp11.wrapping_sub(tmp1),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp12.wrapping_add(tmp2),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp12.wrapping_sub(tmp2),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp13.wrapping_add(tmp3),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp13.wrapping_sub(tmp3),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(tmp14, CONST_BITS + PASS1_BITS + 3));
    }
    out
}

/// `_jpeg_idct_10x10`: 10x10 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L1073-L1256 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_10x10(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 80];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut z3 = dequantize(coef[ctr], quant[ctr]);
        z3 = lshift(z3, CONST_BITS);
        // Add fudge factor here for final descale.
        z3 = z3.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z4 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z1 = mul(z4, fix(1.144_122_806));
        let mut z2 = mul(z4, fix(0.437_016_024));
        let mut tmp10 = z3.wrapping_add(z1);
        let mut tmp11 = z3.wrapping_sub(z2);
        let tmp22 = rshift(
            z3.wrapping_sub(lshift(z1.wrapping_sub(z2), 1)),
            CONST_BITS - PASS1_BITS,
        );
        z2 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        z3 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        z1 = mul(z2.wrapping_add(z3), fix(0.831_253_876));
        let mut tmp12 = z1.wrapping_add(mul(z2, fix(0.513_743_148)));
        let mut tmp13 = z1.wrapping_sub(mul(z3, fix(2.176_250_899)));
        let tmp20 = tmp10.wrapping_add(tmp12);
        let tmp24 = tmp10.wrapping_sub(tmp12);
        let tmp21 = tmp11.wrapping_add(tmp13);
        let tmp23 = tmp11.wrapping_sub(tmp13);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp11 = z2.wrapping_add(z4);
        tmp13 = z2.wrapping_sub(z4);
        tmp12 = mul(tmp13, fix(0.309_016_994));
        let z5 = lshift(z3, CONST_BITS);
        z2 = mul(tmp11, fix(0.951_056_516));
        z4 = z5.wrapping_add(tmp12);
        tmp10 = mul(z1, fix(1.396_802_247))
            .wrapping_add(z2)
            .wrapping_add(z4);
        let tmp14 = mul(z1, fix(0.221_231_742))
            .wrapping_sub(z2)
            .wrapping_add(z4);
        z2 = mul(tmp11, fix(0.587_785_252));
        z4 = z5
            .wrapping_sub(tmp12)
            .wrapping_sub(lshift(tmp13, CONST_BITS - 1));
        tmp12 = lshift(z1.wrapping_sub(tmp13).wrapping_sub(z3), PASS1_BITS);
        tmp11 = mul(z1, fix(1.260_073_511))
            .wrapping_sub(z2)
            .wrapping_sub(z4);
        tmp13 = mul(z1, fix(0.642_039_522))
            .wrapping_sub(z2)
            .wrapping_add(z4);
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = tmp22.wrapping_add(tmp12);
        workspace[ctr + 56] = tmp22.wrapping_sub(tmp12);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp23.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 10 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..10usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut z3 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        z3 = lshift(z3, CONST_BITS);
        let mut z4 = workspace[8 * ctr + 4];
        let mut z1 = mul(z4, fix(1.144_122_806));
        let mut z2 = mul(z4, fix(0.437_016_024));
        let mut tmp10 = z3.wrapping_add(z1);
        let mut tmp11 = z3.wrapping_sub(z2);
        let tmp22 = z3.wrapping_sub(lshift(z1.wrapping_sub(z2), 1));
        z2 = workspace[8 * ctr + 2];
        z3 = workspace[8 * ctr + 6];
        z1 = mul(z2.wrapping_add(z3), fix(0.831_253_876));
        let mut tmp12 = z1.wrapping_add(mul(z2, fix(0.513_743_148)));
        let mut tmp13 = z1.wrapping_sub(mul(z3, fix(2.176_250_899)));
        let tmp20 = tmp10.wrapping_add(tmp12);
        let tmp24 = tmp10.wrapping_sub(tmp12);
        let tmp21 = tmp11.wrapping_add(tmp13);
        let tmp23 = tmp11.wrapping_sub(tmp13);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z3 = lshift(z3, CONST_BITS);
        z4 = workspace[8 * ctr + 7];
        tmp11 = z2.wrapping_add(z4);
        tmp13 = z2.wrapping_sub(z4);
        tmp12 = mul(tmp13, fix(0.309_016_994));
        z2 = mul(tmp11, fix(0.951_056_516));
        z4 = z3.wrapping_add(tmp12);
        tmp10 = mul(z1, fix(1.396_802_247))
            .wrapping_add(z2)
            .wrapping_add(z4);
        let tmp14 = mul(z1, fix(0.221_231_742))
            .wrapping_sub(z2)
            .wrapping_add(z4);
        z2 = mul(tmp11, fix(0.587_785_252));
        z4 = z3
            .wrapping_sub(tmp12)
            .wrapping_sub(lshift(tmp13, CONST_BITS - 1));
        tmp12 = lshift(z1.wrapping_sub(tmp13), CONST_BITS).wrapping_sub(z3);
        tmp11 = mul(z1, fix(1.260_073_511))
            .wrapping_sub(z2)
            .wrapping_sub(z4);
        tmp13 = mul(z1, fix(0.642_039_522))
            .wrapping_sub(z2)
            .wrapping_add(z4);
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
    }
    out
}

/// `_jpeg_idct_11x11`: 11x11 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L1268-L1450 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_11x11(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 88];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut tmp10 = dequantize(coef[ctr], quant[ctr]);
        tmp10 = lshift(tmp10, CONST_BITS);
        // Add fudge factor here for final descale.
        tmp10 = tmp10.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z1 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        let mut z2 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z3 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        let mut tmp20 = mul(z2.wrapping_sub(z3), fix(2.546_640_132));
        let mut tmp23 = mul(z2.wrapping_sub(z1), fix(0.430_815_045));
        let mut z4 = z1.wrapping_add(z3);
        let mut tmp24 = mul(z4, -fix(1.155_664_402));
        z4 = z4.wrapping_sub(z2);
        let mut tmp25 = tmp10.wrapping_add(mul(z4, fix(1.356_927_976)));
        let tmp21 = tmp20
            .wrapping_add(tmp23)
            .wrapping_add(tmp25)
            .wrapping_sub(mul(z2, fix(1.821_790_775)));
        tmp20 = tmp20.wrapping_add(tmp25.wrapping_add(mul(z3, fix(2.115_825_087))));
        tmp23 = tmp23.wrapping_add(tmp25.wrapping_sub(mul(z1, fix(1.513_598_477))));
        tmp24 = tmp24.wrapping_add(tmp25);
        let tmp22 = tmp24.wrapping_sub(mul(z3, fix(0.788_749_120)));
        tmp24 = tmp24
            .wrapping_add(mul(z2, fix(1.944_413_522)).wrapping_sub(mul(z1, fix(1.390_975_730))));
        tmp25 = tmp10.wrapping_sub(mul(z4, fix(1.414_213_562)));
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        let mut tmp11 = z1.wrapping_add(z2);
        let mut tmp14 = mul(tmp11.wrapping_add(z3).wrapping_add(z4), fix(0.398_430_003));
        tmp11 = mul(tmp11, fix(0.887_983_902));
        let mut tmp12 = mul(z1.wrapping_add(z3), fix(0.670_361_295));
        let mut tmp13 = tmp14.wrapping_add(mul(z1.wrapping_add(z4), fix(0.366_151_574)));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z1, fix(0.923_107_866)));
        z1 = tmp14.wrapping_sub(mul(z2.wrapping_add(z3), fix(1.163_011_579)));
        tmp11 = tmp11.wrapping_add(z1.wrapping_add(mul(z2, fix(2.073_276_588))));
        tmp12 = tmp12.wrapping_add(z1.wrapping_sub(mul(z3, fix(1.192_193_623))));
        z1 = mul(z2.wrapping_add(z4), -fix(1.798_248_910));
        tmp11 = tmp11.wrapping_add(z1);
        tmp13 = tmp13.wrapping_add(z1.wrapping_add(mul(z4, fix(2.102_458_632))));
        tmp14 = tmp14.wrapping_add(
            mul(z2, -fix(1.467_221_301))
                .wrapping_add(mul(z3, fix(1.001_388_905)))
                .wrapping_sub(mul(z4, fix(1.684_843_907))),
        );
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 80] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp22.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp23.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25, CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 11 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..11usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut tmp10 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        tmp10 = lshift(tmp10, CONST_BITS);
        let mut z1 = workspace[8 * ctr + 2];
        let mut z2 = workspace[8 * ctr + 4];
        let mut z3 = workspace[8 * ctr + 6];
        let mut tmp20 = mul(z2.wrapping_sub(z3), fix(2.546_640_132));
        let mut tmp23 = mul(z2.wrapping_sub(z1), fix(0.430_815_045));
        let mut z4 = z1.wrapping_add(z3);
        let mut tmp24 = mul(z4, -fix(1.155_664_402));
        z4 = z4.wrapping_sub(z2);
        let mut tmp25 = tmp10.wrapping_add(mul(z4, fix(1.356_927_976)));
        let tmp21 = tmp20
            .wrapping_add(tmp23)
            .wrapping_add(tmp25)
            .wrapping_sub(mul(z2, fix(1.821_790_775)));
        tmp20 = tmp20.wrapping_add(tmp25.wrapping_add(mul(z3, fix(2.115_825_087))));
        tmp23 = tmp23.wrapping_add(tmp25.wrapping_sub(mul(z1, fix(1.513_598_477))));
        tmp24 = tmp24.wrapping_add(tmp25);
        let tmp22 = tmp24.wrapping_sub(mul(z3, fix(0.788_749_120)));
        tmp24 = tmp24
            .wrapping_add(mul(z2, fix(1.944_413_522)).wrapping_sub(mul(z1, fix(1.390_975_730))));
        tmp25 = tmp10.wrapping_sub(mul(z4, fix(1.414_213_562)));
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z4 = workspace[8 * ctr + 7];
        let mut tmp11 = z1.wrapping_add(z2);
        let mut tmp14 = mul(tmp11.wrapping_add(z3).wrapping_add(z4), fix(0.398_430_003));
        tmp11 = mul(tmp11, fix(0.887_983_902));
        let mut tmp12 = mul(z1.wrapping_add(z3), fix(0.670_361_295));
        let mut tmp13 = tmp14.wrapping_add(mul(z1.wrapping_add(z4), fix(0.366_151_574)));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z1, fix(0.923_107_866)));
        z1 = tmp14.wrapping_sub(mul(z2.wrapping_add(z3), fix(1.163_011_579)));
        tmp11 = tmp11.wrapping_add(z1.wrapping_add(mul(z2, fix(2.073_276_588))));
        tmp12 = tmp12.wrapping_add(z1.wrapping_sub(mul(z3, fix(1.192_193_623))));
        z1 = mul(z2.wrapping_add(z4), -fix(1.798_248_910));
        tmp11 = tmp11.wrapping_add(z1);
        tmp13 = tmp13.wrapping_add(z1.wrapping_add(mul(z4, fix(2.102_458_632))));
        tmp14 = tmp14.wrapping_add(
            mul(z2, -fix(1.467_221_301))
                .wrapping_add(mul(z3, fix(1.001_388_905)))
                .wrapping_sub(mul(z4, fix(1.684_843_907))),
        );
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(tmp25, CONST_BITS + PASS1_BITS + 3));
    }
    out
}

/// `_jpeg_idct_12x12`: 12x12 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L1462-L1666 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_12x12(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 96];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut z3 = dequantize(coef[ctr], quant[ctr]);
        z3 = lshift(z3, CONST_BITS);
        // Add fudge factor here for final descale.
        z3 = z3.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z4 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        z4 = mul(z4, fix(1.224_744_871));
        let mut tmp10 = z3.wrapping_add(z4);
        let mut tmp11 = z3.wrapping_sub(z4);
        let mut z1 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        z4 = mul(z1, fix(1.366_025_404));
        z1 = lshift(z1, CONST_BITS);
        let mut z2 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        z2 = lshift(z2, CONST_BITS);
        let mut tmp12 = z1.wrapping_sub(z2);
        let tmp21 = z3.wrapping_add(tmp12);
        let tmp24 = z3.wrapping_sub(tmp12);
        tmp12 = z4.wrapping_add(z2);
        let tmp20 = tmp10.wrapping_add(tmp12);
        let tmp25 = tmp10.wrapping_sub(tmp12);
        tmp12 = z4.wrapping_sub(z1).wrapping_sub(z2);
        let tmp22 = tmp11.wrapping_add(tmp12);
        let tmp23 = tmp11.wrapping_sub(tmp12);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp11 = mul(z2, fix(1.306_562_965));
        let mut tmp14 = mul(z2, -fix(0.541_196_100));
        tmp10 = z1.wrapping_add(z3);
        let mut tmp15 = mul(tmp10.wrapping_add(z4), fix(0.860_918_669));
        tmp12 = tmp15.wrapping_add(mul(tmp10, fix(0.261_052_384)));
        tmp10 = tmp12
            .wrapping_add(tmp11)
            .wrapping_add(mul(z1, fix(0.280_143_716)));
        let mut tmp13 = mul(z3.wrapping_add(z4), -fix(1.045_510_580));
        tmp12 = tmp12.wrapping_add(
            tmp13
                .wrapping_add(tmp14)
                .wrapping_sub(mul(z3, fix(1.478_575_242))),
        );
        tmp13 = tmp13.wrapping_add(
            tmp15
                .wrapping_sub(tmp11)
                .wrapping_add(mul(z4, fix(1.586_706_681))),
        );
        tmp15 = tmp15.wrapping_add(
            tmp14
                .wrapping_sub(mul(z1, fix(0.676_326_758)))
                .wrapping_sub(mul(z4, fix(1.982_889_723))),
        );
        z1 = z1.wrapping_sub(z4);
        z2 = z2.wrapping_sub(z3);
        z3 = mul(z1.wrapping_add(z2), fix(0.541_196_100));
        tmp11 = z3.wrapping_add(mul(z1, fix(0.765_366_865)));
        tmp14 = z3.wrapping_sub(mul(z2, fix(1.847_759_065)));
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 88] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 80] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp22.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp23.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25.wrapping_add(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp25.wrapping_sub(tmp15), CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 12 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..12usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut z3 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        z3 = lshift(z3, CONST_BITS);
        let mut z4 = workspace[8 * ctr + 4];
        z4 = mul(z4, fix(1.224_744_871));
        let mut tmp10 = z3.wrapping_add(z4);
        let mut tmp11 = z3.wrapping_sub(z4);
        let mut z1 = workspace[8 * ctr + 2];
        z4 = mul(z1, fix(1.366_025_404));
        z1 = lshift(z1, CONST_BITS);
        let mut z2 = workspace[8 * ctr + 6];
        z2 = lshift(z2, CONST_BITS);
        let mut tmp12 = z1.wrapping_sub(z2);
        let tmp21 = z3.wrapping_add(tmp12);
        let tmp24 = z3.wrapping_sub(tmp12);
        tmp12 = z4.wrapping_add(z2);
        let tmp20 = tmp10.wrapping_add(tmp12);
        let tmp25 = tmp10.wrapping_sub(tmp12);
        tmp12 = z4.wrapping_sub(z1).wrapping_sub(z2);
        let tmp22 = tmp11.wrapping_add(tmp12);
        let tmp23 = tmp11.wrapping_sub(tmp12);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z4 = workspace[8 * ctr + 7];
        tmp11 = mul(z2, fix(1.306_562_965));
        let mut tmp14 = mul(z2, -fix(0.541_196_100));
        tmp10 = z1.wrapping_add(z3);
        let mut tmp15 = mul(tmp10.wrapping_add(z4), fix(0.860_918_669));
        tmp12 = tmp15.wrapping_add(mul(tmp10, fix(0.261_052_384)));
        tmp10 = tmp12
            .wrapping_add(tmp11)
            .wrapping_add(mul(z1, fix(0.280_143_716)));
        let mut tmp13 = mul(z3.wrapping_add(z4), -fix(1.045_510_580));
        tmp12 = tmp12.wrapping_add(
            tmp13
                .wrapping_add(tmp14)
                .wrapping_sub(mul(z3, fix(1.478_575_242))),
        );
        tmp13 = tmp13.wrapping_add(
            tmp15
                .wrapping_sub(tmp11)
                .wrapping_add(mul(z4, fix(1.586_706_681))),
        );
        tmp15 = tmp15.wrapping_add(
            tmp14
                .wrapping_sub(mul(z1, fix(0.676_326_758)))
                .wrapping_sub(mul(z4, fix(1.982_889_723))),
        );
        z1 = z1.wrapping_sub(z4);
        z2 = z2.wrapping_sub(z3);
        z3 = mul(z1.wrapping_add(z2), fix(0.541_196_100));
        tmp11 = z3.wrapping_add(mul(z1, fix(0.765_366_865)));
        tmp14 = z3.wrapping_sub(mul(z2, fix(1.847_759_065)));
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][11] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp25.wrapping_add(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp25.wrapping_sub(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
    }
    out
}

/// `_jpeg_idct_13x13`: 13x13 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L1678-L1894 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_13x13(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 104];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut z1 = dequantize(coef[ctr], quant[ctr]);
        z1 = lshift(z1, CONST_BITS);
        // Add fudge factor here for final descale.
        z1 = z1.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z2 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        let mut z3 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z4 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        let mut tmp10 = z3.wrapping_add(z4);
        let mut tmp11 = z3.wrapping_sub(z4);
        let mut tmp12 = mul(tmp10, fix(1.155_388_986));
        let mut tmp13 = mul(tmp11, fix(0.096_834_934)).wrapping_add(z1);
        let tmp20 = mul(z2, fix(1.373_119_086))
            .wrapping_add(tmp12)
            .wrapping_add(tmp13);
        let tmp22 = mul(z2, fix(0.501_487_041))
            .wrapping_sub(tmp12)
            .wrapping_add(tmp13);
        tmp12 = mul(tmp10, fix(0.316_450_131));
        tmp13 = mul(tmp11, fix(0.486_914_739)).wrapping_add(z1);
        let tmp21 = mul(z2, fix(1.058_554_052))
            .wrapping_sub(tmp12)
            .wrapping_add(tmp13);
        let tmp25 = mul(z2, -fix(1.252_223_920))
            .wrapping_add(tmp12)
            .wrapping_add(tmp13);
        tmp12 = mul(tmp10, fix(0.435_816_023));
        tmp13 = mul(tmp11, fix(0.937_303_064)).wrapping_sub(z1);
        let tmp23 = mul(z2, -fix(0.170_464_608))
            .wrapping_sub(tmp12)
            .wrapping_sub(tmp13);
        let tmp24 = mul(z2, -fix(0.803_364_869))
            .wrapping_add(tmp12)
            .wrapping_sub(tmp13);
        let tmp26 = mul(tmp11.wrapping_sub(z2), fix(1.414_213_562)).wrapping_add(z1);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp11 = mul(z1.wrapping_add(z2), fix(1.322_312_651));
        tmp12 = mul(z1.wrapping_add(z3), fix(1.163_874_945));
        let mut tmp15 = z1.wrapping_add(z4);
        tmp13 = mul(tmp15, fix(0.937_797_057));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z1, fix(2.020_082_300)));
        let mut tmp14 = mul(z2.wrapping_add(z3), -fix(0.338_443_458));
        tmp11 = tmp11.wrapping_add(tmp14.wrapping_add(mul(z2, fix(0.837_223_564))));
        tmp12 = tmp12.wrapping_add(tmp14.wrapping_sub(mul(z3, fix(1.572_116_027))));
        tmp14 = mul(z2.wrapping_add(z4), -fix(1.163_874_945));
        tmp11 = tmp11.wrapping_add(tmp14);
        tmp13 = tmp13.wrapping_add(tmp14.wrapping_add(mul(z4, fix(2.205_608_352))));
        tmp14 = mul(z3.wrapping_add(z4), -fix(0.657_217_813));
        tmp12 = tmp12.wrapping_add(tmp14);
        tmp13 = tmp13.wrapping_add(tmp14);
        tmp15 = mul(tmp15, fix(0.338_443_458));
        tmp14 = tmp15
            .wrapping_add(mul(z1, fix(0.318_774_355)))
            .wrapping_sub(mul(z2, fix(0.466_105_296)));
        z1 = mul(z3.wrapping_sub(z2), fix(0.937_797_057));
        tmp14 = tmp14.wrapping_add(z1);
        tmp15 = tmp15.wrapping_add(
            z1.wrapping_add(mul(z3, fix(0.384_515_595)))
                .wrapping_sub(mul(z4, fix(1.742_345_811))),
        );
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 96] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 88] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 80] = rshift(tmp22.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp23.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25.wrapping_add(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp25.wrapping_sub(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp26, CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 13 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..13usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut z1 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        z1 = lshift(z1, CONST_BITS);
        let mut z2 = workspace[8 * ctr + 2];
        let mut z3 = workspace[8 * ctr + 4];
        let mut z4 = workspace[8 * ctr + 6];
        let mut tmp10 = z3.wrapping_add(z4);
        let mut tmp11 = z3.wrapping_sub(z4);
        let mut tmp12 = mul(tmp10, fix(1.155_388_986));
        let mut tmp13 = mul(tmp11, fix(0.096_834_934)).wrapping_add(z1);
        let tmp20 = mul(z2, fix(1.373_119_086))
            .wrapping_add(tmp12)
            .wrapping_add(tmp13);
        let tmp22 = mul(z2, fix(0.501_487_041))
            .wrapping_sub(tmp12)
            .wrapping_add(tmp13);
        tmp12 = mul(tmp10, fix(0.316_450_131));
        tmp13 = mul(tmp11, fix(0.486_914_739)).wrapping_add(z1);
        let tmp21 = mul(z2, fix(1.058_554_052))
            .wrapping_sub(tmp12)
            .wrapping_add(tmp13);
        let tmp25 = mul(z2, -fix(1.252_223_920))
            .wrapping_add(tmp12)
            .wrapping_add(tmp13);
        tmp12 = mul(tmp10, fix(0.435_816_023));
        tmp13 = mul(tmp11, fix(0.937_303_064)).wrapping_sub(z1);
        let tmp23 = mul(z2, -fix(0.170_464_608))
            .wrapping_sub(tmp12)
            .wrapping_sub(tmp13);
        let tmp24 = mul(z2, -fix(0.803_364_869))
            .wrapping_add(tmp12)
            .wrapping_sub(tmp13);
        let tmp26 = mul(tmp11.wrapping_sub(z2), fix(1.414_213_562)).wrapping_add(z1);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z4 = workspace[8 * ctr + 7];
        tmp11 = mul(z1.wrapping_add(z2), fix(1.322_312_651));
        tmp12 = mul(z1.wrapping_add(z3), fix(1.163_874_945));
        let mut tmp15 = z1.wrapping_add(z4);
        tmp13 = mul(tmp15, fix(0.937_797_057));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z1, fix(2.020_082_300)));
        let mut tmp14 = mul(z2.wrapping_add(z3), -fix(0.338_443_458));
        tmp11 = tmp11.wrapping_add(tmp14.wrapping_add(mul(z2, fix(0.837_223_564))));
        tmp12 = tmp12.wrapping_add(tmp14.wrapping_sub(mul(z3, fix(1.572_116_027))));
        tmp14 = mul(z2.wrapping_add(z4), -fix(1.163_874_945));
        tmp11 = tmp11.wrapping_add(tmp14);
        tmp13 = tmp13.wrapping_add(tmp14.wrapping_add(mul(z4, fix(2.205_608_352))));
        tmp14 = mul(z3.wrapping_add(z4), -fix(0.657_217_813));
        tmp12 = tmp12.wrapping_add(tmp14);
        tmp13 = tmp13.wrapping_add(tmp14);
        tmp15 = mul(tmp15, fix(0.338_443_458));
        tmp14 = tmp15
            .wrapping_add(mul(z1, fix(0.318_774_355)))
            .wrapping_sub(mul(z2, fix(0.466_105_296)));
        z1 = mul(z3.wrapping_sub(z2), fix(0.937_797_057));
        tmp14 = tmp14.wrapping_add(z1);
        tmp15 = tmp15.wrapping_add(
            z1.wrapping_add(mul(z3, fix(0.384_515_595)))
                .wrapping_sub(mul(z4, fix(1.742_345_811))),
        );
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][12] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][11] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp25.wrapping_add(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp25.wrapping_sub(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(tmp26, CONST_BITS + PASS1_BITS + 3));
    }
    out
}

/// `_jpeg_idct_14x14`: 14x14 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L1906-L2120 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_14x14(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 112];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut z1 = dequantize(coef[ctr], quant[ctr]);
        z1 = lshift(z1, CONST_BITS);
        // Add fudge factor here for final descale.
        z1 = z1.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z4 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z2 = mul(z4, fix(1.274_162_392));
        let mut z3 = mul(z4, fix(0.314_692_123));
        z4 = mul(z4, fix(0.881_747_734));
        let mut tmp10 = z1.wrapping_add(z2);
        let mut tmp11 = z1.wrapping_add(z3);
        let mut tmp12 = z1.wrapping_sub(z4);
        let tmp23 = rshift(
            z1.wrapping_sub(lshift(z2.wrapping_add(z3).wrapping_sub(z4), 1)),
            CONST_BITS - PASS1_BITS,
        );
        z1 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        z2 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        z3 = mul(z1.wrapping_add(z2), fix(1.105_676_686));
        let mut tmp13 = z3.wrapping_add(mul(z1, fix(0.273_079_590)));
        let mut tmp14 = z3.wrapping_sub(mul(z2, fix(1.719_280_954)));
        let mut tmp15 = mul(z1, fix(0.613_604_268)).wrapping_sub(mul(z2, fix(1.378_756_276)));
        let tmp20 = tmp10.wrapping_add(tmp13);
        let tmp26 = tmp10.wrapping_sub(tmp13);
        let tmp21 = tmp11.wrapping_add(tmp14);
        let tmp25 = tmp11.wrapping_sub(tmp14);
        let tmp22 = tmp12.wrapping_add(tmp15);
        let tmp24 = tmp12.wrapping_sub(tmp15);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp13 = lshift(z4, CONST_BITS);
        tmp14 = z1.wrapping_add(z3);
        tmp11 = mul(z1.wrapping_add(z2), fix(1.334_852_607));
        tmp12 = mul(tmp14, fix(1.197_448_846));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(tmp13)
            .wrapping_sub(mul(z1, fix(1.126_980_169)));
        tmp14 = mul(tmp14, fix(0.752_406_978));
        let mut tmp16 = tmp14.wrapping_sub(mul(z1, fix(1.061_150_426)));
        z1 = z1.wrapping_sub(z2);
        tmp15 = mul(z1, fix(0.467_085_129)).wrapping_sub(tmp13);
        tmp16 = tmp16.wrapping_add(tmp15);
        z1 = z1.wrapping_add(z4);
        z4 = mul(z2.wrapping_add(z3), -fix(0.158_341_681)).wrapping_sub(tmp13);
        tmp11 = tmp11.wrapping_add(z4.wrapping_sub(mul(z2, fix(0.424_103_948))));
        tmp12 = tmp12.wrapping_add(z4.wrapping_sub(mul(z3, fix(2.373_959_773))));
        z4 = mul(z3.wrapping_sub(z2), fix(1.405_321_284));
        tmp14 = tmp14.wrapping_add(
            z4.wrapping_add(tmp13)
                .wrapping_sub(mul(z3, fix(1.690_643_133_4))),
        );
        tmp15 = tmp15.wrapping_add(z4.wrapping_add(mul(z2, fix(0.674_957_567))));
        tmp13 = lshift(z1.wrapping_sub(z3), PASS1_BITS);
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 104] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 96] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 88] = rshift(tmp22.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = tmp23.wrapping_add(tmp13);
        workspace[ctr + 80] = tmp23.wrapping_sub(tmp13);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25.wrapping_add(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp25.wrapping_sub(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp26.wrapping_add(tmp16), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp26.wrapping_sub(tmp16), CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 14 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..14usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut z1 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        z1 = lshift(z1, CONST_BITS);
        let mut z4 = workspace[8 * ctr + 4];
        let mut z2 = mul(z4, fix(1.274_162_392));
        let mut z3 = mul(z4, fix(0.314_692_123));
        z4 = mul(z4, fix(0.881_747_734));
        let mut tmp10 = z1.wrapping_add(z2);
        let mut tmp11 = z1.wrapping_add(z3);
        let mut tmp12 = z1.wrapping_sub(z4);
        let tmp23 = z1.wrapping_sub(lshift(z2.wrapping_add(z3).wrapping_sub(z4), 1));
        z1 = workspace[8 * ctr + 2];
        z2 = workspace[8 * ctr + 6];
        z3 = mul(z1.wrapping_add(z2), fix(1.105_676_686));
        let mut tmp13 = z3.wrapping_add(mul(z1, fix(0.273_079_590)));
        let mut tmp14 = z3.wrapping_sub(mul(z2, fix(1.719_280_954)));
        let mut tmp15 = mul(z1, fix(0.613_604_268)).wrapping_sub(mul(z2, fix(1.378_756_276)));
        let tmp20 = tmp10.wrapping_add(tmp13);
        let tmp26 = tmp10.wrapping_sub(tmp13);
        let tmp21 = tmp11.wrapping_add(tmp14);
        let tmp25 = tmp11.wrapping_sub(tmp14);
        let tmp22 = tmp12.wrapping_add(tmp15);
        let tmp24 = tmp12.wrapping_sub(tmp15);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z4 = workspace[8 * ctr + 7];
        z4 = lshift(z4, CONST_BITS);
        tmp14 = z1.wrapping_add(z3);
        tmp11 = mul(z1.wrapping_add(z2), fix(1.334_852_607));
        tmp12 = mul(tmp14, fix(1.197_448_846));
        tmp10 = tmp11
            .wrapping_add(tmp12)
            .wrapping_add(z4)
            .wrapping_sub(mul(z1, fix(1.126_980_169)));
        tmp14 = mul(tmp14, fix(0.752_406_978));
        let mut tmp16 = tmp14.wrapping_sub(mul(z1, fix(1.061_150_426)));
        z1 = z1.wrapping_sub(z2);
        tmp15 = mul(z1, fix(0.467_085_129)).wrapping_sub(z4);
        tmp16 = tmp16.wrapping_add(tmp15);
        tmp13 = mul(z2.wrapping_add(z3), -fix(0.158_341_681)).wrapping_sub(z4);
        tmp11 = tmp11.wrapping_add(tmp13.wrapping_sub(mul(z2, fix(0.424_103_948))));
        tmp12 = tmp12.wrapping_add(tmp13.wrapping_sub(mul(z3, fix(2.373_959_773))));
        tmp13 = mul(z3.wrapping_sub(z2), fix(1.405_321_284));
        tmp14 = tmp14.wrapping_add(
            tmp13
                .wrapping_add(z4)
                .wrapping_sub(mul(z3, fix(1.690_643_133_4))),
        );
        tmp15 = tmp15.wrapping_add(tmp13.wrapping_add(mul(z2, fix(0.674_957_567))));
        tmp13 = lshift(z1.wrapping_sub(z3), CONST_BITS).wrapping_add(z4);
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][13] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][12] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][11] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp25.wrapping_add(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp25.wrapping_sub(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp26.wrapping_add(tmp16),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp26.wrapping_sub(tmp16),
            CONST_BITS + PASS1_BITS + 3,
        ));
    }
    out
}

/// `_jpeg_idct_15x15`: 15x15 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L2132-L2362 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_15x15(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 120];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut z1 = dequantize(coef[ctr], quant[ctr]);
        z1 = lshift(z1, CONST_BITS);
        // Add fudge factor here for final descale.
        z1 = z1.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z2 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        let mut z3 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut z4 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        let mut tmp10 = mul(z4, fix(0.437_016_024));
        let mut tmp11 = mul(z4, fix(1.144_122_806));
        let mut tmp12 = z1.wrapping_sub(tmp10);
        let mut tmp13 = z1.wrapping_add(tmp11);
        z1 = z1.wrapping_sub(lshift(tmp11.wrapping_sub(tmp10), 1));
        z4 = z2.wrapping_sub(z3);
        z3 = z3.wrapping_add(z2);
        tmp10 = mul(z3, fix(1.337_628_990));
        tmp11 = mul(z4, fix(0.045_680_613));
        z2 = mul(z2, fix(1.439_773_946));
        let tmp20 = tmp13.wrapping_add(tmp10).wrapping_add(tmp11);
        let tmp23 = tmp12
            .wrapping_sub(tmp10)
            .wrapping_add(tmp11)
            .wrapping_add(z2);
        tmp10 = mul(z3, fix(0.547_059_574));
        tmp11 = mul(z4, fix(0.399_234_004));
        let tmp25 = tmp13.wrapping_sub(tmp10).wrapping_sub(tmp11);
        let tmp26 = tmp12
            .wrapping_add(tmp10)
            .wrapping_sub(tmp11)
            .wrapping_sub(z2);
        tmp10 = mul(z3, fix(0.790_569_415));
        tmp11 = mul(z4, fix(0.353_553_391));
        let tmp21 = tmp12.wrapping_add(tmp10).wrapping_add(tmp11);
        let tmp24 = tmp13.wrapping_sub(tmp10).wrapping_add(tmp11);
        tmp11 = tmp11.wrapping_add(tmp11);
        let tmp22 = z1.wrapping_add(tmp11);
        let tmp27 = z1.wrapping_sub(tmp11).wrapping_sub(tmp11);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z4 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z3 = mul(z4, fix(1.224_744_871));
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp13 = z2.wrapping_sub(z4);
        let mut tmp15 = mul(z1.wrapping_add(tmp13), fix(0.831_253_876));
        tmp11 = tmp15.wrapping_add(mul(z1, fix(0.513_743_148)));
        let tmp14 = tmp15.wrapping_sub(mul(tmp13, fix(2.176_250_899)));
        tmp13 = mul(z2, -fix(0.831_253_876));
        tmp15 = mul(z2, -fix(1.344_997_024));
        z2 = z1.wrapping_sub(z4);
        tmp12 = z3.wrapping_add(mul(z2, fix(1.406_466_353)));
        tmp10 = tmp12
            .wrapping_add(mul(z4, fix(2.457_431_844)))
            .wrapping_sub(tmp15);
        let tmp16 = tmp12
            .wrapping_sub(mul(z1, fix(1.112_434_820)))
            .wrapping_add(tmp13);
        tmp12 = mul(z2, fix(1.224_744_871)).wrapping_sub(z3);
        z2 = mul(z1.wrapping_add(z4), fix(0.575_212_477));
        tmp13 = tmp13.wrapping_add(
            z2.wrapping_add(mul(z1, fix(0.475_753_014)))
                .wrapping_sub(z3),
        );
        tmp15 = tmp15.wrapping_add(
            z2.wrapping_sub(mul(z4, fix(0.869_244_010)))
                .wrapping_add(z3),
        );
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 112] = rshift(tmp20.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 104] = rshift(tmp21.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 96] = rshift(tmp22.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 88] = rshift(tmp23.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 80] = rshift(tmp24.wrapping_sub(tmp14), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25.wrapping_add(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp25.wrapping_sub(tmp15), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp26.wrapping_add(tmp16), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp26.wrapping_sub(tmp16), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp27, CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 15 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..15usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut z1 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        z1 = lshift(z1, CONST_BITS);
        let mut z2 = workspace[8 * ctr + 2];
        let mut z3 = workspace[8 * ctr + 4];
        let mut z4 = workspace[8 * ctr + 6];
        let mut tmp10 = mul(z4, fix(0.437_016_024));
        let mut tmp11 = mul(z4, fix(1.144_122_806));
        let mut tmp12 = z1.wrapping_sub(tmp10);
        let mut tmp13 = z1.wrapping_add(tmp11);
        z1 = z1.wrapping_sub(lshift(tmp11.wrapping_sub(tmp10), 1));
        z4 = z2.wrapping_sub(z3);
        z3 = z3.wrapping_add(z2);
        tmp10 = mul(z3, fix(1.337_628_990));
        tmp11 = mul(z4, fix(0.045_680_613));
        z2 = mul(z2, fix(1.439_773_946));
        let tmp20 = tmp13.wrapping_add(tmp10).wrapping_add(tmp11);
        let tmp23 = tmp12
            .wrapping_sub(tmp10)
            .wrapping_add(tmp11)
            .wrapping_add(z2);
        tmp10 = mul(z3, fix(0.547_059_574));
        tmp11 = mul(z4, fix(0.399_234_004));
        let tmp25 = tmp13.wrapping_sub(tmp10).wrapping_sub(tmp11);
        let tmp26 = tmp12
            .wrapping_add(tmp10)
            .wrapping_sub(tmp11)
            .wrapping_sub(z2);
        tmp10 = mul(z3, fix(0.790_569_415));
        tmp11 = mul(z4, fix(0.353_553_391));
        let tmp21 = tmp12.wrapping_add(tmp10).wrapping_add(tmp11);
        let tmp24 = tmp13.wrapping_sub(tmp10).wrapping_add(tmp11);
        tmp11 = tmp11.wrapping_add(tmp11);
        let tmp22 = z1.wrapping_add(tmp11);
        let tmp27 = z1.wrapping_sub(tmp11).wrapping_sub(tmp11);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z4 = workspace[8 * ctr + 5];
        z3 = mul(z4, fix(1.224_744_871));
        z4 = workspace[8 * ctr + 7];
        tmp13 = z2.wrapping_sub(z4);
        let mut tmp15 = mul(z1.wrapping_add(tmp13), fix(0.831_253_876));
        tmp11 = tmp15.wrapping_add(mul(z1, fix(0.513_743_148)));
        let tmp14 = tmp15.wrapping_sub(mul(tmp13, fix(2.176_250_899)));
        tmp13 = mul(z2, -fix(0.831_253_876));
        tmp15 = mul(z2, -fix(1.344_997_024));
        z2 = z1.wrapping_sub(z4);
        tmp12 = z3.wrapping_add(mul(z2, fix(1.406_466_353)));
        tmp10 = tmp12
            .wrapping_add(mul(z4, fix(2.457_431_844)))
            .wrapping_sub(tmp15);
        let tmp16 = tmp12
            .wrapping_sub(mul(z1, fix(1.112_434_820)))
            .wrapping_add(tmp13);
        tmp12 = mul(z2, fix(1.224_744_871)).wrapping_sub(z3);
        z2 = mul(z1.wrapping_add(z4), fix(0.575_212_477));
        tmp13 = tmp13.wrapping_add(
            z2.wrapping_add(mul(z1, fix(0.475_753_014)))
                .wrapping_sub(z3),
        );
        tmp15 = tmp15.wrapping_add(
            z2.wrapping_sub(mul(z4, fix(0.869_244_010)))
                .wrapping_add(z3),
        );
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][14] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][13] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][12] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][11] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp14),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp25.wrapping_add(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp25.wrapping_sub(tmp15),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp26.wrapping_add(tmp16),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp26.wrapping_sub(tmp16),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(tmp27, CONST_BITS + PASS1_BITS + 3));
    }
    out
}

/// `_jpeg_idct_16x16`: 16x16 inverse DCT with the accurate integer kernel.
// Port of: libjpeg-turbo src/jidctint.c#L2374-L2624 (libjpeg_turbo@e14cbfaa, 3.1.0)
pub(crate) fn idct_16x16(
    coef: &[i16; DCTSIZE2],
    quant: &[i32; DCTSIZE2],
    rl: &RangeLimit,
) -> IdctOut {
    let mut workspace = [0i32; 128];
    // Pass 1: process columns from input, store into work array.
    for ctr in 0..8usize {
        // Even part
        let mut tmp0 = dequantize(coef[ctr], quant[ctr]);
        tmp0 = lshift(tmp0, CONST_BITS);
        // Add fudge factor here for final descale.
        tmp0 = tmp0.wrapping_add(1i32 << CONST_BITS - PASS1_BITS - 1);
        let mut z1 = dequantize(coef[ctr + 32], quant[ctr + 32]);
        let mut tmp1 = mul(z1, fix(1.306_562_965));
        let mut tmp2 = mul(z1, fix(0.541_196_100));
        let mut tmp10 = tmp0.wrapping_add(tmp1);
        let mut tmp11 = tmp0.wrapping_sub(tmp1);
        let mut tmp12 = tmp0.wrapping_add(tmp2);
        let mut tmp13 = tmp0.wrapping_sub(tmp2);
        z1 = dequantize(coef[ctr + 16], quant[ctr + 16]);
        let mut z2 = dequantize(coef[ctr + 48], quant[ctr + 48]);
        let mut z3 = z1.wrapping_sub(z2);
        let mut z4 = mul(z3, fix(0.275_899_379));
        z3 = mul(z3, fix(1.387_039_845));
        tmp0 = z3.wrapping_add(mul(z2, fix(2.562_915_447)));
        tmp1 = z4.wrapping_add(mul(z1, fix(0.899_976_223)));
        tmp2 = z3.wrapping_sub(mul(z1, fix(0.601_344_887)));
        let mut tmp3 = z4.wrapping_sub(mul(z2, fix(0.509_795_579)));
        let tmp20 = tmp10.wrapping_add(tmp0);
        let tmp27 = tmp10.wrapping_sub(tmp0);
        let tmp21 = tmp12.wrapping_add(tmp1);
        let tmp26 = tmp12.wrapping_sub(tmp1);
        let tmp22 = tmp13.wrapping_add(tmp2);
        let tmp25 = tmp13.wrapping_sub(tmp2);
        let tmp23 = tmp11.wrapping_add(tmp3);
        let tmp24 = tmp11.wrapping_sub(tmp3);
        // Odd part
        z1 = dequantize(coef[ctr + 8], quant[ctr + 8]);
        z2 = dequantize(coef[ctr + 24], quant[ctr + 24]);
        z3 = dequantize(coef[ctr + 40], quant[ctr + 40]);
        z4 = dequantize(coef[ctr + 56], quant[ctr + 56]);
        tmp11 = z1.wrapping_add(z3);
        tmp1 = mul(z1.wrapping_add(z2), fix(1.353_318_001));
        tmp2 = mul(tmp11, fix(1.247_225_013));
        tmp3 = mul(z1.wrapping_add(z4), fix(1.093_201_867));
        tmp10 = mul(z1.wrapping_sub(z4), fix(0.897_167_586));
        tmp11 = mul(tmp11, fix(0.666_655_658));
        tmp12 = mul(z1.wrapping_sub(z2), fix(0.410_524_528));
        tmp0 = tmp1
            .wrapping_add(tmp2)
            .wrapping_add(tmp3)
            .wrapping_sub(mul(z1, fix(2.286_341_144)));
        tmp13 = tmp10
            .wrapping_add(tmp11)
            .wrapping_add(tmp12)
            .wrapping_sub(mul(z1, fix(1.835_730_603)));
        z1 = mul(z2.wrapping_add(z3), fix(0.138_617_169));
        tmp1 = tmp1.wrapping_add(z1.wrapping_add(mul(z2, fix(0.071_888_074))));
        tmp2 = tmp2.wrapping_add(z1.wrapping_sub(mul(z3, fix(1.125_726_048))));
        z1 = mul(z3.wrapping_sub(z2), fix(1.407_403_738));
        tmp11 = tmp11.wrapping_add(z1.wrapping_sub(mul(z3, fix(0.766_367_282))));
        tmp12 = tmp12.wrapping_add(z1.wrapping_add(mul(z2, fix(1.971_951_411))));
        z2 = z2.wrapping_add(z4);
        z1 = mul(z2, -fix(0.666_655_658));
        tmp1 = tmp1.wrapping_add(z1);
        tmp3 = tmp3.wrapping_add(z1.wrapping_add(mul(z4, fix(1.065_388_962))));
        z2 = mul(z2, -fix(1.247_225_013));
        tmp10 = tmp10.wrapping_add(z2.wrapping_add(mul(z4, fix(3.141_271_809))));
        tmp12 = tmp12.wrapping_add(z2);
        z2 = mul(z3.wrapping_add(z4), -fix(1.353_318_001));
        tmp2 = tmp2.wrapping_add(z2);
        tmp3 = tmp3.wrapping_add(z2);
        z2 = mul(z4.wrapping_sub(z3), fix(0.410_524_528));
        tmp10 = tmp10.wrapping_add(z2);
        tmp11 = tmp11.wrapping_add(z2);
        // Final output stage
        workspace[ctr] = rshift(tmp20.wrapping_add(tmp0), CONST_BITS - PASS1_BITS);
        workspace[ctr + 120] = rshift(tmp20.wrapping_sub(tmp0), CONST_BITS - PASS1_BITS);
        workspace[ctr + 8] = rshift(tmp21.wrapping_add(tmp1), CONST_BITS - PASS1_BITS);
        workspace[ctr + 112] = rshift(tmp21.wrapping_sub(tmp1), CONST_BITS - PASS1_BITS);
        workspace[ctr + 16] = rshift(tmp22.wrapping_add(tmp2), CONST_BITS - PASS1_BITS);
        workspace[ctr + 104] = rshift(tmp22.wrapping_sub(tmp2), CONST_BITS - PASS1_BITS);
        workspace[ctr + 24] = rshift(tmp23.wrapping_add(tmp3), CONST_BITS - PASS1_BITS);
        workspace[ctr + 96] = rshift(tmp23.wrapping_sub(tmp3), CONST_BITS - PASS1_BITS);
        workspace[ctr + 32] = rshift(tmp24.wrapping_add(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 88] = rshift(tmp24.wrapping_sub(tmp10), CONST_BITS - PASS1_BITS);
        workspace[ctr + 40] = rshift(tmp25.wrapping_add(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 80] = rshift(tmp25.wrapping_sub(tmp11), CONST_BITS - PASS1_BITS);
        workspace[ctr + 48] = rshift(tmp26.wrapping_add(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 72] = rshift(tmp26.wrapping_sub(tmp12), CONST_BITS - PASS1_BITS);
        workspace[ctr + 56] = rshift(tmp27.wrapping_add(tmp13), CONST_BITS - PASS1_BITS);
        workspace[ctr + 64] = rshift(tmp27.wrapping_sub(tmp13), CONST_BITS - PASS1_BITS);
    }
    // Pass 2: process 16 rows from work array, store into output array.
    let mut out = [[0u8; 16]; 16];
    for ctr in 0..16usize {
        // Even part
        // Add fudge factor here for final descale.
        let mut tmp0 = workspace[8 * ctr].wrapping_add(1i32 << PASS1_BITS + 2);
        tmp0 = lshift(tmp0, CONST_BITS);
        let mut z1 = workspace[8 * ctr + 4];
        let mut tmp1 = mul(z1, fix(1.306_562_965));
        let mut tmp2 = mul(z1, fix(0.541_196_100));
        let mut tmp10 = tmp0.wrapping_add(tmp1);
        let mut tmp11 = tmp0.wrapping_sub(tmp1);
        let mut tmp12 = tmp0.wrapping_add(tmp2);
        let mut tmp13 = tmp0.wrapping_sub(tmp2);
        z1 = workspace[8 * ctr + 2];
        let mut z2 = workspace[8 * ctr + 6];
        let mut z3 = z1.wrapping_sub(z2);
        let mut z4 = mul(z3, fix(0.275_899_379));
        z3 = mul(z3, fix(1.387_039_845));
        tmp0 = z3.wrapping_add(mul(z2, fix(2.562_915_447)));
        tmp1 = z4.wrapping_add(mul(z1, fix(0.899_976_223)));
        tmp2 = z3.wrapping_sub(mul(z1, fix(0.601_344_887)));
        let mut tmp3 = z4.wrapping_sub(mul(z2, fix(0.509_795_579)));
        let tmp20 = tmp10.wrapping_add(tmp0);
        let tmp27 = tmp10.wrapping_sub(tmp0);
        let tmp21 = tmp12.wrapping_add(tmp1);
        let tmp26 = tmp12.wrapping_sub(tmp1);
        let tmp22 = tmp13.wrapping_add(tmp2);
        let tmp25 = tmp13.wrapping_sub(tmp2);
        let tmp23 = tmp11.wrapping_add(tmp3);
        let tmp24 = tmp11.wrapping_sub(tmp3);
        // Odd part
        z1 = workspace[8 * ctr + 1];
        z2 = workspace[8 * ctr + 3];
        z3 = workspace[8 * ctr + 5];
        z4 = workspace[8 * ctr + 7];
        tmp11 = z1.wrapping_add(z3);
        tmp1 = mul(z1.wrapping_add(z2), fix(1.353_318_001));
        tmp2 = mul(tmp11, fix(1.247_225_013));
        tmp3 = mul(z1.wrapping_add(z4), fix(1.093_201_867));
        tmp10 = mul(z1.wrapping_sub(z4), fix(0.897_167_586));
        tmp11 = mul(tmp11, fix(0.666_655_658));
        tmp12 = mul(z1.wrapping_sub(z2), fix(0.410_524_528));
        tmp0 = tmp1
            .wrapping_add(tmp2)
            .wrapping_add(tmp3)
            .wrapping_sub(mul(z1, fix(2.286_341_144)));
        tmp13 = tmp10
            .wrapping_add(tmp11)
            .wrapping_add(tmp12)
            .wrapping_sub(mul(z1, fix(1.835_730_603)));
        z1 = mul(z2.wrapping_add(z3), fix(0.138_617_169));
        tmp1 = tmp1.wrapping_add(z1.wrapping_add(mul(z2, fix(0.071_888_074))));
        tmp2 = tmp2.wrapping_add(z1.wrapping_sub(mul(z3, fix(1.125_726_048))));
        z1 = mul(z3.wrapping_sub(z2), fix(1.407_403_738));
        tmp11 = tmp11.wrapping_add(z1.wrapping_sub(mul(z3, fix(0.766_367_282))));
        tmp12 = tmp12.wrapping_add(z1.wrapping_add(mul(z2, fix(1.971_951_411))));
        z2 = z2.wrapping_add(z4);
        z1 = mul(z2, -fix(0.666_655_658));
        tmp1 = tmp1.wrapping_add(z1);
        tmp3 = tmp3.wrapping_add(z1.wrapping_add(mul(z4, fix(1.065_388_962))));
        z2 = mul(z2, -fix(1.247_225_013));
        tmp10 = tmp10.wrapping_add(z2.wrapping_add(mul(z4, fix(3.141_271_809))));
        tmp12 = tmp12.wrapping_add(z2);
        z2 = mul(z3.wrapping_add(z4), -fix(1.353_318_001));
        tmp2 = tmp2.wrapping_add(z2);
        tmp3 = tmp3.wrapping_add(z2);
        z2 = mul(z4.wrapping_sub(z3), fix(0.410_524_528));
        tmp10 = tmp10.wrapping_add(z2);
        tmp11 = tmp11.wrapping_add(z2);
        // Final output stage
        out[ctr][0] = rl.idct(rshift(
            tmp20.wrapping_add(tmp0),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][15] = rl.idct(rshift(
            tmp20.wrapping_sub(tmp0),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][1] = rl.idct(rshift(
            tmp21.wrapping_add(tmp1),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][14] = rl.idct(rshift(
            tmp21.wrapping_sub(tmp1),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][2] = rl.idct(rshift(
            tmp22.wrapping_add(tmp2),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][13] = rl.idct(rshift(
            tmp22.wrapping_sub(tmp2),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][3] = rl.idct(rshift(
            tmp23.wrapping_add(tmp3),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][12] = rl.idct(rshift(
            tmp23.wrapping_sub(tmp3),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][4] = rl.idct(rshift(
            tmp24.wrapping_add(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][11] = rl.idct(rshift(
            tmp24.wrapping_sub(tmp10),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][5] = rl.idct(rshift(
            tmp25.wrapping_add(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][10] = rl.idct(rshift(
            tmp25.wrapping_sub(tmp11),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][6] = rl.idct(rshift(
            tmp26.wrapping_add(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][9] = rl.idct(rshift(
            tmp26.wrapping_sub(tmp12),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][7] = rl.idct(rshift(
            tmp27.wrapping_add(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
        out[ctr][8] = rl.idct(rshift(
            tmp27.wrapping_sub(tmp13),
            CONST_BITS + PASS1_BITS + 3,
        ));
    }
    out
}

/// Dispatches to the method for `dct_scaled_size` (1..=16), as `jddctmgr.c` does.
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
        9 => idct_9x9(coef, quant, rl),
        10 => idct_10x10(coef, quant, rl),
        11 => idct_11x11(coef, quant, rl),
        12 => idct_12x12(coef, quant, rl),
        13 => idct_13x13(coef, quant, rl),
        14 => idct_14x14(coef, quant, rl),
        15 => idct_15x15(coef, quant, rl),
        16 => idct_16x16(coef, quant, rl),
        _ => return None,
    })
}
