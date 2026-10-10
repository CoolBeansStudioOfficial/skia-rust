// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGGroup.h, modules/sksg/src/SkSGGroup.cpp (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::render_node::{
    Hit, RenderContext, RenderNode, ScopedRenderContext, has_children_inval, node_at,
    observe_children, unobserve_children,
};

/// A render node that draws its children in order. Children that overlap require the group to be
/// isolated in a layer when it has paint overrides.
// Port of: modules/sksg/include/SkSGGroup.h#L11-L40 (chrome/m156) (`class Group`)
#[doc(alias = "sksg::Group")]
#[derive(Debug)]
pub struct Group {
    core: NodeCore,
    children: RefCell<Vec<Rc<dyn RenderNode>>>,
    requires_isolation: Cell<bool>,
}

impl Group {
    /// An empty group (`Group::Make()`).
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> Rc<Self> {
        Self::make_with_children(&[])
    }

    /// A group of `children` (`Group::Make(std::vector<sk_sp<RenderNode>>)`).
    // Port of: modules/sksg/include/SkSGGroup.h#L14-L17 (chrome/m156)
    #[must_use]
    pub fn make_with_children(children: &[Rc<dyn RenderNode>]) -> Rc<Self> {
        let group = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            children: RefCell::new(children.to_vec()),
            requires_isolation: Cell::new(true),
        });
        // Port of: modules/sksg/src/SkSGGroup.cpp#L13-L19 (chrome/m156) (`Group::Group`)
        observe_children(group.as_ref(), children);
        group
    }

    /// Adds `node` as the last child, unless it is already a child (`addChild`).
    // Port of: modules/sksg/src/SkSGGroup.cpp#L31-L40 (chrome/m156) (`Group::addChild`)
    #[doc(alias = "addChild")]
    pub fn add_child(&self, node: Rc<dyn RenderNode>) {
        // should we allow duplicates?
        if self
            .children
            .borrow()
            .iter()
            .any(|child| Rc::ptr_eq(child, &node))
        {
            return;
        }
        self.observe_inval(node.as_ref());
        self.children.borrow_mut().push(node);
        self.invalidate();
    }

    /// Removes `node` from the children (`removeChild`).
    // Port of: modules/sksg/src/SkSGGroup.cpp#L42-L49 (chrome/m156) (`Group::removeChild`)
    #[doc(alias = "removeChild")]
    pub fn remove_child(&self, node: &Rc<dyn RenderNode>) {
        self.children
            .borrow_mut()
            .retain(|child| !Rc::ptr_eq(child, node));
        self.unobserve_inval(node.as_ref());
        self.invalidate();
    }

    /// The number of children (`size`).
    #[must_use]
    pub fn size(&self) -> usize {
        self.children.borrow().len()
    }

    /// True if there are no children (`empty`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.borrow().is_empty()
    }

    /// Removes all children (`clear`). The node is not invalidated.
    // Port of: modules/sksg/src/SkSGGroup.cpp#L21-L28 (chrome/m156) (`Group::clear`)
    pub fn clear(&self) {
        let children = std::mem::take(&mut *self.children.borrow_mut());
        unobserve_children(self, &children);
    }

    /// True if any child needs revalidation.
    #[must_use]
    pub fn has_children_inval(&self) -> bool {
        has_children_inval(&self.children.borrow())
    }
}

impl Drop for Group {
    // Port of: modules/sksg/src/SkSGGroup.cpp#L18-L20 (chrome/m156) (`Group::~Group`)
    fn drop(&mut self) {
        let children = self.children.get_mut().clone();
        unobserve_children(self, &children);
    }
}

impl Node for Group {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGGroup.cpp#L101-L127 (chrome/m156) (`Group::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let mut bounds = Rect::new_empty();
        self.requires_isolation.set(false);
        // Snapshot the children: revalidation may run arbitrary code.
        let children = self.children.borrow().clone();
        for (i, child) in children.iter().enumerate() {
            let child_bounds = child.revalidate(ic.as_deref_mut(), ctm);
            // If any of the child nodes overlap, group effects require layer isolation.
            // Testing conservatively against the union of prev bounds is cheap and good enough.
            if !self.requires_isolation.get() && i > 0 && child_bounds.intersects(bounds) {
                self.requires_isolation.set(true);
            }
            bounds.join(child_bounds);
        }
        bounds
    }
}

impl RenderNode for Group {
    // Port of: modules/sksg/src/SkSGGroup.cpp#L52-L62 (chrome/m156) (`Group::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &self.core.bounds(),
            &canvas.total_matrix(),
            self.requires_isolation.get(),
        );
        let children = self.children.borrow().clone();
        for child in &children {
            child.render(canvas, Some(scope.context()));
        }
    }

    // Port of: modules/sksg/src/SkSGGroup.cpp#L64-L72 (chrome/m156) (`Group::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        let children = self.children.borrow().clone();
        for child in children.iter().rev() {
            if let Some(node) = node_at(child, p) {
                return Some(Hit::Child(node));
            }
        }
        None
    }
}
