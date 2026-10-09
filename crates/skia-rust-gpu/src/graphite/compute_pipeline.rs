// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ComputePipeline.h (placeholder; the backend half is G11b)

//! The seam for `skgpu::graphite::ComputePipeline`, which
//! [`crate::graphite::global_cache::GlobalCache`] stores. The pipeline itself (the wgpu compute
//! pipeline) is ported with G11b.

use std::any::Any;
use std::fmt::Debug;

/// A compute pipeline, as far as the global cache is concerned.
// Port of: src/gpu/graphite/ComputePipeline.h (chrome/m156)
#[doc(alias = "skgpu::graphite::ComputePipeline")]
pub trait ComputePipeline: Send + Sync + Debug {
    /// For downcasting to the concrete pipeline.
    fn as_any(&self) -> &dyn Any;
}
