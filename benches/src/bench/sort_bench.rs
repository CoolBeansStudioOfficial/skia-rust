// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/SortBench.cpp

//! Sorting 1000 ints from five input shapes with Skia's quicksort and heapsort, and the Rust
//! standard library's sort (non-rendering benches).
//!
//! Not ported: the `qsort` (libc) algorithm. The standard library has no `qsort`, and calling
//! libc's needs FFI outside `skia-rust-simd`, which the crate rules forbid; its five
//! `NewQSort(...)` entries stay `todo` with that reason in the manifest.

// The int casts mirror the C++ `int` arithmetic of the sort inputs (`rand() % mod`, `-i`).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::random::Random;
use skia_rust_core::t_sort::{t_heap_sort, t_q_sort};

use crate::def_bench;
use crate::prelude::*;

const N: usize = 1000;

/// `Type`: the input shapes.
#[derive(Clone, Copy)]
enum Type {
    Rand,
    RandN,
    Fore,
    Back,
    Same,
}

impl Type {
    /// `gRec[t].fName`.
    // Port of: bench/SortBench.cpp#L56-L62 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Type::Rand => "rand",
            Type::RandN => "rand10",
            Type::Fore => "forward",
            Type::Back => "backward",
            Type::Same => "repeated",
        }
    }

    /// `gRec[t].fProc(array)`.
    fn fill(self, array: &mut [i32; N]) {
        match self {
            Type::Rand => rand_proc(array),
            Type::RandN => rand_n_proc(array),
            Type::Fore => forward_proc(array),
            Type::Back => backward_proc(array),
            Type::Same => same_proc(array),
        }
    }
}

// Port of: bench/SortBench.cpp#L20-L26 (chrome/m156)
fn rand_proc(array: &mut [i32; N]) {
    let mut rand = Random::default();
    for a in array.iter_mut() {
        *a = rand.next_s();
    }
}

// Port of: bench/SortBench.cpp#L28-L35 (chrome/m156)
fn rand_n_proc(array: &mut [i32; N]) {
    let mut rand = Random::default();
    let mod_ = (N / 10) as u32;
    for a in array.iter_mut() {
        // rand.nextU() % mod: unsigned, then stored into the int array.
        *a = (rand.next_u() % mod_) as i32;
    }
}

// Port of: bench/SortBench.cpp#L37-L42 (chrome/m156)
fn forward_proc(array: &mut [i32; N]) {
    for (i, a) in array.iter_mut().enumerate() {
        *a = i as i32;
    }
}

// Port of: bench/SortBench.cpp#L44-L49 (chrome/m156)
fn backward_proc(array: &mut [i32; N]) {
    for (i, a) in array.iter_mut().enumerate() {
        *a = -(i as i32);
    }
}

// Port of: bench/SortBench.cpp#L51-L56 (chrome/m156)
fn same_proc(array: &mut [i32; N]) {
    for a in array.iter_mut() {
        *a = N as i32;
    }
}

/// `SortType`: the algorithms this port registers, in `gSorts` order.
#[derive(Clone, Copy)]
enum SortType {
    SkQSort,
    SkHeap,
    StdSort,
}

impl SortType {
    /// `gSorts[s].fName`.
    // Port of: bench/SortBench.cpp#L93-L98 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            SortType::SkQSort => "skqsort",
            SortType::SkHeap => "skheap",
            SortType::StdSort => "stdsort",
        }
    }

    /// `gSorts[s].fProc(array)`.
    fn sort(self, array: &mut [i32]) {
        match self {
            // SkTQSort<int>(array, array + N)
            SortType::SkQSort => t_q_sort(array, |a: &i32, b: &i32| a < b),
            // SkTHeapSort<int>(array, N)
            SortType::SkHeap => t_heap_sort(array, &|a: &i32, b: &i32| a < b),
            // std::sort(array, array+N): the standard library's sort.
            SortType::StdSort => array.sort_unstable(),
        }
    }
}

/// `class SortBench`.
// Port of: bench/SortBench.cpp#L100-L137 (chrome/m156)
struct SortBench {
    name: String,
    ty: Type,
    sort_type: SortType,
    unsorted: Vec<i32>,
}

impl SortBench {
    // Port of: bench/SortBench.cpp#L107-L110 (chrome/m156)
    fn new(t: Type, s: SortType) -> Self {
        Self {
            name: format!("sort_{}_{}", s.name(), t.name()),
            ty: t,
            sort_type: s,
            unsorted: Vec::new(),
        }
    }
}

impl Benchmark for SortBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/SortBench.cpp#L112-L115 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Delayed initialization only done if onDraw will be called.
    // Port of: bench/SortBench.cpp#L122-L126 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let mut unsorted = [0_i32; N];
        self.ty.fill(&mut unsorted);
        self.unsorted = unsorted.to_vec();
    }

    // Port of: bench/SortBench.cpp#L128-L141 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut sorted = vec![0_i32; N];
        for _ in 0..loops {
            sorted.copy_from_slice(&self.unsorted);
            self.sort_type.sort(&mut sorted);
            // SK_DEBUG: every adjacent pair is in order.
            debug_assert!(sorted.windows(2).all(|w| w[0] <= w[1]));
        }
        std::hint::black_box(&sorted);
    }
}

// Port of: bench/SortBench.cpp#L170-L170 (chrome/m156)
def_bench!(
    sort_bench_sk_q_sort_rand = "NewSkQSort(kRand)",
    SortBench::new(Type::Rand, SortType::SkQSort)
);
// Port of: bench/SortBench.cpp#L171-L171 (chrome/m156)
def_bench!(
    sort_bench_sk_heap_rand = "NewSkHeap(kRand)",
    SortBench::new(Type::Rand, SortType::SkHeap)
);
// Port of: bench/SortBench.cpp#L173-L173 (chrome/m156)
def_bench!(
    sort_bench_std_sort_rand = "NewStdSort(kRand)",
    SortBench::new(Type::Rand, SortType::StdSort)
);
// Port of: bench/SortBench.cpp#L175-L175 (chrome/m156)
def_bench!(
    sort_bench_sk_q_sort_rand_n = "NewSkQSort(kRandN)",
    SortBench::new(Type::RandN, SortType::SkQSort)
);
// Port of: bench/SortBench.cpp#L176-L176 (chrome/m156)
def_bench!(
    sort_bench_sk_heap_rand_n = "NewSkHeap(kRandN)",
    SortBench::new(Type::RandN, SortType::SkHeap)
);
// Port of: bench/SortBench.cpp#L178-L178 (chrome/m156)
def_bench!(
    sort_bench_std_sort_rand_n = "NewStdSort(kRandN)",
    SortBench::new(Type::RandN, SortType::StdSort)
);
// Port of: bench/SortBench.cpp#L180-L180 (chrome/m156)
def_bench!(
    sort_bench_sk_q_sort_fore = "NewSkQSort(kFore)",
    SortBench::new(Type::Fore, SortType::SkQSort)
);
// Port of: bench/SortBench.cpp#L181-L181 (chrome/m156)
def_bench!(
    sort_bench_sk_heap_fore = "NewSkHeap(kFore)",
    SortBench::new(Type::Fore, SortType::SkHeap)
);
// Port of: bench/SortBench.cpp#L183-L183 (chrome/m156)
def_bench!(
    sort_bench_std_sort_fore = "NewStdSort(kFore)",
    SortBench::new(Type::Fore, SortType::StdSort)
);
// Port of: bench/SortBench.cpp#L185-L185 (chrome/m156)
def_bench!(
    sort_bench_sk_q_sort_back = "NewSkQSort(kBack)",
    SortBench::new(Type::Back, SortType::SkQSort)
);
// Port of: bench/SortBench.cpp#L186-L186 (chrome/m156)
def_bench!(
    sort_bench_sk_heap_back = "NewSkHeap(kBack)",
    SortBench::new(Type::Back, SortType::SkHeap)
);
// Port of: bench/SortBench.cpp#L188-L188 (chrome/m156)
def_bench!(
    sort_bench_std_sort_back = "NewStdSort(kBack)",
    SortBench::new(Type::Back, SortType::StdSort)
);
// Port of: bench/SortBench.cpp#L190-L190 (chrome/m156)
def_bench!(
    sort_bench_sk_q_sort_same = "NewSkQSort(kSame)",
    SortBench::new(Type::Same, SortType::SkQSort)
);
// Port of: bench/SortBench.cpp#L191-L191 (chrome/m156)
def_bench!(
    sort_bench_sk_heap_same = "NewSkHeap(kSame)",
    SortBench::new(Type::Same, SortType::SkHeap)
);
// Port of: bench/SortBench.cpp#L193-L193 (chrome/m156)
def_bench!(
    sort_bench_std_sort_same = "NewStdSort(kSame)",
    SortBench::new(Type::Same, SortType::StdSort)
);
