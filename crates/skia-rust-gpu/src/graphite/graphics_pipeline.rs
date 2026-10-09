// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipeline.h, GraphicsPipeline.cpp (the base class
// members; the backend half is `graphite::wgpu::graphics_pipeline`, G11b)

//! The seam for `skgpu::graphite::GraphicsPipeline`.
//!
//! Tasks only hand pipelines to visitors (`Task::visitPipelines`), and the
//! [`crate::graphite::global_cache::GlobalCache`] keeps them in its LRU. Both need the base
//! class's bookkeeping, which is [`GraphicsPipelineBase`] and is reached through
//! [`GraphicsPipeline::base`]. The backend pipeline
//! ([`crate::graphite::wgpu::graphics_pipeline::WgpuGraphicsPipeline`]) embeds one.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use bitflags::bitflags;

use crate::gpu::gpu_types::StdSteadyClockTimePoint;
use crate::graphite::draw_types::PipelineStageFlags;
use crate::graphite::resource_types::DstReadStrategy;
use crate::graphite::shader_info::ShaderInfo;

bitflags! {
    /// `PipelineCreationFlags`: how a pipeline is being requested.
    // Port of: src/gpu/graphite/GraphicsPipeline.h#L17-L21 (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PipelineCreationFlags: u8 {
        /// `kNone`.
        const NONE = 0;
        /// `kForPrecompilation`: the request comes from the precompile API.
        const FOR_PRECOMPILATION = 0b001;
    }
}

/// A graphics pipeline, as far as the task graph and the global cache are concerned.
// Port of: src/gpu/graphite/GraphicsPipeline.h (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipeline")]
pub trait GraphicsPipeline: Send + Sync + Debug {
    /// For downcasting to the concrete pipeline.
    fn as_any(&self) -> &dyn Any;

    /// The base class members: the label, the key hash, the epoch and use bookkeeping.
    fn base(&self) -> &GraphicsPipelineBase;

    /// `didAsyncCompilationFail()`: the failure message if compiling the pipeline on the
    /// executor failed. Pipelines compiled synchronously never fail this way.
    // Port of: src/gpu/graphite/GraphicsPipeline.h#L110-L110 (chrome/m156)
    #[doc(alias = "didAsyncCompilationFail")]
    fn did_async_compilation_fail(&self) -> Option<String> {
        None
    }

    /// `getLabel()`.
    #[doc(alias = "getLabel")]
    fn label(&self) -> &str {
        self.base().label()
    }

    /// `dstReadStrategy()`.
    #[doc(alias = "dstReadStrategy")]
    fn dst_read_strategy(&self) -> DstReadStrategy {
        self.base().pipeline_info().dst_read_strategy
    }

    /// `numFragTexturesAndSamplers()`.
    #[doc(alias = "numFragTexturesAndSamplers")]
    fn num_frag_textures_and_samplers(&self) -> i32 {
        self.base().pipeline_info().num_frag_textures_and_samplers
    }

    /// `hasCombinedUniforms()`.
    #[doc(alias = "hasCombinedUniforms")]
    fn has_combined_uniforms(&self) -> bool {
        self.base().pipeline_info().has_combined_uniforms
    }

    /// `usesStorageBuffer()`.
    #[doc(alias = "usesStorageBuffer")]
    fn uses_storage_buffer(&self) -> bool {
        self.base().pipeline_info().uses_storage_buffer()
    }

    /// `vsUsesStorage()`.
    #[doc(alias = "vsUsesStorage")]
    fn vs_uses_storage(&self) -> bool {
        self.base().pipeline_info().vs_uses_storage()
    }

    /// `fsUsesStorage()`.
    #[doc(alias = "fsUsesStorage")]
    fn fs_uses_storage(&self) -> bool {
        self.base().pipeline_info().fs_uses_storage()
    }

    /// `storageBufferStages()`.
    #[doc(alias = "storageBufferStages")]
    fn storage_buffer_stages(&self) -> PipelineStageFlags {
        self.base().pipeline_info().storage_buffer_stages
    }

    /// `fromPrecompile()`.
    #[doc(alias = "fromPrecompile")]
    // The name mirrors Skia's `fromPrecompile()`, which is a query, not a conversion.
    #[allow(clippy::wrong_self_convention)]
    fn from_precompile(&self) -> bool {
        self.base().from_precompile()
    }
}

/// The immutable part of `GraphicsPipeline::PipelineInfo` that comes from the `ShaderInfo`.
///
/// The `GPU_TEST_UTILS` copies of the `SkSL` and native shader text are kept as well, since the GPU
/// oracle's DM ran in that build. `SkShaderUtils::PrettyPrint` of the `SkSL` is not ported, so the
/// `SkSL` is stored as generated.
// Port of: src/gpu/graphite/GraphicsPipeline.h#L55-L96 (chrome/m156)
#[derive(Clone, Debug)]
pub struct PipelineInfo {
    /// `fDstReadStrategy`.
    pub dst_read_strategy: DstReadStrategy,
    /// `fNumFragTexturesAndSamplers`.
    pub num_frag_textures_and_samplers: i32,
    /// `fHasCombinedUniforms`.
    pub has_combined_uniforms: bool,
    /// `fStorageBufferStages`.
    pub storage_buffer_stages: PipelineStageFlags,
    /// `fSkSLVertexShader` (`GPU_TEST_UTILS`).
    pub sksl_vertex_shader: String,
    /// `fSkSLFragmentShader` (`GPU_TEST_UTILS`).
    pub sksl_fragment_shader: String,
    /// `fNativeVertexShader` (`GPU_TEST_UTILS`): filled in by the backend.
    pub native_vertex_shader: String,
    /// `fNativeFragmentShader` (`GPU_TEST_UTILS`): filled in by the backend.
    pub native_fragment_shader: String,
}

impl Default for PipelineInfo {
    /// `PipelineInfo() = default`: the member initializers.
    // Port of: src/gpu/graphite/GraphicsPipeline.h#L66-L69 (chrome/m156)
    fn default() -> Self {
        Self {
            dst_read_strategy: DstReadStrategy::NoneRequired,
            num_frag_textures_and_samplers: 0,
            has_combined_uniforms: false,
            storage_buffer_stages: PipelineStageFlags::NONE,
            sksl_vertex_shader: String::new(),
            sksl_fragment_shader: String::new(),
            native_vertex_shader: String::new(),
            native_fragment_shader: String::new(),
        }
    }
}

impl PipelineInfo {
    /// `PipelineInfo(shaderInfo, flags, uniqueKeyHash, compilationID)`, without the hash, ID and
    /// flags, which [`GraphicsPipelineBase::with_info`] takes.
    // Port of: src/gpu/graphite/GraphicsPipeline.cpp#L31-L46 (chrome/m156)
    #[must_use]
    pub fn from_shader_info(shader_info: &ShaderInfo) -> Self {
        Self {
            dst_read_strategy: shader_info.dst_read_strategy(),
            num_frag_textures_and_samplers: shader_info.num_fragment_textures_and_samplers(),
            has_combined_uniforms: shader_info.has_combined_uniforms(),
            storage_buffer_stages: shader_info.storage_buffer_stages(),
            sksl_vertex_shader: shader_info.vertex_sksl().to_owned(),
            sksl_fragment_shader: shader_info.fragment_sksl().to_owned(),
            native_vertex_shader: String::new(),
            native_fragment_shader: String::new(),
        }
    }

    /// `usesStorageBuffer()`.
    #[doc(alias = "usesStorageBuffer")]
    #[must_use]
    pub fn uses_storage_buffer(&self) -> bool {
        !self.storage_buffer_stages.is_empty()
    }

    /// `vsUsesStorage()`.
    #[doc(alias = "vsUsesStorage")]
    #[must_use]
    pub fn vs_uses_storage(&self) -> bool {
        self.storage_buffer_stages
            .contains(PipelineStageFlags::VERTEX_SHADER)
    }

    /// `fsUsesStorage()`.
    #[doc(alias = "fsUsesStorage")]
    #[must_use]
    pub fn fs_uses_storage(&self) -> bool {
        self.storage_buffer_stages
            .contains(PipelineStageFlags::FRAGMENT_SHADER)
    }
}

/// The members of `GraphicsPipeline` and its `PipelineInfo` that the cache reads and updates.
///
/// Skia keeps them in the pipeline object and updates the epoch and the use flag under the
/// `GlobalCache`'s spinlock. They are atomics and a mutex here, so the pipeline can be shared
/// through an `Arc`.
// Port of: src/gpu/graphite/GraphicsPipeline.h#L37-L110 (chrome/m156)
#[derive(Debug)]
pub struct GraphicsPipelineBase {
    /// `fLabel`.
    label: String,
    /// `fPipelineInfo`, the part that comes from the shader info.
    info: PipelineInfo,
    /// `fPipelineInfo.fUniqueKeyHash`.
    unique_key_hash: u32,
    /// `fPipelineInfo.fCompilationID`.
    compilation_id: u32,
    /// `fPipelineInfo.fFromPrecompile`.
    from_precompile: bool,
    /// `fPipelineInfo.fWasUsed`.
    was_used: AtomicBool,
    /// `fPipelineInfo.fEpoch`: the last epoch in which the pipeline was touched.
    epoch: AtomicU16,
    /// `Resource::lastAccessTime()`, which `updateAccessTime()` sets.
    last_access: Mutex<StdSteadyClockTimePoint>,
}

impl GraphicsPipelineBase {
    /// `GraphicsPipeline(sharedContext, pipelineInfo, label)`: the members start unused, in epoch
    /// zero, and accessed now.
    // Port of: src/gpu/graphite/GraphicsPipeline.cpp (chrome/m156)
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        unique_key_hash: u32,
        compilation_id: u32,
        from_precompile: bool,
    ) -> Self {
        Self::with_info(
            label,
            PipelineInfo::default(),
            unique_key_hash,
            compilation_id,
            from_precompile,
        )
    }

    /// `GraphicsPipeline(sharedContext, pipelineInfo, label)` with the shader-info part of the
    /// pipeline info.
    // Port of: src/gpu/graphite/GraphicsPipeline.cpp#L15-L23 (chrome/m156)
    #[must_use]
    pub fn with_info(
        label: impl Into<String>,
        info: PipelineInfo,
        unique_key_hash: u32,
        compilation_id: u32,
        from_precompile: bool,
    ) -> Self {
        Self {
            label: label.into(),
            info,
            unique_key_hash,
            compilation_id,
            from_precompile,
            was_used: AtomicBool::new(false),
            epoch: AtomicU16::new(0),
            last_access: Mutex::new(StdSteadyClockTimePoint::now()),
        }
    }

    /// `getLabel()`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// `getPipelineInfo()`: the shader-info part of the pipeline info (`dstReadStrategy()`,
    /// `numFragTexturesAndSamplers()`, `hasCombinedUniforms()`, the storage buffer stages).
    #[doc(alias = "getPipelineInfo")]
    #[must_use]
    pub fn pipeline_info(&self) -> &PipelineInfo {
        &self.info
    }

    /// `getPipelineInfo().fUniqueKeyHash`.
    #[must_use]
    pub fn unique_key_hash(&self) -> u32 {
        self.unique_key_hash
    }

    /// `getPipelineInfo().fCompilationID`.
    #[must_use]
    pub fn compilation_id(&self) -> u32 {
        self.compilation_id
    }

    /// `fromPrecompile()`.
    #[must_use]
    pub fn from_precompile(&self) -> bool {
        self.from_precompile
    }

    /// `markUsed()`.
    pub fn mark_used(&self) {
        self.was_used.store(true, Ordering::Relaxed);
    }

    /// `wasUsed()`.
    #[must_use]
    pub fn was_used(&self) -> bool {
        self.was_used.load(Ordering::Relaxed)
    }

    /// `markEpoch(epoch)`.
    pub fn mark_epoch(&self, epoch: u16) {
        self.epoch.store(epoch, Ordering::Relaxed);
    }

    /// `epoch()`.
    #[must_use]
    pub fn epoch(&self) -> u16 {
        self.epoch.load(Ordering::Relaxed)
    }

    /// `updateAccessTime()`.
    pub fn update_access_time(&self) {
        *self
            .last_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = StdSteadyClockTimePoint::now();
    }

    /// `lastAccessTime()`.
    #[must_use]
    pub fn last_access_time(&self) -> StdSteadyClockTimePoint {
        *self
            .last_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
