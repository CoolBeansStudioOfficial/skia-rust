// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RecordingXfermodeTest.cpp (chrome/m156)

#![cfg(test)]

use std::fmt::Write as _;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

use crate::{def_test, reporter_assert};

// Verify that replay of a recording into a clipped canvas
// produces the correct bitmap.
// This arose from http://crbug.com/401593 which has
// https://code.google.com/p/skia/issues/detail?id=1291 as its root cause.

// Port of: tests/RecordingXfermodeTest.cpp#L20-L60 (chrome/m156), Drawer
struct Drawer {
    image_info: ImageInfo,
    circle_image: Image,
}

impl Drawer {
    // Port of: tests/RecordingXfermodeTest.cpp#L22-L30 (chrome/m156), Drawer::Drawer
    fn new() -> Self {
        let mut surf = surfaces::raster_n32_premul((100, 100)).expect("surface");
        surf.canvas().clear(0xffff_ffff_u32);
        let mut circle_paint = Paint::default();
        circle_paint.set_color(Color::new(0xff00_0000));
        surf.canvas()
            .draw_circle(Point::new(50.0, 50.0), 50.0, &circle_paint);
        let circle_image = surf.image_snapshot().expect("snapshot");
        Drawer {
            image_info: ImageInfo::new_n32_premul((200, 100), None),
            circle_image,
        }
    }

    // Port of: tests/RecordingXfermodeTest.cpp#L32 (chrome/m156), Drawer::imageInfo
    fn image_info(&self) -> &ImageInfo {
        &self.image_info
    }

    // Port of: tests/RecordingXfermodeTest.cpp#L34-L56 (chrome/m156), Drawer::draw
    // SkIntToScalar of the image size is an int-to-float cast, exact for these sizes.
    #[allow(clippy::cast_precision_loss)]
    fn draw(&self, canvas: &Canvas, clip_rect: &Rect, mode: BlendMode) {
        let mut green_paint = Paint::default();
        green_paint.set_color(Color::new(0xff00_8000));
        let mut black_paint = Paint::default();
        black_paint.set_color(Color::new(0xff00_0000));
        let mut white_paint = Paint::default();
        white_paint.set_color(Color::new(0xffff_ffff));
        let mut layer_paint = Paint::default();
        layer_paint.set_color(Color::new(0xff00_0000));
        layer_paint.set_blend_mode(mode);
        let canvas_rect = Rect::from_wh(
            self.image_info.width() as f32,
            self.image_info.height() as f32,
        );

        canvas.clip_rect(clip_rect, None, None);
        canvas.clear(0xff00_0000_u32);

        canvas.save_layer(&SaveLayerRec::default().paint(&black_paint));
        canvas.draw_rect(canvas_rect, &green_paint);
        canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
        canvas.draw_image_rect_with_sampling_options(
            &self.circle_image,
            None,
            Rect::from_xywh(20.0, 20.0, 60.0, 60.0),
            SamplingOptions::default(),
            &black_paint,
        );
        canvas.restore();
        canvas.restore();
    }
}

// Port of: tests/RecordingXfermodeTest.cpp#L62-L68 (chrome/m156), RecordingStrategy
trait RecordingStrategy {
    fn record_and_replay(&mut self, drawer: &Drawer, into_clip: &Rect, mode: BlendMode) -> &Bitmap;
}

// Port of: tests/RecordingXfermodeTest.cpp#L70-L86 (chrome/m156), BitmapBackedCanvasStrategy
// This version just draws into a bitmap-backed canvas.
struct BitmapBackedCanvasStrategy {
    bitmap: Bitmap,
}

impl BitmapBackedCanvasStrategy {
    // Port of: tests/RecordingXfermodeTest.cpp#L73-L75 (chrome/m156), BitmapBackedCanvasStrategy
    fn new(image_info: &ImageInfo) -> Self {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(image_info, None);
        BitmapBackedCanvasStrategy { bitmap }
    }
}

impl RecordingStrategy for BitmapBackedCanvasStrategy {
    // Port of: tests/RecordingXfermodeTest.cpp#L77-L85 (chrome/m156), recordAndReplay
    fn record_and_replay(&mut self, drawer: &Drawer, into_clip: &Rect, mode: BlendMode) -> &Bitmap {
        {
            let canvas = Canvas::from_bitmap(&mut self.bitmap, None).expect("canvas");
            canvas.clear(0xffff_ffff_u32);
            // Note that the scene is drawn just into the clipped region!
            canvas.clip_rect(into_clip, None, None);
            drawer.draw(&canvas, into_clip, mode); // Shouild be canvas-wide...
        }
        &self.bitmap
    }
}

// Port of: tests/RecordingXfermodeTest.cpp#L88-L119 (chrome/m156), PictureStrategy
// This version draws the entire scene into an SkPictureRecorder.
// Then it then replays the scene through a clip rectangle.
// This backend proved to be buggy.
struct PictureStrategy {
    bitmap: Bitmap,
    width: i32,
    height: i32,
}

impl PictureStrategy {
    // Port of: tests/RecordingXfermodeTest.cpp#L93-L97 (chrome/m156), PictureStrategy
    fn new(image_info: &ImageInfo) -> Self {
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(image_info, None);
        PictureStrategy {
            bitmap,
            width: image_info.width(),
            height: image_info.height(),
        }
    }
}

impl RecordingStrategy for PictureStrategy {
    // Port of: tests/RecordingXfermodeTest.cpp#L99-L114 (chrome/m156), recordAndReplay
    // SkIntToScalar of the picture size is an int-to-float cast, exact for these sizes.
    #[allow(clippy::cast_precision_loss)]
    fn record_and_replay(&mut self, drawer: &Drawer, into_clip: &Rect, mode: BlendMode) -> &Bitmap {
        let mut recorder = PictureRecorder::new();
        let canvas_rect = Rect::from_wh(self.width as f32, self.height as f32);
        let canvas = recorder.begin_recording_with_factory(
            Rect::from_wh(self.width as f32, self.height as f32),
            Some(&skia_rust_core::bbh_factory::RTreeFactory),
        );
        drawer.draw(canvas, &canvas_rect, mode);
        let picture: Picture = recorder
            .finish_recording_as_picture(None)
            .expect("a picture");

        {
            let replay_canvas = Canvas::from_bitmap(&mut self.bitmap, None).expect("canvas");
            replay_canvas.clear(0xffff_ffff_u32);
            replay_canvas.clip_rect(into_clip, None, None);
            picture.playback(&replay_canvas);
        }
        &self.bitmap
    }
}

// Port of: tests/RecordingXfermodeTest.cpp#L121-L182 (chrome/m156), SkRecordingAccuracyXfermode
def_test!(SkRecordingAccuracyXfermode, |reporter| {
    let drawer = Drawer::new();

    let mut golden = BitmapBackedCanvasStrategy::new(drawer.image_info());
    let mut picture = PictureStrategy::new(drawer.image_info());

    let mut num_errors: u32 = 0;
    let mut errors = String::new();

    for mode in BlendMode::VALUES {
        let clip = Rect::from_xywh(100.0, 0.0, 100.0, 100.0);

        let golden_bm = golden.record_and_replay(&drawer, &clip, mode);
        let golden_px = golden_bm.pixmap();
        let golden_bytes = golden_px.addr().unwrap_or(&[]);
        let golden_size = golden_bm.compute_byte_size();

        let picture_bm = picture.record_and_replay(&drawer, &clip, mode);
        let picture_px = picture_bm.pixmap();
        let picture_bytes = picture_px.addr().unwrap_or(&[]);
        let picture_size = picture_bm.compute_byte_size();

        reporter_assert!(reporter, golden_size == picture_size);

        // The pixel arrays should match.
        if golden_bytes.get(..golden_size) != picture_bytes.get(..picture_size) {
            num_errors += 1;
            let _ = writeln!(
                errors,
                "For SkBlendMode {} {}:    SkPictureRecorder bitmap is wrong",
                mode as i32,
                mode.name()
            );
        }
    }

    reporter_assert!(reporter, num_errors == 0, "{}", errors);
});
