// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PictureTest.cpp (chrome/m156)
//
// Not ported yet:
// - `Picture`: its `test_typeface` needs `SkFont` and `drawString`; the rest of its steps are
//   covered by the other tests of the file.
// - `Picture_nested_draw_drawable`, `Picture_recursion_limit`: need `SkDrawable` (not ported).
// `ClipCountingCanvas` is not used by any test of the file and is not ported.

#![cfg(test)]

use crate::{Reporter, def_test, def_tier_test, reporter_assert};
use skia_rust_core::bbh_factory::{BBHFactory, BBoxHierarchy, RTreeFactory};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{
    Canvas, CanvasHooks, SaveLayerRec, SaveLayerStrategy, SrcRectConstraint,
};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::rect::rect_priv::make_largest;
use skia_rust_core::scalar::scalar_ceil_to_int;
use skia_rust_core::stream::{DynamicMemoryWStream, MemoryStream};
use skia_rust_raster::raster_canvas::RasterCanvas;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

// The counters of `SaveCountingCanvas`.
// Port of: tests/PictureTest.cpp#L110-L156 (chrome/m156)
#[derive(Default)]
#[allow(clippy::struct_field_names)] // fSaveCount, fSaveLayerCount, ... in the C++
struct SaveCounts {
    save_count: Cell<u32>,
    save_layer_count: Cell<u32>,
    save_behind_count: Cell<u32>,
    restore_count: Cell<u32>,
}

struct SaveCountingHooks {
    counts: Rc<SaveCounts>,
}

impl CanvasHooks for SaveCountingHooks {
    fn get_save_layer_strategy(&mut self, _rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        self.counts
            .save_layer_count
            .set(self.counts.save_layer_count.get() + 1);
        SaveLayerStrategy::FullLayer // INHERITED::getSaveLayerStrategy(rec)
    }

    fn will_save(&mut self) {
        self.counts.save_count.set(self.counts.save_count.get() + 1);
    }

    fn will_restore(&mut self) {
        self.counts
            .restore_count
            .set(self.counts.restore_count.get() + 1);
    }
}

/**
 * A canvas that records the number of saves, saveLayers and restores.
 */
struct SaveCountingCanvas {
    canvas: Canvas,
    counts: Rc<SaveCounts>,
}

impl Deref for SaveCountingCanvas {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl SaveCountingCanvas {
    fn new(width: i32, height: i32) -> Self {
        let canvas = Canvas::new_no_pixels((width, height), None).unwrap();
        let counts = Rc::new(SaveCounts::default());
        canvas.set_hooks(Some(Box::new(SaveCountingHooks {
            counts: Rc::clone(&counts),
        })));
        SaveCountingCanvas { canvas, counts }
    }

    fn get_save_count(&self) -> u32 {
        self.counts.save_count.get()
    }

    fn get_save_layer_count(&self) -> u32 {
        self.counts.save_layer_count.get()
    }

    #[allow(dead_code)] // `onDoSaveBehind` is not ported, so the count stays 0
    fn get_save_behind_count(&self) -> u32 {
        self.counts.save_behind_count.get()
    }

    fn get_restore_count(&self) -> u32 {
        self.counts.restore_count.get()
    }
}

// Port of: tests/PictureTest.cpp#L158-L170 (chrome/m156)
fn check_save_state(
    reporter: &mut Reporter,
    picture: &Picture,
    num_saves: u32,
    num_save_layers: u32,
    num_restores: u32,
) {
    let canvas = SaveCountingCanvas::new(
        scalar_ceil_to_int(picture.cull_rect().width()),
        scalar_ceil_to_int(picture.cull_rect().height()),
    );

    picture.playback(&canvas);

    // Optimizations may have removed these,
    // so expect to have seen no more than num{Saves,SaveLayers,Restores}.
    reporter_assert!(reporter, num_saves >= canvas.get_save_count());
    reporter_assert!(reporter, num_save_layers >= canvas.get_save_layer_count());
    reporter_assert!(reporter, num_restores >= canvas.get_restore_count());
}

// This class exists so SkPicture can friend it and give it access to
// the 'partialReplay' method.
// Port of: tests/PictureTest.cpp#L172-L184 (chrome/m156)
struct PictureRecorderReplayTester;

impl PictureRecorderReplayTester {
    fn copy(recorder: &PictureRecorder) -> Picture {
        let mut recorder2 = PictureRecorder::new();

        let canvas = recorder2.begin_recording(Rect::from_wh(10.0, 10.0), false);

        recorder.partial_replay(canvas);

        recorder2.finish_recording_as_picture(None).unwrap()
    }
}

// Port of: tests/PictureTest.cpp#L186-L196 (chrome/m156)
fn create_imbalance(canvas: &Canvas) {
    let clip_rect = Rect::from_wh(2.0, 2.0);
    let draw_rect = Rect::from_wh(10.0, 10.0);
    canvas.save();
    canvas.clip_rect(clip_rect, ClipOp::Intersect, false);
    canvas.translate((1.0, 1.0));
    let mut p = Paint::default();
    p.set_color(Color::GREEN);
    canvas.draw_rect(draw_rect, &p);
    // no restore
}

// This tests that replaying a potentially unbalanced picture into a canvas
// doesn't affect the canvas' save count or matrix/clip state.
// Port of: tests/PictureTest.cpp#L198-L219 (chrome/m156)
fn check_balance(reporter: &mut Reporter, picture: &Picture) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((4, 3), false);
    let canvas = Canvas::from_bitmap(&mut bm, None).unwrap();

    let before_save_count = canvas.save_count();

    let before_matrix = canvas.total_matrix();

    let before_clip = canvas.local_clip_bounds();

    canvas.draw_picture(picture, None, None);

    reporter_assert!(reporter, before_save_count == canvas.save_count());
    reporter_assert!(reporter, before_matrix == canvas.total_matrix());

    let after_clip = canvas.local_clip_bounds();

    reporter_assert!(reporter, after_clip == before_clip);
}

// Test out SkPictureRecorder::partialReplay
// Port of: tests/PictureTest.cpp#L221-L261 (chrome/m156)
def_test!(PictureRecorder_replay, |reporter| {
    // check save/saveLayer state
    {
        let mut recorder = PictureRecorder::new();

        recorder.begin_recording(Rect::from_wh(10.0, 10.0), false);

        // (`canvas` is fetched again with `recording_canvas()` where Skia keeps the pointer: the
        // recorder is borrowed for as long as the canvas returned by `begin_recording` lives.)
        recorder
            .recording_canvas()
            .unwrap()
            .save_layer(&SaveLayerRec::default());

        let copy = PictureRecorderReplayTester::copy(&recorder);

        // The extra save and restore comes from the Copy process.
        check_save_state(reporter, &copy, 2, 1, 3);

        recorder
            .recording_canvas()
            .unwrap()
            .save_layer(&SaveLayerRec::default());

        let final_ = recorder.finish_recording_as_picture(None).unwrap();

        check_save_state(reporter, &final_, 1, 2, 3);

        // The copy shouldn't pick up any operations added after it was made
        check_save_state(reporter, &copy, 2, 1, 3);
    }

    // Recreate the Android partialReplay test case
    {
        let mut recorder = PictureRecorder::new();

        recorder.begin_recording(Rect::from_wh(4.0, 3.0), false);
        create_imbalance(recorder.recording_canvas().unwrap());

        let _expected_save_count = recorder.recording_canvas().unwrap().save_count();

        let copy = PictureRecorderReplayTester::copy(&recorder);
        check_balance(reporter, &copy);

        // skia-rust: Skia's assertion is the assignment `expectedSaveCount = canvas->getSaveCount()`
        // (a typo for `==`), true whenever the save count is not 0.
        let expected_save_count = recorder.recording_canvas().unwrap().save_count();
        reporter_assert!(reporter, expected_save_count != 0);

        // End the recording of source to test the picture finalization
        // process isn't complicated by the partialReplay step
        let _final = recorder.finish_recording_as_picture(None).unwrap();
    }
});

// Port of: tests/PictureTest.cpp#L598-L614 (chrome/m156)
struct CountingBBH {
    search_calls: AtomicI32,
}

impl CountingBBH {
    fn new() -> Self {
        CountingBBH {
            search_calls: AtomicI32::new(0),
        }
    }
}

impl BBoxHierarchy for CountingBBH {
    fn search(&self, _query: &Rect, _results: &mut Vec<usize>) {
        self.search_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn insert(&self, _rects: &[Rect]) {}

    fn bytes_used(&self) -> usize {
        0
    }
}

// Port of: tests/PictureTest.cpp#L616-L624 (chrome/m156)
struct SpoonFedBBHFactory {
    bbh: Arc<dyn BBoxHierarchy>,
}

impl SpoonFedBBHFactory {
    fn new(bbh: Arc<dyn BBoxHierarchy>) -> Self {
        SpoonFedBBHFactory { bbh }
    }
}

impl BBHFactory for SpoonFedBBHFactory {
    fn make(&self) -> Option<Arc<dyn BBoxHierarchy>> {
        Some(Arc::clone(&self.bbh))
    }
}

// When the canvas clip covers the full picture, we don't need to call the BBH.
// Port of: tests/PictureTest.cpp#L634-L654 (chrome/m156)
def_test!(Picture_SkipBBH, |r| {
    let bound = Rect::from_wh(320.0, 240.0);

    let bbh = Arc::new(CountingBBH::new());
    let factory = SpoonFedBBHFactory::new(bbh.clone());

    let mut recorder = PictureRecorder::new();
    let c = recorder.begin_recording_with_factory(bound, Some(&factory));
    // Record a few ops so we don't hit a small- or empty- picture optimization.
    c.draw_rect(bound, &Paint::default());
    c.draw_rect(bound, &Paint::default());
    let picture = recorder.finish_recording_as_picture(None).unwrap();

    let big = Canvas::new_no_pixels((640, 480), None).unwrap();
    let small = Canvas::new_no_pixels((300, 200), None).unwrap();

    picture.playback(&big);
    reporter_assert!(r, bbh.search_calls.load(Ordering::Relaxed) == 0);

    picture.playback(&small);
    reporter_assert!(r, bbh.search_calls.load(Ordering::Relaxed) == 1);
});

// getRecordingCanvas() should return a SkCanvas when recording, null when not recording.
// Port of: tests/PictureTest.cpp#L690-L699 (chrome/m156)
def_test!(Picture_getRecordingCanvas, |r| {
    let mut rec = PictureRecorder::new();
    reporter_assert!(r, rec.recording_canvas().is_none());
    for _ in 0..3 {
        rec.begin_recording(Rect::from_wh(100.0, 100.0), false);
        reporter_assert!(r, rec.recording_canvas().is_some());
        rec.finish_recording_as_picture(None);
        reporter_assert!(r, rec.recording_canvas().is_none());
    }
});

// If we record bounded ops into a picture with a big cull and calculate the
// bounds of those ops, we should trim down the picture cull to the ops' bounds.
// If we're not using an SkBBH, we shouldn't change it.
// Port of: tests/PictureTest.cpp#L727-L740 (chrome/m156)
def_test!(Picture_UpdatedCull_1, |r| {
    let factory = RTreeFactory;
    let mut recorder = PictureRecorder::new();

    let canvas = recorder.begin_recording_with_factory(make_largest(), Some(&factory));
    canvas.draw_rect(Rect::from_wh(20.0, 20.0), &Paint::default());
    let pic = recorder.finish_recording_as_picture(None).unwrap();
    reporter_assert!(r, pic.cull_rect() == Rect::from_wh(20.0, 20.0));

    let canvas = recorder.begin_recording(make_largest(), false);
    canvas.draw_rect(Rect::from_wh(20.0, 20.0), &Paint::default());
    let pic = recorder.finish_recording_as_picture(None).unwrap();
    reporter_assert!(r, pic.cull_rect() == make_largest());
});

// Port of: tests/PictureTest.cpp#L741-L756 (chrome/m156)
def_test!(Picture_UpdatedCull_2, |r| {
    let factory = RTreeFactory;
    let mut recorder = PictureRecorder::new();

    let canvas = recorder.begin_recording_with_factory(make_largest(), Some(&factory));
    canvas.draw_rect(Rect::from_wh(20.0, 20.0), &Paint::default());
    canvas.draw_rect(Rect::from_wh(10.0, 40.0), &Paint::default());
    let pic = recorder.finish_recording_as_picture(None).unwrap();
    reporter_assert!(r, pic.cull_rect() == Rect::from_wh(20.0, 40.0));

    let canvas = recorder.begin_recording(make_largest(), false);
    canvas.draw_rect(Rect::from_wh(20.0, 20.0), &Paint::default());
    canvas.draw_rect(Rect::from_wh(10.0, 40.0), &Paint::default());
    let pic = recorder.finish_recording_as_picture(None).unwrap();
    reporter_assert!(r, pic.cull_rect() == make_largest());
});

// Port of: tests/PictureTest.cpp#L701-L721 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares the scalars with ==
    Picture_preserveCullRect,
    |r| {
        let mut recorder = PictureRecorder::new();
        let canvas = recorder.begin_recording(Rect::new(1.0, 2.0, 3.0, 4.0), false);
        canvas.clear(Color::CYAN);

        let picture = recorder.finish_recording_as_picture(None).unwrap();
        let mut wstream = DynamicMemoryWStream::new();
        // default SkSerialProcs here and SkDeserialProcs below are fine because we don't
        // have any image or typeface data to serialize.
        picture.serialize_into(&mut wstream, None);

        let mut rstream = MemoryStream::from_data(Some(wstream.detach_as_data()));
        let deserialized_picture = Picture::from_stream(&mut rstream, None);

        reporter_assert!(r, deserialized_picture.is_some());
        let cull = deserialized_picture.unwrap().cull_rect();
        reporter_assert!(r, cull.left == 1.0);
        reporter_assert!(r, cull.top == 2.0);
        reporter_assert!(r, cull.right == 3.0);
        reporter_assert!(r, cull.bottom == 4.0);
    }
);

// Port of: tests/PictureTest.cpp#L782-L794 (chrome/m156)
def_test!(Picture_empty_serial, |reporter| {
    let mut rec = PictureRecorder::new();
    rec.begin_recording(Rect::new(0.0, 0.0, 10.0, 10.0), false);
    let pic = rec.finish_recording_as_picture(None);
    reporter_assert!(reporter, pic.is_some());
    let pic = pic.unwrap();

    // explicitly testing the default SkSerialProcs here and SkDeserialProcs below.
    let data = pic.serialize(None);
    reporter_assert!(reporter, data.is_some());
    let data = data.unwrap();

    let pic2 = Picture::from_data(data.as_bytes(), None);
    reporter_assert!(reporter, pic2.is_some());
});

// Port of: tests/PictureTest.cpp#L758-L780 (chrome/m156)
def_test!(Placeholder, |r| {
    let cull = Rect::new(0.0, 0.0, 10.0, 20.0);

    // Each placeholder is unique.
    let p1 = Picture::new_placeholder(cull);
    let p2 = Picture::new_placeholder(cull);
    reporter_assert!(r, p1.cull_rect() == p2.cull_rect());
    reporter_assert!(r, p1.cull_rect() == cull);
    reporter_assert!(r, p1.unique_id() != p2.unique_id());

    // Placeholders are never unrolled by SkCanvas (while other small pictures may be).
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(cull, false);
    canvas.draw_picture(&p1, None, None);
    canvas.draw_picture(&p2, None, None);
    let pic = recorder.finish_recording_as_picture(None).unwrap();
    reporter_assert!(r, pic.approximate_op_count() == 2);

    // Any upper limit when recursing into nested placeholders is fine as long
    // as it doesn't overflow an int.
    reporter_assert!(r, pic.approximate_op_count_nested(true) >= 2);
    reporter_assert!(r, pic.approximate_op_count_nested(true) <= 10);
});

// Port of: tests/PictureTest.cpp#L797-L825 (chrome/m156)
def_test!(Picture_drawsNothing, |r| {
    // Tests that pic->cullRect().isEmpty() is a good way to test a picture
    // recorded with an R-tree draws nothing.
    struct Case {
        draws_nothing: bool,
        func: fn(&Canvas),
    }
    let cases = [
        Case {
            draws_nothing: true,
            func: |_c| {},
        },
        Case {
            draws_nothing: true,
            func: |c| {
                c.save();
                c.restore();
            },
        },
        Case {
            draws_nothing: true,
            func: |c| {
                c.save();
                c.clip_rect(Rect::new(0.0, 0.0, 5.0, 5.0), None, None);
                c.restore();
            },
        },
        Case {
            draws_nothing: true,
            func: |c| {
                c.clip_rect(Rect::new(0.0, 0.0, 5.0, 5.0), None, None);
            },
        },
        Case {
            draws_nothing: false,
            func: |c| {
                c.draw_rect(Rect::new(0.0, 0.0, 5.0, 5.0), &Paint::default());
            },
        },
        Case {
            draws_nothing: false,
            func: |c| {
                c.save();
                c.draw_rect(Rect::new(0.0, 0.0, 5.0, 5.0), &Paint::default());
                c.restore();
            },
        },
        Case {
            draws_nothing: false,
            func: |c| {
                c.draw_rect(Rect::new(0.0, 0.0, 5.0, 5.0), &Paint::default());
                c.draw_rect(Rect::new(5.0, 5.0, 10.0, 10.0), &Paint::default());
            },
        },
    ];

    for c in &cases {
        let mut rec = PictureRecorder::new();
        let factory = RTreeFactory;
        (c.func)(rec.begin_recording_with_factory(Rect::from_wh(10.0, 10.0), Some(&factory)));
        let pic = rec.finish_recording_as_picture(None).unwrap();

        reporter_assert!(r, pic.cull_rect().is_empty() == c.draws_nothing);
    }
});

// Port of: tests/PictureTest.cpp#L827-L867 (chrome/m156)
def_test!(Picture_emptyNestedPictureBug, |r| {
    let bounds = Rect::new(-5000.0, -5000.0, 5000.0, 5000.0);

    let mut recorder = PictureRecorder::new();
    let factory = RTreeFactory;

    // These three pictures should all draw the same but due to bugs they don't:
    //
    //   1) inner has enough content that all of its content
    //      falls outside the positive/positive quadrant,
    //      and it is recorded with an R-tree so we contract the cullRect to those bounds;
    //
    //   2) middle wraps inner,
    //      and it its recorded with an R-tree so we update middle's cullRect to inner's;
    //
    //   3) outer wraps inner,
    //      and notices that middle contains only one op, drawPicture(inner),
    //      so it plays middle back during recording rather than ref'ing middle,
    //      querying middle's R-tree with its SkCanvas' bounds* {0,0, 5000,5000},
    //      finding nothing to draw.
    //
    //  * The bug was that these bounds were not tracked as {-5000,-5000, 5000,5000}.
    {
        let canvas = recorder.begin_recording_with_factory(bounds, Some(&factory));
        canvas.translate((-100.0, -100.0));
        canvas.draw_rect(Rect::new(0.0, 0.0, 50.0, 50.0), &Paint::default());
    }
    let inner = recorder.finish_recording_as_picture(None).unwrap();

    recorder
        .begin_recording_with_factory(bounds, Some(&factory))
        .draw_picture(&inner, None, None);
    let middle = recorder.finish_recording_as_picture(None).unwrap();

    // This doesn't need &factory to reproduce the bug,
    // but it's nice to see we come up with the same {-100,-100, -50,-50} bounds.
    recorder
        .begin_recording_with_factory(bounds, Some(&factory))
        .draw_picture(&middle, None, None);
    let outer = recorder.finish_recording_as_picture(None).unwrap();

    reporter_assert!(
        r,
        inner.cull_rect() == Rect::new(-100.0, -100.0, -50.0, -50.0)
    );
    reporter_assert!(
        r,
        middle.cull_rect() == Rect::new(-100.0, -100.0, -50.0, -50.0)
    );
    reporter_assert!(
        r,
        outer.cull_rect() == Rect::new(-100.0, -100.0, -50.0, -50.0)
    ); // Used to fail.
});

// Port of: tests/PictureTest.cpp#L869-L893 (chrome/m156)
def_test!(Picture_fillsBBH, |r| {
    // Test empty (0 draws), mini (1 draw), and big (2+) pictures, making sure they fill the BBH.
    let rects = [
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Rect::new(20.0, 20.0, 40.0, 40.0),
    ];

    for n in 0..=2 {
        let factory = RTreeFactory;
        let mut rec = PictureRecorder::new();

        let bbh = factory.make().unwrap();

        let c =
            rec.begin_recording_with_bbh(Rect::new(0.0, 0.0, 100.0, 100.0), Some(Arc::clone(&bbh)));
        for rect in rects.iter().take(n) {
            c.draw_rect(rect, &Paint::default());
        }
        let _pic = rec.finish_recording_as_picture(None).unwrap();

        let mut results: Vec<usize> = Vec::new();
        bbh.search(&Rect::new(0.0, 0.0, 100.0, 100.0), &mut results);
        reporter_assert!(
            r,
            results.len() == n,
            "results.size() == {}, want {}\n",
            results.len(),
            n
        );
    }
});

// Port of: tests/PictureTest.cpp#L895-L934 (chrome/m156)
def_test!(Picture_nested_op_count, |r| {
    let make_pic = |n: i32, pic: Option<&Picture>| -> Picture {
        let mut rec = PictureRecorder::new();
        let c = rec.begin_recording(Rect::new(0.0, 0.0, 100.0, 100.0), false);
        for _ in 0..n {
            if let Some(pic) = pic {
                c.draw_picture(pic, None, None);
            } else {
                c.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &Paint::default());
            }
        }
        rec.finish_recording_as_picture(None).unwrap()
    };

    let mut check = |pic: &Picture, shallow: usize, nested: usize| {
        let s = pic.approximate_op_count_nested(false);
        let n = pic.approximate_op_count_nested(true);
        reporter_assert!(r, s == shallow);
        reporter_assert!(r, n == nested);
    };

    let leaf1 = make_pic(1, None);
    // Base picture with 1 drawRect op: 1 shallow, 1 nested.
    check(&leaf1, 1, 1);

    let leaf10 = make_pic(10, None);
    // Base picture with 10 drawRect ops: 10 shallow, 10 nested.
    check(&leaf10, 10, 10);

    // leaf1 has <= kMaxPictureOpsToUnrollInsteadOfRef (1) ops, so drawPicture unrolls/inlines
    // the drawRect directly instead of referencing leaf1: 1 shallow, 1 nested.
    check(&make_pic(1, Some(&leaf1)), 1, 1);
    // leaf10 exceeds the unroll limit (10 > 1), so 1 DrawPicture op is recorded:
    // 1 shallow; 1 (DrawPicture) + 10 (leaf10 ops) = 11 nested.
    check(&make_pic(1, Some(&leaf10)), 1, 11);
    // Each of the 10 drawPicture(leaf1) calls unrolls into 1 drawRect: 10 shallow, 10 nested.
    check(&make_pic(10, Some(&leaf1)), 10, 10);
    // 10 DrawPicture ops recorded, each holding 10 ops: 10 shallow; 10 * (1 + 10) = 110 nested.
    check(&make_pic(10, Some(&leaf10)), 10, 110);
});

// Port of: tests/PictureTest.cpp#L48-L54 (chrome/m156)
fn make_bm(bm: &mut Bitmap, w: i32, h: i32, color: Color, immutable: bool) {
    bm.alloc_n32_pixels((w, h), None);
    bm.erase_color(color);
    if immutable {
        bm.set_immutable();
    }
}

// Port of: tests/PictureTest.cpp#L535-L548 (chrome/m156)
fn draw_bitmaps(bitmap: &Bitmap, canvas: &Canvas) {
    let rect = Rect::new(5.0, 5.0, 8.0, 8.0);
    // (`drawImage(nullptr, ...)` returns without drawing.)
    let Some(img) = bitmap.as_image() else {
        return;
    };

    // Don't care what these record, as long as they're legal.
    canvas.draw_image(&img, (0.0, 0.0), None);
    canvas.draw_image_rect(
        &img,
        Some((&rect, SrcRectConstraint::Strict)),
        rect,
        &Paint::default(),
    );
    canvas.draw_image(&img, (1.0, 1.0), None); // drawSprite
}

// Port of: tests/PictureTest.cpp#L542-L547 (chrome/m156)
fn test_draw_bitmaps(canvas: &Canvas) {
    let mut empty = Bitmap::new();
    draw_bitmaps(&empty, canvas);
    let _ = empty.set_info(&ImageInfo::new_n32_premul((10, 10), None), 0);
    draw_bitmaps(&empty, canvas);
}

// Port of: tests/PictureTest.cpp#L549-L553 (chrome/m156)
def_tier_test!(Picture_EmptyBitmap, |_r| {
    let mut recorder = PictureRecorder::new();
    test_draw_bitmaps(recorder.begin_recording(Rect::from_wh(10.0, 10.0), false));
    let _picture = recorder.finish_recording_as_picture(None);
});

// Port of: tests/PictureTest.cpp#L555-L561 (chrome/m156)
def_tier_test!(Canvas_EmptyBitmap, |_r| {
    let mut dst = Bitmap::new();
    dst.alloc_n32_pixels((10, 10), None);
    let canvas = Canvas::from_bitmap(&mut dst, None).expect("a canvas");

    test_draw_bitmaps(&canvas);
});

// Port of: tests/PictureTest.cpp#L563-L608 (chrome/m156)
def_tier_test!(DontOptimizeSaveLayerDrawDrawRestore, |reporter| {
    // This test is from crbug.com/344987.
    // The commands are:
    //   saveLayer with paint that modifies alpha
    //     drawBitmapRect
    //     drawBitmapRect
    //   restore
    // The bug was that this structure was modified so that:
    //  - The saveLayer and restore were eliminated
    //  - The alpha was only applied to the first drawBitmapRectToRect

    // This test draws blue and red squares inside a 50% transparent
    // layer.  Both colours should show up muted.
    // When the bug is present, the red square (the second bitmap)
    // shows upwith full opacity.

    let mut blue_bm = Bitmap::new();
    make_bm(
        &mut blue_bm,
        100,
        100,
        Color::from_argb(255, 0, 0, 255),
        true,
    );
    let mut red_bm = Bitmap::new();
    make_bm(
        &mut red_bm,
        100,
        100,
        Color::from_argb(255, 255, 0, 0),
        true,
    );
    let mut semi_transparent = Paint::default();
    semi_transparent.set_alpha(0x80);

    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(100.0, 100.0), false);
    canvas.draw_color(Color::new(0), None);

    canvas.save_layer(&SaveLayerRec::default().paint(&semi_transparent));
    canvas.draw_image(blue_bm.as_image().expect("an image"), (25.0, 25.0), None);
    canvas.draw_image(red_bm.as_image().expect("an image"), (50.0, 50.0), None);
    canvas.restore();

    let picture = recorder
        .finish_recording_as_picture(None)
        .expect("a picture");

    // Now replay the picture back on another canvas
    // and check a couple of its pixels.
    let mut replay_bm = Bitmap::new();
    make_bm(&mut replay_bm, 100, 100, Color::BLACK, false);
    {
        let replay_canvas = Canvas::from_bitmap(&mut replay_bm, None).expect("a canvas");
        picture.playback(&replay_canvas);
    }

    // With the bug present, at (55, 55) we would get a fully opaque red
    // intead of a dark red.
    reporter_assert!(
        reporter,
        replay_bm.get_color((30, 30)) == Color::new(0xff00_0080)
    );
    reporter_assert!(
        reporter,
        replay_bm.get_color((55, 55)) == Color::new(0xff80_0000)
    );
});

// Port of: tests/PictureTest.cpp#L656-L684 (chrome/m156)
def_test!(Picture_BitmapLeak, |r| {
    let mut mut_bm = Bitmap::new();
    let mut immut = Bitmap::new();
    mut_bm.alloc_n32_pixels((300, 200), None);
    immut.alloc_n32_pixels((300, 200), None);
    immut.set_immutable();
    debug_assert!(!mut_bm.is_immutable());
    debug_assert!(immut.is_immutable());

    // No one can hold a ref on our pixels yet.
    reporter_assert!(r, mut_bm.pixel_ref_is_unique());
    reporter_assert!(r, immut.pixel_ref_is_unique());

    let pic = {
        // we want the recorder to go out of scope before our subsequent checks, so we
        // place it inside local braces.
        let mut rec = PictureRecorder::new();
        let canvas = rec.begin_recording(Rect::from_wh(1920.0, 1200.0), false);
        canvas.draw_image(mut_bm.as_image().expect("an image"), (0.0, 0.0), None);
        canvas.draw_image(immut.as_image().expect("an image"), (800.0, 600.0), None);
        rec.finish_recording_as_picture(None)
    };

    // The picture shares the immutable pixels but copies the mutable ones.
    reporter_assert!(r, mut_bm.pixel_ref_is_unique());
    reporter_assert!(r, !immut.pixel_ref_is_unique());

    // When the picture goes away, it's just our bitmaps holding the refs.
    drop(pic);
    reporter_assert!(r, mut_bm.pixel_ref_is_unique());
    reporter_assert!(r, immut.pixel_ref_is_unique());
});
