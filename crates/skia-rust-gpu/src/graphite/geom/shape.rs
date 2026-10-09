// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/Shape.h, src/gpu/graphite/geom/Shape.cpp

//! `skgpu::graphite::Shape`: a tagged union over the geometric shapes Graphite draws.
//!
//! The C++ class stores a `union` with a `fType` tag and an `fInverted` flag. Here the union is an
//! enum that carries its payload, and `inverted` stays a separate flag as in the C++.
//!
//! Not yet ported: `keySize()` and `writeKey()` (the path-data key encoding, `Shape.cpp#L143-L310`).

use skia_rust_core::arc::{self, Arc};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rrect::RRect;
use skia_rust_simd::vx::Float2;

use crate::graphite::geom::rect::Rect;

/// The kind of geometry stored in a [`Shape`], as in `Shape::Type`.
// Port of: src/gpu/graphite/geom/Shape.h#L35-L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::Shape::Type")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// No geometry (the default).
    Empty,
    /// A line segment, stored as its two corners.
    Line,
    /// An axis-aligned rect.
    Rect,
    /// A round rect.
    RRect,
    /// An arc or wedge of an oval.
    Arc,
    /// An arbitrary path.
    Path,
}

/// The geometry itself. Each variant carries the data for its `Type`.
#[derive(Clone, Debug)]
enum Kind {
    Empty,
    // p0 = top-left, p1 = bot-right (may be unsorted)
    Line(Rect),
    Rect(Rect),
    RRect(RRect),
    Arc(Arc),
    Path(Path),
}

/// `Shape` is effectively a tagged union over different geometric shapes, with the most complex
/// being a `Path`. It provides a consistent way to query geometric properties, such as convexity,
/// point containment, or iteration.
#[doc(alias = "skgpu::graphite::Shape")]
#[derive(Clone, Debug)]
pub struct Shape {
    kind: Kind,
    inverted: bool,
}

impl Default for Shape {
    // Port of: src/gpu/graphite/geom/Shape.h#L46 (chrome/m156)
    fn default() -> Self {
        Self {
            kind: Kind::Empty,
            inverted: false,
        }
    }
}

impl Shape {
    /// `Shape::Shape(p0, p1)`: a line segment.
    // Port of: src/gpu/graphite/geom/Shape.h#L51 (chrome/m156)
    #[must_use]
    pub fn new_line(p0: Float2, p1: Float2) -> Self {
        Self {
            kind: Kind::Line(Rect::from_corners(p0, p1)),
            inverted: false,
        }
    }

    /// `Shape::Shape(const Rect&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L53 (chrome/m156)
    #[must_use]
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            kind: Kind::Rect(rect),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkRRect&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L55 (chrome/m156)
    #[must_use]
    pub fn from_rrect(rrect: RRect) -> Self {
        Self {
            kind: Kind::RRect(rrect),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkArc&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L56 (chrome/m156)
    #[must_use]
    pub fn from_arc(arc: Arc) -> Self {
        Self {
            kind: Kind::Arc(arc),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkPath&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L57 (chrome/m156)
    #[must_use]
    pub fn from_path(path: Path) -> Self {
        let inverted = path.is_inverse_fill_type();
        Self {
            kind: Kind::Path(path),
            inverted,
        }
    }

    /// `type()`: the type of the data last stored in the `Shape`. This does not incorporate any
    /// possible simplifications (a degenerate round rect with 0 radius corners is `RRect`).
    // Port of: src/gpu/graphite/geom/Shape.h#L69 (chrome/m156)
    #[must_use]
    pub fn type_(&self) -> Type {
        match self.kind {
            Kind::Empty => Type::Empty,
            Kind::Line(_) => Type::Line,
            Kind::Rect(_) => Type::Rect,
            Kind::RRect(_) => Type::RRect,
            Kind::Arc(_) => Type::Arc,
            Kind::Path(_) => Type::Path,
        }
    }

    /// `isEmpty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.type_() == Type::Empty
    }

    /// `isLine()`.
    #[must_use]
    pub fn is_line(&self) -> bool {
        self.type_() == Type::Line
    }

    /// `isRect()`.
    #[must_use]
    pub fn is_rect(&self) -> bool {
        self.type_() == Type::Rect
    }

    /// `isRRect()`.
    #[must_use]
    pub fn is_rrect(&self) -> bool {
        self.type_() == Type::RRect
    }

    /// `isArc()`.
    #[must_use]
    pub fn is_arc(&self) -> bool {
        self.type_() == Type::Arc
    }

    /// `isPath()`.
    #[must_use]
    pub fn is_path(&self) -> bool {
        self.type_() == Type::Path
    }

    /// `isFloodFill()`.
    #[must_use]
    pub fn is_flood_fill(&self) -> bool {
        self.is_empty() && self.inverted()
    }

    /// `isVolatilePath()`.
    #[must_use]
    pub fn is_volatile_path(&self) -> bool {
        matches!(&self.kind, Kind::Path(p) if p.is_volatile())
    }

    /// `inverted()`.
    #[must_use]
    pub fn inverted(&self) -> bool {
        debug_assert!(
            !matches!(&self.kind, Kind::Path(p) if self.inverted != p.is_inverse_fill_type())
        );
        self.inverted
    }

    /// `setInverted(inverted)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L89-L94 (chrome/m156)
    pub fn set_inverted(&mut self, inverted: bool) {
        if let Kind::Path(p) = &mut self.kind
            && inverted != p.is_inverse_fill_type()
        {
            p.toggle_inverse_fill_type();
        }
        self.inverted = inverted;
    }

    /// `fillType()`.
    // Port of: src/gpu/graphite/geom/Shape.h#L96-L102 (chrome/m156)
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        match &self.kind {
            // already incorporates invertedness
            Kind::Path(p) => p.fill_type(),
            _ => {
                if self.inverted {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                }
            }
        }
    }

    /// `conservativeContains(const Rect&)`: true if the given bounding box is completely inside
    /// the shape, if it's conservatively treated as a filled, closed shape.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L43-L68 (chrome/m156)
    #[must_use]
    pub fn conservative_contains_rect(&self, rect: Rect) -> bool {
        match &self.kind {
            Kind::Empty | Kind::Line(_) => false,
            Kind::Rect(r) => r.contains(rect),
            Kind::RRect(rr) => rr.contains(rect.as_sk_rect()),
            // We need to ensure the path is non-inverted.
            Kind::Path(p) => {
                if self.inverted() {
                    let mut non_inverted = p.clone();
                    non_inverted.toggle_inverse_fill_type();
                    non_inverted.conservatively_contains_rect(rect.as_sk_rect())
                } else {
                    p.conservatively_contains_rect(rect.as_sk_rect())
                }
            }
            Kind::Arc(a) => {
                if a.is_wedge() {
                    let mut arc_path = self.as_path();
                    if self.inverted() {
                        arc_path.toggle_inverse_fill_type();
                    }
                    arc_path.conservatively_contains_rect(rect.as_sk_rect())
                } else {
                    false
                }
            }
        }
    }

    /// `conservativeContains(skvx::float2 point)`.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L70-L87 (chrome/m156)
    #[must_use]
    pub fn conservative_contains_point(&self, point: Float2) -> bool {
        match &self.kind {
            Kind::Empty | Kind::Line(_) | Kind::Arc(_) => false,
            Kind::Rect(r) => r.contains(Rect::point(point)),
            Kind::RRect(rr) => rr.contains_point(Point::new(point.x(), point.y())),
            // We need to ensure the path is non-inverted.
            Kind::Path(p) => {
                if self.inverted() {
                    let mut non_inverted = p.clone();
                    non_inverted.toggle_inverse_fill_type();
                    non_inverted.contains((point.x(), point.y()))
                } else {
                    p.contains((point.x(), point.y()))
                }
            }
        }
    }

    /// `convex(simpleFill)`: true if the underlying shape is known to be convex, assuming no other
    /// styles. If `simple_fill` is true, it is assumed the contours will be implicitly closed when
    /// drawn or used.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L89-L99 (chrome/m156)
    #[must_use]
    pub fn convex(&self, simple_fill: bool) -> bool {
        match &self.kind {
            Kind::Path(p) => (simple_fill || p.is_last_contour_closed()) && p.is_convex(),
            Kind::Arc(a) => arc::draw_arc_is_convex(a.sweep_angle, a.kind, simple_fill),
            // Every other shape type is convex by construction.
            _ => true,
        }
    }

    /// `bounds()`: the bounding box of the shape.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L101-L111 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match &self.kind {
            Kind::Empty => Rect::new(0.0, 0.0, 0.0, 0.0),
            // sorting corners computes bbox of segment
            Kind::Line(r) => r.make_sorted(),
            // assuming it's sorted
            Kind::Rect(r) => *r,
            Kind::RRect(rr) => Rect::from(*rr.bounds()),
            Kind::Arc(a) => Rect::from(a.oval),
            Kind::Path(p) => Rect::from(*p.bounds()),
        }
    }

    /// `asPath()`: converts the shape into a path that describes the same geometry.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L113-L140 (chrome/m156)
    #[must_use]
    pub fn as_path(&self) -> Path {
        match &self.kind {
            Kind::Path(p) => p.clone(),
            Kind::Arc(a) => {
                // Filled ovals are already culled out so we assume no simple fills
                let mut out = arc::create_draw_arc_path(a, false);
                // CreateDrawArcPath resets the output path and configures its fill type, so we
                // just have to ensure invertedness is correct.
                if self.inverted {
                    out.toggle_inverse_fill_type();
                }
                out
            }
            _ => {
                let mut builder = PathBuilder::new_with_fill_type(self.fill_type());
                match &self.kind {
                    Kind::Empty | Kind::Path(_) | Kind::Arc(_) => {}
                    Kind::Line(r) => {
                        builder
                            .move_to((r.left(), r.top()))
                            .line_to((r.right(), r.bot()));
                    }
                    Kind::Rect(r) => {
                        builder.add_rect(r.as_sk_rect(), Some(PathDirection::CW), Some(0));
                    }
                    Kind::RRect(rr) => {
                        builder.add_rrect(*rr, Some(PathDirection::CW), Some(0));
                    }
                }
                builder.detach()
            }
        }
    }

    /// `p0()`: the first corner of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line (`SkASSERT(isLine())` in the C++).
    #[must_use]
    pub fn p0(&self) -> Float2 {
        self.line_rect().top_left()
    }

    /// `p1()`: the second corner of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line.
    #[must_use]
    pub fn p1(&self) -> Float2 {
        self.line_rect().bot_right()
    }

    /// `line()`: `[left, top, right, bot]` of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line.
    #[must_use]
    pub fn line(&self) -> skia_rust_simd::vx::Float4 {
        self.line_rect().ltrb()
    }

    /// `rect()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a rect.
    #[must_use]
    pub fn rect(&self) -> &Rect {
        let Kind::Rect(r) = &self.kind else {
            panic!("Shape::rect() called on a {:?}", self.type_());
        };
        r
    }

    /// `rrect()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a round rect.
    #[must_use]
    pub fn rrect(&self) -> &RRect {
        let Kind::RRect(rr) = &self.kind else {
            panic!("Shape::rrect() called on a {:?}", self.type_());
        };
        rr
    }

    /// `arc()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not an arc.
    #[must_use]
    pub fn arc(&self) -> &Arc {
        let Kind::Arc(a) = &self.kind else {
            panic!("Shape::arc() called on a {:?}", self.type_());
        };
        a
    }

    /// `path()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a path.
    #[must_use]
    pub fn path(&self) -> &Path {
        let Kind::Path(p) = &self.kind else {
            panic!("Shape::path() called on a {:?}", self.type_());
        };
        p
    }

    // The line's corners, for `p0()`, `p1()` and `line()`.
    fn line_rect(&self) -> &Rect {
        let Kind::Line(r) = &self.kind else {
            panic!("Shape line accessor called on a {:?}", self.type_());
        };
        r
    }

    /// `setLine(p0, p1)`: resets inversion to the default for lines.
    // Port of: src/gpu/graphite/geom/Shape.h#L146-L150 (chrome/m156)
    pub fn set_line(&mut self, p0: Float2, p1: Float2) {
        self.kind = Kind::Line(Rect::from_corners(p0, p1));
        self.inverted = false;
    }

    /// `setRect(rect)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L152-L156 (chrome/m156)
    pub fn set_rect(&mut self, rect: Rect) {
        self.kind = Kind::Rect(rect);
        self.inverted = false;
    }

    /// `setRRect(rrect)`. Performs no simplification: a round rect with 0 radii is still an
    /// `RRect`.
    // Port of: src/gpu/graphite/geom/Shape.h#L157-L161 (chrome/m156)
    pub fn set_rrect(&mut self, rrect: RRect) {
        self.kind = Kind::RRect(rrect);
        self.inverted = false;
    }

    /// `setArc(arc)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L162-L166 (chrome/m156)
    pub fn set_arc(&mut self, arc: Arc) {
        self.kind = Kind::Arc(arc);
        self.inverted = false;
    }

    /// `setPath(path)`: the inversion follows the path's own fill type.
    // Port of: src/gpu/graphite/geom/Shape.h#L167-L177 (chrome/m156)
    pub fn set_path(&mut self, path: Path) {
        self.inverted = path.is_inverse_fill_type();
        self.kind = Kind::Path(path);
    }

    /// `reset()`: back to the empty, non-inverted shape.
    // Port of: src/gpu/graphite/geom/Shape.h#L179-L182 (chrome/m156)
    pub fn reset(&mut self) {
        self.kind = Kind::Empty;
        self.inverted = false;
    }
}
