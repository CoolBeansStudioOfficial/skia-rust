// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPixelRef.h, src/core/SkPixelRef.cpp,
// include/private/SkPixelStorage.h (unused parts omitted), src/core/SkPixelRefPriv.h

//! [`PixelRef`]: the shared container for pixel memory used with [`Bitmap`](crate::bitmap::Bitmap).
//!
//! skia-rust: `SkPixelRef` points at raw memory owned by someone else (a `malloc`ed block, an
//! `SkData`, or caller-managed storage with a release proc). Here the [`PixelRef`] always *owns*
//! its bytes (a `Vec<u8>`, or shared immutable [`Data`]). It is a reference-counted handle:
//! clones share the pixels, reads borrow them ([`PixelRef::pixels`]) and a write
//! ([`PixelRef::pixels_mut`]) first detaches the handle onto a private copy if the pixels are
//! shared (copy-on-write; `docs/design/pixels.md`). There are no locks, so pixel access can
//! neither block nor deadlock. Caller-managed storage is handed over together with a release proc
//! that gets the bytes back when the last reference goes away (see
//! [`crate::pixel_ref_priv::make_pixel_ref_with_proc`]).
//!
//! `SkPixelStorage` (the base class, tracking storage and content IDs for GPU proxies) is not
//! ported; the only subclass that exists so far is `SkPixelRef`.
//! `SkBitmapCache` is not ported, so `SkNotifyBitmapGenIDIsStale` is a no-op.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::data::Data;
use crate::id_change_listener::{IdChangeListener, IdChangeListenerList};
use crate::size::ISize;

/// Called with the pixel bytes when the last reference to a [`PixelRef`] goes away
/// (`void (*releaseProc)(void* addr, void* ctx)`; the context is whatever the closure captures).
pub type ReleaseProc = Box<dyn FnOnce(Vec<u8>) + Send>;

// Port of: src/core/SkPixelRef.cpp#L22-L31 (chrome/m156)
pub fn next_image_id() -> u32 {
    // We never set the low bit.... see PixelRef::gen_id_is_unique().
    static NEXT_ID: AtomicU32 = AtomicU32::new(2);

    loop {
        let id = NEXT_ID.fetch_add(2, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

// Port of: include/core/SkPixelRef.h#L98-L102 (chrome/m156)
const MUTABLE: u8 = 0; // PixelRefs begin mutable.
const TEMPORARILY_IMMUTABLE: u8 = 1; // Considered immutable, but can revert to mutable.
const IMMUTABLE: u8 = 2; // Once set to this state, it never leaves.

// The pixel memory: bytes that the pixel ref owns and that can be written to, or shared immutable
// [`Data`] (`SkMallocPixelRef::MakeWithData`).
enum Pixels {
    Owned(Vec<u8>),
    Data(Data),
}

impl Pixels {
    fn bytes(&self) -> &[u8] {
        match self {
            Pixels::Owned(bytes) => bytes,
            Pixels::Data(data) => data.as_bytes(),
        }
    }
}

struct Inner {
    width: i32,
    height: i32,
    row_bytes: usize,
    pixels: Pixels,
    // 0, or a gen ID with the low bit set if it is unique to this pixel ref.
    tagged_gen_id: AtomicU32,
    gen_id_change_listeners: IdChangeListenerList,
    added_to_cache: AtomicBool,
    mutability: AtomicU8,
    // Only taken (with `get_mut`) when the pixel ref is dropped: the `Mutex` just makes `Inner`
    // `Sync` around a `Send`-only closure. It is never locked, so it cannot block.
    release_proc: Mutex<Option<ReleaseProc>>,
}

/// This class is the smart container for pixel memory, and is used with
/// [`Bitmap`](crate::bitmap::Bitmap). This class can be shared/accessed between multiple threads.
///
/// Cloning a [`PixelRef`] shares the pixels (it is a reference-counted handle). Writing through
/// [`PixelRef::pixels_mut`] while they are shared first gives this handle its own copy.
// Port of: include/core/SkPixelRef.h#L33-L113 (chrome/m156)
#[doc(alias = "SkPixelRef")]
#[derive(Clone)]
pub struct PixelRef(Arc<Inner>);

impl fmt::Debug for PixelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PixelRef")
            .field("dimensions", &self.dimensions())
            .field("row_bytes", &self.row_bytes())
            .field("generation_id", &self.generation_id())
            .field("is_immutable", &self.is_immutable())
            .finish_non_exhaustive()
    }
}

impl PixelRef {
    /// Creates a pixel ref of `width` by `height` pixels over the bytes `pixels`, with
    /// `row_bytes` between rows.
    // Port of: src/core/SkPixelRef.cpp#L35-L45 (chrome/m156)
    #[must_use]
    pub fn new(width: i32, height: i32, pixels: Vec<u8>, row_bytes: usize) -> Self {
        Self::with_release_proc(width, height, pixels, row_bytes, None)
    }

    // The common constructor: `SkPixelRef::SkPixelRef` plus the derived classes' destructors
    // (`sk_free`, the release proc, dropping the `SkData`), which are all "drop the storage".
    pub(crate) fn with_release_proc(
        width: i32,
        height: i32,
        pixels: Vec<u8>,
        row_bytes: usize,
        release_proc: Option<ReleaseProc>,
    ) -> Self {
        Self::from_pixels(
            width,
            height,
            row_bytes,
            Pixels::Owned(pixels),
            release_proc,
        )
    }

    // The pixel ref over the immutable bytes of `data`, as `SkMallocPixelRef::MakeWithData`.
    pub(crate) fn with_data(width: i32, height: i32, row_bytes: usize, data: Data) -> Self {
        Self::from_pixels(width, height, row_bytes, Pixels::Data(data), None)
    }

    fn from_pixels(
        width: i32,
        height: i32,
        row_bytes: usize,
        pixels: Pixels,
        release_proc: Option<ReleaseProc>,
    ) -> Self {
        let inner = Inner {
            width,
            height,
            row_bytes,
            pixels,
            tagged_gen_id: AtomicU32::new(0),
            gen_id_change_listeners: IdChangeListenerList::new(),
            added_to_cache: AtomicBool::new(false),
            mutability: AtomicU8::new(MUTABLE),
            release_proc: Mutex::new(release_proc),
        };
        let pixel_ref = PixelRef(Arc::new(inner));
        pixel_ref.needs_new_gen_id();
        pixel_ref
    }

    /// Returns true if `self` and `other` are the same pixel ref (the same shared pixels).
    #[must_use]
    pub fn ptr_eq(&self, other: &PixelRef) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Returns the width and height in pixels.
    // Port of: include/core/SkPixelRef.h#L41 (chrome/m156)
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        ISize::new(self.0.width, self.0.height)
    }

    /// Returns the width in pixels.
    // Port of: include/core/SkPixelRef.h#L43 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> i32 {
        self.0.width
    }

    /// Returns the height in pixels.
    // Port of: include/core/SkPixelRef.h#L44 (chrome/m156)
    #[must_use]
    pub fn height(&self) -> i32 {
        self.0.height
    }

    /// The pixel bytes (`SkPixelRef::pixels()`), borrowed for as long as this handle is.
    // Port of: include/core/SkPixelRef.h#L45 (chrome/m156)
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        self.0.pixels.bytes()
    }

    /// The pixel bytes, writable.
    ///
    /// skia-rust: `SkPixelRef::pixels()` returns a writable `void*` from a `const` method, and
    /// every sharer sees the writes. Here, if the pixels are shared (another handle to this pixel
    /// ref exists, or they are immutable [`Data`]), this handle is first detached onto a private
    /// copy of the bytes: a new pixel ref with the same dimensions and row bytes, mutable, without
    /// listeners or release proc, with a new generation ID. Other handles keep the old pixels
    /// (copy-on-write, like `Arc::make_mut`; see `docs/design/pixels.md`).
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        let unique =
            matches!(self.0.pixels, Pixels::Owned(_)) && Arc::get_mut(&mut self.0).is_some();
        if !unique {
            let copy = self.0.pixels.bytes().to_vec();
            *self = Self::from_pixels(
                self.0.width,
                self.0.height,
                self.0.row_bytes,
                Pixels::Owned(copy),
                None,
            );
        }
        match Arc::get_mut(&mut self.0).map(|inner| &mut inner.pixels) {
            Some(Pixels::Owned(bytes)) => bytes,
            // Made unique and owned just above.
            _ => unreachable!("a detached pixel ref owns its bytes"),
        }
    }

    /// Returns true if this handle is the only one to its pixels and they can be written in
    /// place: [`Self::pixels_mut`] will not copy them.
    #[must_use]
    pub fn is_unique(&self) -> bool {
        matches!(self.0.pixels, Pixels::Owned(_)) && Arc::strong_count(&self.0) == 1
    }

    /// Returns the size of one pixel row in bytes.
    // Port of: include/core/SkPixelRef.h#L46 (chrome/m156)
    #[doc(alias = "rowBytes")]
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        self.0.row_bytes
    }

    // Port of: include/core/SkPixelRef.h#L97 (chrome/m156)
    fn gen_id_is_unique(&self) -> bool {
        self.0.tagged_gen_id.load(Ordering::SeqCst) & 1 != 0
    }

    // Port of: src/core/SkPixelRef.cpp#L61-L64 (chrome/m156)
    fn needs_new_gen_id(&self) {
        self.0.tagged_gen_id.store(0, Ordering::SeqCst);
        // This method isn't threadsafe, so the assert should be fine.
        debug_assert!(!self.gen_id_is_unique());
    }

    /// Returns a non-zero, unique value corresponding to the pixels in this pixel ref. Each time
    /// the pixels are changed (and [`Self::notify_pixels_changed`] is called), a different
    /// generation ID will be returned.
    // Port of: src/core/SkPixelRef.cpp#L66-L79 (chrome/m156)
    #[doc(alias = "getGenerationID")]
    #[must_use]
    pub fn generation_id(&self) -> u32 {
        let mut id = self.0.tagged_gen_id.load(Ordering::SeqCst);
        if 0 == id {
            let next = next_image_id() | 1u32;
            match self.0.tagged_gen_id.compare_exchange(
                id,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                // There was no race or we won the race. tagged_gen_id is next now.
                Ok(_) => id = next,
                // We lost a race to set tagged_gen_id. compare_exchange() filled id with the
                // winner.
                Err(winner) => id = winner,
            }
            // We can't quite assert(gen_id_is_unique()). It could be non-unique if we got here
            // via the else path (pretty unlikely, but possible).
        }
        id & !1u32 // Mask off bottom unique bit.
    }

    /// Registers a listener that is called when the generation ID is invalidated. `None` is
    /// ignored.
    // Port of: src/core/SkPixelRef.cpp#L81-L88 (chrome/m156)
    #[doc(alias = "addGenIDChangeListener")]
    pub fn add_gen_id_change_listener(&self, listener: Option<Arc<IdChangeListener>>) {
        let Some(listener) = listener else {
            return;
        };
        if !self.gen_id_is_unique() {
            // No point in tracking this if we're not going to call it.
            return;
        }
        debug_assert!(!listener.should_deregister());
        self.0.gen_id_change_listeners.add(listener);
    }

    // we need to be called *before* the genID gets changed or zerod
    // Port of: src/core/SkPixelRef.cpp#L90-L102 (chrome/m156)
    fn call_gen_id_change_listeners(&self) {
        // We don't invalidate ourselves if we think another PixelRef is sharing our genID.
        if self.gen_id_is_unique() {
            self.0.gen_id_change_listeners.changed();
            if self.0.added_to_cache.swap(false, Ordering::SeqCst) {
                // SkNotifyBitmapGenIDIsStale(this->getGenerationID()): SkBitmapCache is not
                // ported yet, so there is no cache to notify.
            }
        } else {
            // Listeners get at most one shot, so even though these weren't triggered or not,
            // blow them away.
            self.0.gen_id_change_listeners.reset();
        }
    }

    /// Call this if you have changed the contents of the pixels. This will in turn cause a
    /// different generation ID value to be returned from [`Self::generation_id`].
    // Port of: src/core/SkPixelRef.cpp#L104-L111 (chrome/m156)
    #[doc(alias = "notifyPixelsChanged")]
    pub fn notify_pixels_changed(&self) {
        #[cfg(debug_assertions)]
        if self.is_immutable() {
            eprintln!("========== notifyPixelsChanged called on immutable pixelref");
        }
        self.call_gen_id_change_listeners();
        self.needs_new_gen_id();
    }

    /// Returns true if the pixel ref is marked as immutable, meaning that the contents of its
    /// pixels will not change for the lifetime of the pixel ref.
    // Port of: include/core/SkPixelRef.h#L66 (chrome/m156)
    #[doc(alias = "isImmutable")]
    #[must_use]
    pub fn is_immutable(&self) -> bool {
        self.0.mutability.load(Ordering::SeqCst) != MUTABLE
    }

    /// Marks this pixel ref as immutable, meaning that the contents of its pixels will not change
    /// for the lifetime of the pixel ref. This state can be set on a pixel ref, but it cannot be
    /// cleared once it is set.
    // Port of: src/core/SkPixelRef.cpp#L113-L115 (chrome/m156)
    #[doc(alias = "setImmutable")]
    pub fn set_immutable(&self) {
        self.0.mutability.store(IMMUTABLE, Ordering::SeqCst);
    }

    /// Marks the pixel ref as immutable with the externally chosen generation ID `gen_id`.
    // Port of: src/core/SkPixelRef.cpp#L117-L126 (chrome/m156)
    #[doc(alias = "setImmutableWithID")]
    #[allow(dead_code)] // used by SkBitmapCache, which is not ported yet
    pub(crate) fn set_immutable_with_id(&self, gen_id: u32) {
        // We are forcing the genID to match an external value. The caller must ensure that this
        // value does not conflict with other content.
        //
        // One use is to force this pixelref's id to match an Image's id.
        self.0.mutability.store(IMMUTABLE, Ordering::SeqCst);
        self.0.tagged_gen_id.store(gen_id, Ordering::SeqCst);
    }

    // Port of: src/core/SkPixelRef.cpp#L128-L131 (chrome/m156)
    #[allow(dead_code)] // used by SkSurface_Raster, which is not ported yet
    pub(crate) fn set_temporarily_immutable(&self) {
        debug_assert!(self.0.mutability.load(Ordering::SeqCst) != IMMUTABLE);
        self.0
            .mutability
            .store(TEMPORARILY_IMMUTABLE, Ordering::SeqCst);
    }

    // Port of: src/core/SkPixelRef.cpp#L133-L136 (chrome/m156)
    #[allow(dead_code)] // used by SkSurface_Raster, which is not ported yet
    pub(crate) fn restore_mutability(&self) {
        debug_assert!(self.0.mutability.load(Ordering::SeqCst) != IMMUTABLE);
        self.0.mutability.store(MUTABLE, Ordering::SeqCst);
    }

    /// Marks the pixel ref as having been added to the bitmap cache.
    // Port of: include/core/SkPixelRef.h#L77-L79 (chrome/m156)
    #[doc(alias = "notifyAddedToCache")]
    pub fn notify_added_to_cache(&self) {
        self.0.added_to_cache.store(true, Ordering::SeqCst);
    }
}

// Port of: src/core/SkPixelRef.cpp#L47-L49 (chrome/m156)
impl Drop for Inner {
    fn drop(&mut self) {
        // The derived classes' destructors run first: free the pixels, or hand them back to the
        // release proc.
        if let Pixels::Owned(pixels) = &mut self.pixels {
            let pixels = std::mem::take(pixels);
            let release_proc = self
                .release_proc
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(release_proc) = release_proc {
                release_proc(pixels);
            }
        }

        // ~SkPixelRef(): callGenIDChangeListeners(). The listener list itself is dropped after
        // this, which is a no-op because the list is now empty.
        // We don't invalidate ourselves if we think another PixelRef is sharing our genID.
        if self.tagged_gen_id.load(Ordering::SeqCst) & 1 != 0 {
            self.gen_id_change_listeners.changed();
            if self.added_to_cache.swap(false, Ordering::SeqCst) {
                // SkNotifyBitmapGenIDIsStale: SkBitmapCache is not ported yet.
            }
        } else {
            self.gen_id_change_listeners.reset();
        }
    }
}
