// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ShaderInfo.{h,cpp} (chrome/m156)

//! [`ShaderInfo`]: the `SkSL` of one pipeline (a `RenderStep` and a `PaintParamsKey`), the blend
//! state the fragment shader leaves to the hardware, and the labels.
//!
//! `ShaderInfo` holds all root `ShaderNode`s defined for a `PaintParams` as well as the extracted
//! fixed function blending parameters and other aggregate requirements for the effect trees that
//! have been linked into a single fragment program (sans any `RenderStep` fragment work and fixed
//! `SkSL` logic required for all rendering in Graphite).
//!
//! Deviations from the C++:
//!
//! - The `SK_DEBUG`-only comments in the generated `SkSL` (`// [%d] %s`, `// static attrs`, ...)
//!   are not written: the goldens come from a non-`SK_DEBUG` build (`docs/design/sksl.md`).
//! - The shader node trees are owned by the [`RootNodesInfo`] that `Make` builds, not by an arena.
//!   The lifted expressions borrow from it.
//! - Mesh attributes have owned names, so [`ShaderInfo::append_attributes`] returns
//!   [`ShaderAttribute`]s, which are `Attribute`s whose names may be owned.
//! - `GPU_TEST_UTILS`'s `EmitStorageFallbackTexture` is [`emit_storage_fallback_texture`].

use std::fmt::Write as _;
use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::PMColor4f;
use skia_rust_core::mesh;

use crate::gpu::blend::{BlendCoeff, BlendEquation, BlendInfo, blend_modifies_dst};
use crate::gpu::blend_formula::{
    BlendFormula, OutputType, get_blend_formula, get_lcd_blend_formula,
};
use crate::gpu::gpu_types::BackendApi;
use crate::gpu::swizzle::Swizzle;
use crate::graphite::attribute::{Attribute, Interpolation};
use crate::graphite::built_in_code_snippet_id::{
    BUILT_IN_CODE_SNIPPET_ID_COUNT, BuiltInCodeSnippetID, FIXED_BLEND_ID_OFFSET,
};
use crate::graphite::caps::{Caps, ResourceBindingRequirements};
use crate::graphite::context_utils::{
    INTRINSIC_UNIFORMS, can_use_hardware_blending, emit_sampler_layout,
};
use crate::graphite::draw_types::{PipelineStageFlags, VertexAttribType};
use crate::graphite::paint_params_key::{PaintParamsKey, RootNodesInfo};
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::{
    Coverage, RenderStep, SSBO_INDEX_ATTRIBUTE, SSBO_INDEX_VARYING,
};
use crate::graphite::resource_types::{DstReadStrategy, Layout, SamplerDesc};
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_code_dictionary::{
    LiftableExpressionType, ShaderCodeDictionary, ShaderNode, ShaderSnippetArgs,
    SnippetRequirementFlags,
};
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::uniform::{K_NON_ARRAY, Uniform};
use crate::graphite::uniform_manager::UniformOffsetCalculator;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;
use crate::sksl_type_shared::SkSLType;

/// `kFixedVaryings`: the varyings the key layer reserves (the SSBO index and the local coords).
// Port of: src/gpu/graphite/ShaderInfo.cpp#L972 (chrome/m156)
const FIXED_VARYINGS: i32 = 2;

/// `MeshRenderStep::kMeshVaryingMangleSuffix`.
// Port of: src/gpu/graphite/render/MeshRenderStep.h#L27 (chrome/m156)
pub const MESH_VARYING_MANGLE_SUFFIX: &str = "_SkMeshSpecificationUniform";

/// A vertex or instance attribute whose name may be owned (`Attribute` for a mesh's attributes,
/// which `Attribute::MakeFromSkMeshAttribute` creates with a pointer into the specification).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderAttribute {
    name: String,
    cpu_type: VertexAttribType,
    gpu_type: SkSLType,
}

impl ShaderAttribute {
    /// `Attribute::MakeFromSkMeshAttribute(attr)`.
    // Port of: src/gpu/graphite/Attribute.h#L48-L72 (chrome/m156)
    #[doc(alias = "MakeFromSkMeshAttribute")]
    #[must_use]
    pub fn from_mesh_attribute(attr: &mesh::Attribute) -> Self {
        let (cpu_type, gpu_type) = match attr.ty {
            mesh::AttributeType::Float => (VertexAttribType::Float, SkSLType::Float),
            mesh::AttributeType::Float2 => (VertexAttribType::Float2, SkSLType::Float2),
            mesh::AttributeType::Float3 => (VertexAttribType::Float3, SkSLType::Float3),
            mesh::AttributeType::Float4 => (VertexAttribType::Float4, SkSLType::Float4),
            mesh::AttributeType::UByte4Unorm => (VertexAttribType::UByte4Norm, SkSLType::Half4),
        };
        Self {
            name: attr.name.clone(),
            cpu_type,
            gpu_type,
        }
    }

    /// `name()`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `cpuType()`.
    #[must_use]
    pub fn cpu_type(&self) -> VertexAttribType {
        self.cpu_type
    }

    /// `gpuType()`.
    #[must_use]
    pub fn gpu_type(&self) -> SkSLType {
        self.gpu_type
    }

    /// `size()`: the size of the CPU-side attribute in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.cpu_type.size()
    }

    /// `sizeAlign4()`.
    #[doc(alias = "sizeAlign4")]
    #[must_use]
    pub fn size_align4(&self) -> usize {
        (self.size() + 3) & !3
    }
}

impl From<&Attribute> for ShaderAttribute {
    fn from(a: &Attribute) -> Self {
        Self {
            name: a.name().to_owned(),
            cpu_type: a.cpu_type(),
            gpu_type: a.gpu_type(),
        }
    }
}

/// `SkMeshSpecificationPriv::VaryingTypeAsSLType`.
// Port of: src/core/SkMeshPriv.h#L40-L52 (chrome/m156)
fn varying_type_as_sksl_type(ty: mesh::VaryingType) -> SkSLType {
    match ty {
        mesh::VaryingType::Float => SkSLType::Float,
        mesh::VaryingType::Float2 => SkSLType::Float2,
        mesh::VaryingType::Float3 => SkSLType::Float3,
        mesh::VaryingType::Float4 => SkSLType::Float4,
        mesh::VaryingType::Half => SkSLType::Half,
        mesh::VaryingType::Half2 => SkSLType::Half2,
        mesh::VaryingType::Half3 => SkSLType::Half3,
        mesh::VaryingType::Half4 => SkSLType::Half4,
    }
}

/// An expression lifted from the fragment to the vertex shader (the file-local
/// `LiftedExpression`).
// Port of: src/gpu/graphite/ShaderInfo.cpp#L28-L36 (chrome/m156)
struct LiftedExpression<'a> {
    /// The node who's expression should be lifted.
    node: &'a ShaderNode,
    /// The arguments to use as input to the lifted expression.
    args: ShaderSnippetArgs,
    /// If true, capture the expression's resolved value in a varying. This is false for
    /// expressions whose output is only used in other lifted expressions.
    emit_varying: bool,
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L38-L43 (chrome/m156)
fn get_uniform_header(set: i32, buffer_id: i32) -> String {
    format!("layout (set={set}, binding={buffer_id}) uniform CombinedUniforms {{\n")
}

/// The name of a uniform, or `None` when it is a paint color that has been written already.
// Port of: src/gpu/graphite/ShaderInfo.cpp#L45-L71, #L123-L150 (chrome/m156)
fn uniform_name(
    u: &Uniform,
    mangling_suffix: i32,
    wrote_paint_color: &mut Option<&mut bool>,
) -> Option<String> {
    let mut name = u.name().to_owned();
    match wrote_paint_color {
        Some(wrote) if u.is_paint_color() => {
            if **wrote {
                return None;
            }
            **wrote = true;
        }
        _ => {
            if mangling_suffix >= 0 {
                name.push('_');
                name.push_str(&mangling_suffix.to_string());
            }
        }
    }
    Some(name)
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L45-L85 (chrome/m156)
fn get_uniforms(
    offsetter: &mut UniformOffsetCalculator,
    uniforms: &[Uniform],
    mangling_suffix: i32,
    mut wrote_paint_color: Option<&mut bool>,
) -> String {
    let mut result = String::new();
    for u in uniforms {
        let Some(name) = uniform_name(u, mangling_suffix, &mut wrote_paint_color) else {
            continue;
        };
        let _ = write!(
            result,
            "layout(offset={}) {} {}",
            offsetter.advance_offset(u.ty(), u.count()),
            u.ty().as_str(),
            name
        );
        if u.count() != 0 {
            result.push('[');
            result.push_str(&u.count().to_string());
            result.push(']');
        }
        result.push_str(";\n");
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L87-L121 (chrome/m156)
fn get_node_uniforms(
    offsetter: &mut UniformOffsetCalculator,
    node: &ShaderNode,
    num_uniforms: &mut i32,
    num_unlifted_uniforms: &mut i32,
    wrote_paint_color: &mut bool,
) -> String {
    let mut result = String::new();
    let uniforms = &node.entry().uniforms;

    if !uniforms.is_empty() {
        let n = i32::try_from(uniforms.len()).expect("a few uniforms");
        *num_uniforms += n;
        if !(node
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
            || node
                .required_flags()
                .contains(SnippetRequirementFlags::OMIT_EXPRESSION))
        {
            *num_unlifted_uniforms += n;
        }

        if let Some(struct_name) = node.entry().uniform_struct_name {
            let mut substruct = UniformOffsetCalculator::for_struct(offsetter.layout());
            for u in uniforms {
                substruct.advance_offset(u.ty(), u.count());
            }

            let struct_offset = offsetter.advance_struct(&substruct, K_NON_ARRAY);
            let _ = write!(
                result,
                "layout(offset={struct_offset}) {struct_name} node_{};",
                node.key_index()
            );
        } else {
            result += &get_uniforms(
                offsetter,
                uniforms,
                node.key_index(),
                Some(&mut *wrote_paint_color),
            );
        }
    }

    for child in node.children() {
        result += &get_node_uniforms(
            offsetter,
            child,
            num_uniforms,
            num_unlifted_uniforms,
            wrote_paint_color,
        );
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L123-L156 (chrome/m156)
fn get_ssbo_fields(
    uniforms: &[Uniform],
    mangling_suffix: i32,
    mut wrote_paint_color: Option<&mut bool>,
) -> String {
    let mut result = String::new();
    for u in uniforms {
        let Some(name) = uniform_name(u, mangling_suffix, &mut wrote_paint_color) else {
            continue;
        };
        let _ = write!(result, "{} {}", u.ty().as_str(), name);
        if u.count() != 0 {
            let _ = write!(result, "[{}]", u.count());
        }
        result.push_str(";\n");
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L158-L190 (chrome/m156)
fn get_node_ssbo_fields(
    node: &ShaderNode,
    num_uniforms: &mut i32,
    num_unlifted_uniforms: &mut i32,
    wrote_paint_color: &mut bool,
) -> String {
    let mut result = String::new();
    let uniforms = &node.entry().uniforms;

    if !uniforms.is_empty() {
        let n = i32::try_from(uniforms.len()).expect("a few uniforms");
        *num_uniforms += n;
        if !(node
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
            || node
                .required_flags()
                .contains(SnippetRequirementFlags::OMIT_EXPRESSION))
        {
            *num_unlifted_uniforms += n;
        }

        if let Some(struct_name) = node.entry().uniform_struct_name {
            let _ = write!(result, "{struct_name} node_{};", node.key_index());
        } else {
            result += &get_ssbo_fields(uniforms, node.key_index(), Some(&mut *wrote_paint_color));
        }
    }

    for child in node.children() {
        result += &get_node_ssbo_fields(
            child,
            num_uniforms,
            num_unlifted_uniforms,
            wrote_paint_color,
        );
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L192-L220 (chrome/m156)
fn emit_intrinsic_constants(binding_reqs: &ResourceBindingRequirements) -> String {
    let mut offsetter =
        UniformOffsetCalculator::for_top_level(binding_reqs.uniform_buffer_layout, 0);
    let mut result = if binding_reqs.use_push_constants_for_intrinsic_constants {
        debug_assert!(matches!(
            binding_reqs.backend_api,
            BackendApi::Vulkan | BackendApi::Dawn
        ));
        format!(
            "layout ({}, push_constant) uniform IntrinsicUniforms {{\n",
            if binding_reqs.backend_api == BackendApi::Vulkan {
                "vulkan"
            } else {
                "webgpu"
            }
        )
    } else {
        format!(
            "layout (set={}, binding={}) uniform IntrinsicUniforms {{\n",
            binding_reqs.uniforms_set_idx, binding_reqs.intrinsic_buffer_binding
        )
    };
    result += &get_uniforms(&mut offsetter, &INTRINSIC_UNIFORMS, -1, None);
    result.push_str("};\n\n");
    debug_assert!(
        binding_reqs.use_push_constants_for_intrinsic_constants || !result.contains('['),
        "Arrays are not supported in intrinsic uniforms"
    );
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L222-L255 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn emit_combined_uniforms(
    set: i32,
    buffer_id: i32,
    layout: Layout,
    nodes: &[ShaderNode],
    step_uniforms: &[Uniform],
    num_paint_uniforms: &mut i32,
    num_unlifted_paint_uniforms: &mut i32,
    wrote_paint_color: &mut bool,
) -> String {
    let mut offsetter = UniformOffsetCalculator::for_top_level(layout, 0);

    let mut result = get_uniform_header(set, buffer_id);
    for n in nodes {
        result += &get_node_uniforms(
            &mut offsetter,
            n,
            num_paint_uniforms,
            num_unlifted_paint_uniforms,
            wrote_paint_color,
        );
    }

    // Paint and RenderStep uniforms share a binding. When RenderSteps are processed in DrawList,
    // the paint uniforms are always processed before the render step ones, so the emitted uniforms
    // must respect that ordering.
    if !step_uniforms.is_empty() {
        result += &get_uniforms(&mut offsetter, step_uniforms, -1, None);
    }

    result.push_str("};\n\n");

    if *num_paint_uniforms == 0 && step_uniforms.is_empty() {
        // No uniforms were added
        return String::new();
    }

    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L257-L286 (chrome/m156)
fn emit_combined_storage_buffer(
    set: i32,
    buffer_id: i32,
    nodes: &[ShaderNode],
    step_uniforms: &[Uniform],
    num_paint_uniforms: &mut i32,
    num_unlifted_paint_uniforms: &mut i32,
    wrote_paint_color: &mut bool,
) -> String {
    let mut fields = String::new();
    for n in nodes {
        fields += &get_node_ssbo_fields(
            n,
            num_paint_uniforms,
            num_unlifted_paint_uniforms,
            wrote_paint_color,
        );
    }

    if !step_uniforms.is_empty() {
        fields += &get_ssbo_fields(step_uniforms, -1, None);
    }

    if *num_paint_uniforms == 0 && step_uniforms.is_empty() {
        // No uniforms were added
        return String::new();
    }

    format!(
        "struct CombinedUniformData {{{fields}}};\n\
         layout (set={set}, binding={buffer_id}) readonly buffer CombinedUniforms {{\
         CombinedUniformData combinedUniformData[];}};\n"
    )
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L288-L305 (chrome/m156)
fn emit_uniforms_from_storage_buffer(index_variable_name: &str, uniforms: &[Uniform]) -> String {
    let mut result = String::new();

    for u in uniforms {
        let _ = write!(result, "{} {}", u.ty().as_str(), u.name());
        if u.count() != 0 {
            let _ = write!(result, "[{}]", u.count());
        }
        let _ = writeln!(
            result,
            " = combinedUniformData[{index_variable_name}].{};",
            u.name()
        );
    }

    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L307-L338 (chrome/m156)
fn emit_step_storage_buffer(
    binding_reqs: &ResourceBindingRequirements,
    step: &dyn RenderStep,
) -> String {
    // NOTE: currently no renderstep declares fsUsesStorage() = true. Only gradients use fragment
    // shader storage, but they emit their storage buffer declaration directly inside
    // generateFragmentSkSL(). This is redundant and will be fixed in a follow-on patch.
    debug_assert!(!step.fs_uses_storage());

    debug_assert!(step.num_storage_uniforms() > 0);
    let mut fields = String::new();
    for a in step.storage_uniforms() {
        let _ = write!(fields, "{} {}", a.ty().as_str(), a.name());
        if a.count() != 0 {
            fields.push('[');
            fields.push_str(&a.count().to_string());
            fields.push(']');
        }
        fields.push_str(";\n");
    }
    format!(
        "struct StepStorageData {{\n{fields}}};\n\
         layout (set={}, binding={}) readonly buffer StepStorageBuffer {{\n\
         \x20   StepStorageData stepStorageData[];\n\
         }};\n\
         inline StepStorageData readStepStorageData(int idx) {{\n\
         \x20   return stepStorageData[idx];\n\
         }}\n\
         inline StepStorageData readStepStorageData(uint idx) {{\n\
         \x20   return stepStorageData[idx];\n\
         }}\n",
        binding_reqs.uniforms_set_idx, binding_reqs.storage_buffer_binding
    )
}

/// The component names of a texel (`kSwizzles`).
const SWIZZLES: [&str; 4] = ["x", "y", "z", "w"];

/// `EmitStorageFallbackTexture(bindingReqs, step)`: the `SkSL` that reads a step's storage
/// uniforms from a texture when the device has no storage buffers.
///
/// # Panics
/// If a storage uniform's offset is negative, which the layout calculator does not produce.
// Port of: src/gpu/graphite/ShaderInfo.cpp#L340-L453 (chrome/m156)
#[doc(alias = "EmitStorageFallbackTexture")]
#[must_use]
pub fn emit_storage_fallback_texture(
    binding_reqs: &ResourceBindingRequirements,
    step: &dyn RenderStep,
) -> String {
    if step.storage_uniforms().is_empty() {
        return String::new();
    }

    let mut fields = String::new();
    for a in step.storage_uniforms() {
        // Array storage uniforms are not supported by the texture fallback.
        debug_assert_eq!(a.count(), 0);
        let _ = writeln!(fields, "    {} {};", a.ty().as_str(), a.name());
    }

    let mut field_assignments = String::new();

    // Force Std430 rules to match the struct layout defined by RenderStep.
    let mut calculator = UniformOffsetCalculator::for_struct(Layout::Std430);

    for a in step.storage_uniforms() {
        let byte_offset = calculator.advance_offset(a.ty(), a.count());
        let float_offset = byte_offset / 4;
        let start_texel = float_offset / 4;
        let start_comp = usize::try_from(float_offset % 4).expect("a non-negative offset");

        let mat = a.ty().matrix_size();
        let vec = a.ty().vec_length();

        let mut expr = if mat == 2 {
            if start_comp == 0 {
                format!("float2x2(t{start_texel}.xy, t{start_texel}.zw)")
            } else {
                format!("float2x2(t{start_texel}.zw, t{}.xy)", start_texel + 1)
            }
        } else if mat == 3 {
            // std140/std430 dictates 16-byte alignment for 3-component column vectors.
            debug_assert_eq!(start_comp, 0);
            format!(
                "float3x3(t{start_texel}.xyz, t{}.xyz, t{}.xyz)",
                start_texel + 1,
                start_texel + 2
            )
        } else if mat == 4 {
            // std140/std430 dictates 16-byte alignment for 4-component column vectors.
            debug_assert_eq!(start_comp, 0);
            format!(
                "float4x4(t{start_texel}, t{}, t{}, t{})",
                start_texel + 1,
                start_texel + 2,
                start_texel + 3
            )
        } else if vec == 4 {
            // std140/std430 dictates 16-byte alignment for 4-component vectors.
            debug_assert_eq!(start_comp, 0);
            format!("t{start_texel}")
        } else if vec == 3 {
            // std140/std430 dictates 16-byte alignment for vec3; startComp must be 0 to avoid OOB
            // swizzles.
            debug_assert_eq!(start_comp, 0);
            format!(
                "t{start_texel}.{}{}{}",
                SWIZZLES[start_comp],
                SWIZZLES[start_comp + 1],
                SWIZZLES[start_comp + 2]
            )
        } else if vec == 2 {
            format!(
                "t{start_texel}.{}{}",
                SWIZZLES[start_comp],
                SWIZZLES[start_comp + 1]
            )
        } else {
            format!("t{start_texel}.{}", SWIZZLES[start_comp])
        };

        match a.ty() {
            SkSLType::Int | SkSLType::Int2 | SkSLType::Int3 | SkSLType::Int4 => {
                expr = format!("floatBitsToInt({expr})");
            }
            SkSLType::UInt | SkSLType::UInt2 | SkSLType::UInt3 | SkSLType::UInt4 => {
                expr = format!("floatBitsToUint({expr})");
            }
            _ => {}
        }

        let _ = writeln!(field_assignments, "    data.{} = {expr};", a.name());
    }

    let n_texels = (calculator.size() + 15) / 16;
    let mut texel_loads = format!(
        "    const int texWidth = {};\n",
        binding_reqs.max_fallback_texture_size
    );
    for i in 0..n_texels {
        let _ = write!(
            texel_loads,
            "    int linearIdx{i} = index * {n_texels} + {i};\n\
             \x20   int2 coords{i} = int2(linearIdx{i} % texWidth, linearIdx{i} / texWidth);\n\
             \x20   float4 t{i} = float4(textureRead(storageFallbackTexture, uint2(coords{i})));\n"
        );
    }

    format!(
        "layout(set={}, binding={}) readonly texture2D storageFallbackTexture;\n\
         struct StepStorageData {{\n{fields}}};\n\
         StepStorageData readStepStorageData(int index) {{\n\
         {texel_loads}\
         \x20   StepStorageData data;\n\
         {field_assignments}\
         \x20   return data;\n\
         }}\n\
         inline StepStorageData readStepStorageData(uint index) {{\n\
         \x20   return readStepStorageData(int(index));\n\
         }}\n",
        binding_reqs.uniforms_set_idx, binding_reqs.storage_buffer_binding
    )
}

/// Appends the `SamplerDesc`s of the `samplerData` words (`append_sampler_descs`).
// Port of: src/gpu/graphite/ShaderInfo.cpp#L455-L482 (chrome/m156)
fn append_sampler_descs(sampler_data: &[u32], out_descs: &mut Vec<SamplerDesc>) {
    // Sampler data consists of variable-length SamplerDesc representations which can differ based
    // upon a sampler's immutability and format. For this reason, handle incrementing i in the loop.
    let mut i = 0;
    while i < sampler_data.len() {
        // Create a default-initialized SamplerDesc (which only takes up one uint32). If we are
        // using a dynamic sampler, this will be directly inserted into outDescs. Otherwise, it will
        // be populated with actual immutable sampler data and then inserted.
        let mut desc = SamplerDesc::default();
        let mut sampler_desc_length = 1;
        debug_assert_eq!(desc.as_span().len(), sampler_desc_length);

        // Isolate the ImmutableSamplerInfo portion of the SamplerDesc represented by samplerData.
        // If immutableSamplerInfo is non-zero, that means we are using an immutable sampler.
        let immutable_sampler_info = sampler_data[i] >> SamplerDesc::IMMUTABLE_SAMPLER_INFO_SHIFT;
        if immutable_sampler_info != 0 {
            // Consult the first bit of immutableSamplerInfo which tells us whether the sampler uses
            // a known or external format. With this, update sampler description length.
            let uses_external_format = immutable_sampler_info & 0b1 != 0;
            sampler_desc_length = if uses_external_format {
                SamplerDesc::INT32S_NEEDED_EXTERNAL_FORMAT
            } else {
                SamplerDesc::INT32S_NEEDED_KNOWN_FORMAT
            };
            // Populate a SamplerDesc with samplerDescLength quantity of immutable sampler data
            desc = SamplerDesc::from_raw(
                sampler_data[i],
                sampler_data[i + 1],
                if uses_external_format {
                    sampler_data[i + 2]
                } else {
                    0
                },
            );
        }
        out_descs.push(desc);
        i += sampler_desc_length;
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L484-L538 (chrome/m156)
fn get_node_texture_samplers(
    binding_reqs: &ResourceBindingRequirements,
    node: &ShaderNode,
    binding: &mut i32,
    mut out_descs: Option<&mut Vec<SamplerDesc>>,
) -> String {
    let mut result = String::new();
    let samplers = &node.entry().textures_and_samplers;

    if !samplers.is_empty() {
        // Determine whether we need to analyze & interpret a ShaderNode's data as immutable
        // SamplerDescs based upon whether:
        // 1) A backend passes in a non-nullptr outImmutableSamplers param (may be nullptr in
        //    backends or circumstances where we know immutable sampler data is never stored)
        // 2) Any data is stored on the ShaderNode
        // 3) Whether the ShaderNode snippet's ID matches that of any snippet ID that could store
        //    immutable sampler data.
        let snippet_id = node.code_snippet_id();
        if let Some(out_descs) = out_descs.as_deref_mut() {
            // TODO(b/369846881): Refactor checking snippet ID to instead having a named
            // snippet requirement flag that we can check here to decrease fragility.
            if !node.data().is_empty()
                && (snippet_id == BuiltInCodeSnippetID::ImageShader as i32
                    || snippet_id == BuiltInCodeSnippetID::ImageShaderClamp as i32
                    || snippet_id == BuiltInCodeSnippetID::CubicImageShader as i32
                    || snippet_id == BuiltInCodeSnippetID::HWImageShader as i32)
            {
                append_sampler_descs(node.data(), out_descs);
            } else {
                // Add default SamplerDescs for any dynamic samplers to outDescs.
                out_descs.extend(std::iter::repeat_n(SamplerDesc::default(), samplers.len()));
            }
        }

        for t in samplers {
            result += &emit_sampler_layout(binding_reqs, binding);
            let _ = writeln!(result, " sampler2D {}_{};", t.name(), node.key_index());
        }
    }

    for child in node.children() {
        result +=
            &get_node_texture_samplers(binding_reqs, child, binding, out_descs.as_deref_mut());
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L540-L551 (chrome/m156)
fn emit_textures_and_samplers(
    binding_reqs: &ResourceBindingRequirements,
    nodes: &[ShaderNode],
    binding: &mut i32,
    mut out_descs: Option<&mut Vec<SamplerDesc>>,
) -> String {
    let mut result = String::new();
    for n in nodes {
        result += &get_node_texture_samplers(binding_reqs, n, binding, out_descs.as_deref_mut());
    }
    result
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L553-L564 (chrome/m156)
fn sksl_type_for_lifted_expression(ty: LiftableExpressionType) -> SkSLType {
    match ty {
        LiftableExpressionType::None => SkSLType::Void,
        LiftableExpressionType::LocalCoords => SkSLType::Float2,
        LiftableExpressionType::PriorStageOutput => SkSLType::Half4,
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L566-L626 (chrome/m156)
fn emit_varyings(
    step: &dyn RenderStep,
    direction: &str,
    roots: &RootNodesInfo,
    lifted_expressions: &[LiftedExpression<'_>],
    emit_ssbo_index_varying: bool,
    emit_local_coords_varying: bool,
) -> String {
    let mut result = String::new();
    let mut location = 0;

    // (`Varying`'s constructor makes integral types flat.)
    let mut append_varying =
        |name: &str, gpu_type: SkSLType, interpolation: Interpolation, mangle_suffix: &str| {
            let interpolation = if gpu_type.is_integral_type() {
                Interpolation::Flat
            } else {
                interpolation
            };
            let interpolation = match interpolation {
                Interpolation::Perspective => "",
                Interpolation::Linear => "noperspective ",
                Interpolation::Flat => "flat ",
            };
            let _ = writeln!(
                result,
                "layout(location={location}) {direction} {interpolation}{} {name}{mangle_suffix};",
                gpu_type.as_str()
            );
            location += 1;
        };

    if emit_ssbo_index_varying {
        append_varying(
            SSBO_INDEX_VARYING,
            SkSLType::UInt,
            Interpolation::Perspective,
            "",
        );
    }

    if emit_local_coords_varying {
        append_varying(
            "localCoordsVar",
            SkSLType::Float2,
            Interpolation::Perspective,
            "",
        );
    }

    for expr in lifted_expressions {
        if expr.emit_varying {
            let node = expr.node;
            let name = node.get_expression_varying_name();
            append_varying(
                &name,
                sksl_type_for_lifted_expression(node.entry().liftable_expression_type),
                node.entry().liftable_expression_interpolation,
                "",
            );
        }
    }

    for v in step.varyings() {
        append_varying(v.name(), v.gpu_type(), v.interpolation(), "");
    }

    if let Some(mesh_spec) = &roots.mesh_spec {
        for v in mesh_spec.varyings() {
            let ty = varying_type_as_sksl_type(v.ty);
            append_varying(
                &v.name,
                ty,
                Interpolation::Perspective,
                MESH_VARYING_MANGLE_SUFFIX,
            );
        }
    }

    result
}

/// Walks the node tree and generates all preambles, accumulating into `preamble`.
// Port of: src/gpu/graphite/ShaderInfo.cpp#L628-L656 (chrome/m156)
fn emit_preambles(
    shader_info: &ShaderInfo,
    nodes: &[ShaderNode],
    tree_label: &str,
    preamble: &mut String,
) {
    for (i, node) in nodes.iter().enumerate() {
        let node_label = i.to_string();
        let next_label = if tree_label.is_empty() {
            node_label
        } else {
            format!("{tree_label}<-{node_label}")
        };

        if node.num_children() > 0 {
            emit_preambles(shader_info, node.children(), &next_label, preamble);
        }

        let node_preamble = match node.entry().preamble_generator {
            Some(generator) => generator(shader_info, node),
            None => node.generate_default_preamble(shader_info),
        };
        if !node_preamble.is_empty() {
            // (The `SK_DEBUG` build also writes a `// [%d]   %s: %s` comment line.)
            *preamble += &node_preamble;
            preamble.push('\n');
        }
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L658-L684 (chrome/m156)
fn emit_color_output(output_type: OutputType, out_color: &str, in_color: &str) -> String {
    match output_type {
        OutputType::None => format!("{out_color} = half4(0.0);"),
        OutputType::Coverage => format!("{out_color} = outputCoverage;"),
        OutputType::Modulate => format!("{out_color} = {in_color} * outputCoverage;"),
        OutputType::SAModulate => format!("{out_color} = {in_color}.a * outputCoverage;"),
        OutputType::ISAModulate => {
            format!("{out_color} = (1.0 - {in_color}.a) * outputCoverage;")
        }
        OutputType::ISCModulate => {
            format!("{out_color} = (half4(1.0) - {in_color}) * outputCoverage;")
        }
    }
}

/// When using hardware for advanced blend modes, coverage is applied by multiplying it into the
/// src color before blending (the long derivation in `ShaderInfo.cpp` proves that this "just
/// works" for the SVG/PDF blend equations with X=Y=Z=1).
// Port of: src/gpu/graphite/ShaderInfo.cpp#L686-L805 (chrome/m156)
fn emit_advanced_blend_color_output(out_color: &str, in_color: &str) -> String {
    emit_color_output(OutputType::Modulate, out_color, in_color)
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L807-L840 (chrome/m156)
fn collect_lifted_expressions<'a>(
    nodes: &'a [ShaderNode],
    args: &ShaderSnippetArgs,
    lifted: &mut Vec<LiftedExpression<'a>>,
) {
    for node in nodes {
        let emit_varying_in_fs = node
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION);
        let emit_expression_in_vs = emit_varying_in_fs
            || node
                .required_flags()
                .contains(SnippetRequirementFlags::OMIT_EXPRESSION);
        debug_assert!(
            !emit_expression_in_vs || node.entry().liftable_expression_generator.is_some()
        );

        let mut child_args = args.clone();
        if emit_expression_in_vs && node.entry().liftable_expression_generator.is_some() {
            lifted.push(LiftedExpression {
                node,
                args: args.clone(),
                emit_varying: emit_varying_in_fs,
            });
            match node.entry().liftable_expression_type {
                LiftableExpressionType::LocalCoords => {
                    child_args.frag_coord = node.get_expression_varying_name();
                }
                LiftableExpressionType::PriorStageOutput => {
                    child_args.prior_stage_output = node.get_expression_varying_name();
                }
                LiftableExpressionType::None => unreachable!("SkUNREACHABLE"),
            }
        }

        collect_lifted_expressions(node.children(), &child_args, lifted);
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L842-L849 (chrome/m156)
fn collect_lifted_expressions_of(nodes: &[ShaderNode]) -> Vec<LiftedExpression<'_>> {
    let mut lifted = Vec::new();
    let mut args = ShaderSnippetArgs::default_args();
    args.frag_coord = String::from("stepLocalCoords"); // Render Steps' stepLocalCoords
    collect_lifted_expressions(nodes, &args, &mut lifted);
    lifted
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L851-L866 (chrome/m156)
fn dst_read_strategy_to_str(strategy: DstReadStrategy) -> &'static str {
    match strategy {
        DstReadStrategy::NoneRequired => "NoneRequired",
        DstReadStrategy::TextureCopy => "TextureCopy",
        DstReadStrategy::TextureSample => "TextureSample",
        DstReadStrategy::ReadFromInput => "ReadFromInput",
        DstReadStrategy::FramebufferFetch => "FramebufferFetch",
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L868-L875 (chrome/m156)
const fn make_simple_blend_info(src_coeff: BlendCoeff, dst_coeff: BlendCoeff) -> BlendInfo {
    BlendInfo {
        equation: BlendEquation::Add,
        src_blend: src_coeff,
        dst_blend: dst_coeff,
        blend_constant: PMColor4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }, // SK_PMColor4fTRANSPARENT
        writes_color: blend_modifies_dst(BlendEquation::Add, src_coeff, dst_coeff),
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L877-L911 (chrome/m156)
fn get_advanced_blend_equation(mode: BlendMode) -> BlendEquation {
    debug_assert!(mode > BlendMode::LAST_COEFF_MODE);
    match mode {
        BlendMode::Overlay => BlendEquation::Overlay,
        BlendMode::Darken => BlendEquation::Darken,
        BlendMode::Lighten => BlendEquation::Lighten,
        BlendMode::ColorDodge => BlendEquation::ColorDodge,
        BlendMode::ColorBurn => BlendEquation::ColorBurn,
        BlendMode::HardLight => BlendEquation::HardLight,
        BlendMode::SoftLight => BlendEquation::SoftLight,
        BlendMode::Difference => BlendEquation::Difference,
        BlendMode::Exclusion => BlendEquation::Exclusion,
        BlendMode::Multiply => BlendEquation::Multiply,
        BlendMode::Hue => BlendEquation::HslHue,
        BlendMode::Saturation => BlendEquation::HslSaturation,
        BlendMode::Color => BlendEquation::HslColor,
        BlendMode::Luminosity => BlendEquation::HslLuminosity,
        _ => unreachable!("not an advanced blend mode"),
    }
}

// Port of: src/gpu/graphite/ShaderInfo.cpp#L913-L917 (chrome/m156)
fn make_hardware_advanced_blend_info(advanced_blend_mode: BlendMode) -> BlendInfo {
    BlendInfo {
        equation: get_advanced_blend_equation(advanced_blend_mode),
        ..BlendInfo::default()
    }
}

/// `gBlendTable[mode]`: the hardware blend state of each blend mode.
// Port of: src/gpu/graphite/ShaderInfo.cpp#L919-L953 (chrome/m156)
fn blend_table(mode: BlendMode) -> BlendInfo {
    use BlendCoeff as C;
    match mode {
        /* Porter-Duff blend modes */
        BlendMode::Clear => make_simple_blend_info(C::Zero, C::Zero),
        BlendMode::Src => make_simple_blend_info(C::One, C::Zero),
        BlendMode::Dst => make_simple_blend_info(C::Zero, C::One),
        BlendMode::SrcOver => make_simple_blend_info(C::One, C::ISA),
        BlendMode::DstOver => make_simple_blend_info(C::IDA, C::One),
        BlendMode::SrcIn => make_simple_blend_info(C::DA, C::Zero),
        BlendMode::DstIn => make_simple_blend_info(C::Zero, C::SA),
        BlendMode::SrcOut => make_simple_blend_info(C::IDA, C::Zero),
        BlendMode::DstOut => make_simple_blend_info(C::Zero, C::ISA),
        BlendMode::SrcATop => make_simple_blend_info(C::DA, C::ISA),
        BlendMode::DstATop => make_simple_blend_info(C::IDA, C::SA),
        BlendMode::Xor => make_simple_blend_info(C::IDA, C::ISA),
        BlendMode::Plus => make_simple_blend_info(C::One, C::One),
        BlendMode::Modulate => make_simple_blend_info(C::Zero, C::SC),
        BlendMode::Screen => make_simple_blend_info(C::One, C::ISC),
        /* BlendInfo for advanced blend modes */
        advanced => make_hardware_advanced_blend_info(advanced),
    }
}

/// The state computed by the constructor of `ShaderInfo::SharedGeneratorData` that both shader
/// stages read.
// Port of: src/gpu/graphite/ShaderInfo.cpp#L955-L1076 (chrome/m156)
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ struct's flags
struct SharedGeneratorData<'a> {
    /// The shader tree decompressed into explicit root nodes.
    roots_info: &'a RootNodesInfo,

    /// The expressions lifted from the shader tree.
    lifted_expr: Vec<LiftedExpression<'a>>,

    /// The base `SkSL` preamble (uniforms, varyings) shared by both stages.
    shared_preamble: String,

    // Shared calculated properties
    needs_local_coords: bool,
    has_ssbo_index_varying: bool,
    has_step_uniforms: bool,
    has_paint_uniforms: bool,
    #[allow(dead_code)] // `fHasLiftedPaintUniforms`: computed like Skia, which does not read it
    has_lifted_paint_uniforms: bool,

    // Buffer usage flags needed by generators
    use_uniform_storage_buffer_vs: bool,
    use_uniform_storage_buffer_fs: bool,
}

impl<'a> SharedGeneratorData<'a> {
    // Port of: src/gpu/graphite/ShaderInfo.cpp#L955-L1076 (chrome/m156)
    #[allow(clippy::similar_names)] // the `Vs` and `Fs` flags, as in Skia
    fn new(
        caps: &dyn Caps,
        step: &dyn RenderStep,
        roots_info: &'a RootNodesInfo,
        has_paint: bool,
        uniform_ssbo_index: Option<&str>,
    ) -> Self {
        let has_step_uniforms = step.num_uniforms() > 0;

        // Determine Local Coords
        let needs_local_coords = has_paint
            && roots_info.src_color().is_some_and(|src| {
                src.required_flags()
                    .contains(SnippetRequirementFlags::LOCAL_COORDS)
            });

        let mut all_step_uniforms: Vec<Uniform> = step.uniforms().to_vec();

        // Declare SkMesh uniforms after the render step uniforms are declared so the
        // MeshRenderStep can write the uniforms and have them an the expected location.
        if let Some(mesh_spec) = &roots_info.mesh_spec {
            debug_assert!(has_step_uniforms);
            debug_assert!(roots_info.mesh_shader().is_some());
            debug_assert_eq!(
                step.render_step_id(),
                crate::graphite::render_step::RenderStepID::Mesh
            );
            all_step_uniforms.extend(ShaderCodeDictionary::convert_runtime_effect_uniforms(
                mesh_spec.uniforms(),
            ));
        }

        // Lift Expressions & Check Uniforms
        let lifted_expr = collect_lifted_expressions_of(&roots_info.roots);
        let vs_has_lifted_paint_uniforms = lifted_expr
            .iter()
            .any(|expr| !expr.node.entry().uniforms.is_empty());

        let needs_combined_buffer_vs = has_step_uniforms || vs_has_lifted_paint_uniforms;
        let use_uniform_storage_buffer_vs =
            caps.storage_buffer_support() && needs_combined_buffer_vs;
        let use_uniform_storage_buffer_fs =
            caps.storage_buffer_support() && step.performs_shading();
        let use_uniform_storage_buffer =
            use_uniform_storage_buffer_vs || use_uniform_storage_buffer_fs;

        // Emit Preamble
        let mut num_paint_uniforms = 0;
        let mut num_unlifted_paint_uniforms = 0;
        let mut wrote_paint_color = false;
        let binding_reqs = caps.resource_binding_requirements();

        let mut shared_preamble = if use_uniform_storage_buffer {
            emit_combined_storage_buffer(
                binding_reqs.uniforms_set_idx,
                binding_reqs.combined_uniform_buffer_binding,
                &roots_info.roots,
                &all_step_uniforms,
                &mut num_paint_uniforms,
                &mut num_unlifted_paint_uniforms,
                &mut wrote_paint_color,
            )
        } else {
            emit_combined_uniforms(
                binding_reqs.uniforms_set_idx,
                binding_reqs.combined_uniform_buffer_binding,
                binding_reqs.uniform_buffer_layout,
                &roots_info.roots,
                &all_step_uniforms,
                &mut num_paint_uniforms,
                &mut num_unlifted_paint_uniforms,
                &mut wrote_paint_color,
            )
        };

        // Calculate Final Flags
        let has_paint_uniforms = num_paint_uniforms > 0;
        let has_lifted_paint_uniforms = (num_paint_uniforms - num_unlifted_paint_uniforms) > 0;
        let has_unlifted_paint_uniforms = num_unlifted_paint_uniforms > 0;

        let has_ssbo_index_varying = use_uniform_storage_buffer
            && (has_unlifted_paint_uniforms
                || (has_step_uniforms && step.uses_uniforms_in_fragment_sksl()));

        // Append SSBO Index to preamble if required
        if let (true, Some(index)) = (use_uniform_storage_buffer, uniform_ssbo_index) {
            let _ = writeln!(shared_preamble, "uint {index};");
        }

        Self {
            roots_info,
            lifted_expr,
            shared_preamble,
            needs_local_coords,
            has_ssbo_index_varying,
            has_step_uniforms,
            has_paint_uniforms,
            has_lifted_paint_uniforms,
            use_uniform_storage_buffer_vs,
            use_uniform_storage_buffer_fs,
        }
    }
}

/// Holds all root `ShaderNode`s defined for a `PaintParams` as well as the extracted fixed
/// function blending parameters and other aggregate requirements for the effect trees that have
/// been linked into a single fragment program.
// Port of: src/gpu/graphite/ShaderInfo.h#L33-L146 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShaderInfo")]
#[derive(Debug)]
pub struct ShaderInfo {
    shader_code_dictionary: ShaderCodeDictionary,
    runtime_effect_dictionary: Arc<RuntimeEffectDictionary>,
    uniform_ssbo_index: Option<&'static str>,

    /// The blendInfo represents the actual GPU blend operations, which may or may not completely
    /// implement the paint and coverage blending defined by the root nodes.
    blend_info: BlendInfo,
    dst_read_strategy: DstReadStrategy,

    vertex_sksl: String,
    fragment_sksl: String,
    vs_label: String,
    fs_label: String,
    pipeline_label: String,

    num_fragment_textures_and_samplers: i32,
    has_combined_uniforms: bool,
    storage_buffer_stages: PipelineStageFlags,

    /// Append attributes defined by the render step and any attributes within the paint key via
    /// a mesh shader snippet.
    append_attrs: Vec<ShaderAttribute>,
}

impl ShaderInfo {
    /// Name used in-shader for storage buffer uniform (`kStorageBufferName`).
    // Port of: src/gpu/graphite/ShaderInfo.h#L91 (chrome/m156)
    #[doc(alias = "kStorageBufferName")]
    pub const STORAGE_BUFFER_NAME: &'static str = "fsStorageBuffer";

    /// The `ShaderInfo(dict, rteDict, uniformSsboIndex, dstReadStrategy)` constructor.
    ///
    /// `uniform_ssbo_index` is the `SkSL` expression that indexes the storage buffer of combined
    /// uniforms, or `None` when the uniforms are in a regular uniform block. The result has no
    /// `SkSL` yet; [`ShaderInfo::make`] fills it. (With no dst read.)
    // Port of: src/gpu/graphite/ShaderInfo.cpp#L1156-L1164 (chrome/m156)
    #[must_use]
    pub fn new(
        shader_code_dictionary: &ShaderCodeDictionary,
        runtime_effect_dictionary: Arc<RuntimeEffectDictionary>,
        uniform_ssbo_index: Option<&'static str>,
    ) -> Self {
        Self::with_dst_read_strategy(
            shader_code_dictionary,
            runtime_effect_dictionary,
            uniform_ssbo_index,
            DstReadStrategy::NoneRequired,
        )
    }

    fn with_dst_read_strategy(
        shader_code_dictionary: &ShaderCodeDictionary,
        runtime_effect_dictionary: Arc<RuntimeEffectDictionary>,
        uniform_ssbo_index: Option<&'static str>,
        dst_read_strategy: DstReadStrategy,
    ) -> Self {
        Self {
            shader_code_dictionary: shader_code_dictionary.clone(),
            runtime_effect_dictionary,
            uniform_ssbo_index,
            blend_info: BlendInfo::default(),
            dst_read_strategy,
            vertex_sksl: String::new(),
            fragment_sksl: String::new(),
            vs_label: String::new(),
            fs_label: String::new(),
            pipeline_label: String::new(),
            num_fragment_textures_and_samplers: 0,
            has_combined_uniforms: false,
            storage_buffer_stages: PipelineStageFlags::NONE,
            append_attrs: Vec::new(),
        }
    }

    /// `ShaderInfo::Make(caps, dict, rteDict, rpDesc, step, paintID, outDescs)`.
    ///
    /// Accepts a real or, by default, an invalid/nullptr pointer to a container of
    /// `SamplerDesc`s. Backend implementations which may utilize static / immutable samplers
    /// should pass in a real pointer to indicate that shader node data must be analyzed to
    /// determine whether immutable samplers are used, and if so, ascertain `SamplerDesc`s for
    /// them.
    ///
    /// If provided a valid container, this function will delegate the addition of `SamplerDesc`s
    /// for each sampler the nodes utilize (dynamic and immutable). This way, a `SamplerDesc`'s
    /// index within the container can inform its binding order. Each `SamplerDesc` will be either:
    /// 1) a default-constructed `SamplerDesc`, indicating the use of a "regular" dynamic sampler
    ///    which requires no special handling OR
    /// 2) a real `SamplerDesc` describing an immutable sampler. Backend pipelines can then use the
    ///    desc to obtain a real immutable sampler pointer (which typically must be included in
    ///    pipeline layouts)
    ///
    /// `rte_dict` is `None` where Skia passes `nullptr` (a key without runtime effects).
    ///
    /// # Panics
    /// If the paint key is corrupt (it does not have 2 to 4 root nodes with a source color and a
    /// final blend), as Skia's `SkASSERTF_RELEASE` does.
    // Port of: src/gpu/graphite/ShaderInfo.cpp#L1078-L1154 (chrome/m156)
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        dict: &ShaderCodeDictionary,
        rte_dict: Option<Arc<RuntimeEffectDictionary>>,
        rp_desc: &RenderPassDesc,
        step: &dyn RenderStep,
        paint_id: UniquePaintParamsID,
        out_descs: Option<&mut Vec<SamplerDesc>>,
    ) -> Box<ShaderInfo> {
        let rte_dict = rte_dict.unwrap_or_default();

        // Determine if an SSBO index is needed at all (by either stage)
        let needs_ssbo_index =
            caps.storage_buffer_support() && (step.performs_shading() || step.num_uniforms() > 0);
        let uniform_ssbo_index = needs_ssbo_index.then_some("uniformSsboIndex");

        let has_frag_shader = paint_id.is_valid() && step.performs_shading();

        // Create the final ShaderInfo object.
        let mut result = Box::new(Self::with_dst_read_strategy(
            dict,
            rte_dict.clone(),
            uniform_ssbo_index,
            if has_frag_shader {
                rp_desc.dst_read_strategy
            } else {
                DstReadStrategy::NoneRequired
            },
        ));

        // Decompress Root Nodes
        let roots_info = if paint_id.is_valid() {
            let key_data = dict.lookup(paint_id);
            let key = PaintParamsKey::new(&key_data);
            debug_assert!(key.is_valid());

            let available_varyings = caps.max_varyings()
                - FIXED_VARYINGS
                - i32::try_from(step.varyings().len()).expect("a few varyings");

            let can_lift_coords = step.fragment_color_sksl_local_coords_variable().is_none();
            key.get_root_nodes(caps, dict, &rte_dict, available_varyings, can_lift_coords)
        } else {
            RootNodesInfo::default()
        };

        let shared_data = SharedGeneratorData::new(
            caps,
            step,
            &roots_info,
            paint_id.is_valid(),
            result.uniform_ssbo_index,
        );
        result.has_combined_uniforms =
            shared_data.has_step_uniforms || shared_data.has_paint_uniforms;

        result.append_attrs.reserve(
            step.append_attributes().len()
                + roots_info
                    .mesh_spec
                    .as_ref()
                    .map_or(0, |spec| spec.attributes().len()),
        );
        for render_step_attr in step.append_attributes() {
            result.append_attrs.push(render_step_attr.into());
        }
        if let Some(mesh_spec) = &roots_info.mesh_spec {
            for spec_attr in mesh_spec.attributes() {
                result
                    .append_attrs
                    .push(ShaderAttribute::from_mesh_attribute(spec_attr));
            }
        }

        let paint_label = dict.id_to_string(caps, paint_id);
        if has_frag_shader {
            result.generate_fragment_sksl(
                caps,
                dict,
                &paint_label,
                step,
                rp_desc.color_attachment.format,
                rp_desc.write_swizzle,
                out_descs,
                &shared_data,
            );
        } else {
            result.blend_info.writes_color = false;
        }

        result.storage_buffer_stages |= step.storage_buffer_stages();

        result.generate_vertex_sksl(caps, step, &shared_data);
        step.name().clone_into(&mut result.vs_label);
        if shared_data.needs_local_coords {
            result.vs_label.push_str(" (w/ local coords)");
        }

        step.name().clone_into(&mut result.fs_label);
        result.fs_label.push_str(" + ");
        result.fs_label.push_str(&paint_label);
        if rp_desc.write_swizzle != Swizzle::rgba()
            || result.dst_read_strategy != DstReadStrategy::NoneRequired
        {
            result.fs_label.push('(');
            result.fs_label.push_str(&rp_desc.write_swizzle.as_string());
            if result.dst_read_strategy != DstReadStrategy::NoneRequired {
                result.fs_label.push_str(", ");
                result
                    .fs_label
                    .push_str(dst_read_strategy_to_str(result.dst_read_strategy));
            }
            result.fs_label.push(')');
        }

        // KEEP IN SYNC with ContextUtils::GetPipelineLabel()
        result.pipeline_label = rp_desc.to_pipeline_label();
        result.pipeline_label.push_str(" + ");
        result.pipeline_label.push_str(step.name());
        result.pipeline_label.push_str(" + ");
        result.pipeline_label.push_str(&paint_label);

        result
    }

    /// `shaderCodeDictionary()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L57-L59 (chrome/m156)
    #[doc(alias = "shaderCodeDictionary")]
    #[must_use]
    pub fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_code_dictionary
    }

    /// `runtimeEffectDictionary()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L60-L62 (chrome/m156)
    #[doc(alias = "runtimeEffectDictionary")]
    #[must_use]
    pub fn runtime_effect_dictionary(&self) -> &RuntimeEffectDictionary {
        &self.runtime_effect_dictionary
    }

    /// `uniformSsboIndex()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L64 (chrome/m156)
    #[doc(alias = "uniformSsboIndex")]
    #[must_use]
    pub fn uniform_ssbo_index(&self) -> Option<&str> {
        self.uniform_ssbo_index
    }

    /// `dstReadStrategy()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L66 (chrome/m156)
    #[doc(alias = "dstReadStrategy")]
    #[must_use]
    pub fn dst_read_strategy(&self) -> DstReadStrategy {
        self.dst_read_strategy
    }

    /// `blendInfo()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L67 (chrome/m156)
    #[doc(alias = "blendInfo")]
    #[must_use]
    pub fn blend_info(&self) -> &BlendInfo {
        &self.blend_info
    }

    /// `vertexSkSL()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L69 (chrome/m156)
    #[doc(alias = "vertexSkSL")]
    #[must_use]
    pub fn vertex_sksl(&self) -> &str {
        &self.vertex_sksl
    }

    /// `fragmentSkSL()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L70 (chrome/m156)
    #[doc(alias = "fragmentSkSL")]
    #[must_use]
    pub fn fragment_sksl(&self) -> &str {
        &self.fragment_sksl
    }

    /// `vsLabel()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L72 (chrome/m156)
    #[doc(alias = "vsLabel")]
    #[must_use]
    pub fn vs_label(&self) -> &str {
        &self.vs_label
    }

    /// `fsLabel()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L73 (chrome/m156)
    #[doc(alias = "fsLabel")]
    #[must_use]
    pub fn fs_label(&self) -> &str {
        &self.fs_label
    }

    /// `pipelineLabel()`: matches `ContextUtils::GetPipelineLabel()` for the same args that were
    /// passed to `Make()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L75 (chrome/m156)
    #[doc(alias = "pipelineLabel")]
    #[must_use]
    pub fn pipeline_label(&self) -> &str {
        &self.pipeline_label
    }

    /// `numFragmentTexturesAndSamplers()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L77 (chrome/m156)
    #[doc(alias = "numFragmentTexturesAndSamplers")]
    #[must_use]
    pub fn num_fragment_textures_and_samplers(&self) -> i32 {
        self.num_fragment_textures_and_samplers
    }

    /// `hasCombinedUniforms()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L78 (chrome/m156)
    #[doc(alias = "hasCombinedUniforms")]
    #[must_use]
    pub fn has_combined_uniforms(&self) -> bool {
        self.has_combined_uniforms
    }

    /// `usesStorageBuffer()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L79 (chrome/m156)
    #[doc(alias = "usesStorageBuffer")]
    #[must_use]
    pub fn uses_storage_buffer(&self) -> bool {
        !self.storage_buffer_stages.is_empty()
    }

    /// `storageBufferStages()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L80 (chrome/m156)
    #[doc(alias = "storageBufferStages")]
    #[must_use]
    pub fn storage_buffer_stages(&self) -> PipelineStageFlags {
        self.storage_buffer_stages
    }

    /// `vsUsesStorage()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L81-L83 (chrome/m156)
    #[doc(alias = "vsUsesStorage")]
    #[must_use]
    pub fn vs_uses_storage(&self) -> bool {
        self.storage_buffer_stages
            .contains(PipelineStageFlags::VERTEX_SHADER)
    }

    /// `fsUsesStorage()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L84-L86 (chrome/m156)
    #[doc(alias = "fsUsesStorage")]
    #[must_use]
    pub fn fs_uses_storage(&self) -> bool {
        self.storage_buffer_stages
            .contains(PipelineStageFlags::FRAGMENT_SHADER)
    }

    /// `appendAttributes()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L88 (chrome/m156)
    #[doc(alias = "appendAttributes")]
    #[must_use]
    pub fn append_attributes(&self) -> &[ShaderAttribute] {
        &self.append_attrs
    }

    /// The current, incomplete, model for shader construction is:
    ///   - Static code snippets (which can have an arbitrary signature) live in the Graphite
    ///     pre-compiled module, which is located at `src/sksl/sksl_graphite_frag.sksl`.
    ///   - Glue code is generated in a `main` method which calls these static code snippets.
    ///     The glue code is responsible for:
    ///       1) gathering the correct (mangled) uniforms
    ///       2) passing the uniforms and any other parameters to the helper method
    ///   - The result of the final code snippet is then copied into `sk_FragColor`.
    ///
    ///   Note: each entry's `fStaticFunctionName` field is expected to match the name of a
    ///   function in the Graphite pre-compiled module, or be null if the preamble and expression
    ///   generators are overridden to not use a static function.
    // Port of: src/gpu/graphite/ShaderInfo.cpp#L1177-L1523 (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::if_not_else)] // keeps the order of the C++ branches
    fn generate_fragment_sksl(
        &mut self,
        caps: &dyn Caps,
        dict: &ShaderCodeDictionary,
        label: &str,
        step: &dyn RenderStep,
        target_format: TextureFormat,
        write_swizzle: Swizzle,
        mut out_descs: Option<&mut Vec<SamplerDesc>>,
        shared_data: &SharedGeneratorData<'_>,
    ) {
        let roots_info = shared_data.roots_info;
        let _ = dict; // only the debug-only validation below reads it

        #[cfg(debug_assertions)]
        {
            // Validate the root count of the key.
            debug_assert!(roots_info.roots.len() >= 2 && roots_info.roots.len() <= 4);
            // With source color node all snippets return a half4, so we just require that its
            // signature takes no extra args or just local coords.
            let src_snippet = dict
                .get_entry(
                    roots_info
                        .src_color()
                        .expect("a source color")
                        .code_snippet_id(),
                )
                .expect("a known snippet");
            // TODO(b/349997190): Once SkEmptyShader doesn't use the passthrough snippet, we can
            // assert that srcSnippet->needsPriorStageOutput() is false.
            debug_assert!(!src_snippet.needs_blender_dst_color());
            // Final blender node must take both the src color and dst color, and not any local
            // coordinate.
            let blend_snippet = dict
                .get_entry(
                    roots_info
                        .final_blend()
                        .expect("a final blend")
                        .code_snippet_id(),
                )
                .expect("a known snippet");
            debug_assert!(
                blend_snippet.needs_prior_stage_output() && blend_snippet.needs_blender_dst_color()
            );
            debug_assert!(!blend_snippet.needs_local_coords());
            let clip_snippet = roots_info.clip().map(|clip| {
                dict.get_entry(clip.code_snippet_id())
                    .expect("a known snippet")
            });
            debug_assert!(clip_snippet.is_none_or(|clip| {
                !clip.needs_prior_stage_output() && !clip.needs_blender_dst_color()
            }));
        }

        // Check for unexpected corruption / illegal instructions occurring in the wild.
        assert!(
            (roots_info.roots.len() >= 2 && roots_info.roots.len() <= 4)
                && roots_info.src_color().is_some()
                && roots_info.final_blend().is_some(),
            "root node size = {}, label = {label}",
            roots_info.roots.len()
        );

        // Extract the root nodes for clarity
        let src_color_root = roots_info.src_color().expect("checked above");
        let final_blend_root = roots_info.final_blend().expect("checked above");
        let final_blend_root_snippet_id = final_blend_root.code_snippet_id();
        let clip_root = roots_info.clip();

        // Determine the algorithm for final blending: direct HW blending, coverage-modified HW
        // blending (w/ or w/o dual-source blending) or via dst-read requirement.
        let mut final_coverage = step.coverage();
        if final_coverage == Coverage::None && clip_root.is_some() {
            final_coverage = Coverage::SingleChannel;
        }

        // Initialize the final blend mode to the final snippet's blend mode. It may be changed
        // based upon whether or not we can use hardware blending.
        let mut final_blend_mode: Option<BlendMode> = None;
        if (FIXED_BLEND_ID_OFFSET..BUILT_IN_CODE_SNIPPET_ID_COUNT)
            .contains(&final_blend_root_snippet_id)
        {
            final_blend_mode =
                BlendMode::from_i32(final_blend_root_snippet_id - FIXED_BLEND_ID_OFFSET);
        }
        if final_blend_mode.is_some_and(|mode| {
            can_use_hardware_blending(caps, target_format, mode, final_coverage)
        }) {
            // If we can use hardware blending, update the dstReadStrategy to be kNoneRequired to
            // ensure that ShaderInfo properly informs PipelineInfo of the pipeline's dst read
            // requirement.
            self.dst_read_strategy = DstReadStrategy::NoneRequired;
        } else {
            // If we cannot use hardware blending, then we must perform a dst read within the
            // shader. Therefore we should assert that a valid strategy to do so was passed in.
            // Later operations also expect the blend mode to be kSrc, so update that here.
            debug_assert_ne!(self.dst_read_strategy, DstReadStrategy::NoneRequired);
            final_blend_mode = Some(BlendMode::Src);
        }

        let mut all_req_flags = src_color_root.required_flags() | final_blend_root.required_flags();
        if let Some(clip_root) = clip_root {
            all_req_flags |= clip_root.required_flags();
        }

        let mut fs_preamble = String::new();
        let binding_reqs = caps.resource_binding_requirements();
        fs_preamble += &emit_intrinsic_constants(binding_reqs);
        fs_preamble += &emit_varyings(
            step,
            "in",
            roots_info,
            &shared_data.lifted_expr,
            shared_data.has_ssbo_index_varying,
            shared_data.needs_local_coords,
        );

        if self.dst_read_strategy == DstReadStrategy::ReadFromInput {
            // If this shader reads the dst texture as an input attachment, assert that a valid set
            // index has been assigned within ResourceBindingRequirements.
            debug_assert_ne!(
                binding_reqs.input_attachment_set_idx,
                ResourceBindingRequirements::UNASSIGNED
            );
            // TODO: The following SkSL depends upon the fact that Vulkan is currently the only
            // backend that utilizes DstReadStrategy::kReadFromInput. Update accordingly if other
            // backends add support for this DstReadStrategy.
            let _ = writeln!(
                fs_preamble,
                "layout (vulkan, input_attachment_index={}, set={}, binding={}) \
                 subpassInput DstTextureInput;",
                /* input attachment idx within set= */ 0,
                /* input attachment set idx= */ binding_reqs.input_attachment_set_idx,
                /* binding= */ 0
            );
        }

        let use_storage_buffer = caps.storage_buffer_support()
            && all_req_flags.contains(SnippetRequirementFlags::STORAGE_BUFFER);
        debug_assert!(
            caps.storage_buffer_support()
                || !all_req_flags.contains(SnippetRequirementFlags::STORAGE_BUFFER)
        );
        if use_storage_buffer {
            let _ = write!(
                fs_preamble,
                "layout (set={}, binding={}) readonly buffer FSStorageBuffer {{\n\
                 float {}[];\n\
                 }};\n",
                binding_reqs.uniforms_set_idx,
                binding_reqs.storage_buffer_binding,
                Self::STORAGE_BUFFER_NAME
            );
            self.storage_buffer_stages |= PipelineStageFlags::FRAGMENT_SHADER;
        }

        if step.fs_uses_storage() && step.num_storage_uniforms() > 0 {
            if caps.storage_buffer_support() {
                fs_preamble += &emit_step_storage_buffer(binding_reqs, step);
            } else {
                fs_preamble += &emit_storage_fallback_texture(binding_reqs, step);
            }
            self.storage_buffer_stages |= PipelineStageFlags::FRAGMENT_SHADER;
        }

        let use_dst_sampler = self.dst_read_strategy == DstReadStrategy::TextureCopy
            || self.dst_read_strategy == DstReadStrategy::TextureSample;
        {
            let mut binding = 0;
            fs_preamble += &emit_textures_and_samplers(
                binding_reqs,
                &roots_info.roots,
                &mut binding,
                out_descs.as_deref_mut(),
            );
            let paint_texture_count = binding;
            if step.has_textures() {
                fs_preamble += &step.textures_and_samplers_sksl(binding_reqs, &mut binding);
                if let Some(out_descs) = out_descs.as_mut() {
                    // Determine how many render step samplers were used by comparing the binding
                    // value against paintTextureCount, taking into account the binding
                    // requirements. We assume and do not anticipate the render steps to use
                    // immutable samplers.
                    let render_step_sampler_count =
                        if binding_reqs.separate_texture_and_sampler_binding {
                            (binding - paint_texture_count) / 2
                        } else {
                            binding - paint_texture_count
                        };
                    // Add default SamplerDescs for all the dynamic samplers used by the render
                    // step so the size of outDescs will be equivalent to the total number of
                    // samplers.
                    out_descs.extend(std::iter::repeat_n(
                        SamplerDesc::default(),
                        usize::try_from(render_step_sampler_count).expect("a count"),
                    ));
                }
            }
            if use_dst_sampler {
                fs_preamble += &emit_sampler_layout(binding_reqs, &mut binding);
                fs_preamble += " sampler2D dstSampler;";
                // Add default SamplerDesc for the intrinsic dstSampler to stay consistent with
                // `fNumFragmentTexturesAndSamplers`.
                if let Some(out_descs) = out_descs {
                    out_descs.push(SamplerDesc::default());
                }
            }

            // Record how many textures and samplers are used.
            self.num_fragment_textures_and_samplers = binding;
        }

        // Emit preamble declarations and helper functions required for snippets. In the default
        // case this adds functions that bind a node's specific mangled uniforms to the snippet's
        // implementation in the SkSL modules.
        emit_preambles(self, &roots_info.roots, "", &mut fs_preamble);
        if let Some(mesh_shader) = roots_info.mesh_shader() {
            fs_preamble += &ShaderCodeDictionary::generate_mesh_fs_preamble(self, mesh_shader);
        }

        let mut main_body = String::from("void main() {");

        if shared_data.has_ssbo_index_varying {
            let _ = writeln!(
                main_body,
                "{} = {};",
                self.uniform_ssbo_index.expect("an SSBO index"),
                SSBO_INDEX_VARYING
            );
        }

        if src_color_root
            .required_flags()
            .contains(SnippetRequirementFlags::PRIMITIVE_COLOR)
        {
            debug_assert!(step.emits_primitive_color());
            main_body += "half4 primitiveColor;";
            main_body += &step.fragment_color_sksl(roots_info);
        } else if step.fragment_color_sksl_local_coords_variable().is_some() {
            main_body += &step.fragment_color_sksl(roots_info);
        }
        // else the RenderStep may be producing a primitive color but the paint is not consuming it
        // so just skip injecting that SkSL entirely.

        // Using kDefaultArgs as the initial value means it will refer to undefined variables, but
        // the root nodes should--at most--be depending on the coordinate when "needsLocalCoords"
        // is true. If the PaintParamsKey violates that structure, this will produce SkSL compile
        // errors.
        let mut args = ShaderSnippetArgs::default_args();
        step.fragment_color_sksl_local_coords_variable()
            .unwrap_or("localCoordsVar") // the varying added in emit_varyings()
            .clone_into(&mut args.frag_coord);
        // TODO(b/349997190): The paint root node should not depend on any prior stage's output,
        // but it can happen with how SkEmptyShader is currently mapped to `sk_passthrough`. In
        // this case it requires that prior stage color to be transparent black. When SkEmptyShader
        // can instead cause the draw to be skipped, this can go away.
        args.prior_stage_output = String::from("half4(0)");

        // Calculate the src color and stash its output variable in `args`
        args.prior_stage_output = src_color_root.invoke_and_assign(self, &args, &mut main_body);

        // If not using hardware blending, we perform a dst read in the shader and must add SkSL
        // accordingly.
        if self.dst_read_strategy != DstReadStrategy::NoneRequired {
            // Get the current dst color into a local variable, it may be used later on for
            // coverage blending as well as the final blend.
            main_body += "half4 dstColor;";
            if use_dst_sampler {
                // dstReadBounds is in frag coords and already includes the replay translation. The
                // reciprocol of the dstCopy dimensions are in ZW.
                main_body += "dstColor = sample(dstSampler,\
                              dstReadBounds.zw*(sk_FragCoord.xy - dstReadBounds.xy));";
            } else if self.dst_read_strategy == DstReadStrategy::ReadFromInput {
                // The dst texture should have been written to with the appropriate write swizzle,
                // so we do not need to worry about the read swizzle when accessing that value for
                // blending.
                // (The `SK_DEBUG` build also writes a `// Read color from input attachment` line.)
                main_body += "dstColor = subpassLoad(DstTextureInput);\n";
            } else {
                debug_assert_eq!(self.dst_read_strategy, DstReadStrategy::FramebufferFetch);
                main_body += "dstColor = sk_LastFragColor;";
            }

            args.blender_dst_color = String::from("dstColor");
            args.prior_stage_output =
                final_blend_root.invoke_and_assign(self, &args, &mut main_body);
        }

        if write_swizzle != Swizzle::rgba() {
            let _ = write!(
                main_body,
                "{} = {}.{};",
                args.prior_stage_output,
                args.prior_stage_output,
                write_swizzle.as_string()
            );
        }

        if final_coverage == Coverage::None {
            // Either direct HW blending or a dst-read w/o any extra coverage. In both cases we
            // just need to assign directly to sk_FragCoord and update the HW blend info to
            // finalBlendMode.
            let mode = final_blend_mode.expect("a final blend mode");
            self.blend_info = blend_table(mode);
            let _ = write!(main_body, "sk_FragColor = {};", args.prior_stage_output);
        } else {
            // Accumulate the output coverage. This will either modify the src color and secondary
            // outputs for dual-source blending, or be combined directly with the in-shader
            // blended final color if a dst-readback was required.

            if shared_data.use_uniform_storage_buffer_fs && shared_data.has_step_uniforms {
                main_body += &emit_uniforms_from_storage_buffer(
                    self.uniform_ssbo_index.expect("an SSBO index"),
                    step.uniforms(),
                );
            }

            main_body += "half4 outputCoverage = half4(1);";
            if step.coverage() != Coverage::None {
                main_body += step.fragment_coverage_sksl();
            }

            if let Some(clip_root) = clip_root {
                // The clip block node is invoked with device coords, not local coords like the
                // main shading root node. However sk_FragCoord includes any replay translation and
                // we need to recover the original device coordinate.
                main_body += "float2 devCoord = sk_FragCoord.xy - viewport.xy;";
                args.frag_coord = String::from("devCoord");
                let clip_block_output = clip_root.invoke_and_assign(self, &args, &mut main_body);
                let _ = write!(main_body, "outputCoverage *= {clip_block_output}.a;");
            }

            let out_color = args.prior_stage_output.as_str();
            if self.dst_read_strategy != DstReadStrategy::NoneRequired {
                // If this draw uses a non-coherent dst read, we want to keep the existing dst
                // color (or whatever has been previously drawn) when there's no coverage. This
                // helps for batching text draws that need to read from a dst copy for blends.
                // However, this only helps the case where the outer bounding boxes of each letter
                // overlap and not two actual parts of the text.
                if use_dst_sampler {
                    // We don't think any shaders actually output negative coverage, but just as a
                    // safety check for floating point precision errors, we compare with <= here.
                    // We just check the RGB values of the coverage, since the alpha may not have
                    // been set when using LCD. If we are using single-channel coverage, alpha will
                    // be equal to RGB anyway.
                    main_body += "if (all(lessThanEqual(outputCoverage.rgb, half3(0)))) {\
                                  discard;\
                                  }";
                }

                // Use kSrc HW BlendInfo and do the coverage blend with dst in the shader.
                debug_assert_eq!(final_blend_mode, Some(BlendMode::Src));
                self.blend_info = blend_table(BlendMode::Src);
                let _ = write!(
                    main_body,
                    "sk_FragColor = {out_color} * outputCoverage + dstColor * (1.0 - outputCoverage);"
                );
                if final_coverage == Coverage::Lcd {
                    let _ = write!(
                        main_body,
                        "half3 lerpRGB = mix(dstColor.aaa, {out_color}.aaa, outputCoverage.rgb);\
                         sk_FragColor.a = max(max(lerpRGB.r, lerpRGB.g), lerpRGB.b);"
                    );
                }
            } else {
                // Adjust the shader output(s) to incorporate the coverage so that HW blending
                // produces the correct output.
                let mode = final_blend_mode.expect("a final blend mode");
                if mode > BlendMode::LAST_COEFF_MODE {
                    debug_assert_eq!(final_coverage, Coverage::SingleChannel);
                    self.blend_info = blend_table(mode);
                    main_body += &emit_advanced_blend_color_output("sk_FragColor", out_color);
                } else {
                    // Porter-Duff blend modes can utilize BlendFormula.
                    // TODO: Determine whether draw is opaque and pass that to GetBlendFormula.
                    let coverage_blend_formula: BlendFormula = if final_coverage == Coverage::Lcd {
                        get_lcd_blend_formula(mode)
                    } else {
                        get_blend_formula(
                            /* is_opaque= */ false, /* has_coverage= */ true, mode,
                        )
                    };
                    self.blend_info = BlendInfo {
                        equation: coverage_blend_formula.equation(),
                        src_blend: coverage_blend_formula.src_coeff(),
                        dst_blend: coverage_blend_formula.dst_coeff(),
                        blend_constant: PMColor4f {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }, // SK_PMColor4fTRANSPARENT
                        writes_color: coverage_blend_formula.modifies_dst(),
                    };

                    if final_coverage == Coverage::Lcd {
                        main_body += "outputCoverage.a = max(max(outputCoverage.r, \
                                      outputCoverage.g), \
                                      outputCoverage.b);";
                    }

                    main_body += &emit_color_output(
                        coverage_blend_formula.primary_output(),
                        "sk_FragColor",
                        out_color,
                    );
                    if coverage_blend_formula.has_secondary_output() {
                        debug_assert!(caps.shader_caps().dual_source_blending_support);
                        main_body += &emit_color_output(
                            coverage_blend_formula.secondary_output(),
                            "sk_SecondaryFragColor",
                            out_color,
                        );
                    }
                }
            }
        }
        main_body += "}\n";

        debug_assert_eq!(self.fragment_sksl, "");
        let mut fragment_sksl = String::with_capacity(
            shared_data.shared_preamble.len() + fs_preamble.len() + main_body.len() + 2,
        );
        fragment_sksl += &shared_data.shared_preamble;
        fragment_sksl += "\n";
        fragment_sksl += &fs_preamble;
        fragment_sksl += "\n";
        fragment_sksl += &main_body;
        self.fragment_sksl = fragment_sksl;
    }

    // Port of: src/gpu/graphite/ShaderInfo.cpp#L1525-L1651 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn generate_vertex_sksl(
        &mut self,
        caps: &dyn Caps,
        step: &dyn RenderStep,
        shared_data: &SharedGeneratorData<'_>,
    ) {
        // Fixed program header (intrinsics are always declared as an uniform interface block)
        let binding_reqs = caps.resource_binding_requirements();
        let mut vs_preamble = emit_intrinsic_constants(binding_reqs);
        // Varyings needed by RenderStep and potentially lifted expressions
        vs_preamble += &emit_varyings(
            step,
            "out",
            shared_data.roots_info,
            &shared_data.lifted_expr,
            shared_data.has_ssbo_index_varying,
            shared_data.needs_local_coords,
        );

        // Declare vertex storage buffer (or fallback texture) if the RenderStep has storage
        // uniforms
        if step.vs_uses_storage() && step.num_storage_uniforms() > 0 {
            if caps.storage_buffer_support() {
                vs_preamble += &emit_step_storage_buffer(binding_reqs, step);
            } else {
                vs_preamble += &emit_storage_fallback_texture(binding_reqs, step);
            }
            self.storage_buffer_stages |= PipelineStageFlags::VERTEX_SHADER;
        }

        // Add vertex attributes
        let static_attrs: Vec<ShaderAttribute> =
            step.static_attributes().iter().map(Into::into).collect();
        let append_attrs: Vec<ShaderAttribute> = if self.append_attrs.is_empty() {
            step.append_attributes().iter().map(Into::into).collect()
        } else {
            self.append_attrs.clone()
        };
        if !static_attrs.is_empty() || !append_attrs.is_empty() {
            let mut attr = 0;
            let mut add_attrs = |attrs: &[ShaderAttribute]| {
                for a in attrs {
                    let _ = writeln!(
                        vs_preamble,
                        "layout(location={attr}) in {} {};",
                        a.gpu_type().as_str(),
                        a.name()
                    );
                    attr += 1;
                }
            };
            // (The `SK_DEBUG` build also writes `// static attrs` and `// append attrs` lines.)
            if !static_attrs.is_empty() {
                add_attrs(&static_attrs);
            }
            if !append_attrs.is_empty() {
                add_attrs(&append_attrs);
            }
        }

        if let Some(mesh_shader) = shared_data.roots_info.mesh_shader() {
            vs_preamble += &ShaderCodeDictionary::generate_mesh_vs_preamble(self, mesh_shader);
        }

        // Vertex shader function declaration
        let mut main_body = String::from("void main() {");
        // Create stepLocalCoords which render steps can write to.
        main_body += "float2 stepLocalCoords = float2(0);";

        // We define the SSBO index variable immediately if the VS is using storage buffers. This
        // covers both the "Step Uniforms" case and the "Lifted Uniforms Only" case.
        if shared_data.use_uniform_storage_buffer_vs {
            let _ = writeln!(
                main_body,
                "{} = {};",
                self.uniform_ssbo_index.expect("an SSBO index"),
                SSBO_INDEX_ATTRIBUTE
            );
            if shared_data.has_step_uniforms {
                main_body += &emit_uniforms_from_storage_buffer(
                    self.uniform_ssbo_index.expect("an SSBO index"),
                    step.uniforms(),
                );
            }
        }

        // Inject RenderStep's main vertex logic
        main_body += &step.vertex_sksl(shared_data.roots_info);

        // Calculate sk_Position
        main_body += "sk_Position = float4(viewport.zw*devPosition.xy - sign(viewport.zw)*devPosition.ww,\
                      devPosition.zw);";

        // Assign local coords to varying if needed
        if shared_data.needs_local_coords {
            main_body += "localCoordsVar = stepLocalCoords;";
        }

        // Generate lifted expressions
        for expr in &shared_data.lifted_expr {
            let node = expr.node;
            // Determine the SkSL type string if not emitting directly to a varying
            let type_str = if expr.emit_varying {
                ""
            } else {
                sksl_type_for_lifted_expression(node.entry().liftable_expression_type).as_str()
            };
            let var_name = node.get_expression_varying_name();

            // Generate the expression code, potentially extracting uniforms from SSBO if needed
            let generator = node
                .entry()
                .liftable_expression_generator
                .expect("a lifted expression has a generator");
            let expression = generator(self, node, &expr.args);

            // Assign the expression result to the varying or a temporary variable
            let _ = write!(main_body, "{type_str} {var_name} = {expression};");
        }

        // Assign SSBO index to varying if needed
        if shared_data.has_ssbo_index_varying {
            if shared_data.use_uniform_storage_buffer_vs {
                // Use the local variable we already defined
                let _ = write!(
                    main_body,
                    "{SSBO_INDEX_VARYING} = {};",
                    self.uniform_ssbo_index.expect("an SSBO index")
                );
            } else {
                // No local variable, read directly from attribute
                let _ = write!(main_body, "{SSBO_INDEX_VARYING} = {SSBO_INDEX_ATTRIBUTE};");
            }
        }

        main_body += "}"; // End main()

        debug_assert_eq!(self.vertex_sksl, "");
        let mut vertex_sksl = String::with_capacity(
            shared_data.shared_preamble.len() + vs_preamble.len() + main_body.len() + 2,
        );
        vertex_sksl += &shared_data.shared_preamble;
        vertex_sksl += "\n";
        vertex_sksl += &vs_preamble;
        vertex_sksl += "\n";
        vertex_sksl += &main_body;
        self.vertex_sksl = vertex_sksl;
    }
}
