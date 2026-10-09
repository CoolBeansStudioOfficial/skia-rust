// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GlobalCache.h, src/gpu/graphite/GlobalCache.cpp (chrome/m156)

//! `GlobalCache`: the resources every `Recorder` of a `SharedContext` shares. The graphics and
//! compute pipeline caches (LRU, with the purge callbacks), the pipeline client callbacks, the
//! dynamic samplers, and the static resources the cache keeps alive.
//!
//! Skia guards the cache with a spinlock and splits lookups from insertions. Here one `Mutex`
//! guards the same state; the callbacks run outside it, as in Skia.
//!
//! Deviations, all in histograms and in the deprecated `SK_DEBUG` checks: the `Graphite.*`
//! histograms (`reportPrecompileStats`, `reportCacheStats`) are not recorded, and the
//! `isResourceTracked` debug query is not ported. Skia's `setPipelineCallback(context, ...)`
//! takes the client context as a `void*`; here the callbacks are closures that capture it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use skia_rust_core::data::Data;
use skia_rust_core::lru_cache::LruCache;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode};
use skia_rust_core::tile_mode::TileMode;

use crate::gpu::gpu_types::StdSteadyClockTimePoint;
use crate::gpu::resource_key::UniqueKey;
use crate::graphite::buffer::Buffer;
use crate::graphite::buffer_manager::StaticVertexCopyRanges;
use crate::graphite::caps::Caps;
use crate::graphite::compute_pipeline::ComputePipeline;
use crate::graphite::context_options::{
    Callback, PipelineCacheOp, PipelineCachingCallbackFn, PipelineCallbackFn,
};
use crate::graphite::graphics_pipeline::{GraphicsPipeline, PipelineCreationFlags};
use crate::graphite::resource::{AnyResourceRef, Resource, ResourceRef};
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::{ImmutableSamplerInfo, SamplerDesc};
use crate::graphite::sampler::Sampler;

/// `kGlobalGraphicsPipelineCacheSizeLimit` under `GPU_TEST_UTILS`, the build the GPU oracle's
/// DM ran in. The non-test value is 512.
// Port of: src/gpu/graphite/GlobalCache.cpp#L36-L43 (chrome/m156)
const GLOBAL_GRAPHICS_PIPELINE_CACHE_SIZE_LIMIT: usize = 1 << 13;
/// `kGlobalComputePipelineCacheSizeLimit`.
// Port of: src/gpu/graphite/GlobalCache.cpp#L36-L43 (chrome/m156)
const GLOBAL_COMPUTE_PIPELINE_CACHE_SIZE_LIMIT: usize = 256;

/// `kNumDynamicSamplers`: one slot for every dynamic `SamplerDesc` index.
// Port of: src/gpu/graphite/GlobalCache.h#L170 (chrome/m156)
const NUM_DYNAMIC_SAMPLERS: usize = 1 << SamplerDesc::IMMUTABLE_SAMPLER_INFO_SHIFT;

/// Source of `compilationID`s (`next_compilation_id()`). Skia does not worry about wrap-around,
/// since the IDs are only for debug logging.
// Port of: src/gpu/graphite/GlobalCache.cpp#L23-L28 (chrome/m156)
static NEXT_COMPILATION_ID: AtomicU32 = AtomicU32::new(0);

fn next_compilation_id() -> u32 {
    NEXT_COMPILATION_ID.fetch_add(1, Ordering::Relaxed)
}

/// `GlobalCache::PipelineStats`.
// Port of: src/gpu/graphite/GlobalCache.h#L77-L100 (chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineStats {
    /// `fGraphicsCacheHits`.
    pub graphics_cache_hits: u32,
    /// `fGraphicsCacheMisses`.
    pub graphics_cache_misses: u32,
    /// `fGraphicsCacheAdditions`.
    pub graphics_cache_additions: u32,
    /// `fGraphicsRaces`: pipelines that lost a creation race and were discarded.
    pub graphics_races: u32,
    /// `fGraphicsPurges`: pipelines removed by `purgePipelinesNotUsedSince`.
    pub graphics_purges: u32,
    /// `fNormalPreemptedByPrecompile`: normally compiled pipelines skipped because a precompiled
    /// pipeline was already cached.
    pub normal_preempted_by_precompile: u32,
    /// `fUnpreemptedPrecompilePipelines`: precompiled pipelines that made it into the cache.
    pub unpreempted_precompile_pipelines: u32,
    /// `fPurgedUnusedPrecompiledPipelines`: precompiled pipelines purged before any use.
    pub purged_unused_precompiled_pipelines: u32,
    /// `fPipelineUsesInEpoch`: the number of pipelines requested since the last epoch report.
    pub pipeline_uses_in_epoch: u32,
}

/// The purge callback of the graphics pipeline cache (`GlobalCache::PurgeCB`, `LogPurge`).
// Port of: src/gpu/graphite/GlobalCache.cpp#L172-L188 (chrome/m156)
fn log_purge(
    stats: &mut PipelineStats,
    _key: &UniqueKey,
    pipeline: &mut Arc<dyn GraphicsPipeline>,
) {
    if pipeline.from_precompile() && !pipeline.base().was_used() {
        stats.purged_unused_precompiled_pipelines += 1;
    }
}

/// The client callbacks, set once by the `Context` (`setPipelineCallback`).
#[derive(Debug, Default)]
struct PipelineCallbacks {
    /// `fPipelineCachingCallback`.
    caching: Option<Callback<PipelineCachingCallbackFn>>,
    /// `fDeprecatedPipelineCallback`.
    deprecated: Option<Callback<PipelineCallbackFn>>,
}

/// The state the `GlobalCache`'s lock guards.
#[derive(Debug)]
struct Inner {
    /// `fGraphicsPipelineCache`. The statistics are its purge context.
    graphics_pipelines: LruCache<UniqueKey, Arc<dyn GraphicsPipeline>, PipelineStats>,
    /// `fComputePipelineCache`.
    compute_pipelines: LruCache<UniqueKey, Arc<dyn ComputePipeline>>,
    /// `fStaticResource`: kept alive for the lifetime of the cache.
    static_resources: Vec<AnyResourceRef>,
    /// `fEpochCounter`.
    epoch_counter: u16,
    /// `fDynamicSamplers`, indexed by `SamplerDesc::desc()`. The samplers are also in
    /// `static_resources`.
    dynamic_samplers: Vec<Option<ResourceRef<Sampler>>>,
    /// `fStaticVertexInfo` (`GPU_TEST_UTILS`).
    static_vertex_info: Vec<StaticVertexCopyRanges>,
    /// `fStaticVertexBuffer` (`GPU_TEST_UTILS`).
    static_vertex_buffer: Option<Arc<Resource<Buffer>>>,
}

/// `skgpu::graphite::GlobalCache`.
// Port of: src/gpu/graphite/GlobalCache.h#L31-L226 (chrome/m156)
#[doc(alias = "skgpu::graphite::GlobalCache")]
#[derive(Debug)]
pub struct GlobalCache {
    inner: Mutex<Inner>,
    /// Set once, by `setPipelineCallback`.
    callbacks: OnceLock<PipelineCallbacks>,
}

impl Default for GlobalCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalCache {
    /// `GlobalCache()`.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L56-L61 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                graphics_pipelines: LruCache::with_purge(
                    GLOBAL_GRAPHICS_PIPELINE_CACHE_SIZE_LIMIT,
                    PipelineStats::default(),
                    Some(log_purge),
                ),
                compute_pipelines: LruCache::new(GLOBAL_COMPUTE_PIPELINE_CACHE_SIZE_LIMIT),
                static_resources: Vec::new(),
                epoch_counter: 1,
                dynamic_samplers: vec![None; NUM_DYNAMIC_SAMPLERS],
                static_vertex_info: Vec::new(),
                static_vertex_buffer: None,
            }),
            callbacks: OnceLock::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `deleteResources()`: drops the dynamic samplers, the pipelines (without purge callbacks,
    /// as `reset()` does) and the static resources.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L93-L102 (chrome/m156)
    #[doc(alias = "deleteResources")]
    pub fn delete_resources(&self) {
        let mut inner = self.lock();
        inner.dynamic_samplers.iter_mut().for_each(|s| *s = None);
        inner.graphics_pipelines.reset();
        inner.compute_pipelines.reset();
        inner.static_resources.clear();
    }

    /// `setPipelineCallback(context, callback, deprecatedCallback)`. Only the first call has an
    /// effect; Skia asserts that it is the only one.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L63-L71 (chrome/m156)
    #[doc(alias = "setPipelineCallback")]
    pub fn set_pipeline_callback(
        &self,
        caching: Option<Callback<PipelineCachingCallbackFn>>,
        deprecated: Option<Callback<PipelineCallbackFn>>,
    ) {
        let set = self.callbacks.set(PipelineCallbacks {
            caching,
            deprecated,
        });
        debug_assert!(set.is_ok(), "the pipeline callbacks are set once");
    }

    /// `hasPipelineCallback()`.
    #[doc(alias = "hasPipelineCallback")]
    #[must_use]
    pub fn has_pipeline_callback(&self) -> bool {
        self.callbacks
            .get()
            .is_some_and(|c| c.caching.is_some() || c.deprecated.is_some())
    }

    /// `invokePipelineCallback(op, pipeline, serializedKey)`. The caching callback preempts the
    /// deprecated one, which only runs for an added pipeline with a serialized key.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L73-L89 (chrome/m156)
    #[doc(alias = "invokePipelineCallback")]
    pub fn invoke_pipeline_callback(
        &self,
        op: PipelineCacheOp,
        pipeline: &dyn GraphicsPipeline,
        serialized_key: Option<&Data>,
    ) {
        let Some(callbacks) = self.callbacks.get() else {
            return;
        };
        if let Some(caching) = &callbacks.caching {
            (caching.0)(
                op,
                pipeline.label(),
                pipeline.base().unique_key_hash(),
                pipeline.from_precompile(),
                serialized_key,
            );
        } else if let (Some(deprecated), Some(key)) = (&callbacks.deprecated, serialized_key) {
            (deprecated.0)(key);
        }
    }

    /// `findGraphicsPipeline(key, flags, compilationID)`: a cache hit marks the pipeline used
    /// (unless the request is for precompilation), and a miss writes the new `compilationID`
    /// into `compilation_id`, when given.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L190-L266 (chrome/m156)
    #[doc(alias = "findGraphicsPipeline")]
    pub fn find_graphics_pipeline(
        &self,
        key: &UniqueKey,
        flags: PipelineCreationFlags,
        compilation_id: Option<&mut u32>,
    ) -> Option<Arc<dyn GraphicsPipeline>> {
        let for_precompile = flags.contains(PipelineCreationFlags::FOR_PRECOMPILATION);
        let result = {
            let mut inner = self.lock();
            let epoch_counter = inner.epoch_counter;
            if let Some(found) = inner.graphics_pipelines.find(key) {
                let result = found.clone();
                if result.did_async_compilation_fail().is_some() {
                    // If the pipeline failed, remove it from the cache and let it be regenerated.
                    Self::remove_graphics_pipeline(&mut inner.graphics_pipelines, &result);
                    return None;
                }
                let stats = inner.graphics_pipelines.context_mut();
                stats.graphics_cache_hits += 1;
                if result.base().epoch() != epoch_counter {
                    // Update the epoch due to use in a new epoch.
                    result.base().mark_epoch(epoch_counter);
                    stats.pipeline_uses_in_epoch += 1;
                }
                if !for_precompile && result.from_precompile() && !result.base().was_used() {
                    stats.normal_preempted_by_precompile += 1;
                }
                // A precompile request that hits the cache does not count as a use.
                if !for_precompile {
                    result.base().update_access_time();
                    result.base().mark_used();
                }
                Some(result)
            } else {
                inner.graphics_pipelines.context_mut().graphics_cache_misses += 1;
                if let Some(id) = compilation_id {
                    // A miss means the next step creates a pipeline, which takes this ID.
                    *id = next_compilation_id();
                }
                None
            }
        };
        if let Some(pipeline) = &result {
            self.invoke_pipeline_callback(PipelineCacheOp::PipelineFound, &**pipeline, None);
        }
        result
    }

    /// `addGraphicsPipeline(key, pipeline)`: associates the pipeline with the key. If a pipeline
    /// is already associated with it, that one is returned with `false`, and `pipeline` is
    /// discarded.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L268-L321 (chrome/m156)
    #[doc(alias = "addGraphicsPipeline")]
    pub fn add_graphics_pipeline(
        &self,
        key: &UniqueKey,
        pipeline: Arc<dyn GraphicsPipeline>,
    ) -> (Arc<dyn GraphicsPipeline>, bool) {
        let mut inner = self.lock();
        let epoch_counter = inner.epoch_counter;
        if let Some(existing) = inner.graphics_pipelines.find(key) {
            // Another thread won the race creating this pipeline, so return the winner.
            let winner = existing.clone();
            inner.graphics_pipelines.context_mut().graphics_races += 1;
            return (winner, false);
        }
        // No equivalent pipeline was stored between `findGraphicsPipeline` returning null and
        // this call.
        let stored = inner
            .graphics_pipelines
            .insert(key.clone(), pipeline)
            .clone();
        let from_precompile = stored.from_precompile();
        let stats = inner.graphics_pipelines.context_mut();
        stats.graphics_cache_additions += 1;
        debug_assert_eq!(stored.base().epoch(), 0);
        // Mark with the epoch in which the pipeline was created.
        stored.base().mark_epoch(epoch_counter);
        stats.pipeline_uses_in_epoch += 1;
        if from_precompile {
            stats.unpreempted_precompile_pipelines += 1;
        } else {
            // Precompiled pipelines are marked used only on a cache hit in
            // `findGraphicsPipeline`.
            stored.base().update_access_time();
            stored.base().mark_used();
        }
        (stored, true)
    }

    /// `removeGraphicsPipeline(pipeline)`: removes the entries holding this pipeline object (by
    /// identity, not by key). Called only for pipelines whose compilation failed.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L323-L340 (chrome/m156)
    fn remove_graphics_pipeline(
        cache: &mut LruCache<UniqueKey, Arc<dyn GraphicsPipeline>, PipelineStats>,
        pipeline: &Arc<dyn GraphicsPipeline>,
    ) {
        let mut to_remove = Vec::new();
        cache.foreach(|key, cached| {
            if Arc::ptr_eq(cached, pipeline) {
                to_remove.push(key.clone());
            }
        });
        debug_assert!(to_remove.len() <= 1, "a pipeline has at most one key");
        for key in &to_remove {
            cache.remove(key);
        }
    }

    /// `purgePipelinesNotUsedSince(purgeTime)`. Compute pipelines are not purged (Skia TODO
    /// b/389073204).
    // Port of: src/gpu/graphite/GlobalCache.cpp#L342-L363 (chrome/m156)
    #[doc(alias = "purgePipelinesNotUsedSince")]
    pub fn purge_pipelines_not_used_since(&self, purge_time: StdSteadyClockTimePoint) {
        let mut inner = self.lock();
        let mut to_remove = Vec::new();
        inner.graphics_pipelines.foreach(|key, pipeline| {
            if pipeline.base().last_access_time() < purge_time {
                to_remove.push(key.clone());
            }
        });
        for key in &to_remove {
            inner.graphics_pipelines.context_mut().graphics_purges += 1;
            inner.graphics_pipelines.remove(key);
        }
    }

    /// `reportCacheStats()`: starts a new epoch. The `Graphite.PipelineCache.*` histogram is not
    /// recorded.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L380-L397 (chrome/m156)
    #[doc(alias = "reportCacheStats")]
    pub fn report_cache_stats(&self) {
        let mut inner = self.lock();
        inner
            .graphics_pipelines
            .context_mut()
            .pipeline_uses_in_epoch = 0;
        inner.epoch_counter = inner.epoch_counter.wrapping_add(1);
        if inner.epoch_counter == 0 {
            // The epoch counter has wrapped around: reset the marks and the counter.
            inner
                .graphics_pipelines
                .foreach(|_, pipeline| pipeline.base().mark_epoch(0));
            inner.epoch_counter = 1;
        }
    }

    /// `getStats()`.
    #[doc(alias = "getStats")]
    #[must_use]
    pub fn stats(&self) -> PipelineStats {
        *self.lock().graphics_pipelines.context()
    }

    /// `numGraphicsPipelines()`.
    #[must_use]
    pub fn num_graphics_pipelines(&self) -> usize {
        self.lock().graphics_pipelines.count()
    }

    /// `resetGraphicsPipelines()`.
    pub fn reset_graphics_pipelines(&self) {
        self.lock().graphics_pipelines.reset();
    }

    /// `forEachGraphicsPipeline(fn)`, from the most to the least recently used.
    pub fn for_each_graphics_pipeline(&self, mut f: impl FnMut(&UniqueKey, &dyn GraphicsPipeline)) {
        self.lock()
            .graphics_pipelines
            .foreach(|key, pipeline| f(key, &**pipeline));
    }

    /// `getEpoch()`.
    #[must_use]
    pub fn epoch(&self) -> u16 {
        self.lock().epoch_counter
    }

    /// `forceNextEpochOverflow()`: the next `reportCacheStats` wraps the epoch counter.
    pub fn force_next_epoch_overflow(&self) {
        self.lock().epoch_counter = u16::MAX;
    }

    /// `findComputePipeline(key)`.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L503-L507 (chrome/m156)
    #[doc(alias = "findComputePipeline")]
    pub fn find_compute_pipeline(&self, key: &UniqueKey) -> Option<Arc<dyn ComputePipeline>> {
        self.lock().compute_pipelines.find(key).cloned()
    }

    /// `addComputePipeline(key, pipeline)`: returns the pipeline already associated with the key,
    /// or `pipeline` if there is none.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L509-L517 (chrome/m156)
    #[doc(alias = "addComputePipeline")]
    pub fn add_compute_pipeline(
        &self,
        key: &UniqueKey,
        pipeline: Arc<dyn ComputePipeline>,
    ) -> Arc<dyn ComputePipeline> {
        let mut inner = self.lock();
        if let Some(existing) = inner.compute_pipelines.find(key) {
            return existing.clone();
        }
        inner
            .compute_pipelines
            .insert(key.clone(), pipeline)
            .clone()
    }

    /// `addStaticResource(resource)`: the cache keeps the resource alive until it is deleted.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L519-L522 (chrome/m156)
    #[doc(alias = "addStaticResource")]
    pub fn add_static_resource(&self, resource: AnyResourceRef) {
        self.lock().static_resources.push(resource);
    }

    /// `getDynamicSampler(desc)`: the sampler for a dynamic `SamplerDesc`, which
    /// [`GlobalCache::initialize_dynamic_samplers`] created. `None` if the description is
    /// immutable or was not created.
    // Port of: src/gpu/graphite/GlobalCache.h#L129-L136 (chrome/m156)
    #[doc(alias = "getDynamicSampler")]
    #[must_use]
    pub fn dynamic_sampler(&self, desc: SamplerDesc) -> Option<ResourceRef<Sampler>> {
        if desc.is_immutable() {
            return None;
        }
        let index = desc.desc() as usize;
        self.lock().dynamic_samplers.get(index).cloned().flatten()
    }

    /// `initializeDynamicSamplers(resourceProvider, caps)`: creates one sampler for every dynamic
    /// combination of sampling options and tile modes. Decal is skipped without clamp-to-border.
    /// Returns `false` if a sampler cannot be created.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L110-L154 (chrome/m156)
    #[doc(alias = "initializeDynamicSamplers")]
    pub fn initialize_dynamic_samplers(
        &self,
        resource_provider: &mut ResourceProvider,
        caps: &dyn Caps,
    ) -> bool {
        const TILE_MODES: [TileMode; 4] = [
            TileMode::Clamp,
            TileMode::Repeat,
            TileMode::Mirror,
            TileMode::Decal,
        ];
        // Manually unrolled: the sampling options that can be dynamic samplers. Cubic sampling
        // options do not contribute.
        // TODO: Support anisotropic filters.
        const SAMPLING_OPTIONS: [(FilterMode, MipmapMode); 6] = [
            (FilterMode::Nearest, MipmapMode::None),
            (FilterMode::Linear, MipmapMode::None),
            (FilterMode::Nearest, MipmapMode::Nearest),
            (FilterMode::Linear, MipmapMode::Nearest),
            (FilterMode::Nearest, MipmapMode::Linear),
            (FilterMode::Linear, MipmapMode::Linear),
        ];

        let mut inner = self.lock();
        debug_assert!(
            inner.dynamic_samplers[0].is_none(),
            "the dynamic samplers are initialized once"
        );
        let supports_clamp_to_border = caps.clamp_to_border_support();
        for (filter, mipmap) in SAMPLING_OPTIONS {
            let sampling_options = SamplingOptions::new(filter, mipmap);
            for tile_x in TILE_MODES {
                for tile_y in TILE_MODES {
                    if !supports_clamp_to_border
                        && (tile_x == TileMode::Decal || tile_y == TileMode::Decal)
                    {
                        continue;
                    }
                    let dynamic_desc = SamplerDesc::new_with_tile_modes(
                        &sampling_options,
                        (tile_x, tile_y),
                        ImmutableSamplerInfo::default(),
                    );
                    debug_assert!(
                        !dynamic_desc.is_immutable() && dynamic_desc.as_span().len() == 1
                    );
                    let Some(sampler) =
                        resource_provider.find_or_create_compatible_sampler(&dynamic_desc)
                    else {
                        return false;
                    };
                    // The lock is held, so add directly to the static resources.
                    inner.static_resources.push(sampler.to_any());
                    let index = dynamic_desc.desc() as usize;
                    inner.dynamic_samplers[index] = Some(sampler);
                }
            }
        }
        true
    }

    /// `testingOnly_SetStaticVertexInfo(ranges, buffer)`.
    // Port of: src/gpu/graphite/GlobalCache.cpp#L553-L558 (chrome/m156)
    pub fn testing_only_set_static_vertex_info(
        &self,
        ranges: Vec<StaticVertexCopyRanges>,
        buffer: Option<Arc<Resource<Buffer>>>,
    ) {
        let mut inner = self.lock();
        inner.static_vertex_info = ranges;
        inner.static_vertex_buffer = buffer;
    }

    /// `getStaticVertexCopyRanges()`.
    #[must_use]
    pub fn static_vertex_copy_ranges(&self) -> Vec<StaticVertexCopyRanges> {
        self.lock().static_vertex_info.clone()
    }

    /// `getStaticVertexBuffer()`.
    #[must_use]
    pub fn static_vertex_buffer(&self) -> Option<Arc<Resource<Buffer>>> {
        self.lock().static_vertex_buffer.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::resource_key::UniqueKeyBuilder;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    /// A pipeline whose failure and label the test controls.
    #[derive(Debug)]
    struct FakePipeline {
        base: crate::graphite::graphics_pipeline::GraphicsPipelineBase,
        failure: Option<String>,
    }

    impl FakePipeline {
        fn arc(label: &str, hash: u32, from_precompile: bool) -> Arc<dyn GraphicsPipeline> {
            Arc::new(Self {
                base: crate::graphite::graphics_pipeline::GraphicsPipelineBase::new(
                    label,
                    hash,
                    0,
                    from_precompile,
                ),
                failure: None,
            })
        }

        fn failing(label: &str, hash: u32) -> Arc<dyn GraphicsPipeline> {
            Arc::new(Self {
                base: crate::graphite::graphics_pipeline::GraphicsPipelineBase::new(
                    label, hash, 0, false,
                ),
                failure: Some("compile failed".to_string()),
            })
        }
    }

    impl GraphicsPipeline for FakePipeline {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn base(&self) -> &crate::graphite::graphics_pipeline::GraphicsPipelineBase {
            &self.base
        }

        fn did_async_compilation_fail(&self) -> Option<String> {
            self.failure.clone()
        }
    }

    /// A fresh unique key. Each key gets its own domain, so keys never collide; the argument only
    /// names the test case.
    fn key(_test_case: u32) -> UniqueKey {
        let mut key = UniqueKey::new();
        {
            let _builder = UniqueKeyBuilder::new(&mut key, UniqueKey::generate_domain(), 0, None);
        }
        key
    }

    fn purge_order(cache: &GlobalCache) -> Vec<u32> {
        let mut hashes = Vec::new();
        cache.for_each_graphics_pipeline(|_, p| hashes.push(p.base().unique_key_hash()));
        hashes
    }

    #[test]
    fn add_then_find_hits_and_marks_used() {
        let cache = GlobalCache::new();
        let k = key(1);
        let (stored, added) = cache.add_graphics_pipeline(&k, FakePipeline::arc("a", 1, false));
        assert!(added);
        assert!(stored.base().was_used());

        let found = cache.find_graphics_pipeline(&k, PipelineCreationFlags::NONE, None);
        assert!(found.is_some_and(|p| p.label() == "a"));
        let stats = cache.stats();
        assert_eq!(stats.graphics_cache_additions, 1);
        assert_eq!(stats.graphics_cache_hits, 1);
        assert_eq!(stats.graphics_cache_misses, 0);
    }

    #[test]
    fn a_miss_writes_a_compilation_id() {
        let cache = GlobalCache::new();
        let mut first = 0;
        let mut second = 0;
        assert!(
            cache
                .find_graphics_pipeline(&key(2), PipelineCreationFlags::NONE, Some(&mut first))
                .is_none()
        );
        assert!(
            cache
                .find_graphics_pipeline(&key(3), PipelineCreationFlags::NONE, Some(&mut second))
                .is_none()
        );
        assert!(second > first);
        assert_eq!(cache.stats().graphics_cache_misses, 2);
    }

    #[test]
    fn a_race_returns_the_winner() {
        let cache = GlobalCache::new();
        let k = key(4);
        let (winner, added) = cache.add_graphics_pipeline(&k, FakePipeline::arc("first", 4, false));
        assert!(added);
        let (loser, added) = cache.add_graphics_pipeline(&k, FakePipeline::arc("second", 4, false));
        assert!(!added);
        assert!(Arc::ptr_eq(&winner, &loser));
        assert_eq!(cache.stats().graphics_races, 1);
    }

    #[test]
    fn precompile_lookups_do_not_mark_used() {
        let cache = GlobalCache::new();
        let k = key(5);
        cache.add_graphics_pipeline(&k, FakePipeline::arc("p", 5, true));
        let _ = cache.find_graphics_pipeline(&k, PipelineCreationFlags::FOR_PRECOMPILATION, None);
        let mut stats = cache.stats();
        assert_eq!(stats.normal_preempted_by_precompile, 0);
        let _ = cache.find_graphics_pipeline(&k, PipelineCreationFlags::NONE, None);
        stats = cache.stats();
        assert_eq!(stats.normal_preempted_by_precompile, 1);
    }

    #[test]
    fn failed_pipelines_are_removed_on_lookup() {
        let cache = GlobalCache::new();
        let k = key(6);
        cache.add_graphics_pipeline(&k, FakePipeline::failing("bad", 6));
        assert_eq!(cache.num_graphics_pipelines(), 1);
        assert!(
            cache
                .find_graphics_pipeline(&k, PipelineCreationFlags::NONE, None)
                .is_none()
        );
        assert_eq!(cache.num_graphics_pipelines(), 0);
    }

    #[test]
    fn lru_eviction_purges_unused_precompiled_pipelines() {
        let cache = GlobalCache::new();
        // Fill past the limit with precompiled pipelines that are never used; the oldest are
        // evicted and counted as purged-unused.
        let total = GLOBAL_GRAPHICS_PIPELINE_CACHE_SIZE_LIMIT as u32 + 3;
        for n in 0..total {
            cache.add_graphics_pipeline(&key(n), FakePipeline::arc("pre", n, true));
        }
        assert_eq!(
            cache.num_graphics_pipelines(),
            GLOBAL_GRAPHICS_PIPELINE_CACHE_SIZE_LIMIT
        );
        assert_eq!(cache.stats().purged_unused_precompiled_pipelines, 3);
        // The most recently added survive, most recent first.
        let hashes = purge_order(&cache);
        assert_eq!(hashes[0], total - 1);
        assert_eq!(*hashes.last().unwrap(), 3);
    }

    #[test]
    fn purge_removes_pipelines_older_than_the_time_point() {
        let cache = GlobalCache::new();
        cache.add_graphics_pipeline(&key(10), FakePipeline::arc("old", 10, false));
        let cutoff: StdSteadyClockTimePoint = StdSteadyClockTimePoint::now();
        std::thread::sleep(Duration::from_millis(2));
        cache.add_graphics_pipeline(&key(11), FakePipeline::arc("new", 11, false));
        cache.purge_pipelines_not_used_since(cutoff);
        assert_eq!(purge_order(&cache), vec![11]);
        assert_eq!(cache.stats().graphics_purges, 1);
    }

    #[test]
    fn epochs_advance_and_wrap_to_one() {
        let cache = GlobalCache::new();
        assert_eq!(cache.epoch(), 1);
        cache.add_graphics_pipeline(&key(20), FakePipeline::arc("e", 20, false));
        assert_eq!(cache.stats().pipeline_uses_in_epoch, 1);
        cache.report_cache_stats();
        assert_eq!(cache.epoch(), 2);
        assert_eq!(cache.stats().pipeline_uses_in_epoch, 0);
        cache.force_next_epoch_overflow();
        cache.report_cache_stats();
        assert_eq!(cache.epoch(), 1);
    }

    #[test]
    fn callbacks_see_found_and_added_pipelines() {
        let seen: Arc<StdMutex<Vec<(PipelineCacheOp, String, u32, bool)>>> =
            Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let caching: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |op: PipelineCacheOp, label: &str, hash: u32, pre: bool, key: Option<&Data>| {
                assert!(key.is_none());
                sink.lock()
                    .unwrap()
                    .push((op, label.to_string(), hash, pre));
            },
        );
        let cache = GlobalCache::new();
        cache.set_pipeline_callback(Some(Callback(caching)), None);
        assert!(cache.has_pipeline_callback());

        let k = key(30);
        cache.add_graphics_pipeline(&k, FakePipeline::arc("cb", 30, true));
        let added = cache
            .add_graphics_pipeline(&k, FakePipeline::arc("cb", 30, true))
            .0;
        cache.invoke_pipeline_callback(PipelineCacheOp::AddingPipeline, &*added, None);
        let _ = cache.find_graphics_pipeline(&k, PipelineCreationFlags::NONE, None);

        let log = seen.lock().unwrap();
        assert_eq!(
            *log,
            vec![
                (PipelineCacheOp::AddingPipeline, "cb".to_string(), 30, true),
                (PipelineCacheOp::PipelineFound, "cb".to_string(), 30, true),
            ]
        );
    }

    #[test]
    fn deprecated_callback_needs_a_serialized_key_and_no_caching_callback() {
        let calls = Arc::new(StdMutex::new(0usize));
        let counter = Arc::clone(&calls);
        let deprecated: Arc<PipelineCallbackFn> = Arc::new(move |_data: &Data| {
            *counter.lock().unwrap() += 1;
        });
        let cache = GlobalCache::new();
        cache.set_pipeline_callback(None, Some(Callback(deprecated)));
        let pipeline = FakePipeline::arc("d", 40, false);
        cache.invoke_pipeline_callback(PipelineCacheOp::AddingPipeline, &*pipeline, None);
        assert_eq!(*calls.lock().unwrap(), 0);
        cache.invoke_pipeline_callback(
            PipelineCacheOp::AddingPipeline,
            &*pipeline,
            Some(&Data::new_copy(b"key")),
        );
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn delete_resources_empties_the_caches() {
        let cache = GlobalCache::new();
        cache.add_graphics_pipeline(&key(50), FakePipeline::arc("x", 50, false));
        cache.delete_resources();
        assert_eq!(cache.num_graphics_pipelines(), 0);
        assert!(cache.static_vertex_buffer().is_none());
    }
}
