// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ResourceCache.h, src/gpu/graphite/ResourceCache.cpp

//! `skgpu::graphite::ResourceCache`: keeps resources for reuse, tracks the budget and purges in
//! least-recently-used order.
//!
//! The cache is owned by its `ResourceProvider` and used from one thread (`&mut self`, Skia's
//! `SingleOwner`). Resources return to it from any thread through the shared [`ReturnQueue`]
//! (see [`resource`](super::resource)). Every container keeps Skia's order:
//!
//! - the purgeable queue is a port of `SkTDPQueue` ordered by use token;
//! - the non-purgeable array uses Skia's swap-with-tail removal;
//! - the resource map is `SkTMultiMap` semantics (a new value goes to the front of its key's list;
//!   removal keeps the others' order);
//! - the return queue is processed last-in first-out.
//!
//! `GlobalResourceStats` (process-wide tracing counters) and `dumpMemoryStatistics`
//! (`SkTraceMemoryDump`) are not ported; neither affects the cache's behaviour.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use skia_rust_core::t_dp_queue::TDPQueue;
use skia_rust_core::t_sort::t_q_sort;

use crate::gpu::gpu_types::{Budgeted, StdSteadyClockTimePoint};
use crate::graphite::graphite_resource_key::GraphiteResourceKey;
use crate::graphite::proxy_cache::ProxyCache;
use crate::graphite::resource::{
    AnyResource, AnyResourceRef, DeleteAsap, ResourceBase, ResourceObject, ResourceRef,
    ResourceUniqueId, initial_usage_ref, register_with_cache, synchronize_backend_label,
    unref_cache, unref_return_queue, update_gpu_memory_size,
};
use crate::graphite::resource_types::{Ownership, Shareable};
use crate::graphite::texture::Texture;

/// `kMaxUseToken`: the token of every zero-sized resource.
// Port of: src/gpu/graphite/ResourceCache.cpp#L31 (chrome/m156)
const MAX_USE_TOKEN: u32 = 0xFFFF_FFFF;

/// `SK_InvalidGenID`.
const INVALID_GEN_ID: u32 = 0;

/// `ResourceCache::ScratchResourceSet`: resources a scratch request must not reuse.
pub type ScratchResourceSet = HashSet<ResourceUniqueId>;

/// The queue resources are returned through, shared by the cache and its resources.
///
/// `None` is Skia's sentinel head: the cache has shut down and rejects returns.
// Port of: src/gpu/graphite/ResourceCache.h#L195-L198 (chrome/m156)
#[derive(Default)]
pub struct ReturnQueue {
    queue: Mutex<Option<Vec<Arc<dyn AnyResource>>>>,
}

impl std::fmt::Debug for ReturnQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnQueue").finish_non_exhaustive()
    }
}

impl ReturnQueue {
    fn new() -> Self {
        Self {
            queue: Mutex::new(Some(Vec::new())),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Vec<Arc<dyn AnyResource>>>> {
        self.queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn is_shutdown(&self) -> bool {
        self.lock().is_none()
    }

    /// `returnResource()`: thread safe; a no-op returning false once the cache has shut down.
    /// Returns true if the resource was added to the return queue (or took a usage ref back in
    /// `prepareForReturnToCache`).
    // Port of: src/gpu/graphite/ResourceCache.cpp#L286-L374 (chrome/m156)
    pub(crate) fn return_resource(&self, resource: &Arc<dyn AnyResource>) -> bool {
        let base = resource.base();
        // We only allow one instance of a Resource to be in the return queue at a time but it
        // should have already added a return queue ref.
        debug_assert!(!base.in_return_queue() && base.has_return_queue_ref());

        // Check once to try and minimize the amount of wasted preparation work if the cache is
        // already shutdown.
        if self.is_shutdown() {
            return false;
        }

        // When a non-shareable resource's CB and Usage refs are both zero, give it a chance to
        // prepare itself to be reused.
        if base.should_delete_asap() == DeleteAsap::No
            && base.requires_prepare_for_return_to_cache()
        {
            // This adds a usage ref AND removes the return queue ref. By immediately unreffing
            // the return queue ref before the resource can be exposed to another thread, the
            // resource will always be able to be re-returned when the async work completes.
            let mut take_ref_actually_called = false;
            let take_ref_called =
                resource
                    .object()
                    .prepare_for_return_to_cache(&mut || -> AnyResourceRef {
                        let usage_ref = initial_usage_ref(resource);
                        unref_return_queue(resource);
                        take_ref_actually_called = true;
                        usage_ref
                    });

            debug_assert_eq!(take_ref_called, take_ref_actually_called);
            if take_ref_called {
                // Return 'true' here because we've removed the return queue ref already. Since we
                // added an initial ref, this resource will be re-returned once the async
                // prepare-for-return work has finished.
                return true;
            }
        }

        // Set the newly returned resource to be the head of the list.
        let mut queue = self.lock();
        match queue.as_mut() {
            None => {
                // Once the cache is shutdown, it can never be re-opened and we don't want to
                // actually return this resource.
                false
            }
            Some(list) => {
                base.set_in_return_queue(true);
                list.push(Arc::clone(resource));
                true
            }
        }
    }

    // Takes the whole queue, replacing it with an empty one (or the shutdown sentinel).
    // Returned in processing order: most recently returned first.
    fn take(&self, shutdown: bool) -> Vec<Arc<dyn AnyResource>> {
        let mut queue = self.lock();
        let old = if shutdown {
            queue.take()
        } else {
            queue.as_mut().map(std::mem::take)
        };
        let mut old = old.unwrap_or_default();
        old.reverse();
        old
    }
}

/// Graphite's resource cache.
#[doc(alias = "skgpu::graphite::ResourceCache")]
pub struct ResourceCache {
    // NOTE: every Resource held by the map, array and queue has a cache ref keeping it alive
    // until after it has been removed.
    purgeable_queue: TDPQueue<Arc<dyn AnyResource>>,
    nonpurgeable_resources: Vec<Arc<dyn AnyResource>>,
    resource_map: HashMap<GraphiteResourceKey, Vec<Arc<dyn AnyResource>>>,
    resource_map_count: usize,

    proxy_cache: Option<ProxyCache>,

    // Our budget
    max_bytes: usize,
    budgeted_bytes: usize,
    purgeable_bytes: usize,

    // Whenever a resource is added to the cache or the result of a cache lookup, `use_token` is
    // assigned as the resource's last use token and then incremented. The purgeable queue orders
    // resources by this value, so it is used to purge resources in LRU order. Zero-sized
    // resources get MAX_USE_TOKEN, which keeps them at the end of the queue.
    use_token: u32,

    return_queue: Arc<ReturnQueue>,
    count: usize,
}

impl std::fmt::Debug for ResourceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceCache")
            .field("resource_count", &self.get_resource_count())
            .field("max_bytes", &self.max_bytes)
            .field("budgeted_bytes", &self.budgeted_bytes)
            .field("purgeable_bytes", &self.purgeable_bytes)
            .finish_non_exhaustive()
    }
}

// Port of: src/gpu/graphite/ResourceCache.h#L178-L181 (chrome/m156)
fn compare_use_token(a: &Arc<dyn AnyResource>, b: &Arc<dyn AnyResource>) -> bool {
    a.base().last_use_token() < b.base().last_use_token()
}

fn set_resource_index(r: &Arc<dyn AnyResource>, index: i32) {
    r.base().set_cache_index(index);
}

fn get_resource_index(r: &Arc<dyn AnyResource>) -> i32 {
    r.base().cache_index()
}

fn same_resource(a: &Arc<dyn AnyResource>, b: &ResourceBase) -> bool {
    a.base().unique_id() == b.unique_id()
}

impl ResourceCache {
    /// `ResourceCache::Make(singleOwner, recorderID, maxBytes)`. A proxy cache is created when
    /// `recorder_id` is valid (non-zero).
    // Port of: src/gpu/graphite/ResourceCache.cpp#L57-L74 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new(recorder_id: u32, max_bytes: usize) -> Self {
        Self {
            purgeable_queue: TDPQueue::with_index(
                compare_use_token,
                set_resource_index,
                get_resource_index,
            ),
            nonpurgeable_resources: Vec::new(),
            resource_map: HashMap::new(),
            resource_map_count: 0,
            proxy_cache: (recorder_id != INVALID_GEN_ID).then(|| ProxyCache::new(recorder_id)),
            max_bytes,
            budgeted_bytes: 0,
            purgeable_bytes: 0,
            use_token: 0,
            return_queue: Arc::new(ReturnQueue::new()),
            count: 0,
        }
    }

    /// `shutdown()`: called by the `ResourceProvider` when it drops the cache. No more resources
    /// can be returned (besides those already in the return queue) or retrieved afterwards.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L81-L124 (chrome/m156)
    pub fn shutdown(&mut self) {
        // At this point no more changes will happen to the return queue or the resources within
        // it. We do need to finish processing them for a graceful shutdown.
        self.process_returned_resources_impl(true);

        if let Some(proxy_cache) = self.proxy_cache.as_mut() {
            proxy_cache.purge_all();
            // NOTE: any resources that would become purgeable or reusable from purging the proxy
            // cache are not added to the return queue and remain in the nonpurgeable array.
            // Below their cache ref will be removed, causing them to be deleted immediately.
        }

        while let Some(back) = self.nonpurgeable_resources.last().cloned() {
            debug_assert!(!back.base().was_destroyed());
            self.remove_from_nonpurgeable_array(&back);
            // Resources will delete themselves as needed.
            unref_cache(&back);
        }

        while self.purgeable_queue.count() > 0 {
            let top = self.purgeable_queue.peek().clone();
            debug_assert!(!top.base().was_destroyed());
            self.remove_from_purgeable_queue(&top);
            unref_cache(&top);
        }
    }

    /// `insertResource()`: registers a newly created resource with the cache.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L126-L180 (chrome/m156)
    #[doc(alias = "insertResource")]
    pub fn insert_resource<T: ResourceObject>(
        &mut self,
        resource: &ResourceRef<T>,
        key: &GraphiteResourceKey,
        budgeted: Budgeted,
        shareable: Shareable,
    ) {
        let resource = &resource.erased();
        let base = resource.base();
        debug_assert!(key.is_valid());
        debug_assert!(shareable == Shareable::No || budgeted == Budgeted::Yes);

        debug_assert!(!self.is_in_cache(resource.base()));
        debug_assert!(!base.was_destroyed());
        debug_assert!(!base.is_purgeable());
        debug_assert!(!base.key().is_valid());
        // All resources in the cache are owned.
        debug_assert_eq!(base.ownership(), Ownership::Owned);

        // Make sure we have the most accurate memory size for "memoryless" resources.
        update_gpu_memory_size(resource);

        // Return resources first to get an accurate accounting of our memory usage (some can go
        // from unbudgeted to budgeted when they return), unless the new resource has a size of 0.
        if base.gpu_memory_size() > 0 {
            self.process_returned_resources();
        }

        register_with_cache(resource, &self.return_queue, key, budgeted, shareable);

        // We must set the use token before adding to the array in case the token wraps and we
        // wind up iterating over all the resources that already have use tokens.
        let token = self.get_next_use_token();
        Self::set_resource_use_token(resource, token);
        base.update_access_time();

        self.add_to_nonpurgeable_array(resource);

        self.count += 1;

        if base.shareable() != Shareable::No {
            // Scratch and shareable resources are always available for reuse
            self.add_to_resource_map(resource);
        }

        if base.budgeted() == Budgeted::Yes {
            self.budgeted_bytes += base.gpu_memory_size();
        }

        self.purge_as_needed();
    }

    /// `findAndRefResource()`: a resource matching `key`, with a new usage ref. For
    /// [`Shareable::Scratch`] requests, `unavailable` filters out resources already in use.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L182-L264 (chrome/m156)
    #[doc(alias = "findAndRefResource")]
    pub fn find_and_ref_resource(
        &mut self,
        key: &GraphiteResourceKey,
        budgeted: Budgeted,
        shareable: Shareable,
        label: &str,
        unavailable: Option<&ScratchResourceSet>,
    ) -> Option<AnyResourceRef> {
        debug_assert!(key.is_valid());
        debug_assert!(shareable == Shareable::No || budgeted == Budgeted::Yes);
        debug_assert!(shareable != Shareable::Scratch || unavailable.is_some());

        let shareable_predicate = |r: &Arc<dyn AnyResource>| {
            // If the resource is in the map then it's available, so a non-shareable state means it
            // really has no outstanding uses and can be converted to any other shareable state.
            // Otherwise it can only be reused with the same mode. Additionally, scratch resources
            // cannot already be in the `unavailable` set passed in.
            let rs = r.base().shareable();
            (rs == Shareable::No || rs == shareable)
                && (shareable != Shareable::Scratch
                    || !unavailable.is_some_and(|u| u.contains(&r.base().unique_id())))
        };

        let mut resource = self.find_in_map(key, shareable_predicate);
        if resource.is_none() {
            // Process the return queue only if we first failed to find a matching resource.
            if self.process_returned_resources() {
                resource = self.find_in_map(key, shareable_predicate);
            }
        }
        let result = resource.map(|resource| {
            let base = resource.base();
            // All resources we pull out of the cache for use should be budgeted
            debug_assert_eq!(base.budgeted(), Budgeted::Yes);
            debug_assert_eq!(base.key(), key);

            if shareable == Shareable::No {
                // If the returned resource is no longer shareable then we remove it from the map
                // so that it isn't found again.
                debug_assert_eq!(base.shareable(), Shareable::No);
                self.remove_from_resource_map(&resource);
                if budgeted == Budgeted::No {
                    base.set_budgeted(Budgeted::No);
                    self.budgeted_bytes -= base.gpu_memory_size();
                }
                // It is safe to update non-shareable resources when returning them from the cache.
                base.set_label(label);
                synchronize_backend_label(&*resource);
            } else {
                // Shareable and scratch resources should never be requested as non-budgeted
                debug_assert_eq!(budgeted, Budgeted::Yes);

                if shareable == Shareable::Scratch {
                    base.set_label(label);
                    synchronize_backend_label(&*resource);
                } else {
                    // Shareable resource labels should never change after initial creation.
                    debug_assert!(shareable == Shareable::Yes && base.label() == label);
                }

                base.set_shareable(shareable);
            }
            let usage_ref = self.ref_and_make_resource_mru(&resource);
            self.validate();
            usage_ref
        });

        // processReturnedResources may have added resources back into our budget, but we delay
        // purging until now so we don't purge a resource we're looking for in this function.
        self.purge_as_needed();

        result
    }

    // `fResourceMap.find(key, predicate)`: the first match in the key's list.
    // Port of: src/core/SkTMultiMap.h#L108-L118 (chrome/m156)
    fn find_in_map(
        &self,
        key: &GraphiteResourceKey,
        predicate: impl Fn(&Arc<dyn AnyResource>) -> bool,
    ) -> Option<Arc<dyn AnyResource>> {
        self.resource_map
            .get(key)
            .and_then(|list| list.iter().find(|r| predicate(r)).cloned())
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L266-L279 (chrome/m156)
    fn ref_and_make_resource_mru(&mut self, resource: &Arc<dyn AnyResource>) -> AnyResourceRef {
        debug_assert!(self.is_in_cache(resource.base()));

        if self.in_purgeable_queue(resource.base()) {
            // It's about to become unpurgeable.
            self.remove_from_purgeable_queue(resource);
            self.add_to_nonpurgeable_array(resource);
        }
        let usage_ref = initial_usage_ref(resource);

        let token = self.get_next_use_token();
        Self::set_resource_use_token(resource, token);
        self.validate();
        usage_ref
    }

    /// `forceProcessReturnedResources()`.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L281-L284 (chrome/m156)
    #[doc(alias = "forceProcessReturnedResources")]
    pub fn force_process_returned_resources(&mut self) {
        self.process_returned_resources();
    }

    // Returns true if any resources were actually returned to the cache.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L376-L396 (chrome/m156)
    fn process_returned_resources(&mut self) -> bool {
        self.process_returned_resources_impl(false)
    }

    fn process_returned_resources_impl(&mut self, shutdown: bool) -> bool {
        // Move the returned resources off of the return queue before processing them so that we
        // can manipulate the resources without blocking subsequent returns on other threads.
        let old_queue = self.return_queue.take(shutdown);

        let mut return_count = 0;
        for resource in old_queue {
            return_count += 1;
            self.process_returned_resource(&resource);
        }

        return_count > 0
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L398-L511 (chrome/m156)
    fn process_returned_resource(&mut self, resource: &Arc<dyn AnyResource>) {
        let base = resource.base();
        // A resource should not have been destroyed when placed into the return queue.
        debug_assert!(!base.was_destroyed());
        debug_assert!(self.is_in_cache(resource.base()));

        let (is_reusable, is_purgeable) = unref_return_queue(resource);

        if base.shareable() != Shareable::No {
            // Shareable resources should still be discoverable in the resource map
            debug_assert!(self.map_has(resource));
            debug_assert!(base.is_available_for_reuse());

            // Reset the resource's sharing mode so that any shareable request can use it. This is
            // only safe when there are no outstanding usage refs.
            if is_reusable {
                base.set_shareable(Shareable::No);
            }
        } else if is_reusable {
            // Non-shareable resources are removed from the resource map when they are given out by
            // the cache. Becoming purgeable always implies becoming reusable, so as long as a
            // previous return hasn't put it into the resource map already, we do that now.
            if !base.is_available_for_reuse() {
                debug_assert!(!self.map_has(resource));
                self.add_to_resource_map(resource);

                if base.budgeted() == Budgeted::No {
                    base.set_budgeted(Budgeted::Yes);
                    self.budgeted_bytes += base.gpu_memory_size();
                }
            }

            debug_assert!(self.map_has(resource));
            debug_assert!(base.is_available_for_reuse());
            debug_assert!(base.is_usable_as_scratch());
        } else {
            // This was a stale entry in the return queue, which can arise when a Resource becomes
            // reusable while it has outstanding command buffer refs.
            debug_assert!(!self.map_has(resource));
            debug_assert!(!base.is_available_for_reuse());
        }

        // Update GPU budget now that the budget policy is up to date.
        let old_size = base.gpu_memory_size();
        update_gpu_memory_size(resource);
        if old_size != base.gpu_memory_size() && base.budgeted() == Budgeted::Yes {
            self.budgeted_bytes -= old_size;
            self.budgeted_bytes += base.gpu_memory_size();
        }

        let token = self.get_next_use_token();
        Self::set_resource_use_token(resource, token);

        // If the resource was not purgeable at the time the return queue ref was released, the
        // resource should still be in the non-purgeable array from when it was originally given
        // out.
        debug_assert!(self.in_nonpurgeable_array(resource.base()));
        if !is_purgeable {
            self.validate();
            return;
        }

        // Since the resource is purgeable, only the current cache thread can add new refs.
        self.remove_from_nonpurgeable_array(resource);

        if base.should_delete_asap() == DeleteAsap::Yes {
            self.purge_resource(resource);
        } else {
            // We don't purge this resource immediately even if we are overbudget. This allows
            // later purge_as_needed() calls to prioritize deleting less-recently-used Resources.
            base.update_access_time();
            self.purgeable_queue.insert(Arc::clone(resource));
            self.purgeable_bytes += base.gpu_memory_size();
        }
        self.validate();
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L513-L519 (chrome/m156)
    fn add_to_resource_map(&mut self, resource: &Arc<dyn AnyResource>) {
        debug_assert!(self.is_in_cache(resource.base()));
        debug_assert!(!resource.base().is_available_for_reuse());
        debug_assert!(!self.map_has(resource));
        // SkTMultiMap::insert: the new value becomes the head of the key's list.
        self.resource_map
            .entry(resource.base().key().clone())
            .or_default()
            .insert(0, Arc::clone(resource));
        self.resource_map_count += 1;
        resource.base().set_available_for_reuse(true);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L521-L527 (chrome/m156)
    fn remove_from_resource_map(&mut self, resource: &Arc<dyn AnyResource>) {
        debug_assert!(self.is_in_cache(resource.base()));
        debug_assert!(resource.base().is_available_for_reuse());
        debug_assert!(self.map_has(resource));
        let key = resource.base().key();
        if let Some(list) = self.resource_map.get_mut(key)
            && let Some(pos) = list.iter().position(|r| same_resource(r, resource.base()))
        {
            list.remove(pos);
            if list.is_empty() {
                self.resource_map.remove(key);
            }
            self.resource_map_count -= 1;
        }
        resource.base().set_available_for_reuse(false);
    }

    // `fResourceMap.has(resource, resource->key())`.
    fn map_has(&self, resource: &Arc<dyn AnyResource>) -> bool {
        self.resource_map
            .get(resource.base().key())
            .is_some_and(|list| list.iter().any(|r| same_resource(r, resource.base())))
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L529-L535 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTDArray index is int
    fn add_to_nonpurgeable_array(&mut self, resource: &Arc<dyn AnyResource>) {
        debug_assert!(!self.in_nonpurgeable_array(resource.base()));

        let index = self.nonpurgeable_resources.len();
        self.nonpurgeable_resources.push(Arc::clone(resource));
        resource.base().set_cache_index(index as i32);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L537-L549 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the index is valid while in the array
    fn remove_from_nonpurgeable_array(&mut self, resource: &Arc<dyn AnyResource>) {
        debug_assert!(self.in_nonpurgeable_array(resource.base()));

        let index = resource.base().cache_index();
        // Fill the hole we will create in the array with the tail object, adjust its index, and
        // then pop the array
        let tail = Arc::clone(
            self.nonpurgeable_resources
                .last()
                .expect("non-empty nonpurgeable array"),
        );
        debug_assert!(same_resource(
            &self.nonpurgeable_resources[index as usize],
            resource.base()
        ));
        self.nonpurgeable_resources[index as usize] = Arc::clone(&tail);
        tail.base().set_cache_index(index);
        self.nonpurgeable_resources.pop();
        resource.base().set_cache_index(-1);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L551-L562 (chrome/m156)
    fn remove_from_purgeable_queue(&mut self, resource: &Arc<dyn AnyResource>) {
        debug_assert!(self.in_purgeable_queue(resource.base()));

        self.purgeable_queue.remove(resource);
        self.purgeable_bytes -= resource.base().gpu_memory_size();
        // SkTDPQueue will set the index back to -1 in debug builds, but we are using the index as
        // a flag for whether the Resource has been purged from the cache or not. So we need to
        // make sure it always gets set.
        resource.base().set_cache_index(-1);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L564-L568 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // checked non-negative first
    fn in_purgeable_queue(&self, resource: &ResourceBase) -> bool {
        let index = resource.cache_index();
        index >= 0
            && (index as usize) < self.purgeable_queue.count()
            && same_resource(self.purgeable_queue.at(index as usize), resource)
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L572-L576 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // checked non-negative first
    fn in_nonpurgeable_array(&self, resource: &ResourceBase) -> bool {
        let index = resource.cache_index();
        index >= 0
            && (index as usize) < self.nonpurgeable_resources.len()
            && same_resource(&self.nonpurgeable_resources[index as usize], resource)
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L578-L589 (chrome/m156)
    fn is_in_cache(&self, resource: &ResourceBase) -> bool {
        if self.in_purgeable_queue(resource) || self.in_nonpurgeable_array(resource) {
            debug_assert!(resource.has_cache_ref());
            debug_assert!(
                resource
                    .return_cache()
                    .is_some_and(|q| Arc::ptr_eq(&q, &self.return_queue))
            );
            return true;
        }
        // Resource index should have been set to -1 if the resource is not in the cache
        debug_assert_eq!(resource.cache_index(), -1);
        false
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L593-L613 (chrome/m156)
    fn purge_resource(&mut self, resource: &Arc<dyn AnyResource>) {
        let base = resource.base();
        debug_assert!(base.is_purgeable());

        self.remove_from_resource_map(resource);

        if base.should_delete_asap() == DeleteAsap::No {
            debug_assert!(self.in_purgeable_queue(resource.base()));
            self.remove_from_purgeable_queue(resource);
        }

        debug_assert!(!self.is_in_cache(resource.base()));
        // Unbudgeted resources always transition through becoming reusable and budgeted before
        // they are purged.
        debug_assert_eq!(base.budgeted(), Budgeted::Yes);
        self.budgeted_bytes -= base.gpu_memory_size();
        unref_cache(resource);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L615-L641 (chrome/m156)
    fn purge_as_needed(&mut self) {
        if self.overbudget()
            && let Some(proxy_cache) = self.proxy_cache.as_mut()
        {
            proxy_cache.free_uniquely_held();

            // After the image cache frees resources we need to return those resources to the cache
            self.process_returned_resources();
        }
        while self.overbudget() && self.purgeable_queue.count() > 0 {
            let resource = self.purgeable_queue.peek().clone();
            debug_assert!(!resource.base().was_destroyed());
            debug_assert!(self.map_has(&resource));

            if resource.base().last_use_token() == MAX_USE_TOKEN {
                // If we hit a resource that is at MAX_USE_TOKEN, then we've hit the part of the
                // purgeable queue with all zero sized resources. We don't want to actually remove
                // those so we just break here.
                debug_assert_eq!(resource.base().gpu_memory_size(), 0);
                break;
            }

            self.purge_resource(&resource);
        }

        self.validate();
    }

    /// `purgeResourcesNotUsedSince()`: purges resources not used since `purge_time`, stopping at
    /// `quit_purging_time` if given. Zero-sized resources are not purged for being over budget.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L643-L648 (chrome/m156)
    #[doc(alias = "purgeResourcesNotUsedSince")]
    pub fn purge_resources_not_used_since(
        &mut self,
        purge_time: StdSteadyClockTimePoint,
        quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
        self.purge_resources_impl(Some(purge_time), quit_purging_time);
    }

    /// `purgeResources()`: purges every unlocked resource.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L650-L653 (chrome/m156)
    #[doc(alias = "purgeResources")]
    pub fn purge_resources(&mut self) {
        self.purge_resources_impl(None, None);
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L655-L710 (chrome/m156)
    fn purge_resources_impl(
        &mut self,
        purge_time: Option<StdSteadyClockTimePoint>,
        quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
        if let Some(proxy_cache) = self.proxy_cache.as_mut() {
            proxy_cache.purge_proxies_not_used_since(purge_time, quit_purging_time);
        }
        self.process_returned_resources();

        let time_remains_before_stop_time =
            || quit_purging_time.is_none_or(|quit| StdSteadyClockTimePoint::now() < quit);

        // Early out if the very first item is too new to purge to avoid sorting the queue when
        // nothing will be deleted or if we have somehow already exceeded the time limit.
        if self.purgeable_queue.count() > 0
            && let Some(purge_time) = purge_time
            && self.purgeable_queue.peek().base().last_access_time() >= purge_time
        {
            return;
        }
        if !time_remains_before_stop_time() {
            return;
        }

        // Sort the queue
        self.purgeable_queue.sort();

        // Make a list of the scratch resources to delete
        let mut resources_to_purge = Vec::new();
        for i in 0..self.purgeable_queue.count() {
            let resource = self.purgeable_queue.at(i);

            let resource_time = resource.base().last_access_time();
            if purge_time.is_some_and(|t| resource_time >= t) {
                // scratch or not, all later iterations will be too recently used to purge.
                break;
            }
            debug_assert!(resource.base().is_purgeable());
            resources_to_purge.push(Arc::clone(resource));
        }

        // Delete the scratch resources. This must be done as a separate pass to avoid messing up
        // the sorted order of the queue.
        for resource in &resources_to_purge {
            self.purge_resource(resource);
            if !time_remains_before_stop_time() {
                break;
            }
        }

        // We could still end up over budget even after purging resources based on purgeTime, so
        // call purge_as_needed at the end (not limited by the purge end time).
        self.purge_as_needed();
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L712-L772 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTDArray index is int
    #[allow(clippy::similar_names)] // Skia's currP/currNP and tsP/tsNP
    fn get_next_use_token(&mut self) -> u32 {
        // If we wrap then all the existing resources will appear older than any resources that get
        // a token after the wrap. We wrap one value early when we reach MAX_USE_TOKEN so that we
        // can continue to use MAX_USE_TOKEN as a special case for zero sized resources.
        if self.use_token == MAX_USE_TOKEN {
            self.use_token = 0;
            let count = self.get_resource_count();
            if count > 0 {
                // Reset all the tokens. We sort the resources by their use token and then assign
                // sequential tokens beginning with 0. This is O(n*lg(n)) but it should be rare.
                let mut sorted_purgeable_resources =
                    Vec::with_capacity(self.purgeable_queue.count());

                while self.purgeable_queue.count() > 0 {
                    sorted_purgeable_resources.push(self.purgeable_queue.peek().clone());
                    self.purgeable_queue.pop();
                }

                t_q_sort(&mut self.nonpurgeable_resources, compare_use_token);

                // Pick resources out of the purgeable and non-purgeable arrays based on lowest use
                // token and assign new tokens.
                let mut curr_p = 0;
                let mut curr_np = 0;
                while curr_p < sorted_purgeable_resources.len()
                    && curr_np < self.nonpurgeable_resources.len()
                {
                    let ts_p = sorted_purgeable_resources[curr_p].base().last_use_token();
                    let ts_np = self.nonpurgeable_resources[curr_np].base().last_use_token();
                    debug_assert_ne!(ts_p, ts_np);
                    if ts_p < ts_np {
                        let r = Arc::clone(&sorted_purgeable_resources[curr_p]);
                        curr_p += 1;
                        let token = self.use_token;
                        self.use_token += 1;
                        Self::set_resource_use_token(&r, token);
                    } else {
                        // Correct the index in the nonpurgeable array stored on the resource
                        // post-sort.
                        let r = Arc::clone(&self.nonpurgeable_resources[curr_np]);
                        r.base().set_cache_index(curr_np as i32);
                        curr_np += 1;
                        let token = self.use_token;
                        self.use_token += 1;
                        Self::set_resource_use_token(&r, token);
                    }
                }

                // The above loop ended when we hit the end of one array. Finish the other one.
                while curr_p < sorted_purgeable_resources.len() {
                    let r = Arc::clone(&sorted_purgeable_resources[curr_p]);
                    curr_p += 1;
                    let token = self.use_token;
                    self.use_token += 1;
                    Self::set_resource_use_token(&r, token);
                }
                while curr_np < self.nonpurgeable_resources.len() {
                    let r = Arc::clone(&self.nonpurgeable_resources[curr_np]);
                    r.base().set_cache_index(curr_np as i32);
                    curr_np += 1;
                    let token = self.use_token;
                    self.use_token += 1;
                    Self::set_resource_use_token(&r, token);
                }

                // Rebuild the queue.
                for r in sorted_purgeable_resources {
                    self.purgeable_queue.insert(r);
                }

                self.validate();
                debug_assert_eq!(count, self.get_resource_count());

                // count should be the next use token we return.
                debug_assert_eq!(self.use_token as usize, count);
            }
        }
        let token = self.use_token;
        self.use_token = self.use_token.wrapping_add(1);
        token
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L774-L780 (chrome/m156)
    fn set_resource_use_token(resource: &Arc<dyn AnyResource>, mut token: u32) {
        // We always set the use token for zero-sized resources to be MAX_USE_TOKEN
        if resource.base().gpu_memory_size() == 0 {
            token = MAX_USE_TOKEN;
        }
        resource.base().set_last_use_token(token);
    }

    /// `proxyCache()`.
    #[doc(alias = "proxyCache")]
    pub fn proxy_cache(&mut self) -> Option<&mut ProxyCache> {
        self.proxy_cache.as_mut()
    }

    /// `getResourceCount()`.
    // Port of: src/gpu/graphite/ResourceCache.h#L80 (chrome/m156)
    #[doc(alias = "getResourceCount")]
    #[must_use]
    pub fn get_resource_count(&self) -> usize {
        self.purgeable_queue.count() + self.nonpurgeable_resources.len()
    }

    /// `getMaxBudget()`.
    #[doc(alias = "getMaxBudget")]
    #[must_use]
    pub fn get_max_budget(&self) -> usize {
        self.max_bytes
    }

    /// `setMaxBudget()`.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L798-L802 (chrome/m156)
    #[doc(alias = "setMaxBudget")]
    pub fn set_max_budget(&mut self, bytes: usize) {
        self.max_bytes = bytes;
        self.process_returned_resources();
        self.purge_as_needed();
    }

    /// `currentBudgetedBytes()`.
    #[doc(alias = "currentBudgetedBytes")]
    #[must_use]
    pub fn current_budgeted_bytes(&self) -> usize {
        self.budgeted_bytes
    }

    /// `currentPurgeableBytes()`.
    #[doc(alias = "currentPurgeableBytes")]
    #[must_use]
    pub fn current_purgeable_bytes(&self) -> usize {
        self.purgeable_bytes
    }

    // Port of: src/gpu/graphite/ResourceCache.h#L139 (chrome/m156)
    fn overbudget(&self) -> bool {
        self.budgeted_bytes > self.max_bytes
    }

    // ---- GPU_TEST_UTILS -----------------------------------------------------------------------

    /// `forcePurgeAsNeeded()` (test utility).
    #[doc(alias = "forcePurgeAsNeeded")]
    pub fn force_purge_as_needed(&mut self) {
        self.purge_as_needed();
    }

    /// `numFindableResources()` (test utility): shared resources plus non-shareable resources that
    /// have been returned to the cache.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L930-L932 (chrome/m156)
    #[doc(alias = "numFindableResources")]
    #[must_use]
    pub fn num_findable_resources(&self) -> usize {
        self.resource_map_count
    }

    /// `topOfPurgeableQueue()` (test utility): the least recently used purgeable resource.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L934-L939 (chrome/m156)
    #[doc(alias = "topOfPurgeableQueue")]
    #[must_use]
    pub fn top_of_purgeable_queue(&self) -> Option<&ResourceBase> {
        if self.purgeable_queue.count() == 0 {
            return None;
        }
        Some(self.purgeable_queue.peek().base())
    }

    /// `testingInPurgeableQueue()` (test utility).
    #[doc(alias = "testingInPurgeableQueue")]
    #[must_use]
    pub fn testing_in_purgeable_queue(&self, resource: &ResourceBase) -> bool {
        self.in_purgeable_queue(resource)
    }

    /// `testingInReturnQueue()` (test utility).
    #[doc(alias = "testingInReturnQueue")]
    #[must_use]
    pub fn testing_in_return_queue(&self, resource: &ResourceBase) -> bool {
        resource.in_return_queue()
    }

    /// `visitTextures()` (test utility).
    // Port of: src/gpu/graphite/ResourceCache.cpp#L941-L953 (chrome/m156)
    #[doc(alias = "visitTextures")]
    pub fn visit_textures(&self, mut func: impl FnMut(&Texture, bool)) {
        for r in &self.nonpurgeable_resources {
            if let Some(tex) = r.object().as_texture() {
                func(tex, /* purgeable= */ false);
            }
        }
        for i in 0..self.purgeable_queue.count() {
            if let Some(tex) = self.purgeable_queue.at(i).object().as_texture() {
                func(tex, /* purgeable= */ true);
            }
        }
    }

    // Port of: src/gpu/graphite/ResourceCache.cpp#L806-L924 (chrome/m156)
    // skia-rust: Skia validates a random sample once the cache holds more than 15 resources; this
    // validates every time (debug builds only).
    fn validate(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        let mut shareable = 0;
        let mut scratch = 0;
        let mut budgeted_bytes = 0;
        let mut purgeable_bytes = 0;

        let mut update = |cache: &Self, resource: &Arc<dyn AnyResource>| {
            let base = resource.base();
            debug_assert!(base.key().is_valid());
            // All resources in the cache are owned.
            debug_assert_eq!(base.ownership(), Ownership::Owned);

            if base.shareable() == Shareable::Yes {
                debug_assert!(base.is_available_for_reuse());
                debug_assert!(cache.map_has(resource));
                debug_assert_eq!(base.budgeted(), Budgeted::Yes);
                shareable += 1;
            } else if base.is_available_for_reuse() {
                // Scratch resources (non-shareable with no refs that are returned, or explicitly
                // scratch shared) are tracked separately from fully shareable.
                debug_assert!(base.is_usable_as_scratch());
                debug_assert!(cache.map_has(resource));
                scratch += 1;
            } else {
                // This should be a non-shareable resource that isn't available for reuse.
                debug_assert_eq!(base.shareable(), Shareable::No);
                debug_assert!(!cache.map_has(resource));
            }

            if base.budgeted() == Budgeted::Yes {
                budgeted_bytes += base.gpu_memory_size();
            }

            if base.gpu_memory_size() == 0 {
                debug_assert_eq!(base.last_use_token(), MAX_USE_TOKEN);
            } else {
                debug_assert!(base.last_use_token() < MAX_USE_TOKEN);
            }

            if cache.in_purgeable_queue(resource.base()) {
                debug_assert!(base.is_purgeable());
                purgeable_bytes += base.gpu_memory_size();
            }
        };

        let mut count = 0;
        for list in self.resource_map.values() {
            for resource in list {
                debug_assert!(
                    resource.base().is_usable_as_scratch()
                        || resource.base().shareable() == Shareable::Yes
                );
                debug_assert_eq!(resource.base().budgeted(), Budgeted::Yes);
                debug_assert!(resource.base().is_available_for_reuse());
                debug_assert!(self.is_in_cache(resource.base()));
                count += 1;
            }
        }
        debug_assert_eq!(count, self.resource_map_count);

        for (i, resource) in self.nonpurgeable_resources.iter().enumerate() {
            debug_assert!(self.is_in_cache(resource.base()));
            debug_assert_eq!(usize::try_from(resource.base().cache_index()).ok(), Some(i));
            debug_assert!(!resource.base().was_destroyed());
            debug_assert!(!self.in_purgeable_queue(resource.base()));
            update(self, resource);
        }
        let mut first_purgeable_is_size_zero = false;
        for i in 0..self.purgeable_queue.count() {
            let resource = self.purgeable_queue.at(i);
            if i == 0 {
                first_purgeable_is_size_zero = resource.base().gpu_memory_size() == 0;
            }
            if first_purgeable_is_size_zero {
                // If the least recently used purgeable resource is sized zero, all other purgeable
                // resources must also be sized zero.
                debug_assert_eq!(resource.base().gpu_memory_size(), 0);
            }
            debug_assert!(self.is_in_cache(resource.base()));
            debug_assert!(resource.base().is_purgeable());
            debug_assert_eq!(usize::try_from(resource.base().cache_index()).ok(), Some(i));
            debug_assert!(!resource.base().was_destroyed());
            update(self, resource);
        }

        debug_assert_eq!(scratch + shareable, self.resource_map_count);
        debug_assert_eq!(budgeted_bytes, self.budgeted_bytes);
        debug_assert_eq!(purgeable_bytes, self.purgeable_bytes);
    }
}

impl Drop for ResourceCache {
    // Skia asserts that the provider shut the cache down first (`~ResourceCache`); the port shuts
    // it down here if that has not happened, so resources never keep a dead cache's queue alive.
    // Port of: src/gpu/graphite/ResourceCache.cpp#L76-L79 (chrome/m156)
    fn drop(&mut self) {
        if !self.return_queue.is_shutdown() {
            self.shutdown();
        }
    }
}
