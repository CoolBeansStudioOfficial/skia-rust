// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ProxyCacheTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_gpu::gpu::gpu_types::StdSteadyClockTimePoint;
use skia_rust_gpu::gpu::resource_key::{UniqueKey, UniqueKeyBuilder};
use skia_rust_gpu::graphite::context_priv::SharedResourceProvider;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::proxy_cache::{ProxyCache, find_or_create_cached_proxy_from_bitmap};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::resource_provider::ResourceProvider;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::tools::graphite_test_context::synced_submit;
use crate::tools::tool_utils::get_resource_as_bitmap;
use crate::{def_graphite_test_for_all_contexts, reporter_assert};

/// `static const UniqueKey::Domain kTestDomain`: generated once, shared by every call.
static TEST_DOMAIN: std::sync::LazyLock<u16> = std::sync::LazyLock::new(UniqueKey::generate_domain);

/// `ProxyCache::numCached()` of the recorder's cache.
fn num_cached(provider: &SharedResourceProvider) -> usize {
    let mut provider = provider.lock().unwrap_or_else(PoisonError::into_inner);
    provider.proxy_cache().map_or(0, |cache| cache.num_cached())
}

/// Runs `f` on the recorder's proxy cache.
fn with_proxy_cache<R>(
    provider: &SharedResourceProvider,
    f: impl FnOnce(&mut ProxyCache) -> R,
) -> R {
    let mut provider = provider.lock().unwrap_or_else(PoisonError::into_inner);
    f(provider.proxy_cache().expect("a proxy cache"))
}

/// Runs `f` on the recorder's resource cache.
fn with_resource_provider<R>(
    provider: &SharedResourceProvider,
    f: impl FnOnce(&mut ResourceProvider) -> R,
) -> R {
    let mut provider = provider.lock().unwrap_or_else(PoisonError::into_inner);
    f(&mut provider)
}

/// `proxyCache->findOrCreateCachedProxy(recorder, bitmap, label)`: the proxy of `bitmap`'s pixels,
/// cached under the bitmap's key.
fn find_or_create_cached_proxy(
    recorder: &Recorder,
    bitmap: &Bitmap,
    label: &str,
) -> Option<Arc<TextureProxy>> {
    let key = ProxyCache::bitmap_key(bitmap);
    find_or_create_cached_proxy_from_bitmap(recorder, &key, || bitmap.clone(), label)
}

/// Submits what `recorder` has recorded (`recorder->snap()`, `insertRecording`), then `submit`.
fn snap_and_submit(context: &mut WgpuContext, recorder: &mut Recorder, sync: SyncToCpu) {
    if let Some(mut recording) = recorder.snap() {
        let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    }
    let _ = context.submit(SubmitInfo::new(sync));
}

// This test exercises the basic MessageBus behavior of the ProxyCache by manually inserting an
// SkBitmap into the proxy cache and then changing its contents. This simple test should create
// an IDChangeListener that will remove the entry in the cache when the bitmap is changed and
// the resulting message processed.
// Port of: tests/graphite/ProxyCacheTest.cpp#L32-L55 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest1, |reporter, context| {
    let recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let bitmap = get_resource_as_bitmap("images/mandrill_128.png");
    reporter_assert!(reporter, bitmap.is_some());
    let Some(mut bitmap) = bitmap else {
        return;
    };

    reporter_assert!(reporter, num_cached(&provider) == 0);
    let proxy = find_or_create_cached_proxy(&recorder, &bitmap, "ProxyCacheTestTexture");
    reporter_assert!(reporter, num_cached(&provider) == 1);
    drop(proxy);

    bitmap.erase_color(Color::BLACK);
    with_proxy_cache(&provider, ProxyCache::force_process_invalid_key_msgs);
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// This test checks that, if the same bitmap is added to two separate ProxyCaches, when it is
// changed, both of the ProxyCaches will receive the message.
// Port of: tests/graphite/ProxyCacheTest.cpp#L59-L90 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest2, |reporter, context| {
    let recorder1 = context.make_recorder(None);
    let provider1 = Arc::clone(recorder1.priv_().resource_provider());
    let recorder2 = context.make_recorder(None);
    let provider2 = Arc::clone(recorder2.priv_().resource_provider());

    let bitmap = get_resource_as_bitmap("images/mandrill_128.png");
    reporter_assert!(reporter, bitmap.is_some());
    let Some(mut bitmap) = bitmap else {
        return;
    };

    reporter_assert!(reporter, num_cached(&provider1) == 0);
    reporter_assert!(reporter, num_cached(&provider2) == 0);
    let proxy1 = find_or_create_cached_proxy(&recorder1, &bitmap, "ProxyCacheTestTexture");
    let proxy2 = find_or_create_cached_proxy(&recorder2, &bitmap, "ProxyCacheTestTexture");
    reporter_assert!(reporter, num_cached(&provider1) == 1);
    reporter_assert!(reporter, num_cached(&provider2) == 1);
    drop((proxy1, proxy2));

    bitmap.erase_color(Color::BLACK);
    with_proxy_cache(&provider1, ProxyCache::force_process_invalid_key_msgs);
    with_proxy_cache(&provider2, ProxyCache::force_process_invalid_key_msgs);
    reporter_assert!(reporter, num_cached(&provider1) == 0);
    reporter_assert!(reporter, num_cached(&provider2) == 0);
});

/// `ProxyCacheSetup`: two bitmaps, their proxies, and the times the proxies were made.
#[derive(Default)]
struct ProxyCacheSetup {
    bitmap1: Bitmap,
    proxy1: Option<Arc<TextureProxy>>,
    bitmap2: Bitmap,
    proxy2: Option<Arc<TextureProxy>>,
    time_between_proxy_creation: Option<StdSteadyClockTimePoint>,
    time_after_all_proxy_creation: Option<StdSteadyClockTimePoint>,
}

impl ProxyCacheSetup {
    // Port of: tests/graphite/ProxyCacheTest.cpp#L110-L112 (chrome/m156), `valid`
    fn valid(&self) -> bool {
        !self.bitmap1.is_empty()
            && !self.bitmap2.is_empty()
            && self.proxy1.is_some()
            && self.proxy2.is_some()
    }
}

/// `setup_test`: caches two proxies, timestamping each by submitting its texture's upload.
// Port of: tests/graphite/ProxyCacheTest.cpp#L94-L134 (chrome/m156)
fn setup_test(
    reporter: &mut crate::Reporter,
    context: &mut WgpuContext,
    recorder: &mut Recorder,
) -> ProxyCacheSetup {
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let mut setup = ProxyCacheSetup::default();
    let bitmap1 = get_resource_as_bitmap("images/mandrill_32.png");
    let bitmap2 = get_resource_as_bitmap("images/mandrill_64.png");
    let (Some(bitmap1), Some(bitmap2)) = (bitmap1, bitmap2) else {
        return setup;
    };
    setup.bitmap1 = bitmap1;
    setup.bitmap2 = bitmap2;

    reporter_assert!(reporter, num_cached(&provider) == 0);
    setup.proxy1 = find_or_create_cached_proxy(recorder, &setup.bitmap1, "ProxyCacheTestTexture");
    reporter_assert!(reporter, num_cached(&provider) == 1);
    // Ensure proxy1's Texture is created (and timestamped) at this time
    snap_and_submit(context, recorder, SyncToCpu::Yes);

    std::thread::sleep(Duration::from_millis(2));
    setup.time_between_proxy_creation = Some(StdSteadyClockTimePoint::now());
    std::thread::sleep(Duration::from_millis(2));

    setup.proxy2 = find_or_create_cached_proxy(recorder, &setup.bitmap2, "ProxyCacheTestTexture");
    reporter_assert!(reporter, num_cached(&provider) == 2);
    // Ensure proxy2's Texture is created (and timestamped) at this time
    if let Some(mut recording) = recorder.snap() {
        let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    }
    synced_submit(context);

    std::thread::sleep(Duration::from_millis(2));
    setup.time_after_all_proxy_creation = Some(StdSteadyClockTimePoint::now());
    std::thread::sleep(Duration::from_millis(2));
    setup
}

// This test exercises the ProxyCache's freeUniquelyHeld method.
// Port of: tests/graphite/ProxyCacheTest.cpp#L160-L185 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest4, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let mut setup = setup_test(reporter, context, &mut recorder);
    reporter_assert!(reporter, setup.valid());
    if !setup.valid() {
        return;
    }

    with_proxy_cache(&provider, ProxyCache::force_free_uniquely_held);
    reporter_assert!(reporter, num_cached(&provider) == 2);
    setup.proxy1 = None;
    with_proxy_cache(&provider, ProxyCache::force_free_uniquely_held);
    reporter_assert!(reporter, num_cached(&provider) == 1);
    setup.proxy2 = None;
    with_proxy_cache(&provider, ProxyCache::force_free_uniquely_held);
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// This test exercises the ProxyCache's purgeProxiesNotUsedSince method.
// Port of: tests/graphite/ProxyCacheTest.cpp#L188-L211 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest5, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let setup = setup_test(reporter, context, &mut recorder);
    reporter_assert!(reporter, setup.valid());
    if !setup.valid() {
        return;
    }
    let (Some(between), Some(after_all)) = (
        setup.time_between_proxy_creation,
        setup.time_after_all_proxy_creation,
    ) else {
        return;
    };

    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(between);
    });
    reporter_assert!(reporter, num_cached(&provider) == 1);
    let test = with_proxy_cache(&provider, |cache| cache.find(&setup.bitmap1));
    // proxy1 should've been purged
    reporter_assert!(reporter, test.is_none());

    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(after_all);
    });
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// This test simply verifies that the ProxyCache is correctly updating the Resource's
// last access time stamp.
// Port of: tests/graphite/ProxyCacheTest.cpp#L215-L249 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest6, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let setup = setup_test(reporter, context, &mut recorder);
    reporter_assert!(reporter, setup.valid());
    if !setup.valid() {
        return;
    }
    let (Some(between), Some(after_all)) = (
        setup.time_between_proxy_creation,
        setup.time_after_all_proxy_creation,
    ) else {
        return;
    };

    // update proxy1's timestamp
    let test = find_or_create_cached_proxy(&recorder, &setup.bitmap1, "ProxyCacheTestTexture");
    reporter_assert!(
        reporter,
        matches!((&test, &setup.proxy1), (Some(a), Some(b)) if Arc::ptr_eq(a, b))
    );
    std::thread::sleep(Duration::from_millis(2));
    let time_after_proxy1_update = StdSteadyClockTimePoint::now();
    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(between);
    });
    reporter_assert!(reporter, num_cached(&provider) == 2);
    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(after_all);
    });
    reporter_assert!(reporter, num_cached(&provider) == 1);
    let test = with_proxy_cache(&provider, |cache| cache.find(&setup.bitmap2));
    // proxy2 should've been purged
    reporter_assert!(reporter, test.is_none());
    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(time_after_proxy1_update);
    });
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// Verify that the ProxyCache's purgeProxiesNotUsedSince method can clear out multiple proxies.
// Port of: tests/graphite/ProxyCacheTest.cpp#L252-L269 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest7, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    let setup = setup_test(reporter, context, &mut recorder);
    reporter_assert!(reporter, setup.valid());
    if !setup.valid() {
        return;
    }
    let Some(after_all) = setup.time_after_all_proxy_creation else {
        return;
    };

    with_proxy_cache(&provider, |cache| {
        cache.force_purge_proxies_not_used_since(after_all);
    });
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// Verify that the ProxyCache's freeUniquelyHeld behavior is working in the ResourceCache.
// Port of: tests/graphite/ProxyCacheTest.cpp#L272-L319 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest8, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    with_resource_provider(&provider, |p| p.resource_cache().set_max_budget(0));
    let mut setup = setup_test(reporter, context, &mut recorder);
    reporter_assert!(reporter, setup.valid());
    if !setup.valid() {
        return;
    }

    with_resource_provider(&provider, |p| p.resource_cache().force_purge_as_needed());
    reporter_assert!(reporter, num_cached(&provider) == 2);
    setup.proxy1 = None;
    with_proxy_cache(&provider, ProxyCache::force_process_invalid_key_msgs);
    // unreffing proxy1 and forcing message processing shouldn't purge proxy1 from the cache
    let test = with_proxy_cache(&provider, |cache| cache.find(&setup.bitmap1));
    reporter_assert!(reporter, test.is_some());
    drop(test);
    with_resource_provider(&provider, |p| p.resource_cache().force_purge_as_needed());
    reporter_assert!(reporter, num_cached(&provider) == 1);
    let test = with_proxy_cache(&provider, |cache| cache.find(&setup.bitmap1));
    // proxy1 should've been purged
    reporter_assert!(reporter, test.is_none());

    setup.proxy2 = None;
    with_proxy_cache(&provider, ProxyCache::force_process_invalid_key_msgs);
    // unreffing proxy2 and forcing message processing shouldn't purge proxy2 from the cache
    let test = with_proxy_cache(&provider, |cache| cache.find(&setup.bitmap2));
    reporter_assert!(reporter, test.is_some());
    drop(test);
    with_resource_provider(&provider, |p| p.resource_cache().force_purge_as_needed());
    reporter_assert!(reporter, num_cached(&provider) == 0);
});

// Verify that the ProxyCache's explicit keying only generates the bitmaps as needed.
// Port of: tests/graphite/ProxyCacheTest.cpp#L428-L472 (chrome/m156)
def_graphite_test_for_all_contexts!(ProxyCacheTest10, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let provider = Arc::clone(recorder.priv_().resource_provider());
    reporter_assert!(reporter, num_cached(&provider) == 0);

    let mut regenerated = false;
    let proxy1 = find_or_create_by_key(&recorder, 1, &mut regenerated);
    reporter_assert!(
        reporter,
        proxy1.as_ref().is_some_and(|p| p.dimensions().width == 32)
    );
    reporter_assert!(reporter, regenerated);
    let proxy2 = find_or_create_by_key(&recorder, 2, &mut regenerated);
    reporter_assert!(
        reporter,
        proxy2.as_ref().is_some_and(|p| p.dimensions().width == 64)
    );
    reporter_assert!(reporter, regenerated);
    reporter_assert!(reporter, num_cached(&provider) == 2);

    // These cached proxies shouldn't be deleted because we hold local refs still
    with_proxy_cache(&provider, ProxyCache::force_free_uniquely_held);
    reporter_assert!(reporter, num_cached(&provider) == 2);

    // Cache hit should not invoke the bitmap generation function.
    let proxy1b = find_or_create_by_key(&recorder, 1, &mut regenerated);
    reporter_assert!(
        reporter,
        matches!((&proxy1, &proxy1b), (Some(a), Some(b)) if Arc::ptr_eq(a, b))
    );
    reporter_assert!(reporter, !regenerated);

    drop(proxy1);
    drop(proxy1b);
    drop(proxy2);
    // Dump pending commands to release internal refs to the cached proxies
    let _ = recorder.snap();
    // Now the cache should clean the cache entries up
    with_proxy_cache(&provider, ProxyCache::force_free_uniquely_held);
    reporter_assert!(reporter, num_cached(&provider) == 0);

    // And regeneration functions as expected
    let proxy1 = find_or_create_by_key(&recorder, 1, &mut regenerated);
    reporter_assert!(
        reporter,
        proxy1.as_ref().is_some_and(|p| p.dimensions().width == 32)
    );
    reporter_assert!(reporter, regenerated);
});

/// `find_or_create_by_key`: the proxy cached for the explicit key `id`, generated from the
/// 32-pixel (`id` 1) or 64-pixel (`id` 2) mandrill when it is not cached. `regenerated` is set
/// when the generator ran.
// Port of: tests/graphite/ProxyCacheTest.cpp#L392-L419 (chrome/m156)
fn find_or_create_by_key(
    recorder: &Recorder,
    id: u32,
    regenerated: &mut bool,
) -> Option<Arc<TextureProxy>> {
    *regenerated = false;
    let mut key = UniqueKey::new();
    {
        let mut builder = UniqueKeyBuilder::new(&mut key, *TEST_DOMAIN, 1, Some("TestExplicitKey"));
        builder[0] = id;
    }
    let regenerated_flag = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&regenerated_flag);
    let proxy = find_or_create_cached_proxy_from_bitmap(
        recorder,
        &key,
        move || {
            *flag.lock().unwrap_or_else(PoisonError::into_inner) = true;
            let path = if id == 1 {
                "images/mandrill_32.png"
            } else {
                "images/mandrill_64.png"
            };
            get_resource_as_bitmap(path).unwrap_or_default()
        },
        "",
    );
    *regenerated = *regenerated_flag
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    proxy
}
