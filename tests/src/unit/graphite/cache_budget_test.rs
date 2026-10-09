// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/CacheBudgetTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which declares its constant inline.
#![allow(clippy::items_after_statements)]

use skia_rust_gpu::graphite::recorder::RecorderOptions;

use crate::{def_graphite_test_for_contexts_with_options, reporter_assert};

const K_CONTEXT_BUDGET: usize = 1_234_567;

def_graphite_test_for_contexts_with_options!(
    CacheBudgetTest,
    |options| {
        // Port of: tests/graphite/CacheBudgetTest.cpp#L19-L21 (chrome/m156)
        options.gpu_budget_in_bytes = K_CONTEXT_BUDGET;
    },
    |reporter, context| {
        // Port of: tests/graphite/CacheBudgetTest.cpp#L23-L42 (chrome/m156)
        reporter_assert!(reporter, context.max_budgeted_bytes() == K_CONTEXT_BUDGET);

        const K_RECORDER_BUDGET: usize = 7_654_321;
        let recorder_options = RecorderOptions {
            gpu_budget_in_bytes: K_RECORDER_BUDGET,
            ..RecorderOptions::default()
        };

        let recorder = context.make_recorder(Some(&recorder_options));

        reporter_assert!(reporter, recorder.max_budgeted_bytes() == K_RECORDER_BUDGET);
    }
);
