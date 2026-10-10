// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G12a: the path atlases (`PathAtlas`, `RasterPathAtlas`), the `AtlasProvider`, the clip atlas
//! (`ClipAtlasManager`) and the path renderer strategy, checked on wgpu's noop adapter.
//!
//! The atlas draws are recorded on a `Device` and the passes they bind are checked: an atlas path
//! draw binds the `CoverageMask` render step only, and a cache hit returns the entry of the first
//! draw. The noop adapter renders nothing, so the pixels are checked in `path_atlas_pixels.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashSet;
use std::sync::Arc;

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::color::Color4f;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::IRect;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::atlas_provider::AtlasProvider;
use skia_rust_gpu::graphite::clip_atlas_manager::ClipAtlasManager;
use skia_rust_gpu::graphite::clip_stack::ClipElement;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_pass::DrawPassCommand;
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_gpu::graphite::geom::shape::Shape;
use skia_rust_gpu::graphite::geom::transform::Transform;
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::path_atlas::PathAtlas;
use skia_rust_gpu::graphite::raster_path_atlas::RasterPathAtlas;
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderSharedContext};
use skia_rust_gpu::graphite::render_step::RenderStepID;
use skia_rust_gpu::graphite::renderer::Renderer;
use skia_rust_gpu::graphite::renderer_provider::{PathRendererStrategy, RendererProvider};
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::task::Task;
use skia_rust_gpu::graphite::wgpu::{
    WgpuContext, make_context, noop_backend_context_with_features,
};

const SIZE: i32 = 128;

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

/// A context on the noop device, with the given options.
fn context_with(options: &ContextOptions) -> WgpuContext {
    make_context(&pipeline_device(), options).expect("a context on the noop device")
}

fn context() -> WgpuContext {
    context_with(&ContextOptions::default())
}

/// The options that choose the raster path atlas (the GPU test utilities override).
fn raster_atlas_options() -> ContextOptions {
    ContextOptions {
        path_renderer_strategy: Some(PathRendererStrategy::RasterAtlas),
        ..ContextOptions::default()
    }
}

/// The options that choose the small path atlas: paths up to `min_size` use the atlas.
fn small_atlas_options(min_size: f32) -> ContextOptions {
    ContextOptions {
        minimum_path_size_for_msaa: min_size,
        ..ContextOptions::default()
    }
}

fn make_device(recorder: &Recorder) -> Device {
    Device::make_with_info(
        Some(recorder),
        &ImageInfo::new_n32_premul((SIZE, SIZE), None),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "PathAtlasTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

/// The passes of what `device` recorded: the commands and the pipeline descriptions of each.
struct Pass {
    commands: Vec<DrawPassCommand>,
    steps: Vec<RenderStepID>,
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
                        .map(GraphicsPipelineDesc::render_step_id)
                        .collect(),
                });
            }
        }
    });
    passes
}

fn record(context: &WgpuContext, draw: impl FnOnce(&mut Device)) -> Vec<Pass> {
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw(&mut device);
    let passes = snap(&mut device);
    drop(device);
    passes
}

/// The render steps the passes bind, as a set.
fn bound_steps(passes: &[Pass]) -> HashSet<RenderStepID> {
    passes
        .iter()
        .flat_map(|pass| pass.steps.iter().copied())
        .collect()
}

fn render_step_ids(renderer: &Renderer) -> HashSet<RenderStepID> {
    renderer
        .steps()
        .iter()
        .map(|step| step.render_step_id())
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
    let drawn: u32 = passes
        .iter()
        .flat_map(|pass| pass.commands.iter())
        .map(|command| match command {
            DrawPassCommand::Draw { .. }
            | DrawPassCommand::DrawIndexed { .. }
            | DrawPassCommand::DrawIndirect { .. }
            | DrawPassCommand::DrawIndexedIndirect { .. } => 1,
            DrawPassCommand::DrawInstanced { instance_count, .. }
            | DrawPassCommand::DrawIndexedInstanced { instance_count, .. } => *instance_count,
            _ => 0,
        })
        .sum();
    assert!(drawn > 0);
}

fn provider(context: &WgpuContext) -> &RendererProvider {
    let shared_context = context.shared_context();
    RecorderSharedContext::renderer_provider(&**shared_context)
}

fn fill_paint() -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    paint
}

/// A convex hexagon of `scale` times the size of the one in the target (the centre is kept).
fn hexagon(fill_type: PathFillType, scale: f32) -> Path {
    let points = [
        Point::new(64.0, 20.0),
        Point::new(104.0, 42.0),
        Point::new(104.0, 86.0),
        Point::new(64.0, 108.0),
        Point::new(24.0, 86.0),
        Point::new(24.0, 42.0),
    ];
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    let scaled = |p: Point| Point::new(64.0 + (p.x - 64.0) * scale, 64.0 + (p.y - 64.0) * scale);
    builder.move_to(scaled(points[0]));
    for point in &points[1..] {
        builder.line_to(scaled(*point));
    }
    builder.close();
    builder.detach()
}

fn stroke_rec() -> StrokeRec {
    StrokeRec::new(InitStyle::Fill)
}

/// A shape of `path` placed with the identity transform, and its device bounds.
fn shape_of(path: &Path) -> (Shape, Rect) {
    let shape = Shape::from_path(path.clone());
    let bounds = shape.bounds();
    (shape, bounds)
}

/// The raster path atlas of a recorder, through its atlas provider.
fn with_raster_atlas<R>(recorder: &Recorder, f: impl FnOnce(&mut RasterPathAtlas) -> R) -> R {
    let rp = recorder.priv_();
    let atlas_provider = rp.atlas_provider();
    let mut atlas_provider = atlas_provider.borrow_mut();
    f(atlas_provider.raster_path_atlas())
}

// A path fill with the raster atlas strategy is drawn with the coverage mask renderer only.
// Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), the atlas dispatch of chooseRenderer
#[test]
fn a_path_fill_with_the_raster_atlas_draws_the_coverage_mask() {
    let context = context_with(&raster_atlas_options());
    assert_eq!(
        provider(&context).path_renderer_strategy(),
        PathRendererStrategy::RasterAtlas
    );
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding, 1.0), &fill_paint());
    });
    assert_drawn_with(&passes, provider(&context).coverage_mask());
}

// The inverse fill of a path goes through the same atlas path.
#[test]
fn an_inverse_fill_with_the_raster_atlas_draws_the_coverage_mask() {
    let context = context_with(&raster_atlas_options());
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::InverseWinding, 1.0), &fill_paint());
    });
    assert_drawn_with(&passes, provider(&context).coverage_mask());
}

// With the small path atlas strategy a small path is atlased and a larger one is tessellated.
// Port of: src/gpu/graphite/Device.cpp#L2268-L2276 (chrome/m156), kTessellationAndSmallAtlas
#[test]
fn a_small_path_uses_the_small_path_atlas_and_a_large_one_is_tessellated() {
    let context = context_with(&small_atlas_options(100.0));
    assert_eq!(
        provider(&context).path_renderer_strategy(),
        PathRendererStrategy::TessellationAndSmallAtlas
    );

    // The hexagon is 80 by 88 pixels, within the 100 pixel limit.
    let small = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding, 1.0), &fill_paint());
    });
    assert_drawn_with(&small, provider(&context).coverage_mask());

    // Twice the size is over the limit, so it is tessellated as a convex shape.
    let large = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding, 2.0), &fill_paint());
    });
    assert_drawn_with(&large, provider(&context).convex_tessellated_wedges());
}

// The default strategy for a context without atlas caps is tessellation: no atlas draw.
#[test]
fn the_default_strategy_tessellates_a_path() {
    let context = context();
    assert_eq!(
        provider(&context).path_renderer_strategy(),
        PathRendererStrategy::Tessellation
    );
    let passes = record(&context, |device| {
        device.draw_path(&hexagon(PathFillType::Winding, 1.0), &fill_paint());
    });
    assert_drawn_with(&passes, provider(&context).convex_tessellated_wedges());
}

// A mask that is already in the atlas is found again: the second add of the same shape returns the
// same texture at the same place, without rendering it again.
// Port of: src/gpu/graphite/PathAtlas.cpp#L129-L166 (chrome/m156), the ShapeCache hit
#[test]
fn a_path_atlas_entry_is_reused_on_a_cache_hit() {
    let context = context_with(&raster_atlas_options());
    let recorder = context.make_recorder(None);
    let (shape, bounds) = shape_of(&hexagon(PathFillType::Winding, 1.0));
    let transform = Transform::identity();
    let style = stroke_rec();

    let first = with_raster_atlas(&recorder, |atlas| {
        atlas.add_shape(&recorder, &bounds, &shape, &transform, &style)
    })
    .expect("the mask fits in the atlas");
    let second = with_raster_atlas(&recorder, |atlas| {
        atlas.add_shape(&recorder, &bounds, &shape, &transform, &style)
    })
    .expect("the cached mask is found");

    let (renderer, first_mask) = first;
    assert!(std::ptr::eq(renderer, provider(&context).coverage_mask()));
    let (_, second_mask) = second;
    assert!(Arc::ptr_eq(
        first_mask.texture_proxy_arc(),
        second_mask.texture_proxy_arc()
    ));
    assert_eq!(first_mask.texture_origin(), second_mask.texture_origin());
    assert_eq!(first_mask.mask_size(), second_mask.mask_size());
    assert!(!first_mask.inverted());
}

// A different shape gets a different entry in the atlas.
#[test]
fn a_different_shape_gets_its_own_atlas_entry() {
    let context = context_with(&raster_atlas_options());
    let recorder = context.make_recorder(None);
    let transform = Transform::identity();
    let style = stroke_rec();
    let (small, small_bounds) = shape_of(&hexagon(PathFillType::Winding, 1.0));
    let (large, large_bounds) = shape_of(&hexagon(PathFillType::Winding, 1.5));

    let (_, small_mask) = with_raster_atlas(&recorder, |atlas| {
        atlas.add_shape(&recorder, &small_bounds, &small, &transform, &style)
    })
    .expect("the small mask fits");
    let (_, large_mask) = with_raster_atlas(&recorder, |atlas| {
        atlas.add_shape(&recorder, &large_bounds, &large, &transform, &style)
    })
    .expect("the large mask fits");
    assert_ne!(small_mask.texture_origin(), large_mask.texture_origin());
    assert_ne!(small_mask.mask_size(), large_mask.mask_size());
}

// The clip atlas exists only for the raster atlas strategy.
// Port of: src/gpu/graphite/AtlasProvider.cpp#L23-L27 (chrome/m156), use_clip_atlas
#[test]
fn the_clip_atlas_is_made_only_for_the_raster_atlas_strategy() {
    let context = context_with(&raster_atlas_options());
    let mut with_clip = AtlasProvider::new(&*context_caps(&context), true);
    assert!(with_clip.clip_atlas_manager().is_some());
    let mut without_clip = AtlasProvider::new(&*context_caps(&context), false);
    assert!(without_clip.clip_atlas_manager().is_none());
}

fn context_caps(context: &WgpuContext) -> Arc<dyn skia_rust_gpu::graphite::caps::Caps> {
    let caps: Arc<skia_rust_gpu::graphite::wgpu::caps::WgpuCaps> =
        Arc::clone(context.shared_context().caps());
    caps
}

// A clip mask is found again in the clip atlas: the same elements at the same bounds share an
// entry, and the second request returns the same texture and position.
// Port of: src/gpu/graphite/ClipAtlasManager.cpp#L84-L127 (chrome/m156), the entry cache
#[test]
fn a_clip_mask_is_reused_from_the_clip_atlas() {
    let context = context_with(&raster_atlas_options());
    let recorder = context.make_recorder(None);
    let element = ClipElement {
        shape: Shape::from_path(hexagon(PathFillType::Winding, 1.0)),
        local_to_device: Transform::identity(),
        op: ClipOp::Intersect,
    };
    let bounds = IRect::from_xywh(20, 16, 90, 100);

    let mut manager = ClipAtlasManager::new(&*context_caps(&context));
    let mut first_pos = IPoint::new(0, 0);
    let first = manager
        .find_or_create_entry(&recorder, 1, &[&element], bounds, &mut first_pos)
        .expect("the clip mask fits in the atlas");
    let mut second_pos = IPoint::new(0, 0);
    let second = manager
        .find_or_create_entry(&recorder, 1, &[&element], bounds, &mut second_pos)
        .expect("the cached clip mask is found");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first_pos, second_pos);

    // The mask of the same elements, but a smaller bounds inside the cached one, starts inside
    // the same entry.
    let mut inner_pos = IPoint::new(0, 0);
    let inner = manager
        .find_or_create_entry(
            &recorder,
            1,
            &[&element],
            IRect::from_xywh(30, 26, 40, 40),
            &mut inner_pos,
        )
        .expect("the inner clip mask is found");
    assert!(Arc::ptr_eq(&first, &inner));
    assert_eq!(inner_pos, IPoint::new(first_pos.x + 10, first_pos.y + 10));
}
