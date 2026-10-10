// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGCircle.h, modules/svg/src/SkSVGCircle.cpp

//! The `circle` element (`SkSVGCircle`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, LengthType, RenderContext};
use crate::shape::{render_shape, set_parsed, svg_attr};
use crate::transformable_node::TransformableNode;
use crate::types::Length;
use crate::value::Value;

/// The `circle` element.
// Port of: modules/svg/include/SkSVGCircle.h#L21-L52 (chrome/m156)
#[doc(alias = "SkSVGCircle")]
#[derive(Debug, Clone)]
pub struct Circle {
    transformable: TransformableNode,
    cx: Length,
    cy: Length,
    r: Length,
}

impl Default for Circle {
    fn default() -> Self {
        Self::new()
    }
}

impl Circle {
    // Port of: modules/svg/src/SkSVGCircle.cpp#L18 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Circle),
            cx: Length::new(0.0),
            cy: Length::new(0.0),
            r: Length::new(0.0),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGCircle>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(cx, set_cx, cx, Length);
    svg_attr!(cy, set_cy, cy, Length);
    svg_attr!(r, set_r, r, Length);

    /// Resolves and returns the center and radius values.
    // Port of: modules/svg/src/SkSVGCircle.cpp#L28-L35 (chrome/m156)
    fn resolve(&self, lctx: &LengthContext) -> (Point, scalar) {
        let cx = lctx.resolve(&self.cx, LengthType::Horizontal);
        let cy = lctx.resolve(&self.cy, LengthType::Vertical);
        let r = lctx.resolve(&self.r, LengthType::Other);

        (Point::new(cx, cy), r)
    }

    // Port of: modules/svg/src/SkSVGCircle.cpp#L36-L44 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas, lctx: &LengthContext, paint: &SkPaint, _: PathFillType) {
        let (pos, r) = self.resolve(lctx);

        if r > 0.0 {
            canvas.draw_circle(pos, r, paint);
        }
    }
}

impl SvgNode for Circle {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGCircle.cpp#L20-L26 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.cx, AttributeParser::parse_named("cx", n, v))
            || set_parsed(&mut self.cy, AttributeParser::parse_named("cy", n, v))
            || set_parsed(&mut self.r, AttributeParser::parse_named("r", n, v))
    }

    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        self.transformable
            .on_prepare_to_render(self.has_children(), ctx)
    }

    fn on_render(&self, ctx: &RenderContext<'_>) {
        render_shape(ctx, |canvas, lctx, paint, fill_type| {
            self.on_draw(canvas, lctx, paint, fill_type);
        });
    }

    // Port of: modules/svg/src/SkSVGCircle.cpp#L46-L51 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        let (pos, r) = self.resolve(&ctx.length_context());

        self.transformable
            .map_to_parent_path(&Path::circle(pos, r, None))
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // Port of: modules/svg/src/SkSVGCircle.cpp#L53-L56 (chrome/m156)
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        let (pos, r) = self.resolve(ctx.length_context());
        let obb = Rect::from_xywh(pos.x - r, pos.y - r, 2.0 * r, 2.0 * r);
        self.transformable.on_object_bounding_box(ctx, obb)
    }
}
