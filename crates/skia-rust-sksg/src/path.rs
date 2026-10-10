// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGPath.h, modules/sksg/src/SkSGPath.cpp (chrome/m156)

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};

/// A path geometry.
// Port of: modules/sksg/include/SkSGPath.h#L12-L45 (chrome/m156) (`class Path`)
#[doc(alias = "sksg::Path")]
#[derive(Debug)]
pub struct Path {
    core: NodeCore,
    path: RefCell<SkPath>,
}

impl Path {
    /// An empty path geometry (`Path::Make()`).
    #[must_use]
    pub fn make_empty() -> Rc<Self> {
        Self::make(SkPath::default())
    }

    /// A path geometry (`Path::Make(const SkPath&)`).
    // Port of: modules/sksg/include/SkSGPath.h#L15-L16 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(path: SkPath) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            path: RefCell::new(path),
        })
    }

    /// The path.
    #[must_use]
    pub fn path(&self) -> SkPath {
        self.path.borrow().clone()
    }

    /// Sets the path, invalidating the node if it changed (`setPath`).
    #[doc(alias = "setPath")]
    pub fn set_path(&self, path: SkPath) {
        if *self.path.borrow() == path {
            return;
        }
        *self.path.borrow_mut() = path;
        self.invalidate();
    }

    /// The fill type of the path.
    // Port of: modules/sksg/include/SkSGPath.h#L26-L28 (chrome/m156) (`Path::getFillType`)
    #[doc(alias = "getFillType")]
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        self.path.borrow().fill_type()
    }

    /// Sets the fill type, invalidating the node if it changed (`setFillType`).
    // Port of: modules/sksg/include/SkSGPath.h#L29-L36 (chrome/m156) (`Path::setFillType`)
    #[doc(alias = "setFillType")]
    pub fn set_fill_type(&self, fill_type: PathFillType) {
        if fill_type != self.path.borrow().fill_type() {
            self.path.borrow_mut().set_fill_type(fill_type);
            self.invalidate();
        }
    }
}

impl Node for Path {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGPath.cpp#L23-L31 (chrome/m156) (`Path::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let path = self.path.borrow();
        let ft = path.fill_type();
        if ft == PathFillType::Winding || ft == PathFillType::EvenOdd {
            // "Containing" fills have finite bounds.
            path.compute_tight_bounds()
        } else {
            // Inverse fills are "infinite".
            crate::util::make_large_s32()
        }
    }
}

impl GeometryNode for Path {
    // Port of: modules/sksg/src/SkSGPath.cpp#L11-L13 (chrome/m156) (`Path::onClip`)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_path(&self.path.borrow(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/sksg/src/SkSGPath.cpp#L15-L17 (chrome/m156) (`Path::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        canvas.draw_path(&self.path.borrow(), paint);
    }

    // Port of: modules/sksg/src/SkSGPath.cpp#L19-L21 (chrome/m156) (`Path::onContains`)
    fn on_contains(&self, p: Point) -> bool {
        self.path.borrow().contains(p)
    }

    // Port of: modules/sksg/src/SkSGPath.cpp#L23-L25 (chrome/m156) (`Path::onAsPath`) (copy)
    fn on_as_path(&self) -> SkPath {
        self.path.borrow().clone()
    }
}
