// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGEffectNode.h, modules/sksg/src/SkSGEffectNode.cpp
// (chrome/m156)
//
// `EffectNode` is a render node with one child. Its three hooks forward to the child, so the
// effect nodes here call these helpers from their own `RenderNode`/`Node` impls.

use std::rc::Rc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::invalidation_controller::InvalidationController;
use crate::render_node::{Hit, RenderContext, RenderNode, node_at};

/// `EffectNode::onRender`: renders the child with the same context.
// Port of: modules/sksg/src/SkSGEffectNode.cpp#L20-L22 (chrome/m156) (`EffectNode::onRender`)
pub fn effect_on_render(child: &Rc<dyn RenderNode>, canvas: &Canvas, ctx: Option<&RenderContext>) {
    child.render(canvas, ctx);
}

/// `EffectNode::onNodeAt`: hit-tests the child.
// Port of: modules/sksg/src/SkSGEffectNode.cpp#L24-L26 (chrome/m156) (`EffectNode::onNodeAt`)
#[must_use]
pub fn effect_on_node_at(child: &Rc<dyn RenderNode>, p: Point) -> Option<Hit> {
    node_at(child, p).map(Hit::Child)
}

/// `EffectNode::onRevalidate`: revalidates the child, and returns its bounds.
// Port of: modules/sksg/src/SkSGEffectNode.cpp#L28-L32 (chrome/m156) (`EffectNode::onRevalidate`)
pub fn effect_on_revalidate(
    child: &Rc<dyn RenderNode>,
    ic: Option<&mut InvalidationController>,
    ctm: &skia_rust_core::matrix::Matrix,
) -> Rect {
    child.revalidate(ic, ctm)
}
