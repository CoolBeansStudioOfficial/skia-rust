// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Regions recorded in a picture (`CLIP_REGION` and `DRAW_REGION`), serialized and read back, draw
//! the same pixels as the calls drawn directly. This is our own test, not a port: the regions and
//! the clip ops around them are the ones `SkPictureRecord::onClipRegion` and `onDrawRegion` record.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};
use skia_rust_effects::flattenable::REGISTRY;
use skia_rust_raster::surfaces;

const SIZE: i32 = 32;
const SIZE_F: f32 = 32.0;

/// The pixels of `picture` drawn on a transparent raster surface.
fn render(picture: &Picture) -> Vec<u8> {
    let info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    picture.playback(surface.canvas());
    let image = surface.image_snapshot().expect("a snapshot");
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(SIZE).unwrap_or(0)];
    assert!(image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    pixels
}

/// A region of two rects, so that the complex-region encoding is used.
fn two_rect_region() -> Region {
    let mut region = Region::from_rect(IRect::new(2, 2, 12, 30));
    assert!(region.op_rect(IRect::new(20, 4, 30, 28), Op::Union));
    region
}

/// Records a picture that clips with a region, draws a paint through it, and draws a region.
fn picture() -> Picture {
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(SIZE_F, SIZE_F), false);
    let mut paint = Paint::default();
    paint.set_color(Color::from_argb(0xFF, 0x40, 0x90, 0x30));

    canvas.save();
    canvas.clip_region(&two_rect_region(), Some(ClipOp::Intersect));
    canvas.draw_paint(&paint);
    canvas.restore();

    let mut drawn = Paint::default();
    drawn.set_color(Color::from_argb(0x80, 0xC0, 0x20, 0x20));
    canvas.draw_region(&two_rect_region(), &drawn);
    recorder
        .finish_recording_as_picture(None)
        .expect("a picture")
}

#[test]
fn regions_in_picture_round_trip_matches_direct_draw() {
    let recorded = picture();
    let data = recorded
        .serialize(None)
        .expect("regions are encoded, so the picture serializes");
    let read_back = Picture::from_data_with_registry(data.as_bytes(), None, &REGISTRY)
        .expect("the serialized picture reads back");

    let direct = render(&recorded);
    // The comparison must not be between two empty drawings.
    assert!(direct.iter().any(|&byte| byte != 0));
    assert_eq!(direct, render(&read_back));
}

#[test]
fn clip_region_difference_round_trips() {
    let mut rec = PictureRecorder::new();
    let canvas = rec.begin_recording(Rect::from_wh(SIZE_F, SIZE_F), false);
    let mut paint = Paint::default();
    paint.set_color(Color::from_argb(0xFF, 0x10, 0x10, 0xF0));
    canvas.clip_region(
        &Region::from_rect(IRect::new(8, 8, 24, 24)),
        Some(ClipOp::Difference),
    );
    canvas.draw_paint(&paint);
    let recorded = rec.finish_recording_as_picture(None).expect("a picture");

    let data = recorded.serialize(None).expect("the picture serializes");
    let read_back = Picture::from_data_with_registry(data.as_bytes(), None, &REGISTRY)
        .expect("the serialized picture reads back");
    let direct = render(&recorded);
    assert!(direct.iter().any(|&byte| byte != 0));
    assert_eq!(direct, render(&read_back));
}
