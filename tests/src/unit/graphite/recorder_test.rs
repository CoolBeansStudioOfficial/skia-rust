// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/RecorderTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::{Arc, Mutex};

use skia_rust_gpu::gpu::gpu_types::CallbackResult;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, InsertStatus};
use skia_rust_gpu::graphite::recorder::RecorderOptions;
use skia_rust_gpu::graphite::recording::Recording;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `insert` of the C++ test: inserts `recording` into `context`.
fn insert(context: &mut WgpuContext, recording: &mut Recording) -> InsertStatus {
    context.insert_recording(InsertRecordingInfo::new(recording))
}

// Port of: tests/graphite/RecorderTest.cpp#L107-L147 (chrome/m156)
def_graphite_test_for_all_contexts!(RecorderOrderingTest, |reporter, context| {
    let mut ordered_recorder;
    let mut unordered_recorder;
    if context.caps().require_ordered_recordings() {
        ordered_recorder = context.make_recorder(None);
        let opts = RecorderOptions {
            require_ordered_recordings: Some(false),
            ..RecorderOptions::default()
        };
        unordered_recorder = context.make_recorder(Some(&opts));
    } else {
        let opts = RecorderOptions {
            require_ordered_recordings: Some(true),
            ..RecorderOptions::default()
        };
        ordered_recorder = context.make_recorder(Some(&opts));
        unordered_recorder = context.make_recorder(None);
    }

    let mut o1 = ordered_recorder.snap().expect("snap");
    let mut o2 = ordered_recorder.snap().expect("snap");
    let mut o3 = ordered_recorder.snap().expect("snap");
    let mut u1 = unordered_recorder.snap().expect("snap");
    let mut u2 = unordered_recorder.snap().expect("snap");

    // Unordered insertion of an unordered Recorder succeeds.
    // NOTE: These Recordings are all out-of-order with respect to orderedRecorder, which had been
    // snapped multiple times before unorderedRecorder. That is always allowed.
    reporter_assert!(reporter, insert(context, &mut u2).is_success());
    reporter_assert!(reporter, insert(context, &mut u1).is_success());

    // Unordered insertion of an ordered Recorder fails
    reporter_assert!(reporter, insert(context, &mut o1).is_success()); // succeeds (first insertion)
    reporter_assert!(reporter, !insert(context, &mut o3).is_success()); // fails for out of order
    reporter_assert!(reporter, insert(context, &mut o2).is_success()); // succeeds and recovers
    reporter_assert!(reporter, insert(context, &mut o3).is_success()); // now in order success
});

// Test to make sure inserting a null Recording (e.g. Recorder::snap() failed and the client didn't
// check it) returns the correct invalid status.
// Port of: tests/graphite/RecorderTest.cpp#L151-L170 (chrome/m156)
def_graphite_test_for_all_contexts!(NullRecordingInsertTest, |reporter, context| {
    // The finished proc records the result it is called with; the C++ test asserts it inside the
    // proc, here it is checked after the insert, which runs the proc before it returns.
    let finish_result: Arc<Mutex<Option<CallbackResult>>> = Arc::new(Mutex::new(None));
    let recorded = finish_result.clone();

    // Null recording: set to null to simulate a Recorder::snap() failure.
    let info = InsertRecordingInfo {
        finished_proc: Some(Box::new(move |result| {
            *recorded.lock().expect("the result lock") = Some(result);
        })),
        ..InsertRecordingInfo::default()
    };
    // Should not crash on the null Recording!
    let status = context.insert_recording(info);
    reporter_assert!(reporter, status == InsertStatus::InvalidRecording);
    let result = *finish_result.lock().expect("the result lock");
    reporter_assert!(reporter, result == Some(CallbackResult::Failed));
});
