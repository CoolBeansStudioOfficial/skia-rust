// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! An image drawn into a picture, serialized with an image procedure and read back with the
//! matching deserial procedure, draws the same pixels as the image drawn directly. This is our
//! own test, not a port: it covers the `IMAGE` section and `DRAW_IMAGE_RECT2` round trip, with raw
//! pixels as the serialized bytes (the Rust port has no encoder).

use std::sync::Arc;

use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::serial_procs::{DeserialProcs, SerialProcs};
use skia_rust_raster::surfaces;

const SIZE: i32 = 16;
const SIZE_F: f32 = 16.0;
const CANVAS: i32 = 40;
const CANVAS_F: f32 = 40.0;

/// The pixels of a raster surface after `draw`, which starts from white.
fn render(draw: impl FnOnce(&skia_rust_core::canvas::Canvas)) -> Vec<u8> {
    let info = ImageInfo::new_n32_premul((CANVAS, CANVAS), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    surface.canvas().clear(Color::WHITE);
    draw(surface.canvas());
    let image = surface.image_snapshot().expect("a snapshot");
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(CANVAS).unwrap_or(0)];
    assert!(image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    pixels
}

/// A 16x16 image with two colors, so that a wrong pixel order or offset shows.
fn source_image() -> Image {
    let info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    surface.canvas().clear(Color::GREEN);
    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    surface
        .canvas()
        .draw_rect(Rect::from_wh(SIZE_F / 2.0, SIZE_F / 2.0), &paint);
    surface.image_snapshot().expect("a snapshot")
}

#[test]
fn image_in_picture_round_trip_matches_direct_draw() {
    let image = source_image();
    let info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let row_bytes = info.min_row_bytes();
    let byte_count = row_bytes * usize::try_from(SIZE).unwrap_or(0);

    let direct = render(|canvas| {
        canvas.draw_image(&image, (5.0, 6.0), None);
    });

    let mut recorder = PictureRecorder::new();
    let recording = recorder.begin_recording(Rect::from_wh(CANVAS_F, CANVAS_F), false);
    recording.draw_image(&image, (5.0, 6.0), None);
    let picture = recorder
        .finish_recording_as_picture(None)
        .expect("a picture");

    // The raw pixels are the serialized bytes of the image.
    let serial_info = info.clone();
    let serial_procs = SerialProcs {
        image: Some(Arc::new(move |image: &Image| {
            let mut pixels = vec![0u8; byte_count];
            if !image.read_pixels(&serial_info, &mut pixels, row_bytes, (0, 0)) {
                return None;
            }
            Some(Data::new_from_vec(pixels))
        })),
        ..SerialProcs::default()
    };
    let data = picture
        .serialize(Some(&serial_procs))
        .expect("a serialized picture");

    let deserial_info = info.clone();
    let deserial_procs = DeserialProcs {
        image: Some(Arc::new(move |bytes: &[u8], _alpha| {
            images::raster_from_data(
                &deserial_info,
                Data::new_from_vec(bytes.to_vec()),
                row_bytes,
            )
        })),
        ..DeserialProcs::default()
    };
    let read_back =
        Picture::from_data(data.as_bytes(), Some(&deserial_procs)).expect("a read picture");
    let played = render(|canvas| {
        canvas.draw_picture(&read_back, None, None);
    });

    // The image must have drawn something, or the comparison proves nothing.
    assert!(direct.iter().any(|&byte| byte != 0xFF));
    assert_eq!(direct, played);
}
