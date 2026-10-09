// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/GpuTypes.h, src/gpu/GpuTypesPriv.h

//! `skgpu` enums shared by the GPU back ends (`GpuTypes.h`), with skia-safe's names
//! (`skia_safe::gpu::{BackendApi, Budgeted, Mipmapped, Protected, Renderable, Origin}`).

/// Possible 3D APIs that may be used by Graphite.
// Port of: include/gpu/GpuTypes.h#L17-L27 (chrome/m156)
#[doc(alias = "skgpu::BackendApi")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum BackendApi {
    /// Dawn (WebGPU); in skia-rust, wgpu.
    #[doc(alias = "kDawn")]
    Dawn,
    /// Metal.
    #[doc(alias = "kMetal")]
    Metal,
    /// Vulkan.
    #[doc(alias = "kVulkan")]
    Vulkan,
    /// The mock back end.
    #[doc(alias = "kMock")]
    Mock,
    /// Graphite doesn't support some context types (e.g. Direct3D) and will return
    /// `Unsupported`.
    #[doc(alias = "kUnsupported")]
    #[default]
    Unsupported,
}

/// `BackendApiToStr`: the C++ enumerator name, e.g. `"kDawn"`.
// Port of: src/gpu/GpuTypesPriv.h#L58-L67 (chrome/m156)
#[doc(alias = "BackendApiToStr")]
#[must_use]
pub const fn backend_api_to_str(backend: BackendApi) -> &'static str {
    match backend {
        BackendApi::Dawn => "kDawn",
        BackendApi::Metal => "kMetal",
        BackendApi::Vulkan => "kVulkan",
        BackendApi::Mock => "kMock",
        BackendApi::Unsupported => "kUnsupported",
    }
}

/// Indicates whether an allocation should count against a cache budget.
// Port of: include/gpu/GpuTypes.h#L29-L32 (chrome/m156)
#[doc(alias = "skgpu::Budgeted")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Budgeted {
    /// Not counted against the budget.
    #[doc(alias = "kNo")]
    #[default]
    No = 0,
    /// Counted against the budget.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// The result of a GPU callback.
// Port of: include/gpu/GpuTypes.h#L34-L37 (chrome/m156)
#[doc(alias = "skgpu::CallbackResult")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CallbackResult {
    /// The work failed.
    #[doc(alias = "kFailed")]
    Failed = 0,
    /// The work succeeded.
    #[doc(alias = "kSuccess")]
    Success = 1,
}

bitflags::bitflags! {
    /// Which GPU statistics a submission should collect.
    // Port of: include/gpu/GpuTypes.h#L83-L87 (chrome/m156)
    #[doc(alias = "skgpu::GpuStatsFlags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct GpuStatsFlags: u32 {
        /// The GPU time the work took.
        #[doc(alias = "kElapsedTime")]
        const ELAPSED_TIME = 0b01;
        /// The number of samples that passed the occlusion test.
        #[doc(alias = "kOcclusionPassSamples")]
        const OCCLUSION_PASS_SAMPLES = 0b10;
    }
}

/// GPU statistics reported to finished procs.
// Port of: include/gpu/GpuTypes.h#L90-L93 (chrome/m156)
#[doc(alias = "skgpu::GpuStats")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuStats {
    /// `elapsedTime`.
    pub elapsed_time: u64,
    /// `numOcclusionPassSamples`.
    pub num_occlusion_pass_samples: u64,
}

/// Is the texture mipmapped or not.
// Port of: include/gpu/GpuTypes.h#L39-L42 (chrome/m156)
#[doc(alias = "skgpu::Mipmapped")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mipmapped {
    /// Single level.
    #[doc(alias = "kNo")]
    #[default]
    No = 0,
    /// Full mip chain.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// Is the data protected on the GPU or not.
// Port of: include/gpu/GpuTypes.h#L44-L47 (chrome/m156)
#[doc(alias = "skgpu::Protected")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Protected {
    /// Ordinary memory.
    #[doc(alias = "kNo")]
    #[default]
    No = 0,
    /// Protected memory.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// Can the texture be rendered to.
// Port of: include/gpu/GpuTypes.h#L49-L52 (chrome/m156)
#[doc(alias = "skgpu::Renderable")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Renderable {
    /// Sample-only.
    #[doc(alias = "kNo")]
    #[default]
    No = 0,
    /// Renderable.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// What is the logical origin of a `BackendTexture` passed into Skia.
// Port of: include/gpu/GpuTypes.h#L78-L81 (chrome/m156)
#[doc(alias = "skgpu::Origin")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Origin {
    /// Row 0 is the top.
    #[doc(alias = "kTopLeft")]
    #[default]
    TopLeft,
    /// Row 0 is the bottom.
    #[doc(alias = "kBottomLeft")]
    BottomLeft,
}

/// `skgpu::StdSteadyClock::time_point`: Graphite's monotonic timestamps.
// Port of: src/gpu/GpuTypesPriv.h#L28-L33 (chrome/m156)
#[doc(alias = "StdSteadyClock")]
pub type StdSteadyClockTimePoint = std::time::Instant;
