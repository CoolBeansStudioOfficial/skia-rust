// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkMaskCache.cpp (chrome/m156), src/core/SkMaskCache.h (chrome/m156)

//! `SkMaskCache`: caches the masks of blurred rrects and rects in a [`ResourceCache`].
//!
//! The cached pixels live in a [`CachedData`]; a record of the cache holds one reference to it
//! (`attachToCacheAndRef`), and the record drops that reference when it is removed
//! (`detachFromCacheAndUnref`). The `localCache` argument of the C++ is `Option<&mut ResourceCache>`;
//! `None` selects the global cache (`SkResourceCache::Find`/`Add`).

use std::any::Any;
use std::sync::{Arc, PoisonError};

use crate::blur_types::BlurStyle;
use crate::cached_data::CachedData;
use crate::mask::MaskFormat;
use crate::rect::{IRect, Rect, RoundOut};
use crate::resource_cache::{Key, Rec, ResourceCache};
use crate::rrect::{Corner, RRect};

/// The mask of a cache entry: the cached data holds its pixels (`SkMask` with `fImage` pointing
/// at the data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedMask {
    /// `fBounds`.
    pub bounds: IRect,
    /// `fRowBytes`.
    pub row_bytes: u32,
    /// `fFormat`.
    pub format: MaskFormat,
}

/// `MaskValue`: the mask description plus the cached data that holds its pixels.
#[derive(Clone)]
struct MaskValue {
    mask: CachedMask,
    data: Arc<CachedData>,
}

/// The entries of the cache shared by both key kinds, with the reference-count bookkeeping of
/// `RRectBlurRec` / `RectsBlurRec`.
struct BlurRec<K> {
    key: K,
    value: MaskValue,
}

impl<K> BlurRec<K> {
    fn new(key: K, mask: &CachedMask, data: &Arc<CachedData>) -> Self {
        // The record takes a reference to the data while it is in the cache.
        data.attach_to_cache_and_ref();
        Self {
            key,
            value: MaskValue {
                mask: mask.clone(),
                data: Arc::clone(data),
            },
        }
    }
}

impl<K> Drop for BlurRec<K> {
    fn drop(&mut self) {
        // `~RRectBlurRec` / `~RectsBlurRec`: drops the reference the record took.
        self.value.data.detach_from_cache_and_unref();
    }
}

/// `RRectBlurKey`'s namespace label (the address of the C++ `gRRectBlurKeyNamespaceLabel`).
static RRECT_BLUR_KEY_NAMESPACE_LABEL: u8 = 0;
/// `RectsBlurKey`'s namespace label (the address of the C++ `gRectsBlurKeyNamespaceLabel`).
static RECTS_BLUR_KEY_NAMESPACE_LABEL: u8 = 0;

/// The `ResourceCache` namespace of the rrect keys.
fn rrect_blur_namespace() -> usize {
    std::ptr::from_ref(&RRECT_BLUR_KEY_NAMESPACE_LABEL) as usize
}

/// The `ResourceCache` namespace of the rects keys.
fn rects_blur_namespace() -> usize {
    std::ptr::from_ref(&RECTS_BLUR_KEY_NAMESPACE_LABEL) as usize
}

/// The bits of an `i32`, as key words.
fn i32_bits(v: i32) -> u32 {
    u32::from_ne_bytes(v.to_ne_bytes())
}

/// `RRectBlurKey`: the sigma, style and rrect a blurred rrect mask was made for.
// Port of: src/core/SkMaskCache.cpp#L26-L45 (chrome/m156)
struct RRectBlurKey {
    key: Key,
}

impl RRectBlurKey {
    // Port of: src/core/SkMaskCache.cpp#L31-L38 (chrome/m156)
    fn new(sigma: f32, rrect: &RRect, style: BlurStyle) -> Self {
        let bounds = rrect.rect();
        let mut contents = vec![
            sigma.to_bits(),
            i32_bits(style as i32),
            bounds.left.to_bits(),
            bounds.top.to_bits(),
            bounds.right.to_bits(),
            bounds.bottom.to_bits(),
        ];
        for corner in [
            Corner::UpperLeft,
            Corner::UpperRight,
            Corner::LowerRight,
            Corner::LowerLeft,
        ] {
            let radii = rrect.radii(corner);
            contents.push(radii.x.to_bits());
            contents.push(radii.y.to_bits());
        }
        Self {
            key: Key::new(rrect_blur_namespace(), 0, &contents),
        }
    }
}

/// `RectsBlurKey`: the sigma, style and rects a blurred rects mask was made for.
// Port of: src/core/SkMaskCache.cpp#L72-L99 (chrome/m156)
struct RectsBlurKey {
    key: Key,
}

impl RectsBlurKey {
    // Port of: src/core/SkMaskCache.cpp#L79-L97 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // the C++ key is scalar arithmetic on the int bounds
    fn new(sigma: f32, style: BlurStyle, rects: &[Rect]) -> Self {
        debug_assert!(rects.len() == 1 || rects.len() == 2);
        let ir: IRect = rects[0].round_out();
        let mut sizes = [(0.0_f32, 0.0_f32); 4];
        sizes[0] = (rects[0].width(), rects[0].height());
        if rects.len() == 2 {
            sizes[1] = (rects[1].width(), rects[1].height());
            sizes[2] = (rects[0].x() - rects[1].x(), rects[0].y() - rects[1].y());
        } else {
            sizes[1] = (0.0, 0.0);
            sizes[2] = (0.0, 0.0);
        }
        sizes[3] = (rects[0].x() - ir.x() as f32, rects[0].y() - ir.y() as f32);

        let mut contents = vec![sigma.to_bits(), i32_bits(style as i32)];
        for (w, h) in sizes {
            contents.push(w.to_bits());
            contents.push(h.to_bits());
        }
        Self {
            key: Key::new(rects_blur_namespace(), 0, &contents),
        }
    }
}

/// `RRectBlurRec`: an rrect blur cache entry.
struct RRectBlurRec {
    inner: BlurRec<RRectBlurKey>,
}

impl Rec for RRectBlurRec {
    fn key(&self) -> &Key {
        &self.inner.key.key
    }

    // Port of: src/core/SkMaskCache.cpp#L56-L57 (chrome/m156)
    fn bytes_used(&self) -> usize {
        std::mem::size_of::<Self>() + self.inner.value.data.size()
    }

    fn category(&self) -> &'static str {
        "rrect-blur"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// `RectsBlurRec`: a rects blur cache entry.
struct RectsBlurRec {
    inner: BlurRec<RectsBlurKey>,
}

impl Rec for RectsBlurRec {
    fn key(&self) -> &Key {
        &self.inner.key.key
    }

    // Port of: src/core/SkMaskCache.cpp#L134-L135 (chrome/m156)
    fn bytes_used(&self) -> usize {
        std::mem::size_of::<Self>() + self.inner.value.data.size()
    }

    fn category(&self) -> &'static str {
        "rects-blur"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Runs `f` on the local cache, or on the global cache when there is none
/// (`CHECK_LOCAL`).
fn with_cache<R>(local: Option<&mut ResourceCache>, f: impl FnOnce(&mut ResourceCache) -> R) -> R {
    if let Some(cache) = local {
        f(cache)
    } else {
        let mut global = ResourceCache::global()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        f(&mut global)
    }
}

/// The entry a lookup found: the cached data (with a reference for the caller, who must
/// `unref` it) and the mask description.
pub type FoundMask = (Arc<CachedData>, CachedMask);

/// Finds the cached data of a visitor-checked entry: the visitor takes the record's reference
/// before it checks the data is still there (`Visitor`).
fn find_entry(
    local: Option<&mut ResourceCache>,
    key: &Key,
    matches: impl FnOnce(&dyn Rec) -> Option<MaskValue>,
) -> Option<FoundMask> {
    let mut found: Option<MaskValue> = None;
    with_cache(local, |cache| {
        cache.find(key, |rec| {
            let Some(value) = matches(rec) else {
                return false;
            };
            found = Some(value);
            true
        })
    });
    found.map(|value| (value.data, value.mask))
}

/// `SkMaskCache::FindAndRef` for an rrect: the cached mask of a blurred rrect, if any.
// Port of: src/core/SkMaskCache.cpp#L47-L70 (chrome/m156)
#[must_use]
pub fn find_and_ref_rrect(
    sigma: f32,
    style: BlurStyle,
    rrect: &RRect,
    local_cache: Option<&mut ResourceCache>,
) -> Option<FoundMask> {
    let key = RRectBlurKey::new(sigma, rrect, style);
    find_entry(local_cache, &key.key, |rec| {
        let rec = rec.as_any().downcast_ref::<RRectBlurRec>()?;
        visit_value(&rec.inner.value)
    })
}

/// `SkMaskCache::FindAndRef` for rects: the cached mask of a blurred set of rects, if any.
// Port of: src/core/SkMaskCache.cpp#L101-L125 (chrome/m156)
#[must_use]
pub fn find_and_ref_rects(
    sigma: f32,
    style: BlurStyle,
    rects: &[Rect],
    local_cache: Option<&mut ResourceCache>,
) -> Option<FoundMask> {
    let key = RectsBlurKey::new(sigma, style, rects);
    find_entry(local_cache, &key.key, |rec| {
        let rec = rec.as_any().downcast_ref::<RectsBlurRec>()?;
        visit_value(&rec.inner.value)
    })
}

/// `Visitor`: takes a reference to the data and keeps the entry only if the data is still
/// present (its pixels are not purged).
// Port of: src/core/SkMaskCache.cpp#L36-L44 (chrome/m156)
fn visit_value(value: &MaskValue) -> Option<MaskValue> {
    let data = &value.data;
    data.add_ref();
    if data.data().is_none() {
        data.unref();
        return None;
    }
    Some(value.clone())
}

/// `SkMaskCache::Add` for an rrect: caches `data` (the mask pixels) under the blurred rrect.
// Port of: src/core/SkMaskCache.cpp#L127-L132 (chrome/m156)
pub fn add_rrect(
    sigma: f32,
    style: BlurStyle,
    rrect: &RRect,
    mask: &CachedMask,
    data: &Arc<CachedData>,
    local_cache: Option<&mut ResourceCache>,
) {
    let key = RRectBlurKey::new(sigma, rrect, style);
    let rec = RRectBlurRec {
        inner: BlurRec::new(key, mask, data),
    };
    with_cache(local_cache, |cache| cache.add(Box::new(rec)));
}

/// `SkMaskCache::Add` for rects: caches `data` (the mask pixels) under the blurred rects.
// Port of: src/core/SkMaskCache.cpp#L163-L173 (chrome/m156)
pub fn add_rects(
    sigma: f32,
    style: BlurStyle,
    rects: &[Rect],
    mask: &CachedMask,
    data: &Arc<CachedData>,
    local_cache: Option<&mut ResourceCache>,
) {
    let key = RectsBlurKey::new(sigma, style, rects);
    let rec = RectsBlurRec {
        inner: BlurRec::new(key, mask, data),
    };
    with_cache(local_cache, |cache| cache.add(Box::new(rec)));
}
