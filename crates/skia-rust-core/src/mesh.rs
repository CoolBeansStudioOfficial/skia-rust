// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMesh.h, src/core/SkMesh.cpp and src/core/SkMeshPriv.h
// (chrome/m156).

//! Custom meshes: a [`MeshSpecification`] (the `SkSL` vertex and fragment programs, and the
//! attributes and varyings they use), the [`VertexBuffer`]s and [`IndexBuffer`]s that hold the
//! vertex data, and the [`Mesh`] that draws a range of them with `Canvas::draw_mesh`.
//!
//! There is no CPU rasterization of meshes: `SkBitmapDevice::drawMesh` draws nothing in this
//! version of Skia, and the raster device here does the same.

use std::fmt::{self, Write as _};
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_sksl::analysis::{
    self, ProgramVisitor, walk_expression, walk_program_element, walk_statement,
};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::ir::{
    ElemId, ExprId, ExpressionKind, IrPool, Program, ProgramElementKind, StatementKind, StmtId,
    TypeId,
};
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

use crate::alpha_type::AlphaType;
use crate::checksum::hash32;
use crate::color_space::ColorSpace;
use crate::data::Data;
use crate::rect::Rect;
use crate::runtime_effect::uniform::Flags as UniformFlags;
use crate::runtime_effect::{Child, ChildPtr, Uniform};
use crate::runtime_effect_priv::{child_type_to_str, var_as_child, var_as_uniform};

/// `SkMeshSpecification::Attribute::Type`: the CPU format of an attribute. The shader sees
/// `float`, `float2`, `float3`, `float4`, or `half4` for `UByte4Unorm`.
#[doc(alias = "SkMeshSpecification::Attribute::Type")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeType {
    /// `kFloat`: one float.
    Float,
    /// `kFloat2`: two floats.
    Float2,
    /// `kFloat3`: three floats.
    Float3,
    /// `kFloat4`: four floats.
    Float4,
    /// `kUByte4_unorm`: four bytes, read as normalized values in a `half4`.
    UByte4Unorm,
}

/// `SkMeshSpecification::Attribute`: one member of the vertex data.
#[doc(alias = "SkMeshSpecification::Attribute")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    /// The CPU format of the attribute.
    pub ty: AttributeType,
    /// The byte offset of the attribute within a vertex. Must be a multiple of 4.
    pub offset: usize,
    /// The name of the attribute in the vertex shader's `Attributes` struct.
    pub name: String,
}

/// `SkMeshSpecification::Varying::Type`: the type of a varying, as the shaders see it.
#[doc(alias = "SkMeshSpecification::Varying::Type")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VaryingType {
    /// `kFloat`: `float`.
    Float,
    /// `kFloat2`: `float2`.
    Float2,
    /// `kFloat3`: `float3`.
    Float3,
    /// `kFloat4`: `float4`.
    Float4,
    /// `kHalf`: `half`.
    Half,
    /// `kHalf2`: `half2`.
    Half2,
    /// `kHalf3`: `half3`.
    Half3,
    /// `kHalf4`: `half4`.
    Half4,
}

/// `SkMeshSpecification::Varying`: one member of the data passed from the vertex to the
/// fragment shader.
#[doc(alias = "SkMeshSpecification::Varying")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Varying {
    /// The type of the varying.
    pub ty: VaryingType,
    /// The name of the varying in the `Varyings` struct.
    pub name: String,
}

/// `SkMeshSpecification::ColorType`: what the fragment shader returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorType {
    None,
    Half4,
    Float4,
}

/// `SkMeshSpecification::Result`: the specification, or the error text if it is `None`.
#[doc(alias = "SkMeshSpecification::Result")]
#[derive(Default)]
pub struct MeshSpecificationResult {
    /// `specification`.
    pub specification: Option<Arc<MeshSpecification>>,
    /// `error`: empty when the specification was made.
    pub error: String,
}

/// `SkMeshSpecification`: the attributes, varyings, uniforms and children of a custom mesh, and
/// the vertex and fragment `SkSL` programs that use them.
#[doc(alias = "SkMeshSpecification")]
pub struct MeshSpecification {
    attributes: Vec<Attribute>,
    varyings: Vec<Varying>,
    uniforms: Vec<Uniform>,
    children: Vec<Child>,
    vs: Program,
    fs: Program,
    stride: usize,
    passthrough_local_coords_varying_index: i32,
    dead_varying_mask: u32,
    color_type: ColorType,
    color_space: Option<ColorSpace>,
    alpha_type: AlphaType,
    hash: u32,
}

impl MeshSpecification {
    /// `kMaxStride`.
    pub const MAX_STRIDE: usize = 1024;
    /// `kMaxAttributes`.
    pub const MAX_ATTRIBUTES: usize = 8;
    /// `kStrideAlignment`.
    pub const STRIDE_ALIGNMENT: usize = 4;
    /// `kOffsetAlignment`.
    pub const OFFSET_ALIGNMENT: usize = 4;
    /// `kMaxVaryings`.
    pub const MAX_VARYINGS: usize = 6;

    /// `SkMeshSpecification::Make` with the sRGB color space and premultiplied alpha.
    #[doc(alias = "SkMeshSpecification::Make")]
    // Port of: src/core/SkMesh.cpp#L389-L401 (chrome/m156)
    #[must_use]
    pub fn make(
        attributes: &[Attribute],
        vertex_stride: usize,
        varyings: &[Varying],
        vs: &str,
        fs: &str,
    ) -> MeshSpecificationResult {
        Self::make_with_alpha_type(
            attributes,
            vertex_stride,
            varyings,
            vs,
            fs,
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        )
    }

    /// `SkMeshSpecification::Make` with a color space and premultiplied alpha.
    // Port of: src/core/SkMesh.cpp#L403-L410 (chrome/m156)
    #[must_use]
    pub fn make_with_color_space(
        attributes: &[Attribute],
        vertex_stride: usize,
        varyings: &[Varying],
        vs: &str,
        fs: &str,
        cs: Option<ColorSpace>,
    ) -> MeshSpecificationResult {
        Self::make_with_alpha_type(
            attributes,
            vertex_stride,
            varyings,
            vs,
            fs,
            cs,
            AlphaType::Premul,
        )
    }

    /// `SkMeshSpecification::Make` with every option.
    ///
    /// The `position` varying is added (as `float2`) unless the caller declares one. The
    /// structs for the attributes and varyings are prepended to the shaders.
    // Port of: src/core/SkMesh.cpp#L412-L471 (chrome/m156)
    #[doc(alias = "SkMeshSpecification::Make")]
    #[must_use]
    pub fn make_with_alpha_type(
        attributes: &[Attribute],
        vertex_stride: usize,
        varyings: &[Varying],
        vs: &str,
        fs: &str,
        cs: Option<ColorSpace>,
        at: AlphaType,
    ) -> MeshSpecificationResult {
        let mut attributes_struct = String::from("struct Attributes {\n");
        for a in attributes {
            // Writing to a String cannot fail.
            let _ = writeln!(
                attributes_struct,
                "  {} {};",
                attribute_type_string(a.ty),
                a.name
            );
        }
        attributes_struct += "};\n";

        let mut user_provided_position_varying = false;
        for v in varyings {
            if v.name == "position" {
                if v.ty != VaryingType::Float2 {
                    return MeshSpecificationResult {
                        specification: None,
                        error: String::from("Varying \"position\" must have type float2."),
                    };
                }
                user_provided_position_varying = true;
            }
        }

        let mut temp_varyings: Vec<Varying> = Vec::new();
        let mut varyings: &[Varying] = varyings;
        if !user_provided_position_varying {
            // Even though we check the # of varyings in MakeFromSourceWithStructs we check here,
            // too, to avoid overflow with + 1.
            if varyings.len() > Self::MAX_VARYINGS - 1 {
                return fail(format!(
                    "A maximum of {} varyings is allowed.",
                    Self::MAX_VARYINGS
                ));
            }
            temp_varyings.extend_from_slice(varyings);
            temp_varyings.push(Varying {
                ty: VaryingType::Float2,
                name: String::from("position"),
            });
            varyings = &temp_varyings;
        }

        let mut varying_struct = String::from("struct Varyings {\n");
        for v in varyings {
            // Writing to a String cannot fail.
            let _ = writeln!(
                varying_struct,
                "  {} {};",
                varying_type_string(v.ty),
                v.name
            );
        }
        varying_struct += "};\n";

        let mut vs_source = String::new();
        vs_source += &varying_struct;
        vs_source += &attributes_struct;
        vs_source += vs;

        let mut fs_source = String::new();
        fs_source += &varying_struct;
        fs_source += fs;

        Self::make_from_source_with_structs(
            attributes,
            vertex_stride,
            varyings,
            &vs_source,
            &fs_source,
            cs,
            at,
        )
    }

    /// `MakeFromSourceWithStructs`: validates the layout, compiles both shaders and collects the
    /// uniforms, children and varying analysis.
    // Port of: src/core/SkMesh.cpp#L473-L593 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    fn make_from_source_with_structs(
        attributes: &[Attribute],
        stride: usize,
        varyings: &[Varying],
        vs: &str,
        fs: &str,
        cs: Option<ColorSpace>,
        at: AlphaType,
    ) -> MeshSpecificationResult {
        if let Err(error) = check_vertex_offsets_and_stride(attributes, stride) {
            return fail(error);
        }

        for a in attributes {
            if !check_name(&a.name) {
                return fail(format!("\"{}\" is not a valid attribute name.", a.name));
            }
        }

        if varyings.len() > Self::MAX_VARYINGS {
            return fail(format!(
                "A maximum of {} varyings is allowed.",
                Self::MAX_VARYINGS
            ));
        }

        for v in varyings {
            if !check_name(&v.name) {
                return fail(format!("\"{}\" is not a valid varying name.", v.name));
            }
        }

        let mut uniforms: Vec<Uniform> = Vec::new();
        let mut children: Vec<Child> = Vec::new();
        let mut offset: usize = 0;

        let mut compiler = Compiler::new();

        // Disable memory pooling; this might slow down compilation slightly, but it will ensure
        // that a long-lived mesh specification doesn't waste memory.
        let settings = ProgramSettings {
            use_memory_pool: false,
            ..ProgramSettings::default()
        };

        // TODO(skbug.com/40042585): Add SkCapabilities to the API, check against required version.
        let Some(vs_program) =
            compiler.convert_program(ProgramKind::MeshVertex, vs.as_bytes(), settings)
        else {
            return fail(format!(
                "VS: {}",
                error_text(&compiler.error_text_bytes(true))
            ));
        };

        if let Err(error) = gather_uniforms_and_check_for_main(
            &vs_program,
            &mut uniforms,
            &mut children,
            UniformFlags::VERTEX,
            &mut offset,
        ) {
            return fail(error);
        }

        if calls_color_transform_intrinsics(&vs_program) {
            return fail(String::from(
                "Color transform intrinsics are not permitted in custom mesh shaders",
            ));
        }

        let Some(fs_program) =
            compiler.convert_program(ProgramKind::MeshFragment, fs.as_bytes(), settings)
        else {
            return fail(format!(
                "FS: {}",
                error_text(&compiler.error_text_bytes(true))
            ));
        };

        if let Err(error) = gather_uniforms_and_check_for_main(
            &fs_program,
            &mut uniforms,
            &mut children,
            UniformFlags::FRAGMENT,
            &mut offset,
        ) {
            return fail(error);
        }

        if calls_color_transform_intrinsics(&fs_program) {
            return fail(String::from(
                "Color transform intrinsics are not permitted in custom mesh shaders",
            ));
        }

        let ct = get_fs_color_type(&fs_program);

        let (cs, at) = if ct == ColorType::None {
            (None, AlphaType::Premul)
        } else {
            if cs.is_none() {
                return fail(String::from(
                    "Must provide a color space if FS returns a color.",
                ));
            }
            if at == AlphaType::Unknown {
                return fail(String::from(
                    "Must provide a valid alpha type if FS returns a color.",
                ));
            }
            (cs, at)
        };

        let (passthrough_local_coords_varying_index, dead_varying_mask) =
            check_for_passthrough_local_coords_and_dead_varyings(&fs_program);

        if let Ok(index) = usize::try_from(passthrough_local_coords_varying_index) {
            debug_assert_eq!(varyings[index].ty, VaryingType::Float2);
        }

        // The hash covers the shader sources, the attribute offsets and types (the SkSL structs
        // have the GPU types, not the CPU data format), the stride, and the color space.
        let mut hash = hash32(&vs_program.source, 0);
        hash = hash32(&fs_program.source, hash);
        for a in attributes {
            hash = hash32(&a.offset.to_ne_bytes(), hash);
            hash = hash32(&(a.ty as u32).to_ne_bytes(), hash);
        }
        hash = hash32(&stride.to_ne_bytes(), hash);
        let cs_hash: u64 = cs.as_ref().map_or(0, ColorSpace::hash);
        hash = hash32(&cs_hash.to_ne_bytes(), hash);
        hash = hash32(&(at as u32).to_ne_bytes(), hash);

        MeshSpecificationResult {
            specification: Some(Arc::new(MeshSpecification {
                attributes: attributes.to_vec(),
                varyings: varyings.to_vec(),
                uniforms,
                children,
                vs: vs_program,
                fs: fs_program,
                stride,
                passthrough_local_coords_varying_index,
                dead_varying_mask,
                color_type: ct,
                color_space: cs,
                alpha_type: at,
                hash,
            })),
            error: String::new(),
        }
    }

    /// `uniformSize()`: the size of the uniform block, rounded up to 4 bytes.
    // Port of: src/core/SkMesh.cpp#L642-L645 (chrome/m156)
    #[must_use]
    pub fn uniform_size(&self) -> usize {
        self.uniforms
            .last()
            .map_or(0, |u| align4(u.offset() + u.size_in_bytes()))
    }

    /// `attributes()`.
    #[must_use]
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// `uniforms()`.
    #[must_use]
    pub fn uniforms(&self) -> &[Uniform] {
        &self.uniforms
    }

    /// `children()`.
    #[must_use]
    pub fn children(&self) -> &[Child] {
        &self.children
    }

    /// `varyings()`, including the `position` varying the specification added, if any.
    #[must_use]
    pub fn varyings(&self) -> &[Varying] {
        &self.varyings
    }

    /// `stride()`: the size in bytes of one vertex.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// `colorSpace()`.
    #[must_use]
    pub fn color_space(&self) -> Option<&ColorSpace> {
        self.color_space.as_ref()
    }

    /// `findChild(name)`.
    #[must_use]
    pub fn find_child(&self, name: &str) -> Option<&Child> {
        self.children.iter().find(|c| c.name() == name)
    }

    /// `findUniform(name)`.
    #[must_use]
    pub fn find_uniform(&self, name: &str) -> Option<&Uniform> {
        self.uniforms.iter().find(|u| u.name() == name)
    }

    /// `findAttribute(name)`.
    #[must_use]
    pub fn find_attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|a| a.name == name)
    }

    /// `findVarying(name)`.
    #[must_use]
    pub fn find_varying(&self, name: &str) -> Option<&Varying> {
        self.varyings.iter().find(|v| v.name == name)
    }
}

impl fmt::Debug for MeshSpecification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeshSpecification")
            .field("attributes", &self.attributes)
            .field("varyings", &self.varyings)
            .field("stride", &self.stride)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for MeshSpecificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeshSpecificationResult")
            .field("specification", &self.specification)
            .field("error", &self.error)
            .finish()
    }
}

impl fmt::Debug for Mesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mesh")
            .field("spec", &self.spec)
            .field("vertex_offset", &self.v_offset)
            .field("vertex_count", &self.v_count)
            .field("index_offset", &self.i_offset)
            .field("index_count", &self.i_count)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for MeshResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeshResult")
            .field("mesh", &self.mesh)
            .field("error", &self.error)
            .finish()
    }
}

/// `SkMesh::Mode`: how the vertices form triangles.
#[doc(alias = "SkMesh::Mode")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// `kTriangles`: every three vertices (or indices) form a triangle.
    #[default]
    Triangles,
    /// `kTriangleStrip`: each vertex after the first two forms a triangle with the two before.
    TriangleStrip,
}

/// `SkMesh::Result`: the mesh, or the error text if the mesh is not valid.
#[doc(alias = "SkMesh::Result")]
#[derive(Default)]
pub struct MeshResult {
    /// `mesh`: default (not valid) if there was an error.
    pub mesh: Mesh,
    /// `error`: empty when the mesh is valid.
    pub error: String,
}

/// `SkMesh`: a range of a vertex buffer (and optionally an index buffer), drawn with a
/// specification, uniforms and children.
#[doc(alias = "SkMesh")]
#[derive(Clone, Default)]
pub struct Mesh {
    spec: Option<Arc<MeshSpecification>>,
    vb: Option<Arc<VertexBuffer>>,
    ib: Option<Arc<IndexBuffer>>,
    uniforms: Option<Data>,
    children: Vec<ChildPtr>,
    v_offset: usize,
    v_count: usize,
    i_offset: usize,
    i_count: usize,
    mode: Mode,
    bounds: Rect,
}

impl Mesh {
    /// `SkMesh::Make`: a mesh of `vertex_count` vertices from `vertex_offset`, and the rest of
    /// its drawing state. The result is valid only if [`Mesh::validate`] accepts it.
    #[doc(alias = "SkMesh::Make")]
    #[allow(clippy::too_many_arguments)]
    // Skia's SkMesh::Make takes these eight parameters
    // Port of: src/core/SkMesh.cpp#L694-L716 (chrome/m156)
    #[must_use]
    pub fn make(
        spec: Option<Arc<MeshSpecification>>,
        mode: Mode,
        vb: Option<Arc<VertexBuffer>>,
        vertex_count: usize,
        vertex_offset: usize,
        uniforms: Option<Data>,
        children: &[ChildPtr],
        bounds: Rect,
    ) -> MeshResult {
        let mesh = Self {
            spec,
            mode,
            vb,
            uniforms,
            children: children.to_vec(),
            v_count: vertex_count,
            v_offset: vertex_offset,
            bounds,
            ..Self::default()
        };
        Self::finish(mesh)
    }

    /// `SkMesh::MakeIndexed`: like [`Mesh::make`], drawing through an index buffer.
    #[doc(alias = "SkMesh::MakeIndexed")]
    #[allow(clippy::too_many_arguments)]
    // Skia's SkMesh::MakeIndexed takes these eleven parameters
    // Port of: src/core/SkMesh.cpp#L718-L751 (chrome/m156)
    #[must_use]
    pub fn make_indexed(
        spec: Option<Arc<MeshSpecification>>,
        mode: Mode,
        vb: Option<Arc<VertexBuffer>>,
        vertex_count: usize,
        vertex_offset: usize,
        ib: Option<Arc<IndexBuffer>>,
        index_count: usize,
        index_offset: usize,
        uniforms: Option<Data>,
        children: &[ChildPtr],
        bounds: Rect,
    ) -> MeshResult {
        if ib.is_none() {
            // We check this before calling validate to disambiguate from a non-indexed mesh
            // where IB is expected to be null.
            return MeshResult {
                mesh: Mesh::default(),
                error: String::from("An index buffer is required."),
            };
        }
        let mesh = Self {
            spec,
            mode,
            vb,
            v_count: vertex_count,
            v_offset: vertex_offset,
            ib,
            uniforms,
            children: children.to_vec(),
            i_count: index_count,
            i_offset: index_offset,
            bounds,
        };
        Self::finish(mesh)
    }

    /// Returns the mesh if it validates, or the default mesh and the error if it does not.
    fn finish(mesh: Self) -> MeshResult {
        match mesh.validate() {
            Ok(()) => MeshResult {
                mesh,
                error: String::new(),
            },
            Err(error) => MeshResult {
                mesh: Mesh::default(),
                error,
            },
        }
    }

    /// `isValid()`: whether the mesh has a specification (a mesh that validated has one).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let valid = self.spec.is_some();
        debug_assert_eq!(valid, self.validate().is_ok());
        valid
    }

    /// `validate()`.
    ///
    /// # Errors
    /// The reason the mesh is not valid: the message Skia gives for the first check that fails.
    // Port of: src/core/SkMesh.cpp#L767-L862 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    pub fn validate(&self) -> Result<(), String> {
        let Some(spec) = self.spec.as_ref() else {
            return Err(String::from("SkMeshSpecification is required."));
        };

        let Some(vb) = self.vb.as_ref() else {
            return Err(String::from("A vertex buffer is required."));
        };

        if spec.children().len() != self.children.len() {
            return Err(format!(
                "The mesh specification declares {} child effects, but the mesh supplies {}.",
                spec.children().len(),
                self.children.len()
            ));
        }

        for (index, mesh_child) in self.children.iter().enumerate() {
            let spec_child = &spec.children()[index];
            if let Some(child_ty) = mesh_child.ty()
                && spec_child.ty() != child_ty
            {
                return Err(format!(
                    "Child effect '{}' was specified as a {}, but passed as a {}.",
                    spec_child.name(),
                    child_type_to_str(spec_child.ty()),
                    child_type_to_str(child_ty),
                ));
            }
        }

        let mut sm = SafeMath::new();
        let vsize = sm.mul(spec.stride(), self.v_count);
        if sm.add(vsize, self.v_offset) > vb.size() {
            return Err(String::from(
                "The vertex buffer offset and vertex count reads beyond the end of the vertex \
                 buffer.",
            ));
        }

        if !self.v_offset.is_multiple_of(spec.stride()) {
            return Err(format!(
                "The vertex offset ({}) must be a multiple of the vertex stride ({}).",
                self.v_offset,
                spec.stride()
            ));
        }

        let uniform_size = spec.uniform_size();
        if uniform_size != 0 {
            // C++ reads `fUniforms->size()` here even when there are no uniforms, which is a
            // null dereference; the size of absent data is taken as 0 instead.
            let have = self.uniforms.as_ref().map_or(0, Data::size);
            if self.uniforms.is_none() || have < uniform_size {
                return Err(format!(
                    "The uniform data is {have} bytes but must be at least {uniform_size}."
                ));
            }
        }

        let mode_str = match self.mode {
            Mode::Triangles => "triangles",
            Mode::TriangleStrip => "triangle-strip",
        };
        let min_vcount = min_vcount_for_mode(self.mode);
        if let Some(ib) = self.ib.as_ref() {
            if self.i_count < min_vcount {
                return Err(format!(
                    "{} mode requires at least {} indices but index count is {}.",
                    mode_str, min_vcount, self.i_count
                ));
            }
            let isize = sm.mul(2, self.i_count);
            if sm.add(isize, self.i_offset) > ib.size() {
                return Err(String::from(
                    "The index buffer offset and index count reads beyond the end of the index \
                     buffer.",
                ));
            }
            // If we allow 32 bit indices then this should enforce 4 byte alignment in that case.
            if !self.i_offset.is_multiple_of(2) {
                return Err(String::from("The index offset must be a multiple of 2."));
            }
        } else {
            if self.v_count < min_vcount {
                // Skia prints the index count here, which is 0 for a mesh without indices.
                return Err(format!(
                    "{} mode requires at least {} vertices but vertex count is {}.",
                    mode_str, min_vcount, self.i_count
                ));
            }
            debug_assert_eq!(self.i_count, 0);
            debug_assert_eq!(self.i_offset, 0);
        }

        if !sm.ok() {
            return Err(String::from("Overflow"));
        }
        Ok(())
    }

    /// `spec()`.
    #[must_use]
    pub fn spec(&self) -> Option<&Arc<MeshSpecification>> {
        self.spec.as_ref()
    }

    /// `mode()`.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// `vertexBuffer()`.
    #[must_use]
    pub fn vertex_buffer(&self) -> Option<&Arc<VertexBuffer>> {
        self.vb.as_ref()
    }

    /// `vertexOffset()`.
    #[must_use]
    pub fn vertex_offset(&self) -> usize {
        self.v_offset
    }

    /// `vertexCount()`.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.v_count
    }

    /// `indexBuffer()`.
    #[must_use]
    pub fn index_buffer(&self) -> Option<&Arc<IndexBuffer>> {
        self.ib.as_ref()
    }

    /// `indexOffset()`.
    #[must_use]
    pub fn index_offset(&self) -> usize {
        self.i_offset
    }

    /// `indexCount()`.
    #[must_use]
    pub fn index_count(&self) -> usize {
        self.i_count
    }

    /// `uniforms()`.
    #[must_use]
    pub fn uniforms(&self) -> Option<&Data> {
        self.uniforms.as_ref()
    }

    /// `children()`.
    #[must_use]
    pub fn children(&self) -> &[ChildPtr] {
        &self.children
    }

    /// `bounds()`.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// `SkMeshSpecification::kStrideAlignment`-style rounding of `SkAlign4`.
fn align4(x: usize) -> usize {
    (x + 3) & !3
}

fn fail(error: String) -> MeshSpecificationResult {
    MeshSpecificationResult {
        specification: None,
        error,
    }
}

/// The text of a compiler error, as `SkString` holds it.
fn error_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `SkSafeMath`: arithmetic that records overflow instead of wrapping. Once overflow happens,
/// every later result is 0 and [`SafeMath::ok`] is false.
struct SafeMath {
    ok: bool,
}

impl SafeMath {
    fn new() -> Self {
        Self { ok: true }
    }

    fn mul(&mut self, a: usize, b: usize) -> usize {
        self.check(a.checked_mul(b))
    }

    fn add(&mut self, a: usize, b: usize) -> usize {
        self.check(a.checked_add(b))
    }

    fn check(&mut self, value: Option<usize>) -> usize {
        match value {
            Some(v) if self.ok => v,
            _ => {
                self.ok = false;
                0
            }
        }
    }

    fn ok(&self) -> bool {
        self.ok
    }
}

/// `min_vcount_for_mode`.
fn min_vcount_for_mode(mode: Mode) -> usize {
    match mode {
        Mode::Triangles | Mode::TriangleStrip => 3,
    }
}

/// `attribute_type_size`.
// Port of: src/core/SkMesh.cpp#L175-L184 (chrome/m156)
fn attribute_type_size(ty: AttributeType) -> usize {
    match ty {
        AttributeType::Float | AttributeType::UByte4Unorm => 4,
        AttributeType::Float2 => 2 * 4,
        AttributeType::Float3 => 3 * 4,
        AttributeType::Float4 => 4 * 4,
    }
}

/// `attribute_type_string`: the `SkSL` type of the attribute's struct member.
// Port of: src/core/SkMesh.cpp#L186-L195 (chrome/m156)
fn attribute_type_string(ty: AttributeType) -> &'static str {
    match ty {
        AttributeType::Float => "float",
        AttributeType::Float2 => "float2",
        AttributeType::Float3 => "float3",
        AttributeType::Float4 => "float4",
        AttributeType::UByte4Unorm => "half4",
    }
}

/// `varying_type_string`.
// Port of: src/core/SkMesh.cpp#L197-L210 (chrome/m156)
fn varying_type_string(ty: VaryingType) -> &'static str {
    match ty {
        VaryingType::Float => "float",
        VaryingType::Float2 => "float2",
        VaryingType::Float3 => "float3",
        VaryingType::Float4 => "float4",
        VaryingType::Half => "half",
        VaryingType::Half2 => "half2",
        VaryingType::Half3 => "half3",
        VaryingType::Half4 => "half4",
    }
}

/// `check_name`: a non-empty name of letters, digits and underscores. This is not a full check
/// of the name; the compiler does that. It only keeps several tokens from one name.
// Port of: src/core/SkMesh.cpp#L163-L173 (chrome/m156)
fn check_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|c| c == b'_' || c.is_ascii_alphanumeric())
}

/// `check_vertex_offsets_and_stride`.
// Port of: src/core/SkMesh.cpp#L212-L252 (chrome/m156)
fn check_vertex_offsets_and_stride(attributes: &[Attribute], stride: usize) -> Result<(), String> {
    // Vulkan 1.0 has a minimum maximum attribute count of 2048.
    // ES 2 has a max of 8.
    // Four bytes alignment is required by Metal.
    // ES2 has a minimum maximum of 8. We may need one for a broken gl_FragCoord workaround and
    // one for local coords.
    if attributes.is_empty() {
        return Err(String::from("At least 1 attribute is required."));
    }
    if attributes.len() > MeshSpecification::MAX_ATTRIBUTES {
        return Err(format!(
            "A maximum of {} attributes is allowed.",
            MeshSpecification::MAX_ATTRIBUTES
        ));
    }
    if stride == 0 || stride & (MeshSpecification::STRIDE_ALIGNMENT - 1) != 0 {
        return Err(format!(
            "Vertex stride must be a non-zero multiple of {}.",
            MeshSpecification::STRIDE_ALIGNMENT
        ));
    }
    if stride > MeshSpecification::MAX_STRIDE {
        return Err(format!(
            "Stride cannot exceed {}.",
            MeshSpecification::MAX_STRIDE
        ));
    }
    for a in attributes {
        if a.offset & (MeshSpecification::OFFSET_ALIGNMENT - 1) != 0 {
            return Err(format!(
                "Attribute offset must be a multiple of {}.",
                MeshSpecification::OFFSET_ALIGNMENT
            ));
        }
        // This equivalent to vertexAttributeAccessBeyondStride==VK_FALSE in
        // VK_KHR_portability_subset. First check is to avoid overflow in second check.
        if a.offset >= stride || a.offset + attribute_type_size(a.ty) > stride {
            return Err(String::from(
                "Attribute offset plus size cannot exceed stride.",
            ));
        }
    }
    Ok(())
}

/// `gather_uniforms_and_check_for_main`: collects the uniforms and children of a shader, and
/// checks that it has a `main`. A uniform that both shaders declare must match in both.
// Port of: src/core/SkMesh.cpp#L76-L137 (chrome/m156)
fn gather_uniforms_and_check_for_main(
    program: &Program,
    uniforms: &mut Vec<Uniform>,
    children: &mut Vec<Child>,
    stage: UniformFlags,
    offset: &mut usize,
) -> Result<(), String> {
    let pool = &program.pool;
    let mut found_main = false;
    for elem in program.elements() {
        match &pool.element(elem).kind {
            ProgramElementKind::Function(defn) => {
                if pool.function(defn.declaration).is_main {
                    found_main = true;
                }
            }
            ProgramElementKind::GlobalVar(global) => {
                let StatementKind::VarDeclaration(var_decl) =
                    &pool.statement(global.declaration).kind
                else {
                    unreachable!("a global variable declaration holds a VarDeclaration");
                };
                let var_id = var_decl.var;
                let var = pool.variable(var_id);
                if !var.modifier_flags.is_uniform() {
                    continue;
                }
                if pool.ty(var.ty).is_effect_child() {
                    // This is a child effect; add it to our list of children.
                    children.push(var_as_child(pool, var_id, children.len()));
                } else {
                    // This is a uniform variable; make sure it exists in our list of uniforms,
                    // and ensure that the type and layout matches between VS and FS.
                    let name: &str = &var.name;
                    match uniforms.iter().position(|u| u.name() == name) {
                        None => {
                            let mut uniform = var_as_uniform(pool, var_id, offset);
                            uniform.flags |= stage;
                            uniforms.push(uniform);
                        }
                        Some(index) => {
                            // Check that the two declarations are equivalent.
                            let mut ignored_offset = 0;
                            let uniform = var_as_uniform(pool, var_id, &mut ignored_offset);
                            let existing = &uniforms[index];
                            if uniform.is_array() != existing.is_array()
                                || uniform.ty() != existing.ty()
                                || uniform.count() != existing.count()
                            {
                                return Err(format!(
                                    "Uniform {name} declared with different types in vertex and \
                                     fragment shaders."
                                ));
                            }
                            if uniform.is_color() != existing.is_color() {
                                return Err(format!(
                                    "Uniform {name} declared with different color layout in \
                                     vertex and fragment shaders."
                                ));
                            }
                            uniforms[index].flags |= stage;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !found_main {
        return Err(String::from("No main function found."));
    }
    Ok(())
}

/// `calls_color_transform_intrinsics`, with the usage analysis of the program.
// Port of: src/core/SkMesh.cpp#L389-L471 (the `Analysis::CallsColorTransformIntrinsics` calls)
fn calls_color_transform_intrinsics(program: &Program) -> bool {
    let usage = analysis::get_usage(program);
    analysis::calls_color_transform_intrinsics(program, &usage)
}

/// `get_fs_color_type`: whether the fragment shader's `main` takes an output color, and of
/// which type.
// Port of: src/core/SkMesh.cpp#L139-L161 (chrome/m156)
fn get_fs_color_type(fs_program: &Program) -> ColorType {
    let pool = &fs_program.pool;
    for elem in fs_program.elements() {
        if let ProgramElementKind::Function(defn) = &pool.element(elem).kind {
            let decl = pool.function(defn.declaration);
            if decl.is_main {
                debug_assert!(decl.parameters.len() == 1 || decl.parameters.len() == 2);
                if decl.parameters.len() == 1 {
                    return ColorType::None;
                }
                let param_ty = pool.variable(decl.parameters[1]).ty;
                debug_assert!(
                    pool.ty(param_ty).matches(TypeId::HALF4)
                        || pool.ty(param_ty).matches(TypeId::FLOAT4)
                );
                return if pool.ty(param_ty).matches(TypeId::HALF4) {
                    ColorType::Half4
                } else {
                    ColorType::Float4
                };
            }
        }
    }
    unreachable!("a fragment shader without main was rejected earlier")
}

/// `check_for_passthrough_local_coords_and_dead_varyings`: returns the index of the varying that
/// `main` returns unchanged (or `-1` if there is none, `-2` if the returns are not all one
/// passthrough), and the mask of varyings that are never used except for that passthrough.
// Port of: src/core/SkMesh.cpp#L254-L387 (chrome/m156)
fn check_for_passthrough_local_coords_and_dead_varyings(fs_program: &Program) -> (i32, u32) {
    let mut visitor = PassthroughVisitor::new();
    visitor.visit(fs_program);
    (visitor.passthrough_field_index, !visitor.field_use_mask)
}

/// `kFailed` in `check_for_passthrough_local_coords_and_dead_varyings`.
const PASSTHROUGH_FAILED: i32 = -2;

/// The `Visitor` of `check_for_passthrough_local_coords_and_dead_varyings`.
struct PassthroughVisitor {
    varyings_type: Option<TypeId>,
    varyings: Option<skia_rust_sksl::ir::VarId>,
    passthrough_field_index: i32,
    in_main: bool,
    field_use_mask: u32,
}

impl PassthroughVisitor {
    fn new() -> Self {
        Self {
            varyings_type: None,
            varyings: None,
            passthrough_field_index: -1,
            in_main: false,
            field_use_mask: 0,
        }
    }

    fn passthrough_failed(&mut self) {
        if self.passthrough_field_index >= 0 {
            self.field_use_mask |= 1 << self.passthrough_field_index;
        }
        self.passthrough_field_index = PASSTHROUGH_FAILED;
    }
}

impl ProgramVisitor for PassthroughVisitor {
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        match &pool.element(element).kind {
            ProgramElementKind::StructDefinition(def) => {
                if pool.ty(def.ty).name() == "Varyings" {
                    self.varyings_type = Some(def.ty);
                }
                // No reason to keep looking at this type definition.
                false
            }
            ProgramElementKind::Function(defn) if pool.function(defn.declaration).is_main => {
                debug_assert!(self.varyings.is_none());
                let varyings = pool.function(defn.declaration).parameters[0];
                self.varyings = Some(varyings);

                debug_assert!(
                    self.varyings_type
                        .is_some_and(|t| pool.ty(t).matches(pool.variable(varyings).ty))
                );

                self.in_main = true;
                let result = walk_program_element(self, pool, element);
                self.in_main = false;
                result
            }
            _ => walk_program_element(self, pool, element),
        }
    }

    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if !self.in_main {
            return walk_statement(self, pool, stmt);
        }
        // We should only get here if are in main and therefore found the varyings parameter.
        debug_assert!(self.varyings.is_some());
        debug_assert!(self.varyings_type.is_some());

        if self.passthrough_field_index == PASSTHROUGH_FAILED {
            // We've already determined there are return statements that aren't passthrough or
            // return different fields.
            return walk_statement(self, pool, stmt);
        }
        let StatementKind::Return(rs) = &pool.statement(stmt).kind else {
            return walk_statement(self, pool, stmt);
        };

        // We just detect simple cases like "return varyings.foo;"
        let Some(expr) = rs.expression else {
            self.passthrough_failed();
            return walk_statement(self, pool, stmt);
        };
        let ExpressionKind::FieldAccess(fa) = &pool.expression(expr).kind else {
            self.passthrough_failed();
            return walk_statement(self, pool, stmt);
        };
        let ExpressionKind::VariableReference(base_ref) = &pool.expression(fa.base).kind else {
            self.passthrough_failed();
            return walk_statement(self, pool, stmt);
        };
        if Some(base_ref.variable) != self.varyings {
            self.passthrough_failed();
            return walk_statement(self, pool, stmt);
        }
        // A struct has far fewer than 2^31 fields.
        let field_index = i32::try_from(fa.field_index).expect("a field index fits in an i32");
        if self.passthrough_field_index >= 0 {
            // We already found an OK return statement. Check if this one returns the same field.
            if field_index != self.passthrough_field_index {
                self.passthrough_failed();
                return walk_statement(self, pool, stmt);
            }
            // We don't call our base class here because we don't want to hit visitExpression and
            // mark the returned field as used.
            return false;
        }
        let varyings_ty = pool.variable(self.varyings.expect("set in main")).ty;
        let field = &pool.ty(varyings_ty).fields()[fa.field_index];
        if !pool.ty(field.ty).matches(TypeId::FLOAT2) {
            self.passthrough_failed();
            return walk_statement(self, pool, stmt);
        }
        self.passthrough_field_index = field_index;
        // We don't call our base class here because we don't want to hit visitExpression and
        // mark the returned field as used.
        false
    }

    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        // Anything before the Varyings struct is defined doesn't matter.
        let Some(varyings_type) = self.varyings_type else {
            return false;
        };
        let e = pool.expression(expr);
        let ExpressionKind::FieldAccess(fa) = &e.kind else {
            return walk_expression(self, pool, expr);
        };
        let base_ty = pool.expression(fa.base).ty;
        if !pool.ty(base_ty).matches(varyings_type) {
            return walk_expression(self, pool, expr);
        }
        self.field_use_mask |= 1 << fa.field_index;
        false
    }
}

/// `SkMeshSpecificationPriv` and `SkMeshPriv`: accessors the tests and the rest of the crate
/// read, which are not part of the public API.
pub mod mesh_priv {
    use skia_rust_sksl::ir::Program;

    use super::{ColorType, MeshSpecification};
    use crate::alpha_type::AlphaType;

    /// `SkMeshSpecificationPriv::VS`: the compiled vertex shader.
    #[must_use]
    pub fn vs(spec: &MeshSpecification) -> &Program {
        &spec.vs
    }

    /// `SkMeshSpecificationPriv::FS`: the compiled fragment shader.
    #[must_use]
    pub fn fs(spec: &MeshSpecification) -> &Program {
        &spec.fs
    }

    /// `SkMeshSpecificationPriv::Hash`.
    #[must_use]
    pub fn hash(spec: &MeshSpecification) -> u32 {
        spec.hash
    }

    /// `SkMeshSpecificationPriv::HasColors`: whether the fragment shader returns a color.
    #[must_use]
    pub fn has_colors(spec: &MeshSpecification) -> bool {
        spec.color_type != ColorType::None
    }

    /// `SkMeshSpecificationPriv::AlphaType`.
    #[must_use]
    pub fn alpha_type(spec: &MeshSpecification) -> AlphaType {
        spec.alpha_type
    }

    /// `SkMeshSpecificationPriv::PassthroughLocalCoordsVaryingIndex`.
    #[must_use]
    pub fn passthrough_local_coords_varying_index(spec: &MeshSpecification) -> i32 {
        spec.passthrough_local_coords_varying_index
    }

    /// `SkMeshSpecificationPriv::VaryingIsDead`: a varying is dead if it is never referenced, or
    /// only referenced as a passthrough for local coordinates.
    // Port of: src/core/SkMeshPriv.h#L60-L70 (chrome/m156)
    #[must_use]
    pub fn varying_is_dead(spec: &MeshSpecification, v: usize) -> bool {
        debug_assert!(v < spec.varyings.len());
        (1u32 << v) & spec.dead_varying_mask != 0
    }
}

/// `SkMesh::VertexBuffer`, CPU-backed (`SkMeshPriv::CpuVertexBuffer`).
#[doc(alias = "SkMesh::VertexBuffer")]
#[derive(Debug)]
pub struct VertexBuffer {
    data: Mutex<Vec<u8>>,
}

/// `SkMesh::IndexBuffer`, CPU-backed (`SkMeshPriv::CpuIndexBuffer`).
#[doc(alias = "SkMesh::IndexBuffer")]
#[derive(Debug)]
pub struct IndexBuffer {
    data: Mutex<Vec<u8>>,
}

/// The data of a CPU buffer, and the update rule `SkMeshPriv::CpuBuffer` shares.
fn cpu_size(data: &Mutex<Vec<u8>>) -> usize {
    data.lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// `CheckUpdate` of `SkMesh::VertexBuffer::update`: the update must be non-empty, 4-byte aligned,
/// and inside the buffer.
// Port of: src/core/SkMesh.cpp#L864-L872 (chrome/m156)
fn check_update(offset: usize, size: usize, buffer_size: usize) -> bool {
    let mut sm = SafeMath::new();
    let end = sm.add(offset, size);
    size != 0 && offset.is_multiple_of(4) && size.is_multiple_of(4) && end <= buffer_size && sm.ok()
}

/// `SkMeshPriv::CpuBuffer::onUpdate`: copies `data` into the buffer at `offset`.
fn cpu_update(data: &Mutex<Vec<u8>>, bytes: &[u8], offset: usize) -> bool {
    let mut buffer = data.lock().unwrap_or_else(PoisonError::into_inner);
    if !check_update(offset, bytes.len(), buffer.len()) {
        return false;
    }
    buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
    true
}

impl VertexBuffer {
    /// `size()`: the size of the buffer in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        cpu_size(&self.data)
    }

    /// `update(dc, data, offset, size)` without a `GrDirectContext`: copies the bytes of `data`
    /// into the buffer at `offset`. The data is `data.len()` bytes; the offset and length must
    /// be multiples of 4 and must fit in the buffer. Returns false if they do not.
    // Port of: src/core/SkMesh.cpp#L881-L887 (chrome/m156)
    pub fn update(&self, data: &[u8], offset: usize) -> bool {
        cpu_update(&self.data, data, offset)
    }
}

impl IndexBuffer {
    /// `size()`: the size of the buffer in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        cpu_size(&self.data)
    }

    /// `update(dc, data, offset, size)` without a `GrDirectContext`; see
    /// [`VertexBuffer::update`].
    // Port of: src/core/SkMesh.cpp#L874-L879 (chrome/m156)
    pub fn update(&self, data: &[u8], offset: usize) -> bool {
        cpu_update(&self.data, data, offset)
    }
}

/// `SkMeshes`: makes and copies the CPU vertex and index buffers.
#[doc(alias = "SkMeshes")]
pub mod meshes {
    use std::sync::{Arc, Mutex};

    use super::{IndexBuffer, VertexBuffer};

    /// `SkMeshes::MakeVertexBuffer(data, size)`: a copy of `size` bytes of `data`, or zeros if
    /// there is no data.
    ///
    /// # Panics
    /// If `data` is shorter than `size`.
    // Port of: src/core/SkMesh.cpp#L905-L907 (chrome/m156)
    #[doc(alias = "MakeVertexBuffer")]
    #[must_use]
    pub fn make_vertex_buffer(data: Option<&[u8]>, size: usize) -> Arc<VertexBuffer> {
        Arc::new(VertexBuffer {
            data: Mutex::new(bytes_or_zeros(data, size)),
        })
    }

    /// `SkMeshes::CopyVertexBuffer`: a new buffer with the same contents. `None` for no buffer.
    // Port of: src/core/SkMesh.cpp#L909-L917 (chrome/m156)
    #[doc(alias = "CopyVertexBuffer")]
    #[must_use]
    pub fn copy_vertex_buffer(src: Option<&Arc<VertexBuffer>>) -> Option<Arc<VertexBuffer>> {
        let src = src?;
        let bytes = src
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if bytes.is_empty() {
            return None;
        }
        Some(make_vertex_buffer(Some(&bytes), bytes.len()))
    }

    /// `SkMeshes::MakeIndexBuffer(data, size)`: a copy of `size` bytes of `data`, or zeros if
    /// there is no data.
    ///
    /// # Panics
    /// If `data` is shorter than `size`.
    // Port of: src/core/SkMesh.cpp#L889-L891 (chrome/m156)
    #[doc(alias = "MakeIndexBuffer")]
    #[must_use]
    pub fn make_index_buffer(data: Option<&[u8]>, size: usize) -> Arc<IndexBuffer> {
        Arc::new(IndexBuffer {
            data: Mutex::new(bytes_or_zeros(data, size)),
        })
    }

    /// `SkMeshes::CopyIndexBuffer`: a new buffer with the same contents. `None` for no buffer.
    // Port of: src/core/SkMesh.cpp#L893-L903 (chrome/m156)
    #[doc(alias = "CopyIndexBuffer")]
    #[must_use]
    pub fn copy_index_buffer(src: Option<&Arc<IndexBuffer>>) -> Option<Arc<IndexBuffer>> {
        let src = src?;
        let bytes = src
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if bytes.is_empty() {
            return None;
        }
        Some(make_index_buffer(Some(&bytes), bytes.len()))
    }

    fn bytes_or_zeros(data: Option<&[u8]>, size: usize) -> Vec<u8> {
        match data {
            Some(data) => data[..size].to_vec(),
            None => vec![0; size],
        }
    }
}
