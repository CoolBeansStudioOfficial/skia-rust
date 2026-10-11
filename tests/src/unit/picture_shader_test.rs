// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PictureShaderTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::PoisonError;

use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_priv;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::resource_cache::{Rec, ResourceCache};
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_raster::surfaces;

use crate::{def_test, reporter_assert};

// Port of: tests/PictureShaderTest.cpp#L26-L59 (chrome/m156), the `makePicture` lambda
fn make_picture() -> Picture {
    let mut recorder = PictureRecorder::new();
    recorder
        .begin_recording(Rect::from_wh(100.0, 100.0), false)
        .draw_color(Color::GREEN, None);
    recorder
        .finish_recording_as_picture(None)
        .expect("a picture")
}

// Test that the SkPictureShader cache is purged on shader deletion.
// Port of: tests/PictureShaderTest.cpp#L26-L59 (chrome/m156), PictureShader_caching
def_test!(PictureShader_caching, |reporter| {
    let picture = make_picture();
    reporter_assert!(reporter, picture.unique());

    let mut surface = surfaces::raster_n32_premul((100, 100)).expect("a raster surface");

    {
        let mut paint = Paint::default();
        paint.set_shader(picture.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            FilterMode::Nearest,
            None,
            None,
        ));
        surface.canvas().draw_paint(&paint);

        // We should have about 3 refs by now: local + shader + shader cache.
        reporter_assert!(reporter, !picture.unique());
    }

    // Draw another picture shader to have a chance to purge.
    {
        let mut paint = Paint::default();
        paint.set_shader(make_picture().to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            FilterMode::Nearest,
            None,
            None,
        ));
        surface.canvas().draw_paint(&paint);
    }

    // All but the local ref should be gone now.
    reporter_assert!(reporter, picture.unique());
});

// Counts the cache entries whose shared ID is `shared_id`.
fn count_entries(shared_id: u64) -> usize {
    let mut count = 0;
    ResourceCache::global()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .visit_all(|rec: &dyn Rec| {
            if rec.key().shared_id() == shared_id {
                count += 1;
            }
        });
    count
}

/*
 *  Check caching of picture-shaders
 *  - we do cache the underlying image (i.e. there is a cache entry)
 *  - there is only 1 entry, even with differing tile modes
 *  - after deleting the picture, the cache entry is purged
 */
// Port of: tests/PictureShaderTest.cpp#L67-L119 (chrome/m156), PictureShader_caching2
def_test!(PictureShader_caching2, |reporter| {
    let picture = make_picture();
    reporter_assert!(reporter, picture.unique());

    let shared_id = picture_priv::make_shared_id(picture.unique_id());

    reporter_assert!(reporter, count_entries(shared_id) == 0);

    // Draw with a view variants of picture-shaders that all use the same picture.
    // Only expect 1 cache entry for all (since same CTM for all).
    let mut surface = surfaces::raster_n32_premul((100, 100)).expect("a raster surface");
    for m in [
        TileMode::Clamp,
        TileMode::Repeat,
        TileMode::Repeat,
        TileMode::Decal,
    ] {
        let mut paint = Paint::default();
        paint.set_shader(picture.to_shader((m, m), FilterMode::Nearest, None, None));
        surface.canvas().draw_paint(&paint);
    }

    // Don't expect any additional refs on the picture
    reporter_assert!(reporter, picture.unique());

    // Check that we did cache something, but only 1 thing
    reporter_assert!(reporter, count_entries(shared_id) == 1);

    // Now delete the picture, and check the we purge the cache entry
    drop(picture);
    ResourceCache::global()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .check_messages();

    reporter_assert!(reporter, count_entries(shared_id) == 0);
});
