// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arc::Arc as SkArc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::executor::{Executor, ThreadPool, WorkOrder};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::context_options::Callback;
use skia_rust_gpu::graphite::draw_types::DrawTypeFlags;
use skia_rust_gpu::graphite::graphite_types::{DepthStencilFlags, InsertRecordingInfo};
use skia_rust_gpu::graphite::precompile::paint_options::PaintOptions;
use skia_rust_gpu::graphite::public_precompile::{RenderPassProperties, precompile};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{Reporter, def_graphite_adapter_test_with_options, errorf, reporter_assert};

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L17-L22 (chrome/m156)
fn rgba_1_d() -> RenderPassProperties {
    RenderPassProperties {
        ds_flags: DepthStencilFlags::Depth,
        dst_ct: ColorType::RGBA8888,
        dst_cs: None,
        requires_msaa: false,
    }
}

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L26-L33 (chrome/m156), the
// anonymous `precompile_an_arc_draw`
fn precompile_an_arc_draw(context: &WgpuContext) {
    let precompile_context = context.make_precompile_context();

    let mut paint_options = PaintOptions::default();
    paint_options.add_blend_mode(BlendMode::SrcOver);
    precompile(
        &precompile_context,
        &paint_options,
        DrawTypeFlags::CIRCULAR_ARC,
        &[rgba_1_d()],
    );
}

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L35-L53 (chrome/m156), the
// anonymous `draw_an_arc`
fn draw_an_arc(reporter: &mut Reporter, context: &mut WgpuContext) {
    let mut recorder = context.make_recorder(None);
    let ii = ImageInfo::new((16, 16), ColorType::RGBA8888, AlphaType::Premul, None);
    let surf =
        Surface::render_target(&recorder, &ii, Mipmapped::No, None, "").expect("a render target");
    let canvas = surf.canvas();
    let arc = SkArc::new(Rect::new(0.0, 0.0, 16.0, 16.0), 0.0, 270.0, true);
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::SrcOver);
    canvas.draw_arc_2(&arc, &paint);

    let Some(mut recording) = recorder.snap() else {
        errorf!(reporter, "Failed to make recording");
        return;
    };
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
}

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L55-L81 (chrome/m156), the
// anonymous `run_test`. The context and its executor are the test's options (`allowThreads`
// sets the executor there); this checks the stats of the context the test made.
fn run_test(reporter: &mut Reporter, context: &mut WgpuContext, precompile_first: bool) {
    if precompile_first {
        precompile_an_arc_draw(context);
    }
    draw_an_arc(reporter, context);
    if !precompile_first {
        precompile_an_arc_draw(context);
    }
    crate::tools::graphite_test_context::synced_submit(context);

    let stats = context.shared_context().base().global_cache().stats();
    let mgr_stats = context
        .shared_context()
        .base()
        .pipeline_manager()
        .get_stats();
    reporter_assert!(reporter, stats.graphics_cache_additions == 1);
    reporter_assert!(reporter, mgr_stats.num_tasks_created == 1);
}

// The executor that forces a threaded PipelineManager (`MakeMultiListFIFOThreadPool(2, 1, false)`).
fn threaded_options(options: &mut skia_rust_gpu::graphite::context_options::ContextOptions) {
    let executor: Arc<dyn Executor> = Arc::new(ThreadPool::new(WorkOrder::Fifo, 2, 1, false));
    options.executor = Some(Callback(executor));
}

// The goal here is to test out the PipelineManager's de-duplication of Pipeline
// creation tasks for both threaded and non-threaded PipelineManagers.
// The tests will request the same Pipeline via both Precompilation and drawing -
// altering which one comes first.
// These two tests forcibly create a threaded PipelineManager and then request the
// same Pipeline.
// If precompilation is first the draw should use the precompiled Pipeline.
// If the draw is first the precompilation should just find it.
// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L124-L135 (chrome/m156)
def_graphite_adapter_test_with_options!(
    PipelineManagerThreadedTest_1,
    |options| { threaded_options(options) },
    |reporter, context| {
        run_test(reporter, context, /* precompile_first= */ true)
    }
);

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L137-L148 (chrome/m156)
def_graphite_adapter_test_with_options!(
    PipelineManagerThreadedTest_2,
    |options| { threaded_options(options) },
    |reporter, context| {
        run_test(reporter, context, /* precompile_first= */ false)
    }
);

// The next two tests are the same as above but deliberately create a non-threaded
// PipelineManager.
// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L152-L163 (chrome/m156)
def_graphite_adapter_test_with_options!(
    PipelineManagerThreadedTest_3,
    |_options| {},
    |reporter, context| {
        run_test(reporter, context, /* precompile_first= */ true)
    }
);

// Port of: tests/graphite/precompile/PipelineManagerThreaded.cpp#L165-L176 (chrome/m156)
def_graphite_adapter_test_with_options!(
    PipelineManagerThreadedTest_4,
    |_options| {},
    |reporter, context| {
        run_test(reporter, context, /* precompile_first= */ false)
    }
);
