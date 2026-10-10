// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGDraw.h, modules/sksg/src/SkSGDraw.cpp (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Style;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::geometry_node::GeometryNode;
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::paint_node::PaintNode;
use crate::render_node::{Hit, RenderContext, RenderNode};

/// Draws a geometry with a paint.
// Port of: modules/sksg/include/SkSGDraw.h#L14-L34 (chrome/m156) (`class Draw`)
#[doc(alias = "sksg::Draw")]
#[derive(Debug)]
pub struct Draw {
    core: NodeCore,
    geometry: Rc<dyn GeometryNode>,
    paint: Rc<dyn PaintNode>,
}

impl Draw {
    /// Draws `geo` with `paint`. `None` if either is missing (`Draw::Make`).
    // Port of: modules/sksg/include/SkSGDraw.h#L17-L19 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        geo: Option<Rc<dyn GeometryNode>>,
        paint: Option<Rc<dyn PaintNode>>,
    ) -> Option<Rc<Self>> {
        let (geometry, paint) = (geo?, paint?);
        let draw = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            geometry: Rc::clone(&geometry),
            paint: Rc::clone(&paint),
        });
        // Port of: modules/sksg/src/SkSGDraw.cpp#L11-L14 (chrome/m156) (`Draw::Draw`)
        draw.observe_inval(draw.geometry.as_ref());
        draw.observe_inval(draw.paint.as_ref());
        Some(draw)
    }
}

impl Drop for Draw {
    // Port of: modules/sksg/src/SkSGDraw.cpp#L16-L19 (chrome/m156) (`Draw::~Draw`)
    fn drop(&mut self) {
        self.unobserve_inval(self.geometry.as_ref());
        self.unobserve_inval(self.paint.as_ref());
    }
}

impl Node for Draw {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGDraw.cpp#L58-L66 (chrome/m156) (`Draw::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let mut bounds = self.geometry.revalidate(ic.as_deref_mut(), ctm);
        self.paint.revalidate(ic, ctm);
        let paint = self.paint.make_paint();
        debug_assert!(paint.can_compute_fast_bounds());
        bounds = paint.compute_fast_bounds(&bounds);
        bounds
    }
}

impl RenderNode for Draw {
    // Port of: modules/sksg/src/SkSGDraw.cpp#L20-L35 (chrome/m156) (`Draw::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let mut paint = self.paint.make_paint();
        if let Some(ctx) = ctx {
            ctx.modulate_paint(&canvas.total_matrix(), &mut paint, false);
        }
        let skip_draw = paint.nothing_to_draw()
            || (paint.style() == Style::Stroke && paint.stroke_width() <= 0.0);
        if !skip_draw {
            self.geometry.draw(canvas, &paint);
        }
    }

    // Port of: modules/sksg/src/SkSGDraw.cpp#L37-L55 (chrome/m156) (`Draw::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        let paint = self.paint.make_paint();
        if paint.alpha() == 0 {
            return None;
        }
        if paint.style() == Style::Fill && self.geometry.contains(p) {
            return Some(Hit::This);
        }
        let (stroke_path, filled) = skia_rust_core::path_utils::fill_path_with_paint_to_path(
            &self.geometry.as_path(),
            &paint,
        );
        if !filled {
            return None;
        }
        // todo: can we share code (via SkPathRaw) for the impl of contains() in builder?
        if stroke_path.contains(p) {
            Some(Hit::This)
        } else {
            None
        }
    }
}
