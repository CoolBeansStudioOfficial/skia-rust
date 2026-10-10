// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::executor::{Executor, ThreadPool, WorkOrder};
use skia_rust_effects::gradient::Interpolation;
use skia_rust_gpu::graphite::context_options::{Callback, ContextOptions};
use skia_rust_gpu::graphite::draw_types::DrawTypeFlags;
use skia_rust_gpu::graphite::graphite_types::DepthStencilFlags;
use skia_rust_gpu::graphite::precompile::paint_options::PaintOptions;
use skia_rust_gpu::graphite::precompile::shader::{GradientShaderFlags, PrecompileShaders};
use skia_rust_gpu::graphite::precompile_context::PrecompileContext;
use skia_rust_gpu::graphite::public_precompile::{RenderPassProperties, precompile};

use crate::tools::graphite_test_context::real_context_with_options;

// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L24-L53 (chrome/m156)
fn render_pass_properties() -> Vec<RenderPassProperties> {
    vec![
        // kR_1_D
        RenderPassProperties {
            ds_flags: DepthStencilFlags::Depth,
            dst_ct: ColorType::Alpha8,
            dst_cs: None,
            requires_msaa: false,
        },
        // kRGBA_1_D
        RenderPassProperties {
            ds_flags: DepthStencilFlags::Depth,
            dst_ct: ColorType::RGBA8888,
            dst_cs: None,
            requires_msaa: false,
        },
        // kRGBA_1_D_SRGB
        RenderPassProperties {
            ds_flags: DepthStencilFlags::Depth,
            dst_ct: ColorType::RGBA8888,
            dst_cs: Some(ColorSpace::new_srgb()),
            requires_msaa: false,
        },
        // kRGBA_4_DS
        RenderPassProperties {
            ds_flags: DepthStencilFlags::DepthStencil,
            dst_ct: ColorType::RGBA8888,
            dst_cs: None,
            requires_msaa: true,
        },
        // kRGBA_4_DS_SRGB
        RenderPassProperties {
            ds_flags: DepthStencilFlags::DepthStencil,
            dst_ct: ColorType::RGBA8888,
            dst_cs: Some(ColorSpace::new_srgb()),
            requires_msaa: true,
        },
    ]
}

// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L55-L70 (chrome/m156), the
// anonymous `add_precompilation`
fn add_precompilation(precompile_context: &PrecompileContext) {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::linear_gradient(
        GradientShaderFlags::ALL,
        Interpolation::default(),
    )]);

    precompile(
        precompile_context,
        &paint_options,
        DrawTypeFlags::SIMPLE_SHAPE | DrawTypeFlags::NON_SIMPLE_SHAPE,
        &render_pass_properties(),
    );
}

// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L72-L100 (chrome/m156), the
// anonymous `run_test`. Skips (returns) without a rendering adapter, as `real_context_with_options`
// does.
fn run_test(
    orig_options: &ContextOptions,
    allow_threads: bool,
    keep_precompile_context_alive: bool,
) {
    // Ensure the threaded PipelineManager will be used
    let executor: Option<Arc<ThreadPool>> =
        allow_threads.then(|| Arc::new(ThreadPool::new(WorkOrder::Fifo, 2, 1, false)));

    let mut new_options = orig_options.clone();
    new_options.executor = executor
        .as_ref()
        .map(|executor| Callback(Arc::clone(executor) as Arc<dyn Executor>));

    let Some((_, context)) = real_context_with_options(&new_options) else {
        return;
    };

    let mut precompile_context: Option<PrecompileContext>;

    // Create a scoped Context so we can destroy it early
    {
        precompile_context = Some(context.make_precompile_context());

        // Queue up an excess of precompilations
        add_precompilation(precompile_context.as_ref().expect("made above"));

        if !keep_precompile_context_alive {
            precompile_context = None;
        }

        // Destroying the Context here should trigger the PipelineManager to shut down with work
        // still pending.
        drop(context);
    }

    // However, PrecompileContext can keep things alive on its own but will be downgraded
    // to non-threaded compilation.
    if keep_precompile_context_alive {
        add_precompilation(precompile_context.as_ref().expect("kept alive above"));
    }

    // Success is defined by not crashing or triggering any *SAN errors
    drop(precompile_context);
    drop(executor);
}

// Verify that the threaded PipelineManager terminates cleanly
// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L120-L131 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn PipelineManagerEarlyExitTest_1() {
    run_test(
        &ContextOptions::default(),
        /* allow_threads= */ true,
        /* keep_precompile_context_alive= */ false,
    );
}

// Verify that the PrecompileContext pins everything it needs to operate on its own
// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L134-L145 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn PipelineManagerEarlyExitTest_2() {
    run_test(
        &ContextOptions::default(),
        /* allow_threads= */ true,
        /* keep_precompile_context_alive= */ true,
    );
}

// The next two are the same as above but single threaded
// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L148-L159 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn PipelineManagerEarlyExitTest_3() {
    run_test(
        &ContextOptions::default(),
        /* allow_threads= */ false,
        /* keep_precompile_context_alive= */ false,
    );
}

// Port of: tests/graphite/precompile/PipelineManagerEarlyExit.cpp#L161-L172 (chrome/m156)
#[test]
#[cfg_attr(
    not(skia_rust_adapter_tests),
    ignore = "needs a real adapter in CI (lavapipe job)"
)]
#[allow(non_snake_case)]
fn PipelineManagerEarlyExitTest_4() {
    run_test(
        &ContextOptions::default(),
        /* allow_threads= */ false,
        /* keep_precompile_context_alive= */ true,
    );
}
