// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/VerticesRenderStep.h, VerticesRenderStep.cpp

//! [`VerticesRenderStep`]: draws an `SkVertices` (positions, and optionally colors and texture
//! coordinates). The vertices are transformed by the GPU, so the uniforms are the local-to-device
//! matrix and the depth, and each vertex is appended to the draw's vertex data.

use skia_rust_core::color::Color;
use skia_rust_core::vert_state::VertState;

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Vertices};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LEQUAL_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// `SK_ColorTRANSPARENT`, the color that a vertex without colors reads.
// Port of: include/core/SkColor.h (SK_ColorTRANSPARENT)
const SK_COLOR_TRANSPARENT: Color = Color::TRANSPARENT;

const POSITION_ATTR: Attribute =
    Attribute::new("position", VertexAttribType::Float2, SkSLType::Float2);
const TEX_COORD_ATTR: Attribute =
    Attribute::new("texCoords", VertexAttribType::Float2, SkSLType::Float2);
const COLOR_ATTR: Attribute =
    Attribute::new("vertColor", VertexAttribType::UByte4Norm, SkSLType::Half4);
const SSBO_INDEX_ATTR: Attribute =
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt);

// Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L13-L35 (chrome/m156), the attribute
// lists indexed by `2 * hasTexCoords + hasColor`.
const ATTRIBUTES_POSITION_ONLY: [Attribute; 2] = [POSITION_ATTR, SSBO_INDEX_ATTR];
const ATTRIBUTES_COLOR: [Attribute; 3] = [POSITION_ATTR, COLOR_ATTR, SSBO_INDEX_ATTR];
const ATTRIBUTES_TEX_COORDS: [Attribute; 3] = [POSITION_ATTR, TEX_COORD_ATTR, SSBO_INDEX_ATTR];
const ATTRIBUTES_COLOR_AND_TEX_COORDS: [Attribute; 4] =
    [POSITION_ATTR, COLOR_ATTR, TEX_COORD_ATTR, SSBO_INDEX_ATTR];
const ATTRIBUTES: [&[Attribute]; 4] = [
    &ATTRIBUTES_POSITION_ONLY,
    &ATTRIBUTES_COLOR,
    &ATTRIBUTES_TEX_COORDS,
    &ATTRIBUTES_COLOR_AND_TEX_COORDS,
];

// Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L37-L44 (chrome/m156), the varyings
const VARYINGS_NONE: [Varying; 0] = [];
const VARYINGS_COLOR: [Varying; 1] = [Varying::new(
    "color",
    SkSLType::Half4,
    Interpolation::Perspective,
)];

// Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L100-L104 (chrome/m156), the uniforms
const UNIFORMS: [Uniform; 2] = [
    Uniform::new("localToDevice", SkSLType::Float4x4),
    Uniform::new("depth", SkSLType::Float),
];

/// The `VerticesRenderStep`: one of four variants, by whether it has colors and texture
/// coordinates.
// Port of: src/gpu/graphite/render/VerticesRenderStep.h#L16-L44 (chrome/m156)
#[doc(alias = "skgpu::graphite::VerticesRenderStep")]
#[derive(Debug)]
pub struct VerticesRenderStep {
    base: RenderStepBase,
    has_color: bool,
    has_tex_coords: bool,
}

impl VerticesRenderStep {
    /// `VerticesRenderStep(layout, hasColor, hasTexCoords)`.
    // Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L93-L115 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, has_color: bool, has_tex_coords: bool) -> Self {
        let render_step_id = match (has_color, has_tex_coords) {
            (true, true) => RenderStepID::Vertices_PosColorTexCoords,
            (true, false) => RenderStepID::Vertices_PosColor,
            (false, true) => RenderStepID::Vertices_PosTexCoords,
            (false, false) => RenderStepID::Vertices_Pos,
        };
        let color_flag = if has_color {
            RenderStepFlags::EMITS_PRIMITIVE_COLOR
        } else {
            RenderStepFlags::NONE
        };
        let varyings: &[Varying] = if has_color {
            &VARYINGS_COLOR
        } else {
            &VARYINGS_NONE
        };
        let append_attrs = ATTRIBUTES[2 * usize::from(has_tex_coords) + usize::from(has_color)];
        Self {
            base: RenderStepBase::new(
                layout,
                render_step_id,
                color_flag | RenderStepFlags::PERFORMS_SHADING | RenderStepFlags::APPEND_VERTICES,
                &UNIFORMS,
                PrimitiveType::Triangles,
                DIRECT_DEPTH_LEQUAL_PASS,
                &[],
                append_attrs,
                &[],
                varyings,
            ),
            has_color,
            has_tex_coords,
        }
    }
}

impl RenderStep for VerticesRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L117-L144 (chrome/m156)
    fn vertex_sksl(&self, _roots: &crate::graphite::paint_params_key::RootNodesInfo) -> String {
        match (self.has_color, self.has_tex_coords) {
            (true, true) => concat!(
                "color = half4(vertColor.bgr * vertColor.a, vertColor.a);\n",
                "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
                "devPosition.z = depth;\n",
                "stepLocalCoords = texCoords;\n",
            ),
            (false, true) => concat!(
                "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
                "devPosition.z = depth;\n",
                "stepLocalCoords = texCoords;\n",
            ),
            (true, false) => concat!(
                "color = half4(vertColor.bgr * vertColor.a, vertColor.a);\n",
                "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
                "devPosition.z = depth;\n",
                "stepLocalCoords = position;\n",
            ),
            (false, false) => concat!(
                "float4 devPosition = localToDevice * float4(position, 0.0, 1.0);\n",
                "devPosition.z = depth;\n",
                "stepLocalCoords = position;\n",
            ),
        }
        .to_owned()
    }

    // Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L146-L151 (chrome/m156)
    fn fragment_color_sksl(
        &self,
        _roots: &crate::graphite::paint_params_key::RootNodesInfo,
    ) -> String {
        if self.has_color {
            "primitiveColor = color;\n".to_owned()
        } else {
            String::new()
        }
    }

    // Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L153-L203 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let info = params.geometry().vertices();
        let vertex_count = info.vertex_count();
        let index_count = info.index_count();
        let positions = info.positions();
        let indices = info.indices();
        let colors = info.colors();
        let tex_coords = info.tex_coords();

        // Without indices, the reserve is the vertex count, otherwise the index count.
        let mut verts = Vertices::new(writer);
        let reserve = if indices.is_some() {
            index_count
        } else {
            vertex_count
        };
        verts.reserve(u32::try_from(reserve).expect("the vertex count fits in u32"));

        let mut state = VertState::new(vertex_count, indices, index_count);
        let vert_proc = state.choose_proc(info.mode());
        while vert_proc(&mut state) {
            let mut vw = verts.append(3);
            for f in [state.f0, state.f1, state.f2] {
                let p = positions[f];
                vw.put(&[p.x, p.y]);
                if self.has_color {
                    let color = colors.map_or(SK_COLOR_TRANSPARENT, |c| c[f]);
                    vw.put(&u32::from(color));
                }
                if self.has_tex_coords {
                    let t = tex_coords.map_or([0.0_f32, 0.0], |t| [t[f].x, t[f].y]);
                    vw.put(&t);
                }
                vw.put(&ssbo_index);
            }
        }
    }

    // Port of: src/gpu/graphite/render/VerticesRenderStep.cpp#L205-L213 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        // Vertices are transformed on the GPU. The depth is a uniform, so the same depth is not
        // copied for each vertex.
        let uniforms = gatherer.uniform_manager();
        uniforms.write_m44(params.transform().matrix());
        uniforms.write_f32(params.order().depth_as_float());
    }
}
