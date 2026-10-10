// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ThreadSafeResourceProvider.h, src/gpu/graphite/ThreadSafeResourceProvider.cpp

//! The resource provider shared by every thread that uses a `SharedContext`.
//!
//! It wraps one backend `ResourceProvider` behind a lock. Skia uses a spin lock; the Rust port
//! uses a `Mutex`, which only differs in how it waits. Every operation locks the wrapped provider
//! for its whole duration, as Skia does.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::gpu::gpu_types::StdSteadyClockTimePoint;
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::sampler::Sampler;

/// `kThreadedSafeResourceBudget`: the budget of the thread-safe provider's cache, in bytes.
// Port of: src/gpu/graphite/SharedContext.h#L104 (chrome/m156)
pub const THREADED_SAFE_RESOURCE_BUDGET: usize = 256;

/// A [`ResourceProvider`] that many threads can use at once.
// Port of: src/gpu/graphite/ThreadSafeResourceProvider.h#L29-L52 (chrome/m156)
#[doc(alias = "skgpu::graphite::ThreadSafeResourceProvider")]
pub struct ThreadSafeResourceProvider {
    /// `fWrappedProvider`, guarded by `fSpinLock` in Skia.
    wrapped_provider: Mutex<ResourceProvider>,
}

impl std::fmt::Debug for ThreadSafeResourceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeResourceProvider")
            .finish_non_exhaustive()
    }
}

impl ThreadSafeResourceProvider {
    /// `ThreadSafeResourceProvider(resourceProvider)`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L15-L17 (chrome/m156)
    #[must_use]
    pub fn new(resource_provider: ResourceProvider) -> Self {
        Self {
            wrapped_provider: Mutex::new(resource_provider),
        }
    }

    /// Locks the wrapped provider. A poisoned lock is recovered: the provider's state is not
    /// left half-updated by a panic in a lock holder that Graphite would not already survive.
    fn lock(&self) -> MutexGuard<'_, ResourceProvider> {
        self.wrapped_provider
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `findOrCreateCompatibleSampler(desc)`. The sampler holds no GPU memory, which Skia asserts.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L19-L25 (chrome/m156)
    #[doc(alias = "findOrCreateCompatibleSampler")]
    #[must_use]
    pub fn find_or_create_compatible_sampler(
        &self,
        sampler_desc: &SamplerDesc,
    ) -> Option<ResourceRef<Sampler>> {
        let sampler = self
            .lock()
            .find_or_create_compatible_sampler(sampler_desc)?;
        debug_assert_eq!(sampler.base().gpu_memory_size(), 0);
        Some(sampler)
    }

    /// `getResourceCacheLimit()`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L28-L31 (chrome/m156)
    #[doc(alias = "getResourceCacheLimit")]
    #[must_use]
    pub fn get_resource_cache_limit(&self) -> usize {
        self.lock().get_resource_cache_limit()
    }

    /// `getResourceCacheCurrentBudgetedBytes()`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L33-L36 (chrome/m156)
    #[doc(alias = "getResourceCacheCurrentBudgetedBytes")]
    #[must_use]
    pub fn get_resource_cache_current_budgeted_bytes(&self) -> usize {
        self.lock().get_resource_cache_current_budgeted_bytes()
    }

    /// `getResourceCacheCurrentPurgeableBytes()`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L38-L41 (chrome/m156)
    #[doc(alias = "getResourceCacheCurrentPurgeableBytes")]
    #[must_use]
    pub fn get_resource_cache_current_purgeable_bytes(&self) -> usize {
        self.lock().get_resource_cache_current_purgeable_bytes()
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L49-L52 (chrome/m156)
    #[doc(alias = "freeGpuResources")]
    pub fn free_gpu_resources(&self) {
        self.lock().free_gpu_resources();
    }

    /// `purgeResourcesNotUsedSince(purgeTime)`. The thread-safe cache holds only trivial
    /// objects, so the purge is never time-limited.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L54-L60 (chrome/m156)
    #[doc(alias = "purgeResourcesNotUsedSince")]
    pub fn purge_resources_not_used_since(&self, purge_time: StdSteadyClockTimePoint) {
        self.lock().purge_resources_not_used_since(purge_time, None);
    }

    /// `forceProcessReturnedResources()`.
    // Port of: src/gpu/graphite/ThreadSafeResourceProvider.cpp#L62-L66 (chrome/m156)
    #[doc(alias = "forceProcessReturnedResources")]
    pub fn force_process_returned_resources(&self) {
        self.lock().force_process_returned_resources();
    }
}
