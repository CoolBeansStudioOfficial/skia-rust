// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ComputePipelineDesc.h

//! [`ComputePipelineDesc`]: the state needed to create a backend specific compute pipeline.

use std::sync::Arc;

use crate::graphite::compute::compute_step::ComputeStep;

/// `ComputePipelineDesc` represents the state needed to create a backend specific
/// `ComputePipeline`.
///
/// Skia holds a raw `const ComputeStep*` that "must outlive this `ComputePipelineDesc`"; the
/// description shares the step through an `Arc` instead.
// Port of: src/gpu/graphite/ComputePipelineDesc.h#L18-L34 (chrome/m156)
#[doc(alias = "skgpu::graphite::ComputePipelineDesc")]
#[derive(Clone, Debug)]
pub struct ComputePipelineDesc {
    /// `fComputeStep`.
    compute_step: Arc<dyn ComputeStep>,
}

impl ComputePipelineDesc {
    /// `ComputePipelineDesc(computeStep)`.
    // Port of: src/gpu/graphite/ComputePipelineDesc.h#L22-L22 (chrome/m156)
    #[must_use]
    pub fn new(compute_step: Arc<dyn ComputeStep>) -> Self {
        Self { compute_step }
    }

    /// `computeStep()`.
    // Port of: src/gpu/graphite/ComputePipelineDesc.h#L28-L28 (chrome/m156)
    #[doc(alias = "computeStep")]
    #[must_use]
    pub fn compute_step(&self) -> &dyn ComputeStep {
        &*self.compute_step
    }

    /// `uniqueID()`.
    // Port of: src/gpu/graphite/ComputePipelineDesc.h#L30-L30 (chrome/m156)
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.compute_step.unique_id()
    }
}

impl PartialEq for ComputePipelineDesc {
    // Port of: src/gpu/graphite/ComputePipelineDesc.h#L24-L26 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.unique_id() == other.unique_id()
    }
}

impl Eq for ComputePipelineDesc {}
