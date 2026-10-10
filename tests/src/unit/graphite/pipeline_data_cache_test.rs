// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/PipelineDataCacheTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which declares similarly named bindings in one long function.
#![allow(clippy::similar_names, clippy::too_many_lines)]

use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::pipeline_data::{
    PipelineDataGatherer, TextureDataCache, UniformDataCache,
};
use skia_rust_gpu::graphite::resource_types::{Layout, SamplerDesc};
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::uniform::Uniform;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `SkDEBUGCODE(UniformExpectationsValidator uev(&gatherer, uniforms);)`: the validator of the port
// borrows the gatherer's uniform manager, which the test also writes through, so the test starts
// and ends the expectations itself (the validator's constructor and destructor).
#[cfg(debug_assertions)]
fn expect_uniforms(gatherer: &mut PipelineDataGatherer, uniforms: &[Uniform]) {
    gatherer
        .uniform_manager()
        .set_expected_uniforms(uniforms, /* is_substruct= */ false);
}

#[cfg(debug_assertions)]
fn done_with_expected_uniforms(gatherer: &mut PipelineDataGatherer) {
    gatherer.uniform_manager().done_with_expected_uniforms();
}

#[cfg(not(debug_assertions))]
fn expect_uniforms(_gatherer: &mut PipelineDataGatherer, _uniforms: &[Uniform]) {}

#[cfg(not(debug_assertions))]
fn done_with_expected_uniforms(_gatherer: &mut PipelineDataGatherer) {}

// Port of: tests/graphite/PipelineDataCacheTest.cpp#L21-L307 (chrome/m156)
def_graphite_test_for_all_contexts!(PipelineDataCacheTest, |reporter, context| {
    let caps = ContextPriv::caps(context);
    let resource_provider = ContextPriv::resource_provider(context);

    // Setup caches
    let mut u_cache = UniformDataCache::new();
    let mut t_cache = TextureDataCache::new();

    reporter_assert!(reporter, u_cache.count() == 0);
    reporter_assert!(reporter, t_cache.binding_count() == 0);

    // Create testing textures and uniforms
    let k_uniforms = [Uniform::new("data", SkSLType::Float4)];
    let info = caps.get_default_sampled_texture_info(
        ColorType::Alpha8,
        Mipmapped::No,
        Protected::No,
        Renderable::Yes,
    );
    reporter_assert!(reporter, info.is_valid());

    let proxy_a = TextureProxy::make(
        caps,
        &mut resource_provider.lock().unwrap(),
        ISize::new(32, 32),
        &info,
        Budgeted::Yes,
        "TestDataProxyA",
    );
    let proxy_b = TextureProxy::make(
        caps,
        &mut resource_provider.lock().unwrap(),
        ISize::new(32, 32),
        &info,
        Budgeted::Yes,
        "TestDataProxyB",
    );
    reporter_assert!(reporter, proxy_a.is_some() && proxy_b.is_some());
    let (Some(proxy_a), Some(proxy_b)) = (proxy_a, proxy_b) else {
        return;
    };

    // Block A: Add a new, unique set of uniforms and textures for a render step
    let u_id1;
    let t_id1;
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        expect_uniforms(&mut gatherer, &k_uniforms);
        gatherer.uniform_manager().write_vec([1.0, 2.0, 3.0, 4.0]);
        gatherer.add(Some(proxy_a.clone()), SamplerDesc::default());

        let (udb, tdb) = gatherer.end_combined_data(/* performs_shading= */ true);

        u_id1 = u_cache.insert(udb.clone());
        t_id1 = t_cache.insert(tdb.clone());

        reporter_assert!(reporter, u_cache.count() == 1);
        reporter_assert!(reporter, t_cache.binding_count() == 1);
        reporter_assert!(reporter, t_cache.unique_texture_count() == 1);

        // Verify lookup
        reporter_assert!(reporter, *u_cache.lookup(u_id1).cpu_data() == udb);
        reporter_assert!(reporter, t_cache.lookup(t_id1) == tdb);
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block B: Add the exact same render step data to test de-duplication
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        expect_uniforms(&mut gatherer, &k_uniforms);
        gatherer.uniform_manager().write_vec([1.0, 2.0, 3.0, 4.0]); // Same uniform data
        gatherer.add(Some(proxy_a.clone()), SamplerDesc::default()); // Same texture

        let (udb, tdb) = gatherer.end_combined_data(/* performs_shading= */ true);

        let u_id2 = u_cache.insert(udb);
        let t_id2 = t_cache.insert(tdb);

        reporter_assert!(reporter, u_id2 == u_id1); // Index should be the same
        reporter_assert!(reporter, t_id2 == t_id1); // Index should be the same

        reporter_assert!(reporter, u_cache.count() == 1); // Count should NOT increase
        reporter_assert!(reporter, t_cache.binding_count() == 1); // Count should NOT increase
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block C: Add new unique render step uniforms but the same texture
    let u_id3;
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        expect_uniforms(&mut gatherer, &k_uniforms);
        gatherer.uniform_manager().write_vec([5.0, 6.0, 7.0, 8.0]); // Different uniform data
        gatherer.add(Some(proxy_a.clone()), SamplerDesc::default()); // Same texture

        let (udb, tdb) = gatherer.end_combined_data(/* performs_shading= */ true);

        u_id3 = u_cache.insert(udb);
        let t_id3 = t_cache.insert(tdb);

        reporter_assert!(reporter, u_id3 != u_id1); // New unique uniform index
        reporter_assert!(reporter, t_id3 == t_id1); // Same texture binding index

        reporter_assert!(reporter, u_cache.count() == 2); // Uniform count increases
        reporter_assert!(reporter, t_cache.binding_count() == 1); // Texture count does not
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block D: Add the same render step uniforms but a new unique texture
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        expect_uniforms(&mut gatherer, &k_uniforms);
        gatherer.uniform_manager().write_vec([1.0, 2.0, 3.0, 4.0]); // Same uniform data as Block A
        gatherer.add(Some(proxy_b.clone()), SamplerDesc::default()); // Different texture

        let (udb, tdb) = gatherer.end_combined_data(/* performs_shading= */ true);

        let u_id4 = u_cache.insert(udb);
        let t_id4 = t_cache.insert(tdb);

        reporter_assert!(reporter, u_id4 == u_id1); // Same uniform index
        reporter_assert!(reporter, t_id4 != t_id1); // New unique texture index

        reporter_assert!(reporter, u_cache.count() == 2); // Uniform count does not increase
        reporter_assert!(reporter, t_cache.binding_count() == 2); // Texture count increases
        reporter_assert!(reporter, t_cache.unique_texture_count() == 2);
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block E: Add a unique paint uniform.
    let u_id5;
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        expect_uniforms(&mut gatherer, &k_uniforms);
        // New unique paint uniform data
        gatherer
            .uniform_manager()
            .write_vec([10.0, 20.0, 30.0, 40.0]);

        let (udb, _tdb) = gatherer.end_combined_data(/* performs_shading= */ true);
        u_id5 = u_cache.insert(udb);

        reporter_assert!(reporter, u_id5 != u_id1 && u_id5 != u_id3); // New unique uniform index
        reporter_assert!(reporter, u_cache.count() == 3); // Uniform count increases
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block F: Add the same paint uniform to test de-duplication.
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);
        expect_uniforms(&mut gatherer, &k_uniforms);
        // Same paint uniform data as Block E
        gatherer
            .uniform_manager()
            .write_vec([10.0, 20.0, 30.0, 40.0]);

        let (udb, _tdb) = gatherer.end_combined_data(/* performs_shading= */ true);
        let u_id6 = u_cache.insert(udb);
        reporter_assert!(reporter, u_id6 == u_id5); // Index should be the same
        reporter_assert!(reporter, u_cache.count() == 3); // Count should NOT increase
        done_with_expected_uniforms(&mut gatherer);
    }

    // Block G: Test paint and render step uniforms together.
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        {
            expect_uniforms(&mut gatherer, &k_uniforms);
            // Same paint uniform as Block E
            gatherer
                .uniform_manager()
                .write_vec([10.0, 20.0, 30.0, 40.0]);
            gatherer.add(Some(proxy_a.clone()), SamplerDesc::default());
            done_with_expected_uniforms(&mut gatherer);
        }

        {
            expect_uniforms(&mut gatherer, &k_uniforms);
            // Same render step uniform as Block A
            gatherer.uniform_manager().write_vec([1.0, 2.0, 3.0, 4.0]);
            gatherer.add(Some(proxy_b.clone()), SamplerDesc::default());
            done_with_expected_uniforms(&mut gatherer);
        }

        let (combined_udb, combined_tdb) =
            gatherer.end_combined_data(/* performs_shading= */ true);
        let combined_uid = u_cache.insert(combined_udb);

        // This ID should be new because it combines Block A (RenderStep) and Block E (Paint)
        // into a single block of memory {10, 20, 30, 40, 1, 2, 3, 4}
        reporter_assert!(reporter, combined_uid != u_id1);
        reporter_assert!(reporter, combined_uid != u_id5);
        reporter_assert!(reporter, u_cache.count() == 4); // Count increases

        reporter_assert!(reporter, combined_tdb.num_textures() == 2);
        let combined_tid = t_cache.insert(combined_tdb);

        reporter_assert!(reporter, combined_tid != t_id1); // New binding
        reporter_assert!(reporter, t_cache.binding_count() == 3); // Count increases
        reporter_assert!(reporter, t_cache.unique_texture_count() == 2); // Still 2 unique proxies
    }

    // Block H: Simulate multiple recordDraw calls for a single geometry without an resetForDraw().
    {
        let k_combined_uniforms = [
            Uniform::new("step1", SkSLType::Float4),
            Uniform::new("step2", SkSLType::Float4),
        ];
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);

        gatherer.reset_for_draw();

        // First recordDraw, paint AND renderstep
        let first_combined_id;
        {
            {
                expect_uniforms(&mut gatherer, &k_uniforms);
                // First unique paint data
                gatherer
                    .uniform_manager()
                    .write_vec([11.0, 22.0, 33.0, 44.0]);
                done_with_expected_uniforms(&mut gatherer);
            }

            {
                gatherer.mark_offset_and_align(
                    /* performs_shading= */ true, /* required_alignment= */ 1,
                );

                expect_uniforms(&mut gatherer, &k_combined_uniforms);
                // Renderstep Uniforms 1
                gatherer
                    .uniform_manager()
                    .write_vec([111.0, 222.0, 333.0, 444.0]);
                // Renderstep Uniforms 2
                gatherer
                    .uniform_manager()
                    .write_vec([1111.0, 2222.0, 3333.0, 4444.0]);

                let (udb, _tdb) = gatherer.end_combined_data(/* performs_shading= */ true);
                first_combined_id = u_cache.insert(udb);
                done_with_expected_uniforms(&mut gatherer);
            }

            reporter_assert!(reporter, u_cache.count() == 5); // Count increases
        }

        // Second recordDraw
        {
            gatherer.rewind_for_render_step();
            gatherer.mark_offset_and_align(
                /* performs_shading= */ true, /* required_alignment= */ 1,
            );

            expect_uniforms(&mut gatherer, &k_combined_uniforms);
            // Renderstep Uniforms 1
            gatherer
                .uniform_manager()
                .write_vec([111.0, 222.0, 333.0, 444.0]);
            // Renderstep Uniforms 2
            gatherer
                .uniform_manager()
                .write_vec([1111.0, 2222.0, 3333.0, 4444.0]);

            let (udb, _tdb) = gatherer.end_combined_data(/* performs_shading= */ true);
            let second_combined_id = u_cache.insert(udb);

            // Should be same block
            reporter_assert!(reporter, second_combined_id == first_combined_id);
            // Count remains the same
            reporter_assert!(reporter, u_cache.count() == 5);
            done_with_expected_uniforms(&mut gatherer);
        }
    }

    // Block I: Interleaved Shading and Non-Shading steps with varying alignment.
    {
        let mut gatherer = PipelineDataGatherer::new(Layout::Std430);
        gatherer.reset_for_draw();

        // Paint Data
        {
            let k_paint_uniforms = [Uniform::new("paintVal", SkSLType::Float)];
            expect_uniforms(&mut gatherer, &k_paint_uniforms);
            gatherer.uniform_manager().write_f32(1.0);
            done_with_expected_uniforms(&mut gatherer);
        }

        // RenderStep: Non-Shading
        {
            gatherer.mark_offset_and_align(
                /* performs_shading= */ false, /* required_alignment= */ 16,
            );

            let k_step_uniforms = [Uniform::new("stepA", SkSLType::Float4)];
            expect_uniforms(&mut gatherer, &k_step_uniforms);
            gatherer
                .uniform_manager()
                .write_vec([10.0, 10.0, 10.0, 10.0]);

            let (udb, _) = gatherer.end_combined_data(/* performs_shading= */ false);
            u_cache.insert(udb.clone());

            reporter_assert!(reporter, udb.size() == 16); // 16 (renderStep)
            done_with_expected_uniforms(&mut gatherer);
        }

        // RenderStep: Shading
        {
            gatherer.rewind_for_render_step();
            gatherer.mark_offset_and_align(
                /* performs_shading= */ true, /* required_alignment= */ 1,
            );

            let k_step_uniforms = [Uniform::new("stepB", SkSLType::Float4)];
            expect_uniforms(&mut gatherer, &k_step_uniforms);
            gatherer
                .uniform_manager()
                .write_vec([20.0, 20.0, 20.0, 20.0]);

            let (udb, _) = gatherer.end_combined_data(/* performs_shading= */ true);
            u_cache.insert(udb.clone());

            reporter_assert!(reporter, udb.size() == 32); // 4 (paint) + 12 (pad) + 16 (renderStep)
            done_with_expected_uniforms(&mut gatherer);
        }

        // RenderStep: Non-Shading
        {
            gatherer.rewind_for_render_step();
            gatherer.mark_offset_and_align(
                /* performs_shading= */ false, /* required_alignment= */ 8,
            );

            let k_step_uniforms = [Uniform::new("stepC", SkSLType::Float2)];
            expect_uniforms(&mut gatherer, &k_step_uniforms);
            gatherer.uniform_manager().write_vec([30.0, 30.0]);

            let (udb, _) = gatherer.end_combined_data(/* performs_shading= */ false);
            u_cache.insert(udb.clone());

            reporter_assert!(reporter, udb.size() == 8); // 8 (renderStep)
            done_with_expected_uniforms(&mut gatherer);
        }

        reporter_assert!(reporter, u_cache.count() == 8);
    }
});
