// Copyright 2015 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/enc/backward_references_cost_enc.c` for the colour-cache-free case: the
//! cost model built from the first parse, the lowest-cost path over the hash chain (the
//! interval manager), and the trace back that turns it into backward references.
//!
//! The C interval list is pointer-linked, and its free and recycled lists are LIFO. The port
//! keeps the same structure in an index arena and reuses slots in the same order, so the
//! sequence of operations (including the reuse of a just-popped interval) is the C sequence.

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

use super::backward_refs::{
    BackwardRefs, HashChain, MAX_LENGTH, PixOrCopy, distance_to_plane_code,
};
use super::entropy::fast_log2;
use super::histogram::{Histogram, NUM_DISTANCE_CODES, histogram_num_codes};
use super::prefix::prefix_encode_bits;

/// Port of `VALUES_IN_BYTE`.
const VALUES_IN_BYTE: usize = 256;
/// Port of `COST_CACHE_INTERVAL_SIZE_MAX`.
const COST_CACHE_INTERVAL_SIZE_MAX: i32 = 500;
/// Port of `COST_MANAGER_MAX_FREE_LIST`: the number of statically allocated intervals.
const COST_MANAGER_MAX_FREE_LIST: usize = 10;
/// Port of `FLT_MAX`.
const FLT_MAX: f32 = f32::MAX;

/// Port of `CostModel` (the literal alphabet has no colour-cache codes here).
struct CostModel {
    alpha: [f32; VALUES_IN_BYTE],
    red: [f32; VALUES_IN_BYTE],
    blue: [f32; VALUES_IN_BYTE],
    distance: [f32; NUM_DISTANCE_CODES],
    literal: Vec<f32>,
}

/// Port of `ConvertPopulationCountTableToBitEstimates`.
fn convert_population_count_table_to_bit_estimates(counts: &[u32], output: &mut [f32]) {
    let num_symbols = counts.len();
    let mut sum: u32 = 0;
    let mut nonzeros = 0;
    for &c in counts {
        sum = sum.wrapping_add(c);
        if c > 0 {
            nonzeros += 1;
        }
    }
    if nonzeros <= 1 {
        for o in output.iter_mut().take(num_symbols) {
            *o = 0.0;
        }
    } else {
        let logsum = fast_log2(sum);
        for i in 0..num_symbols {
            output[i] = logsum - fast_log2(counts[i]);
        }
    }
}

/// Port of `CostModelBuild`: the bit estimates of each alphabet, from the parse `refs`
/// (copy distances as 2D plane codes).
fn cost_model_build(xsize: i32, histo_cache_bits: i32, refs: &BackwardRefs) -> CostModel {
    let mut histo = Histogram::new(histo_cache_bits);
    let modifier = |dist: u32| distance_to_plane_code(xsize, dist as i32) as u32;
    for v in &refs.refs {
        histo.add_single_pix_or_copy_with(v, Some(&modifier));
    }
    let mut m = CostModel {
        alpha: [0.0; VALUES_IN_BYTE],
        red: [0.0; VALUES_IN_BYTE],
        blue: [0.0; VALUES_IN_BYTE],
        distance: [0.0; NUM_DISTANCE_CODES],
        literal: vec![0.0; histogram_num_codes(histo_cache_bits)],
    };
    let literal_len = histogram_num_codes(histo.palette_code_bits);
    convert_population_count_table_to_bit_estimates(&histo.literal[..literal_len], &mut m.literal);
    convert_population_count_table_to_bit_estimates(&histo.red, &mut m.red);
    convert_population_count_table_to_bit_estimates(&histo.blue, &mut m.blue);
    convert_population_count_table_to_bit_estimates(&histo.alpha, &mut m.alpha);
    convert_population_count_table_to_bit_estimates(&histo.distance, &mut m.distance);
    m
}

/// Port of `GetLiteralCost`.
fn get_literal_cost(m: &CostModel, v: u32) -> f32 {
    m.alpha[(v >> 24) as usize]
        + m.red[((v >> 16) & 0xff) as usize]
        + m.literal[((v >> 8) & 0xff) as usize]
        + m.blue[(v & 0xff) as usize]
}

/// Port of `GetLengthCost`.
fn get_length_cost(m: &CostModel, length: u32) -> f32 {
    let (code, extra_bits) = prefix_encode_bits(length as i32);
    m.literal[VALUES_IN_BYTE + code as usize] + extra_bits as f32
}

/// Port of `GetDistanceCost`.
fn get_distance_cost(m: &CostModel, distance: u32) -> f32 {
    let (code, extra_bits) = prefix_encode_bits(distance as i32);
    m.distance[code as usize] + extra_bits as f32
}

/// Port of `CostCacheInterval`.
#[derive(Clone, Copy)]
struct CostCacheInterval {
    cost: f32,
    start: i32,
    end: i32,
}

/// Port of `CostInterval`, stored in the manager's arena.
#[derive(Clone, Copy, Default)]
struct CostInterval {
    cost: f32,
    start: i32,
    end: i32,
    index: i32,
    previous: Option<usize>,
    next: Option<usize>,
}

/// Port of `CostManager`.
struct CostManager {
    /// Arena of intervals; the first `COST_MANAGER_MAX_FREE_LIST` are the static ones.
    intervals: Vec<CostInterval>,
    /// Free list (LIFO; the top is the last element).
    free: Vec<usize>,
    /// Recycled list (LIFO).
    recycled: Vec<usize>,
    head: Option<usize>,
    count: i32,
    cache_intervals: Vec<CostCacheInterval>,
    cost_cache: Vec<f32>,
    costs: Vec<f32>,
    dist_array: Vec<u16>,
}

impl CostManager {
    /// Port of `CostManagerInit`.
    fn new(dist_array: Vec<u16>, pix_count: usize, cost_model: &CostModel) -> Self {
        let cost_cache_size = pix_count.min(MAX_LENGTH as usize);
        let mut cost_cache = vec![0.0f32; MAX_LENGTH as usize];
        for (i, c) in cost_cache.iter_mut().enumerate().take(cost_cache_size) {
            *c = get_length_cost(cost_model, i as u32);
        }
        let mut cache_intervals = vec![CostCacheInterval {
            cost: cost_cache[0],
            start: 0,
            end: 1,
        }];
        for i in 1..cost_cache_size {
            let cost_val = cost_cache[i];
            if cost_val != cache_intervals.last().map_or(0.0, |c| c.cost) {
                cache_intervals.push(CostCacheInterval {
                    cost: cost_val,
                    start: i as i32,
                    end: 0,
                });
            }
            if let Some(last) = cache_intervals.last_mut() {
                last.end = i as i32 + 1;
            }
        }
        let intervals = vec![CostInterval::default(); COST_MANAGER_MAX_FREE_LIST];
        // CostManagerInitFreeList: every static interval is pushed in order.
        let free: Vec<usize> = (0..COST_MANAGER_MAX_FREE_LIST).collect();
        Self {
            intervals,
            free,
            recycled: Vec::new(),
            head: None,
            count: 0,
            cache_intervals,
            cost_cache,
            costs: vec![FLT_MAX; pix_count],
            dist_array,
        }
    }

    fn is_static(idx: usize) -> bool {
        idx < COST_MANAGER_MAX_FREE_LIST
    }

    /// Port of `UpdateCost`.
    fn update_cost(&mut self, i: usize, position: usize, cost: f32) {
        let k = i - position;
        if self.costs[i] > cost {
            self.costs[i] = cost;
            self.dist_array[i] = (k + 1) as u16;
        }
    }

    /// Port of `UpdateCostPerInterval`.
    fn update_cost_per_interval(&mut self, start: i32, end: i32, position: usize, cost: f32) {
        for i in start..end {
            self.update_cost(i as usize, position, cost);
        }
    }

    /// Port of `ConnectIntervals`.
    fn connect(&mut self, prev: Option<usize>, next: Option<usize>) {
        match prev {
            Some(p) => self.intervals[p].next = next,
            None => self.head = next,
        }
        if let Some(n) = next {
            self.intervals[n].previous = prev;
        }
    }

    /// Port of `PopInterval`.
    fn pop_interval(&mut self, interval: usize) {
        let (prev, next) = (
            self.intervals[interval].previous,
            self.intervals[interval].next,
        );
        self.connect(prev, next);
        if Self::is_static(interval) {
            self.free.push(interval);
        } else {
            self.recycled.push(interval);
        }
        self.count -= 1;
    }

    /// Port of `UpdateCostAtIndex`.
    fn update_cost_at_index(&mut self, i: usize, do_clean_intervals: bool) {
        let mut current = self.head;
        while let Some(c) = current {
            if self.intervals[c].start as usize > i {
                break;
            }
            let next = self.intervals[c].next;
            if self.intervals[c].end as usize <= i {
                if do_clean_intervals {
                    self.pop_interval(c);
                }
            } else {
                let (index, cost) = (self.intervals[c].index as usize, self.intervals[c].cost);
                self.update_cost(i, index, cost);
            }
            current = next;
        }
    }

    /// Port of `PositionOrphanInterval`.
    fn position_orphan_interval(&mut self, current: usize, previous_in: Option<usize>) {
        let mut previous = match previous_in {
            Some(p) => Some(p),
            None => self.head,
        };
        let cur_start = self.intervals[current].start;
        while let Some(p) = previous {
            if cur_start >= self.intervals[p].start {
                break;
            }
            previous = self.intervals[p].previous;
        }
        while let Some(p) = previous {
            match self.intervals[p].next {
                Some(n) if self.intervals[n].start < cur_start => previous = Some(n),
                _ => break,
            }
        }
        if let Some(p) = previous {
            let next = self.intervals[p].next;
            self.connect(Some(current), next);
        } else {
            let head = self.head;
            self.connect(Some(current), head);
        }
        self.connect(previous, Some(current));
    }

    /// Port of `InsertInterval`.
    fn insert_interval(
        &mut self,
        interval_in: Option<usize>,
        cost: f32,
        position: usize,
        start: i32,
        end: i32,
    ) {
        if start >= end {
            return;
        }
        if self.count >= COST_CACHE_INTERVAL_SIZE_MAX {
            self.update_cost_per_interval(start, end, position, cost);
            return;
        }
        let new_idx = if let Some(i) = self.free.pop() {
            i
        } else if let Some(i) = self.recycled.pop() {
            i
        } else {
            self.intervals.push(CostInterval::default());
            self.intervals.len() - 1
        };
        self.intervals[new_idx].cost = cost;
        self.intervals[new_idx].index = position as i32;
        self.intervals[new_idx].start = start;
        self.intervals[new_idx].end = end;
        self.position_orphan_interval(new_idx, interval_in);
        self.count += 1;
    }

    /// Port of `PushInterval`.
    fn push_interval(&mut self, distance_cost: f32, position: usize, len: usize) {
        const K_SKIP_DISTANCE: usize = 10;
        if len < K_SKIP_DISTANCE {
            for j in position..position + len {
                let k = j - position;
                let cost_tmp = distance_cost + self.cost_cache[k];
                if self.costs[j] > cost_tmp {
                    self.costs[j] = cost_tmp;
                    self.dist_array[j] = (k + 1) as u16;
                }
            }
            return;
        }
        let mut interval: Option<usize> = self.head;
        let mut i = 0;
        while i < self.cache_intervals.len() && (self.cache_intervals[i].start as usize) < len {
            let ci = self.cache_intervals[i];
            let mut start = (position as i32) + ci.start;
            let end = (position as i32)
                + if ci.end as usize > len {
                    len as i32
                } else {
                    ci.end
                };
            let cost = distance_cost + ci.cost;
            loop {
                let Some(iv) = interval else { break };
                if self.intervals[iv].start >= end {
                    break;
                }
                let interval_next = self.intervals[iv].next;
                if start >= self.intervals[iv].end {
                    interval = interval_next;
                    continue;
                }
                if cost >= self.intervals[iv].cost {
                    let start_new = self.intervals[iv].end;
                    let iv_start = self.intervals[iv].start;
                    self.insert_interval(Some(iv), cost, position, start, iv_start);
                    start = start_new;
                    if start >= end {
                        break;
                    }
                    interval = interval_next;
                    continue;
                }
                if start <= self.intervals[iv].start {
                    if self.intervals[iv].end <= end {
                        self.pop_interval(iv);
                    } else {
                        self.intervals[iv].start = end;
                        break;
                    }
                } else if end < self.intervals[iv].end {
                    let end_original = self.intervals[iv].end;
                    self.intervals[iv].end = start;
                    let (c, idx) = (self.intervals[iv].cost, self.intervals[iv].index as usize);
                    self.insert_interval(Some(iv), c, idx, end, end_original);
                    interval = self.intervals[iv].next;
                    break;
                } else {
                    self.intervals[iv].end = start;
                }
                interval = interval_next;
            }
            self.insert_interval(interval, cost, position, start, end);
            i += 1;
        }
    }
}

/// Port of `AddSingleLiteralWithCostModel` (no colour cache).
fn add_single_literal_with_cost_model(
    argb: &[u32],
    cost_model: &CostModel,
    idx: usize,
    prev_cost: f32,
    cost: &mut [f32],
    dist_array: &mut [u16],
) {
    let color = argb[idx];
    let mul1: f32 = 0.82;
    let cost_val = prev_cost + get_literal_cost(cost_model, color) * mul1;
    if cost[idx] > cost_val {
        cost[idx] = cost_val;
        dist_array[idx] = 1; // only one is inserted.
    }
}

/// Port of `BackwardReferencesHashChainDistanceOnly`: the distance chosen for each position.
fn backward_references_hash_chain_distance_only(
    xsize: usize,
    ysize: usize,
    argb: &[u32],
    hash_chain: &HashChain,
    refs: &BackwardRefs,
) -> Vec<u16> {
    let pix_count = xsize * ysize;
    let cost_model = cost_model_build(xsize as i32, 0, refs);
    let mut mgr = CostManager::new(vec![0u16; pix_count], pix_count, &cost_model);
    mgr.dist_array[0] = 0;
    {
        let (costs, dist) = (&mut mgr.costs, &mut mgr.dist_array);
        add_single_literal_with_cost_model(argb, &cost_model, 0, 0.0, costs, dist);
    }
    let mut offset_prev: i32 = -1;
    let mut len_prev: i32 = -1;
    let mut offset_cost: f32 = -1.0;
    let mut first_offset_is_constant: i32 = -1;
    let mut reach: usize = 0;
    let find_copy = |p: usize| -> (i32, i32) {
        if p < pix_count {
            hash_chain.find_copy(p)
        } else {
            (0, 0)
        }
    };
    for i in 1..pix_count {
        let prev_cost = mgr.costs[i - 1];
        let (offset, len) = find_copy(i);
        {
            let (costs, dist) = (&mut mgr.costs, &mut mgr.dist_array);
            add_single_literal_with_cost_model(argb, &cost_model, i, prev_cost, costs, dist);
        }
        if len >= 2 {
            if offset != offset_prev {
                let code = distance_to_plane_code(xsize as i32, offset);
                offset_cost = get_distance_cost(&cost_model, code as u32);
                first_offset_is_constant = 1;
                mgr.push_interval(prev_cost + offset_cost, i, len as usize);
            } else {
                if first_offset_is_constant == 1 {
                    reach = (i as i64 - 1 + len_prev as i64 - 1).max(0) as usize;
                    first_offset_is_constant = 0;
                }
                if i as i64 + len as i64 - 1 > reach as i64 {
                    let mut j = i;
                    let mut len_j: i32 = 0;
                    while j <= reach {
                        let (offset_j, l) = find_copy(j + 1);
                        len_j = l;
                        if offset_j != offset {
                            let (_, l2) = find_copy(j);
                            len_j = l2;
                            break;
                        }
                        j += 1;
                    }
                    mgr.update_cost_at_index(j - 1, false);
                    mgr.update_cost_at_index(j, false);
                    let base = mgr.costs[j - 1] + offset_cost;
                    mgr.push_interval(base, j, len_j as usize);
                    reach = (j as i64 + len_j as i64 - 1).max(0) as usize;
                }
            }
        }
        mgr.update_cost_at_index(i, true);
        offset_prev = offset;
        len_prev = len;
    }
    mgr.dist_array
}

/// Port of `TraceBackwards`: the chosen path, as the lengths of its segments.
fn trace_backwards(dist_array: &[u16]) -> Vec<u16> {
    let mut path: Vec<u16> = Vec::new();
    let mut cur: isize = dist_array.len() as isize - 1;
    while cur >= 0 {
        let k = dist_array[cur as usize];
        path.push(k);
        cur -= k as isize;
    }
    path.reverse();
    path
}

/// Port of `BackwardReferencesHashChainFollowChosenPath` (no colour cache).
fn backward_references_hash_chain_follow_chosen_path(
    argb: &[u32],
    chosen_path: &[u16],
    hash_chain: &HashChain,
    refs: &mut BackwardRefs,
) {
    refs.clear();
    let mut i: usize = 0;
    for &len in chosen_path {
        if len != 1 {
            let offset = hash_chain.find_offset(i);
            refs.push(PixOrCopy::copy(offset as u32, u32::from(len)));
            i += len as usize;
        } else {
            refs.push(PixOrCopy::literal(argb[i]));
            i += 1;
        }
    }
}

/// Port of `VP8LBackwardReferencesTraceBackwards` for `cache_bits == 0`: the parse of the
/// cheapest path through the hash chain under the cost model of `refs_src`.
#[must_use]
pub fn backward_references_trace_backwards(
    xsize: usize,
    ysize: usize,
    argb: &[u32],
    hash_chain: &HashChain,
    refs_src: &BackwardRefs,
) -> BackwardRefs {
    let dist_array =
        backward_references_hash_chain_distance_only(xsize, ysize, argb, hash_chain, refs_src);
    let chosen_path = trace_backwards(&dist_array);
    let mut refs_dst = BackwardRefs::default();
    backward_references_hash_chain_follow_chosen_path(
        argb,
        &chosen_path,
        hash_chain,
        &mut refs_dst,
    );
    refs_dst
}
