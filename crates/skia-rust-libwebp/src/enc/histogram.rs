// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/enc/histogram_enc.{c,h}`: the symbol histograms of the entropy-coded
//! image, their bit-cost estimates, and the clustering of the image's histograms into the
//! Huffman groups that the entropy image selects.
//!
//! The float cost model keeps the C expressions and their evaluation order (see `entropy.rs`).
//! `NUM_PARTITIONS` binning and the greedy and stochastic combiners are ported in full.

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

use super::backward_refs::{BackwardRefs, PixOrCopy, PixOrCopyMode};
use super::entropy::{
    BitEntropy, NON_TRIVIAL_SYM, Streaks, extra_cost, extra_cost_combined,
    get_combined_entropy_unrefined, get_entropy_unrefined,
};
use super::prefix::prefix_encode_bits;

/// Port of `NUM_LITERAL_CODES`.
pub const NUM_LITERAL_CODES: usize = 256;
/// Port of `NUM_LENGTH_CODES`.
pub const NUM_LENGTH_CODES: usize = 24;
/// Port of `NUM_DISTANCE_CODES`.
pub const NUM_DISTANCE_CODES: usize = 40;
/// Port of `MAX_BIT_COST` (`FLT_MAX`).
const MAX_BIT_COST: f32 = f32::MAX;
/// Port of `NUM_PARTITIONS`.
pub const NUM_PARTITIONS: usize = 4;
/// Port of `BIN_SIZE`.
const BIN_SIZE: usize = NUM_PARTITIONS * NUM_PARTITIONS * NUM_PARTITIONS;
/// Port of `MAX_HISTO_GREEDY`.
const MAX_HISTO_GREEDY: i32 = 100;
/// Port of `CODE_LENGTH_CODES`.
const CODE_LENGTH_CODES: i32 = 19;
/// Port of `kInvalidHistogramSymbol`.
pub const INVALID_HISTOGRAM_SYMBOL: u16 = u16::MAX;

/// Port of `VP8LHistogramNumCodes`: the literal alphabet size for `palette_code_bits`.
#[must_use]
pub fn histogram_num_codes(palette_code_bits: i32) -> usize {
    NUM_LITERAL_CODES
        + NUM_LENGTH_CODES
        + if palette_code_bits > 0 {
            1usize << palette_code_bits
        } else {
            0
        }
}

/// Port of `VP8LHistogram`.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    /// `literal_`: the green, length and colour-cache alphabet (`histogram_num_codes` long).
    pub literal: Vec<u32>,
    pub red: [u32; NUM_LITERAL_CODES],
    pub blue: [u32; NUM_LITERAL_CODES],
    pub alpha: [u32; NUM_LITERAL_CODES],
    pub distance: [u32; NUM_DISTANCE_CODES],
    pub palette_code_bits: i32,
    /// True if the red, blue and alpha codes are trivial (a single symbol), else
    /// `NON_TRIVIAL_SYM`.
    pub trivial_symbol: u32,
    pub bit_cost: f32,
    pub literal_cost: f32,
    pub red_cost: f32,
    pub blue_cost: f32,
    /// Literal, red, blue, alpha, distance.
    pub is_used: [u8; 5],
}

impl Histogram {
    /// Port of `VP8LAllocateHistogram` with `VP8LHistogramInit(init_arrays = 0)`, zeroed.
    #[must_use]
    pub fn new(cache_bits: i32) -> Self {
        Self {
            literal: vec![0; histogram_num_codes(cache_bits)],
            red: [0; NUM_LITERAL_CODES],
            blue: [0; NUM_LITERAL_CODES],
            alpha: [0; NUM_LITERAL_CODES],
            distance: [0; NUM_DISTANCE_CODES],
            palette_code_bits: cache_bits,
            trivial_symbol: 0,
            bit_cost: 0.0,
            literal_cost: 0.0,
            red_cost: 0.0,
            blue_cost: 0.0,
            is_used: [0; 5],
        }
    }

    /// Port of `HistogramClear`: zeroes every count and cached cost, keeping the alphabet size.
    fn clear(&mut self) {
        let bits = self.palette_code_bits;
        *self = Self::new(bits);
    }

    /// Port of `VP8LHistogramAddSinglePixOrCopy` (without a distance modifier).
    pub fn add_single_pix_or_copy(&mut self, v: &PixOrCopy) {
        self.add_single_pix_or_copy_with(v, None);
    }

    /// Port of `VP8LHistogramAddSinglePixOrCopy` with an optional distance modifier (the 2D
    /// plane code of the cost model).
    pub fn add_single_pix_or_copy_with(
        &mut self,
        v: &PixOrCopy,
        distance_modifier: Option<&dyn Fn(u32) -> u32>,
    ) {
        match v.mode {
            PixOrCopyMode::Literal => {
                self.alpha[v.literal_component(3) as usize] += 1;
                self.red[v.literal_component(2) as usize] += 1;
                self.literal[v.literal_component(1) as usize] += 1;
                self.blue[v.literal_component(0) as usize] += 1;
            }
            PixOrCopyMode::CacheIdx => {
                let literal_ix = NUM_LITERAL_CODES + NUM_LENGTH_CODES + v.argb_or_distance as usize;
                self.literal[literal_ix] += 1;
            }
            PixOrCopyMode::Copy => {
                let (code, _extra_bits) = prefix_encode_bits(v.length() as i32);
                self.literal[NUM_LITERAL_CODES + code as usize] += 1;
                let distance = match distance_modifier {
                    None => v.argb_or_distance,
                    Some(modify) => modify(v.argb_or_distance),
                };
                let (code, _extra_bits) = prefix_encode_bits(distance as i32);
                self.distance[code as usize] += 1;
            }
        }
    }

    /// Port of `VP8LHistogramStoreRefs`.
    pub fn store_refs(&mut self, refs: &BackwardRefs) {
        for v in &refs.refs {
            self.add_single_pix_or_copy(v);
        }
    }

    /// Port of `VP8LHistogramCreate`.
    pub fn create(&mut self, refs: &BackwardRefs, palette_code_bits: i32) {
        if palette_code_bits >= 0 {
            self.palette_code_bits = palette_code_bits;
        }
        self.clear();
        self.store_refs(refs);
    }

    /// Port of `VP8LHistogramInit(p, bits, 0)`: resets only the cached scalars.
    fn reset_scalars(&mut self) {
        self.trivial_symbol = 0;
        self.bit_cost = 0.0;
        self.literal_cost = 0.0;
        self.red_cost = 0.0;
        self.blue_cost = 0.0;
        self.is_used = [0; 5];
    }
}

/// Port of `VP8LHistogramSet`: `max_size` slots of which the first `size` are in use (the C
/// `NULL` slots are `None`).
#[derive(Clone, Debug, Default)]
pub struct HistogramSet {
    pub size: usize,
    pub max_size: usize,
    pub histograms: Vec<Option<Box<Histogram>>>,
}

impl HistogramSet {
    /// Port of `VP8LAllocateHistogramSet(size, cache_bits)`.
    #[must_use]
    pub fn new(size: usize, cache_bits: i32) -> Self {
        let mut histograms = Vec::with_capacity(size);
        for _ in 0..size {
            let mut h = Box::new(Histogram::new(cache_bits));
            h.reset_scalars();
            histograms.push(Some(h));
        }
        Self {
            size,
            max_size: size,
            histograms,
        }
    }

    /// Port of `VP8LHistogramSetClear`: every slot becomes an empty histogram.
    pub fn clear(&mut self) {
        let cache_bits = self.histograms[0]
            .as_ref()
            .map_or(0, |h| h.palette_code_bits);
        self.histograms = (0..self.max_size)
            .map(|_| Some(Box::new(Histogram::new(cache_bits))))
            .collect();
        self.size = self.max_size;
    }

    /// Port of `HistogramSetRemoveHistogram`.
    fn remove(&mut self, i: usize, num_used: &mut i32) {
        self.histograms[i] = None;
        *num_used -= 1;
        if i + 1 == self.size {
            while self.size >= 1 && self.histograms[self.size - 1].is_none() {
                self.size -= 1;
            }
        }
    }

    /// Port of `RemoveEmptyHistograms`: compacts the used slots to the front.
    fn remove_empty(&mut self) {
        let mut size = 0;
        for i in 0..self.size {
            if self.histograms[i].is_some() {
                self.histograms.swap(size, i);
                size += 1;
            }
        }
        self.size = size;
    }

    fn get(&self, i: usize) -> &Histogram {
        self.histograms[i]
            .as_deref()
            .expect("histogram slot in use")
    }
}

/// Port of `BitsEntropyRefine`.
fn bits_entropy_refine(entropy: &BitEntropy) -> f32 {
    let mix: f32;
    if entropy.nonzeros < 5 {
        if entropy.nonzeros <= 1 {
            return 0.0;
        }
        if entropy.nonzeros == 2 {
            return 0.99 * entropy.sum as f32 + 0.01 * entropy.entropy;
        }
        if entropy.nonzeros == 3 {
            mix = 0.95;
        } else {
            mix = 0.7; // nonzeros == 4.
        }
    } else {
        mix = 0.627;
    }
    let mut min_limit = 2.0 * entropy.sum as f32 - entropy.max_val as f32;
    min_limit = mix * min_limit + (1.0 - mix) * entropy.entropy;
    if entropy.entropy < min_limit {
        min_limit
    } else {
        entropy.entropy
    }
}

/// Port of `InitialHuffmanCost`.
fn initial_huffman_cost() -> f32 {
    let k_huffman_code_of_huffman_code_size = CODE_LENGTH_CODES * 3;
    let k_small_bias: f32 = 9.1;
    k_huffman_code_of_huffman_code_size as f32 - k_small_bias
}

/// Port of `FinalHuffmanCost`.
fn final_huffman_cost(stats: &Streaks) -> f32 {
    let mut retval = initial_huffman_cost();
    retval += stats.counts[0] as f32 * 1.5625 + 0.234_375 * stats.streaks[0][1] as f32;
    retval += stats.counts[1] as f32 * 2.578_125 + 0.703_125 * stats.streaks[1][1] as f32;
    retval += 1.796_875 * stats.streaks[0][0] as f32;
    retval += 3.28125 * stats.streaks[1][0] as f32;
    retval
}

/// Port of `PopulationCost`: the cost of one alphabet, its trivial symbol (if requested) and
/// whether it is used.
fn population_cost(
    population: &[u32],
    length: usize,
    trivial_sym: Option<&mut u32>,
    is_used: &mut u8,
) -> f32 {
    let (bit_entropy, stats) = get_entropy_unrefined(population, length);
    if let Some(t) = trivial_sym {
        *t = if bit_entropy.nonzeros == 1 {
            bit_entropy.nonzero_code
        } else {
            NON_TRIVIAL_SYM
        };
    }
    *is_used = u8::from(stats.streaks[1][0] != 0 || stats.streaks[1][1] != 0);
    bits_entropy_refine(&bit_entropy) + final_huffman_cost(&stats)
}

/// Port of `GetCombinedEntropy`.
fn get_combined_entropy(
    x: &[u32],
    y: &[u32],
    length: usize,
    is_x_used: bool,
    is_y_used: bool,
    trivial_at_end: bool,
) -> f32 {
    if trivial_at_end {
        let mut stats = Streaks::default();
        stats.streaks[1][0] = 1;
        stats.counts[0] = 1;
        stats.streaks[0][1] = length as i32 - 1;
        return final_huffman_cost(&stats);
    }
    let (bit_entropy, stats) = if is_x_used {
        if is_y_used {
            get_combined_entropy_unrefined(x, y, length)
        } else {
            get_entropy_unrefined(x, length)
        }
    } else if is_y_used {
        get_entropy_unrefined(y, length)
    } else {
        let mut stats = Streaks::default();
        stats.counts[0] = 1;
        stats.streaks[0][usize::from(length > 3)] = length as i32;
        (BitEntropy::new(), stats)
    };
    bits_entropy_refine(&bit_entropy) + final_huffman_cost(&stats)
}

/// Port of `VP8LHistogramEstimateBits`.
#[must_use]
pub fn histogram_estimate_bits(p: &mut Histogram) -> f32 {
    let num_codes = histogram_num_codes(p.palette_code_bits);
    let mut used = p.is_used;
    let mut r = population_cost(&p.literal, num_codes, None, &mut used[0]);
    r += population_cost(&p.red, NUM_LITERAL_CODES, None, &mut used[1]);
    r += population_cost(&p.blue, NUM_LITERAL_CODES, None, &mut used[2]);
    r += population_cost(&p.alpha, NUM_LITERAL_CODES, None, &mut used[3]);
    r += population_cost(&p.distance, NUM_DISTANCE_CODES, None, &mut used[4]);
    r += extra_cost(&p.literal[NUM_LITERAL_CODES..], NUM_LENGTH_CODES) as f32;
    r += extra_cost(&p.distance, NUM_DISTANCE_CODES) as f32;
    p.is_used = used;
    r
}

/// Port of `GetCombinedHistogramEntropy`: adds the cost of `a + b` to `cost`, returning false as
/// soon as it exceeds `cost_threshold`.
fn get_combined_histogram_entropy(
    a: &Histogram,
    b: &Histogram,
    cost_threshold: f32,
    cost: &mut f32,
) -> bool {
    let palette_code_bits = a.palette_code_bits;
    let mut trivial_at_end = false;
    *cost += get_combined_entropy(
        &a.literal,
        &b.literal,
        histogram_num_codes(palette_code_bits),
        a.is_used[0] != 0,
        b.is_used[0] != 0,
        false,
    );
    *cost += extra_cost_combined(
        &a.literal[NUM_LITERAL_CODES..],
        &b.literal[NUM_LITERAL_CODES..],
        NUM_LENGTH_CODES,
    ) as f32;
    if *cost > cost_threshold {
        return false;
    }
    if a.trivial_symbol != NON_TRIVIAL_SYM && a.trivial_symbol == b.trivial_symbol {
        let color_a = (a.trivial_symbol >> 24) & 0xff;
        let color_r = (a.trivial_symbol >> 16) & 0xff;
        let color_b = a.trivial_symbol & 0xff;
        if (color_a == 0 || color_a == 0xff)
            && (color_r == 0 || color_r == 0xff)
            && (color_b == 0 || color_b == 0xff)
        {
            trivial_at_end = true;
        }
    }
    *cost += get_combined_entropy(
        &a.red,
        &b.red,
        NUM_LITERAL_CODES,
        a.is_used[1] != 0,
        b.is_used[1] != 0,
        trivial_at_end,
    );
    if *cost > cost_threshold {
        return false;
    }
    *cost += get_combined_entropy(
        &a.blue,
        &b.blue,
        NUM_LITERAL_CODES,
        a.is_used[2] != 0,
        b.is_used[2] != 0,
        trivial_at_end,
    );
    if *cost > cost_threshold {
        return false;
    }
    *cost += get_combined_entropy(
        &a.alpha,
        &b.alpha,
        NUM_LITERAL_CODES,
        a.is_used[3] != 0,
        b.is_used[3] != 0,
        trivial_at_end,
    );
    if *cost > cost_threshold {
        return false;
    }
    *cost += get_combined_entropy(
        &a.distance,
        &b.distance,
        NUM_DISTANCE_CODES,
        a.is_used[4] != 0,
        b.is_used[4] != 0,
        false,
    );
    *cost += extra_cost_combined(&a.distance, &b.distance, NUM_DISTANCE_CODES) as f32;
    if *cost > cost_threshold {
        return false;
    }
    true
}

/// Port of `VP8LHistogramAdd` for `b != out`: `out = a + b` with the used flags OR-ed, and the
/// C branches that copy an unused operand instead of adding it.
fn histogram_add_into(a: &Histogram, b: &Histogram, out: &mut Histogram) {
    let literal_size = histogram_num_codes(a.palette_code_bits);
    add_vector_into(
        a.is_used[0],
        &a.literal[..literal_size],
        b.is_used[0],
        &b.literal[..literal_size],
        &mut out.literal[..literal_size],
    );
    add_array_into(a.is_used[1], &a.red, b.is_used[1], &b.red, &mut out.red);
    add_array_into(a.is_used[2], &a.blue, b.is_used[2], &b.blue, &mut out.blue);
    add_array_into(
        a.is_used[3],
        &a.alpha,
        b.is_used[3],
        &b.alpha,
        &mut out.alpha,
    );
    add_array_into(
        a.is_used[4],
        &a.distance,
        b.is_used[4],
        &b.distance,
        &mut out.distance,
    );
    for i in 0..5 {
        out.is_used[i] = a.is_used[i] | b.is_used[i];
    }
}

/// Port of `VP8LHistogramAdd` for `b == out` (the `ADD_EQ` branch): `out += a`.
fn histogram_add_eq(a: &Histogram, out: &mut Histogram) {
    let literal_size = histogram_num_codes(a.palette_code_bits);
    add_eq_into(
        a.is_used[0],
        &a.literal[..literal_size],
        out.is_used[0],
        &mut out.literal[..literal_size],
    );
    add_eq_array(a.is_used[1], &a.red, &mut out.red, out.is_used[1]);
    add_eq_array(a.is_used[2], &a.blue, &mut out.blue, out.is_used[2]);
    add_eq_array(a.is_used[3], &a.alpha, &mut out.alpha, out.is_used[3]);
    add_eq_array(a.is_used[4], &a.distance, &mut out.distance, out.is_used[4]);
    for i in 0..5 {
        out.is_used[i] |= a.is_used[i];
    }
}

fn add_vector_into(a_used: u8, a: &[u32], b_used: u8, b: &[u32], out: &mut [u32]) {
    if a_used != 0 {
        if b_used != 0 {
            for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
                *o = x.wrapping_add(y);
            }
        } else {
            out.copy_from_slice(a);
        }
    } else if b_used != 0 {
        out.copy_from_slice(b);
    } else {
        out.fill(0);
    }
}

fn add_array_into<const N: usize>(
    a_used: u8,
    a: &[u32; N],
    b_used: u8,
    b: &[u32; N],
    out: &mut [u32; N],
) {
    add_vector_into(a_used, a, b_used, b, out);
}

fn add_eq_into(a_used: u8, a: &[u32], out_used: u8, out: &mut [u32]) {
    if a_used != 0 {
        if out_used != 0 {
            for (o, &x) in out.iter_mut().zip(a) {
                *o = o.wrapping_add(x);
            }
        } else {
            out.copy_from_slice(a);
        }
    }
}

fn add_eq_array<const N: usize>(a_used: u8, a: &[u32; N], out: &mut [u32; N], out_used: u8) {
    add_eq_into(a_used, a, out_used, out);
}

/// Port of `HistogramAdd` for `b != out`: the sums, then the trivial symbol.
fn histogram_add_full_into(a: &Histogram, b: &Histogram, out: &mut Histogram) {
    histogram_add_into(a, b, out);
    out.trivial_symbol = if a.trivial_symbol == b.trivial_symbol {
        a.trivial_symbol
    } else {
        NON_TRIVIAL_SYM
    };
}

/// Port of `HistogramAdd` for `b == out`.
fn histogram_add_full_eq(a: &Histogram, out: &mut Histogram) {
    let b_trivial = out.trivial_symbol;
    histogram_add_eq(a, out);
    out.trivial_symbol = if a.trivial_symbol == b_trivial {
        a.trivial_symbol
    } else {
        NON_TRIVIAL_SYM
    };
}

/// Port of `HistogramAddEval`: the cost change of merging `a` and `b` into `out` (when it is
/// below the threshold, `out` receives the merge).
fn histogram_add_eval(
    a: &Histogram,
    b: &Histogram,
    out: &mut Histogram,
    cost_threshold: f32,
) -> f32 {
    let mut cost: f32 = 0.0;
    let sum_cost = a.bit_cost + b.bit_cost;
    let cost_threshold = cost_threshold + sum_cost;
    if get_combined_histogram_entropy(a, b, cost_threshold, &mut cost) {
        histogram_add_full_into(a, b, out);
        out.bit_cost = cost;
        out.palette_code_bits = a.palette_code_bits;
    }
    cost - sum_cost
}

/// Port of `HistogramAddThresh`.
fn histogram_add_thresh(a: &Histogram, b: &Histogram, cost_threshold: f32) -> f32 {
    let mut cost = -a.bit_cost;
    get_combined_histogram_entropy(a, b, cost_threshold, &mut cost);
    cost
}

/// Port of `DominantCostRange`.
#[derive(Clone, Copy, Debug)]
struct DominantCostRange {
    literal_max: f32,
    literal_min: f32,
    red_max: f32,
    red_min: f32,
    blue_max: f32,
    blue_min: f32,
}

impl DominantCostRange {
    fn new() -> Self {
        Self {
            literal_max: 0.0,
            literal_min: MAX_BIT_COST,
            red_max: 0.0,
            red_min: MAX_BIT_COST,
            blue_max: 0.0,
            blue_min: MAX_BIT_COST,
        }
    }

    fn update(&mut self, h: &Histogram) {
        if self.literal_max < h.literal_cost {
            self.literal_max = h.literal_cost;
        }
        if self.literal_min > h.literal_cost {
            self.literal_min = h.literal_cost;
        }
        if self.red_max < h.red_cost {
            self.red_max = h.red_cost;
        }
        if self.red_min > h.red_cost {
            self.red_min = h.red_cost;
        }
        if self.blue_max < h.blue_cost {
            self.blue_max = h.blue_cost;
        }
        if self.blue_min > h.blue_cost {
            self.blue_min = h.blue_cost;
        }
    }
}

/// Port of `UpdateHistogramCost`: the cached costs and trivial symbol of `h`.
fn update_histogram_cost(h: &mut Histogram) {
    let mut alpha_sym: u32 = 0;
    let mut red_sym: u32 = 0;
    let mut blue_sym: u32 = 0;
    let mut used = h.is_used;
    let alpha_cost = population_cost(
        &h.alpha,
        NUM_LITERAL_CODES,
        Some(&mut alpha_sym),
        &mut used[3],
    );
    let distance_cost = population_cost(&h.distance, NUM_DISTANCE_CODES, None, &mut used[4])
        + extra_cost(&h.distance, NUM_DISTANCE_CODES) as f32;
    let num_codes = histogram_num_codes(h.palette_code_bits);
    h.literal_cost = population_cost(&h.literal, num_codes, None, &mut used[0])
        + extra_cost(&h.literal[NUM_LITERAL_CODES..], NUM_LENGTH_CODES) as f32;
    h.red_cost = population_cost(&h.red, NUM_LITERAL_CODES, Some(&mut red_sym), &mut used[1]);
    h.blue_cost = population_cost(
        &h.blue,
        NUM_LITERAL_CODES,
        Some(&mut blue_sym),
        &mut used[2],
    );
    h.is_used = used;
    h.bit_cost = h.literal_cost + h.red_cost + h.blue_cost + alpha_cost + distance_cost;
    if (alpha_sym | red_sym | blue_sym) == NON_TRIVIAL_SYM {
        h.trivial_symbol = NON_TRIVIAL_SYM;
    } else {
        h.trivial_symbol = (alpha_sym << 24) | (red_sym << 16) | blue_sym;
    }
}

/// Port of `GetBinIdForEntropy`. The C expression `(NUM_PARTITIONS - 1e-6) * delta / range` is
/// evaluated in `double`.
fn get_bin_id_for_entropy(min: f32, max: f32, val: f32) -> usize {
    let range = max - min;
    if range > 0.0 {
        let delta = val - min;
        ((NUM_PARTITIONS as f64 - 1e-6) * f64::from(delta) / f64::from(range)) as usize
    } else {
        0
    }
}

/// Port of `GetHistoBinIndex`.
fn get_histo_bin_index(h: &Histogram, c: &DominantCostRange, low_effort: bool) -> usize {
    let mut bin_id = get_bin_id_for_entropy(c.literal_min, c.literal_max, h.literal_cost);
    if !low_effort {
        bin_id = bin_id * NUM_PARTITIONS + get_bin_id_for_entropy(c.red_min, c.red_max, h.red_cost);
        bin_id =
            bin_id * NUM_PARTITIONS + get_bin_id_for_entropy(c.blue_min, c.blue_max, h.blue_cost);
    }
    bin_id
}

/// Port of `HistogramBuild`.
fn histogram_build(
    xsize: usize,
    histo_bits: i32,
    refs: &BackwardRefs,
    image_histo: &mut HistogramSet,
) {
    let mut x: usize = 0;
    let mut y: usize = 0;
    let histo_xsize = sub_sample_size(xsize, histo_bits);
    image_histo.clear();
    for v in &refs.refs {
        let ix = (y >> histo_bits) * histo_xsize + (x >> histo_bits);
        image_histo.histograms[ix]
            .as_mut()
            .expect("slot")
            .add_single_pix_or_copy(v);
        x += v.length() as usize;
        while x >= xsize {
            x -= xsize;
            y += 1;
        }
    }
}

/// Port of `VP8LSubSampleSize`.
fn sub_sample_size(size: usize, sampling_bits: i32) -> usize {
    (size + (1usize << sampling_bits) - 1) >> sampling_bits
}

/// Port of `HistogramCopyAndAnalyze`.
fn histogram_copy_and_analyze(
    orig_histo: &mut HistogramSet,
    image_histo: &mut HistogramSet,
    num_used: &mut i32,
    histogram_symbols: &mut [u16],
) {
    let mut num_used_orig = *num_used;
    let mut cluster_id: u16 = 0;
    for i in 0..orig_histo.max_size {
        let histo = orig_histo.histograms[i].as_mut().expect("orig slot");
        update_histogram_cost(histo);
        let h: Histogram = (**histo).clone();
        if h.is_used.iter().all(|&u| u == 0) {
            image_histo.remove(i, num_used);
            orig_histo.remove(i, &mut num_used_orig);
            histogram_symbols[i] = INVALID_HISTOGRAM_SYMBOL;
        } else {
            **image_histo.histograms[i].as_mut().expect("image slot") = h;
            histogram_symbols[i] = cluster_id;
            cluster_id += 1;
        }
    }
}

/// Port of `HistogramAnalyzeEntropyBin`.
fn histogram_analyze_entropy_bin(
    image_histo: &HistogramSet,
    bin_map: &mut [u16],
    low_effort: bool,
) {
    let histo_size = image_histo.size;
    let mut cost_range = DominantCostRange::new();
    for i in 0..histo_size {
        if let Some(h) = &image_histo.histograms[i] {
            cost_range.update(h);
        }
    }
    for i in 0..histo_size {
        if let Some(h) = &image_histo.histograms[i] {
            bin_map[i] = get_histo_bin_index(h, &cost_range, low_effort) as u16;
        }
    }
}

/// Port of `HistogramCombineEntropyBin`.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
fn histogram_combine_entropy_bin(
    image_histo: &mut HistogramSet,
    num_used: &mut i32,
    clusters: &[u16],
    cluster_mappings: &mut [u16],
    cur_combo: &mut Histogram,
    bin_map: &[u16],
    num_bins: usize,
    combine_cost_factor: f32,
    low_effort: bool,
) {
    // (first, num_combine_failures) per bin.
    let mut bin_info: Vec<(i32, u16)> = vec![(-1, 0); num_bins];
    for idx in 0..*num_used as usize {
        cluster_mappings[idx] = idx as u16;
    }
    for idx in 0..image_histo.size {
        if image_histo.histograms[idx].is_none() {
            continue;
        }
        let bin_id = bin_map[idx] as usize;
        let first = bin_info[bin_id].0;
        if first == -1 {
            bin_info[bin_id].0 = idx as i32;
        } else if low_effort {
            let first = first as usize;
            let a = image_histo.histograms[idx].take().expect("slot");
            histogram_add_full_eq(&a, image_histo.histograms[first].as_mut().expect("first"));
            image_histo.histograms[idx] = Some(a);
            image_histo.remove(idx, num_used);
            cluster_mappings[clusters[idx] as usize] = clusters[first as usize];
        } else {
            let first = first as usize;
            let bit_cost = image_histo.get(idx).bit_cost;
            let bit_cost_thresh = -bit_cost * combine_cost_factor;
            let curr_cost_diff = histogram_add_eval(
                image_histo.get(first),
                image_histo.get(idx),
                cur_combo,
                bit_cost_thresh,
            );
            if curr_cost_diff < bit_cost_thresh {
                let try_combine = cur_combo.trivial_symbol != NON_TRIVIAL_SYM
                    || (image_histo.get(idx).trivial_symbol == NON_TRIVIAL_SYM
                        && image_histo.get(first).trivial_symbol == NON_TRIVIAL_SYM);
                let max_combine_failures = 32;
                if try_combine || i32::from(bin_info[bin_id].1) >= max_combine_failures {
                    // HistogramSwap(&cur_combo, &histograms[first]).
                    let slot = image_histo.histograms[first].as_mut().expect("first");
                    std::mem::swap(&mut **slot, cur_combo);
                    image_histo.remove(idx, num_used);
                    cluster_mappings[clusters[idx] as usize] = clusters[first];
                } else {
                    bin_info[bin_id].1 += 1;
                }
            }
        }
    }
    if low_effort {
        for idx in 0..image_histo.size {
            if let Some(h) = image_histo.histograms[idx].as_mut() {
                update_histogram_cost(h);
            }
        }
    }
}

/// Port of `MyRand`: the Park-Miller generator used by the stochastic combiner.
fn my_rand(seed: &mut u32) -> u32 {
    *seed = ((u64::from(*seed) * 48271) % 2_147_483_647) as u32;
    *seed
}

/// Port of `HistogramPair`.
#[derive(Clone, Copy, Debug, Default)]
struct HistogramPair {
    idx1: i32,
    idx2: i32,
    cost_diff: f32,
    cost_combo: f32,
}

/// Port of `HistoQueue`: a bounded queue of the cheapest merges, kept with its best pair at [0].
struct HistoQueue {
    queue: Vec<HistogramPair>,
    max_size: usize,
}

impl HistoQueue {
    fn new(max_size: usize) -> Self {
        Self {
            queue: Vec::with_capacity(max_size + 1),
            max_size,
        }
    }

    /// Port of `HistoQueuePopPair`: removes entry `i` by moving the last entry into its place.
    fn pop_pair(&mut self, i: usize) {
        let last = *self.queue.last().expect("non-empty queue");
        self.queue[i] = last;
        self.queue.pop();
    }

    /// Port of `HistoQueueUpdateHead`: if entry `i` beats the head, swap them.
    fn update_head(&mut self, i: usize) {
        if self.queue[i].cost_diff < self.queue[0].cost_diff {
            self.queue.swap(0, i);
        }
    }
}

/// Port of `HistoQueueUpdatePair`.
fn histo_queue_update_pair(
    h1: &Histogram,
    h2: &Histogram,
    threshold: f32,
    pair: &mut HistogramPair,
) {
    let sum_cost = h1.bit_cost + h2.bit_cost;
    pair.cost_combo = 0.0;
    get_combined_histogram_entropy(h1, h2, sum_cost + threshold, &mut pair.cost_combo);
    pair.cost_diff = pair.cost_combo - sum_cost;
}

/// Port of `HistoQueuePush`: adds the merge of `idx1` and `idx2` if it beats `threshold`.
fn histo_queue_push(
    q: &mut HistoQueue,
    histograms: &[Option<Box<Histogram>>],
    idx1: i32,
    idx2: i32,
    threshold: f32,
) -> f32 {
    if q.queue.len() == q.max_size {
        return 0.0;
    }
    let (idx1, idx2) = if idx1 > idx2 {
        (idx2, idx1)
    } else {
        (idx1, idx2)
    };
    let mut pair = HistogramPair {
        idx1,
        idx2,
        cost_diff: 0.0,
        cost_combo: 0.0,
    };
    let h1 = histograms[idx1 as usize].as_deref().expect("h1");
    let h2 = histograms[idx2 as usize].as_deref().expect("h2");
    histo_queue_update_pair(h1, h2, threshold, &mut pair);
    if pair.cost_diff >= threshold {
        return 0.0;
    }
    q.queue.push(pair);
    let last = q.queue.len() - 1;
    q.update_head(last);
    pair.cost_diff
}

/// Port of `HistogramCombineGreedy`.
fn histogram_combine_greedy(image_histo: &mut HistogramSet, num_used: &mut i32) {
    let image_histo_size = image_histo.size;
    let mut queue = HistoQueue::new(image_histo_size * image_histo_size);
    for i in 0..image_histo_size {
        if image_histo.histograms[i].is_none() {
            continue;
        }
        for j in i + 1..image_histo_size {
            if image_histo.histograms[j].is_none() {
                continue;
            }
            histo_queue_push(&mut queue, &image_histo.histograms, i as i32, j as i32, 0.0);
        }
    }
    while !queue.queue.is_empty() {
        let idx1 = queue.queue[0].idx1;
        let idx2 = queue.queue[0].idx2;
        let a = image_histo.histograms[idx2 as usize].take().expect("idx2");
        histogram_add_full_eq(
            &a,
            image_histo.histograms[idx1 as usize]
                .as_mut()
                .expect("idx1"),
        );
        image_histo.histograms[idx2 as usize] = Some(a);
        image_histo.histograms[idx1 as usize]
            .as_mut()
            .expect("idx1")
            .bit_cost = queue.queue[0].cost_combo;
        image_histo.remove(idx2 as usize, num_used);
        let mut i = 0;
        while i < queue.queue.len() {
            let p = queue.queue[i];
            if p.idx1 == idx1 || p.idx2 == idx1 || p.idx1 == idx2 || p.idx2 == idx2 {
                queue.pop_pair(i);
            } else {
                queue.update_head(i);
                i += 1;
            }
        }
        for i in 0..image_histo.size {
            if i as i32 == idx1 || image_histo.histograms[i].is_none() {
                continue;
            }
            histo_queue_push(&mut queue, &image_histo.histograms, idx1, i as i32, 0.0);
        }
    }
}

/// Port of `HistogramCombineStochastic`. Returns `do_greedy`.
fn histogram_combine_stochastic(
    image_histo: &mut HistogramSet,
    num_used: &mut i32,
    min_cluster_size: i32,
) -> bool {
    let mut seed: u32 = 1;
    let mut tries_with_no_success: i32 = 0;
    let outer_iters = *num_used;
    let num_tries_no_success = outer_iters / 2;
    const K_HISTO_QUEUE_SIZE: usize = 9;
    if *num_used < min_cluster_size {
        return true;
    }
    let mut mappings: Vec<i32> = Vec::with_capacity(*num_used as usize);
    let mut queue = HistoQueue::new(K_HISTO_QUEUE_SIZE);
    for iter in 0..image_histo.size {
        if image_histo.histograms[iter].is_some() {
            mappings.push(iter as i32);
        }
    }
    let mut iter: i32 = 0;
    loop {
        if !(iter < outer_iters && *num_used >= min_cluster_size && {
            tries_with_no_success += 1;
            tries_with_no_success < num_tries_no_success
        }) {
            break;
        }
        let mut best_cost = if queue.queue.is_empty() {
            0.0
        } else {
            queue.queue[0].cost_diff
        };
        let rand_range = ((*num_used - 1) as u32).wrapping_mul(*num_used as u32);
        let num_tries = *num_used / 2;
        let mut j = 0;
        while *num_used >= 2 && j < num_tries {
            let tmp = my_rand(&mut seed) % rand_range;
            let idx1 = tmp / (*num_used - 1) as u32;
            let mut idx2 = tmp % (*num_used - 1) as u32;
            if idx2 >= idx1 {
                idx2 += 1;
            }
            let idx1 = mappings[idx1 as usize];
            let idx2 = mappings[idx2 as usize];
            let curr_cost =
                histo_queue_push(&mut queue, &image_histo.histograms, idx1, idx2, best_cost);
            if curr_cost < 0.0 {
                best_cost = curr_cost;
                if queue.queue.len() == queue.max_size {
                    break;
                }
            }
            j += 1;
        }
        if !queue.queue.is_empty() {
            let best_idx1 = queue.queue[0].idx1;
            let best_idx2 = queue.queue[0].idx2;
            let pos = mappings[..*num_used as usize]
                .binary_search(&best_idx2)
                .expect("best_idx2 is mapped");
            mappings.remove(pos);
            let a = image_histo.histograms[best_idx2 as usize]
                .take()
                .expect("best_idx2");
            histogram_add_full_eq(
                &a,
                image_histo.histograms[best_idx1 as usize]
                    .as_mut()
                    .expect("best_idx1"),
            );
            image_histo.histograms[best_idx2 as usize] = Some(a);
            image_histo.histograms[best_idx1 as usize]
                .as_mut()
                .expect("best_idx1")
                .bit_cost = queue.queue[0].cost_combo;
            image_histo.remove(best_idx2 as usize, num_used);
            let mut j = 0;
            while j < queue.queue.len() {
                let p = queue.queue[j];
                let is_idx1_best = p.idx1 == best_idx1 || p.idx1 == best_idx2;
                let is_idx2_best = p.idx2 == best_idx1 || p.idx2 == best_idx2;
                let mut do_eval = false;
                if is_idx1_best && is_idx2_best {
                    queue.pop_pair(j);
                    continue;
                }
                let mut p = p;
                if is_idx1_best {
                    p.idx1 = best_idx1;
                    do_eval = true;
                } else if is_idx2_best {
                    p.idx2 = best_idx1;
                    do_eval = true;
                }
                if p.idx1 > p.idx2 {
                    std::mem::swap(&mut p.idx1, &mut p.idx2);
                }
                if do_eval {
                    histo_queue_update_pair(
                        image_histo.get(p.idx1 as usize),
                        image_histo.get(p.idx2 as usize),
                        0.0,
                        &mut p,
                    );
                    if p.cost_diff >= 0.0 {
                        queue.pop_pair(j);
                        continue;
                    }
                }
                queue.queue[j] = p;
                queue.update_head(j);
                j += 1;
            }
            tries_with_no_success = 0;
        }
        iter += 1;
    }
    let do_greedy = *num_used <= min_cluster_size;
    do_greedy
}

/// Port of `HistogramRemap`: assigns each image histogram to its best cluster, then rebuilds
/// `out` from those assignments.
fn histogram_remap(input: &HistogramSet, out: &mut HistogramSet, symbols: &mut [u16]) {
    let in_size = out.max_size;
    let out_size = out.size;
    if out_size > 1 {
        for i in 0..in_size {
            let mut best_out: usize = 0;
            let mut best_bits = MAX_BIT_COST;
            let Some(in_h) = input.histograms[i].as_deref() else {
                symbols[i] = symbols[i - 1];
                continue;
            };
            for k in 0..out_size {
                let out_h = out.histograms[k].as_deref().expect("out slot");
                let cur_bits = histogram_add_thresh(out_h, in_h, best_bits);
                if k == 0 || cur_bits < best_bits {
                    best_bits = cur_bits;
                    best_out = k;
                }
            }
            symbols[i] = best_out as u16;
        }
    } else {
        for s in symbols.iter_mut().take(in_size) {
            *s = 0;
        }
    }
    out.clear();
    out.size = out_size;
    for i in 0..in_size {
        let Some(in_h) = input.histograms[i].as_deref() else {
            continue;
        };
        let idx = symbols[i] as usize;
        let slot = out.histograms[idx].as_mut().expect("out slot");
        histogram_add_full_eq(in_h, slot);
    }
}

/// Port of `GetCombineCostFactor`.
fn get_combine_cost_factor(histo_size: i32, quality: i32) -> f32 {
    let mut combine_cost_factor: f32 = 0.16;
    if quality < 90 {
        if histo_size > 256 {
            combine_cost_factor /= 2.0;
        }
        if histo_size > 512 {
            combine_cost_factor /= 2.0;
        }
        if histo_size > 1024 {
            combine_cost_factor /= 2.0;
        }
        if quality <= 50 {
            combine_cost_factor /= 2.0;
        }
    }
    combine_cost_factor
}

/// Port of `OptimizeHistogramSymbols`.
fn optimize_histogram_symbols(
    set: &HistogramSet,
    cluster_mappings: &mut [u16],
    num_clusters: usize,
    cluster_mappings_tmp: &mut [u16],
    symbols: &mut [u16],
) {
    let mut do_continue = true;
    while do_continue {
        do_continue = false;
        for i in 0..num_clusters {
            let mut k = cluster_mappings[i] as usize;
            while k != cluster_mappings[k] as usize {
                cluster_mappings[k] = cluster_mappings[cluster_mappings[k] as usize];
                k = cluster_mappings[k] as usize;
            }
            if k != cluster_mappings[i] as usize {
                do_continue = true;
                cluster_mappings[i] = k as u16;
            }
        }
    }
    let mut cluster_max: u16 = 0;
    for v in cluster_mappings_tmp.iter_mut().take(set.max_size) {
        *v = 0;
    }
    for i in 0..set.max_size {
        if symbols[i] == INVALID_HISTOGRAM_SYMBOL {
            continue;
        }
        let cluster = cluster_mappings[symbols[i] as usize] as usize;
        if cluster > 0 && cluster_mappings_tmp[cluster] == 0 {
            cluster_max += 1;
            cluster_mappings_tmp[cluster] = cluster_max;
        }
        symbols[i] = cluster_mappings_tmp[cluster];
    }
}

/// Port of `VP8LGetHistoImageSymbols`: clusters the histograms of the `histogram_bits` tiles
/// into `image_histo` and writes each tile's cluster index into `histogram_symbols`.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
pub fn get_histo_image_symbols(
    xsize: usize,
    ysize: usize,
    refs: &BackwardRefs,
    quality: i32,
    low_effort: bool,
    histogram_bits: i32,
    cache_bits: i32,
    image_histo: &mut HistogramSet,
    tmp_histo: &mut Histogram,
    histogram_symbols: &mut [u16],
) {
    let histo_xsize = if histogram_bits != 0 {
        sub_sample_size(xsize, histogram_bits)
    } else {
        1
    };
    let histo_ysize = if histogram_bits != 0 {
        sub_sample_size(ysize, histogram_bits)
    } else {
        1
    };
    let image_histo_raw_size = histo_xsize * histo_ysize;
    let mut orig_histo = HistogramSet::new(image_histo_raw_size, cache_bits);
    let entropy_combine_num_bins = if low_effort { NUM_PARTITIONS } else { BIN_SIZE };
    let mut bin_map = vec![0u16; image_histo_raw_size];
    let mut cluster_mappings = vec![0u16; image_histo_raw_size];
    let mut num_used = image_histo_raw_size as i32;
    histogram_build(xsize, histogram_bits, refs, &mut orig_histo);
    histogram_copy_and_analyze(
        &mut orig_histo,
        image_histo,
        &mut num_used,
        histogram_symbols,
    );
    let entropy_combine = (num_used as usize > entropy_combine_num_bins * 2) && quality < 100;
    if entropy_combine {
        let num_clusters = num_used as usize;
        histogram_analyze_entropy_bin(image_histo, &mut bin_map, low_effort);
        let combine_cost_factor = get_combine_cost_factor(image_histo_raw_size as i32, quality);
        histogram_combine_entropy_bin(
            image_histo,
            &mut num_used,
            histogram_symbols,
            &mut cluster_mappings,
            tmp_histo,
            &bin_map,
            entropy_combine_num_bins,
            combine_cost_factor,
            low_effort,
        );
        let mut tmp = vec![0u16; image_histo_raw_size];
        optimize_histogram_symbols(
            image_histo,
            &mut cluster_mappings,
            num_clusters,
            &mut tmp,
            histogram_symbols,
        );
    }
    if !low_effort || !entropy_combine {
        let x = quality as f32 / 100.0;
        let threshold_size = (1.0 + (x * x * x) * (MAX_HISTO_GREEDY - 1) as f32) as i32;
        let do_greedy = histogram_combine_stochastic(image_histo, &mut num_used, threshold_size);
        if do_greedy {
            image_histo.remove_empty();
            histogram_combine_greedy(image_histo, &mut num_used);
        }
    }
    image_histo.remove_empty();
    histogram_remap(&orig_histo, image_histo, histogram_symbols);
}
