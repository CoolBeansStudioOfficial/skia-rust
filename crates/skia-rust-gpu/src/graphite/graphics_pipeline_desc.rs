// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipelineDesc.h, src/gpu/graphite/GraphicsPipelineHandle.h,
//                   src/gpu/graphite/PipelineData.h (GraphicsPipelineCache)

//! [`GraphicsPipelineDesc`], the cache that de-duplicates them for a draw pass, and the
//! [`GraphicsPipelineHandle`] a draw pass holds for each of them.
//!
//! The handle and the [`PipelineHandleFactory`] are the seam to `PipelineManager` and
//! `PipelineCreationTask` (G9b, `docs/design/gpu.md` §5.4): the manager implements the factory
//! and the creation task implements [`PipelineCreation`]. A handle is resolved to a pipeline
//! after `DrawPass::addResourceRefs()`.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use crate::graphite::graphics_pipeline::{GraphicsPipeline, PipelineCreationFlags};
use crate::graphite::pipeline_data::DenseBiMap;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::RenderStepID;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// Describes the geometric portion of a pipeline's program and the pipeline's fixed state, plus
/// the paint it shades with.
///
/// Each `RenderStep` defines a fixed set of attributes and rasterization state, as well as the
/// shader fragments that control the geometry and coverage calculations. The `RenderStep`'s
/// shader is combined with the rest of the shader generated from the `PaintParams`. Because each
/// `RenderStep` is fixed, its ID can be used as a proxy for everything that it specifies in the
/// `GraphicsPipeline`.
// Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L19-L55 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipelineDesc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphicsPipelineDesc {
    render_step_id: RenderStepID,
    paint_id: UniquePaintParamsID,
}

impl Default for GraphicsPipelineDesc {
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L22-L23 (chrome/m156)
    fn default() -> Self {
        Self {
            render_step_id: RenderStepID::Invalid,
            paint_id: UniquePaintParamsID::invalid(),
        }
    }
}

impl GraphicsPipelineDesc {
    /// `GraphicsPipelineDesc(renderStepID, paintID)`.
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L24-L26 (chrome/m156)
    #[must_use]
    pub const fn new(render_step_id: RenderStepID, paint_id: UniquePaintParamsID) -> Self {
        Self {
            render_step_id,
            paint_id,
        }
    }

    /// `renderStepID()`: describes the geometric portion of the pipeline's program and the
    /// pipeline's fixed state (except for renderpass-level state that will never change between
    /// draws).
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L36 (chrome/m156)
    #[doc(alias = "renderStepID")]
    #[must_use]
    pub const fn render_step_id(&self) -> RenderStepID {
        self.render_step_id
    }

    /// `paintParamsID()`: the `UniquePaintParamsID` of the required `PaintParams`.
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L38 (chrome/m156)
    #[doc(alias = "paintParamsID")]
    #[must_use]
    pub const fn paint_params_id(&self) -> UniquePaintParamsID {
        self.paint_id
    }
}

/// A `GraphicsPipelineCache` is used to de-duplicate `GraphicsPipelineDesc`s when they will all be
/// used by one draw pass.
// Port of: src/gpu/graphite/PipelineData.h#L230-L233 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipelineCache")]
pub type GraphicsPipelineCache = DenseBiMap<GraphicsPipelineDesc, GraphicsPipelineDesc>;

/// What a `PipelineCreationTask` hands out once it has run (the `sk_sp<PipelineCreationTask>`
/// half of the handle's variant).
// Port of: src/gpu/graphite/PipelineCreationTask.h (chrome/m156)
#[doc(alias = "skgpu::graphite::PipelineCreationTask")]
pub trait PipelineCreation: Send + Sync + Debug {
    /// The created pipeline, waiting for the compile to finish if it is still in flight; `None`
    /// if the pipeline could not be created.
    fn pipeline(&self) -> Option<Arc<dyn GraphicsPipeline>>;

    /// For downcasting to the concrete task (`PipelineManager::resolveHandle` waits on it).
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone, Debug)]
enum TaskOrPipeline {
    Task(Arc<dyn PipelineCreation>),
    Pipeline(Option<Arc<dyn GraphicsPipeline>>),
}

/// A draw pass's reference to a pipeline that may still be compiling (`GraphicsPipelineHandle`).
// Port of: src/gpu/graphite/GraphicsPipelineHandle.h#L19-L36 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipelineHandle")]
#[derive(Clone, Debug)]
pub struct GraphicsPipelineHandle {
    task_or_pipeline: TaskOrPipeline,
}

impl GraphicsPipelineHandle {
    /// A handle on a pipeline that is being created by `task` (made by `PipelineManager`).
    // Port of: src/gpu/graphite/GraphicsPipelineHandle.h#L29 (chrome/m156)
    #[must_use]
    pub fn from_task(task: Arc<dyn PipelineCreation>) -> Self {
        Self {
            task_or_pipeline: TaskOrPipeline::Task(task),
        }
    }

    /// A handle on a pipeline that already exists, or on none if creating it failed (made by
    /// `PipelineManager`).
    // Port of: src/gpu/graphite/GraphicsPipelineHandle.h#L31 (chrome/m156)
    #[must_use]
    pub fn from_pipeline(pipeline: Option<Arc<dyn GraphicsPipeline>>) -> Self {
        Self {
            task_or_pipeline: TaskOrPipeline::Pipeline(pipeline),
        }
    }

    /// The task that creates the pipeline, if the handle holds one.
    #[must_use]
    pub fn task(&self) -> Option<&Arc<dyn PipelineCreation>> {
        match &self.task_or_pipeline {
            TaskOrPipeline::Task(task) => Some(task),
            TaskOrPipeline::Pipeline(_) => None,
        }
    }

    /// `pipelineOrNull()`: this should only be called after `Context::insertRecording` or
    /// `DrawPass::addResourceRefs`. Otherwise, retrieval of the `GraphicsPipeline` could be racy.
    // Port of: src/gpu/graphite/GraphicsPipelineHandle.cpp (chrome/m156)
    #[doc(alias = "pipelineOrNull")]
    #[must_use]
    pub fn pipeline_or_null(&self) -> Option<Arc<dyn GraphicsPipeline>> {
        match &self.task_or_pipeline {
            TaskOrPipeline::Task(task) => task.pipeline(),
            TaskOrPipeline::Pipeline(pipeline) => pipeline.clone(),
        }
    }
}

/// The two `PipelineManager` functions a `DrawPass` calls.
///
/// `PipelineManager` is ported with G9b (`docs/design/gpu.md` §5.4). The shared context hands the
/// draw list the factory when it snaps a draw pass (`SharedContext::pipelineManager()`).
// Port of: src/gpu/graphite/PipelineManager.h (createHandle, resolveHandle) (chrome/m156)
#[doc(alias = "skgpu::graphite::PipelineManager")]
pub trait PipelineHandleFactory: Send + Sync + Debug {
    /// `createHandle(sharedContext, runtimeDict, pipelineDesc, renderPassDesc, flags)`.
    #[doc(alias = "createHandle")]
    fn create_handle(
        &self,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        flags: PipelineCreationFlags,
    ) -> GraphicsPipelineHandle;

    /// `resolveHandle(handle)`: the pipeline of `handle`, or `None` if it failed to compile.
    #[doc(alias = "resolveHandle")]
    fn resolve_handle(&self, handle: &GraphicsPipelineHandle) -> Option<Arc<dyn GraphicsPipeline>> {
        handle.pipeline_or_null()
    }
}
