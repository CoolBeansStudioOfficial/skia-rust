// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnQueueManager.h, src/gpu/graphite/dawn/DawnQueueManager.cpp,
//                   src/gpu/graphite/dawn/DawnAsyncWait.h

//! The wgpu half of `QueueManager`: makes command buffers and submits them to the wgpu queue.
//!
//! Completion is tracked as Skia's Dawn submission does: `Queue::on_submitted_work_done` sets a
//! flag, and `Device::poll` lets the flag be observed. The submission ends when the flag is set.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::gpu::gpu_types::Protected;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::gpu_work_submission::{GpuWorkSubmission, GpuWorkSubmissionBackend};
use crate::graphite::graphite_types::SubmitInfo;
use crate::graphite::queue_manager::QueueManagerBackend;
use crate::graphite::wgpu::command_buffer::{WgpuCommandBuffer, new_wgpu_command_buffer};
use crate::graphite::wgpu::shared_context::WgpuSharedContext;

/// The wgpu queue manager backend: wraps the shared context (the device and queue) of the context.
// Port of: src/gpu/graphite/dawn/DawnQueueManager.h#L20-L40 (chrome/m156)
#[doc(alias = "skgpu::graphite::DawnQueueManager")]
#[derive(Debug)]
pub struct WgpuQueueManagerBackend {
    shared_context: Arc<WgpuSharedContext>,
}

impl WgpuQueueManagerBackend {
    /// `DawnQueueManager(queue, sharedContext)`.
    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (constructor, chrome/m156)
    #[must_use]
    pub fn new(shared_context: Arc<WgpuSharedContext>) -> Self {
        Self { shared_context }
    }
}

impl QueueManagerBackend for WgpuQueueManagerBackend {
    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp#L110-L115 (chrome/m156)
    fn get_new_command_buffer(
        &mut self,
        resource_provider: &SharedResourceProvider,
        protected: Protected,
    ) -> Option<Box<dyn CommandBuffer>> {
        new_wgpu_command_buffer(
            protected,
            resource_provider.clone(),
            Arc::clone(&self.shared_context),
        )
        .map(|command_buffer| Box::new(command_buffer) as Box<dyn CommandBuffer>)
    }

    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp#L116-L135 (chrome/m156)
    fn on_submit_to_gpu(
        &mut self,
        mut command_buffer: Box<dyn CommandBuffer>,
        _submit_info: &SubmitInfo,
    ) -> Option<GpuWorkSubmission> {
        let wgpu_command_buffer = command_buffer
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<WgpuCommandBuffer>())
            .and_then(|command_buffer| command_buffer.backend_mut().finish_encoding());
        let Some(wgpu_command_buffer) = wgpu_command_buffer else {
            command_buffer.call_finished_procs(/* success= */ false);
            return None;
        };

        self.shared_context.queue().submit([wgpu_command_buffer]);
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("submit")
        );

        // `DawnWorkSubmissionWithFuture`: the future of `OnSubmittedWorkDone`, which is done
        // when the flag is set by the callback that `Device::poll` runs.
        let done = Arc::new(AtomicBool::new(false));
        let signal = done.clone();
        self.shared_context.queue().on_submitted_work_done(move || {
            signal.store(true, Ordering::Release);
        });
        Some(GpuWorkSubmission::new(
            command_buffer,
            Box::new(WgpuGpuWorkSubmissionBackend {
                device: self.shared_context.device().clone(),
                done,
            }),
        ))
    }

    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (tick, chrome/m156)
    fn tick(&self) {
        // `Poll` never fails in a way a caller could act on: a lost device fails the next call.
        let _ = self.shared_context.device().poll(wgpu::PollType::Poll);
    }
}

/// The wgpu half of a submission: whether the queue has finished its work.
// Port of: src/gpu/graphite/dawn/DawnAsyncWait.h (the completion check, chrome/m156)
#[derive(Debug)]
pub struct WgpuGpuWorkSubmissionBackend {
    device: wgpu::Device,
    done: Arc<AtomicBool>,
}

impl GpuWorkSubmissionBackend for WgpuGpuWorkSubmissionBackend {
    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (onIsFinished, chrome/m156)
    fn on_is_finished(&self) -> bool {
        if !self.done.load(Ordering::Acquire) {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        self.done.load(Ordering::Acquire)
    }

    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (onWaitUntilFinished, chrome/m156)
    fn on_wait_until_finished(&self) {
        while !self.done.load(Ordering::Acquire) {
            // A lost device never runs the callback: stop waiting instead of spinning forever. The
            // next call on the device reports the loss.
            if self
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .is_err()
            {
                break;
            }
        }
    }
}
