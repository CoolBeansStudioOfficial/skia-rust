// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Resource.h, src/gpu/graphite/Resource.cpp

//! `skgpu::graphite::Resource`: an object the [`ResourceCache`](super::resource_cache) can keep
//! and reuse, with Graphite's explicit reference counts.
//!
//! # Ownership model (`docs/design/gpu.md` §5.1)
//!
//! Skia packs four counts into one atomic 64-bit word: usage refs (public `sk_sp`s), command
//! buffer refs (GPU work in flight), one cache ref and one return-queue ref. When a usage or
//! command buffer unref makes the resource *reusable* or *purgeable*, the unreffing thread adds
//! the return-queue ref and pushes the resource onto its cache's return queue; the cache later
//! decides whether to keep it. When the word reaches zero the resource frees its GPU data.
//!
//! The port keeps that word, its bit layout and its compare-and-swap loop verbatim, so the
//! cache's observable bookkeeping (which resources are purgeable, budgeted or findable, and in
//! which order they come back) is the same as in C++. Rust's memory management sits underneath:
//!
//! - a resource lives in an [`Arc<Resource<T>>`](Resource) (`T` is the backend object, e.g. a
//!   texture); the `Arc` only keeps the memory alive, while Skia's counts decide when the GPU data
//!   is freed (`internalDispose`);
//! - [`ResourceRef<T>`] is `sk_sp<T>`: it owns one usage ref, `Clone` adds one and `Drop` removes
//!   one (which may push the resource onto the return queue);
//! - [`CommandBufferRef`] owns one command buffer ref;
//! - the cache and its return queue hold type-erased `Arc<dyn AnyResource>`s and add or remove
//!   the cache and return-queue refs explicitly.
//!
//! Skia's lock-free intrusive return queue becomes a `Mutex<Vec<_>>` processed last-in first-out,
//! which is the order Skia's singly-linked list (pushed at the head) yields.

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, Weak};

use crate::gpu::gpu_types::{Budgeted, Protected, StdSteadyClockTimePoint};
use crate::graphite::graphite_resource_key::GraphiteResourceKey;
use crate::graphite::resource_cache::ReturnQueue;
use crate::graphite::resource_types::{Ownership, Shareable};
use crate::graphite::texture::Texture;

/// `Resource::RefType`.
// Port of: src/gpu/graphite/Resource.h#L195-L200 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefType {
    // Counts controlled by `sk_sp` and tracks liveness from external code.
    Usage,
    // Incremented in Context::insertRecording, decremented by finish procs.
    CommandBuffer,
    // At most 1 ref, added in registerWithCache(), removed on cache shutdown or purge.
    Cache,
    // At most 1 ref, held while in the cache's return queue.
    ReturnQueue,
}

// Resource tracks its different ref counts packed into a single atomic 64-bit value:
// commandBufferRefs:31, usageRefs:31, returnQueueRef:1, cacheRefs:1.
// Port of: src/gpu/graphite/Resource.h#L549-L557 (chrome/m156)
const fn ref_increment(ref_type: RefType) -> u64 {
    match ref_type {
        RefType::CommandBuffer => 1 << 33,
        RefType::Usage => 1 << 2,
        RefType::Cache => 1 << 1,
        RefType::ReturnQueue => 1,
    }
}

// Port of: src/gpu/graphite/Resource.h#L558-L566 (chrome/m156)
const fn ref_mask(ref_type: RefType) -> u64 {
    match ref_type {
        RefType::CommandBuffer => ((1u64 << 31) - 1) << 33,
        RefType::Usage => ((1u64 << 31) - 1) << 2,
        RefType::Cache => 0b10,
        RefType::ReturnQueue => 0b01,
    }
}

// Port of: src/gpu/graphite/Resource.h#L567-L569 (chrome/m156)
const fn purgeable_mask() -> u64 {
    ref_mask(RefType::Usage) | ref_mask(RefType::CommandBuffer)
}

/// `Resource::UniqueID`: an id that is unique for each resource object and never 0.
// Port of: src/gpu/graphite/Resource.h#L248-L261 (chrome/m156)
#[doc(alias = "skgpu::graphite::Resource::UniqueID")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceUniqueId(u32);

impl ResourceUniqueId {
    /// `UniqueID(id)`.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// `asUInt()`.
    #[doc(alias = "asUInt")]
    #[must_use]
    pub const fn as_uint(self) -> u32 {
        self.0
    }
}

// Port of: src/gpu/graphite/Resource.cpp#L15-L23 (chrome/m156)
fn create_unique_id() -> ResourceUniqueId {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return ResourceUniqueId(id);
        }
    }
}

/// `Resource::DeleteASAP`.
// Port of: src/gpu/graphite/Resource.h#L327-L330 (chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeleteAsap {
    #[default]
    No,
    Yes,
}

/// The backend half of a resource (`Resource`'s virtual functions).
///
/// Graphite's `Resource` subclasses (textures, buffers, samplers, pipelines) implement this; the
/// shared state lives in [`Resource`].
// Port of: src/gpu/graphite/Resource.h#L242-L287, #L388, #L510-L527 (chrome/m156)
pub trait ResourceObject: Send + Sync + 'static {
    /// `getResourceType()`: the kind of resource, for memory dumps.
    #[doc(alias = "getResourceType")]
    fn resource_type(&self) -> &'static str;

    /// `freeGpuData()`: frees the backend objects.
    #[doc(alias = "freeGpuData")]
    fn free_gpu_data(&self);

    /// `invokeReleaseProc()`: calls any release callbacks.
    #[doc(alias = "invokeReleaseProc")]
    fn invoke_release_proc(&self) {}

    /// `setBackendLabel()`: sets the label on the backend object.
    #[doc(alias = "setBackendLabel")]
    fn set_backend_label(&self, _label: &str) {}

    /// `onUpdateGpuMemorySize()`: a more up-to-date size in bytes; `current` is the tracked size.
    #[doc(alias = "onUpdateGpuMemorySize")]
    fn on_update_gpu_memory_size(&self, current: usize) -> usize {
        current
    }

    /// `prepareForReturnToCache(takeRef)`: lets a non-shareable resource prepare itself to
    /// re-enter the cache. Calling `take_ref` hands the resource its first usage ref back (the
    /// returned handle owns it); the resource then returns again when that ref is dropped.
    /// Returns true if `take_ref` was called.
    #[doc(alias = "prepareForReturnToCache")]
    fn prepare_for_return_to_cache(&self, _take_ref: &mut dyn FnMut() -> AnyResourceRef) -> bool {
        false
    }

    /// `isProtected()`.
    #[doc(alias = "isProtected")]
    fn is_protected(&self) -> Protected {
        Protected::No
    }

    /// `asTexture()`.
    #[doc(alias = "asTexture")]
    fn as_texture(&self) -> Option<&Texture> {
        None
    }
}

/// The state the cache mutates on its thread (`fReturnCache::fSingleOwner`-guarded in Skia).
// Port of: src/gpu/graphite/Resource.h#L713-L760 (chrome/m156)
#[derive(Debug)]
struct CacheState {
    gpu_memory_size: usize,
    budgeted: Budgeted,
    shareable: Shareable,
    delete_asap: DeleteAsap,
    available_for_reuse: bool,
    cache_array_index: i32,
    last_use_token: u32,
    last_access: StdSteadyClockTimePoint,
    label: String,
    backend_label_dirty: bool,
}

/// The shared half of a resource: Skia's `Resource` fields.
// Port of: src/gpu/graphite/Resource.h#L194-L761 (chrome/m156)
pub struct ResourceBase {
    // See ref_increment() for how the bits in this field are interpreted.
    refs: AtomicU64,
    // RefMask(kUsage) or PurgeableMask(), always including RefMask(kReturnQueue).
    reusable_ref_mask: u64,
    // `fSharedContext == nullptr` after internalDispose().
    destroyed: AtomicBool,
    unique_id: ResourceUniqueId,
    ownership: Ownership,
    requires_prepare_for_return_to_cache: bool,
    // The resource key and return cache are both set at most once, during registerWithCache().
    key: OnceLock<GraphiteResourceKey>,
    return_cache: OnceLock<Weak<ReturnQueue>>,
    // `fNextInReturnQueue != nullptr`.
    in_return_queue: AtomicBool,
    state: Mutex<CacheState>,
}

impl std::fmt::Debug for ResourceBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resource")
            .field("unique_id", &self.unique_id)
            .field("refs", &self.refs.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// A Graphite resource: the shared state plus the backend object `T`.
///
/// It derefs to `T`. Handles to it are [`ResourceRef<T>`] (usage refs).
#[doc(alias = "skgpu::graphite::Resource")]
#[derive(Debug)]
pub struct Resource<T: ResourceObject> {
    base: ResourceBase,
    object: T,
}

impl<T: ResourceObject> std::ops::Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.object
    }
}

/// A type-erased [`Resource`], as the cache stores it.
pub trait AnyResource: Send + Sync + 'static {
    /// The shared state.
    fn base(&self) -> &ResourceBase;
    /// The backend object.
    fn object(&self) -> &dyn ResourceObject;
    /// Upcast for downcasting to a concrete `Resource<T>`.
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

impl<T: ResourceObject> AnyResource for Resource<T> {
    fn base(&self) -> &ResourceBase {
        &self.base
    }

    fn object(&self) -> &dyn ResourceObject {
        &self.object
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

/// The key of a resource that was never registered with a cache.
static INVALID_KEY: LazyLock<GraphiteResourceKey> = LazyLock::new(GraphiteResourceKey::new);

impl<T: ResourceObject> Resource<T> {
    /// `Resource(sharedContext, ownership, gpuMemorySize, label, reusableRequiresPurgeable,
    /// requiresPrepareForReturnToCache)`: a new resource holding a single usage ref, which the
    /// returned handle owns.
    ///
    /// skia-rust: the `SharedContext` pointer is not stored; `was_destroyed()` tracks disposal.
    // Port of: src/gpu/graphite/Resource.cpp#L25-L49 (chrome/m156)
    #[allow(clippy::new_ret_no_self)] // a resource is created holding its first usage ref
    pub fn new(
        object: T,
        ownership: Ownership,
        gpu_memory_size: usize,
        label: &str,
        reusable_requires_purgeable: bool,
        requires_prepare_for_return_to_cache: bool,
    ) -> ResourceRef<T> {
        let reusable_ref_mask = (if reusable_requires_purgeable {
            purgeable_mask()
        } else {
            ref_mask(RefType::Usage)
        }) | ref_mask(RefType::ReturnQueue);
        let resource = Arc::new(Resource {
            base: ResourceBase {
                // Start with 1 usage ref and no others
                refs: AtomicU64::new(ref_increment(RefType::Usage)),
                reusable_ref_mask,
                destroyed: AtomicBool::new(false),
                unique_id: create_unique_id(),
                ownership,
                requires_prepare_for_return_to_cache,
                key: OnceLock::new(),
                return_cache: OnceLock::new(),
                in_return_queue: AtomicBool::new(false),
                state: Mutex::new(CacheState {
                    gpu_memory_size,
                    budgeted: Budgeted::No,
                    shareable: Shareable::No,
                    delete_asap: DeleteAsap::No,
                    available_for_reuse: false,
                    cache_array_index: -1,
                    last_use_token: 0,
                    last_access: StdSteadyClockTimePoint::now(),
                    label: label.to_owned(),
                    backend_label_dirty: !label.is_empty(),
                }),
            },
            object,
        });
        // At initialization time, a Resource should not be considered budgeted because it does
        // not yet belong to a ResourceCache.
        debug_assert!(resource.base.is_uniquely_held());
        ResourceRef { resource }
    }

    /// The backend object.
    #[must_use]
    pub fn object(&self) -> &T {
        &self.object
    }
}

impl ResourceBase {
    fn state(&self) -> MutexGuard<'_, CacheState> {
        // The state is only touched in short, non-reentrant sections, so poisoning can only come
        // from a panic elsewhere; keep going with the data as Skia would.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `isBusyOnGPU()`: whether any command buffer refs are held.
    // Port of: src/gpu/graphite/Resource.h#L231-L233 (chrome/m156)
    #[doc(alias = "isBusyOnGPU")]
    #[must_use]
    pub fn is_busy_on_gpu(&self) -> bool {
        (self.refs.load(Ordering::Acquire) & ref_mask(RefType::CommandBuffer)) != 0
    }

    /// `ownership()`.
    #[must_use]
    pub fn ownership(&self) -> Ownership {
        self.ownership
    }

    /// `requiresPrepareForReturnToCache()`.
    #[must_use]
    pub fn requires_prepare_for_return_to_cache(&self) -> bool {
        self.requires_prepare_for_return_to_cache
    }

    /// `budgeted()`.
    #[must_use]
    pub fn budgeted(&self) -> Budgeted {
        self.state().budgeted
    }

    /// `shareable()`.
    #[must_use]
    pub fn shareable(&self) -> Shareable {
        self.state().shareable
    }

    /// `key()`: invalid until the resource is registered with a cache.
    #[must_use]
    pub fn key(&self) -> &GraphiteResourceKey {
        self.key.get().unwrap_or(&INVALID_KEY)
    }

    /// `gpuMemorySize()`: the approximate GPU memory used by this resource in bytes.
    #[doc(alias = "gpuMemorySize")]
    #[must_use]
    pub fn gpu_memory_size(&self) -> usize {
        self.state().gpu_memory_size
    }

    /// `uniqueID()`.
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> ResourceUniqueId {
        self.unique_id
    }

    /// `getLabel()`.
    #[doc(alias = "getLabel")]
    #[must_use]
    pub fn label(&self) -> String {
        self.state().label.clone()
    }

    /// `wasDestroyed()`: true once the GPU data has been freed.
    #[doc(alias = "wasDestroyed")]
    #[must_use]
    pub fn was_destroyed(&self) -> bool {
        self.destroyed.load(Ordering::Acquire)
    }

    /// `testingShouldDeleteASAP()`.
    #[doc(alias = "testingShouldDeleteASAP")]
    #[must_use]
    pub fn testing_should_delete_asap(&self) -> bool {
        self.state().delete_asap == DeleteAsap::Yes
    }

    // ---- ProxyCache / GlobalCache interface ---------------------------------------------------

    /// `setDeleteASAP()`: the resource is purged as soon as it returns to the cache (protected
    /// in Skia, for subclasses).
    // Port of: src/gpu/graphite/Resource.h#L315 (chrome/m156)
    #[doc(alias = "setDeleteASAP")]
    pub fn set_delete_asap(&self) {
        self.state().delete_asap = DeleteAsap::Yes;
    }

    // Port of: src/gpu/graphite/Resource.h#L332 (chrome/m156)
    pub(crate) fn should_delete_asap(&self) -> DeleteAsap {
        self.state().delete_asap
    }

    // Port of: src/gpu/graphite/Resource.h#L337-L338 (chrome/m156)
    pub(crate) fn update_access_time(&self) {
        self.state().last_access = StdSteadyClockTimePoint::now();
    }

    /// `lastAccessTime()`.
    #[doc(alias = "lastAccessTime")]
    #[must_use]
    pub fn last_access_time(&self) -> StdSteadyClockTimePoint {
        self.state().last_access
    }

    // ---- ResourceCache interface --------------------------------------------------------------

    // Port of: src/gpu/graphite/Resource.h#L346-L349 (chrome/m156)
    pub(crate) fn set_budgeted(&self, budgeted: Budgeted) {
        debug_assert!(budgeted == Budgeted::No || self.ownership == Ownership::Owned);
        self.state().budgeted = budgeted;
    }

    // Port of: src/gpu/graphite/Resource.h#L350-L353 (chrome/m156)
    pub(crate) fn set_shareable(&self, shareable: Shareable) {
        let mut state = self.state();
        debug_assert!(shareable == Shareable::No || state.budgeted == Budgeted::Yes);
        state.shareable = shareable;
    }

    // Port of: src/gpu/graphite/Resource.h#L355-L356 (chrome/m156)
    pub(crate) fn set_available_for_reuse(&self, avail: bool) {
        self.state().available_for_reuse = avail;
    }

    pub(crate) fn is_available_for_reuse(&self) -> bool {
        self.state().available_for_reuse
    }

    // Port of: src/gpu/graphite/Resource.h#L358-L359 (chrome/m156)
    pub(crate) fn last_use_token(&self) -> u32 {
        self.state().last_use_token
    }

    pub(crate) fn set_last_use_token(&self, token: u32) {
        self.state().last_use_token = token;
    }

    // `*accessCacheIndex()`.
    // Port of: src/gpu/graphite/Resource.h#L366 (chrome/m156)
    pub(crate) fn cache_index(&self) -> i32 {
        self.state().cache_array_index
    }

    pub(crate) fn set_cache_index(&self, index: i32) {
        self.state().cache_array_index = index;
    }

    // Port of: src/gpu/graphite/Resource.h#L374 (chrome/m156)
    pub(crate) fn set_gpu_memory_size(&self, size: usize) {
        self.state().gpu_memory_size = size;
    }

    // We allow the label on a Resource to change when used for a different function.
    // Port of: src/gpu/graphite/Resource.h#L453-L463 (chrome/m156)
    pub(crate) fn set_label(&self, label: &str) {
        let mut state = self.state();
        if state.label == label {
            return;
        }

        label.clone_into(&mut state.label);

        // It is not always safe to immediately update the backend GPU resource label. Mark it
        // as dirty so it can be updated when appropriate.
        state.backend_label_dirty = true;
    }

    /// `hasCacheRef()`.
    // Port of: src/gpu/graphite/Resource.h#L466-L468 (chrome/m156)
    #[must_use]
    pub fn has_cache_ref(&self) -> bool {
        (self.refs.load(Ordering::Acquire) & ref_mask(RefType::Cache)) != 0
    }

    /// `hasReturnQueueRef()`.
    // Port of: src/gpu/graphite/Resource.h#L470-L472 (chrome/m156)
    #[must_use]
    pub fn has_return_queue_ref(&self) -> bool {
        (self.refs.load(Ordering::Acquire) & ref_mask(RefType::ReturnQueue)) != 0
    }

    /// `inReturnQueue()`.
    // Port of: src/gpu/graphite/Resource.h#L474-L476 (chrome/m156)
    #[must_use]
    pub fn in_return_queue(&self) -> bool {
        self.has_return_queue_ref() && self.in_return_queue.load(Ordering::Acquire)
    }

    // Port of: src/gpu/graphite/Resource.h#L361-L364 (chrome/m156)
    pub(crate) fn set_in_return_queue(&self, in_queue: bool) {
        debug_assert!(self.has_return_queue_ref());
        self.in_return_queue.store(in_queue, Ordering::Release);
    }

    /// `isUsableAsScratch()`.
    // Port of: src/gpu/graphite/Resource.h#L478-L484 (chrome/m156)
    #[must_use]
    pub fn is_usable_as_scratch(&self) -> bool {
        // This is only called by the ResourceCache, so the state of the Resource's refs won't
        // be changed by another thread when isReusable is true.
        let orig_refs = self.refs.load(Ordering::Acquire) & !ref_mask(RefType::ReturnQueue);
        let is_reusable = (orig_refs & self.reusable_ref_mask) == 0;
        let shareable = self.shareable();
        shareable == Shareable::Scratch || (shareable == Shareable::No && is_reusable)
    }

    /// `isPurgeable()`: no usage or command buffer refs.
    // Port of: src/gpu/graphite/Resource.h#L486-L490 (chrome/m156)
    #[must_use]
    pub fn is_purgeable(&self) -> bool {
        (self.refs.load(Ordering::Acquire) & purgeable_mask()) == 0
    }

    /// `isUniquelyHeld()`: exactly one usage ref and nothing else.
    // Port of: src/gpu/graphite/Resource.h#L492-L496 (chrome/m156)
    #[must_use]
    pub fn is_uniquely_held(&self) -> bool {
        self.refs.load(Ordering::Acquire) == ref_increment(RefType::Usage)
    }

    /// `hasAnyRefs()`.
    // Port of: src/gpu/graphite/Resource.h#L498-L503 (chrome/m156)
    #[must_use]
    pub fn has_any_refs(&self) -> bool {
        self.refs.load(Ordering::Acquire) != 0
    }

    // The return queue this resource was registered with, if it is still alive.
    pub(crate) fn return_cache(&self) -> Option<Arc<ReturnQueue>> {
        self.return_cache.get().and_then(Weak::upgrade)
    }

    // Port of: src/gpu/graphite/Resource.h#L571-L584 (chrome/m156)
    fn add_ref(&self, ref_type: RefType, must_have_usage_refs: bool) {
        debug_assert!(
            ref_type != RefType::ReturnQueue,
            "return queue refs cannot be added directly"
        );
        let increment = ref_increment(ref_type);
        // No barrier required
        let orig_cnt = self.refs.fetch_add(increment, Ordering::Relaxed);
        // Require that there was an already held usage ref in order to add this new ref.
        debug_assert!(!must_have_usage_refs || (orig_cnt & ref_mask(RefType::Usage)) > 0);
        // And make sure that the specific type of ref did not overflow into another field
        debug_assert!((ref_mask(ref_type) - (orig_cnt & ref_mask(ref_type))) >= increment);
    }
}

/// Removes a ref of `ref_type`; may push the resource onto its cache's return queue, or free its
/// GPU data when no refs remain. Returns the ref word before the removal.
// Port of: src/gpu/graphite/Resource.h#L586-L673 (chrome/m156)
fn remove_ref(this: &Arc<dyn AnyResource>, ref_type: RefType) -> u64 {
    let base = this.base();
    let increment = ref_increment(ref_type);

    let mut orig_refs;
    if ref_type == RefType::Cache
        || ref_type == RefType::ReturnQueue
        || base.return_cache.get().is_none()
    {
        // Without a ResourceCache, or when it's a cache/return-queue unref, there is no
        // non-atomic work that has to happen so simply update the ref count. If the net ref
        // count reaches 0 we can safely delete the resource because no other thread will
        // increase the refs.
        orig_refs = base.refs.fetch_sub(increment, Ordering::AcqRel);
        debug_assert!((orig_refs & ref_mask(ref_type)) >= increment); // had a ref to remove

        if orig_refs == increment {
            debug_assert!(!base.has_any_refs());
            internal_dispose(this);
        }
    } else {
        debug_assert!(ref_type == RefType::CommandBuffer || ref_type == RefType::Usage);
        // When removing a usage or CB ref and the resource is registered with the cache, it may
        // need to be returned to the cache. A resource can only be in the return queue a single
        // time and must remain alive until cache removes it from the queue. A CAS loop is used to
        // atomically decrement the ref and add the return queue ref.
        let purgeable_return_mask: u64 = purgeable_mask() | ref_mask(RefType::ReturnQueue);
        debug_assert!((base.reusable_ref_mask & ref_mask(RefType::ReturnQueue)) != 0);
        debug_assert_eq!(increment & ref_mask(RefType::ReturnQueue), 0);
        let mut needs_return;
        orig_refs = base.refs.load(Ordering::Acquire);
        loop {
            debug_assert!((orig_refs & ref_mask(ref_type)) >= increment); // have a ref to remove

            // The Resource needs to return to the queue when it's not already in the return
            // queue AND it's transitioning from non-reusable -> reusable OR non-purgeable ->
            // purgeable. Including RefMask(kReturnQueue) in the masks before comparing to the
            // increment ensures that the return queue ref was 0 in origRefs.
            needs_return = ((orig_refs & purgeable_return_mask) == increment)
                || ((orig_refs & base.reusable_ref_mask) == increment);

            let next_refs = (orig_refs - increment)
                | if needs_return {
                    ref_mask(RefType::ReturnQueue)
                } else {
                    0
                };
            // If origRefs already included a return queue ref, nextRefs hasn't changed that
            debug_assert!(
                (orig_refs & ref_mask(RefType::ReturnQueue))
                    == (next_refs & ref_mask(RefType::ReturnQueue))
                    || needs_return
            );
            match base.refs.compare_exchange_weak(
                orig_refs,
                next_refs,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => orig_refs = current,
            }
        }
        // Because RefMask(kReturnQueue) was included in the `needs_return` check, we know that it
        // was unset in `orig_refs` and was added to `next_refs`. The CAS ensures that this was the
        // thread that added the return queue ref if `needs_return` is true.

        if needs_return && !return_to_cache(this) {
            // The cache rejected the resource, so we need to unset the "return queue" ref that we
            // added above, which may be the last ref keeping the object alive.
            debug_assert!(!base.in_return_queue.load(Ordering::Acquire));
            orig_refs = remove_ref(this, RefType::ReturnQueue);
        }
        // else we weren't returning the resource yet, or the cache is maintaining the return ref
        // until the return queue has been drained.
    }

    orig_refs
}

// Try to add the Resource to the cache's return queue for pending reuse.
// Port of: src/gpu/graphite/Resource.cpp#L69-L81 (chrome/m156)
fn return_to_cache(this: &Arc<dyn AnyResource>) -> bool {
    let base = this.base();
    // No resource should have been destroyed if there was still any sort of ref on it.
    debug_assert!(!base.was_destroyed());
    debug_assert!(base.return_cache.get().is_some());
    debug_assert!(base.has_return_queue_ref());
    // skia-rust: a cache dropped without shutdown() rejects returns like a shut-down one.
    match base.return_cache() {
        Some(cache) => cache.return_resource(this),
        None => false,
    }
}

// Frees the object in the underlying 3D API.
// Port of: src/gpu/graphite/Resource.cpp#L83-L91 (chrome/m156)
fn internal_dispose(this: &Arc<dyn AnyResource>) {
    let base = this.base();
    debug_assert!(!base.was_destroyed());
    this.object().invoke_release_proc();
    this.object().free_gpu_data();
    base.destroyed.store(true, Ordering::Release);
}

// ---- cache-side ref operations (on the erased resource) ----------------------------------------

/// `registerWithCache()`: adds the cache ref. May only be called once, while uniquely held.
// Port of: src/gpu/graphite/Resource.cpp#L56-L73 (chrome/m156)
pub(crate) fn register_with_cache(
    this: &Arc<dyn AnyResource>,
    return_cache: &Arc<ReturnQueue>,
    key: &GraphiteResourceKey,
    initial_budgeted_state: Budgeted,
    initial_shareable_state: Shareable,
) {
    let base = this.base();
    // ResourceCache should be registered before the Resource escapes the ResourceProvider.
    debug_assert!(base.is_uniquely_held());
    debug_assert!(base.return_cache.get().is_none());

    let key_was_unset = base.key.set(key.clone()).is_ok();
    let cache_was_unset = base.return_cache.set(Arc::downgrade(return_cache)).is_ok();
    debug_assert!(key_was_unset && cache_was_unset);

    base.add_ref(RefType::Cache, true);

    base.set_budgeted(initial_budgeted_state);
    base.set_shareable(initial_shareable_state);
}

/// `initialUsageRef()`: adds a usage ref when the usage count may be 0 (cache only), returning
/// the handle that owns it.
// Port of: src/gpu/graphite/Resource.h#L395-L397 (chrome/m156)
pub(crate) fn initial_usage_ref(this: &Arc<dyn AnyResource>) -> AnyResourceRef {
    this.base().add_ref(RefType::Usage, false);
    AnyResourceRef {
        resource: Arc::clone(this),
    }
}

/// `unrefCache()`.
// Port of: src/gpu/graphite/Resource.h#L401-L404 (chrome/m156)
pub(crate) fn unref_cache(this: &Arc<dyn AnyResource>) {
    debug_assert!(this.base().return_cache.get().is_some());
    remove_ref(this, RefType::Cache);
}

/// `unrefReturnQueue()`: removes the return-queue ref after the cache took the resource off the
/// queue. Returns `(is_reusable, is_purgeable)` as of that removal.
// Port of: src/gpu/graphite/Resource.h#L427-L447 (chrome/m156)
pub(crate) fn unref_return_queue(this: &Arc<dyn AnyResource>) -> (bool, bool) {
    let base = this.base();
    // We must reset the in-queue state *before* removing the return queue ref.
    base.in_return_queue.store(false, Ordering::Release);

    let orig_refs = remove_ref(this, RefType::ReturnQueue);

    // Since we should always have a cache ref when this is called, the Resource will never be
    // transitioning to having zero refs.
    debug_assert!((orig_refs & ref_mask(RefType::Cache)) != 0);
    // `reusable_ref_mask` always includes the ReturnQueue bit, and since we just removed the
    // return ref value, `orig_refs` also includes it. PurgeableMask() does not, so it can compare
    // to zero.
    (
        (orig_refs & base.reusable_ref_mask) == ref_mask(RefType::ReturnQueue),
        (orig_refs & purgeable_mask()) == 0,
    )
}

/// `updateGpuMemorySize()`.
// Port of: src/gpu/graphite/Resource.h#L374 (chrome/m156)
pub(crate) fn update_gpu_memory_size(this: &Arc<dyn AnyResource>) {
    let current = this.base().gpu_memory_size();
    let updated = this.object().on_update_gpu_memory_size(current);
    this.base().set_gpu_memory_size(updated);
}

/// `synchronizeBackendLabel()`: pushes the label to the backend object.
// Port of: src/gpu/graphite/Resource.h#L306-L312 (chrome/m156)
pub(crate) fn synchronize_backend_label(this: &dyn AnyResource) {
    let label = {
        let mut state = this.base().state();
        state.backend_label_dirty = false;
        state.label.clone()
    };
    if !label.is_empty() {
        let full_label = format!("Skia_{label}");
        this.object().set_backend_label(&full_label);
    }
}

// ---- handles -----------------------------------------------------------------------------------

/// `sk_sp<T>` for a Graphite resource: owns one usage ref.
///
/// Cloning adds a usage ref (`ref()`); dropping removes it (`unref()`), which may return the
/// resource to its cache or free its GPU data.
pub struct ResourceRef<T: ResourceObject> {
    resource: Arc<Resource<T>>,
}

impl<T: ResourceObject> ResourceRef<T> {
    /// The resource.
    #[must_use]
    pub fn resource(&self) -> &Resource<T> {
        &self.resource
    }

    /// The shared state of the resource.
    #[must_use]
    pub fn base(&self) -> &ResourceBase {
        &self.resource.base
    }

    /// A type-erased handle owning a new usage ref.
    #[must_use]
    pub fn to_any(&self) -> AnyResourceRef {
        self.resource.base.add_ref(RefType::Usage, true);
        AnyResourceRef {
            resource: self.resource.clone(),
        }
    }

    /// Converts into a type-erased handle.
    ///
    /// The new handle takes its own usage ref before this one is dropped, so the count never
    /// passes through a reusable or purgeable state.
    #[must_use]
    pub fn into_any(self) -> AnyResourceRef {
        self.to_any()
    }

    /// `refCommandBuffer()`: a handle owning a new command buffer ref.
    #[doc(alias = "refCommandBuffer")]
    #[must_use]
    pub fn ref_command_buffer(&self) -> CommandBufferRef {
        self.resource.base.add_ref(RefType::CommandBuffer, true);
        CommandBufferRef {
            resource: self.resource.clone(),
        }
    }

    /// True if both handles refer to the same resource.
    #[must_use]
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.resource, &b.resource)
    }

    pub(crate) fn erased(&self) -> Arc<dyn AnyResource> {
        self.resource.clone()
    }

    /// The shared allocation, without adding a usage ref: Skia's non-owning `const T*` (for
    /// example `BindBufferInfo::fBuffer`). Holding it keeps the memory alive but does not
    /// count as a usage, command buffer or cache ref.
    #[must_use]
    pub fn as_arc(&self) -> &Arc<Resource<T>> {
        &self.resource
    }

    /// `sk_ref_sp(resource)`: a handle owning a new usage ref on a resource that still has one.
    #[must_use]
    pub fn from_arc(resource: &Arc<Resource<T>>) -> Self {
        resource.base.add_ref(RefType::Usage, true);
        Self {
            resource: resource.clone(),
        }
    }
}

impl<T: ResourceObject> Clone for ResourceRef<T> {
    // Port of: src/gpu/graphite/Resource.h#L209-L212 (chrome/m156)
    fn clone(&self) -> Self {
        self.resource.base.add_ref(RefType::Usage, true);
        Self {
            resource: self.resource.clone(),
        }
    }
}

impl<T: ResourceObject> Drop for ResourceRef<T> {
    // Port of: src/gpu/graphite/Resource.h#L215-L217 (chrome/m156)
    fn drop(&mut self) {
        let erased: Arc<dyn AnyResource> = self.resource.clone();
        remove_ref(&erased, RefType::Usage);
    }
}

impl<T: ResourceObject> std::ops::Deref for ResourceRef<T> {
    type Target = Resource<T>;

    fn deref(&self) -> &Resource<T> {
        &self.resource
    }
}

impl<T: ResourceObject + std::fmt::Debug> std::fmt::Debug for ResourceRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ResourceRef").field(&self.resource).finish()
    }
}

/// A type-erased usage ref (an `sk_sp<Resource>`).
pub struct AnyResourceRef {
    resource: Arc<dyn AnyResource>,
}

impl AnyResourceRef {
    /// The shared state of the resource.
    #[must_use]
    pub fn base(&self) -> &ResourceBase {
        self.resource.base()
    }

    /// The backend object.
    #[must_use]
    pub fn object(&self) -> &dyn ResourceObject {
        self.resource.object()
    }

    /// `static_cast<T*>(resource)`: the typed handle, keeping this handle's usage ref, or
    /// `Err(self)` if the resource is not a `Resource<T>`.
    ///
    /// # Errors
    /// Returns the handle unchanged if the resource holds another object type.
    pub fn downcast<T: ResourceObject>(self) -> Result<ResourceRef<T>, Self> {
        match self.resource.clone().into_any().downcast::<Resource<T>>() {
            Ok(resource) => {
                // The new handle takes its own usage ref before `self` drops its one, so the
                // count never passes through a reusable or purgeable state.
                resource.base.add_ref(RefType::Usage, true);
                Ok(ResourceRef { resource })
            }
            Err(_) => Err(self),
        }
    }

    /// `refCommandBuffer()`.
    #[must_use]
    pub fn ref_command_buffer(&self) -> CommandBufferRef {
        self.resource.base().add_ref(RefType::CommandBuffer, true);
        CommandBufferRef {
            resource: self.resource.clone(),
        }
    }
}

impl Clone for AnyResourceRef {
    fn clone(&self) -> Self {
        self.resource.base().add_ref(RefType::Usage, true);
        Self {
            resource: self.resource.clone(),
        }
    }
}

impl Drop for AnyResourceRef {
    fn drop(&mut self) {
        remove_ref(&self.resource, RefType::Usage);
    }
}

impl std::fmt::Debug for AnyResourceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AnyResourceRef")
            .field(self.resource.base())
            .finish()
    }
}

/// A command buffer ref (`refCommandBuffer()` / `unrefCommandBuffer()` on drop).
pub struct CommandBufferRef {
    resource: Arc<dyn AnyResource>,
}

impl CommandBufferRef {
    /// The shared state of the resource.
    #[must_use]
    pub fn base(&self) -> &ResourceBase {
        self.resource.base()
    }
}

impl Drop for CommandBufferRef {
    // Port of: src/gpu/graphite/Resource.h#L225-L227 (chrome/m156)
    fn drop(&mut self) {
        remove_ref(&self.resource, RefType::CommandBuffer);
    }
}

impl std::fmt::Debug for CommandBufferRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CommandBufferRef")
            .field(self.resource.base())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Debug)]
    struct Counting(Arc<AtomicUsize>);

    impl ResourceObject for Counting {
        fn resource_type(&self) -> &'static str {
            "Counting"
        }

        fn free_gpu_data(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn uncached_resource_is_freed_with_its_last_ref() {
        let freed = Arc::new(AtomicUsize::new(0));
        let r = Resource::new(
            Counting(freed.clone()),
            Ownership::Owned,
            4,
            "",
            false,
            false,
        );
        assert!(r.base().is_uniquely_held());
        let r2 = r.clone();
        let cb = r.ref_command_buffer();
        assert!(r.base().is_busy_on_gpu());
        drop(r);
        drop(r2);
        assert_eq!(freed.load(Ordering::Relaxed), 0);
        let base_destroyed = cb.base().was_destroyed();
        assert!(!base_destroyed);
        drop(cb);
        assert_eq!(freed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn ref_word_layout() {
        assert_eq!(
            ref_mask(RefType::Usage) & ref_mask(RefType::CommandBuffer),
            0
        );
        assert_eq!(purgeable_mask() & 0b11, 0);
        assert_eq!(ref_increment(RefType::CommandBuffer), 1 << 33);
    }

    #[test]
    fn downcast_round_trip() {
        let freed = Arc::new(AtomicUsize::new(0));
        let r = Resource::new(
            Counting(freed.clone()),
            Ownership::Owned,
            4,
            "",
            false,
            false,
        );
        let any = r.into_any();
        assert!(any.base().is_uniquely_held());
        let typed = any.downcast::<Counting>().expect("same type");
        assert!(typed.base().is_uniquely_held());
        drop(typed);
        assert_eq!(freed.load(Ordering::Relaxed), 1);
    }
}
