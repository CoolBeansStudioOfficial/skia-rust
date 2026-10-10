// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGPoly.h, modules/svg/src/SkSVGPoly.cpp

//! The `polygon` and `polyline` elements (`SkSVGPoly`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, RenderContext};
use crate::shape::render_shape;
use crate::transformable_node::TransformableNode;
use crate::types::PointsType;
use crate::value::Value;

/// Handles `<polygon>` and `<polyline>` elements.
// Port of: modules/svg/include/SkSVGPoly.h#L21-L52 (chrome/m156)
#[doc(alias = "SkSVGPoly")]
#[derive(Debug, Clone)]
pub struct Poly {
    transformable: TransformableNode,
    points: PointsType,
    path: Path,
}

impl Poly {
    // Port of: modules/svg/include/SkSVGPoly.h#L24-L26 (chrome/m156)
    #[doc(alias = "MakePolygon")]
    #[must_use]
    pub fn new_polygon() -> Self {
        Self::with_tag(Tag::Polygon)
    }

    // Port of: modules/svg/include/SkSVGPoly.h#L28-L30 (chrome/m156)
    #[doc(alias = "MakePolyline")]
    #[must_use]
    pub fn new_polyline() -> Self {
        Self::with_tag(Tag::Polyline)
    }

    // Port of: modules/svg/src/SkSVGPoly.cpp#L18 (chrome/m156)
    fn with_tag(tag: Tag) -> Self {
        Self {
            transformable: TransformableNode::new(tag),
            points: PointsType::new(),
            path: Path::default(),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGPoly>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    #[must_use]
    pub fn points(&self) -> &PointsType {
        &self.points
    }

    pub fn set_points(&mut self, points: PointsType) {
        self.points = points;
    }

    // Port of: modules/svg/src/SkSVGPoly.cpp#L33-L40 (chrome/m156)
    fn on_draw(
        &self,
        canvas: &Canvas,
        _: &LengthContext,
        paint: &SkPaint,
        fill_type: PathFillType,
    ) {
        // the passed fillType follows inheritance rules and needs to be applied at draw time.
        // (`fPath` is mutable in Skia; a copy has the same effect.)
        let path = self.path.with_fill_type(fill_type);
        canvas.draw_path(&path, paint);
    }
}

impl SvgNode for Poly {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGPoly.cpp#L20-L31 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        if self.transformable.base_mut().parse_and_set_attribute(n, v) {
            return true;
        }

        if let Some(points) = AttributeParser::parse_named::<PointsType>("points", n, v) {
            // TODO: we can likely just keep the points array and create the SkPath when needed.
            // only polygons are auto-closed
            self.path = Path::polygon(&points, self.transformable.base().tag() == Tag::Polygon, None, None);
            self.points = points;
        }

        // No other attributes on this node
        false
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

    // Port of: modules/svg/src/SkSVGPoly.cpp#L42-L49 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        // clip-rule can be inherited and needs to be applied at clip time.
        let path = self.path.with_fill_type(
            ctx.presentation_context()
                .inherited
                .clip_rule
                .as_fill_type(),
        );

        self.transformable.map_to_parent_path(&path)
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // Port of: modules/svg/src/SkSVGPoly.cpp#L51-L53 (chrome/m156)
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.transformable
            .on_object_bounding_box(ctx, *self.path.bounds())
    }
}
