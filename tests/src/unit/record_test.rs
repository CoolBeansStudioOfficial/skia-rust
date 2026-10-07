// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordTest.cpp (chrome/m156)
//
// Not ported: `Record_Alignment` tests the alignment of the arena allocations of
// `SkRecord::alloc<T>`; the Rust record stores its commands in a `Vec` (manifest: excluded).

#![cfg(test)]

use super::record_test_utils::assert_type;
use crate::{def_test, reporter_assert};
use skia_rust_core::paint::Paint;
use skia_rust_core::record::Record;
use skia_rust_core::records::{ClipRect, Command, DrawRect, NoOp, Restore, Save};
use skia_rust_core::rect::Rect;

// Sums the area of any DrawRect command it sees.
// Port of: tests/RecordTest.cpp#L21-L43 (chrome/m156)
#[derive(Default)]
struct AreaSummer {
    area: i32,
}

impl AreaSummer {
    #[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast in the C++
    fn apply(&mut self, record: &Record) {
        for i in 0..record.count() {
            record.visit(i, |command| {
                if let Command::DrawRect(draw) = command {
                    self.area += (draw.rect.width() * draw.rect.height()) as i32;
                }
            });
        }
    }

    fn area(&self) -> i32 {
        self.area
    }
}

// Scales out the bottom-right corner of any DrawRect command it sees by 2x.
// Port of: tests/RecordTest.cpp#L45-L58 (chrome/m156)
struct Stretch;

impl Stretch {
    #[allow(clippy::unused_self)] // a functor object in the C++
    fn apply(&self, record: &mut Record) {
        for i in 0..record.count() {
            record.mutate(i, |command| {
                if let Command::DrawRect(draw) = command {
                    draw.rect.right *= 2.0;
                    draw.rect.bottom *= 2.0;
                }
            });
        }
    }
}

// Basic tests for the low-level SkRecord code.
// Port of: tests/RecordTest.cpp#L62-L81 (chrome/m156)
def_test!(Record, |r| {
    let mut record = Record::new();

    // Add a simple DrawRect command.
    let rect = Rect::from_wh(10.0, 10.0);
    let paint = Paint::default();
    record.append(DrawRect { paint, rect });

    // Its area should be 100.
    let mut summer = AreaSummer::default();
    summer.apply(&record);
    reporter_assert!(r, summer.area() == 100);

    // Scale 2x.
    let stretch = Stretch;
    stretch.apply(&mut record);

    // Now its area should be 100 + 400.
    summer.apply(&record);
    reporter_assert!(r, summer.area() == 500);
});

// Port of: tests/RecordTest.cpp#L83-L98 (chrome/m156)
def_test!(Record_defrag, |r| {
    let mut record = Record::new();
    record.append(Save);
    record.append(ClipRect::default());
    record.append(NoOp);
    record.append(DrawRect::default());
    record.append(NoOp);
    record.append(NoOp);
    record.append(Restore::default());
    reporter_assert!(r, record.count() == 7);

    record.defrag();
    reporter_assert!(r, record.count() == 4);
    assert_type::<Save>(r, &record, 0);
    assert_type::<ClipRect>(r, &record, 1);
    assert_type::<DrawRect>(r, &record, 2);
    assert_type::<Restore>(r, &record, 3);
});
