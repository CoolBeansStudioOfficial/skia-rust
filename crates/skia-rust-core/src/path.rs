// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPath.h, src/core/SkPath.cpp

//! Immutable paths (`SkPath.h`).
//!
//! A [`Path`] contains geometry: it may be empty, or contain one or more verbs that outline a
//! figure. A path always starts with a move verb to a Cartesian coordinate, and may be followed by
//! additional verbs that add lines or curves. Adding a close verb makes the geometry into a
//! continuous loop, a closed contour. A path may contain any number of contours, each beginning
//! with a move verb.
//!
//! When used to draw a filled area, the path describes whether the fill is inside or outside the
//! geometry, and the winding rule used to fill overlapping contours. Internally, a path lazily
//! computes its convexity.
//!
//! In m156 `SkPath` is immutable: build paths with [`PathBuilder`](crate::path_builder::PathBuilder).

use std::sync::{Arc, OnceLock};

use crate::floating_point::is_nan;
use crate::geometry::Conic;
use crate::matrix::Matrix;
use crate::path_data::PathData;
use crate::path_enums::{PathConvexity, ResolveConvexity};
use crate::path_iter::{PathIter, PathIterRec};
use crate::path_priv::{self, RRectAsEnum};
use crate::path_raw::PathRaw;
use crate::path_ref::{PathOvalInfo, PathRRectInfo};
use crate::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use crate::point::{Point, Vector, point_priv};
use crate::rect::Rect;
use crate::rrect::RRect;
use crate::scalar::scalar;

pub use crate::path_iter::PathIterRec as IterRec;

/// `SkPath::SegmentMask`.
pub type SegmentMask = PathSegmentMask;

/// How `PathBuilder::add_path` appends. Adding one path to another can extend the last contour
/// or start a new contour.
// Port of: include/core/SkPath.h#L571-L584 (chrome/m156)
#[doc(alias = "SkPath::AddPathMode")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum AddPathMode {
    /// Appended to destination unaltered. Since a path's verb array begins with a move, if the
    /// destination is not empty, the source's starting move begins a new contour.
    #[default]
    Append,
    /// If the destination is closed or empty, start a new contour. If the destination is not
    /// empty, add a line from its last point to the source's first point, then append the
    /// remaining source verbs.
    Extend,
}

/// The legacy `SkPath::Verb`, which adds `Done` to [`PathVerb`].
// Port of: include/core/SkPath.h#L652-L660 (chrome/m156)
#[doc(alias = "SkPath::Verb")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Verb {
    Move = 0,
    Line = 1,
    Quad = 2,
    Conic = 3,
    Cubic = 4,
    Close = 5,
    Done = 6,
}

impl Verb {
    /// The largest number of points returned with a verb.
    pub const MAX_POINTS: usize = 4;

    /// The number of points returned by `Iter` for this verb.
    #[must_use]
    pub fn points(self) -> usize {
        match self {
            Self::Move => 1,
            Self::Line => 2,
            Self::Quad | Self::Conic => 3,
            Self::Cubic => 4,
            Self::Close | Self::Done => 0,
        }
    }
}

impl From<PathVerb> for Verb {
    fn from(v: PathVerb) -> Self {
        match v {
            PathVerb::Move => Self::Move,
            PathVerb::Line => Self::Line,
            PathVerb::Quad => Self::Quad,
            PathVerb::Conic => Self::Conic,
            PathVerb::Cubic => Self::Cubic,
            PathVerb::Close => Self::Close,
        }
    }
}

/// An immutable path: shared [`PathData`] plus a fill type and a volatility flag.
///
/// Cloning a path is cheap (it shares the data, as the C++ copy does).
// Port of: include/core/SkPath.h#L64-L1143 (chrome/m156)
#[doc(alias = "SkPath")]
#[derive(Clone, Debug)]
pub struct Path {
    pub(crate) data: Arc<PathData>,
    pub(crate) fill_type: PathFillType,
    pub(crate) is_volatile: bool,
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

// Port of: src/core/SkPath.cpp#L63-L66 (chrome/m156)
impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || (self.fill_type == other.fill_type && *self.data == *other.data)
    }
}

impl Path {
    // Port of: src/core/SkPath.cpp#L32-L38 (chrome/m156)
    pub(crate) fn from_data(pd: Arc<PathData>, ft: PathFillType, is_volatile: bool) -> Self {
        Self {
            data: pd,
            fill_type: ft,
            is_volatile,
        }
    }

    /// The singleton that marks a path whose data is in error: either because the inputs were
    /// invalid (e.g. bad verbs), or its coordinates were non-finite.
    // Port of: src/core/SkPath.cpp#L743-L751 (chrome/m156)
    pub(crate) fn peek_error_singleton() -> &'static Arc<PathData> {
        static ERROR: OnceLock<Arc<PathData>> = OnceLock::new();
        // Distinct from the standard empty instance.
        ERROR.get_or_init(|| Arc::new(PathData::new_empty_unshared()))
    }

    // Port of: src/core/SkPath.cpp#L753-L758 (chrome/m156)
    pub(crate) fn make_null_check(
        pdata: Option<Arc<PathData>>,
        ft: PathFillType,
        is_volatile: bool,
    ) -> Self {
        let pdata = pdata.unwrap_or_else(|| Arc::clone(Self::peek_error_singleton()));
        Self::from_data(pdata, ft, is_volatile)
    }

    /// An empty path with the default fill type.
    // Port of: include/core/SkPath.h#L210 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_fill_type(PathFillType::DEFAULT)
    }

    /// An empty path with the given fill type.
    // Port of: src/core/SkPath.cpp#L40-L44 (chrome/m156)
    #[must_use]
    pub fn new_with_fill_type(fill_type: PathFillType) -> Self {
        Self::from_data(PathData::empty(), fill_type, false)
    }

    /// A path with copies of the given points, verbs and conic weights. If the verbs are illegal
    /// or the counts don't match, the result is a non-finite (empty) path.
    // Port of: src/core/SkPath.cpp#L108-L111 (chrome/m156)
    #[doc(alias = "Raw")]
    #[must_use]
    pub fn raw(
        points: &[Point],
        verbs: &[PathVerb],
        conic_weights: &[scalar],
        fill_type: PathFillType,
        is_volatile: impl Into<Option<bool>>,
    ) -> Self {
        Self::make_null_check(
            PathData::make(points, verbs, conic_weights),
            fill_type,
            is_volatile.into().unwrap_or(false),
        )
    }

    /// Deprecated: use [`Path::raw`]. Verbs are raw bytes.
    // Port of: include/core/SkPath.h#L163-L171 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn new_from(
        points: &[Point],
        verbs: &[u8],
        conic_weights: &[scalar],
        fill_type: PathFillType,
        is_volatile: impl Into<Option<bool>>,
    ) -> Self {
        let verbs: Option<Vec<PathVerb>> = verbs.iter().map(|&v| PathVerb::from_u8(v)).collect();
        match verbs {
            Some(verbs) => Self::raw(points, &verbs, conic_weights, fill_type, is_volatile),
            None => Self::make_null_check(None, fill_type, is_volatile.into().unwrap_or(false)),
        }
    }

    /// A closed rectangle with the given fill type and direction (start index 0).
    #[doc(alias = "Rect")]
    #[must_use]
    pub fn rect_with_fill_type(
        rect: impl AsRef<Rect>,
        fill_type: PathFillType,
        dir: impl Into<Option<PathDirection>>,
    ) -> Self {
        Self::rect_with_fill_type_and_start_index(rect, fill_type, dir, 0)
    }

    /// `SkPath::Rect(r, ft, dir, startIndex)`.
    // Port of: src/core/SkPath.cpp#L113-L116 (chrome/m156)
    #[must_use]
    pub fn rect_with_fill_type_and_start_index(
        rect: impl AsRef<Rect>,
        fill_type: PathFillType,
        dir: impl Into<Option<PathDirection>>,
        start_index: usize,
    ) -> Self {
        // keep it legal
        #[allow(clippy::cast_possible_truncation)] // masked to 0..=3
        let start_index = (start_index & 3) as u32;
        Self::make_null_check(
            PathData::rect(rect.as_ref(), dir.into().unwrap_or_default(), start_index),
            fill_type,
            false,
        )
    }

    /// A closed rectangle (default fill type, start index 0).
    #[must_use]
    pub fn rect(rect: impl AsRef<Rect>, dir: impl Into<Option<PathDirection>>) -> Self {
        Self::rect_with_fill_type(rect, PathFillType::default(), dir)
    }

    /// `SkPath::Rect(r, dir, startIndex)`.
    #[must_use]
    pub fn rect_with_start_index(
        rect: impl AsRef<Rect>,
        dir: PathDirection,
        start_index: usize,
    ) -> Self {
        Self::rect_with_fill_type_and_start_index(rect, PathFillType::default(), dir, start_index)
    }

    /// An oval inscribed in `oval` (start index 1).
    // Port of: src/core/SkPath.cpp#L704-L707 (chrome/m156)
    #[doc(alias = "Oval")]
    #[must_use]
    pub fn oval(oval: impl AsRef<Rect>, dir: impl Into<Option<PathDirection>>) -> Self {
        // legacy start index: 1
        Self::oval_with_start_index(oval, dir.into().unwrap_or_default(), 1)
    }

    /// An oval inscribed in `oval`, starting at `start_index`.
    // Port of: src/core/SkPath.cpp#L118-L121 (chrome/m156)
    #[must_use]
    pub fn oval_with_start_index(
        oval: impl AsRef<Rect>,
        dir: PathDirection,
        start_index: usize,
    ) -> Self {
        // keep it legal
        #[allow(clippy::cast_possible_truncation)] // masked to 0..=3
        let start_index = (start_index & 3) as u32;
        Self::make_null_check(
            PathData::oval(oval.as_ref(), dir, start_index),
            PathFillType::DEFAULT,
            false,
        )
    }

    /// A circle; empty if `radius` is negative.
    // Port of: src/core/SkPath.cpp#L709-L715 (chrome/m156)
    #[doc(alias = "Circle")]
    #[must_use]
    pub fn circle(
        center: impl Into<Point>,
        radius: scalar,
        dir: impl Into<Option<PathDirection>>,
    ) -> Self {
        let c = center.into();
        let (x, y, r) = (c.x, c.y, radius);
        if r >= 0.0 {
            Self::oval(Rect::from_ltrb(x - r, y - r, x + r, y + r), dir)
        } else {
            Self::new()
        }
    }

    /// A round rectangle (start index 6 for CW, 7 for CCW).
    // Port of: src/core/SkPath.cpp#L699-L702 (chrome/m156)
    #[doc(alias = "RRect")]
    #[must_use]
    pub fn rrect(rrect: impl AsRef<RRect>, dir: impl Into<Option<PathDirection>>) -> Self {
        let dir = dir.into().unwrap_or_default();
        // legacy start indices: 6 (CW) and 7 (CCW)
        Self::rrect_with_start_index(rrect, dir, if dir == PathDirection::CW { 6 } else { 7 })
    }

    /// A round rectangle starting at `start_index`.
    // Port of: src/core/SkPath.cpp#L123-L141 (chrome/m156)
    #[must_use]
    pub fn rrect_with_start_index(
        rrect: impl AsRef<RRect>,
        dir: PathDirection,
        start_index: usize,
    ) -> Self {
        let rr = rrect.as_ref();
        // keep it legal
        #[allow(clippy::cast_possible_truncation)] // masked to 0..=7
        let start_index = (start_index & 7) as u32;
        // To be backwards compatible with the old impl for building a rrect path, we
        // first check to see if the rrect itself can be simplified...
        let bounds = *rr.rect();
        let (as_type, new_index) = path_priv::simplify_rrect(rr, start_index);
        match as_type {
            RRectAsEnum::Rect => {
                return Self::rect_with_fill_type_and_start_index(
                    bounds,
                    PathFillType::DEFAULT,
                    dir,
                    new_index as usize,
                );
            }
            RRectAsEnum::Oval => {
                return Self::oval_with_start_index(bounds, dir, new_index as usize);
            }
            RRectAsEnum::RRect => {} // fall through
        }
        Self::make_null_check(
            PathData::rrect(rr, dir, new_index),
            PathFillType::DEFAULT,
            false,
        )
    }

    /// `SkPath::RRect(bounds, rx, ry, dir)`.
    // Port of: src/core/SkPath.cpp#L717-L719 (chrome/m156)
    #[must_use]
    pub fn rrect_xy(
        bounds: impl AsRef<Rect>,
        rx: scalar,
        ry: scalar,
        dir: impl Into<Option<PathDirection>>,
    ) -> Self {
        Self::rrect(RRect::new_rect_xy(bounds, rx, ry), dir)
    }

    /// A polygon through `pts`.
    // Port of: src/core/SkPath.cpp#L143-L146 (chrome/m156)
    #[doc(alias = "Polygon")]
    #[must_use]
    pub fn polygon(
        pts: &[Point],
        is_closed: bool,
        fill_type: impl Into<Option<PathFillType>>,
        is_volatile: impl Into<Option<bool>>,
    ) -> Self {
        Self::make_null_check(
            PathData::polygon(pts, is_closed),
            fill_type.into().unwrap_or_default(),
            is_volatile.into().unwrap_or(false),
        )
    }

    /// A single line from `a` to `b`.
    // Port of: include/core/SkPath.h#L159-L161 (chrome/m156)
    #[doc(alias = "Line")]
    #[must_use]
    pub fn line(a: impl Into<Point>, b: impl Into<Point>) -> Self {
        Self::polygon(&[a.into(), b.into()], false, None, None)
    }

    /// A copy of this path (cheap: the data is shared).
    #[must_use]
    pub fn snapshot(&self) -> Self {
        self.clone()
    }

    /// The fill type.
    #[doc(alias = "getFillType")]
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        self.fill_type
    }

    /// A copy with the fill type replaced.
    // Port of: src/core/SkPath.cpp#L623-L627 (chrome/m156)
    #[doc(alias = "makeFillType")]
    #[must_use]
    pub fn with_fill_type(&self, new_fill_type: PathFillType) -> Path {
        let mut copy = self.clone();
        copy.set_fill_type(new_fill_type);
        copy
    }

    /// `SkPath::makeFillType`.
    #[must_use]
    pub fn make_fill_type(&self, new_fill_type: PathFillType) -> Path {
        self.with_fill_type(new_fill_type)
    }

    /// True if the fill type is inverse.
    #[doc(alias = "isInverseFillType")]
    #[must_use]
    pub fn is_inverse_fill_type(&self) -> bool {
        self.fill_type.is_inverse()
    }

    /// A copy with the fill type toggled between inverse and non-inverse.
    // Port of: src/core/SkPath.cpp#L629-L631 (chrome/m156)
    #[doc(alias = "makeToggleInverseFillType")]
    #[must_use]
    pub fn with_toggle_inverse_fill_type(&self) -> Self {
        self.with_fill_type(self.fill_type.toggle_inverse())
    }

    /// True if the path is convex; computes (and caches) the convexity if needed.
    // Port of: src/core/SkPath.cpp#L295-L297 (chrome/m156)
    #[doc(alias = "isConvex")]
    #[must_use]
    pub fn is_convex(&self) -> bool {
        self.get_convexity().is_convex()
    }

    /// The bounds if the path was built as an oval or circle.
    // Port of: src/core/SkPath.cpp#L318-L326 (chrome/m156)
    #[doc(alias = "isOval")]
    #[must_use]
    pub fn is_oval(&self) -> Option<Rect> {
        self.get_oval_info().map(|info| info.bounds)
    }

    /// The round rect if the path was built as one.
    // Port of: src/core/SkPath.cpp#L328-L336 (chrome/m156)
    #[doc(alias = "isRRect")]
    #[must_use]
    pub fn is_rrect(&self) -> Option<RRect> {
        self.get_rrect_info().map(|info| info.rrect)
    }

    /// True if the path has no verbs.
    // Port of: src/core/SkPath.cpp#L290-L293 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs().is_empty()
    }

    /// True if the last contour ends with a close verb.
    // Port of: src/core/SkPath.cpp#L270-L273 (chrome/m156)
    #[doc(alias = "isLastContourClosed")]
    #[must_use]
    pub fn is_last_contour_closed(&self) -> bool {
        self.verbs().last() == Some(&PathVerb::Close)
    }

    /// True if all points are finite (false for a path in error).
    // Port of: src/core/SkPath.cpp#L89-L91 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        !Arc::ptr_eq(&self.data, Self::peek_error_singleton())
    }

    /// True if the path is marked volatile.
    #[doc(alias = "isVolatile")]
    #[must_use]
    pub fn is_volatile(&self) -> bool {
        self.is_volatile
    }

    /// A copy with the volatility flag replaced.
    // Port of: src/core/SkPath.cpp#L633-L637 (chrome/m156)
    #[doc(alias = "makeIsVolatile")]
    #[must_use]
    pub fn with_is_volatile(&self, is_volatile: bool) -> Self {
        let mut copy = self.clone();
        copy.is_volatile = is_volatile;
        copy
    }

    /// True if the line from `p1` to `p2` is (nearly, unless `exact`) zero length.
    // Port of: src/core/SkPath.cpp#L681-L683 (chrome/m156)
    #[doc(alias = "IsLineDegenerate")]
    #[must_use]
    pub fn is_line_degenerate(p1: impl Into<Point>, p2: impl Into<Point>, exact: bool) -> bool {
        let (p1, p2) = (p1.into(), p2.into());
        if exact {
            p1 == p2
        } else {
            point_priv::equals_within_tolerance(p1, p2)
        }
    }

    /// True if the quad is (nearly, unless `exact`) a point.
    // Port of: src/core/SkPath.cpp#L685-L689 (chrome/m156)
    #[doc(alias = "IsQuadDegenerate")]
    #[must_use]
    pub fn is_quad_degenerate(
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
        exact: bool,
    ) -> bool {
        let (p1, p2, p3) = (p1.into(), p2.into(), p3.into());
        if exact {
            p1 == p2 && p2 == p3
        } else {
            point_priv::equals_within_tolerance(p1, p2)
                && point_priv::equals_within_tolerance(p2, p3)
        }
    }

    /// True if the cubic is (nearly, unless `exact`) a point.
    // Port of: src/core/SkPath.cpp#L691-L697 (chrome/m156)
    #[doc(alias = "IsCubicDegenerate")]
    #[must_use]
    pub fn is_cubic_degenerate(
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
        p4: impl Into<Point>,
        exact: bool,
    ) -> bool {
        let (p1, p2, p3, p4) = (p1.into(), p2.into(), p3.into(), p4.into());
        if exact {
            p1 == p2 && p2 == p3 && p3 == p4
        } else {
            point_priv::equals_within_tolerance(p1, p2)
                && point_priv::equals_within_tolerance(p2, p3)
                && point_priv::equals_within_tolerance(p3, p4)
        }
    }

    /// The two points if the path contains exactly one line.
    // Port of: src/core/SkPath.cpp#L275-L288 (chrome/m156)
    #[doc(alias = "isLine")]
    #[must_use]
    pub fn is_line(&self) -> Option<(Point, Point)> {
        let verbs = self.verbs();
        if verbs.len() == 2 && verbs[1] == PathVerb::Line {
            debug_assert_eq!(verbs[0], PathVerb::Move);
            let pts = self.points();
            debug_assert_eq!(pts.len(), 2);
            return Some((pts[0], pts[1]));
        }
        None
    }

    /// The path's points.
    #[must_use]
    pub fn points(&self) -> &[Point] {
        self.data.points()
    }

    /// The path's verbs.
    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        self.data.verbs()
    }

    /// The path's conic weights.
    #[doc(alias = "conicWeights")]
    #[must_use]
    pub fn conic_weights(&self) -> &[scalar] {
        self.data.conics()
    }

    /// The number of points.
    #[doc(alias = "countPoints")]
    #[must_use]
    pub fn count_points(&self) -> usize {
        self.points().len()
    }

    /// The number of verbs.
    #[doc(alias = "countVerbs")]
    #[must_use]
    pub fn count_verbs(&self) -> usize {
        self.verbs().len()
    }

    /// The last point, if any.
    // Port of: src/core/SkPath.cpp#L373-L380 (chrome/m156)
    #[doc(alias = "getLastPt")]
    #[must_use]
    pub fn last_pt(&self) -> Option<Point> {
        self.points().last().copied()
    }

    /// Deprecated: the point at `index`, or `None` if out of range (C++ returns (0, 0)).
    // Port of: src/core/SkPath.cpp#L348-L354 (chrome/m156)
    #[doc(alias = "getPoint")]
    #[must_use]
    pub fn get_point(&self, index: usize) -> Option<Point> {
        self.points().get(index).copied()
    }

    /// Deprecated: copies `min(#points, points.len())` points; returns the number of points.
    // Port of: src/core/SkPath.cpp#L339-L346 (chrome/m156)
    #[doc(alias = "getPoints")]
    pub fn get_points(&self, points: &mut [Point]) -> usize {
        let src = self.points();
        let n = points.len().min(src.len());
        points[..n].copy_from_slice(&src[..n]);
        src.len()
    }

    /// Deprecated: copies `min(#verbs, verbs.len())` verbs; returns the number of verbs.
    // Port of: src/core/SkPath.cpp#L356-L363 (chrome/m156)
    #[doc(alias = "getVerbs")]
    pub fn get_verbs(&self, verbs: &mut [u8]) -> usize {
        let src = self.verbs();
        let n = verbs.len().min(src.len());
        for (d, s) in verbs[..n].iter_mut().zip(src) {
            *d = *s as u8;
        }
        src.len()
    }

    /// The approximate size of the path in memory.
    // Port of: src/core/SkPath.cpp#L366-L371 (chrome/m156)
    #[doc(alias = "approximateBytesUsed")]
    #[must_use]
    pub fn approximate_bytes_used(&self) -> usize {
        std::mem::size_of::<Path>()
            + std::mem::size_of_val(self.points())
            + std::mem::size_of_val(self.verbs())
            + std::mem::size_of_val(self.conic_weights())
    }

    /// The bounds of the path's points (ignoring a trailing move). Empty if the path has no verbs
    /// or is not finite.
    // Port of: src/core/SkPath.cpp#L81-L83 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &Rect {
        self.data.bounds()
    }

    /// The bounds of the lines and curves in the path.
    // Port of: src/core/SkPath.cpp#L670-L679 (chrome/m156)
    #[doc(alias = "computeTightBounds")]
    #[must_use]
    pub fn compute_tight_bounds(&self) -> Rect {
        // If we're only lines, then our (quick) bounds is also tight.
        if self.segment_masks() == SegmentMask::LINE {
            return *self.bounds();
        }

        path_priv::compute_tight_bounds(self.points(), self.verbs(), self.conic_weights())
    }

    /// True if `rect` is (conservatively) contained in the path. Only returns true if the path
    /// has one contour and is convex.
    // Port of: src/core/SkPath.cpp#L195-L268 (chrome/m156)
    #[doc(alias = "conservativelyContainsRect")]
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // panics only if an internal invariant is broken
    pub fn conservatively_contains_rect(&self, rect: impl AsRef<Rect>) -> bool {
        let rect = rect.as_ref();
        let convexity = self.get_convexity();
        if !convexity.is_convex() {
            return false;
        }

        let Some(direction) = convexity.to_direction() else {
            return false;
        };

        let mut first_pt = Point::default();
        let mut prev_pt = Point::default();
        let mut segment_count = 0;

        for (verb, pts, weight) in path_priv::iterate(self) {
            if verb == PathVerb::Close || (segment_count > 0 && verb == PathVerb::Move) {
                // Closing the current contour; but since convexity is a precondition, it's the
                // only contour that matters.
                segment_count += 1;
                break;
            } else if verb == PathVerb::Move {
                // A move at the start of the contour (or multiple leading moves, in which case we
                // keep the last one before a non-move verb).
                debug_assert_eq!(segment_count, 0);
                first_pt = pts[0];
                prev_pt = pts[0];
            } else {
                let point_count = path_priv::pts_in_verb_usize(verb);
                debug_assert!(point_count > 0);

                if !path_priv::all_points_eq(&pts[..=point_count]) {
                    let next_pt = point_count;
                    segment_count += 1;

                    if prev_pt == pts[next_pt] {
                        // A pre-condition to getting here is that the path is convex, so if a
                        // verb's start and end points are the same, it means it's the only
                        // verb in the contour (and the only contour). While it's possible for
                        // such a single verb to be a convex curve, we do not have any non-zero
                        // length edges to conservatively test against without splitting or
                        // evaluating the curve. For simplicity, just reject the rectangle.
                        return false;
                    } else if verb == PathVerb::Conic {
                        let orig = Conic::from_points(pts, weight.unwrap_or(1.0));
                        let mut quad_pts = [Point::default(); 5];
                        let count = orig.chop_into_quads_pow2(&mut quad_pts, 1);
                        assert_eq!(count, 2);

                        if !check_edge_against_rect(quad_pts[0], quad_pts[2], rect, direction) {
                            return false;
                        }
                        if !check_edge_against_rect(quad_pts[2], quad_pts[4], rect, direction) {
                            return false;
                        }
                    } else if !check_edge_against_rect(prev_pt, pts[next_pt], rect, direction) {
                        return false;
                    }
                    prev_pt = pts[next_pt];
                }
            }
        }

        if segment_count != 0 {
            return check_edge_against_rect(prev_pt, first_pt, rect, direction);
        }
        false
    }

    /// Approximates a conic with quads; returns the number of quads written to `pts` (which must
    /// hold `1 + 2 * (1 << pow2)` points), or `None` if `pts` is too small.
    // Port of: src/core/SkPath.cpp#L662-L666 (chrome/m156)
    #[doc(alias = "ConvertConicToQuads")]
    pub fn convert_conic_to_quads(
        p0: impl Into<Point>,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        w: scalar,
        pts: &mut [Point],
        pow2: usize,
    ) -> Option<usize> {
        let max_pts_count = 1 + 2 * (1 << pow2);
        if pts.len() < max_pts_count {
            return None;
        }
        let conic = Conic::new(p0.into(), p1.into(), p2.into(), w);
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // small pow2
        Some(conic.chop_into_quads_pow2(pts, pow2 as i32))
    }

    /// `Some((rect, is_closed, direction))` if the path is equivalent to a rectangle when filled.
    // Port of: src/core/SkPath.cpp#L299-L316 (chrome/m156)
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> Option<(Rect, bool, PathDirection)> {
        let rc = path_priv::is_rect_contour(
            self.points(),
            self.verbs(),
            self.segment_masks().bits(),
            false,
        )?;
        Some((rc.rect, rc.is_closed, rc.direction))
    }

    /// The path transformed by `matrix`, or `None` if the result is not finite.
    // Port of: src/core/SkPath.cpp#L148-L153 (chrome/m156)
    #[doc(alias = "tryMakeTransform")]
    #[must_use]
    pub fn try_make_transform(&self, matrix: &Matrix) -> Option<Path> {
        let pdata = self.data.make_transform(matrix)?;
        Some(Path::from_data(pdata, self.fill_type, self.is_volatile))
    }

    /// `tryMakeOffset`.
    #[doc(alias = "tryMakeOffset")]
    #[must_use]
    pub fn try_make_offset(&self, d: impl Into<Vector>) -> Option<Path> {
        self.try_make_transform(&Matrix::translate(d))
    }

    /// `tryMakeScale`.
    #[doc(alias = "tryMakeScale")]
    #[must_use]
    pub fn try_make_scale(&self, (sx, sy): (scalar, scalar)) -> Option<Path> {
        self.try_make_transform(&Matrix::scale((sx, sy)))
    }

    /// The path transformed by `matrix`. If the result is not finite, the returned path reports
    /// `is_finite() == false`.
    // Port of: src/core/SkPath.cpp#L155-L163 (chrome/m156)
    #[doc(alias = "makeTransform")]
    #[must_use]
    pub fn with_transform(&self, matrix: &Matrix) -> Path {
        if !self.is_finite() {
            return self.clone();
        }
        if let Some(newpath) = self.try_make_transform(matrix) {
            return newpath;
        }
        Path::from_data(
            Arc::clone(Self::peek_error_singleton()),
            self.fill_type,
            false,
        )
    }

    /// `SkPath::makeTransform`.
    #[must_use]
    pub fn make_transform(&self, m: &Matrix) -> Path {
        self.with_transform(m)
    }

    /// The path offset by `d`.
    #[doc(alias = "makeOffset")]
    #[must_use]
    pub fn with_offset(&self, d: impl Into<Vector>) -> Path {
        self.make_transform(&Matrix::translate(d))
    }

    /// `SkPath::makeOffset`.
    #[must_use]
    pub fn make_offset(&self, d: impl Into<Vector>) -> Path {
        self.with_offset(d)
    }

    /// The path scaled by `(sx, sy)`.
    #[doc(alias = "makeScale")]
    #[must_use]
    pub fn make_scale(&self, (sx, sy): (scalar, scalar)) -> Path {
        self.make_transform(&Matrix::scale((sx, sy)))
    }

    /// The kinds of segments in the path.
    // Port of: src/core/SkPath.cpp#L85-L87 (chrome/m156)
    #[doc(alias = "getSegmentMasks")]
    #[must_use]
    pub fn segment_masks(&self) -> SegmentMask {
        SegmentMask::from_bits_truncate(u32::from(self.data.segment_mask()))
    }

    /// Sets the volatility flag.
    #[doc(alias = "setIsVolatile")]
    pub fn set_is_volatile(&mut self, is_volatile: bool) -> &mut Self {
        self.is_volatile = is_volatile;
        self
    }

    /// Exchanges the contents of two paths.
    // Port of: src/core/SkPath.cpp#L68-L74 (chrome/m156)
    pub fn swap(&mut self, other: &mut Path) -> &mut Self {
        std::mem::swap(self, other);
        self
    }

    /// Sets the fill type.
    #[doc(alias = "setFillType")]
    pub fn set_fill_type(&mut self, ft: PathFillType) -> &mut Self {
        self.fill_type = ft;
        self
    }

    /// Toggles the fill type between inverse and non-inverse.
    #[doc(alias = "toggleInverseFillType")]
    pub fn toggle_inverse_fill_type(&mut self) -> &mut Self {
        self.fill_type = self.fill_type.toggle_inverse();
        self
    }

    /// Resets to an empty path with the default fill type.
    // Port of: src/core/SkPath.cpp#L76-L79 (chrome/m156)
    pub fn reset(&mut self) -> &mut Self {
        *self = Path::new();
        self
    }

    /// Returns a copy of this path and resets it to empty.
    #[must_use]
    pub fn detach(&mut self) -> Self {
        let result = self.clone();
        self.reset();
        result
    }

    /// Iterates the verbs with their points.
    // Port of: src/core/SkPath.cpp#L393-L395 (chrome/m156)
    #[must_use]
    #[allow(clippy::iter_without_into_iter)] // mirrors the C++ / skia-safe `iter()`
    pub fn iter(&self) -> PathIter<'_> {
        PathIter::new(self.points(), self.verbs(), self.conic_weights())
    }

    /// True if `point` is inside the path (taking the fill type into account).
    // Port of: src/core/SkPath.cpp#L657-L660 (chrome/m156)
    #[must_use]
    pub fn contains(&self, point: impl Into<Point>) -> bool {
        let p = point.into();
        let raw = path_priv::raw(self, ResolveConvexity::No);
        raw.is_some_and(|raw| path_priv::contains(&raw, p))
    }

    /// A non-zero, globally unique value that changes when the geometry changes (not when the
    /// fill type changes).
    // Port of: src/core/SkPath.cpp#L95 (chrome/m156)
    #[doc(alias = "getGenerationID")]
    #[must_use]
    pub fn generation_id(&self) -> u64 {
        self.data.unique_id()
    }

    /// True if the path data is consistent (same as `is_finite`).
    // Port of: src/core/SkPath.cpp#L93 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.is_finite()
    }

    // Port of: src/core/SkPath.cpp#L101-L102 (chrome/m156)
    pub(crate) fn get_oval_info(&self) -> Option<PathOvalInfo> {
        self.data.as_oval()
    }

    pub(crate) fn get_rrect_info(&self) -> Option<PathRRectInfo> {
        self.data.as_rrect()
    }

    // Port of: src/core/SkPath.cpp#L165-L167 (chrome/m156)
    #[allow(clippy::unnecessary_wraps)] // keeps the C++ optional signature
    pub(crate) fn raw_view(&self, rc: ResolveConvexity) -> Option<PathRaw<'_>> {
        Some(self.data.raw(self.fill_type, rc))
    }

    // Port of: src/core/SkPath.cpp#L55-L61 (chrome/m156)
    pub(crate) fn set_convexity(&self, c: PathConvexity) {
        self.data.set_convexity(c);
    }

    pub(crate) fn get_convexity_or_unknown(&self) -> PathConvexity {
        self.data.get_convexity_or_unknown()
    }

    // Port of: src/core/SkPath.cpp#L382-L391 (chrome/m156)
    pub(crate) fn get_convexity(&self) -> PathConvexity {
        let mut convexity = self.get_convexity_or_unknown();
        if convexity == PathConvexity::Unknown {
            convexity = self.compute_convexity();
        }
        debug_assert_ne!(convexity, PathConvexity::Unknown);
        convexity
    }

    // Port of: src/core/SkPath.cpp#L639-L655 (chrome/m156)
    fn compute_convexity(&self) -> PathConvexity {
        let c = self.get_convexity_or_unknown();
        if c != PathConvexity::Unknown {
            return c;
        }

        let mut convexity = PathConvexity::Concave;

        if self.is_finite() {
            convexity =
                path_priv::compute_convexity(self.points(), self.verbs(), self.conic_weights());
        }

        debug_assert_ne!(convexity, PathConvexity::Unknown);
        self.set_convexity(convexity);
        convexity
    }
}

// Port of: src/core/SkPath.cpp#L169-L193 (chrome/m156)
fn check_edge_against_rect(p0: Point, p1: Point, rect: &Rect, dir: PathDirection) -> bool {
    let (v, edge_begin) = if dir == PathDirection::CW {
        (p1 - p0, p0)
    } else {
        (p0 - p1, p1)
    };
    if v.x != 0.0 || v.y != 0.0 {
        // check the cross product of v with the vec from edgeBegin to each rect corner
        let y_l = v.y * (rect.left - edge_begin.x);
        let x_t = v.x * (rect.top - edge_begin.y);
        let y_r = v.y * (rect.right - edge_begin.x);
        let x_b = v.x * (rect.bottom - edge_begin.y);
        if (x_t < y_l) || (x_t < y_r) || (x_b < y_l) || (x_b < y_r) {
            return false;
        }
    }
    true
}

/// Iterates a path's verbs, points and conic weights; can force-close open contours.
// Port of: include/core/SkPath.h#L788-L908, src/core/SkPath.cpp#L399-L578 (chrome/m156)
#[doc(alias = "SkPath::Iter")]
#[derive(Clone, Debug, Default)]
pub struct Iter<'a> {
    pts: &'a [Point],
    verbs: &'a [PathVerb],
    conics: &'a [scalar],
    p: usize,
    v: usize,
    // C++ keeps a pointer that begins one behind; this is the number of conics returned so far.
    conics_seen: usize,
    move_to: Point,
    last_pt: Point,
    storage: [Point; 4],
    force_close: bool,
    need_close: bool,
    close_line: bool,
}

impl<'a> Iter<'a> {
    /// Iterates `path`. If `force_close`, open contours generate a line and a close.
    // Port of: src/core/SkPath.cpp#L412-L414 (chrome/m156)
    #[must_use]
    pub fn new(path: &'a Path, force_close: bool) -> Self {
        let mut it = Self::default();
        it.set_path(path, force_close);
        it
    }

    /// Restarts the iteration on `path`.
    // Port of: src/core/SkPath.cpp#L416-L433 (chrome/m156)
    #[doc(alias = "setPath")]
    pub fn set_path(&mut self, path: &'a Path, force_close: bool) {
        self.pts = path.points();
        self.verbs = path.verbs();
        self.conics = path.conic_weights();
        self.p = 0;
        self.v = 0;
        self.conics_seen = 0;
        self.last_pt = Point::new(0.0, 0.0);
        self.move_to = Point::new(0.0, 0.0);
        self.force_close = force_close;
        self.need_close = false;
    }

    /// The weight of the conic last returned, or `None` if no conic has been returned yet.
    #[doc(alias = "conicWeight")]
    #[must_use]
    pub fn conic_weight(&self) -> Option<scalar> {
        if self.conics_seen == 0 {
            None
        } else {
            Some(self.conics[self.conics_seen - 1])
        }
    }

    /// True if the last line returned was generated by a close.
    #[doc(alias = "isCloseLine")]
    #[must_use]
    pub fn is_close_line(&self) -> bool {
        self.close_line
    }

    /// True if subsequent calls return a close before a move.
    // Port of: src/core/SkPath.cpp#L435-L461 (chrome/m156)
    #[doc(alias = "isClosedContour")]
    #[must_use]
    pub fn is_closed_contour(&self) -> bool {
        if self.v >= self.verbs.len() {
            return false;
        }
        if self.force_close {
            return true;
        }

        let mut verbs = self.v;
        let stop = self.verbs.len();

        if self.verbs[verbs] == PathVerb::Move {
            verbs += 1; // skip the initial moveto
        }

        while verbs < stop {
            let v = self.verbs[verbs];
            verbs += 1;
            if v == PathVerb::Move {
                break;
            }
            if v == PathVerb::Close {
                return true;
            }
        }
        false
    }

    // Port of: src/core/SkPath.cpp#L463-L482 (chrome/m156)
    fn auto_close(&mut self, pts: &mut [Point; 4]) -> PathVerb {
        if self.last_pt != self.move_to {
            // A special case: if both points are NaN, SkPoint::operation== returns
            // false, but the iterator expects that they are treated as the same.
            // (consider SkPoint is a 2-dimension float point).
            if is_nan(self.last_pt.x)
                || is_nan(self.last_pt.y)
                || is_nan(self.move_to.x)
                || is_nan(self.move_to.y)
            {
                return PathVerb::Close;
            }

            pts[0] = self.last_pt;
            pts[1] = self.move_to;
            self.last_pt = self.move_to;
            self.close_line = true;
            return PathVerb::Line;
        }
        pts[0] = self.move_to;
        PathVerb::Close
    }

    /// The next verb (`Verb::Done` at the end), with its points written to `pts`.
    // Port of: src/core/SkPath.cpp#L484-L557 (chrome/m156)
    #[doc(alias = "next")]
    pub fn next_verb(&mut self, pts: &mut [Point; 4]) -> Verb {
        if self.v == self.verbs.len() {
            // Close the curve if requested and if there is some curve to close
            if self.need_close {
                if self.auto_close(pts) == PathVerb::Line {
                    return Verb::Line;
                }
                self.need_close = false;
                return Verb::Close;
            }
            return Verb::Done;
        }

        let mut verb = self.verbs[self.v];
        self.v += 1;
        let mut src = self.p;

        match verb {
            PathVerb::Move => {
                if self.need_close {
                    self.v -= 1; // move back one verb
                    let verb = self.auto_close(pts);
                    if verb == PathVerb::Close {
                        self.need_close = false;
                    }
                    return verb.into();
                }
                if self.v == self.verbs.len() {
                    // might be a trailing moveto
                    return Verb::Done;
                }
                self.move_to = self.pts[src];
                pts[0] = self.pts[src];
                src += 1;
                self.last_pt = self.move_to;
                self.need_close = self.force_close;
            }
            PathVerb::Line => {
                pts[0] = self.last_pt;
                pts[1] = self.pts[src];
                self.last_pt = self.pts[src];
                self.close_line = false;
                src += 1;
            }
            PathVerb::Conic | PathVerb::Quad => {
                if verb == PathVerb::Conic {
                    self.conics_seen += 1;
                }
                pts[0] = self.last_pt;
                pts[1..3].copy_from_slice(&self.pts[src..src + 2]);
                self.last_pt = self.pts[src + 1];
                src += 2;
            }
            PathVerb::Cubic => {
                pts[0] = self.last_pt;
                pts[1..4].copy_from_slice(&self.pts[src..src + 3]);
                self.last_pt = self.pts[src + 2];
                src += 3;
            }
            PathVerb::Close => {
                verb = self.auto_close(pts);
                if verb == PathVerb::Line {
                    self.v -= 1; // move back one verb
                } else {
                    self.need_close = false;
                }
                self.last_pt = self.move_to;
            }
        }
        self.p = src;
        verb.into()
    }

    /// The next verb with its points, or `None` at the end (the `std::optional<IterRec>`
    /// overload).
    // Port of: src/core/SkPath.cpp#L566-L578 (chrome/m156)
    pub fn next_rec(&mut self) -> Option<IterRec> {
        let mut storage = self.storage;
        let legacy_verb = self.next_verb(&mut storage);
        self.storage = storage;
        let verb = match legacy_verb {
            Verb::Done => return None,
            Verb::Move => PathVerb::Move,
            Verb::Line => PathVerb::Line,
            Verb::Quad => PathVerb::Quad,
            Verb::Conic => PathVerb::Conic,
            Verb::Cubic => PathVerb::Cubic,
            Verb::Close => PathVerb::Close,
        };
        let n = iter_points_per_verb(verb);
        let w = if verb == PathVerb::Conic {
            self.conics[self.conics_seen - 1]
        } else {
            1.0
        };
        Some(PathIterRec::new(&self.storage[..n], w, verb))
    }
}

// Port of: src/core/SkPath.cpp#L559-L564 (chrome/m156)
fn iter_points_per_verb(verb: PathVerb) -> usize {
    const COUNTS: [usize; 6] = [1, 2, 3, 3, 4, 0];
    COUNTS[verb as usize]
}

impl Iterator for Iter<'_> {
    type Item = (Verb, Vec<Point>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut points = [Point::default(); Verb::MAX_POINTS];
        let verb = self.next_verb(&mut points);
        if verb == Verb::Done {
            None
        } else {
            Some((verb, points[..verb.points()].to_vec()))
        }
    }
}

/// Iterates a path's verbs and points unaltered. Use [`Iter`] instead.
// Port of: include/core/SkPath.h#L1001-L1058, src/core/SkPath.cpp#L580-L619 (chrome/m156)
#[doc(alias = "SkPath::RawIter")]
#[derive(Clone, Debug)]
pub struct RawIter<'a> {
    iter: path_priv::RangeIter<'a>,
    conic_weight: scalar,
}

impl Default for RawIter<'_> {
    fn default() -> Self {
        Self {
            iter: path_priv::iterate_raw(&[], &[], &[]),
            conic_weight: 0.0,
        }
    }
}

impl<'a> RawIter<'a> {
    /// Iterates `path`.
    #[must_use]
    pub fn new(path: &'a Path) -> RawIter<'a> {
        RawIter::default().set_path(path)
    }

    /// Restarts the iteration on `path`.
    // Port of: src/core/SkPath.cpp#L580-L584 (chrome/m156)
    #[must_use]
    pub fn set_path(mut self, path: &'a Path) -> RawIter<'a> {
        self.iter = path_priv::iterate(path);
        self
    }

    /// The next verb, without advancing (`Verb::Done` at the end).
    #[must_use]
    pub fn peek(&self) -> Verb {
        self.iter.peek_verb().map_or(Verb::Done, Verb::from)
    }

    /// The weight of the conic last returned.
    #[doc(alias = "conicWeight")]
    #[must_use]
    pub fn conic_weight(&self) -> scalar {
        self.conic_weight
    }

    /// The next verb (`Verb::Done` at the end), with its points written to `pts`.
    // Port of: src/core/SkPath.cpp#L586-L606 (chrome/m156)
    #[doc(alias = "next")]
    pub fn next_verb(&mut self, pts: &mut [Point; 4]) -> Verb {
        let Some((verb, iter_pts, weight)) = self.iter.next() else {
            return Verb::Done;
        };
        let num_pts = match verb {
            PathVerb::Move => 1,
            PathVerb::Line => 2,
            PathVerb::Quad => 3,
            PathVerb::Conic => {
                self.conic_weight = weight.unwrap_or(0.0);
                3
            }
            PathVerb::Cubic => 4,
            PathVerb::Close => 0,
        };
        pts[..num_pts].copy_from_slice(&iter_pts[..num_pts]);
        verb.into()
    }

    /// The next verb with its points, or `None` at the end.
    // Port of: src/core/SkPath.cpp#L608-L619 (chrome/m156)
    pub fn next_rec(&mut self) -> Option<IterRec> {
        let (verb, iter_pts, weight) = self.iter.next()?;
        let n = iter_points_per_verb(verb);
        Some(PathIterRec::new(
            &iter_pts[..n],
            weight.unwrap_or(1.0),
            verb,
        ))
    }
}

impl Iterator for RawIter<'_> {
    type Item = (Verb, Vec<Point>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut points = [Point::default(); Verb::MAX_POINTS];
        let verb = self.next_verb(&mut points);
        (verb != Verb::Done).then(|| (verb, points[..verb.points()].to_vec()))
    }
}
