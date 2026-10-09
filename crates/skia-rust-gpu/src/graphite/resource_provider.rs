// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ResourceProvider.h, src/gpu/graphite/ResourceProvider.cpp
//                   (the backend-neutral half)

//! `ResourceProvider`: finds resources in the [`ResourceCache`] or creates them.
//!
//! This is the backend-neutral half of Skia's class. The backend half (Skia's pure virtuals plus
//! the `Caps` queries the neutral half makes) is the [`ResourceProviderBackend`] trait, which the
//! wgpu back end implements (`docs/design/gpu.md` §4.1: each base/backend pair becomes one struct;
//! the trait is the seam until that back end exists).
//!
//! Buffers, samplers, wrapped textures and backend textures are ported. Compute pipelines are
//! found or created by [`crate::graphite::shared_context::SharedContext::find_or_create_compute_pipeline`] (G11b); the shared flow
//! ([`ResourceProvider::find_or_create_keyed`]) serves the resource kinds.

use std::sync::LazyLock;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Budgeted, StdSteadyClockTimePoint};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer::Buffer;
use crate::graphite::graphite_resource_key::{GraphiteResourceKey, GraphiteResourceKeyBuilder};
use crate::graphite::proxy_cache::ProxyCache;
use crate::graphite::resource::{ResourceObject, ResourceRef};
use crate::graphite::resource_cache::{ResourceCache, ScratchResourceSet};
use crate::graphite::resource_types::{
    AccessPattern, BufferType, Ownership, ResourceType, SamplerDesc, Shareable,
};
use crate::graphite::sampler::Sampler;
use crate::graphite::texture::Texture;
use crate::graphite::texture_info::TextureInfo;

/// The backend half of a [`ResourceProvider`].
pub trait ResourceProviderBackend: Send {
    /// `caps()->maxTextureSize()`.
    fn max_texture_size(&self) -> i32;

    /// `caps()->buildKeyForTexture(dimensions, info, type, key)`.
    fn build_key_for_texture(
        &self,
        dimensions: ISize,
        info: &TextureInfo,
        ty: ResourceType,
        key: &mut GraphiteResourceKey,
    );

    /// `createTexture()`.
    fn create_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>>;

    /// `createBuffer()`.
    fn create_buffer(
        &mut self,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>>;

    /// `createSampler()`.
    fn create_sampler(&mut self, _sampler_desc: &SamplerDesc) -> Option<ResourceRef<Sampler>> {
        None
    }

    /// `onCreateWrappedTexture()`.
    fn on_create_wrapped_texture(
        &mut self,
        _texture: &BackendTexture,
        _label: &str,
    ) -> Option<ResourceRef<Texture>> {
        None
    }

    /// `onCreateBackendTexture()`: an invalid texture if it cannot be created.
    fn on_create_backend_texture(
        &mut self,
        _dimensions: ISize,
        _info: &TextureInfo,
    ) -> BackendTexture {
        BackendTexture::new()
    }

    /// `onDeleteBackendTexture()`.
    fn on_delete_backend_texture(&mut self, _texture: &BackendTexture) {}

    /// For downcasting to the concrete backend provider, which has members the base class does
    /// not (`DawnResourceProvider`'s bind group helpers, …). `None` by default.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    /// `onFreeGpuResources()`.
    fn on_free_gpu_resources(&mut self) {}

    /// `onPurgeResourcesNotUsedSince()`.
    fn on_purge_resources_not_used_since(
        &mut self,
        _purge_time: StdSteadyClockTimePoint,
        _quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
    }
}

static TEXTURE_RESOURCE_TYPE: LazyLock<ResourceType> =
    LazyLock::new(GraphiteResourceKey::generate_resource_type);
static SAMPLER_RESOURCE_TYPE: LazyLock<ResourceType> =
    LazyLock::new(GraphiteResourceKey::generate_resource_type);
static BUFFER_RESOURCE_TYPE: LazyLock<ResourceType> =
    LazyLock::new(GraphiteResourceKey::generate_resource_type);

/// Finds or creates GPU resources, through the resource cache it owns.
#[doc(alias = "skgpu::graphite::ResourceProvider")]
pub struct ResourceProvider {
    // Each ResourceProvider owns one local cache.
    resource_cache: ResourceCache,
    backend: Box<dyn ResourceProviderBackend>,
}

impl std::fmt::Debug for ResourceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceProvider")
            .field("resource_cache", &self.resource_cache)
            .finish_non_exhaustive()
    }
}

impl ResourceProvider {
    /// `ResourceProvider(sharedContext, singleOwner, recorderID, resourceBudget)`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L33-L38 (chrome/m156)
    #[must_use]
    pub fn new(
        backend: Box<dyn ResourceProviderBackend>,
        recorder_id: u32,
        resource_budget: usize,
    ) -> Self {
        Self {
            resource_cache: ResourceCache::new(recorder_id, resource_budget),
            backend,
        }
    }

    /// The backend half.
    pub fn backend(&mut self) -> &mut dyn ResourceProviderBackend {
        &mut *self.backend
    }

    /// `findOrCreateShareableTexture()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L60-L64 (chrome/m156)
    #[doc(alias = "findOrCreateShareableTexture")]
    pub fn find_or_create_shareable_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        self.find_or_create_texture(dimensions, info, label, Budgeted::Yes, Shareable::Yes, None)
    }

    /// `findOrCreateNonShareableTexture()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L66-L71 (chrome/m156)
    #[doc(alias = "findOrCreateNonShareableTexture")]
    pub fn find_or_create_non_shareable_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
        budgeted: Budgeted,
    ) -> Option<ResourceRef<Texture>> {
        self.find_or_create_texture(dimensions, info, label, budgeted, Shareable::No, None)
    }

    /// `findOrCreateScratchTexture()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L73-L80 (chrome/m156)
    #[doc(alias = "findOrCreateScratchTexture")]
    pub fn find_or_create_scratch_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
        unavailable: &ScratchResourceSet,
    ) -> Option<ResourceRef<Texture>> {
        self.find_or_create_texture(
            dimensions,
            info,
            label,
            Budgeted::Yes,
            Shareable::Scratch,
            Some(unavailable),
        )
    }

    // Port of: src/gpu/graphite/ResourceProvider.cpp#L82-L116 (chrome/m156)
    fn find_or_create_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
        budgeted: Budgeted,
        shareable: Shareable,
        unavailable: Option<&ScratchResourceSet>,
    ) -> Option<ResourceRef<Texture>> {
        // If the resource is shareable it should be budgeted since it shouldn't be backing any
        // client owned object.
        debug_assert!(shareable == Shareable::No || budgeted == Budgeted::Yes);
        debug_assert!(shareable != Shareable::Scratch || unavailable.is_some());

        if !info.is_valid() {
            // Checking for a valid TextureInfo here allows callers to consolidate error checking
            // for both TextureInfo and Texture creation to checking for a null returned Texture.
            return None;
        }

        let mut key = GraphiteResourceKey::new();
        self.backend
            .build_key_for_texture(dimensions, info, *TEXTURE_RESOURCE_TYPE, &mut key);

        self.find_or_create_keyed(&key, budgeted, shareable, label, unavailable, |backend| {
            backend.create_texture(dimensions, info, label)
        })
    }

    /// The flow `findOrCreateTexture`, `findOrCreateBuffer` and `findOrCreateCompatibleSampler`
    /// share: return a cached resource for `key` if one fits, else create one with `create` and
    /// insert it into the cache.
    ///
    /// # Panics
    /// If the cache returns a resource of another type for `key` (keys encode the resource type).
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L105-L115 (chrome/m156)
    pub fn find_or_create_keyed<T: ResourceObject>(
        &mut self,
        key: &GraphiteResourceKey,
        budgeted: Budgeted,
        shareable: Shareable,
        label: &str,
        unavailable: Option<&ScratchResourceSet>,
        create: impl FnOnce(&mut dyn ResourceProviderBackend) -> Option<ResourceRef<T>>,
    ) -> Option<ResourceRef<T>> {
        if let Some(resource) =
            self.resource_cache
                .find_and_ref_resource(key, budgeted, shareable, label, unavailable)
        {
            return Some(
                resource
                    .downcast::<T>()
                    .expect("resource keys encode their resource type"),
            );
        }

        let resource = create(&mut *self.backend)?;
        self.resource_cache
            .insert_resource(&resource, key, budgeted, shareable);
        Some(resource)
    }

    /// The key `findOrCreateCompatibleSampler` uses: the sampler description's words.
    ///
    /// # Panics
    /// Never: a sampler description has at most 3 words.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L125-L142 (chrome/m156)
    #[must_use]
    pub fn sampler_key(sampler_desc: &SamplerDesc) -> GraphiteResourceKey {
        let mut key = GraphiteResourceKey::new();
        {
            // The size of the returned span accurately captures the quantity of uint32s needed
            // whether the sampler is immutable or not.
            let sampler_data = sampler_desc.as_span();
            let count = u16::try_from(sampler_data.len()).expect("at most 3 words");
            let mut builder =
                GraphiteResourceKeyBuilder::new(&mut key, *SAMPLER_RESOURCE_TYPE, count);

            for (i, word) in sampler_data.iter().enumerate() {
                builder[i] = *word;
            }
        }
        key
    }

    /// The key `findOrCreateBuffer` uses: type and access pattern, then the size in 32-bit words.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L183-L210 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // (uint32_t) szKey, as in C++
    #[must_use]
    pub fn buffer_key(
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
    ) -> GraphiteResourceKey {
        let mut key = GraphiteResourceKey::new();
        {
            // For the key we need ((sizeof(size_t) + (sizeof(uint32_t) - 1)) / (sizeof(uint32_t))
            // uint32_t's for the size and one uint32_t for the rest.
            const SIZE_KEY_NUM32_DATA_CNT: u16 = std::mem::size_of::<usize>().div_ceil(4) as u16;
            const KEY_NUM32_DATA_CNT: u16 = SIZE_KEY_NUM32_DATA_CNT + 1;

            debug_assert!((ty as u32) < (1 << 4));
            debug_assert!((access_pattern as u32) < (1 << 2));

            let mut builder = GraphiteResourceKeyBuilder::new(
                &mut key,
                *BUFFER_RESOURCE_TYPE,
                KEY_NUM32_DATA_CNT,
            );
            builder[0] = (ty as u32) | ((access_pattern as u32) << 4);
            let mut sz_key = size as u64;
            for i in 0..usize::from(SIZE_KEY_NUM32_DATA_CNT) {
                builder[i + 1] = sz_key as u32;

                // If size_t is 4 bytes, we cannot do a shift of 32.
                if SIZE_KEY_NUM32_DATA_CNT > 1 {
                    sz_key >>= 32;
                }
            }
        }
        key
    }

    /// `createWrappedTexture()`: wraps the client's backend texture.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L118-L123 (chrome/m156)
    #[doc(alias = "createWrappedTexture")]
    pub fn create_wrapped_texture(
        &mut self,
        backend_texture: &BackendTexture,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        let texture = self
            .backend
            .on_create_wrapped_texture(backend_texture, label);
        debug_assert!(
            texture
                .as_ref()
                .is_none_or(|texture| texture.base().ownership() == Ownership::Wrapped)
        );
        texture
    }

    /// `findOrCreateCompatibleSampler()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L125-L156 (chrome/m156)
    #[doc(alias = "findOrCreateCompatibleSampler")]
    pub fn find_or_create_compatible_sampler(
        &mut self,
        sampler_desc: &SamplerDesc,
    ) -> Option<ResourceRef<Sampler>> {
        // The size of the returned span accurately captures the quantity of uint32s needed
        // whether the sampler is immutable or not. Each backend will already have encoded any
        // specific immutable sampler details into the SamplerDesc, so there is no need to
        // delegate to Caps to create a specific key.
        let key = Self::sampler_key(sampler_desc);
        self.find_or_create_keyed(&key, Budgeted::Yes, Shareable::Yes, "", None, |backend| {
            backend.create_sampler(sampler_desc)
        })
    }

    /// `createBackendTexture()`: creates a texture the client owns, or an invalid one if the
    /// dimensions are empty or too large or the backend cannot create it.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L212-L236 (chrome/m156)
    #[doc(alias = "createBackendTexture")]
    pub fn create_backend_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
    ) -> BackendTexture {
        let max_texture_size = self.backend.max_texture_size();
        if dimensions.width <= 0
            || dimensions.height <= 0
            || dimensions.width > max_texture_size
            || dimensions.height > max_texture_size
        {
            skia_log_w!(
                "Call to createBackendTexture has requested dimensions ({}, {}) larger than the \
                 supported gpu max texture size: {}. Or the dimensions are empty.",
                dimensions.width,
                dimensions.height,
                max_texture_size
            );
            return BackendTexture::new();
        }
        self.backend.on_create_backend_texture(dimensions, info)
    }

    /// `deleteBackendTexture()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L276-L284 (chrome/m156)
    #[doc(alias = "deleteBackendTexture")]
    pub fn delete_backend_texture(&mut self, texture: &BackendTexture) {
        self.backend.on_delete_backend_texture(texture);
    }

    /// `findOrCreateNonShareableBuffer()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L158-L164 (chrome/m156)
    #[doc(alias = "findOrCreateNonShareableBuffer")]
    pub fn find_or_create_non_shareable_buffer(
        &mut self,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        self.find_or_create_buffer(size, ty, access_pattern, label, Shareable::No, None)
    }

    /// `findOrCreateScratchBuffer()`: scratch buffers must be GPU only.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L166-L177 (chrome/m156)
    #[doc(alias = "findOrCreateScratchBuffer")]
    pub fn find_or_create_scratch_buffer(
        &mut self,
        size: usize,
        ty: BufferType,
        access: AccessPattern,
        label: &str,
        unavailable: &ScratchResourceSet,
    ) -> Option<ResourceRef<Buffer>> {
        // Scratch buffers must be GPU only, mapped access makes it too difficult to scope their
        // reads and writes within the actual command buffer execution.
        debug_assert_ne!(access, AccessPattern::HostVisible);
        self.find_or_create_buffer(
            size,
            ty,
            access,
            label,
            Shareable::Scratch,
            Some(unavailable),
        )
    }

    // Port of: src/gpu/graphite/ResourceProvider.cpp#L179-L226 (chrome/m156)
    fn find_or_create_buffer(
        &mut self,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
        shareable: Shareable,
        unavailable: Option<&ScratchResourceSet>,
    ) -> Option<ResourceRef<Buffer>> {
        let key = Self::buffer_key(size, ty, access_pattern);
        self.find_or_create_keyed(
            &key,
            Budgeted::Yes,
            shareable,
            label,
            unavailable,
            |backend| backend.create_buffer(size, ty, access_pattern, label),
        )
    }

    /// `proxyCache()`.
    #[doc(alias = "proxyCache")]
    pub fn proxy_cache(&mut self) -> Option<&mut ProxyCache> {
        self.resource_cache.proxy_cache()
    }

    /// `setResourceCacheLimit()`.
    #[doc(alias = "setResourceCacheLimit")]
    pub fn set_resource_cache_limit(&mut self, bytes: usize) {
        self.resource_cache.set_max_budget(bytes);
    }

    /// `getResourceCacheLimit()`.
    #[doc(alias = "getResourceCacheLimit")]
    #[must_use]
    pub fn get_resource_cache_limit(&self) -> usize {
        self.resource_cache.get_max_budget()
    }

    /// `getResourceCacheCurrentBudgetedBytes()`.
    #[doc(alias = "getResourceCacheCurrentBudgetedBytes")]
    #[must_use]
    pub fn get_resource_cache_current_budgeted_bytes(&self) -> usize {
        self.resource_cache.current_budgeted_bytes()
    }

    /// `getResourceCacheCurrentPurgeableBytes()`.
    #[doc(alias = "getResourceCacheCurrentPurgeableBytes")]
    #[must_use]
    pub fn get_resource_cache_current_purgeable_bytes(&self) -> usize {
        self.resource_cache.current_purgeable_bytes()
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L290-L298 (chrome/m156)
    #[doc(alias = "freeGpuResources")]
    pub fn free_gpu_resources(&mut self) {
        self.backend.on_free_gpu_resources();

        self.resource_cache.purge_resources();
    }

    /// `purgeResourcesNotUsedSince()`.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L300-L311 (chrome/m156)
    #[doc(alias = "purgeResourcesNotUsedSince")]
    pub fn purge_resources_not_used_since(
        &mut self,
        purge_time: StdSteadyClockTimePoint,
        max_purging_duration: Option<std::time::Duration>,
    ) {
        let quit_purging_time =
            max_purging_duration.map(|duration| StdSteadyClockTimePoint::now() + duration);

        self.backend
            .on_purge_resources_not_used_since(purge_time, quit_purging_time);
        self.resource_cache
            .purge_resources_not_used_since(purge_time, quit_purging_time);
    }

    /// `forceProcessReturnedResources()`.
    #[doc(alias = "forceProcessReturnedResources")]
    pub fn force_process_returned_resources(&mut self) {
        self.resource_cache.force_process_returned_resources();
    }

    /// `resourceCache()` (test utility).
    #[doc(alias = "resourceCache")]
    pub fn resource_cache(&mut self) -> &mut ResourceCache {
        &mut self.resource_cache
    }
}

impl Drop for ResourceProvider {
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L40-L42 (chrome/m156)
    fn drop(&mut self) {
        self.resource_cache.shutdown();
    }
}
