// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipeline.h (placeholder)

//! The seam for `skgpu::graphite::GraphicsPipeline`.
//!
//! The pipeline (the base class plus the wgpu half, one struct) is ported with G11b. Tasks only
//! hand pipelines to visitors (`Task::visitPipelines`), so a pipeline here is any object the
//! visitor can inspect through its own downcasting.

use std::any::Any;
use std::fmt::Debug;

/// A graphics pipeline, as far as the task graph is concerned.
// Port of: src/gpu/graphite/GraphicsPipeline.h (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipeline")]
pub trait GraphicsPipeline: Send + Sync + Debug {
    /// For downcasting to the concrete pipeline.
    fn as_any(&self) -> &dyn Any;

    /// `didAsyncCompilationFail()`: the failure message if compiling the pipeline on the
    /// executor failed. Pipelines compiled synchronously never fail this way.
    // Port of: src/gpu/graphite/GraphicsPipeline.h (didAsyncCompilationFail, chrome/m156)
    #[doc(alias = "didAsyncCompilationFail")]
    fn did_async_compilation_fail(&self) -> Option<String> {
        None
    }
}
