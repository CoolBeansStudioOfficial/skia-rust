// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Text blobs recorded into a picture draw the same pixels as the same blobs drawn directly
//! (docs/design/text.md §6, pictures; `SkRecordCanvas::onDrawTextBlob` and `SkRecords::DrawTextBlob`).
//! This is our own test, not a port: the Skia picture serialization it would need is not ported.

use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

const WIDTH: i32 = 240;
const WIDTH_F: f32 = 240.0;
const HEIGHT: i32 = 100;
const HEIGHT_F: f32 = 100.0;

/// The pixels of a raster surface after `draw`, which starts from white.
fn render(draw: impl FnOnce(&skia_rust_core::canvas::Canvas)) -> Vec<u8> {
    let info = ImageInfo::new_n32_premul((WIDTH, HEIGHT), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    surface.canvas().clear(Color::WHITE);
    draw(surface.canvas());
    let image = surface.image_snapshot().expect("a snapshot");
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(HEIGHT).unwrap_or(0)];
    assert!(image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    pixels
}

#[test]
fn text_blob_in_picture_matches_direct_draw() {
    let mut font = default_portable_font();
    font.set_size(40.0);
    let blob = TextBlob::from_str("Picture", &font).expect("the text has glyphs");
    let origin = (10.0, 60.0);

    let direct = render(|canvas| {
        canvas.draw_text_blob(&blob, origin, &Paint::default());
    });

    let mut recorder = PictureRecorder::new();
    let recording = recorder.begin_recording(Rect::from_wh(WIDTH_F, HEIGHT_F), false);
    recording.draw_text_blob(&blob, origin, &Paint::default());
    let picture = recorder
        .finish_recording_as_picture(None)
        .expect("a picture");
    let played = render(|canvas| {
        canvas.draw_picture(&picture, None, None);
    });

    // The text must have drawn something, or the comparison proves nothing.
    assert!(direct.iter().any(|&byte| byte != 0xFF));
    assert_eq!(direct, played);
}
