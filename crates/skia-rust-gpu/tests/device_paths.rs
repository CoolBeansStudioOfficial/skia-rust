// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G10c: path draws recorded on a Graphite `Device` over wgpu's noop adapter. Each test checks
//! which renderer `Device::chooseRenderer` picked (through the render steps of the pipelines the
//! pass binds) and what the `DrawPass` holds: fills, inverse fills, strokes, stroke-and-fills and
//! hairlines.
//!
//! The noop adapter renders nothing, so the checks are about what the recorder holds before the
//! GPU would run it. The pixel checks of the same draws are in `device_path_pixels.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashSet;

use skia_rust_core::color::Color4f;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_pass::DrawPassCommand;
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderSharedContext};
use skia_rust_gpu::graphite::render_step::RenderStepID;
use skia_rust_gpu::graphite::renderer::Renderer;
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::resource_types::{LoadOp, StoreOp};
use skia_rust_gpu::graphite::task::Task;
use skia_rust_gpu::graphite::wgpu::{
    WgpuContext, make_context, noop_backend_context_with_features,
};

/// A noop device that can create the pipelines of the draws.
fn pipeline_device() -> skia_rust_gpu::graphite::wgpu::WgpuBackendContext {
    noop_backend_context_with_features(
        wgpu::Features::IMMEDIATES | wgpu::Features::DUAL_SOURCE_BLENDING,
        wgpu::Limits {
            max_immediate_size: 64,
            ..wgpu::Limits::default()
        },
    )
    .expect("the noop backend creates a device")
}

fn context() -> WgpuContext {
    make_context(&pipeline_device(), &ContextOptions::default())
        .expect("a context on the noop device")
}

const SIZE: i32 = 128;

fn make_device(recorder: &Recorder) -> Device {
    Device::make_with_info(
        Some(recorder),
        &ImageInfo::new_n32_premul((SIZE, SIZE), None),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "DevicePathsTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

/// The passes of what `device` recorded: the commands and the pipeline descriptions of each.
struct Pass {
    commands: Vec<DrawPassCommand>,
    steps: Vec<RenderStepID>,
    ops: (LoadOp, StoreOp),
}

impl Pass {
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

    fn pipeline_binds(&self) -> usize {
        self.commands
            .iter()
            .filter(|c| matches!(c, DrawPassCommand::BindGraphicsPipeline { .. }))
            .count()
    }
}

/// Snaps what `device` recorded and reads the passes of its render pass tasks.
fn snap(device: &mut Device) -> Vec<Pass> {
    let task = device
        .testing_only_snap_draw_task()
        .expect("the device recorded something");
    let guard = task.lock();
    let Task::Draw(draw_task) = &*guard else {
        panic!("the device snaps a DrawTask");
    };
    let mut passes = Vec::new();
    draw_task.child_tasks().visit(|child, _| {
        let child = child.lock();
        if let Task::RenderPass(render_pass) = &*child {
            for pass in render_pass.draw_passes() {
                passes.push(Pass {
                    commands: pass.commands().to_vec(),
                    steps: pass
                        .pipeline_descs()
                        .iter()
                        .map(|desc| desc.render_step_id())
                        .collect(),
                    ops: pass.ops(),
                });
            }
        }
    });
    passes
}

fn render_step_ids(renderer: &Renderer) -> HashSet<RenderStepID> {
    renderer
        .steps()
        .iter()
        .map(|step| step.render_step_id())
        .collect()
}

/// The render steps the passes bind, as a set.
fn bound_steps(passes: &[Pass]) -> HashSet<RenderStepID> {
    passes
        .iter()
        .flat_map(|pass| pass.steps.iter().copied())
        .collect()
}

/// Checks that the draws of `passes` are exactly the steps of `renderer`, and that they draw.
fn assert_drawn_with(passes: &[Pass], renderer: &Renderer) {
    assert!(!passes.is_empty(), "the draw recorded a pass");
    assert_eq!(
        bound_steps(passes),
        render_step_ids(renderer),
        "the pipelines are the steps of {}",
        renderer.name()
    );
    assert!(passes.iter().map(Pass::drawn).sum::<u32>() > 0);
}

fn provider(context: &WgpuContext) -> &RendererProvider {
    let shared_context = context.shared_context();
    RecorderSharedContext::renderer_provider(&**shared_context)
}

fn paint(style: Style, width: f32) -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(true);
    paint.set_style(style);
    paint.set_stroke_width(width);
    paint
}

/// A convex hexagon, centred in the target.
fn hexagon(fill_type: PathFillType) -> Path {
    let points = [
        Point::new(64.0, 20.0),
        Point::new(104.0, 42.0),
        Point::new(104.0, 86.0),
        Point::new(64.0, 108.0),
        Point::new(24.0, 86.0),
        Point::new(24.0, 42.0),
    ];
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.close();
    builder.detach()
}

/// A five-pointed star, which is not convex.
fn star(fill_type: PathFillType) -> Path {
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    for i in 0..10 {
        let radius = if i % 2 == 0 { 50.0_f32 } else { 20.0 };
        // Only the angle of each vertex matters: five spikes, the odd ones inside.
        let angle = std::f32::consts::PI / 5.0 * i as f32 - std::f32::consts::FRAC_PI_2;
        let p = Point::new(64.0 + radius * angle.cos(), 64.0 + radius * angle.sin());
        if i == 0 {
            builder.move_to(p);
        } else {
            builder.line_to(p);
        }
    }
    builder.close();
    builder.detach()
}

fn record(context: &WgpuContext, draw: impl FnOnce(&mut Device)) -> Vec<Pass> {
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw(&mut device);
    let passes = snap(&mut device);
    drop(device);
    passes
}

// A filled convex path is tessellated as wedges of its convex outline (`chooseMSAARenderer`).
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `convexTessellatedWedges`
#[test]
fn a_convex_path_fill_chooses_the_convex_wedges() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding), &paint(Style::Fill, 0.0));
    });
    assert_drawn_with(&passes, provider(&context).convex_tessellated_wedges());
    assert_eq!(passes[0].ops, (LoadOp::Clear, StoreOp::Store));
    assert_eq!(passes[0].pipeline_binds(), 1);
}

// A concave fill of a small path prefers the stencil wedges to the curves and triangles.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `preferWedges`
#[test]
fn a_concave_path_fill_chooses_the_stencil_wedges() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&star(PathFillType::Winding), &paint(Style::Fill, 0.0));
    });
    assert_drawn_with(
        &passes,
        provider(&context).stencil_tessellated_wedges(PathFillType::Winding),
    );
}

// The even-odd rule has its own stencil steps, so the pipelines differ from the winding ones.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `fillType` of the stencil
#[test]
fn an_even_odd_path_fill_chooses_the_even_odd_stencil_wedges() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&star(PathFillType::EvenOdd), &paint(Style::Fill, 0.0));
    });
    assert_drawn_with(
        &passes,
        provider(&context).stencil_tessellated_wedges(PathFillType::EvenOdd),
    );
    let winding = render_step_ids(
        provider(&context).stencil_tessellated_wedges(PathFillType::Winding),
    );
    // The cover step is shared by both rules; the tessellated wedges are not.
    assert!(
        !bound_steps(&passes).is_subset(&winding),
        "the even-odd pipelines are not the winding ones"
    );
}

// An inverse fill is never convex-wedged: it takes the stencil wedges with the inverse cover.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `shape.inverted()`
#[test]
fn an_inverse_fill_chooses_the_inverse_stencil_wedges() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(
            &hexagon(PathFillType::InverseWinding),
            &paint(Style::Fill, 0.0),
        );
    });
    assert_drawn_with(
        &passes,
        provider(&context).stencil_tessellated_wedges(PathFillType::InverseWinding),
    );
    assert!(
        bound_steps(&passes).contains(&RenderStepID::CoverBounds_InverseCover),
        "the inverse fill covers the outside of the path"
    );
}

// A stroke is tessellated by the stroke renderer, whatever its outline.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kStroke_Style`
#[test]
fn a_stroked_path_chooses_the_tessellated_strokes() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding), &paint(Style::Stroke, 4.0));
    });
    assert_drawn_with(&passes, provider(&context).tessellated_strokes(false));
}

// A zero-width stroke is a hairline, which the same stroke renderer draws.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kHairline_Style`
#[test]
fn a_hairline_path_chooses_the_tessellated_strokes() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding), &paint(Style::Stroke, 0.0));
    });
    assert_drawn_with(&passes, provider(&context).tessellated_strokes(false));
}

// A stroke-and-fill draws its fill with the fill renderer and its stroke with the stroke one.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kStrokeAndFill_Style`
#[test]
fn a_stroke_and_fill_draws_the_fill_and_the_stroke() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(
            &hexagon(PathFillType::Winding),
            &paint(Style::StrokeAndFill, 4.0),
        );
    });
    let provider = provider(&context);
    let fill = render_step_ids(provider.convex_tessellated_wedges());
    let stroke = render_step_ids(provider.tessellated_strokes(false));
    let bound = bound_steps(&passes);
    assert!(fill.is_subset(&bound), "the fill is drawn");
    assert!(stroke.is_subset(&bound), "the stroke is drawn");
    assert!(
        bound.is_subset(&fill.union(&stroke).copied().collect()),
        "nothing else is drawn"
    );
}

// Every path draw is recorded in the one render pass of the device, in painters order.
#[test]
fn path_draws_share_one_render_pass_in_order() {
    let context = context();
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding), &paint(Style::Fill, 0.0));
        device.draw_path(&star(PathFillType::Winding), &paint(Style::Stroke, 2.0));
    });
    let [pass] = &passes[..] else {
        panic!("one render pass, found {}", passes.len());
    };
    assert_eq!(pass.ops, (LoadOp::Clear, StoreOp::Store));
    assert!(pass.drawn() >= 2);
    let scissors: Vec<IRect> = pass
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawPassCommand::SetScissor { scissor } => Some(
                scissor.get_rect(IPoint::new(0, 0), IRect::from_wh(i32::MAX, i32::MAX)),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(scissors, [IRect::from_wh(SIZE, SIZE)]);
}
