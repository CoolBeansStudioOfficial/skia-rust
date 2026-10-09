// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Attribute.h

//! [`Attribute`] (a vertex or instance attribute), [`Interpolation`] and [`Varying`] (a value
//! passed from the vertex to the fragment shader).

use crate::graphite::draw_types::VertexAttribType;
use crate::sksl_type_shared::SkSLType;

/// Describes a vertex or instance attribute (`skgpu::graphite::Attribute`).
///
/// Skia's `Attribute` stores a non-owning `const char*` name. The name here is a
/// `&'static str`, which is the same contract: every attribute name in the ported `RenderStep`s is
/// a string literal. `Attribute::MakeFromSkMeshAttribute` is not ported because `SkMesh` is not
/// ported yet.
// Port of: src/gpu/graphite/Attribute.h#L17-L62 (chrome/m156)
#[doc(alias = "skgpu::graphite::Attribute")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribute {
    name: &'static str,
    cpu_type: VertexAttribType,
    gpu_type: SkSLType,
}

impl Default for Attribute {
    // `Attribute() = default`: an uninitialized attribute (`gpuType` is `kVoid`).
    fn default() -> Self {
        Self {
            name: "",
            cpu_type: VertexAttribType::Float,
            gpu_type: SkSLType::Void,
        }
    }
}

impl Attribute {
    /// `Attribute(name, cpuType, gpuType)`.
    ///
    /// # Panics
    /// If `gpu_type` is `kVoid`.
    // Port of: src/gpu/graphite/Attribute.h#L24-L30 (chrome/m156)
    #[must_use]
    pub const fn new(name: &'static str, cpu_type: VertexAttribType, gpu_type: SkSLType) -> Self {
        assert!(!matches!(gpu_type, SkSLType::Void));
        Self {
            name,
            cpu_type,
            gpu_type,
        }
    }

    /// `isInitialized()`.
    // Port of: src/gpu/graphite/Attribute.h#L32 (chrome/m156)
    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        !matches!(self.gpu_type, SkSLType::Void)
    }

    /// `name()`.
    // Port of: src/gpu/graphite/Attribute.h#L34 (chrome/m156)
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// `cpuType()`.
    // Port of: src/gpu/graphite/Attribute.h#L35 (chrome/m156)
    #[must_use]
    pub const fn cpu_type(&self) -> VertexAttribType {
        self.cpu_type
    }

    /// `gpuType()`.
    // Port of: src/gpu/graphite/Attribute.h#L36 (chrome/m156)
    #[must_use]
    pub const fn gpu_type(&self) -> SkSLType {
        self.gpu_type
    }

    /// `size()`: the size of the CPU-side attribute in bytes.
    // Port of: src/gpu/graphite/Attribute.h#L37 (chrome/m156)
    #[must_use]
    pub const fn size(&self) -> usize {
        self.cpu_type.size()
    }

    /// `sizeAlign4()`: `size()` rounded up to a multiple of 4 (`SkAlign4`).
    // Port of: src/gpu/graphite/Attribute.h#L38 (chrome/m156)
    #[doc(alias = "sizeAlign4")]
    #[must_use]
    pub const fn size_align4(&self) -> usize {
        (self.size() + 3) & !3
    }
}

/// How a `Varying` is interpolated across a primitive (`skgpu::graphite::Interpolation`).
// Port of: src/gpu/graphite/Attribute.h#L64-L77 (chrome/m156)
#[doc(alias = "skgpu::graphite::Interpolation")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// The default perspective-correct interpolation for floating point types.
    #[default]
    Perspective,
    /// Screen-space linear interpolation for floating point types.
    Linear,
    /// No guarantee on what the provoking vertex is. The only supported interpolation for
    /// integer types.
    Flat,
}

/// An interpolated value passed between a vertex and fragment shader (`skgpu::graphite::Varying`).
// Port of: src/gpu/graphite/Attribute.h#L79-L110 (chrome/m156)
#[doc(alias = "skgpu::graphite::Varying")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Varying {
    name: &'static str,
    gpu_type: SkSLType,
    interpolation: Interpolation,
}

impl Default for Varying {
    fn default() -> Self {
        Self {
            name: "",
            gpu_type: SkSLType::Void,
            interpolation: Interpolation::Perspective,
        }
    }
}

impl Varying {
    /// `Varying(name, gpuType, interpolation)`. Integral types always use `Flat`; explicitly
    /// requesting `Linear` for an integral type is not allowed.
    ///
    /// # Panics
    /// If `gpu_type` is `kVoid` or has no components, or if `Linear` is requested for an
    /// integral type.
    // Port of: src/gpu/graphite/Attribute.h#L84-L99 (chrome/m156)
    #[must_use]
    pub const fn new(name: &'static str, gpu_type: SkSLType, interpolation: Interpolation) -> Self {
        assert!(!matches!(gpu_type, SkSLType::Void));
        assert!(gpu_type.vec_length() >= 1);
        assert!(
            gpu_type.is_float_type() || !matches!(interpolation, Interpolation::Linear),
            "explicitly requesting kLinear for integer types is not allowed"
        );
        Self {
            name,
            gpu_type,
            interpolation: if gpu_type.is_integral_type() {
                Interpolation::Flat
            } else {
                interpolation
            },
        }
    }

    /// `Varying(name, gpuType)`: the default `kPerspective` interpolation.
    // Port of: src/gpu/graphite/Attribute.h#L84 (chrome/m156)
    #[must_use]
    pub const fn with_default_interpolation(name: &'static str, gpu_type: SkSLType) -> Self {
        Self::new(name, gpu_type, Interpolation::Perspective)
    }

    /// `name()`.
    // Port of: src/gpu/graphite/Attribute.h#L101 (chrome/m156)
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// `gpuType()`.
    // Port of: src/gpu/graphite/Attribute.h#L102 (chrome/m156)
    #[must_use]
    pub const fn gpu_type(&self) -> SkSLType {
        self.gpu_type
    }

    /// `interpolation()`.
    // Port of: src/gpu/graphite/Attribute.h#L103 (chrome/m156)
    #[must_use]
    pub const fn interpolation(&self) -> Interpolation {
        self.interpolation
    }
}
