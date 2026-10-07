// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordPatternTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::paint::Paint;
use skia_rust_core::record::Record;
use skia_rust_core::record_canvas::{RecordCanvas, SharedRecord};
use skia_rust_core::record_pattern::{Greedy, Is, IsDraw, Not, Or, Pattern};
use skia_rust_core::records::{ClipRect, Restore, Save};
use skia_rust_core::rect::Rect;
use std::cell::RefCell;
use std::rc::Rc;

type SaveClipRectRestore = Pattern<(Is<Save>, Is<ClipRect>, Is<Restore>)>;

// Port of: tests/RecordPatternTest.cpp#L19-L40 (chrome/m156)
def_test!(RecordPattern_Simple, |r| {
    let mut pattern = SaveClipRectRestore::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == 0);

    let recorder = RecordCanvas::new(&record, 1920, 1200);

    // Build up a save-clip-restore block.  The pattern will match only it's complete.
    recorder.save();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == 0);

    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == 0);

    recorder.restore();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) != 0);
    reporter_assert!(r, pattern.first::<Save>(&record.borrow()).is_some());
    reporter_assert!(r, pattern.second::<ClipRect>(&record.borrow()).is_some());
    reporter_assert!(r, pattern.third::<Restore>(&record.borrow()).is_some());
});

// Port of: tests/RecordPatternTest.cpp#L42-L63 (chrome/m156)
def_test!(RecordPattern_StartingIndex, |r| {
    let mut pattern = SaveClipRectRestore::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1200);

    // There will be two save-clipRect-restore blocks [0,3) and [3,6).
    for _ in 0..2 {
        recorder.save();
        recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
        recorder.restore();
    }

    // We should match only at 0 and 3.  Going over the length should fail gracefully.
    for i in 0..8 {
        if i == 0 || i == 3 {
            reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), i) == i + 3);
        } else {
            reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), i) == 0);
        }
    }
});

// Port of: tests/RecordPatternTest.cpp#L65-L78 (chrome/m156)
def_test!(RecordPattern_DontMatchSubsequences, |r| {
    let mut pattern = SaveClipRectRestore::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1200);

    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.draw_rect(Rect::from_wh(600.0, 300.0), &Paint::default());
    recorder.restore();

    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == 0);
});

// Port of: tests/RecordPatternTest.cpp#L80-L99 (chrome/m156)
def_test!(RecordPattern_Greedy, |r| {
    let mut pattern = Pattern::<(Is<Save>, Greedy<Is<ClipRect>>, Is<Restore>)>::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1200);
    let mut index = 0;

    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.restore();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), index) != 0);
    index += 3;

    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.clip_rect(Rect::from_wh(100.0, 100.0), None, None);
    recorder.restore();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), index) != 0);
});

// Port of: tests/RecordPatternTest.cpp#L101-L141 (chrome/m156)
def_test!(RecordPattern_Complex, |r| {
    let mut pattern = Pattern::<(
        Is<Save>,
        Greedy<Not<Or<(Is<Save>, Is<Restore>, IsDraw)>>>,
        Is<Restore>,
    )>::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1200);
    let mut start;
    let mut begin = 0;
    let mut end;

    start = record.borrow().count();
    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.restore();
    let count = record.borrow().count();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == count);
    end = start;
    reporter_assert!(
        r,
        pattern.search(&mut record.borrow_mut(), &mut begin, &mut end)
    );
    reporter_assert!(r, begin == start);
    reporter_assert!(r, end == record.borrow().count());

    start = record.borrow().count();
    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.draw_rect(Rect::from_wh(100.0, 3000.0), &Paint::default());
    recorder.restore();
    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), start) == 0);
    end = start;
    reporter_assert!(
        r,
        !pattern.search(&mut record.borrow_mut(), &mut begin, &mut end)
    );

    start = record.borrow().count();
    recorder.save();
    recorder.clip_rect(Rect::from_wh(300.0, 200.0), None, None);
    recorder.clip_rect(Rect::from_wh(100.0, 400.0), None, None);
    recorder.restore();
    let count = record.borrow().count();
    reporter_assert!(
        r,
        pattern.match_at(&mut record.borrow_mut(), start) == count
    );
    end = start;
    reporter_assert!(
        r,
        pattern.search(&mut record.borrow_mut(), &mut begin, &mut end)
    );
    reporter_assert!(r, begin == start);
    reporter_assert!(r, end == record.borrow().count());

    reporter_assert!(
        r,
        !pattern.search(&mut record.borrow_mut(), &mut begin, &mut end)
    );
});

// Port of: tests/RecordPatternTest.cpp#L143-L152 (chrome/m156)
def_test!(RecordPattern_SaveLayerIsNotADraw, |r| {
    let mut pattern = Pattern::<(IsDraw,)>::default();

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1200);
    recorder.save_layer(&skia_rust_core::canvas::SaveLayerRec::default());

    reporter_assert!(r, pattern.match_at(&mut record.borrow_mut(), 0) == 0);
});
