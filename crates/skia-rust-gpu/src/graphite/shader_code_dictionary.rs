// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ShaderCodeDictionary.{h,cpp}

//! [`ShaderCodeDictionary`]: the thread-safe dictionary of [`ShaderSnippet`]s for use with
//! creating `PaintParamsKey`s, as well as assigning unique ids to each encountered key.
//!
//! It defines a snippet for every `BuiltInCodeSnippetID` (the table is generated from Skia's
//! source by `oracle/graphite-snippets/gen_snippet_table.py`, the `snippet_table` module) and keeps
//! records of the ids per `SkRuntimeEffect`, including de-duplicating equivalent effects.
//!
//! Deviations from the C++:
//!
//! - `ShaderNode`s own their children and data instead of pointing into an arena, and snippets
//!   are shared with `Arc` instead of being pointed at (the dictionary hands out snippets that
//!   outlive its lock).
//! - `ShaderCodeDictionary` is a cheap handle to shared state (`Clone`), so builders and
//!   contexts can hold it without a lifetime.
//! - The `SK_DEBUG`-only `// [%d] %s` comment that `invokeAndAssign` writes is not written: the
//!   goldens come from a non-`SK_DEBUG` build (`docs/design/sksl.md`).
//! - Skia's `SKIA_LOG_W` warning about too many user-defined known runtime effects is not logged.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bitflags::bitflags;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::known_runtime_effects::{
    SKIA_BUILT_IN_RESERVED_CNT, SKIA_KNOWN_RUNTIME_EFFECTS_START, StableKey,
    UNKNOWN_RUNTIME_EFFECT_ID_START, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT,
    USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START, is_skia_known_runtime_effect,
    is_user_defined_runtime_effect, is_viable_user_defined_known_runtime_effect,
    maybe_get_known_runtime_effect, stable_key_from_u32,
};
use skia_rust_core::mesh::{MeshSpecification, mesh_priv};
use skia_rust_core::runtime_effect::{RuntimeEffect, UniformType, uniform};
use skia_rust_core::runtime_effect_priv;
use skia_rust_sksl::codegen::pipeline_stage::{Callbacks, convert_program};
use skia_rust_sksl::ir::{IrPool, VarDeclaration};

use crate::gpu::blend::blend_func_name;
use crate::graphite::attribute::Interpolation;
use crate::graphite::built_in_code_snippet_id::{
    BUILT_IN_CODE_SNIPPET_ID_COUNT, BuiltInCodeSnippetID, FIXED_BLEND_ID_OFFSET,
};
use crate::graphite::caps::Caps;
use crate::graphite::paint_params_key::{PaintParamsKey, PaintParamsKeyBuilder};
use crate::graphite::resource_types::Layout;
use crate::graphite::shader_info::ShaderInfo;
use crate::graphite::uniform::Uniform;
use crate::graphite::uniform_manager::UniformOffsetCalculator;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;
use crate::sksl_type_shared::SkSLType;

mod snippet_table;

// static_assert(static_cast<int>(BuiltInCodeSnippetID::kLast) < kSkiaBuiltInReservedCnt);
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L33 (chrome/m156)
const _: () = assert!((BuiltInCodeSnippetID::LAST as u32) < SKIA_BUILT_IN_RESERVED_CNT);

/// The name of the main function of a mesh's vertex shader (`MeshRenderStep::kMeshVSMainName`).
// Port of: src/gpu/graphite/render/MeshRenderStep.h#L25 (chrome/m156)
pub const MESH_VS_MAIN_NAME: &str = "drawMeshVSMain";
/// The name of the main function of a mesh's fragment shader (`MeshRenderStep::kMeshFSMainName`).
// Port of: src/gpu/graphite/render/MeshRenderStep.h#L26 (chrome/m156)
pub const MESH_FS_MAIN_NAME: &str = "drawMeshFSMain";

// The id ranges of `SkKnownRuntimeEffects` as the `int`s that code snippet ids are.
const SKIA_KNOWN_RUNTIME_EFFECTS_START_ID: i32 = SKIA_KNOWN_RUNTIME_EFFECTS_START.cast_signed();
const USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START_ID: i32 =
    USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START.cast_signed();
const UNKNOWN_RUNTIME_EFFECT_ID_START_ID: i32 = UNKNOWN_RUNTIME_EFFECT_ID_START.cast_signed();

/// The name of a runtime effect snippet whose effect has none.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L994 (chrome/m156)
const DEFAULT_RUNTIME_EFFECT_NAME: &str = "RuntimeEffect";
/// The name of a user-defined known runtime effect snippet whose effect has none.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1080 (chrome/m156)
const DEFAULT_USER_DEFINED_KNOWN_NAME: &str = "UserDefinedKnownRuntimeEffect";

/// The index of `code_snippet_id` in the block of ids that starts at `start`; callers check that
/// the id is in the block.
#[allow(clippy::cast_sign_loss)] // the callers checked that the id is not below the block
fn block_index(code_snippet_id: i32, start: i32) -> usize {
    debug_assert!(code_snippet_id >= start);
    (code_snippet_id - start) as usize
}

/// `kStableKeyCnt`: the number of Skia known runtime effects, with the invalid key.
// Port of: src/core/SkKnownRuntimeEffects.h#L99-L100 (chrome/m156)
const STABLE_KEY_CNT: usize = StableKey::LAST as usize - StableKey::Invalid as usize + 1;

// TODO: How to represent the type (e.g., 2D) of texture being sampled?
/// The name of a texture (and its sampler) a snippet samples (`TextureAndSampler`).
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L39-L47 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextureAndSampler")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureAndSampler {
    name: &'static str,
}

impl TextureAndSampler {
    /// `TextureAndSampler(name)`.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// `name()`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

bitflags! {
    /// What a snippet needs in order to be invoked (`SnippetRequirementFlags`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.h#L49-L69 (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct SnippetRequirementFlags: u32 {
        /// `kNone`.
        const NONE = 0x0;

        // Signature of the ShaderNode
        /// `kLocalCoords`.
        const LOCAL_COORDS = 0x1;
        /// `kPriorStageOutput`: AKA the "input" color, or the "src" argument for a blender.
        const PRIOR_STAGE_OUTPUT = 0x2;
        /// `kBlenderDstColor`: The "dst" argument for a blender.
        const BLENDER_DST_COLOR = 0x4;

        // Special values and/or behaviors required for the snippet
        /// `kPrimitiveColor`.
        const PRIMITIVE_COLOR = 0x8;
        /// `kStorageBuffer`.
        const STORAGE_BUFFER = 0x10;
        /// `kStoresSamplerDescData`: Indicates that the node stores numerical sampler data.
        const STORES_SAMPLER_DESC_DATA = 0x20;
        /// `kPassthroughLocalCoords`: Indicates that the node will pass through local coords
        /// unmodified to its children.
        const PASSTHROUGH_LOCAL_COORDS = 0x40;
        /// `kLiftExpression`: Indicates that the liftable expression generated by this node will
        /// be lifted to the vertex shader and passed to the fragment shader as a varying.
        const LIFT_EXPRESSION = 0x80;
        /// `kOmitExpression`: Indicates that the liftable expression generated by this node will
        /// be used in a compound expression lifted to the vertex shader and omitted from the
        /// fragment shader entirely.
        const OMIT_EXPRESSION = 0x100;
    }
}

/// The names of the dynamic arguments of a snippet's invocation (`ShaderSnippet::Args`).
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L79-L83 (chrome/m156)
#[doc(alias = "ShaderSnippet::Args")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderSnippetArgs {
    /// `fPriorStageOutput`.
    pub prior_stage_output: String,
    /// `fBlenderDstColor`.
    pub blender_dst_color: String,
    /// `fFragCoord`.
    pub frag_coord: String,
}

impl ShaderSnippetArgs {
    /// `ShaderSnippet::kDefaultArgs`.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L208 (chrome/m156)
    #[doc(alias = "kDefaultArgs")]
    #[must_use]
    pub fn default_args() -> Self {
        Self {
            prior_stage_output: String::from("inColor"),
            blender_dst_color: String::from("destColor"),
            frag_coord: String::from("pos"),
        }
    }
}

/// `ShaderSnippet::GeneratePreambleForSnippetFn`.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L85-L86 (chrome/m156)
pub type GeneratePreambleForSnippetFn = fn(&ShaderInfo, &ShaderNode) -> String;
/// `ShaderSnippet::GenerateLiftableExpressionFn`.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L87-L89 (chrome/m156)
pub type GenerateLiftableExpressionFn = fn(&ShaderInfo, &ShaderNode, &ShaderSnippetArgs) -> String;

/// If a snippet has an expression that can be lifted from the fragment shader to the vertex
/// shader, this specifies how the resolved expression is used by subsequent shader nodes
/// (`ShaderSnippet::LiftableExpressionType`).
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L95-L99 (chrome/m156)
#[doc(alias = "ShaderSnippet::LiftableExpressionType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiftableExpressionType {
    /// `kNone`.
    #[default]
    None,
    /// `kLocalCoords`.
    LocalCoords,
    /// `kPriorStageOutput`.
    PriorStageOutput,
}

/// `ShaderSnippet`s define the "ABI" of a `SkSL` module function and its required uniform data,
/// as well as functions for generating the invoking `SkSL`. Snippets are composed into an effect
/// tree using [`ShaderNode`]s.
///
/// Skia's constructor takes the fields as arguments; here they are set in a struct expression
/// (with `..ShaderSnippet::default()`) and [`assert_valid`](Self::assert_valid) runs the
/// constructor's asserts.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L78-L165 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShaderSnippet")]
#[derive(Clone, Debug)]
pub struct ShaderSnippet {
    /// `fName`.
    pub name: String,
    /// `fStaticFunctionName`.
    pub static_function_name: Option<&'static str>,

    /// The features and args that this shader snippet requires in order to be invoked
    /// (`fSnippetRequirementFlags`).
    pub snippet_requirement_flags: SnippetRequirementFlags,

    /// If set, the list of uniforms in `uniforms` describes an existing struct type declared in
    /// the Graphite modules with the given name. Instead of inlining the each uniform in the
    /// top-level interface block or aggregate struct, there will be a single member of this
    /// struct's type (`fUniformStructName`).
    pub uniform_struct_name: Option<&'static str>,
    /// If the uniforms are being embedded as a sub-struct, this is the required starting
    /// alignment (`fRequiredAlignment`).
    pub required_alignment: i32,

    /// `fUniforms`.
    pub uniforms: Vec<Uniform>,
    /// `fTexturesAndSamplers`.
    pub textures_and_samplers: Vec<TextureAndSampler>,

    /// `fNumChildren`.
    pub num_children: i32,
    /// `fPreambleGenerator`.
    pub preamble_generator: Option<GeneratePreambleForSnippetFn>,

    /// `fLiftableExpressionInterpolation`.
    pub liftable_expression_interpolation: Interpolation,
    /// `fLiftableExpressionType`.
    pub liftable_expression_type: LiftableExpressionType,
    /// `fLiftableExpressionGenerator`.
    pub liftable_expression_generator: Option<GenerateLiftableExpressionFn>,
}

impl Default for ShaderSnippet {
    fn default() -> Self {
        Self {
            name: String::new(),
            static_function_name: None,
            snippet_requirement_flags: SnippetRequirementFlags::NONE,
            uniform_struct_name: None,
            required_alignment: -1,
            uniforms: Vec::new(),
            textures_and_samplers: Vec::new(),
            num_children: 0,
            preamble_generator: None,
            liftable_expression_interpolation: Interpolation::Perspective,
            liftable_expression_type: LiftableExpressionType::None,
            liftable_expression_generator: None,
        }
    }
}

impl ShaderSnippet {
    /// The asserts of the `ShaderSnippet` constructor: it must always have a name; the static
    /// function is not optional if using the default (no) preamble generation logic.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.h#L125-L126 (chrome/m156)
    pub fn assert_valid(&self) {
        debug_assert_ne!(self.name, "");
        debug_assert!(self.static_function_name.is_some() || self.preamble_generator.is_some());
    }

    /// `needsLocalCoords()`.
    #[doc(alias = "needsLocalCoords")]
    #[must_use]
    pub fn needs_local_coords(&self) -> bool {
        self.snippet_requirement_flags
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    }

    /// `needsPriorStageOutput()`.
    #[doc(alias = "needsPriorStageOutput")]
    #[must_use]
    pub fn needs_prior_stage_output(&self) -> bool {
        self.snippet_requirement_flags
            .contains(SnippetRequirementFlags::PRIOR_STAGE_OUTPUT)
    }

    /// `needsBlenderDstColor()`.
    #[doc(alias = "needsBlenderDstColor")]
    #[must_use]
    pub fn needs_blender_dst_color(&self) -> bool {
        self.snippet_requirement_flags
            .contains(SnippetRequirementFlags::BLENDER_DST_COLOR)
    }

    /// `storesSamplerDescData()`.
    #[doc(alias = "storesSamplerDescData")]
    #[must_use]
    pub fn stores_sampler_desc_data(&self) -> bool {
        self.snippet_requirement_flags
            .contains(SnippetRequirementFlags::STORES_SAMPLER_DESC_DATA)
    }
}

/// `ShaderNode`s organize snippets into an effect tree, and provide random access to the
/// dynamically bound child snippets. Each node has a fixed number of children defined by its
/// code ID (either a `BuiltInCodeSnippetID` or a runtime effect's assigned ID). All children are
/// non-null. A `ShaderNode` tree represents a decompressed `PaintParamsKey`.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L171-L247 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShaderNode")]
#[derive(Clone, Debug)]
pub struct ShaderNode {
    entry: Arc<ShaderSnippet>, // Owned by the ShaderCodeDictionary
    children: Vec<ShaderNode>,

    code_id: i32,
    key_index: i32, // index back to PaintParamsKey, unique across nodes within a ShaderInfo

    required_flags: SnippetRequirementFlags,
    data: Vec<u32>, // Copy of the subspan of PaintParamsKey's data
}

impl ShaderNode {
    /// `ShaderNode(snippet, children, codeID, keyIndex, data)`.
    ///
    /// # Panics
    /// In debug builds if `children` is not as long as the snippet's number of children, or if
    /// `data` is given for a snippet that does not store sampler data.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.h#L174-L210 (chrome/m156)
    #[must_use]
    pub fn new(
        snippet: Arc<ShaderSnippet>,
        children: Vec<ShaderNode>,
        code_id: i32,
        key_index: i32,
        data: Vec<u32>,
    ) -> Self {
        debug_assert_eq!(
            i32::try_from(children.len()).expect("a few children"),
            snippet.num_children
        );

        let mut required_flags = snippet.snippet_requirement_flags;
        // Propagate requirement flags from children towards root.
        let is_runtime_effect = code_id >= BUILT_IN_CODE_SNIPPET_ID_COUNT;
        let is_compose = code_id == BuiltInCodeSnippetID::Compose as i32
            || code_id == BuiltInCodeSnippetID::BlendCompose as i32;
        for (i, child) in children.iter().enumerate() {
            // Mask off flags to not propagate.
            let mut mask = SnippetRequirementFlags::PASSTHROUGH_LOCAL_COORDS
                | SnippetRequirementFlags::LIFT_EXPRESSION
                | SnippetRequirementFlags::OMIT_EXPRESSION;
            // Runtime effects invoke children with explicit parameters so those requirements
            // never need to propagate to the root. Similarly, compose only needs to propagate
            // the variable parameters for the inner children.
            if is_runtime_effect || (is_compose && i == children.len() - 1) {
                mask |= SnippetRequirementFlags::LOCAL_COORDS
                    | SnippetRequirementFlags::PRIOR_STAGE_OUTPUT
                    | SnippetRequirementFlags::BLENDER_DST_COLOR;
            }
            required_flags |= child.required_flags() & !mask;
        }

        // Data should only be provided if the snippet has the kStoresSamplerDescData flag.
        debug_assert!(data.is_empty() || snippet.stores_sampler_desc_data());

        Self {
            entry: snippet,
            children,
            code_id,
            key_index,
            required_flags,
            data,
        }
    }

    /// `generateDefaultPreamble`.
    ///
    /// If we have no children, we don't need to add anything into the preamble. If we have child
    /// entries, we create a function in the preamble with a signature of:
    ///
    /// ```text
    /// half4 SnippetName_N(/* required variable inputs (e.g. float2 pos) */) { ... }
    /// ```
    ///
    /// This function invokes each child in sequence, and then calls the built-in function,
    /// passing all uniforms and child outputs along:
    ///
    /// ```text
    /// half4 BuiltinFunctionName(/* required variable inputs (e.g. float2 pos) */,
    ///                           /* all uniforms as parameters */,
    ///                           /* all child output variable names as parameters */);
    /// ```
    ///
    /// # Panics
    /// If the node has children and its snippet has no static function (such a snippet has its own
    /// preamble generator).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L221-L249 (chrome/m156)
    #[doc(alias = "generateDefaultPreamble")]
    #[must_use]
    pub fn generate_default_preamble(&self, shader_info: &ShaderInfo) -> String {
        if self.num_children() == 0 {
            // We don't need a helper function to wrap the snippet's static function
            return String::new();
        }

        let mut code = emit_helper_declaration(self) + " {";

        // Invoke each child with unmodified input values and collect in a list of local variables
        let mut child_output_var_names: Vec<String> = Vec::with_capacity(2);
        for child in self.children() {
            // Emit glue code into our helper function body (i.e. lifting the child execution up
            // front so their outputs can be passed to the static module function for the node's
            // snippet).
            child_output_var_names.push(child.invoke_and_assign(
                shader_info,
                &ShaderSnippetArgs::default_args(),
                &mut code,
            ));
        }

        // Finally, invoke the snippet from the helper function, passing uniforms and child
        // outputs.
        let mut params: Vec<String> = Vec::with_capacity(3);
        append_defaults(&mut params, self, Some(&ShaderSnippetArgs::default_args()));
        append_uniforms(&mut params, shader_info, self, &child_output_var_names);

        let _ = write!(
            code,
            "return {}({});}}",
            self.entry()
                .static_function_name
                .expect("a node with children and the default preamble has a static function"),
            stitch_csv(&params)
        );
        code
    }

    /// Emit the glue code needed to invoke a single static helper isolated within its own scope.
    /// Glue code will assign the resulting color into a variable `half4 outColor%d`, where the
    /// `%d` is filled in with `key_index()` (`invokeAndAssign`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L254-L274 (chrome/m156)
    #[doc(alias = "invokeAndAssign")]
    #[must_use]
    pub fn invoke_and_assign(
        &self,
        shader_info: &ShaderInfo,
        args: &ShaderSnippetArgs,
        func_body: &mut String,
    ) -> String {
        let expr = invoke_node(shader_info, self, args);
        let output_var = get_mangled_name("outColor", self.key_index());
        // (The `SK_DEBUG` build also writes a `// [%d] %s` comment line, see the module docs.)
        let _ = write!(func_body, "half4 {output_var} = {expr};");
        output_var
    }

    /// Return a name that should be used as a varying passing the result of an expression
    /// emitted by this node, if the expression is being lifted from the fragment shader to the
    /// vertex shader. The choice of name is arbitrary, but it must be used consistently
    /// (`getExpressionVaryingName`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L279-L281 (chrome/m156)
    #[doc(alias = "getExpressionVaryingName")]
    #[must_use]
    pub fn get_expression_varying_name(&self) -> String {
        get_mangled_name(&self.entry().name, self.key_index()) + "_Var"
    }

    /// `codeSnippetId()`.
    #[doc(alias = "codeSnippetId")]
    #[must_use]
    pub fn code_snippet_id(&self) -> i32 {
        self.code_id
    }

    /// `keyIndex()`.
    #[doc(alias = "keyIndex")]
    #[must_use]
    pub fn key_index(&self) -> i32 {
        self.key_index
    }

    /// `entry()`.
    #[must_use]
    pub fn entry(&self) -> &ShaderSnippet {
        &self.entry
    }

    /// `requiredFlags()`.
    #[doc(alias = "requiredFlags")]
    #[must_use]
    pub fn required_flags(&self) -> SnippetRequirementFlags {
        self.required_flags
    }

    /// `setLiftExpressionFlag()`.
    #[doc(alias = "setLiftExpressionFlag")]
    pub fn set_lift_expression_flag(&mut self) {
        self.required_flags |= SnippetRequirementFlags::LIFT_EXPRESSION;
    }

    /// `setOmitExpressionFlag()`.
    #[doc(alias = "setOmitExpressionFlag")]
    pub fn set_omit_expression_flag(&mut self) {
        self.required_flags |= SnippetRequirementFlags::OMIT_EXPRESSION;
    }

    /// `unsetLocalCoordsFlag()`.
    #[doc(alias = "unsetLocalCoordsFlag")]
    pub fn unset_local_coords_flag(&mut self) {
        self.required_flags &= !SnippetRequirementFlags::LOCAL_COORDS;
    }

    /// `numChildren()`.
    #[doc(alias = "numChildren")]
    #[must_use]
    pub fn num_children(&self) -> i32 {
        self.entry.num_children
    }

    /// `children()`.
    #[must_use]
    pub fn children(&self) -> &[ShaderNode] {
        &self.children
    }

    /// `children()`, mutable.
    #[doc(alias = "children")]
    pub fn children_mut(&mut self) -> &mut [ShaderNode] {
        &mut self.children
    }

    /// `child(childIndex)`.
    ///
    /// # Panics
    /// If `child_index` is out of range.
    #[must_use]
    pub fn child(&self, child_index: i32) -> &ShaderNode {
        &self.children[usize::try_from(child_index).expect("a child index")]
    }

    /// `data()`.
    #[must_use]
    pub fn data(&self) -> &[u32] {
        &self.data
    }
}

//--------------------------------------------------------------------------------------------------
// Helpers of the snippet generators

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L51-L54 (chrome/m156)
fn get_storage_buffer_access(ssbo_index: &str, uniform_name: &str) -> String {
    format!("combinedUniformData[{ssbo_index}].{uniform_name}")
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L56-L58 (chrome/m156)
fn get_mangled_name(base_name: &str, mangling_suffix: i32) -> String {
    format!("{base_name}_{mangling_suffix}")
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L60-L75 (chrome/m156)
fn get_mangled_uniform_name(
    shader_info: &ShaderInfo,
    uniform: &Uniform,
    mangling_suffix: i32,
) -> String {
    let mut result = if uniform.is_paint_color() {
        // Due to deduplication there will only ever be one of these
        uniform.name().to_owned()
    } else {
        format!("{}_{}", uniform.name(), mangling_suffix)
    };
    if let Some(ssbo_index) = shader_info.uniform_ssbo_index() {
        result = get_storage_buffer_access(ssbo_index, &result);
    }
    result
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L77-L79 (chrome/m156)
fn get_mangled_sampler_name(tex: &TextureAndSampler, mangling_suffix: i32) -> String {
    format!("{}_{}", tex.name(), mangling_suffix)
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L81-L89 (chrome/m156)
fn get_mangled_struct_reference(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
    debug_assert!(node.entry().uniform_struct_name.is_some());
    let mut result = format!("node_{}", node.key_index()); // Field holding the struct
    if let Some(ssbo_index) = shader_info.uniform_ssbo_index() {
        result = get_storage_buffer_access(ssbo_index, &result);
    }
    result
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L91-L101 (chrome/m156)
fn stitch_csv(args: &[String]) -> String {
    args.join(", ")
}

// If 'args' is None, the generated list is assumed to be for parameter declarations. If it's
// Some, it is assumed to be the expressions to invoke the default signature.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L105-L125 (chrome/m156)
fn append_defaults(list: &mut Vec<String>, node: &ShaderNode, args: Option<&ShaderSnippetArgs>) {
    // Use the node's aggregate required flags so that the provided dynamic variables propagate
    // to the child nodes that require them.
    if node
        .required_flags()
        .contains(SnippetRequirementFlags::PRIOR_STAGE_OUTPUT)
    {
        list.push(args.map_or_else(
            || String::from("half4 inColor"),
            |a| a.prior_stage_output.clone(),
        ));
    }
    if node
        .required_flags()
        .contains(SnippetRequirementFlags::BLENDER_DST_COLOR)
    {
        list.push(args.map_or_else(
            || String::from("half4 destColor"),
            |a| a.blender_dst_color.clone(),
        ));
    }
    if node
        .required_flags()
        .contains(SnippetRequirementFlags::LOCAL_COORDS)
    {
        list.push(args.map_or_else(|| String::from("float2 pos"), |a| a.frag_coord.clone()));
    }

    // Special variables and/or "global" scope variables that have to propagate
    // through the node tree.
    if node
        .required_flags()
        .contains(SnippetRequirementFlags::PRIMITIVE_COLOR)
    {
        list.push(String::from(if args.is_some() {
            "primitiveColor"
        } else {
            "half4 primitiveColor"
        }));
    }
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L127-L160 (chrome/m156)
fn append_uniforms(
    list: &mut Vec<String>,
    shader_info: &ShaderInfo,
    node: &ShaderNode,
    child_outputs: &[String],
) {
    let entry = node.entry();

    if entry.uniform_struct_name.is_some() {
        // The node's uniforms are aggregated in a sub-struct within the global uniforms so we
        // just need to append a reference to the node's instance
        list.push(get_mangled_struct_reference(shader_info, node));
    } else {
        // The uniforms are in the global scope, so just pass in the ones bound to 'node'
        for uniform in &entry.uniforms {
            list.push(get_mangled_uniform_name(
                shader_info,
                uniform,
                node.key_index(),
            ));
        }
    }

    // Append samplers
    for tex in &entry.textures_and_samplers {
        list.push(get_mangled_sampler_name(tex, node.key_index()));
    }

    // Append storage buffer.
    if node
        .required_flags()
        .contains(SnippetRequirementFlags::STORAGE_BUFFER)
    {
        list.push(String::from(ShaderInfo::STORAGE_BUFFER_NAME));
    }

    // Append child output names.
    list.extend_from_slice(child_outputs);
}

// If we have no children, the default expression just calls a built-in snippet with the
// signature:
//     half4 BuiltinFunctionName(/* required variable inputs (e.g. float2 pos) */,
//                               /* all uniforms as parameters (bound to node's values) */) { ... }
// If we do have children, we will have created a glue function in the preamble and that is called
// instead. Its signature looks like this:
//     half4 SnippetName_N(/* required variable inputs (e.g. float2 pos) */) { ... }
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L168-L188 (chrome/m156)
fn invoke_node(shader_info: &ShaderInfo, node: &ShaderNode, args: &ShaderSnippetArgs) -> String {
    let fn_name: String;
    let mut params: Vec<String> = Vec::with_capacity(3); // 1-2 inputs and a uniform struct or texture

    if let (0, Some(static_fn)) = (node.num_children(), node.entry().static_function_name) {
        // We didn't generate a helper function in the preamble, so add uniforms to the parameter
        // list and call the static function directly.
        fn_name = static_fn.to_owned();
        append_defaults(&mut params, node, Some(args));
        append_uniforms(&mut params, shader_info, node, &[]);
    } else {
        // Invoke the generated helper function added to the preamble, which will handle invoking
        // any children and appending their values to the rest of the static fn's arguments.
        fn_name = get_mangled_name(&node.entry().name, node.key_index());
        append_defaults(&mut params, node, Some(args));
    }

    format!("{}({})", fn_name, stitch_csv(&params))
}

// Emit a declaration for a helper function that represents the ShaderNode (named using the
// node's mangled name). The dynamic parameters are declared to match kDefaultArgs. The returned
// string can either be followed by a "{ body }" to fully define it or a ";" for a forward
// declaration.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L193-L201 (chrome/m156)
fn emit_helper_declaration(node: &ShaderNode) -> String {
    let entry = node.entry();
    let helper_fn_name = get_mangled_name(&entry.name, node.key_index());

    let mut params: Vec<String> = Vec::with_capacity(3);
    append_defaults(&mut params, node, None); // None args emits declarations

    format!("half4 {}({})", helper_fn_name, stitch_csv(&params))
}

//--------------------------------------------------------------------------------------------------
// The snippet generators

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L386-L392 (chrome/m156)
fn generate_solid_color_expression(
    shader_info: &ShaderInfo,
    node: &ShaderNode,
    _args: &ShaderSnippetArgs,
) -> String {
    let uniform =
        get_mangled_uniform_name(shader_info, &node.entry().uniforms[0], node.key_index());
    format!("half4({uniform})")
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L394-L407 (chrome/m156)
fn generate_solid_color_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
    let mut code = emit_helper_declaration(node) + " {return ";

    if node
        .required_flags()
        .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    {
        code += &node.get_expression_varying_name();
    } else if node
        .required_flags()
        .contains(SnippetRequirementFlags::OMIT_EXPRESSION)
    {
        code += "half4(0)";
    } else {
        code +=
            &generate_solid_color_expression(shader_info, node, &ShaderSnippetArgs::default_args());
    }

    code + ";}"
}

// Generate the expression that applies a non-perspective local matrix to coordinates.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L412-L425 (chrome/m156)
fn generate_local_matrix_expression(
    shader_info: &ShaderInfo,
    node: &ShaderNode,
    args: &ShaderSnippetArgs,
) -> String {
    // NOTE: upper2x2 is a float2x2 packed in column major order into a float4
    let upper2x2 =
        get_mangled_uniform_name(shader_info, &node.entry().uniforms[0], node.key_index());
    let translation =
        get_mangled_uniform_name(shader_info, &node.entry().uniforms[1], node.key_index());
    format!(
        "float2x2({upper2x2}.xy, {upper2x2}.zw)*{} + {translation}",
        args.frag_coord
    )
}

/// `kNumCoordinateManipulateChildren`.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L427 (chrome/m156)
const NUM_COORDINATE_MANIPULATE_CHILDREN: i32 = 1;

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L429-L437 (chrome/m156)
fn generate_coord_normalize_expression(
    shader_info: &ShaderInfo,
    node: &ShaderNode,
    args: &ShaderSnippetArgs,
) -> String {
    let uniform =
        get_mangled_uniform_name(shader_info, &node.entry().uniforms[0], node.key_index());
    format!("({uniform} * {})", args.frag_coord)
}

// Create a helper function that manipulates the coordinates passed into a child. The specific
// manipulation is pre-determined by the code id (local matrix or clamp).
// TODO: This is effectively GenerateComposePreamble except that 'node' is counting as the inner.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L442-L486 (chrome/m156)
fn generate_coord_manipulation_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
    debug_assert_eq!(node.num_children(), NUM_COORDINATE_MANIPULATE_CHILDREN);

    let mut perspective_statement = String::new();

    let default_args = ShaderSnippetArgs::default_args();
    let mut local_args = ShaderSnippetArgs::default_args();
    if node
        .child(0)
        .required_flags()
        .contains(SnippetRequirementFlags::LOCAL_COORDS)
    {
        let control_uni =
            get_mangled_uniform_name(shader_info, &node.entry().uniforms[0], node.key_index());

        if node.code_snippet_id() == BuiltInCodeSnippetID::LocalMatrixShader as i32 {
            if node
                .required_flags()
                .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
            {
                local_args.frag_coord = node.get_expression_varying_name();
            } else if !node
                .required_flags()
                .contains(SnippetRequirementFlags::OMIT_EXPRESSION)
            {
                local_args.frag_coord =
                    generate_local_matrix_expression(shader_info, node, &default_args);
            }
        } else if node.code_snippet_id() == BuiltInCodeSnippetID::LocalMatrixShaderPersp as i32 {
            perspective_statement = format!(
                "float3 perspCoord = {control_uni} * {}.xy1;",
                default_args.frag_coord
            );
            local_args.frag_coord = String::from("perspCoord.xy / perspCoord.z");
        } else if node.code_snippet_id() == BuiltInCodeSnippetID::CoordNormalizeShader as i32 {
            if node
                .required_flags()
                .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
            {
                local_args.frag_coord = node.get_expression_varying_name();
            } else if !node
                .required_flags()
                .contains(SnippetRequirementFlags::OMIT_EXPRESSION)
            {
                local_args.frag_coord =
                    generate_coord_normalize_expression(shader_info, node, &default_args);
            }
        } else {
            debug_assert_eq!(
                node.code_snippet_id(),
                BuiltInCodeSnippetID::CoordClampShader as i32
            );
            local_args.frag_coord = format!(
                "clamp({}, {control_uni}.LT, {control_uni}.RB)",
                default_args.frag_coord
            );
        }
    } // else this is a no-op

    let decl = emit_helper_declaration(node);
    let invoke_child = invoke_node(shader_info, node.child(0), &local_args);
    format!("{decl} {{ {perspective_statement} return {invoke_child}; }}")
}

// Compose N-1 children into the Nth child, must have at least two children. The ith child
// provides the value for the ith enabled ShaderSnippet::Arg.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L492-L521 (chrome/m156)
fn generate_compose_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
    debug_assert!(node.num_children() >= 2);

    let outer = node.child(node.num_children() - 1);

    #[cfg(debug_assertions)]
    {
        let num_outer_parameters = i32::from(
            outer
                .required_flags()
                .contains(SnippetRequirementFlags::PRIOR_STAGE_OUTPUT),
        ) + i32::from(
            outer
                .required_flags()
                .contains(SnippetRequirementFlags::BLENDER_DST_COLOR),
        ) + i32::from(
            outer
                .required_flags()
                .contains(SnippetRequirementFlags::LOCAL_COORDS),
        );
        debug_assert_eq!(node.num_children(), num_outer_parameters + 1);
    }

    let default_args = ShaderSnippetArgs::default_args();
    let mut outer_args = ShaderSnippetArgs::default_args();
    let mut child = 0;
    if outer
        .required_flags()
        .contains(SnippetRequirementFlags::LOCAL_COORDS)
    {
        outer_args.frag_coord = invoke_node(shader_info, node.child(child), &default_args);
        child += 1;
    }
    if outer
        .required_flags()
        .contains(SnippetRequirementFlags::PRIOR_STAGE_OUTPUT)
    {
        outer_args.prior_stage_output = invoke_node(shader_info, node.child(child), &default_args);
        child += 1;
    }
    if outer
        .required_flags()
        .contains(SnippetRequirementFlags::BLENDER_DST_COLOR)
    {
        outer_args.blender_dst_color = invoke_node(shader_info, node.child(child), &default_args);
    }

    let decl = emit_helper_declaration(node);
    let invoke_outer = invoke_node(shader_info, outer, &outer_args);
    format!("{decl} {{ return {invoke_outer}; }}")
}

// The toLinearSRGB and fromLinearSRGB RTE built-ins should only be colorspace transform
// functions. This recurses the node to make sure it contains only Compose, Passthrough, or
// CSXform blocks.
// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L526-L566 (chrome/m156)
fn validate_linearsrgb_node(node: &ShaderNode) {
    use BuiltInCodeSnippetID as Id;

    if node.code_snippet_id() == Id::Compose as i32 {
        for child in node.children() {
            validate_linearsrgb_node(child);
        }
    } else {
        debug_assert_eq!(node.num_children(), 0);
        debug_assert!(node.code_snippet_id() < BUILT_IN_CODE_SNIPPET_ID_COUNT);
        let id = u32::try_from(node.code_snippet_id())
            .ok()
            .and_then(Id::from_u32);
        // Valid stages are Passthrough and the transfer function and gamut stages. The generic
        // PreAlpha and PostAlpha stages are not valid: the to/fromLinearSRGB builtins trigger
        // CSXform specialization, so we shouldn't be seeding these generic stage blocks. The
        // builtins have kOpaque alpha type, so since they are specialized there shouldn't be any
        // alpha stage blocks (AlphaOnly, ForceOpaque, Unpremul, Premul) at all. Anything else is
        // invalid too.
        debug_assert!(matches!(
            id,
            Some(
                Id::PriorOutput
                    | Id::CSXformsRGB
                    | Id::CSXformPQ
                    | Id::CSXformHLG
                    | Id::CSXformHLGInv
                    | Id::CSXformGamut
            )
        ));
    }
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L570-L674 (chrome/m156)
struct GraphitePipelineCallbacks<'a> {
    shader_info: &'a ShaderInfo,
    node: &'a ShaderNode,
    preamble: String,
    uses_color_transform: bool,
}

impl<'a> GraphitePipelineCallbacks<'a> {
    fn new(shader_info: &'a ShaderInfo, node: &'a ShaderNode, effect: &RuntimeEffect) -> Self {
        Self {
            shader_info,
            node,
            preamble: String::new(),
            uses_color_transform: runtime_effect_priv::uses_color_transform(effect),
        }
    }
}

impl Callbacks for GraphitePipelineCallbacks<'_> {
    fn declare_uniform(&mut self, pool: &IrPool, decl: &VarDeclaration) -> String {
        let mut result = get_mangled_name(&pool.variable(decl.var).name, self.node.key_index());
        if let Some(ssbo_index) = self.shader_info.uniform_ssbo_index() {
            result = get_storage_buffer_access(ssbo_index, &result);
        }
        result
    }

    fn define_function(&mut self, decl: &str, body: &str, is_main: bool) {
        if is_main {
            let _ = write!(
                self.preamble,
                "{} {{ {} }}",
                emit_helper_declaration(self.node),
                body
            );
        } else {
            let _ = writeln!(self.preamble, "{decl} {{{body}}}");
        }
    }

    fn declare_function(&mut self, decl: &str) {
        self.preamble += decl;
    }

    fn define_struct(&mut self, definition: &str) {
        self.preamble += definition;
    }

    fn declare_global(&mut self, declaration: &str) {
        self.preamble += declaration;
    }

    fn sample_shader(&mut self, index: i32, coords: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        coords.clone_into(&mut args.frag_coord);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn sample_color_filter(&mut self, index: i32, color: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        color.clone_into(&mut args.prior_stage_output);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn sample_blender(&mut self, index: i32, src: &str, dst: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        src.clone_into(&mut args.prior_stage_output);
        dst.clone_into(&mut args.blender_dst_color);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn to_linear_srgb(&mut self, color: &str) -> String {
        debug_assert!(self.uses_color_transform);
        // If we use color transforms (e.g. reference [to|from]LinearSrgb(), we dynamically add
        // two children to the runtime effect's node after all explicitly declared children. The
        // conversion *to* linear srgb is the second-to-last child node, and the conversion
        // *from* linear srgb is the last child node.)
        let to_linear_srgb_node = self.node.child(self.node.num_children() - 2);
        validate_linearsrgb_node(to_linear_srgb_node);

        let mut args = ShaderSnippetArgs::default_args();
        args.prior_stage_output = format!("({color}).rgb1");
        let xformed_color = invoke_node(self.shader_info, to_linear_srgb_node, &args);
        format!("({xformed_color}).rgb")
    }

    fn from_linear_srgb(&mut self, color: &str) -> String {
        debug_assert!(self.uses_color_transform);
        // If we use color transforms (e.g. reference [to|from]LinearSrgb()), we dynamically add
        // two children to the runtime effect's node after all explicitly declared children. The
        // conversion *to* linear srgb is the second-to-last child node, and the conversion
        // *from* linear srgb is the last child node.)
        let from_linear_srgb_node = self.node.child(self.node.num_children() - 1);
        validate_linearsrgb_node(from_linear_srgb_node);

        let mut args = ShaderSnippetArgs::default_args();
        args.prior_stage_output = format!("({color}).rgb1");
        let xformed_color = invoke_node(self.shader_info, from_linear_srgb_node, &args);
        format!("({xformed_color}).rgb")
    }

    fn get_mangled_name(&mut self, name: &str) -> String {
        get_mangled_name(name, self.node.key_index())
    }
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L676-L735 (chrome/m156)
struct MeshProgramCallbacks<'a> {
    preamble: String,
    main_fn: &'static str,
    shader_info: &'a ShaderInfo,
    node: &'a ShaderNode,
}

impl<'a> MeshProgramCallbacks<'a> {
    fn new(main_fn: &'static str, shader_info: &'a ShaderInfo, node: &'a ShaderNode) -> Self {
        Self {
            preamble: String::new(),
            main_fn,
            shader_info,
            node,
        }
    }
}

impl Callbacks for MeshProgramCallbacks<'_> {
    fn get_main_name(&mut self) -> String {
        self.main_fn.to_owned()
    }

    fn define_function(&mut self, declaration: &str, body: &str, _is_main: bool) {
        let _ = writeln!(self.preamble, "{declaration} {{\n {body} }}");
    }

    fn declare_function(&mut self, _declaration: &str) {}

    fn define_struct(&mut self, definition: &str) {
        self.preamble += definition;
        self.preamble += "\n";
    }

    fn declare_global(&mut self, declaration: &str) {
        self.preamble += declaration;
        self.preamble += "\n";
    }

    fn declare_uniform(&mut self, pool: &IrPool, decl: &VarDeclaration) -> String {
        let uniform_name = pool.variable(decl.var).name.to_string();
        if let Some(ssbo_index) = self.shader_info.uniform_ssbo_index() {
            return format!("combinedUniformData[{ssbo_index}].{uniform_name}");
        }
        uniform_name
    }

    fn sample_shader(&mut self, index: i32, coords: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        coords.clone_into(&mut args.frag_coord);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn sample_color_filter(&mut self, index: i32, color: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        color.clone_into(&mut args.prior_stage_output);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn sample_blender(&mut self, index: i32, src: &str, dst: &str) -> String {
        let mut args = ShaderSnippetArgs::default_args();
        src.clone_into(&mut args.prior_stage_output);
        dst.clone_into(&mut args.blender_dst_color);
        invoke_node(self.shader_info, self.node.child(index), &args)
    }

    fn to_linear_srgb(&mut self, _color: &str) -> String {
        panic!("Color transform intrinsics not allowed.");
    }

    fn from_linear_srgb(&mut self, _color: &str) -> String {
        panic!("Color transform intrinsics not allowed.");
    }
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L737-L739 (chrome/m156)
fn noop_preamble(_shader_info: &ShaderInfo, _node: &ShaderNode) -> String {
    String::new()
}

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L741-L770 (chrome/m156)
fn generate_runtime_shader_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
    // Find this runtime effect in the shader-code or runtime-effect dictionary.
    debug_assert!(node.code_snippet_id() >= BUILT_IN_CODE_SNIPPET_ID_COUNT);
    let id = node.code_snippet_id();
    let effect: Option<RuntimeEffect> = if is_skia_known_runtime_effect(id) {
        maybe_get_known_runtime_effect(id.cast_unsigned())
    } else if is_viable_user_defined_known_runtime_effect(id) {
        shader_info
            .shader_code_dictionary()
            .get_user_defined_known_runtime_effect(id)
    } else {
        debug_assert!(is_user_defined_runtime_effect(id));
        shader_info.runtime_effect_dictionary().find(id)
    };
    // This should always be true given the circumstances in which we call convertRuntimeEffect
    let effect = effect.expect("the runtime effect of a runtime effect snippet is registered");

    let args = ShaderSnippetArgs::default_args();
    let mut callbacks = GraphitePipelineCallbacks::new(shader_info, node, &effect);
    runtime_effect_priv::with_program(&effect, |program| {
        convert_program(
            program,
            &args.frag_coord,
            &args.prior_stage_output,
            &args.blender_dst_color,
            &mut callbacks,
        );
    });
    callbacks.preamble
}

//--------------------------------------------------------------------------------------------------

// Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L37-L49 (chrome/m156)
fn get_known_rte_name(key: StableKey) -> &'static str {
    match key {
        StableKey::Invalid => "$Invalid",
        StableKey::OneDBlur4 => "$1DBlur4",
        StableKey::OneDBlur8 => "$1DBlur8",
        StableKey::OneDBlur12 => "$1DBlur12",
        StableKey::OneDBlur16 => "$1DBlur16",
        StableKey::OneDBlur20 => "$1DBlur20",
        StableKey::OneDBlur28 => "$1DBlur28",
        StableKey::TwoDBlur4 => "$2DBlur4",
        StableKey::TwoDBlur8 => "$2DBlur8",
        StableKey::TwoDBlur12 => "$2DBlur12",
        StableKey::TwoDBlur16 => "$2DBlur16",
        StableKey::TwoDBlur20 => "$2DBlur20",
        StableKey::TwoDBlur28 => "$2DBlur28",
        StableKey::Blend => "$Blend",
        StableKey::Decal => "$Decal",
        StableKey::Displacement => "$Displacement",
        StableKey::Lighting => "$Lighting",
        StableKey::LinearMorphology => "$LinearMorphology",
        StableKey::Magnifier => "$Magnifier",
        StableKey::MatrixConvUniforms => "$MatrixConvUniforms",
        StableKey::MatrixConvTexSm => "$MatrixConvTexSm",
        StableKey::MatrixConvTexLg => "$MatrixConvTexLg",
        StableKey::Normal => "$Normal",
        StableKey::SparseMorphology => "$SparseMorphology",
        StableKey::Arithmetic => "$Arithmetic",
        StableKey::HighContrast => "$HighContrast",
        StableKey::Lerp => "$Lerp",
        StableKey::Luma => "$Luma",
        StableKey::Overdraw => "$Overdraw",
    }
}

/// The names of the known runtime effects' snippets, in the order of their stable keys. Exposed
/// for the test that compares them with `SK_ALL_STABLEKEYS`.
#[doc(hidden)]
#[must_use]
pub fn known_runtime_effect_snippet_name(key: StableKey) -> &'static str {
    get_known_rte_name(key)
}

fn all_sample_usages_are_passthrough(effect: &RuntimeEffect) -> bool {
    for i in 0..effect.children().len() {
        if !runtime_effect_priv::child_sample_usage(effect, i).is_pass_through() {
            return false;
        }
    }
    true
}

/// The identity of a runtime effect in the dictionary (`RuntimeEffectKey`): the hash of its
/// program and the size of its uniforms.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L343-L350 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RuntimeEffectKey {
    hash: u32,
    uniform_size: u32,
}

/// The identity of a mesh specification in the dictionary (`MeshSpecKey`).
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L351-L360 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MeshSpecKey {
    hash: u32,
    attribute_stride: u32,
    uniform_size: u32,
}

// Everything the spin lock guards.
#[derive(Debug)]
struct Guarded {
    known_runtime_effect_code_snippets: Vec<Option<Arc<ShaderSnippet>>>,

    // The value returned from 'getEntry' must be stable so, hold the user-defined code snippet
    // entries as shared pointers.
    user_defined_code_snippets: Vec<Arc<ShaderSnippet>>,

    paint_key_to_id: HashMap<Arc<[i32]>, UniquePaintParamsID>,
    id_to_paint_key: Vec<Arc<[i32]>>,

    // A map from RuntimeEffectKeys (hash plus uniforms) to code-snippet IDs. RuntimeEffectKeys
    // don't track the lifetime of a runtime effect at all; they live forever, and a newly-
    // instantiated runtime effect with the same program as a previously-discarded effect will
    // reuse an existing ID. Entries in the runtime-effect map are never removed; they only
    // disappear when the context is discarded, which takes the ShaderCodeDictionary along with
    // it. However, they are extremely small (< 20 bytes) so the memory footprint should be
    // unnoticeable.
    runtime_effect_map: HashMap<RuntimeEffectKey, i32>,
    mesh_map: HashMap<MeshSpecKey, i32>,
}

#[derive(Debug)]
struct DictionaryInner {
    built_in_code_snippets: Vec<Arc<ShaderSnippet>>,

    // These two arrays are not guarded by a lock since they are only initialized in the ctor
    user_defined_known_code_snippets: Vec<Arc<ShaderSnippet>>,
    user_defined_known_runtime_effects: Vec<RuntimeEffect>,

    guarded: Mutex<Guarded>,
}

/// `ShaderCodeDictionary` is a thread-safe dictionary of `ShaderSnippet`s to code IDs for use
/// with creating `PaintParamKey`s, as well as assigning unique IDs to each encountered
/// `PaintParamKey`. It defines `ShaderSnippet`s for every `BuiltInCodeSnippetID` and maintains
/// records for IDs per `SkRuntimeEffect`, including de-duplicating equivalent `SkRuntimeEffect`
/// objects.
///
/// Cloning gives another handle to the same dictionary.
// Port of: src/gpu/graphite/ShaderCodeDictionary.h#L253-L380 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShaderCodeDictionary")]
#[derive(Clone, Debug)]
pub struct ShaderCodeDictionary {
    inner: Arc<DictionaryInner>,
}

impl ShaderCodeDictionary {
    /// `ShaderCodeDictionary(layout, userDefinedKnownRuntimeEffects)`.
    ///
    /// # Panics
    /// If the snippet table is incomplete, or in debug builds if two snippets have the same name.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1127-L1776 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout, user_defined_known_runtime_effects: &[RuntimeEffect]) -> Self {
        // The 0th index is reserved as invalid
        let mut guarded = Guarded {
            known_runtime_effect_code_snippets: vec![None; STABLE_KEY_CNT],
            user_defined_code_snippets: Vec::new(),
            paint_key_to_id: HashMap::new(),
            id_to_paint_key: vec![Arc::from([])],
            runtime_effect_map: HashMap::new(),
            mesh_map: HashMap::new(),
        };

        let mut table: Vec<Option<ShaderSnippet>> =
            vec![None; usize::try_from(BUILT_IN_CODE_SNIPPET_ID_COUNT).expect("a count")];
        snippet_table::fill_built_in_snippets(&mut table);

        // Fixed-function blend mode snippets are all the same, their functionality is entirely
        // defined by their unique code snippet IDs.
        for i in 0..=(BlendMode::LAST_MODE as i32) {
            let ff_blend_mode_id = FIXED_BLEND_ID_OFFSET + i;
            let mode = BlendMode::from_i32(i).expect("a blend mode");
            table[usize::try_from(ff_blend_mode_id).expect("a built-in id")] =
                Some(ShaderSnippet {
                    name: mode.name().to_owned(),
                    static_function_name: Some(blend_func_name(mode)),
                    snippet_requirement_flags: SnippetRequirementFlags::PRIOR_STAGE_OUTPUT
                        | SnippetRequirementFlags::BLENDER_DST_COLOR,
                    ..ShaderSnippet::default()
                });
        }

        // Complete layout calculations for builtin snippets
        let mut built_in_code_snippets = Vec::with_capacity(table.len());
        for slot in table {
            let mut snippet = slot.expect("Should not have missed a built-in");
            snippet.assert_valid();

            if snippet.uniform_struct_name.is_some() {
                let mut offset_calculator = UniformOffsetCalculator::for_struct(layout);
                for uniform in &snippet.uniforms {
                    debug_assert!(!uniform.is_paint_color()); // paint color shouldn't be embedded
                    offset_calculator.advance_offset(uniform.ty(), uniform.count());
                }
                snippet.required_alignment = offset_calculator.required_alignment();
            }
            built_in_code_snippets.push(Arc::new(snippet));
        }

        // Check for duplicate snippet names.
        if cfg!(debug_assertions) {
            let mut snippet_names: HashSet<&str> = HashSet::new();
            for snippet in &built_in_code_snippets {
                assert!(
                    snippet_names.insert(&snippet.name),
                    "duplicate snippet name {}",
                    snippet.name
                );
            }
        }

        let (user_defined_known_code_snippets, user_defined_known_runtime_effects) =
            Self::register_user_defined_known_runtime_effects(
                &mut guarded,
                user_defined_known_runtime_effects,
            );

        Self {
            inner: Arc::new(DictionaryInner {
                built_in_code_snippets,
                user_defined_known_code_snippets,
                user_defined_known_runtime_effects,
                guarded: Mutex::new(guarded),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Guarded> {
        self.inner
            .guarded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Finds the id of `ppk`, or assigns it the next id (`findOrCreate(const PaintParamsKey&)`).
    /// An invalid key has the invalid id.
    ///
    /// # Panics
    /// If there are more than `u32::MAX` keys.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L292-L312 (chrome/m156)
    #[doc(alias = "findOrCreate")]
    #[must_use]
    pub fn find_or_create(&self, ppk: &PaintParamsKey<'_>) -> UniquePaintParamsID {
        if !ppk.is_valid() {
            return UniquePaintParamsID::invalid();
        }

        let mut guarded = self.lock();

        if let Some(existing_entry) = guarded.paint_key_to_id.get(ppk.data()) {
            let existing_entry = *existing_entry;
            debug_assert_eq!(
                &*guarded.id_to_paint_key[existing_entry.as_uint() as usize],
                ppk.data()
            );
            return existing_entry;
        }

        // Detach from the builder and copy into the arena
        let key = ppk.clone_data();
        let new_id = UniquePaintParamsID::new(
            u32::try_from(guarded.id_to_paint_key.len()).expect("fewer than 2^32 keys"),
        );

        guarded.paint_key_to_id.insert(key.clone(), new_id);
        guarded.id_to_paint_key.push(key);
        new_id
    }

    /// `findOrCreate(PaintParamsKeyBuilder*)`: locks the builder as a key, to find or create its
    /// id.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L286-L290 (chrome/m156)
    #[doc(alias = "findOrCreate")]
    #[must_use]
    pub fn find_or_create_for_builder(
        &self,
        builder: &mut PaintParamsKeyBuilder,
    ) -> UniquePaintParamsID {
        let key_view = builder.lock_as_key();

        self.find_or_create(&key_view.key())
    }

    /// The key with the id `code_id`; the data of the invalid key if the id is invalid. Use
    /// [`PaintParamsKey::new`] to view the data as a key (`lookup`).
    ///
    /// # Panics
    /// In debug builds if `code_id` was not made by this dictionary.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L314-L322 (chrome/m156)
    #[must_use]
    pub fn lookup(&self, code_id: UniquePaintParamsID) -> Arc<[i32]> {
        if !code_id.is_valid() {
            return Arc::from([]);
        }

        let guarded = self.lock();
        debug_assert!((code_id.as_uint() as usize) < guarded.id_to_paint_key.len());
        guarded.id_to_paint_key[code_id.as_uint() as usize].clone()
    }

    /// The description of the key with the id `id` (`idToString`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.h#L264-L266 (chrome/m156)
    #[doc(alias = "idToString")]
    #[must_use]
    pub fn id_to_string(&self, caps: &dyn Caps, id: UniquePaintParamsID) -> String {
        PaintParamsKey::new(&self.lookup(id)).to_string(caps, self)
    }

    /// Whether `snippet_id` is a code snippet id this dictionary knows (`isValidID`, `SK_DEBUG`
    /// only in Skia).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L807-L831 (chrome/m156)
    #[doc(alias = "isValidID")]
    #[must_use]
    pub fn is_valid_id(&self, snippet_id: i32) -> bool {
        if snippet_id < 0 {
            return false;
        }

        if snippet_id < BUILT_IN_CODE_SNIPPET_ID_COUNT {
            return true;
        }
        if is_skia_known_runtime_effect(snippet_id) {
            return true;
        }

        if self.is_user_defined_known_runtime_effect(snippet_id) {
            return true;
        }

        let guarded = self.lock();

        if is_user_defined_runtime_effect(snippet_id) {
            let user_defined_code_snippet_id =
                block_index(snippet_id, UNKNOWN_RUNTIME_EFFECT_ID_START_ID);
            return user_defined_code_snippet_id < guarded.user_defined_code_snippets.len();
        }

        false
    }

    /// Prints the key with the id `id` to stderr (`dump`, `SK_DEBUG` only in Skia).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L833-L835 (chrome/m156)
    pub fn dump(&self, caps: &dyn Caps, id: UniquePaintParamsID) {
        PaintParamsKey::new(&self.lookup(id)).dump(caps, self, id);
    }

    /// The snippet with the code snippet id `code_snippet_id`, if there is one (`getEntry(int)`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L324-L362 (chrome/m156)
    #[doc(alias = "getEntry")]
    #[must_use]
    #[allow(clippy::cast_sign_loss)] // the ids are checked to be in range first
    pub fn get_entry(&self, code_snippet_id: i32) -> Option<Arc<ShaderSnippet>> {
        if code_snippet_id < 0 {
            return None;
        }

        if code_snippet_id < BUILT_IN_CODE_SNIPPET_ID_COUNT {
            return Some(self.inner.built_in_code_snippets[code_snippet_id as usize].clone());
        }

        let guarded = self.lock();

        if is_skia_known_runtime_effect(code_snippet_id) {
            let known_rte_code_snippet_id =
                block_index(code_snippet_id, SKIA_KNOWN_RUNTIME_EFFECTS_START_ID);

            // TODO(b/238759147): if the snippet hasn't been initialized, get the SkRuntimeEffect
            // and initialize it here
            let snippet =
                guarded.known_runtime_effect_code_snippets[known_rte_code_snippet_id].clone();
            debug_assert!(snippet.is_some());
            return snippet;
        }

        if is_viable_user_defined_known_runtime_effect(code_snippet_id) {
            let index = block_index(code_snippet_id, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START_ID);
            if index >= self.inner.user_defined_known_code_snippets.len() {
                return None;
            }

            return Some(self.inner.user_defined_known_code_snippets[index].clone());
        }

        if is_user_defined_runtime_effect(code_snippet_id) {
            let user_defined_code_snippet_id =
                (code_snippet_id - UNKNOWN_RUNTIME_EFFECT_ID_START_ID) as usize;
            if user_defined_code_snippet_id < guarded.user_defined_code_snippets.len() {
                return Some(
                    guarded.user_defined_code_snippets[user_defined_code_snippet_id].clone(),
                );
            }
        }

        None
    }

    /// The snippet of a built-in code snippet (`getEntry(BuiltInCodeSnippetID)`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.h#L276-L279 (chrome/m156)
    #[doc(alias = "getEntry")]
    #[must_use]
    pub fn get_entry_built_in(&self, code_snippet_id: BuiltInCodeSnippetID) -> &ShaderSnippet {
        // Built-in code snippets are initialized once so there is no need to take a lock
        &self.inner.built_in_code_snippets[code_snippet_id as usize]
    }

    /// `getEntry` can be used to retrieve the snippet for a user-defined known runtime effect
    /// but, since the dictionary owns those runtime effects, we need another entry point to
    /// retrieve the actual effect. For unknown runtime effects this is handled by the
    /// `RuntimeEffectDictionary` which, transiently, holds a ref on the encountered runtime
    /// effects (`getUserDefinedKnownRuntimeEffect`).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L364-L381 (chrome/m156)
    #[doc(alias = "getUserDefinedKnownRuntimeEffect")]
    #[must_use]
    #[allow(clippy::cast_sign_loss)] // the id is checked to be in range first
    pub fn get_user_defined_known_runtime_effect(
        &self,
        code_snippet_id: i32,
    ) -> Option<RuntimeEffect> {
        if code_snippet_id < 0 {
            return None;
        }

        if is_viable_user_defined_known_runtime_effect(code_snippet_id) {
            let index = block_index(code_snippet_id, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START_ID);
            return self
                .inner
                .user_defined_known_runtime_effects
                .get(index)
                .cloned();
        }

        None
    }

    /// `isUserDefinedKnownRuntimeEffect`.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1102-L1113 (chrome/m156)
    #[doc(alias = "isUserDefinedKnownRuntimeEffect")]
    #[must_use]
    #[allow(clippy::cast_sign_loss)] // the id is checked to be in range first
    pub fn is_user_defined_known_runtime_effect(&self, candidate: i32) -> bool {
        if !is_viable_user_defined_known_runtime_effect(candidate) {
            return false;
        }

        let index = block_index(candidate, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START_ID);
        if index >= self.inner.user_defined_known_code_snippets.len() {
            return false;
        }

        true
    }

    /// `numUserDefinedRuntimeEffects` (`GPU_TEST_UTILS` in Skia).
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1116-L1120 (chrome/m156)
    #[doc(alias = "numUserDefinedRuntimeEffects")]
    #[must_use]
    pub fn num_user_defined_runtime_effects(&self) -> usize {
        self.lock().user_defined_code_snippets.len()
    }

    /// `numUserDefinedKnownRuntimeEffects` (`GPU_TEST_UTILS` in Skia).
    #[doc(alias = "numUserDefinedKnownRuntimeEffects")]
    #[must_use]
    pub fn num_user_defined_known_runtime_effects(&self) -> usize {
        self.inner.user_defined_known_code_snippets.len()
    }

    /// `UniformTypeToSkSLType`.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L838-L872 (chrome/m156)
    #[doc(alias = "UniformTypeToSkSLType")]
    #[must_use]
    pub fn uniform_type_to_sksl_type(u: &skia_rust_core::runtime_effect::Uniform) -> SkSLType {
        if u.flags().contains(uniform::Flags::HALF_PRECISION) {
            match u.ty() {
                UniformType::Float => SkSLType::Half,
                UniformType::Float2 => SkSLType::Half2,
                UniformType::Float3 => SkSLType::Half3,
                UniformType::Float4 => SkSLType::Half4,
                UniformType::Float2x2 => SkSLType::Half2x2,
                UniformType::Float3x3 => SkSLType::Half3x3,
                UniformType::Float4x4 => SkSLType::Half4x4,
                // NOTE: shorts cannot be uniforms, so we shouldn't ever get here.
                // Defensively return the full precision integer type.
                UniformType::Int => {
                    debug_assert!(false, "unsupported uniform type");
                    SkSLType::Int
                }
                UniformType::Int2 => {
                    debug_assert!(false, "unsupported uniform type");
                    SkSLType::Int2
                }
                UniformType::Int3 => {
                    debug_assert!(false, "unsupported uniform type");
                    SkSLType::Int3
                }
                UniformType::Int4 => {
                    debug_assert!(false, "unsupported uniform type");
                    SkSLType::Int4
                }
            }
        } else {
            match u.ty() {
                UniformType::Float => SkSLType::Float,
                UniformType::Float2 => SkSLType::Float2,
                UniformType::Float3 => SkSLType::Float3,
                UniformType::Float4 => SkSLType::Float4,
                UniformType::Float2x2 => SkSLType::Float2x2,
                UniformType::Float3x3 => SkSLType::Float3x3,
                UniformType::Float4x4 => SkSLType::Float4x4,
                UniformType::Int => SkSLType::Int,
                UniformType::Int2 => SkSLType::Int2,
                UniformType::Int3 => SkSLType::Int3,
                UniformType::Int4 => SkSLType::Int4,
            }
        }
    }

    /// `ConvertRuntimeEffectUniforms`: the Graphite uniforms of a runtime effect's uniforms.
    ///
    /// Skia copies the names into an arena because the effect may eventually disappear; the
    /// converted uniforms own theirs.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L885-L908 (chrome/m156)
    #[doc(alias = "ConvertRuntimeEffectUniforms")]
    #[must_use]
    pub fn convert_runtime_effect_uniforms(
        uniforms: &[skia_rust_core::runtime_effect::Uniform],
    ) -> Vec<Uniform> {
        // Convert the SkRuntimeEffect::Uniform array into its Uniform equivalent.
        uniforms
            .iter()
            .map(|u| {
                // Add one Uniform to our array.
                let ty = Self::uniform_type_to_sksl_type(u);
                if u.flags().contains(uniform::Flags::ARRAY) {
                    Uniform::new_owned(u.name().to_owned(), ty, u.count())
                } else {
                    Uniform::new_owned(u.name().to_owned(), ty, 0)
                }
            })
            .collect()
    }

    /// `GenerateMeshVSPreamble`.
    ///
    /// # Panics
    /// If the runtime effect dictionary has no mesh specification for the node.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L774-L788 (chrome/m156)
    #[doc(alias = "GenerateMeshVSPreamble")]
    #[must_use]
    pub fn generate_mesh_vs_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
        let spec = shader_info
            .runtime_effect_dictionary()
            .find_mesh_spec(node.code_snippet_id())
            .expect("the mesh specification is registered");

        let mut vs_callbacks = MeshProgramCallbacks::new(MESH_VS_MAIN_NAME, shader_info, node);
        // (`sampleCoords`, `inputColor` and `destColor` are null for mesh programs, which do not
        // refer to them.)
        convert_program(mesh_priv::vs(&spec), "", "", "", &mut vs_callbacks);
        vs_callbacks.preamble
    }

    /// `GenerateMeshFSPreamble`.
    ///
    /// # Panics
    /// If the runtime effect dictionary has no mesh specification for the node.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L790-L804 (chrome/m156)
    #[doc(alias = "GenerateMeshFSPreamble")]
    #[must_use]
    pub fn generate_mesh_fs_preamble(shader_info: &ShaderInfo, node: &ShaderNode) -> String {
        let spec = shader_info
            .runtime_effect_dictionary()
            .find_mesh_spec(node.code_snippet_id())
            .expect("the mesh specification is registered");

        let mut fs_callbacks = MeshProgramCallbacks::new(MESH_FS_MAIN_NAME, shader_info, node);
        convert_program(mesh_priv::fs(&spec), "", "", "", &mut fs_callbacks);
        fs_callbacks.preamble
    }

    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L919-L952 (chrome/m156)
    fn convert_runtime_effect(effect: &RuntimeEffect, name: &str) -> ShaderSnippet {
        let mut snippet_flags = SnippetRequirementFlags::NONE;
        if effect.allow_shader() {
            // TODO(b/412621191) Ideally we would have a way to tell exactly which children of a
            // runtime shader are sampled with modified coords, or whether coordinates are
            // required at all. For now we assume all runtime shaders need coordinates, and if
            // any children are sampled with modified coords, we assume they all are.
            snippet_flags |= SnippetRequirementFlags::LOCAL_COORDS;
            if all_sample_usages_are_passthrough(effect) {
                snippet_flags |= SnippetRequirementFlags::PASSTHROUGH_LOCAL_COORDS;
            }
        } else if effect.allow_color_filter() {
            snippet_flags |= SnippetRequirementFlags::PRIOR_STAGE_OUTPUT;
        } else if effect.allow_blender() {
            snippet_flags |= SnippetRequirementFlags::PRIOR_STAGE_OUTPUT; // src
            snippet_flags |= SnippetRequirementFlags::BLENDER_DST_COLOR; // dst
        }

        // If the runtime effect references toLinearSrgb() or fromLinearSrgb(), we append two
        // color space transform children that are invoked when converting those "built-in"
        // expressions.
        let num_children_inc_color_transforms = i32::try_from(effect.children().len())
            .expect("a few children")
            + if runtime_effect_priv::uses_color_transform(effect) {
                2
            } else {
                0
            };

        // TODO: We can have the custom runtime effect preamble generator define structs for its
        // uniforms if it has a lot of uniforms, and then calculate the required alignment here.
        let snippet = ShaderSnippet {
            name: name.to_owned(),
            static_function_name: None,
            snippet_requirement_flags: snippet_flags,
            uniforms: Self::convert_runtime_effect_uniforms(effect.uniforms()),
            preamble_generator: Some(generate_runtime_shader_preamble),
            num_children: num_children_inc_color_transforms,
            ..ShaderSnippet::default()
        };
        snippet.assert_valid();
        snippet
    }

    /// The code snippet id of `effect`, creating the snippet if it is the first time the
    /// dictionary sees the effect; `None` on failure (`findOrCreateRuntimeEffectSnippet`, which
    /// returns -1).
    ///
    /// # Panics
    /// If the effect's uniform size is more than `u32::MAX`.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L954-L1003 (chrome/m156)
    #[doc(alias = "findOrCreateRuntimeEffectSnippet")]
    #[must_use]
    #[allow(clippy::cast_possible_wrap)] // the ids are in the small ranges of SkKnownRuntimeEffects
    #[allow(clippy::cast_sign_loss)] // the ids are checked to be in range first
    pub fn find_or_create_runtime_effect_snippet(&self, effect: &RuntimeEffect) -> Option<i32> {
        let mut guarded = self.lock();

        let stable_key = runtime_effect_priv::stable_key(effect) as i32;
        if stable_key != 0 {
            if is_skia_known_runtime_effect(stable_key) {
                let index = block_index(stable_key, SKIA_KNOWN_RUNTIME_EFFECTS_START_ID);

                if guarded.known_runtime_effect_code_snippets[index].is_none() {
                    let name = get_known_rte_name(
                        stable_key_from_u32(stable_key as u32).expect("a viable stable key"),
                    );
                    guarded.known_runtime_effect_code_snippets[index] =
                        Some(Arc::new(Self::convert_runtime_effect(effect, name)));
                }

                return Some(stable_key);
            } else if is_viable_user_defined_known_runtime_effect(stable_key) {
                let index = block_index(stable_key, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START_ID);
                if index >= self.inner.user_defined_known_code_snippets.len() {
                    return None;
                }

                return Some(stable_key);
            }

            return None;
        }

        // Use the combination of {SkSL program hash, uniform size} as our key.
        // In the unfortunate event of a hash collision, at least we'll have the right amount of
        // uniform data available.
        let key = RuntimeEffectKey {
            hash: runtime_effect_priv::hash(effect),
            uniform_size: u32::try_from(effect.uniform_size()).expect("a small uniform size"),
        };

        if let Some(existing_code_snippet_id) = guarded.runtime_effect_map.get(&key) {
            return Some(*existing_code_snippet_id);
        }

        // TODO: the memory for user-defined entries could go in the dictionary's arena but that
        // would have to be a thread safe allocation since the arena also stores entries for
        // 'fHash' and 'fEntryVector'
        guarded
            .user_defined_code_snippets
            .push(Arc::new(Self::convert_runtime_effect(
                effect,
                if runtime_effect_priv::has_name(effect) {
                    runtime_effect_priv::get_name(effect)
                } else {
                    DEFAULT_RUNTIME_EFFECT_NAME
                },
            )));
        let new_code_snippet_id = UNKNOWN_RUNTIME_EFFECT_ID_START_ID
            + i32::try_from(guarded.user_defined_code_snippets.len()).expect("a few snippets")
            - 1;

        guarded.runtime_effect_map.insert(key, new_code_snippet_id);
        Some(new_code_snippet_id)
    }

    /// The code snippet id of `spec`, creating the snippet if it is the first time the
    /// dictionary sees the specification (`findOrCreateMeshSnippet`).
    ///
    /// # Panics
    /// If the specification's stride or uniform size is more than `u32::MAX`.
    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1005-L1028 (chrome/m156)
    #[doc(alias = "findOrCreateMeshSnippet")]
    #[must_use]
    #[allow(clippy::cast_possible_wrap)] // the ids are in the small ranges of SkKnownRuntimeEffects
    pub fn find_or_create_mesh_snippet(&self, spec: &MeshSpecification) -> i32 {
        let mut guarded = self.lock();

        // Use the combination of SkMeshSpecification hash, attribute stride, and uniform stride
        // as our key. In the event of a hash collision, due to differing shader code, color
        // space, or alpha type, we will still have the correct uniform size and attribute stride
        // to prevent unexpected memory sizes for allocated buffers.
        let key = MeshSpecKey {
            hash: mesh_priv::hash(spec),
            attribute_stride: u32::try_from(spec.stride()).expect("a small stride"),
            uniform_size: u32::try_from(spec.uniform_size()).expect("a small uniform size"),
        };

        if let Some(existing_code_snippet_id) = guarded.mesh_map.get(&key) {
            return *existing_code_snippet_id;
        }

        let snippet = Self::convert_mesh_shader(spec);
        guarded.user_defined_code_snippets.push(Arc::new(snippet));
        let new_code_snippet_id = UNKNOWN_RUNTIME_EFFECT_ID_START_ID
            + i32::try_from(guarded.user_defined_code_snippets.len()).expect("a few snippets")
            - 1;

        guarded.mesh_map.insert(key, new_code_snippet_id);
        new_code_snippet_id
    }

    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1030-L1049 (chrome/m156)
    fn convert_mesh_shader(spec: &MeshSpecification) -> ShaderSnippet {
        let num_children = i32::try_from(spec.children().len()).expect("a few children");

        // We emit uniforms for mesh geometry manually at the end of the uniform list where
        // RenderStep uniforms would normally be written. This allows the MeshRenderStep to
        // be responsible for writing the uniform data, since it is where render step
        // uniforms normally reside.
        //
        // We also provide a Noop preamble so any children runtime effects that the user's
        // fragment main function relies on are emitted before we emit their main function.
        // Then after emitting all other preambles, we emit the user's main function at the
        // very end since only the MeshRenderStep relies on it.
        let snippet = ShaderSnippet {
            name: String::from("MeshShader"),
            static_function_name: None,
            snippet_requirement_flags: SnippetRequirementFlags::NONE,
            preamble_generator: Some(noop_preamble),
            num_children,
            ..ShaderSnippet::default()
        };
        snippet.assert_valid();
        snippet
    }

    // Port of: src/gpu/graphite/ShaderCodeDictionary.cpp#L1051-L1100 (chrome/m156)
    fn register_user_defined_known_runtime_effects(
        guarded: &mut Guarded,
        user_defined_known_runtime_effects: &[RuntimeEffect],
    ) -> (Vec<Arc<ShaderSnippet>>, Vec<RuntimeEffect>) {
        let mut known_code_snippets: Vec<Arc<ShaderSnippet>> = Vec::new();
        let mut known_runtime_effects: Vec<RuntimeEffect> = Vec::new();

        for u in user_defined_known_runtime_effects {
            if known_code_snippets.len()
                >= usize::try_from(USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT)
                    .expect("a count")
            {
                // too many user-defined known runtime effects
                // (Skia logs "Too many user-defined known runtime effects. Only %d out of %zu
                // will be known." here.)
                break;
            }

            let key = RuntimeEffectKey {
                hash: runtime_effect_priv::hash(u),
                uniform_size: u32::try_from(u.uniform_size()).expect("a small uniform size"),
            };

            if guarded.runtime_effect_map.contains_key(&key) {
                continue; // This is a duplicate
            }

            known_code_snippets.push(Arc::new(Self::convert_runtime_effect(
                u,
                if runtime_effect_priv::has_name(u) {
                    runtime_effect_priv::get_name(u)
                } else {
                    DEFAULT_USER_DEFINED_KNOWN_NAME
                },
            )));
            let stable_id = USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START
                + u32::try_from(known_code_snippets.len()).expect("a few snippets")
                - 1;

            runtime_effect_priv::set_stable_key(u, stable_id);

            known_runtime_effects.push(u.clone());

            // We register the key with the runtime effect map so that, if the user uses the same
            // code in a separate runtime effect (which they should *not* do), it will be
            // discovered during the unknown-runtime-effect processing and mapped back to the
            // registered user-defined known runtime effect.
            guarded
                .runtime_effect_map
                .insert(key, i32::try_from(stable_id).expect("a small id"));
        }

        debug_assert_eq!(known_code_snippets.len(), known_runtime_effects.len());
        (known_code_snippets, known_runtime_effects)
    }
}
