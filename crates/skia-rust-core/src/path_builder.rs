// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathBuilder.h, src/core/SkPathBuilder.cpp

//! Building paths (`SkPathBuilder.h`).

use std::sync::Arc;

use crate::floating_point::is_finite;
use crate::geometry::chop_cubic_at_half;
use crate::geometry::{Conic, MAX_CONICS_FOR_ARC};
use crate::matrix::Matrix;
use crate::path::{AddPathMode, Path};
use crate::path_data::PathData;
use crate::path_enums::{PathConvexity, ResolveConvexity, direction_to_convexity};
use crate::path_iter::PathIter;
use crate::path_priv::{self, RRectAsEnum};
use crate::path_raw::PathRaw;
use crate::path_raw_shapes;
use crate::path_ref::{PathIsAData, PathIsAType};
use crate::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use crate::point::{Point, Vector};
use crate::rect::Rect;
use crate::rrect::RRect;
use crate::safe32::sat_add;
use crate::scalar::{
    SCALAR_HALF, SCALAR_PI, Scalar, degrees_to_radians, scalar, scalar_abs, scalar_atan2,
    scalar_ceil_to_int, scalar_copy_sign, scalar_cos, scalar_cos_snap_to_zero,
    scalar_floor_to_scalar, scalar_mod, scalar_round_to_scalar, scalar_sin,
    scalar_sin_snap_to_zero, scalar_sqrt, scalar_tan,
};

/// Which of the two possible arcs `arc_to_radius` draws.
// Port of: include/core/SkPathBuilder.h#L415-L418 (chrome/m156)
#[doc(alias = "SkPathBuilder::ArcSize")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum ArcSize {
    /// Smaller of the arc pair.
    #[default]
    Small = 0,
    /// Larger of the arc pair.
    Large = 1,
}

/// How [`PathBuilder::add_raw`] reserves storage.
// Port of: include/core/SkPathBuilder.h#L974-L977 (chrome/m156)
#[doc(alias = "SkPathBuilder::Reserve")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Reserve {
    Exact,
    Grow,
}

/// Number format of [`PathBuilder::dump_to_string`].
// Port of: include/core/SkPathBuilder.h#L989-L992 (chrome/m156)
#[doc(alias = "SkPathBuilder::DumpFormat")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum DumpFormat {
    #[default]
    Decimal,
    Hex,
}

/// Builds [`Path`]s.
// Port of: include/core/SkPathBuilder.h#L38-L1050 (chrome/m156)
#[doc(alias = "SkPathBuilder")]
#[derive(Clone, Debug)]
pub struct PathBuilder {
    pub(crate) pts: Vec<Point>,
    pub(crate) verbs: Vec<PathVerb>,
    pub(crate) conic_weights: Vec<scalar>,

    pub(crate) fill_type: PathFillType,
    pub(crate) is_volatile: bool,
    pub(crate) convexity: PathConvexity,

    pub(crate) segment_mask: u32,
    pub(crate) last_move_index: i32, // only needed until SkPath is immutable

    pub(crate) ty: PathIsAType,
    pub(crate) is_a: PathIsAData,
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Port of: src/core/SkPathBuilder.cpp#L108-L119 (chrome/m156)
impl PartialEq for PathBuilder {
    fn eq(&self, o: &Self) -> bool {
        // quick-accept
        if std::ptr::eq(self, o) {
            return true;
        }
        // quick-reject
        if self.segment_mask != o.segment_mask || self.fill_type != o.fill_type {
            return false;
        }
        // deep compare
        self.verbs == o.verbs && self.pts == o.pts && self.conic_weights == o.conic_weights
    }
}

impl From<PathBuilder> for Path {
    fn from(mut builder: PathBuilder) -> Self {
        builder.detach()
    }
}

// Port of: src/core/SkPathBuilder.cpp#L38-L48 (chrome/m156)
fn subdivide_cubic_to(path: &mut PathBuilder, pts: &[Point], level: i32) {
    let level = level - 1;
    if level >= 0 {
        let mut tmp = [Point::default(); 7];

        chop_cubic_at_half(pts, &mut tmp);
        subdivide_cubic_to(path, &tmp[0..4], level);
        subdivide_cubic_to(path, &tmp[3..7], level);
    } else {
        path.cubic_to(pts[1], pts[2], pts[3]);
    }
}

// Port of: src/core/SkPathBuilder.cpp#L351-L370 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in C++
fn arc_is_lone_point(oval: &Rect, start_angle: scalar, sweep_angle: scalar) -> Option<Point> {
    if 0.0 == sweep_angle && (0.0 == start_angle || 360.0 == start_angle) {
        // Chrome uses this path to move into and out of ovals. If not
        // treated as a special case the moves can distort the oval's
        // bounding box (and break the circle special case).
        return Some(Point::new(oval.right, oval.center_y()));
    } else if 0.0 == oval.width() && 0.0 == oval.height() {
        // Chrome will sometimes create 0 radius round rects. Having degenerate
        // quad segments in the path prevents the path from being recognized as
        // a rect.
        // TODO: optimizing the case where only one of width or height is zero
        // should also be considered. This case, however, doesn't seem to be
        // as common as the single point case.
        return Some(Point::new(oval.right, oval.top));
    }
    None
}

// Return the unit vectors pointing at the start/stop points for the given start/sweep angles
// Port of: src/core/SkPathBuilder.cpp#L374-L406 (chrome/m156)
fn angles_to_unit_vectors(
    start_angle: scalar,
    sweep_angle: scalar,
) -> (Vector, Vector, PathDirection) {
    let start_rad = degrees_to_radians(start_angle);
    let mut stop_rad = degrees_to_radians(start_angle + sweep_angle);

    let start_v = Vector::new(
        scalar_cos_snap_to_zero(start_rad),
        scalar_sin_snap_to_zero(start_rad),
    );
    let mut stop_v = Vector::new(
        scalar_cos_snap_to_zero(stop_rad),
        scalar_sin_snap_to_zero(stop_rad),
    );

    /*  If the sweep angle is nearly (but less than) 360, then due to precision
    loss in radians-conversion and/or sin/cos, we may end up with coincident
    vectors, which will fool SkBuildQuadArc into doing nothing (bad) instead
    of drawing a nearly complete circle (good).
    e.g. canvas.drawArc(0, 359.99, ...)
    -vs- canvas.drawArc(0, 359.9, ...)
    We try to detect this edge case, and tweak the stop vector
    */
    if start_v == stop_v {
        let sw = scalar_abs(sweep_angle);
        if sw < 360.0 && sw > 359.0 {
            // make a guess at a tiny angle (in radians) to tweak by
            let delta_rad = scalar_copy_sign(1.0 / 512.0, sweep_angle);
            // not sure how much will be enough, so we use a loop
            loop {
                stop_rad -= delta_rad;
                stop_v.y = scalar_sin_snap_to_zero(stop_rad);
                stop_v.x = scalar_cos_snap_to_zero(stop_rad);
                if start_v != stop_v {
                    break;
                }
            }
        }
    }
    let dir = if sweep_angle > 0.0 {
        PathDirection::CW
    } else {
        PathDirection::CCW
    };
    (start_v, stop_v, dir)
}

// If this returns 0, then the caller should just line-to the singlePt, else it should
// ignore singlePt and append the specified number of conics.
// Port of: src/core/SkPathBuilder.cpp#L412-L425 (chrome/m156)
fn build_arc_conics(
    oval: &Rect,
    start: Vector,
    stop: Vector,
    dir: PathDirection,
    conics: &mut [Conic; MAX_CONICS_FOR_ARC],
    single_pt: &mut Point,
) -> usize {
    let mut matrix = Matrix::default();

    matrix.set_scale((oval.width() / 2.0, oval.height() / 2.0), None);
    matrix.post_translate((oval.center_x(), oval.center_y()));

    let count = Conic::build_unit_arc(start, stop, dir, Some(&matrix), conics);
    if 0 == count {
        *single_pt = matrix.map_point(stop);
    }
    count
}

// Port of: src/core/SkPathBuilder.cpp#L427-L430 (chrome/m156)
fn nearly_equal(a: Point, b: Point) -> bool {
    scalar::nearly_equal(a.x, b.x, None) && scalar::nearly_equal(a.y, b.y, None)
}

// It's tempting to just look at fSegmentMask, but we could have a degenerate path (move,close)
// before this, which is two verbs but still fSegmentMask == 0.
// Port of: src/core/SkPathBuilder.cpp#L738-L740 (chrome/m156)
fn is_empty_or_moves(verbs: &[PathVerb]) -> bool {
    verbs.is_empty() || (verbs.len() == 1 && verbs[0] == PathVerb::Move)
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// `SkTArray::reserve(n)`: make sure the capacity is at least `n`.
fn reserve_total<T>(v: &mut Vec<T>, n: i32) {
    if let Ok(n) = usize::try_from(n) {
        v.reserve(n.saturating_sub(v.len()));
    }
}

fn reserve_exact_total<T>(v: &mut Vec<T>, n: i32) {
    if let Ok(n) = usize::try_from(n) {
        v.reserve_exact(n.saturating_sub(v.len()));
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTArray sizes are int
fn len_i32<T>(v: &[T]) -> i32 {
    v.len() as i32
}

impl PathBuilder {
    /// An empty builder.
    // Port of: src/core/SkPathBuilder.cpp#L52-L54 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        let mut b = Self {
            pts: Vec::new(),
            verbs: Vec::new(),
            conic_weights: Vec::new(),
            fill_type: PathFillType::DEFAULT,
            is_volatile: false,
            convexity: PathConvexity::Unknown,
            segment_mask: 0,
            last_move_index: -1,
            ty: PathIsAType::General,
            is_a: PathIsAData::default(),
        };
        b.reset();
        b
    }

    /// An empty builder with the given fill type.
    // Port of: src/core/SkPathBuilder.cpp#L62-L65 (chrome/m156)
    #[must_use]
    pub fn new_with_fill_type(fill_type: PathFillType) -> Self {
        let mut b = Self::new();
        b.fill_type = fill_type;
        b
    }

    /// A builder initialized with the contents of `path`.
    // Port of: src/core/SkPathBuilder.cpp#L67-L69 (chrome/m156)
    #[must_use]
    pub fn new_path(path: &Path) -> Self {
        let mut b = Self::new();
        b.assign_path(path);
        b
    }

    /// Replaces the contents with `src` (`operator=(const SkPath&)`).
    // Port of: src/core/SkPathBuilder.cpp#L71-L88 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ casts
    pub fn assign_path(&mut self, src: &Path) -> &mut Self {
        self.reset().set_fill_type(src.fill_type());
        self.set_is_volatile(src.is_volatile());

        if src.is_empty() {
            return self;
        }

        self.add_raw(
            &src.data.raw(src.fill_type(), ResolveConvexity::Yes),
            Reserve::Exact,
        );

        // These are not part of SkPathRaw, so we set them separately
        self.last_move_index = path_priv::find_last_move_to_index(&self.verbs, self.pts.len());
        debug_assert!((self.last_move_index as usize) < self.pts.len());
        self.ty = src.data.ty();
        self.is_a = src.data.is_a();

        self
    }

    /// The fill type.
    #[doc(alias = "fillType")]
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        self.fill_type
    }

    /// The bounds of the points (ignoring a trailing move), or `None` if any point is not finite.
    // Port of: src/core/SkPathBuilder.cpp#L1113-L1115 (chrome/m156)
    #[doc(alias = "computeFiniteBounds")]
    #[must_use]
    pub fn compute_finite_bounds(&self) -> Option<Rect> {
        path_priv::trimmed_bounds(self.points(), self.verbs())
    }

    /// The bounds of the curves, or `None` if any point is not finite.
    // Port of: src/core/SkPathBuilder.cpp#L1117-L1122 (chrome/m156)
    #[doc(alias = "computeTightBounds")]
    #[must_use]
    pub fn compute_tight_bounds(&self) -> Option<Rect> {
        if !self.is_finite() {
            return None;
        }
        Some(path_priv::compute_tight_bounds(
            self.points(),
            self.verbs(),
            self.conic_weights(),
        ))
    }

    /// The finite bounds, or empty.
    // Port of: include/core/SkPathBuilder.h#L111-L116 (chrome/m156)
    #[doc(alias = "computeBounds")]
    #[must_use]
    pub fn compute_bounds(&self) -> Rect {
        self.compute_finite_bounds().unwrap_or_else(Rect::new_empty)
    }

    /// A path with the current contents.
    #[must_use]
    pub fn snapshot(&self) -> Path {
        self.snapshot_and_transform(None)
    }

    /// A path with the current contents transformed by `mx`.
    // Port of: src/core/SkPathBuilder.cpp#L290-L308 (chrome/m156)
    #[doc(alias = "snapshot")]
    #[must_use]
    pub fn snapshot_and_transform<'m>(&self, mx: impl Into<Option<&'m Matrix>>) -> Path {
        let mx = mx.into().unwrap_or(Matrix::i());

        let mut pdata = None;
        if let Some(raw) = path_priv::raw_builder(self, ResolveConvexity::No) {
            pdata = PathData::make_transform_raw(&raw, mx);
        }
        if let Some(p) = pdata.as_mut()
            && self.ty != PathIsAType::General
        {
            debug_assert!(path_priv::is_axis_aligned(&self.pts));
            if mx.rect_stays_rect() && !p.bounds().is_empty() {
                let (dir, start) = path_priv::transform_dir_and_start(
                    mx,
                    self.ty == PathIsAType::RRect,
                    self.is_a.direction,
                    u32::from(self.is_a.start_index),
                );
                PathData::setup_is_a_arc(p, self.ty, dir, start);
            }
        }
        Path::make_null_check(pdata, self.fill_type, self.is_volatile)
    }

    /// A path with the current contents; the builder is reset.
    #[must_use]
    pub fn detach(&mut self) -> Path {
        self.detach_and_transform(None)
    }

    /// A path with the current contents transformed by `mx`; the builder is reset.
    // Port of: src/core/SkPathBuilder.cpp#L310-L314 (chrome/m156)
    #[must_use]
    pub fn detach_and_transform<'m>(&mut self, mx: impl Into<Option<&'m Matrix>>) -> Path {
        let path = self.snapshot_and_transform(mx);
        self.reset();
        path
    }

    /// The current contents as path data, or `None` if they are not valid.
    // Port of: src/core/SkPathBuilder.cpp#L318-L341 (chrome/m156)
    #[doc(alias = "snapshotData")]
    #[must_use]
    pub fn snapshot_data(&self) -> Option<Arc<PathData>> {
        if self.verbs.len() <= 1 {
            return Some(PathData::empty());
        }

        match self.ty {
            PathIsAType::General => {}
            PathIsAType::Oval => {
                let r = Rect::bounds(&self.pts)?;
                return PathData::oval(&r, self.is_a.direction, u32::from(self.is_a.start_index));
            }
            PathIsAType::RRect => {
                let r = Rect::bounds(&self.pts)?;
                return PathData::rrect(
                    &path_priv::deduce_rrect_from_contour(&r, &self.pts, &self.verbs),
                    self.is_a.direction,
                    u32::from(self.is_a.start_index),
                );
            }
        }
        debug_assert_eq!(self.ty, PathIsAType::General);

        PathData::make(&self.pts, &self.verbs, &self.conic_weights)
    }

    /// [`snapshot_data`](Self::snapshot_data), then resets the builder.
    // Port of: src/core/SkPathBuilder.cpp#L343-L347 (chrome/m156)
    #[doc(alias = "detachData")]
    pub fn detach_data(&mut self) -> Option<Arc<PathData>> {
        let data = self.snapshot_data();
        self.reset();
        data
    }

    /// Sets the fill type.
    #[doc(alias = "setFillType")]
    pub fn set_fill_type(&mut self, ft: PathFillType) -> &mut Self {
        self.fill_type = ft;
        self
    }

    /// Sets the volatility flag.
    #[doc(alias = "setIsVolatile")]
    pub fn set_is_volatile(&mut self, is_volatile: bool) -> &mut Self {
        self.is_volatile = is_volatile;
        self
    }

    /// Resets to an empty builder with the default fill type.
    // Port of: src/core/SkPathBuilder.cpp#L90-L106 (chrome/m156)
    pub fn reset(&mut self) -> &mut Self {
        self.pts.clear();
        self.verbs.clear();
        self.conic_weights.clear();
        self.fill_type = PathFillType::DEFAULT;
        self.is_volatile = false;

        // these are internal state

        self.segment_mask = 0;
        self.last_move_index = -1; // illegal

        self.ty = PathIsAType::General;
        self.convexity = PathConvexity::Unknown;

        self
    }

    // Port of: include/core/SkPathBuilder.h#L1027-L1034 (chrome/m156)
    fn ensure_move(&mut self) {
        self.ty = PathIsAType::General;
        if self.verbs.is_empty() {
            self.move_to((0.0, 0.0));
        } else if self.verbs.last() == Some(&PathVerb::Close) {
            #[allow(clippy::cast_sign_loss)] // a close implies a previous move
            let p = self.pts[self.last_move_index as usize];
            self.move_to(p);
        }
    }

    /// Starts a new contour at `pt` (replacing a trailing move).
    // Port of: src/core/SkPathBuilder.cpp#L159-L179 (chrome/m156)
    #[doc(alias = "moveTo")]
    #[allow(clippy::missing_panics_doc)] // panics only if an internal invariant is broken
    pub fn move_to(&mut self, pt: impl Into<Point>) -> &mut Self {
        let pt = pt.into();
        if self.verbs.last() == Some(&PathVerb::Move) {
            *self.pts.last_mut().expect("a move has a point") = pt;

            debug_assert!(self.ty != PathIsAType::Oval && self.ty != PathIsAType::RRect);
            debug_assert_eq!(self.convexity, PathConvexity::Unknown);
            debug_assert_eq!(self.last_move_index, len_i32(&self.pts) - 1);
        } else {
            self.last_move_index = len_i32(&self.pts);

            self.pts.push(pt);
            self.verbs.push(PathVerb::Move);

            if self.ty == PathIsAType::Oval || self.ty == PathIsAType::RRect {
                self.ty = PathIsAType::General;
            }
            self.convexity = PathConvexity::Unknown;
        }

        self
    }

    /// Adds a line to `pt`.
    // Port of: src/core/SkPathBuilder.cpp#L181-L189 (chrome/m156)
    #[doc(alias = "lineTo")]
    pub fn line_to(&mut self, pt: impl Into<Point>) -> &mut Self {
        let pt = pt.into();
        self.ensure_move();

        self.pts.push(pt);
        self.verbs.push(PathVerb::Line);

        self.segment_mask |= PathSegmentMask::LINE.bits();
        self
    }

    /// Adds a quad.
    // Port of: src/core/SkPathBuilder.cpp#L191-L201 (chrome/m156)
    #[doc(alias = "quadTo")]
    pub fn quad_to(&mut self, p1: impl Into<Point>, p2: impl Into<Point>) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        self.ensure_move();

        self.pts.push(p1);
        self.pts.push(p2);
        self.verbs.push(PathVerb::Quad);

        self.segment_mask |= PathSegmentMask::QUAD.bits();
        self
    }

    /// Adds a conic. A weight `<= 0` adds a line; `1` adds a quad; a non-finite weight adds two
    /// lines.
    // Port of: src/core/SkPathBuilder.cpp#L203-L226 (chrome/m156)
    #[doc(alias = "conicTo")]
    #[allow(clippy::float_cmp)] // exact comparison, as in C++
    pub fn conic_to(&mut self, p1: impl Into<Point>, p2: impl Into<Point>, w: scalar) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        self.ensure_move();

        if w <= 0.0 {
            return self.line_to(p2);
        }
        self.pts.push(p1);
        self.pts.push(p2);
        if w == 1.0 {
            self.verbs.push(PathVerb::Quad);
            self.segment_mask |= PathSegmentMask::QUAD.bits();
        } else if is_finite(w) {
            self.verbs.push(PathVerb::Conic);
            self.conic_weights.push(w);
            self.segment_mask |= PathSegmentMask::CONIC.bits();
        } else {
            self.verbs.push(PathVerb::Line);
            self.verbs.push(PathVerb::Line);
            self.segment_mask |= PathSegmentMask::LINE.bits();
        }

        self
    }

    /// Adds a cubic.
    // Port of: src/core/SkPathBuilder.cpp#L228-L239 (chrome/m156)
    #[doc(alias = "cubicTo")]
    pub fn cubic_to(
        &mut self,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
    ) -> &mut Self {
        let (p1, p2, p3) = (p1.into(), p2.into(), p3.into());
        self.ensure_move();

        self.pts.push(p1);
        self.pts.push(p2);
        self.pts.push(p3);
        self.verbs.push(PathVerb::Cubic);

        self.segment_mask |= PathSegmentMask::CUBIC.bits();
        self
    }

    /// Closes the current contour (a second close is ignored).
    // Port of: src/core/SkPathBuilder.cpp#L241-L248 (chrome/m156)
    pub fn close(&mut self) -> &mut Self {
        // If this is a 2nd 'close', we just ignore it
        if !self.verbs.is_empty() && self.verbs.last() != Some(&PathVerb::Close) {
            self.ensure_move();
            self.verbs.push(PathVerb::Close);
        }
        self
    }

    /// Adds lines to each of `pts`.
    // Port of: src/core/SkPathBuilder.cpp#L817-L828 (chrome/m156)
    #[doc(alias = "polylineTo")]
    pub fn polyline_to(&mut self, pts: &[Point]) -> &mut Self {
        if !pts.is_empty() {
            self.ensure_move();

            let count = len_i32(pts);
            self.inc_reserve(count, count, 0);
            self.pts.extend_from_slice(pts);
            self.verbs
                .extend(std::iter::repeat_n(PathVerb::Line, pts.len()));
            self.segment_mask |= PathSegmentMask::LINE.bits();
        }
        self
    }

    /// Starts a new contour relative to the last point.
    // Port of: src/core/SkPathBuilder.cpp#L252-L263 (chrome/m156)
    #[doc(alias = "rMoveTo")]
    pub fn r_move_to(&mut self, pt: impl Into<Vector>) -> &mut Self {
        let pt = pt.into();
        let mut last_pt = Point::new(0.0, 0.0); // in case we're empty
        if !self.pts.is_empty() {
            debug_assert!(self.last_move_index >= 0);
            if self.verbs.last() == Some(&PathVerb::Close) {
                #[allow(clippy::cast_sign_loss)] // checked above
                {
                    last_pt = self.pts[self.last_move_index as usize];
                }
            } else {
                last_pt = self.pts[self.pts.len() - 1];
            }
        }
        self.move_to(last_pt + pt)
    }

    fn last_point(&self) -> Point {
        self.pts[self.pts.len() - 1]
    }

    /// Adds a line relative to the last point.
    // Port of: src/core/SkPathBuilder.cpp#L265-L268 (chrome/m156)
    #[doc(alias = "rLineTo")]
    pub fn r_line_to(&mut self, p1: impl Into<Vector>) -> &mut Self {
        let p1 = p1.into();
        self.ensure_move();
        let base = self.last_point();
        self.line_to(base + p1)
    }

    /// Adds a quad relative to the last point.
    // Port of: src/core/SkPathBuilder.cpp#L270-L274 (chrome/m156)
    #[doc(alias = "rQuadTo")]
    pub fn r_quad_to(&mut self, p1: impl Into<Vector>, p2: impl Into<Vector>) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        self.ensure_move();
        let base = self.last_point();
        self.quad_to(base + p1, base + p2)
    }

    /// Adds a conic relative to the last point.
    // Port of: src/core/SkPathBuilder.cpp#L276-L280 (chrome/m156)
    #[doc(alias = "rConicTo")]
    pub fn r_conic_to(
        &mut self,
        p1: impl Into<Vector>,
        p2: impl Into<Vector>,
        w: scalar,
    ) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        self.ensure_move();
        let base = self.last_point();
        self.conic_to(base + p1, base + p2, w)
    }

    /// Adds a cubic relative to the last point.
    // Port of: src/core/SkPathBuilder.cpp#L282-L286 (chrome/m156)
    #[doc(alias = "rCubicTo")]
    pub fn r_cubic_to(
        &mut self,
        p1: impl Into<Vector>,
        p2: impl Into<Vector>,
        p3: impl Into<Vector>,
    ) -> &mut Self {
        let (p1, p2, p3) = (p1.into(), p2.into(), p3.into());
        self.ensure_move();
        let base = self.last_point();
        self.cubic_to(base + p1, base + p2, base + p3)
    }

    /// Appends an arc of `oval` from `start_angle` sweeping `sweep_angle` degrees.
    // Port of: src/core/SkPathBuilder.cpp#L432-L495 (chrome/m156)
    #[doc(alias = "arcTo")]
    pub fn arc_to(
        &mut self,
        oval: impl AsRef<Rect>,
        start_angle_deg: scalar,
        sweep_angle_deg: scalar,
        force_move_to: bool,
    ) -> &mut Self {
        let oval = *oval.as_ref();
        let sweep_angle = sweep_angle_deg;
        let mut force_move_to = force_move_to;
        if oval.width() < 0.0 || oval.height() < 0.0 {
            return self;
        }

        let start_angle = scalar_mod(start_angle_deg, 360.0);

        if self.verbs.is_empty() {
            force_move_to = true;
        }

        if let Some(lone_pt) = arc_is_lone_point(&oval, start_angle, sweep_angle) {
            return if force_move_to {
                self.move_to(lone_pt)
            } else {
                self.line_to(lone_pt)
            };
        }

        let (start_v, stop_v, dir) = angles_to_unit_vectors(start_angle, sweep_angle);

        let mut single_pt = Point::default();

        // Adds a move-to to 'pt' if forceMoveTo is true. Otherwise a lineTo unless we're
        // sufficiently close to 'pt' currently. This prevents spurious lineTos when adding a
        // series of contiguous arcs from the same oval.
        let add_pt = |this: &mut Self, pt: Point| {
            if force_move_to {
                this.move_to(pt);
            } else if !nearly_equal(this.last_point(), pt) {
                this.line_to(pt);
            }
        };

        // At this point, we know that the arc is not a lone point, but startV == stopV
        // indicates that the sweepAngle is too small such that angles_to_unit_vectors
        // cannot handle it.
        if start_v == stop_v {
            let end_angle = degrees_to_radians(start_angle + sweep_angle);
            let radius_x = oval.width() / 2.0;
            let radius_y = oval.height() / 2.0;
            // We do not use SkScalar[Sin|Cos]SnapToZero here. When sin(startAngle) is 0 and
            // sweepAngle is very small and radius is huge, the expected behavior here is to draw
            // a line. But calling SkScalarSinSnapToZero will make sin(endAngle) be 0 which will
            // then draw a dot.
            single_pt.set(
                oval.center_x() + radius_x * scalar_cos(end_angle),
                oval.center_y() + radius_y * scalar_sin(end_angle),
            );
            add_pt(self, single_pt);
            return self;
        }

        let mut conics = [Conic::default(); MAX_CONICS_FOR_ARC];
        let count = build_arc_conics(&oval, start_v, stop_v, dir, &mut conics, &mut single_pt);
        if count != 0 {
            let n = len_i32(&conics[..count]) * 2 + 1;
            self.inc_reserve(n, n, 0);
            let pt = conics[0].pts[0];
            add_pt(self, pt);
            for c in &conics[..count] {
                self.conic_to(c.pts[1], c.pts[2], c.w);
            }
        } else {
            add_pt(self, single_pt);
        }
        self
    }

    /// SVG-style relative arc: radii, rotation, size and sweep, to `last point + dxdy`.
    // Port of: src/core/SkPathBuilder.cpp#L497-L501 (chrome/m156)
    #[doc(alias = "rArcTo")]
    pub fn r_arc_to(
        &mut self,
        r: impl Into<Vector>,
        x_axis_rotate: scalar,
        large_arc: ArcSize,
        sweep: PathDirection,
        dxdy: impl Into<Vector>,
    ) -> &mut Self {
        let current_point = self.get_last_pt().unwrap_or(Point::new(0.0, 0.0));
        let end = current_point + dxdy.into();
        self.arc_to_radius(r.into(), x_axis_rotate, large_arc, sweep, end)
    }

    /// Adds an arc of `oval` as a new contour (an oval if the sweep is a full circle starting at
    /// a multiple of 90 degrees).
    // Port of: src/core/SkPathBuilder.cpp#L503-L525 (chrome/m156)
    #[doc(alias = "addArc")]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // `(unsigned) startIndex`
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pub fn add_arc(
        &mut self,
        oval: impl AsRef<Rect>,
        start_angle_deg: scalar,
        sweep_angle_deg: scalar,
    ) -> &mut Self {
        let oval = *oval.as_ref();
        let start_angle = start_angle_deg;
        let sweep_angle = sweep_angle_deg;
        if oval.is_empty() || 0.0 == sweep_angle {
            return self;
        }

        const FULL_CIRCLE_ANGLE: scalar = 360.0;

        if sweep_angle >= FULL_CIRCLE_ANGLE || sweep_angle <= -FULL_CIRCLE_ANGLE {
            // We can treat the arc as an oval if it begins at one of our legal starting positions.
            // See SkPath::addOval() docs.
            let start_over_90 = start_angle / 90.0;
            let start_over_90_i = scalar_round_to_scalar(start_over_90);
            let error = start_over_90 - start_over_90_i;
            if scalar::nearly_equal(error, 0.0, None) {
                // Index 1 is at startAngle == 0.
                let mut start_index = (start_over_90_i + 1.0) % 4.0;
                start_index = if start_index < 0.0 {
                    start_index + 4.0
                } else {
                    start_index
                };
                return self.add_oval(
                    oval,
                    if sweep_angle > 0.0 {
                        PathDirection::CW
                    } else {
                        PathDirection::CCW
                    },
                    start_index as usize,
                );
            }
        }
        self.arc_to(oval, start_angle, sweep_angle, true)
    }

    /// Adds a line and a conic tangent to the lines (`p0`=last, `p1`) and (`p1`, `p2`).
    // Port of: src/core/SkPathBuilder.cpp#L527-L561 (chrome/m156)
    #[doc(alias = "arcTo")]
    #[allow(clippy::cast_possible_truncation)] // SkDoubleToScalar
    pub fn arc_to_tangent(
        &mut self,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        radius: scalar,
    ) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        self.ensure_move();

        if radius == 0.0 {
            return self.line_to(p1);
        }

        // need to know our prev pt so we can construct tangent vectors
        let start = self.last_point();

        // need double precision for these calcs.
        let befored = normalize_d2([f64::from(p1.x - start.x), f64::from(p1.y - start.y)]);
        let afterd = normalize_d2([f64::from(p2.x - p1.x), f64::from(p2.y - p1.y)]);
        let cosh = dot_d2(befored, afterd);
        let sinh = cross_d2(befored, afterd);

        // If the previous point equals the first point, befored will be denormalized.
        // If the two points equal, afterd will be denormalized.
        // If the second point equals the first point, sinh will be zero.
        // In all these cases, we cannot construct an arc, so we construct a line to the first
        // point.
        if !isfinite_d2(befored) || !isfinite_d2(afterd) || (sinh as scalar).nearly_zero(None) {
            return self.line_to(p1);
        }

        // safe to convert back to floats now
        let dist = scalar_abs((f64::from(radius) * (1.0 - cosh) / sinh) as scalar);
        let xx = (f64::from(p1.x) - f64::from(dist) * befored[0]) as scalar;
        let yy = (f64::from(p1.y) - f64::from(dist) * befored[1]) as scalar;

        let mut after = Vector::new(afterd[0] as scalar, afterd[1] as scalar);
        after.set_length(dist);
        self.line_to((xx, yy));
        let weight = scalar_sqrt((f64::from(SCALAR_HALF) + cosh * 0.5) as scalar);
        self.conic_to(p1, p1 + after, weight)
    }

    /// SVG-style arc to `xy` with radii `r`, rotated by `x_axis_rotate` degrees.
    ///
    /// This converts the SVG arc to conics. Partly adapted from Niko's code in
    /// kdelibs/kdecore/svgicons, then transcribed from webkit/chrome's
    /// `SVGPathNormalizer::decomposeArcToCubic()`. Note that `arcSweep` is flipped from the
    /// original implementation.
    // Port of: src/core/SkPathBuilder.cpp#L569-L695 (chrome/m156)
    #[doc(alias = "arcTo")]
    #[allow(clippy::too_many_lines, clippy::similar_names)] // mirrors the C++ function
    #[allow(clippy::missing_panics_doc)] // panics only if an internal invariant is broken
    pub fn arc_to_radius(
        &mut self,
        r: impl Into<Point>,
        x_axis_rotate: scalar,
        large_arc: ArcSize,
        sweep: PathDirection,
        xy: impl Into<Point>,
    ) -> &mut Self {
        let rad = r.into();
        let angle = x_axis_rotate;
        let arc_large = large_arc;
        let arc_sweep = sweep;
        let end_pt = xy.into();
        self.ensure_move();

        let src_pts = [self.last_point(), end_pt];

        // If rx = 0 or ry = 0 then this arc is treated as a straight line segment (a "lineto")
        // joining the endpoints.
        // http://www.w3.org/TR/SVG/implnote.html#ArcOutOfRangeParameters
        if rad.x == 0.0 || rad.y == 0.0 {
            return self.line_to(end_pt);
        }
        // If the current point and target point for the arc are identical, it should be treated
        // as a zero length path. This ensures continuity in animations.
        if src_pts[0] == src_pts[1] {
            return self.line_to(end_pt);
        }
        let mut rx = scalar_abs(rad.x);
        let mut ry = scalar_abs(rad.y);
        let mut mid_point_distance = src_pts[0] - src_pts[1];
        mid_point_distance *= 0.5;

        let mut point_transform = Matrix::default();
        point_transform.set_rotate(-angle, None);

        let transformed_mid_point = point_transform.map_point(mid_point_distance);
        let square_rx = rx * rx;
        let square_ry = ry * ry;
        let square_x = transformed_mid_point.x * transformed_mid_point.x;
        let square_y = transformed_mid_point.y * transformed_mid_point.y;

        // Check if the radii are big enough to draw the arc, scale radii if not.
        // http://www.w3.org/TR/SVG/implnote.html#ArcCorrectionOutOfRangeRadii
        let mut radii_scale = square_x / square_rx + square_y / square_ry;
        if radii_scale > 1.0 {
            radii_scale = scalar_sqrt(radii_scale);
            rx *= radii_scale;
            ry *= radii_scale;
        }

        point_transform.set_scale((1.0 / rx, 1.0 / ry), None);
        point_transform.pre_rotate(-angle, None);

        let mut unit_pts = [Point::default(); 2];
        point_transform.map_points(&mut unit_pts, &src_pts);
        let mut delta = unit_pts[1] - unit_pts[0];

        let d = delta.x * delta.x + delta.y * delta.y;
        let scale_factor_squared = std_max(1.0 / d - 0.25, 0.0);

        let mut scale_factor = scalar_sqrt(scale_factor_squared);
        // flipped from the original implementation
        if (arc_sweep == PathDirection::CCW) != (arc_large == ArcSize::Large) {
            scale_factor = -scale_factor;
        }
        delta.scale(scale_factor);
        let mut center_point = unit_pts[0] + unit_pts[1];
        center_point *= 0.5;
        center_point.offset((-delta.y, delta.x));
        unit_pts[0] -= center_point;
        unit_pts[1] -= center_point;
        let theta1 = scalar_atan2(unit_pts[0].y, unit_pts[0].x);
        let theta2 = scalar_atan2(unit_pts[1].y, unit_pts[1].x);
        let mut theta_arc = theta2 - theta1;
        if theta_arc < 0.0 && (arc_sweep == PathDirection::CW) {
            // arcSweep flipped from the original implementation
            theta_arc += SCALAR_PI * 2.0;
        } else if theta_arc > 0.0 && (arc_sweep != PathDirection::CW) {
            // arcSweep flipped from the original implementation
            theta_arc -= SCALAR_PI * 2.0;
        }

        // Very tiny angles cause our subsequent math to go wonky (skbug.com/40040578)
        // so we do a quick check here. The precise tolerance amount is just made up.
        // PI/million happens to fix the bug in 9272, but a larger value is probably
        // ok too.
        if scalar_abs(theta_arc) < (SCALAR_PI / (1000.0 * 1000.0)) {
            return self.line_to(end_pt);
        }

        point_transform.set_rotate(angle, None);
        point_transform.pre_scale((rx, ry), None);

        // the arc may be slightly bigger than 1/4 circle, so allow up to 1/3rd
        let segments = scalar_ceil_to_int(scalar_abs(theta_arc / (2.0 * SCALAR_PI / 3.0)));
        #[allow(clippy::cast_precision_loss)] // int to float, as in C++
        let theta_width = theta_arc / segments as scalar;
        let t = scalar_tan(0.5 * theta_width);
        if !is_finite(t) {
            return self;
        }
        let mut start_theta = theta1;
        let w = scalar_sqrt(SCALAR_HALF + scalar_cos(theta_width) * SCALAR_HALF);
        #[allow(clippy::float_cmp)] // exact comparison, as in C++
        let scalar_is_integer = |s: scalar| -> bool { s == scalar_floor_to_scalar(s) };
        let expect_integers = (SCALAR_PI / 2.0 - scalar_abs(theta_width)).nearly_zero(None)
            && scalar_is_integer(rx)
            && scalar_is_integer(ry)
            && scalar_is_integer(end_pt.x)
            && scalar_is_integer(end_pt.y);

        for _ in 0..segments {
            let end_theta = start_theta + theta_width;
            let sin_end_theta = scalar_sin_snap_to_zero(end_theta);
            let cos_end_theta = scalar_cos_snap_to_zero(end_theta);

            unit_pts[1].set(cos_end_theta, sin_end_theta);
            unit_pts[1] += center_point;
            unit_pts[0] = unit_pts[1];
            unit_pts[0].offset((t * sin_end_theta, -t * cos_end_theta));
            let mut mapped = [Point::default(); 2];
            point_transform.map_points(&mut mapped, &unit_pts);
            /*
            Computing the arc width introduces rounding errors that cause arcs to start
            outside their marks. A round rect may lose convexity as a result. If the input
            values are on integers, place the conic on integers as well.
             */
            if expect_integers {
                for point in &mut mapped {
                    point.x = scalar_round_to_scalar(point.x);
                    point.y = scalar_round_to_scalar(point.y);
                }
            }
            self.conic_to(mapped[0], mapped[1], w);
            start_theta = end_theta;
        }

        // The final point should match the input point (by definition); replace it to
        // ensure that rounding errors in the above math don't cause any problems.
        *self.pts.last_mut().expect("arc added points") = end_pt;
        self
    }

    /// Adds a line from `a` to `b` as a new contour.
    #[doc(alias = "addLine")]
    pub fn add_line(&mut self, a: impl Into<Point>, b: impl Into<Point>) -> &mut Self {
        self.move_to(a).line_to(b)
    }

    /// Iterates the verbs.
    // Port of: src/core/SkPathBuilder.cpp#L699-L701 (chrome/m156)
    #[must_use]
    #[allow(clippy::iter_without_into_iter)] // mirrors the C++ / skia-safe `iter()`
    pub fn iter(&self) -> PathIter<'_> {
        PathIter::new(&self.pts, &self.verbs, &self.conic_weights)
    }

    /// Appends the contents of `raw`.
    // Port of: src/core/SkPathBuilder.cpp#L703-L734 (chrome/m156)
    #[doc(alias = "addRaw")]
    pub fn add_raw(&mut self, raw: &PathRaw<'_>, reserve: Reserve) -> &mut Self {
        let (np, nv, nc) = (
            len_i32(raw.points()),
            len_i32(raw.verbs()),
            len_i32(raw.conics()),
        );
        if reserve == Reserve::Grow {
            self.inc_reserve(np, nv, nc);
        } else {
            let (lp, lv, lc) = (
                len_i32(&self.pts),
                len_i32(&self.verbs),
                len_i32(&self.conic_weights),
            );
            reserve_exact_total(&mut self.pts, sat_add(lp, np));
            reserve_exact_total(&mut self.verbs, sat_add(lv, nv));
            reserve_exact_total(&mut self.conic_weights, sat_add(lc, nc));
        }

        for rec in raw.iter() {
            let pts = rec.points();
            match rec.verb() {
                PathVerb::Move => self.move_to(pts[0]),
                PathVerb::Line => self.line_to(pts[1]),
                PathVerb::Quad => self.quad_to(pts[1], pts[2]),
                PathVerb::Conic => self.conic_to(pts[1], pts[2], rec.conic_weight()),
                PathVerb::Cubic => self.cubic_to(pts[1], pts[2], pts[3]),
                PathVerb::Close => self.close(),
            };
        }

        let has_trailing_move = |vbs: &[PathVerb]| vbs.last() == Some(&PathVerb::Move);

        // if the iterator 'trimmed' off a trialing move, we restore it here
        if has_trailing_move(raw.verbs()) && !has_trailing_move(self.verbs()) {
            self.move_to(raw.points()[raw.points().len() - 1]);
        }

        self
    }

    /// Adds a closed rectangle (start index 0 is the top-left corner).
    // Port of: src/core/SkPathBuilder.cpp#L742-L752 (chrome/m156)
    #[doc(alias = "addRect")]
    pub fn add_rect(
        &mut self,
        rect: impl AsRef<Rect>,
        dir: impl Into<Option<PathDirection>>,
        start_index: impl Into<Option<usize>>,
    ) -> &mut Self {
        let dir = dir.into().unwrap_or_default();
        #[allow(clippy::cast_possible_truncation)] // `unsigned` in C++
        let index = start_index.into().unwrap_or(0) as u32;
        let was_empty = is_empty_or_moves(&self.verbs);

        self.add_raw(
            &path_raw_shapes::Rect::new(rect.as_ref(), dir, index).raw(),
            Reserve::Grow,
        );

        if was_empty {
            // now we're a rect
            self.convexity = direction_to_convexity(dir);
        }
        self
    }

    /// Adds a closed oval (start index 1 is the right-middle point).
    // Port of: src/core/SkPathBuilder.cpp#L754-L767 (chrome/m156)
    #[doc(alias = "addOval")]
    pub fn add_oval(
        &mut self,
        oval: impl AsRef<Rect>,
        dir: impl Into<Option<PathDirection>>,
        start_index: impl Into<Option<usize>>,
    ) -> &mut Self {
        let dir = dir.into().unwrap_or_default();
        // m86: default start index changed from 0 to 1
        #[allow(clippy::cast_possible_truncation)] // `unsigned` in C++
        let index = start_index.into().unwrap_or(1) as u32;
        let was_empty = is_empty_or_moves(&self.verbs);

        self.add_raw(
            &path_raw_shapes::Oval::new(oval.as_ref(), dir, index).raw(),
            Reserve::Grow,
        );

        if was_empty {
            self.ty = PathIsAType::Oval;
            self.is_a.direction = dir;
            #[allow(clippy::cast_possible_truncation)] // < 4
            {
                self.is_a.start_index = (index % 4) as u8;
            }
            self.convexity = direction_to_convexity(dir);
        }

        self
    }

    /// Adds a closed round rect (start index 6 for CW, 7 for CCW by default).
    // Port of: src/core/SkPathBuilder.cpp#L769-L794 (chrome/m156)
    #[doc(alias = "addRRect")]
    pub fn add_rrect(
        &mut self,
        rrect: impl AsRef<RRect>,
        dir: impl Into<Option<PathDirection>>,
        start_index: impl Into<Option<usize>>,
    ) -> &mut Self {
        let rrect = rrect.as_ref();
        let dir = dir.into().unwrap_or_default();
        // m86: default start index changed from 0 to 6 or 7 depending on the path's direction.
        #[allow(clippy::cast_possible_truncation)] // `unsigned` in C++
        let index = start_index
            .into()
            .unwrap_or(if dir == PathDirection::CW { 6 } else { 7 }) as u32;
        let bounds = *rrect.rect();

        let (as_type, new_index) = path_priv::simplify_rrect(rrect, index);
        match as_type {
            RRectAsEnum::Rect => return self.add_rect(bounds, dir, new_index as usize),
            RRectAsEnum::Oval => return self.add_oval(bounds, dir, new_index as usize),
            RRectAsEnum::RRect => {} // fall through ...
        }

        let was_empty = is_empty_or_moves(&self.verbs);

        self.add_raw(
            &path_raw_shapes::RRect::new(rrect, dir, index).raw(),
            Reserve::Grow,
        );

        if was_empty {
            self.ty = PathIsAType::RRect;
            self.is_a.direction = dir;
            #[allow(clippy::cast_possible_truncation)] // < 8
            {
                self.is_a.start_index = (index % 8) as u8;
            }
            self.convexity = direction_to_convexity(dir);
        }
        self
    }

    /// Adds a circle (nothing if `radius` is negative).
    // Port of: src/core/SkPathBuilder.cpp#L796-L802 (chrome/m156)
    #[doc(alias = "addCircle")]
    pub fn add_circle(
        &mut self,
        center: impl Into<Point>,
        radius: scalar,
        dir: impl Into<Option<PathDirection>>,
    ) -> &mut Self {
        let center = center.into();
        let r = radius;
        if r >= 0.0 {
            self.add_oval(
                Rect::from_ltrb(center.x - r, center.y - r, center.x + r, center.y + r),
                dir,
                None,
            );
        }
        self
    }

    /// Adds a polygon through `pts`.
    // Port of: src/core/SkPathBuilder.cpp#L804-L815 (chrome/m156)
    #[doc(alias = "addPolygon")]
    pub fn add_polygon(&mut self, pts: &[Point], close: bool) -> &mut Self {
        if pts.is_empty() {
            return self;
        }

        self.move_to(pts[0]);
        self.polyline_to(&pts[1..]);
        if close {
            self.close();
        }
        self
    }

    /// Appends `path`.
    #[doc(alias = "addPath")]
    pub fn add_path(&mut self, path: &Path, mode: impl Into<Option<AddPathMode>>) -> &mut Self {
        self.add_path_with_transform(path, Matrix::i(), mode)
    }

    /// Appends `path` offset by `offset`.
    // Port of: src/core/SkPathBuilder.cpp#L839-L843 (chrome/m156)
    #[doc(alias = "addPath")]
    pub fn add_path_with_offset(
        &mut self,
        path: &Path,
        offset: impl Into<Vector>,
        mode: impl Into<Option<AddPathMode>>,
    ) -> &mut Self {
        let matrix = Matrix::translate(offset);
        self.add_path_with_transform(path, &matrix, mode)
    }

    /// Appends `src` transformed by `matrix`.
    // Port of: src/core/SkPathBuilder.cpp#L845-L930 (chrome/m156)
    #[doc(alias = "addPath")]
    pub fn add_path_with_transform(
        &mut self,
        src: &Path,
        matrix: &Matrix,
        mode: impl Into<Option<AddPathMode>>,
    ) -> &mut Self {
        let mode = mode.into().unwrap_or_default();
        if src.is_empty() {
            return self;
        }

        let can_replace_this =
            (mode == AddPathMode::Append && self.verbs().len() <= 1) || self.verbs().is_empty();
        if can_replace_this && matrix.is_identity() {
            let fill_type = self.fill_type;
            self.assign_path(src);
            self.fill_type = fill_type;
            return self;
        }

        // We're about to append - clear convexity.
        self.convexity = PathConvexity::Unknown;

        if AddPathMode::Append == mode && !matrix.has_perspective() {
            // If the current builder ends with a moveTo and src starts with one (which is always
            // true if non-empty), we must discard the builder moveTo in order to maintain
            // internal consistency after append (no repeating moveTos).
            if self.verbs.last() == Some(&PathVerb::Move) && !src.is_empty() {
                debug_assert_eq!(src.verbs()[0], PathVerb::Move);
                self.verbs.pop();
                self.pts.pop();
                debug_assert_ne!(self.verbs.last(), Some(&PathVerb::Move));
            }

            let last_move_to_index =
                path_priv::find_last_move_to_index(src.verbs(), src.points().len());
            debug_assert!(last_move_to_index >= 0);
            self.last_move_index = last_move_to_index + len_i32(&self.pts);

            // growForVerbsInPath
            self.segment_mask |= src.segment_masks().bits();
            if !src.verbs().is_empty() {
                self.ty = PathIsAType::General;
                self.verbs.extend_from_slice(src.verbs());
            }
            let start = self.pts.len();
            self.pts
                .resize(start + src.points().len(), Point::default());
            matrix.map_points(&mut self.pts[start..], src.points());
            self.conic_weights.extend_from_slice(src.conic_weights());
            return self;
        }

        let mut first_verb = true;
        for (verb, pts, w) in path_priv::iterate(src) {
            let mut mapped_pts = [Point::default(); 3];
            match verb {
                PathVerb::Move => {
                    matrix.map_points(&mut mapped_pts[..1], &pts[..1]);
                    if first_verb && mode == AddPathMode::Extend && !self.is_empty() {
                        self.ensure_move(); // In case last contour is closed
                        let last_pt = self.get_last_pt();
                        // don't add lineTo if it is degenerate
                        if last_pt.is_none_or(|p| p != mapped_pts[0]) {
                            self.line_to(mapped_pts[0]);
                        }
                    } else {
                        self.move_to(mapped_pts[0]);
                    }
                }
                PathVerb::Line => {
                    matrix.map_points(&mut mapped_pts[..1], &pts[1..2]);
                    self.line_to(mapped_pts[0]);
                }
                PathVerb::Quad => {
                    matrix.map_points(&mut mapped_pts[..2], &pts[1..3]);
                    self.quad_to(mapped_pts[0], mapped_pts[1]);
                }
                PathVerb::Conic => {
                    matrix.map_points(&mut mapped_pts[..2], &pts[1..3]);
                    self.conic_to(mapped_pts[0], mapped_pts[1], w.unwrap_or(1.0));
                }
                PathVerb::Cubic => {
                    matrix.map_points(&mut mapped_pts[..3], &pts[1..4]);
                    self.cubic_to(mapped_pts[0], mapped_pts[1], mapped_pts[2]);
                }
                PathVerb::Close => {
                    self.close();
                }
            }
            first_verb = false;
        }
        self
    }

    // ignore the last point of the 1st contour
    // Port of: src/core/SkPathBuilder.cpp#L933-L968 (chrome/m156)
    pub(crate) fn private_reverse_path_to(&mut self, path: &Path) -> &mut Self {
        let verbs = path.verbs();
        if verbs.is_empty() {
            return self;
        }

        let all_pts = path.points();
        let conics = path.conic_weights();
        let mut v = verbs.len();
        // pts points at the last point (C++: path.points().end() - 1)
        let mut pts = all_pts.len() - 1;
        let mut cw = conics.len();

        while v > 0 {
            v -= 1;
            let verb = verbs[v];
            // C++ pointer arithmetic: may step one before the first point on the final move.
            pts = pts.wrapping_sub(path_priv::pts_in_verb_usize(verb));
            match verb {
                PathVerb::Move => {
                    // if the path has multiple contours, stop after reversing the last
                    return self;
                }
                PathVerb::Line => {
                    self.line_to(all_pts[pts]);
                }
                PathVerb::Quad => {
                    self.quad_to(all_pts[pts + 1], all_pts[pts]);
                }
                PathVerb::Conic => {
                    cw -= 1;
                    self.conic_to(all_pts[pts + 1], all_pts[pts], conics[cw]);
                }
                PathVerb::Cubic => {
                    self.cubic_to(all_pts[pts + 2], all_pts[pts + 1], all_pts[pts]);
                }
                PathVerb::Close => {}
            }
        }
        self
    }

    // Port of: src/core/SkPathBuilder.cpp#L970-L1020 (chrome/m156)
    pub(crate) fn private_reverse_add_path(&mut self, src: &Path) -> &mut Self {
        let verbs = src.verbs();
        if verbs.is_empty() {
            return self;
        }

        let all_pts = src.points();
        let conics = src.conic_weights();
        let mut v = verbs.len();
        let mut pts = all_pts.len(); // C++: src.points().end()
        let mut cw = conics.len();

        let mut need_move = true;
        let mut need_close = false;
        while v > 0 {
            v -= 1;
            let verb = verbs[v];
            let n = path_priv::pts_in_verb_usize(verb);

            // C++ pointer arithmetic: `pts` may step one before the first point (and back).
            if need_move {
                pts = pts.wrapping_sub(1);
                self.move_to(all_pts[pts]);
                need_move = false;
            }
            pts = pts.wrapping_sub(n);
            match verb {
                PathVerb::Move => {
                    if need_close {
                        self.close();
                        need_close = false;
                    }
                    need_move = true;
                    pts = pts.wrapping_add(1); // so we see the point in "if (needMove)" above
                }
                PathVerb::Line => {
                    self.line_to(all_pts[pts]);
                }
                PathVerb::Quad => {
                    self.quad_to(all_pts[pts + 1], all_pts[pts]);
                }
                PathVerb::Conic => {
                    cw -= 1;
                    self.conic_to(all_pts[pts + 1], all_pts[pts], conics[cw]);
                }
                PathVerb::Cubic => {
                    self.cubic_to(all_pts[pts + 2], all_pts[pts + 1], all_pts[pts]);
                }
                PathVerb::Close => {
                    need_close = true;
                }
            }
        }
        self
    }

    /// Reserves room for more points, verbs and conic weights (negative counts are ignored).
    // Port of: src/core/SkPathBuilder.cpp#L121-L125 (chrome/m156)
    #[doc(alias = "incReserve")]
    pub fn inc_reserve(&mut self, extra_pt_count: i32, extra_vb_count: i32, extra_cn_count: i32) {
        let (lp, lv, lc) = (
            len_i32(&self.pts),
            len_i32(&self.verbs),
            len_i32(&self.conic_weights),
        );
        reserve_total(&mut self.pts, sat_add(lp, extra_pt_count));
        reserve_total(&mut self.verbs, sat_add(lv, extra_vb_count));
        reserve_total(&mut self.conic_weights, sat_add(lc, extra_cn_count));
    }

    /// Offsets every point by `d`.
    // Port of: src/core/SkPathBuilder.cpp#L832-L837 (chrome/m156)
    pub fn offset(&mut self, d: impl Into<Vector>) -> &mut Self {
        let d = d.into();
        for p in &mut self.pts {
            *p += Vector::new(d.x, d.y);
        }
        self
    }

    /// Transforms the contents by `matrix` (perspective clips, promotes quads to conics and
    /// subdivides cubics).
    // Port of: src/core/SkPathBuilder.cpp#L1048-L1111 (chrome/m156)
    pub fn transform(&mut self, matrix: &Matrix) -> &mut Self {
        if matrix.is_identity() || self.is_empty() {
            return self;
        }

        if matrix.has_perspective() {
            let mut src = self.detach();

            // remember this from before the detach()
            self.set_fill_type(src.fill_type());

            if let Some(clipped) = path_priv::perspective_clip(&src, matrix) {
                src = clipped;
            }

            for (verb, pts, wt) in path_priv::iterate(&src) {
                match verb {
                    PathVerb::Move => {
                        self.move_to(pts[0]);
                    }
                    PathVerb::Line => {
                        self.line_to(pts[1]);
                    }
                    PathVerb::Quad => {
                        // promote the quad to a conic
                        self.conic_to(pts[1], pts[2], Conic::transform_w(pts, 1.0, matrix));
                    }
                    PathVerb::Conic => {
                        self.conic_to(
                            pts[1],
                            pts[2],
                            Conic::transform_w(pts, wt.unwrap_or(1.0), matrix),
                        );
                    }
                    PathVerb::Cubic => {
                        subdivide_cubic_to(self, pts, 2);
                    }
                    PathVerb::Close => {
                        self.close();
                    }
                }
            }
        } else {
            // Can we maintain our special case shape?
            if !matrix.rect_stays_rect() || !path_priv::is_axis_aligned(&self.pts) {
                self.ty = PathIsAType::General;
                // lose convexity (just to be numerically safe)
                if self.convexity.is_convex() {
                    self.convexity = PathConvexity::Unknown;
                }
            }

            // If we're still a special case, check if we need to reverse our winding
            if self.ty == PathIsAType::Oval || self.ty == PathIsAType::RRect {
                let (dir, start) = path_priv::transform_dir_and_start(
                    matrix,
                    self.ty == PathIsAType::RRect,
                    self.is_a.direction,
                    u32::from(self.is_a.start_index),
                );
                self.is_a.direction = dir;
                #[allow(clippy::cast_possible_truncation)] // start < 8
                {
                    self.is_a.start_index = start as u8;
                }
            }
        }
        matrix.map_points_inplace(&mut self.pts);

        self
    }

    /// True if the builder is empty, or all of its points are finite.
    // Port of: src/core/SkPathBuilder.cpp#L1124-L1131 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.pts.iter().all(|p| p.is_finite())
    }

    /// Toggles the fill type between inverse and non-inverse.
    #[doc(alias = "toggleInverseFillType")]
    pub fn toggle_inverse_fill_type(&mut self) -> &mut Self {
        self.fill_type = self.fill_type.toggle_inverse();
        self
    }

    /// True if there are no verbs.
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// The last point, if any.
    // Port of: src/core/SkPathBuilder.cpp#L1022-L1028 (chrome/m156)
    #[doc(alias = "getLastPt")]
    #[must_use]
    pub fn get_last_pt(&self) -> Option<Point> {
        self.pts.last().copied()
    }

    /// Replaces the point at `index` (ignored if out of range).
    // Port of: src/core/SkPathBuilder.cpp#L1041-L1046 (chrome/m156)
    #[doc(alias = "setPoint")]
    pub fn set_point(&mut self, index: usize, p: impl Into<Point>) {
        if index < self.pts.len() {
            self.pts[index] = p.into();
            self.ty = PathIsAType::General;
        }
    }

    /// Replaces the last point.
    // Port of: include/core/SkPathBuilder.h#L924-L926 (chrome/m156)
    #[doc(alias = "setLastPoint")]
    pub fn set_last_point(&mut self, p: impl Into<Point>) {
        self.set_point(self.pts.len().wrapping_sub(1), p);
    }

    /// Replaces the last point, or moves to `p` if the builder is empty.
    // Port of: src/core/SkPathBuilder.cpp#L1031-L1038 (chrome/m156)
    #[doc(alias = "setLastPt")]
    pub fn set_last_pt(&mut self, p: impl Into<Point>) {
        let p = p.into();
        let count = self.pts.len();
        if count == 0 {
            self.move_to(p);
        } else {
            self.set_point(count - 1, p);
        }
    }

    /// The number of points.
    #[doc(alias = "countPoints")]
    #[must_use]
    pub fn count_points(&self) -> usize {
        self.pts.len()
    }

    /// True if the fill type is inverse.
    #[doc(alias = "isInverseFillType")]
    #[must_use]
    pub fn is_inverse_fill_type(&self) -> bool {
        self.fill_type.is_inverse()
    }

    /// The points.
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.pts
    }

    /// The verbs.
    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        &self.verbs
    }

    /// The conic weights.
    #[doc(alias = "conicWeights")]
    #[must_use]
    pub fn conic_weights(&self) -> &[scalar] {
        &self.conic_weights
    }

    /// True if `p` is inside the path (taking the fill type into account).
    // Port of: src/core/SkPathBuilder.cpp#L1148-L1151 (chrome/m156)
    #[must_use]
    pub fn contains(&self, p: impl Into<Point>) -> bool {
        let p = p.into();
        let raw = path_priv::raw_builder(self, ResolveConvexity::No);
        raw.is_some_and(|raw| path_priv::contains(&raw, p))
    }

    // Port of: src/core/SkPathBuilder.cpp#L1133-L1146 (chrome/m156)
    #[allow(dead_code)] // private helper kept for parity with C++
    fn is_zero_length_since_point(&self, start_pt_index: usize) -> bool {
        let count = self.pts.len().saturating_sub(start_pt_index);
        if count < 2 {
            return true;
        }
        let pts = &self.pts[start_pt_index..];
        let first = pts[0];
        for p in &pts[1..count] {
            if first != *p {
                return false;
            }
        }
        true
    }
}

// skvx::double2 helpers used by arc_to_tangent (SkVx.h dot / cross / normalize / isfinite).
fn dot_d2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [a[0] * b[0], a[1] * b[1]];
    ab[0] + ab[1]
}

fn cross_d2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let x = [a[0] * b[1], a[1] * b[0]];
    x[0] - x[1]
}

fn normalize_d2(v: [f64; 2]) -> [f64; 2] {
    let len = dot_d2(v, v).sqrt();
    [v[0] / len, v[1] / len]
}

fn isfinite_d2(v: [f64; 2]) -> bool {
    v[0].is_finite() && v[1].is_finite()
}
