// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGGeometryNode.h, modules/sksg/src/SkSGGeometryNode.cpp
// (chrome/m156)

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;

use crate::node::{Node, inval_traits};
use crate::util::rect_contains;

/// A node that describes a shape: it clips, draws, hit-tests and converts to a path.
// Port of: modules/sksg/include/SkSGGeometryNode.h#L14-L40 (chrome/m156) (`class GeometryNode`)
#[doc(alias = "sksg::GeometryNode")]
pub trait GeometryNode: Node {
    /// Clips the canvas to the shape (`onClip`).
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool);
    /// Draws the shape with `paint` (`onDraw`).
    fn on_draw(&self, canvas: &Canvas, paint: &Paint);
    /// True if the shape contains `p` (`onContains`).
    fn on_contains(&self, p: Point) -> bool;
    /// The shape as a path (`onAsPath`).
    fn on_as_path(&self) -> Path;

    /// Clips the canvas to the shape.
    // Port of: modules/sksg/src/SkSGGeometryNode.cpp#L15-L19 (chrome/m156) (`GeometryNode::clip`)
    fn clip(&self, canvas: &Canvas, anti_alias: bool) {
        debug_assert!(!self.core().has_inval());
        self.on_clip(canvas, anti_alias);
    }

    /// Draws the shape.
    // Port of: modules/sksg/src/SkSGGeometryNode.cpp#L21-L25 (chrome/m156) (`GeometryNode::draw`)
    fn draw(&self, canvas: &Canvas, paint: &Paint) {
        debug_assert!(!self.core().has_inval());
        self.on_draw(canvas, paint);
    }

    /// True if the shape contains `p`.
    // Port of: modules/sksg/src/SkSGGeometryNode.cpp#L27-L31 (chrome/m156) (`GeometryNode::contains`)
    fn contains(&self, p: Point) -> bool {
        debug_assert!(!self.core().has_inval());
        if rect_contains(&self.core().bounds(), p) {
            self.on_contains(p)
        } else {
            false
        }
    }

    /// The shape as a path.
    // Port of: modules/sksg/src/SkSGGeometryNode.cpp#L33-L37 (chrome/m156) (`GeometryNode::asPath`)
    fn as_path(&self) -> Path {
        debug_assert!(!self.core().has_inval());
        self.on_as_path()
    }
}

/// The traits of geometry nodes: their damage bubbles up to the draws that use them.
// Port of: modules/sksg/src/SkSGGeometryNode.cpp#L11-L11 (chrome/m156)
pub const GEOMETRY_TRAITS: u32 = inval_traits::BUBBLE_DAMAGE;
