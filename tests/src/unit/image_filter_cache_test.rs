// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageFilterCacheTest.cpp (chrome/m156)
//
// The raster-backed tests only. The Ganesh tests (`ImageFilterCache_GPUBacked`,
// `ImageFilterCache_ImageBackedGPU`) are excluded. `PurgeImageFilterCache` is a `DEF_SERIAL_TEST`
// and needs `def_serial_test!`, which is not on this branch, so it is not ported here.
//
// skia-rust: `SkDEBUGCODE(REPORTER_ASSERT(...))` checks are made in every build, which is stronger
// than the C++ debug-only checks.

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filters;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter_cache::{ImageFilterCache, ImageFilterCacheKey};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_effects::image_filters;

use crate::{Reporter, def_test, reporter_assert};

/// `kSmallerSize`.
const SMALLER_SIZE: i32 = 10;
/// `kPad`.
const PAD: i32 = 3;
/// `kFullSize`.
const FULL_SIZE: i32 = SMALLER_SIZE + 2 * PAD;

/// `create_bm()`: a transparent, immutable `kFullSize` square of premultiplied RGBA.
// Port of: tests/ImageFilterCacheTest.cpp#L26-L35 (chrome/m156)
fn create_bm() -> Bitmap {
    let ii = ImageInfo::new(
        ISize::new(FULL_SIZE, FULL_SIZE),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ii, None);
    bm.erase_color(Color::new(0x0000_0000)); // SK_ColorTRANSPARENT
    bm.set_immutable();
    bm
}

/// `make_filter()`: a color filter that replaces the source with blue.
// Port of: tests/ImageFilterCacheTest.cpp#L37-L40 (chrome/m156)
fn make_filter() -> skia_rust_core::image_filter::ImageFilter {
    // SK_ColorBLUE, as a color filter (the SkColor overload converts to SkColor4f).
    let blue = Color4f::from_color(Color::new(0xFF00_00FF));
    let filter = color_filters::blend(blue, None, BlendMode::SrcIn).expect("SkColorFilters::Blend");
    image_filters::color_filter(Some(filter), None, None).expect("SkImageFilters::ColorFilter")
}

/// `SkSpecialImages::MakeFromRaster(subset, const SkImage*, props)`: the subset of an image's
/// pixels, sharing them.
// Port of: src/core/SkSpecialImage.cpp (SkSpecialImages::MakeFromRaster for an SkImage) (chrome/m156)
fn make_from_raster_image(subset: &IRect, image: &Image) -> Option<SpecialImage> {
    let bm = image.get_ro_pixels()?;
    SpecialImage::make_from_raster(subset, &bm, &SurfaceProps::default())
}

/// Ensure the cache can return a cached image.
// Port of: tests/ImageFilterCacheTest.cpp#L42-L60 (chrome/m156)
fn test_find_existing(
    reporter: &mut Reporter,
    image: &Arc<SpecialImage>,
    subset: &Arc<SpecialImage>,
) {
    const CACHE_SIZE: usize = 1_000_000;
    let cache = ImageFilterCache::create(CACHE_SIZE);
    let clip = IRect::from_wh(100, 100);
    let key1 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip,
        image.unique_id(),
        image.subset(),
    );
    let key2 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip,
        subset.unique_id(),
        subset.subset(),
    );
    let offset = IPoint::new(3, 4);
    let filter = make_filter();
    cache.set(
        key1.clone(),
        filter.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    let found_image = cache.get(&key1);
    reporter_assert!(reporter, found_image.is_some());
    reporter_assert!(
        reporter,
        found_image.as_ref().is_some_and(|found: &FilterResult| {
            IRect::from_xywh(offset.x, offset.y, image.width(), image.height())
                == found.layer_bounds()
        })
    );
    reporter_assert!(reporter, cache.get(&key2).is_none());
}

/// If either id is different or the clip or the matrix are different the cached image won't be
/// found. Even if it is caching the same bitmap.
// Port of: tests/ImageFilterCacheTest.cpp#L62-L81 (chrome/m156)
fn test_dont_find_if_diff_key(
    reporter: &mut Reporter,
    image: &Arc<SpecialImage>,
    subset: &Arc<SpecialImage>,
) {
    const CACHE_SIZE: usize = 1_000_000;
    let cache = ImageFilterCache::create(CACHE_SIZE);
    let clip1 = IRect::from_wh(100, 100);
    let clip2 = IRect::from_wh(200, 200);
    let key0 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip1,
        image.unique_id(),
        image.subset(),
    );
    let key1 = ImageFilterCacheKey::new(
        1,
        Matrix::new_identity(),
        clip1,
        image.unique_id(),
        image.subset(),
    );
    let key2 = ImageFilterCacheKey::new(
        0,
        Matrix::translate((5.0, 5.0)),
        clip1,
        image.unique_id(),
        image.subset(),
    );
    let key3 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip2,
        image.unique_id(),
        image.subset(),
    );
    let key4 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip1,
        subset.unique_id(),
        subset.subset(),
    );
    let offset = IPoint::new(3, 4);
    let filter = make_filter();
    cache.set(
        key0,
        filter.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    reporter_assert!(reporter, cache.get(&key1).is_none());
    reporter_assert!(reporter, cache.get(&key2).is_none());
    reporter_assert!(reporter, cache.get(&key3).is_none());
    reporter_assert!(reporter, cache.get(&key4).is_none());
}

/// Test purging when the max cache size is exceeded.
// Port of: tests/ImageFilterCacheTest.cpp#L83-L104 (chrome/m156)
fn test_internal_purge(reporter: &mut Reporter, image: &Arc<SpecialImage>) {
    debug_assert!(image.get_size() != 0);
    let cache_size = image.get_size() + 10;
    let cache = ImageFilterCache::create(cache_size);
    let clip = IRect::from_wh(100, 100);
    let key1 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip,
        image.unique_id(),
        image.subset(),
    );
    let key2 = ImageFilterCacheKey::new(
        1,
        Matrix::new_identity(),
        clip,
        image.unique_id(),
        image.subset(),
    );
    let offset = IPoint::new(3, 4);
    let filter1 = make_filter();
    cache.set(
        key1.clone(),
        filter1.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    reporter_assert!(reporter, cache.get(&key1).is_some());

    // This should knock the first one out of the cache
    let filter2 = make_filter();
    cache.set(
        key2.clone(),
        filter2.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    reporter_assert!(reporter, cache.get(&key2).is_some());
    reporter_assert!(reporter, cache.get(&key1).is_none());
}

/// Exercise the purgeByKey and purge methods.
// Port of: tests/ImageFilterCacheTest.cpp#L106-L137 (chrome/m156)
fn test_explicit_purging(
    reporter: &mut Reporter,
    image: &Arc<SpecialImage>,
    subset: &Arc<SpecialImage>,
) {
    const CACHE_SIZE: usize = 1_000_000;
    let cache = ImageFilterCache::create(CACHE_SIZE);
    let clip = IRect::from_wh(100, 100);
    let key1 = ImageFilterCacheKey::new(
        0,
        Matrix::new_identity(),
        clip,
        image.unique_id(),
        image.subset(),
    );
    let key2 = ImageFilterCacheKey::new(
        1,
        Matrix::new_identity(),
        clip,
        subset.unique_id(),
        image.subset(),
    );
    let offset = IPoint::new(3, 4);
    let filter1 = make_filter();
    let filter2 = make_filter();
    cache.set(
        key1.clone(),
        filter1.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    cache.set(
        key2.clone(),
        filter2.unique_id(),
        &FilterResult::new(Some(image.clone()), offset),
    );
    reporter_assert!(reporter, cache.count() == 2);
    reporter_assert!(reporter, cache.get(&key1).is_some());
    reporter_assert!(reporter, cache.get(&key2).is_some());

    cache.purge_by_image_filter(filter1.unique_id());
    reporter_assert!(reporter, cache.count() == 1);
    reporter_assert!(reporter, cache.get(&key1).is_none());
    reporter_assert!(reporter, cache.get(&key2).is_some());

    cache.purge();
    reporter_assert!(reporter, cache.count() == 0);
    reporter_assert!(reporter, cache.get(&key1).is_none());
    reporter_assert!(reporter, cache.get(&key2).is_none());
}

/// Shared test code for the raster and image-backed cases (`test_image_backed`, raster path).
// Port of: tests/ImageFilterCacheTest.cpp#L209-L232 (chrome/m156), without the rContext branch
fn test_image_backed(reporter: &mut Reporter, src_image: &Image) {
    let full = IRect::from_wh(FULL_SIZE, FULL_SIZE);
    let full_img = Arc::new(make_from_raster_image(&full, src_image).expect("MakeFromRaster"));

    let subset = IRect::from_xywh(PAD, PAD, SMALLER_SIZE, SMALLER_SIZE);
    let subset_img = Arc::new(make_from_raster_image(&subset, src_image).expect("MakeFromRaster"));

    test_find_existing(reporter, &full_img, &subset_img);
    test_dont_find_if_diff_key(reporter, &full_img, &subset_img);
    test_internal_purge(reporter, &full_img);
    test_explicit_purging(reporter, &full_img, &subset_img);
}

// Port of: tests/ImageFilterCacheTest.cpp#L191-L207 (chrome/m156)
def_test!(ImageFilterCache_RasterBacked, |reporter| {
    let src_bm = create_bm();
    let full = IRect::from_wh(FULL_SIZE, FULL_SIZE);
    let full_img = Arc::new(
        SpecialImage::make_from_raster(&full, &src_bm, &SurfaceProps::default())
            .expect("MakeFromRaster"),
    );
    let subset = IRect::from_xywh(PAD, PAD, SMALLER_SIZE, SMALLER_SIZE);
    let subset_img = Arc::new(
        SpecialImage::make_from_raster(&subset, &src_bm, &SurfaceProps::default())
            .expect("MakeFromRaster"),
    );

    test_find_existing(reporter, &full_img, &subset_img);
    test_dont_find_if_diff_key(reporter, &full_img, &subset_img);
    test_internal_purge(reporter, &full_img);
    test_explicit_purging(reporter, &full_img, &subset_img);
});

// Port of: tests/ImageFilterCacheTest.cpp#L244-L250 (chrome/m156)
def_test!(ImageFilterCache_ImageBackedRaster, |reporter| {
    let src_bm = create_bm();
    let src_image = src_bm.as_image().expect("srcBM.asImage()");
    test_image_backed(reporter, &src_image);
});
