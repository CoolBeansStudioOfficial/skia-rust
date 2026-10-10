// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/YUVCacheTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::cached_data::CachedData;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::YUVColorSpace;
use skia_rust_core::resource_cache::ResourceCache;
use skia_rust_core::size::ISize;
use skia_rust_core::yuv_planes_cache;
use skia_rust_core::yuva_info::{PlaneConfig, Siting, Subsampling, YUVAInfo};
use skia_rust_core::yuva_pixmaps::{DataType, YUVAPixmapInfo, YUVAPixmaps};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/YUVCacheTest.cpp#L18-L23 (chrome/m156), `LockedState`
#[derive(Clone, Copy, PartialEq, Eq)]
enum LockedState {
    Unlocked,
    Locked,
}

// Port of: tests/YUVCacheTest.cpp#L25-L28 (chrome/m156), `CachedState`
#[derive(Clone, Copy, PartialEq, Eq)]
enum CachedState {
    NotInCache,
    InCache,
}

// Port of: tests/YUVCacheTest.cpp#L30-L36 (chrome/m156), `check_data`
fn check_data(
    reporter: &mut Reporter,
    data: &CachedData,
    refcnt: i32,
    cache_state: CachedState,
    locked_state: LockedState,
) {
    reporter_assert!(reporter, data.testing_only_get_ref_cnt() == refcnt);
    reporter_assert!(
        reporter,
        data.testing_only_is_in_cache() == (cache_state == CachedState::InCache)
    );
    let is_locked = data.data().is_some();
    reporter_assert!(reporter, is_locked == (locked_state == LockedState::Locked));
}

// Port of: tests/YUVCacheTest.cpp#L38-L87 (chrome/m156), `YUVPlanesCache`
def_test!(YUVPlanesCache, |reporter| {
    let mut cache = ResourceCache::new(1024);

    let Some(yuva_info) = YUVAInfo::new(
        ISize::new(5, 5),
        PlaneConfig::Y_U_V,
        Subsampling::S420,
        YUVColorSpace::Rec601Limited,
        EncodedOrigin::TopLeft,
        (Siting::Centered, Siting::Centered),
    ) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(yuva_pixmap_info) = YUVAPixmapInfo::from_data_type(&yuva_info, DataType::Unorm8, None)
    else {
        reporter_assert!(reporter, false);
        return;
    };
    let gen_id = 12_345_678;

    let found = yuv_planes_cache::find_and_ref(gen_id, Some(&mut cache));
    reporter_assert!(reporter, found.is_none());

    let size = yuva_pixmap_info.compute_total_bytes(None);
    let Some(data) = cache.new_cached_data(size) else {
        reporter_assert!(reporter, false);
        return;
    };
    if let Some(mut bytes) = data.writable_data() {
        bytes.fill(0xff);
    }

    let Some(yuva_pixmaps) = YUVAPixmaps::from_cached_data(&yuva_pixmap_info, Arc::clone(&data))
    else {
        reporter_assert!(reporter, false);
        return;
    };

    yuv_planes_cache::add(gen_id, &data, &yuva_pixmaps, Some(&mut cache));
    check_data(reporter, &data, 2, CachedState::InCache, LockedState::Locked);

    data.unref();
    check_data(reporter, &data, 1, CachedState::InCache, LockedState::Unlocked);

    let found = yuv_planes_cache::find_and_ref(gen_id, Some(&mut cache));
    reporter_assert!(reporter, found.is_some());
    let Some((found_data, yuva_pixmaps_read)) = found else {
        return;
    };

    reporter_assert!(reporter, found_data.size() == size);
    reporter_assert!(
        reporter,
        yuva_pixmaps_read.yuva_info() == yuva_pixmaps.yuva_info()
    );

    for i in 0..yuva_pixmaps.num_planes() {
        // Each view locks the cached data it views, so the two views are taken one at a time.
        let (info, row_bytes, addr) = {
            let plane = yuva_pixmaps.plane(i);
            (
                plane.info().clone(),
                plane.row_bytes(),
                plane.addr().map(<[u8]>::as_ptr),
            )
        };
        let (info_read, row_bytes_read, addr_read) = {
            let plane_read = yuva_pixmaps_read.plane(i);
            (
                plane_read.info().clone(),
                plane_read.row_bytes(),
                plane_read.addr().map(<[u8]>::as_ptr),
            )
        };
        reporter_assert!(reporter, info == info_read);
        // The planes must view the same memory, not copies of it.
        reporter_assert!(reporter, addr == addr_read);
        reporter_assert!(reporter, row_bytes == row_bytes_read);
    }

    check_data(
        reporter,
        &found_data,
        2,
        CachedState::InCache,
        LockedState::Locked,
    );

    cache.purge_all();
    check_data(
        reporter,
        &found_data,
        1,
        CachedState::NotInCache,
        LockedState::Locked,
    );
    found_data.unref();
});
