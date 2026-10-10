// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGPath.h, modules/svg/src/SkSVGPath.cpp

//! The `path` element (`SkSVGPath`).

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::parse_path;

use crate::attribute::Attribute;
use crate::attribute_parser::{AttributeParser, Parse};
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, RenderContext};
use crate::shape::{render_shape, set_parsed, svg_attr};
use crate::transformable_node::TransformableNode;
use crate::value::Value;

// Port of: modules/svg/src/SkSVGPath.cpp#L21-L28 (chrome/m156)
impl Parse for SkPath {
    fn parse(parser: &mut AttributeParser<'_>) -> Option<Self> {
        parse_path::from_svg(parser.remaining())
    }
}

/// The `path` element.
// Port of: modules/svg/include/SkSVGPath.h#L21-L42 (chrome/m156)
#[doc(alias = "SkSVGPath")]
#[derive(Debug, Clone)]
pub struct Path {
    transformable: TransformableNode,
    path: SkPath,
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl Path {
    // Port of: modules/svg/src/SkSVGPath.cpp#L15 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Path),
            path: SkPath::default(),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGPath>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(path, set_path, path, SkPath);

    // Port of: modules/svg/src/SkSVGPath.cpp#L30-L36 (chrome/m156)
    fn on_draw(
        &self,
        canvas: &Canvas,
        _: &LengthContext,
        paint: &SkPaint,
        fill_type: PathFillType,
    ) {
        // the passed fillType follows inheritance rules and needs to be applied at draw time.
        let path = self.path.with_fill_type(fill_type); // Note: point and verb data are CoW
        canvas.draw_path(&path, paint);
    }
}

impl SvgNode for Path {
    impl_svg_node_basics!(transformable.base);

    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to an SVG shape.
    }

    // Port of: modules/svg/src/SkSVGPath.cpp#L17-L19 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.path, AttributeParser::parse_named("d", n, v))
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

    // Port of: modules/svg/src/SkSVGPath.cpp#L38-L43 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> SkPath {
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

    // Port of: modules/svg/src/SkSVGPath.cpp#L45-L47 (chrome/m156)
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.transformable
            .on_object_bounding_box(ctx, self.path.compute_tight_bounds())
    }
}
