// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGPlane.h, modules/sksg/src/SkSGPlane.cpp (chrome/m156)

use std::rc::Weak;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};

/// An infinite plane: a geometry that covers everything (`Plane`).
// Port of: modules/sksg/include/SkSGPlane.h#L13-L28 (chrome/m156) (`class Plane`)
#[doc(alias = "sksg::Plane")]
#[derive(Debug)]
pub struct Plane {
    core: NodeCore,
}

impl Plane {
    /// `Plane::Make()`.
    // Port of: modules/sksg/include/SkSGPlane.h#L16-L16 (chrome/m156) (`Plane::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> std::rc::Rc<Self> {
        std::rc::Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
        })
    }
}

impl Node for Plane {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGPlane.cpp#L30-L34 (chrome/m156) (`Plane::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        Rect::from_ltrb(f32::MIN, f32::MIN, f32::MAX, f32::MAX)
    }
}

impl GeometryNode for Plane {
    // Port of: modules/sksg/src/SkSGPlane.cpp#L22-L22 (chrome/m156) (`Plane::onClip`)
    fn on_clip(&self, _canvas: &Canvas, _anti_alias: bool) {}

    // Port of: modules/sksg/src/SkSGPlane.cpp#L24-L26 (chrome/m156) (`Plane::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        canvas.draw_paint(paint);
    }

    // Port of: modules/sksg/src/SkSGPlane.cpp#L28-L28 (chrome/m156) (`Plane::onContains`)
    fn on_contains(&self, _p: Point) -> bool {
        true
    }

    // Port of: modules/sksg/src/SkSGPlane.cpp#L36-L41 (chrome/m156) (`Plane::onAsPath`)
    fn on_as_path(&self) -> SkPath {
        let mut path = SkPath::default();
        path.set_fill_type(PathFillType::InverseWinding);
        path
    }
}
