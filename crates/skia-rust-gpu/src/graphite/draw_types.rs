// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawTypes.h, include/gpu/graphite/GraphiteTypes.h
// (DrawTypeFlags) and src/gpu/graphite/DescriptorData.h (PipelineStageFlags)

//! Primitive, vertex attribute, depth and stencil types shared by the draw path.

use bitflags::bitflags;

/// Geometric primitives used for drawing (`PrimitiveType`).
// Port of: src/gpu/graphite/DrawTypes.h#L22-L26 (chrome/m156)
#[doc(alias = "skgpu::graphite::PrimitiveType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveType {
    /// `kTriangles`.
    #[default]
    Triangles,
    /// `kTriangleStrip`.
    TriangleStrip,
    /// `kPoints`.
    Points,
}

/// Types used to describe the format of vertices in buffers (`VertexAttribType`).
// Port of: src/gpu/graphite/DrawTypes.h#L31-L62 (chrome/m156)
#[doc(alias = "skgpu::graphite::VertexAttribType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(non_camel_case_types)] // Names mirror Skia's enumerators (e.g. kUByte4_norm).
pub enum VertexAttribType {
    /// `kFloat`.
    #[default]
    Float = 0,
    /// `kFloat2`.
    Float2,
    /// `kFloat3`.
    Float3,
    /// `kFloat4`.
    Float4,
    /// `kHalf`.
    Half,
    /// `kHalf2`.
    Half2,
    /// `kHalf4`.
    Half4,
    /// `kInt2`: vector of 2 32-bit ints.
    Int2,
    /// `kInt3`: vector of 3 32-bit ints.
    Int3,
    /// `kInt4`: vector of 4 32-bit ints.
    Int4,
    /// `kUInt2`: vector of 2 32-bit unsigned ints.
    UInt2,
    /// `kByte`: signed byte.
    Byte,
    /// `kByte2`: vector of 2 8-bit signed bytes.
    Byte2,
    /// `kByte4`: vector of 4 8-bit signed bytes.
    Byte4,
    /// `kUByte`: unsigned byte.
    UByte,
    /// `kUByte2`: vector of 2 8-bit unsigned bytes.
    UByte2,
    /// `kUByte4`: vector of 4 8-bit unsigned bytes.
    UByte4,
    /// `kUByte_norm`: unsigned byte, e.g. coverage, 0 -> 0.0f, 255 -> 1.0f.
    UByteNorm,
    /// `kUByte4_norm`: vector of 4 unsigned bytes, e.g. colors, 0 -> 0.0f, 255 -> 1.0f.
    UByte4Norm,
    /// `kShort2`: vector of 2 16-bit shorts.
    Short2,
    /// `kShort4`: vector of 4 16-bit shorts.
    Short4,
    /// `kUShort2`: vector of 2 unsigned shorts. 0 -> 0, 65535 -> 65535.
    UShort2,
    /// `kUShort2_norm`: vector of 2 unsigned shorts. 0 -> 0.0f, 65535 -> 1.0f.
    UShort2Norm,
    /// `kInt`.
    Int,
    /// `kUInt`.
    UInt,
    /// `kUShort_norm`: unsigned short, e.g. depth, 0 -> 0.0f, 65535 -> 1.0f.
    UShortNorm,
    /// `kUShort4_norm`: vector of 4 unsigned shorts. 0 -> 0.0f, 65535 -> 1.0f.
    UShort4Norm,
}

/// `kVertexAttribTypeCount`.
// Port of: src/gpu/graphite/DrawTypes.h#L63 (chrome/m156)
#[doc(alias = "kVertexAttribTypeCount")]
pub const VERTEX_ATTRIB_TYPE_COUNT: usize = VertexAttribType::UShort4Norm as usize + 1;

impl VertexAttribType {
    /// `VertexAttribTypeSize()`: the size of the attribute type in bytes.
    // Port of: src/gpu/graphite/DrawTypes.h#L65-L120 (chrome/m156)
    #[doc(alias = "VertexAttribTypeSize")]
    #[must_use]
    // Several attribute types share a size (e.g. all the one-byte types), so arms repeat, as in
    // the C++ switch.
    #[allow(clippy::match_same_arms)]
    pub const fn size(self) -> usize {
        use std::mem::size_of;
        match self {
            Self::Float => size_of::<f32>(),
            Self::Float2 => 2 * size_of::<f32>(),
            Self::Float3 => 3 * size_of::<f32>(),
            Self::Float4 => 4 * size_of::<f32>(),
            Self::Half => size_of::<u16>(),
            Self::Half2 => 2 * size_of::<u16>(),
            Self::Half4 => 4 * size_of::<u16>(),
            Self::Int2 => 2 * size_of::<i32>(),
            Self::Int3 => 3 * size_of::<i32>(),
            Self::Int4 => 4 * size_of::<i32>(),
            Self::UInt2 => 2 * size_of::<u32>(),
            Self::Byte => size_of::<i8>(),
            Self::Byte2 => 2 * size_of::<i8>(),
            Self::Byte4 => 4 * size_of::<i8>(),
            Self::UByte => size_of::<i8>(),
            Self::UByte2 => 2 * size_of::<i8>(),
            Self::UByte4 => 4 * size_of::<i8>(),
            Self::UByteNorm => size_of::<i8>(),
            Self::UByte4Norm => 4 * size_of::<i8>(),
            Self::Short2 => 2 * size_of::<i16>(),
            Self::Short4 => 4 * size_of::<i16>(),
            Self::UShort2 | Self::UShort2Norm => 2 * size_of::<u16>(),
            Self::Int => size_of::<i32>(),
            Self::UInt => size_of::<u32>(),
            Self::UShortNorm => size_of::<u16>(),
            Self::UShort4Norm => 4 * size_of::<u16>(),
        }
    }
}

/// Comparison function for depth and stencil tests (`CompareOp`).
// Port of: src/gpu/graphite/DrawTypes.h#L144-L154 (chrome/m156)
#[doc(alias = "skgpu::graphite::CompareOp")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CompareOp {
    /// `kAlways`.
    #[default]
    Always,
    /// `kNever`.
    Never,
    /// `kGreater`.
    Greater,
    /// `kGEqual`.
    GEqual,
    /// `kLess`.
    Less,
    /// `kLEqual`.
    LEqual,
    /// `kEqual`.
    Equal,
    /// `kNotEqual`.
    NotEqual,
}

/// Stencil operation (`StencilOp`).
// Port of: src/gpu/graphite/DrawTypes.h#L156-L170 (chrome/m156)
#[doc(alias = "skgpu::graphite::StencilOp")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StencilOp {
    /// `kKeep`.
    #[default]
    Keep,
    /// `kZero`.
    Zero,
    /// `kReplace`: replace stencil value with reference (only the bits enabled in the write mask).
    Replace,
    /// `kInvert`.
    Invert,
    /// `kIncWrap`.
    IncWrap,
    /// `kDecWrap`.
    DecWrap,
    /// `kIncClamp`. Clamping occurs before the write mask.
    IncClamp,
    /// `kDecClamp`.
    DecClamp,
}

/// Barrier types. These are not used by all backends, but they affect how draws are grouped and
/// sorted (`BarrierType`).
// Port of: src/gpu/graphite/DrawTypes.h#L172-L176 (chrome/m156)
#[doc(alias = "skgpu::graphite::BarrierType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BarrierType {
    /// `kNone`.
    #[default]
    None,
    /// `kAdvancedNoncoherentBlend`.
    AdvancedNoncoherentBlend,
    /// `kReadDstFromInput`.
    ReadDstFromInput,
}

/// The uniform buffer slots a draw binds (`UniformSlot`).
// Port of: src/gpu/graphite/DrawTypes.h#L134-L139 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniformSlot")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UniformSlot {
    /// `kCombinedUniforms`: slot for paints and render step uniforms.
    CombinedUniforms,
    /// `kStorage`: storage buffer slot.
    Storage,
}

bitflags! {
    /// `RenderStateFlags`: which kind of vertex and instance data a pipeline state appends.
    // Port of: src/gpu/graphite/DrawTypes.h#L194-L201 (chrome/m156)
    #[doc(alias = "skgpu::graphite::RenderStateFlags")]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct RenderStateFlags: u8 {
        /// `kNone`.
        const NONE = 0b0000;
        /// `kFixed`: uses explicit `DrawWriter::draw` functions.
        const FIXED = 0b0001;
        /// `kAppendVertices`: appends vertices.
        const APPEND_VERTICES = 0b0010;
        /// `kAppendInstances`: appends instances with a static vertex count.
        const APPEND_INSTANCES = 0b0100;
        /// `kAppendDynamicInstances`: appends instances with a flexible vertex count.
        const APPEND_DYNAMIC_INSTANCES = 0b1000;
    }
}

bitflags! {
    /// `DstUsage`: how a pipeline depends on the prior values of the dst pixels.
    // Port of: src/gpu/graphite/DrawTypes.h#L178-L192 (chrome/m156)
    #[doc(alias = "skgpu::graphite::DstUsage")]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DstUsage: u8 {
        /// `kNone`: prior values of dst pixels have no effect on the final written color for any
        /// use of the pipeline.
        const NONE = 0b0000;
        /// `kDependsOnDst`: prior values of dst pixels can affect the final written color.
        const DEPENDS_ON_DST = 0b0001;
        /// `kDstReadRequired`: the prior values must be available in the fragment shader.
        const DST_READ_REQUIRED = 0b0010;
        /// `kAdvancedBlend`: the final color uses an advanced blend function, which may need
        /// barriers for hardware.
        const ADVANCED_BLEND = 0b0100;
        /// `kDstOnlyUsedByRenderer`: the only reason for `DEPENDS_ON_DST` is analytic coverage
        /// from the renderer.
        const DST_ONLY_USED_BY_RENDERER = 0b1000;
    }
}

bitflags! {
    /// `PipelineStageFlags`: the shader stages that use a storage buffer.
    // Port of: src/gpu/graphite/DescriptorData.h#L37-L42 (chrome/m156)
    #[doc(alias = "skgpu::graphite::PipelineStageFlags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct PipelineStageFlags: u8 {
        /// No stage.
        const NONE = 0;
        /// `kVertexShader`.
        const VERTEX_SHADER = 0b001;
        /// `kFragmentShader`.
        const FRAGMENT_SHADER = 0b010;
        /// `kCompute`.
        const COMPUTE = 0b100;
    }
}

bitflags! {
    /// `DrawTypeFlags` (public `skgpu::graphite::DrawTypeFlags`): the kinds of draw a precompiled
    /// `Renderer` serves.
    // Port of: include/gpu/graphite/GraphiteTypes.h#L252-L306 (chrome/m156)
    #[doc(alias = "skgpu::graphite::DrawTypeFlags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct DrawTypeFlags: u16 {
        /// `kNone`.
        const NONE = 0;
        /// `kBitmapText_Mask`: `BitmapTextRenderStep[mask]`.
        const BITMAP_TEXT_MASK = 1 << 0;
        /// `kBitmapText_LCD`: `BitmapTextRenderStep[LCD]`.
        const BITMAP_TEXT_LCD = 1 << 1;
        /// `kBitmapText_Color`: `BitmapTextRenderStep[color]`.
        const BITMAP_TEXT_COLOR = 1 << 2;
        /// `kSDFText`: `SDFTextRenderStep`.
        const SDF_TEXT = 1 << 3;
        /// `kSDFText_LCD`: `SDFTextLCDRenderStep`.
        const SDF_TEXT_LCD = 1 << 4;
        /// `kDrawVertices`: `VerticesRenderStep[*]`.
        const DRAW_VERTICES = 1 << 5;
        /// `kCircularArc`: `CircularArcRenderStep`.
        const CIRCULAR_ARC = 1 << 6;
        /// `kAnalyticRRect`: `AnalyticRRectRenderStep`.
        const ANALYTIC_RRECT = 1 << 7;
        /// `kPerEdgeAAQuad`: `PerEdgeAAQuadRenderStep`.
        const PER_EDGE_AA_QUAD = 1 << 8;
        /// `kNonAAFillRect`: `CoverBoundsRenderStep[NonAAFill]`.
        const NON_AA_FILL_RECT = 1 << 9;
        /// `kSimpleShape`: `kAnalyticRRect | kPerEdgeAAQuad | kNonAAFillRect`.
        const SIMPLE_SHAPE = Self::ANALYTIC_RRECT.bits()
            | Self::PER_EDGE_AA_QUAD.bits()
            | Self::NON_AA_FILL_RECT.bits();
        /// `kNonSimpleShape`: `CoverageMaskRenderStep`, the cover steps and the tessellation
        /// steps.
        const NON_SIMPLE_SHAPE = 1 << 10;
        /// `kDropShadows`: the blur render steps.
        const DROP_SHADOWS = 1 << 11;
        /// `kAnalyticClip`: combined with the primary draw type for analytic clip pipelines.
        const ANALYTIC_CLIP = 1 << 12;
        /// `kDrawMesh`: `MeshRenderStep`.
        const DRAW_MESH = 1 << 13;
        /// `kSparseStrips`: the sparse strips render steps.
        const SPARSE_STRIPS = 1 << 14;
    }
}

/// `CompareOp` count (`kCompareOpCount`).
// Port of: src/gpu/graphite/DrawTypes.h#L153 (chrome/m156)
#[doc(alias = "kCompareOpCount")]
pub const COMPARE_OP_COUNT: usize = CompareOp::NotEqual as usize + 1;

/// `StencilOp` count (`kStencilOpCount`).
// Port of: src/gpu/graphite/DrawTypes.h#L171 (chrome/m156)
#[doc(alias = "kStencilOpCount")]
pub const STENCIL_OP_COUNT: usize = StencilOp::DecClamp as usize + 1;

/// Stencil and depth settings of a `RenderStep` (`DepthStencilSettings`).
// Port of: src/gpu/graphite/DrawTypes.h#L203-L266 (chrome/m156)
#[doc(alias = "skgpu::graphite::DepthStencilSettings")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepthStencilSettings {
    /// `fFrontStencil`.
    pub front_stencil: Face,
    /// `fBackStencil`.
    pub back_stencil: Face,
    /// `fStencilReferenceValue`.
    pub stencil_reference_value: u32,
    /// `fDepthCompareOp`.
    pub depth_compare_op: CompareOp,
    /// `fStencilTestEnabled`.
    pub stencil_test_enabled: bool,
    /// `fDepthTestEnabled`.
    pub depth_test_enabled: bool,
    /// `fDepthWriteEnabled`.
    pub depth_write_enabled: bool,
}

impl Default for DepthStencilSettings {
    // The C++ member initializers: a default `DepthStencilSettings` has `kAlways` depth compare
    // and every test disabled.
    fn default() -> Self {
        Self {
            front_stencil: Face::default(),
            back_stencil: Face::default(),
            stencil_reference_value: 0,
            depth_compare_op: CompareOp::Always,
            stencil_test_enabled: false,
            depth_test_enabled: false,
            depth_write_enabled: false,
        }
    }
}

impl DepthStencilSettings {
    /// The `DepthStencilSettings(front, back, stencilRef, stencilTest, depthCompare, depthTest,
    /// depthWrite)` constructor.
    // Port of: src/gpu/graphite/DrawTypes.h#L224-L236 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Mirrors the C++ constructor's parameter list.
    pub const fn new(
        front: Face,
        back: Face,
        stencil_ref: u32,
        stencil_test: bool,
        depth_compare: CompareOp,
        depth_test: bool,
        depth_write: bool,
    ) -> Self {
        Self {
            front_stencil: front,
            back_stencil: back,
            stencil_reference_value: stencil_ref,
            depth_compare_op: depth_compare,
            stencil_test_enabled: stencil_test,
            depth_test_enabled: depth_test,
            depth_write_enabled: depth_write,
        }
    }
}

/// Per-face stencil settings (`DepthStencilSettings::Face`).
// Port of: src/gpu/graphite/DrawTypes.h#L205-L240 (chrome/m156)
#[doc(alias = "DepthStencilSettings::Face")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    /// `fStencilFailOp`.
    pub stencil_fail_op: StencilOp,
    /// `fDepthFailOp`.
    pub depth_fail_op: StencilOp,
    /// `fDepthStencilPassOp`.
    pub depth_stencil_pass_op: StencilOp,
    /// `fCompareOp`.
    pub compare_op: CompareOp,
    /// `fReadMask`.
    pub read_mask: u32,
    /// `fWriteMask`.
    pub write_mask: u32,
}

impl Default for Face {
    fn default() -> Self {
        Self {
            stencil_fail_op: StencilOp::Keep,
            depth_fail_op: StencilOp::Keep,
            depth_stencil_pass_op: StencilOp::Keep,
            compare_op: CompareOp::Always,
            read_mask: 0xffff_ffff,
            write_mask: 0xffff_ffff,
        }
    }
}

impl Face {
    /// The `Face(stencilFail, depthFail, dsPass, compare, readMask, writeMask)` constructor.
    // Port of: src/gpu/graphite/DrawTypes.h#L213-L222 (chrome/m156)
    #[must_use]
    pub const fn new(
        stencil_fail: StencilOp,
        depth_fail: StencilOp,
        ds_pass: StencilOp,
        compare: CompareOp,
        read_mask: u32,
        write_mask: u32,
    ) -> Self {
        Self {
            stencil_fail_op: stencil_fail,
            depth_fail_op: depth_fail,
            depth_stencil_pass_op: ds_pass,
            compare_op: compare,
            read_mask,
            write_mask,
        }
    }
}
