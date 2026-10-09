// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The wgpu back end (G11a) on wgpu's `noop` backend: the resource creation paths of
//! `DawnTexture`, `DawnBuffer`, `DawnSampler`, `DawnBackendTexture`, `DawnResourceProvider` and
//! `DawnSharedContext`, with no GPU. The noop backend stores no texels and runs no passes, so
//! nothing here reads pixels (`docs/design/gpu.md` §7).
#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::gpu_types::{BackendApi, Budgeted, CallbackResult, Mipmapped, Protected};
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::buffer::BindBufferInfo;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::graphite_types::SampleCount;
use skia_rust_gpu::graphite::recorder::RecorderSharedContext;
use skia_rust_gpu::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use skia_rust_gpu::graphite::resource_provider::ResourceProvider;
use skia_rust_gpu::graphite::resource_types::{
    AccessPattern, BufferType, Ownership, SamplerDesc, Shareable,
};
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_info::TextureInfo;
use skia_rust_gpu::graphite::wgpu::buffer::as_wgpu_buffer;
use skia_rust_gpu::graphite::wgpu::resource_provider::{
    find_or_create_discardable_msaa_load_texture, wgpu_backend,
};
use skia_rust_gpu::graphite::wgpu::sampler::as_wgpu_sampler;
use skia_rust_gpu::graphite::wgpu::texture::as_wgpu_texture;
use skia_rust_gpu::graphite::wgpu::{
    CapsProfile, DeviceFeatures, WgpuBackendContext, WgpuSharedContext, WgpuTextureInfo,
    backend_textures, make_context, noop_backend_context, noop_backend_context_with_features,
    texture_infos,
};
use std::sync::{Arc, Mutex};

/// A shared context on a noop device that has the features `profile` needs.
fn shared_context(profile: &CapsProfile, options: &ContextOptions) -> Arc<WgpuSharedContext> {
    let mut features = wgpu::Features::empty();
    if profile
        .features
        .contains(DeviceFeatures::BUFFER_MAP_EXTENDED_USAGES)
    {
        features |= wgpu::Features::MAPPABLE_PRIMARY_BUFFERS;
    }
    let backend_context =
        noop_backend_context_with_features(features, wgpu::Limits::default()).unwrap();
    WgpuSharedContext::make_with_profile(&backend_context, profile, options).unwrap()
}

/// The Dawn D3D12 profile, but with mappable draw buffers so host-visible buffers can be tested.
fn mappable_profile() -> CapsProfile {
    let mut profile = CapsProfile::dawn_d3d12();
    profile.features |= DeviceFeatures::BUFFER_MAP_EXTENDED_USAGES;
    profile
}

fn provider(shared_context: &Arc<WgpuSharedContext>) -> ResourceProvider {
    shared_context.make_resource_provider(1, 1 << 24)
}

fn texture_info(
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    samples: SampleCount,
    mipmapped: Mipmapped,
) -> TextureInfo {
    texture_infos::make_wgpu(&WgpuTextureInfo::new(
        samples,
        mipmapped,
        format,
        usage,
        wgpu::TextureAspect::All,
    ))
}

fn sampled_info() -> TextureInfo {
    texture_info(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        SampleCount::One,
        Mipmapped::No,
    )
}

#[test]
fn shared_context_reads_the_device() {
    // A bare noop device has no optional features; the caps follow what the device has.
    let backend_context = noop_backend_context();
    let shared = WgpuSharedContext::make(&backend_context, &ContextOptions::default()).unwrap();
    assert_eq!(shared.backend(), BackendApi::Dawn);
    assert_eq!(shared.is_protected(), Protected::No);
    assert!(shared.has_tick());
    let caps = shared.caps();
    assert_eq!(caps.profile().backend, wgpu::Backend::Noop);
    assert!(!caps.supports_half_precision());
    assert!(!caps.shader_caps().dual_source_blending_support);
    assert!(!Caps::draw_buffer_can_be_mapped(&**caps));
    assert_eq!(caps.max_texture_size(), 8192);
    // wgpu has no DawnLoadResolveTexture or transient attachment here: Graphite emulates the
    // load/store resolve.
    assert!(caps.emulate_load_store_resolve());

    // Features the device was created with show up in the caps.
    let backend_context = noop_backend_context_with_features(
        wgpu::Features::SHADER_F16
            | wgpu::Features::DUAL_SOURCE_BLENDING
            | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
            | wgpu::Features::TEXTURE_COMPRESSION_BC,
        wgpu::Limits::default(),
    )
    .unwrap();
    let shared = WgpuSharedContext::make(&backend_context, &ContextOptions::default()).unwrap();
    let caps = shared.caps();
    assert!(caps.supports_half_precision());
    assert!(caps.shader_caps().dual_source_blending_support);
    assert!(Caps::draw_buffer_can_be_mapped(&**caps));
    assert!(
        caps.profile()
            .features
            .contains(DeviceFeatures::TEXTURE_COMPRESSION_BC)
    );
    assert!(
        caps.format_support(TextureFormat::RGBA8_BC1)
            .usage
            .contains(skia_rust_gpu::graphite::resource_types::TextureUsage::SAMPLE)
    );
}

#[test]
fn make_context_makes_recorders() {
    let backend_context = noop_backend_context();
    let context = make_context(&backend_context, &ContextOptions::default()).unwrap();
    assert_eq!(context.backend(), BackendApi::Dawn);
    assert_eq!(ContextPriv::caps(&context).max_texture_size(), 8192);
    assert_eq!(
        ContextPriv::resource_provider(&context)
            .lock()
            .unwrap()
            .get_resource_cache_limit(),
        ContextOptions::default().gpu_budget_in_bytes
    );

    let mut recorder = context.make_recorder(None);
    assert_eq!(recorder.backend(), BackendApi::Dawn);
    assert_eq!(recorder.max_texture_size(), 8192);
    // Nothing was recorded.
    let mut recording = recorder.snap().expect("an empty recording");
    assert!(!recording.priv_().has_tasks());
}

#[test]
fn creates_and_caches_textures() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let info = sampled_info();
    let dims = ISize::new(16, 8);

    let texture = provider
        .find_or_create_non_shareable_texture(dims, &info, "T", Budgeted::Yes)
        .unwrap();
    assert_eq!(texture.dimensions(), dims);
    assert_eq!(texture.texture_info(), &info);
    assert_eq!(texture.base().ownership(), Ownership::Owned);
    assert_eq!(texture.base().gpu_memory_size(), 16 * 8 * 4);
    let wgpu_texture = as_wgpu_texture(&texture).unwrap();
    let raw = wgpu_texture.wgpu_texture().unwrap();
    assert_eq!((raw.width(), raw.height()), (16, 8));
    assert_eq!(raw.format(), wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(raw.mip_level_count(), 1);
    assert!(wgpu_texture.sample_texture_view().is_some());
    assert!(wgpu_texture.render_texture_view().is_some());

    // A shareable texture goes back to the cache and is found again.
    let shared_texture = provider
        .find_or_create_shareable_texture(dims, &info, "S")
        .unwrap();
    let id = shared_texture.base().unique_id();
    drop(shared_texture);
    provider.force_process_returned_resources();
    let again = provider
        .find_or_create_shareable_texture(dims, &info, "S")
        .unwrap();
    assert_eq!(again.base().unique_id(), id);
    assert_eq!(again.base().shareable(), Shareable::Yes);
    // A different size is a different texture.
    let other = provider
        .find_or_create_shareable_texture(ISize::new(16, 9), &info, "S")
        .unwrap();
    assert_ne!(other.base().unique_id(), id);

    // Freeing the GPU data destroys the wgpu objects.
    drop(texture);
    provider.free_gpu_resources();
}

#[test]
fn texture_mip_levels_and_msaa() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);

    let mipped = texture_info(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        SampleCount::One,
        Mipmapped::Yes,
    );
    let texture = provider
        .find_or_create_shareable_texture(ISize::new(16, 16), &mipped, "M")
        .unwrap();
    let raw = as_wgpu_texture(&texture).unwrap().wgpu_texture().unwrap();
    assert_eq!(raw.mip_level_count(), 5);

    // The render view of a mipmapped texture is distinct from its sample view.
    let msaa = texture_info(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        SampleCount::Four,
        Mipmapped::No,
    );
    let msaa_texture = provider
        .find_or_create_shareable_texture(ISize::new(8, 8), &msaa, "MSAA")
        .unwrap();
    let raw = as_wgpu_texture(&msaa_texture)
        .unwrap()
        .wgpu_texture()
        .unwrap();
    assert_eq!(raw.sample_count(), 4);
    assert_eq!(msaa_texture.base().gpu_memory_size(), 8 * 8 * 4 * 4);

    // The load texture of an MSAA texture is single sampled and sampleable.
    let load = find_or_create_discardable_msaa_load_texture(&mut provider, ISize::new(8, 8), &msaa)
        .unwrap();
    let load_info = texture_infos::get_wgpu_texture_info(load.texture_info()).unwrap();
    assert_eq!(load_info.sample_count, SampleCount::One);
    assert!(
        load_info.usage.contains(
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
        )
    );
}

#[test]
fn rejects_invalid_textures() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let info = sampled_info();

    // Too large.
    assert!(
        provider
            .find_or_create_shareable_texture(ISize::new(8193, 1), &info, "")
            .is_none()
    );
    // An invalid info.
    assert!(
        provider
            .find_or_create_shareable_texture(ISize::new(4, 4), &TextureInfo::new(), "")
            .is_none()
    );
    // A usage the format does not support: RGBA8 cannot be a storage texture of 8 samples, and
    // BC1 needs the compression feature.
    let bc1 = texture_info(
        wgpu::TextureFormat::Bc1RgbaUnorm,
        wgpu::TextureUsages::TEXTURE_BINDING,
        SampleCount::One,
        Mipmapped::No,
    );
    assert!(
        provider
            .find_or_create_shareable_texture(ISize::new(8, 8), &bc1, "")
            .is_none()
    );
    let eight = texture_info(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        SampleCount::Eight,
        Mipmapped::No,
    );
    assert!(
        provider
            .find_or_create_shareable_texture(ISize::new(8, 8), &eight, "")
            .is_none()
    );
}

#[test]
fn creates_buffers_of_every_type() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    for ty in [
        BufferType::Vertex,
        BufferType::Index,
        BufferType::XferCpuToGpu,
        BufferType::XferGpuToCpu,
        BufferType::Uniform,
        BufferType::Storage,
        BufferType::Query,
        BufferType::Indirect,
        BufferType::VertexStorage,
        BufferType::IndexStorage,
    ] {
        let buffer = provider
            .find_or_create_non_shareable_buffer(256, ty, AccessPattern::GpuOnly, "B")
            .unwrap_or_else(|| panic!("{ty:?} buffer"));
        assert_eq!(buffer.size(), 256);
        assert!(!buffer.is_mapped());
        let raw = as_wgpu_buffer(&buffer).unwrap().wgpu_buffer().unwrap();
        assert_eq!(raw.size(), 256);
    }
    // An empty buffer cannot be created.
    assert!(
        provider
            .find_or_create_non_shareable_buffer(0, BufferType::Vertex, AccessPattern::GpuOnly, "")
            .is_none()
    );
}

#[test]
fn host_visible_buffers_remap_when_returned_to_the_cache() {
    let shared = shared_context(&mappable_profile(), &ContextOptions::default());
    let mut provider = provider(&shared);

    // The device can map draw buffers, so a host-visible vertex buffer is created mapped.
    let buffer = provider
        .find_or_create_non_shareable_buffer(
            64,
            BufferType::Vertex,
            AccessPattern::HostVisible,
            "H",
        )
        .unwrap();
    let id = buffer.base().unique_id();
    assert!(buffer.is_unmappable());
    let usage = as_wgpu_buffer(&buffer)
        .unwrap()
        .wgpu_buffer()
        .unwrap()
        .usage();
    assert!(usage.contains(wgpu::BufferUsages::MAP_WRITE));
    assert!(!usage.contains(wgpu::BufferUsages::COPY_DST));

    // Write through the staging block; the bytes land in the wgpu mapped range.
    let mut staging = buffer.map().expect("mapped at creation");
    assert_eq!(staging.len(), 64);
    staging[..4].copy_from_slice(&[1, 2, 3, 4]);
    buffer.unmap_with(&staging);
    assert!(!buffer.is_unmappable());
    assert!(!buffer.is_mapped());

    // Dropping the last ref returns it to the cache, which starts an async map before it can
    // be reused.
    drop(buffer);
    provider.force_process_returned_resources();
    shared.tick();
    shared.wait_for_gpu();
    provider.force_process_returned_resources();
    let again = provider
        .find_or_create_non_shareable_buffer(
            64,
            BufferType::Vertex,
            AccessPattern::HostVisible,
            "H",
        )
        .unwrap();
    assert_eq!(again.base().unique_id(), id);
    assert!(again.is_unmappable());
    assert!(again.map().is_some());
    again.unmap();
}

#[test]
fn async_map_of_a_readback_buffer() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    assert!(Caps::buffer_maps_are_async(&**shared.caps()));
    let mut provider = provider(&shared);
    let buffer = provider
        .find_or_create_non_shareable_buffer(
            32,
            BufferType::XferGpuToCpu,
            AccessPattern::HostVisible,
            "R",
        )
        .unwrap();
    assert!(!buffer.is_unmappable());

    let result = Arc::new(Mutex::new(None));
    let result_in_proc = result.clone();
    buffer.async_map(Some(Box::new(move |r| {
        *result_in_proc.lock().unwrap() = Some(r);
    })));
    assert!(buffer.is_unmappable());
    shared.wait_for_gpu();
    shared.tick();
    assert_eq!(*result.lock().unwrap(), Some(CallbackResult::Success));

    // Already mapped: the proc runs at once.
    let again = Arc::new(Mutex::new(None));
    let again_in_proc = again.clone();
    buffer.async_map(Some(Box::new(move |r| {
        *again_in_proc.lock().unwrap() = Some(r);
    })));
    assert_eq!(*again.lock().unwrap(), Some(CallbackResult::Success));

    let contents = buffer.map().expect("mapped");
    assert_eq!(contents, vec![0; 32]);
    buffer.unmap();
    assert!(!buffer.is_unmappable());
}

#[test]
fn creates_samplers_through_the_cache() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let desc = SamplerDesc::new(
        &SamplingOptions {
            filter: FilterMode::Linear,
            mipmap: MipmapMode::Nearest,
            ..SamplingOptions::default()
        },
        TileMode::Repeat,
    );
    let sampler = provider.find_or_create_compatible_sampler(&desc).unwrap();
    assert_eq!(as_wgpu_sampler(&sampler).unwrap().sampler_desc(), desc);
    assert!(as_wgpu_sampler(&sampler).unwrap().wgpu_sampler().is_some());
    let id = sampler.base().unique_id();
    drop(sampler);
    provider.force_process_returned_resources();

    // The same description finds the same sampler, a different one does not.
    let again = provider.find_or_create_compatible_sampler(&desc).unwrap();
    assert_eq!(again.base().unique_id(), id);
    let other = provider
        .find_or_create_compatible_sampler(&SamplerDesc::new(
            &SamplingOptions::default(),
            TileMode::Clamp,
        ))
        .unwrap();
    assert_ne!(other.base().unique_id(), id);

    // wgpu has no immutable (YCbCr) samplers.
    let immutable = SamplerDesc::from_raw(
        desc.desc() | (1 << SamplerDesc::IMMUTABLE_SAMPLER_INFO_SHIFT),
        3,
        0,
    );
    if immutable.is_immutable() {
        assert!(
            provider
                .find_or_create_compatible_sampler(&immutable)
                .is_none()
        );
    }
}

#[test]
fn backend_textures_wrap_and_delete() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let info = sampled_info();
    let dims = ISize::new(12, 6);

    let backend_texture = provider.create_backend_texture(dims, &info);
    assert!(backend_texture.is_valid());
    assert_eq!(backend_texture.backend(), BackendApi::Dawn);
    assert_eq!(backend_texture.dimensions(), dims);
    // The info is queried from the wgpu texture.
    let queried = texture_infos::get_wgpu_texture_info(&backend_texture.info()).unwrap();
    assert_eq!(queried.format, Some(wgpu::TextureFormat::Rgba8Unorm));
    assert!(queried.usage.contains(wgpu::TextureUsages::TEXTURE_BINDING));
    let raw = backend_textures::get_wgpu_texture(&backend_texture).unwrap();
    assert!(backend_textures::get_wgpu_texture_view(&backend_texture).is_none());

    // Copies are equal; a different texture is not.
    assert_eq!(backend_texture, backend_texture.clone());
    let other = provider.create_backend_texture(dims, &info);
    assert_ne!(backend_texture, other);
    assert_ne!(backend_texture, BackendTexture::new());

    // A wrapped texture is not owned and not cached, and wraps the same wgpu texture.
    let wrapped = provider
        .create_wrapped_texture(&backend_texture, "W")
        .unwrap();
    assert_eq!(wrapped.base().ownership(), Ownership::Wrapped);
    assert_eq!(wrapped.dimensions(), dims);
    assert_eq!(
        as_wgpu_texture(&wrapped).unwrap().wgpu_texture().unwrap(),
        raw
    );

    // Wrapping a bare view.
    let view = as_wgpu_texture(&wrapped)
        .unwrap()
        .sample_texture_view()
        .unwrap();
    let from_view = backend_textures::make_wgpu_from_view(
        dims,
        &WgpuTextureInfo::new(
            SampleCount::One,
            Mipmapped::No,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureAspect::All,
        ),
        &view,
    );
    // The copy usages cannot be used without the texture.
    let stripped = texture_infos::get_wgpu_texture_info(&from_view.info()).unwrap();
    assert_eq!(stripped.usage, wgpu::TextureUsages::TEXTURE_BINDING);
    let wrapped_view = provider.create_wrapped_texture(&from_view, "V").unwrap();
    assert!(
        as_wgpu_texture(&wrapped_view)
            .unwrap()
            .wgpu_texture()
            .is_none()
    );
    assert!(
        as_wgpu_texture(&wrapped_view)
            .unwrap()
            .sample_texture_view()
            .is_some()
    );

    // Invalid dimensions, and an invalid backend texture, make nothing.
    assert!(
        !provider
            .create_backend_texture(ISize::new(0, 3), &info)
            .is_valid()
    );
    assert!(
        !provider
            .create_backend_texture(ISize::new(9000, 3), &info)
            .is_valid()
    );
    assert!(
        provider
            .create_wrapped_texture(&BackendTexture::new(), "")
            .is_none()
    );

    // Freeing a wrapped texture leaves the client's texture alone; deleting destroys it.
    drop(wrapped);
    provider.free_gpu_resources();
    provider.delete_backend_texture(&backend_texture);
    provider.delete_backend_texture(&other);
}

#[test]
fn single_texture_sampler_bind_groups_are_cached_on_the_texture() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let texture = provider
        .find_or_create_shareable_texture(ISize::new(4, 4), &sampled_info(), "T")
        .unwrap();
    let sampler = provider
        .find_or_create_compatible_sampler(&SamplerDesc::new(
            &SamplingOptions::default(),
            TileMode::Clamp,
        ))
        .unwrap();
    let other_sampler = provider
        .find_or_create_compatible_sampler(&SamplerDesc::new(
            &SamplingOptions {
                filter: FilterMode::Linear,
                ..SamplingOptions::default()
            },
            TileMode::Mirror,
        ))
        .unwrap();

    let wgpu = wgpu_backend(&mut provider).unwrap();
    let first = wgpu
        .find_or_create_single_texture_sampler_bind_group(&sampler, &texture)
        .unwrap();
    let second = wgpu
        .find_or_create_single_texture_sampler_bind_group(&sampler, &texture)
        .unwrap();
    assert_eq!(first, second);
    let third = wgpu
        .find_or_create_single_texture_sampler_bind_group(&other_sampler, &texture)
        .unwrap();
    assert_ne!(first, third);
}

#[test]
fn single_uniform_bind_groups_are_cached_on_the_buffer() {
    // The D3D12 profile has 64 bytes of immediates, so intrinsics are push constants and a
    // draw binds a single combined buffer, a storage buffer as the profile supports them.
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    assert!(shared.caps().storage_buffer_support());
    let mut provider = provider(&shared);
    let buffer = provider
        .find_or_create_non_shareable_buffer(256, BufferType::Storage, AccessPattern::GpuOnly, "U")
        .unwrap();
    let info = BindBufferInfo::new(&buffer, 0, 100);

    let wgpu = wgpu_backend(&mut provider).unwrap();
    let first = wgpu
        .find_or_create_single_uniform_bind_group(&info)
        .unwrap();
    assert_eq!(
        wgpu.find_or_create_single_uniform_bind_group(&info)
            .unwrap(),
        first
    );
    // A different binding size is a different bind group.
    let other = BindBufferInfo::new(&buffer, 0, 200);
    assert_ne!(
        wgpu.find_or_create_single_uniform_bind_group(&other)
            .unwrap(),
        first
    );
    // The null buffer and texture view exist once.
    let null = wgpu.get_or_create_null_buffer().clone();
    assert_eq!(&null, wgpu.get_or_create_null_buffer());
    assert_eq!(null.size(), 16);
    let _ = wgpu.get_or_create_null_texture_view();
}

#[test]
fn blit_with_draw_pipelines_are_created_and_cached() {
    let shared = shared_context(&CapsProfile::dawn_vulkan(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let desc = RenderPassDesc {
        color_attachment: AttachmentDesc {
            format: TextureFormat::RGBA8,
            sample_count: SampleCount::Four,
            ..AttachmentDesc::default()
        },
        depth_stencil_attachment: AttachmentDesc {
            format: TextureFormat::D24_S8,
            sample_count: SampleCount::Four,
            ..AttachmentDesc::default()
        },
        ..RenderPassDesc::default()
    };
    let wgpu = wgpu_backend(&mut provider).unwrap();
    // Both shaders (single-sampled and MSAA source) are valid WGSL and make valid pipelines.
    assert!(
        wgpu.find_or_create_blit_with_draw_encoder(&desc, SampleCount::One)
            .is_valid()
    );
    assert!(
        wgpu.find_or_create_blit_with_draw_encoder(&desc, SampleCount::Four)
            .is_valid()
    );
    assert!(
        wgpu.find_or_create_blit_with_draw_encoder(&desc, SampleCount::One)
            .is_valid()
    );
}

#[test]
fn resource_cache_budget_counts_wgpu_resources() {
    let shared = shared_context(&CapsProfile::dawn_d3d12(), &ContextOptions::default());
    let mut provider = provider(&shared);
    let texture = provider
        .find_or_create_shareable_texture(ISize::new(8, 8), &sampled_info(), "T")
        .unwrap();
    assert_eq!(
        provider.get_resource_cache_current_budgeted_bytes(),
        8 * 8 * 4
    );
    assert_eq!(provider.get_resource_cache_current_purgeable_bytes(), 0);
    drop(texture);
    provider.force_process_returned_resources();
    assert_eq!(
        provider.get_resource_cache_current_purgeable_bytes(),
        8 * 8 * 4
    );
    provider.free_gpu_resources();
    assert_eq!(provider.get_resource_cache_current_budgeted_bytes(), 0);
}

#[test]
fn backend_context_defaults() {
    let backend_context = noop_backend_context();
    let WgpuBackendContext { has_tick, .. } = backend_context;
    assert!(has_tick, "native targets can poll");
}
