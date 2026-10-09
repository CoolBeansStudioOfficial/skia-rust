// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/Task.h

//! `skgpu::graphite::Task` and the tasks of `src/gpu/graphite/task`.
//!
//! Skia's `Task` is a virtual base class held in `sk_sp<Task>`. Here the closed set of tasks is
//! the [`Task`] enum (`docs/design/gpu.md` §4.1), and shared ownership is [`TaskRef`]
//! (`Arc<TaskNode>`): a device's last task and the root task list may hold the same node. The
//! node's lock guards the state `prepareResources()` and `addCommands()` mutate; it is never
//! contended because a task graph is walked by one thread at a time.
//!
//! The functions take what C++ passes as raw pointers (`ResourceProvider*`,
//! `ScratchResourceManager*`, `Context*`, `CommandBuffer*`) as references. The context and the
//! command buffer are the [`ContextPriv`] and [`CommandBuffer`] seams until G9b/G11c port them.

pub mod clear_buffers_task;
pub mod compute_task;
pub mod copy_task;
pub mod draw_task;
pub mod render_pass_task;
pub mod synchronize_to_cpu_task;
pub mod task_list;
pub mod upload_task;

use std::sync::{Arc, Mutex, MutexGuard};

use skia_rust_core::point::IVector;
use skia_rust_core::rect::IRect;

use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::resource::Resource;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::texture::Texture;
use crate::graphite::texture_proxy::TextureProxy;

use self::clear_buffers_task::ClearBuffersTask;
use self::compute_task::ComputeTask;
use self::copy_task::{CopyBufferToBufferTask, CopyTextureToBufferTask, CopyTextureToTextureTask};
use self::draw_task::DrawTask;
use self::render_pass_task::RenderPassTask;
use self::synchronize_to_cpu_task::SynchronizeToCpuTask;
use self::upload_task::UploadTask;

/// What a task step reports (`Task::Status`).
// Port of: src/gpu/graphite/task/Task.h#L38-L55 (chrome/m156)
#[doc(alias = "Task::Status")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The task step (`prepareResources` or `addCommands`) succeeded, proceed to the next task.
    /// If the Recording is replayed, this task should be executed again.
    #[doc(alias = "kSuccess")]
    Success,
    /// The task step succeeded, but it was a one-time-only operation and should be removed from
    /// the task list. If this is returned from `prepareResources()`, the task is removed before
    /// `addCommands()` will ever be called. If this is returned from `addCommands()`, it will
    /// not be part of any replayed Recording, but any added commands from the first call will be
    /// executed once.
    ///
    /// NOTE: If a task step needs to be conditionally processed but repeatable, it should
    /// internally skip work and still return `Success` instead of `Discard`.
    #[doc(alias = "kDiscard")]
    Discard,
    /// The step failed and cannot be recovered so the Recording is invalidated.
    #[doc(alias = "kFail")]
    Fail,
}

/// Holds a render target and translation to use in the task's work, if necessary.
// Port of: src/gpu/graphite/task/Task.h#L27-L31 (chrome/m156)
#[doc(alias = "Task::ReplayTargetData")]
#[derive(Clone, Debug, Default)]
pub struct ReplayTargetData {
    /// `fTarget`: the replay target texture (not owned by this struct; compared by identity).
    pub target: Option<Arc<Resource<Texture>>>,
    /// `fTranslation`.
    pub translation: IVector,
    /// `fClip`.
    pub clip: IRect,
}

impl ReplayTargetData {
    /// True if `texture` is the replay target (`fTextureProxy->texture() == replayData.fTarget`).
    #[must_use]
    pub fn is_target(&self, texture: Option<&Arc<Resource<Texture>>>) -> bool {
        match (&self.target, texture) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// The unit of GPU work a `Recorder` accumulates and a `Recording` replays.
// Port of: src/gpu/graphite/task/Task.h#L25-L99 (chrome/m156)
#[doc(alias = "skgpu::graphite::Task")]
#[derive(Debug)]
pub enum Task {
    /// `DrawTask`.
    Draw(DrawTask),
    /// `RenderPassTask`.
    RenderPass(RenderPassTask),
    /// `UploadTask`.
    Upload(UploadTask),
    /// `CopyBufferToBufferTask`.
    CopyBufferToBuffer(CopyBufferToBufferTask),
    /// `CopyTextureToBufferTask`.
    CopyTextureToBuffer(CopyTextureToBufferTask),
    /// `CopyTextureToTextureTask`.
    CopyTextureToTexture(CopyTextureToTextureTask),
    /// `ClearBuffersTask`.
    ClearBuffers(ClearBuffersTask),
    /// `SynchronizeToCpuTask`.
    SynchronizeToCpu(SynchronizeToCpuTask),
    /// `ComputeTask`.
    Compute(ComputeTask),
}

impl Task {
    /// `prepareResources()`: instantiate and prepare any Resources that must happen while the
    /// Task is still on the Recorder.
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        match self {
            Task::Draw(t) => t.prepare_resources(resource_provider, scratch_manager, runtime_dict),
            Task::RenderPass(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::Upload(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::CopyBufferToBuffer(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::CopyTextureToBuffer(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::CopyTextureToTexture(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::ClearBuffers(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::SynchronizeToCpu(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
            Task::Compute(t) => {
                t.prepare_resources(resource_provider, scratch_manager, runtime_dict)
            }
        }
    }

    /// `addCommands()`: records the task's commands.
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        match self {
            Task::Draw(t) => t.add_commands(context, command_buffer, replay_data),
            Task::RenderPass(t) => t.add_commands(context, command_buffer, replay_data),
            Task::Upload(t) => t.add_commands(context, command_buffer, replay_data),
            Task::CopyBufferToBuffer(t) => t.add_commands(context, command_buffer, replay_data),
            Task::CopyTextureToBuffer(t) => t.add_commands(context, command_buffer, replay_data),
            Task::CopyTextureToTexture(t) => t.add_commands(context, command_buffer, replay_data),
            Task::ClearBuffers(t) => t.add_commands(context, command_buffer, replay_data),
            Task::SynchronizeToCpu(t) => t.add_commands(context, command_buffer, replay_data),
            Task::Compute(t) => t.add_commands(context, command_buffer, replay_data),
        }
    }

    /// `visitPipelines()`: visits all pipelines until `visitor` returns false to end early. By
    /// default assume the task uses none.
    ///
    /// WARNING: These visit functions will visit all tasks and their children, including
    /// revisiting anything that was added multiple times. Ideally the task graph should be
    /// visited after `prepare_resources()` has been called because that will clean out cycles
    /// and re-references.
    pub fn visit_pipelines(
        &mut self,
        visitor: &mut dyn FnMut(Option<&dyn GraphicsPipeline>) -> bool,
    ) -> bool {
        match self {
            Task::Draw(t) => t.visit_pipelines(visitor),
            Task::RenderPass(t) => t.visit_pipelines(visitor),
            _ => true,
        }
    }

    /// `visitProxies()`: visits all proxies until `visitor` returns false to end early. By
    /// default assume the task uses none.
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        match self {
            Task::Draw(t) => t.visit_proxies(visitor, reads_only),
            Task::RenderPass(t) => t.visit_proxies(visitor, reads_only),
            Task::Upload(t) => t.visit_proxies(visitor, reads_only),
            Task::CopyTextureToBuffer(t) => t.visit_proxies(visitor, reads_only),
            Task::CopyTextureToTexture(t) => t.visit_proxies(visitor, reads_only),
            Task::CopyBufferToBuffer(_)
            | Task::ClearBuffers(_)
            | Task::SynchronizeToCpu(_)
            | Task::Compute(_) => true,
        }
    }

    /// `getTaskName()` (`SK_DUMP_TASKS`).
    #[doc(alias = "getTaskName")]
    #[must_use]
    pub fn task_name(&self) -> &'static str {
        match self {
            Task::Draw(_) => "Draw Task",
            Task::RenderPass(_) => "RenderPass Task",
            Task::Upload(_) => "Upload Task",
            Task::CopyBufferToBuffer(_) => "Copy BtoB Task",
            Task::CopyTextureToBuffer(_) => "Copy TtoB Task",
            Task::CopyTextureToTexture(_) => "Copy TtoT Task",
            Task::ClearBuffers(_) => "Clear Buffers Task",
            Task::SynchronizeToCpu(_) => "Sync to CPU Task",
            Task::Compute(_) => "Compute Task",
        }
    }

    /// Wraps the task into a shareable node.
    #[must_use]
    pub fn into_ref(self) -> TaskRef {
        TaskNode::new(self)
    }
}

/// A shared task (`sk_sp<Task>`): the node's lock guards the state the task steps mutate.
#[derive(Debug)]
pub struct TaskNode {
    task: Mutex<Task>,
}

/// `sk_sp<Task>`.
pub type TaskRef = Arc<TaskNode>;

impl TaskNode {
    /// A new node holding `task`.
    #[must_use]
    pub fn new(task: Task) -> TaskRef {
        Arc::new(TaskNode {
            task: Mutex::new(task),
        })
    }

    /// Locks the task for inspection or a task step. The same thread must not lock a node it
    /// already holds (a task graph never contains itself).
    pub fn lock(&self) -> MutexGuard<'_, Task> {
        self.task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
