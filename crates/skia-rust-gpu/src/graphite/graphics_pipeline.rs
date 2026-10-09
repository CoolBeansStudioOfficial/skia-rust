// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipeline.h (the base class members; the backend half
// is G11b)

//! The seam for `skgpu::graphite::GraphicsPipeline`.
//!
//! The pipeline (the base class plus the wgpu half, one struct) is ported with G11b. Tasks only
//! hand pipelines to visitors (`Task::visitPipelines`), and the [`crate::graphite::global_cache::GlobalCache`]
//! keeps them in its LRU. Both need the base class's bookkeeping, which is [`GraphicsPipelineBase`]
//! and is reached through [`GraphicsPipeline::base`]. G11b embeds one in each pipeline.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use bitflags::bitflags;

use crate::gpu::gpu_types::StdSteadyClockTimePoint;

bitflags! {
    /// `PipelineCreationFlags`: how a pipeline is being requested.
    // Port of: src/gpu/graphite/GraphicsPipeline.h#L17-L21 (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PipelineCreationFlags: u8 {
        /// `kNone`.
        const NONE = 0;
        /// `kForPrecompilation`: the request comes from the precompile API.
        const FOR_PRECOMPILATION = 0b001;
    }
}

/// A graphics pipeline, as far as the task graph and the global cache are concerned.
// Port of: src/gpu/graphite/GraphicsPipeline.h (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipeline")]
pub trait GraphicsPipeline: Send + Sync + Debug {
    /// For downcasting to the concrete pipeline.
    fn as_any(&self) -> &dyn Any;

    /// The base class members: the label, the key hash, the epoch and use bookkeeping.
    fn base(&self) -> &GraphicsPipelineBase;

    /// `didAsyncCompilationFail()`: the failure message if compiling the pipeline on the
    /// executor failed. Pipelines compiled synchronously never fail this way.
    // Port of: src/gpu/graphite/GraphicsPipeline.h#L110-L110 (chrome/m156)
    #[doc(alias = "didAsyncCompilationFail")]
    fn did_async_compilation_fail(&self) -> Option<String> {
        None
    }

    /// `getLabel()`.
    #[doc(alias = "getLabel")]
    fn label(&self) -> &str {
        self.base().label()
    }

    /// `fromPrecompile()`.
    #[doc(alias = "fromPrecompile")]
    fn from_precompile(&self) -> bool {
        self.base().from_precompile()
    }
}

/// The members of `GraphicsPipeline` and its `PipelineInfo` that the cache reads and updates.
///
/// Skia keeps them in the pipeline object and updates the epoch and the use flag under the
/// `GlobalCache`'s spinlock. They are atomics and a mutex here, so the pipeline can be shared
/// through an `Arc`.
// Port of: src/gpu/graphite/GraphicsPipeline.h#L37-L110 (chrome/m156)
#[derive(Debug)]
pub struct GraphicsPipelineBase {
    /// `fLabel`.
    label: String,
    /// `fPipelineInfo.fUniqueKeyHash`.
    unique_key_hash: u32,
    /// `fPipelineInfo.fCompilationID`.
    compilation_id: u32,
    /// `fPipelineInfo.fFromPrecompile`.
    from_precompile: bool,
    /// `fPipelineInfo.fWasUsed`.
    was_used: AtomicBool,
    /// `fPipelineInfo.fEpoch`: the last epoch in which the pipeline was touched.
    epoch: AtomicU16,
    /// `Resource::lastAccessTime()`, which `updateAccessTime()` sets.
    last_access: Mutex<StdSteadyClockTimePoint>,
}

impl GraphicsPipelineBase {
    /// `GraphicsPipeline(sharedContext, pipelineInfo, label)`: the members start unused, in epoch
    /// zero, and accessed now.
    // Port of: src/gpu/graphite/GraphicsPipeline.cpp (chrome/m156)
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        unique_key_hash: u32,
        compilation_id: u32,
        from_precompile: bool,
    ) -> Self {
        Self {
            label: label.into(),
            unique_key_hash,
            compilation_id,
            from_precompile,
            was_used: AtomicBool::new(false),
            epoch: AtomicU16::new(0),
            last_access: Mutex::new(StdSteadyClockTimePoint::now()),
        }
    }

    /// `getLabel()`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// `getPipelineInfo().fUniqueKeyHash`.
    #[must_use]
    pub fn unique_key_hash(&self) -> u32 {
        self.unique_key_hash
    }

    /// `getPipelineInfo().fCompilationID`.
    #[must_use]
    pub fn compilation_id(&self) -> u32 {
        self.compilation_id
    }

    /// `fromPrecompile()`.
    #[must_use]
    pub fn from_precompile(&self) -> bool {
        self.from_precompile
    }

    /// `markUsed()`.
    pub fn mark_used(&self) {
        self.was_used.store(true, Ordering::Relaxed);
    }

    /// `wasUsed()`.
    #[must_use]
    pub fn was_used(&self) -> bool {
        self.was_used.load(Ordering::Relaxed)
    }

    /// `markEpoch(epoch)`.
    pub fn mark_epoch(&self, epoch: u16) {
        self.epoch.store(epoch, Ordering::Relaxed);
    }

    /// `epoch()`.
    #[must_use]
    pub fn epoch(&self) -> u16 {
        self.epoch.load(Ordering::Relaxed)
    }

    /// `updateAccessTime()`.
    pub fn update_access_time(&self) {
        *self
            .last_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = StdSteadyClockTimePoint::now();
    }

    /// `lastAccessTime()`.
    #[must_use]
    pub fn last_access_time(&self) -> StdSteadyClockTimePoint {
        *self
            .last_access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
