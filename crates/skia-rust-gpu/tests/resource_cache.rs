// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: tests/graphite/GraphiteResourceCacheTest.cpp

//! The cache-only bodies of Skia's `GraphiteResourceCacheTest`, run against a bare
//! [`ResourceCache`].
//!
//! Skia's tests get their cache from `context->makeRecorder()`, which needs a GPU context (not
//! ported yet), so they cannot be the 1:1 manifest ports; those land with the recorder and the
//! wgpu back end. The assertions below are Skia's, in Skia's order.

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use skia_rust_gpu::gpu::gpu_types::{Budgeted, StdSteadyClockTimePoint};
use skia_rust_gpu::graphite::graphite_resource_key::{
    GraphiteResourceKey, GraphiteResourceKeyBuilder,
};
use skia_rust_gpu::graphite::resource::{
    Resource, ResourceBase, ResourceObject, ResourceRef, ResourceUniqueId,
};
use skia_rust_gpu::graphite::resource_cache::{ResourceCache, ScratchResourceSet};
use skia_rust_gpu::graphite::resource_types::{Ownership, ResourceType, Shareable};

#[derive(Debug, Default)]
struct TestResource {
    freed: Arc<AtomicUsize>,
}

impl ResourceObject for TestResource {
    fn resource_type(&self) -> &'static str {
        "Test Resource"
    }

    fn free_gpu_data(&self) {
        self.freed.fetch_add(1, Ordering::Relaxed);
    }
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L58-L64 (chrome/m156)
fn create_key() -> GraphiteResourceKey {
    // All unit tests that currently use TestResource are able to work with a single Resource, so
    // the key doesn't require any real state.
    static TYPE: LazyLock<ResourceType> =
        LazyLock::new(GraphiteResourceKey::generate_resource_type);
    let mut key = GraphiteResourceKey::new();
    {
        let _builder = GraphiteResourceKeyBuilder::new(&mut key, *TYPE, 0);
    }
    key
}

// TestResource::Make
// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L38-L56 (chrome/m156)
fn make_test_resource(
    cache: &mut ResourceCache,
    owned: Ownership,
    budgeted: Budgeted,
    shareable: Shareable,
    gpu_memory_size: usize,
) -> ResourceRef<TestResource> {
    let resource = Resource::new(
        TestResource::default(),
        owned,
        gpu_memory_size,
        "",
        false,
        false,
    );
    cache.insert_resource(&resource, &create_key(), budgeted, shareable);
    resource
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L286-L302 (chrome/m156)
fn add_new_resource(
    cache: &mut ResourceCache,
    gpu_memory_size: usize,
    budgeted: Budgeted,
) -> ResourceRef<TestResource> {
    make_test_resource(
        cache,
        Ownership::Owned,
        budgeted,
        Shareable::No,
        gpu_memory_size,
    )
}

// Returns the (unique id of the) resource, which is now only held by the cache.
// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L304-L318 (chrome/m156)
fn add_new_purgeable_resource(
    cache: &mut ResourceCache,
    gpu_memory_size: usize,
) -> ResourceUniqueId {
    let resource = add_new_resource(cache, gpu_memory_size, Budgeted::Yes);
    let id = resource.base().unique_id();
    drop(resource);
    cache.force_process_returned_resources();
    id
}

// `resourceCache->topOfPurgeableQueue()`, compared by identity.
fn top_id(cache: &ResourceCache) -> Option<ResourceUniqueId> {
    cache.top_of_purgeable_queue().map(ResourceBase::unique_id)
}

fn new_cache() -> ResourceCache {
    // A recorder's cache: it has a proxy cache.
    ResourceCache::new(1, 256 * (1 << 20))
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L320-L445 (chrome/m156)
#[test]
fn graphite_purge_as_needed_resources_test() {
    let mut cache = new_cache();

    cache.set_max_budget(10);

    let resource_size10 = add_new_resource(&mut cache, 10, Budgeted::Yes);

    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 10);

    let resource_size1 = add_new_resource(&mut cache, 1, Budgeted::Yes);

    // We should now be over budget, but nothing should be purged since neither resource is
    // purgeable.
    assert_eq!(cache.get_resource_count(), 2);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 11);

    // Dropping the ref to the size 1 resource should cause it to get purged when we add a new
    // resource to the cache.
    drop(resource_size1);

    let resource_size2 = add_new_resource(&mut cache, 2, Budgeted::Yes);

    assert_eq!(cache.get_resource_count(), 2);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 12);

    // Reset the cache back to no resources by setting budget to 0.
    drop(resource_size10);
    drop(resource_size2);
    cache.force_process_returned_resources();
    cache.set_max_budget(0);

    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 0);

    // Add a bunch of purgeable resources that keeps us under budget. Nothing should ever get
    // purged.
    cache.set_max_budget(10);
    let resource_size1_ptr = add_new_purgeable_resource(&mut cache, 1);
    let _resource_size2_ptr = add_new_purgeable_resource(&mut cache, 2);
    let resource_size3_ptr = add_new_purgeable_resource(&mut cache, 3);
    let _resource_size4_ptr = add_new_purgeable_resource(&mut cache, 4);

    assert_eq!(cache.get_resource_count(), 4);
    assert_eq!(top_id(&cache), Some(resource_size1_ptr));
    assert_eq!(cache.current_budgeted_bytes(), 10);

    // Add a size 2 resource should purge the original size 1 and size 2
    add_new_purgeable_resource(&mut cache, 2);

    assert_eq!(cache.get_resource_count(), 3);
    assert_eq!(top_id(&cache), Some(resource_size3_ptr));
    assert_eq!(cache.current_budgeted_bytes(), 9);

    // Adding a non-purgeable resource should also trigger resources to be purged from purgeable
    // queue.
    let resource_size10 = add_new_resource(&mut cache, 10, Budgeted::Yes);

    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 10);

    // Adding a resources that is purgeable back to the cache shouldn't trigger the previous
    // non-purgeable resource or itself to be purged yet.
    let resource_size4_ptr = add_new_purgeable_resource(&mut cache, 4);

    assert_eq!(cache.get_resource_count(), 2);
    assert_eq!(top_id(&cache), Some(resource_size4_ptr));
    assert_eq!(cache.current_budgeted_bytes(), 14);

    // Resetting the budget to 0 should trigger purging the size 4 purgeable resource but should
    // leave the non purgeable size 10 alone.
    cache.set_max_budget(0);
    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 10);

    drop(resource_size10);
    cache.force_process_returned_resources();
    cache.force_purge_as_needed();

    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(top_id(&cache), None);
    assert_eq!(cache.current_budgeted_bytes(), 0);
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L447-L537 (chrome/m156)
#[test]
fn graphite_zero_sized_resources_test() {
    let mut cache = new_cache();

    // First make a normal resource that has a non zero size
    let resource_ptr = add_new_purgeable_resource(&mut cache, 1);

    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(cache.num_findable_resources(), 1);
    assert_eq!(top_id(&cache), Some(resource_ptr));

    // First confirm if we set the max budget to zero, this sized resource is removed.
    cache.set_max_budget(0);
    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(cache.num_findable_resources(), 0);
    assert_eq!(top_id(&cache), None);

    // Set the budget back to something higher
    cache.set_max_budget(100);

    // Now create a zero sized resource and add it to the cache.
    let resource_ptr = add_new_purgeable_resource(&mut cache, 0);

    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(cache.num_findable_resources(), 1);
    assert_eq!(top_id(&cache), Some(resource_ptr));

    // Setting the budget down to 0 should not cause the zero sized resource to be purged
    cache.set_max_budget(0);
    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(cache.num_findable_resources(), 1);
    assert_eq!(top_id(&cache), Some(resource_ptr));

    // Now add a sized resource to cache. Set budget higher again so that it fits
    cache.set_max_budget(100);

    let sized_resource_ptr = add_new_purgeable_resource(&mut cache, 1);

    assert_eq!(cache.get_resource_count(), 2);
    assert_eq!(cache.num_findable_resources(), 2);
    // Even though the zero sized resource was added to the cache first, the top of the purgeable
    // stack should be the sized resource.
    assert_eq!(top_id(&cache), Some(sized_resource_ptr));

    // Add another zero sized resource
    add_new_purgeable_resource(&mut cache, 0);

    assert_eq!(cache.get_resource_count(), 3);
    assert_eq!(cache.num_findable_resources(), 3);
    // Again the sized resource should still be the top of the purgeable queue
    assert_eq!(top_id(&cache), Some(sized_resource_ptr));

    // If we set the cache budget to 0, it should clear out the sized resource but leave the two
    // zero-sized resources.
    cache.set_max_budget(0);
    assert_eq!(cache.get_resource_count(), 2);
    assert_eq!(cache.num_findable_resources(), 2);
    assert_eq!(
        cache
            .top_of_purgeable_queue()
            .map(ResourceBase::gpu_memory_size),
        Some(0)
    );

    // However, purging all resources should clear the zero-sized resources.
    cache.purge_resources();
    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(cache.num_findable_resources(), 0);
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L543-L550 (chrome/m156)
fn force_newer_timepoint(prev_time: StdSteadyClockTimePoint) -> StdSteadyClockTimePoint {
    let mut time = StdSteadyClockTimePoint::now();
    while time <= prev_time {
        time = StdSteadyClockTimePoint::now();
    }
    time
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L552-L638 (chrome/m156)
#[test]
fn graphite_purge_not_used_since_resources_test() {
    let mut cache = new_cache();

    // Basic test where we purge 1 resource
    let before_time = StdSteadyClockTimePoint::now();

    add_new_purgeable_resource(&mut cache, 1);

    assert_eq!(cache.get_resource_count(), 1);

    let after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

    // purging beforeTime should not get rid of the resource
    cache.purge_resources_not_used_since(before_time, None);

    assert_eq!(cache.get_resource_count(), 1);

    // purging at afterTime which is after resource became purgeable should purge it.
    cache.purge_resources_not_used_since(after_time, None);

    assert_eq!(cache.get_resource_count(), 0);

    // Test making 2 purgeable resources, but asking to purge on a time between the two.
    let resource_ptr1 = add_new_purgeable_resource(&mut cache, 1);

    let between_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

    let resource_ptr2 = add_new_purgeable_resource(&mut cache, 1);

    let after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

    assert_eq!(cache.get_resource_count(), 2);
    // skia-rust: Skia checks `testingInPurgeableQueue` on both raw pointers; both are purgeable
    // and the older one is the top of the queue.
    assert_eq!(top_id(&cache), Some(resource_ptr1));

    cache.purge_resources_not_used_since(between_time, None);

    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(top_id(&cache), Some(resource_ptr2));

    cache.purge_resources_not_used_since(after_time, None);
    assert_eq!(cache.get_resource_count(), 0);

    // purgeResourcesNotUsedSince should have no impact on non-purgeable resources
    let resource = add_new_resource(&mut cache, 1, Budgeted::Yes);

    assert_eq!(cache.get_resource_count(), 1);

    let after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());
    cache.purge_resources_not_used_since(after_time, None);
    assert_eq!(cache.get_resource_count(), 1);
    assert!(!cache.testing_in_purgeable_queue(resource.base()));

    let id = resource.base().unique_id();
    drop(resource);
    // purgeResourcesNotUsedSince should check the mailbox for the returned resource. Though the
    // time is set before that happens so nothing should purge.
    cache.purge_resources_not_used_since(StdSteadyClockTimePoint::now(), None);
    assert_eq!(cache.get_resource_count(), 1);
    assert_eq!(top_id(&cache), Some(id));

    // Now it should be purged since it is already purgeable
    cache.purge_resources_not_used_since(
        force_newer_timepoint(StdSteadyClockTimePoint::now()),
        None,
    );
    assert_eq!(cache.get_resource_count(), 0);
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L643-L730 (chrome/m156)
#[test]
fn graphite_purge_not_used_over_budget_test() {
    let mut cache = new_cache();

    // set resourceCache budget to 10 for testing.
    let budget: usize = 10;
    cache.set_max_budget(budget);

    // First make a purgeable resources
    let resource_ptr = add_new_purgeable_resource(&mut cache, 1);

    // Now create a bunch of non purgeable (yet) resources that are not budgeted, but will cause us
    // to go over our budget limit when they do return to cache.
    let resource1 = add_new_resource(&mut cache, budget - 1, Budgeted::No);
    let resource2 = add_new_resource(&mut cache, budget - 2, Budgeted::No);
    let resource3 = add_new_resource(&mut cache, budget - 3, Budgeted::No);
    let watch1 = resource1.base().unique_id();
    let watch2 = resource2.base().unique_id();
    let watch3 = resource3.base().unique_id();

    assert_eq!(cache.get_resource_count(), 4);
    assert_eq!(cache.current_budgeted_bytes(), 1);

    let time_before_returning_to_cache = StdSteadyClockTimePoint::now();

    // Now reset all the non budgeted resources so they return to the cache and become budgeted.
    drop(resource1);
    drop(resource2);
    drop(resource3);

    cache.force_process_returned_resources();

    assert_eq!(cache.get_resource_count(), 4);
    assert_eq!(cache.current_budgeted_bytes(), 25);
    assert_eq!(top_id(&cache), Some(resource_ptr));

    // The original resource is older than the time so it gets purged; being over budget then
    // purges two of the others.
    cache.purge_resources_not_used_since(time_before_returning_to_cache, None);
    assert_eq!(cache.get_resource_count(), 1);
    let remaining = top_id(&cache);
    if cache.current_budgeted_bytes() == budget - 1 {
        assert_eq!(remaining, Some(watch1));
    } else if cache.current_budgeted_bytes() == budget - 2 {
        assert_eq!(remaining, Some(watch2));
    } else {
        assert_eq!(cache.current_budgeted_bytes(), budget - 3);
        assert_eq!(remaining, Some(watch3));
    }
    // skia-rust: the return queue is processed last-in first-out, as Skia's linked list is, so
    // the first resource dropped is the most recently used and the one that survives.
    assert_eq!(remaining, Some(watch1));
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L734-L799 (chrome/m156)
#[test]
fn graphite_purge_resources_test() {
    let mut cache = new_cache();

    // set resourceCache budget to 10 for testing.
    cache.set_max_budget(10);

    // Basic test where we purge 1 resource
    add_new_purgeable_resource(&mut cache, 1);

    assert_eq!(cache.get_resource_count(), 1);

    // purging should purge the one unlocked resource.
    cache.purge_resources();
    assert_eq!(cache.get_resource_count(), 0);

    // Test making 2 purgeable resources
    add_new_purgeable_resource(&mut cache, 1);
    add_new_purgeable_resource(&mut cache, 1);

    assert_eq!(cache.get_resource_count(), 2);

    cache.purge_resources();
    assert_eq!(cache.get_resource_count(), 0);

    // purgeResources should have no impact on non-purgeable resources
    let resource = add_new_resource(&mut cache, 1, Budgeted::Yes);

    assert_eq!(cache.get_resource_count(), 1);

    cache.purge_resources();
    assert_eq!(cache.get_resource_count(), 1);
    assert!(!cache.testing_in_purgeable_queue(resource.base()));

    let freed = Arc::clone(&resource.freed);
    drop(resource);
    cache.purge_resources();
    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(freed.load(Ordering::Relaxed), 1);
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L801-L897 (chrome/m156)
#[test]
fn graphite_scratch_resources_test() {
    let mut cache = new_cache();

    assert_eq!(cache.get_resource_count(), 0);
    assert_eq!(cache.num_findable_resources(), 0);

    // Test making a non budgeted, non shareable resource.
    let resource = make_test_resource(&mut cache, Ownership::Owned, Budgeted::No, Shareable::No, 1);
    let resource_ptr = resource.base().unique_id();

    assert_eq!(resource.base().budgeted(), Budgeted::No);
    assert_eq!(cache.get_resource_count(), 1);
    // Resource is not shareable and we have a ref on it. Thus it shouldn't be findable.
    assert_eq!(cache.num_findable_resources(), 0);

    // Requesting a scratch shareable resource will not return the non-shareable resource.
    let key = create_key();

    let mut unavailable = ScratchResourceSet::new();

    assert_eq!(&key, resource.base().key());
    let resource_ptr2 = cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        "",
        Some(&unavailable),
    );
    assert!(resource_ptr2.is_none());

    // Return the non-shareable resource and verify that it can now be requested as scratch
    drop(resource);
    cache.force_process_returned_resources();
    assert_eq!(cache.num_findable_resources(), 1);

    let resource = cache
        .find_and_ref_resource(
            &key,
            Budgeted::Yes,
            Shareable::Scratch,
            "",
            Some(&unavailable),
        )
        .expect("scratch resource");
    assert_eq!(resource.base().unique_id(), resource_ptr);
    assert_eq!(resource.base().budgeted(), Budgeted::Yes);
    assert_eq!(resource.base().shareable(), Shareable::Scratch);
    assert_eq!(cache.num_findable_resources(), 1); // still findable

    // A request of the same key as non-shareable will not return the scratch resource
    let resource_ptr2 = cache.find_and_ref_resource(&key, Budgeted::Yes, Shareable::No, "", None);
    assert!(resource_ptr2.is_none());

    // Similarly, a request for a fully shareable resource cannot be satisfied by a scratch resource
    let resource_ptr2 = cache.find_and_ref_resource(&key, Budgeted::Yes, Shareable::Yes, "", None);
    assert!(resource_ptr2.is_none());

    // A request for another scratch resource can return the existing one if it hasn't been
    // marked unavailable in the set passed to the cache.
    let resource_ptr2 = cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        "",
        Some(&unavailable),
    );
    assert_eq!(
        resource_ptr2.as_ref().map(|r| r.base().unique_id()),
        Some(resource_ptr)
    );
    drop(resource_ptr2);

    // Mark the original resource as unvailable and now it shouldn't be seen by the request.
    unavailable.insert(resource_ptr);
    let resource_ptr2 = cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        "",
        Some(&unavailable),
    );
    assert!(resource_ptr2.is_none());

    // Return the scratch resource, and then simulate a threading race where there's a request for
    // the scratch resource that comes in before the return queue is processed (adding a usage
    // ref), and then the queue is processed as part of a non-shareable request.
    unavailable.clear();
    drop(resource);
    let resource = cache
        .find_and_ref_resource(
            &key,
            Budgeted::Yes,
            Shareable::Scratch,
            "",
            Some(&unavailable),
        )
        .expect("scratch resource");
    assert_eq!(resource.base().unique_id(), resource_ptr);
    // At this point, resourcePtr has a usage ref and should be in the return queue
    assert!(cache.testing_in_return_queue(resource.base()));
    cache.force_process_returned_resources();
    // Its shareable type should not have changed after being processed.
    assert!(!cache.testing_in_return_queue(resource.base()));
    assert_eq!(resource.base().shareable(), Shareable::Scratch);

    // Now actually return the resource and confirm that it can be used for non-shareable requests
    // once all usage refs are dropped.
    drop(resource);
    cache.force_process_returned_resources();
    assert_eq!(cache.num_findable_resources(), 1);
    // skia-rust: Skia reads `resourcePtr` through a raw pointer; it is now the only (purgeable)
    // resource, so it is the top of the purgeable queue.
    let returned = cache.top_of_purgeable_queue().expect("returned resource");
    assert_eq!(returned.unique_id(), resource_ptr);
    assert_eq!(returned.shareable(), Shareable::No);

    // Returning the scratch resource allows it to be changed to a different shareable type
    let resource_ptr2 = cache
        .find_and_ref_resource(&key, Budgeted::Yes, Shareable::Yes, "", None)
        .expect("shareable resource");
    assert_eq!(resource_ptr2.base().unique_id(), resource_ptr);
    assert_eq!(resource_ptr2.base().shareable(), Shareable::Yes);
    drop(resource_ptr2);
}

#[test]
fn resources_are_freed_on_shutdown() {
    let mut cache = new_cache();
    let held = add_new_resource(&mut cache, 4, Budgeted::Yes);
    add_new_purgeable_resource(&mut cache, 4);
    let freed = Arc::clone(&held.freed);
    drop(cache);
    // The cache dropped its ref; the resource lives on until its last usage ref goes away.
    assert!(!held.base().was_destroyed());
    drop(held);
    assert_eq!(freed.load(Ordering::Relaxed), 1);
}

#[test]
fn command_buffer_refs_keep_resources_non_purgeable() {
    let mut cache = new_cache();
    let r = add_new_resource(&mut cache, 4, Budgeted::Yes);
    let cb = r.ref_command_buffer();
    let id = r.base().unique_id();
    drop(r);
    cache.force_process_returned_resources();
    // Reusable (usage refs are 0) but not purgeable.
    assert_eq!(cache.num_findable_resources(), 1);
    assert_eq!(top_id(&cache), None);
    drop(cb);
    cache.force_process_returned_resources();
    assert_eq!(top_id(&cache), Some(id));
}
