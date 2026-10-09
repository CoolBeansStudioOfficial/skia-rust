// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Sampler.h, src/gpu/graphite/Sampler.cpp

//! `skgpu::graphite::Sampler`: a GPU sampler resource.
//!
//! The backend half (the wgpu sampler) implements [`SamplerBackend`] and is owned by the sampler.

use std::any::Any;
use std::fmt;

use crate::graphite::resource::{Resource, ResourceObject, ResourceRef};
use crate::graphite::resource_types::Ownership;

/// The backend half of a [`Sampler`].
pub trait SamplerBackend: Send + Sync + fmt::Debug + 'static {
    /// `freeGpuData()`.
    fn free_gpu_data(&self);
    /// For downcasting to the concrete backend sampler.
    fn as_any(&self) -> &dyn Any;
}

/// A GPU sampler.
#[doc(alias = "skgpu::graphite::Sampler")]
#[derive(Debug)]
pub struct Sampler {
    backend: Box<dyn SamplerBackend>,
}

impl Sampler {
    /// `Sampler(sharedContext)`: creates the sampler resource, holding one usage ref.
    // Port of: src/gpu/graphite/Sampler.cpp#L12-L16 (chrome/m156)
    #[must_use]
    pub fn make(backend: Box<dyn SamplerBackend>) -> ResourceRef<Sampler> {
        Resource::new(
            Sampler { backend },
            Ownership::Owned,
            /* gpuMemorySize= */ 0,
            "",
            false,
            false,
        )
    }

    /// The backend half.
    #[must_use]
    pub fn backend(&self) -> &dyn SamplerBackend {
        &*self.backend
    }
}

impl ResourceObject for Sampler {
    // Port of: src/gpu/graphite/Sampler.h#L21 (chrome/m156)
    fn resource_type(&self) -> &'static str {
        "Sampler"
    }

    fn free_gpu_data(&self) {
        self.backend.free_gpu_data();
    }
}
