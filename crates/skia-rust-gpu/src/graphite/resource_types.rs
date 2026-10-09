// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ResourceTypes.h (only the `Layout` enum so far)

//! Graphite's resource types. Only [`Layout`] is ported so far: the rest of `ResourceTypes.h`
//! (buffers, textures, samplers) comes with the Graphite resource layer.

/// Data layout requirements on host-shareable buffer contents.
// Port of: src/gpu/graphite/ResourceTypes.h#L116-L123 (chrome/m156)
#[doc(alias = "skgpu::graphite::Layout")]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layout {
    /// The layout of an uninitialised `UniformOffsetCalculator`.
    #[default]
    #[doc(alias = "kInvalid")]
    Invalid = 0,
    /// OpenGL's std140 uniform block layout.
    #[doc(alias = "kStd140")]
    Std140,
    /// std140 with half-precision uniforms stored as 16-bit floats.
    #[doc(alias = "kStd140_F16")]
    Std140F16,
    /// std430 (shader storage buffers).
    #[doc(alias = "kStd430")]
    Std430,
    /// std430 with half-precision uniforms stored as 16-bit floats.
    #[doc(alias = "kStd430_F16")]
    Std430F16,
    /// Metal's layout (`vec3` takes the size of a `vec4`).
    #[doc(alias = "kMetal")]
    Metal,
}

impl Layout {
    /// The layout's name, for diagnostics.
    // Port of: src/gpu/graphite/ResourceTypes.h#L125-L136 (chrome/m156)
    #[doc(alias = "LayoutString")]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Std140 => "std140",
            Self::Std140F16 => "std140-f16",
            Self::Std430 => "std430",
            Self::Std430F16 => "std430-f16",
            Self::Metal => "metal",
            Self::Invalid => "invalid",
        }
    }
}
