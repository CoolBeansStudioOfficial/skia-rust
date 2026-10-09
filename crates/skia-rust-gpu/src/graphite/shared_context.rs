// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/SharedContext.h, src/gpu/graphite/SharedContext.cpp

//! The backend-neutral half of `SharedContext`: the caps, the shader dictionary, the renderer
//! provider and the thread-safe resource provider. The wgpu half
//! ([`crate::graphite::wgpu::WgpuSharedContext`]) embeds it.
//!
//! It also owns the `GlobalCache` and the `PipelineManager`.

use std::sync::{Arc, OnceLock};

use skia_rust_core::executor::Executor;

use crate::gpu::gpu_types::{BackendApi, StdSteadyClockTimePoint};
use crate::gpu::resource_key::UniqueKey;
use crate::graphite::caps::Caps;
use crate::graphite::compute_pipeline::ComputePipeline;
use crate::graphite::context_options::PipelineCacheOp;
use crate::graphite::global_cache::GlobalCache;
use crate::graphite::graphics_pipeline::{GraphicsPipeline, PipelineCreationFlags};
use crate::graphite::pipeline_manager::PipelineManager;
use crate::graphite::renderer_provider::RendererProvider;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::thread_safe_resource_provider::ThreadSafeResourceProvider;

/// The state every backend shares, and that the `Context`, the recorders and the precompile
/// context keep alive through an `Arc`.
///
/// Skia sets the renderer provider and the thread-safe resource provider once, after the
/// shared context exists (`SharedContext::setRendererProvider`, and the backend constructor).
/// They are `OnceLock`s here so that the `Arc` can be shared before they are set.
// Port of: src/gpu/graphite/SharedContext.h#L35-L136 (chrome/m156)
#[doc(alias = "skgpu::graphite::SharedContext")]
#[derive(Debug)]
pub struct SharedContext {
    /// `fCaps`.
    caps: Arc<dyn Caps>,
    /// `fBackend`.
    backend: BackendApi,
    /// `fShaderDictionary`.
    shader_dictionary: ShaderCodeDictionary,
    /// `fRendererProvider`: set once by `Context::finishInitialization`.
    renderer_provider: OnceLock<RendererProvider>,
    /// `fThreadSafeResourceProvider`: set once by the backend constructor.
    thread_safe_resource_provider: OnceLock<ThreadSafeResourceProvider>,
    /// `fGlobalCache`: the pipelines, the dynamic samplers and the static resources.
    global_cache: GlobalCache,
    /// `fPipelineManager`: compiles the graphics pipelines, on the executor if there is one.
    pipeline_manager: PipelineManager,
}

impl SharedContext {
    /// `SharedContext(caps, backend, executor, userDefinedKnownRuntimeEffects)`. The renderer
    /// provider and the thread-safe resource provider are set later.
    // Port of: src/gpu/graphite/SharedContext.cpp#L34-L48 (chrome/m156)
    #[must_use]
    pub fn new(
        caps: Arc<dyn Caps>,
        backend: BackendApi,
        shader_dictionary: ShaderCodeDictionary,
        executor: Option<Arc<dyn Executor>>,
    ) -> Self {
        Self {
            caps,
            backend,
            shader_dictionary,
            renderer_provider: OnceLock::new(),
            thread_safe_resource_provider: OnceLock::new(),
            global_cache: GlobalCache::new(),
            pipeline_manager: PipelineManager::new(executor),
        }
    }

    /// `pipelineManager()`.
    // Port of: src/gpu/graphite/SharedContext.h#L112 (chrome/m156)
    #[doc(alias = "pipelineManager")]
    #[must_use]
    pub fn pipeline_manager(&self) -> &PipelineManager {
        &self.pipeline_manager
    }

    /// `globalCache()`.
    #[doc(alias = "globalCache")]
    #[must_use]
    pub fn global_cache(&self) -> &GlobalCache {
        &self.global_cache
    }

    /// `caps()`.
    #[must_use]
    pub fn caps(&self) -> &dyn Caps {
        &*self.caps
    }

    /// `caps()` as the shared handle.
    #[must_use]
    pub fn caps_arc(&self) -> &Arc<dyn Caps> {
        &self.caps
    }

    /// `backend()`.
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        self.backend
    }

    /// `shaderCodeDictionary()`.
    #[doc(alias = "shaderCodeDictionary")]
    #[must_use]
    pub fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_dictionary
    }

    /// `rendererProvider()`: `None` until `Context::finishInitialization` sets it.
    // Port of: src/gpu/graphite/SharedContext.h#L58 (chrome/m156)
    #[doc(alias = "rendererProvider")]
    #[must_use]
    pub fn renderer_provider(&self) -> Option<&RendererProvider> {
        self.renderer_provider.get()
    }

    /// `setRendererProvider(rendererProvider)`: may only be called once.
    ///
    /// # Panics
    /// If the renderer provider was already set.
    // Port of: src/gpu/graphite/SharedContext.cpp#L52-L56 (chrome/m156)
    #[doc(alias = "setRendererProvider")]
    pub fn set_renderer_provider(&self, renderer_provider: RendererProvider) {
        assert!(
            self.renderer_provider.set(renderer_provider).is_ok(),
            "the renderer provider is set once"
        );
    }

    /// `threadSafeResourceProvider()`: `None` until the backend sets it.
    #[doc(alias = "threadSafeResourceProvider")]
    #[must_use]
    pub fn thread_safe_resource_provider(&self) -> Option<&ThreadSafeResourceProvider> {
        self.thread_safe_resource_provider.get()
    }

    /// Sets the thread-safe resource provider, which the backend constructor does once.
    ///
    /// # Panics
    /// If it was already set.
    // Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L74-L76 (chrome/m156)
    pub fn set_thread_safe_resource_provider(&self, resource_provider: ResourceProvider) {
        assert!(
            self.thread_safe_resource_provider
                .set(ThreadSafeResourceProvider::new(resource_provider))
                .is_ok(),
            "the thread-safe resource provider is set once"
        );
    }

    /// `getResourceCacheLimit()` (debug builds only in Skia).
    // Port of: src/gpu/graphite/SharedContext.cpp#L136-L138 (chrome/m156)
    #[doc(alias = "getResourceCacheLimit")]
    #[must_use]
    pub fn resource_cache_limit(&self) -> Option<usize> {
        self.thread_safe_resource_provider
            .get()
            .map(ThreadSafeResourceProvider::get_resource_cache_limit)
    }

    /// `getResourceCacheCurrentBudgetedBytes()`.
    // Port of: src/gpu/graphite/SharedContext.cpp#L139-L141 (chrome/m156)
    #[doc(alias = "getResourceCacheCurrentBudgetedBytes")]
    #[must_use]
    pub fn resource_cache_current_budgeted_bytes(&self) -> Option<usize> {
        self.thread_safe_resource_provider
            .get()
            .map(ThreadSafeResourceProvider::get_resource_cache_current_budgeted_bytes)
    }

    /// `getResourceCacheCurrentPurgeableBytes()`.
    // Port of: src/gpu/graphite/SharedContext.cpp#L142-L144 (chrome/m156)
    #[doc(alias = "getResourceCacheCurrentPurgeableBytes")]
    #[must_use]
    pub fn resource_cache_current_purgeable_bytes(&self) -> Option<usize> {
        self.thread_safe_resource_provider
            .get()
            .map(ThreadSafeResourceProvider::get_resource_cache_current_purgeable_bytes)
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/SharedContext.cpp#L150-L152 (chrome/m156)
    #[doc(alias = "freeGpuResources")]
    pub fn free_gpu_resources(&self) {
        if let Some(provider) = self.thread_safe_resource_provider.get() {
            provider.free_gpu_resources();
        }
    }

    /// `purgeResourcesNotUsedSince(purgeTime)`.
    // Port of: src/gpu/graphite/SharedContext.cpp#L153-L155 (chrome/m156)
    #[doc(alias = "purgeResourcesNotUsedSince")]
    pub fn purge_resources_not_used_since(&self, purge_time: StdSteadyClockTimePoint) {
        if let Some(provider) = self.thread_safe_resource_provider.get() {
            provider.purge_resources_not_used_since(purge_time);
        }
    }

    /// `forceProcessReturnedResources()`.
    // Port of: src/gpu/graphite/SharedContext.cpp#L156-L158 (chrome/m156)
    #[doc(alias = "forceProcessReturnedResources")]
    pub fn force_process_returned_resources(&self) {
        if let Some(provider) = self.thread_safe_resource_provider.get() {
            provider.force_process_returned_resources();
        }
    }

    /// `findOrCreateGraphicsPipeline(runtimeDict, pipelineKey, pipelineDesc, renderPassDesc, flags)`
    /// over the global cache. A miss calls `create` with the compilation ID the cache assigned; the
    /// backend's pipeline creation (`createGraphicsPipeline`, G11b) is that closure. A pipeline it
    /// creates is added to the cache, which may return a pipeline another thread added first.
    ///
    /// The `kAddingPipeline` callback runs with no serialized key: `PipelineDescToData`
    /// (`SerializationUtils`, G14) is not ported, so the deprecated `PipelineCallback` never runs.
    // Port of: src/gpu/graphite/SharedContext.cpp#L73-L125 (chrome/m156)
    #[doc(alias = "findOrCreateGraphicsPipeline")]
    pub fn find_or_create_graphics_pipeline(
        &self,
        pipeline_key: &UniqueKey,
        flags: PipelineCreationFlags,
        create: impl FnOnce(u32) -> Option<Arc<dyn GraphicsPipeline>>,
    ) -> Option<Arc<dyn GraphicsPipeline>> {
        let mut compilation_id = 0;
        if let Some(pipeline) =
            self.global_cache
                .find_graphics_pipeline(pipeline_key, flags, Some(&mut compilation_id))
        {
            return Some(pipeline);
        }
        // Pipelines are shared across Recorders, so two threads can create equivalent pipelines.
        // The global cache returns the first one in, and this thread's copy is discarded.
        let pipeline = create(compilation_id)?;
        let (pipeline, added_to_cache) = self
            .global_cache
            .add_graphics_pipeline(pipeline_key, pipeline);
        if added_to_cache && self.global_cache.has_pipeline_callback() {
            self.global_cache.invoke_pipeline_callback(
                PipelineCacheOp::AddingPipeline,
                &*pipeline,
                None,
            );
        }
        Some(pipeline)
    }

    /// `findOrCreateComputePipeline(key)` over the global cache: a miss calls `create`, and the
    /// pipeline it returns is added unless another thread added one first. Skia keeps this on the
    /// `ResourceProvider`, which needs the `ComputeStep` (G13) to build the descriptor; the
    /// cache half is the same, so it is here.
    // Port of: src/gpu/graphite/ResourceProvider.cpp#L44-L60 (chrome/m156), the cache half
    #[doc(alias = "findOrCreateComputePipeline")]
    pub fn find_or_create_compute_pipeline(
        &self,
        key: &UniqueKey,
        create: impl FnOnce() -> Option<Arc<dyn ComputePipeline>>,
    ) -> Option<Arc<dyn ComputePipeline>> {
        if let Some(pipeline) = self.global_cache.find_compute_pipeline(key) {
            return Some(pipeline);
        }
        let pipeline = create()?;
        Some(self.global_cache.add_compute_pipeline(key, pipeline))
    }
}
