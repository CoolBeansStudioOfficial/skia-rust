// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordDrawTest.cpp (chrome/m156)
//
// Not ported yet:
// - `RecordDraw_SaveLayerAffectsClipBounds`, `RecordDraw_Metadata`: need
//   `SkImageFilters::DropShadow` (image filters, Phase 3).
// - `RecordDraw_EmptySaveLayerWithBackdropFilterAffectsCullRect`: needs `SkImageFilters::Blur`
//   (image filters, Phase 3).
// - `RecordDraw_drawImage`: needs `SkImage` (a surface snapshot) and an `SkCanvas` subclass
//   over a size (images, Phase 3).
// Excluded (manifest): `RecordDraw_BasicBounds` and `RecordDraw_SaveLayerBoundsAffectsClipBounds`
// are inside `#if 0` in Skia ("This would be nice, but we can't get it right today").

#![cfg(test)]

use super::record_test_utils::{assert_type, count_instances_of_type};
use crate::{def_test, reporter_assert};
use skia_rust_core::color::Color;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::AbortCallback;
use skia_rust_core::record::Record;
use skia_rust_core::record_canvas::{RecordCanvas, SharedRecord};
use skia_rust_core::record_draw::record_draw;
use skia_rust_core::records::{ClipRect, DrawPaint, DrawRect, Restore, Save, Scale, SetM44};
use skia_rust_core::rect::Rect;
use std::cell::RefCell;
use std::rc::Rc;

const W: i32 = 1920;
const H: i32 = 1080;

// Port of: tests/RecordDrawTest.cpp#L35-L42 (chrome/m156)
struct JustOneDraw {
    calls: i32,
}

impl JustOneDraw {
    fn new() -> Self {
        JustOneDraw { calls: 0 }
    }
}

impl AbortCallback for JustOneDraw {
    fn abort(&mut self) -> bool {
        let c = self.calls;
        self.calls += 1;
        c > 0
    }
}

// Port of: tests/RecordDrawTest.cpp#L44-L69 (chrome/m156)
def_test!(RecordDraw_LazySaves, |r| {
    // Record two commands.
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);

    reporter_assert!(r, 0 == record.borrow().count());
    recorder.save();
    reporter_assert!(r, 0 == record.borrow().count()); // the save was not recorded (yet)
    recorder.draw_color(Color::RED, None);
    reporter_assert!(r, 1 == record.borrow().count());
    recorder.scale((2.0, 2.0));
    reporter_assert!(r, 3 == record.borrow().count()); // now we see the save
    recorder.restore();
    reporter_assert!(r, 4 == record.borrow().count());

    assert_type::<DrawPaint>(r, &record.borrow(), 0);
    assert_type::<Save>(r, &record.borrow(), 1);
    assert_type::<Scale>(r, &record.borrow(), 2);
    assert_type::<Restore>(r, &record.borrow(), 3);

    recorder.save();
    recorder.save();
    recorder.restore();
    recorder.restore();
    reporter_assert!(r, 4 == record.borrow().count());
});

// Port of: tests/RecordDrawTest.cpp#L71-L86 (chrome/m156)
def_test!(RecordDraw_Abort, |r| {
    // Record two commands.
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);
    recorder.draw_rect(Rect::from_wh(200.0, 300.0), &Paint::default());
    recorder.clip_rect(Rect::from_wh(100.0, 200.0), None, None);

    let rerecord: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let canvas = RecordCanvas::new(&rerecord, W, H);

    let mut callback = JustOneDraw::new();
    record_draw(&record.borrow(), &canvas, None, Some(&mut callback));

    reporter_assert!(
        r,
        1 == count_instances_of_type::<DrawRect>(&rerecord.borrow())
    );
    reporter_assert!(
        r,
        0 == count_instances_of_type::<ClipRect>(&rerecord.borrow())
    );
});

// Port of: tests/RecordDrawTest.cpp#L88-L101 (chrome/m156)
def_test!(RecordDraw_Unbalanced, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);
    recorder.save(); // We won't balance this, but SkRecordDraw will for us.
    recorder.scale((2.0, 2.0));

    let rerecord: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let canvas = RecordCanvas::new(&rerecord, W, H);
    record_draw(&record.borrow(), &canvas, None, None);

    let save_count = count_instances_of_type::<Save>(&rerecord.borrow());
    let restore_count = count_instances_of_type::<Save>(&rerecord.borrow());
    reporter_assert!(r, save_count == restore_count);
});

// Port of: tests/RecordDrawTest.cpp#L103-L135 (chrome/m156)
def_test!(RecordDraw_SetMatrixClobber, |r| {
    // Set up an SkRecord that just scales by 2x,3x.
    let scale_record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let scale_canvas = RecordCanvas::new(&scale_record, W, H);
    let mut scale = Matrix::new_identity();
    scale.set_scale((2.0, 3.0), None);
    scale_canvas.set_matrix(&M44::from(&scale));

    // Set up an SkRecord with an initial +20, +20 translate.
    let translate_record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let translate_canvas = RecordCanvas::new(&translate_record, W, H);
    let mut translate = Matrix::new_identity();
    translate.set_translate((20.0, 20.0));
    translate_canvas.set_matrix(&M44::from(&translate));

    record_draw(&scale_record.borrow(), &translate_canvas, None, None);
    reporter_assert!(r, 4 == translate_record.borrow().count());
    assert_type::<SetM44>(r, &translate_record.borrow(), 0);
    assert_type::<Save>(r, &translate_record.borrow(), 1);
    assert_type::<SetM44>(r, &translate_record.borrow(), 2);
    assert_type::<Restore>(r, &translate_record.borrow(), 3);

    // When we look at translateRecord now, it should have its first +20,+20 translate,
    // then a 2x,3x scale that's been concatted with that +20,+20 translate.
    let record = translate_record.borrow();
    let set_matrix = assert_type::<SetM44>(r, &record, 0);
    reporter_assert!(
        r,
        set_matrix.is_some_and(|s| s.matrix == M44::from(&translate))
    );

    let set_matrix = assert_type::<SetM44>(r, &record, 2);
    let mut expected = scale.clone();
    expected.post_concat(&translate);
    reporter_assert!(
        r,
        set_matrix.is_some_and(|s| s.matrix == M44::from(&expected))
    );
});
