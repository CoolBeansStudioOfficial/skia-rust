// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PictureBBHTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::bbh_factory::{BBHFactory, RTreeFactory};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: tests/PictureBBHTest.cpp#L21-L64 (chrome/m156)
struct PictureBBHTestBase {
    result_bitmap: Bitmap,
    picture_width: i32,
    picture_height: i32,
}

impl PictureBBHTestBase {
    fn new(
        playback_width: i32,
        playback_height: i32,
        record_width: i32,
        record_height: i32,
    ) -> Self {
        let mut result_bitmap = Bitmap::new();
        result_bitmap.alloc_n32_pixels((playback_width, playback_height), None);
        PictureBBHTestBase {
            result_bitmap,
            picture_width: record_width,
            picture_height: record_height,
        }
    }

    fn run(&mut self, reporter: &mut Reporter, do_test: fn(&Canvas, &Canvas)) {
        // No BBH
        self.run_with(None, reporter, do_test);

        // With an R-Tree
        let rtree_factory = RTreeFactory;
        self.run_with(Some(&rtree_factory), reporter, do_test);
    }

    fn run_with(
        &mut self,
        factory: Option<&dyn BBHFactory>,
        reporter: &mut Reporter,
        do_test: fn(&Canvas, &Canvas),
    ) {
        {
            // The canvas owns the bitmap while it draws and gives it back when it drops.
            let playback_canvas = Canvas::from_bitmap(&mut self.result_bitmap, None).unwrap();
            playback_canvas.clear(Color::GREEN);
            let mut recorder = PictureRecorder::new();
            #[allow(clippy::cast_precision_loss)] // SkIntToScalar
            let bounds = Rect::from_wh(self.picture_width as f32, self.picture_height as f32);
            let record_canvas = recorder.begin_recording_with_factory(bounds, factory);
            do_test(&playback_canvas, record_canvas);
            let picture = recorder.finish_recording_as_picture(None).unwrap();
            playback_canvas.draw_picture(&picture, None, None);
        }
        reporter_assert!(
            reporter,
            Color::GREEN == self.result_bitmap.get_color((0, 0))
        );
    }
}

// Test to verify the playback of an empty picture
//
// Port of: tests/PictureBBHTest.cpp#L66-L74 (chrome/m156)
struct DrawEmptyPictureBBHTest {
    base: PictureBBHTestBase,
}

impl DrawEmptyPictureBBHTest {
    fn new() -> Self {
        DrawEmptyPictureBBHTest {
            base: PictureBBHTestBase::new(2, 2, 1, 1),
        }
    }

    fn do_test(_playback_canvas: &Canvas, _recording_canvas: &Canvas) {}

    fn run(&mut self, reporter: &mut Reporter) {
        self.base.run(reporter, Self::do_test);
    }
}

// Test to verify the playback of a picture into a canvas that has
// an empty clip.
//
// Port of: tests/PictureBBHTest.cpp#L76-L88 (chrome/m156)
struct EmptyClipPictureBBHTest {
    base: PictureBBHTestBase,
}

impl EmptyClipPictureBBHTest {
    fn new() -> Self {
        EmptyClipPictureBBHTest {
            base: PictureBBHTestBase::new(2, 2, 3, 3),
        }
    }

    fn do_test(playback_canvas: &Canvas, recording_canvas: &Canvas) {
        // intersect with out of bounds rect -> empty clip.
        playback_canvas.clip_rect(Rect::from_xywh(10.0, 10.0, 1.0, 1.0), None, None);
        let paint = Paint::default();
        recording_canvas.draw_rect(Rect::from_wh(3.0, 3.0), &paint);
    }

    fn run(&mut self, reporter: &mut Reporter) {
        self.base.run(reporter, Self::do_test);
    }
}

// Port of: tests/PictureBBHTest.cpp#L90-L97 (chrome/m156)
def_test!(PictureBBH, |reporter| {
    let mut empty_picture_test = DrawEmptyPictureBBHTest::new();
    empty_picture_test.run(reporter);

    let mut empty_clip_picture_test = EmptyClipPictureBBHTest::new();
    empty_clip_picture_test.run(reporter);
});

// Port of: tests/PictureBBHTest.cpp#L99-L127 (chrome/m156)
def_test!(PictureNegativeSpace, |r| {
    let factory = RTreeFactory;
    let mut recorder = PictureRecorder::new();

    let cull = Rect::new(-200.0, -200.0, 200.0, 200.0);

    {
        let bbh = factory.make();
        let canvas = recorder.begin_recording_with_bbh(cull, bbh);
        canvas.save();
        canvas.clip_rect(cull, None, None);
        canvas.draw_rect(Rect::new(-20.0, -20.0, -10.0, -10.0), &Paint::default());
        canvas.draw_rect(Rect::new(-20.0, -20.0, -10.0, -10.0), &Paint::default());
        canvas.restore();
        let pic = recorder.finish_recording_as_picture(None).unwrap();
        reporter_assert!(r, pic.approximate_op_count() == 5);
        reporter_assert!(r, pic.cull_rect() == Rect::new(-20.0, -20.0, -10.0, -10.0));
    }

    {
        let canvas = recorder.begin_recording_with_factory(cull, Some(&factory));
        canvas.clip_rect(cull, None, None);
        canvas.draw_rect(Rect::new(-20.0, -20.0, -10.0, -10.0), &Paint::default());
        canvas.draw_rect(Rect::new(-20.0, -20.0, -10.0, -10.0), &Paint::default());
        let pic = recorder.finish_recording_as_picture(None).unwrap();
        reporter_assert!(r, pic.approximate_op_count() == 3);
        reporter_assert!(r, pic.cull_rect() == Rect::new(-20.0, -20.0, -10.0, -10.0));
    }
});
