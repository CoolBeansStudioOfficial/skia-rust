// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MaskCacheTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::cached_data::CachedData;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::mask_cache::{
    CachedMask, add_rects, add_rrect, find_and_ref_rects, find_and_ref_rrect,
};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::resource_cache::ResourceCache;
use skia_rust_core::rrect::RRect;

use crate::{Reporter, def_test, reporter_assert};

/// `check_data(reporter, data, refcnt, cacheState, lockedState)`.
// Port of: tests/MaskCacheTest.cpp#L16-L24 (chrome/m156)
fn check_data(
    reporter: &mut Reporter,
    data: &CachedData,
    refcnt: i32,
    in_cache: bool,
    locked: bool,
) {
    reporter_assert!(reporter, data.testing_only_get_ref_cnt() == refcnt);
    reporter_assert!(reporter, data.testing_only_is_in_cache() == in_cache);
    let is_locked = data.data().is_some();
    reporter_assert!(reporter, is_locked == locked);
}

// Port of: tests/MaskCacheTest.cpp#L39-L74 (chrome/m156)
def_test!(RRectMaskCache, |reporter| {
    let mut cache = ResourceCache::new(1024);

    let sigma = 0.8_f32;
    let base_rect = Rect::from_wh(100.0, 100.0);
    let mut rrect = RRect::new();
    rrect.set_rect_xy(base_rect, 30.0, 30.0);
    let style = BlurStyle::Normal;

    let found = find_and_ref_rrect(sigma, style, &rrect, Some(&mut cache));
    reporter_assert!(reporter, found.is_none());

    let size = 256;
    let data = cache.new_cached_data(size).expect("cached data");
    if let Some(mut bytes) = data.writable_data() {
        bytes.fill(0xff);
    }
    let mask = CachedMask {
        bounds: IRect::from_xywh(0, 0, 100, 100),
        row_bytes: 100,
        format: MaskFormat::BW,
    };
    add_rrect(sigma, style, &rrect, &mask, &data, Some(&mut cache));
    check_data(reporter, &data, 2, true, true);
    data.unref();
    check_data(reporter, &data, 1, true, false);

    let found = find_and_ref_rrect(sigma, style, &rrect, Some(&mut cache));
    reporter_assert!(reporter, found.is_some());
    let Some((found_data, found_mask)) = found else {
        return;
    };
    reporter_assert!(reporter, found_data.size() == size);
    reporter_assert!(
        reporter,
        found_mask.bounds.top() == 0 && found_mask.bounds.bottom() == 100
    );
    // `data->data() == lazyMask->fImage`: the mask pixels are the cached data's.
    reporter_assert!(reporter, found_data.data().is_some());
    check_data(reporter, &found_data, 2, true, true);
    cache.purge_all();
    check_data(reporter, &found_data, 1, false, true);
    found_data.unref();
});

// Port of: tests/MaskCacheTest.cpp#L76-L110 (chrome/m156)
def_test!(RectsMaskCache, |reporter| {
    let mut cache = ResourceCache::new(1024);

    let sigma = 0.8_f32;
    let rect = Rect::from_wh(100.0, 100.0);
    let rects = [rect, Rect::default()];
    let style = BlurStyle::Normal;

    let found = find_and_ref_rects(sigma, style, &rects[..1], Some(&mut cache));
    reporter_assert!(reporter, found.is_none());

    let size = 256;
    let data = cache.new_cached_data(size).expect("cached data");
    if let Some(mut bytes) = data.writable_data() {
        bytes.fill(0xff);
    }
    let mask = CachedMask {
        bounds: IRect::from_xywh(0, 0, 100, 100),
        row_bytes: 100,
        format: MaskFormat::BW,
    };
    add_rects(sigma, style, &rects[..1], &mask, &data, Some(&mut cache));
    check_data(reporter, &data, 2, true, true);
    data.unref();
    check_data(reporter, &data, 1, true, false);

    let found = find_and_ref_rects(sigma, style, &rects[..1], Some(&mut cache));
    reporter_assert!(reporter, found.is_some());
    let Some((found_data, found_mask)) = found else {
        return;
    };
    reporter_assert!(reporter, found_data.size() == size);
    reporter_assert!(
        reporter,
        found_mask.bounds.top() == 0 && found_mask.bounds.bottom() == 100
    );
    // `data->data() == lazyMask->fImage`: the mask pixels are the cached data's.
    reporter_assert!(reporter, found_data.data().is_some());
    check_data(reporter, &found_data, 2, true, true);
    cache.purge_all();
    check_data(reporter, &found_data, 1, false, true);
    found_data.unref();
});
