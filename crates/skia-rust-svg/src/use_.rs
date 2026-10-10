// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGUse.h, modules/svg/src/SkSVGUse.cpp

//! The `use` element (`SkSVGUse`).

use std::sync::Arc;

use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;

use crate::attribute::Attribute;
use crate::attribute_parser::AttributeParser;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthType, RenderContext};
use crate::shape::{set_parsed, svg_attr};
use crate::transformable_node::TransformableNode;
use crate::types::{Iri, Length};
use crate::value::Value;

/// The `use` element.
// Port of: modules/svg/include/SkSVGUse.h#L19-L45 (chrome/m156)
#[doc(alias = "SkSVGUse")]
#[derive(Debug, Clone)]
pub struct Use {
    transformable: TransformableNode,
    x: Length,
    y: Length,
    href: Iri,
}

impl Default for Use {
    fn default() -> Self {
        Self::new()
    }
}

impl Use {
    // Port of: modules/svg/src/SkSVGUse.cpp#L18 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transformable: TransformableNode::new(Tag::Use),
            x: Length::new(0.0),
            y: Length::new(0.0),
            href: Iri::default(),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGUse>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(x, set_x, x, Length);
    svg_attr!(y, set_y, y, Length);
    svg_attr!(href, set_href, href, Iri);
}

impl SvgNode for Use {
    impl_svg_node_basics!(transformable.base);

    // Port of: modules/svg/src/SkSVGUse.cpp#L20-L22 (chrome/m156)
    fn append_child(&mut self, _node: Node) {
        // cannot append child nodes to this element.
    }

    // Port of: modules/svg/src/SkSVGUse.cpp#L24-L28 (chrome/m156)
    fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        self.transformable.base_mut().parse_and_set_attribute(n, v)
            || set_parsed(&mut self.x, AttributeParser::parse_named("x", n, v))
            || set_parsed(&mut self.y, AttributeParser::parse_named("y", n, v))
            || set_parsed(
                &mut self.href,
                AttributeParser::parse_named("xlink:href", n, v),
            )
    }

    // Port of: modules/svg/src/SkSVGUse.cpp#L30-L44 (chrome/m156)
    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        if self.href.iri().is_empty()
            || !self
                .transformable
                .on_prepare_to_render(self.has_children(), ctx)
        {
            return false;
        }

        if self.x.value() != 0.0 || self.y.value() != 0.0 {
            // Restored when the local SkSVGRenderContext leaves scope.
            ctx.save_once();
            ctx.canvas().translate((self.x.value(), self.y.value()));
        }

        // TODO: width/height override for <svg> targets.

        true
    }

    // Port of: modules/svg/src/SkSVGUse.cpp#L46-L53 (chrome/m156)
    fn on_render(&self, ctx: &RenderContext<'_>) {
        let reference = ctx.find_node_by_id(&self.href);
        let Some(reference) = reference.get() else {
            return;
        };

        reference.render(ctx);
    }

    // Port of: modules/svg/src/SkSVGUse.cpp#L55-L62 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        let reference = ctx.find_node_by_id(&self.href);
        let Some(reference) = reference.get() else {
            return Path::default();
        };

        reference.as_path(ctx)
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    // Port of: modules/svg/src/SkSVGUse.cpp#L64-L77 (chrome/m156)
    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        let obb = {
            let reference = ctx.find_node_by_id(&self.href);
            match reference.get() {
                None => Rect::new_empty(),
                Some(reference) => {
                    let lctx = ctx.length_context();
                    let x = lctx.resolve(&self.x, LengthType::Horizontal);
                    let y = lctx.resolve(&self.y, LengthType::Vertical);

                    let mut bounds = reference.on_object_bounding_box(ctx);
                    bounds.offset((x, y));

                    bounds
                }
            }
        };
        self.transformable.on_object_bounding_box(ctx, obb)
    }
}
