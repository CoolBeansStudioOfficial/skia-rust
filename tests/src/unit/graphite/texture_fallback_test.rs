// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/TextureFallbackTest.cpp (chrome/m156), the tests that inspect the SkSL
// `EmitStorageFallbackTexture` writes. `TextureFallbackMultiStopGradientsDrawTest` draws to a
// surface and reads the result back (G11c), so it is not ported here.

#![cfg(test)]
// Mirrors the C++ tests, which declare constants and similarly named bindings inline.
#![allow(
    clippy::items_after_statements,
    clippy::similar_names,
    clippy::too_many_lines
)]

use skia_rust_gpu::graphite::caps::{Caps, ResourceBindingRequirements};
use skia_rust_gpu::graphite::draw_params::DrawParams;
use skia_rust_gpu::graphite::draw_types::{DepthStencilSettings, PrimitiveType};
use skia_rust_gpu::graphite::draw_writer::DrawWriter;
use skia_rust_gpu::graphite::paint_params_key::RootNodesInfo;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::render_step::{
    RenderStep, RenderStepBase, RenderStepFlags, RenderStepID,
};
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::shader_info::emit_storage_fallback_texture;
use skia_rust_gpu::graphite::storage_context::StorageContext;
use skia_rust_gpu::graphite::uniform::Uniform;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

/// `TestStep`: a mesh step whose only interesting part is its storage uniforms.
// Port of: tests/graphite/TextureFallbackTest.cpp#L28-L50 (chrome/m156)
#[derive(Debug)]
struct TestStep {
    base: RenderStepBase,
}

impl TestStep {
    fn new(storage_uniforms: &[Uniform]) -> Self {
        TestStep {
            base: RenderStepBase::new(
                Layout::Std430,
                RenderStepID::Mesh,
                RenderStepFlags::PERFORMS_SHADING | RenderStepFlags::FS_USES_STORAGE,
                &[],
                PrimitiveType::TriangleStrip,
                DepthStencilSettings::default(),
                &[],
                &[],
                storage_uniforms,
                &[],
            ),
        }
    }
}

impl RenderStep for TestStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    fn write_vertices(&self, _writer: &mut DrawWriter<'_>, _params: &DrawParams, _ssbo_index: u32) {
    }

    fn write_uniforms_and_textures(
        &self,
        _params: &DrawParams,
        _gatherer: &mut PipelineDataGatherer,
    ) {
    }

    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        "stepLocalCoords = float2(0.0);".to_owned()
    }

    fn fragment_coverage_sksl(&self) -> &'static str {
        ""
    }
}

/// `TextureFallbackTest::emitFallback(reqs, step)`.
// Port of: tests/graphite/TextureFallbackTest.cpp#L56-L59 (chrome/m156)
fn emit_fallback(reqs: &ResourceBindingRequirements, step: &TestStep) -> String {
    emit_storage_fallback_texture(reqs, step)
}

/// The storage-buffer-free bindings every test starts from: uniforms at set 0, storage at 0.
fn fallback_reqs(caps: &dyn Caps) -> ResourceBindingRequirements {
    let mut reqs = *caps.resource_binding_requirements();
    reqs.uniforms_set_idx = 0;
    reqs.storage_buffer_binding = 0;
    reqs
}

// Port of: tests/graphite/TextureFallbackTest.cpp#L63-L92 (chrome/m156)
def_graphite_test_for_all_contexts!(
    TextureFallbackScalarsAndBitcastsTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let reqs = fallback_reqs(&**recorder.priv_().caps());
        let step = TestStep::new(&[
            Uniform::new("f", SkSLType::Float),
            Uniform::new("h", SkSLType::Half),
            Uniform::new("i", SkSLType::Int),
            Uniform::new("u", SkSLType::UInt),
        ]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(reporter, sksl.contains("float f;"));
        reporter_assert!(reporter, sksl.contains("half h;"));
        reporter_assert!(reporter, sksl.contains("int i;"));
        reporter_assert!(reporter, sksl.contains("uint u;"));
        reporter_assert!(reporter, sksl.contains("data.f = t0.x;"));
        reporter_assert!(reporter, sksl.contains("data.h = t0.y;"));
        reporter_assert!(reporter, sksl.contains("data.i = floatBitsToInt(t0.z);"));
        reporter_assert!(reporter, sksl.contains("data.u = floatBitsToUint(t0.w);"));
        reporter_assert!(reporter, sksl.contains("int linearIdx0 = index * 1 + 0;"));
        reporter_assert!(reporter, !sksl.contains("int linearIdx1"));
    }
);

// Port of: tests/graphite/TextureFallbackTest.cpp#L95-L208 (chrome/m156)
def_graphite_test_for_all_contexts!(
    TextureFallbackAlignmentAndPaddingTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let reqs = fallback_reqs(&**recorder.priv_().caps());
        // Case A: float + float2 + float (8-byte alignment skips t0.y)
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float),
                Uniform::new("b", SkSLType::Float2),
                Uniform::new("c", SkSLType::Float),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.x;"));
            reporter_assert!(reporter, sksl.contains("data.b = t0.zw;"));
            reporter_assert!(reporter, sksl.contains("data.c = t1.x;"));
        }
        // Case B: float + float + float2 (8-byte alignment packed tight: t0.xy, t0.zw)
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float),
                Uniform::new("b", SkSLType::Float),
                Uniform::new("c", SkSLType::Float2),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.x;"));
            reporter_assert!(reporter, sksl.contains("data.b = t0.y;"));
            reporter_assert!(reporter, sksl.contains("data.c = t0.zw;"));
            reporter_assert!(reporter, !sksl.contains("int linearIdx1"));
        }
        // Case C: float + float3 (16-byte alignment skips t0.yzw)
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float),
                Uniform::new("b", SkSLType::Float3),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.x;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1.xyz;"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float),
                Uniform::new("b", SkSLType::Float4),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.x;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1;"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float2),
                Uniform::new("b", SkSLType::Float3),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.xy;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1.xyz;"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float2),
                Uniform::new("b", SkSLType::Float4),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.xy;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1;"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float3),
                Uniform::new("b", SkSLType::Float),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.xyz;"));
            reporter_assert!(reporter, sksl.contains("data.b = t0.w;"));
            reporter_assert!(reporter, !sksl.contains("int linearIdx1"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float3),
                Uniform::new("b", SkSLType::Float2),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.xyz;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1.xy;"));
        }
        {
            let step = TestStep::new(&[
                Uniform::new("a", SkSLType::Float3),
                Uniform::new("b", SkSLType::Float3),
            ]);
            let sksl = emit_fallback(&reqs, &step);
            reporter_assert!(reporter, sksl.contains("data.a = t0.xyz;"));
            reporter_assert!(reporter, sksl.contains("data.b = t1.xyz;"));
        }
    }
);

// Port of: tests/graphite/TextureFallbackTest.cpp#L211-L263 (chrome/m156)
def_graphite_test_for_all_contexts!(TextureFallbackMatricesTest, |reporter, context| {
    let recorder = context.make_recorder(None);
    let reqs = fallback_reqs(&**recorder.priv_().caps());
    {
        let step = TestStep::new(&[Uniform::new("m2", SkSLType::Float2x2)]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(reporter, sksl.contains("data.m2 = float2x2(t0.xy, t0.zw);"));
        reporter_assert!(reporter, !sksl.contains("int linearIdx1"));
    }
    {
        let step = TestStep::new(&[
            Uniform::new("v2", SkSLType::Float2),
            Uniform::new("m2", SkSLType::Float2x2),
        ]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(reporter, sksl.contains("data.v2 = t0.xy;"));
        reporter_assert!(reporter, sksl.contains("data.m2 = float2x2(t0.zw, t1.xy);"));
    }
    {
        let step = TestStep::new(&[Uniform::new("m3", SkSLType::Float3x3)]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(
            reporter,
            sksl.contains("data.m3 = float3x3(t0.xyz, t1.xyz, t2.xyz);")
        );
    }
    {
        let step = TestStep::new(&[Uniform::new("m4", SkSLType::Float4x4)]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(
            reporter,
            sksl.contains("data.m4 = float4x4(t0, t1, t2, t3);")
        );
    }
});

// Port of: tests/graphite/TextureFallbackTest.cpp#L266-L291 (chrome/m156)
def_graphite_test_for_all_contexts!(TextureFallbackVectorBitcastsTest, |reporter, context| {
    let recorder = context.make_recorder(None);
    let reqs = fallback_reqs(&**recorder.priv_().caps());
    let step = TestStep::new(&[
        Uniform::new("i2", SkSLType::Int2),
        Uniform::new("u2", SkSLType::UInt2),
        Uniform::new("i3", SkSLType::Int3),
        Uniform::new("u3", SkSLType::UInt3),
        Uniform::new("i4", SkSLType::Int4),
        Uniform::new("u4", SkSLType::UInt4),
    ]);
    let sksl = emit_fallback(&reqs, &step);
    reporter_assert!(reporter, sksl.contains("data.i2 = floatBitsToInt(t0.xy);"));
    reporter_assert!(reporter, sksl.contains("data.u2 = floatBitsToUint(t0.zw);"));
    reporter_assert!(reporter, sksl.contains("data.i3 = floatBitsToInt(t1.xyz);"));
    reporter_assert!(
        reporter,
        sksl.contains("data.u3 = floatBitsToUint(t2.xyz);")
    );
    reporter_assert!(reporter, sksl.contains("data.i4 = floatBitsToInt(t3);"));
    reporter_assert!(reporter, sksl.contains("data.u4 = floatBitsToUint(t4);"));
});

// Port of: tests/graphite/TextureFallbackTest.cpp#L294-L401 (chrome/m156)
def_graphite_test_for_all_contexts!(TextureFallbackMultiInstanceMathTest, |reporter, context| {
    let recorder = context.make_recorder(None);
    let mut reqs = fallback_reqs(&**recorder.priv_().caps());
    reqs.uniforms_set_idx = 2;
    reqs.storage_buffer_binding = 5;
    let step = TestStep::new(&[
        Uniform::new("v4_0", SkSLType::Float4),
        Uniform::new("v4_1", SkSLType::Float4),
        Uniform::new("v4_2", SkSLType::Float4),
    ]);
    let sksl = emit_fallback(&reqs, &step);
    reporter_assert!(
        reporter,
        sksl.contains("layout(set=2, binding=5) readonly texture2D storageFallbackTexture;")
    );
    let max_atlas_width = reqs.max_fallback_texture_size;
    let expected_width = format!("const int texWidth = {max_atlas_width};");
    reporter_assert!(reporter, sksl.contains(&expected_width));
    reporter_assert!(
        reporter,
        reqs.max_fallback_texture_bytes == max_atlas_width * max_atlas_width * 16
    );
    let mut custom_reqs = reqs;
    custom_reqs.max_fallback_texture_size = 4096;
    let custom_sksl = emit_fallback(&custom_reqs, &step);
    reporter_assert!(reporter, custom_sksl.contains("const int texWidth = 4096;"));
    reporter_assert!(reporter, sksl.contains("int linearIdx0 = index * 3 + 0;"));
    reporter_assert!(
        reporter,
        sksl.contains("int2 coords0 = int2(linearIdx0 % texWidth, linearIdx0 / texWidth);")
    );
    reporter_assert!(
        reporter,
        sksl.contains("float4 t0 = float4(textureRead(storageFallbackTexture, uint2(coords0)));")
    );
    reporter_assert!(reporter, sksl.contains("int linearIdx1 = index * 3 + 1;"));
    reporter_assert!(
        reporter,
        sksl.contains("int2 coords1 = int2(linearIdx1 % texWidth, linearIdx1 / texWidth);")
    );
    reporter_assert!(
        reporter,
        sksl.contains("float4 t1 = float4(textureRead(storageFallbackTexture, uint2(coords1)));")
    );
    reporter_assert!(reporter, sksl.contains("int linearIdx2 = index * 3 + 2;"));
    reporter_assert!(
        reporter,
        sksl.contains("int2 coords2 = int2(linearIdx2 % texWidth, linearIdx2 / texWidth);")
    );
    reporter_assert!(
        reporter,
        sksl.contains("float4 t2 = float4(textureRead(storageFallbackTexture, uint2(coords2)));")
    );
    reporter_assert!(reporter, step.base().storage_uniform_stride() == 48);
    reporter_assert!(reporter, step.base().storage_uniform_alignment() == 16);
    let single_float_step = TestStep::new(&[Uniform::new("s", SkSLType::Float)]);
    reporter_assert!(
        reporter,
        single_float_step.base().storage_uniform_stride() == 4
    );
    reporter_assert!(
        reporter,
        single_float_step.base().storage_uniform_alignment() == 4
    );
    let float2_step = TestStep::new(&[Uniform::new("v2", SkSLType::Float2)]);
    reporter_assert!(reporter, float2_step.base().storage_uniform_stride() == 8);
    reporter_assert!(
        reporter,
        float2_step.base().storage_uniform_alignment() == 8
    );
    let float3_step = TestStep::new(&[Uniform::new("v3", SkSLType::Float3)]);
    reporter_assert!(reporter, float3_step.base().storage_uniform_stride() == 16);
    reporter_assert!(
        reporter,
        float3_step.base().storage_uniform_alignment() == 16
    );
    {
        let mut fallback_storage_context = StorageContext::new(
            reqs.max_fallback_texture_size,
            /*storage_buffer_support=*/ false,
        );
        fallback_storage_context.record_alignment(
            single_float_step.base().storage_uniform_stride(),
            single_float_step.base().storage_uniform_alignment(),
        );
        reporter_assert!(reporter, fallback_storage_context.running_lcm() == 16);
        fallback_storage_context.record_alignment(
            float2_step.base().storage_uniform_stride(),
            float2_step.base().storage_uniform_alignment(),
        );
        reporter_assert!(reporter, fallback_storage_context.running_lcm() == 16);
    }
    {
        let mut ssbo_storage_context = StorageContext::new(
            reqs.max_fallback_texture_size,
            /*storage_buffer_support=*/ true,
        );
        ssbo_storage_context.record_alignment(
            single_float_step.base().storage_uniform_stride(),
            single_float_step.base().storage_uniform_alignment(),
        );
        reporter_assert!(reporter, ssbo_storage_context.running_lcm() == 4);
        ssbo_storage_context.record_alignment(
            float2_step.base().storage_uniform_stride(),
            float2_step.base().storage_uniform_alignment(),
        );
        reporter_assert!(reporter, ssbo_storage_context.running_lcm() == 8);
    }
    reporter_assert!(
        reporter,
        sksl.contains("inline StepStorageData readStepStorageData(uint index)")
    );
});

// Port of: tests/graphite/TextureFallbackTest.cpp#L405-L460 (chrome/m156)
def_graphite_test_for_all_contexts!(
    TextureFallbackStressMixedTypesAndAlignmentTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let mut reqs = fallback_reqs(&**recorder.priv_().caps());
        reqs.uniforms_set_idx = 1;
        reqs.storage_buffer_binding = 3;
        let step = TestStep::new(&[
            Uniform::new("s0", SkSLType::Float),
            Uniform::new("i0", SkSLType::Int),
            Uniform::new("v2_0", SkSLType::Float2),
            Uniform::new("m2_0", SkSLType::Float2x2),
            Uniform::new("s1", SkSLType::Float),
            Uniform::new("m2_1", SkSLType::Float2x2),
            Uniform::new("u0", SkSLType::UInt),
            Uniform::new("s2", SkSLType::Float),
            Uniform::new("v3_0", SkSLType::Float3),
            Uniform::new("i1", SkSLType::Int),
            Uniform::new("m3_0", SkSLType::Float3x3),
            Uniform::new("h0", SkSLType::Half),
            Uniform::new("i2_0", SkSLType::Int2),
            Uniform::new("v4_0", SkSLType::Float4),
            Uniform::new("u3_0", SkSLType::UInt3),
            Uniform::new("m4_0", SkSLType::Float4x4),
            Uniform::new("u4_0", SkSLType::UInt4),
            Uniform::new("s3", SkSLType::Float),
            Uniform::new("v3_1", SkSLType::Float3),
            Uniform::new("i4_0", SkSLType::Int4),
        ]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(reporter, sksl.contains("data.s0 = t0.x;"));
        reporter_assert!(reporter, sksl.contains("data.i0 = floatBitsToInt(t0.y);"));
        reporter_assert!(reporter, sksl.contains("data.v2_0 = t0.zw;"));
        reporter_assert!(
            reporter,
            sksl.contains("data.m2_0 = float2x2(t1.xy, t1.zw);")
        );
        reporter_assert!(reporter, sksl.contains("data.s1 = t2.x;"));
        reporter_assert!(
            reporter,
            sksl.contains("data.m2_1 = float2x2(t2.zw, t3.xy);")
        );
        reporter_assert!(reporter, sksl.contains("data.u0 = floatBitsToUint(t3.z);"));
        reporter_assert!(reporter, sksl.contains("data.s2 = t3.w;"));
        reporter_assert!(reporter, sksl.contains("data.v3_0 = t4.xyz;"));
        reporter_assert!(reporter, sksl.contains("data.i1 = floatBitsToInt(t4.w);"));
        reporter_assert!(
            reporter,
            sksl.contains("data.m3_0 = float3x3(t5.xyz, t6.xyz, t7.xyz);")
        );
        reporter_assert!(reporter, sksl.contains("data.h0 = t8.x;"));
        reporter_assert!(
            reporter,
            sksl.contains("data.i2_0 = floatBitsToInt(t8.zw);")
        );
        reporter_assert!(reporter, sksl.contains("data.v4_0 = t9;"));
        reporter_assert!(
            reporter,
            sksl.contains("data.u3_0 = floatBitsToUint(t10.xyz);")
        );
        reporter_assert!(
            reporter,
            sksl.contains("data.m4_0 = float4x4(t11, t12, t13, t14);")
        );
        reporter_assert!(reporter, sksl.contains("data.u4_0 = floatBitsToUint(t15);"));
        reporter_assert!(reporter, sksl.contains("data.s3 = t16.x;"));
        reporter_assert!(reporter, sksl.contains("data.v3_1 = t17.xyz;"));
        reporter_assert!(reporter, sksl.contains("data.i4_0 = floatBitsToInt(t18);"));
        reporter_assert!(reporter, sksl.contains("int linearIdx0 = index * 19 + 0;"));
        reporter_assert!(
            reporter,
            sksl.contains("int linearIdx18 = index * 19 + 18;")
        );
        reporter_assert!(reporter, !sksl.contains("int linearIdx19"));
    }
);

// Port of: tests/graphite/TextureFallbackTest.cpp#L463-L499 (chrome/m156)
def_graphite_test_for_all_contexts!(
    TextureFallbackStressBackToBackComplexStructuresTest,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let reqs = fallback_reqs(&**recorder.priv_().caps());
        let step = TestStep::new(&[
            Uniform::new("offset_pad", SkSLType::Float2),
            Uniform::new("m2", SkSLType::Float2x2),
            Uniform::new("m3", SkSLType::Float3x3),
            Uniform::new("m4", SkSLType::Float4x4),
            Uniform::new("v3_a", SkSLType::Float3),
            Uniform::new("v2_a", SkSLType::Float2),
            Uniform::new("v3_b", SkSLType::Float3),
            Uniform::new("v4_a", SkSLType::Float4),
        ]);
        let sksl = emit_fallback(&reqs, &step);
        reporter_assert!(reporter, sksl.contains("data.offset_pad = t0.xy;"));
        reporter_assert!(reporter, sksl.contains("data.m2 = float2x2(t0.zw, t1.xy);"));
        reporter_assert!(
            reporter,
            sksl.contains("data.m3 = float3x3(t2.xyz, t3.xyz, t4.xyz);")
        );
        reporter_assert!(
            reporter,
            sksl.contains("data.m4 = float4x4(t5, t6, t7, t8);")
        );
        reporter_assert!(reporter, sksl.contains("data.v3_a = t9.xyz;"));
        reporter_assert!(reporter, sksl.contains("data.v2_a = t10.xy;"));
        reporter_assert!(reporter, sksl.contains("data.v3_b = t11.xyz;"));
        reporter_assert!(reporter, sksl.contains("data.v4_a = t12;"));
        reporter_assert!(reporter, sksl.contains("int linearIdx0 = index * 13 + 0;"));
        reporter_assert!(
            reporter,
            sksl.contains("int linearIdx12 = index * 13 + 12;")
        );
        reporter_assert!(reporter, !sksl.contains("int linearIdx13"));
    }
);
