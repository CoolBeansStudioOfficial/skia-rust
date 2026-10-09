// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontHostStreamTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color::Color;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::graphics::purge_font_cache;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::stream::{DynamicMemoryWStream, MemoryStream};
use skia_rust_core::typeface::{SerializeBehavior, Typeface};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::{create_test_typeface, test_font_mgr};

use crate::{def_font_test, errorf, reporter_assert};

/// The side of the square the text is drawn into (`create(&bm, origRect)` with 64x64).
const SIDE: i32 = 64;

/// `drawBG` and the `drawString("A", 24, 32)` of the test, rendered to the pixels of a white
/// canvas. The raster canvas is N32, so the bytes are the same for any test run.
// Port of: tests/FontHostStreamTest.cpp#L20-L36 (chrome/m156), create and drawBG, with the
// drawString of the test body
fn render_a(font: &Font, paint: &Paint) -> Vec<u8> {
    let info = ImageInfo::new_n32_premul((SIDE, SIDE), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    surface.canvas().clear(Color::WHITE);
    surface.canvas().draw_str("A", (24.0, 32.0), font, paint);
    let image = surface.image_snapshot().expect("a snapshot");
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(SIDE).unwrap_or(0)];
    assert!(image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    pixels
}

// Port of: tests/FontHostStreamTest.cpp#L50-L124 (chrome/m156), FontHostStream
def_font_test!(FontHostStream, |reporter| {
    let mut paint = Paint::default();
    paint.set_color(Color::GRAY);

    let mut font = Font::from_size(
        create_test_typeface(Some("Georgia"), FontStyle::default()),
        30.0,
    );
    font.set_edging(Edging::Alias);

    let orig_pixels = render_a(&font, &paint);

    let mgr = test_font_mgr();
    let typeface: Typeface = font.typeface().clone();

    {
        // Test: origTypeface and streamTypeface from orig data draw the same
        let mut wstream = DynamicMemoryWStream::new();
        if !typeface.serialize_to(&mut wstream, SerializeBehavior::DoIncludeData) {
            errorf!(reporter, "serialize typeface");
            return;
        }
        let data = wstream.detach_as_data();
        let mut stream = MemoryStream::make_copy(data.as_bytes());
        let Some(deserialized_typeface) =
            Typeface::make_deserialize(&mut *stream, Some(&mgr), None)
        else {
            reporter_assert!(reporter, false, "the typeface deserializes");
            return;
        };

        let (_desc, must_serialize_data) = deserialized_typeface.font_descriptor();
        reporter_assert!(reporter, must_serialize_data);

        font.set_typeface(Some(deserialized_typeface));
        let deserialized_pixels = render_a(&font, &paint);
        reporter_assert!(reporter, deserialized_pixels == orig_pixels);
    }

    {
        let Some((font_stream, _ttc_index)) = typeface.open_stream() else {
            reporter_assert!(reporter, false, "the typeface has a stream");
            return;
        };

        let Some(stream_typeface) = mgr.make_from_stream(Some(font_stream), 0) else {
            // TODO (C++): enable assert after SkTypeface::MakeFromStream uses factories
            return;
        };

        let (_desc, must_serialize_data) = stream_typeface.font_descriptor();
        reporter_assert!(reporter, must_serialize_data);

        font.set_typeface(Some(stream_typeface));
        let stream_pixels = render_a(&font, &paint);
        reporter_assert!(reporter, stream_pixels == orig_pixels);
    }

    // Make sure the typeface is deleted and removed.
    purge_font_cache();
});
