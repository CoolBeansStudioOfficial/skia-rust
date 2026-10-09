// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/ClipStack.cpp

//! G10b: the `ClipStack`.
//!
//! The expectations are derived from the control flow of `ClipStack.cpp` (and `KeyHelpers.cpp`
//! for the analytic clip's block), worked out by hand for each scenario, not read back from the
//! port. The first half drives the stack directly with a recording stand-in for the `Device`: the
//! element tree after save/restore sequences, `visitClipStackForDraw`, `updateClipStateForDraw`
//! and `recordDeferredClipDraws`. The second half records clipped draws on a headless `Device`
//! (wgpu's noop adapter, the three capability profiles of the other context tests, both draw
//! lists) and checks the depth-only clip draws in the `DrawPass`. The third half is the key and
//! the uniforms of the analytic clip.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::m44::M44;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::color_shader::ColorShader;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::clip_stack::{
    ClipAtlasManager, ClipDrawHooks, ClipElement, ClipStack, ClipState, ElementList, PixelSnapping,
};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_list_base::DrawListBase;
use skia_rust_gpu::graphite::draw_list_types::{DrawParamsId, LayerId};
use skia_rust_gpu::graphite::draw_order::{CompressedPaintersOrder, DrawOrder, PaintersDepth};
use skia_rust_gpu::graphite::draw_params::Clip;
use skia_rust_gpu::graphite::draw_pass::DrawPassCommand;
use skia_rust_gpu::graphite::draw_types::DstUsage;
use skia_rust_gpu::graphite::geom::bounds_manager::NaiveBoundsManager;
use skia_rust_gpu::graphite::geom::geometry::Geometry;
use skia_rust_gpu::graphite::geom::non_msaa_clip::{AnalyticClip, AtlasClip, NonMSAAClip};
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_gpu::graphite::geom::shape::Shape;
use skia_rust_gpu::graphite::geom::transform::{Transform, Type as TransformType};
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::paint_params::{PaintParams, ShadingParams};
use skia_rust_gpu::graphite::paint_params_key::PaintParamsKeyBuilder;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderSharedContext};
use skia_rust_gpu::graphite::render_step::Coverage;
use skia_rust_gpu::graphite::resource_types::{Layout, LoadOp};
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::task::Task;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::wgpu::pipeline_shaders::make_pipeline_shaders;
use skia_rust_gpu::graphite::wgpu::{
    CapsProfile, WgpuContext, WgpuSharedContext, make_context, noop_backend_context,
};
use support::MockCaps;
use support::wgsl_corpus::{Recorded, all_steps, render_pass_desc};

// ===========================================================================================
// The stack, driven directly
// ===========================================================================================

/// One depth-only clip draw that the stack asked its device for.
#[derive(Debug, Clone)]
struct ClipDraw {
    shape: Shape,
    local_to_device: Transform,
    draw_bounds: Rect,
    transformed_shape_bounds: Rect,
    scissor: IRect,
    order: DrawOrder,
}

/// What the `Device` does for the stack, recorded.
#[derive(Default)]
struct Hooks {
    draws: Vec<ClipDraw>,
    depth_updates: Vec<PaintersDepth>,
}

impl ClipDrawHooks for Hooks {
    fn draw_clip_shape(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    ) {
        self.draws.push(ClipDraw {
            shape: shape.clone(),
            local_to_device: *local_to_device,
            draw_bounds: clip.draw_bounds(),
            transformed_shape_bounds: clip.transformed_shape_bounds(),
            scissor: clip.scissor(),
            order,
        });
    }

    fn draw_clip_shape_immediate(
        &mut self,
        _local_to_device: &Transform,
        _shape: &Shape,
        _clip: &Clip,
        _order: DrawOrder,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        unreachable!("the sort-based draw list records clip draws when they are deferred")
    }

    fn update_next_depth_for_clipping(&mut self, depth: PaintersDepth) {
        self.depth_updates.push(depth);
    }

    fn use_draw_list_layer(&self) -> bool {
        false
    }

    fn update_clip_draw(
        &mut self,
        _params: DrawParamsId,
        _order: DrawOrder,
        _draw_bounds: Rect,
        _scissor: IRect,
    ) {
        unreachable!("only the layer-based draw list captures clip draws")
    }

    fn layer_order(&self, _layer: LayerId) -> Option<CompressedPaintersOrder> {
        None
    }
}

const SIZE: i32 = 100;

fn sk(l: f32, t: f32, r: f32, b: f32) -> SkRect {
    SkRect::new(l, t, r, b)
}

fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
    Rect::new(l, t, r, b)
}

fn rect_shape(l: f32, t: f32, r: f32, b: f32) -> Shape {
    Shape::from_rect(rect(l, t, r, b))
}

fn path_shape(points: &[(f32, f32)]) -> Shape {
    let points: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y)).collect();
    Shape::from_path(Path::polygon(&points, true, None, None))
}

fn clip_shape(stack: &mut ClipStack, hooks: &mut Hooks, shape: &Shape, op: ClipOp) {
    stack.clip_shape(hooks, &Transform::identity(), shape, op, PixelSnapping::No);
}

fn fill() -> StrokeRec {
    StrokeRec::new(InitStyle::Fill)
}

/// A shear `x' = x + y / 2 + 10`: an affine transform that is not rect-stays-rect and whose
/// inverse is exact in floats.
fn shear() -> Transform {
    let mut m = M44::new_identity();
    m.set_rc(0, 1, 0.5);
    m.set_rc(0, 3, 10.0);
    Transform::new(m)
}

fn all_elements(stack: &ClipStack) -> Vec<ClipElement> {
    stack.elements().cloned().collect()
}

#[test]
fn a_new_stack_is_wide_open() {
    let stack = ClipStack::new(SIZE, SIZE);
    assert_eq!(stack.clip_state(), ClipState::WideOpen);
    assert_eq!(stack.conservative_bounds(), rect(0.0, 0.0, 100.0, 100.0));
    assert_eq!(stack.elements().count(), 0);
    assert_eq!(stack.save_record_count(), 1);
    assert_eq!(stack.max_deferred_clip_draws(), 0);
}

#[test]
fn a_device_rect_replaces_the_wide_open_clip() {
    // Simplify(save(wide open), rect): the save record's inner bounds contain the rect, so only
    // the rect (kBOnly) is needed; the record replaces itself with the element, whose bounds are
    // the rect's, and the record's state mirrors the element's: an identity-transformed
    // intersect rect is a device rect. The element is inverse-filled (intersect clips draw
    // depth where they clip out).
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );

    assert_eq!(stack.clip_state(), ClipState::DeviceRect);
    assert_eq!(stack.conservative_bounds(), rect(10.0, 10.0, 50.0, 50.0));
    let elements = all_elements(&stack);
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0].op, ClipOp::Intersect);
    assert!(elements[0].shape.is_rect());
    assert!(elements[0].shape.inverted());
    assert_eq!(elements[0].local_to_device.type_(), TransformType::Identity);
    assert_eq!(*elements[0].shape.rect(), rect(10.0, 10.0, 50.0, 50.0));
    let (inner, outer) = stack.save_record_bounds();
    assert_eq!(inner, rect(10.0, 10.0, 50.0, 50.0));
    assert_eq!(outer, rect(10.0, 10.0, 50.0, 50.0));
    assert!(hooks.draws.is_empty());
}

#[test]
fn rect_stays_rect_transforms_are_applied_to_the_element() {
    // The constructor of RawElement maps a rect by a rect-stays-rect transform, clips it to the
    // device and stores it with an identity transform.
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    stack.clip_shape(
        &mut hooks,
        &Transform::translate(20.0, 30.0),
        &rect_shape(0.0, 0.0, 100.0, 100.0),
        ClipOp::Intersect,
        PixelSnapping::No,
    );

    assert_eq!(stack.clip_state(), ClipState::DeviceRect);
    // (20, 30, 120, 130) clipped to the device.
    assert_eq!(stack.conservative_bounds(), rect(20.0, 30.0, 100.0, 100.0));
    let elements = all_elements(&stack);
    assert_eq!(elements[0].local_to_device.type_(), TransformType::Identity);
    assert_eq!(*elements[0].shape.rect(), rect(20.0, 30.0, 100.0, 100.0));
}

#[test]
fn pixel_snapping_rounds_device_rects() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    stack.clip_shape(
        &mut hooks,
        &Transform::identity(),
        &rect_shape(10.25, 10.75, 49.5, 50.4),
        ClipOp::Intersect,
        PixelSnapping::Yes,
    );

    // Rect::round() rounds each edge to the nearest integer.
    assert_eq!(stack.conservative_bounds(), rect(10.0, 11.0, 50.0, 50.0));
    assert_eq!(stack.clip_state(), ClipState::DeviceRect);
}

#[test]
fn nested_intersecting_rects_combine_and_restore() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );
    stack.save();
    // The deferred save is not a record yet.
    assert_eq!(stack.save_record_count(), 1);

    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(20.0, 20.0, 60.0, 60.0),
        ClipOp::Intersect,
    );

    // Simplify(save(10..50), rect(20..60)) is kBoth. The stack's bounds become the intersection.
    // Then the older element is compared with the new one: kBoth again, and since both are
    // intersect rects in the same space, `combine()` intersects the new element with the old
    // (the old one gets invalidated, the new one is the clip).
    assert_eq!(stack.save_record_count(), 2);
    assert_eq!(stack.element_stack_len(), 2);
    assert!(stack.element_is_invalid(0));
    assert!(!stack.element_is_invalid(1));
    assert_eq!(stack.clip_state(), ClipState::DeviceRect);
    assert_eq!(stack.conservative_bounds(), rect(20.0, 20.0, 50.0, 50.0));
    let elements = all_elements(&stack);
    assert_eq!(elements.len(), 1, "invalid elements are skipped");
    assert_eq!(*elements[0].shape.rect(), rect(20.0, 20.0, 50.0, 50.0));
    assert!(elements[0].shape.inverted());

    // restore() removes the record's element and revalidates the older one.
    stack.restore(&mut hooks);
    assert_eq!(stack.save_record_count(), 1);
    assert_eq!(stack.element_stack_len(), 1);
    assert!(!stack.element_is_invalid(0));
    assert_eq!(stack.clip_state(), ClipState::DeviceRect);
    assert_eq!(stack.conservative_bounds(), rect(10.0, 10.0, 50.0, 50.0));
    assert!(hooks.draws.is_empty(), "no draw used the elements");
}

#[test]
fn a_clip_that_changes_nothing_adds_no_save_record() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    stack.save();
    // Larger than the device: Simplify(save(wide open), rect) is kAOnly.
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(-10.0, -10.0, 200.0, 200.0),
        ClipOp::Intersect,
    );
    assert_eq!(stack.save_record_count(), 1);
    assert_eq!(stack.element_stack_len(), 0);
    assert_eq!(stack.clip_state(), ClipState::WideOpen);
    // The save() is still deferred, so restore() just undoes it.
    stack.restore(&mut hooks);
    assert_eq!(stack.save_record_count(), 1);
}

#[test]
fn a_difference_that_covers_the_clip_makes_it_empty() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );
    // Simplify(intersect save, difference rect): the difference's inner bounds contain the
    // save record's outer bounds, so nothing remains (kEmpty); the elements are removed.
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(0.0, 0.0, 100.0, 100.0),
        ClipOp::Difference,
    );
    assert_eq!(stack.clip_state(), ClipState::Empty);
    assert!(stack.conservative_bounds().is_empty_negative_or_nan());
    assert_eq!(stack.elements().count(), 0);
    assert_eq!(stack.element_stack_len(), 0);

    // Everything is clipped out, and later clips are ignored.
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(0.0, 0.0, 5.0, 5.0),
        ClipOp::Intersect,
    );
    assert_eq!(stack.clip_state(), ClipState::Empty);
    let mut geometry = Geometry::Shape(rect_shape(1.0, 1.0, 2.0, 2.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    assert!(clip.is_clipped_out());
    assert_eq!(effective, ElementList::new());
}

#[test]
fn an_empty_intersect_empties_the_clip_and_an_empty_difference_is_ignored() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    // An empty difference changes nothing.
    stack.save();
    stack.clip_shape(
        &mut hooks,
        &Transform::identity(),
        &Shape::default(),
        ClipOp::Difference,
        PixelSnapping::No,
    );
    assert_eq!(stack.clip_state(), ClipState::WideOpen);
    assert_eq!(stack.save_record_count(), 1);

    // A rect that is off the device is an empty shape; intersecting with it empties the clip, in
    // a record of its own.
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(200.0, 200.0, 300.0, 300.0),
        ClipOp::Intersect,
    );
    assert_eq!(stack.clip_state(), ClipState::Empty);
    assert_eq!(stack.save_record_count(), 2);
    stack.restore(&mut hooks);
    assert_eq!(stack.clip_state(), ClipState::WideOpen);
    assert_eq!(stack.save_record_count(), 1);
}

#[test]
fn a_difference_on_a_wide_open_clip_is_complex_with_device_bounds() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(20.0, 20.0, 30.0, 30.0),
        ClipOp::Difference,
    );
    // Only intersect clips of identity-transformed rects are device rects.
    assert_eq!(stack.clip_state(), ClipState::Complex);
    // A hole is not representable as a rectangle: `subtract(deviceBounds, inner, exact)` keeps
    // the device bounds.
    assert_eq!(stack.conservative_bounds(), rect(0.0, 0.0, 100.0, 100.0));
    let elements = all_elements(&stack);
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0].op, ClipOp::Difference);
    assert!(!elements[0].shape.inverted());

    // A difference that cuts off an edge of the device is representable.
    let mut stack = ClipStack::new(SIZE, SIZE);
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(0.0, 0.0, 100.0, 40.0),
        ClipOp::Difference,
    );
    assert_eq!(stack.conservative_bounds(), rect(0.0, 40.0, 100.0, 100.0));
}

#[test]
fn rrects_and_rotated_rects_keep_their_kind() {
    let mut hooks = Hooks::default();
    // A round rect under a rect-stays-rect transform is transformed in place: a device rrect.
    let mut stack = ClipStack::new(SIZE, SIZE);
    let rrect = RRect::new_rect_xy(sk(10.0, 10.0, 50.0, 50.0), 8.0, 8.0);
    clip_shape(
        &mut stack,
        &mut hooks,
        &Shape::from_rrect(rrect),
        ClipOp::Intersect,
    );
    assert_eq!(stack.clip_state(), ClipState::DeviceRRect);
    let elements = all_elements(&stack);
    assert!(elements[0].shape.is_rrect());
    assert_eq!(elements[0].local_to_device.type_(), TransformType::Identity);
    let (inner, outer) = stack.save_record_bounds();
    assert_eq!(outer, rect(10.0, 10.0, 50.0, 50.0));
    assert!(!inner.is_empty_negative_or_nan() && outer.contains(inner));
    assert_ne!(inner, outer, "the corners cut the inner bounds");

    // A skewed rect keeps its transform and is complex; its bounds are the mapped bounds.
    let mut stack = ClipStack::new(SIZE, SIZE);
    stack.clip_shape(
        &mut hooks,
        &shear(),
        &rect_shape(0.0, 0.0, 20.0, 20.0),
        ClipOp::Intersect,
        PixelSnapping::No,
    );
    assert_eq!(stack.clip_state(), ClipState::Complex);
    let elements = all_elements(&stack);
    assert_eq!(elements[0].local_to_device.type_(), TransformType::Affine);
    assert_eq!(*elements[0].shape.rect(), rect(0.0, 0.0, 20.0, 20.0));
    assert_eq!(stack.conservative_bounds(), rect(10.0, 0.0, 40.0, 20.0));
}

#[test]
fn the_clip_shader_makes_the_clip_complex_and_is_restored() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    let shader = Shader::from_base(ColorShader::new(Color4f::new(1.0, 0.0, 0.0, 1.0)));
    stack.save();
    stack.clip_shader(shader);
    assert!(stack.clip_shader_ref().is_some());
    // SaveRecord::state() reports kComplex for a clip with a shader (unless empty).
    assert_eq!(stack.clip_state(), ClipState::Complex);
    assert_eq!(stack.save_record_count(), 2);
    // A geometric clip in the same record inherits the shader.
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );
    assert!(stack.clip_shader_ref().is_some());
    stack.restore(&mut hooks);
    assert!(stack.clip_shader_ref().is_none());
    assert_eq!(stack.clip_state(), ClipState::WideOpen);
}

#[test]
fn a_draw_in_a_wide_open_clip_is_unaffected() {
    let stack = ClipStack::new(SIZE, SIZE);
    let mut geometry = Geometry::Shape(rect_shape(10.5, 10.5, 40.25, 40.25));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    // Simplify(save(wide open), draw) is kBOnly (the clip does not influence the draw), so the
    // draw is clipped only by the scissor, which is the device.
    assert_eq!(effective, ElementList::new());
    assert!(!clip.is_clipped_out());
    assert_eq!(clip.draw_bounds(), rect(10.5, 10.5, 40.25, 40.25));
    assert_eq!(
        clip.transformed_shape_bounds(),
        rect(10.5, 10.5, 40.25, 40.25)
    );
    assert_eq!(clip.scissor(), IRect::new(0, 0, 100, 100));
    assert!(clip.non_msaa_clip().is_empty());
    assert!(!clip.needs_coverage());
    let Geometry::Shape(shape) = &geometry else {
        panic!("the geometry is still a shape");
    };
    assert_eq!(*shape.rect(), rect(10.5, 10.5, 40.25, 40.25));
}

#[test]
fn degenerate_draws_are_clipped_out() {
    let stack = ClipStack::new(SIZE, SIZE);
    let mut effective = ElementList::new();
    // A zero-width fill, a non-finite rect, and a zero-length butt-capped stroke.
    for shape in [
        rect_shape(10.0, 10.0, 10.0, 40.0),
        rect_shape(0.0, 0.0, f32::INFINITY, 40.0),
    ] {
        let mut geometry = Geometry::Shape(shape);
        let clip = stack.visit_clip_stack_for_draw(
            &Transform::identity(),
            &mut geometry,
            &fill(),
            &mut effective,
            None,
        );
        assert!(clip.is_clipped_out());
    }
}

#[test]
fn a_draw_outside_the_clip_is_clipped_out() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );
    let mut geometry = Geometry::Shape(rect_shape(60.0, 60.0, 90.0, 90.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    // The scissor is the clip's outer bounds snapped to a multiple of 4: (4, 4, 56, 56). The
    // draw's outer bounds intersected with it are empty.
    assert!(clip.is_clipped_out());
}

#[test]
fn a_device_rect_clip_is_applied_to_the_geometry_of_a_rect_draw() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(20.0, 20.0, 60.0, 60.0),
        ClipOp::Intersect,
    );
    let mut geometry = Geometry::Shape(rect_shape(0.5, 0.5, 40.25, 40.25));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );

    // The scissor is the clip's outer bounds outset by 3 and rounded out to a multiple of 4:
    // (17, 17, 63, 63) -> (16, 16, 64, 64). The draw and the clip are in the same space and the
    // draw is a plain rect, so `intersect_shape` intersects the clip into the draw's geometry,
    // and no element is left over.
    assert_eq!(effective, ElementList::new());
    assert_eq!(clip.scissor(), IRect::new(16, 16, 64, 64));
    let Geometry::Shape(shape) = &geometry else {
        panic!("the geometry is still a shape");
    };
    assert_eq!(*shape.rect(), rect(20.0, 20.0, 40.25, 40.25));
    assert_eq!(clip.draw_bounds(), rect(20.0, 20.0, 40.25, 40.25));
    assert_eq!(
        clip.transformed_shape_bounds(),
        rect(20.0, 20.0, 40.25, 40.25)
    );
    assert!(clip.non_msaa_clip().is_empty());
}

#[test]
fn an_edge_aa_quad_keeps_its_type_when_the_clip_is_applied_to_it() {
    use skia_rust_gpu::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags};

    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(20.0, 20.0, 60.0, 60.0),
        ClipOp::Intersect,
    );
    let quad = EdgeAAQuad::from_rect(rect(0.5, 0.5, 40.25, 40.25), Flags::NONE);
    let mut geometry = Geometry::EdgeAAQuad(quad);
    let mut effective = ElementList::new();
    let _ = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    // The quad's left and top edges are cut by the clip, so they turn their AA on (the clip
    // supplies the edge coverage); the others, which are inside the clip, stay as they were.
    let Geometry::EdgeAAQuad(quad) = &geometry else {
        panic!("a rect quad stays a quad");
    };
    assert!(quad.is_rect());
    assert_eq!(quad.bounds(), rect(20.0, 20.0, 40.25, 40.25));
    assert_eq!(quad.edge_flags(), Flags::LEFT | Flags::TOP);
}

#[test]
fn a_skewed_rect_clip_becomes_an_analytic_clip() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    stack.clip_shape(
        &mut hooks,
        &shear(),
        &rect_shape(0.0, 0.0, 20.0, 20.0),
        ClipOp::Intersect,
        PixelSnapping::No,
    );
    // The draw covers the clip's bounds entirely, so (kReplacesDraw) it becomes a flood fill and
    // the clip supplies all the coverage. The clip cannot be intersected into the geometry (the
    // relative transform is not rect-stays-rect), is not a device rect, and is an affine rect:
    // `can_apply_analytic_clip`.
    let mut geometry = Geometry::Shape(rect_shape(0.0, 0.0, 60.0, 60.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    assert_eq!(effective, ElementList::new());
    let Geometry::Shape(shape) = &geometry else {
        panic!("the geometry is a shape");
    };
    assert!(shape.is_flood_fill());

    let analytic = clip.non_msaa_clip().analytic_clip;
    assert!(!analytic.is_empty());
    assert!(clip.needs_coverage());
    // device-to-local 2x2 of x' = x + y/2 + 10 is [[1, -0.5], [0, 1]], stored column-major.
    assert_eq!(
        [
            analytic.xform[0],
            analytic.xform[1],
            analytic.xform[2],
            analytic.xform[3]
        ],
        [1.0, 0.0, -0.5, 1.0]
    );
    // The translation (10, 0) is folded into the rect: tx = xform.x * 10 + xform.z * 0.
    assert_eq!(analytic.bounds, sk(10.0, 0.0, 30.0, 20.0));
    assert_eq!(
        analytic.radii,
        skia_rust_simd::vx::Float4::new(0.0, 0.0, 0.0, 0.0)
    );
    assert!(analytic.inverted, "intersect clips are inverse-filled");
    // The scissor is the clip's outer bounds (10, 0, 40, 20) outset by 3, rounded out to a
    // multiple of 4 and clipped to the device: (4, 0, 44, 24). A flood fill's draw bounds are the
    // scissor.
    assert_eq!(clip.scissor(), IRect::new(4, 0, 44, 24));
    assert_eq!(clip.draw_bounds(), rect(4.0, 0.0, 44.0, 24.0));
}

#[test]
fn rrect_clips_are_analytic_when_the_corners_are_circular() {
    let mut hooks = Hooks::default();
    let triangle = path_shape(&[(0.0, 0.0), (60.0, 0.0), (0.0, 60.0)]);
    // A path cannot be intersected into, so the clip is left as a clip.
    let mut analytic = |radii: (f32, f32)| {
        let mut stack = ClipStack::new(SIZE, SIZE);
        let rrect = RRect::new_rect_xy(sk(10.0, 10.0, 50.0, 50.0), radii.0, radii.1);
        clip_shape(
            &mut stack,
            &mut hooks,
            &Shape::from_rrect(rrect),
            ClipOp::Intersect,
        );
        let mut geometry = Geometry::Shape(triangle.clone());
        let mut effective = ElementList::new();
        let clip = stack.visit_clip_stack_for_draw(
            &Transform::identity(),
            &mut geometry,
            &fill(),
            &mut effective,
            None,
        );
        (clip, effective)
    };

    // All four corners circular with radius 8.
    let (clip, effective) = analytic((8.0, 8.0));
    assert_eq!(effective, ElementList::new());
    let a = clip.non_msaa_clip().analytic_clip;
    assert_eq!(a.bounds, sk(10.0, 10.0, 50.0, 50.0));
    assert_eq!(a.radii, skia_rust_simd::vx::Float4::new(8.0, 8.0, 8.0, 8.0));
    assert_eq!(a.xform, skia_rust_simd::vx::Float4::new(1.0, 0.0, 0.0, 1.0));
    assert!(a.inverted);

    // Elliptical corners with rx = 2 ry are circular after scaling y by 2: the rect's top and
    // bottom and the device-to-local y column are scaled.
    let (clip, effective) = analytic((8.0, 4.0));
    assert_eq!(effective, ElementList::new());
    let a = clip.non_msaa_clip().analytic_clip;
    assert_eq!(a.bounds, sk(10.0, 20.0, 50.0, 100.0));
    assert_eq!(a.radii, skia_rust_simd::vx::Float4::new(8.0, 8.0, 8.0, 8.0));
    assert_eq!(a.xform, skia_rust_simd::vx::Float4::new(1.0, 0.0, 0.0, 2.0));
}

#[test]
fn rrect_corners_that_do_not_agree_are_not_analytic() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    // Upper-left is circular with radius 8, upper-right is elliptical (rx = 2 ry): no single y
    // scale makes both circular.
    let mut rrect = RRect::new_empty();
    rrect.set_rect_radii(
        sk(10.0, 10.0, 50.0, 50.0),
        &[
            Point::new(8.0, 8.0),
            Point::new(8.0, 4.0),
            Point::new(8.0, 8.0),
            Point::new(8.0, 8.0),
        ],
    );
    clip_shape(
        &mut stack,
        &mut hooks,
        &Shape::from_rrect(rrect),
        ClipOp::Intersect,
    );
    let mut geometry = Geometry::Shape(path_shape(&[(0.0, 0.0), (60.0, 0.0), (0.0, 60.0)]));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    assert!(clip.non_msaa_clip().is_empty());
    assert_eq!(effective.len(), 1, "the clip is left for a depth-only draw");
}

/// Two overlapping triangle clips, neither of which can be reduced: each is a depth-only clip
/// draw for the draws it affects.
fn two_path_clips() -> (ClipStack, Hooks) {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &path_shape(&[(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)]),
        ClipOp::Intersect,
    );
    clip_shape(
        &mut stack,
        &mut hooks,
        &path_shape(&[(30.0, 30.0), (90.0, 30.0), (30.0, 90.0)]),
        ClipOp::Intersect,
    );
    (stack, hooks)
}

#[test]
fn path_clips_stay_elements_and_become_deferred_clip_draws() {
    let (mut stack, mut hooks) = two_path_clips();
    assert_eq!(stack.clip_state(), ClipState::Complex);
    assert_eq!(stack.element_stack_len(), 2);
    assert!(!stack.element_is_invalid(0) && !stack.element_is_invalid(1));
    // The record's bounds are the intersection of the elements' bounds.
    assert_eq!(stack.conservative_bounds(), rect(30.0, 30.0, 60.0, 60.0));

    let mut geometry = Geometry::Shape(rect_shape(20.0, 20.0, 70.0, 70.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    // The draw covers the clip's bounds, so it becomes a flood fill whose draw bounds are the
    // scissor: the outer bounds (30, 30, 60, 60) outset by 3 and rounded out to 4: (24, 24, 64, 64).
    assert_eq!(clip.scissor(), IRect::new(24, 24, 64, 64));
    assert_eq!(clip.draw_bounds(), rect(24.0, 24.0, 64.0, 64.0));
    // Both paths remain, most recent first.
    assert_eq!(effective, vec![1, 0]);
    assert!(stack.element(1).shape.is_path());

    // Nothing is pending until a draw is recorded.
    assert!(!stack.element_usage(0).0);
    assert!(!stack.element_usage(1).0);

    // Recording the draw: each affected element takes the order after the most recent draw under
    // its bounds (none: `NaiveBoundsManager` starts at kNoIntersection), so the next one.
    let bounds_manager = NaiveBoundsManager::new();
    let z = DrawOrder::K_CLEAR_DEPTH.next();
    let (order, layer) =
        stack.update_clip_state_for_draw(&mut hooks, &clip, &effective, &bounds_manager, z);
    assert_eq!(order, DrawOrder::K_NO_INTERSECTION.next());
    assert_eq!(layer, None);
    for index in [0, 1] {
        let (pending, usage) = stack.element_usage(index);
        assert!(pending);
        // The usage is the draw's bounds snapped to a multiple of 4: (24, 24, 64, 64) is
        // outset by 3 and rounded out: (20, 20, 68, 68).
        assert_eq!(usage, rect(20.0, 20.0, 68.0, 68.0));
    }
    assert!(hooks.draws.is_empty(), "the clip draws are deferred");

    // Recording the deferred clip draws emits one depth-only draw per element, in stack order,
    // writing 1 + the max Z of the draws they affect.
    stack.record_deferred_clip_draws(&mut hooks);
    assert_eq!(hooks.draws.len(), 2);
    for draw in &hooks.draws {
        assert_eq!(draw.order.depth(), z.next());
        assert_eq!(draw.order.paint_order(), order);
        assert!(draw.shape.is_path());
        assert!(
            draw.shape.inverted(),
            "intersect clips draw where they clip out"
        );
        assert_eq!(draw.local_to_device.type_(), TransformType::Identity);
    }
    // Path 1 has outer bounds (10, 10, 60, 60), which snap to (4, 4, 64, 64) (outset by 3, rounded
    // out to a multiple of 4). The usage (20, 20, 68, 68) intersected with it is (20, 20, 64, 64),
    // whose area 1936 is more than half of the snapped bounds' 3600 (1800), so the scissor is the
    // snapped outer bounds.
    assert_eq!(hooks.draws[0].scissor, IRect::new(4, 4, 64, 64));
    assert_eq!(hooks.draws[0].draw_bounds, rect(4.0, 4.0, 64.0, 64.0));
    assert_eq!(
        hooks.draws[0].transformed_shape_bounds,
        rect(10.0, 10.0, 60.0, 60.0)
    );
    // Path 2: outer (30, 30, 90, 90), snapped (24, 24, 96, 96); usage ∩ snapped is (24, 24, 68,
    // 68), area 1936, which is less than half of 72 * 72 (2592): the tight scissor stays.
    assert_eq!(hooks.draws[1].scissor, IRect::new(24, 24, 68, 68));
    assert_eq!(hooks.draws[1].draw_bounds, rect(24.0, 24.0, 68.0, 68.0));
    assert_eq!(
        hooks.depth_updates.len(),
        0,
        "only the layered list updates"
    );

    // The usage is reset by drawing, so a second flush emits nothing.
    assert!(!stack.element_usage(0).0);
    stack.record_deferred_clip_draws(&mut hooks);
    assert_eq!(hooks.draws.len(), 2);
}

#[test]
fn restoring_a_record_draws_the_elements_that_were_used() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    stack.save();
    clip_shape(
        &mut stack,
        &mut hooks,
        &path_shape(&[(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)]),
        ClipOp::Intersect,
    );
    let mut geometry = Geometry::Shape(rect_shape(0.0, 0.0, 70.0, 70.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    assert_eq!(effective.len(), 1);
    let bounds_manager = NaiveBoundsManager::new();
    let _ = stack.update_clip_state_for_draw(
        &mut hooks,
        &clip,
        &effective,
        &bounds_manager,
        DrawOrder::K_CLEAR_DEPTH.next(),
    );
    assert!(hooks.draws.is_empty());

    // The element is destroyed with its save record, so the draw is recorded at that point
    // (`SaveRecord::removeElements`).
    stack.restore(&mut hooks);
    assert_eq!(hooks.draws.len(), 1);
    assert_eq!(stack.element_stack_len(), 0);
    assert_eq!(stack.clip_state(), ClipState::WideOpen);

    // A clip that no draw used records nothing when it is popped.
    stack.save();
    clip_shape(
        &mut stack,
        &mut hooks,
        &path_shape(&[(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)]),
        ClipOp::Intersect,
    );
    stack.restore(&mut hooks);
    assert_eq!(hooks.draws.len(), 1);
}

#[test]
fn a_later_draw_grows_the_usage_and_depth_of_an_element() {
    let (mut stack, mut hooks) = two_path_clips();
    let bounds_manager = NaiveBoundsManager::new();
    let run = |stack: &mut ClipStack, hooks: &mut Hooks, r: Rect, z: PaintersDepth| {
        let mut geometry = Geometry::Shape(Shape::from_rect(r));
        let mut effective = ElementList::new();
        let clip = stack.visit_clip_stack_for_draw(
            &Transform::identity(),
            &mut geometry,
            &fill(),
            &mut effective,
            None,
        );
        let result = stack.update_clip_state_for_draw(hooks, &clip, &effective, &bounds_manager, z);
        (result, effective)
    };
    let z1 = DrawOrder::K_CLEAR_DEPTH.next();
    let z2 = z1.next().next();

    // The first draw crosses the edges of both triangles (path 1's hypotenuse x + y = 70 and
    // path 2's legs at 30): both are affected. Its bounds (25, 25, 45, 45) snap to (20, 20, 48,
    // 48).
    let ((first_order, _), effective) =
        run(&mut stack, &mut hooks, rect(25.0, 25.0, 45.0, 45.0), z1);
    assert_eq!(effective, vec![1, 0]);
    assert_eq!(stack.element_usage(0).1, rect(20.0, 20.0, 48.0, 48.0));
    assert_eq!(stack.element_usage(1).1, rect(20.0, 20.0, 48.0, 48.0));

    // The second draw, (45, 45, 58, 58), is inside path 2 (x + y <= 120) but crosses path 1's
    // hypotenuse: only path 1 is affected. Its bounds snap to (40, 40, 64, 64).
    let ((second_order, _), effective) =
        run(&mut stack, &mut hooks, rect(45.0, 45.0, 58.0, 58.0), z2);
    assert_eq!(effective, vec![0]);
    // The element keeps the order assigned by its first usage, but its usage bounds are the
    // union of the draws' snapped bounds...
    assert_eq!(second_order, first_order);
    assert_eq!(stack.element_usage(0).1, rect(20.0, 20.0, 64.0, 64.0));
    assert_eq!(stack.element_usage(1).1, rect(20.0, 20.0, 48.0, 48.0));

    // ...and the clip draws write 1 + the max Z of the draws they affect.
    stack.record_deferred_clip_draws(&mut hooks);
    assert_eq!(hooks.draws.len(), 2);
    assert_eq!(hooks.draws[0].order.depth(), z2.next());
    assert_eq!(hooks.draws[1].order.depth(), z1.next());
    assert_eq!(hooks.draws[0].order.paint_order(), first_order);
}

#[test]
fn difference_clips_draw_only_the_part_inside_their_bounds() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    clip_shape(
        &mut stack,
        &mut hooks,
        &path_shape(&[(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)]),
        ClipOp::Difference,
    );
    // A difference path: the draw is large enough to contain the element, but the clip's own
    // (difference) bounds do not clip the draw out.
    let mut geometry = Geometry::Shape(rect_shape(0.0, 0.0, 80.0, 80.0));
    let mut effective = ElementList::new();
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        None,
    );
    // The scissor of a difference clip is the device.
    assert_eq!(clip.scissor(), IRect::new(0, 0, 100, 100));
    assert_eq!(effective, vec![0]);
    let bounds_manager = NaiveBoundsManager::new();
    let _ = stack.update_clip_state_for_draw(
        &mut hooks,
        &clip,
        &effective,
        &bounds_manager,
        DrawOrder::K_CLEAR_DEPTH.next(),
    );
    stack.record_deferred_clip_draws(&mut hooks);
    assert_eq!(hooks.draws.len(), 1);
    let draw = &hooks.draws[0];
    // A difference clip draws a regular fill, and only its own bounds ∩ the scissor.
    assert!(!draw.shape.inverted());
    assert_eq!(draw.draw_bounds, rect(10.0, 10.0, 60.0, 60.0));
}

#[test]
fn the_gen_id_changes_with_the_elements() {
    let mut stack = ClipStack::new(SIZE, SIZE);
    let mut hooks = Hooks::default();
    // Wide open and empty have reserved ids.
    assert_eq!(stack.gen_id(), 2);
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(10.0, 10.0, 50.0, 50.0),
        ClipOp::Intersect,
    );
    let first = stack.gen_id();
    assert!(first >= 3);
    stack.save();
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(20.0, 20.0, 60.0, 60.0),
        ClipOp::Intersect,
    );
    let second = stack.gen_id();
    assert!(second >= 3 && second != first);
    stack.restore(&mut hooks);
    assert_eq!(stack.gen_id(), first);
    clip_shape(
        &mut stack,
        &mut hooks,
        &rect_shape(60.0, 60.0, 70.0, 70.0),
        ClipOp::Intersect,
    );
    assert_eq!(stack.gen_id(), 1, "an empty clip has the reserved id");
}

/// The seam to the clip atlas: records what the stack hands it.
struct RecordingAtlas {
    calls: Vec<(u32, usize, IRect)>,
    texture: Option<Arc<TextureProxy>>,
}

impl ClipAtlasManager for RecordingAtlas {
    fn find_or_create_entry(
        &mut self,
        stack_record_id: u32,
        element_list: &[&ClipElement],
        mask_bounds: IRect,
        out_pos: &mut IPoint,
    ) -> Option<Arc<TextureProxy>> {
        self.calls
            .push((stack_record_id, element_list.len(), mask_bounds));
        *out_pos = IPoint::new(3, 5);
        self.texture.clone()
    }
}

#[test]
fn the_remaining_elements_go_to_the_clip_atlas_when_there_is_one() {
    let (stack, _hooks) = two_path_clips();
    let mut geometry = Geometry::Shape(rect_shape(20.0, 20.0, 70.0, 70.0));
    let mut effective = ElementList::new();
    // The atlas cannot place the mask: the elements stay.
    let mut atlas = RecordingAtlas {
        calls: Vec::new(),
        texture: None,
    };
    let clip = stack.visit_clip_stack_for_draw(
        &Transform::identity(),
        &mut geometry,
        &fill(),
        &mut effective,
        Some(&mut atlas),
    );
    // The mask covers the record's outer bounds, rounded out.
    assert_eq!(
        atlas.calls,
        [(stack.gen_id(), 2, IRect::new(30, 30, 60, 60))]
    );
    assert_eq!(effective.len(), 2);
    assert!(clip.non_msaa_clip().atlas_clip.is_empty());
}

// ===========================================================================================
// Clipped draws on a headless Device
// ===========================================================================================

#[derive(Debug)]
struct PassInfo {
    commands: Vec<DrawPassCommand>,
    descs: Vec<GraphicsPipelineDesc>,
}

impl PassInfo {
    // The pipelines the commands bind, in order, as "is the pipeline depth-only".
    fn bound_depth_only(&self) -> Vec<bool> {
        self.commands
            .iter()
            .filter_map(|c| match c {
                DrawPassCommand::BindGraphicsPipeline { pipeline_index } => Some(
                    !self.descs[*pipeline_index as usize]
                        .paint_params_id()
                        .is_valid(),
                ),
                _ => None,
            })
            .collect()
    }

    // The number of draws the commands make.
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
}

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
        "ClipStackTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

// Snaps what `device` recorded and reads its passes.
fn snap(device: &mut Device) -> Vec<PassInfo> {
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
                passes.push(PassInfo {
                    commands: pass.commands().to_vec(),
                    descs: pass.pipeline_descs().to_vec(),
                });
            }
        }
    });
    passes
}

fn solid(r: f32, g: f32, b: f32, aa: bool) -> Paint {
    let mut paint = Paint::new(Color4f::new(r, g, b, 1.0), None);
    paint.set_anti_alias(aa);
    paint
}

fn triangle(points: &[(f32, f32)]) -> Path {
    let mut builder = PathBuilder::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            builder.move_to(Point::new(x, y));
        } else {
            builder.line_to(Point::new(x, y));
        }
    }
    builder.close();
    builder.detach()
}

#[test]
fn clip_paths_record_depth_only_draws_before_the_draws_they_clip() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            device.push_clip_stack();
            device.clip_path(
                &triangle(&[(4.0, 4.0), (40.0, 4.0), (4.0, 40.0)]),
                ClipOp::Intersect,
                true,
            );
            device.clip_path(
                &triangle(&[(16.0, 16.0), (60.0, 16.0), (16.0, 60.0)]),
                ClipOp::Intersect,
                true,
            );
            assert_eq!(
                device.testing_only_with_clip_stack(ClipStack::element_stack_len),
                2,
                "{name}"
            );
            // Nothing was drawn by the clips themselves.
            assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");

            device.draw_rect(&sk(0.5, 0.5, 50.25, 50.25), &solid(1.0, 0.0, 0.0, true));
            // The shading draw, plus the clip draws that are recorded eagerly in the layered draw
            // list (they must exist to be ordered against the shading draw).
            let steps_after_draw = device.testing_only_pending_render_steps();
            assert!(steps_after_draw >= 1, "{name}");
            device.pop_clip_stack();

            let passes = snap(&mut device);
            let [pass] = &passes[..] else {
                panic!("{name}: one pass");
            };
            let depth_only = pass.bound_depth_only();
            // Both paths affected the draw, so both were drawn: each path is a stencil-and-cover
            // MSAA clip draw with depth-only pipelines (no paint).
            assert!(
                depth_only.iter().filter(|&&d| d).count() >= 2,
                "{name} layer={use_draw_list_layer}: {depth_only:?}"
            );
            // And exactly one shading pipeline draws the rect.
            assert_eq!(
                depth_only.iter().filter(|&&d| !d).count(),
                1,
                "{name} layer={use_draw_list_layer}: {depth_only:?}"
            );
            // The clip draws come first: the shading draw is ordered after the clips that affect
            // it (`order.dependsOnPaintersOrder(clipOrder)`).
            let first_shading = depth_only
                .iter()
                .position(|&d| !d)
                .expect("a shading pipeline");
            assert!(
                depth_only[..first_shading].iter().any(|&d| d),
                "{name} layer={use_draw_list_layer}: {depth_only:?}"
            );
            assert!(
                depth_only[first_shading..].iter().all(|&d| !d),
                "{name} layer={use_draw_list_layer}: {depth_only:?}"
            );
        }
    }
}

#[test]
fn clips_that_do_not_affect_a_draw_record_no_clip_draws() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            device.push_clip_stack();
            device.clip_path(
                &triangle(&[(40.0, 40.0), (60.0, 40.0), (40.0, 60.0)]),
                ClipOp::Intersect,
                true,
            );
            // The draw is entirely outside the clip's bounds: clipped out, nothing recorded.
            device.draw_rect(&sk(2.0, 2.0, 20.0, 20.0), &solid(1.0, 0.0, 0.0, true));
            assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
            device.pop_clip_stack();
            assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");

            // A clip that was never used by a draw is not drawn when it is popped either.
            device.push_clip_stack();
            device.clip_path(
                &triangle(&[(40.0, 40.0), (60.0, 40.0), (40.0, 60.0)]),
                ClipOp::Intersect,
                true,
            );
            device.pop_clip_stack();
            assert_eq!(device.testing_only_pending_render_steps(), 0, "{name}");
        }
    }
}

#[test]
fn a_flush_records_the_pending_clip_draws_and_the_clip_keeps_working() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            device.push_clip_stack();
            device.clip_path(
                &triangle(&[(4.0, 4.0), (60.0, 4.0), (4.0, 60.0)]),
                ClipOp::Intersect,
                true,
            );
            device.draw_rect(&sk(0.5, 0.5, 50.25, 50.25), &solid(1.0, 0.0, 0.0, true));
            // `internalFlush()` calls `recordDeferredClipDraws()`; the element stays in the stack.
            let passes = snap(&mut device);
            let [pass] = &passes[..] else {
                panic!("{name}: one pass");
            };
            let depth_only = pass.bound_depth_only();
            assert!(depth_only.iter().any(|&d| d), "{name}: {depth_only:?}");
            assert_eq!(
                device.testing_only_with_clip_stack(ClipStack::element_stack_len),
                1,
                "{name}"
            );
            assert!(
                !device.testing_only_with_clip_stack(|clip| clip.element_usage(0).0),
                "{name}: the usage was reset by the flush"
            );

            // A draw after the flush uses the element again and gets a clip draw of its own.
            device.draw_rect(&sk(1.5, 1.5, 30.25, 30.25), &solid(0.0, 1.0, 0.0, true));
            device.pop_clip_stack();
            let passes = snap(&mut device);
            let [pass] = &passes[..] else {
                panic!("{name}: one pass");
            };
            assert!(
                pass.bound_depth_only().iter().any(|&d| d),
                "{name}: the second draw has its clip draw"
            );
        }
    }
}

#[test]
fn an_analytic_clip_needs_no_depth_only_draw() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            // The same path draw with and without a circular-cornered rrect clip. The path cannot
            // be intersected with the clip, so the clip is an analytic clip in the paint's key:
            // the pipelines differ in their paint, but there are no extra (clip) draws.
            let record = |clip: bool| {
                let recorder = context.make_recorder(None);
                let mut device = make_device(&recorder, 64);
                device.push_clip_stack();
                if clip {
                    device.clip_rrect(
                        &RRect::new_rect_xy(sk(8.0, 8.0, 48.0, 48.0), 6.0, 6.0),
                        ClipOp::Intersect,
                        true,
                    );
                }
                device.draw_path(
                    &triangle(&[(0.0, 0.0), (60.0, 0.0), (0.0, 60.0)]),
                    &solid(1.0, 0.0, 0.0, true),
                );
                device.pop_clip_stack();
                assert_eq!(
                    device.testing_only_with_clip_stack(ClipStack::element_stack_len),
                    0,
                    "{name}"
                );
                let mut passes = snap(&mut device);
                assert_eq!(passes.len(), 1, "{name}: one pass");
                passes.remove(0)
            };
            let clipped: PassInfo = record(true);
            let unclipped: PassInfo = record(false);

            assert_eq!(
                clipped.bound_depth_only(),
                unclipped.bound_depth_only(),
                "{name} layer={use_draw_list_layer}: no clip draws"
            );
            assert_eq!(clipped.drawn(), unclipped.drawn(), "{name}");
            let shading_ids = |pass: &PassInfo| -> Vec<_> {
                pass.descs
                    .iter()
                    .filter(|desc| desc.paint_params_id().is_valid())
                    .map(GraphicsPipelineDesc::paint_params_id)
                    .collect()
            };
            assert_eq!(shading_ids(&clipped).len(), 1, "{name}");
            assert_eq!(shading_ids(&unclipped).len(), 1, "{name}");
        }
    }
}

#[test]
fn a_device_with_layered_draws_orders_clip_draws_into_layers() {
    let (name, context) = contexts(true).remove(0);
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder, 64);
    device.push_clip_stack();
    device.clip_path(
        &triangle(&[(4.0, 4.0), (60.0, 4.0), (4.0, 60.0)]),
        ClipOp::Intersect,
        true,
    );
    device.draw_rect(&sk(0.5, 0.5, 50.25, 50.25), &solid(1.0, 0.0, 0.0, true));
    // In the layered list the clip draw is recorded immediately (`drawClipShapeImmediate()`), and
    // before the shading draw, so there is a layer holding it and the shading draw is in the same
    // or a later layer.
    let layers = device.testing_only_with_draw_context(|dc| match dc.pending_draws() {
        DrawListBase::Layer(list) => list.layer_count(),
        DrawListBase::List(_) => panic!("{name}: a layered draw list"),
    });
    assert!(layers >= 1, "{name}");
    assert!(
        device.testing_only_with_clip_stack(|clip| clip.element_usage(0).0),
        "{name}: the element has pending usage"
    );
    device.pop_clip_stack();
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
// rejects (`docs/design/gpu.md` section 6.3; pinned by tests/wgsl_pipelines.rs).
const KNOWN_NAGA_LIMITATION: &str = "InvalidArgumentPointerSpace";

#[test]
fn every_pipeline_of_clipped_draws_makes_valid_wgsl() {
    for use_draw_list_layer in [false, true] {
        for (name, context) in contexts(use_draw_list_layer) {
            let recorder = context.make_recorder(None);
            let mut device = make_device(&recorder, 64);
            let path = triangle(&[(0.0, 0.0), (60.0, 0.0), (0.0, 60.0)]);

            // An analytic rrect clip with a path draw and a rect draw.
            device.push_clip_stack();
            device.clip_rrect(
                &RRect::new_rect_xy(sk(8.0, 8.0, 48.0, 48.0), 6.0, 6.0),
                ClipOp::Intersect,
                true,
            );
            device.draw_path(&path, &solid(1.0, 0.0, 0.0, true));
            device.draw_rect(&sk(0.5, 0.5, 30.25, 30.25), &solid(0.0, 1.0, 0.0, true));
            device.pop_clip_stack();

            // An analytic skewed-rect clip, as a difference.
            device.push_clip_stack();
            let mut skew = M44::new_identity();
            skew.set_rc(0, 1, 0.5);
            device.state_mut().set_local_to_device(&skew);
            device.clip_rect(&sk(4.0, 4.0, 20.0, 20.0), ClipOp::Difference, true);
            device.state_mut().set_local_to_device(&M44::new_identity());
            device.draw_path(&path, &solid(0.0, 0.0, 1.0, true));
            device.pop_clip_stack();

            // Two path clips: depth-only clip draws and the draws they clip.
            device.push_clip_stack();
            device.clip_path(
                &triangle(&[(4.0, 4.0), (40.0, 4.0), (4.0, 40.0)]),
                ClipOp::Intersect,
                true,
            );
            device.clip_path(
                &triangle(&[(16.0, 16.0), (60.0, 16.0), (16.0, 60.0)]),
                ClipOp::Intersect,
                true,
            );
            device.draw_rect(&sk(0.5, 0.5, 50.25, 50.25), &solid(1.0, 0.0, 1.0, true));
            device.pop_clip_stack();

            let passes = snap(&mut device);
            let [pass] = &passes[..] else {
                panic!("{name}: one pass");
            };
            assert!(
                pass.descs
                    .iter()
                    .any(|desc| !desc.paint_params_id().is_valid()),
                "{name}: depth-only clip pipelines"
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
            // (The analytic clip's snippet is in the pipelines' shaders.)
            assert!(
                pass.descs
                    .iter()
                    .filter(|desc| desc.paint_params_id().is_valid())
                    .count()
                    >= 3,
                "{name}: the clipped draws have pipelines of their own"
            );
        }
    }
}

// ===========================================================================================
// The analytic clip in the paint's key
// ===========================================================================================

fn srgb_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

struct Fixture {
    caps: Arc<dyn Caps>,
    builder: RefCell<PaintParamsKeyBuilder>,
    gatherer: RefCell<PipelineDataGatherer>,
    dict: ShaderCodeDictionary,
}

impl Fixture {
    fn new(caps: Arc<dyn Caps>) -> Self {
        let dict = ShaderCodeDictionary::new(Layout::Std140, &[]);
        let builder = RefCell::new(PaintParamsKeyBuilder::new(&dict));
        Self {
            caps,
            builder,
            gatherer: RefCell::new(PipelineDataGatherer::new(Layout::Std140)),
            dict,
        }
    }

    fn key_string(&self) -> String {
        let mut builder = self.builder.borrow_mut();
        let lock = builder.lock_as_key();
        lock.key().to_string(&*self.caps, &self.dict)
    }

    fn key_is_valid(&self) -> bool {
        let mut builder = self.builder.borrow_mut();
        let lock = builder.lock_as_key();
        lock.key().is_valid()
    }

    fn data(&self) -> (Vec<u8>, Vec<Option<Arc<TextureProxy>>>) {
        let (uniforms, textures) = self.gatherer.borrow_mut().end_combined_data(true);
        (
            uniforms.data().to_vec(),
            textures.textures().iter().map(|(p, _)| p.clone()).collect(),
        )
    }
}

fn key_with_clip(
    fixture: &Fixture,
    clip: &NonMSAAClip,
    coverage: Coverage,
) -> Option<(
    skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID,
    DstUsage,
)> {
    let info = srgb_info();
    let context = KeyContext::new(
        fixture.caps.clone(),
        &fixture.builder,
        &fixture.gatherer,
        &fixture.dict,
        Arc::new(RuntimeEffectDictionary::new()),
        &info,
    );
    let paint = Paint::new(Color4f::new(0.25, 0.5, 0.75, 1.0), None);
    let params = PaintParams::new(&paint, None, false, false);
    let shading = ShadingParams::new(
        fixture.caps.as_ref(),
        &params,
        Some(clip),
        None,
        coverage,
        TextureFormat::RGBA8,
    );
    shading.to_key(&context)
}

const SOLID: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

#[test]
fn an_intersect_rect_clip_is_keyed_as_an_analytic_clip_block() {
    // PaintParams::toKey: source color, final blend, then the clip root; the clip root is
    // `AddAnalyticClip`, which writes xform, bounds and radii-with-inverse. An intersect (inverse
    // fill) clip encodes the radii as (r + 1) * (1, 1, -1, 1).
    let fixture = Fixture::new(Arc::new(MockCaps::default()));
    let clip = NonMSAAClip {
        analytic_clip: AnalyticClip {
            bounds: sk(10.0, 20.0, 30.0, 40.0),
            radii: skia_rust_simd::vx::Float4::new(0.0, 0.0, 0.0, 0.0),
            xform: skia_rust_simd::vx::Float4::new(1.0, 0.0, 0.0, 1.0),
            inverted: true,
        },
        atlas_clip: AtlasClip::default(),
    };
    let (id, dst_usage) = key_with_clip(&fixture, &clip, Coverage::None).expect("a valid key");

    assert!(id.is_valid());
    assert!(fixture.key_is_valid(), "no error block any more");
    // With an analytic clip the final src-over keeps the dst (the clip's coverage blends).
    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "SolidColor SrcOver AnalyticClip ");
    let mut expected = f32_bytes(&SOLID);
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 1.0])); // xform
    expected.extend(f32_bytes(&[10.0, 20.0, 30.0, 40.0])); // rect
    expected.extend(f32_bytes(&[1.0, 1.0, -1.0, 1.0])); // (0 + 1) * kIntersectEncode
    let (uniforms, textures) = fixture.data();
    assert_eq!(uniforms, expected);
    assert!(textures.is_empty());
}

#[test]
fn a_difference_rrect_clip_encodes_its_radii_and_transform() {
    // A difference clip (a regular fill) uses kDifferenceEncode = (-1, 1, 1, 1).
    let fixture = Fixture::new(Arc::new(MockCaps::default()));
    let clip = NonMSAAClip {
        analytic_clip: AnalyticClip {
            bounds: sk(0.0, 0.0, 16.0, 8.0),
            radii: skia_rust_simd::vx::Float4::new(5.0, 0.0, 3.0, 2.0),
            xform: skia_rust_simd::vx::Float4::new(1.0, 0.0, -0.5, 1.0),
            inverted: false,
        },
        atlas_clip: AtlasClip::default(),
    };
    let (_, dst_usage) = key_with_clip(&fixture, &clip, Coverage::SingleChannel).expect("a key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "SolidColor SrcOver AnalyticClip ");
    let mut expected = f32_bytes(&SOLID);
    expected.extend(f32_bytes(&[1.0, 0.0, -0.5, 1.0]));
    expected.extend(f32_bytes(&[0.0, 0.0, 16.0, 8.0]));
    // (radii + 1) * (-1, 1, 1, 1)
    expected.extend(f32_bytes(&[-6.0, 1.0, 4.0, 3.0]));
    assert_eq!(fixture.data().0, expected);
}

#[test]
fn an_empty_clip_adds_no_clip_root() {
    let fixture = Fixture::new(Arc::new(MockCaps::default()));
    let (_, dst_usage) =
        key_with_clip(&fixture, &NonMSAAClip::default(), Coverage::None).expect("a valid key");
    // Without `kPreferFixedSrcBlend` an opaque src-over paint keeps its blend (and the dst).
    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "SolidColor SrcOver ");
}

#[test]
fn an_atlas_clip_adds_the_atlas_block_and_binds_the_texture() {
    // The atlas texture: the target of a device on the noop context.
    let (_, context) = contexts(false).remove(0);
    let recorder = context.make_recorder(None);
    let device = make_device(&recorder, 64);
    let texture = device.target().ref_proxy().expect("a target");
    let caps = recorder.priv_().caps().clone();

    let fixture = Fixture::new(caps);
    let clip = NonMSAAClip {
        analytic_clip: AnalyticClip {
            bounds: sk(10.0, 20.0, 30.0, 40.0),
            radii: skia_rust_simd::vx::Float4::new(0.0, 0.0, 0.0, 0.0),
            xform: skia_rust_simd::vx::Float4::new(1.0, 0.0, 0.0, 1.0),
            inverted: true,
        },
        atlas_clip: AtlasClip {
            mask_bounds: IRect::new(30, 30, 60, 50),
            out_pos: IPoint::new(7, 9),
            atlas_texture: Some(texture.clone()),
        },
    };
    let (_, _) = key_with_clip(&fixture, &clip, Coverage::None).expect("a valid key");

    assert_eq!(
        fixture.key_string(),
        "SolidColor SrcOver AnalyticAndAtlasClip "
    );
    let mut expected = f32_bytes(&SOLID);
    expected.extend(f32_bytes(&[1.0, 0.0, 0.0, 1.0]));
    expected.extend(f32_bytes(&[10.0, 20.0, 30.0, 40.0]));
    expected.extend(f32_bytes(&[1.0, 1.0, -1.0, 1.0]));
    // The mask bounds in the atlas, outset by half a texel: (7, 9) + (30, 20), then the offset
    // from device space to atlas space (7 - 30, 9 - 30), then 1 / the atlas size.
    expected.extend(f32_bytes(&[6.5, 8.5, 37.5, 29.5]));
    expected.extend(f32_bytes(&[-23.0, -21.0]));
    expected.extend(f32_bytes(&[1.0 / 64.0, 1.0 / 64.0]));
    let (uniforms, textures) = fixture.data();
    assert_eq!(uniforms, expected);
    // The atlas texture is bound once.
    assert_eq!(textures.len(), 1);
    assert!(Arc::ptr_eq(
        textures[0].as_ref().expect("a bound proxy"),
        &texture
    ));
}

// A `Clip` that has an analytic clip needs coverage; without it or a shader it does not.
#[test]
fn a_clip_with_an_analytic_clip_needs_coverage() {
    let analytic = NonMSAAClip {
        analytic_clip: AnalyticClip {
            bounds: sk(1.0, 1.0, 5.0, 5.0),
            ..AnalyticClip::default()
        },
        atlas_clip: AtlasClip::default(),
    };
    let clip = Clip::new(
        rect(0.0, 0.0, 8.0, 8.0),
        rect(0.0, 0.0, 8.0, 8.0),
        IRect::new(0, 0, 8, 8),
        analytic,
        false,
    );
    assert!(clip.needs_coverage());
    let clip = Clip::new(
        rect(0.0, 0.0, 8.0, 8.0),
        rect(0.0, 0.0, 8.0, 8.0),
        IRect::new(0, 0, 8, 8),
        NonMSAAClip::default(),
        false,
    );
    assert!(!clip.needs_coverage());
}
