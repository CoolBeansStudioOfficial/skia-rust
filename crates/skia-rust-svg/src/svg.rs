// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGSVG.h, modules/svg/src/SkSVGSVG.cpp

//! The `svg` element (`SkSVGSVG`).

use std::sync::Arc;

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;

use crate::attribute::Attribute;
use crate::container::Container;
use crate::node::{Node, SvgNode, Tag, compute_viewbox_matrix, impl_svg_node_basics};
use crate::render_context::{BboxContext, LengthContext, LengthType, RenderContext};
use crate::shape::{svg_attr, svg_optional_attr};
use crate::types::{Iri, Length, LengthUnit, PreserveAspectRatio, ViewBoxType};
use crate::value::Value;

/// Whether an `svg` element is the outermost one (`SkSVGSVG::Type`).
// Port of: modules/svg/include/SkSVGSVG.h#L24-L27 (chrome/m156)
#[doc(alias = "SkSVGSVG::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgType {
    Root,
    Inner,
}

/// The `svg` element.
// Port of: modules/svg/include/SkSVGSVG.h#L22-L57 (chrome/m156)
#[doc(alias = "SkSVGSVG")]
#[derive(Debug, Clone)]
pub struct Svg {
    container: Container,
    x: Length,
    y: Length,
    width: Length,
    height: Length,
    preserve_aspect_ratio: PreserveAspectRatio,
    view_box: Option<ViewBoxType>,
    // Some attributes behave differently for the outermost svg element.
    ty: SvgType,
}

impl Default for Svg {
    fn default() -> Self {
        Self::new(SvgType::Inner)
    }
}

impl Svg {
    // Port of: modules/svg/include/SkSVGSVG.h#L28-L29 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new(ty: SvgType) -> Self {
        Self {
            container: Container::new(Tag::Svg),
            x: Length::new(0.0),
            y: Length::new(0.0),
            width: Length::with_unit(100.0, LengthUnit::Percentage),
            height: Length::with_unit(100.0, LengthUnit::Percentage),
            preserve_aspect_ratio: PreserveAspectRatio::default(),
            view_box: None,
            ty,
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGSVG>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }

    svg_attr!(x, set_x, x, Length);
    svg_attr!(y, set_y, y, Length);
    svg_attr!(width, set_width, width, Length);
    svg_attr!(height, set_height, height, Length);
    svg_attr!(
        preserve_aspect_ratio,
        set_preserve_aspect_ratio,
        preserve_aspect_ratio,
        PreserveAspectRatio
    );
    svg_optional_attr!(view_box, set_view_box, view_box, ViewBoxType);

    // https://www.w3.org/TR/SVG11/coords.html#IntrinsicSizing
    // Port of: modules/svg/src/SkSVGSVG.cpp#L100-L115 (chrome/m156)
    #[doc(alias = "intrinsicSize")]
    #[must_use]
    pub fn intrinsic_size(&self, lctx: &LengthContext) -> Size {
        // Percentage values do not provide an intrinsic size.
        if self.width.unit() == LengthUnit::Percentage
            || self.height.unit() == LengthUnit::Percentage
        {
            return Size::new(0.0, 0.0);
        }

        Size::new(
            lctx.resolve(&self.width, LengthType::Horizontal),
            lctx.resolve(&self.height, LengthType::Vertical),
        )
    }

    /// Renders the node with the given id as if it were the only child.
    // Port of: modules/svg/src/SkSVGSVG.cpp#L17-L36 (chrome/m156)
    #[doc(alias = "renderNode")]
    pub fn render_node(&self, ctx: &RenderContext<'_>, iri: &Iri) {
        let me: &dyn SvgNode = self;
        let mut local_context = RenderContext::with_node(ctx, me);
        let node = local_context.find_node_by_id(iri);
        let Some(node) = node.get() else {
            return;
        };

        if self.on_prepare_to_render(&mut local_context) {
            if self.base().key() == node.base().key() {
                self.on_render(ctx);
            } else {
                node.render(&local_context);
            }
        }
    }
}

impl SvgNode for Svg {
    impl_svg_node_basics!(container.transformable.base);

    fn append_child(&mut self, node: Node) {
        self.container.append_child(node);
    }

    fn has_children(&self) -> bool {
        self.container.has_children()
    }

    // Port of: modules/svg/src/SkSVGSVG.cpp#L38-L76 (chrome/m156)
    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        // x/y are ignored for outermost svg elements
        let zero = Length::new(0.0);
        let x = if self.ty == SvgType::Inner { &self.x } else { &zero };
        let y = if self.ty == SvgType::Inner { &self.y } else { &zero };

        let view_port_rect = ctx
            .length_context()
            .resolve_rect(x, y, &self.width, &self.height);
        let mut content_matrix = Matrix::translate((view_port_rect.x(), view_port_rect.y()));
        let mut view_port = Size::new(view_port_rect.width(), view_port_rect.height());

        if let Some(view_box) = &self.view_box {
            // An empty viewbox disables rendering.
            if view_box.is_empty() {
                return false;
            }

            // A viewBox overrides the intrinsic viewport.
            view_port = Size::new(view_box.width(), view_box.height());

            content_matrix.pre_concat(&compute_viewbox_matrix(
                view_box,
                &view_port_rect,
                self.preserve_aspect_ratio,
            ));
        }

        if !content_matrix.is_identity() {
            ctx.save_once();
            ctx.canvas().concat(&content_matrix);
        }

        if view_port != *ctx.length_context().view_port() {
            ctx.update_length_context(|lctx| lctx.set_view_port(view_port));
        }

        self.container.on_prepare_to_render(ctx)
    }

    fn on_render(&self, ctx: &RenderContext<'_>) {
        self.container.on_render(ctx);
    }

    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        self.container.on_as_path(ctx)
    }

    // Port of: modules/svg/src/SkSVGSVG.cpp#L78-L98 (chrome/m156)
    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        match attr {
            Attribute::X => {
                if let Some(x) = v.as_length() {
                    self.x = *x;
                }
            }
            Attribute::Y => {
                if let Some(y) = v.as_length() {
                    self.y = *y;
                }
            }
            Attribute::Width => {
                if let Some(w) = v.as_length() {
                    self.width = *w;
                }
            }
            Attribute::Height => {
                if let Some(h) = v.as_length() {
                    self.height = *h;
                }
            }
            Attribute::ViewBox => {
                if let Some(vb) = v.as_view_box() {
                    self.view_box = Some(*vb);
                }
            }
            Attribute::PreserveAspectRatio => {
                if let Some(par) = v.as_preserve_aspect_ratio() {
                    self.preserve_aspect_ratio = *par;
                }
            }
            _ => self.container.on_set_attribute(attr, v),
        }
    }

    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.container.on_object_bounding_box(ctx)
    }
}
