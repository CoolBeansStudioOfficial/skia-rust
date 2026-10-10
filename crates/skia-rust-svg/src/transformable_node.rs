// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGTransformableNode.h,
// modules/svg/src/SkSVGTransformableNode.cpp

//! Nodes with a `transform` (`SkSVGTransformableNode`).

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;

use crate::attribute::Attribute;
use crate::node::{NodeBase, Tag};
use crate::render_context::{BboxContext, RenderContext};
use crate::types::TransformType;
use crate::value::Value;

/// The data and behaviour that `SkSVGTransformableNode` adds to `SkSVGNode`. Node types that
/// derive from it embed one.
// Port of: modules/svg/include/SkSVGTransformableNode.h#L19-L47 (chrome/m156)
#[doc(alias = "SkSVGTransformableNode")]
#[derive(Debug, Clone)]
pub struct TransformableNode {
    pub(crate) base: NodeBase,
    // FIXME: should be sparse
    transform: TransformType,
}

impl TransformableNode {
    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L18-L20 (chrome/m156)
    #[must_use]
    pub fn new(tag: Tag) -> Self {
        Self {
            base: NodeBase::new(tag),
            transform: Matrix::new_identity(),
        }
    }

    #[must_use]
    pub fn base(&self) -> &NodeBase {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }

    #[doc(alias = "setTransform")]
    pub fn set_transform(&mut self, t: &TransformType) {
        self.transform = t.clone();
    }

    /// The transform of the node.
    #[must_use]
    pub fn transform(&self) -> &TransformType {
        &self.transform
    }

    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L23-L30 (chrome/m156)
    #[doc(alias = "onPrepareToRender")]
    #[must_use]
    pub fn on_prepare_to_render(&self, has_children: bool, ctx: &mut RenderContext<'_>) -> bool {
        if !self.transform.is_identity() {
            ctx.save_once();
            ctx.canvas().concat(&self.transform);
        }

        self.base.on_prepare_to_render(has_children, ctx)
    }

    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L32-L42 (chrome/m156)
    #[doc(alias = "onSetAttribute")]
    pub fn on_set_attribute(&mut self, attr: Attribute, v: &Value<'_>) {
        if attr == Attribute::Transform
            && let Some(transform) = v.as_transform()
        {
            self.set_transform(transform);
        }
    }

    /// Transforms the path to parent node coordinates.
    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L44-L47 (chrome/m156)
    #[doc(alias = "mapToParent")]
    #[must_use]
    pub fn map_to_parent_path(&self, path: &Path) -> Path {
        path.make_transform(&self.transform)
    }

    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L49-L51 (chrome/m156)
    #[doc(alias = "mapToParent")]
    #[must_use]
    pub fn map_to_parent_rect(&self, rect: &Rect) -> Rect {
        self.transform.map_rect(rect).0
    }

    /// `SkSVGTransformableNode::onObjectBoundingBox`, given the box that
    /// `onTransformableObjectBoundingBox` computed.
    // Port of: modules/svg/src/SkSVGTransformableNode.cpp#L56-L63 (chrome/m156)
    #[doc(alias = "onObjectBoundingBox")]
    #[must_use]
    pub fn on_object_bounding_box(&self, ctx: &BboxContext<'_>, transformable_obb: Rect) -> Rect {
        let mut obb = transformable_obb;

        if !ctx.is_scope_node(&self.base) && !self.transform.is_identity() {
            obb = self.map_to_parent_rect(&obb);
        }
        obb
    }
}
