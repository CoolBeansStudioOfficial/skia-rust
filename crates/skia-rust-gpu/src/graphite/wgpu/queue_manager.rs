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
use crate::graphite::wgpu::command_buffer::new_wgpu_command_buffer;

/// The wgpu queue manager backend: wraps the device and queue of the context.
// Port of: src/gpu/graphite/dawn/DawnQueueManager.h#L20-L40 (chrome/m156)
#[doc(alias = "skgpu::graphite::DawnQueueManager")]
#[derive(Debug)]
pub struct WgpuQueueManagerBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WgpuQueueManagerBackend {
    /// `DawnQueueManager(queue, sharedContext)`.
    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (constructor, chrome/m156)
    #[must_use]
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self { device, queue }
    }
}

impl QueueManagerBackend for WgpuQueueManagerBackend {
    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp#L110-L115 (chrome/m156)
    fn get_new_command_buffer(
        &mut self,
        resource_provider: &SharedResourceProvider,
        protected: Protected,
    ) -> Option<Box<dyn CommandBuffer>> {
        Some(Box::new(new_wgpu_command_buffer(
            protected,
            resource_provider.clone(),
        )))
    }

    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp#L116-L135 (chrome/m156)
    fn on_submit_to_gpu(
        &mut self,
        command_buffer: Box<dyn CommandBuffer>,
        _submit_info: &SubmitInfo,
    ) -> Option<GpuWorkSubmission> {
        // Every recording hook of the wgpu command buffer reports failure until G11c, so the
        // command buffer holds no commands and the wgpu queue has nothing to submit. The
        // submission is still a fence: it completes when the queue has done all earlier work.
        self.queue.submit(std::iter::empty());
        let done = Arc::new(AtomicBool::new(false));
        let signal = done.clone();
        self.queue.on_submitted_work_done(move || {
            signal.store(true, Ordering::Release);
        });
        Some(GpuWorkSubmission::new(
            command_buffer,
            Box::new(WgpuGpuWorkSubmissionBackend {
                device: self.device.clone(),
                done,
            }),
        ))
    }

    // Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp (tick, chrome/m156)
    fn tick(&self) {
        // `Poll` never fails in a way a caller could act on: a lost device fails the next call.
        let _ = self.device.poll(wgpu::PollType::Poll);
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
