// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/SubmitWithFinishProcTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_gpu::gpu::gpu_types::{CallbackResult, Mipmapped};
use skia_rust_gpu::graphite::graphite_types::{
    GpuFinishedProc, InsertRecordingInfo, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::surface_graphite::Surface;

use crate::{def_graphite_test_for_all_contexts, errorf, reporter_assert};

const CALLED_NOT: u8 = 0;
const CALLED_SUCCESS: u8 = 1;
const CALLED_OTHER: u8 = 2;

// `FinishProc(ctx, result)`: `REPORTER_ASSERT(result == kSuccess); fCalled = true`. The result is
// kept for the test to assert, as the closure cannot borrow the reporter.
fn finish_proc(called: &Arc<AtomicU8>) -> GpuFinishedProc {
    let called = Arc::clone(called);
    Box::new(move |result| {
        called.store(
            if result == CallbackResult::Success {
                CALLED_SUCCESS
            } else {
                CALLED_OTHER
            },
            Ordering::Release,
        );
    })
}

// Port of: tests/graphite/SubmitWithFinishProcTest.cpp#L36-L78 (chrome/m156)
def_graphite_test_for_all_contexts!(
    SubmitWithFinishProc_PendingCommandBuffer,
    |reporter, context| {
        let recording_called = Arc::new(AtomicU8::new(CALLED_NOT));
        let submit_called = Arc::new(AtomicU8::new(CALLED_NOT));

        // Ensure the Context is fully idle initially.
        let _ = context.submit(SubmitInfo::new(SyncToCpu::Yes));

        let mut recorder = context.make_recorder(None);
        let ii = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
        let Some(surface) = Surface::render_target(&recorder, &ii, Mipmapped::No, None, "") else {
            errorf!(reporter, "Failed to create surface");
            return;
        };
        surface.canvas().clear(Color4f::new(1.0, 0.0, 0.0, 1.0));

        // Add work to the pending command buffer with a finish proc on the recording.
        let mut recording = recorder.snap();
        let info = InsertRecordingInfo {
            recording: recording.as_mut(),
            finished_proc: Some(finish_proc(&recording_called)),
            ..InsertRecordingInfo::default()
        };
        let _ = context.insert_recording(info);

        // Submit the pending command buffer with a finish proc attached.
        let submit_info = SubmitInfo {
            finished_proc: Some(finish_proc(&submit_called)),
            ..SubmitInfo::default()
        };
        let _ = context.submit(submit_info);

        // Syncing should trigger both callbacks.
        let _ = context.submit(SubmitInfo::new(SyncToCpu::Yes));
        reporter_assert!(
            reporter,
            recording_called.load(Ordering::Acquire) == CALLED_SUCCESS
        );
        reporter_assert!(
            reporter,
            submit_called.load(Ordering::Acquire) == CALLED_SUCCESS
        );
    }
);

// Port of: tests/graphite/SubmitWithFinishProcTest.cpp#L80-L99 (chrome/m156)
def_graphite_test_for_all_contexts!(
    SubmitWithFinishProc_NoPendingCommandBuffer,
    |reporter, context| {
        // Ensure the Context is fully idle (`testCtx->syncedSubmit(context)`).
        let _ = context.submit(SubmitInfo::new(SyncToCpu::Yes));

        let called = Arc::new(AtomicBool::new(false));
        let signal = called.clone();

        // No pending command buffer and GPU is idle: the proc should be triggered immediately.
        let mut submit_info = SubmitInfo::new(SyncToCpu::No);
        submit_info.finished_proc = Some(Box::new(move |_| {
            signal.store(true, Ordering::Release);
        }));
        let _ = context.submit(submit_info);

        reporter_assert!(reporter, called.load(Ordering::Acquire));
    }
);
