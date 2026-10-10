// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGRect.h, modules/sksg/src/SkSGRect.cpp (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::rrect::RRect as SkRRect;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::util::rect_contains;

/// The direction and initial point of a shape's contour (`AttrContainer`).
// Port of: modules/sksg/include/SkSGRect.h#L44-L55 (chrome/m156) (`Rect::AttrContainer`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrContainer {
    direction: PathDirection,
    /// Two bits in Skia; always 0..=3.
    initial_point_index: u8,
}

impl Default for AttrContainer {
    // Port of: modules/sksg/include/SkSGRect.h#L62-L62 (chrome/m156) (`fAttrContaier = { kCW, 0 }`)
    fn default() -> Self {
        Self {
            direction: PathDirection::CW,
            initial_point_index: 0,
        }
    }
}

/// A rectangle geometry.
// Port of: modules/sksg/include/SkSGRect.h#L12-L42 (chrome/m156) (`class Rect`)
#[doc(alias = "sksg::Rect")]
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // mirrors SkSGRect.h, where the field is `fRect`
pub struct Rect {
    core: NodeCore,
    rect: Cell<SkRect>,
    attrs: Cell<AttrContainer>,
}

impl Rect {
    /// An empty rectangle (`Rect::Make()`).
    #[must_use]
    pub fn make_empty() -> Rc<Self> {
        Self::make(SkRect::new_empty())
    }

    /// A rectangle geometry (`Rect::Make(const SkRect&)`).
    // Port of: modules/sksg/include/SkSGRect.h#L15-L16 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(r: SkRect) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            rect: Cell::new(r),
            attrs: Cell::new(AttrContainer::default()),
        })
    }

    /// The rectangle.
    #[must_use]
    pub fn rect(&self) -> SkRect {
        self.rect.get()
    }

    /// Sets one edge of the rectangle, invalidating the node if it changed.
    #[allow(clippy::float_cmp)] // SG_ATTRIBUTE compares the edge with ==, as Skia does
    fn set_edge(&self, edge: fn(&mut SkRect) -> &mut f32, value: f32) {
        let mut r = self.rect.get();
        if *edge(&mut r) == value {
            return;
        }
        *edge(&mut r) = value;
        self.rect.set(r);
        self.invalidate();
    }

    /// Sets the left edge (`setL`).
    #[doc(alias = "setL")]
    pub fn set_l(&self, v: f32) {
        self.set_edge(|r| &mut r.left, v);
    }

    /// Sets the top edge (`setT`).
    #[doc(alias = "setT")]
    pub fn set_t(&self, v: f32) {
        self.set_edge(|r| &mut r.top, v);
    }

    /// Sets the right edge (`setR`).
    #[doc(alias = "setR")]
    pub fn set_r(&self, v: f32) {
        self.set_edge(|r| &mut r.right, v);
    }

    /// Sets the bottom edge (`setB`).
    #[doc(alias = "setB")]
    pub fn set_b(&self, v: f32) {
        self.set_edge(|r| &mut r.bottom, v);
    }

    /// The contour direction.
    #[must_use]
    pub fn direction(&self) -> PathDirection {
        self.attrs.get().direction
    }

    /// Sets the contour direction, invalidating the node if it changed.
    pub fn set_direction(&self, dir: PathDirection) {
        let mut a = self.attrs.get();
        if a.direction == dir {
            return;
        }
        a.direction = dir;
        self.attrs.set(a);
        self.invalidate();
    }

    /// The index of the contour's initial point.
    #[must_use]
    pub fn initial_point_index(&self) -> u8 {
        self.attrs.get().initial_point_index
    }

    /// Sets the index of the contour's initial point, invalidating the node if it changed.
    pub fn set_initial_point_index(&self, idx: u8) {
        let mut a = self.attrs.get();
        if a.initial_point_index == idx {
            return;
        }
        a.initial_point_index = idx;
        self.attrs.set(a);
        self.invalidate();
    }
}

impl Node for Rect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L29-L33 (chrome/m156) (`Rect::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> SkRect {
        debug_assert!(self.core.has_inval());
        self.rect.get()
    }
}

impl GeometryNode for Rect {
    // Port of: modules/sksg/src/SkSGRect.cpp#L11-L13 (chrome/m156) (`Rect::onClip`)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_rect(self.rect.get(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L15-L17 (chrome/m156) (`Rect::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        canvas.draw_rect(self.rect.get(), paint);
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L19-L21 (chrome/m156) (`Rect::onContains`)
    fn on_contains(&self, p: Point) -> bool {
        rect_contains(&self.rect.get(), p)
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L27-L29 (chrome/m156) (`Rect::onAsPath`)
    fn on_as_path(&self) -> SkPath {
        let a = self.attrs.get();
        SkPath::rect_with_start_index(
            self.rect.get(),
            a.direction,
            usize::from(a.initial_point_index),
        )
    }
}

/// A rounded-rectangle geometry.
// Port of: modules/sksg/include/SkSGRect.h#L72-L101 (chrome/m156) (`class RRect`)
#[doc(alias = "sksg::RRect")]
#[derive(Debug)]
pub struct RRect {
    core: NodeCore,
    rrect: std::cell::RefCell<SkRRect>,
    attrs: Cell<AttrContainer>,
}

impl RRect {
    /// An empty rounded rectangle (`RRect::Make()`).
    #[must_use]
    pub fn make_empty() -> Rc<Self> {
        Self::make(SkRRect::default())
    }

    /// A rounded-rectangle geometry (`RRect::Make(const SkRRect&)`).
    // Port of: modules/sksg/include/SkSGRect.h#L75-L76 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(rr: SkRRect) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            rrect: std::cell::RefCell::new(rr),
            attrs: Cell::new(AttrContainer::default()),
        })
    }

    /// The rounded rectangle.
    #[must_use]
    pub fn rrect(&self) -> SkRRect {
        *self.rrect.borrow()
    }

    /// Sets the rounded rectangle, invalidating the node if it changed (`setRRect`).
    #[doc(alias = "setRRect")]
    pub fn set_rrect(&self, rr: SkRRect) {
        if *self.rrect.borrow() == rr {
            return;
        }
        *self.rrect.borrow_mut() = rr;
        self.invalidate();
    }

    /// The contour direction.
    #[must_use]
    pub fn direction(&self) -> PathDirection {
        self.attrs.get().direction
    }

    /// Sets the contour direction, invalidating the node if it changed.
    pub fn set_direction(&self, dir: PathDirection) {
        let mut a = self.attrs.get();
        if a.direction == dir {
            return;
        }
        a.direction = dir;
        self.attrs.set(a);
        self.invalidate();
    }

    /// The index of the contour's initial point.
    #[must_use]
    pub fn initial_point_index(&self) -> u8 {
        self.attrs.get().initial_point_index
    }

    /// Sets the index of the contour's initial point, invalidating the node if it changed.
    pub fn set_initial_point_index(&self, idx: u8) {
        let mut a = self.attrs.get();
        if a.initial_point_index == idx {
            return;
        }
        a.initial_point_index = idx;
        self.attrs.set(a);
        self.invalidate();
    }
}

impl Node for RRect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L49-L53 (chrome/m156) (`RRect::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> SkRect {
        debug_assert!(self.core.has_inval());
        *self.rrect.borrow().bounds()
    }
}

impl GeometryNode for RRect {
    // Port of: modules/sksg/src/SkSGRect.cpp#L35-L37 (chrome/m156) (`RRect::onClip`)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_rrect(*self.rrect.borrow(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L39-L41 (chrome/m156) (`RRect::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        canvas.draw_rrect(*self.rrect.borrow(), paint);
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L43-L45 (chrome/m156) (`RRect::onContains`)
    fn on_contains(&self, p: Point) -> bool {
        self.rrect.borrow().contains_point(p)
    }

    // Port of: modules/sksg/src/SkSGRect.cpp#L55-L57 (chrome/m156) (`RRect::onAsPath`)
    fn on_as_path(&self) -> SkPath {
        let a = self.attrs.get();
        SkPath::rrect_with_start_index(
            *self.rrect.borrow(),
            a.direction,
            usize::from(a.initial_point_index),
        )
    }
}
