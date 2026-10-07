// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTSort.h

//! Skia's sorting templates (`SkTSort.h`).
//!
//! skia-rust: most of Skia's uses of these sorts are interchangeable with `slice::sort_by`, but
//! the scan converters sort edges whose comparison does not distinguish every pair of edges
//! (`compare_edges` in `SkScan_Path.cpp`/`SkScan_AAAPath.cpp`). Which of two "equal" edges comes
//! first then depends on the algorithm, and it can change pixels, so the scan converters use this
//! exact port of Skia's introsort instead of `std`'s sorts.

use crate::math_priv::next_log2;

/// Sifts a broken heap. The input array is a heap from root to bottom except that the root entry
/// may be out of place.
///
/// Sinks a hole from `array[root]` to leaf and then sifts the original `array[root]` element from
/// the leaf level up. `root` and `bottom` are one based.
// Port of: src/core/SkTSort.h#L33-L57 (chrome/m156)
#[doc(alias = "SkTHeapSort_SiftUp")]
fn heap_sort_sift_up<T: Copy, C: Fn(&T, &T) -> bool>(
    array: &mut [T],
    mut root: usize,
    bottom: usize,
    less_than: &C,
) {
    let x = array[root - 1];
    let start = root;
    let mut j = root << 1;
    while j <= bottom {
        if j < bottom && less_than(&array[j - 1], &array[j]) {
            j += 1;
        }
        array[root - 1] = array[j - 1];
        root = j;
        j = root << 1;
    }
    j = root >> 1;
    while j >= start {
        if less_than(&array[j - 1], &x) {
            array[root - 1] = array[j - 1];
            root = j;
            j = root >> 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

/// Sifts a broken heap: sifts the `array[root]` element from the root down. `root` and `bottom`
/// are one based.
// Port of: src/core/SkTSort.h#L67-L84 (chrome/m156)
#[doc(alias = "SkTHeapSort_SiftDown")]
fn heap_sort_sift_down<T: Copy, C: Fn(&T, &T) -> bool>(
    array: &mut [T],
    mut root: usize,
    bottom: usize,
    less_than: &C,
) {
    let x = array[root - 1];
    let mut child = root << 1;
    while child <= bottom {
        if child < bottom && less_than(&array[child - 1], &array[child]) {
            child += 1;
        }
        if less_than(&x, &array[child - 1]) {
            array[root - 1] = array[child - 1];
            root = child;
            child = root << 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

/// Sorts `array` with `less_than` using a heap sort (`SkTHeapSort`).
// Port of: src/core/SkTSort.h#L93-L103 (chrome/m156)
#[doc(alias = "SkTHeapSort")]
pub fn heap_sort<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], less_than: &C) {
    let count = array.len();
    let mut i = count >> 1;
    while i > 0 {
        heap_sort_sift_down(array, i, count, less_than);
        i -= 1;
    }

    let mut i = count.saturating_sub(1);
    while i > 0 {
        array.swap(0, i);
        heap_sort_sift_up(array, 1, i, less_than);
        i -= 1;
    }
}

/// Sorts `array` with `less_than` using an insertion sort (`SkTInsertionSort`).
// Port of: src/core/SkTSort.h#L113-L128 (chrome/m156)
#[doc(alias = "SkTInsertionSort")]
pub fn insertion_sort<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], less_than: &C) {
    if array.is_empty() {
        return;
    }
    let right = array.len() - 1;
    for next in 1..=right {
        if !less_than(&array[next], &array[next - 1]) {
            continue;
        }
        let insert = array[next];
        let mut hole = next;
        loop {
            array[hole] = array[hole - 1];
            hole -= 1;
            if !(0 < hole && less_than(&insert, &array[hole - 1])) {
                break;
            }
        }
        array[hole] = insert;
    }
}

// Port of: src/core/SkTSort.h#L132-L148 (chrome/m156)
fn q_sort_partition<T: Copy, C: Fn(&T, &T) -> bool>(
    array: &mut [T],
    pivot: usize,
    less_than: &C,
) -> usize {
    let right = array.len() - 1;
    let pivot_value = array[pivot];
    array.swap(pivot, right);
    let mut new_pivot = 0;
    let mut left = 0;
    while left < right {
        if less_than(&array[left], &pivot_value) {
            array.swap(left, new_pivot);
            new_pivot += 1;
        }
        left += 1;
    }
    array.swap(new_pivot, right);
    new_pivot
}

/// Introsort: a quicksort that switches to insertion sort for small regions and to heap sort
/// when `depth` reaches zero. It recurses on the left region after pivoting and loops on the
/// right.
// Port of: src/core/SkTSort.h#L162-L184 (chrome/m156)
#[doc(alias = "SkTIntroSort")]
fn intro_sort<T: Copy, C: Fn(&T, &T) -> bool>(mut depth: i32, mut array: &mut [T], less_than: &C) {
    loop {
        let count = array.len();
        if count <= 32 {
            insertion_sort(array, less_than);
            return;
        }

        if depth == 0 {
            heap_sort(array, less_than);
            return;
        }
        depth -= 1;

        let middle = (count - 1) >> 1;
        let pivot_count = q_sort_partition(array, middle, less_than);

        let (left, rest) = array.split_at_mut(pivot_count);
        intro_sort(depth, left, less_than);
        array = &mut rest[1..];
    }
}

/// Sorts `array` with `less_than` using Skia's introsort (`SkTQSort`). Not stable: elements that
/// compare equal end up in the same order as in Skia.
// Port of: src/core/SkTSort.h#L193-L202 (chrome/m156)
#[doc(alias = "SkTQSort")]
#[allow(clippy::cast_possible_truncation)] // mirrors SkToInt: counts fit in an int
pub fn t_q_sort<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], less_than: C) {
    let n = array.len();
    if n <= 1 {
        return;
    }
    let depth = 2 * next_log2((n - 1) as u32);
    intro_sort(depth, array, &less_than);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::Random;

    // Not an upstream test (Skia's SkTSort tests are excluded); checks the port sorts, and that
    // equal keys keep Skia's (unstable) order on a small hand-traced case.
    #[test]
    fn sorts_like_skia() {
        let mut rand = Random::new(0);
        for n in [0usize, 1, 2, 5, 32, 33, 100, 1000] {
            let mut v: Vec<u32> = (0..n).map(|_| rand.next_u() % 50).collect();
            let mut expected = v.clone();
            expected.sort_unstable();
            t_q_sort(&mut v, |a, b| a < b);
            assert_eq!(v, expected);
            let mut h: Vec<u32> = (0..n).map(|_| rand.next_u() % 50).collect();
            let mut expected = h.clone();
            expected.sort_unstable();
            heap_sort(&mut h, &|a: &u32, b: &u32| a < b);
            assert_eq!(h, expected);
        }

        // Insertion sort moves an element only past strictly greater ones.
        let mut pairs = [(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd')];
        t_q_sort(&mut pairs, |a, b| a.0 < b.0);
        assert_eq!(pairs, [(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c')]);
    }
}
