// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/KeyTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which names its keys `keyA`, `keyB` and `keyC`.
#![allow(clippy::similar_names)]

use skia_rust_core::blend_mode::{BlendMode, BlendModeCoeff};
use skia_rust_core::color::PMColor4f;
use skia_rust_gpu::gpu::blend::{BlendCoeff, BlendEquation, blend_modifies_dst};
use skia_rust_gpu::gpu::swizzle::Swizzle;
use skia_rust_gpu::graphite::built_in_code_snippet_id::{
    BuiltInCodeSnippetID, FIXED_BLEND_ID_OFFSET,
};
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::context_utils::can_use_hardware_blending;
use skia_rust_gpu::graphite::graphite_types::SampleCount;
use skia_rust_gpu::graphite::paint_params_key::{
    PaintParamsKey, PaintParamsKeyBuilder, RootBlockType,
};
use skia_rust_gpu::graphite::render_pass_desc::RenderPassDesc;
use skia_rust_gpu::graphite::resource_types::DstReadStrategy;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::shader_info::ShaderInfo;
use skia_rust_gpu::graphite::texture_format::{TextureFormat, texture_format_auto_clamps};
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;

use crate::tools::graphite_test_context::renderer_provider;
use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// Port of: tests/graphite/KeyTest.cpp#L26-L29 (chrome/m156)
fn add_block(builder: &mut PaintParamsKeyBuilder, snippet_id: i32) {
    builder.begin_block(snippet_id.cast_unsigned());
    builder.end_block();
}

// Port of: tests/graphite/KeyTest.cpp#L31-L37 (chrome/m156)
// (The C++ clones the key into an arena; the port returns the key's data, which a
// `PaintParamsKey` views.)
fn create_key(dict: &ShaderCodeDictionary, snippet_id: i32) -> Vec<i32> {
    let mut builder = PaintParamsKeyBuilder::new(dict);
    add_block(&mut builder, snippet_id);

    let key_view = builder.lock_as_key();
    key_view.key().data().to_vec()
}

// Port of: tests/graphite/KeyTest.cpp#L39-L53 (chrome/m156)
fn coeff_equal(sk_coeff: BlendModeCoeff, gpu_coeff: BlendCoeff) -> bool {
    match sk_coeff {
        BlendModeCoeff::Zero => BlendCoeff::Zero == gpu_coeff,
        BlendModeCoeff::One => BlendCoeff::One == gpu_coeff,
        BlendModeCoeff::SC => BlendCoeff::SC == gpu_coeff,
        BlendModeCoeff::ISC => BlendCoeff::ISC == gpu_coeff,
        BlendModeCoeff::DC => BlendCoeff::DC == gpu_coeff,
        BlendModeCoeff::IDC => BlendCoeff::IDC == gpu_coeff,
        BlendModeCoeff::SA => BlendCoeff::SA == gpu_coeff,
        BlendModeCoeff::ISA => BlendCoeff::ISA == gpu_coeff,
        BlendModeCoeff::DA => BlendCoeff::DA == gpu_coeff,
        BlendModeCoeff::IDA => BlendCoeff::IDA == gpu_coeff,
    }
}

// These are intended to be unit tests of the PaintParamsKeyBuilder and PaintParamsKey.
// Port of: tests/graphite/KeyTest.cpp#L59-L89 (chrome/m156)
def_graphite_test_for_all_contexts!(KeyWithInvalidCodeSnippetIDTest, |reporter, context| {
    let dict = context.shared_context().shader_code_dictionary();
    let caps = ContextPriv::caps(context);

    // A builder without any data is invalid. The Builder and the PaintParamKeys can include
    // invalid IDs without themselves becoming invalid. Normally adding an invalid ID triggers an
    // assert in debug builds, since the properly functioning key system should never encounter an
    // invalid ID.
    let mut builder = PaintParamsKeyBuilder::new(dict);
    let key_view = builder.lock_as_key();
    reporter_assert!(reporter, !key_view.key().is_valid());
    reporter_assert!(reporter, !PaintParamsKey::invalid().is_valid());
    drop(key_view);

    // However, if the program gets in a malformed state on release builds, the key
    // could contain an invalid ID. In that case the invalid snippet IDs are detected when
    // reconstructing the key into an effect tree for SkSL generation. To test this, we manually
    // construct an invalid span and test that it returns a null shader node tree when treated as
    // a PaintParamsKey.
    // NOTE: The C++ intentionally abuses memory here: the span it builds claims
    // `sizeof(invalidKeyData)` elements, not `std::size(invalidKeyData)`. The first element is
    // not a root block marker, so the key is rejected before anything past it is read; the port
    // builds the span over the three ids.
    let invalid_key_data = [
        BuiltInCodeSnippetID::SolidColorShader as i32,
        skia_rust_core::known_runtime_effects::SKIA_BUILT_IN_RESERVED_CNT.cast_signed() - 1,
        BuiltInCodeSnippetID::FixedBlendSrc as i32,
    ];
    let fake_key = PaintParamsKey::new(&invalid_key_data);
    let rte_dict = RuntimeEffectDictionary::default();
    reporter_assert!(
        reporter,
        fake_key
            .get_root_nodes(caps, dict, &rte_dict, 0, /* can_lift_coords= */ true)
            .roots
            .is_empty()
    );
});

// Port of: tests/graphite/KeyTest.cpp#L91-L105 (chrome/m156)
def_graphite_test_for_all_contexts!(KeyEqualityChecksSnippetID, |reporter, context| {
    let dict = context.shared_context().shader_code_dictionary();

    let key_a_data = create_key(dict, BuiltInCodeSnippetID::SolidColorShader as i32);
    let key_b_data = create_key(dict, BuiltInCodeSnippetID::SolidColorShader as i32);
    let key_c_data = create_key(dict, BuiltInCodeSnippetID::RGBPaintColor as i32);
    let key_a = PaintParamsKey::new(&key_a_data);
    let key_b = PaintParamsKey::new(&key_b_data);
    let key_c = PaintParamsKey::new(&key_c_data);

    // Verify that keyA matches keyB, and that it does not match keyC.
    reporter_assert!(reporter, key_a == key_b);
    reporter_assert!(reporter, key_a != key_c);
    reporter_assert!(reporter, !(key_a == key_c));
    reporter_assert!(reporter, !(key_a != key_b));
});

// Port of: tests/graphite/KeyTest.cpp#L107-L172 (chrome/m156)
def_graphite_test_for_all_contexts!(ShaderInfoDetectsFixedFunctionBlend, |reporter, context| {
    let dict = context.shared_context().shader_code_dictionary();
    let caps = ContextPriv::caps(context);
    let renderer_provider = renderer_provider(context);

    // Iterate over all coeff modes, plus one extra iteration to handle the manually clamped kPlus
    for bm in 0..=(BlendMode::LAST_COEFF_MODE as i32 + 1) {
        let mode = if bm > BlendMode::LAST_COEFF_MODE as i32 {
            BlendMode::Plus
        } else {
            BlendMode::from_i32(bm).expect("a blend mode")
        };
        let format = if bm > BlendMode::LAST_COEFF_MODE as i32 {
            TextureFormat::RGBA16F
        } else {
            TextureFormat::RGBA8
        };

        let mut builder = PaintParamsKeyBuilder::new(dict);
        // Use a solid color as the 1st root node; the 2nd root node represents the final blend.
        builder.add_root_block_header(RootBlockType::SrcColor);
        add_block(&mut builder, BuiltInCodeSnippetID::SolidColorShader as i32);
        builder.add_root_block_header(RootBlockType::FinalBlend);
        add_block(&mut builder, mode as i32 + FIXED_BLEND_ID_OFFSET);
        let paint_id: UniquePaintParamsID = dict.find_or_create_for_builder(&mut builder);

        let render_step = renderer_provider.non_aa_bounds_fill().step(0);
        let dst_read_required =
            !can_use_hardware_blending(caps, format, mode, render_step.coverage());

        let mut rp_desc = RenderPassDesc::default();
        rp_desc.color_attachment.format = format;
        rp_desc.write_swizzle = Swizzle::rgba();
        rp_desc.sample_count = SampleCount::One;
        rp_desc.dst_read_strategy = if dst_read_required {
            caps.get_dst_read_strategy()
        } else {
            DstReadStrategy::NoneRequired
        };

        // ShaderInfo expects to receive a concrete determination of dstReadStrategy based upon
        // whether a dst read is needed. Therefore, we need to decide whether to pass in the
        // dstReadStrategy reported by caps OR DstReadStrategy::kNoneRequired.
        let shader_info = ShaderInfo::make(
            caps,
            dict,
            /* rte_dict= */ None,
            &rp_desc,
            render_step,
            paint_id,
            None,
        );

        let mut expected_bm = mode;
        if expected_bm == BlendMode::Plus
            && (!texture_format_auto_clamps(format)
                || caps.get_dst_read_strategy() != DstReadStrategy::TextureCopy)
        {
            // The kPlus "coefficient" blend mode in non-clamping render targets triggers shader
            // blending to add a clamping. HW kPlus blending is an approximation when there's
            // coverage, so shader blending is used when no dst copy is required.
            // Shader-based blending always uses kSrc HW blending.
            expected_bm = BlendMode::Src;
        }
        let coeffs = expected_bm.as_coeff();
        reporter_assert!(reporter, coeffs.is_some());
        let (expected_src, expected_dst) = coeffs.expect("a coefficient blend mode");
        reporter_assert!(
            reporter,
            coeff_equal(expected_src, shader_info.blend_info().src_blend)
        );
        reporter_assert!(
            reporter,
            coeff_equal(expected_dst, shader_info.blend_info().dst_blend)
        );

        reporter_assert!(
            reporter,
            shader_info.blend_info().equation == BlendEquation::Add
        );
        reporter_assert!(
            reporter,
            shader_info.blend_info().blend_constant == PMColor4f::default()
        );

        let expected_write_color = blend_modifies_dst(
            BlendEquation::Add,
            shader_info.blend_info().src_blend,
            shader_info.blend_info().dst_blend,
        );
        reporter_assert!(
            reporter,
            shader_info.blend_info().writes_color == expected_write_color
        );
    }
});

// TODO: Add unit tests for converting a complex key to a ShaderInfo
