// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecorderTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::canvas::{SaveLayerRec, SrcRectConstraint};
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::record::Record;
use skia_rust_core::record_canvas::{RecordCanvas, SharedRecord};
use skia_rust_core::records::{DrawImageRect, DrawRect, RecordKind, Type};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_MAX, SCALAR_MIN};
use skia_rust_core::shaders;
use skia_rust_raster::surfaces;
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

// Port of: tests/RecorderTest.cpp#L82-L119 (chrome/m156)
def_test!(Recorder_drawImage_takeReference, |reporter| {
    let image;
    {
        let mut surface =
            surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None)
                .expect("surface");
        surface.canvas().clear(Color::GREEN);
        image = surface.image_snapshot().expect("a snapshot");
    }

    {
        let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
        let recorder = RecordCanvas::new(&record, 100, 100);

        // DrawImage is supposed to take a reference
        recorder.draw_image_with_sampling_options(
            &image,
            (0.0, 0.0),
            SamplingOptions::default(),
            None,
        );
        reporter_assert!(reporter, !image.is_unique());

        let mut tally = Tally::default();
        tally.apply(&record.borrow());

        reporter_assert!(reporter, 1 == tally.count::<DrawImageRect>());
    }
    reporter_assert!(reporter, image.is_unique());

    {
        let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
        let recorder = RecordCanvas::new(&record, 100, 100);

        // DrawImageRect is supposed to take a reference
        recorder.draw_image_rect_with_sampling_options(
            &image,
            Some((&Rect::from_wh(100.0, 100.0), SrcRectConstraint::Fast)),
            Rect::from_wh(100.0, 100.0),
            SamplingOptions::default(),
            &Paint::default(),
        );
        reporter_assert!(reporter, !image.is_unique());

        let mut tally = Tally::default();
        tally.apply(&record.borrow());

        reporter_assert!(reporter, 1 == tally.count::<DrawImageRect>());
    }
    reporter_assert!(reporter, image.is_unique());
});
