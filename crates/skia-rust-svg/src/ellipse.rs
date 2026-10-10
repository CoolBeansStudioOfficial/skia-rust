// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGEllipse.h, modules/svg/src/SkSVGEllipse.cpp

//! The `ellipse` element (`SkSVGEllipse`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::rect::resolve_optional_radii;
use crate::render_context::{BboxContext, LengthContext, LengthType, RenderContext};
use crate::shape::{render_shape, set_parsed, set_parsed_optional, svg_attr, svg_optional_attr};
use crate::transformable_node::TransformableNode;
use crate::types::Length;
use crate::value::Value;

/// The `ellipse` element.
// Port of: modules/svg/include/SkSVGEllipse.h#L21-L47 (chrome/m156)
#[doc(alias = "SkSVGEllipse")]
#[derive(Debug, Clone)]
pub struct Ellipse {
    transformable: TransformableNode,
    cx: Length,
    cy: Length,
    rx: Option<Length>,
    ry: Option<Length>,
}

impl Default for Ellipse {
    fn default() -> Self {
        Self::new()
    }
}

impl Ellipse {
    // Port of: modules/svg/src/SkSVGEllipse.cpp#L18 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Ellipse),
            cx: Length::new(0.0),
            cy: Length::new(0.0),
            rx: None,
            ry: None,
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGEllipse>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(cx, set_cx, cx, Length);
    svg_attr!(cy, set_cy, cy, Length);
    svg_optional_attr!(rx, set_rx, rx, Length);
    svg_optional_attr!(ry, set_ry, ry, Length);

    // Port of: modules/svg/src/SkSVGEllipse.cpp#L28-L44 (chrome/m156)
    fn resolve(&self, lctx: &LengthContext) -> Rect {
        let cx = lctx.resolve(&self.cx, LengthType::Horizontal);
        let cy = lctx.resolve(&self.cy, LengthType::Vertical);

        // https://www.w3.org/TR/SVG2/shapes.html#EllipseElement
        //
        // An auto value for either rx or ry is converted to a used value, following the rules
        // given above for rectangles (but without any clamping based on width or height).
        let (rx, ry) = resolve_optional_radii(self.rx.as_ref(), self.ry.as_ref(), lctx);

        // A computed value of zero for either dimension, or a computed value of auto for both
        // dimensions, disables rendering of the element.
        if rx > 0.0 && ry > 0.0 {
            Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0)
        } else {
            Rect::new_empty()
        }
    }

    // Port of: modules/svg/src/SkSVGEllipse.cpp#L46-L49 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas, lctx: &LengthContext, paint: &SkPaint, _: PathFillType) {
        canvas.draw_oval(self.resolve(lctx), paint);
    }
}

impl SvgNode for Ellipse {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGEllipse.cpp#L20-L26 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.cx, AttributeParser::parse_named("cx", n, v))
            || set_parsed(&mut self.cy, AttributeParser::parse_named("cy", n, v))
            || set_parsed_optional(&mut self.rx, AttributeParser::parse_named("rx", n, v))
            || set_parsed_optional(&mut self.ry, AttributeParser::parse_named("ry", n, v))
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

    // Port of: modules/svg/src/SkSVGEllipse.cpp#L51-L54 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        self.transformable
            .map_to_parent_path(&Path::oval(self.resolve(&ctx.length_context()), None))
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // `SkSVGEllipse` does not override `onTransformableObjectBoundingBox`: it is empty.
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.transformable
            .on_object_bounding_box(ctx, Rect::new_empty())
    }
}
