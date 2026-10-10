// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::known_runtime_effects::USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::shader::Shader;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::gpu::resource_key::UniqueKey;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::draw_types::DrawTypeFlags;
use skia_rust_gpu::graphite::graphite_types::{
    DepthStencilFlags, InsertRecordingInfo, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::precompile::paint_options::PaintOptions;
use skia_rust_gpu::graphite::precompile_context::PrecompileContext;
use skia_rust_gpu::graphite::public_precompile::{RenderPassProperties, precompile};
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderOptions};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::tools::graphite_test_context::real_context_with_options;
use crate::tools::pipeline_callback_handler::PipelineCallBackHandler;
use crate::tools::precompile_effect_factories::{
    create_annulus_runtime_shader, create_combo_runtime_blender, create_combo_runtime_color_filter,
    get_annulus_shader_code, get_annulus_shader_effect, get_combo_blender_effect,
    get_combo_color_filter_effect, get_double_color_filter_effect, get_dst_blender_effect,
    get_half_color_filter_effect, get_src_blender_effect,
};
use crate::tools::unique_key_utils::fetch_unique_keys;
use crate::{Reporter, reporter_assert};

// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L43-L58 (chrome/m156), the
// anonymous `create_paint_and_options`
fn create_paint_and_options(add_blenders: bool) -> (Paint, PaintOptions) {
    let mut paint = Paint::default();
    let mut paint_options = PaintOptions::default();

    let (shader, shader_option) = create_annulus_runtime_shader();
    paint.set_shader(shader);
    paint_options.set_shaders(&[shader_option]);

    let (color_filter, color_filter_option) = create_combo_runtime_color_filter();
    paint.set_color_filter(color_filter);
    paint_options.set_color_filters(&[Some(color_filter_option)]);

    if add_blenders {
        let (blender, blender_option) = create_combo_runtime_blender();
        paint.set_blender(blender);
        paint_options.set_blenders(&[blender_option]);
    }

    (paint, paint_options)
}

// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L60-L80 (chrome/m156), the
// anonymous `draw_with_normal_api`
fn draw_with_normal_api(context: &mut WgpuContext, recorder: &mut Recorder, paint: &Paint) -> bool {
    let ii = ImageInfo::new((256, 256), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(surface) = Surface::render_target(recorder, &ii, Mipmapped::No, None, "") else {
        return false;
    };
    let canvas = surface.canvas();
    canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), paint);

    let Some(mut recording) = recorder.snap() else {
        return false;
    };
    if context.insert_recording(InsertRecordingInfo::new(&mut recording))
        != skia_rust_gpu::graphite::graphite_types::InsertStatus::Success
    {
        return false;
    }
    context.submit(SubmitInfo::new(SyncToCpu::Yes))
}

// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L82-L97 (chrome/m156), the
// anonymous `fetch_keys_and_reset`
fn fetch_keys_and_reset(
    reporter: &mut Reporter,
    handler: &PipelineCallBackHandler,
    precompile_context: &PrecompileContext,
    reset: bool,
) -> (Vec<UniqueKey>, Vec<Data>) {
    let unique_keys = fetch_unique_keys(precompile_context);
    let serialized_keys = handler.retrieve_keys();
    if reset {
        let global_cache = precompile_context
            .shared_context()
            .shared_context()
            .global_cache();
        global_cache.reset_graphics_pipelines();
        reporter_assert!(reporter, global_cache.num_graphics_pipelines() == 0);
        handler.reset();
    }
    (unique_keys, serialized_keys)
}

/// `shaderCodeDictionary()->numUserDefinedRuntimeEffects()` of the context.
fn num_user_defined_runtime_effects(context: &WgpuContext) -> usize {
    context
        .shared_context()
        .shader_code_dictionary()
        .num_user_defined_runtime_effects()
}

// Get the existing keys, reset, and try recreating them all w/ the serialized pipeline keys
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L99-L144 (chrome/m156), the
// anonymous `reset_and_recreate_pipelines_with_serialized_keys`
fn reset_and_recreate_pipelines_with_serialized_keys(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    recorder: &mut Recorder,
    precompile_context: &PrecompileContext,
    handler: &PipelineCallBackHandler,
) {
    let global_cache = precompile_context
        .shared_context()
        .shared_context()
        .global_cache();
    let (paint, _) = create_paint_and_options(/* add_blenders= */ true);
    draw_with_normal_api(context, recorder, &paint);

    // None of the user-defined stable runtime effects should've been transmuted to not-stable
    reporter_assert!(reporter, num_user_defined_runtime_effects(context) == 0);

    let (orig_keys, android_style_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    // Given 'draw_with_normal_api' we expect one serialized key - full of user-defined stable keys
    reporter_assert!(reporter, orig_keys.len() == 1);
    reporter_assert!(reporter, android_style_keys.len() == 1);

    // Use the serialized keys to regenerate the Pipelines
    for d in &android_style_keys {
        let result = precompile_context.precompile(Some(d));
        assert!(result, "the serialized key precompiles");
    }

    // We need to explicitly wait for the precompilation to finish here
    context
        .shared_context()
        .base()
        .pipeline_manager()
        .wait_test_only();

    // None of the user-defined stable runtime effects should've been transmuted to not-stable
    reporter_assert!(reporter, num_user_defined_runtime_effects(context) == 0);

    let (recreated_keys, recreated_android_style_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ false,
    );

    reporter_assert!(reporter, recreated_keys.len() == 1);
    reporter_assert!(reporter, orig_keys[0] == recreated_keys[0]);
    reporter_assert!(reporter, recreated_android_style_keys.len() == 1);
    reporter_assert!(
        reporter,
        android_style_keys[0] == recreated_android_style_keys[0]
    );

    let num_before_second_draw = global_cache.num_graphics_pipelines();
    draw_with_normal_api(context, recorder, &paint);

    // None of the user-defined stable runtime effects should've been transmuted to not-stable
    reporter_assert!(reporter, num_user_defined_runtime_effects(context) == 0);

    // Re-drawing shouldn't create any new pipelines
    reporter_assert!(
        reporter,
        num_before_second_draw == global_cache.num_graphics_pipelines(),
        "{} != {}",
        num_before_second_draw,
        global_cache.num_graphics_pipelines()
    );
}

// Get the existing keys, reset, and then try recreating them all using the normal precompile API
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L146-L213 (chrome/m156), the
// anonymous `reset_and_recreate_pipelines_with_normal_precompile_api`
fn reset_and_recreate_pipelines_with_normal_precompile_api(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    recorder: &mut Recorder,
    precompile_context: &PrecompileContext,
    handler: &PipelineCallBackHandler,
) {
    // We don't attach runtime blenders to the SkPaint and PaintOptions in this case bc that will
    // force a dest read and complicate the normal-pipeline/Precompile-pipeline
    // comparison on Native Mac and Vulkan. This is bc, for those platforms, Precompile skips some
    // LoadOp combinations which don't matter for those platforms (please see 'numLoadOps' in
    // Precompile) but are serialized (for simplicity) in the serialized pipeline keys.
    let (paint, paint_options) = create_paint_and_options(/* add_blenders= */ false);
    draw_with_normal_api(context, recorder, &paint);

    let (orig_keys, android_style_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    // Given 'draw_with_normal_api' we expect one serialized key - full of user-defined stable keys
    reporter_assert!(reporter, orig_keys.len() == 1);
    reporter_assert!(reporter, android_style_keys.len() == 1);

    // gGraphiteAvoidDepth is off (the default), so the render pass has a depth attachment.
    let render_pass_props = RenderPassProperties {
        ds_flags: DepthStencilFlags::Depth,
        ..RenderPassProperties::default()
    };
    precompile(
        precompile_context,
        &paint_options,
        DrawTypeFlags::SIMPLE_SHAPE,
        std::slice::from_ref(&render_pass_props),
    );

    // We need to explicitly wait for the precompilation to finish here
    context
        .shared_context()
        .base()
        .pipeline_manager()
        .wait_test_only();

    let (recreated_keys, recreated_android_style_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    // The normal precompile API will overgenerate, so we need to search for a match
    reporter_assert!(reporter, recreated_keys.iter().any(|k| *k == orig_keys[0]));
    reporter_assert!(
        reporter,
        recreated_android_style_keys
            .iter()
            .any(|k| *k == android_style_keys[0])
    );
}

// This helper creates a defective user-defined known runtime effect pair:
//     the blessed shader will have a user-defined stable key
//     the cursed shader has the same SkSL as the blessed one but no stable key
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L215-L226 (chrome/m156), the
// anonymous `make_defective_annulus_shader_pair`
fn make_defective_annulus_shader_pair() -> (Shader, Shader) {
    let blessed = get_annulus_shader_effect();
    let cursed = RuntimeEffect::make_for_shader(get_annulus_shader_code(), None)
        .expect("the annulus shader compiles");
    let uniforms = {
        let bytes: Vec<u8> = [50.0_f32, 50.0, 40.0, 50.0]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        Data::new_copy(&bytes)
    };
    let blessed_shader = blessed
        .make_shader(uniforms.clone(), &[], None)
        .expect("the blessed shader makes a shader");
    let cursed_shader = cursed
        .make_shader(uniforms, &[], None)
        .expect("the cursed shader makes a shader");
    (blessed_shader, cursed_shader)
}

// Draw once with a registered runtime effect, reset, and then re-draw w/ an un-registered
// runtime effect that uses the same SkSL.
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L228-L264 (chrome/m156), the
// anonymous `test_sksl_reuse`
fn test_sksl_reuse(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    recorder: &mut Recorder,
    precompile_context: &PrecompileContext,
    handler: &PipelineCallBackHandler,
) {
    let (blessed_shader, cursed_shader) = make_defective_annulus_shader_pair();

    // The blessed paint is the static one from the PrecompileFactories which has been
    // registered as a user-defined known runtime effect.
    let mut blessed_paint = Paint::default();
    blessed_paint.set_shader(blessed_shader);

    // The cursed paint uses the same SkSL as the blessed version but uses a wholly separate
    // runtime effect (which is not registered).
    let mut cursed_paint = Paint::default();
    cursed_paint.set_shader(cursed_shader);

    draw_with_normal_api(context, recorder, &blessed_paint);

    let (orig_keys, serialized_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    // Given 'draw_with_normal_api' we expect one serialized key - full of user-defined stable keys
    reporter_assert!(reporter, orig_keys.len() == 1);
    reporter_assert!(reporter, serialized_keys.len() == 1);

    draw_with_normal_api(context, recorder, &cursed_paint);

    let (recreated_keys, recreated_serialized_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    reporter_assert!(reporter, recreated_keys.len() == 1);
    reporter_assert!(reporter, orig_keys[0] == recreated_keys[0]);

    // The un-registered runtime effect should've been mapped back to the registered one
    // and successfully serialized.
    reporter_assert!(reporter, recreated_serialized_keys.len() == 1);
    reporter_assert!(reporter, serialized_keys[0] == recreated_serialized_keys[0]);
}

// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L266-L300 (chrome/m156), the
// anonymous `test_get_pipeline_label_api`
fn test_get_pipeline_label_api(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    recorder: &mut Recorder,
    precompile_context: &PrecompileContext,
    handler: &PipelineCallBackHandler,
) {
    let (paint, _) = create_paint_and_options(/* add_blenders= */ true);
    draw_with_normal_api(context, recorder, &paint);

    let (orig_keys, android_style_keys) = fetch_keys_and_reset(
        reporter,
        handler,
        precompile_context,
        /* reset= */ true,
    );

    // Given 'draw_with_normal_api' we expect one serialized key - full of user-defined stable keys
    reporter_assert!(reporter, orig_keys.len() == 1);
    reporter_assert!(reporter, android_style_keys.len() == 1);

    let label = precompile_context.get_pipeline_label(android_style_keys.first(), None);
    reporter_assert!(reporter, label.contains("AnnulusShader"));
    reporter_assert!(reporter, label.contains("SrcBlender"));
    reporter_assert!(reporter, label.contains("DstBlender"));
    reporter_assert!(reporter, label.contains("ComboBlender"));
    reporter_assert!(reporter, label.contains("DoubleColorFilter"));
    reporter_assert!(reporter, label.contains("ComboColorFilter"));
    // We withheld the HalfColorFilter name to test the default name case
    reporter_assert!(reporter, !label.contains("HalfColorFilter"));
    reporter_assert!(reporter, label.contains("UserDefinedKnownRuntimeEffect"));
}

/// Runs `body` on a context made with `options` (`ContextFactory workaroundFactory(newOptions)`),
/// on a rendering adapter. Without one the test says so and does nothing, as the harness does.
fn with_context(
    reporter: &mut Reporter,
    options: &ContextOptions,
    body: impl FnOnce(&mut Reporter, &mut WgpuContext),
) {
    if let Some((context_name, mut context)) = real_context_with_options(options) {
        reporter.set_context(Some(context_name));
        body(reporter, &mut context);
        reporter.set_context(None);
    }
}

/// `SkRuntimeEffectPriv::ResetStableKey` over the effects, as the test does before it registers
/// them.
fn reset_stable_keys(effects: &[RuntimeEffect]) {
    for e in effects {
        runtime_effect_priv::set_stable_key(e, 0);
    }
}

// This test adds some user-defined stably-keyed runtime effects and then verifies that
// everything behaves as expected. For the purposes of this test, "behaves as expected" means:
//    1) the user-defined stably keyed runtime effects appear as such in the ShaderCodeDictionary
//    2) the user-defined stable keys appear in the serialized Pipeline
//       keys (i.e., from the PipelineCallBackHandler)
//    3) said keys correctly (re)generate the desired pipelines
//    4) the normal (non-serialized-key) Precompile API generates the same keys.
//    5) if the blessed stable runtime-effects aren't used, the free-range runtime-effects are
//       mapped back to the stable runtime-effects (clients really shouldn't do this though!)
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L357-L447 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn UserDefinedStableKeyTest() {
    // The number of user-defined stable keys (the PrecompileFactories effects).
    const NUM_USER_DEFINED_STABLE_KEYS: usize = 7;
    let mut reporter = Reporter::new("UserDefinedStableKeyTest");
    let handler = PipelineCallBackHandler::new();
    let mut new_options = ContextOptions {
        pipeline_caching_callback: Some(handler.caching_callback()),
        ..ContextOptions::default()
    };

    // We're going to also use all these runtime effects via the normal API
    // (c.f. create_paint_and_options)
    let user_defined_known_runtime_effects = [
        get_annulus_shader_effect(),
        get_src_blender_effect(),
        get_dst_blender_effect(),
        get_combo_blender_effect(),
        get_double_color_filter_effect(),
        get_half_color_filter_effect(),
        get_combo_color_filter_effect(),
    ];
    // The PrecompileFactories runtime effects are static so prior runs may have already
    // set their StableKeys. Reset the StableKeys so all our expectations/asserts will be met.
    reset_stable_keys(&user_defined_known_runtime_effects);
    new_options.user_defined_known_runtime_effects = user_defined_known_runtime_effects
        .iter()
        .cloned()
        .map(Some)
        .collect();

    with_context(&mut reporter, &new_options, |reporter, context| {
        let precompile_context = context.make_precompile_context();
        let global_cache = precompile_context
            .shared_context()
            .shared_context()
            .global_cache();
        let shader_code_dictionary = context.shared_context().shader_code_dictionary();
        let mut recorder = context.make_recorder(Some(&RecorderOptions::default()));

        reporter_assert!(reporter, global_cache.num_graphics_pipelines() == 0);

        // The next two lines check #1 above
        reporter_assert!(reporter, num_user_defined_runtime_effects(context) == 0);
        reporter_assert!(
            reporter,
            shader_code_dictionary.num_user_defined_known_runtime_effects()
                == NUM_USER_DEFINED_STABLE_KEYS
        );

        // This verifies #2 and #3 above
        reset_and_recreate_pipelines_with_serialized_keys(
            reporter,
            context,
            &mut recorder,
            &precompile_context,
            &handler,
        );
        global_cache.reset_graphics_pipelines();
        handler.reset();

        // This tests out #4 above
        reset_and_recreate_pipelines_with_normal_precompile_api(
            reporter,
            context,
            &mut recorder,
            &precompile_context,
            &handler,
        );
        global_cache.reset_graphics_pipelines();
        handler.reset();

        // This tests out #5 above
        test_sksl_reuse(
            reporter,
            context,
            &mut recorder,
            &precompile_context,
            &handler,
        );
        global_cache.reset_graphics_pipelines();
        handler.reset();

        // Extra little test to check on the getPipelineLabel API
        test_get_pipeline_label_api(
            reporter,
            context,
            &mut recorder,
            &precompile_context,
            &handler,
        );
    });

    reporter.finish();
}

// Test that the ShaderCodeDictionary can deduplicate the user-defined known runtime effect list
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L449-L487 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn UserDefinedStableKeyTest_Duplicates() {
    let mut reporter = Reporter::new("UserDefinedStableKeyTest_Duplicates");
    let handler = PipelineCallBackHandler::new();
    let mut new_options = ContextOptions {
        pipeline_caching_callback: Some(handler.caching_callback()),
        ..ContextOptions::default()
    };

    let user_defined_known_runtime_effects =
        [get_annulus_shader_effect(), get_annulus_shader_effect()];
    // The PrecompileFactories runtime effects are static so prior runs may have already
    // set their StableKeys. Reset the StableKeys so all our expectations/asserts will be met.
    reset_stable_keys(&user_defined_known_runtime_effects);
    new_options.user_defined_known_runtime_effects = user_defined_known_runtime_effects
        .iter()
        .cloned()
        .map(Some)
        .collect();

    with_context(&mut reporter, &new_options, |reporter, context| {
        let shader_code_dictionary = context.shared_context().shader_code_dictionary();
        reporter_assert!(
            reporter,
            shader_code_dictionary.num_user_defined_known_runtime_effects() == 1
        );
    });

    reporter.finish();
}

// Test that the ShaderCodeDictionary can handle nullptrs in the
// user-defined known runtime effect list
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L489-L528 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn UserDefinedStableKeyTest_Nullptrs() {
    let mut reporter = Reporter::new("UserDefinedStableKeyTest_Nullptrs");
    let handler = PipelineCallBackHandler::new();
    let mut new_options = ContextOptions {
        pipeline_caching_callback: Some(handler.caching_callback()),
        ..ContextOptions::default()
    };

    let annulus = get_annulus_shader_effect();
    let src = get_src_blender_effect();
    // The PrecompileFactories runtime effects are static so prior runs may have already
    // set their StableKeys. Reset the StableKeys so all our expectations/asserts will be met.
    reset_stable_keys(&[annulus.clone(), src.clone()]);
    new_options.user_defined_known_runtime_effects = vec![Some(annulus), None, Some(src)];

    with_context(&mut reporter, &new_options, |reporter, context| {
        let shader_code_dictionary = context.shared_context().shader_code_dictionary();
        reporter_assert!(
            reporter,
            shader_code_dictionary.num_user_defined_known_runtime_effects() == 2
        );
    });

    reporter.finish();
}

// Test that the ShaderCodeDictionary can handle excess user-defined known runtime effects
// Port of: tests/graphite/precompile/UserdefinedStableKeyTest.cpp#L530-L566 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn UserDefinedStableKeyTest_Overflow() {
    let mut reporter = Reporter::new("UserDefinedStableKeyTest_Overflow");
    let handler = PipelineCallBackHandler::new();
    let mut new_options = ContextOptions {
        pipeline_caching_callback: Some(handler.caching_callback()),
        ..ContextOptions::default()
    };

    let count = 2 * USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT;
    let mut user_defined_known_runtime_effects = Vec::new();
    for i in 0..count {
        let sksl = format!(
            "half4 main(float2 xy) {{ return half4({i}/255.0, {i}/255.0, {i}/255.0, 1.0); }}"
        );
        user_defined_known_runtime_effects.push(RuntimeEffect::make_for_shader(sksl, None).ok());
    }
    new_options.user_defined_known_runtime_effects = user_defined_known_runtime_effects;

    with_context(&mut reporter, &new_options, |reporter, context| {
        let shader_code_dictionary = context.shared_context().shader_code_dictionary();
        reporter_assert!(
            reporter,
            shader_code_dictionary.num_user_defined_known_runtime_effects()
                == USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT as usize
        );
    });

    reporter.finish();
}
