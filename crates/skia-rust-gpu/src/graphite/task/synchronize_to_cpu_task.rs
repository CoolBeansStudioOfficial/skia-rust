// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/SynchronizeToCpuTask.h,
//                   src/gpu/graphite/task/SynchronizeToCpuTask.cpp

//! `SynchronizeToCpuTask`: makes the GPU's writes to a buffer visible to the CPU.

use std::sync::Arc;

use crate::graphite::buffer::Buffer;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};

/// Task that synchronizes the contents of a buffer from the GPU to the CPU. This task ensures
/// that all modifications to the buffer made the GPU are visible from the CPU. This task may not
/// result in any work if the underlying buffer does not require synchronization (e.g. a shared
/// memory buffer).
// Port of: src/gpu/graphite/task/SynchronizeToCpuTask.h#L27-L49 (chrome/m156)
#[doc(alias = "skgpu::graphite::SynchronizeToCpuTask")]
#[derive(Debug)]
pub struct SynchronizeToCpuTask {
    // `std::move`d into the command buffer by addCommands().
    buffer: Option<ResourceRef<Buffer>>,
}

impl SynchronizeToCpuTask {
    /// `Make(buffer)`.
    // Port of: src/gpu/graphite/task/SynchronizeToCpuTask.cpp#L16-L18 (chrome/m156)
    #[must_use]
    pub fn make(buffer: ResourceRef<Buffer>) -> TaskRef {
        Task::SynchronizeToCpu(SynchronizeToCpuTask {
            buffer: Some(buffer),
        })
        .into_ref()
    }

    /// `prepareResources()`.
    pub fn prepare_resources(
        &mut self,
        _resource_provider: &mut ResourceProvider,
        _scratch_manager: &mut ScratchResourceManager,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/SynchronizeToCpuTask.cpp#L22-L28 (chrome/m156)
    pub fn add_commands(
        &mut self,
        _context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        _replay_data: &ReplayTargetData,
    ) -> Status {
        // C++ moves a null buffer into the command buffer on a second run; the port fails the
        // step instead.
        let Some(buffer) = self.buffer.take() else {
            return Status::Fail;
        };
        if command_buffer.synchronize_buffer_to_cpu(buffer) {
            Status::Success
        } else {
            Status::Fail
        }
    }
}
