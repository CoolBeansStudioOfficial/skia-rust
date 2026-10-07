// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordTestUtils.h (chrome/m156)

//! The helpers of the record tests (`RecordTestUtils.h`).

#![cfg(test)]

use crate::{Reporter, reporter_assert};
use skia_rust_core::record::Record;
use skia_rust_core::records::{RecordKind, Type};

// If the command we're reading is a U, set ptr to it, otherwise set it to nullptr.
// Port of: tests/RecordTestUtils.h#L16-L31 (chrome/m156)
struct ReadAs<'r, U: RecordKind> {
    ptr: Option<&'r U>,
    type_: Option<Type>,
}

impl<'r, U: RecordKind> ReadAs<'r, U> {
    fn new() -> Self {
        // `SkRecords::Type(~0)`: no type.
        ReadAs {
            ptr: None,
            type_: None,
        }
    }

    // `operator()(const U&)` sets ptr and type; `operator()(const T&)` sets only the type (the
    // C++ sets it to `U::kType` in both cases).
    fn visit(&mut self, command: &'r skia_rust_core::records::Command) {
        if let Some(r) = U::from_command(command) {
            self.ptr = Some(r);
        }
        self.type_ = Some(U::TYPE);
    }
}

// Assert that the ith command in record is of type T, and return it.
// Port of: tests/RecordTestUtils.h#L33-L40 (chrome/m156)
pub fn assert_type<'r, T: RecordKind>(
    r: &mut Reporter,
    record: &'r Record,
    index: usize,
) -> Option<&'r T> {
    let mut reader = ReadAs::<T>::new();
    reader.visit(record.get(index));
    reporter_assert!(r, Some(T::TYPE) == reader.type_);
    reporter_assert!(r, reader.ptr.is_some());
    reader.ptr
}

// Port of: tests/RecordTestUtils.h#L42-L56 (chrome/m156)
#[must_use]
pub fn count_instances_of_type<DrawT: RecordKind>(record: &Record) -> i32 {
    let mut counter = 0;
    for i in 0..record.count() {
        counter += i32::from(record.visit(i, |c| DrawT::from_command(c).is_some()));
    }
    counter
}

// Port of: tests/RecordTestUtils.h#L58-L66 (chrome/m156)
#[must_use]
#[allow(dead_code)] // part of the helper set; no ported test uses it yet
pub fn find_first_instances_of_type<DrawT: RecordKind>(record: &Record) -> Option<usize> {
    (0..record.count()).find(|&i| record.visit(i, |c| DrawT::from_command(c).is_some()))
}
