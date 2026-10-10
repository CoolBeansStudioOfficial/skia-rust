// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGLine.h, modules/svg/src/SkSVGLine.cpp

//! The `line` element (`SkSVGLine`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, LengthType, RenderContext};
use crate::shape::{render_shape, set_parsed, svg_attr};
use crate::transformable_node::TransformableNode;
use crate::types::Length;
use crate::value::Value;

/// The `line` element.
// Port of: modules/svg/include/SkSVGLine.h#L21-L49 (chrome/m156)
#[doc(alias = "SkSVGLine")]
#[derive(Debug, Clone)]
pub struct Line {
    transformable: TransformableNode,
    x1: Length,
    y1: Length,
    x2: Length,
    y2: Length,
}

impl Default for Line {
    fn default() -> Self {
        Self::new()
    }
}

impl Line {
    // Port of: modules/svg/src/SkSVGLine.cpp#L18 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Line),
            x1: Length::new(0.0),
            y1: Length::new(0.0),
            x2: Length::new(0.0),
            y2: Length::new(0.0),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGLine>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(x1, set_x1, x1, Length);
    svg_attr!(y1, set_y1, y1, Length);
    svg_attr!(x2, set_x2, x2, Length);
    svg_attr!(y2, set_y2, y2, Length);

    /// Resolves and returns the two endpoints.
    // Port of: modules/svg/src/SkSVGLine.cpp#L29-L36 (chrome/m156)
    fn resolve(&self, lctx: &LengthContext) -> (Point, Point) {
        (
            Point::new(
                lctx.resolve(&self.x1, LengthType::Horizontal),
                lctx.resolve(&self.y1, LengthType::Vertical),
            ),
            Point::new(
                lctx.resolve(&self.x2, LengthType::Horizontal),
                lctx.resolve(&self.y2, LengthType::Vertical),
            ),
        )
    }

    // Port of: modules/svg/src/SkSVGLine.cpp#L38-L44 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas, lctx: &LengthContext, paint: &SkPaint, _: PathFillType) {
        let (p0, p1) = self.resolve(lctx);

        canvas.draw_line(p0, p1, paint);
    }
}

impl SvgNode for Line {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGLine.cpp#L20-L27 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.x1, AttributeParser::parse_named("x1", n, v))
            || set_parsed(&mut self.y1, AttributeParser::parse_named("y1", n, v))
            || set_parsed(&mut self.x2, AttributeParser::parse_named("x2", n, v))
            || set_parsed(&mut self.y2, AttributeParser::parse_named("y2", n, v))
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

    // Port of: modules/svg/src/SkSVGLine.cpp#L46-L49 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        let (p0, p1) = self.resolve(&ctx.length_context());

        self.transformable.map_to_parent_path(&Path::line(p0, p1))
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // `SkSVGLine` does not override `onTransformableObjectBoundingBox`: it is empty.
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.transformable
            .on_object_bounding_box(ctx, Rect::new_empty())
    }
}
