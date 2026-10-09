// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/GraphiteResourceCacheTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ tests, which declare constants and similarly named bindings inline.
#![allow(clippy::items_after_statements, clippy::similar_names)]

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use skia_rust_gpu::gpu::gpu_types::{Budgeted, StdSteadyClockTimePoint};
use skia_rust_gpu::graphite::graphite_resource_key::{
    GraphiteResourceKey, GraphiteResourceKeyBuilder,
};
use skia_rust_gpu::graphite::resource::{AnyResource, Resource, ResourceObject, ResourceRef};
use skia_rust_gpu::graphite::resource_cache::{ResourceCache, ScratchResourceSet};
use skia_rust_gpu::graphite::resource_types::{Ownership, ResourceType, Shareable};

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// The tests below use raw `Resource*` pointers to name resources the cache owns; the port uses an
// `Arc` clone of the resource, which keeps its memory (not a usage ref) alive.
type ResourcePtr = Arc<Resource<TestResource>>;

#[derive(Debug)]
struct TestResource;

impl ResourceObject for TestResource {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L51 (chrome/m156)
    fn resource_type(&self) -> &'static str {
        "Test Resource"
    }

    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L63 (chrome/m156)
    fn free_gpu_data(&self) {}
}

impl TestResource {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L38-L49 (chrome/m156)
    fn make(
        resource_cache: &mut ResourceCache,
        owned: Ownership,
        budgeted: Budgeted,
        shareable: Shareable,
        gpu_memory_size: usize,
    ) -> ResourceRef<TestResource> {
        let resource = Resource::new(TestResource, owned, gpu_memory_size, "", false, false);

        let key = Self::create_key();

        resource_cache.insert_resource(&resource, &key, budgeted, shareable);
        resource
    }

    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L53-L59 (chrome/m156)
    fn create_key() -> GraphiteResourceKey {
        // All unit tests that currently use TestResource are able to work with a single Resource,
        // so the key doesn't require any real state.
        static K_TYPE: LazyLock<ResourceType> =
            LazyLock::new(GraphiteResourceKey::generate_resource_type);
        let mut key = GraphiteResourceKey::new();
        {
            let _builder = GraphiteResourceKeyBuilder::new(&mut key, *K_TYPE, 0);
        }
        key
    }
}

// `resourceCache->topOfPurgeableQueue() == resourcePtr`.
fn top_of_purgeable_queue_is(resource_cache: &ResourceCache, resource: &ResourcePtr) -> bool {
    resource_cache
        .top_of_purgeable_queue()
        .is_some_and(|top| top.unique_id() == resource.base().unique_id())
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L286-L302 (chrome/m156)
// (`TestResource::Make` cannot fail here, so the `nullptr` checks of the callers are dropped.)
fn add_new_resource(
    resource_cache: &mut ResourceCache,
    gpu_memory_size: usize,
    budgeted: Budgeted,
) -> ResourceRef<TestResource> {
    TestResource::make(
        resource_cache,
        Ownership::Owned,
        budgeted,
        Shareable::No,
        gpu_memory_size,
    )
}

// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L304-L318 (chrome/m156)
fn add_new_purgeable_resource(
    resource_cache: &mut ResourceCache,
    gpu_memory_size: usize,
) -> ResourcePtr {
    let resource = add_new_resource(resource_cache, gpu_memory_size, Budgeted::Yes);

    let ptr = resource.as_arc().clone();
    drop(resource);
    resource_cache.force_process_returned_resources();
    ptr
}

def_graphite_test_for_all_contexts!(GraphitePurgeAsNeededResourcesTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L320-L445 (chrome/m156)
    let recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let mut resource_provider = resource_provider.lock().unwrap();
    let resource_cache = resource_provider.resource_cache();

    resource_cache.set_max_budget(10);

    let resource_size10 = add_new_resource(resource_cache, 10, Budgeted::Yes);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 10);

    let resource_size1 = add_new_resource(resource_cache, 1, Budgeted::Yes);

    // We should now be over budget, but nothing should be purged since neither resource is
    // purgeable.
    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 11);

    // Dropping the ref to the size 1 resource should cause it to get purged when we add a new
    // resource to the cache.
    drop(resource_size1);

    let resource_size2 = add_new_resource(resource_cache, 2, Budgeted::Yes);

    // The purging should have happened when we return the resource above so we also shouldn't
    // see anything in the purgeable queue.
    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 12);

    // Reset the cache back to no resources by setting budget to 0.
    drop(resource_size10);
    drop(resource_size2);
    resource_cache.force_process_returned_resources();
    resource_cache.set_max_budget(0);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 0);

    // Add a bunch of purgeable resources that keeps us under budget. Nothing should ever get
    // purged.
    resource_cache.set_max_budget(10);
    let resource_size1_ptr = add_new_purgeable_resource(resource_cache, 1);
    /*auto resourceSize2Ptr=*/
    add_new_purgeable_resource(resource_cache, 2);
    let resource_size3_ptr = add_new_purgeable_resource(resource_cache, 3);
    /*auto resourceSize4Ptr=*/
    add_new_purgeable_resource(resource_cache, 4);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 4);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_size1_ptr)
    );
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 10);

    // Now add some resources that should cause things to get purged.
    // Add a size 2 resource should purge the original size 1 and size 2
    add_new_purgeable_resource(resource_cache, 2);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 3);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_size3_ptr)
    );
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 9);

    // Adding a non-purgeable resource should also trigger resources to be purged from purgeable
    // queue.
    let resource_size10 = add_new_resource(resource_cache, 10, Budgeted::Yes);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 10);

    // Adding a resources that is purgeable back to the cache shouldn't trigger the previous
    // non-purgeable resource or itself to be purged yet (since processing our return mailbox
    // doesn't trigger the purgeAsNeeded call)
    let resource_size4_ptr = add_new_purgeable_resource(resource_cache, 4);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_size4_ptr)
    );
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 14);

    // Resetting the budget to 0 should trigger purging the size 4 purgeable resource but should
    // leave the non purgeable size 10 alone.
    resource_cache.set_max_budget(0);
    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 10);

    drop(resource_size10);
    resource_cache.force_process_returned_resources();
    resource_cache.force_purge_as_needed();

    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 0);
});

def_graphite_test_for_all_contexts!(GraphiteZeroSizedResourcesTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L447-L537 (chrome/m156)
    let recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let mut resource_provider = resource_provider.lock().unwrap();
    let resource_cache = resource_provider.resource_cache();

    // First make a normal resource that has a non zero size
    let mut resource_ptr = add_new_purgeable_resource(resource_cache, 1);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_ptr)
    );

    // First confirm if we set the max budget to zero, this sized resource is removed.
    resource_cache.set_max_budget(0);
    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 0);
    reporter_assert!(reporter, resource_cache.top_of_purgeable_queue().is_none());

    // Set the budget back to something higher
    resource_cache.set_max_budget(100);

    // Now create a zero sized resource and add it to the cache.
    resource_ptr = add_new_purgeable_resource(resource_cache, 0);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_ptr)
    );

    // Setting the budget down to 0 should not cause the zero sized resource to be purged
    resource_cache.set_max_budget(0);
    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1);
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &resource_ptr)
    );

    // Now add a sized resource to cache. Set budget higher again so that it fits
    resource_cache.set_max_budget(100);

    let sized_resource_ptr = add_new_purgeable_resource(resource_cache, 1);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 2);
    // Even though the zero sized resource was added to the cache first, the top of the purgeable
    // stack should be the sized resource.
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &sized_resource_ptr)
    );

    // Add another zero sized resource
    resource_ptr = add_new_purgeable_resource(resource_cache, 0);
    let _ = &resource_ptr;

    reporter_assert!(reporter, resource_cache.get_resource_count() == 3);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 3);
    // Again the sized resource should still be the top of the purgeable queue
    reporter_assert!(
        reporter,
        top_of_purgeable_queue_is(resource_cache, &sized_resource_ptr)
    );

    // If we set the cache budget to 0, it should clear out the sized resource but leave the two
    // zero-sized resources.
    resource_cache.set_max_budget(0);
    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 2);
    reporter_assert!(
        reporter,
        resource_cache
            .top_of_purgeable_queue()
            .is_some_and(|top| top.gpu_memory_size() == 0)
    );

    // However, purging all resources should clear the zero-sized resources.
    resource_cache.purge_resources();
    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 0);
});

// Depending on the granularity of the clock for a given device, in the
// GraphitePurgeNotUsedSinceResourcesTest we may end up with times that are all equal which messes
// up the expected behavior of the purge calls. So this helper forces us to return a new time that
// is different from a previous one.
// Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L543-L550 (chrome/m156)
fn force_newer_timepoint(prev_time: StdSteadyClockTimePoint) -> StdSteadyClockTimePoint {
    let mut time = StdSteadyClockTimePoint::now();
    while time <= prev_time {
        time = StdSteadyClockTimePoint::now();
    }
    time
}

def_graphite_test_for_all_contexts!(
    GraphitePurgeNotUsedSinceResourcesTest,
    |reporter, context| {
        // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L552-L638 (chrome/m156)
        let recorder = context.make_recorder(None);
        let resource_provider = recorder.priv_().resource_provider().clone();
        let mut resource_provider = resource_provider.lock().unwrap();
        let resource_cache = resource_provider.resource_cache();

        // Basic test where we purge 1 resource
        let before_time = StdSteadyClockTimePoint::now();

        add_new_purgeable_resource(resource_cache, 1);

        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);

        let mut after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

        let k_no_purging_time_limit = None;

        // purging beforeTime should not get rid of the resource
        resource_cache.purge_resources_not_used_since(before_time, k_no_purging_time_limit);

        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);

        // purging at afterTime which is after resource became purgeable should purge it.
        resource_cache.purge_resources_not_used_since(after_time, k_no_purging_time_limit);

        reporter_assert!(reporter, resource_cache.get_resource_count() == 0);

        // Test making 2 purgeable resources, but asking to purge on a time between the two.
        let resource_ptr1 = add_new_purgeable_resource(resource_cache, 1);

        let between_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

        let resource_ptr2 = add_new_purgeable_resource(resource_cache, 1);

        after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());

        reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource_ptr1.base())
        );
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource_ptr2.base())
        );

        resource_cache.purge_resources_not_used_since(between_time, k_no_purging_time_limit);

        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource_ptr2.base())
        );

        resource_cache.purge_resources_not_used_since(after_time, k_no_purging_time_limit);
        reporter_assert!(reporter, resource_cache.get_resource_count() == 0);

        // purgeResourcesNotUsedSince should have no impact on non-purgeable resources
        let resource = add_new_resource(resource_cache, 1, Budgeted::Yes);
        let resource_ptr = resource.as_arc().clone();

        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);

        after_time = force_newer_timepoint(StdSteadyClockTimePoint::now());
        resource_cache.purge_resources_not_used_since(after_time, k_no_purging_time_limit);
        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
        reporter_assert!(
            reporter,
            !resource_cache.testing_in_purgeable_queue(resource_ptr.base())
        );

        drop(resource);
        // purgeResourcesNotUsedSince should check the mailbox for the returned resource. Though the
        // time is set before that happens so nothing should purge.
        resource_cache.purge_resources_not_used_since(
            StdSteadyClockTimePoint::now(),
            k_no_purging_time_limit,
        );
        reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource_ptr.base())
        );

        // Now it should be purged since it is already purgeable
        resource_cache.purge_resources_not_used_since(
            force_newer_timepoint(StdSteadyClockTimePoint::now()),
            k_no_purging_time_limit,
        );
        reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    }
);

// This test is used to check the case where we call purgeNotUsedSince, which triggers us to return
// resources from mailbox. Even though the returned resources aren't purged by the last used, we
// still end up purging things to get under budget.
def_graphite_test_for_all_contexts!(GraphitePurgeNotUsedOverBudgetTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L643-L730 (chrome/m156)
    let recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let mut resource_provider = resource_provider.lock().unwrap();
    let resource_cache = resource_provider.resource_cache();

    // set resourceCache budget to 10 for testing.
    const K_BUDGET: usize = 10;
    resource_cache.set_max_budget(K_BUDGET);

    // First make a purgeable resources
    let resource_ptr = add_new_purgeable_resource(resource_cache, /* gpuMemorySize= */ 1);

    // Now create a bunch of non purgeable (yet) resources that are not budgeted (i.e. in real
    // world they would be wrapped in an SkSurface or SkImage), but will cause us to go over our
    // budget limit when they do return to cache. These are sized so that once they become
    // budgeted, only one will remain when purging to become under budget.

    let resource1 = add_new_resource(
        resource_cache,
        /* gpuMemorySize= */ K_BUDGET - 1,
        Budgeted::No,
    );

    let resource2 = add_new_resource(
        resource_cache,
        /* gpuMemorySize= */ K_BUDGET - 2,
        Budgeted::No,
    );

    let resource3 = add_new_resource(
        resource_cache,
        /* gpuMemorySize= */ K_BUDGET - 3,
        Budgeted::No,
    );

    let resource1_ptr = resource1.as_arc().clone();
    let resource2_ptr = resource2.as_arc().clone();
    let resource3_ptr = resource3.as_arc().clone();

    reporter_assert!(reporter, resource_cache.get_resource_count() == 4);
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 1);

    let time_before_returning_to_cache = StdSteadyClockTimePoint::now();

    // Now reset all the non budgeted resources so they return to the cache and become budgeted.
    // Returning to the cache will not immedidately trigger a purgeAsNeeded call.
    drop(resource1);
    drop(resource2);
    drop(resource3);

    // All three resources are being processed together, and within one processing, there's no
    // assumed requirement that resources get put into the purgeable queue in the same order they
    // were in the return queue.
    resource_cache.force_process_returned_resources();

    reporter_assert!(reporter, resource_cache.get_resource_count() == 4);
    reporter_assert!(reporter, resource_cache.current_budgeted_bytes() == 25);
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource_ptr.base())
    );
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource1_ptr.base())
    );
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource2_ptr.base())
    );
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource3_ptr.base())
    );

    // Now we call purgeNotUsedSince with timeBeforeReturnToCache. The original resource should get
    // purged because it is older than this time. The three originally non budgeted resources are
    // newer than this time so they won't be purged by the time on this call. However, since we are
    // overbudget it should trigger us to purge two of them. Since each independently fits within
    // the budget, one (unspecified) will remain the purgeable queue.
    resource_cache.purge_resources_not_used_since(
        time_before_returning_to_cache,
        /* quitPurgingTime= */ None,
    );
    reporter_assert!(
        reporter,
        resource_cache.get_resource_count() == 1,
        "count = {}",
        resource_cache.get_resource_count()
    );
    if resource_cache.current_budgeted_bytes() == K_BUDGET - 1 {
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource1_ptr.base())
        );
    } else if resource_cache.current_budgeted_bytes() == K_BUDGET - 2 {
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource2_ptr.base())
        );
    } else {
        reporter_assert!(
            reporter,
            resource_cache.current_budgeted_bytes() == K_BUDGET - 3
        );
        reporter_assert!(
            reporter,
            resource_cache.testing_in_purgeable_queue(resource3_ptr.base())
        );
    }
});

// Test call purgeResources on the ResourceCache and make sure all unlocked resources are getting
// purged regardless of when they were last used.
def_graphite_test_for_all_contexts!(GraphitePurgeResourcesTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L734-L799 (chrome/m156)
    let recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let mut resource_provider = resource_provider.lock().unwrap();
    let resource_cache = resource_provider.resource_cache();

    // set resourceCache budget to 10 for testing.
    resource_cache.set_max_budget(10);

    // Basic test where we purge 1 resource
    add_new_purgeable_resource(resource_cache, 1);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);

    // purging should purge the one unlocked resource.
    resource_cache.purge_resources();
    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);

    // Test making 2 purgeable resources
    let resource_ptr1 = add_new_purgeable_resource(resource_cache, 1);

    let resource_ptr2 = add_new_purgeable_resource(resource_cache, 1);

    reporter_assert!(reporter, resource_cache.get_resource_count() == 2);
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource_ptr1.base())
    );
    reporter_assert!(
        reporter,
        resource_cache.testing_in_purgeable_queue(resource_ptr2.base())
    );

    resource_cache.purge_resources();
    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);

    // purgeResources should have no impact on non-purgeable resources
    let resource = add_new_resource(resource_cache, /* gpuMemorySize= */ 1, Budgeted::Yes);
    let resource_ptr = resource.as_arc().clone();

    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);

    resource_cache.purge_resources();
    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    reporter_assert!(
        reporter,
        !resource_cache.testing_in_purgeable_queue(resource_ptr.base())
    );

    drop(resource);
    resource_cache.purge_resources();
    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
});

def_graphite_test_for_all_contexts!(GraphiteScratchResourcesTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L801-L897 (chrome/m156)
    let recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let mut resource_provider = resource_provider.lock().unwrap();
    let resource_cache = resource_provider.resource_cache();

    reporter_assert!(reporter, resource_cache.get_resource_count() == 0);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 0);

    // Test making a non budgeted, non shareable resource.
    let mut resource = TestResource::make(
        resource_cache,
        Ownership::Owned,
        Budgeted::No,
        Shareable::No,
        1,
    );
    let resource_ptr = resource.as_arc().clone();

    reporter_assert!(reporter, resource.base().budgeted() == Budgeted::No);
    reporter_assert!(reporter, resource_cache.get_resource_count() == 1);
    // Resource is not shareable and we have a ref on it. Thus it shouldn't be findable in the
    // cache
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 0);

    // Requesting a scratch shareable resouce will not return the non-shareable resource.
    let key = TestResource::create_key();

    let mut unavailable = ScratchResourceSet::new();

    reporter_assert!(reporter, &key == resource.base().key());
    let mut resource_ptr2 = resource_cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        /* label= */ "",
        Some(&unavailable),
    );
    reporter_assert!(reporter, resource_ptr2.is_none());

    // Return the non-shareable resource and verify that it can now be requested as scratch
    drop(resource);
    resource_cache.force_process_returned_resources();
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1);

    resource = resource_cache
        .find_and_ref_resource(
            &key,
            Budgeted::Yes,
            Shareable::Scratch,
            /* label= */ "",
            Some(&unavailable),
        )
        .and_then(|found| found.downcast::<TestResource>().ok())
        .expect("the scratch resource is found");
    reporter_assert!(
        reporter,
        resource.base().unique_id() == resource_ptr.base().unique_id()
    );
    reporter_assert!(reporter, resource.base().budgeted() == Budgeted::Yes);
    reporter_assert!(reporter, resource.base().shareable() == Shareable::Scratch);
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1); // still findable

    // A request of the same key as non-shareable will not return the scratch resource
    resource_ptr2 =
        resource_cache.find_and_ref_resource(&key, Budgeted::Yes, Shareable::No, "", None);
    reporter_assert!(reporter, resource_ptr2.is_none());

    // Similarly, a request for a fully shareable resource cannot be satisfied by a scratch
    // resource
    resource_ptr2 =
        resource_cache.find_and_ref_resource(&key, Budgeted::Yes, Shareable::Yes, "", None);
    reporter_assert!(reporter, resource_ptr2.is_none());

    // A request for another scratch resource can return the existing one if it hasn't been marked
    // unavailable in the set passed to the cache.
    resource_ptr2 = resource_cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        /* label= */ "",
        Some(&unavailable),
    );
    reporter_assert!(
        reporter,
        resource_ptr2
            .as_ref()
            .is_some_and(|found| found.base().unique_id() == resource_ptr.base().unique_id())
    );
    drop(resource_ptr2.take()); // resourcePtr2->unref()

    // Mark the original resource as unvailable and now it shouldn't be seen by the request.
    unavailable.insert(resource_ptr.base().unique_id());
    resource_ptr2 = resource_cache.find_and_ref_resource(
        &key,
        Budgeted::Yes,
        Shareable::Scratch,
        /* label= */ "",
        Some(&unavailable),
    );
    reporter_assert!(reporter, resource_ptr2.is_none());

    // Return the scratch resource, and then simulate a threading race where there's a request for
    // the scratch resource that comes in before the return queue is processed (adding a usage
    // ref), and then the queue is processed as part of a non-shareable request (which should then
    // fail).
    unavailable.clear();
    drop(resource);
    resource = resource_cache
        .find_and_ref_resource(
            &key,
            Budgeted::Yes,
            Shareable::Scratch,
            /* label= */ "",
            Some(&unavailable),
        )
        .and_then(|found| found.downcast::<TestResource>().ok())
        .expect("the scratch resource is found");
    reporter_assert!(
        reporter,
        resource.base().unique_id() == resource_ptr.base().unique_id()
    );
    // At this point, resourcePtr has a usage ref and should be in the return queue
    reporter_assert!(
        reporter,
        resource_cache.testing_in_return_queue(resource_ptr.base())
    );
    resource_cache.force_process_returned_resources();
    // Its shareable type should not have changed after being processed.
    reporter_assert!(
        reporter,
        !resource_cache.testing_in_return_queue(resource_ptr.base())
    );
    reporter_assert!(reporter, resource.base().shareable() == Shareable::Scratch);

    // Now actually return the resource and confirm that it can be used for non-shareable requests
    // once all usage refs are dropped.
    drop(resource);
    resource_cache.force_process_returned_resources();
    reporter_assert!(reporter, resource_cache.num_findable_resources() == 1);
    reporter_assert!(reporter, resource_ptr.base().shareable() == Shareable::No);

    // Returning the scratch resource allows it to be changed to a different shareable type
    resource_ptr2 =
        resource_cache.find_and_ref_resource(&key, Budgeted::Yes, Shareable::Yes, "", None);
    reporter_assert!(
        reporter,
        resource_ptr2
            .as_ref()
            .is_some_and(|found| found.base().unique_id() == resource_ptr.base().unique_id())
    );
    reporter_assert!(
        reporter,
        resource_ptr2
            .as_ref()
            .is_some_and(|found| found.base().shareable() == Shareable::Yes)
    );
    drop(resource_ptr2.take()); // resourcePtr2->unref()
});

def_graphite_test_for_all_contexts!(GraphiteTimeLimitedPurgeTest, |reporter, context| {
    // Port of: tests/graphite/GraphiteResourceCacheTest.cpp#L899-L988 (chrome/m156)
    let mut recorder = context.make_recorder(None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    // The recorder locks its resource provider itself (`performDeferredCleanup`), so the cache
    // is locked only for the duration of each use.
    let count = || {
        resource_provider
            .lock()
            .unwrap()
            .resource_cache()
            .get_resource_count()
    };
    let process_returned_resources = || {
        resource_provider
            .lock()
            .unwrap()
            .resource_cache()
            .force_process_returned_resources();
    };
    let add_purgeable_resource = |gpu_memory_size: usize| {
        let mut resource_provider = resource_provider.lock().unwrap();
        add_new_purgeable_resource(resource_provider.resource_cache(), gpu_memory_size)
    };

    reporter_assert!(reporter, count() == 0);
    reporter_assert!(
        reporter,
        resource_provider
            .lock()
            .unwrap()
            .resource_cache()
            .num_findable_resources()
            == 0
    );

    // Add a singular purgeable resource to the cache.
    add_purgeable_resource(/* gpuMemorySize= */ 1);
    reporter_assert!(reporter, count() == 1);

    // Trigger the purging of the resource with an impossibly small duration limit, confirming that
    // the resource remains in the cache.
    const K_ZERO_MS: Duration = Duration::from_millis(0);
    let k_no_purging_time_limit = None;
    // Before performing cleanup, force process the cache's returned resources to ensure that
    // resources' last access times are updated.
    process_returned_resources();
    // Make sure we actually get a new time point such that resources unused in the last 0 ms
    // actually leads to resource purging.
    let _ = force_newer_timepoint(StdSteadyClockTimePoint::now());
    recorder.perform_deferred_cleanup(K_ZERO_MS, Some(K_ZERO_MS));
    reporter_assert!(reporter, count() == 1);

    // Now purge with no time limit given to actually empty out the cache.
    process_returned_resources(); // Forcibly update resources' last used times
    let _ = force_newer_timepoint(StdSteadyClockTimePoint::now()); // Ensures last used times < now
    recorder.perform_deferred_cleanup(K_ZERO_MS, k_no_purging_time_limit);
    reporter_assert!(reporter, count() == 0);

    // Now fill up the cache with kLargeResourceCount purgeable resources.
    const K_LARGE_RESOURCE_COUNT: usize = 1000;
    for _ in 0..K_LARGE_RESOURCE_COUNT {
        add_purgeable_resource(/* gpuMemorySize= */ 1);
    }
    // Update resources' last used times and empty out the return queue before checking cache size
    process_returned_resources();
    reporter_assert!(reporter, count() == K_LARGE_RESOURCE_COUNT);

    // Record how long it takes to completely purge all kLargeResourceCount resources.
    let time_before_full_purge = force_newer_timepoint(StdSteadyClockTimePoint::now());
    recorder.perform_deferred_cleanup(K_ZERO_MS, k_no_purging_time_limit);
    let time_after_full_purge = force_newer_timepoint(StdSteadyClockTimePoint::now());
    reporter_assert!(reporter, count() == 0);
    let actual_full_purge_duration = time_after_full_purge - time_before_full_purge;
    reporter_assert!(reporter, actual_full_purge_duration.as_nanos() > 0);

    // Re-populate the cache.
    for _ in 0..K_LARGE_RESOURCE_COUNT {
        add_purgeable_resource(/* gpuMemorySize= */ 1);
    }
    // Update resources' last used times and empty out the return queue before checking cache size
    process_returned_resources();
    reporter_assert!(reporter, count() == K_LARGE_RESOURCE_COUNT);

    // Finally, try purging with a time duration significantly smaller than the actual time
    // recorded in the previous step. This should force only a subset of the resources to be
    // purged.
    let time_before_partial_purge = force_newer_timepoint(StdSteadyClockTimePoint::now());
    let small_duration = Duration::from_micros(
        u64::try_from((actual_full_purge_duration / 5).as_micros()).unwrap_or(u64::MAX),
    );
    recorder.perform_deferred_cleanup(K_ZERO_MS, Some(small_duration));
    let actual_partial_purge_duration =
        force_newer_timepoint(StdSteadyClockTimePoint::now()) - time_before_partial_purge;

    // The actual duration should be >0. We expect resources to be purged until we have
    // *exceeded* the duration given. However, we should be able to see that not *all* purgeable
    // resources were purged (i.e. the stop time triggered an early exit of purging).
    reporter_assert!(
        reporter,
        actual_partial_purge_duration.as_nanos() > 0
            && actual_partial_purge_duration > small_duration
    );

    reporter_assert!(reporter, count() > 0);
    reporter_assert!(reporter, count() < K_LARGE_RESOURCE_COUNT);
});
