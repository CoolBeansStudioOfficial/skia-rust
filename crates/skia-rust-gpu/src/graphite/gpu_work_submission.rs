// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GpuWorkSubmission.h, src/gpu/graphite/GpuWorkSubmission.cpp

//! One submission of a command buffer to the GPU, outstanding until the GPU finishes it.
//!
//! The neutral half is [`GpuWorkSubmission`]: it owns the command buffer, the async-map counter
//! and the finished procs. The backend half ([`GpuWorkSubmissionBackend`], `onIsFinished` and
//! `onWaitUntilFinished` in Skia) is the wgpu completion tracking that G11c provides.

use std::sync::Arc;

use crate::gpu::ref_cnted_callback::RefCntedCallback;
use crate::graphite::buffer::Buffer;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::resource::ResourceRef;

/// The backend half of a submission (`onIsFinished` and `onWaitUntilFinished`).
// Port of: src/gpu/graphite/GpuWorkSubmission.h#L39-L40 (chrome/m156)
pub trait GpuWorkSubmissionBackend: Send {
    /// `onIsFinished()`: true once the GPU has finished the submitted work.
    #[doc(alias = "onIsFinished")]
    fn on_is_finished(&self) -> bool;

    /// `onWaitUntilFinished()`: blocks until the GPU has finished the submitted work.
    #[doc(alias = "onWaitUntilFinished")]
    fn on_wait_until_finished(&self);
}

/// A command buffer submitted to the GPU and not yet known to be finished.
// Port of: src/gpu/graphite/GpuWorkSubmission.h#L24-L41 (chrome/m156)
#[doc(alias = "skgpu::graphite::GpuWorkSubmission")]
pub struct GpuWorkSubmission {
    /// `fCommandBuffer`.
    command_buffer: Option<Box<dyn CommandBuffer>>,
    /// `fOutstandingAsyncMapCounter`: each pending async map holds a clone; the submission is
    /// finished once only this handle is left.
    outstanding_async_map_counter: Option<Arc<()>>,
    /// The backend half.
    backend: Box<dyn GpuWorkSubmissionBackend>,
}

impl std::fmt::Debug for GpuWorkSubmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuWorkSubmission")
            .field("has_command_buffer", &self.command_buffer.is_some())
            .field(
                "outstanding_async_maps",
                &self
                    .outstanding_async_map_counter
                    .as_ref()
                    .map(Arc::strong_count),
            )
            .finish_non_exhaustive()
    }
}

impl GpuWorkSubmission {
    /// `GpuWorkSubmission(cmdBuffer, queueManager)`: starts the async maps the command buffer
    /// asked for on submit.
    // Port of: src/gpu/graphite/GpuWorkSubmission.cpp#L17-L32 (chrome/m156)
    #[must_use]
    pub fn new(
        command_buffer: Box<dyn CommandBuffer>,
        backend: Box<dyn GpuWorkSubmissionBackend>,
    ) -> Self {
        let buffers: Vec<ResourceRef<Buffer>> =
            command_buffer.buffers_to_async_map_on_submit().to_vec();
        let outstanding_async_map_counter = if buffers.is_empty() {
            None
        } else {
            let counter = Arc::new(());
            for buffer in &buffers {
                debug_assert!(!buffer.is_unmappable());
                let pending = counter.clone();
                buffer.async_map(Some(Box::new(move |_| drop(pending))));
            }
            Some(counter)
        };
        Self {
            command_buffer: Some(command_buffer),
            outstanding_async_map_counter,
            backend,
        }
    }

    /// `isFinished(sharedContext)`.
    // Port of: src/gpu/graphite/GpuWorkSubmission.cpp#L41-L44 (chrome/m156)
    #[doc(alias = "isFinished")]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.backend.on_is_finished()
            && self
                .outstanding_async_map_counter
                .as_ref()
                .is_none_or(|counter| Arc::strong_count(counter) == 1)
    }

    /// `waitUntilFinished(sharedContext)`. `tick` is `QueueManager::tick()`, called while the
    /// async maps are still pending.
    // Port of: src/gpu/graphite/GpuWorkSubmission.cpp#L46-L53 (chrome/m156)
    #[doc(alias = "waitUntilFinished")]
    pub fn wait_until_finished(&self, tick: &dyn Fn()) {
        self.backend.on_wait_until_finished();
        if let Some(counter) = &self.outstanding_async_map_counter {
            while Arc::strong_count(counter) > 1 {
                tick();
            }
        }
    }

    /// `addFinishedProc()`.
    // Port of: src/gpu/graphite/GpuWorkSubmission.cpp#L55-L57 (chrome/m156)
    #[doc(alias = "addFinishedProc")]
    pub fn add_finished_proc(&mut self, finished_proc: Arc<RefCntedCallback>) {
        if let Some(command_buffer) = self.command_buffer.as_mut() {
            command_buffer.add_finished_proc(finished_proc);
        }
    }

    /// The `~GpuWorkSubmission()` steps that run before the command buffer goes back to the
    /// queue manager: the finished procs succeed, and the command buffer is reset. Returns the
    /// command buffer for [`crate::graphite::queue_manager::QueueManager`] to pool.
    // Port of: src/gpu/graphite/GpuWorkSubmission.cpp (destructor) (chrome/m156)
    pub(crate) fn retire(mut self) -> Option<Box<dyn CommandBuffer>> {
        let mut command_buffer = self.command_buffer.take()?;
        command_buffer.call_finished_procs(true);
        command_buffer.reset_command_buffer();
        Some(command_buffer)
    }
}
