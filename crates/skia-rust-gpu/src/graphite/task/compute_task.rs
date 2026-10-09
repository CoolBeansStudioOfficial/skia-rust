// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/ComputeTask.h, src/gpu/graphite/task/ComputeTask.cpp

//! `ComputeTask`: records `DispatchGroup`s as compute passes.
//!
//! The task is a faithful port. `DispatchGroup` (a compute step and its dispatch description)
//! comes with the compute port (G13), so the task is written against the [`DispatchGroup`] trait
//! with the three members it uses.

use std::fmt::Debug;
use std::sync::Arc;

use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};

/// The members of `DispatchGroup` that `ComputeTask` uses (G13 ports the class).
// Port of: src/gpu/graphite/compute/DispatchGroup.h (chrome/m156)
#[doc(alias = "skgpu::graphite::DispatchGroup")]
pub trait DispatchGroup: Send + Debug {
    /// `snapChildTask()`: the task that must execute before this group, if any.
    #[doc(alias = "snapChildTask")]
    fn snap_child_task(&mut self) -> Option<TaskRef>;

    /// `prepareResources()`.
    #[doc(alias = "prepareResources")]
    fn prepare_resources(&mut self, resource_provider: &mut ResourceProvider) -> bool;
}

/// `ComputeTask` handles preparing and recording `DispatchGroup`s into a series of compute
/// dispatches within a command buffer. It is guaranteed that dispatches within a
/// `DispatchGroup` will be executed sequentially.
// Port of: src/gpu/graphite/task/ComputeTask.h#L28-L56 (chrome/m156)
#[doc(alias = "skgpu::graphite::ComputeTask")]
#[derive(Debug)]
pub struct ComputeTask {
    dispatch_groups: Vec<Box<dyn DispatchGroup>>,

    // Every element of this list is a task that must execute before the DispatchGroup stored at
    // the same array index. Child tasks are allowed to be a nullptr to represent NOP (i.e. the
    // corresponding DispatchGroup doesn't have any pre-tasks).
    child_tasks: Vec<Option<TaskRef>>,
}

impl ComputeTask {
    /// `Make(dispatchGroups)`.
    // Port of: src/gpu/graphite/task/ComputeTask.cpp#L22-L24 (chrome/m156)
    #[must_use]
    pub fn make(dispatch_groups: Vec<Box<dyn DispatchGroup>>) -> TaskRef {
        Task::Compute(ComputeTask::new(dispatch_groups)).into_ref()
    }

    // Port of: src/gpu/graphite/task/ComputeTask.cpp#L26-L31 (chrome/m156)
    fn new(mut dispatch_groups: Vec<Box<dyn DispatchGroup>>) -> Self {
        let mut child_tasks = Vec::with_capacity(dispatch_groups.len());
        for group in &mut dispatch_groups {
            child_tasks.push(group.snap_child_task());
        }
        Self {
            dispatch_groups,
            child_tasks,
        }
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/ComputeTask.cpp#L35-L58 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        for slot in &mut self.child_tasks {
            if let Some(child) = slot {
                let status = child.lock().prepare_resources(
                    resource_provider,
                    scratch_manager,
                    runtime_dict,
                );
                if status == Status::Fail {
                    return Status::Fail;
                } else if status == Status::Discard {
                    *slot = None;
                }
            }
        }

        for group in &mut self.dispatch_groups {
            // TODO: Allow ComputeTasks to instantiate with scratch textures and return them.
            if !group.prepare_resources(resource_provider) {
                return Status::Fail;
            }
        }

        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/ComputeTask.cpp#L60-L94 (chrome/m156)
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        if self.dispatch_groups.is_empty() {
            return Status::Discard;
        }

        debug_assert_eq!(self.dispatch_groups.len(), self.child_tasks.len());
        let mut current_span_start = 0usize;
        let mut current_span_size = 0usize;
        for i in 0..self.dispatch_groups.len() {
            // If the next DispatchGroup has a dependent task, then encode the accumulated span
            // as a compute pass now. CommandBuffer encodes each compute pass with a separate
            // encoder, so the dependent task can use a non-compute encoder if needed.
            if let Some(child) = &self.child_tasks[i] {
                if current_span_size > 0
                    && !command_buffer.add_compute_pass(
                        &self.dispatch_groups
                            [current_span_start..current_span_start + current_span_size],
                    )
                {
                    return Status::Fail;
                }
                if current_span_size > 0 {
                    current_span_start = i;
                    current_span_size = 0;
                }

                let status = child
                    .lock()
                    .add_commands(context, command_buffer, replay_data);
                if status == Status::Fail {
                    return Status::Fail;
                } else if status == Status::Discard {
                    self.child_tasks[i] = None;
                }
            }

            current_span_size += 1;
        }

        if current_span_size == 0
            || command_buffer.add_compute_pass(
                &self.dispatch_groups[current_span_start..current_span_start + current_span_size],
            )
        {
            Status::Success
        } else {
            Status::Fail
        }
    }
}
