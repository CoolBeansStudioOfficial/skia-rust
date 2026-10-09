// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/MeshRenderStep.h, src/gpu/graphite/render/MeshRenderStep.cpp

//! [`MeshRenderStep`]: draws an `SkMesh` with its specification. The vertex data of the mesh is
//! copied into the draw's vertices (one triangle at a time, through `VertState`), and each vertex
//! carries the attributes of the specification after the SSBO index.

use skia_rust_core::mesh::mesh_priv;
use skia_rust_core::mesh::{MeshSpecification, Mode};
use skia_rust_core::runtime_effect_priv::transform_uniforms;
use skia_rust_core::vert_state::VertState;
use skia_rust_core::vertices::VertexMode;

use crate::graphite::attribute::Attribute;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Vertices};
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LEQUAL_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::resource_types::Layout;
use crate::graphite::shader_code_dictionary::{
    MESH_FS_MAIN_NAME, MESH_VS_MAIN_NAME, ShaderCodeDictionary,
};
use crate::graphite::shader_info::{MESH_VARYING_MANGLE_SUFFIX, ShaderAttribute};
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// `kMeshFSLocalCoordsName`: the fragment shader's local coordinates, returned by the mesh's
/// fragment function.
// Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L23 (chrome/m156)
const MESH_FS_LOCAL_COORDS_NAME: &str = "meshLocalCoordsOverride";

/// `kStepUniforms`: the depth and the local-to-device matrix, which every mesh writes.
// Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L25-L26 (chrome/m156)
const STEP_UNIFORMS: [Uniform; 2] = [
    Uniform::new("depth", SkSLType::Float),
    Uniform::new("localToDevice", SkSLType::Float4x4),
];

/// The `ssboIndex` attribute every appended vertex carries (`appendAttrs` of the constructor).
// Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L44 (chrome/m156)
const SSBO_INDEX_ATTR: Attribute =
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt);

/// `vertex_mode(mode)`: the `VertexMode` of a mesh mode.
// Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L28-L35 (chrome/m156)
fn vertex_mode(mode: Mode) -> VertexMode {
    match mode {
        Mode::Triangles => VertexMode::Triangles,
        Mode::TriangleStrip => VertexMode::TriangleStrip,
    }
}

/// The `Uniform` of a specification uniform, for writing its value: the array count is the
/// uniform's count if it is an array (`Uniform(nullptr, UniformTypeToSkSLType(u), ...)`).
// Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L178-L182 (chrome/m156)
fn spec_uniform_for_write(u: &skia_rust_core::runtime_effect::Uniform) -> Uniform {
    let ty = ShaderCodeDictionary::uniform_type_to_sksl_type(u);
    let count = if u.is_array() { u.count() } else { 0 };
    Uniform::new_owned(String::new(), ty, count)
}

/// The `MeshRenderStep`: draws the triangles of an `SkMesh`, shaded by its specification.
// Port of: src/gpu/graphite/render/MeshRenderStep.h#L16-L41 (chrome/m156)
#[doc(alias = "skgpu::graphite::MeshRenderStep")]
#[derive(Debug)]
pub struct MeshRenderStep {
    base: RenderStepBase,
}

impl MeshRenderStep {
    /// `MeshRenderStep(layout)`.
    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L37-L47 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        Self {
            base: RenderStepBase::new(
                layout,
                RenderStepID::Mesh,
                RenderStepFlags::PERFORMS_SHADING
                    | RenderStepFlags::APPEND_VERTICES
                    | RenderStepFlags::EMITS_PRIMITIVE_COLOR,
                &STEP_UNIFORMS,
                PrimitiveType::Triangles,
                DIRECT_DEPTH_LEQUAL_PASS,
                &[],
                &[SSBO_INDEX_ATTR],
                &[],
                &[],
            ),
        }
    }
}

/// The mesh specification of the roots of a draw. The mesh render step is only chosen for draws
/// whose paint has a mesh shader, so the specification is always there.
fn roots_spec(roots: &RootNodesInfo) -> &MeshSpecification {
    roots
        .mesh_spec
        .as_deref()
        .expect("a mesh draw has a mesh specification")
}

impl RenderStep for MeshRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L49-L77 (chrome/m156)
    fn vertex_sksl(&self, roots: &RootNodesInfo) -> String {
        let spec = roots_spec(roots);

        // Any attributes and varyings defined by the SkMeshSpecification will be emitted within
        // `ShaderInfo::generateVertexSkSL` with the same names as defined in the mesh
        // specification, we mangle the varyings' names to ensure there is not a name conflict
        // between an attribute and varying.
        let mut attrs = String::from("Attributes attributes;\n");
        for attr in spec.attributes() {
            attrs += &format!("attributes.{} = {};\n", attr.name, attr.name);
        }

        let mut varying_assignments = String::new();
        for v in spec.varyings() {
            varying_assignments += &format!(
                "{}{} = varyings.{};\n",
                v.name, MESH_VARYING_MANGLE_SUFFIX, v.name
            );
        }

        format!(
            "{attrs}\nVaryings varyings = {MESH_VS_MAIN_NAME}(attributes);\n\
             float4 devPosition       = localToDevice * float4(varyings.position, depth, 1.0);\n\
             stepLocalCoords = varyings.position;\n\
             {varying_assignments}"
        )
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L79-L125 (chrome/m156)
    fn fragment_color_sksl(&self, roots: &RootNodesInfo) -> String {
        let spec = roots_spec(roots);

        // Varyings defined by the SkMeshSpecification will be emitted within
        // `ShaderInfo::generateFragmentSkSL` following the same varying name mangling.
        let mut s = String::from("Varyings varyings;\n");
        for v in spec.varyings() {
            s += &format!(
                "varyings.{} = {}{};\n",
                v.name, v.name, MESH_VARYING_MANGLE_SUFFIX
            );
        }

        let needs_color_conversion = mesh_priv::color_type_is_float4(spec);
        let mut out_color_name = "primitiveColor";
        if needs_color_conversion {
            out_color_name = "primitiveColorFloat4";
            s += &format!("float4 {out_color_name};\n");
        }

        // Check if the mesh FS should have a primitive color output parameter.
        let has_color_output = mesh_priv::has_colors(spec);
        let method_call = if has_color_output {
            format!("{MESH_FS_MAIN_NAME}(varyings, {out_color_name})")
        } else {
            format!("{MESH_FS_MAIN_NAME}(varyings)")
        };

        s += &format!("float2 {MESH_FS_LOCAL_COORDS_NAME} = {method_call};\n");
        if needs_color_conversion {
            s += &format!("primitiveColor = half4({out_color_name});\n");
        }
        s
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.h#L35 (chrome/m156)
    fn fragment_color_sksl_local_coords_variable(&self) -> Option<&'static str> {
        Some(MESH_FS_LOCAL_COORDS_NAME)
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L127-L175 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let mesh = params.geometry().mesh();
        let spec = mesh.spec().expect("a valid mesh has a specification");
        // SkMesh::isValid() ensures the vertex buffer exists, so the data is there.
        let vertex_buffer = mesh
            .vertex_buffer()
            .expect("a valid mesh has a vertex buffer");
        let vertex_stride = spec.stride();
        let vertex_count = mesh.vertex_count();
        let vertex_offset = mesh.vertex_offset();
        let index_count = mesh.index_count();
        let index_offset = mesh.index_offset();

        vertex_buffer.with_data(|vertex_bytes| {
            let vertex_data = &vertex_bytes[vertex_offset..];
            // The indices are read from the index buffer's bytes, two per `u16`.
            let indices: Option<Vec<u16>> = mesh.index_buffer().map(|ib| {
                ib.with_data(|index_bytes| {
                    index_bytes[index_offset..index_offset + index_count * 2]
                        .chunks_exact(2)
                        .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                        .collect()
                })
            });

            let mut verts = Vertices::new(writer);
            verts.reserve(
                u32::try_from(if indices.is_some() {
                    index_count
                } else {
                    vertex_count
                })
                .expect("the vertex count fits in u32"),
            );

            let mut state = VertState::new(vertex_count, indices.as_deref(), index_count);
            let vert_proc = state.choose_proc(vertex_mode(mesh.mode()));
            while vert_proc(&mut state) {
                let mut vw = verts.append(3);
                for vert_index in [state.f0, state.f1, state.f2] {
                    let vertex_base = vert_index * vertex_stride;
                    vw.put(&ssbo_index);
                    for attr in spec.attributes() {
                        let size = mesh_priv::attr_type_byte_size(attr.ty);
                        let start = vertex_base + attr.offset;
                        vw.put(&vertex_data[start..start + size]);
                    }
                }
            }
        });
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L177-L212 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        let mesh = params.geometry().mesh();
        let spec = mesh.spec().expect("a valid mesh has a specification");

        // The step uniforms are written first: the depth, then the local-to-device matrix.
        #[cfg(debug_assertions)]
        gatherer.check_rewind();
        let uniforms = gatherer.uniform_manager();
        #[cfg(debug_assertions)]
        {
            let mut expected: Vec<Uniform> = STEP_UNIFORMS.to_vec();
            expected.extend(ShaderCodeDictionary::convert_runtime_effect_uniforms(
                spec.uniforms(),
            ));
            uniforms.set_expected_uniforms(&expected, false);
        }
        uniforms.write_f32(params.order().depth_as_float());
        uniforms.write_m44(params.transform().matrix());

        // Then the specification uniforms, converted to the destination color space.
        let original = mesh
            .uniforms()
            .cloned()
            .unwrap_or_else(skia_rust_core::data::Data::new_empty);
        let transformed = transform_uniforms(spec.uniforms(), &original, spec.color_space());
        let bytes = transformed.as_bytes();
        for u in spec.uniforms() {
            let write = spec_uniform_for_write(u);
            uniforms.write_uniform(&write, &bytes[u.offset()..]);
        }
        #[cfg(debug_assertions)]
        uniforms.done_with_expected_uniforms();
    }

    // Port of: src/gpu/graphite/render/MeshRenderStep.cpp#L214-L222 (chrome/m156)
    fn append_data_stride(&self, params: &DrawParams) -> usize {
        let spec = params
            .geometry()
            .mesh()
            .spec()
            .expect("a valid mesh has a specification");
        let mut stride: usize = self
            .base
            .append_attributes()
            .iter()
            .map(Attribute::size_align4)
            .sum();
        for attr in spec.attributes() {
            stride += ShaderAttribute::from_mesh_attribute(attr).size_align4();
        }
        stride
    }
}
