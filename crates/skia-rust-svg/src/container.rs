// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGContainer.h, modules/svg/src/SkSVGContainer.cpp,
// modules/svg/include/SkSVGHiddenContainer.h, modules/svg/include/SkSVGG.h,
// modules/svg/include/SkSVGDefs.h

//! Nodes with children: `Container`, and the `g`, `a` and `defs` elements.

use std::sync::Arc;

use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::PathOpsExt;
use skia_rust_pathops::path_op::PathOp;

use crate::attribute::Attribute;
use crate::node::{Node, SvgNode, Tag, impl_svg_node_basics, impl_svg_node_by_delegation};
use crate::render_context::{BboxContext, RenderContext};
use crate::transformable_node::TransformableNode;
use crate::value::Value;

/// A node that has children (`SkSVGContainer`).
// Port of: modules/svg/include/SkSVGContainer.h#L19-L48 (chrome/m156)
#[doc(alias = "SkSVGContainer")]
#[derive(Debug, Clone)]
pub struct Container {
    pub(crate) transformable: TransformableNode,
    children: Vec<Node>,
}

impl Container {
    // Port of: modules/svg/src/SkSVGContainer.cpp#L18 (chrome/m156)
    #[must_use]
    pub(crate) fn new(tag: Tag) -> Self {
        Self {
            transformable: TransformableNode::new(tag),
            children: Vec::new(),
        }
    }

    /// The children of the node.
    #[must_use]
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    /// The data of the transformable node this container derives from.
    #[must_use]
    pub fn transformable(&self) -> &TransformableNode {
        &self.transformable
    }

    /// Calls `func` for each child with the given tag.
    // Port of: modules/svg/include/SkSVGContainer.h#L36-L42 (chrome/m156)
    #[doc(alias = "forEachChild")]
    pub fn for_each_child(&self, tag: Tag, mut func: impl FnMut(&dyn SvgNode)) {
        for child in &self.children {
            if child.tag() == tag {
                func(&**child);
            }
        }
    }

    // Port of: modules/svg/src/SkSVGContainer.cpp#L48-L56 (chrome/m156)
    #[must_use]
    pub(crate) fn on_transformable_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        let mut bounds = Rect::new_empty();

        for child in &self.children {
            let child_bounds = child.on_object_bounding_box(ctx);
            bounds.join(child_bounds);
        }

        bounds
    }
}

impl SvgNode for Container {
    impl_svg_node_basics!(transformable.base);

    // Port of: modules/svg/src/SkSVGContainer.cpp#L20-L23 (chrome/m156)
    fn append_child(&mut self, node: Node) {
        self.children.push(node);
    }

    // Port of: modules/svg/src/SkSVGContainer.cpp#L25-L27 (chrome/m156)
    fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        self.transformable
            .on_prepare_to_render(self.has_children(), ctx)
    }

    // Port of: modules/svg/src/SkSVGContainer.cpp#L29-L33 (chrome/m156)
    fn on_render(&self, ctx: &RenderContext<'_>) {
        for child in &self.children {
            child.render(ctx);
        }
    }

    // Port of: modules/svg/src/SkSVGContainer.cpp#L35-L46 (chrome/m156)
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        let mut path = Path::default();

        for child in &self.children {
            let child_path = child.as_path(ctx);

            if let Some(result) = path.op(&child_path, PathOp::Union) {
                path = result;
            }
        }

        self.transformable.map_to_parent_path(&path)
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.transformable.on_set_attribute(attr, v);
    }

    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.transformable
            .on_object_bounding_box(ctx, self.on_transformable_object_bounding_box(ctx))
    }
}

/// A container whose content is not rendered directly (`SkSVGHiddenContainer`).
// Port of: modules/svg/include/SkSVGHiddenContainer.h#L14-L24 (chrome/m156)
#[doc(alias = "SkSVGHiddenContainer")]
#[derive(Debug, Clone)]
pub struct HiddenContainer {
    container: Container,
}

impl HiddenContainer {
    #[must_use]
    pub(crate) fn new(tag: Tag) -> Self {
        Self {
            container: Container::new(tag),
        }
    }

    /// The container this one derives from.
    #[must_use]
    pub fn container(&self) -> &Container {
        &self.container
    }
}

impl SvgNode for HiddenContainer {
    impl_svg_node_basics!(container.transformable.base);

    fn append_child(&mut self, node: Node) {
        self.container.append_child(node);
    }

    fn has_children(&self) -> bool {
        self.container.has_children()
    }

    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        self.container.on_prepare_to_render(ctx)
    }

    // Port of: modules/svg/include/SkSVGHiddenContainer.h#L19 (chrome/m156)
    fn on_render(&self, _ctx: &RenderContext<'_>) {}

    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path {
        self.container.on_as_path(ctx)
    }

    fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        self.container.on_set_attribute(attr, v);
    }

    fn on_object_bounding_box(&self, ctx: &BboxContext<'_>) -> Rect {
        self.container.on_object_bounding_box(ctx)
    }

    fn on_as_paint(&self, _ctx: &RenderContext<'_>, _paint: &mut SkPaint) -> bool {
        false
    }
}

/// The `g` and `a` elements (`SkSVGG`).
// Port of: modules/svg/include/SkSVGG.h#L14-L22 (chrome/m156)
#[doc(alias = "SkSVGG")]
#[derive(Debug, Clone)]
pub struct G {
    container: Container,
}

impl Default for G {
    fn default() -> Self {
        Self::new()
    }
}

impl G {
    // Port of: modules/svg/include/SkSVGG.h#L16-L19 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            container: Container::new(Tag::G),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGG>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }
}

impl SvgNode for G {
    impl_svg_node_by_delegation!(container);
}

/// The `defs` element (`SkSVGDefs`).
// Port of: modules/svg/include/SkSVGDefs.h#L14-L22 (chrome/m156)
#[doc(alias = "SkSVGDefs")]
#[derive(Debug, Clone)]
pub struct Defs {
    container: HiddenContainer,
}

impl Default for Defs {
    fn default() -> Self {
        Self::new()
    }
}

impl Defs {
    // Port of: modules/svg/include/SkSVGDefs.h#L16-L19 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            container: HiddenContainer::new(Tag::Defs),
        }
    }

    /// Wraps the node for use in a tree (`sk_sp<SkSVGDefs>`).
    #[must_use]
    pub fn into_node(self) -> Node {
        Arc::new(self)
    }
}

impl SvgNode for Defs {
    impl_svg_node_by_delegation!(container);
}
