// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkEdgeBuilder.h, src/core/SkEdgeBuilder.cpp

//! Builds the edges of a path for the scan converters (`SkEdgeBuilder.h`).
//!
//! skia-rust: the C++ `SkEdgeBuilder` base class with virtual `addLine`/`addQuad`/`addCubic`
//! becomes the [`EdgeBuilder`] trait with the shared `build*` code as provided methods.
//! `SkBasicEdgeBuilder` is [`BasicEdgeBuilder`] and `SkAnalyticEdgeBuilder` is
//! [`AnalyticEdgeBuilder`]. The `addPolyLine`/`allocEdges` hooks are not ported: m156's
//! `buildPoly` no longer calls them.

use skia_rust_core::edge_clipper::EdgeClipper;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::geometry::{AutoConicToQuads, chop_cubic_at_y_extrema, chop_quad_at_y_extrema};
use skia_rust_core::line_clipper::{MAX_CLIPPED_LINE_SEGMENTS, MAX_POINTS, clip_line};
use skia_rust_core::path::Path;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv::{self, Edge as PathEdge, PathEdgeIter};
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::PathSegmentMask;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::safe_math::SafeMath;

use crate::analytic_edge::{
    AnalyticCubicEdge, AnalyticEdge, AnalyticQuadraticEdge, AnyAnalyticEdge,
};
use crate::edge::{AnyEdge, CubicEdge, Edge, EdgeType, QuadraticEdge};
use skia_rust_core::fixed::Fixed;
use skia_rust_core::safe32::abs32;

/// The result of trying to merge a new vertical edge into the previous one.
// Port of: src/core/SkEdgeBuilder.h#L38-L42 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Combine {
    No,
    Partial,
    Total,
}

/// Builds edges from a path (`SkEdgeBuilder`). Implementors collect the edges that
/// `add_line`/`add_quad`/`add_cubic` create.
// Port of: src/core/SkEdgeBuilder.h#L23-L55 (chrome/m156)
#[doc(alias = "SkEdgeBuilder")]
pub trait EdgeBuilder {
    /// `recoverClip`: the clip as floats, in the coordinate space the edges are built in.
    fn recover_clip(&self, src: &IRect) -> Rect;

    /// `addLine(pts)`.
    fn add_line(&mut self, pts: &[Point]);
    /// `addQuad(pts)`.
    fn add_quad(&mut self, pts: &[Point]);
    /// `addCubic(pts)`.
    fn add_cubic(&mut self, pts: &[Point]);

    /// The number of edges collected so far (`fList.size()`).
    fn edge_count(&self) -> usize;

    /// Builds the edges of `raw`, clipped to `shifted_clip` if given. Returns the number of
    /// edges.
    // Port of: src/core/SkEdgeBuilder.cpp#L348-L366 (chrome/m156)
    #[doc(alias = "buildEdges")]
    fn build_edges(&mut self, raw: &PathRaw<'_>, shifted_clip: Option<&IRect>) -> usize
    where
        Self: Sized,
    {
        // If we're convex, then we need both edges, even if the right edge is past the clip.
        let can_cull_to_the_right = !raw.is_known_to_be_convex();

        // We can use our buildPoly() optimization if all the segments are lines.
        // (Edges are homogeneous and stored contiguously in memory, no need for indirection.)
        let count = if PathSegmentMask::LINE.bits() == raw.segment_masks() {
            build_poly(self, raw, shifted_clip, can_cull_to_the_right)
        } else {
            build(self, raw, shifted_clip, can_cull_to_the_right)
        };

        // If we can't cull to the right, we should have count > 1 (or 0).
        if !can_cull_to_the_right {
            debug_assert!(count != 1);
        }
        count
    }

    /// [`build_edges`](Self::build_edges) for a [`Path`]; returns 0 if the path has no usable
    /// raw view (e.g. non-finite).
    // Port of: src/core/SkEdgeBuilder.cpp#L368-L373 (chrome/m156)
    #[doc(alias = "buildEdges")]
    fn build_edges_path(&mut self, path: &Path, shifted_clip: Option<&IRect>) -> usize
    where
        Self: Sized,
    {
        if let Some(raw) = path_priv::raw(path, ResolveConvexity::Yes) {
            return self.build_edges(&raw, shifted_clip);
        }
        0 // no edges were built
    }
}

// TODO(C++): maybe get rid of buildPoly() entirely?
// Port of: src/core/SkEdgeBuilder.cpp#L222-L270 (chrome/m156)
fn build_poly<B: EdgeBuilder>(
    this: &mut B,
    raw: &PathRaw<'_>,
    iclip: Option<&IRect>,
    can_cull_to_the_right: bool,
) -> usize {
    if iclip.is_some() {
        // clipping can turn 1 line into (up to) kMaxClippedLineSegments, since
        // we turn portions that are clipped out on the left/right into vertical
        // segments.
        let mut safe = SafeMath::new();
        let _max_edge_count = safe.mul(raw.points.len(), MAX_CLIPPED_LINE_SEGMENTS);
        if !safe.ok() {
            return 0;
        }
    }

    let mut iter = PathEdgeIter::new(raw);
    if let Some(iclip) = iclip {
        let clip = this.recover_clip(iclip);

        while let Some(e) = iter.next() {
            match e.edge {
                PathEdge::Line => {
                    let mut lines = [Point::default(); MAX_POINTS];
                    let line_count = clip_line(
                        &[e.pts[0], e.pts[1]],
                        &clip,
                        &mut lines,
                        can_cull_to_the_right,
                    );
                    debug_assert!(line_count <= MAX_CLIPPED_LINE_SEGMENTS);
                    for i in 0..line_count {
                        this.add_line(&lines[i..]);
                    }
                }
                _ => debug_assert!(false, "unexpected verb"),
            }
        }
    } else {
        while let Some(e) = iter.next() {
            match e.edge {
                PathEdge::Line => this.add_line(&e.pts),
                _ => debug_assert!(false, "unexpected verb"),
            }
        }
    }
    this.edge_count()
}

// Port of: src/core/SkEdgeBuilder.cpp#L308-L313 (chrome/m156)
fn handle_quad<B: EdgeBuilder>(this: &mut B, pts: &[Point]) {
    let mut mono_x = [Point::default(); 5];
    let n = chop_quad_at_y_extrema(pts, &mut mono_x);
    for i in 0..=n {
        this.add_quad(&mono_x[i * 2..]);
    }
}

// Port of: src/core/SkEdgeBuilder.cpp#L272-L346 (chrome/m156)
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn build<B: EdgeBuilder>(
    this: &mut B,
    raw: &PathRaw<'_>,
    iclip: Option<&IRect>,
    can_cull_to_the_right: bool,
) -> usize {
    if let Some(iclip) = iclip {
        let clip = this.recover_clip(iclip);
        let mut is_finite_flag = true;

        EdgeClipper::clip_path(raw, &clip, can_cull_to_the_right, |clipper, _| {
            let mut pts = [Point::default(); 4];

            while let Some(verb) = clipper.next(&mut pts) {
                #[allow(clippy::cast_sign_loss)] // PtsInIter is positive for edge verbs
                let count = path_priv::pts_in_iter(verb) as usize;
                if !pts[..count]
                    .iter()
                    .all(|p| is_finite(p.x) && is_finite(p.y))
                {
                    is_finite_flag = false;
                    return;
                }
                match verb {
                    skia_rust_core::path_types::PathVerb::Line => this.add_line(&pts),
                    skia_rust_core::path_types::PathVerb::Quad => this.add_quad(&pts),
                    skia_rust_core::path_types::PathVerb::Cubic => this.add_cubic(&pts),
                    _ => {}
                }
            }
        });
        return if is_finite_flag { this.edge_count() } else { 0 };
    }

    let mut iter = PathEdgeIter::new(raw);
    let mut quadder = AutoConicToQuads::new();
    const CONIC_TOL: f32 = 0.25;
    let mut mono_y = [Point::default(); 10];

    while let Some(e) = iter.next() {
        match e.edge {
            PathEdge::Line => this.add_line(&e.pts),
            PathEdge::Quad => handle_quad(this, &e.pts),
            PathEdge::Conic => {
                let quad_pts = quadder
                    .compute_quads_with_weight(&e.pts, iter.conic_weight(), CONIC_TOL)
                    .to_vec();
                for i in 0..quadder.count_quads() {
                    handle_quad(this, &quad_pts[i * 2..]);
                }
            }
            PathEdge::Cubic => {
                let n = chop_cubic_at_y_extrema(&e.pts, Some(&mut mono_y));
                for i in 0..=n {
                    this.add_cubic(&mono_y[i * 3..]);
                }
            }
        }
    }
    this.edge_count()
}

// We only consider edges that were originally lines to be vertical to avoid numerical issues
// (crbug.com/1154864).
// Port of: src/core/SkEdgeBuilder.cpp#L122-L127 (chrome/m156)
fn is_vertical(edge: &Edge) -> bool {
    edge.dx_dy == 0 && edge.edge_type == EdgeType::Line
}

// Port of: src/core/SkEdgeBuilder.cpp#L27-L68 (chrome/m156)
fn combine_vertical(edge: &Edge, last: &mut Edge) -> Combine {
    // We only consider edges that were originally lines to be vertical to avoid numerical issues
    // (crbug.com/1154864).
    if last.edge_type != EdgeType::Line || last.dx_dy != 0 || edge.x != last.x {
        return Combine::No;
    }
    if edge.winding == last.winding {
        if edge.last_y.wrapping_add(1) == last.first_y {
            last.first_y = edge.first_y;
            return Combine::Partial;
        }
        if edge.first_y == last.last_y.wrapping_add(1) {
            last.last_y = edge.last_y;
            return Combine::Partial;
        }
        return Combine::No;
    }
    if edge.first_y == last.first_y {
        if edge.last_y == last.last_y {
            return Combine::Total;
        }
        if edge.last_y < last.last_y {
            last.first_y = edge.last_y.wrapping_add(1);
            return Combine::Partial;
        }
        last.first_y = last.last_y.wrapping_add(1);
        last.last_y = edge.last_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    if edge.last_y == last.last_y {
        if edge.first_y > last.first_y {
            last.last_y = edge.first_y.wrapping_sub(1);
            return Combine::Partial;
        }
        last.last_y = last.first_y.wrapping_sub(1);
        last.first_y = edge.first_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    Combine::No
}

/// The edge builder of the non-antialiased scan converter (`SkBasicEdgeBuilder`).
// Port of: src/core/SkEdgeBuilder.h#L57-L80 (chrome/m156)
#[doc(alias = "SkBasicEdgeBuilder")]
#[derive(Clone, Debug, Default)]
pub struct BasicEdgeBuilder {
    list: Vec<AnyEdge>,
}

impl BasicEdgeBuilder {
    /// `SkBasicEdgeBuilder()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The built edges (`edgeList()`).
    #[doc(alias = "edgeList")]
    #[must_use]
    pub fn edge_list(&mut self) -> &mut [AnyEdge] {
        &mut self.list
    }

    /// Takes the built edges.
    #[must_use]
    pub fn into_edges(self) -> Vec<AnyEdge> {
        self.list
    }
}

impl EdgeBuilder for BasicEdgeBuilder {
    // Port of: src/core/SkEdgeBuilder.cpp#L209-L211 (chrome/m156)
    fn recover_clip(&self, src: &IRect) -> Rect {
        Rect::from(*src)
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L139-L152 (chrome/m156)
    fn add_line(&mut self, pts: &[Point]) {
        let mut edge = Edge::default();
        if edge.set_line(pts[0], pts[1]) {
            let combine = if is_vertical(&edge) && !self.list.is_empty() {
                let last = self.list.last_mut().expect("list is not empty");
                combine_vertical(&edge, last)
            } else {
                Combine::No
            };

            match combine {
                Combine::Total => {
                    self.list.pop();
                }
                Combine::Partial => {}
                Combine::No => self.list.push(AnyEdge::Line(edge)),
            }
        }
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L168-L173 (chrome/m156)
    fn add_quad(&mut self, pts: &[Point]) {
        let mut edge = QuadraticEdge::default();
        if edge.set_quadratic(pts) {
            self.list.push(AnyEdge::Quad(edge));
        }
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L181-L186 (chrome/m156)
    fn add_cubic(&mut self, pts: &[Point]) {
        let mut edge = CubicEdge::default();
        if edge.set_cubic(pts) {
            self.list.push(AnyEdge::Cubic(edge));
        }
    }

    fn edge_count(&self) -> usize {
        self.list.len()
    }
}

// Port of: src/core/SkEdgeBuilder.cpp#L70-L120 (chrome/m156)
fn combine_vertical_analytic(edge: &AnalyticEdge, last: &mut AnalyticEdge) -> Combine {
    let approximately_equal = |a: Fixed, b: Fixed| abs32(a.wrapping_sub(b)) < 0x100;

    // We only consider edges that were originally lines to be vertical to avoid numerical issues
    // (crbug.com/1154864).
    if last.edge_type != EdgeType::Line || last.dx != 0 || edge.x != last.x {
        return Combine::No;
    }
    if edge.winding == last.winding {
        if edge.lower_y == last.upper_y {
            last.upper_y = edge.upper_y;
            last.y = last.upper_y;
            return Combine::Partial;
        }
        if approximately_equal(edge.upper_y, last.lower_y) {
            last.lower_y = edge.lower_y;
            return Combine::Partial;
        }
        return Combine::No;
    }
    if approximately_equal(edge.upper_y, last.upper_y) {
        if approximately_equal(edge.lower_y, last.lower_y) {
            return Combine::Total;
        }
        if edge.lower_y < last.lower_y {
            last.upper_y = edge.lower_y;
            last.y = last.upper_y;
            return Combine::Partial;
        }
        last.upper_y = last.lower_y;
        last.y = last.upper_y;
        last.lower_y = edge.lower_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    if approximately_equal(edge.lower_y, last.lower_y) {
        if edge.upper_y > last.upper_y {
            last.lower_y = edge.upper_y;
            return Combine::Partial;
        }
        last.lower_y = last.upper_y;
        last.upper_y = edge.upper_y;
        last.y = last.upper_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    Combine::No
}

// We only consider edges that were originally lines to be vertical to avoid numerical issues
// (crbug.com/1154864).
// Port of: src/core/SkEdgeBuilder.cpp#L129-L134 (chrome/m156)
fn is_vertical_analytic(edge: &AnalyticEdge) -> bool {
    edge.dx == 0 && edge.edge_type == EdgeType::Line
}

/// The edge builder of the analytic antialiasing scan converter (`SkAnalyticEdgeBuilder`).
// Port of: src/core/SkEdgeBuilder.h#L82-L98 (chrome/m156)
#[doc(alias = "SkAnalyticEdgeBuilder")]
#[derive(Clone, Debug, Default)]
pub struct AnalyticEdgeBuilder {
    list: Vec<AnyAnalyticEdge>,
}

impl AnalyticEdgeBuilder {
    /// `SkAnalyticEdgeBuilder()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The built edges (`analyticEdgeList()`).
    #[doc(alias = "analyticEdgeList")]
    #[must_use]
    pub fn analytic_edge_list(&mut self) -> &mut [AnyAnalyticEdge] {
        &mut self.list
    }

    /// Takes the built edges.
    #[must_use]
    pub fn into_edges(self) -> Vec<AnyAnalyticEdge> {
        self.list
    }
}

impl EdgeBuilder for AnalyticEdgeBuilder {
    // Port of: src/core/SkEdgeBuilder.cpp#L212-L214 (chrome/m156)
    fn recover_clip(&self, src: &IRect) -> Rect {
        Rect::from(*src)
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L153-L167 (chrome/m156)
    fn add_line(&mut self, pts: &[Point]) {
        let mut edge = AnalyticEdge::default();
        if edge.set_line(pts[0], pts[1]) {
            let combine = if is_vertical_analytic(&edge) && !self.list.is_empty() {
                let last = self.list.last_mut().expect("list is not empty");
                combine_vertical_analytic(&edge, last)
            } else {
                Combine::No
            };

            match combine {
                Combine::Total => {
                    self.list.pop();
                }
                Combine::Partial => {}
                Combine::No => self.list.push(AnyAnalyticEdge::Line(edge)),
            }
        }
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L174-L179 (chrome/m156)
    fn add_quad(&mut self, pts: &[Point]) {
        let mut edge = AnalyticQuadraticEdge::default();
        if edge.set_quadratic(pts) {
            self.list.push(AnyAnalyticEdge::Quad(edge));
        }
    }

    // Port of: src/core/SkEdgeBuilder.cpp#L187-L192 (chrome/m156)
    fn add_cubic(&mut self, pts: &[Point]) {
        let mut edge = AnalyticCubicEdge::default();
        if edge.set_cubic(pts) {
            self.list.push(AnyAnalyticEdge::Cubic(edge));
        }
    }

    fn edge_count(&self) -> usize {
        self.list.len()
    }
}
