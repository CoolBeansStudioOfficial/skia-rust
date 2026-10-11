// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/RadialWipeEffect.cpp (chrome/m156)
//
// The radial wipe effect: a custom render node that masks the layer with a sweep gradient around
// the wipe center, cached during revalidation.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color4f;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::Hit;
use skia_rust_sksg::util::scalar_changed;
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The custom render node of the wipe: it masks its child with the cached sweep gradient.
// Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L15-L110 (chrome/m156) (`RWipeRenderNode`)
#[derive(Debug)]
pub(super) struct RWipeRenderNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    completion: Cell<f32>,
    start_angle: Cell<f32>,
    wipe_center: Cell<Point>,
    wipe: Cell<f32>,
    feather: Cell<f32>,
    // Cached during revalidation.
    mask_shader: RefCell<Option<Shader>>,
}

impl RWipeRenderNode {
    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L17-L19 (chrome/m156) (`RWipeRenderNode::RWipeRenderNode`)
    fn make(layer: &Rc<dyn RenderNode>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let node = Self {
                core: NodeCore::new(0, weak.clone()),
                child: Rc::clone(layer),
                completion: Cell::new(0.0),
                start_angle: Cell::new(0.0),
                wipe_center: Cell::new(Point { x: 0.0, y: 0.0 }),
                wipe: Cell::new(0.0),
                feather: Cell::new(0.0),
                mask_shader: RefCell::new(None),
            };
            // The custom node observes its child.
            node.observe_inval(node.child.as_ref());
            node
        })
    }

    /// Sets a scalar attribute, invalidating the node if it changed (`SG_ATTRIBUTE`).
    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L21-L26 (chrome/m156) (`SG_ATTRIBUTE`)
    fn set_scalar(&self, cell: &Cell<f32>, value: f32) {
        if scalar_changed(cell.get(), value) {
            cell.set(value);
            self.invalidate();
        }
    }

    /// Sets the completion (`setCompletion`).
    pub(super) fn set_completion(&self, value: f32) {
        self.set_scalar(&self.completion, value);
    }

    /// Sets the start angle (`setStartAngle`).
    pub(super) fn set_start_angle(&self, value: f32) {
        self.set_scalar(&self.start_angle, value);
    }

    /// Sets the wipe center (`setWipeCenter`).
    pub(super) fn set_wipe_center(&self, value: Point) {
        if self.wipe_center.get() != value {
            self.wipe_center.set(value);
            self.invalidate();
        }
    }

    /// Sets the wipe mode (`setWipe`).
    pub(super) fn set_wipe(&self, value: f32) {
        self.set_scalar(&self.wipe, value);
    }

    /// Sets the feather (`setFeather`).
    pub(super) fn set_feather(&self, value: f32) {
        self.set_scalar(&self.feather, value);
    }

    /// The angle offset of the wipe mode (`wipeAlignment`).
    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L104-L115 (chrome/m156) (`RWipeRenderNode::wipeAlignment`)
    fn wipe_alignment(&self) -> f32 {
        match scalar_round_to_int(self.wipe.get()) {
            // 1 (clockwise) and the default have no offset.
            2 => -360.0, // Counterclockwise
            3 => -180.0, // Both/center
            _ => 0.0,
        }
    }
}

/// `std::fmod`-based angle sanitizer: the angle in `[0, 360)`.
// Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L63-L68 (chrome/m156) (`sanitize_angle`)
fn sanitize_angle(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a < 0.0 {
        a += 360.0;
    }
    a
}

impl Drop for RWipeRenderNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for RWipeRenderNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L35-L61 (chrome/m156) (`RWipeRenderNode::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let content_bounds = self.child.revalidate(ic, ctm);
        let completion = self.completion.get();
        if completion >= 100.0 {
            return Rect::new_empty();
        }
        if completion <= 0.0 {
            *self.mask_shader.borrow_mut() = None;
        } else {
            // The edge feather blur is disabled in Skia (`fMaskSigma` only feeds that branch).
            let t = completion * 0.01_f32;
            // Note: this could be simplified as a one-hard-stop gradient + local matrix (to apply
            // rotation). Alas, local matrices are no longer supported in SkSG.
            let mut c0 = Color4f::new(0.0, 0.0, 0.0, 0.0);
            let mut c1 = Color4f::new(1.0, 1.0, 1.0, 1.0);

            let mut a0 = sanitize_angle(self.start_angle.get() - 90.0 + t * self.wipe_alignment());
            let mut a1 = sanitize_angle(a0 + t * 360.0);
            if a0 > a1 {
                std::mem::swap(&mut a0, &mut a1);
                std::mem::swap(&mut c0, &mut c1);
            }

            let grad_colors = [c1, c0, c0, c1];
            let grad_pos = [0.0_f32, 0.0, 1.0, 1.0];
            let gradient = Gradient::new(
                Colors::new(&grad_colors[..], Some(&grad_pos[..]), TileMode::Clamp, None),
                Interpolation::default(),
            );
            let center = self.wipe_center.get();
            *self.mask_shader.borrow_mut() =
                shaders::sweep_gradient(center, (a0, a1), &gradient, None);
        }
        content_bounds
    }
}

impl RenderNode for RWipeRenderNode {
    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L69-L80 (chrome/m156) (`RWipeRenderNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        if self.completion.get() >= 100.0 {
            // Fully masked out.
            return;
        }
        let mask = self.mask_shader.borrow().clone();
        let local_ctx = ScopedRenderContext::new(canvas, ctx)
            .modulate_mask_shader(mask, &canvas.total_matrix());
        self.child.render(canvas, Some(local_ctx.context()));
    }

    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L65-L67 (chrome/m156) (`RWipeRenderNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// The radial wipe adapter: the completion, start angle, center, wipe mode and feather.
// Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L117-L152 (chrome/m156) (`RadialWipeAdapter`)
struct RadialWipeAdapter {
    base: DiscardableAdapterBase<RWipeRenderNode>,
    completion: Prop<ScalarValue>,
    start_angle: Prop<ScalarValue>,
    wipe_center: Prop<Vec2Value>,
    wipe: Prop<ScalarValue>,
    feather: Prop<ScalarValue>,
}

impl RadialWipeAdapter {
    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L121-L135 (chrome/m156) (`RadialWipeAdapter::RadialWipeAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), RWipeRenderNode::make(layer));
            let completion = Prop::new(0.0);
            let start_angle = Prop::new(0.0);
            let wipe_center = Prop::new(Vec2Value::new(0.0, 0.0));
            let wipe = Prop::new(0.0);
            let feather = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &completion)
                .bind(1, &start_angle)
                .bind(2, &wipe_center)
                .bind(3, &wipe)
                .bind(4, &feather);
            Self {
                base,
                completion,
                start_angle,
                wipe_center,
                wipe,
                feather,
            }
        })
    }
}

impl AnimatablePropertyContainer for RadialWipeAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L137-L146 (chrome/m156) (`RadialWipeAdapter::onSync`)
    fn on_sync(&self) {
        let wiper = self.base.node();
        wiper.set_completion(*self.completion.borrow());
        wiper.set_start_angle(*self.start_angle.borrow());
        let center = *self.wipe_center.borrow();
        wiper.set_wipe_center(Point::new(center.x, center.y));
        wiper.set_wipe(*self.wipe.borrow());
        wiper.set_feather(*self.feather.borrow());
    }
}

impl_container_animator!(RadialWipeAdapter);

/// The radial wipe effect (`ADBE Radial Wipe`, or the legacy 'ty' 26).
// Port of: modules/skottie/src/effects/RadialWipeEffect.cpp#L154-L160 (chrome/m156) (`EffectBuilder::attachRadialWipeEffect`)
pub(super) fn attach_radial_wipe_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = RadialWipeAdapter::make(jprops, &layer, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
