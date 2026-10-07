// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordOptsTest.cpp (chrome/m156)
//
// Not ported yet:
// - `RecordOpts_NoopSaveLayerDrawRestore`: its last case needs `SkImageFilters::Blur` (image
//   filters, Phase 3) for the backdrop.
// - `RecordOpts_MergeSvgOpacityAndFilterLayers`: needs `SkImageFilters::Blur` and
//   `SkImageFilters::Picture` (image filters, Phase 3) and `SkColorFilters::Blend` (color filters,
//   Phase 3); its module is also `svg`.

#![cfg(test)]

use super::record_test_utils::{assert_type, count_instances_of_type};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::record::Record;
use skia_rust_core::record_canvas::{RecordCanvas, SharedRecord};
use skia_rust_core::record_opts::record_noop_save_restores;
use skia_rust_core::records::{DrawRect, NoOp, Restore, Save, SaveLayer};
use skia_rust_core::rect::Rect;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;
use std::cell::RefCell;
use std::rc::Rc;

const W: i32 = 1920;
const H: i32 = 1080;

// Port of: tests/RecordOptsTest.cpp#L33-L47 (chrome/m156)
def_test!(RecordOpts_NoopDraw, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);

    recorder.draw_rect(Rect::from_wh(200.0, 200.0), &Paint::default());
    recorder.draw_rect(Rect::from_wh(300.0, 300.0), &Paint::default());
    recorder.draw_rect(Rect::from_wh(100.0, 100.0), &Paint::default());

    record.borrow_mut().replace(1, NoOp); // NoOps should be allowed.

    record_noop_save_restores(&mut record.borrow_mut());

    reporter_assert!(
        r,
        2 == count_instances_of_type::<DrawRect>(&record.borrow())
    );
});

// Port of: tests/RecordOptsTest.cpp#L49-L60 (chrome/m156)
def_test!(RecordOpts_SingleNoopSaveRestore, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);

    recorder.save();
    recorder.clip_rect(Rect::from_wh(200.0, 200.0), None, None);
    recorder.restore();

    record_noop_save_restores(&mut record.borrow_mut());
    for i in 0..3 {
        assert_type::<NoOp>(r, &record.borrow(), i);
    }
});

// Port of: tests/RecordOptsTest.cpp#L62-L84 (chrome/m156)
def_test!(RecordOpts_NoopSaveRestores, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);

    // The second pass will clean up this pair after the first pass noops all the innards.
    recorder.save();
    // A simple pointless pair of save/restore.
    recorder.save();
    recorder.restore();

    // As long as we don't draw in there, everything is a noop.
    recorder.save();
    recorder.clip_rect(Rect::from_wh(200.0, 200.0), None, None);
    recorder.clip_rect(Rect::from_wh(100.0, 100.0), None, None);
    recorder.restore();
    recorder.restore();

    record_noop_save_restores(&mut record.borrow_mut());
    let count = record.borrow().count();
    for index in 0..count {
        assert_type::<NoOp>(r, &record.borrow(), index);
    }
});

// Port of: tests/RecordOptsTest.cpp#L86-L111 (chrome/m156)
def_test!(RecordOpts_SaveSaveLayerRestoreRestore, |r| {
    let record: SharedRecord = Rc::new(RefCell::new(Record::new()));
    let recorder = RecordCanvas::new(&record, W, H);

    // A previous bug NoOp'd away the first 3 commands.
    recorder.save();
    recorder.save_layer(&SaveLayerRec::default());
    recorder.restore();
    recorder.restore();

    record_noop_save_restores(&mut record.borrow_mut());
    let count = record.borrow().count();
    match count {
        4 => {
            assert_type::<Save>(r, &record.borrow(), 0);
            assert_type::<SaveLayer>(r, &record.borrow(), 1);
            assert_type::<Restore>(r, &record.borrow(), 2);
            assert_type::<Restore>(r, &record.borrow(), 3);
        }
        2 => {
            assert_type::<SaveLayer>(r, &record.borrow(), 0);
            assert_type::<Restore>(r, &record.borrow(), 1);
        }
        0 => {}
        _ => {
            reporter_assert!(r, false);
        }
    }
});

// Port of: tests/RecordOptsTest.cpp#L113-L128 (chrome/m156)
#[allow(dead_code)] // used by the tests that are not ported yet (see the header)
fn assert_savelayer_restore(
    r: &mut Reporter,
    record: &mut Record,
    i: usize,
    should_be_noped: bool,
) {
    skia_rust_core::record_opts::record_noop_save_layer_draw_restores(record);
    if should_be_noped {
        assert_type::<NoOp>(r, record, i);
        assert_type::<NoOp>(r, record, i + 1);
    } else {
        assert_type::<SaveLayer>(r, record, i);
        assert_type::<Restore>(r, record, i + 1);
    }
}

// Port of: tests/RecordOptsTest.cpp#L130-L145 (chrome/m156)
#[allow(dead_code)] // used by the tests that are not ported yet (see the header)
fn assert_savelayer_draw_restore(
    r: &mut Reporter,
    record: &mut Record,
    i: usize,
    should_be_noped: bool,
) {
    skia_rust_core::record_opts::record_noop_save_layer_draw_restores(record);
    if should_be_noped {
        assert_type::<NoOp>(r, record, i);
        assert_type::<NoOp>(r, record, i + 2);
    } else {
        assert_type::<SaveLayer>(r, record, i);
        assert_type::<Restore>(r, record, i + 2);
    }
}

// Port of: tests/RecordOptsTest.cpp#L333-L346 (chrome/m156)
fn do_draw(canvas: &skia_rust_core::canvas::Canvas, color: Color, do_layer: bool) {
    canvas.draw_color(Color::WHITE, None);

    let mut p = Paint::default();
    p.set_color(color);

    if do_layer {
        canvas.save_layer(&SaveLayerRec::default());
        p.set_blend_mode(BlendMode::Src);
        canvas.draw_paint(&p);
        canvas.restore();
    } else {
        canvas.draw_paint(&p);
    }
}

// Port of: tests/RecordOptsTest.cpp#L348-L354 (chrome/m156)
fn is_equal(a: &mut Surface<'_>, b: &mut Surface<'_>) -> bool {
    let info = ImageInfo::new_n32_premul((1, 1), None);
    let mut ca = [0u8; 4];
    let mut cb = [0u8; 4];
    a.read_pixels(&info, &mut ca, 4, (0, 0));
    b.read_pixels(&info, &mut cb, 4, (0, 0));
    ca == cb
}

// Test drawing w/ and w/o a simple layer (no bounds or paint), so see that drawing ops
// that *should* draw the same in fact do.
//
// Perform this test twice : once directly, and once via a picture
//
// Port of: tests/RecordOptsTest.cpp#L356-L388 (chrome/m156)
fn do_savelayer_srcmode(r: &mut Reporter, color: Color) {
    for do_picture in 0..=1 {
        let mut surf0 = surfaces::raster_n32_premul((10, 10)).unwrap();
        let mut surf1 = surfaces::raster_n32_premul((10, 10)).unwrap();

        let mut rec0 = PictureRecorder::new();
        let mut rec1 = PictureRecorder::new();
        if do_picture != 0 {
            rec0.begin_recording(Rect::from_wh(10.0, 10.0), false);
            rec1.begin_recording(Rect::from_wh(10.0, 10.0), false);

            do_draw(rec0.recording_canvas().unwrap(), color, false);
            do_draw(rec1.recording_canvas().unwrap(), color, true);

            surf0.canvas().draw_picture(
                rec0.finish_recording_as_picture(None).unwrap(),
                None,
                None,
            );
            surf1.canvas().draw_picture(
                rec1.finish_recording_as_picture(None).unwrap(),
                None,
                None,
            );
        } else {
            do_draw(surf0.canvas(), color, false);
            do_draw(surf1.canvas(), color, true);
        }

        // we replicate the assert so we can see which line is reported if there is a failure
        if do_picture != 0 {
            reporter_assert!(r, is_equal(&mut surf0, &mut surf1));
        } else {
            reporter_assert!(r, is_equal(&mut surf0, &mut surf1));
        }
    }
}

// Port of: tests/RecordOptsTest.cpp#L390-L392 (chrome/m156)
def_test!(savelayer_srcmode_opaque, |r| {
    do_savelayer_srcmode(r, Color::RED);
});

// Port of: tests/RecordOptsTest.cpp#L394-L396 (chrome/m156)
def_test!(savelayer_srcmode_alpha, |r| {
    do_savelayer_srcmode(r, Color::new(0x80FF_0000));
});
