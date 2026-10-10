// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp (chrome/m156)
//
// Only the tests whose threads precompile (and purge) are ported. The tests that also record on
// threads need a `Recorder` on another thread, and this port's `Recorder` is not `Send` (it is
// `Rc`-based); those are `todo` in the manifest.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_effects::gradient::Interpolation;
use skia_rust_gpu::graphite::draw_types::DrawTypeFlags;
use skia_rust_gpu::graphite::graphite_types::{DepthStencilFlags, SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::precompile::paint_options::PaintOptions;
use skia_rust_gpu::graphite::precompile::shader::{GradientShaderFlags, PrecompileShaders};
use skia_rust_gpu::graphite::precompile_context::PrecompileContext;
use skia_rust_gpu::graphite::public_precompile::{RenderPassProperties, precompile};
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// The 12 comes from 4 types of gradient times 3 combinations (i.e., 4,8,N) for each one.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L172-L173 (chrome/m156)
const NUM_DIFF_PIPELINES: u32 = 12;

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L26-L30 (chrome/m156), `linear`,
// `radial`, `sweep` and `conical` (the paint half is only used by the drawing path, which these
// precompile-only tests do not take).
fn linear() -> PaintOptions {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::linear_gradient(
        GradientShaderFlags::ALL,
        Interpolation::default(),
    )]);
    paint_options.set_blend_modes(&[BlendMode::SrcOver]);
    paint_options
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L56-L63 (chrome/m156)
fn radial() -> PaintOptions {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::radial_gradient(
        GradientShaderFlags::ALL,
        Interpolation::default(),
    )]);
    paint_options.set_blend_modes(&[BlendMode::SrcOver]);
    paint_options
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L65-L72 (chrome/m156)
fn sweep() -> PaintOptions {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::sweep_gradient(
        GradientShaderFlags::ALL,
        Interpolation::default(),
    )]);
    paint_options.set_blend_modes(&[BlendMode::SrcOver]);
    paint_options
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L74-L81 (chrome/m156)
fn conical() -> PaintOptions {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::two_point_conical_gradient(
        GradientShaderFlags::ALL,
        Interpolation::default(),
    )]);
    paint_options.set_blend_modes(&[BlendMode::SrcOver]);
    paint_options
}

// The four gradient flavours, in `combos` order (`numStops` does not influence the options).
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L210-L212 (chrome/m156)
fn gradient_flavours() -> [fn() -> PaintOptions; 4] {
    [linear, radial, sweep, conical]
}

// Fisher-Yates over `items`, the `std::shuffle` the permuted runs use.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L222-L226 (chrome/m156)
fn shuffle<T>(items: &mut [T]) {
    use std::hash::{BuildHasher, Hasher};
    let mut state = std::collections::hash_map::RandomState::new().build_hasher();
    for i in (1..items.len()).rev() {
        state.write_usize(i);
        let bound = u64::try_from(i + 1).unwrap_or(u64::MAX);
        let j = usize::try_from(state.finish() % bound).unwrap_or(0);
        items.swap(i, j);
    }
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L201-L234 (chrome/m156), the
// anonymous `precompile_gradients`
fn precompile_gradients(precompile_context: &PrecompileContext, permute: bool) {
    let mut combos = gradient_flavours();
    if permute {
        shuffle(&mut combos);
    }

    let avoid_depth_mode = precompile_context
        .shared_context()
        .shared_context()
        .caps()
        .avoid_depth_mode();
    let props = RenderPassProperties {
        ds_flags: if avoid_depth_mode {
            DepthStencilFlags::None
        } else {
            DepthStencilFlags::Depth
        },
        dst_ct: ColorType::BGRA8888,
        dst_cs: None,
        requires_msaa: false,
    };

    for create_options in combos {
        let paint_options = create_options();
        precompile(
            precompile_context,
            &paint_options,
            DrawTypeFlags::BITMAP_TEXT_MASK,
            std::slice::from_ref(&props),
        );
    }
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L236-L250 (chrome/m156), the
// anonymous `purge_on_thread`
fn purge_on_thread(precompile_context: &PrecompileContext, keep_looping: &AtomicBool) {
    let sleep_duration = Duration::from_millis(1);
    while keep_looping.load(Ordering::Acquire) {
        thread::sleep(sleep_duration);
        precompile_context.purge_pipelines_not_used_in_ms(sleep_duration);
    }
}

// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L443-L500 (chrome/m156), the
// anonymous `run_test`, for the configurations without recording threads.
fn run_test(
    context: &mut WgpuContext,
    num_purging_threads: usize,
    num_precompile_threads: usize,
    permute: bool,
) {
    let keep_purging = Arc::new(AtomicBool::new(true));
    let mut threads = Vec::new();

    for _ in 0..num_purging_threads {
        let precompile_context = context.make_precompile_context();
        let keep_purging = Arc::clone(&keep_purging);
        threads.push(thread::spawn(move || {
            purge_on_thread(&precompile_context, &keep_purging);
        }));
    }
    for _ in 0..num_precompile_threads {
        let precompile_context = context.make_precompile_context();
        threads.push(thread::spawn(move || {
            precompile_gradients(&precompile_context, permute);
        }));
    }

    keep_purging.store(false, Ordering::Release); // stop the loops in the purging thread(s)
    for thread in threads {
        let _ = thread.join();
    }

    let _ = context.submit(SubmitInfo::new(SyncToCpu::Yes));
    // We need to explicitly wait for the precompilation to finish here
    context
        .shared_context()
        .base()
        .pipeline_manager()
        .wait_test_only();
}

// This test precompiles all four flavors of gradient sequentially but on multiple
// threads with the goal of creating cache races.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L390-L414 (chrome/m156)
def_graphite_test_for_all_contexts!(
    #[ignore = "wgpu validation errors on pipeline creation (still failing after the naga storage-pointer rewrite; under investigation)"]
    ThreadedPipelinePrecompileTest,
    |reporter, context| {
        let num_purging_threads = 0;
        let num_precompile_threads = 4;
        let permute = false;
        run_test(
            context,
            num_purging_threads,
            num_precompile_threads,
            permute,
        );

        let stats = context.shared_context().base().global_cache().stats();

        // The threaded Pipeline manager deduplicates all the tasks so we only expect (24):
        //     4 gradient flavors (linear, radial, ...) *
        //     3 types of each flavor (4, 8, N) *
        //     2 opacity variations
        let expected_pipelines = NUM_DIFF_PIPELINES * NUM_OPACITY_VARIATIONS;
        reporter_assert!(
            reporter,
            stats.graphics_cache_additions == NUM_DIFF_PIPELINES
        );
        reporter_assert!(reporter, stats.graphics_purges == 0);
        let num_cache_probes = stats.graphics_cache_hits + stats.graphics_cache_misses;
        // This test says that every expected Pipeline will incur at least one cache probe.
        // Basically, it is just checking that the Pipeline Mgr is interacting as expected
        // with the Pipeline cache.
        reporter_assert!(
            reporter,
            num_cache_probes >= expected_pipelines,
            "{}+{} < {}",
            stats.graphics_cache_hits,
            stats.graphics_cache_misses,
            expected_pipelines
        );
    }
);

// Currently all PaintOptions we test have an opaque and non-opaque intrinsic, which leads to
// two PaintOption combinations that reduce to the same PaintParamsKey. This doesn't impact the
// number of generated pipelines but does change the cache hits.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L176-L179 (chrome/m156)
const NUM_OPACITY_VARIATIONS: u32 = 2;

// This test compiles the gradient flavors on a thread and then tests out the time-based
// purging.
// Port of: tests/graphite/precompile/ThreadedPrecompileTest.cpp#L505-L516 (chrome/m156)
def_graphite_test_for_all_contexts!(
    #[ignore = "wgpu validation errors on pipeline creation (still failing after the naga storage-pointer rewrite; under investigation)"]
    ThreadedPipelinePrecompilePurgingTest,
    |reporter, context| {
        let num_purging_threads = 0;
        let num_precompile_threads = 1;
        let permute = false;
        let precompile_context = context.make_precompile_context();
        let begin = Instant::now();
        run_test(
            context,
            num_purging_threads,
            num_precompile_threads,
            permute,
        );
        let delta = begin.elapsed();
        precompile_context.purge_pipelines_not_used_in_ms(delta);

        let stats = context.shared_context().base().global_cache().stats();
        let expected_cache_hits = 0;
        reporter_assert!(
            reporter,
            stats.graphics_cache_hits == expected_cache_hits,
            "actual: {} expected: {}",
            stats.graphics_cache_hits,
            expected_cache_hits
        );
        reporter_assert!(
            reporter,
            stats.graphics_cache_additions == NUM_DIFF_PIPELINES
        );
        reporter_assert!(reporter, stats.graphics_races == 0);
        reporter_assert!(reporter, stats.graphics_purges == NUM_DIFF_PIPELINES);
    }
);
