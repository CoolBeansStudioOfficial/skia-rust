// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGRect.h, modules/svg/src/SkSVGRect.cpp,
// modules/svg/src/SkSVGRectPriv.h

//! The `rect` element (`SkSVGRect`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::rrect::RRect;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, LengthType, RenderContext};
use crate::shape::{render_shape, set_parsed, set_parsed_optional, svg_attr, svg_optional_attr};
use crate::transformable_node::TransformableNode;
use crate::types::Length;
use crate::value::Value;

// Port of: modules/svg/src/SkSVGRect.cpp#L14-L50 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ names (opt_rx, opt_ry)
pub(crate) fn resolve_optional_radii(
    opt_rx: Option<&Length>,
    opt_ry: Option<&Length>,
    lctx: &LengthContext,
) -> (f32, f32) {
    // https://www.w3.org/TR/SVG2/shapes.html#RectElement
    //
    // The used values for rx and ry are determined from the computed values by following these
    // steps in order:
    //
    // 1. If both rx and ry have a computed value of auto (since auto is the initial value for both
    //    properties, this will also occur if neither are specified by the author or if all
    //    author-supplied values are invalid), then the used value of both rx and ry is 0.
    //    (This will result in square corners.)
    // 2. Otherwise, convert specified values to absolute values as follows:
    //     1. If rx is set to a length value or a percentage, but ry is auto, calculate an absolute
    //        length equivalent for rx, resolving percentages against the used width of the
    //        rectangle; the absolute value for ry is the same.
    //     2. If ry is set to a length value or a percentage, but rx is auto, calculate the absolute
    //        length equivalent for ry, resolving percentages against the used height of the
    //        rectangle; the absolute value for rx is the same.
    //     3. If both rx and ry were set to lengths or percentages, absolute values are generated
    //        individually, resolving rx percentages against the used width, and resolving ry
    //        percentages against the used height.
    let rx = opt_rx.map_or(0.0, |l| lctx.resolve(l, LengthType::Horizontal));
    let ry = opt_ry.map_or(0.0, |l| lctx.resolve(l, LengthType::Vertical));

    (
        if opt_rx.is_some() { rx } else { ry },
        if opt_ry.is_some() { ry } else { rx },
    )
}

/// The `rect` element.
// Port of: modules/svg/include/SkSVGRect.h#L21-L52 (chrome/m156)
#[doc(alias = "SkSVGRect")]
#[derive(Debug, Clone)]
pub struct Rect {
    transformable: TransformableNode,
    x: Length,
    y: Length,
    width: Length,
    height: Length,
    rx: Option<Length>,
    ry: Option<Length>,
}

impl Default for Rect {
    fn default() -> Self {
        Self::new()
    }
}

impl Rect {
    // Port of: modules/svg/src/SkSVGRect.cpp#L52 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Rect),
            x: Length::new(0.0),
            y: Length::new(0.0),
            width: Length::new(0.0),
            height: Length::new(0.0),
            rx: None,
            ry: None,
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGRect>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(x, set_x, x, Length);
    svg_attr!(y, set_y, y, Length);
    svg_attr!(width, set_width, width, Length);
    svg_attr!(height, set_height, height, Length);
    svg_optional_attr!(rx, set_rx, rx, Length);
    svg_optional_attr!(ry, set_ry, ry, Length);

    // Port of: modules/svg/src/SkSVGRect.cpp#L65-L84 (chrome/m156)
    fn resolve(&self, lctx: &LengthContext) -> RRect {
        let rect = lctx.resolve_rect(&self.x, &self.y, &self.width, &self.height);
        let (rx, ry) = resolve_optional_radii(self.rx.as_ref(), self.ry.as_ref(), lctx);

        // https://www.w3.org/TR/SVG2/shapes.html#RectElement
        // ...
        // 3. Finally, apply clamping to generate the used values:
        //     1. If the absolute rx (after the above steps) is greater than half of the used width,
        //        then the used value of rx is half of the used width.
        //     2. If the absolute ry (after the above steps) is greater than half of the used
        //        height, then the used value of ry is half of the used height.
        //     3. Otherwise, the used values of rx and ry are the absolute values computed
        //        previously.

        // std::min(a, b) is `(b < a) ? b : a`
        let half_w = rect.width() / 2.0;
        let half_h = rect.height() / 2.0;
        RRect::new_rect_xy(
            rect,
            if half_w < rx { half_w } else { rx },
            if half_h < ry { half_h } else { ry },
        )
    }

    // Port of: modules/svg/src/SkSVGRect.cpp#L86-L89 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas, lctx: &LengthContext, paint: &SkPaint, _: PathFillType) {
        canvas.draw_rrect(self.resolve(lctx), paint);
    }
}

impl SvgNode for Rect {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGRect.cpp#L54-L63 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.x, AttributeParser::parse_named("x", n, v))
            || set_parsed(&mut self.y, AttributeParser::parse_named("y", n, v))
            || set_parsed(&mut self.width, AttributeParser::parse_named("width", n, v))
            || set_parsed(&mut self.height, AttributeParser::parse_named("height", n, v))
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

    // Port of: modules/svg/src/SkSVGRect.cpp#L91-L93 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        self.transformable
            .map_to_parent_path(&Path::rrect(self.resolve(&ctx.length_context()), None))
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // Port of: modules/svg/src/SkSVGRect.cpp#L95-L97 (chrome/m156)
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> SkRect {
        let obb = ctx
            .length_context()
            .resolve_rect(&self.x, &self.y, &self.width, &self.height);
        self.transformable.on_object_bounding_box(ctx, obb)
    }
}
