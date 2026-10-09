// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Renderer.h (RenderStep), src/gpu/graphite/Renderer.cpp

//! [`RenderStep`]: one step of a technique for rasterizing a draw. Each step fixes a vertex
//! layout, writes the per-draw vertex and instance data, and supplies the `SkSL` that runs on the
//! GPU for it.

use std::fmt::Debug;

use bitflags::bitflags;
use skia_rust_core::rect::IRect;

use crate::graphite::attribute::{Attribute, Varying};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{DepthStencilSettings, PipelineStageFlags, PrimitiveType};
use crate::graphite::draw_writer::DrawWriter;
use crate::graphite::geom::rect::Rect;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::graphite::uniform_manager::UniformOffsetCalculator;

/// The coverage a `RenderStep` adds to the fragment shader (`enum class Coverage`).
// Port of: src/gpu/graphite/Renderer.h#L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::Coverage")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Coverage {
    /// `kNone`.
    #[default]
    None,
    /// `kSingleChannel`.
    SingleChannel,
    /// `kLCD`.
    Lcd,
}

bitflags! {
    /// Properties of a `RenderStep` (`RenderStep::Flags`).
    // Port of: src/gpu/graphite/Renderer.h#L186-L206 (chrome/m156), the `Flags` enum
    #[doc(alias = "skgpu::graphite::RenderStep::Flags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct RenderStepFlags: u16 {
        /// `kNone`.
        const NONE = 0x0000;
        /// `kFixed`: uses explicit `DrawWriter::draw` functions.
        const FIXED = 0x0001;
        /// `kAppendVertices`: appends vertices.
        const APPEND_VERTICES = 0x0002;
        /// `kAppendInstances`: appends instances with a static vertex count.
        const APPEND_INSTANCES = 0x0004;
        /// `kAppendDynamicInstances`: appends instances with a flexible vertex count.
        const APPEND_DYNAMIC_INSTANCES = 0x0008;
        /// `kRequiresMSAA`: MSAA is required for anti-aliasing.
        const REQUIRES_MSAA = 0x0010;
        /// `kPerformsShading`: this step is responsible for shading and color output.
        const PERFORMS_SHADING = 0x0020;
        /// `kHasTextures`: adds textures via `texturesAndSamplersSkSL()`.
        const HAS_TEXTURES = 0x0040;
        /// `kEmitsCoverage`: adds analytic coverage via `fragmentCoverageSkSL()`.
        const EMITS_COVERAGE = 0x0080;
        /// `kLCDCoverage`: the added analytic coverage is LCD, not single channel.
        const LCD_COVERAGE = 0x0100;
        /// `kEmitsPrimitiveColor`: injects primitive color via `fragmentColorSkSL()`.
        const EMITS_PRIMITIVE_COLOR = 0x0200;
        /// `kOutsetBoundsForAA`: drawn geometry is outset beyond the shape's bounds for AA.
        const OUTSET_BOUNDS_FOR_AA = 0x0400;
        /// `kUseNonAAInnerFill`: opts into `Device` recording extra inner fill draws.
        const USE_NON_AA_INNER_FILL = 0x0800;
        /// `kIgnoreInverseFill`: rasterization treats all shapes as non-inverted for the scissor.
        const IGNORE_INVERSE_FILL = 0x1000;
        /// `kInverseFillsScissor`: rasterization of inverse fills scissors geometrically.
        const INVERSE_FILLS_SCISSOR = 0x2000;
        /// `kVsUsesStorage`: the vertex shader requires storage buffer access.
        const VS_USES_STORAGE = 0x4000;
        /// `kFsUsesStorage`: the fragment shader requires storage buffer access.
        const FS_USES_STORAGE = 0x8000;
    }
}

/// The identifier of each `RenderStep` (`RenderStep::RenderStepID`). Variants of one step are
/// named `Step_Variant`, as in `SKGPU_RENDERSTEP_TYPES`.
// Port of: src/gpu/graphite/Renderer.h#L88-L125 (chrome/m156), the SKGPU_RENDERSTEP_TYPES list
#[doc(alias = "skgpu::graphite::RenderStep::RenderStepID")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
#[allow(non_camel_case_types)] // Variant names mirror Skia's `k<Step>_<Variant>` enumerators.
pub enum RenderStepID {
    /// `kInvalid`.
    #[default]
    Invalid,
    /// `kCircularArc`.
    CircularArc,
    /// `kAnalyticRRect`.
    AnalyticRRect,
    /// `kAnalyticBlur`.
    AnalyticBlur,
    /// `kAnalyticRRectBlur`.
    AnalyticRRectBlur,
    /// `kPerEdgeAAQuad`.
    PerEdgeAAQuad,
    /// `kCoverBounds_NonAAFill`.
    CoverBounds_NonAAFill,
    /// `kCoverBounds_RegularCover`.
    CoverBounds_RegularCover,
    /// `kCoverBounds_InverseCover`.
    CoverBounds_InverseCover,
    /// `kCoverageMask`.
    CoverageMask,
    /// `kBitmapText_Mask`.
    BitmapText_Mask,
    /// `kBitmapText_LCD`.
    BitmapText_LCD,
    /// `kBitmapText_Color`.
    BitmapText_Color,
    /// `kMiddleOutFan_EvenOdd`.
    MiddleOutFan_EvenOdd,
    /// `kMiddleOutFan_Winding`.
    MiddleOutFan_Winding,
    /// `kSDFTextLCD`.
    SDFTextLCD,
    /// `kSDFText`.
    SDFText,
    /// `kTessellateCurves_EvenOdd`.
    TessellateCurves_EvenOdd,
    /// `kTessellateCurves_Winding`.
    TessellateCurves_Winding,
    /// `kTessellateStrokes_Fill`.
    TessellateStrokes_Fill,
    /// `kTessellateStrokes_InverseFill`.
    TessellateStrokes_InverseFill,
    /// `kTessellateWedges_Convex`.
    TessellateWedges_Convex,
    /// `kTessellateWedges_EvenOdd`.
    TessellateWedges_EvenOdd,
    /// `kTessellateWedges_Winding`.
    TessellateWedges_Winding,
    /// `kVertices_Pos`.
    Vertices_Pos,
    /// `kVertices_PosColor`.
    Vertices_PosColor,
    /// `kVertices_PosTexCoords`.
    Vertices_PosTexCoords,
    /// `kVertices_PosColorTexCoords`.
    Vertices_PosColorTexCoords,
    /// `kMesh`.
    Mesh,
    /// `kEndCap`.
    EndCap,
    /// `kWideTile` (also `kLast`).
    WideTile,
}

impl RenderStepID {
    /// `kLast`: the last valid ID.
    // Port of: src/gpu/graphite/Renderer.h#L185 (chrome/m156)
    #[doc(alias = "kLast")]
    pub const LAST: Self = Self::WideTile;

    /// `RenderStep::RenderStepName(id)`: `"Subclass[variant]"` for the variants, `"Subclass"`
    /// otherwise.
    // Port of: src/gpu/graphite/Renderer.cpp#L109-L120 (chrome/m156)
    #[doc(alias = "RenderStepName")]
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::CircularArc => "CircularArcRenderStep",
            Self::AnalyticRRect => "AnalyticRRectRenderStep",
            Self::AnalyticBlur => "AnalyticBlurRenderStep",
            Self::AnalyticRRectBlur => "AnalyticRRectBlurRenderStep",
            Self::PerEdgeAAQuad => "PerEdgeAAQuadRenderStep",
            Self::CoverBounds_NonAAFill => "CoverBoundsRenderStep[NonAAFill]",
            Self::CoverBounds_RegularCover => "CoverBoundsRenderStep[RegularCover]",
            Self::CoverBounds_InverseCover => "CoverBoundsRenderStep[InverseCover]",
            Self::CoverageMask => "CoverageMaskRenderStep",
            Self::BitmapText_Mask => "BitmapTextRenderStep[Mask]",
            Self::BitmapText_LCD => "BitmapTextRenderStep[LCD]",
            Self::BitmapText_Color => "BitmapTextRenderStep[Color]",
            Self::MiddleOutFan_EvenOdd => "MiddleOutFanRenderStep[EvenOdd]",
            Self::MiddleOutFan_Winding => "MiddleOutFanRenderStep[Winding]",
            Self::SDFTextLCD => "SDFTextLCDRenderStep",
            Self::SDFText => "SDFTextRenderStep",
            Self::TessellateCurves_EvenOdd => "TessellateCurvesRenderStep[EvenOdd]",
            Self::TessellateCurves_Winding => "TessellateCurvesRenderStep[Winding]",
            Self::TessellateStrokes_Fill => "TessellateStrokesRenderStep[Fill]",
            Self::TessellateStrokes_InverseFill => "TessellateStrokesRenderStep[InverseFill]",
            Self::TessellateWedges_Convex => "TessellateWedgesRenderStep[Convex]",
            Self::TessellateWedges_EvenOdd => "TessellateWedgesRenderStep[EvenOdd]",
            Self::TessellateWedges_Winding => "TessellateWedgesRenderStep[Winding]",
            Self::Vertices_Pos => "VerticesRenderStep[Pos]",
            Self::Vertices_PosColor => "VerticesRenderStep[PosColor]",
            Self::Vertices_PosTexCoords => "VerticesRenderStep[PosTexCoords]",
            Self::Vertices_PosColorTexCoords => "VerticesRenderStep[PosColorTexCoords]",
            Self::Mesh => "MeshRenderStep",
            Self::EndCap => "EndCapRenderStep",
            Self::WideTile => "WideTileRenderStep",
        }
    }

    /// `RenderStep::IsValidRenderStepID(id)`.
    // Port of: src/gpu/graphite/Renderer.cpp#L122-L125 (chrome/m156)
    #[doc(alias = "IsValidRenderStepID")]
    #[must_use]
    pub const fn is_valid(id: u32) -> bool {
        id > RenderStepID::Invalid as u32 && id <= RenderStepID::LAST as u32
    }
}

/// `kNumRenderSteps`.
// Port of: src/gpu/graphite/Renderer.h#L212 (chrome/m156)
#[doc(alias = "kNumRenderSteps")]
pub const NUM_RENDER_STEPS: usize = RenderStepID::LAST as usize + 1;

/// `kRenderStepIDVersion`.
// Port of: src/gpu/graphite/Renderer.h#L211 (chrome/m156)
#[doc(alias = "kRenderStepIDVersion")]
pub const RENDER_STEP_ID_VERSION: i32 = 2;

/// The data every `RenderStep` carries (the private members of `RenderStep`). A concrete step
/// holds one of these and returns it from [`RenderStep::base`].
// Port of: src/gpu/graphite/Renderer.h#L213-L268 (chrome/m156), the RenderStep members
#[derive(Clone, Debug)]
pub struct RenderStepBase {
    render_step_id: RenderStepID,
    flags: RenderStepFlags,
    storage_buffer_stages: PipelineStageFlags,
    primitive_type: PrimitiveType,
    depth_stencil_settings: DepthStencilSettings,
    uniforms: Vec<Uniform>,
    static_attrs: Vec<Attribute>,
    append_attrs: Vec<Attribute>,
    storage_uniforms: Vec<Uniform>,
    varyings: Vec<Varying>,
    uniform_alignment: i32,
    static_data_stride: usize,
    append_data_stride: usize,
    storage_uniform_stride: usize,
    storage_uniform_alignment: usize,
}

impl RenderStepBase {
    /// The `RenderStep(layout, id, flags, uniforms, primitiveType, depthStencil, staticAttrs,
    /// appendAttrs, storageUniforms, varyings)` constructor. The layout fixes the uniform and
    /// storage uniform alignments, and the attribute sizes fix the strides.
    ///
    /// # Panics
    /// If a computed uniform or storage alignment is negative, which the layout calculator does
    /// not produce.
    // Port of: src/gpu/graphite/Renderer.cpp#L14-L80 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Mirrors the C++ constructor's parameter list.
    pub fn new(
        layout: Layout,
        render_step_id: RenderStepID,
        flags: RenderStepFlags,
        uniforms: &[Uniform],
        primitive_type: PrimitiveType,
        depth_stencil_settings: DepthStencilSettings,
        static_attrs: &[Attribute],
        append_attrs: &[Attribute],
        storage_uniforms: &[Uniform],
        varyings: &[Varying],
    ) -> Self {
        let mut storage_buffer_stages = PipelineStageFlags::NONE;
        if flags.contains(RenderStepFlags::VS_USES_STORAGE) {
            storage_buffer_stages |= PipelineStageFlags::VERTEX_SHADER;
        }
        if flags.contains(RenderStepFlags::FS_USES_STORAGE) {
            storage_buffer_stages |= PipelineStageFlags::FRAGMENT_SHADER;
        }
        debug_assert!(
            storage_uniforms.is_empty()
                || storage_buffer_stages.intersects(
                    PipelineStageFlags::VERTEX_SHADER | PipelineStageFlags::FRAGMENT_SHADER
                )
        );

        let static_data_stride = static_attrs.iter().map(Attribute::size_align4).sum();
        let append_data_stride = append_attrs.iter().map(Attribute::size_align4).sum();

        let mut storage_uniform_stride = 0;
        let mut storage_uniform_alignment = 1;
        if !storage_uniforms.is_empty() {
            // Storage uniforms are backed by a storage buffer or emulated via a fallback texture.
            // In both cases the struct layout follows Std430.
            let mut calculator = UniformOffsetCalculator::for_struct(Layout::Std430);
            for u in storage_uniforms {
                calculator.advance_offset(u.ty(), u.count());
            }
            storage_uniform_alignment =
                usize::try_from(calculator.required_alignment()).expect("alignment is positive");
            storage_uniform_stride = skia_align_to(
                usize::try_from(calculator.size()).expect("size is non-negative"),
                storage_uniform_alignment,
            );
        }

        let mut calculator = UniformOffsetCalculator::for_top_level(layout, 0);
        for u in uniforms {
            calculator.advance_offset(u.ty(), u.count());
        }
        let uniform_alignment = calculator.required_alignment();

        Self {
            render_step_id,
            flags,
            storage_buffer_stages,
            primitive_type,
            depth_stencil_settings,
            uniforms: uniforms.to_vec(),
            static_attrs: static_attrs.to_vec(),
            append_attrs: append_attrs.to_vec(),
            storage_uniforms: storage_uniforms.to_vec(),
            varyings: varyings.to_vec(),
            uniform_alignment,
            static_data_stride,
            append_data_stride,
            storage_uniform_stride,
            storage_uniform_alignment,
        }
    }

    /// `renderStepID()`.
    // Port of: src/gpu/graphite/Renderer.h#L120 (chrome/m156)
    #[must_use]
    pub const fn render_step_id(&self) -> RenderStepID {
        self.render_step_id
    }

    /// The flags given to the constructor (`fFlags`).
    // Port of: src/gpu/graphite/Renderer.h#L213 (chrome/m156), the fFlags member
    #[must_use]
    pub const fn flags(&self) -> RenderStepFlags {
        self.flags
    }

    /// `storageBufferStages()`.
    // Port of: src/gpu/graphite/Renderer.h#L106 (chrome/m156)
    #[must_use]
    pub const fn storage_buffer_stages(&self) -> PipelineStageFlags {
        self.storage_buffer_stages
    }

    /// `primitiveType()`.
    // Port of: src/gpu/graphite/Renderer.h#L144 (chrome/m156)
    #[must_use]
    pub const fn primitive_type(&self) -> PrimitiveType {
        self.primitive_type
    }

    /// `staticDataStride()`.
    // Port of: src/gpu/graphite/Renderer.h#L145 (chrome/m156)
    #[must_use]
    pub const fn static_data_stride(&self) -> usize {
        self.static_data_stride
    }

    /// `storageUniformStride()`.
    // Port of: src/gpu/graphite/Renderer.h#L147 (chrome/m156)
    #[must_use]
    pub const fn storage_uniform_stride(&self) -> usize {
        self.storage_uniform_stride
    }

    /// `storageUniformAlignment()`.
    // Port of: src/gpu/graphite/Renderer.h#L148 (chrome/m156)
    #[must_use]
    pub const fn storage_uniform_alignment(&self) -> usize {
        self.storage_uniform_alignment
    }

    /// `uniformAlignment()`.
    // Port of: src/gpu/graphite/Renderer.h#L150 (chrome/m156)
    #[must_use]
    pub const fn uniform_alignment(&self) -> i32 {
        self.uniform_alignment
    }

    /// `uniforms()`.
    // Port of: src/gpu/graphite/Renderer.h#L172 (chrome/m156)
    #[must_use]
    pub fn uniforms(&self) -> &[Uniform] {
        &self.uniforms
    }

    /// `staticAttributes()`.
    // Port of: src/gpu/graphite/Renderer.h#L173 (chrome/m156)
    #[must_use]
    pub fn static_attributes(&self) -> &[Attribute] {
        &self.static_attrs
    }

    /// `appendAttributes()`.
    // Port of: src/gpu/graphite/Renderer.h#L174 (chrome/m156)
    #[must_use]
    pub fn append_attributes(&self) -> &[Attribute] {
        &self.append_attrs
    }

    /// `storageUniforms()`.
    // Port of: src/gpu/graphite/Renderer.h#L175 (chrome/m156)
    #[must_use]
    pub fn storage_uniforms(&self) -> &[Uniform] {
        &self.storage_uniforms
    }

    /// `varyings()`.
    // Port of: src/gpu/graphite/Renderer.h#L176 (chrome/m156)
    #[must_use]
    pub fn varyings(&self) -> &[Varying] {
        &self.varyings
    }

    /// `depthStencilSettings()`.
    // Port of: src/gpu/graphite/Renderer.h#L177 (chrome/m156)
    #[must_use]
    pub const fn depth_stencil_settings(&self) -> &DepthStencilSettings {
        &self.depth_stencil_settings
    }
}

/// `SkAlignTo(size, alignment)` for `usize`.
// Port of: include/private/base/SkAlign.h (SkAlignTo), used by the RenderStep constructor
fn skia_align_to(size: usize, alignment: usize) -> usize {
    size.div_ceil(alignment) * alignment
}

/// One step of a `Renderer` (`skgpu::graphite::RenderStep`). Implementors hold a
/// [`RenderStepBase`] and override the methods that depend on the step.
///
/// Parameters that Skia passes but that no ported step reads yet are omitted here:
/// `StorageContext*` (G10a) on `writeVertices` and `RootNodesInfo` (`ShaderInfo`, G6) on
/// `vertexSkSL`. They come back with the code that needs them.
// Port of: src/gpu/graphite/Renderer.h#L126-L210 (chrome/m156)
#[doc(alias = "skgpu::graphite::RenderStep")]
pub trait RenderStep: Send + Sync + Debug {
    /// The data shared by every step.
    fn base(&self) -> &RenderStepBase;

    /// `writeVertices(writer, storageContext, params, ssboIndex)`. Records the vertex and
    /// instance data of one draw. The writer is configured with this step's strides and
    /// primitive type.
    // Port of: src/gpu/graphite/Renderer.h#L140-L144 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32);

    /// `writeUniformsAndTextures(params, gatherer)`. Writes the uniform values, textures and
    /// samplers. The gatherer de-duplicates uniforms across draws of this step.
    // Port of: src/gpu/graphite/Renderer.h#L150-L151 (chrome/m156)
    fn write_uniforms_and_textures(&self, params: &DrawParams, gatherer: &mut PipelineDataGatherer);

    /// `vertexSkSL(...)`: the body of the vertex function. It defines a `float4 devPosition` and
    /// writes the already-defined `float2 stepLocalCoords`.
    // Port of: src/gpu/graphite/Renderer.h#L163 (chrome/m156)
    fn vertex_sksl(&self) -> String;

    /// `fragmentCoverageSkSL()`: writes its coverage into `half4 outputCoverage`, splatted into
    /// all four channels. Only defined when the step has coverage.
    // Port of: src/gpu/graphite/Renderer.h#L172 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        ""
    }

    /// `appendDataStride(params)`. Steps whose stride depends on the draw override this.
    // Port of: src/gpu/graphite/Renderer.h#L113 (chrome/m156)
    fn append_data_stride(&self, _params: &DrawParams) -> usize {
        self.base().append_data_stride
    }

    /// `getScissor(params, currentScissor, deviceBounds)`. Returns `None` when the current
    /// scissor already suits the draw, and otherwise the scissor the draws need.
    // Port of: src/gpu/graphite/Renderer.cpp#L43-L91 (chrome/m156)
    fn get_scissor(
        &self,
        params: &DrawParams,
        current_scissor: IRect,
        device_bounds: IRect,
    ) -> Option<IRect> {
        if current_scissor == params.scissor() {
            return None; // Trivially no change in scissor state is required.
        }

        let mut draw_bounds = params.draw_bounds();
        if params.geometry().is_shape() && params.geometry().shape().inverted() {
            // For inverse filled shapes, the scissor is handled in unique ways.
            if self
                .base()
                .flags
                .contains(RenderStepFlags::INVERSE_FILLS_SCISSOR)
            {
                // The step respects the scissor geometrically, so as long as the current scissor
                // doesn't interfere, no state change is needed.
                if irect_contains(current_scissor, params.scissor()) {
                    return None;
                }
                // This draw needs no scissor at all, so use the device bounds.
                return Some(device_bounds);
            }
            if self
                .base()
                .flags
                .contains(RenderStepFlags::IGNORE_INVERSE_FILL)
            {
                // params.draw_bounds() fills the scissor from the inverse fill rule, but the
                // scissor applies as if it were a regular fill.
                draw_bounds = params.transformed_shape_bounds();
                draw_bounds.intersect(Rect::from_sk_irect(&params.scissor()));
            }
            // Else leave draw_bounds filling the original scissor.
        }

        // Draws unaffected by the clip stack have a scissor matching the device bounds. If their
        // transformed shape bounds, clipped to the current scissor, equal their draw bounds, no
        // state change is needed.
        let mut current_clipped_bounds = params.transformed_shape_bounds();
        current_clipped_bounds.intersect(Rect::from_sk_irect(&current_scissor));
        if current_clipped_bounds == draw_bounds {
            return None;
        }
        if draw_bounds == params.transformed_shape_bounds() {
            // A change is needed, but the registered scissor is a no-op, so use the device bounds
            // as a canonical scissor.
            return Some(device_bounds);
        }
        Some(params.scissor())
    }

    /// `coverage()`: the coverage this step emits, from its flags.
    // Port of: src/gpu/graphite/Renderer.cpp#L93-L98 (chrome/m156), RenderStep::GetCoverage
    fn coverage(&self) -> Coverage {
        get_coverage(self.base().flags)
    }

    /// `name()`.
    // Port of: src/gpu/graphite/Renderer.h#L116 (chrome/m156)
    fn name(&self) -> &'static str {
        self.base().render_step_id.name()
    }

    /// `renderStepID()`.
    // Port of: src/gpu/graphite/Renderer.h#L120 (chrome/m156)
    fn render_step_id(&self) -> RenderStepID {
        self.base().render_step_id
    }

    /// `requiresMSAA()`.
    // Port of: src/gpu/graphite/Renderer.h#L82 (chrome/m156)
    fn requires_msaa(&self) -> bool {
        self.base().flags.contains(RenderStepFlags::REQUIRES_MSAA)
    }

    /// `performsShading()`.
    // Port of: src/gpu/graphite/Renderer.h#L83 (chrome/m156)
    fn performs_shading(&self) -> bool {
        self.base()
            .flags
            .contains(RenderStepFlags::PERFORMS_SHADING)
    }

    /// `hasTextures()`.
    // Port of: src/gpu/graphite/Renderer.h#L84 (chrome/m156)
    fn has_textures(&self) -> bool {
        self.base().flags.contains(RenderStepFlags::HAS_TEXTURES)
    }

    /// `emitsPrimitiveColor()`.
    // Port of: src/gpu/graphite/Renderer.h#L85 (chrome/m156)
    fn emits_primitive_color(&self) -> bool {
        self.base()
            .flags
            .contains(RenderStepFlags::EMITS_PRIMITIVE_COLOR)
    }

    /// `outsetBoundsForAA()`.
    // Port of: src/gpu/graphite/Renderer.h#L86 (chrome/m156)
    fn outset_bounds_for_aa(&self) -> bool {
        self.base()
            .flags
            .contains(RenderStepFlags::OUTSET_BOUNDS_FOR_AA)
    }

    /// `useNonAAInnerFill()`.
    // Port of: src/gpu/graphite/Renderer.h#L87 (chrome/m156)
    fn use_non_aa_inner_fill(&self) -> bool {
        self.base()
            .flags
            .contains(RenderStepFlags::USE_NON_AA_INNER_FILL)
    }

    /// `appendsVertices()`.
    // Port of: src/gpu/graphite/Renderer.h#L88 (chrome/m156)
    fn appends_vertices(&self) -> bool {
        self.base().flags.contains(RenderStepFlags::APPEND_VERTICES)
    }

    /// `usesUniformsInFragmentSkSL()`: by default, steps use their uniforms for coverage or
    /// primitive colors.
    // Port of: src/gpu/graphite/Renderer.h#L101-L104 (chrome/m156)
    fn uses_uniforms_in_fragment_sksl(&self) -> bool {
        self.coverage() != Coverage::None || self.emits_primitive_color()
    }
}

/// `RenderStep::GetCoverage(flags)`.
// Port of: src/gpu/graphite/Renderer.cpp#L93-L98 (chrome/m156)
#[must_use]
pub fn get_coverage(flags: RenderStepFlags) -> Coverage {
    if !flags.contains(RenderStepFlags::EMITS_COVERAGE) {
        Coverage::None
    } else if flags.contains(RenderStepFlags::LCD_COVERAGE) {
        Coverage::Lcd
    } else {
        Coverage::SingleChannel
    }
}

/// `SkIRect::contains(const SkIRect& r)`: `r` is inside `outer`, and neither is empty.
// Port of: include/core/SkRect.h#L480-L484 (chrome/m156)
#[must_use]
pub fn irect_contains(outer: IRect, r: IRect) -> bool {
    !r.is_empty()
        && !outer.is_empty()
        && outer.left <= r.left
        && outer.top <= r.top
        && outer.right >= r.right
        && outer.bottom >= r.bottom
}
