// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecord.h, src/core/SkRecord.cpp

//! `SkRecord`: a sequence of canvas calls, saved for future use.
//!
//! These future uses may include replay, optimization, serialization, or combinations of those.
//! [`RecordCanvas`](crate::record_canvas::RecordCanvas) presents a `Canvas` interface for creating
//! a record, and [`record_draw`](crate::record_draw::record_draw) plays one back into another
//! canvas.
//!
//! skia-rust: Skia stores the records of each call in an arena and a `[type, pointer]` array
//! next to it; here the array holds the [`Command`]s themselves, so `alloc` (and the test of its
//! alignment) has no equivalent. `visit` and `mutate` take closures over the [`Command`] where
//! Skia takes a functor with an overload per type.

use std::mem::{align_of, size_of};

use crate::records::{Command, NoOp, RecordKind};

/// A sequence of canvas calls (`SkRecord`).
// Port of: src/core/SkRecord.h#L38-L170 (chrome/m156)
#[doc(alias = "SkRecord")]
#[derive(Clone, Debug, Default)]
pub struct Record {
    records: Vec<Command>,
    approx_bytes_allocated: usize,
}

impl Record {
    /// An empty record.
    #[must_use]
    pub fn new() -> Record {
        Record::default()
    }

    /// Returns the number of canvas commands in this record (`count`).
    #[must_use]
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// The i-th canvas command.
    #[must_use]
    pub fn get(&self, i: usize) -> &Command {
        &self.records[i]
    }

    /// Visits the i-th canvas command with `f` (`visit`).
    pub fn visit<R>(&self, i: usize, f: impl FnOnce(&Command) -> R) -> R {
        f(&self.records[i])
    }

    /// Mutates the i-th canvas command with `f` (`mutate`).
    pub fn mutate<R>(&mut self, i: usize, f: impl FnOnce(&mut Command) -> R) -> R {
        f(&mut self.records[i])
    }

    /// Adds a new command to the end of this record (`append<T>`); returns it.
    ///
    /// # Panics
    /// Never: the command just pushed is a `T`.
    pub fn append<T: RecordKind>(&mut self, command: T) -> &mut T {
        self.approx_bytes_allocated += Self::bytes_for::<T>();
        self.records.push(command.into_command());
        let last = self.records.last_mut().expect("just pushed");
        T::from_command_mut(last).expect("just pushed")
    }

    /// Replaces the i-th command with a new command (`replace<T>`). References to the original
    /// command are invalidated.
    ///
    /// # Panics
    /// If `i` is out of range.
    pub fn replace<T: RecordKind>(&mut self, i: usize, command: T) -> &mut T {
        debug_assert!(i < self.count());

        self.approx_bytes_allocated += Self::bytes_for::<T>();
        self.records[i] = command.into_command();
        T::from_command_mut(&mut self.records[i]).expect("just replaced")
    }

    // `allocCommand<T>` allocates (and counts) only for non-empty types.
    fn bytes_for<T>() -> usize {
        if size_of::<T>() == 0 {
            0
        } else {
            size_of::<T>() + align_of::<T>()
        }
    }

    /// Does not return the bytes in any pointers embedded in the Records; callers need to
    /// iterate with a visitor to measure those they care for (`bytesUsed`).
    // Port of: src/core/SkRecord.cpp#L23-L26 (chrome/m156)
    #[doc(alias = "bytesUsed")]
    #[must_use]
    pub fn bytes_used(&self) -> usize {
        self.approx_bytes_allocated + size_of::<Record>()
    }

    /// Rearranges and resizes this record to eliminate any `NoOp`s. May change [`count`](Self::count)
    /// and the indices of ops, but preserves their order (`defrag`).
    // Port of: src/core/SkRecord.cpp#L28-L36 (chrome/m156)
    pub fn defrag(&mut self) {
        // Remove all the NoOps, preserving the order of other ops, e.g.
        //      Save, ClipRect, NoOp, DrawRect, NoOp, NoOp, Restore
        //  ->  Save, ClipRect, DrawRect, Restore
        self.records.retain(|op| NoOp::from_command(op).is_none());
    }
}
