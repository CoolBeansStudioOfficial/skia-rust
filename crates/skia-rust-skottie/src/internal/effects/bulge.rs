// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/BulgeEffect.cpp (chrome/m156)
//
// The bulge effect: a custom render node that displaces the layer radially, through an SkSL
// runtime shader that samples the layer content as a repeating picture shader.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::{scalar_abs, scalar_asin};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::{Hit, has_children_inval};
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The bulge SkSL: the bulge is a combination of spherical and exponential displacement along the
/// radius, in a space where the ellipse is a unit circle centered on the origin.
// Port of: modules/skottie/src/effects/BulgeEffect.cpp#L37-L84 (chrome/m156) (`gBulgeDisplacementSkSL`)
const BULGE_DISPLACEMENT_SKSL: &str = concat!(
    "uniform shader u_layer;",
    "uniform float2 u_center;",
    "uniform float2 u_radius;",
    "uniform float2 u_radius_inv;",
    "uniform float u_h;",
    "uniform float u_rcpR;",
    "uniform float u_rcpAsinInvR;",
    "uniform float u_selector;",
    "float2 displace_sph(float2 v) {",
    "float arc_ratio = asin(length(v)*u_rcpR)*u_rcpAsinInvR;",
    "return normalize(v)*arc_ratio - v;",
    "}",
    "float2 displace_exp(float2 v) {",
    "return v*pow(dot(v,v),u_h) - v;",
    "}",
    "half2 displace(float2 v) {",
    "float t = dot(v, v);",
    "if (t >= 1) {",
    "return v;",
    "}",
    "float2 d = displace_sph(v) + displace_exp(v);",
    "return v + (d * u_selector);",
    "}",
    "half4 main(float2 xy) {",
    "xy = (xy - u_center)*u_radius_inv;",
    "xy = displace(xy);",
    "xy = xy*u_radius + u_center;",
    "return u_layer.eval(xy);",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L86-L93 (chrome/m156) (`bulge_effect`)
    static BULGE_EFFECT: RuntimeEffect = RuntimeEffect::make_for_shader(BULGE_DISPLACEMENT_SKSL, None)
        .expect("the bulge effect compiles");
}

/// The bulge node: it displaces the layer content around the bulge center.
// Port of: modules/skottie/src/effects/BulgeEffect.cpp#L95-L124 (chrome/m156) (`BulgeNode`)
#[derive(Debug)]
pub(super) struct BulgeNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    child_size: Size,
    // Cached shaders.
    effect_shader: RefCell<Option<Shader>>,
    content_shader: RefCell<Option<Shader>>,
    center: Cell<Point>,
    radius: Cell<Point>,
    height: Cell<f32>,
}

impl BulgeNode {
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L99-L101 (chrome/m156) (`BulgeNode::BulgeNode`)
    fn make(child: Rc<dyn RenderNode>, child_size: Size) -> Rc<Self> {
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            child_size,
            effect_shader: RefCell::new(None),
            content_shader: RefCell::new(None),
            center: Cell::new(Point { x: 0.0, y: 0.0 }),
            radius: Cell::new(Point { x: 0.0, y: 0.0 }),
            height: Cell::new(0.0),
        });
        // The custom node observes its child.
        node.observe_inval(node.child.as_ref());
        node
    }

    /// Sets the bulge center, invalidating the node if it changed (`setCenter`).
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L103-L105 (chrome/m156) (`SG_ATTRIBUTE(Center)`)
    pub(super) fn set_center(&self, center: Point) {
        if self.center.get() != center {
            self.center.set(center);
            self.invalidate();
        }
    }

    /// Sets the bulge radius, invalidating the node if it changed (`setRadius`).
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L104-L106 (chrome/m156) (`SG_ATTRIBUTE(Radius)`)
    pub(super) fn set_radius(&self, radius: Point) {
        if self.radius.get() != radius {
            self.radius.set(radius);
            self.invalidate();
        }
    }

    /// Sets the bulge height, invalidating the node if it changed (`setHeight`).
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L105-L107 (chrome/m156) (`SG_ATTRIBUTE(Height)`)
    pub(super) fn set_height(&self, height: f32) {
        if self.height.get() != height {
            self.height.set(height);
            self.invalidate();
        }
    }

    /// The layer content as a repeating picture shader (`contentShader`).
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L126-L139 (chrome/m156) (`BulgeNode::contentShader`)
    fn content_shader(&self) -> Option<Shader> {
        if self.content_shader.borrow().is_none() || has_children_inval(std::slice::from_ref(&self.child))
        {
            self.child.revalidate(None, &Matrix::new_identity());
            let mut recorder = PictureRecorder::new();
            let canvas = recorder.begin_recording(Rect::from_size(self.child_size), false);
            self.child.render(canvas, None);
            let picture: Option<Picture> = recorder.finish_recording_as_picture(None);
            *self.content_shader.borrow_mut() = picture.and_then(|picture| {
                picture.to_shader(
                    (TileMode::Repeat, TileMode::Repeat),
                    FilterMode::Linear,
                    None::<&Matrix>,
                    None::<&Rect>,
                )
            });
        }
        self.content_shader.borrow().clone()
    }

    /// The bulge shader (`buildEffectShader`).
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L141-L166 (chrome/m156) (`BulgeNode::buildEffectShader`)
    fn build_effect_shader(&self) -> Option<Shader> {
        let height = self.height.get();
        if height == 0.0 {
            return None;
        }
        let radius = self.radius.get();
        let center = self.center.get();
        let adj_height = scalar_abs(height) / 4.0_f32;
        let r = (1.0_f32 + adj_height) / 2.0_f32 / adj_height.sqrt();
        // `std::pow(float, int)` is evaluated in double precision, then narrowed to float.
        let h = (f64::from(adj_height).powf(3.0) * f64::from(1.3_f32)) as f32;

        let child_shader = self.content_shader();
        BULGE_EFFECT.with(|effect| {
            let mut builder = RuntimeEffectBuilder::new(effect.clone());
            builder.uniform("u_center").set_f32(&[center.x, center.y]);
            builder.uniform("u_radius").set_f32(&[radius.x, radius.y]);
            builder
                .uniform("u_radius_inv")
                .set_f32(&[1.0 / radius.x, 1.0 / radius.y]);
            builder.uniform("u_h").set_f32(&[h]);
            builder.uniform("u_rcpR").set_f32(&[1.0 / r]);
            builder
                .uniform("u_rcpAsinInvR")
                .set_f32(&[1.0 / scalar_asin(1.0 / r)]);
            builder
                .uniform("u_selector")
                .set_f32(&[if height > 0.0 { 1.0 } else { -1.0 }]);
            match child_shader {
                Some(shader) => {
                    builder.child("u_layer").assign(shader);
                }
                None => {
                    builder.child("u_layer").assign_null();
                }
            }
            builder.make_shader(None)
        })
    }
}

impl Drop for BulgeNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for BulgeNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L168-L172 (chrome/m156) (`BulgeNode::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        *self.effect_shader.borrow_mut() = self.build_effect_shader();
        self.child.revalidate(ic, ctm)
    }
}

impl RenderNode for BulgeNode {
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L174-L191 (chrome/m156) (`BulgeNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        if self.height.get() == 0.0 {
            self.child.render(canvas, ctx);
            return;
        }
        let bounds = self.core().bounds();
        let _scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &bounds,
            &canvas.total_matrix(),
            true,
        );
        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));

        let mut effect_paint = Paint::default();
        effect_paint.set_shader(self.effect_shader.borrow().clone());
        effect_paint.set_blend_mode(BlendMode::SrcOver);
        canvas.draw_paint(&effect_paint);
    }

    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L193 (chrome/m156) (`BulgeNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// The bulge adapter: the radii, center and height of the bulge.
// Port of: modules/skottie/src/effects/BulgeEffect.cpp#L195-L236 (chrome/m156) (`BulgeEffectAdapter`)
struct BulgeEffectAdapter {
    base: DiscardableAdapterBase<BulgeNode>,
    center: Prop<Vec2Value>,
    horizontal_radius: Prop<ScalarValue>,
    vertical_radius: Prop<ScalarValue>,
    bulge_height: Prop<ScalarValue>,
}

impl BulgeEffectAdapter {
    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L200-L222 (chrome/m156) (`BulgeEffectAdapter::BulgeEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        node: Rc<BulgeNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let horizontal_radius = Prop::new(0.0);
            let vertical_radius = Prop::new(0.0);
            let center = Prop::new(Vec2Value::new(0.0, 0.0));
            let bulge_height = Prop::new(0.0);
            // kTaper_Index = 4, kAA_Index = 5, kPinning_Index = 6 are not applied.
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &horizontal_radius)
                .bind(1, &vertical_radius)
                .bind(2, &center)
                .bind(3, &bulge_height);
            Self {
                base,
                center,
                horizontal_radius,
                vertical_radius,
                bulge_height,
            }
        })
    }
}

impl AnimatablePropertyContainer for BulgeEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/BulgeEffect.cpp#L224-L231 (chrome/m156) (`BulgeEffectAdapter::onSync`)
    fn on_sync(&self) {
        // pre-shader math
        let node = self.base.node();
        let center = *self.center.borrow();
        node.set_center(Point::new(center.x, center.y));
        node.set_radius(Vector::new(
            *self.horizontal_radius.borrow(),
            *self.vertical_radius.borrow(),
        ));
        node.set_height(*self.bulge_height.borrow());
    }
}

impl_container_animator!(BulgeEffectAdapter);

/// The bulge effect (`ADBE Bulge`).
// Port of: modules/skottie/src/effects/BulgeEffect.cpp#L238-L243 (chrome/m156) (`EffectBuilder::attachBulgeEffect`)
pub(super) fn attach_bulge_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let node = BulgeNode::make(layer, eb.layer_size());
    let adapter = BulgeEffectAdapter::make(jprops, eb.builder(), Rc::clone(&node));
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
