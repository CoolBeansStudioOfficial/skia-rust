// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/SubmitWithFinishProcTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use skia_rust_gpu::graphite::graphite_types::{SubmitInfo, SyncToCpu};

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

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
