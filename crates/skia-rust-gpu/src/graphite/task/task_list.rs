// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/TaskList.h, src/gpu/graphite/task/TaskList.cpp

//! `TaskList`: an ordered list of tasks.

use std::sync::Arc;

use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, TaskRef};
use crate::graphite::texture_proxy::TextureProxy;

/// An ordered list of tasks. Discarded tasks leave an empty slot behind.
#[doc(alias = "skgpu::graphite::TaskList")]
#[derive(Debug, Default)]
pub struct TaskList {
    tasks: Vec<Option<TaskRef>>,
}

impl TaskList {
    /// `TaskList()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `add(TaskList&&)`: moves all of `tasks` to the end of this list.
    // Port of: src/gpu/graphite/task/TaskList.h#L37 (chrome/m156)
    pub fn append(&mut self, mut tasks: TaskList) {
        self.tasks.append(&mut tasks.tasks);
    }

    /// `add(sk_sp<Task>)`.
    // Port of: src/gpu/graphite/task/TaskList.h#L38 (chrome/m156)
    pub fn add(&mut self, task: TaskRef) {
        self.tasks.push(Some(task));
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.tasks.clear();
    }

    /// `size()`: the number of slots, including the ones of discarded tasks.
    #[must_use]
    pub fn size(&self) -> usize {
        self.tasks.len()
    }

    /// `hasTasks()`.
    #[doc(alias = "hasTasks")]
    #[must_use]
    pub fn has_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    // Port of: src/gpu/graphite/task/TaskList.cpp#L20-L38 (chrome/m156)
    fn visit_tasks(&mut self, mut f: impl FnMut(&TaskRef) -> Status) -> Status {
        let mut discard_count = 0;
        for slot in &mut self.tasks {
            let Some(task) = slot else {
                discard_count += 1;
                continue; // Skip over discarded tasks
            };

            let status = f(task);
            if status == Status::Fail {
                return Status::Fail;
            } else if status == Status::Discard {
                *slot = None;
                discard_count += 1;
            }
        }

        if discard_count == self.tasks.len() {
            Status::Discard
        } else {
            Status::Success
        }
    }

    /// `prepareResources()`: returns `Success` if no child task failed and at least one child
    /// didn't return `Discard`. Returns `Discard` if all children were discarded. Returns `Fail`
    /// if any child failed. Automatically removes tasks from its list if they return `Discard`.
    // Port of: src/gpu/graphite/task/TaskList.cpp#L40-L52 (chrome/m156)
    #[doc(alias = "prepareResources")]
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        scratch_manager.push_scope();
        let status = self.visit_tasks(|task| {
            task.lock()
                .prepare_resources(resource_provider, scratch_manager, runtime_dict)
        });
        scratch_manager.pop_scope();
        status
    }

    /// `addCommands()`: like `prepare_resources()`, for the commands.
    // Port of: src/gpu/graphite/task/TaskList.cpp#L54-L60 (chrome/m156)
    #[doc(alias = "addCommands")]
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        self.visit_tasks(|task| {
            task.lock()
                .add_commands(context, command_buffer, replay_data)
        })
    }

    /// `visitPipelines()`.
    // Port of: src/gpu/graphite/task/TaskList.cpp#L62-L70 (chrome/m156)
    #[doc(alias = "visitPipelines")]
    pub fn visit_pipelines(
        &mut self,
        visitor: &mut dyn FnMut(Option<&dyn GraphicsPipeline>) -> bool,
    ) -> bool {
        let status = self.visit_tasks(|task| {
            if task.lock().visit_pipelines(visitor) {
                Status::Success
            } else {
                Status::Fail
            }
        });
        // Map back to simple bool (treat kDiscard as true too, no pipelines to visit means all
        // pipelines were visited).
        status != Status::Fail
    }

    /// `visitProxies()`.
    // Port of: src/gpu/graphite/task/TaskList.cpp#L72-L82 (chrome/m156)
    #[doc(alias = "visitProxies")]
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        let status = self.visit_tasks(|task| {
            if task.lock().visit_proxies(visitor, reads_only) {
                Status::Success
            } else {
                Status::Fail
            }
        });
        // Map back to simple bool (treat kDiscard as true too, no pipelines to visit means all
        // pipelines were visited).
        status != Status::Fail
    }

    /// `visit()` (`SK_DUMP_TASKS`): calls `visitor` with every remaining task and whether it is
    /// the last one.
    // Port of: src/gpu/graphite/task/TaskList.cpp#L85-L100 (chrome/m156)
    pub fn visit(&self, mut visitor: impl FnMut(&TaskRef, bool)) {
        // Find the last non-null task so we know when to draw the corner branch.
        let last_non_null = self.tasks.iter().rposition(Option::is_some);
        for (index, task) in self.tasks.iter().enumerate() {
            if let Some(task) = task {
                visitor(task, Some(index) == last_non_null);
            }
        }
    }
}
