// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G10a: draws recorded on a Graphite `Device` over wgpu's noop adapter, and the `DrawPass` they
//! become: command order, the pipeline set (checked through the G6 WGSL machinery: every
//! pipeline of the pass makes valid WGSL), and the de-duplication of uniform data, textures and
//! pipelines.
//!
//! The noop adapter renders nothing, so everything here is about what the recorder holds before
//! the GPU would run it. Each test runs on the three capability profiles of the context tests
//! (the noop device's own, Dawn on D3D12 and Dawn on Vulkan), with the sort-based draw list and,
//! where it matters, with the layer-based one.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::collections::HashSet;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_pass::DrawPassCommand;
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderSharedContext};
use skia_rust_gpu::graphite::resource_types::{DstReadStrategy, LoadOp, StoreOp};
use skia_rust_gpu::graphite::task::Task;
use skia_rust_gpu::graphite::wgpu::pipeline_shaders::make_pipeline_shaders;
use skia_rust_gpu::graphite::wgpu::{
    CapsProfile, WgpuContext, WgpuSharedContext, make_context, noop_backend_context,
};

use support::wgsl_corpus::{Recorded, all_steps, render_pass_desc};

/// What a device recorded: the tasks of its `DrawTask`, and the passes of its render pass tasks.
#[derive(Debug)]
struct Snapped {
    task_names: Vec<&'static str>,
    passes: Vec<PassInfo>,
}

#[derive(Debug)]
struct PassInfo {
    commands: Vec<DrawPassCommand>,
    descs: Vec<GraphicsPipelineDesc>,
    sampled_textures: usize,
    ops: (LoadOp, StoreOp),
    clear_color: [f32; 4],
}

impl PassInfo {
    fn count(&self, matches: impl Fn(&DrawPassCommand) -> bool) -> usize {
        self.commands.iter().filter(|c| matches(c)).count()
    }

    fn pipeline_binds(&self) -> usize {
        self.count(|c| matches!(c, DrawPassCommand::BindGraphicsPipeline { .. }))
    }

    fn uniform_binds(&self) -> usize {
        self.count(|c| matches!(c, DrawPassCommand::BindUniformBuffer { .. }))
    }

    fn texture_binds(&self) -> usize {
        self.count(|c| matches!(c, DrawPassCommand::BindTexturesAndSamplers { .. }))
    }

    // The draws the commands make: instances of the instanced draws, one per other draw.
    fn drawn(&self) -> u32 {
        self.commands
            .iter()
            .map(|c| match c {
                DrawPassCommand::Draw { .. }
                | DrawPassCommand::DrawIndexed { .. }
                | DrawPassCommand::DrawIndirect { .. }
                | DrawPassCommand::DrawIndexedIndirect { .. } => 1,
                DrawPassCommand::DrawInstanced { instance_count, .. }
                | DrawPassCommand::DrawIndexedInstanced { instance_count, .. } => *instance_count,
                _ => 0,
            })
            .sum()
    }

    // The pipeline descriptions in the order the commands bind them.
    fn bound_descs(&self) -> Vec<GraphicsPipelineDesc> {
        self.commands
            .iter()
            .filter_map(|c| match c {
                DrawPassCommand::BindGraphicsPipeline { pipeline_index } => {
                    Some(self.descs[*pipeline_index as usize])
                }
                _ => None,
            })
            .collect()
    }

    // The scissors the commands set.
    fn scissors(&self) -> Vec<IRect> {
        self.commands
            .iter()
            .filter_map(|c| match c {
                DrawPassCommand::SetScissor { scissor } => {
                    Some(scissor.get_rect(IPoint::new(0, 0), IRect::from_wh(i32::MAX, i32::MAX)))
                }
                _ => None,
            })
            .collect()
    }
}

// The contexts of the tests: the noop device's own caps, and what Dawn reported on D3D12 and
// Vulkan (as the G8/G9 context tests do), with the sort-based (`false`) or the layer-based
// (`true`) draw list.
fn contexts(use_draw_list_layer: bool) -> Vec<(String, WgpuContext)> {
    let options = ContextOptions {
        use_draw_list_layer,
        ..ContextOptions::default()
    };
    let mut contexts = vec![(
        "wgpu-noop".to_owned(),
        make_context(&noop_backend_context(), &options).expect("a context on the noop device"),
    )];
    for profile in [
        CapsProfile::dawn_d3d12(),
        CapsProfile::dawn_vulkan().wgpu_restricted(),
    ] {
        let shared_context =
            WgpuSharedContext::make_with_profile(&noop_backend_context(), &profile, &options)
                .expect("a shared context on the noop device");
        contexts.push((profile.name, WgpuContext::new(shared_context, &options)));
    }
    contexts
}

fn make_device(recorder: &Recorder, size: i32) -> Device {
    Device::make_with_info(
        Some(recorder),
        &ImageInfo::new_n32_premul((size, size), None),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "DrawPassTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

// Snaps what `device` recorded into a `DrawTask` and reads its passes.
fn snap(device: &mut Device) -> Snapped {
    let task = device
        .testing_only_snap_draw_task()
        .expect("the device recorded something");
    let guard = task.lock();
    let Task::Draw(draw_task) = &*guard else {
        panic!("the device snaps a DrawTask");
    };
    let mut snapped = Snapped {
        task_names: Vec::new(),
        passes: Vec::new(),
    };
    draw_task.child_tasks().visit(|child, _| {
        let child = child.lock();
        snapped.task_names.push(child.task_name());
        if let Task::RenderPass(render_pass) = &*child {
            for pass in render_pass.draw_passes() {
                snapped.passes.push(PassInfo {
                    commands: pass.commands().to_vec(),
                    descs: pass.pipeline_descs().to_vec(),
                    sampled_textures: pass.sampled_textures().len(),
                    ops: pass.ops(),
                    clear_color: pass.clear_color(),
                });
            }
        }
    });
    snapped
}

fn solid(r: f32, g: f32, b: f32, aa: bool) -> Paint {
    let mut paint = Paint::new(Color4f::new(r, g, b, 1.0), None);
    paint.set_anti_alias(aa);
    paint
}

fn gradient_paint(aa: bool, stops: usize) -> Paint {
    #[allow(clippy::cast_precision_loss)]
    let colors: Vec<Color4f> = (0..stops)
        .map(|i| {
            Color4f::new(
                i as f32 / stops as f32,
                0.5,
                1.0 - i as f32 / stops as f32,
                1.0,
            )
        })
        .collect();
    let desc = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None::<ColorSpace>),
        Interpolation::default(),
    );
    let shader =
        shaders::linear_gradient((Point::new(0.0, 0.0), Point::new(64.0, 0.0)), &desc, None)
            .expect("a gradient");
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
    paint.set_shader(shader);
    paint.set_anti_alias(aa);
    paint
}

fn rect(l: f32, t: f32, r: f32, b: f32) -> SkRect {
    SkRect::new(l, t, r, b)
}

fn storage_buffers(context: &WgpuContext) -> bool {
    ContextPriv::caps(context).storage_buffer_support()
}

// Parses and validates `wgsl` with naga, with every capability on (the shaders use `enable f16`,
// `var<immediate>` and `@blend_src`).
fn naga_validate(wgsl: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(wgsl).map_err(|e| e.emit_to_string(wgsl))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

// What naga says about a storage buffer pointer function argument, a Tint-valid shape that naga
// rejects (`docs/design/gpu.md` §6.3; pinned by tests/wgsl_pipelines.rs).
const KNOWN_NAGA_LIMITATION: &str = "InvalidArgumentPointerSpace";

#[test]
fn pixel_aligned_rects_with_one_paint_make_one_instanced_draw() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            // Different colors share the paint's key, and so the pipeline.
            device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
            device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &solid(0.0, 0.0, 1.0, true));
            device.draw_rect(&rect(0.0, 20.0, 16.0, 36.0), &solid(1.0, 0.0, 0.0, false));

            let snapped = snap(&mut device);
            assert_eq!(snapped.task_names, ["RenderPass Task"], "{name}");
            let [pass] = &snapped.passes[..] else {
                panic!("{name}: one pass");
            };
            assert_eq!(pass.ops, (LoadOp::Clear, StoreOp::Store), "{name}");
            assert_eq!(pass.descs.len(), 1, "{name}");
            assert_eq!(pass.pipeline_binds(), 1, "{name}");
            assert_eq!(pass.drawn(), 3, "{name}");
            // The first command is the scissor of the whole target.
            assert!(
                matches!(pass.commands[0], DrawPassCommand::SetScissor { .. }),
                "{name}"
            );
            assert_eq!(pass.scissors(), [IRect::from_wh(64, 64)], "{name}");
        }
    }
}

#[test]
fn sorted_draw_list_draws_opaque_rects_front_to_back() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        // Disjoint, opaque, filled rects don't blend with what came before them, so they sort
        // front to back (by reversed depth) and the pipelines alternate in reverse order.
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &gradient_paint(true, 2));
        device.draw_rect(&rect(40.0, 0.0, 56.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(0.0, 20.0, 16.0, 36.0), &gradient_paint(true, 2));

        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        assert_eq!(pass.descs.len(), 2, "{name}");
        let bound = pass.bound_descs();
        assert_eq!(bound.len(), 4, "{name}");
        // The last recorded draw (the gradient) comes first.
        assert_eq!(bound[0], bound[2], "{name}");
        assert_eq!(bound[1], bound[3], "{name}");
        assert_ne!(bound[0], bound[1], "{name}");
        let solid_desc = pass
            .descs
            .iter()
            .copied()
            .min_by_key(|desc| desc.paint_params_id().as_uint())
            .expect("a desc");
        // The first paint that made a key is the solid color's.
        assert_eq!(bound[1], solid_desc, "{name}");
        assert_eq!(pass.drawn(), 4, "{name}");
    }
}

#[test]
fn layered_draw_list_batches_disjoint_draws_by_pipeline() {
    for (name, context) in contexts(true) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &gradient_paint(true, 2));
        device.draw_rect(&rect(40.0, 0.0, 56.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(0.0, 20.0, 16.0, 36.0), &gradient_paint(true, 2));

        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        // The four disjoint draws share a layer, whose binding lists are the two pipelines.
        assert_eq!(pass.descs.len(), 2, "{name}");
        assert_eq!(pass.pipeline_binds(), 2, "{name}");
        assert_eq!(pass.drawn(), 4, "{name}");
    }
}

#[test]
fn overlapping_blended_draws_keep_painters_order() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let tag = format!("{name} layer={use_draw_list_layer}");
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            // Fractional coverage at the edges makes the draws blend with what is under them, so
            // overlapping draws must stay in the order they were recorded in: the pipelines do
            // not batch.
            device.draw_rect(&rect(0.5, 0.5, 30.25, 30.25), &solid(1.0, 0.0, 0.0, true));
            device.draw_rect(&rect(10.5, 10.5, 40.25, 40.25), &gradient_paint(true, 2));
            device.draw_rect(&rect(20.5, 20.5, 50.25, 50.25), &solid(0.0, 1.0, 0.0, true));

            let snapped = snap(&mut device);
            let [pass] = &snapped.passes[..] else {
                panic!("{tag}: one pass");
            };
            assert_eq!(pass.descs.len(), 2, "{tag}");
            let bound = pass.bound_descs();
            assert_eq!(bound.len(), 3, "{tag}");
            assert_eq!(bound[0], bound[2], "{tag}");
            assert_ne!(bound[0], bound[1], "{tag}");
            // The solid color is recorded first, and made the first key.
            let first_key = pass
                .descs
                .iter()
                .map(|desc| desc.paint_params_id().as_uint())
                .min()
                .expect("a desc");
            assert_eq!(bound[0].paint_params_id().as_uint(), first_key, "{tag}");
            assert_eq!(pass.drawn(), 3, "{tag}");
        }
    }
}

#[test]
fn identical_uniforms_share_a_binding() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let storage = storage_buffers(&context);
        let mut device = make_device(&recorder, 64);
        // Three draws of the same color: the paint's uniform data is the same block.
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(40.0, 0.0, 56.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        let same = snap(&mut device);
        let [same] = &same.passes[..] else {
            panic!("{name}: one pass");
        };

        // Three draws of three colors: three blocks.
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &solid(0.0, 1.0, 0.0, true));
        device.draw_rect(&rect(40.0, 0.0, 56.0, 16.0), &solid(0.0, 0.0, 1.0, true));
        let different = snap(&mut device);
        let [different] = &different.passes[..] else {
            panic!("{name}: one pass");
        };

        assert_eq!(same.drawn(), 3, "{name}");
        assert_eq!(different.drawn(), 3, "{name}");
        if storage {
            // With storage buffers, one binding holds every block and each draw carries the index
            // of its block.
            assert_eq!(same.uniform_binds(), 1, "{name}");
            assert_eq!(different.uniform_binds(), 1, "{name}");
        } else {
            // With uniform buffers, a block is a binding.
            assert_eq!(same.uniform_binds(), 1, "{name}");
            assert_eq!(different.uniform_binds(), 3, "{name}");
        }
    }
}

#[test]
fn a_texture_used_by_many_draws_is_bound_once() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            if storage_buffers(&context) {
                // Stops come from the storage buffer: no texture to share.
                continue;
            }
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            // A gradient with more than eight stops reads its colors from a cached bitmap, so
            // draws of one gradient sample the same proxy: one entry in the pass's textures,
            // one binding for its draws.
            let paint = gradient_paint(true, 12);
            device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &paint);
            device.draw_rect(&rect(20.0, 0.0, 36.0, 16.0), &paint);
            device.draw_rect(&rect(40.0, 0.0, 56.0, 16.0), &paint);

            let snapped = snap(&mut device);
            let [pass] = &snapped.passes[..] else {
                panic!("{name}: one pass");
            };
            assert_eq!(pass.sampled_textures, 1, "{name}");
            assert_eq!(pass.texture_binds(), 1, "{name}");
            assert_eq!(pass.drawn(), 3, "{name}");
        }
    }
}

#[test]
fn every_pipeline_of_a_pass_makes_valid_wgsl() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
            device.draw_rect(&rect(20.5, 0.5, 36.25, 16.25), &solid(1.0, 0.0, 0.0, true));
            device.draw_rect(&rect(20.0, 20.0, 36.0, 36.0), &gradient_paint(true, 3));
            device.draw_rect(&rect(0.5, 20.5, 16.25, 36.25), &gradient_paint(true, 12));
            let mut multiply = solid(0.5, 0.5, 0.5, true);
            // (A blend that reads the dst flushes the pass: see the dst-copy test.)
            multiply.set_blend_mode(BlendMode::Plus);
            device.draw_rect(&rect(40.5, 0.5, 56.25, 16.25), &multiply);
            let mut stroke = solid(0.0, 0.0, 1.0, true);
            stroke.set_stroke(true);
            stroke.set_stroke_width(3.0);
            device.draw_rrect(
                &skia_rust_core::rrect::RRect::new_rect_xy(rect(2.0, 40.0, 30.0, 60.0), 4.0, 4.0),
                &stroke,
            );

            let snapped = snap(&mut device);
            let [pass] = &snapped.passes[..] else {
                panic!("{name}: one pass");
            };
            assert!(
                pass.descs.len() >= 5,
                "{name}: {:?} drawn {}",
                pass.descs,
                pass.drawn()
            );

            let shared_context = context.shared_context();
            let provider = RecorderSharedContext::renderer_provider(&**shared_context);
            let steps = all_steps(provider);
            let caps = shared_context.caps();
            let mut rp_desc = render_pass_desc(caps, false);
            rp_desc.dst_read_strategy = caps.get_dst_read_strategy();
            let runtime_dict = recorder.priv_().runtime_effect_dictionary();
            let mut seen = HashSet::new();
            for desc in &pass.descs {
                assert!(seen.insert(*desc), "{name}: a pipeline is listed once");
                let (step_name, step) = steps
                    .iter()
                    .find(|(_, step)| step.render_step_id() == desc.render_step_id())
                    .unwrap_or_else(|| panic!("{name}: a step for {desc:?}"));
                let handler = Recorded::default();
                let shaders = make_pipeline_shaders(
                    caps,
                    RecorderSharedContext::shader_code_dictionary(&**shared_context),
                    Some(runtime_dict.clone()),
                    &rp_desc,
                    step.as_ref(),
                    desc.paint_params_id(),
                    &handler,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "{name}/{step_name}: SkSL to WGSL failed: {:?}",
                        handler.0.lock().unwrap()
                    )
                });
                let mut stages = vec![("vertex", shaders.vertex_wgsl.as_str())];
                if let Some(fragment) = &shaders.fragment_wgsl {
                    stages.push(("fragment", fragment));
                }
                for (stage, wgsl) in stages {
                    match naga_validate(wgsl) {
                        Ok(()) => {}
                        Err(e) if e.contains(KNOWN_NAGA_LIMITATION) => {}
                        Err(e) => panic!("{name}/{step_name} {stage} shader: {e}\n{wgsl}"),
                    }
                }
            }
        }
    }
}

#[test]
fn a_gradient_with_many_stops_draws_in_one_render_pass() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        // More than eight stops are read from the storage buffer, or without one from a cached
        // bitmap texture. Either way the draw is one pass; the stops' upload is the recorder's
        // (the bitmap) or the storage buffer's, not a task of the device.
        device.draw_rect(&rect(0.5, 0.5, 32.5, 32.5), &gradient_paint(true, 12));
        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        assert_eq!(pass.drawn(), 1, "{name}");
        assert_eq!(snapped.task_names, ["RenderPass Task"], "{name}");
    }
}

#[test]
fn a_flood_fill_of_an_opaque_color_is_a_clear() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        device.draw_paint(&solid(1.0, 0.0, 0.0, false));
        assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        assert_eq!(pass.ops, (LoadOp::Clear, StoreOp::Store), "{name}");
        assert_eq!(pass.clear_color, [1.0, 0.0, 0.0, 1.0], "{name}");
        assert_eq!(pass.drawn(), 0, "{name}");

        // A flood fill with a gradient overwrites everything too, but needs the draw: the target
        // is discarded rather than loaded.
        device.draw_rect(&rect(0.5, 0.5, 8.5, 8.5), &solid(0.0, 1.0, 0.0, true));
        device.draw_paint(&gradient_paint(false, 2));
        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        assert_eq!(pass.ops.0, LoadOp::Discard, "{name}");
        assert_eq!(pass.drawn(), 1, "{name}");
    }
}

#[test]
fn a_device_rect_clip_is_applied_to_the_geometry() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        device.push_clip_stack();
        device.clip_rect(
            &rect(8.0, 8.0, 24.0, 24.0),
            skia_rust_core::clip_op::ClipOp::Intersect,
            false,
        );
        assert!(device.is_clip_rect(), "{name}");
        assert!(!device.is_clip_wide_open(), "{name}");
        assert_eq!(device.dev_clip_bounds(), IRect::new(8, 8, 24, 24), "{name}");
        device.draw_rect(&rect(0.5, 0.5, 40.25, 40.25), &solid(1.0, 0.0, 0.0, true));
        device.pop_clip_stack();
        assert!(device.is_clip_wide_open(), "{name}");

        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        // The clip is applied to the draw's geometry (the rect is intersected with the clip), so
        // the draw stays inside the snapped scissor of the clip (4, 4, 28, 28) and needs no
        // scissor of its own.
        assert_eq!(pass.scissors(), [IRect::from_wh(64, 64)], "{name}");
        assert_eq!(pass.drawn(), 1, "{name}");

        // A draw outside the clip records nothing.
        device.push_clip_stack();
        device.clip_rect(
            &rect(8.0, 8.0, 24.0, 24.0),
            skia_rust_core::clip_op::ClipOp::Intersect,
            false,
        );
        device.draw_rect(&rect(40.0, 40.0, 60.0, 60.0), &solid(1.0, 0.0, 0.0, true));
        assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
        device.pop_clip_stack();
    }
}

#[test]
fn a_blend_that_reads_the_dst_copies_the_target() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let strategy = ContextPriv::caps(&context).get_dst_read_strategy();
        let mut device = make_device(&recorder, 64);
        device.draw_rect(&rect(0.5, 0.5, 30.25, 30.25), &solid(1.0, 0.0, 0.0, true));
        let mut overlay = solid(0.5, 0.5, 0.5, true);
        overlay.set_blend_mode(BlendMode::Overlay);
        device.draw_rect(&rect(10.5, 10.5, 40.25, 40.25), &overlay);

        let snapped = snap(&mut device);
        let [pass] = &snapped.passes[..] else {
            panic!("{name}: one pass");
        };
        if strategy == DstReadStrategy::TextureCopy {
            // The draw that reads the dst flushes the earlier draw into the recorder, and its
            // pass starts with a copy of the target.
            assert_eq!(
                snapped.task_names,
                ["Copy TtoT Task", "RenderPass Task"],
                "{name}"
            );
            assert_eq!(pass.drawn(), 1, "{name}");
        } else {
            assert_eq!(snapped.task_names, ["RenderPass Task"], "{name}");
            assert_eq!(pass.drawn(), 2, "{name}");
        }
    }
}

#[test]
fn the_recorder_snaps_what_its_devices_recorded() {
    for (name, context) in contexts(false) {
        let mut recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        // `Recorder::snap()` flushes every tracked device.
        let mut recording = recorder.snap().expect("a recording");
        assert!(recording.priv_().has_tasks(), "{name}");
        assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
    }
}

#[test]
fn a_dropped_device_hands_its_work_to_the_recorder() {
    for (name, context) in contexts(false) {
        let mut recorder = context.make_recorder(None);
        {
            let mut device = make_device(&recorder, 64);
            device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        }
        let mut recording = recorder.snap().expect("a recording");
        assert!(recording.priv_().has_tasks(), "{name}");
    }
}

#[test]
fn draws_after_the_recorder_is_gone_are_dropped() {
    for (name, context) in contexts(false) {
        let recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        drop(recorder);
        device.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
    }
}

#[test]
fn a_layer_device_is_a_registered_device_of_the_recorder() {
    for (name, context) in contexts(false) {
        let mut recorder = context.make_recorder(None);
        let mut device = make_device(&recorder, 64);
        let info = skia_rust_core::device::CreateInfo::new(
            ImageInfo::new_n32_premul((32, 32), None),
            skia_rust_core::surface_props::PixelGeometry::Unknown,
        );
        let mut layer = device.create_device(&info, None).expect("a layer device");
        assert_eq!(layer.state().width(), 32, "{name}");
        layer.draw_rect(&rect(0.0, 0.0, 16.0, 16.0), &solid(1.0, 0.0, 0.0, true));
        drop(layer);
        let mut recording = recorder.snap().expect("a recording");
        assert!(recording.priv_().has_tasks(), "{name}");
    }
}
