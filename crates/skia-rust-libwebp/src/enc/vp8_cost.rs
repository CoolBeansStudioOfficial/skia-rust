// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8 coefficient cost model of libwebp 1.4.0: `src/enc/cost_enc.c` and
//! `cost_enc.h` (`VP8CalculateLevelCosts`, `VariableLevelCost`, `VP8RecordCoeffs`,
//! `VP8RecordStats`, `VP8BitCost`, `VP8LevelCost`), `src/dsp/cost.c` (`GetResidualCost_C`,
//! `SetResidualCoeffs_C`), and the probability state `VP8EncProba` (`vp8i_enc.h`).
//!
//! The C `remapped_costs_` pointer table is `level_cost[type][VP8EncBands[n]][ctx]`; the port
//! reads that directly, which is the same entry the C pointer names.

// Clippy allows for the C arithmetic and control flow: the C code mixes int, uint32_t
// and uint8_t, spells table offsets as `0 + 0 * BPS`, nests the mode trees as `if` chains,
// and indexes by position. The port keeps those shapes so that each line can be checked
// against the C source; the casts are the width and sign conversions of the C source.
#![allow(
    clippy::identity_op,
    clippy::erasing_op,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::bool_to_int_with_if,
    clippy::cast_precision_loss,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::manual_range_contains,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::precedence,
    clippy::unusual_byte_groupings
)]

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use super::cost_tables::{
    VP8_ENC_BANDS, VP8_ENTROPY_COST, VP8_FIXED_COSTS_I4, VP8_FIXED_COSTS_I16, VP8_FIXED_COSTS_UV,
    VP8_LEVEL_CODES, VP8_LEVEL_FIXED_COSTS,
};

/// Port of `NUM_TYPES`.
pub const NUM_TYPES: usize = 4;
/// Port of `NUM_BANDS`.
pub const NUM_BANDS: usize = 8;
/// Port of `NUM_CTX`.
pub const NUM_CTX: usize = 3;
/// Port of `NUM_PROBAS`.
pub const NUM_PROBAS: usize = 11;
/// Port of `MAX_VARIABLE_LEVEL`: the last level with a variable cost.
pub const MAX_VARIABLE_LEVEL: usize = 67;

/// Port of `VP8EncProba` (`vp8i_enc.h`): the coefficient probabilities of the frame, their
/// statistics, and their level costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VP8EncProba {
    /// `segments_`: probabilities for the segment tree.
    pub segments: [u8; 3],
    /// `skip_proba_`: final probability of being skipped.
    pub skip_proba: u8,
    /// `coeffs_[type][band][ctx][i]`.
    pub coeffs: [[[[u8; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
    /// `stats_[type][band][ctx][i]`: the `proba_t` counters (16 bits of zeros, 16 of ones).
    pub stats: [[[[u32; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
    /// `level_cost_[type][band][ctx][level]`.
    pub level_cost: [[[[u16; MAX_VARIABLE_LEVEL + 1]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
    /// `dirty_`: `true` when `level_cost` must be recomputed.
    pub dirty: bool,
    /// `use_skip_proba_`.
    pub use_skip_proba: bool,
    /// `nb_skip_`: the number of skipped blocks.
    pub nb_skip: i32,
}

impl Default for VP8EncProba {
    fn default() -> Self {
        Self {
            segments: [0; 3],
            skip_proba: 0,
            coeffs: [[[[0; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
            stats: [[[[0; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
            level_cost: [[[[0; MAX_VARIABLE_LEVEL + 1]; NUM_CTX]; NUM_BANDS]; NUM_TYPES],
            dirty: false,
            use_skip_proba: false,
            nb_skip: 0,
        }
    }
}

/// Port of `VP8Residual` (`cost_enc.h`): the coefficients of one block being costed or recorded.
/// The probability, statistics and cost tables are those of the `VP8EncProba` passed with it.
#[derive(Debug, Clone, Copy)]
pub struct VP8Residual<'a> {
    /// `first`: the first coefficient position (0, or 1 for the luma AC after a DC).
    pub first: usize,
    /// `last`: the last non-zero position, or -1.
    pub last: i32,
    /// `coeffs`: the 16 quantized levels in zigzag order.
    pub coeffs: &'a [i16],
    /// `coeff_type`: 0 = i16-AC, 1 = i16-DC, 2 = chroma-AC, 3 = i4-AC.
    pub coeff_type: usize,
}

/// Port of `VP8InitResidual`: a residual of `first` and `coeff_type` with no coefficients yet.
#[must_use]
pub fn init_residual<'a>(first: usize, coeff_type: usize) -> VP8Residual<'a> {
    VP8Residual {
        first,
        last: -1,
        coeffs: &[],
        coeff_type,
    }
}

/// Port of `VP8BitCost`.
#[inline]
#[must_use]
pub fn bit_cost(bit: i32, proba: u8) -> i32 {
    if bit == 0 {
        i32::from(VP8_ENTROPY_COST[proba as usize])
    } else {
        i32::from(VP8_ENTROPY_COST[255 - proba as usize])
    }
}

/// Port of `VP8LevelCost`: the fixed plus the variable cost of `level` in `table`.
#[inline]
#[must_use]
pub fn level_cost(table: &[u16; MAX_VARIABLE_LEVEL + 1], level: i32) -> i32 {
    let idx = if level as usize > MAX_VARIABLE_LEVEL {
        MAX_VARIABLE_LEVEL
    } else {
        level as usize
    };
    i32::from(VP8_LEVEL_FIXED_COSTS[level as usize]) + i32::from(table[idx])
}

/// Port of `VP8RecordStats`: counts `bit` in `stats`, halving the counts when they saturate.
/// Returns `bit`.
#[inline]
pub fn record_stats(bit: i32, stats: &mut u32) -> i32 {
    let mut p = *stats;
    if p >= 0xfffe_0000u32 {
        p = ((p.wrapping_add(1)) >> 1) & 0x7fff_7fffu32; // -> divide the stats by 2.
    }
    p = p.wrapping_add(0x0001_0000u32.wrapping_add(bit as u32));
    *stats = p;
    bit
}

/// Port of `VariableLevelCost`.
fn variable_level_cost(level: usize, probas: &[u8; NUM_PROBAS]) -> i32 {
    let mut pattern = i32::from(VP8_LEVEL_CODES[level - 1][0]);
    let mut bits = i32::from(VP8_LEVEL_CODES[level - 1][1]);
    let mut cost = 0;
    let mut i = 2;
    while pattern != 0 {
        if pattern & 1 != 0 {
            cost += bit_cost(bits & 1, probas[i]);
        }
        bits >>= 1;
        pattern >>= 1;
        i += 1;
    }
    cost
}

/// Port of `VP8CalculateLevelCosts`: recomputes the level costs when the probabilities changed.
pub fn calculate_level_costs(proba: &mut VP8EncProba) {
    if !proba.dirty {
        return; // nothing to do.
    }
    for ctype in 0..NUM_TYPES {
        for band in 0..NUM_BANDS {
            for ctx in 0..NUM_CTX {
                let p = proba.coeffs[ctype][band][ctx];
                let cost0 = if ctx > 0 { bit_cost(1, p[0]) } else { 0 };
                let cost_base = bit_cost(1, p[1]) + cost0;
                let table = &mut proba.level_cost[ctype][band][ctx];
                table[0] = (bit_cost(0, p[1]) + cost0) as u16;
                for v in 1..=MAX_VARIABLE_LEVEL {
                    table[v] = (cost_base + variable_level_cost(v, &p)) as u16;
                }
            }
        }
    }
    proba.dirty = false;
}

/// Port of `SetResidualCoeffs_C`: records the coefficients and the position of the last non-zero
/// one.
pub fn set_residual_coeffs<'a>(coeffs: &'a [i16], res: &mut VP8Residual<'a>) {
    res.last = -1;
    for n in (0..16).rev() {
        if coeffs[n] != 0 {
            res.last = n as i32;
            break;
        }
    }
    res.coeffs = coeffs;
}

/// Port of `GetResidualCost_C`: the bit cost of coding `res` with the context `ctx0`.
#[must_use]
pub fn get_residual_cost(proba: &VP8EncProba, ctx0: usize, res: &VP8Residual<'_>) -> i32 {
    let mut n = res.first;
    // should be prob[VP8EncBands[n]], but it's equivalent for n=0 or 1
    let ct = res.coeff_type;
    let p0 = proba.coeffs[ct][usize::from(VP8_ENC_BANDS[n])][ctx0][0];
    // costs[n][ctx0] is the remapped table for position n.
    let mut t = &proba.level_cost[ct][usize::from(VP8_ENC_BANDS[n])][ctx0];
    // bit_cost(1, p0) is already incorporated in t[] tables, but only if ctx != 0
    // (as required by the syntax). For ctx0 == 0, we need to add it here or it'll be missing
    // during the loop.
    let mut cost = if ctx0 == 0 { bit_cost(1, p0) } else { 0 };
    if res.last < 0 {
        return bit_cost(0, p0);
    }
    while (n as i32) < res.last {
        let v = i32::from(res.coeffs[n]).abs();
        let ctx = if v >= 2 { 2 } else { v as usize };
        cost += level_cost(t, v);
        t = &proba.level_cost[ct][usize::from(VP8_ENC_BANDS[n + 1])][ctx];
        n += 1;
    }
    // Last coefficient is always non-zero
    {
        let v = i32::from(res.coeffs[n]).abs();
        cost += level_cost(t, v);
        if n < 15 {
            let b = usize::from(VP8_ENC_BANDS[n + 1]);
            let ctx = if v == 1 { 1 } else { 2 };
            let last_p0 = proba.coeffs[ct][b][ctx][0];
            cost += bit_cost(0, last_p0);
        }
    }
    cost
}

/// Port of `VP8RecordCoeffs`: counts the statistics of `res` in context `ctx`, and returns 1
/// when the block is not empty.
pub fn record_coeffs(proba: &mut VP8EncProba, ctx: usize, res: &VP8Residual<'_>) -> i32 {
    let ct = res.coeff_type;
    let mut n = res.first;
    // `s` is the (band, ctx) of the C `proba_t* s`, and `k` the offset within it.
    let mut s = (n, ctx);
    if res.last < 0 {
        record_stats(0, &mut proba.stats[ct][s.0][s.1][0]);
        return 0;
    }
    while (n as i32) <= res.last {
        record_stats(1, &mut proba.stats[ct][s.0][s.1][0]); // order of record doesn't matter
        let mut v;
        loop {
            v = i32::from(res.coeffs[n]);
            n += 1;
            if v != 0 {
                break;
            }
            record_stats(0, &mut proba.stats[ct][s.0][s.1][1]);
            s = (usize::from(VP8_ENC_BANDS[n]), 0);
        }
        record_stats(1, &mut proba.stats[ct][s.0][s.1][1]);
        let bit = i32::from(2u32 < (v + 1) as u32);
        if record_stats(bit, &mut proba.stats[ct][s.0][s.1][2]) == 0 {
            // v = -1 or 1
            s = (usize::from(VP8_ENC_BANDS[n]), 1);
        } else {
            v = v.abs();
            if v > MAX_VARIABLE_LEVEL as i32 {
                v = MAX_VARIABLE_LEVEL as i32;
            }
            let bits = i32::from(VP8_LEVEL_CODES[(v - 1) as usize][1]);
            let mut pattern = i32::from(VP8_LEVEL_CODES[(v - 1) as usize][0]);
            let mut i = 0;
            loop {
                pattern >>= 1;
                if pattern == 0 {
                    break;
                }
                let mask = 2 << i;
                if pattern & 1 != 0 {
                    let bit = i32::from(bits & mask != 0);
                    record_stats(bit, &mut proba.stats[ct][s.0][s.1][3 + i as usize]);
                }
                i += 1;
            }
            s = (usize::from(VP8_ENC_BANDS[n]), 2);
        }
    }
    if n < 16 {
        record_stats(0, &mut proba.stats[ct][s.0][s.1][0]);
    }
    1
}

/// Port of `VP8FixedCostsUV` (`cost_enc.c`), for the chroma mode costs.
#[must_use]
pub fn fixed_costs_uv(mode: usize) -> i32 {
    i32::from(VP8_FIXED_COSTS_UV[mode])
}

/// Port of `VP8FixedCostsI16`.
#[must_use]
pub fn fixed_costs_i16(mode: usize) -> i32 {
    i32::from(VP8_FIXED_COSTS_I16[mode])
}

/// Port of `VP8FixedCostsI4[top][left][mode]`.
#[must_use]
pub fn fixed_costs_i4(top: usize, left: usize, mode: usize) -> i32 {
    i32::from(VP8_FIXED_COSTS_I4[top][left][mode])
}
