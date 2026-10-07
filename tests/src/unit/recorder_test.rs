// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecorderTest.cpp (chrome/m156)
//
// Not ported yet: `Recorder_drawImage_takeReference` needs `SkImage` and a surface snapshot
// (`makeImageSnapshot`) and `SkCanvas::drawImage`, which are not ported (images, Phase 3).

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::paint::Paint;
use skia_rust_core::record::Record;
use skia_rust_core::record_canvas::{RecordCanvas, SharedRecord};
use skia_rust_core::records::{DrawRect, RecordKind, Type};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_MAX, SCALAR_MIN};
use skia_rust_core::shaders;
use std::cell::RefCell;
use std::rc::Rc;

// Tallies the types of commands it sees into a histogram.
// Port of: tests/RecorderTest.cpp#L28-L50 (chrome/m156)
#[derive(Default)]
struct Tally {
    histogram: std::collections::HashMap<Type, i32>,
}

impl Tally {
    fn count<T: RecordKind>(&self) -> i32 {
        self.histogram.get(&T::TYPE).copied().unwrap_or(0)
    }

    fn apply(&mut self, record: &Record) {
        for i in 0..record.count() {
            record.visit(i, |command| {
                *self.histogram.entry(command.record_type()).or_insert(0) += 1;
            });
        }
    }
}

// Port of: tests/RecorderTest.cpp#L52-L61 (chrome/m156)
def_test!(Recorder, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, 1920, 1080);

    recorder.draw_rect(Rect::from_wh(10.0, 10.0), &Paint::default());

    let mut tally = Tally::default();
    tally.apply(&record.borrow());
    reporter_assert!(r, 1 == tally.count::<DrawRect>());
});

// Regression test for leaking refs held by optional arguments.
// Port of: tests/RecorderTest.cpp#L63-L82 (chrome/m156)
def_test!(Recorder_RefLeaking, |r| {
    // We use SaveLayer to test:
    //   - its SkRect argument is optional and SkRect is POD.  Just testing that that works.
    //   - its SkPaint argument is optional and SkPaint is not POD.  The bug was here.

    let bounds = Rect::from_wh(320.0, 240.0);
    let mut paint = Paint::default();
    paint.set_shader(shaders::empty());

    reporter_assert!(r, paint.shader_ref().unwrap().is_unique());
    {
        let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
        let recorder = RecordCanvas::new(&record, 1920, 1080);
        recorder.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&paint));
        reporter_assert!(r, !paint.shader_ref().unwrap().is_unique());
    }
    reporter_assert!(r, paint.shader_ref().unwrap().is_unique());
});

// skbug.com/40042379
// Port of: tests/RecorderTest.cpp#L118-L127 (chrome/m156)
def_test!(Recorder_boundsOverflow, |reporter| {
    let big_bounds = Rect::new(SCALAR_MIN, SCALAR_MIN, SCALAR_MAX, SCALAR_MAX);

    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new_with_bounds(Some(&record), &big_bounds);
    reporter_assert!(
        reporter,
        recorder.image_info().width() > 0 && recorder.image_info().height() > 0
    );
});
