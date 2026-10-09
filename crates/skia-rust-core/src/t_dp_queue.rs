// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTDPQueue.h

//! `SkTDPQueue`: Skia's binary-heap priority queue whose elements may know their own index.
//!
//! The exact heap layout matters to callers that iterate the queue with [`TDPQueue::at`] or that
//! pop elements whose priorities tie (Graphite's resource cache gives every zero-sized resource the
//! same use token), so this is a line-by-line port rather than `std::collections::BinaryHeap`.

use crate::t_sort::t_q_sort;

/// `SkTDPQueue<T, LESS, INDEX>`.
///
/// `less(a, b)` returns true if `a` is higher priority than `b`. When `set_index` is given, it is
/// called with each element's new position whenever the queue moves it (Skia's `INDEX`), which
/// allows [`TDPQueue::remove`] and [`TDPQueue::priority_did_change`].
// Port of: src/core/SkTDPQueue.h#L18-L220 (chrome/m156)
#[doc(alias = "SkTDPQueue")]
pub struct TDPQueue<T> {
    array: Vec<T>,
    less: fn(&T, &T) -> bool,
    set_index: Option<fn(&T, i32)>,
    get_index: Option<fn(&T) -> i32>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for TDPQueue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TDPQueue")
            .field("array", &self.array)
            .finish_non_exhaustive()
    }
}

impl<T: Clone> TDPQueue<T> {
    /// A queue without an index function.
    #[must_use]
    pub fn new(less: fn(&T, &T) -> bool) -> Self {
        Self {
            array: Vec::new(),
            less,
            set_index: None,
            get_index: None,
        }
    }

    /// A queue whose elements store their own index (`INDEX`), read back with `get_index`.
    #[must_use]
    pub fn with_index(
        less: fn(&T, &T) -> bool,
        set_index: fn(&T, i32),
        get_index: fn(&T) -> i32,
    ) -> Self {
        Self {
            array: Vec::new(),
            less,
            set_index: Some(set_index),
            get_index: Some(get_index),
        }
    }

    /// Number of items in the queue.
    // Port of: src/core/SkTDPQueue.h#L41 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.array.len()
    }

    /// Gets the next item in the queue without popping it.
    // Port of: src/core/SkTDPQueue.h#L44-L45 (chrome/m156)
    #[must_use]
    pub fn peek(&self) -> &T {
        &self.array[0]
    }

    /// Removes the next item.
    // Port of: src/core/SkTDPQueue.h#L48-L62 (chrome/m156)
    pub fn pop(&mut self) {
        self.validate(None);
        if cfg!(debug_assertions)
            && let Some(set_index) = self.set_index
        {
            set_index(&self.array[0], -1);
        }
        if 1 == self.array.len() {
            self.array.pop();
            return;
        }

        let last = self.array.len() - 1;
        self.array[0] = self.array[last].clone();
        self.set_index_at(0);
        self.array.pop();
        self.percolate_down_if_necessary(0);

        self.validate(None);
    }

    /// Inserts a new item in the queue based on its priority.
    // Port of: src/core/SkTDPQueue.h#L65-L72 (chrome/m156)
    pub fn insert(&mut self, entry: T) {
        self.validate(None);
        let index = self.array.len();
        self.array.push(entry);
        self.set_index_at(self.array.len() - 1);
        self.percolate_up_if_necessary(index);
        self.validate(None);
    }

    /// Random access removal. This requires an index function.
    ///
    /// # Panics
    /// If the queue has no index function or `entry` is not in the queue.
    // Port of: src/core/SkTDPQueue.h#L75-L89 (chrome/m156)
    pub fn remove(&mut self, entry: &T) {
        let get_index = self.get_index.expect("SkTDPQueue::remove requires INDEX");
        let index = usize::try_from(get_index(entry)).expect("entry is not in the queue");
        debug_assert!(index < self.array.len());
        self.validate(None);
        if cfg!(debug_assertions)
            && let Some(set_index) = self.set_index
        {
            set_index(&self.array[index], -1);
        }
        if index == self.array.len() - 1 {
            self.array.pop();
            return;
        }
        let last = self.array.len() - 1;
        self.array[index] = self.array[last].clone();
        self.array.pop();
        self.set_index_at(index);
        self.percolate_up_or_down(index);
        self.validate(None);
    }

    /// Notification that the priority of an entry has changed.
    ///
    /// # Panics
    /// If the queue has no index function or `entry` is not in the queue.
    // Port of: src/core/SkTDPQueue.h#L94-L101 (chrome/m156)
    #[doc(alias = "priorityDidChange")]
    pub fn priority_did_change(&mut self, entry: &T) {
        let get_index = self
            .get_index
            .expect("SkTDPQueue::priorityDidChange requires INDEX");
        let index = usize::try_from(get_index(entry)).expect("entry is not in the queue");
        debug_assert!(index < self.array.len());
        self.validate(Some(index));
        self.percolate_up_or_down(index);
        self.validate(None);
    }

    /// Gets the item at index `i` in the priority queue. `at(0)` is equivalent to `peek()`.
    // Port of: src/core/SkTDPQueue.h#L105 (chrome/m156)
    #[must_use]
    pub fn at(&self, i: usize) -> &T {
        &self.array[i]
    }

    /// Sorts the queue into priority order (with `SkTQSort`).
    // Port of: src/core/SkTDPQueue.h#L110-L118 (chrome/m156)
    pub fn sort(&mut self) {
        if self.array.len() > 1 {
            let less = self.less;
            t_q_sort(&mut self.array, less);
            for i in 0..self.array.len() {
                self.set_index_at(i);
            }
            self.validate(None);
        }
    }

    // Port of: src/core/SkTDPQueue.h#L121-L122 (chrome/m156)
    fn left_of(x: usize) -> usize {
        2 * x + 1
    }

    fn parent_of(x: usize) -> usize {
        debug_assert!(x > 0);
        (x - 1) >> 1
    }

    // Port of: src/core/SkTDPQueue.h#L124-L130 (chrome/m156)
    fn percolate_up_or_down(&mut self, index: usize) {
        if !self.percolate_up_if_necessary(index) {
            self.validate(Some(index));
            self.percolate_down_if_necessary(index);
        }
    }

    // Port of: src/core/SkTDPQueue.h#L132-L153 (chrome/m156)
    fn percolate_up_if_necessary(&mut self, mut index: usize) -> bool {
        let mut percolated = false;
        loop {
            if 0 == index {
                self.set_index_at(index);
                return percolated;
            }
            let p = Self::parent_of(index);
            if (self.less)(&self.array[index], &self.array[p]) {
                self.array.swap(index, p);
                self.set_index_at(index);
                index = p;
                percolated = true;
            } else {
                self.set_index_at(index);
                return percolated;
            }
            self.validate(Some(index));
        }
    }

    // Port of: src/core/SkTDPQueue.h#L155-L193 (chrome/m156)
    fn percolate_down_if_necessary(&mut self, mut index: usize) {
        loop {
            let mut child = Self::left_of(index);

            if child >= self.array.len() {
                // We're a leaf.
                self.set_index_at(index);
                return;
            }

            if child + 1 >= self.array.len() {
                // We only have a left child.
                if (self.less)(&self.array[child], &self.array[index]) {
                    self.array.swap(child, index);
                    self.set_index_at(child);
                    self.set_index_at(index);
                    return;
                }
            } else if (self.less)(&self.array[child + 1], &self.array[child]) {
                // The right child is the one we should swap with, if we swap.
                child += 1;
            }

            // Check if we need to swap.
            if (self.less)(&self.array[child], &self.array[index]) {
                self.array.swap(child, index);
                self.set_index_at(index);
                index = child;
            } else {
                // We're less than both our children.
                self.set_index_at(index);
                return;
            }
            self.validate(Some(index));
        }
    }

    // Port of: src/core/SkTDPQueue.h#L195-L200 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTDArray index is int
    fn set_index_at(&self, index: usize) {
        debug_assert!(index < self.array.len());
        if let Some(set_index) = self.set_index {
            set_index(&self.array[index], index as i32);
        }
    }

    // Port of: src/core/SkTDPQueue.h#L202-L213 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTDArray index is int
    fn validate(&self, excluded_index: Option<usize>) {
        if cfg!(debug_assertions) {
            for i in 1..self.array.len() {
                let p = Self::parent_of(i);
                if excluded_index != Some(p) && excluded_index != Some(i) {
                    debug_assert!(!(self.less)(&self.array[i], &self.array[p]));
                    if let Some(get_index) = self.get_index {
                        debug_assert_eq!(get_index(&self.array[i]), i as i32);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn pops_in_priority_order() {
        let mut q = TDPQueue::new(|a: &i32, b: &i32| a < b);
        for v in [5, 3, 8, 1, 9, 2] {
            q.insert(v);
        }
        let mut out = Vec::new();
        while q.count() > 0 {
            out.push(*q.peek());
            q.pop();
        }
        assert_eq!(out, [1, 2, 3, 5, 8, 9]);
    }

    #[derive(Clone, Debug)]
    struct Item {
        pri: i32,
        index: Rc<Cell<i32>>,
    }

    #[test]
    fn random_removal_keeps_indices() {
        let mut q = TDPQueue::with_index(
            |a: &Item, b: &Item| a.pri < b.pri,
            |e, i| e.index.set(i),
            |e| e.index.get(),
        );
        let items: Vec<Item> = [4, 7, 1, 9, 3]
            .iter()
            .map(|&pri| Item {
                pri,
                index: Rc::new(Cell::new(-1)),
            })
            .collect();
        for it in &items {
            q.insert(it.clone());
        }
        q.remove(&items[0]);
        for i in 0..q.count() {
            assert_eq!(q.at(i).index.get(), i32::try_from(i).unwrap());
        }
        assert_eq!(q.peek().pri, 1);
    }
}
