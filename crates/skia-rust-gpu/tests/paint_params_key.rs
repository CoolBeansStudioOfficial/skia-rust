// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/PaintParamsKey.{h,cpp} (the tests that need ShaderInfo,
//                    tests/graphite/KeyTest.cpp and PaintParamsKeyTest.cpp, wait for G6/G14)

//! `PaintParamsKey` and its builder: key building, interning in the dictionary, `toString`, root
//! node extraction (with the lifting of coordinate and color expressions) and serialization
//! checks.

mod support;

use std::sync::Arc;

use skia_rust_gpu::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID as Id;
use skia_rust_gpu::graphite::paint_params_key::{
    PaintParamsKey, PaintParamsKeyBuilder, RootBlockType,
};
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::{
    ShaderCodeDictionary, ShaderNode, SnippetRequirementFlags,
};
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;
use support::MockCaps;

fn dictionary() -> ShaderCodeDictionary {
    ShaderCodeDictionary::new(Layout::Std140, &[])
}

/// `SolidColor`, then a `SrcOver` blend: the key of a plain solid fill.
fn build_solid_src_over(builder: &mut PaintParamsKeyBuilder) {
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.add_block(Id::SolidColorShader);
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrcOver);
}

fn key_data(builder: &mut PaintParamsKeyBuilder) -> Vec<i32> {
    builder.lock_as_key().key().data().to_vec()
}

#[test]
fn builder_writes_the_documented_layout() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    build_solid_src_over(&mut builder);
    assert_eq!(
        key_data(&mut builder),
        vec![
            RootBlockType::SrcColor as i32,
            Id::SolidColorShader as i32,
            RootBlockType::FinalBlend as i32,
            Id::FixedBlendSrcOver as i32,
        ]
    );
    assert_eq!(RootBlockType::SrcColor as i32, -1);
    assert_eq!(RootBlockType::FinalBlend as i32, -2);
    assert_eq!(RootBlockType::Clip as i32, -3);
    assert_eq!(RootBlockType::MeshShader as i32, -4);

    // Children follow their parent depth first.
    builder.reset_for_draw();
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::Compose);
    builder.add_block(Id::SolidColorShader);
    builder.add_block(Id::MatrixColorFilter);
    builder.end_block();
    assert_eq!(
        key_data(&mut builder),
        vec![
            -1,
            Id::Compose as i32,
            Id::SolidColorShader as i32,
            Id::MatrixColorFilter as i32
        ]
    );
}

#[test]
fn data_is_stored_after_the_id_with_an_encoded_length() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::ImageShader);
    builder.add_data(&[1, 2]);
    builder.end_block();
    assert_eq!(
        key_data(&mut builder),
        vec![
            -1,
            Id::ImageShader as i32,
            PaintParamsKey::encode_data_size(2),
            1,
            2
        ]
    );
    assert_eq!(PaintParamsKey::encode_data_size(2), -3);
    assert_eq!(PaintParamsKey::encode_data_size(-3), 2);
    assert_eq!(PaintParamsKey::encode_data_size(0), -1);
    assert_eq!(PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT, 16);
}

#[test]
fn keys_are_interned_with_ids_from_one() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);

    build_solid_src_over(&mut builder);
    let first = dict.find_or_create_for_builder(&mut builder);
    assert!(first.is_valid());
    assert_eq!(first.as_uint(), 1); // 0 is reserved as invalid

    // The same key again.
    builder.reset_for_draw();
    build_solid_src_over(&mut builder);
    assert!(builder.eq_key(&PaintParamsKey::new(&dict.lookup(first))));
    assert_eq!(dict.find_or_create_for_builder(&mut builder), first);

    // A different key gets the next id.
    builder.reset_for_draw();
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.add_block(Id::SolidColorShader);
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrc);
    let second = dict.find_or_create_for_builder(&mut builder);
    assert_eq!(second.as_uint(), 2);
    assert_ne!(first, second);
    assert!(!builder.eq_key(&PaintParamsKey::new(&dict.lookup(first))));

    // Looking the ids up gives the keys' data.
    assert_eq!(
        &*dict.lookup(first),
        &[
            -1,
            Id::SolidColorShader as i32,
            -2,
            Id::FixedBlendSrcOver as i32
        ]
    );
    assert_eq!(
        &*dict.lookup(second),
        &[
            -1,
            Id::SolidColorShader as i32,
            -2,
            Id::FixedBlendSrc as i32
        ]
    );

    // The invalid id has the invalid key, and the invalid key the invalid id.
    assert!(dict.lookup(UniquePaintParamsID::invalid()).is_empty());
    assert!(!PaintParamsKey::new(&dict.lookup(UniquePaintParamsID::invalid())).is_valid());
    assert_eq!(
        dict.find_or_create(&PaintParamsKey::invalid()),
        UniquePaintParamsID::invalid()
    );

    // Keys of two dictionaries are unrelated, and a handle shares its dictionary.
    let other = dictionary();
    let mut other_builder = PaintParamsKeyBuilder::new(&other);
    build_solid_src_over(&mut other_builder);
    assert_eq!(
        other
            .find_or_create_for_builder(&mut other_builder)
            .as_uint(),
        1
    );
    let handle = dict.clone();
    builder.reset_for_draw();
    build_solid_src_over(&mut builder);
    assert_eq!(handle.find_or_create_for_builder(&mut builder), first);
}

#[test]
fn an_error_block_makes_the_key_invalid() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::Compose);
    builder.add_block(Id::SolidColorShader);
    builder.add_error_block();
    builder.end_block();
    {
        let lock = builder.lock_as_key();
        assert!(!lock.key().is_valid());
    }
    assert_eq!(
        dict.find_or_create_for_builder(&mut builder),
        UniquePaintParamsID::invalid()
    );

    // Resetting clears the error.
    builder.reset_for_draw();
    build_solid_src_over(&mut builder);
    assert!(dict.find_or_create_for_builder(&mut builder).is_valid());
}

#[test]
fn replacing_blocks() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::Compose);
    builder.begin_block(Id::ImageShader);
    // Data that looks like the id that is replaced below.
    builder.add_data(&[Id::FixedBlendSrcOver as u32, 7]);
    builder.end_block();
    builder.add_block(Id::FixedBlendSrcOver);
    builder.end_block();
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrcOver);

    builder.replace_blocks(Id::FixedBlendSrcOver, Id::FixedBlendDstOver);
    assert_eq!(
        key_data(&mut builder),
        vec![
            -1,
            Id::Compose as i32,
            Id::ImageShader as i32,
            PaintParamsKey::encode_data_size(2),
            Id::FixedBlendSrcOver as i32, // data is left alone
            7,
            Id::FixedBlendDstOver as i32,
            -2,
            // Skia's loop treats a root block header like the length of embedded data
            // (`EncodeDataSize(-2)` is 1) and skips the block after it. It is only used for the
            // key of the final blend, which `replaceLastBlock` replaces.
            Id::FixedBlendSrcOver as i32,
        ]
    );

    let old = builder.replace_last_block(Id::FixedBlendXor);
    assert_eq!(old, Id::FixedBlendSrcOver);
    assert_eq!(
        *key_data(&mut builder).last().unwrap(),
        Id::FixedBlendXor as i32
    );
}

#[test]
fn the_builder_can_shrink_and_be_reused() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    for _ in 0..3 {
        builder.reset_for_draw();
        build_solid_src_over(&mut builder);
        let id = dict.find_or_create_for_builder(&mut builder);
        assert_eq!(id.as_uint(), 1);
        builder.reset_for_draw();
        builder.try_shrink_capacity();
    }
}

#[cfg(debug_assertions)]
mod debug_checks {
    //! The checks of `SK_DEBUG` builds.
    use super::*;

    #[test]
    #[should_panic(expected = "assertion")]
    fn too_few_children() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.add_root_block_header(RootBlockType::SrcColor);
        builder.begin_block(Id::Compose);
        builder.add_block(Id::SolidColorShader);
        builder.end_block();
    }

    #[test]
    #[should_panic(expected = "num_actual_children")]
    fn too_many_children() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.add_root_block_header(RootBlockType::SrcColor);
        builder.begin_block(Id::SolidColorShader);
        builder.add_block(Id::SolidColorShader);
    }

    #[test]
    #[should_panic(expected = "stores_sampler_desc_data")]
    fn data_for_a_snippet_that_has_none() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.add_root_block_header(RootBlockType::SrcColor);
        builder.begin_block(Id::SolidColorShader);
        builder.add_data(&[1]);
    }

    #[test]
    #[should_panic(expected = "assertion")]
    fn missing_data() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.add_root_block_header(RootBlockType::SrcColor);
        builder.begin_block(Id::ImageShader);
        builder.end_block();
    }

    #[test]
    #[should_panic(expected = "is_valid_id")]
    fn unknown_ids() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.begin_block(9999_u32);
    }

    #[test]
    #[should_panic(expected = "assertion")]
    fn replacement_must_keep_the_children() {
        let dict = dictionary();
        let mut builder = PaintParamsKeyBuilder::new(&dict);
        builder.add_root_block_header(RootBlockType::SrcColor);
        builder.add_block(Id::SolidColorShader);
        builder.replace_last_block(Id::LocalMatrixShader);
    }
}

//--------------------------------------------------------------------------------------------------
// toString

fn to_string_of(
    dict: &ShaderCodeDictionary,
    build: impl FnOnce(&mut PaintParamsKeyBuilder),
) -> String {
    to_string_with_caps(dict, &MockCaps::default(), build)
}

fn to_string_with_caps(
    dict: &ShaderCodeDictionary,
    caps: &MockCaps,
    build: impl FnOnce(&mut PaintParamsKeyBuilder),
) -> String {
    let mut builder = PaintParamsKeyBuilder::new(dict);
    build(&mut builder);
    let id = dict.find_or_create_for_builder(&mut builder);
    assert!(id.is_valid());
    let from_dict = dict.id_to_string(caps, id);
    let lock = builder.lock_as_key();
    assert_eq!(lock.key().to_string(caps, dict), from_dict);
    from_dict
}

#[test]
fn to_string_lists_the_roots() {
    let dict = dictionary();
    assert_eq!(
        to_string_of(&dict, build_solid_src_over),
        "SolidColor SrcOver "
    );
    assert_eq!(
        PaintParamsKey::invalid().to_string(&MockCaps::default(), &dict),
        "(empty)"
    );
    assert_eq!(
        dict.id_to_string(&MockCaps::default(), UniquePaintParamsID::invalid()),
        "(empty)"
    );
}

#[test]
fn to_string_shows_children_in_brackets_and_composes_with_plus() {
    let dict = dictionary();
    // Compose of two leaves is "inner+outer".
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::Compose);
            b.add_block(Id::SolidColorShader);
            b.add_block(Id::MatrixColorFilter);
            b.end_block();
        }),
        "SolidColor+MatrixColorFilter "
    );

    // A Compose with a Compose as its first child is not shortened, so that chains are not
    // ambiguous.
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::Compose);
            b.begin_block(Id::Compose);
            b.add_block(Id::SolidColorShader);
            b.add_block(Id::MatrixColorFilter);
            b.end_block();
            b.add_block(Id::GaussianColorFilter);
            b.end_block();
        }),
        "Compose[SolidColor+MatrixColorFilter, GaussianColorFilter] "
    );

    // A compose as the second child is shortened: A+B+C.
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::Compose);
            b.add_block(Id::SolidColorShader);
            b.begin_block(Id::Compose);
            b.add_block(Id::MatrixColorFilter);
            b.add_block(Id::GaussianColorFilter);
            b.end_block();
            b.end_block();
        }),
        "SolidColor+MatrixColorFilter+GaussianColorFilter "
    );

    // Other snippets with children use brackets and commas.
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::BlendCompose);
            b.add_block(Id::SolidColorShader);
            b.add_block(Id::SolidColorShader);
            b.add_block(Id::FixedBlendMultiply);
            b.end_block();
            b.add_root_block_header(RootBlockType::FinalBlend);
            b.add_block(Id::FixedBlendSrcOver);
            b.add_root_block_header(RootBlockType::Clip);
            b.add_block(Id::AnalyticClip);
        }),
        "BlendCompose[SolidColor, SolidColor, Multiply] SrcOver AnalyticClip "
    );

    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::LocalMatrixShader);
            b.add_block(Id::LinearGradientShader4);
            b.end_block();
        }),
        "LocalMatrix[LinearGradient4] "
    );
}

#[test]
fn to_string_shows_embedded_data() {
    let dict = dictionary();
    // (Skia asserts that the data is not the end of the key, which always has a final blend.)
    // No data: "(0)".
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::ImageShader);
            b.add_data(&[]);
            b.end_block();
            b.add_root_block_header(RootBlockType::FinalBlend);
            b.add_block(Id::FixedBlendSrcOver);
        }),
        "Image(0) SrcOver "
    );
    // Data that is not an immutable sampler: base64 of its bytes, with the number of words.
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::ImageShader);
            b.add_data(&[1, 2]);
            b.end_block();
            b.add_root_block_header(RootBlockType::FinalBlend);
            b.add_block(Id::FixedBlendSrcOver);
        }),
        "Image(2: AQAAAAIAAAA=) SrcOver "
    );
    // An external format has three words.
    assert_eq!(
        to_string_of(&dict, |b| {
            b.add_root_block_header(RootBlockType::SrcColor);
            b.begin_block(Id::HWImageShader);
            b.add_data(&[1, 2, 0xFFFF_FFFD]);
            b.end_block();
            b.add_root_block_header(RootBlockType::FinalBlend);
            b.add_block(Id::FixedBlendSrcOver);
        }),
        "HardwareImage(3: AQAAAAIAAAD9////) SrcOver "
    );

    // An immutable sampler is described by the caps, if they can.
    let immutable_desc = 1_u32 << 7; // an immutable sampler info bit
    let build = |b: &mut PaintParamsKeyBuilder| {
        b.add_root_block_header(RootBlockType::SrcColor);
        b.begin_block(Id::ImageShader);
        b.add_data(&[immutable_desc, 2]);
        b.end_block();
        b.add_root_block_header(RootBlockType::FinalBlend);
        b.add_block(Id::FixedBlendSrcOver);
    };
    let caps = MockCaps {
        immutable_sampler_string: String::from("YCbCr 420"),
        ..MockCaps::default()
    };
    assert_eq!(
        to_string_with_caps(&dict, &caps, build),
        "Image(YCbCr 420) SrcOver "
    );
    // And falls back to the bytes if they have nothing to say.
    let described = to_string_with_caps(&dict, &MockCaps::default(), build);
    assert!(described.starts_with("Image(2: "), "{described}");
}

#[test]
fn to_string_shows_unknown_ids() {
    let dict = dictionary();
    let key = [-1, 9999];
    assert_eq!(
        PaintParamsKey::new(&key).to_string(&MockCaps::default(), &dict),
        "Unknown(9999) "
    );
}

#[test]
fn dump_prints_without_panicking() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::ImageShader);
    builder.add_data(&[1, 2]);
    builder.end_block();
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrcOver);
    let id = dict.find_or_create_for_builder(&mut builder);
    dict.dump(&MockCaps::default(), id);
}

//--------------------------------------------------------------------------------------------------
// getRootNodes

fn roots_of(
    dict: &ShaderCodeDictionary,
    caps: &MockCaps,
    rte_dict: &RuntimeEffectDictionary,
    data: &[i32],
    varyings: i32,
    can_lift_coords: bool,
) -> skia_rust_gpu::graphite::paint_params_key::RootNodesInfo {
    PaintParamsKey::new(data).get_root_nodes(caps, dict, rte_dict, varyings, can_lift_coords)
}

#[test]
fn root_nodes_of_a_key() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::LocalMatrixShader);
    builder.begin_block(Id::ImageShader);
    builder.add_data(&[5, 6]);
    builder.end_block();
    builder.end_block();
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrcOver);
    builder.add_root_block_header(RootBlockType::Clip);
    builder.add_block(Id::AnalyticClip);
    let data = key_data(&mut builder);

    let info = roots_of(&dict, &MockCaps::default(), &rte_dict, &data, 0, true);
    assert_eq!(info.roots.len(), 3);
    assert!(info.mesh_shader().is_none());
    assert!(info.mesh_spec.is_none());

    let src = info.src_color().unwrap();
    assert_eq!(src.code_snippet_id(), Id::LocalMatrixShader as i32);
    assert_eq!(src.key_index(), 1);
    assert_eq!(src.num_children(), 1);
    let image = src.child(0);
    assert_eq!(image.code_snippet_id(), Id::ImageShader as i32);
    assert_eq!(image.key_index(), 2);
    // The embedded data is handed to the node.
    assert_eq!(image.data(), &[5, 6]);
    // Local coordinates are required all the way up.
    assert!(
        src.required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
    assert!(
        src.required_flags()
            .contains(SnippetRequirementFlags::STORES_SAMPLER_DESC_DATA)
    );

    let blend = info.final_blend().unwrap();
    assert_eq!(blend.code_snippet_id(), Id::FixedBlendSrcOver as i32);
    assert_eq!(blend.key_index(), 7);
    assert!(blend.required_flags().contains(
        SnippetRequirementFlags::PRIOR_STAGE_OUTPUT | SnippetRequirementFlags::BLENDER_DST_COLOR
    ));

    let clip = info.clip().unwrap();
    assert_eq!(clip.code_snippet_id(), Id::AnalyticClip as i32);
    assert_eq!(clip.key_index(), 9);
    // The roots are in the order of the key.
    let ids: Vec<i32> = info.roots.iter().map(ShaderNode::code_snippet_id).collect();
    assert_eq!(
        ids,
        vec![
            Id::LocalMatrixShader as i32,
            Id::FixedBlendSrcOver as i32,
            Id::AnalyticClip as i32
        ]
    );
}

#[test]
fn bad_keys_have_no_root_nodes() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let caps = MockCaps::default();
    // Without a root block header.
    assert!(
        roots_of(
            &dict,
            &caps,
            &rte_dict,
            &[Id::SolidColorShader as i32],
            0,
            true
        )
        .roots
        .is_empty()
    );
    // With an unknown id.
    assert!(
        roots_of(&dict, &caps, &rte_dict, &[-1, 9999], 0, true)
            .roots
            .is_empty()
    );
    // An empty key.
    assert!(
        roots_of(&dict, &caps, &rte_dict, &[], 0, true)
            .roots
            .is_empty()
    );
}

#[test]
fn coordinate_expressions_are_lifted_while_varyings_last() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let caps = MockCaps::default();
    // LocalMatrix [ LinearGradient4 ]: both need coordinates.
    let key = [
        -1,
        Id::LocalMatrixShader as i32,
        Id::LinearGradientShader4 as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
    ];

    // No varyings: nothing changes.
    let info = roots_of(&dict, &caps, &rte_dict, &key, 0, true);
    let flags = info.src_color().unwrap().required_flags();
    assert!(flags.contains(SnippetRequirementFlags::LOCAL_COORDS));
    assert!(!flags.contains(SnippetRequirementFlags::LIFT_EXPRESSION));
    assert!(!flags.contains(SnippetRequirementFlags::OMIT_EXPRESSION));

    // Coordinates can't be lifted: nothing changes either.
    let info = roots_of(&dict, &caps, &rte_dict, &key, 4, false);
    let flags = info.src_color().unwrap().required_flags();
    assert!(flags.contains(SnippetRequirementFlags::LOCAL_COORDS));
    assert!(!flags.contains(SnippetRequirementFlags::LIFT_EXPRESSION));

    // One varying: the local matrix is lifted, because its child needs the transformed
    // coordinates in the fragment shader. It no longer needs coordinates itself.
    let info = roots_of(&dict, &caps, &rte_dict, &key, 1, true);
    let src = info.src_color().unwrap();
    assert!(
        src.required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    assert!(
        !src.required_flags()
            .contains(SnippetRequirementFlags::OMIT_EXPRESSION)
    );
    assert!(
        !src.required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
    // The child keeps what it needs.
    assert!(
        src.child(0)
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
    assert!(
        !src.child(0)
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
}

#[test]
fn nested_coordinate_expressions_can_be_omitted() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let caps = MockCaps::default();
    // LocalMatrix [ LocalMatrix [ LinearGradient4 ] ]
    let key = [
        -1,
        Id::LocalMatrixShader as i32,
        Id::LocalMatrixShader as i32,
        Id::LinearGradientShader4 as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
    ];
    let info = roots_of(&dict, &caps, &rte_dict, &key, 2, true);
    let outer = info.src_color().unwrap();
    let inner = outer.child(0);
    // The inner matrix is lifted and the gradient uses its varying, so the outer one is only
    // used in the vertex shader, to compute the inner one.
    assert!(
        inner
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    assert!(
        !inner
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
    assert!(
        outer
            .required_flags()
            .contains(SnippetRequirementFlags::OMIT_EXPRESSION)
    );
    assert!(
        !outer
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    assert!(
        !outer
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );

    // With one varying only the outer one is lifted, and the inner one must stay.
    let info = roots_of(&dict, &caps, &rte_dict, &key, 1, true);
    let outer = info.src_color().unwrap();
    assert!(
        outer
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    assert!(
        !outer
            .child(0)
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    assert!(
        outer
            .child(0)
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
}

#[test]
fn compose_passes_the_coordinates_through() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let caps = MockCaps::default();
    // Compose [ LocalMatrix [ LinearGradient4 ], MatrixColorFilter ]: the compose passes its
    // coordinates through, so the matrix below it is lifted.
    let key = [
        -1,
        Id::Compose as i32,
        Id::LocalMatrixShader as i32,
        Id::LinearGradientShader4 as i32,
        Id::MatrixColorFilter as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
    ];
    let info = roots_of(&dict, &caps, &rte_dict, &key, 1, true);
    let compose = info.src_color().unwrap();
    let matrix = compose.child(0);
    assert!(
        matrix
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    // Nothing under the compose needs coordinates from outside any more.
    assert!(
        !compose
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
}

#[test]
fn solid_colors_are_lifted_with_a_storage_buffer() {
    let dict = dictionary();
    let rte_dict = RuntimeEffectDictionary::new();
    let key = [
        -1,
        Id::SolidColorShader as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
    ];

    let uniform_buffers = MockCaps::default();
    let info = roots_of(&dict, &uniform_buffers, &rte_dict, &key, 2, true);
    assert!(
        !info
            .src_color()
            .unwrap()
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );

    let storage = MockCaps {
        storage_buffer_support: true,
        ..MockCaps::default()
    };
    let info = roots_of(&dict, &storage, &rte_dict, &key, 2, true);
    assert!(
        info.src_color()
            .unwrap()
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    // The blend has nothing to lift.
    assert!(
        !info
            .final_blend()
            .unwrap()
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
    // No varyings, no lifting.
    let info = roots_of(&dict, &storage, &rte_dict, &key, 0, true);
    assert!(
        !info
            .src_color()
            .unwrap()
            .required_flags()
            .contains(SnippetRequirementFlags::LIFT_EXPRESSION)
    );
}

#[test]
fn a_mesh_shader_root_needs_its_specification() {
    use skia_rust_core::mesh::{Attribute, AttributeType, MeshSpecification};

    let dict = dictionary();
    let attributes = [Attribute {
        ty: AttributeType::Float2,
        offset: 0,
        name: String::from("pos"),
    }];
    let result = MeshSpecification::make(
        &attributes,
        8,
        &[],
        "Varyings main(const Attributes a) { Varyings v; v.position = a.pos; return v; }",
        "float2 main(const Varyings v) { return v.position; }",
    );
    let spec = result.specification.expect(&result.error);
    let mesh_id = dict.find_or_create_mesh_snippet(&spec);

    let key = [
        -1,
        Id::SolidColorShader as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
        RootBlockType::MeshShader as i32,
        mesh_id,
    ];
    let caps = MockCaps::default();
    let rte_dict = RuntimeEffectDictionary::new();
    // The dictionary of the recording doesn't know the specification: the key is bad.
    assert!(
        roots_of(&dict, &caps, &rte_dict, &key, 0, true)
            .roots
            .is_empty()
    );

    rte_dict.set_mesh_spec(mesh_id, Arc::clone(&spec));
    let info = roots_of(&dict, &caps, &rte_dict, &key, 0, true);
    assert_eq!(info.roots.len(), 3);
    assert_eq!(info.mesh_shader().unwrap().code_snippet_id(), mesh_id);
    assert!(Arc::ptr_eq(info.mesh_spec.as_ref().unwrap(), &spec));
}

//--------------------------------------------------------------------------------------------------
// isSerializable

#[test]
fn serializable_keys() {
    let dict = dictionary();
    let mut builder = PaintParamsKeyBuilder::new(&dict);
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.begin_block(Id::LocalMatrixShader);
    builder.begin_block(Id::ImageShader);
    builder.add_data(&[1, 2]);
    builder.end_block();
    builder.end_block();
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(Id::FixedBlendSrcOver);
    let good = key_data(&mut builder);
    assert!(PaintParamsKey::new(&good).is_serializable(&dict));
    // The empty key has no malformed part.
    assert!(PaintParamsKey::invalid().is_serializable(&dict));

    // The root blocks need their headers.
    assert!(!PaintParamsKey::new(&[Id::SolidColorShader as i32]).is_serializable(&dict));
    // Unknown ids, and ids that are not built in or Skia known runtime effects.
    assert!(!PaintParamsKey::new(&[-1, 9999]).is_serializable(&dict));
    assert!(!PaintParamsKey::new(&[-1, 1500]).is_serializable(&dict));
    assert!(!PaintParamsKey::new(&[-1, -5]).is_serializable(&dict));
    // A truncated tree.
    assert!(!PaintParamsKey::new(&good[..5]).is_serializable(&dict));
    // (Cutting after the first root leaves a key with just that root.)
    assert!(PaintParamsKey::new(&good[..6]).is_serializable(&dict));
    assert!(
        !PaintParamsKey::new(&[-1, Id::Compose as i32, Id::SolidColorShader as i32])
            .is_serializable(&dict)
    );
    // Data that is cut off, is not negative, or is too long.
    let image = Id::ImageShader as i32;
    assert!(!PaintParamsKey::new(&[-1, image]).is_serializable(&dict));
    assert!(
        !PaintParamsKey::new(&[-1, image, PaintParamsKey::encode_data_size(2), 1])
            .is_serializable(&dict)
    );
    assert!(!PaintParamsKey::new(&[-1, image, 2, 1, 2]).is_serializable(&dict));
    assert!(!PaintParamsKey::new(&[-1, image, i32::MIN, 1, 2]).is_serializable(&dict));
    let too_long = PaintParamsKey::encode_data_size(PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT + 1);
    assert!(!PaintParamsKey::new(&[-1, image, too_long]).is_serializable(&dict));
    let at_limit = PaintParamsKey::encode_data_size(PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT);
    let mut limit_key = vec![-1, image, at_limit];
    limit_key.extend(std::iter::repeat_n(0, 16));
    assert!(PaintParamsKey::new(&limit_key).is_serializable(&dict));
}

#[test]
fn skia_known_runtime_effects_are_serializable_and_others_are_not() {
    use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
    use skia_rust_core::runtime_effect::RuntimeEffect;

    let dict = dictionary();
    // A Skia known runtime effect has its stable id in the key (it takes no children).
    let blend = get_known_runtime_effect(StableKey::Blend).unwrap();
    let id = dict.find_or_create_runtime_effect_snippet(blend).unwrap();
    let key = [-1, id, -2, Id::FixedBlendSrcOver as i32];
    let children = dict.get_entry(id).unwrap().num_children;
    assert!(children > 0);
    // `$Blend` samples its children, so build the key it needs.
    let mut key_with_children = vec![-1, id];
    key_with_children.extend(std::iter::repeat_n(
        Id::SolidColorShader as i32,
        usize::try_from(children).unwrap(),
    ));
    key_with_children.extend([-2, Id::FixedBlendSrcOver as i32]);
    assert!(PaintParamsKey::new(&key_with_children).is_serializable(&dict));
    assert!(!PaintParamsKey::new(&key).is_serializable(&dict));

    // An effect that only this dictionary knows is not serializable.
    let effect = RuntimeEffect::make_for_shader("half4 main(float2 p) { return half4(1); }", None)
        .expect("compiles");
    let unknown = dict.find_or_create_runtime_effect_snippet(&effect).unwrap();
    assert!(
        !PaintParamsKey::new(&[-1, unknown, -2, Id::FixedBlendSrcOver as i32])
            .is_serializable(&dict)
    );
}

#[test]
fn key_hash_depends_on_the_data() {
    let a = [
        -1,
        Id::SolidColorShader as i32,
        -2,
        Id::FixedBlendSrcOver as i32,
    ];
    let b = [
        -1,
        Id::SolidColorShader as i32,
        -2,
        Id::FixedBlendSrc as i32,
    ];
    assert_eq!(
        PaintParamsKey::new(&a).hash(),
        PaintParamsKey::new(&a).hash()
    );
    assert_ne!(
        PaintParamsKey::new(&a).hash(),
        PaintParamsKey::new(&b).hash()
    );
    assert_eq!(PaintParamsKey::new(&a), PaintParamsKey::new(&a.clone()));
    assert_ne!(PaintParamsKey::new(&a), PaintParamsKey::new(&b));
    assert_eq!(&*PaintParamsKey::new(&a).clone_data(), &a);
}
