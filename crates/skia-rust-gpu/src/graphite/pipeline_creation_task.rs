// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PipelineCreationTask.h

//! [`PipelineCreationTask`]: a unit of pipeline compilation, possibly run on an executor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use crate::gpu::resource_key::UniqueKey;
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::pipeline_manager::PipelineCreationContext;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;

/// A `PipelineCreationTask` serves two purposes:
///
/// - Initially it captures the need to compile a pipeline. In this mode it appears in the
///   `PipelineManager`'s active tasks list and is wrapped in
///   [`GraphicsPipelineHandle`](crate::graphite::graphics_pipeline_handle::GraphicsPipelineHandle)s.
/// - Once executed, the task gets the pipeline it helped compile and hangs around in the handles
///   (through an `Arc`) to resolve them to pipelines.
///
/// When the `PipelineManager` is threaded, the same task can appear in two work lists: once in
/// the low priority list if it was initially kicked off via precompile, and once in the high
/// priority list if a normal compilation found the low priority task. Only one of the two
/// compile closures is allowed to actually compile the pipeline, guarded by the `started` flag.
///
/// Once completed, the task locks the created pipeline in the cache (through `pipeline`) until
/// the task is deleted. This is not a `Task` in the sense of the task graph: it is a unit of work
/// possibly delegated to a thread, known only to the `PipelineManager`.
///
/// # The shared context
///
/// Skia's task holds a raw `SharedContext*`: "the `SharedContext` owns the `PipelineManager`
/// which, in turn, waits for all creation tasks to finish in its destructor, thus we can hold a
/// raw `SharedContext` pointer here". The owning relation is a cycle in Rust (the shared context
/// owns the manager, the manager owns the tasks, and the tasks would own the shared context), so
/// the task holds a `Weak` to it. The invariant is the same as in C++: `Context` shuts the
/// manager down (waiting for the executor to drain) before it releases the shared context, so a
/// task that runs always finds it alive. If it does not (a client dropped every strong reference
/// without shutting down), the task completes without a pipeline.
// Port of: src/gpu/graphite/PipelineCreationTask.h#L32-L96 (chrome/m156)
#[doc(alias = "skgpu::graphite::PipelineCreationTask")]
pub struct PipelineCreationTask {
    /// `fSharedContext`: once the pipeline has been created, this is set to `None` so it should
    /// not be relied upon for shared context access.
    shared_context: Mutex<Option<Weak<dyn PipelineCreationContext>>>,
    /// `fRuntimeDict`.
    pub(crate) runtime_dict: Option<Arc<RuntimeEffectDictionary>>,
    /// `fPipelineKey`: used to track this task in the `PipelineManager`.
    pub(crate) pipeline_key: UniqueKey,
    /// `fGraphicsPipelineDesc`.
    pub(crate) graphics_pipeline_desc: GraphicsPipelineDesc,
    /// `fRenderPassDesc`.
    pub(crate) render_pass_desc: RenderPassDesc,

    /// `fPipeline`: once completed, this task has filled it in (with `None` if compilation
    /// failed). This also serves to lock the pipeline in the cache. It is set once, before
    /// `completed`.
    pub(crate) pipeline: OnceLock<Option<Arc<dyn GraphicsPipeline>>>,

    /// `fInWorkList`: boils down to this task having been placed into a work list. It could be
    /// in two at once.
    pub(crate) in_work_list: AtomicBool,
    /// `fIsHighPriority`.
    pub(crate) is_high_priority: AtomicBool,
    /// `fStarted`.
    pub(crate) started: AtomicBool,
    /// `fCompleted`: atomic since it is also read outside the manager's mutex, in
    /// `GraphicsPipelineHandle::pipelineOrNull`.
    pub(crate) completed: AtomicBool,
}

impl std::fmt::Debug for PipelineCreationTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineCreationTask")
            .field("pipeline_key_hash", &self.pipeline_key.hash())
            .field("graphics_pipeline_desc", &self.graphics_pipeline_desc)
            .field("is_high_priority", &self.is_high_priority)
            .field("started", &self.started)
            .field("completed", &self.completed)
            .finish_non_exhaustive()
    }
}

impl PipelineCreationTask {
    /// The `PipelineCreationTask(sharedContext, runtimeDict, pipelineKey, graphicsPipelineDesc,
    /// renderPassDesc, isHighPriority)` constructor.
    // Port of: src/gpu/graphite/PipelineCreationTask.h#L49-L62 (chrome/m156)
    pub(crate) fn new(
        shared_context: Weak<dyn PipelineCreationContext>,
        runtime_dict: Option<Arc<RuntimeEffectDictionary>>,
        pipeline_key: UniqueKey,
        graphics_pipeline_desc: GraphicsPipelineDesc,
        render_pass_desc: RenderPassDesc,
        is_high_priority: bool,
    ) -> Self {
        Self {
            shared_context: Mutex::new(Some(shared_context)),
            runtime_dict,
            pipeline_key,
            graphics_pipeline_desc,
            render_pass_desc,
            pipeline: OnceLock::new(),
            in_work_list: AtomicBool::new(false),
            is_high_priority: AtomicBool::new(is_high_priority),
            started: AtomicBool::new(false),
            completed: AtomicBool::new(false),
        }
    }

    /// `isLowPriority()`.
    // Port of: src/gpu/graphite/PipelineCreationTask.h#L40-L42 (chrome/m156)
    #[doc(alias = "isLowPriority")]
    #[must_use]
    pub fn is_low_priority(&self) -> bool {
        !self.is_high_priority.load(Ordering::Relaxed)
    }

    /// The shared context the task compiles against, or `None` once the task has completed (or
    /// if the shared context is gone).
    pub(crate) fn shared_context(&self) -> Option<Arc<dyn PipelineCreationContext>> {
        self.shared_context
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(Weak::upgrade)
    }

    /// `fSharedContext = nullptr`.
    pub(crate) fn clear_shared_context(&self) {
        *self
            .shared_context
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }
}
