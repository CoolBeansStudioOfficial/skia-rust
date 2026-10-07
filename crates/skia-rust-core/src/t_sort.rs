// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTSort.h

//! Skia's sorting routines (`SkTSort.h`): heap sort, insertion sort and the introsort `SkTQSort`.
//!
//! The scan converter and `SkRegion::addBoundaryPath` sort with a comparator that has ties
//! (edges with equal `fFirstY` and `fX`), and neither sort is stable, so the order of equal
//! elements depends on the exact algorithm. These are ported so that order is the same as
//! Skia's; use them instead of `slice::sort_by` wherever Skia uses `SkTQSort`.

use crate::math_priv::next_log2;

// Sifts a broken heap. The input array is a heap from root to bottom except that the root entry
// may be out of place. `root` and `bottom` are one based.
// Port of: src/core/SkTSort.h#L33-L52 (chrome/m156)
fn heap_sort_sift_up<T: Clone>(
    array: &mut [T],
    root: usize,
    bottom: usize,
    less_than: &impl Fn(&T, &T) -> bool,
) {
    let mut root = root;
    let x = array[root - 1].clone();
    let start = root;
    let mut j = root << 1;
    while j <= bottom {
        if j < bottom && less_than(&array[j - 1], &array[j]) {
            j += 1;
        }
        array[root - 1] = array[j - 1].clone();
        root = j;
        j = root << 1;
    }
    j = root >> 1;
    while j >= start {
        if less_than(&array[j - 1], &x) {
            array[root - 1] = array[j - 1].clone();
            root = j;
            j = root >> 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

// Sifts the `array[root]` element from the root down. `root` and `bottom` are one based.
// Port of: src/core/SkTSort.h#L63-L79 (chrome/m156)
fn heap_sort_sift_down<T: Clone>(
    array: &mut [T],
    root: usize,
    bottom: usize,
    less_than: &impl Fn(&T, &T) -> bool,
) {
    let mut root = root;
    let x = array[root - 1].clone();
    let mut child = root << 1;
    while child <= bottom {
        if child < bottom && less_than(&array[child - 1], &array[child]) {
            child += 1;
        }
        if less_than(&x, &array[child - 1]) {
            array[root - 1] = array[child - 1].clone();
            root = child;
            child = root << 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

/// Sorts `array` with the comparator `less_than` (true if `a` comes before `b`) using a heap
/// sort.
// Port of: src/core/SkTSort.h#L88-L96 (chrome/m156)
#[doc(alias = "SkTHeapSort")]
pub fn t_heap_sort<T: Clone>(array: &mut [T], less_than: &impl Fn(&T, &T) -> bool) {
    let count = array.len();
    let mut i = count >> 1;
    while i > 0 {
        heap_sort_sift_down(array, i, count, less_than);
        i -= 1;
    }

    let mut i = count.wrapping_sub(1);
    while i > 0 && i < count {
        array.swap(0, i);
        heap_sort_sift_up(array, 1, i, less_than);
        i -= 1;
    }
}

/// Sorts `array` with the comparator `less_than` using an insertion sort.
// Port of: src/core/SkTSort.h#L104-L118 (chrome/m156)
#[doc(alias = "SkTInsertionSort")]
pub fn t_insertion_sort<T: Clone>(array: &mut [T], less_than: &impl Fn(&T, &T) -> bool) {
    for next in 1..array.len() {
        if !less_than(&array[next], &array[next - 1]) {
            continue;
        }
        let insert = array[next].clone();
        let mut hole = next;
        loop {
            array[hole] = array[hole - 1].clone();
            hole -= 1;
            if !(0 < hole && less_than(&insert, &array[hole - 1])) {
                break;
            }
        }
        array[hole] = insert;
    }
}

// Port of: src/core/SkTSort.h#L122-L136 (chrome/m156)
fn t_q_sort_partition<T: Clone>(
    array: &mut [T],
    pivot: usize,
    less_than: &impl Fn(&T, &T) -> bool,
) -> usize {
    let right = array.len() - 1;
    let pivot_value = array[pivot].clone();
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

// Introsort is a modified Quicksort. When the region to be sorted is a small constant size, it
// uses Insertion Sort. When depth becomes zero, it switches over to Heap Sort. This recurses on
// the left region after pivoting and loops on the right.
// Port of: src/core/SkTSort.h#L150-L171 (chrome/m156)
fn t_intro_sort<T: Clone>(depth: i32, array: &mut [T], less_than: &impl Fn(&T, &T) -> bool) {
    let mut depth = depth;
    let mut array = array;
    loop {
        let count = array.len();
        if count <= 32 {
            t_insertion_sort(array, less_than);
            return;
        }

        if depth == 0 {
            t_heap_sort(array, less_than);
            return;
        }
        depth -= 1;

        let middle = (count - 1) >> 1;
        let pivot = t_q_sort_partition(array, middle, less_than);

        let (left, right) = array.split_at_mut(pivot);
        t_intro_sort(depth, left, less_than);
        array = &mut right[1..];
    }
}

/// Sorts `array` with the comparator `less_than` (true if `a` comes before `b`) using an
/// introsort. Not stable.
// Port of: src/core/SkTSort.h#L180-L188 (chrome/m156)
#[doc(alias = "SkTQSort")]
pub fn t_q_sort<T: Clone>(array: &mut [T], less_than: impl Fn(&T, &T) -> bool) {
    let n = array.len();
    if n <= 1 {
        return;
    }
    // Limit Introsort recursion depth to no more than 2 * ceil(log2(n-1)).
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToInt; slices here are far below 2^32
    let depth = 2 * next_log2((n - 1) as u32);
    t_intro_sort(depth, array, &less_than);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pseudo_random(n: usize) -> Vec<u32> {
        let mut s = 12345u32;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 16) % 1000
            })
            .collect()
    }

    #[test]
    fn sorts() {
        for n in [0usize, 1, 2, 5, 32, 33, 100, 1000] {
            let mut expected = pseudo_random(n);
            expected.sort_unstable();

            let mut v = pseudo_random(n);
            t_q_sort(&mut v, |a, b| a < b);
            assert_eq!(v, expected, "q_sort n = {n}");

            let mut v = pseudo_random(n);
            t_heap_sort(&mut v, &|a: &u32, b: &u32| a < b);
            assert_eq!(v, expected, "heap_sort n = {n}");

            let mut v = pseudo_random(n);
            t_insertion_sort(&mut v, &|a: &u32, b: &u32| a < b);
            assert_eq!(v, expected, "insertion_sort n = {n}");
        }
    }
}
