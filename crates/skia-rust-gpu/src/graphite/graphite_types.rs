// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/GraphiteTypes.h (the resource-related enums and the
//                   submission types)

//! Public Graphite enums and structs (`include/gpu/graphite/GraphiteTypes.h`): the resource-related
//! enums, and the submission types `Context::insert_recording` and `Context::submit` take.

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

use crate::gpu::gpu_types::{CallbackResult, GpuStats, GpuStatsFlags};
use crate::graphite::recording::Recording;

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

/// The result of `Context::insert_recording`.
///
/// Skia's `InsertStatus` also carries a message. The message only feeds the log in Skia, so it
/// is not ported; the log lines are emitted where the status is produced.
// Port of: include/gpu/graphite/GraphiteTypes.h#L38-L84 (chrome/m156)
#[doc(alias = "skgpu::graphite::InsertStatus")]
#[doc(alias = "InsertStatus::V")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InsertStatus {
    /// Everything successfully added to the underlying command buffer.
    #[doc(alias = "kSuccess")]
    #[default]
    Success,
    /// The recording or the insert info is invalid; no command buffer changes.
    #[doc(alias = "kInvalidRecording")]
    InvalidRecording,
    /// Promise image instantiation failed; no command buffer changes.
    #[doc(alias = "kPromiseImageInstantiationFailed")]
    PromiseImageInstantiationFailed,
    /// Internal failure; the command buffer is partially modified and its state is unknown.
    #[doc(alias = "kAddCommandsFailed")]
    AddCommandsFailed,
    /// Internal failure while compiling shader pipelines; the state is unrecoverable.
    #[doc(alias = "kAsyncShaderCompilesFailed")]
    AsyncShaderCompilesFailed,
    /// The recording is out of order (`RecorderOptions::require_ordered_recordings`).
    #[doc(alias = "kOutOfOrderRecording")]
    OutOfOrderRecording,
}

impl InsertStatus {
    /// `operator bool()`: only [`InsertStatus::Success`] is true.
    // Port of: include/gpu/graphite/GraphiteTypes.h#L63-L66 (chrome/m156)
    #[must_use]
    pub fn is_success(self) -> bool {
        self == Self::Success
    }
}

/// Whether a submission waits for the GPU to finish (`Context::submit`).
// Port of: include/gpu/graphite/GraphiteTypes.h#L178-L182 (chrome/m156)
#[doc(alias = "skgpu::graphite::SyncToCpu")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SyncToCpu {
    /// Wait for the submitted work to finish.
    #[doc(alias = "kYes")]
    Yes,
    /// Do not wait.
    #[doc(alias = "kNo")]
    #[default]
    No,
}

/// Whether a submission marks the end of a frame.
// Port of: include/gpu/graphite/GraphiteTypes.h#L184-L186 (chrome/m156)
#[doc(alias = "skgpu::graphite::MarkFrameBoundary")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MarkFrameBoundary {
    /// The submission marks a frame boundary with `SubmitInfo::frame_id`.
    #[doc(alias = "kYes")]
    Yes,
    /// No frame boundary.
    #[doc(alias = "kNo")]
    #[default]
    No,
}

/// Options for `Context::submit`.
// Port of: include/gpu/graphite/GraphiteTypes.h#L188-L210 (chrome/m156)
#[doc(alias = "skgpu::graphite::SubmitInfo")]
#[derive(Default)]
pub struct SubmitInfo {
    /// `fSync`.
    pub sync: SyncToCpu,
    /// `fMarkBoundary`.
    pub mark_boundary: MarkFrameBoundary,
    /// `fFrameID`.
    pub frame_id: u64,
    /// `fFinishedProc` (its context is captured by the closure): called when all GPU work
    /// submitted by this call has completed.
    pub finished_proc: Option<GpuFinishedProc>,
}

impl SubmitInfo {
    /// `SubmitInfo(sync)`.
    #[must_use]
    pub fn new(sync: SyncToCpu) -> Self {
        Self {
            sync,
            ..Self::default()
        }
    }

    /// `SubmitInfo(sync, frameID)`: marks a frame boundary.
    #[must_use]
    pub fn with_frame_id(sync: SyncToCpu, frame_id: u64) -> Self {
        Self {
            sync,
            mark_boundary: MarkFrameBoundary::Yes,
            frame_id,
            finished_proc: None,
        }
    }
}

impl std::fmt::Debug for SubmitInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubmitInfo")
            .field("sync", &self.sync)
            .field("mark_boundary", &self.mark_boundary)
            .field("frame_id", &self.frame_id)
            .field("finished_proc", &self.finished_proc.is_some())
            .finish()
    }
}

/// The arguments of `Context::insert_recording`.
///
/// The recording is borrowed, as in Skia: the caller keeps owning it, and a failed insert leaves
/// it reinsertable (`RecorderOrderingTest` inserts one again). `None` is Skia's default `nullptr`,
/// which fails with [`InsertStatus::InvalidRecording`]. Skia's `fTargetSurface`,
/// `fTargetTextureState` and the backend semaphores are not ported: the target surface comes with
/// `Surface` (G10d), and wgpu has no semaphores.
// Port of: include/gpu/graphite/GraphiteTypes.h#L125-L163 (chrome/m156)
#[doc(alias = "skgpu::graphite::InsertRecordingInfo")]
#[derive(Default)]
pub struct InsertRecordingInfo<'a> {
    /// `fRecording`.
    pub recording: Option<&'a mut Recording>,
    /// `fTargetTranslation`.
    pub target_translation: IPoint,
    /// `fTargetClip`.
    pub target_clip: IRect,
    /// `fGpuStatsFlags`.
    pub gpu_stats_flags: GpuStatsFlags,
    /// `fFinishedProc`.
    pub finished_proc: Option<GpuFinishedProc>,
    /// `fFinishedWithStatsProc`.
    pub finished_with_stats_proc: Option<GpuFinishedWithStatsProc>,
    /// `fSimulatedStatus`: for unit tests, the status `insert_recording` fails with at the first
    /// point where that status would be produced.
    pub simulated_status: InsertStatus,
}

impl<'a> InsertRecordingInfo<'a> {
    /// `InsertRecordingInfo` with `recording` set and every other field at its default.
    #[must_use]
    pub fn new(recording: &'a mut Recording) -> Self {
        Self {
            recording: Some(recording),
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for InsertRecordingInfo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InsertRecordingInfo")
            .field("recording", &self.recording.is_some())
            .field("target_translation", &self.target_translation)
            .field("target_clip", &self.target_clip)
            .field("gpu_stats_flags", &self.gpu_stats_flags)
            .field("finished_proc", &self.finished_proc.is_some())
            .field(
                "finished_with_stats_proc",
                &self.finished_with_stats_proc.is_some(),
            )
            .field("simulated_status", &self.simulated_status)
            .finish()
    }
}
