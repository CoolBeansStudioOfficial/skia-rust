// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
//
// Revalidation, bounds and hit-testing of the sksg nodes, checked against the semantics of
// Skia's modules/sksg sources (the node names follow the `sksg::` classes).

use std::rc::Rc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color as SkColor, Color4f};
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images::raster_from_data;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix::Matrix as SkMatrix;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::tile_mode::TileMode;

use skia_rust_sksg::Rect as SgRect;
use skia_rust_sksg::transform::Matrix as SgMatrix;
use skia_rust_sksg::{
    ClipEffect, Color, ColorStop, DashEffect, Draw, ExternalColorFilter, FillTypeOverride,
    GeometryNode, GeometryTransform, GradientColorFilter, Image, InvalidationController,
    LayerEffect, LinearGradient, MaskEffect, MaskMode, MaskShaderEffect, ModeColorFilter, Node,
    OffsetEffect, OpacityEffect, PaintNode, Plane, RadialGradient, RenderNode, RoundEffect, Scene,
    ShaderEffect, ShaderNode, ShaderPaint, Text, TrimEffect,
};

/// Asserts the four edges of `r`.
fn assert_rect(r: Rect, left: f32, top: f32, right: f32, bottom: f32) {
    assert_eq!(
        (r.left, r.top, r.right, r.bottom),
        (left, top, right, bottom)
    );
}

/// Revalidates `node` under the identity matrix, as `sksg::Scene::revalidate` does.
fn revalidate(node: &dyn Node) -> Rect {
    node.revalidate(None, &Matrix::new_identity())
}

/// A red filled rectangle: a draw of a rect geometry with a color paint.
fn draw(l: f32, t: f32, r: f32, b: f32) -> Rc<dyn RenderNode> {
    let geo: Rc<dyn GeometryNode> = SgRect::make(SkRect::from_ltrb(l, t, r, b));
    let paint: Rc<dyn PaintNode> = Color::make(SkColor::RED);
    Draw::make(Some(geo), Some(paint)).expect("draw of a rect with a paint")
}

/// A rect geometry node.
fn rect_geometry(l: f32, t: f32, r: f32, b: f32) -> Rc<dyn GeometryNode> {
    SgRect::make(SkRect::from_ltrb(l, t, r, b))
}

fn stop(position: f32, r: f32) -> ColorStop {
    ColorStop {
        position,
        color: Color4f::new(r, 0.0, 0.0, 1.0),
    }
}

#[test]
fn opacity_effect_passes_child_bounds_and_invalidates_on_change() {
    let child = draw(0.0, 0.0, 10.0, 20.0);
    let op = OpacityEffect::make(Some(child), 0.5).expect("opacity effect");
    assert!(op.core().has_inval());
    assert_rect(revalidate(op.as_ref()), 0.0, 0.0, 10.0, 20.0);
    assert!(!op.core().has_inval());

    // Setting the same opacity is not a change.
    op.set_opacity(0.5);
    assert!(!op.core().has_inval());
    op.set_opacity(0.25);
    assert!(op.core().has_inval());
}

#[test]
fn opacity_effect_at_zero_disables_rendering_and_hit_testing() {
    let child = draw(0.0, 0.0, 10.0, 20.0);
    let op = OpacityEffect::make(Some(child), 0.0).expect("opacity effect");
    // opacity <= 0 disables revalidation for the sub-DAG: the bounds are empty.
    assert_rect(revalidate(op.as_ref()), 0.0, 0.0, 0.0, 0.0);
    let as_render: Rc<dyn RenderNode> = op;
    assert!(skia_rust_sksg::render_node::node_at(&as_render, Point { x: 5.0, y: 5.0 }).is_none());
}

#[test]
fn clip_effect_intersects_bounds_with_the_clip() {
    let child = draw(0.0, 0.0, 20.0, 20.0);
    let clip: Rc<dyn GeometryNode> = rect_geometry(0.0, 0.0, 10.0, 10.0);
    let clip_fx = ClipEffect::make(Some(child), Some(clip), false, false).expect("clip effect");
    assert_rect(revalidate(clip_fx.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn clip_effect_hit_test_respects_the_clip() {
    let child = draw(0.0, 0.0, 20.0, 20.0);
    let clip: Rc<dyn GeometryNode> = rect_geometry(0.0, 0.0, 10.0, 10.0);
    let clip_fx = ClipEffect::make(Some(child), Some(clip), false, false).expect("clip effect");
    revalidate(clip_fx.as_ref());
    let as_render: Rc<dyn RenderNode> = clip_fx;
    let inside = skia_rust_sksg::render_node::node_at(&as_render, Point { x: 5.0, y: 5.0 });
    let outside = skia_rust_sksg::render_node::node_at(&as_render, Point { x: 15.0, y: 15.0 });
    assert!(inside.is_some());
    assert!(outside.is_none());
}

#[test]
fn clip_effect_with_force_clip_keeps_child_bounds_inside_the_clip() {
    // A child contained in the clip is elided unless the clip is forced; the bounds are the
    // child bounds in both cases.
    let child = draw(0.0, 0.0, 5.0, 5.0);
    let clip: Rc<dyn GeometryNode> = rect_geometry(0.0, 0.0, 10.0, 10.0);
    let clip_fx = ClipEffect::make(Some(child), Some(clip), false, true).expect("clip effect");
    assert_rect(revalidate(clip_fx.as_ref()), 0.0, 0.0, 5.0, 5.0);
}

#[test]
fn mask_effect_intersects_bounds_with_the_mask() {
    let child = draw(0.0, 0.0, 20.0, 20.0);
    let mask = draw(0.0, 0.0, 10.0, 10.0);
    let fx = MaskEffect::make(Some(child), Some(mask), MaskMode::AlphaNormal).expect("mask");
    assert_rect(revalidate(fx.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn inverted_mask_effect_keeps_child_bounds() {
    let child = draw(0.0, 0.0, 20.0, 20.0);
    let mask = draw(0.0, 0.0, 10.0, 10.0);
    let fx = MaskEffect::make(Some(child), Some(mask), MaskMode::AlphaInvert).expect("mask");
    assert_rect(revalidate(fx.as_ref()), 0.0, 0.0, 20.0, 20.0);
}

#[test]
fn inverted_mask_effect_hit_test_keeps_points_outside_the_mask() {
    let child = draw(0.0, 0.0, 20.0, 20.0);
    let mask = draw(0.0, 0.0, 10.0, 10.0);
    let fx = MaskEffect::make(Some(child), Some(mask), MaskMode::AlphaInvert).expect("mask");
    revalidate(fx.as_ref());
    let as_render: Rc<dyn RenderNode> = fx;
    // Outside the mask the inverted mask lets the child through.
    assert!(skia_rust_sksg::render_node::node_at(&as_render, Point { x: 15.0, y: 15.0 }).is_some());
    // Inside the mask the content is hidden.
    assert!(skia_rust_sksg::render_node::node_at(&as_render, Point { x: 5.0, y: 5.0 }).is_none());
}

#[test]
fn external_color_filter_passes_child_bounds() {
    let child = draw(1.0, 2.0, 30.0, 40.0);
    let cf = ExternalColorFilter::make(Some(child)).expect("external color filter");
    assert_rect(revalidate(cf.as_ref()), 1.0, 2.0, 30.0, 40.0);
}

#[test]
fn mode_color_filter_revalidates_its_color() {
    let child = draw(0.0, 0.0, 10.0, 10.0);
    let color = Color::make(SkColor::BLACK);
    let cf = ModeColorFilter::make(Some(child), Some(color.clone()), BlendMode::SrcIn)
        .expect("mode color filter");
    assert_rect(revalidate(cf.as_ref()), 0.0, 0.0, 10.0, 10.0);
    // The color is revalidated along with its filter, so the color node is clean afterwards.
    assert!(!color.core().has_inval());
    // The color is an observed input of the filter: changing it invalidates the filter.
    color.set_color(SkColor::RED);
    assert!(cf.core().has_inval());
    assert_rect(revalidate(cf.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn gradient_color_filter_needs_two_colors_and_passes_bounds() {
    let child = draw(0.0, 0.0, 10.0, 10.0);
    let c0 = Color::make(SkColor::BLACK);
    let c1 = Color::make(SkColor::RED);
    assert!(GradientColorFilter::make(Some(child.clone()), Some(c0.clone()), None).is_none());
    let cf = GradientColorFilter::make(Some(child), Some(c0), Some(c1)).expect("gradient");
    assert_eq!(cf.weight(), 0.0);
    cf.set_weight(0.5);
    assert!(cf.core().has_inval());
    assert_rect(revalidate(cf.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn offset_effect_grows_the_outline() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let offset = OffsetEffect::make(Some(geo)).expect("offset");
    offset.set_offset(10.0);
    assert_eq!(offset.miter_limit(), 4.0);
    assert_rect(revalidate(offset.as_ref()), -10.0, -10.0, 110.0, 110.0);
}

#[test]
fn offset_effect_shrinks_the_outline_for_negative_offsets() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let offset = OffsetEffect::make(Some(geo)).expect("offset");
    offset.set_offset(-10.0);
    assert_rect(revalidate(offset.as_ref()), 10.0, 10.0, 90.0, 90.0);
}

#[test]
fn zero_offset_effect_keeps_the_path() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let offset = OffsetEffect::make(Some(geo)).expect("offset");
    assert_rect(revalidate(offset.as_ref()), 0.0, 0.0, 100.0, 100.0);
}

#[test]
fn round_effect_keeps_the_bounds_of_a_rect() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let round = RoundEffect::make(Some(geo)).expect("round");
    round.set_radius(10.0);
    assert_eq!(round.radius(), 10.0);
    assert_rect(revalidate(round.as_ref()), 0.0, 0.0, 100.0, 100.0);
}

#[test]
fn fill_type_override_changes_the_fill_type_only() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let override_fx =
        FillTypeOverride::make(Some(geo), PathFillType::InverseWinding).expect("override");
    assert_rect(revalidate(override_fx.as_ref()), 0.0, 0.0, 100.0, 100.0);
    assert_eq!(
        override_fx.as_path().fill_type(),
        PathFillType::InverseWinding
    );
}

#[test]
fn geometry_transform_moves_the_bounds() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let transform: Rc<dyn skia_rust_sksg::Transform> =
        SgMatrix::<SkMatrix>::make(SkMatrix::translate((5.0, 7.0)));
    let fx = GeometryTransform::make(Some(geo), Some(transform)).expect("transform");
    assert_rect(revalidate(fx.as_ref()), 5.0, 7.0, 105.0, 107.0);
}

#[test]
fn trim_effect_keeps_the_first_quarter_of_the_contour() {
    // A rect's contour starts at its top-left corner and runs clockwise: its first quarter is
    // the top edge.
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let trim = TrimEffect::make(Some(geo)).expect("trim");
    trim.set_stop(0.25);
    assert_eq!(trim.start(), 0.0);
    assert_eq!(trim.stop(), 0.25);
    assert_rect(revalidate(trim.as_ref()), 0.0, 0.0, 100.0, 0.0);
}

#[test]
fn dash_effect_without_intervals_keeps_the_path() {
    let geo = rect_geometry(0.0, 0.0, 100.0, 100.0);
    let dash = DashEffect::make(Some(geo)).expect("dash");
    assert_eq!(dash.intervals(), Vec::<f32>::new());
    assert_rect(revalidate(dash.as_ref()), 0.0, 0.0, 100.0, 100.0);
}

#[test]
fn linear_gradient_has_a_shader_only_with_color_stops() {
    let gradient = LinearGradient::make();
    assert_rect(revalidate(gradient.as_ref()), 0.0, 0.0, 0.0, 0.0);
    assert!(gradient.shader().is_none());

    gradient.set_color_stops(vec![stop(0.0, 1.0), stop(1.0, 0.0)]);
    gradient.set_end_point(Point { x: 100.0, y: 0.0 });
    gradient.set_tile_mode(TileMode::Repeat);
    assert_eq!(gradient.tile_mode(), TileMode::Repeat);
    assert!(gradient.core().has_inval());
    // A shader has no bounds of its own.
    assert_rect(revalidate(gradient.as_ref()), 0.0, 0.0, 0.0, 0.0);
    assert!(gradient.shader().is_some());
}

#[test]
fn radial_gradient_has_a_shader_only_with_color_stops() {
    let gradient = RadialGradient::make();
    gradient.set_end_radius(50.0);
    assert_eq!(gradient.end_radius(), 50.0);
    // Without color stops the revalidated shader is absent.
    assert_rect(revalidate(gradient.as_ref()), 0.0, 0.0, 0.0, 0.0);
    assert!(ShaderNode::shader(gradient.as_ref()).is_none());
    gradient.set_color_stops(vec![stop(0.0, 1.0), stop(1.0, 0.0)]);
    assert_rect(revalidate(gradient.as_ref()), 0.0, 0.0, 0.0, 0.0);
    assert!(ShaderNode::shader(gradient.as_ref()).is_some());
    // The centers default to the origin, so the gradient is concentric.
    assert_eq!(gradient.start_center(), Point { x: 0.0, y: 0.0 });
    assert_eq!(gradient.end_center(), Point { x: 0.0, y: 0.0 });
}

#[test]
fn shader_paint_applies_the_gradient_to_the_paint() {
    let gradient = LinearGradient::make();
    gradient.set_color_stops(vec![stop(0.0, 1.0), stop(1.0, 0.0)]);
    let shader: Rc<dyn ShaderNode> = gradient;
    let paint = ShaderPaint::make(Some(shader)).expect("shader paint");
    assert_rect(revalidate(paint.as_ref()), 0.0, 0.0, 0.0, 0.0);
    assert!(paint.make_paint().shader().is_some());
}

#[test]
fn shader_effect_and_mask_shader_effect_pass_child_bounds() {
    let child = draw(0.0, 0.0, 10.0, 10.0);
    let gradient = LinearGradient::make();
    gradient.set_color_stops(vec![stop(0.0, 1.0), stop(1.0, 0.0)]);
    let shader: Rc<dyn ShaderNode> = gradient;
    let fx = ShaderEffect::make(Some(child.clone()), Some(shader)).expect("shader effect");
    assert_rect(revalidate(fx.as_ref()), 0.0, 0.0, 10.0, 10.0);

    let masked = MaskShaderEffect::make(Some(child), None).expect("mask shader effect");
    assert_rect(revalidate(masked.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn layer_effect_passes_child_bounds() {
    let child = draw(0.0, 0.0, 10.0, 10.0);
    let layer = LayerEffect::make(Some(child), BlendMode::Multiply).expect("layer");
    assert_eq!(layer.mode(), BlendMode::Multiply);
    assert_rect(revalidate(layer.as_ref()), 0.0, 0.0, 10.0, 10.0);
}

#[test]
fn plane_covers_everything() {
    let plane = Plane::make();
    assert_rect(
        revalidate(plane.as_ref()),
        f32::MIN,
        f32::MIN,
        f32::MAX,
        f32::MAX,
    );
    assert!(skia_rust_sksg::GeometryNode::contains(
        plane.as_ref(),
        Point {
            x: -1.0e6,
            y: 1.0e6
        }
    ));
    assert_eq!(
        skia_rust_sksg::GeometryNode::as_path(plane.as_ref()).fill_type(),
        PathFillType::InverseWinding
    );
}

#[test]
fn image_bounds_are_the_image_bounds() {
    let info = ImageInfo::new_n32_premul((4, 3), None);
    let image =
        raster_from_data(&info, Data::new_copy(&[0_u8; 4 * 3 * 4]), 16).expect("raster image");
    let node = Image::make(Some(image));
    assert_rect(revalidate(node.as_ref()), 0.0, 0.0, 4.0, 3.0);
    node.set_anti_alias(false);
    assert!(!node.anti_alias());
}

#[test]
fn image_without_pixels_has_empty_bounds() {
    let node = Image::make(None);
    assert_rect(revalidate(node.as_ref()), 0.0, 0.0, 0.0, 0.0);
}

#[test]
fn text_without_glyphs_has_empty_bounds() {
    let node = Text::make(None, "");
    assert_rect(revalidate(node.as_ref()), 0.0, 0.0, 0.0, 0.0);
    node.set_position(Point { x: 3.0, y: 4.0 });
    assert_eq!(node.position(), Point { x: 3.0, y: 4.0 });
    assert_eq!(node.text(), "");
}

#[test]
fn scene_hit_tests_the_front_most_node() {
    let root = draw(0.0, 0.0, 10.0, 10.0);
    let scene = Scene::make(Some(root)).expect("scene");
    let mut ic = InvalidationController::new();
    scene.revalidate(Some(&mut ic));
    assert!(scene.node_at(Point { x: 5.0, y: 5.0 }).is_some());
    assert!(scene.node_at(Point { x: 50.0, y: 50.0 }).is_none());
    assert!(Scene::make(None).is_none());
}

#[test]
fn scene_revalidation_reports_damage_for_changed_nodes() {
    let geo = SgRect::make(SkRect::from_ltrb(0.0, 0.0, 10.0, 10.0));
    let paint: Rc<dyn PaintNode> = Color::make(SkColor::RED);
    let draw = Draw::make(Some(geo.clone()), Some(paint)).expect("draw");
    let scene = Scene::make(Some(draw)).expect("scene");
    scene.revalidate(None);

    let mut ic = InvalidationController::new();
    geo.set_r(20.0);
    geo.set_b(20.0);
    scene.revalidate(Some(&mut ic));
    assert!(!ic.bounds().is_empty());
}

#[test]
fn setting_a_geometry_attribute_invalidates_its_node() {
    let geo = SgRect::make(SkRect::from_ltrb(0.0, 0.0, 10.0, 10.0));
    assert!(geo.core().has_inval());
    revalidate(geo.as_ref());
    assert!(!geo.core().has_inval());
    geo.set_r(20.0);
    assert!(geo.core().has_inval());
}
