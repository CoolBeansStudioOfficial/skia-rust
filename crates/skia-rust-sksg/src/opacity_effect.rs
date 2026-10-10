// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGOpacityEffect.h, modules/sksg/src/SkSGOpacityEffect.cpp
// (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::render_node::{Hit, RenderContext, RenderNode, ScopedRenderContext};
use crate::util::scalar_changed;

/// Modulates the opacity of its child (`OpacityEffect`).
// Port of: modules/sksg/include/SkSGOpacityEffect.h#L14-L35 (chrome/m156) (`class OpacityEffect`)
#[doc(alias = "sksg::OpacityEffect")]
#[derive(Debug)]
pub struct OpacityEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    opacity: Cell<f32>,
}

impl OpacityEffect {
    /// `OpacityEffect::Make(child, opacity)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGOpacityEffect.h#L17-L19 (chrome/m156) (`OpacityEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn RenderNode>>, opacity: f32) -> Option<Rc<Self>> {
        let child = child?;
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            opacity: Cell::new(opacity),
        });
        // The EffectNode base observes its child.
        effect.observe_inval(effect.child.as_ref());
        Some(effect)
    }

    /// The opacity (`getOpacity`).
    #[doc(alias = "getOpacity")]
    #[must_use]
    pub fn opacity(&self) -> f32 {
        self.opacity.get()
    }

    /// Sets the opacity, invalidating the node if it changed (`setOpacity`).
    #[doc(alias = "setOpacity")]
    pub fn set_opacity(&self, opacity: f32) {
        if scalar_changed(self.opacity.get(), opacity) {
            self.opacity.set(opacity);
            self.invalidate();
        }
    }
}

impl Drop for OpacityEffect {
    // Port of: modules/sksg/src/SkSGEffectNode.cpp (chrome/m156) (`EffectNode::~EffectNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for OpacityEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGOpacityEffect.cpp#L42-L47 (chrome/m156) (`OpacityEffect::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        // opacity <= 0 disables rendering AND revalidation for the sub-DAG
        if self.opacity.get() > 0.0 {
            effect_on_revalidate(&self.child, ic, ctm)
        } else {
            Rect::new_empty()
        }
    }
}

impl RenderNode for OpacityEffect {
    // Port of: modules/sksg/src/SkSGOpacityEffect.cpp#L22-L36 (chrome/m156) (`OpacityEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let opacity = self.opacity.get();
        // opacity <= 0 disables rendering
        if opacity <= 0.0 {
            return;
        }
        // opacity >= 1 has no effect
        if opacity >= 1.0 {
            effect_on_render(&self.child, canvas, ctx);
            return;
        }
        let scope = ScopedRenderContext::new(canvas, ctx).modulate_opacity(opacity);
        effect_on_render(&self.child, canvas, Some(scope.context()));
    }

    // Port of: modules/sksg/src/SkSGOpacityEffect.cpp#L38-L40 (chrome/m156) (`OpacityEffect::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        if self.opacity.get() > 0.0 {
            effect_on_node_at(&self.child, p)
        } else {
            None
        }
    }
}
