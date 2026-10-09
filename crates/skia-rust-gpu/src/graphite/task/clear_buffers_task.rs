// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/ClearBuffersTask.h,
//                   src/gpu/graphite/task/ClearBuffersTask.cpp

//! `ClearBuffersTask`: clears a region of a list of buffers to 0.

use std::sync::Arc;

use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};

/// Task that clears a region of a list of buffers to 0.
// Port of: src/gpu/graphite/task/ClearBuffersTask.h#L27-L49 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClearBuffersTask")]
#[derive(Debug)]
pub struct ClearBuffersTask {
    clear_list: Vec<BindBufferInfo>,
}

impl ClearBuffersTask {
    /// `Make(clearList)`.
    // Port of: src/gpu/graphite/task/ClearBuffersTask.cpp#L17-L19 (chrome/m156)
    #[must_use]
    pub fn make(clear_list: Vec<BindBufferInfo>) -> TaskRef {
        Task::ClearBuffers(ClearBuffersTask { clear_list }).into_ref()
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
    ///
    /// # Panics
    /// If a clear list entry has no buffer.
    // Port of: src/gpu/graphite/task/ClearBuffersTask.cpp#L23-L30 (chrome/m156)
    pub fn add_commands(
        &mut self,
        _context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        _replay_data: &ReplayTargetData,
    ) -> Status {
        let mut result = true;
        for c in &self.clear_list {
            result &= command_buffer.clear_buffer(
                c.buffer.as_ref().expect("clear list entries have a buffer"),
                c.offset as usize,
                c.size as usize,
            );
        }
        if result {
            Status::Success
        } else {
            Status::Fail
        }
    }
}
