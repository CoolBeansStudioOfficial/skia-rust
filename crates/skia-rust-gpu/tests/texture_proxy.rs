// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: tests/graphite/TextureProxyTest.cpp, tests/graphite/ProxyCacheTest.cpp

//! `TextureProxy`, `ResourceProvider`, `ScratchResourceManager` and `ProxyCache` against a mock
//! backend. Skia's own tests need a GPU context (caps, backend textures, a recorder), so they are
//! not the 1:1 manifest ports; the scenarios and assertions below follow them.

mod support;

use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{BackendApi, Budgeted, Mipmapped, Protected};
use skia_rust_gpu::graphite::buffer::Buffer;
use skia_rust_gpu::graphite::graphite_resource_key::{
    GraphiteResourceKey, GraphiteResourceKeyBuilder,
};
use skia_rust_gpu::graphite::graphite_types::{SampleCount, Volatile};
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::resource_provider::{ResourceProvider, ResourceProviderBackend};
use skia_rust_gpu::graphite::resource_types::{AccessPattern, BufferType, Ownership, ResourceType};
use skia_rust_gpu::graphite::scratch_resource_manager::{
    ProxyReadCountMap, ScratchResourceManager,
};
use skia_rust_gpu::graphite::texture::{Texture, TextureBackend};
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_info::{TextureInfo, TextureInfoData, texture_info_priv};
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use support::MockCaps;

#[derive(Debug, Clone, PartialEq)]
struct MockTextureInfo {
    format: TextureFormat,
}

impl TextureInfoData for MockTextureInfo {
    fn backend(&self) -> BackendApi {
        BackendApi::Mock
    }

    fn is_protected(&self) -> Protected {
        Protected::No
    }

    fn view_format(&self) -> TextureFormat {
        self.format
    }

    fn to_backend_string(&self) -> String {
        String::new()
    }

    fn is_compatible(&self, that: &TextureInfo, _require_exact: bool) -> bool {
        that.get::<Self>().is_some_and(|that| that == self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct MockTexture;

impl TextureBackend for MockTexture {
    fn free_gpu_data(&self) {}

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct MockBackend {
    created: Arc<AtomicUsize>,
}

impl ResourceProviderBackend for MockBackend {
    fn max_texture_size(&self) -> i32 {
        4096
    }

    fn build_key_for_texture(
        &self,
        dimensions: ISize,
        info: &TextureInfo,
        ty: ResourceType,
        key: &mut GraphiteResourceKey,
    ) {
        let mut builder = GraphiteResourceKeyBuilder::new(key, ty, 3);
        builder[0] = dimensions.width.cast_unsigned();
        builder[1] = dimensions.height.cast_unsigned();
        builder[2] = texture_info_priv::view_format(info) as u32;
    }

    fn create_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        self.created.fetch_add(1, Ordering::Relaxed);
        Some(Texture::make(
            dimensions,
            info,
            false,
            Ownership::Owned,
            label,
            Box::new(MockTexture),
        ))
    }

    fn create_buffer(
        &mut self,
        _size: usize,
        _ty: BufferType,
        _access_pattern: AccessPattern,
        _label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        None
    }
}

fn rgba_info() -> TextureInfo {
    TextureInfo::make(
        MockTextureInfo {
            format: TextureFormat::RGBA8,
        },
        SampleCount::One,
        Mipmapped::No,
    )
}

fn provider() -> (ResourceProvider, Arc<AtomicUsize>) {
    let created = Arc::new(AtomicUsize::new(0));
    let backend = MockBackend {
        created: created.clone(),
    };
    (
        ResourceProvider::new(Box::new(backend), 1, 1 << 20),
        created,
    )
}

fn wrapped_texture(dimensions: ISize) -> ResourceRef<Texture> {
    Texture::make(
        dimensions,
        &rgba_info(),
        false,
        Ownership::Wrapped,
        "TextureProxyTestWrappedTex",
        Box::new(MockTexture),
    )
}


type Slot = Arc<Mutex<Option<ResourceRef<Texture>>>>;

fn same_texture(a: Option<&ResourceRef<Texture>>, b: &ResourceRef<Texture>) -> bool {
    a.is_some_and(|a| ResourceRef::ptr_eq(a, b))
}

// Port of: tests/graphite/TextureProxyTest.cpp#L27-L200 (chrome/m156)
#[test]
#[allow(clippy::too_many_lines)] // one test, as in Skia
fn graphite_texture_proxy_test() {
    let valid_size = ISize::new(1, 1);
    let invalid_size = ISize::new_empty();
    let (mut resource_provider, _) = provider();
    let texture_info = rgba_info();
    let texture = wrapped_texture(valid_size);

    let null_callback = || -> skia_rust_gpu::graphite::texture_proxy::LazyInstantiateCallback {
        Box::new(|_: &mut ResourceProvider| None)
    };
    let callback = |texture: &ResourceRef<Texture>| {
        let texture = texture.clone();
        let cb: skia_rust_gpu::graphite::texture_proxy::LazyInstantiateCallback =
            Box::new(move |_: &mut ResourceProvider| Some(texture.clone()));
        cb
    };
    let assignable_texture: Slot = Arc::new(Mutex::new(None));
    let assignable_callback = {
        let slot = assignable_texture.clone();
        move || -> skia_rust_gpu::graphite::texture_proxy::LazyInstantiateCallback {
            let slot = slot.clone();
            Box::new(move |_: &mut ResourceProvider| slot.lock().unwrap().clone())
        }
    };

    // Invalid parameters.
    let texture_proxy = TextureProxy::make(
        &MockCaps::default(),
        &mut resource_provider,
        invalid_size,
        &texture_info,
        Budgeted::No,
        "TextureProxyTestTexture",
    );
    assert!(texture_proxy.is_none());
    let texture_proxy = TextureProxy::make(
        &MockCaps::default(),
        &mut resource_provider,
        valid_size,
        &TextureInfo::new(),
        Budgeted::No,
        "TextureProxyTestTexture",
    );
    assert!(texture_proxy.is_none());

    // Non-budgeted, non-lazy TextureProxy is instantiated on return
    let texture_proxy = TextureProxy::make(
        &MockCaps::default(),
        &mut resource_provider,
        valid_size,
        &texture_info,
        Budgeted::No,
        "TextureProxyTestTexture",
    )
    .unwrap();
    assert!(!texture_proxy.is_lazy());
    assert!(!texture_proxy.is_fully_lazy());
    assert!(!texture_proxy.is_volatile());
    assert!(texture_proxy.is_instantiated());
    assert_eq!(texture_proxy.dimensions(), valid_size);

    // Budgeted, non-lazy TextureProxy, successful instantiation later on
    let texture_proxy = TextureProxy::make(
        &MockCaps::default(),
        &mut resource_provider,
        valid_size,
        &texture_info,
        Budgeted::Yes,
        "TextureProxyTestTexture",
    )
    .unwrap();
    assert!(!texture_proxy.is_lazy());
    assert!(!texture_proxy.is_fully_lazy());
    assert!(!texture_proxy.is_volatile());
    assert!(!texture_proxy.is_instantiated());
    assert_eq!(texture_proxy.dimensions(), valid_size);

    let instantiate_success = texture_proxy.instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert!(texture_proxy.is_instantiated());
    assert_eq!(texture_proxy.dimensions(), valid_size);
    let created_texture = texture_proxy.ref_texture().unwrap();

    let instantiate_success = texture_proxy.instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert!(texture_proxy.with_texture(|t| same_texture(t, &created_texture)));

    // Lazy, non-volatile TextureProxy, unsuccessful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        &MockCaps::default(),
        valid_size,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        null_callback(),
    )
    .unwrap();
    assert!(texture_proxy.is_lazy());
    assert!(!texture_proxy.is_fully_lazy());
    assert!(!texture_proxy.is_volatile());

    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(!instantiate_success);
    assert!(!texture_proxy.is_instantiated());

    // Lazy, non-volatile TextureProxy, successful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        &MockCaps::default(),
        valid_size,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        callback(&texture),
    )
    .unwrap();

    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert!(texture_proxy.with_texture(|t| same_texture(t, &texture)));

    // Lazy, volatile TextureProxy, unsuccessful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        &MockCaps::default(),
        valid_size,
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        null_callback(),
    )
    .unwrap();
    assert!(texture_proxy.is_lazy());
    assert!(!texture_proxy.is_fully_lazy());
    assert!(texture_proxy.is_volatile());

    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(!instantiate_success);
    assert!(!texture_proxy.is_instantiated());

    // Lazy, volatile TextureProxy, successful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        &MockCaps::default(),
        valid_size,
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        callback(&texture),
    )
    .unwrap();

    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert!(texture_proxy.with_texture(|t| same_texture(t, &texture)));

    texture_proxy.deinstantiate();
    assert!(!texture_proxy.is_instantiated());

    // Fully-lazy TextureProxy.
    let texture_proxy = TextureProxy::make_fully_lazy(
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        assignable_callback(),
    );
    assert!(texture_proxy.is_lazy());
    assert!(texture_proxy.is_fully_lazy());
    assert!(texture_proxy.is_volatile());

    *assignable_texture.lock().unwrap() = Some(texture.clone());
    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert!(texture_proxy.is_instantiated());
    assert!(texture_proxy.is_fully_lazy());
    assert_eq!(texture_proxy.dimensions(), valid_size);

    texture_proxy.deinstantiate();
    assert!(!texture_proxy.is_instantiated());
    assert!(texture_proxy.is_fully_lazy());

    let larger_size = ISize::new(2, 2);
    *assignable_texture.lock().unwrap() = Some(wrapped_texture(larger_size));
    let instantiate_success = texture_proxy.lazy_instantiate(&mut resource_provider);
    assert!(instantiate_success);
    assert_eq!(texture_proxy.dimensions(), larger_size);

    // InstantiateIfNotLazy tests.
    let texture_proxy = TextureProxy::make(
        &MockCaps::default(),
        &mut resource_provider,
        valid_size,
        &texture_info,
        Budgeted::Yes,
        "TextureProxyTestTexture",
    )
    .unwrap();
    let instantiate_success =
        TextureProxy::instantiate_if_not_lazy(&mut resource_provider, &texture_proxy);
    assert!(instantiate_success);

    let texture_proxy = TextureProxy::make_lazy(
        &MockCaps::default(),
        valid_size,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        null_callback(),
    )
    .unwrap();
    let instantiate_success =
        TextureProxy::instantiate_if_not_lazy(&mut resource_provider, &texture_proxy);
    assert!(instantiate_success);
}

#[test]
fn provider_reuses_returned_textures() {
    let (mut provider, created) = provider();
    let info = rgba_info();
    let size = ISize::new(8, 8);

    let t1 = provider
        .find_or_create_non_shareable_texture(size, &info, "a", Budgeted::Yes)
        .unwrap();
    assert_eq!(created.load(Ordering::Relaxed), 1);
    assert_eq!(provider.get_resource_cache_current_budgeted_bytes(), 256);
    let id = t1.base().unique_id();
    drop(t1);

    // The returned texture is found again (after the return queue is processed), relabeled.
    let t2 = provider
        .find_or_create_non_shareable_texture(size, &info, "b", Budgeted::Yes)
        .unwrap();
    assert_eq!(created.load(Ordering::Relaxed), 1);
    assert_eq!(t2.base().unique_id(), id);
    assert_eq!(t2.base().label(), "b");

    // A shareable request for the same key while it is held creates another texture.
    let s1 = provider
        .find_or_create_shareable_texture(size, &info, "s")
        .unwrap();
    let s2 = provider
        .find_or_create_shareable_texture(size, &info, "s")
        .unwrap();
    assert_eq!(created.load(Ordering::Relaxed), 2);
    assert!(ResourceRef::ptr_eq(&s1, &s2));

    drop((t2, s1, s2));
    provider.free_gpu_resources();
    assert_eq!(provider.get_resource_cache_current_budgeted_bytes(), 0);
}

#[test]
fn scratch_manager_filters_unavailable_textures() {
    let (mut provider, created) = provider();
    let info = rgba_info();
    let size = ISize::new(4, 4);
    let mut scratch = ScratchResourceManager::new(ProxyReadCountMap::new());

    let a = scratch
        .get_scratch_texture(&mut provider, size, &info, "a")
        .unwrap();
    // `a` is unavailable to this manager, so a second request creates a new texture.
    let b = scratch
        .get_scratch_texture(&mut provider, size, &info, "b")
        .unwrap();
    assert!(!ResourceRef::ptr_eq(&a, &b));
    assert_eq!(created.load(Ordering::Relaxed), 2);

    // Returning `a` makes it reusable within the recording even though it is still held.
    scratch.return_texture(&a);
    let c = scratch
        .get_scratch_texture(&mut provider, size, &info, "c")
        .unwrap();
    assert!(ResourceRef::ptr_eq(&a, &c));
    assert_eq!(created.load(Ordering::Relaxed), 2);
    scratch.return_texture(&b);
    scratch.return_texture(&c);
}

fn make_bitmap(color: Color) -> Bitmap {
    let mut bitmap = Bitmap::new();
    assert!(bitmap.try_alloc_n32_pixels((4, 4), false));
    bitmap.erase_color(color);
    bitmap
}

// Adapted from: tests/graphite/ProxyCacheTest.cpp#L32-L55 (chrome/m156) (ProxyCacheTest1)
#[test]
fn proxy_cache_invalidates_on_bitmap_change() {
    let (mut provider, _) = provider();
    let info = rgba_info();
    let mut bitmap = make_bitmap(Color::RED);

    let proxy_cache = provider.proxy_cache().unwrap();
    assert_eq!(proxy_cache.num_cached(), 0);

    let key = skia_rust_gpu::graphite::proxy_cache::ProxyCache::bitmap_key(&bitmap);
    let listener = proxy_cache.make_unique_key_invalidation_listener(&key);
    let proxy = proxy_cache.find_or_create_cache_entry(&key, "ProxyCacheTestTexture", |label| {
        assert_eq!(label, "ProxyCacheTestTexture");
        bitmap
            .pixel_ref()
            .unwrap()
            .add_gen_id_change_listener(Some(listener.clone()));
        let proxy = TextureProxy::make_fully_lazy(
            &info,
            Budgeted::Yes,
            Volatile::No,
            Box::new(|_: &mut ResourceProvider| None),
        );
        Some((proxy, Some(listener.clone())))
    });
    assert!(proxy.is_some());
    assert_eq!(proxy_cache.num_cached(), 1);
    assert!(proxy_cache.find(&bitmap).is_some());

    bitmap.erase_color(Color::BLACK);

    proxy_cache.force_process_invalid_key_msgs();

    assert_eq!(proxy_cache.num_cached(), 0);
}

#[test]
fn proxy_cache_frees_uniquely_held_proxies() {
    let (mut provider, _) = provider();
    let info = rgba_info();
    let bitmaps = [make_bitmap(Color::RED), make_bitmap(Color::BLUE)];
    let proxy_cache = provider.proxy_cache().unwrap();

    let mut proxies = Vec::new();
    for bitmap in &bitmaps {
        let key = skia_rust_gpu::graphite::proxy_cache::ProxyCache::bitmap_key(bitmap);
        let proxy = proxy_cache
            .find_or_create_cache_entry(&key, "", |label| {
                assert_eq!(label, "ProxyCache");
                let proxy = TextureProxy::make_fully_lazy(
                    &info,
                    Budgeted::Yes,
                    Volatile::No,
                    Box::new(|_: &mut ResourceProvider| None),
                );
                Some((proxy, None))
            })
            .unwrap();
        proxies.push(proxy);
    }
    assert_eq!(proxy_cache.num_cached(), 2);

    // Both are held outside the cache.
    proxy_cache.force_free_uniquely_held();
    assert_eq!(proxy_cache.num_cached(), 2);

    proxies.pop();
    proxy_cache.force_free_uniquely_held();
    assert_eq!(proxy_cache.num_cached(), 1);
    assert!(proxy_cache.find(&bitmaps[0]).is_some());
    assert!(proxy_cache.find(&bitmaps[1]).is_none());
}
