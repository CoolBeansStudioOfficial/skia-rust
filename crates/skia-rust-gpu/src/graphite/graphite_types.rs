// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/GraphiteTypes.h (the resource-related enums so far)

//! Public Graphite enums (`include/gpu/graphite/GraphiteTypes.h`). Only the ones the resource
//! model and the recorder need are ported so far; the submission types come with the context.

use crate::gpu::gpu_types::{CallbackResult, GpuStats, GpuStatsFlags};

/// Is a lazy proxy fulfilled once or on every insertion.
// Port of: include/gpu/graphite/GraphiteTypes.h#L216-L219 (chrome/m156)
#[doc(alias = "skgpu::graphite::Volatile")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Volatile {
    /// Only fulfilled once.
    #[doc(alias = "kNo")]
    #[default]
    No = 0,
    /// Fulfilled on every insertion call.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// Which of depth and stencil an attachment has.
// Port of: include/gpu/graphite/GraphiteTypes.h#L221-L226 (chrome/m156)
#[doc(alias = "skgpu::graphite::DepthStencilFlags")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DepthStencilFlags {
    /// Neither.
    #[doc(alias = "kNone")]
    #[default]
    None = 0b000,
    /// Depth only.
    #[doc(alias = "kDepth")]
    Depth = 0b001,
    /// Stencil only.
    #[doc(alias = "kStencil")]
    Stencil = 0b010,
    /// Depth and stencil.
    #[doc(alias = "kDepthStencil")]
    DepthStencil = 0b011,
}

/// The valid MSAA sample counts.
// Port of: include/gpu/graphite/GraphiteTypes.h#L228-L234 (chrome/m156)
#[doc(alias = "skgpu::graphite::SampleCount")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SampleCount {
    /// One sample (no MSAA).
    #[doc(alias = "k1")]
    #[default]
    One = 1,
    /// Two samples.
    #[doc(alias = "k2")]
    Two = 2,
    /// Four samples.
    #[doc(alias = "k4")]
    Four = 4,
    /// Eight samples.
    #[doc(alias = "k8")]
    Eight = 8,
    /// Sixteen samples.
    #[doc(alias = "k16")]
    Sixteen = 16,
}

/// `ToSampleCount`: converts an integer to a [`SampleCount`], rounding down to the closest valid
/// count.
// Port of: include/gpu/graphite/GraphiteTypes.h#L240-L246 (chrome/m156)
#[doc(alias = "ToSampleCount")]
#[must_use]
pub const fn to_sample_count(sample_count: u32) -> SampleCount {
    if sample_count >= 16 {
        SampleCount::Sixteen
    } else if sample_count >= 8 {
        SampleCount::Eight
    } else if sample_count >= 4 {
        SampleCount::Four
    } else if sample_count >= 2 {
        SampleCount::Two
    } else {
        SampleCount::One
    }
}

/// The finished proc called when a `Recording` has finished executing on the GPU: with
/// `CallbackResult::Success` if the work completed, `Failed` in all other cases.
///
/// Skia's `GpuFinishedProc` + `GpuFinishedContext` pair is a closure that owns its context.
// Port of: include/gpu/graphite/GraphiteTypes.h#L31-L32 (chrome/m156)
pub type GpuFinishedProc = Box<dyn FnOnce(CallbackResult) + Send>;

/// `GpuFinishedWithStatsProc`.
// Port of: include/gpu/graphite/GraphiteTypes.h#L34-L36 (chrome/m156)
pub type GpuFinishedWithStatsProc = Box<dyn FnOnce(CallbackResult, &GpuStats) + Send>;

/// Provides a finished proc and the stats it wants, to be called when the work in a
/// `Recording` (or everything recorded before an `insertRecording()`) finishes. If the
/// `Recording` is never inserted, or an error happens, the proc is called with
/// `CallbackResult::Failed`.
// Port of: include/gpu/graphite/GraphiteTypes.h#L163-L173 (chrome/m156)
#[doc(alias = "skgpu::graphite::InsertFinishInfo")]
#[derive(Default)]
pub struct InsertFinishInfo {
    /// `fFinishedProc`.
    pub finished_proc: Option<GpuFinishedProc>,
    /// `fFinishedWithStatsProc`.
    pub finished_with_stats_proc: Option<GpuFinishedWithStatsProc>,
    /// `fGpuStatsFlags`.
    pub gpu_stats_flags: GpuStatsFlags,
}

impl InsertFinishInfo {
    /// `InsertFinishInfo(context, proc)`.
    #[must_use]
    pub fn new(finished_proc: GpuFinishedProc) -> Self {
        Self {
            finished_proc: Some(finished_proc),
            ..Self::default()
        }
    }

    /// `InsertFinishInfo(context, withStatsProc)`.
    #[must_use]
    pub fn with_stats(finished_with_stats_proc: GpuFinishedWithStatsProc) -> Self {
        Self {
            finished_with_stats_proc: Some(finished_with_stats_proc),
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for InsertFinishInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InsertFinishInfo")
            .field("finished_proc", &self.finished_proc.is_some())
            .field(
                "finished_with_stats_proc",
                &self.finished_with_stats_proc.is_some(),
            )
            .field("gpu_stats_flags", &self.gpu_stats_flags)
            .finish()
    }
}
