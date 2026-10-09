// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PaintParamsKey.{h,cpp}

//! [`PaintParamsKey`]: the compact representation of the shader needed to implement a given
//! `PaintParams`, and [`PaintParamsKeyBuilder`], which writes one.
//!
//! The key is a series of nodes where each node consists of:
//!
//! - 4 bytes: code-snippet ID
//! - N child nodes, where N is the constant number of children defined by the
//!   `ShaderCodeDictionary` for the node's snippet ID.
//!
//! Some snippet definitions support embedding data into the key, used when something external to
//! the generated `SkSL` needs to produce unique pipelines (e.g. immutable samplers). For snippets
//! that store data, the data is stored immediately after the ID as:
//!
//! - 4 bytes: code-snippet ID (always >= 0)
//! - 4 bytes: data length (encoded as `-len - 1`, so always < 0 if the snippet embeds data)
//! - 0-M: variable length data (arbitrary sign)
//! - N child nodes
//!
//! All children of a child node are stored in the key before the next child is encoded in the key,
//! e.g. iterating the data in a key is a depth-first traversal of the node tree. When iterating a
//! raw key, a negative value (that's not inside a stretch of embedded data) signals the start of
//! embedded data. Skipping `(-v + 1)` entries returns iteration to indices containing snippet IDs.
//!
//! The key stores multiple root nodes, with each root representing an effect tree that affects
//! different parts of the shading pipeline. The key can only hold 2-4 roots:
//!
//! 1. Color root node: produces the "src" color used in final blending with the "dst" color.
//! 2. Final blend node: defines the blend function combining src and dst colors. If this is a
//!    `FixedBlend` snippet the final pipeline may be able to lift it to HW blending.
//! 3. Clipping: optional, produces analytic coverage from a clip shader or shape.
//! 4. Mesh shader: optional, defines the `SkMeshSpecification` used for the current paint, only
//!    expected to be defined for `drawMesh` calls.
//!
//! Each root node within the key is also preceded by a 4 byte header with a value < 0 defining
//! the type of the node as one of the types listed above. Writers of the key should still add the
//! root blocks in a consistent order since that impacts the key hash/comparison even though
//! technically the generated shaders wouldn't be impacted since they would be the same.

use std::fmt::Write as _;
use std::sync::Arc;

use skia_rust_core::checksum::hash32;
use skia_rust_core::mesh::MeshSpecification;

use crate::graphite::built_in_code_snippet_id::{
    BUILT_IN_CODE_SNIPPET_ID_COUNT, BuiltInCodeSnippetID,
};
use crate::graphite::caps::Caps;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_code_dictionary::{
    ShaderCodeDictionary, ShaderNode, SnippetRequirementFlags,
};
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;
use skia_rust_core::known_runtime_effects::is_skia_known_runtime_effect;

/// The type of a root node, which precedes the node in the key (`RootBlockType`).
// Port of: src/gpu/graphite/PaintParamsKey.h#L35-L40 (chrome/m156)
#[doc(alias = "skgpu::graphite::RootBlockType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum RootBlockType {
    /// `kSrcColor`.
    #[doc(alias = "kSrcColor")]
    SrcColor = -1,
    /// `kFinalBlend`.
    #[doc(alias = "kFinalBlend")]
    FinalBlend = -2,
    /// `kClip`.
    #[doc(alias = "kClip")]
    Clip = -3,
    /// `kMeshShader`.
    #[doc(alias = "kMeshShader")]
    MeshShader = -4,
}

impl RootBlockType {
    /// The root block type with the header value `marker`, if it is one.
    #[must_use]
    pub const fn from_i32(marker: i32) -> Option<Self> {
        Some(match marker {
            -1 => Self::SrcColor,
            -2 => Self::FinalBlend,
            -3 => Self::Clip,
            -4 => Self::MeshShader,
            _ => return None,
        })
    }
}

/// The root nodes of a key, and the mesh specification of its mesh shader (`RootNodesInfo`).
///
/// Skia's struct points into the arena that holds the nodes; here the struct owns them, and the
/// `fSrcColor`, `fFinalBlend`, `fClip` and `fMeshShader` pointers are indices into
/// [`roots`](Self::roots).
// Port of: src/gpu/graphite/PaintParamsKey.h#L42-L51 (chrome/m156)
#[doc(alias = "skgpu::graphite::RootNodesInfo")]
#[derive(Debug, Default)]
pub struct RootNodesInfo {
    /// `fRoots`: all root nodes, in the order they appear in the key.
    pub roots: Vec<ShaderNode>,
    src_color: Option<usize>,
    final_blend: Option<usize>,
    clip: Option<usize>,
    mesh_shader: Option<usize>,
    /// `fMeshSpec`.
    pub mesh_spec: Option<Arc<MeshSpecification>>,
}

impl RootNodesInfo {
    /// `fSrcColor`.
    #[doc(alias = "fSrcColor")]
    #[must_use]
    pub fn src_color(&self) -> Option<&ShaderNode> {
        self.src_color.map(|i| &self.roots[i])
    }

    /// `fFinalBlend`.
    #[doc(alias = "fFinalBlend")]
    #[must_use]
    pub fn final_blend(&self) -> Option<&ShaderNode> {
        self.final_blend.map(|i| &self.roots[i])
    }

    /// `fClip`.
    #[doc(alias = "fClip")]
    #[must_use]
    pub fn clip(&self) -> Option<&ShaderNode> {
        self.clip.map(|i| &self.roots[i])
    }

    /// `fMeshShader`.
    #[doc(alias = "fMeshShader")]
    #[must_use]
    pub fn mesh_shader(&self) -> Option<&ShaderNode> {
        self.mesh_shader.map(|i| &self.roots[i])
    }
}

/// A compact representation of the shader needed to implement a given `PaintParams` (see the
/// module docs for the layout).
///
/// Like Skia's, this is a view of memory that someone else owns: the [`PaintParamsKeyBuilder`]
/// that it was locked from, or the dictionary that stores it. It can be passed around by value.
// Port of: src/gpu/graphite/PaintParamsKey.h#L97-L181 (chrome/m156)
#[doc(alias = "skgpu::graphite::PaintParamsKey")]
#[derive(Clone, Copy, Debug, Default)]
pub struct PaintParamsKey<'a> {
    // The memory referenced in 'fData' is always owned by someone else. It either shares the
    // span from the Builder, or lives in the dictionary.
    data: &'a [i32],
}

impl PartialEq for PaintParamsKey<'_> {
    fn eq(&self, that: &Self) -> bool {
        self.data == that.data
    }
}

impl Eq for PaintParamsKey<'_> {}

impl<'a> PaintParamsKey<'a> {
    /// We don't want keys to get that large, so this limit is quite strict
    /// (`kEmbeddedDataSizeLimit`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L166 (chrome/m156)
    #[doc(alias = "kEmbeddedDataSizeLimit")]
    pub const EMBEDDED_DATA_SIZE_LIMIT: i32 = 16;

    /// `PaintParamsKey(span)`.
    #[must_use]
    pub const fn new(data: &'a [i32]) -> Self {
        Self { data }
    }

    /// `Invalid()`.
    #[doc(alias = "Invalid")]
    #[must_use]
    pub const fn invalid() -> Self {
        Self::new(&[])
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        !self.data.is_empty()
    }

    /// `data()`.
    #[must_use]
    pub const fn data(&self) -> &'a [i32] {
        self.data
    }

    /// `Hash{}(key)`: the hash of the key's data.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L148-L152 (chrome/m156)
    #[must_use]
    pub fn hash(&self) -> u32 {
        let bytes: Vec<u8> = self.data.iter().flat_map(|v| v.to_ne_bytes()).collect();
        hash32(&bytes, 0)
    }

    /// Encodes a regular length as a negative number, or decodes an encoded negative length into
    /// its original length >= 0 (`EncodeDataSize`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L163 (chrome/m156)
    #[doc(alias = "EncodeDataSize")]
    #[must_use]
    pub const fn encode_data_size(size: i32) -> i32 {
        -size - 1
    }

    /// Returns a copy of the key's data that is not attached to a builder (`clone`). Skia's
    /// copies live in an arena; here they are shared.
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L114-L118 (chrome/m156)
    #[must_use]
    pub fn clone_data(&self) -> Arc<[i32]> {
        Arc::from(self.data)
    }

    // Returns None if the node or any of its children have an invalid snippet ID. Recursively
    // creates a node and all of its children, incrementing 'currentIndex' by the total number of
    // nodes created.
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L120-L169 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int to index conversions of a trusted key
    fn create_node(
        &self,
        dict: &ShaderCodeDictionary,
        current_index: &mut usize,
    ) -> Option<ShaderNode> {
        debug_assert!(*current_index < self.data.len());
        let index = *current_index;
        *current_index += 1;
        let id = self.data[index];
        debug_assert!(id >= 0); // Otherwise we're calling createNode on a data length somehow

        let Some(entry) = dict.get_entry(id) else {
            // (Skia logs "Unknown snippet ID in key: %d" here.)
            return None;
        };

        let mut data_span: Vec<u32> = Vec::new();
        if entry.stores_sampler_desc_data() {
            // If a snippet stores data, then the subsequent paint key index signifies the length
            // of its data. Determine this data length and iterate currentIndex past it.
            let stored_data_length_idx = *current_index;
            *current_index += 1;
            debug_assert!(stored_data_length_idx < self.data.len());
            let data_length = Self::encode_data_size(self.data[stored_data_length_idx]);
            // Building ShaderNodes is assumed to be done for keys built by PaintParamsKeyBuilder
            // or already validated for deserialization, so we consider them trusted.
            debug_assert!((0..=Self::EMBEDDED_DATA_SIZE_LIMIT).contains(&data_length));
            let data_length = data_length as usize;
            debug_assert!(stored_data_length_idx + data_length < self.data.len());

            // Gather the data contents (length can now be inferred by the consumers of the data)
            // to pass into ShaderNode creation. Iterate the paint key index past the data
            // indices. For the embedded data, we re-interpret the values as u32's since that was
            // what was written by addData().
            data_span = self.data[stored_data_length_idx + 1..][..data_length]
                .iter()
                .map(|&v| v as u32)
                .collect();
            *current_index += data_length;
        }

        let mut children = Vec::with_capacity(entry.num_children as usize);
        for _ in 0..entry.num_children {
            children.push(self.create_node(dict, current_index)?);
        }

        Some(ShaderNode::new(
            entry,
            children,
            id,
            i32::try_from(index).expect("a small key"),
            data_span,
        ))
    }

    /// Converts the key into a forest of `ShaderNode` trees. If the key is valid this will return
    /// at least one root node. If the key contains unknown shader snippet IDs, returns an empty
    /// [`RootNodesInfo`].
    ///
    /// A valid key will produce either 2 or 3 root nodes. The first root node represents how the
    /// source color is computed. The second node defines the final blender between the calculated
    /// source color and the current pixel's dst color. If provided, the third node calculates an
    /// additional analytic coverage value to combine with the geometry's coverage.
    ///
    /// Before returning the `ShaderNode` trees, this method decides which `ShaderNode`
    /// expressions to lift to the vertex shader, depending on how many varyings are available.
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L247-L322 (chrome/m156)
    #[doc(alias = "getRootNodes")]
    #[must_use]
    pub fn get_root_nodes(
        &self,
        caps: &dyn Caps,
        dict: &ShaderCodeDictionary,
        rte_dict: &RuntimeEffectDictionary,
        mut available_varyings: i32,
        can_lift_coords: bool,
    ) -> RootNodesInfo {
        // TODO: Once the PaintParamsKey creation is organized to represent a single tree starting
        // at the final blend, there will only be a single root node and this can be simplified.
        // For now, we don't know how many roots there are, so collect them into a local array.
        let key_size = self.data.len();

        let mut roots_info = RootNodesInfo::default();
        // Normal PaintParams creation will have up to 4 roots for the different stages.
        let mut liftable_roots: Vec<usize> = Vec::with_capacity(2);
        let mut current_index = 0;
        while current_index < key_size {
            let block_marker = self.data[current_index];
            current_index += 1;
            if block_marker >= 0 {
                return RootNodesInfo::default(); // a bad key
            }
            let r#type = RootBlockType::from_i32(block_marker);
            let Some(root) = self.create_node(dict, &mut current_index) else {
                return RootNodesInfo::default(); // a bad key
            };
            let root_index = roots_info.roots.len();
            roots_info.roots.push(root);
            match r#type {
                Some(RootBlockType::SrcColor) => {
                    debug_assert!(roots_info.src_color.is_none());
                    roots_info.src_color = Some(root_index);
                    liftable_roots.push(root_index);
                }
                Some(RootBlockType::FinalBlend) => {
                    debug_assert!(roots_info.final_blend.is_none());
                    roots_info.final_blend = Some(root_index);
                    liftable_roots.push(root_index);
                }
                Some(RootBlockType::Clip) => {
                    debug_assert!(roots_info.clip.is_none());
                    roots_info.clip = Some(root_index);
                }
                Some(RootBlockType::MeshShader) => {
                    debug_assert!(roots_info.mesh_shader.is_none());
                    roots_info.mesh_shader = Some(root_index);
                }
                None => unreachable!("a negative marker is a root block type, SkUNREACHABLE"),
            }
        }

        if let Some(mesh_shader) = roots_info.mesh_shader {
            roots_info.mesh_spec =
                rte_dict.find_mesh_spec(roots_info.roots[mesh_shader].code_snippet_id());
            if roots_info.mesh_spec.is_none() {
                // Couldn't find the SkMeshSpecification for the mesh shader snippet so the key
                // is bad.
                return RootNodesInfo::default();
            }
        }

        // See what expressions we can lift to the vertex shader.
        if can_lift_coords {
            lift_coord_expressions(
                roots_info
                    .roots
                    .iter_mut()
                    .enumerate()
                    .filter(|(i, _)| liftable_roots.contains(i))
                    .map(|(_, node)| node),
                &mut available_varyings,
            );
        }
        // Don't lift constant expressions if we're using regular UBOs, since lifting is likely
        // only beneficial if we're avoiding a storage buffer access.
        if caps.storage_buffer_support() {
            lift_color_expressions(
                roots_info
                    .roots
                    .iter_mut()
                    .enumerate()
                    .filter(|(i, _)| liftable_roots.contains(i))
                    .map(|(_, node)| node),
                &mut available_varyings,
            );
        }

        roots_info
    }

    /// Converts the key to a structured list of snippet information for debugging or labeling
    /// purposes (`toString`).
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L479-L488 (chrome/m156)
    #[doc(alias = "toString")]
    #[must_use]
    pub fn to_string(&self, caps: &dyn Caps, dict: &ShaderCodeDictionary) -> String {
        let mut str = String::new();
        let key_size = self.data.len();
        let mut current_index = 0;
        while current_index < key_size {
            current_index = key_to_string(caps, &mut str, dict, self.data, current_index, -1);
            str.push(' ');
        }
        if str.is_empty() {
            String::from("(empty)")
        } else {
            str
        }
    }

    /// Prints the key and its nodes to stderr (`dump`, `SK_DEBUG` only in Skia).
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L492-L511 (chrome/m156)
    pub fn dump(&self, caps: &dyn Caps, dict: &ShaderCodeDictionary, id: UniquePaintParamsID) {
        let key_size = self.data.len();

        eprintln!("--------------------------------------");
        let mut line = format!("PaintParamsKey {} (keySize: {}): ", id.as_uint(), key_size);
        for &v in self.data {
            let _ = write!(line, "{:x} ", v.cast_unsigned());
        }
        eprintln!("{line}");

        let mut current_index = 0;
        while current_index < key_size {
            let mut node_str = String::new();
            current_index = key_to_string(caps, &mut node_str, dict, self.data, current_index, 1);
            eprint!("{node_str}");
        }
    }

    /// Checks that a given key is viable for serialization and, also, that a deserialized key is,
    /// at least, correctly formed. Other than that all the sizes make sense, this method also
    /// checks that only Skia-internal shader code snippets appear in the key
    /// (`isSerializable`).
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L574-L589 (chrome/m156)
    #[doc(alias = "isSerializable")]
    #[must_use]
    pub fn is_serializable(&self, dict: &ShaderCodeDictionary) -> bool {
        let key_size = self.data.len();

        let mut current_index = 0;
        while current_index < key_size {
            // Ensure root nodes have their headers set properly, if not the key is malformed.
            let header = self.data[current_index];
            current_index += 1;
            if header >= 0 {
                return false;
            }
            if !is_block_valid(dict, self.data, &mut current_index) {
                return false;
            }
        }

        true
    }
}

// Traverse a ShaderNode tree, attempting to lift any coordinate modification expressions.
// Returns whether any of the given nodes need local coordinate inputs after lifting.
// Port of: src/gpu/graphite/PaintParamsKey.cpp#L173-L226 (chrome/m156)
fn lift_coord_expressions<'a>(
    nodes: impl Iterator<Item = &'a mut ShaderNode>,
    available_varyings: &mut i32,
) -> bool {
    use crate::graphite::shader_code_dictionary::LiftableExpressionType;

    let mut any_need_local_coords = false;

    for node in nodes {
        let mut cur_needs_local_coords = node
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS);

        // Lift expressions from nodes whose liftable expressions are on coordinate inputs.
        if *available_varyings > 0
            && cur_needs_local_coords
            && node.entry().liftable_expression_type == LiftableExpressionType::LocalCoords
        {
            *available_varyings -= 1;

            // (SK_USE_LEGACY_UNIFORM_LIFTING_GRAPHITE is not defined.)
            // We can potentially lift the nested expressions under here as well.
            let child_needs_our_coords =
                lift_coord_expressions(node.children_mut().iter_mut(), available_varyings);
            // If no child needs our lifted coords, we can omit them from the fragment shader
            // entirely, and only use them in the vertex shader for calculating other coords.
            if child_needs_our_coords {
                node.set_lift_expression_flag();
            } else {
                node.set_omit_expression_flag();
            }
            // Since we lifted the coordinate expression here, this node no longer needs a local
            // coords argument.
            cur_needs_local_coords = false;
            node.unset_local_coords_flag();

        // If the node passes through its local coords to its children, we check if those perform
        // modifications that can be lifted.
        } else if *available_varyings > 0
            && node
                .required_flags()
                .contains(SnippetRequirementFlags::PASSTHROUGH_LOCAL_COORDS)
        {
            // Assume that this node doesn't need local coordinates unless its actual shader
            // snippet entry does, or one of its children does even after accounting for lifting.
            let entry_needs_local_coords = node.entry().needs_local_coords();
            let child_needs_local_coords =
                lift_coord_expressions(node.children_mut().iter_mut(), available_varyings);
            cur_needs_local_coords = entry_needs_local_coords || child_needs_local_coords;
            if !cur_needs_local_coords {
                node.unset_local_coords_flag();
            }
        }

        any_need_local_coords |= cur_needs_local_coords;
    }

    any_need_local_coords
}

// Traverse a list of ShaderNodes, attempting to lift any expressions that resolve to a color.
// For now, this does not recurse into ShaderNodes' lists of children. In practice we only lift
// solid color expressions, and we only care to lift such expressions if there is no other fragment
// shader work (i.e., if the solid color expression is a root node in a shader's ShaderNode tree).
// If there is other fragment shader work, we'll likely be accessing other fragment shader
// uniforms, the color value will likely be cached, and lifting may not be worth the extra
// varying.
// Port of: src/gpu/graphite/PaintParamsKey.cpp#L234-L245 (chrome/m156)
fn lift_color_expressions<'a>(
    nodes: impl Iterator<Item = &'a mut ShaderNode>,
    available_varyings: &mut i32,
) {
    use crate::graphite::shader_code_dictionary::LiftableExpressionType;

    // (SK_USE_LEGACY_UNIFORM_LIFTING_GRAPHITE is not defined.)
    for node in nodes {
        if *available_varyings > 0
            && node.entry().liftable_expression_type == LiftableExpressionType::PriorStageOutput
        {
            *available_varyings -= 1;
            node.set_lift_expression_flag();
        }
    }
}

// SkBase64::Encode with the default encoding map, which is the standard alphabet with '='
// padding.
// Port of: src/core/SkBase64.cpp#L113-L156 (chrome/m156)
fn base64_encode(src: &[u8]) -> String {
    const ENCODE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut dst = String::with_capacity(src.len().div_ceil(3) * 4);
    for chunk in src.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).map_or(0, |&v| u32::from(v));
        let c = chunk.get(2).map_or(0, |&v| u32::from(v));
        dst.push(ENCODE[(a >> 2) as usize] as char);
        dst.push(ENCODE[((b >> 4 | a << 4) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            dst.push(ENCODE[((c >> 6 | b << 2) & 0x3F) as usize] as char);
        } else {
            dst.push('=');
        }
        if chunk.len() > 2 {
            dst.push(ENCODE[(c & 0x3F) as usize] as char);
        } else {
            dst.push('=');
        }
    }
    dst
}

// Port of: src/gpu/graphite/PaintParamsKey.cpp#L324-L335 (chrome/m156)
fn append_as_base64(str: &mut String, data: &[i32]) {
    str.push('(');
    let _ = write!(str, "{}", data.len());
    str.push_str(": ");
    // Encode data in base64 to shorten it
    let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_ne_bytes()).collect();
    str.push_str(&base64_encode(&bytes));
    str.push(')');
}

const COMPOSE_ID: i32 = BuiltInCodeSnippetID::Compose as i32;

// Port of: src/gpu/graphite/PaintParamsKey.cpp#L337-L477 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ int to index conversions of a trusted key
#[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
fn key_to_string(
    caps: &dyn Caps,
    str: &mut String,
    dict: &ShaderCodeDictionary,
    key_data: &[i32],
    mut current_index: usize,
    indent: i32,
) -> usize {
    debug_assert!(current_index < key_data.len());

    let multiline = indent >= 0;
    if multiline {
        // Format for multi-line printing: `%*c` with the width `2 * indent` and the char ' '.
        // (A width of 0 still prints the character.)
        str.push_str(&" ".repeat((2 * indent).max(1) as usize));
    }

    let mut id = key_data[current_index];
    current_index += 1;
    // Skip the root block headers.
    if id < 0 {
        if multiline {
            match RootBlockType::from_i32(id) {
                Some(RootBlockType::SrcColor) => str.push_str("[RootSrcColor] "),
                Some(RootBlockType::FinalBlend) => str.push_str("[RootFinalBlend] "),
                Some(RootBlockType::Clip) => str.push_str("[RootClip] "),
                Some(RootBlockType::MeshShader) => str.push_str("[RootMeshShader] "),
                None => unreachable!("SkUNREACHABLE"),
            }
        }
        id = key_data[current_index];
        current_index += 1;
    }

    let Some(entry) = dict.get_entry(id) else {
        str.push_str("Unknown(");
        let _ = write!(str, "{id}");
        str.push(')');
        return current_index;
    };

    // Single lined Composes get shortened to just a plus between its two children, e.g. Compose
    // [ A B ] becomes A+B. We don't do a similar prettification for Blend [ A B C ] to (A B)+C
    // because that's not quite as readable and they aren't nearly as common within keys. To make
    // sure chains of Composes are not ambiguous, we only consolidate cases where the inner node
    // is not Compose, e.g. Compose [ A Compose [ B C ]] => A+B+C but [ Compose [ Compose [ A B ]
    // C ] does not become A+B+C
    debug_assert_eq!(
        dict.get_entry_built_in(BuiltInCodeSnippetID::Compose)
            .num_children,
        2
    );
    let pretty_compose =
        // single-lined Compose block
        id == COMPOSE_ID && !multiline &&
        // that doesn't have an inner Compose child
        current_index < key_data.len() && key_data[current_index] != COMPOSE_ID;

    if pretty_compose {
        debug_assert_eq!(entry.num_children, 2); // ordered [inner, outer]
        current_index = key_to_string(caps, str, dict, key_data, current_index, indent);
        str.push('+');
        return key_to_string(caps, str, dict, key_data, current_index, indent);
    }

    str.push_str(&entry.name);

    if entry.stores_sampler_desc_data() {
        debug_assert!(current_index + 1 < key_data.len());

        // If an entry stores data, then the next key value reports the quantity of key indices
        // that are used to house the data for this snippet. This way, we know how many indices to
        // iterate over in order to capture the snippet's data before we may encounter another
        // snippet ID.
        // For example:
        // [snippetId using 2 indices worth of data] [2] [dataValue0] [dataValue1] [next snippet ID]
        let data_index_count = PaintParamsKey::encode_data_size(key_data[current_index]);
        current_index += 1;
        // Printing keys is assumed to be done for keys that were built by PaintParamsKeyBuilder
        // or already validated for deserialization, so we consider them trusted.
        debug_assert!((0..=PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT).contains(&data_index_count));
        let data_index_count = data_index_count as usize;
        debug_assert!(current_index + data_index_count < key_data.len());

        let mut descriptive_form_appended = false;
        if data_index_count == 0 {
            // We shorten the string for the common case of no extra data.
            str.push_str("(0)");
            descriptive_form_appended = true;
        } else {
            debug_assert!(
                data_index_count == 2 || data_index_count == 3,
                "count {data_index_count}"
            );

            // Attempt to append the sampler data as human-readable YCbCr information
            let s = SamplerDesc::from_raw(
                key_data[current_index] as u32,
                key_data[current_index + 1] as u32,
                /* extFormatMSB= */
                if data_index_count == 3 {
                    key_data[current_index + 2] as u32
                } else {
                    0
                },
            );

            if s.is_immutable() {
                let tmp = caps.immutable_sampler_info_to_string(&s.immutable_sampler_info());
                if !tmp.is_empty() {
                    str.push('(');
                    str.push_str(&tmp);
                    str.push(')');
                    descriptive_form_appended = true;
                }
            }
        }

        if !descriptive_form_appended {
            append_as_base64(str, &key_data[current_index..][..data_index_count]);
        }

        // Increment current index past the indices which contain data
        current_index += data_index_count;
    }

    if entry.num_children > 0 {
        let mut indent = indent;
        if multiline {
            str.push_str(":\n");
            indent += 1;
        } else {
            str.push('[');
        }

        for i in 0..entry.num_children {
            if i > 0 {
                str.push_str(", ");
            }
            current_index = key_to_string(caps, str, dict, key_data, current_index, indent);
        }

        if !multiline {
            str.push(']');
        }
    }

    if multiline && entry.num_children == 0 {
        str.push('\n');
    }
    current_index
}

// check a single block and, recursively, all its children
// Port of: src/gpu/graphite/PaintParamsKey.cpp#L518-L569 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // the length is checked to be in range first
#[allow(clippy::cast_possible_wrap)] // mirrors the C++ size_t to int conversions of a key span
fn is_block_valid(
    dict: &ShaderCodeDictionary,
    key_data: &[i32],
    current_index: &mut usize,
) -> bool {
    if *current_index >= key_data.len() {
        return false;
    }

    let id = key_data[*current_index];
    *current_index += 1;
    if id >= BUILT_IN_CODE_SNIPPET_ID_COUNT
        && !is_skia_known_runtime_effect(id)
        && !dict.is_user_defined_known_runtime_effect(id)
    {
        return false;
    }

    let Some(entry) = dict.get_entry(id) else {
        return false;
    };

    if entry.stores_sampler_desc_data() {
        if *current_index >= key_data.len() {
            return false;
        }

        let data_length = key_data[*current_index];
        *current_index += 1;
        // This `dataLength` is untrusted, so check that it doesn't overflow EncodeDataSize
        // and that matches expectations of a valid length (i.e. it started out negative and is
        // now positive and less than the key data limit).
        if data_length >= 0
            || data_length
                < PaintParamsKey::encode_data_size(PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT)
        {
            // Would not produce a valid size after decoding
            return false;
        }
        let data_length = PaintParamsKey::encode_data_size(data_length);
        debug_assert!((0..=PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT).contains(&data_length));
        if *current_index + data_length as usize > key_data.len() {
            return false;
        }

        *current_index += data_length as usize;
    }

    if entry.num_children > 0 {
        for _ in 0..entry.num_children {
            if !is_block_valid(dict, key_data, current_index) {
                return false;
            }
        }
    }

    true
}

//--------------------------------------------------------------------------------------------------
// PaintParamsKeyBuilder

#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug)]
struct StackFrame {
    code_snippet_id: i32,
    num_expected_children: i32,
    num_actual_children: i32,
    data_size: i32,
}

/// Writes a [`PaintParamsKey`].
///
/// The builder and the keys locked from it share the same underlying block of memory. When a key
/// is locked from the builder (`lockAsKey`, `AutoLockBuilderAsKey`), the builder cannot be used
/// until the key goes out of scope.
///
/// This arrangement is intended to improve performance in the expected case, where a builder is
/// being used in a tight loop to generate keys which can be recycled once they've been used to
/// find the dictionary's matching unique id. We don't expect the cost of copying the key's memory
/// into the dictionary to be prohibitive since that should be infrequent.
// Port of: src/gpu/graphite/PaintParamsKey.h#L193-L351 (chrome/m156)
#[doc(alias = "skgpu::graphite::PaintParamsKeyBuilder")]
#[derive(Debug)]
pub struct PaintParamsKeyBuilder {
    // The data array uses clear() on reset so that its underlying storage and repeated use of
    // the builder will hit a high-water mark and avoid lots of allocations when recording draws.
    data: Vec<i32>,
    has_error: bool, // if true, fData may not encode a valid/complete ShaderNode tree.
    data_high_water_mark: usize,

    // (`SkDEBUGCODE(fDict = dict;)`: the dictionary only checks the blocks the builder writes.)
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    dict: ShaderCodeDictionary,
    // Information about the current block being written
    #[cfg(debug_assertions)]
    stack: Vec<StackFrame>,
    #[cfg(debug_assertions)]
    locked: bool,
}

impl PaintParamsKeyBuilder {
    /// `PaintParamsKeyBuilder(dict)`.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L201-L203 (chrome/m156)
    #[must_use]
    pub fn new(dict: &ShaderCodeDictionary) -> Self {
        Self {
            data: Vec::new(),
            has_error: false,
            data_high_water_mark: 0,
            dict: dict.clone(),
            #[cfg(debug_assertions)]
            stack: Vec::new(),
            #[cfg(debug_assertions)]
            locked: false,
        }
    }

    /// Whether the key built so far is the key `that` (`operator==`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L207-L210 (chrome/m156)
    #[must_use]
    pub fn eq_key(&self, that: &PaintParamsKey<'_>) -> bool {
        // Don't need to lock and unlock the builder because this PaintParamsKey goes out of scope.
        PaintParamsKey::new(&self.data) == *that
    }

    /// `addRootBlockHeader`.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L213-L216 (chrome/m156)
    #[doc(alias = "addRootBlockHeader")]
    pub fn add_root_block_header(&mut self, r#type: RootBlockType) {
        #[cfg(debug_assertions)]
        debug_assert!(!self.locked);
        self.data.push(r#type as i32);
    }

    /// `beginBlock(id)`; `code_snippet_id` is a `BuiltInCodeSnippetID` or a runtime effect's id.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L219-L223 (chrome/m156)
    #[doc(alias = "beginBlock")]
    pub fn begin_block(&mut self, code_snippet_id: impl Into<u32>) {
        let code_snippet_id: u32 = code_snippet_id.into();
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.locked);
            self.push_stack(code_snippet_id.cast_signed());
        }
        self.data.push(code_snippet_id.cast_signed());
    }

    // TODO: Have endBlock() be handled automatically with RAII, in which case we could have it
    // validate the snippet ID being popped off the stack frame.
    /// `endBlock`.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L227-L229 (chrome/m156)
    #[doc(alias = "endBlock")]
    pub fn end_block(&mut self) {
        #[cfg(debug_assertions)]
        self.pop_stack();
    }

    /// Checks that the builder has been reset to its initial state prior to creating a new key
    /// (`checkReset`).
    // Port of: src/gpu/graphite/PaintParamsKey.cpp#L28-L33 (chrome/m156)
    #[doc(alias = "checkReset")]
    pub fn check_reset(&self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.locked);
            debug_assert_eq!(self.data.len(), 0);
            debug_assert!(self.stack.is_empty());
            debug_assert!(!self.has_error);
        }
    }

    /// Helper to add blocks that don't have children (`addBlock`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L237-L240 (chrome/m156)
    #[doc(alias = "addBlock")]
    pub fn add_block(&mut self, id: BuiltInCodeSnippetID) {
        self.begin_block(id);
        self.end_block();
    }

    /// `addData`: the data of the current block, which must be one that stores sampler data.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L242-L247 (chrome/m156)
    #[doc(alias = "addData")]
    #[allow(clippy::cast_possible_wrap)] // reinterprets the u32 words as i32, as the C++ does
    #[allow(clippy::cast_possible_truncation)] // the size is limited to kEmbeddedDataSizeLimit
    pub fn add_data(&mut self, data: &[u32]) {
        // First push the data size followed by the actual data.
        #[cfg(debug_assertions)]
        self.validate_data(data.len());
        self.data
            .push(PaintParamsKey::encode_data_size(data.len() as i32));
        self.data.extend(data.iter().map(|&v| v as i32));
    }

    /// `addErrorBlock`.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L249-L254 (chrome/m156)
    #[doc(alias = "addErrorBlock")]
    pub fn add_error_block(&mut self) {
        self.has_error = true;
        // Preserve the structure of parent stack, but since fHasError is true, the builder won't
        // produce a valid PaintParamsKey.
        self.add_block(BuiltInCodeSnippetID::Error);
    }

    /// `tryShrinkCapacity`.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L256-L263 (chrome/m156)
    #[doc(alias = "tryShrinkCapacity")]
    pub fn try_shrink_capacity(&mut self) {
        let half_capacity = self.data.capacity() / 2;
        if self.data_high_water_mark < half_capacity {
            self.data_high_water_mark = 0;
            debug_assert_eq!(self.data.len(), 0);
            // (TArray::reserve_exact never shrinks, and neither does this.)
            self.data.reserve_exact(half_capacity);
        }
    }

    /// Reset to an empty key (`resetForDraw`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L266-L273 (chrome/m156)
    #[doc(alias = "resetForDraw")]
    pub fn reset_for_draw(&mut self) {
        #[cfg(debug_assertions)]
        debug_assert!(!self.locked);
        self.data.clear();
        self.has_error = false;

        #[cfg(debug_assertions)]
        self.stack.clear();
        self.check_reset();
    }

    /// Replaces the id of the last block, which must not have children or data, returning the
    /// old id (`replaceLastBlock`).
    ///
    /// # Panics
    /// If the builder holds no block, or the last block is not a built-in one.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L275-L288 (chrome/m156)
    #[doc(alias = "replaceLastBlock")]
    #[allow(clippy::cast_sign_loss)] // the replaced ids are validated to be built-in ones
    pub fn replace_last_block(&mut self, new_id: BuiltInCodeSnippetID) -> BuiltInCodeSnippetID {
        // A valid replacement cannot have auxiliary data and requires the children count to be
        // same. However, since this is taking the old ID from the last index, the old block by
        // definition has no children so newID cannot either.
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.locked);
            debug_assert!(self.stack.is_empty());
        }

        let index = self.data.len() - 1;
        #[cfg(debug_assertions)]
        self.validate_replacement(self.data[index], new_id as i32);
        let old_id = BuiltInCodeSnippetID::from_u32(self.data[index] as u32)
            .expect("the last block is a built-in one");
        self.data[index] = new_id as i32;
        old_id
    }

    /// Replaces every block with the id `old_id` by one with `new_id` (`replaceBlocks`).
    ///
    /// Like Skia's loop, this treats every negative entry as the length of embedded data, which
    /// includes the root block headers: the block that follows the header of the second, third or
    /// fourth root is skipped. (Nothing in Skia calls it.)
    // Port of: src/gpu/graphite/PaintParamsKey.h#L290-L305 (chrome/m156)
    #[doc(alias = "replaceBlocks")]
    #[allow(clippy::cast_sign_loss)] // the data length of a negative marker is non-negative
    pub fn replace_blocks(&mut self, old_id: BuiltInCodeSnippetID, new_id: BuiltInCodeSnippetID) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.locked);
            debug_assert!(self.stack.is_empty());
            self.validate_replacement(old_id as i32, new_id as i32);
        }
        let mut i = 0;
        while i < self.data.len() {
            if self.data[i] < 0 {
                // This is embedded data, so skip over its length in case any of its data values
                // happened to equal oldID
                i += PaintParamsKey::encode_data_size(self.data[i]) as usize;
            } else if self.data[i] == old_id as i32 {
                // Replace the old ID with the new ID
                self.data[i] = new_id as i32;
            } // else leave other IDs alone
            i += 1;
        }
    }

    /// Returns a view of this builder as a [`PaintParamsKey`]. The builder cannot be used until
    /// the returned guard goes out of scope (`AutoLockBuilderAsKey`).
    // Port of: src/gpu/graphite/PaintParamsKey.h#L312-L319 (chrome/m156)
    #[doc(alias = "AutoLockBuilderAsKey")]
    pub fn lock_as_key(&mut self) -> AutoLockBuilderAsKey<'_> {
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.locked); // lockAsKey() is not re-entrant
            debug_assert!(self.stack.is_empty()); // All beginBlocks() had a matching endBlock()

            self.locked = true;
        }
        self.data_high_water_mark = self.data_high_water_mark.max(self.data.len());
        AutoLockBuilderAsKey { builder: self }
    }

    #[cfg(debug_assertions)]
    #[allow(clippy::cast_possible_wrap)] // child counts are small
    fn push_stack(&mut self, code_snippet_id: i32) {
        debug_assert!(!self.locked);
        debug_assert!(self.dict.is_valid_id(code_snippet_id));
        // If the kError ID is pushed, fHasError must have been set already.
        debug_assert!(code_snippet_id != BuiltInCodeSnippetID::Error as i32 || self.has_error);

        if let Some(back) = self.stack.last_mut() {
            back.num_actual_children += 1;
            debug_assert!(back.num_actual_children <= back.num_expected_children);
        }

        let snippet = self
            .dict
            .get_entry(code_snippet_id)
            .expect("the id was validated");
        self.stack.push(StackFrame {
            code_snippet_id,
            num_expected_children: snippet.num_children,
            num_actual_children: 0,
            data_size: -1,
        });
    }

    #[cfg(debug_assertions)]
    #[allow(clippy::cast_possible_wrap)] // data sizes are limited to kEmbeddedDataSizeLimit
    fn validate_data(&mut self, data_size: usize) {
        // Check that the size of the embedded data fits our self-imposed limit.
        debug_assert!(
            i32::try_from(data_size)
                .is_ok_and(|size| size <= PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT)
        );

        debug_assert!(!self.locked);
        debug_assert!(!self.stack.is_empty()); // addData() called within code snippet block
        // Check that addData() is only called for snippets that support it and is only called once
        let back = *self.stack.last().expect("checked above");
        let snippet = self
            .dict
            .get_entry(back.code_snippet_id)
            .expect("the id was validated");
        debug_assert!(snippet.stores_sampler_desc_data());
        debug_assert!(back.data_size < 0);

        self.stack.last_mut().expect("checked above").data_size =
            i32::try_from(data_size).expect("checked against the limit above");
    }

    #[cfg(debug_assertions)]
    fn pop_stack(&mut self) {
        debug_assert!(!self.locked);
        debug_assert!(!self.stack.is_empty());
        let back = *self.stack.last().expect("checked above");
        debug_assert_eq!(back.num_actual_children, back.num_expected_children);
        let expects_data = self
            .dict
            .get_entry(back.code_snippet_id)
            .expect("the id was validated")
            .stores_sampler_desc_data();
        let has_data = back.data_size >= 0;
        debug_assert_eq!(expects_data, has_data);
        self.stack.pop();
    }

    #[cfg(debug_assertions)]
    fn validate_replacement(&self, old_code_snippet_id: i32, new_code_snippet_id: i32) {
        // Rules for allowed replacements:
        // 1. BuiltInCodeSnippetIDs only
        // 2. Have the same number of children (and 0 if allowChildren is false)
        // 3. Have the same uniform and texture signature
        // 4. No extra data
        //
        // This ensures that replacing on a block by block basis produces a valid ShaderNode tree
        // after modification, and that extracted uniforms and textures for the original key can
        // be used with the new key.
        debug_assert!((0..BUILT_IN_CODE_SNIPPET_ID_COUNT).contains(&old_code_snippet_id));
        debug_assert!((0..BUILT_IN_CODE_SNIPPET_ID_COUNT).contains(&new_code_snippet_id));

        let old_snippet = self
            .dict
            .get_entry(old_code_snippet_id)
            .expect("a built-in id");
        let new_snippet = self
            .dict
            .get_entry(new_code_snippet_id)
            .expect("a built-in id");

        debug_assert_eq!(old_snippet.num_children, new_snippet.num_children);

        // Same signature, not caring about the actual variable names.
        debug_assert_eq!(old_snippet.uniforms.len(), new_snippet.uniforms.len());
        for (old_u, new_u) in old_snippet.uniforms.iter().zip(&new_snippet.uniforms) {
            debug_assert_eq!(old_u.ty(), new_u.ty());
            debug_assert_eq!(old_u.count(), new_u.count());
            debug_assert_eq!(old_u.is_paint_color(), new_u.is_paint_color());
        }

        debug_assert_eq!(
            old_snippet.textures_and_samplers.len(),
            new_snippet.textures_and_samplers.len()
        );

        debug_assert!(!old_snippet.stores_sampler_desc_data());
        debug_assert!(!new_snippet.stores_sampler_desc_data());
    }
}

/// Locks a [`PaintParamsKeyBuilder`] as a key until it is dropped (`AutoLockBuilderAsKey`).
///
/// Skia's class dereferences to the key; here [`key`](Self::key) returns the key view.
// Port of: src/gpu/graphite/PaintParamsKey.h#L353-L370 (chrome/m156)
#[derive(Debug)]
pub struct AutoLockBuilderAsKey<'a> {
    builder: &'a mut PaintParamsKeyBuilder,
}

impl AutoLockBuilderAsKey<'_> {
    /// The key: invalid if the builder had an error (`fKey`).
    #[must_use]
    pub fn key(&self) -> PaintParamsKey<'_> {
        if self.builder.has_error {
            PaintParamsKey::invalid()
        } else {
            PaintParamsKey::new(&self.builder.data)
        }
    }
}

impl Drop for AutoLockBuilderAsKey<'_> {
    // Invalidates any PaintParamsKey returned by lockAsKey() unless it has been cloned.
    // Port of: src/gpu/graphite/PaintParamsKey.h#L322-L325 (chrome/m156)
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.builder.locked);
            self.builder.locked = false;
        }
    }
}
