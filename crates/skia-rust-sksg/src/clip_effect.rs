// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGClipEffect.h, modules/sksg/src/SkSGClipEffect.cpp
// (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::geometry_node::GeometryNode;
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::render_node::{Hit, RenderContext, RenderNode};

/// Clips its child to a geometry (`ClipEffect`).
// Port of: modules/sksg/include/SkSGClipEffect.h#L14-L37 (chrome/m156) (`class ClipEffect`)
#[doc(alias = "sksg::ClipEffect")]
#[derive(Debug)]
pub struct ClipEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    clip_node: Rc<dyn GeometryNode>,
    anti_alias: bool,
    force_clip: bool,
    /// True when the clip can be elided, because the child is contained in it (`fNoop`).
    noop: Cell<bool>,
}

impl ClipEffect {
    /// `ClipEffect::Make(child, clip, aa, force_clip)`: `None` if either is missing.
    // Port of: modules/sksg/include/SkSGClipEffect.h#L17-L24 (chrome/m156) (`ClipEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        clip: Option<Rc<dyn GeometryNode>>,
        anti_alias: bool,
        force_clip: bool,
    ) -> Option<Rc<Self>> {
        let (child, clip) = (child?, clip?);
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            clip_node: Rc::clone(&clip),
            anti_alias,
            force_clip,
            noop: Cell::new(false),
        });
        // The EffectNode base observes the child, then ClipEffect observes the clip.
        effect.observe_inval(effect.child.as_ref());
        // Port of: modules/sksg/src/SkSGClipEffect.cpp#L22-L28 (chrome/m156) (`ClipEffect::ClipEffect`)
        effect.observe_inval(effect.clip_node.as_ref());
        Some(effect)
    }
}

impl Drop for ClipEffect {
    // Port of: modules/sksg/src/SkSGClipEffect.cpp#L30-L32 (chrome/m156) (`ClipEffect::~ClipEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.clip_node.as_ref());
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for ClipEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGClipEffect.cpp#L47-L59 (chrome/m156) (`ClipEffect::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let clip_bounds = self.clip_node.revalidate(ic.as_deref_mut(), ctm);
        let mut child_bounds = effect_on_revalidate(&self.child, ic, ctm);
        // When the child node is fully contained within the clip, it is usually safe to elide.
        // An exception is clip-dependent sizing for saveLayer buffers, where the clip is always
        // significant. For those cases, we provide a mechanism to disable elision.
        self.noop.set(
            !self.force_clip
                && self
                    .clip_node
                    .as_path()
                    .conservatively_contains_rect(child_bounds),
        );
        if child_bounds.intersect(clip_bounds) {
            child_bounds
        } else {
            Rect::new_empty()
        }
    }
}

impl RenderNode for ClipEffect {
    // Port of: modules/sksg/src/SkSGClipEffect.cpp#L34-L41 (chrome/m156) (`ClipEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        // SkAutoCanvasRestore: restores the save count on every exit path.
        let save_count = canvas.save_count();
        if !self.noop.get() {
            canvas.save();
            self.clip_node.clip(canvas, self.anti_alias);
        }
        effect_on_render(&self.child, canvas, ctx);
        canvas.restore_to_count(save_count);
    }

    // Port of: modules/sksg/src/SkSGClipEffect.cpp#L43-L45 (chrome/m156) (`ClipEffect::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        if self.clip_node.contains(p) {
            effect_on_node_at(&self.child, p)
        } else {
            None
        }
    }
}
