// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBitmapCache.{h,cpp} (chrome/m156)

//! The process-wide cache of decoded and generated image bitmaps (`SkBitmapCache`).
//!
//! An entry is keyed by an image's unique ID and the subset it covers ([`BitmapCacheDesc`]). The
//! pixels are made into a [`BitmapCacheRec`] with [`alloc`], written, and added with [`add`],
//! which installs them in the bitmap. [`find`] installs the entry for a key into a bitmap. An image
//! that was added to the cache posts its ID as stale when it is dropped, which purges its entries
//! (`SkNotifyBitmapGenIDIsStale`).
//!
//! skia-rust deviation: the pixels of an entry are in a [`CachedData`], and installing an entry
//! copies them into the bitmap. Skia's bitmap points at the discardable memory, and holds it locked
//! while the bitmap lives; a safe Rust bitmap cannot point at memory that is locked and unlocked
//! behind a guard. So an entry is never kept alive by the bitmaps made from it, and it can be
//! purged whenever the cache is over budget.

use std::any::Any;
use std::mem::size_of;
use std::sync::Arc;

use crate::bitmap::Bitmap;
use crate::cached_data::CachedData;
use crate::font_types::set_four_byte_tag;
use crate::image_info::ImageInfo;
use crate::pixel_ref::next_image_id;
use crate::pixmap::Pixmap;
use crate::rect::IRect;
use crate::resource_cache::{Key, Rec, ResourceCache, post_purge_shared_id};
use crate::strike_cache::lock_unpoisoned;

/// The namespace of the bitmap cache keys. Only its address is used, as Skia's static label is.
static BITMAP_KEY_NAMESPACE_LABEL: u8 = 0;

/// The shared ID of the cache entries of the bitmap with generation ID `bitmap_gen_id`: the
/// `bmap` tag in the high word, and the generation ID in the low word
/// (`SkMakeResourceCacheSharedIDForBitmap`).
// Port of: src/core/SkBitmapCache.cpp#L23-L28 (chrome/m156)
#[doc(alias = "SkMakeResourceCacheSharedIDForBitmap")]
#[must_use]
pub fn make_shared_id_for_bitmap(bitmap_gen_id: u32) -> u64 {
    let shared_id = u64::from(set_four_byte_tag(b'b', b'm', b'a', b'p'));
    (shared_id << 32) | u64::from(bitmap_gen_id)
}

/// Purges the cache entries of the bitmap with generation ID `bitmap_gen_id`, in every cache
/// (`SkNotifyBitmapGenIDIsStale`).
// Port of: src/core/SkBitmapCache.cpp#L30-L32 (chrome/m156)
#[doc(alias = "SkNotifyBitmapGenIDIsStale")]
pub fn notify_bitmap_gen_id_is_stale(bitmap_gen_id: u32) {
    post_purge_shared_id(make_shared_id_for_bitmap(bitmap_gen_id));
}

/// The key of a bitmap cache entry: an image and the subset of it (`SkBitmapCacheDesc`).
// Port of: include/private/SkBitmapCacheDesc.h (chrome/m156)
#[doc(alias = "SkBitmapCacheDesc")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BitmapCacheDesc {
    /// `fImageID`: the unique ID of the image.
    pub image_id: u32,
    /// `fSubset`: the part of the image the entry holds.
    pub subset: IRect,
}

impl BitmapCacheDesc {
    /// Creates a key for `subset` of the image `image_id` (`SkBitmapCacheDesc::Make`).
    // Port of: src/core/SkBitmapCache.cpp#L36-L41 (chrome/m156)
    #[doc(alias = "SkBitmapCacheDesc::Make")]
    #[must_use]
    pub fn make(image_id: u32, subset: IRect) -> Self {
        debug_assert!(image_id != 0);
        debug_assert!(subset.width() > 0 && subset.height() > 0);
        Self { image_id, subset }
    }

    /// The key of a whole image with unique ID `image_id` and size `width` by `height`
    /// (`SkBitmapCacheDesc::Make(const SkImage*)`).
    // Port of: src/core/SkBitmapCache.cpp#L43-L47 (chrome/m156)
    #[doc(alias = "SkBitmapCacheDesc::Make")]
    #[must_use]
    pub fn for_image(image_id: u32, width: i32, height: i32) -> Self {
        Self::make(image_id, IRect::from_wh(width, height))
    }

    // The resource cache key: the bitmap's shared ID, and the descriptor as words.
    // Port of: src/core/SkBitmapCache.cpp#L54-L66 (chrome/m156), `BitmapKey`
    fn key(&self) -> Key {
        let words = [
            self.image_id,
            self.subset.left.cast_unsigned(),
            self.subset.top.cast_unsigned(),
            self.subset.right.cast_unsigned(),
            self.subset.bottom.cast_unsigned(),
        ];
        Key::new(
            std::ptr::addr_of!(BITMAP_KEY_NAMESPACE_LABEL) as usize,
            make_shared_id_for_bitmap(self.image_id),
            &words,
        )
    }
}

/// A bitmap cache record, which holds the pixels of a bitmap until they are added to the cache
/// (`SkBitmapCache::Rec`, and the `RecPtr` that `Alloc` returns).
// Port of: src/core/SkBitmapCache.cpp#L68-L140 (chrome/m156)
#[derive(Debug)]
pub struct BitmapCacheRec {
    key: Key,
    data: Arc<CachedData>,
    info: ImageInfo,
    row_bytes: usize,
    /// The unique ID the installed bitmaps' pixel refs get (`fPrUniqueID`).
    pr_unique_id: u32,
    /// Whether the cache holds a reference on `data`, taken by [`add`].
    attached: bool,
}

impl BitmapCacheRec {
    /// Writes the pixels of the record through `f`, with a pixmap of the record's info over them.
    /// `None` if the data is not locked, or the info does not fit the data.
    #[must_use]
    pub fn with_pixmap_mut<R>(&mut self, f: impl FnOnce(&mut Pixmap<'_>) -> R) -> Option<R> {
        let mut guard = self.data.writable_data()?;
        let mut pixmap = Pixmap::new(&self.info, &mut guard, self.row_bytes)?;
        Some(f(&mut pixmap))
    }

    /// Installs the pixels into `bitmap`, as an immutable bitmap with this record's pixel ref ID.
    /// Returns false if the data cannot be locked (`SkBitmapCache::Rec::install`).
    // Port of: src/core/SkBitmapCache.cpp#L104-L136 (chrome/m156), `install`
    fn install(&self, bitmap: &mut Bitmap) -> bool {
        // Locking the data takes a reference, and dropping it releases the lock again when the
        // cache is the only owner, as `SkDiscardableMemory::lock` and `unlock` do in Skia.
        self.data.add_ref();
        let pixels = self.data.data().map(|bytes| bytes.to_vec());
        self.data.unref();
        let Some(pixels) = pixels else {
            return false;
        };
        if !bitmap.install_pixels(&self.info, pixels, self.row_bytes) {
            return false;
        }
        if let Some(pixel_ref) = bitmap.pixel_ref() {
            pixel_ref.set_immutable_with_id(self.pr_unique_id);
        }
        true
    }
}

impl Drop for BitmapCacheRec {
    // Port of: src/core/SkBitmapCache.cpp#L84-L90 (chrome/m156), `~Rec`
    fn drop(&mut self) {
        if self.attached {
            self.data.detach_from_cache_and_unref();
        }
    }
}

impl Rec for BitmapCacheRec {
    fn key(&self) -> &Key {
        &self.key
    }

    // Port of: src/core/SkBitmapCache.cpp#L93-L95 (chrome/m156), `bytesUsed`
    fn bytes_used(&self) -> usize {
        size_of::<Key>() + self.info.compute_byte_size(self.row_bytes)
    }

    fn category(&self) -> &'static str {
        "bitmap"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Allocates a record for pixels of `info`, for the entry `desc` (`SkBitmapCache::Alloc`). `None`
/// if the size overflows, or the cache's discardable factory fails.
///
/// Write the pixels with [`BitmapCacheRec::with_pixmap_mut`], then add the record with [`add`].
// Port of: src/core/SkBitmapCache.cpp#L142-L166 (chrome/m156)
#[doc(alias = "SkBitmapCache::Alloc")]
#[must_use]
pub fn alloc(desc: &BitmapCacheDesc, info: &ImageInfo) -> Option<Box<BitmapCacheRec>> {
    debug_assert_eq!(info.width(), desc.subset.width());
    debug_assert_eq!(info.height(), desc.subset.height());

    let row_bytes = info.min_row_bytes();
    let size = info.compute_byte_size(row_bytes);
    if ImageInfo::byte_size_overflowed(size) {
        return None;
    }
    // The global cache's discardable factory, or heap memory when it has none.
    let data = lock_unpoisoned(ResourceCache::global()).new_cached_data(size)?;
    Some(Box::new(BitmapCacheRec {
        key: desc.key(),
        data,
        info: info.clone(),
        row_bytes,
        pr_unique_id: next_image_id(),
        attached: false,
    }))
}

/// Adds `rec` to the global cache, and installs the entry the cache keeps into `bitmap`
/// (`SkBitmapCache::Add`). If an unpurgeable entry for the same key is present, that one is kept
/// and installed instead.
// Port of: src/core/SkBitmapCache.cpp#L168-L170 (chrome/m156)
#[doc(alias = "SkBitmapCache::Add")]
pub fn add(mut rec: Box<BitmapCacheRec>, bitmap: &mut Bitmap) {
    // The cache takes its reference, and the reference the allocation holds is dropped, so the
    // data is unlocked once the cache is its only owner.
    rec.data.attach_to_cache_and_ref();
    rec.attached = true;
    rec.data.unref();
    lock_unpoisoned(ResourceCache::global()).add_with_payload(rec, |kept| {
        let installed = kept
            .as_any()
            .downcast_ref::<BitmapCacheRec>()
            .is_some_and(|rec| rec.install(bitmap));
        // SkAssertResult(install): an entry the cache keeps is always installable.
        debug_assert!(installed);
    });
}

/// Finds the entry for `desc` in the global cache, and installs it into `result`
/// (`SkBitmapCache::Find`).
// Port of: src/core/SkBitmapCache.cpp#L172-L176 (chrome/m156)
#[doc(alias = "SkBitmapCache::Find")]
#[must_use]
pub fn find(desc: &BitmapCacheDesc, result: &mut Bitmap) -> bool {
    lock_unpoisoned(ResourceCache::global()).find(&desc.key(), |rec| {
        rec.as_any()
            .downcast_ref::<BitmapCacheRec>()
            .is_some_and(|rec| rec.install(result))
    })
}
