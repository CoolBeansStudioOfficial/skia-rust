// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the C cost kernels of libwebp `src/dsp/lossless_enc.c` (and `lossless_common.h`):
//! the fast `v * log2(v)` table, the bit-entropy and streak statistics, and the extra-bit costs.
//!
//! The float expressions keep the C evaluation order and types: `f32` operations where C uses
//! `float`, and the `double` operations of `FastSLog2Slow` for large counts. Rust never contracts
//! `a * b + c` into an FMA, which matches the `-ffp-contract=off` reference build.

// Module-level clippy allows. The C arithmetic mixes int, uint32_t, size_t and float, and the
// casts below are the width and sign conversions of the C source. The index loops, `if`/`else`
// chains and exact float comparisons keep the C control flow and evaluation order, so that the
// code can be read against the C source; they are not simplified.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::needless_range_loop,
    clippy::float_cmp,
    clippy::manual_midpoint,
    clippy::redundant_else,
    clippy::single_match,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::needless_for_each,
    clippy::while_let_loop,
    clippy::approx_constant,
    clippy::too_many_arguments,
    clippy::match_same_arms,
    clippy::if_not_else,
    clippy::needless_pass_by_value,
    clippy::explicit_iter_loop,
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::manual_range_contains
)]

use super::tables::{K_LOG2_TABLE, K_SLOG2_TABLE};

/// Port of `VP8L_NON_TRIVIAL_SYM`.
pub const NON_TRIVIAL_SYM: u32 = 0xffff_ffff;

/// Port of `LOG_2_RECIPROCAL` (a `double` constant).
const LOG_2_RECIPROCAL: f64 = 1.442_695_040_888_963_4;
/// Port of `LOG_LOOKUP_IDX_MAX`.
const LOG_LOOKUP_IDX_MAX: u32 = 256;
/// Port of `APPROX_LOG_WITH_CORRECTION_MAX`.
const APPROX_LOG_WITH_CORRECTION_MAX: u32 = 65536;
/// Port of `APPROX_LOG_MAX`.
const APPROX_LOG_MAX: u32 = 4096;

/// Port of `BitsLog2Floor`: the index of the highest set bit of `n` (0 for 0, as the C fallback).
#[must_use]
pub fn bits_log2_floor(n: u32) -> i32 {
    if n == 0 {
        0
    } else {
        31 - n.leading_zeros() as i32
    }
}

/// Port of `FastSLog2Slow_C`: `v * log2(v)` for `v >= 256`.
fn fast_slog2_slow(v: u32) -> f32 {
    if v < APPROX_LOG_WITH_CORRECTION_MAX {
        let log_cnt = bits_log2_floor(v) - 7;
        let y: u32 = 1 << log_cnt;
        let v_f = v as f32;
        let orig_v = v;
        let shifted = (v >> log_cnt) as usize;
        let correction = ((23 * (orig_v & (y - 1))) >> 4) as i32;
        // C: `v_f * (kLog2Table[v] + log_cnt) + correction`, all in float.
        v_f * (K_LOG2_TABLE[shifted] + log_cnt as f32) + correction as f32
    } else {
        // C: `(float)(LOG_2_RECIPROCAL * v * log((double)v))`.
        (LOG_2_RECIPROCAL * f64::from(v) * f64::from(v).ln()) as f32
    }
}

/// Port of `VP8LFastSLog2`.
#[must_use]
pub fn fast_slog2(v: u32) -> f32 {
    if v < LOG_LOOKUP_IDX_MAX {
        K_SLOG2_TABLE[v as usize]
    } else {
        fast_slog2_slow(v)
    }
}

/// Port of `FastLog2Slow_C`: `log2(v)` for `v >= 256`.
fn fast_log2_slow(v: u32) -> f32 {
    if v < APPROX_LOG_WITH_CORRECTION_MAX {
        let log_cnt = bits_log2_floor(v) - 7;
        let y: u32 = 1 << log_cnt;
        let orig_v = v;
        let shifted = (v >> log_cnt) as usize;
        // C: `log_2 = kLog2Table[v] + log_cnt;` is a float sum, widened to double.
        let mut log_2 = f64::from(K_LOG2_TABLE[shifted] + log_cnt as f32);
        if orig_v >= APPROX_LOG_MAX {
            let correction = ((23 * (orig_v & (y - 1))) >> 4) as i32;
            log_2 += f64::from(correction) / f64::from(orig_v);
        }
        log_2 as f32
    } else {
        // C: `(float)(LOG_2_RECIPROCAL * log((double)v))`.
        (LOG_2_RECIPROCAL * f64::from(v).ln()) as f32
    }
}

/// Port of `VP8LFastLog2`.
#[must_use]
pub fn fast_log2(v: u32) -> f32 {
    if v < LOG_LOOKUP_IDX_MAX {
        K_LOG2_TABLE[v as usize]
    } else {
        fast_log2_slow(v)
    }
}

/// Port of `VP8LBitEntropy`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BitEntropy {
    pub entropy: f32,
    pub sum: u32,
    pub nonzeros: i32,
    pub max_val: u32,
    pub nonzero_code: u32,
}

impl BitEntropy {
    /// Port of `VP8LBitEntropyInit`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entropy: 0.0,
            sum: 0,
            nonzeros: 0,
            max_val: 0,
            nonzero_code: NON_TRIVIAL_SYM,
        }
    }
}

impl Default for BitEntropy {
    fn default() -> Self {
        Self::new()
    }
}

/// Port of `VP8LStreaks`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Streaks {
    /// Index: 0 = zero streak, 1 = non-zero streak.
    pub counts: [i32; 2],
    /// `[zero/non-zero][streak < 3 / streak >= 3]`.
    pub streaks: [[i32; 2]; 2],
}

/// Port of `VP8LBitsEntropyUnrefined`.
#[must_use]
pub fn bits_entropy_unrefined(array: &[u32]) -> BitEntropy {
    let mut entropy = BitEntropy::new();
    for (i, &a) in array.iter().enumerate() {
        if a != 0 {
            entropy.sum = entropy.sum.wrapping_add(a);
            entropy.nonzero_code = i as u32;
            entropy.nonzeros += 1;
            entropy.entropy -= fast_slog2(a);
            if entropy.max_val < a {
                entropy.max_val = a;
            }
        }
    }
    entropy.entropy += fast_slog2(entropy.sum);
    entropy
}

/// Port of `GetEntropyUnrefinedHelper`: closes the streak of `val_prev` ending before `i`.
fn get_entropy_unrefined_helper(
    val: u32,
    i: usize,
    val_prev: &mut u32,
    i_prev: &mut usize,
    bit_entropy: &mut BitEntropy,
    stats: &mut Streaks,
) {
    let streak = i as i32 - *i_prev as i32;
    if *val_prev != 0 {
        bit_entropy.sum = bit_entropy
            .sum
            .wrapping_add(val_prev.wrapping_mul(streak as u32));
        bit_entropy.nonzeros += streak;
        bit_entropy.nonzero_code = *i_prev as u32;
        bit_entropy.entropy -= fast_slog2(*val_prev) * streak as f32;
        if bit_entropy.max_val < *val_prev {
            bit_entropy.max_val = *val_prev;
        }
    }
    let nz = usize::from(*val_prev != 0);
    stats.counts[nz] += i32::from(streak > 3);
    stats.streaks[nz][usize::from(streak > 3)] += streak;
    *val_prev = val;
    *i_prev = i;
}

/// Port of `GetEntropyUnrefined_C`: the entropy and streak statistics of one population.
#[must_use]
pub fn get_entropy_unrefined(x: &[u32], length: usize) -> (BitEntropy, Streaks) {
    let mut stats = Streaks::default();
    let mut bit_entropy = BitEntropy::new();
    let mut i_prev: usize = 0;
    let mut x_prev = x[0];
    for i in 1..length {
        let xi = x[i];
        if xi != x_prev {
            get_entropy_unrefined_helper(
                xi,
                i,
                &mut x_prev,
                &mut i_prev,
                &mut bit_entropy,
                &mut stats,
            );
        }
    }
    get_entropy_unrefined_helper(
        0,
        length,
        &mut x_prev,
        &mut i_prev,
        &mut bit_entropy,
        &mut stats,
    );
    bit_entropy.entropy += fast_slog2(bit_entropy.sum);
    (bit_entropy, stats)
}

/// Port of `GetCombinedEntropyUnrefined_C`: the statistics of `X[i] + Y[i]`.
#[must_use]
pub fn get_combined_entropy_unrefined(
    x: &[u32],
    y: &[u32],
    length: usize,
) -> (BitEntropy, Streaks) {
    let mut stats = Streaks::default();
    let mut bit_entropy = BitEntropy::new();
    let mut i_prev: usize = 0;
    let mut xy_prev = x[0].wrapping_add(y[0]);
    for i in 1..length {
        let xy = x[i].wrapping_add(y[i]);
        if xy != xy_prev {
            get_entropy_unrefined_helper(
                xy,
                i,
                &mut xy_prev,
                &mut i_prev,
                &mut bit_entropy,
                &mut stats,
            );
        }
    }
    get_entropy_unrefined_helper(
        0,
        length,
        &mut xy_prev,
        &mut i_prev,
        &mut bit_entropy,
        &mut stats,
    );
    bit_entropy.entropy += fast_slog2(bit_entropy.sum);
    (bit_entropy, stats)
}

/// Port of `ExtraCost_C`: the cost of the length or distance extra bits.
#[must_use]
pub fn extra_cost(population: &[u32], length: usize) -> u32 {
    let mut cost = population[4].wrapping_add(population[5]);
    let mut i: usize = 2;
    while i + 1 < length / 2 {
        // C: `for (i = 2; i < length / 2 - 1; ++i)`.
        cost = cost.wrapping_add(
            (i as u32).wrapping_mul(population[2 * i + 2].wrapping_add(population[2 * i + 3])),
        );
        i += 1;
    }
    cost
}

/// Port of `ExtraCostCombined_C`: `extra_cost` of `X + Y`.
#[must_use]
pub fn extra_cost_combined(x: &[u32], y: &[u32], length: usize) -> u32 {
    let mut cost = x[4]
        .wrapping_add(y[4])
        .wrapping_add(x[5])
        .wrapping_add(y[5]);
    let mut i: usize = 2;
    while i + 1 < length / 2 {
        let xy0 = x[2 * i + 2].wrapping_add(y[2 * i + 2]);
        let xy1 = x[2 * i + 3].wrapping_add(y[2 * i + 3]);
        cost = cost.wrapping_add((i as u32).wrapping_mul(xy0.wrapping_add(xy1)));
        i += 1;
    }
    cost
}
