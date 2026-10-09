// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipelineHandle.h and the handle half of
// src/gpu/graphite/PipelineManager.cpp

//! [`GraphicsPipelineHandle`]: a pipeline, or the task that will create it.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::pipeline_creation_task::PipelineCreationTask;

/// What a handle holds (`std::variant<sk_sp<PipelineCreationTask>, sk_sp<GraphicsPipeline>>`).
#[derive(Clone, Debug)]
enum TaskOrPipeline {
    Task(Arc<PipelineCreationTask>),
    Pipeline(Arc<dyn GraphicsPipeline>),
}

/// The `GraphicsPipelineHandle` holds a ref to either a pipeline or the task that will create the
/// pipeline. In the latter case, `PipelineManager::resolve_handle` can be used to wait for the
/// task to complete. How this works is:
///
/// - At `Recorder::snap` time, the `DrawPass` creates handles and kicks off all the tasks (with
///   `PipelineManager::create_handle`).
/// - Upon `Context::insertRecording`, all the handles are resolved to pipelines in
///   `DrawPass::addResourceRefs`. After that the pipeline can be accessed with
///   [`pipeline_or_null`](Self::pipeline_or_null).
///
/// Note that the tasks lock the generated pipelines in the cache until they are deleted. This
/// avoids any race conditions where a pipeline could be purged between when it was created on a
/// thread and when it was actually requested by a `DrawPass`. The pipeline-backed variant also
/// locks the pipeline in the cache, but there is no race condition there.
// Port of: src/gpu/graphite/GraphicsPipelineHandle.h#L35-L52 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipelineHandle")]
#[derive(Clone, Debug)]
pub struct GraphicsPipelineHandle {
    task_or_pipeline: TaskOrPipeline,
}

impl GraphicsPipelineHandle {
    /// `GraphicsPipelineHandle(sk_sp<PipelineCreationTask>)`: only `PipelineManager` makes
    /// handles.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L50-L51 (chrome/m156)
    pub(crate) fn from_task(task: Arc<PipelineCreationTask>) -> Self {
        Self {
            task_or_pipeline: TaskOrPipeline::Task(task),
        }
    }

    /// `GraphicsPipelineHandle(sk_sp<GraphicsPipeline>)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L53-L54 (chrome/m156)
    pub(crate) fn from_pipeline(pipeline: Arc<dyn GraphicsPipeline>) -> Self {
        Self {
            task_or_pipeline: TaskOrPipeline::Pipeline(pipeline),
        }
    }

    /// The task, if the handle holds one (`std::get<sk_sp<PipelineCreationTask>>`).
    pub(crate) fn task(&self) -> Option<&Arc<PipelineCreationTask>> {
        match &self.task_or_pipeline {
            TaskOrPipeline::Task(task) => Some(task),
            TaskOrPipeline::Pipeline(_) => None,
        }
    }

    /// The pipeline, if the handle holds one.
    pub(crate) fn held_pipeline(&self) -> Option<&Arc<dyn GraphicsPipeline>> {
        match &self.task_or_pipeline {
            TaskOrPipeline::Task(_) => None,
            TaskOrPipeline::Pipeline(pipeline) => Some(pipeline),
        }
    }

    /// `pipelineOrNull()`: the pipeline, or `None` if the task has not completed or failed.
    ///
    /// This should only be called after `Context::insertRecording` /
    /// `DrawPass::addResourceRefs`, otherwise the retrieval is racy. Directly accessing the two
    /// variants is thread safe because a given handle does not switch between a task and a
    /// pipeline when the task completes. The `completed` check is atomic and, if the task is
    /// completed, the pipeline access is thread safe. There is, obviously, an inherent race
    /// with the `completed` access. Callers must either ensure that the handle has already been
    /// resolved or accept some raciness in the response.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L36-L48 (chrome/m156)
    #[doc(alias = "pipelineOrNull")]
    #[must_use]
    pub fn pipeline_or_null(&self) -> Option<Arc<dyn GraphicsPipeline>> {
        match &self.task_or_pipeline {
            TaskOrPipeline::Pipeline(pipeline) => Some(Arc::clone(pipeline)),
            TaskOrPipeline::Task(task) => {
                if !task.completed.load(Ordering::Acquire) {
                    return None;
                }
                task.pipeline.get().cloned().flatten()
            }
        }
    }
}
