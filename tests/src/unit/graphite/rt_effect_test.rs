// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/RTEffectTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::known_runtime_effects::is_user_defined_runtime_effect;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `SkMakeRuntimeEffect(SkRuntimeEffect::MakeForShader, sksl)`.
// Port of: src/core/SkRuntimeEffectPriv.h#L167-L175 (chrome/m156)
fn make_shader(sksl: &str) -> RuntimeEffect {
    RuntimeEffect::make_for_shader(sksl, None).expect("the effect compiles")
}

// `SkMakeRuntimeEffect(SkRuntimeEffect::MakeForColorFilter, sksl)`.
// Port of: src/core/SkRuntimeEffectPriv.h#L167-L175 (chrome/m156)
fn make_color_filter(sksl: &str) -> RuntimeEffect {
    RuntimeEffect::make_for_color_filter(sksl, None).expect("the effect compiles")
}

// Port of: tests/graphite/RTEffectTest.cpp#L19-L42 (chrome/m156)
def_graphite_test_for_all_contexts!(
    Shader_FindOrCreateSnippetForRuntimeEffect,
    |reporter, context| {
        let dict = context.shared_context().shader_code_dictionary();

        let test_effect = make_shader(
            "half4 main(float2 coords) {\
             return half4(coords.xy01);\
         }",
        );

        // Create a new runtime-effect snippet.
        let snippet_id = dict
            .find_or_create_runtime_effect_snippet(&test_effect)
            .expect("a snippet id");
        reporter_assert!(reporter, is_user_defined_runtime_effect(snippet_id));

        // Verify that it can be looked up and its name is 'RuntimeEffect'. (The name isn't meaningful,
        // but this is an easy way to verify that we didn't get an unrelated snippet.)
        let snippet = dict.get_entry(snippet_id);
        reporter_assert!(reporter, snippet.is_some());
        let Some(snippet) = snippet else {
            return;
        };
        reporter_assert!(reporter, snippet.name == "RuntimeEffect");

        // If we pass the same effect again, we should get the same snippet ID as before.
        let found_snippet_id = dict.find_or_create_runtime_effect_snippet(&test_effect);
        reporter_assert!(reporter, found_snippet_id == Some(snippet_id));
    }
);

// Port of: tests/graphite/RTEffectTest.cpp#L44-L70 (chrome/m156)
def_graphite_test_for_all_contexts!(
    ColorFilter_FindOrCreateSnippetForRuntimeEffect,
    |reporter, context| {
        let dict = context.shared_context().shader_code_dictionary();

        let test_effect = make_color_filter(
            "half4 main(half4 color) {\
             return color.gbra;\
         }",
        );

        // Create a new runtime-effect snippet.
        let snippet_id = dict
            .find_or_create_runtime_effect_snippet(&test_effect)
            .expect("a snippet id");
        reporter_assert!(reporter, is_user_defined_runtime_effect(snippet_id));

        // Verify that it can be looked up and its name is 'RuntimeEffect'. (The name isn't meaningful,
        // but this is an easy way to verify that we didn't get an unrelated snippet.)
        let snippet = dict.get_entry(snippet_id);
        reporter_assert!(reporter, snippet.is_some());
        let Some(snippet) = snippet else {
            return;
        };
        reporter_assert!(reporter, snippet.name == "RuntimeEffect");

        // If we pass the same effect again, we should get the same snippet ID as before.
        let found_snippet_id = dict.find_or_create_runtime_effect_snippet(&test_effect);
        reporter_assert!(reporter, found_snippet_id == Some(snippet_id));
    }
);

// Port of: tests/graphite/RTEffectTest.cpp#L72-L113 (chrome/m156)
def_graphite_test_for_all_contexts!(
    ShaderUniforms_FindOrCreateSnippetForRuntimeEffect,
    |reporter, context| {
        let dict = context.shared_context().shader_code_dictionary();

        let test_effect = make_shader(
            "uniform float3x3 MyFloat3x3Uniform;\
         uniform int4 MyInt4ArrayUniform[1];\
         uniform half2 MyHalf2ArrayUniform[99];\
         half4 main(float2 coords) {\
             return half4(coords.xy01);\
         }",
        );

        // Create a new runtime-effect snippet.
        let snippet_id = dict
            .find_or_create_runtime_effect_snippet(&test_effect)
            .expect("a snippet id");
        reporter_assert!(reporter, is_user_defined_runtime_effect(snippet_id));

        // Delete the test effect.
        drop(test_effect);

        // Verify that it can be looked up by its snippet ID.
        let snippet = dict.get_entry(snippet_id);
        reporter_assert!(reporter, snippet.is_some());
        let Some(snippet) = snippet else {
            return;
        };

        // The uniform span should match our expectations even though the runtime effect was deleted.
        reporter_assert!(reporter, snippet.uniforms.len() == 3);
        if snippet.uniforms.len() != 3 {
            return;
        }

        reporter_assert!(reporter, snippet.uniforms[0].name() == "MyFloat3x3Uniform");
        reporter_assert!(reporter, snippet.uniforms[0].ty() == SkSLType::Float3x3);
        reporter_assert!(reporter, snippet.uniforms[0].count() == 0);

        reporter_assert!(reporter, snippet.uniforms[1].name() == "MyInt4ArrayUniform");
        reporter_assert!(reporter, snippet.uniforms[1].ty() == SkSLType::Int4);
        reporter_assert!(reporter, snippet.uniforms[1].count() == 1);

        reporter_assert!(
            reporter,
            snippet.uniforms[2].name() == "MyHalf2ArrayUniform"
        );
        reporter_assert!(reporter, snippet.uniforms[2].ty() == SkSLType::Half2);
        reporter_assert!(reporter, snippet.uniforms[2].count() == 99);
    }
);

// Port of: tests/graphite/RTEffectTest.cpp#L115-L157 (chrome/m156)
def_graphite_test_for_all_contexts!(
    ColorFilterUniforms_FindOrCreateSnippetForRuntimeEffect,
    |reporter, context| {
        let dict = context.shared_context().shader_code_dictionary();

        let test_effect = make_color_filter(
            "uniform float3x3 MyFloat3x3Uniform;\
         uniform int4 MyInt4ArrayUniform[1];\
         uniform half2 MyHalf2ArrayUniform[99];\
         half4 main(half4 color) {\
             return color.gbra;\
         }",
        );

        // Create a new runtime-effect snippet.
        let snippet_id = dict
            .find_or_create_runtime_effect_snippet(&test_effect)
            .expect("a snippet id");
        reporter_assert!(reporter, is_user_defined_runtime_effect(snippet_id));

        // Delete the test effect.
        drop(test_effect);

        // Verify that it can be looked up by its snippet ID.
        let snippet = dict.get_entry(snippet_id);
        reporter_assert!(reporter, snippet.is_some());
        let Some(snippet) = snippet else {
            return;
        };

        // The uniform span should match our expectations even though the runtime effect was deleted.
        reporter_assert!(reporter, snippet.uniforms.len() == 3);
        if snippet.uniforms.len() != 3 {
            return;
        }

        reporter_assert!(reporter, snippet.uniforms[0].name() == "MyFloat3x3Uniform");
        reporter_assert!(reporter, snippet.uniforms[0].ty() == SkSLType::Float3x3);
        reporter_assert!(reporter, snippet.uniforms[0].count() == 0);

        reporter_assert!(reporter, snippet.uniforms[1].name() == "MyInt4ArrayUniform");
        reporter_assert!(reporter, snippet.uniforms[1].ty() == SkSLType::Int4);
        reporter_assert!(reporter, snippet.uniforms[1].count() == 1);

        reporter_assert!(
            reporter,
            snippet.uniforms[2].name() == "MyHalf2ArrayUniform"
        );
        reporter_assert!(reporter, snippet.uniforms[2].ty() == SkSLType::Half2);
        reporter_assert!(reporter, snippet.uniforms[2].count() == 99);
    }
);
