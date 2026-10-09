// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSLTypeShared.h, src/core/SkSLTypeShared.cpp

//! The shading-language types shared by Skia's core and GPU code (`SkSLType`).
//!
//! This is the part of `SkSLTypeShared` that the GPU uniform code uses. It belongs to
//! `skia-rust-sksl` in the crate layout (`docs/design/sksl.md`); it moves there when that crate
//! lands, so the names here are the final ones.

/// Types of shader-language-specific boxed variables (`SkSLType`).
// Port of: src/core/SkSLTypeShared.h#L16-L59 (chrome/m156)
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkSLType {
    Void,
    Bool,
    Bool2,
    Bool3,
    Bool4,
    Short,
    Short2,
    Short3,
    Short4,
    UShort,
    UShort2,
    UShort3,
    UShort4,
    Float,
    Float2,
    Float3,
    Float4,
    Float2x2,
    Float3x3,
    Float4x4,
    Half,
    Half2,
    Half3,
    Half4,
    Half2x2,
    Half3x3,
    Half4x4,
    Int,
    Int2,
    Int3,
    Int4,
    UInt,
    UInt2,
    UInt3,
    UInt4,
    Texture2DSampler,
    TextureExternalSampler,
    Texture2DRectSampler,
    Texture2D,
    Sampler,
    Input,
}

/// Returns the GLSL-facing name of `t`.
const fn sksl_type_string(t: SkSLType) -> &'static str {
    match t {
        SkSLType::Void => "void",
        SkSLType::Bool => "bool",
        SkSLType::Bool2 => "bool2",
        SkSLType::Bool3 => "bool3",
        SkSLType::Bool4 => "bool4",
        SkSLType::Short => "short",
        SkSLType::Short2 => "short2",
        SkSLType::Short3 => "short3",
        SkSLType::Short4 => "short4",
        SkSLType::UShort => "ushort",
        SkSLType::UShort2 => "ushort2",
        SkSLType::UShort3 => "ushort3",
        SkSLType::UShort4 => "ushort4",
        SkSLType::Float => "float",
        SkSLType::Float2 => "float2",
        SkSLType::Float3 => "float3",
        SkSLType::Float4 => "float4",
        SkSLType::Float2x2 => "float2x2",
        SkSLType::Float3x3 => "float3x3",
        SkSLType::Float4x4 => "float4x4",
        SkSLType::Half => "half",
        SkSLType::Half2 => "half2",
        SkSLType::Half3 => "half3",
        SkSLType::Half4 => "half4",
        SkSLType::Half2x2 => "half2x2",
        SkSLType::Half3x3 => "half3x3",
        SkSLType::Half4x4 => "half4x4",
        SkSLType::Int => "int",
        SkSLType::Int2 => "int2",
        SkSLType::Int3 => "int3",
        SkSLType::Int4 => "int4",
        SkSLType::UInt => "uint",
        SkSLType::UInt2 => "uint2",
        SkSLType::UInt3 => "uint3",
        SkSLType::UInt4 => "uint4",
        SkSLType::Texture2DSampler => "sampler2D",
        SkSLType::TextureExternalSampler => "samplerExternalOES",
        SkSLType::Texture2DRectSampler => "sampler2DRect",
        SkSLType::Texture2D => "texture2D",
        SkSLType::Sampler => "sampler",
        SkSLType::Input => "subpassInput",
    }
}

impl SkSLType {
    /// Is the shading language type float (including vectors/matrices)?
    // Port of: src/core/SkSLTypeShared.h#L67-L116 (chrome/m156)
    #[doc(alias = "SkSLTypeIsFloatType")]
    #[must_use]
    pub const fn is_float_type(self) -> bool {
        matches!(
            self,
            Self::Float
                | Self::Float2
                | Self::Float3
                | Self::Float4
                | Self::Float2x2
                | Self::Float3x3
                | Self::Float4x4
                | Self::Half
                | Self::Half2
                | Self::Half3
                | Self::Half4
                | Self::Half2x2
                | Self::Half3x3
                | Self::Half4x4
        )
    }

    /// Is the shading language type integral (including vectors)?
    // Port of: src/core/SkSLTypeShared.h#L118-L167 (chrome/m156)
    #[doc(alias = "SkSLTypeIsIntegralType")]
    #[must_use]
    pub const fn is_integral_type(self) -> bool {
        matches!(
            self,
            Self::Short
                | Self::Short2
                | Self::Short3
                | Self::Short4
                | Self::UShort
                | Self::UShort2
                | Self::UShort3
                | Self::UShort4
                | Self::Int
                | Self::Int2
                | Self::Int3
                | Self::Int4
                | Self::UInt
                | Self::UInt2
                | Self::UInt3
                | Self::UInt4
        )
    }

    /// If the type represents a single value or vector return the vector length; otherwise, -1.
    // Port of: src/core/SkSLTypeShared.h#L169-L227 (chrome/m156)
    #[doc(alias = "SkSLTypeVecLength")]
    #[must_use]
    pub const fn vec_length(self) -> i32 {
        match self {
            Self::Float
            | Self::Half
            | Self::Bool
            | Self::Short
            | Self::UShort
            | Self::Int
            | Self::UInt => 1,
            Self::Float2
            | Self::Half2
            | Self::Bool2
            | Self::Short2
            | Self::UShort2
            | Self::Int2
            | Self::UInt2 => 2,
            Self::Float3
            | Self::Half3
            | Self::Bool3
            | Self::Short3
            | Self::UShort3
            | Self::Int3
            | Self::UInt3 => 3,
            Self::Float4
            | Self::Half4
            | Self::Bool4
            | Self::Short4
            | Self::UShort4
            | Self::Int4
            | Self::UInt4 => 4,
            _ => -1,
        }
    }

    /// Is the shading language type supported as a uniform (it has a set function on the GL
    /// program data manager)? Almost "float or integral", but excludes non-full-precision ints.
    // Port of: src/core/SkSLTypeShared.h#L229-L261 (chrome/m156)
    #[doc(alias = "SkSLTypeCanBeUniformValue")]
    #[must_use]
    pub const fn can_be_uniform_value(self) -> bool {
        matches!(
            self,
            Self::Float
                | Self::Float2
                | Self::Float3
                | Self::Float4
                | Self::Float2x2
                | Self::Float3x3
                | Self::Float4x4
                | Self::Half
                | Self::Half2
                | Self::Half3
                | Self::Half4
                | Self::Half2x2
                | Self::Half3x3
                | Self::Half4x4
                | Self::Int
                | Self::Int2
                | Self::Int3
                | Self::Int4
                | Self::UInt
                | Self::UInt2
                | Self::UInt3
                | Self::UInt4
        )
    }

    /// Is the shading language type full precision?
    // Port of: src/core/SkSLTypeShared.cpp#L58-L109 (chrome/m156)
    #[doc(alias = "SkSLTypeIsFullPrecisionNumericType")]
    #[must_use]
    pub const fn is_full_precision_numeric_type(self) -> bool {
        matches!(
            self,
            Self::Int
                | Self::Int2
                | Self::Int3
                | Self::Int4
                | Self::UInt
                | Self::UInt2
                | Self::UInt3
                | Self::UInt4
                | Self::Float
                | Self::Float2
                | Self::Float3
                | Self::Float4
                | Self::Float2x2
                | Self::Float3x3
                | Self::Float4x4
        )
    }

    /// If the type represents a square matrix, return its size; otherwise, -1.
    // Port of: src/core/SkSLTypeShared.cpp#L111-L148 (chrome/m156)
    #[doc(alias = "SkSLTypeMatrixSize")]
    #[must_use]
    pub const fn matrix_size(self) -> i32 {
        match self {
            Self::Float2x2 | Self::Half2x2 => 2,
            Self::Float3x3 | Self::Half3x3 => 3,
            Self::Float4x4 | Self::Half4x4 => 4,
            _ => -1,
        }
    }

    /// The name of the type, as `SkSLTypeString` returns it.
    // Port of: src/core/SkSLTypeShared.cpp#L10-L57 (chrome/m156)
    #[doc(alias = "SkSLTypeString")]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        sksl_type_string(self)
    }
}
