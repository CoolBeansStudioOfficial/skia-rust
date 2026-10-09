// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/DrawTask.h, src/gpu/graphite/task/DrawTask.cpp

//! `DrawTask`: a collection of subtasks that produce some intended image in a target.

use std::sync::Arc;

use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::{PendingUseListener, ScratchResourceManager};
use crate::graphite::task::task_list::TaskList;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};
use crate::graphite::texture_proxy::TextureProxy;

/// `DrawTask` is a collection of subtasks that are executed in order to produce some intended
/// image in the `DrawTask`'s target. As such, at least one of its subtasks will either be a
/// `RenderPassTask`, `ComputeTask` or `CopyXToTextureTask` that directly modify the target.
// Port of: src/gpu/graphite/task/DrawTask.h#L30-L90 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawTask")]
#[derive(Debug)]
pub struct DrawTask {
    target: Arc<TextureProxy>,
    child_tasks: TaskList,

    // Once there is one DrawTask for a scratch device, whether or not the target is
    // instantaited will be equivalent to whether or not prepareResources() has been called
    // already if the task is referenced multiple times in a Recording. Right now, however, a
    // scratch device can still produce several DrawTasks (in which case they will see an
    // instantiated proxy so should still prepare their own resources instead of discarding
    // themselves).
    prepared: bool,
}

// The `PendingUseListener` half of `DrawTask`: skia-rust keeps it apart from the task so the
// scratch manager can call it without locking the task node, which may be in use.
#[derive(Debug)]
struct DrawTaskUse {
    target: Arc<TextureProxy>,
}

impl PendingUseListener for DrawTaskUse {
    // Port of: src/gpu/graphite/task/DrawTask.cpp#L56-L67 (chrome/m156)
    fn on_use_completed(&self, scratch_manager: &mut ScratchResourceManager) {
        // Now that the render task has completed, actually decrement the read count of the
        // target proxy. If the count hits zero, this was the last pending read that needed to
        // use the DrawTask's results so we can return the texture to the ScratchResourceManager
        // for reuse.
        debug_assert!(!self.target.is_lazy() && self.target.is_instantiated());
        debug_assert!(scratch_manager.pending_read_count(&self.target) > 0);
        if scratch_manager.remove_pending_read(&self.target) {
            let texture = self
                .target
                .ref_texture()
                .expect("the target is instantiated");
            scratch_manager.return_texture(&texture);
        }
    }
}

impl DrawTask {
    /// `DrawTask(target)`.
    // Port of: src/gpu/graphite/task/DrawTask.cpp#L17 (chrome/m156)
    #[must_use]
    pub fn new(target: Arc<TextureProxy>) -> Self {
        Self {
            target,
            child_tasks: TaskList::new(),
            prepared: false,
        }
    }

    /// `addTask()`: `DrawTask` is modified directly by `DrawContext` for efficiency, but its
    /// task list will be fixed once `DrawContext` snaps the task.
    #[doc(alias = "addTask")]
    pub fn add_task(&mut self, task: TaskRef) {
        self.child_tasks.add(task);
    }

    /// `hasTasks()`.
    #[doc(alias = "hasTasks")]
    #[must_use]
    pub fn has_tasks(&self) -> bool {
        self.child_tasks.has_tasks()
    }

    /// Wraps the task into a shareable node.
    #[must_use]
    pub fn into_ref(self) -> TaskRef {
        Task::Draw(self).into_ref()
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/DrawTask.cpp#L21-L54 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        let pending_read_count = scratch_manager.pending_read_count(&self.target);
        if pending_read_count != 0 {
            // This DrawTask defines the content of a scratch device that has incremented the
            // pending read count before snap() was called. The target may have already been
            // instantiated if we've processed this task's children before.
            debug_assert!(!self.target.is_lazy());

            // Even though we may discard the task, we always want to mark it as in-use to track
            // the pending reads to know when to return the texture.;
            scratch_manager.mark_resource_in_use(Arc::new(DrawTaskUse {
                target: self.target.clone(),
            }));

            if self.prepared {
                // If the task has already had prepareResources() called once, it should have
                // had its target instantiated.
                debug_assert!(self.target.is_instantiated());
                // Return kDiscard so that this reference to the task is removed and the original
                // encounter in the graph will be the only time addCommands() is invoked.
                return Status::Discard;
            }
        } else {
            // A non-scratch DrawTask should only ever be in the task graph one time.
            debug_assert!(!self.prepared);
        }

        self.prepared = true;

        // NOTE: This prepareResources() pushes a new scope for scratch resource management,
        // which is what we want since the child tasks are what will actually instantiate any
        // scratch device and trigger returns of any grand-child resources. The above
        // markResourceInUse() should happen above this so that pending returns are handled in
        // caller's scope.
        self.child_tasks
            .prepare_resources(resource_provider, scratch_manager, runtime_dict)
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/DrawTask.cpp#L69-L74 (chrome/m156)
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_target: &ReplayTargetData,
    ) -> Status {
        debug_assert!(self.target.is_instantiated());
        self.child_tasks
            .add_commands(context, command_buffer, replay_target)
    }

    /// `visitPipelines()`.
    // Port of: src/gpu/graphite/task/DrawTask.h#L42-L44 (chrome/m156)
    pub fn visit_pipelines(
        &mut self,
        visitor: &mut dyn FnMut(Option<&dyn GraphicsPipeline>) -> bool,
    ) -> bool {
        self.child_tasks.visit_pipelines(visitor)
    }

    /// `visitProxies()`.
    // Port of: src/gpu/graphite/task/DrawTask.h#L46-L49 (chrome/m156)
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        self.child_tasks.visit_proxies(visitor, reads_only)
    }
}
