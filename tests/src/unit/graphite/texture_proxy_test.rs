// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/TextureProxyTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ tests, which declare constants and similarly named bindings inline.
#![allow(clippy::items_after_statements, clippy::similar_names)]

use std::sync::{Arc, Mutex};

use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::graphite_types::Volatile;
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::texture::Texture;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `GraphiteTextureTooLargeTest` and `GraphiteLazyTextureInvalidDimensions` of this file make
// Graphite images, surfaces and promise images (G10d); they stay `todo`.

def_graphite_test_for_all_contexts!(GraphiteTextureProxyTest, |reporter, context| {
    // Port of: tests/graphite/TextureProxyTest.cpp#L27-L200 (chrome/m156)
    let caps = context.wgpu_caps();
    const K_VALID_SIZE: ISize = ISize {
        width: 1,
        height: 1,
    };
    const K_INVALID_SIZE: ISize = ISize {
        width: 0,
        height: 0,
    };
    const K_VALID_COLOR_TYPE: ColorType = ColorType::RGBA8888;
    const K_INVALID_COLOR_TYPE: ColorType = ColorType::Unknown;

    let is_protected = if caps.protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };

    let mut recorder = context.make_recorder(None);
    // `recorder->priv().resourceProvider()`: the recorder locks the provider itself, so it is
    // locked only for the duration of each use.
    let resource_provider = recorder.priv_().resource_provider().clone();
    let texture_info = caps.get_default_sampled_texture_info(
        K_VALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
    );
    let backend_texture = recorder.create_backend_texture(K_VALID_SIZE, &texture_info);
    let texture: Option<ResourceRef<Texture>> = resource_provider
        .lock()
        .unwrap()
        .create_wrapped_texture(&backend_texture, "TextureProxyTestWrappedTex");

    let make_proxy = |dimensions: ISize,
                      color_type: ColorType,
                      mipmapped: Mipmapped,
                      is_protected: Protected,
                      renderable: Renderable,
                      budgeted: Budgeted| {
        let texture_info =
            caps.get_default_sampled_texture_info(color_type, mipmapped, is_protected, renderable);
        TextureProxy::make(
            caps,
            &mut resource_provider.lock().unwrap(),
            dimensions,
            &texture_info,
            budgeted,
            "TextureProxyTestTexture",
        )
    };

    let null_callback = |_: &mut skia_rust_gpu::graphite::resource_provider::ResourceProvider| None;
    let callback_texture = texture.clone();
    let callback = move |_: &mut skia_rust_gpu::graphite::resource_provider::ResourceProvider| {
        callback_texture.clone()
    };

    // Assign to assignableTexture before instantiating with this callback.
    let assignable_texture: Arc<Mutex<Option<ResourceRef<Texture>>>> = Arc::new(Mutex::new(None));
    let assignable_callback = {
        let assignable_texture = assignable_texture.clone();
        move |_: &mut skia_rust_gpu::graphite::resource_provider::ResourceProvider| {
            assignable_texture.lock().unwrap().clone()
        }
    };
    let is_the_texture = |proxy: &TextureProxy, expected: &Option<ResourceRef<Texture>>| {
        proxy.with_texture(|actual| match (actual, expected) {
            (Some(actual), Some(expected)) => ResourceRef::ptr_eq(actual, expected),
            (None, None) => true,
            _ => false,
        })
    };

    // Invalid parameters.
    let mut texture_proxy = make_proxy(
        K_INVALID_SIZE,
        K_VALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
        Budgeted::No,
    );
    reporter_assert!(reporter, texture_proxy.is_none());
    texture_proxy = make_proxy(
        K_VALID_SIZE,
        K_INVALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
        Budgeted::No,
    );
    reporter_assert!(reporter, texture_proxy.is_none());

    // Non-budgeted, non-lazy TextureProxy is instantiated on return
    texture_proxy = make_proxy(
        K_VALID_SIZE,
        K_VALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
        Budgeted::No,
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the non-budgeted proxy is made");
        return;
    };
    reporter_assert!(reporter, !proxy.is_lazy());
    reporter_assert!(reporter, !proxy.is_fully_lazy());
    reporter_assert!(reporter, !proxy.is_volatile());
    reporter_assert!(reporter, proxy.is_instantiated());
    reporter_assert!(reporter, proxy.dimensions() == K_VALID_SIZE);

    // Budgeted, non-lazy TextureProxy, successful instantiation later on
    texture_proxy = make_proxy(
        K_VALID_SIZE,
        K_VALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
        Budgeted::Yes,
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the budgeted proxy is made");
        return;
    };
    reporter_assert!(reporter, !proxy.is_lazy());
    reporter_assert!(reporter, !proxy.is_fully_lazy());
    reporter_assert!(reporter, !proxy.is_volatile());
    reporter_assert!(reporter, !proxy.is_instantiated());
    reporter_assert!(reporter, proxy.dimensions() == K_VALID_SIZE);

    let mut instantiate_success = proxy.instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, proxy.is_instantiated());
    reporter_assert!(reporter, proxy.dimensions() == K_VALID_SIZE);
    let created_texture = proxy.ref_texture();

    instantiate_success = proxy.instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, is_the_texture(proxy, &created_texture));

    // Lazy, non-volatile TextureProxy, unsuccessful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        caps,
        K_VALID_SIZE,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        Box::new(null_callback),
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the lazy proxy is made");
        return;
    };
    reporter_assert!(reporter, proxy.is_lazy());
    reporter_assert!(reporter, !proxy.is_fully_lazy());
    reporter_assert!(reporter, !proxy.is_volatile());

    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, !instantiate_success);
    reporter_assert!(reporter, !proxy.is_instantiated());

    // Lazy, non-volatile TextureProxy, successful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        caps,
        K_VALID_SIZE,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        Box::new(callback.clone()),
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the lazy proxy is made");
        return;
    };

    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, is_the_texture(proxy, &texture));

    // Lazy, volatile TextureProxy, unsuccessful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        caps,
        K_VALID_SIZE,
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        Box::new(null_callback),
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the volatile lazy proxy is made");
        return;
    };
    reporter_assert!(reporter, proxy.is_lazy());
    reporter_assert!(reporter, !proxy.is_fully_lazy());
    reporter_assert!(reporter, proxy.is_volatile());

    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, !instantiate_success);
    reporter_assert!(reporter, !proxy.is_instantiated());

    // Lazy, volatile TextureProxy, successful instantiation.
    let texture_proxy = TextureProxy::make_lazy(
        caps,
        K_VALID_SIZE,
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        Box::new(callback),
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the volatile lazy proxy is made");
        return;
    };

    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, is_the_texture(proxy, &texture));

    proxy.deinstantiate();
    reporter_assert!(reporter, !proxy.is_instantiated());

    // Fully-lazy TextureProxy.
    let texture_proxy = TextureProxy::make_fully_lazy(
        &texture_info,
        Budgeted::No,
        Volatile::Yes,
        Box::new(assignable_callback),
    );
    let proxy = &*texture_proxy;
    reporter_assert!(reporter, proxy.is_lazy());
    reporter_assert!(reporter, proxy.is_fully_lazy());
    reporter_assert!(reporter, proxy.is_volatile());

    *assignable_texture.lock().unwrap() = texture.clone();
    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, proxy.is_instantiated());
    reporter_assert!(reporter, proxy.is_fully_lazy());
    reporter_assert!(reporter, proxy.dimensions() == K_VALID_SIZE);

    proxy.deinstantiate();
    reporter_assert!(reporter, !proxy.is_instantiated());
    reporter_assert!(reporter, proxy.is_fully_lazy());

    const K_LARGER_SIZE: ISize = ISize {
        width: 2,
        height: 2,
    };
    let larger_backend_texture = recorder.create_backend_texture(K_LARGER_SIZE, &texture_info);
    *assignable_texture.lock().unwrap() = resource_provider
        .lock()
        .unwrap()
        .create_wrapped_texture(&larger_backend_texture, "TextureProxyTestWrappedTex");
    instantiate_success = proxy.lazy_instantiate(&mut resource_provider.lock().unwrap());
    reporter_assert!(reporter, instantiate_success);
    reporter_assert!(reporter, proxy.dimensions() == K_LARGER_SIZE);

    // InstantiateIfNotLazy tests.
    let texture_proxy = make_proxy(
        K_VALID_SIZE,
        K_VALID_COLOR_TYPE,
        Mipmapped::No,
        is_protected,
        Renderable::No,
        Budgeted::Yes,
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the proxy is made");
        return;
    };
    instantiate_success =
        TextureProxy::instantiate_if_not_lazy(&mut resource_provider.lock().unwrap(), proxy);
    reporter_assert!(reporter, instantiate_success);

    let texture_proxy = TextureProxy::make_lazy(
        caps,
        K_VALID_SIZE,
        &texture_info,
        Budgeted::No,
        Volatile::No,
        Box::new(null_callback),
    );
    let Some(proxy) = texture_proxy.as_deref() else {
        reporter_assert!(reporter, false, "the lazy proxy is made");
        return;
    };
    instantiate_success =
        TextureProxy::instantiate_if_not_lazy(&mut resource_provider.lock().unwrap(), proxy);
    reporter_assert!(reporter, instantiate_success);
    // Clean up the backend textures.
    recorder.delete_backend_texture(&backend_texture);
    recorder.delete_backend_texture(&larger_backend_texture);
});
