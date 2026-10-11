// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/PipelineCallbackTest.cpp (chrome/m156)
//
// Each test makes a new context with the callback under test (`ContextFactory` in the C++) and
// runs on a real adapter: the callbacks see the pipelines a rendering context compiles
// (`def_test!` with the adapter cfg, as `def_graphite_adapter_test!` does, because the callbacks
// need state shared with the test body, which the options-setting macros do not give).

#![cfg(test)]

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::context_options::{
    Callback, ContextOptions, PipelineCacheOp, PipelineCachingCallbackFn, PipelineCallbackFn,
};
use skia_rust_gpu::graphite::draw_types::DrawTypeFlags;
use skia_rust_gpu::graphite::graphite_types::{
    DepthStencilFlags, InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::precompile::paint_options::PaintOptions;
use skia_rust_gpu::graphite::precompile::shader::{GradientShaderFlags, PrecompileShaders};
use skia_rust_gpu::graphite::precompile_context::PrecompileContext;
use skia_rust_gpu::graphite::public_precompile::{RenderPassProperties, precompile};
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::tools::graphite_test_context::real_context_with_options;
use crate::{Reporter, def_test, reporter_assert};

/// `CallbackContext`: the serialized keys a callback has seen.
#[derive(Default)]
struct CallbackContext {
    data: Mutex<Vec<Data>>,
}

impl CallbackContext {
    fn add(&self, data: Data) {
        self.data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
    }

    fn is_empty(&self) -> bool {
        self.data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }

    fn get_keys(&self) -> Vec<Data> {
        self.data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// `PipelineCheckData`: a serialized key, its unique hash, and whether the checking callback saw
/// it added and with a matching hash.
struct PipelineCheckData {
    data: Data,
    unique_hash: u32,
    seen: bool,
    matched: bool,
}

/// `draw(Context*)`: draws a gradient-filled rect into a Graphite render target and submits it
/// synchronously. Returns whether every step succeeded.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L17-L40 (chrome/m156)
fn draw(context: &mut WgpuContext) -> bool {
    let colors = [
        Color4f::new(1.0, 1.0, 1.0, 1.0),
        Color4f::new(0.0, 0.0, 0.0, 1.0),
    ];
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    let shader = gradient_shaders::linear_gradient(
        (Point::new(0.0, 0.0), Point::new(64.0, 64.0)),
        &gradient,
        None,
    );
    let mut paint = Paint::default();
    paint.set_shader(shader);

    let mut recorder = context.make_recorder(None);
    let ii = ImageInfo::new((64, 64), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(surface) = Surface::render_target(&recorder, &ii, Mipmapped::No, None, "") else {
        return false;
    };
    surface
        .canvas()
        .draw_rect(Rect::from_ltrb(0.0, 0.0, 64.0, 64.0), &paint);
    let Some(mut recording) = recorder.snap() else {
        return false;
    };
    if context.insert_recording(InsertRecordingInfo::new(&mut recording)) != InsertStatus::Success {
        return false;
    }
    context.submit(SubmitInfo::new(SyncToCpu::Yes))
}

/// `run_normal_test`: a context with the callback draws, and the callback saw a pipeline.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L87-L96 (chrome/m156)
fn run_normal_test(reporter: &mut Reporter, options: &ContextOptions, sink: &CallbackContext) {
    let Some((_, mut context)) = real_context_with_options(options) else {
        return;
    };
    reporter_assert!(reporter, draw(&mut context));
    // some Pipeline should've been reported
    reporter_assert!(reporter, !sink.is_empty());
}

/// `precompile_linear_gradient`: precompiles the small linear gradient for rects and rrects.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L115-L126 (chrome/m156)
fn precompile_linear_gradient(precompile_context: &PrecompileContext) {
    let mut paint_options = PaintOptions::default();
    paint_options.set_shaders(&[PrecompileShaders::linear_gradient(
        GradientShaderFlags::SMALL,
        Interpolation::default(),
    )]);
    let rect_and_rrect = DrawTypeFlags::NON_AA_FILL_RECT | DrawTypeFlags::ANALYTIC_RRECT;
    let props = RenderPassProperties {
        ds_flags: DepthStencilFlags::Depth,
        dst_ct: ColorType::RGBA8888,
        dst_cs: None,
        requires_msaa: false,
    };
    precompile(
        precompile_context,
        &paint_options,
        rect_and_rrect,
        std::slice::from_ref(&props),
    );
}

/// `run_precompile_test`: a context with the callback precompiles a gradient, which the callback
/// saw.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L128-L140 (chrome/m156)
fn run_precompile_test(reporter: &mut Reporter, options: &ContextOptions, sink: &CallbackContext) {
    let Some((_, context)) = real_context_with_options(options) else {
        return;
    };
    let precompile_context = context.make_precompile_context();
    precompile_linear_gradient(&precompile_context);
    // We need to explicitly wait for the precompilation to finish here
    context
        .shared_context()
        .base()
        .pipeline_manager()
        .wait_test_only();
    // some Pipeline should've been reported
    reporter_assert!(reporter, !sink.is_empty());
}

// Smoke test for the deprecated ContextOptions::PipelineCallback for normal Pipelines.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L185-L201 (chrome/m156)
def_test!(
    #[cfg_attr(
        not(skia_rust_adapter_tests),
        ignore = "needs a real adapter in CI (lavapipe job)"
    )]
    PipelineCallbackTest_Deprecated_Normal,
    |reporter| {
        let sink = Arc::new(CallbackContext::default());
        let callback_sink = Arc::clone(&sink);
        let deprecated: Arc<PipelineCallbackFn> =
            Arc::new(move |serialized_key: &Data| callback_sink.add(serialized_key.clone()));
        let options = ContextOptions {
            pipeline_callback: Some(Callback(deprecated)),
            ..ContextOptions::default()
        };
        run_normal_test(reporter, &options, &sink);
    }
);

// Smoke test for the ContextOptions::PipelineCachingCallback for normal Pipelines.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L204-L220 (chrome/m156)
def_test!(
    #[cfg_attr(
        not(skia_rust_adapter_tests),
        ignore = "needs a real adapter in CI (lavapipe job)"
    )]
    PipelineCallbackTest_Normal,
    |reporter| {
        let sink = Arc::new(CallbackContext::default());
        let callback_sink = Arc::clone(&sink);
        let caching: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |_op: PipelineCacheOp,
                  label: &str,
                  _hash: u32,
                  from_precompile: bool,
                  serialized_key: Option<&Data>| {
                assert_ne!(label, "");
                assert!(!from_precompile);
                let key = serialized_key.expect("a serialized key");
                callback_sink.add(key.clone());
            },
        );
        let options = ContextOptions {
            pipeline_caching_callback: Some(Callback(caching)),
            ..ContextOptions::default()
        };
        run_normal_test(reporter, &options, &sink);
    }
);

// Smoke test for the deprecated ContextOptions::PipelineCallback for Precompile Pipelines.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L224-L240 (chrome/m156)
def_test!(
    #[cfg_attr(
        not(skia_rust_adapter_tests),
        ignore = "needs a real adapter in CI (lavapipe job)"
    )]
    PipelineCallbackTest_Deprecated_Precompile,
    |reporter| {
        let sink = Arc::new(CallbackContext::default());
        let callback_sink = Arc::clone(&sink);
        let deprecated: Arc<PipelineCallbackFn> =
            Arc::new(move |serialized_key: &Data| callback_sink.add(serialized_key.clone()));
        let options = ContextOptions {
            pipeline_callback: Some(Callback(deprecated)),
            ..ContextOptions::default()
        };
        run_precompile_test(reporter, &options, &sink);
    }
);

// Smoke test for the ContextOptions::PipelineCachingCallback for Precompile Pipelines.
// Port of: tests/graphite/PipelineCallbackTest.cpp#L243-L259 (chrome/m156)
def_test!(
    #[cfg_attr(
        not(skia_rust_adapter_tests),
        ignore = "needs a real adapter in CI (lavapipe job)"
    )]
    PipelineCallbackTest_Precompile,
    |reporter| {
        let sink = Arc::new(CallbackContext::default());
        let callback_sink = Arc::clone(&sink);
        let caching: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |op: PipelineCacheOp,
                  label: &str,
                  _hash: u32,
                  from_precompile: bool,
                  serialized_key: Option<&Data>| {
                assert_ne!(label, "");
                assert!(from_precompile);
                if op == PipelineCacheOp::AddingPipeline {
                    let key = serialized_key.expect("a serialized key");
                    callback_sink.add(key.clone());
                } else {
                    // Some PaintOption combinatorics produce the same paint key (e.g. opacity
                    // variations that lead to no net change). In these cases, we get cache hits
                    // on subsequent callbacks.
                    assert_eq!(op, PipelineCacheOp::PipelineFound);
                }
            },
        );
        let options = ContextOptions {
            pipeline_caching_callback: Some(Callback(caching)),
            ..ContextOptions::default()
        };
        run_precompile_test(reporter, &options, &sink);
    }
);

// Port of: tests/graphite/PipelineCallbackTest.cpp#L340-L371 (chrome/m156)
def_test!(
    #[cfg_attr(
        not(skia_rust_adapter_tests),
        ignore = "needs a real adapter in CI (lavapipe job)"
    )]
    PipelineCallbackTest_UniqueHash,
    |reporter| {
        // This creates a new Context, precompiles some Pipelines and puts the serialized keys
        // in context
        let sink = Arc::new(CallbackContext::default());
        let callback_sink = Arc::clone(&sink);
        let caching: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |op: PipelineCacheOp,
                  label: &str,
                  _hash: u32,
                  from_precompile: bool,
                  serialized_key: Option<&Data>| {
                assert_ne!(label, "");
                assert!(from_precompile);
                if op == PipelineCacheOp::AddingPipeline {
                    let key = serialized_key.expect("a serialized key");
                    callback_sink.add(key.clone());
                } else {
                    assert_eq!(op, PipelineCacheOp::PipelineFound);
                }
            },
        );
        let options = ContextOptions {
            pipeline_caching_callback: Some(Callback(caching)),
            ..ContextOptions::default()
        };
        run_precompile_test(reporter, &options, &sink);

        // The generating Context has been destroyed, all we have are the serialized keys in
        // 'context'
        let orig_keys = sink.get_keys();
        let verification_data = Arc::new(Mutex::new(Vec::<PipelineCheckData>::new()));

        // This calls getPipelineLabel on all the serialized keys in 'origKeys' then precompiles
        // them to verify that the uniqueHashs match.
        let checking_data = Arc::clone(&verification_data);
        let checking: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |op: PipelineCacheOp,
                  label: &str,
                  unique_key_hash: u32,
                  from_precompile: bool,
                  serialized_key: Option<&Data>| {
                assert_ne!(label, "");
                assert!(from_precompile);
                let mut data = checking_data.lock().unwrap_or_else(PoisonError::into_inner);
                if op == PipelineCacheOp::AddingPipeline {
                    let key = serialized_key.expect("a serialized key");
                    for d in data.iter_mut() {
                        if d.data.equals(Some(key)) {
                            d.seen = true;
                            d.matched = d.unique_hash == unique_key_hash;
                        }
                    }
                } else {
                    assert_eq!(op, PipelineCacheOp::PipelineFound);
                    // On found pipelines, serializedKey is null so we can only search for the hash
                    let hash_match = data.iter().any(|d| d.unique_hash == unique_key_hash);
                    assert!(hash_match);
                }
            },
        );
        let options = ContextOptions {
            pipeline_caching_callback: Some(Callback(checking)),
            ..ContextOptions::default()
        };

        // run_checking_test: first get the label and uniqueHash for each serialized key in
        // 'origKeys' and fill up 'verificationData'
        let Some((_, context)) = real_context_with_options(&options) else {
            return;
        };
        let precompile_context = context.make_precompile_context();
        {
            let mut data = verification_data
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            for key in &orig_keys {
                let mut unique_hash = 0u32;
                let _label =
                    precompile_context.get_pipeline_label(Some(key), Some(&mut unique_hash));
                data.push(PipelineCheckData {
                    data: key.clone(),
                    unique_hash,
                    seen: false,
                    matched: false,
                });
            }
        }
        // Next, precompile the Pipelines like usual but the uniqueHashes will be checked in the
        // checking callback.
        precompile_linear_gradient(&precompile_context);
        // We need to explicitly wait for the precompilation to finish here
        context
            .shared_context()
            .base()
            .pipeline_manager()
            .wait_test_only();

        // Verify that all the uniqueHashs matched
        let data = verification_data
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for d in data.iter() {
            reporter_assert!(reporter, d.seen && d.matched);
        }
    }
);
