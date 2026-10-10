// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkYUVPlanesCache.{h,cpp} (chrome/m156)

//! The cache of the YUV planes of decoded images (`SkYUVPlanesCache`).
//!
//! A record holds the [`CachedData`] that stores the planes, and the [`YUVAPixmaps`] whose planes
//! view that data. The record takes a reference on the data while it is in the cache, so the data
//! stays locked while the cache is the only owner, and is unlocked (and so can be purged) once
//! every client has released it.

use std::any::Any;
use std::mem::size_of;
use std::sync::Arc;

use crate::bitmap_cache::make_shared_id_for_bitmap;
use crate::cached_data::CachedData;
use crate::resource_cache::{Key, Rec, ResourceCache};
use crate::strike_cache::lock_unpoisoned;
use crate::yuva_pixmaps::YUVAPixmaps;

/// The namespace of the keys. Only its address is used, as Skia's static label is.
static YUV_PLANES_KEY_NAMESPACE_LABEL: u8 = 0;

/// The key of the planes of the image with generation ID `gen_id` (`YUVPlanesKey`).
// Port of: src/core/SkYUVPlanesCache.cpp#L28-L38 (chrome/m156)
fn plane_key(gen_id: u32) -> Key {
    Key::new(
        std::ptr::addr_of!(YUV_PLANES_KEY_NAMESPACE_LABEL) as usize,
        make_shared_id_for_bitmap(gen_id),
        &[gen_id],
    )
}

/// A cached set of YUV planes (`YUVPlanesRec`).
// Port of: src/core/SkYUVPlanesCache.cpp#L40-L66 (chrome/m156)
struct YUVPlanesRec {
    key: Key,
    data: Arc<CachedData>,
    pixmaps: YUVAPixmaps,
}

impl YUVPlanesRec {
    // The record takes the cache's reference on the data (`attachToCacheAndRef`).
    fn new(key: Key, data: Arc<CachedData>, pixmaps: YUVAPixmaps) -> Self {
        data.attach_to_cache_and_ref();
        Self { key, data, pixmaps }
    }
}

impl Drop for YUVPlanesRec {
    // Port of: src/core/SkYUVPlanesCache.cpp#L48-L50 (chrome/m156), `~YUVPlanesRec`
    fn drop(&mut self) {
        self.data.detach_from_cache_and_unref();
    }
}

impl Rec for YUVPlanesRec {
    fn key(&self) -> &Key {
        &self.key
    }

    // Port of: src/core/SkYUVPlanesCache.cpp#L59 (chrome/m156), `bytesUsed`
    fn bytes_used(&self) -> usize {
        size_of::<Self>() + self.data.size()
    }

    fn category(&self) -> &'static str {
        "yuv-planes"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Looks up the planes of the image `gen_id` in `local_cache`, or in the process-wide cache if
/// there is none. On success returns a reference to the data that holds the pixels, and the
/// planes over it (Skia's out-parameter `SkYUVAPixmaps*`). `None` if there are no planes, or the
/// data is no longer locked.
// Port of: src/core/SkYUVPlanesCache.cpp#L68-L84 (chrome/m156), `FindAndRef`
#[doc(alias = "SkYUVPlanesCache::FindAndRef")]
#[must_use]
pub fn find_and_ref(
    gen_id: u32,
    local_cache: Option<&mut ResourceCache>,
) -> Option<(Arc<CachedData>, YUVAPixmaps)> {
    let key = plane_key(gen_id);
    let mut result = None;
    // Port of: src/core/SkYUVPlanesCache.cpp#L52-L62 (chrome/m156), `YUVPlanesRec::Visitor`
    let visitor = |rec: &dyn Rec| -> bool {
        let Some(rec) = rec.as_any().downcast_ref::<YUVPlanesRec>() else {
            return false;
        };
        rec.data.add_ref();
        // The ref above locks the data for discardable memory, so the lock can now be checked.
        if rec.data.data().is_none() {
            rec.data.unref();
            return false;
        }
        result = Some((Arc::clone(&rec.data), rec.pixmaps.clone()));
        true
    };
    let found = match local_cache {
        Some(cache) => cache.find(&key, visitor),
        None => lock_unpoisoned(ResourceCache::global()).find(&key, visitor),
    };
    if found { result } else { None }
}

/// Adds the planes of the image `gen_id`, stored in `data`, to `local_cache` or to the
/// process-wide cache (`SkYUVPlanesCache::Add`). The planes in `pixmaps` must view `data`.
// Port of: src/core/SkYUVPlanesCache.cpp#L86-L96 (chrome/m156), `Add`
#[doc(alias = "SkYUVPlanesCache::Add")]
pub fn add(
    gen_id: u32,
    data: &Arc<CachedData>,
    pixmaps: &YUVAPixmaps,
    local_cache: Option<&mut ResourceCache>,
) {
    let rec = Box::new(YUVPlanesRec::new(
        plane_key(gen_id),
        Arc::clone(data),
        pixmaps.clone(),
    ));
    match local_cache {
        Some(cache) => cache.add(rec),
        None => lock_unpoisoned(ResourceCache::global()).add(rec),
    }
}
