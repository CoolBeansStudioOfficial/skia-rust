// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkAddIntersections.cpp, src/pathops/SkIntersectionHelper.h,
// and the SkIntersections wrappers of src/pathops/SkIntersections.h (chrome/m156)

//! Finds where the segments of two contours intersect, and records the intersections in the op
//! graph (`AddIntersectTs`).

use skia_rust_core::point::Point;

use crate::intersections::Intersections;
use crate::line::DLine;
use crate::op_state::CoinSetId;
use crate::op_curve::{conic_from, cubic_from, quad_from};
use crate::op_state::{ContourId, OpState, PtTId, SegId};
use crate::point::DPoint;
use crate::rect::Bounds;
use crate::types::almost_less_ulps;

/// `SkIntersectionHelper::SegmentType`: the segment's kind. Horizontal and vertical lines are
/// lines whose bounds are degenerate in one axis.
// Port of: src/pathops/SkIntersectionHelper.h#L20-L27 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SegmentType {
    HorizontalLine,
    VerticalLine,
    Line,
    Quad,
    Conic,
    Cubic,
}

/// `SkIntersectionHelper`: walks the segments of one contour.
// Port of: src/pathops/SkIntersectionHelper.h#L18-L98 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct IntersectionHelper {
    /// `SkOpSegment* fSegment`: `None` once the walk has run past the last segment.
    segment: Option<SegId>,
}

impl IntersectionHelper {
    /// `init(contour)`: starts at the contour's first segment.
    fn init(state: &OpState, contour: ContourId) -> Self {
        Self {
            segment: state.contour_first(contour),
        }
    }

    /// `advance()`: moves to the next segment; returns false past the last one.
    fn advance(&mut self, state: &OpState) -> bool {
        self.segment = self.segment.and_then(|s| state.seg_next(s));
        self.segment.is_some()
    }

    /// `startAfter(after)`: starts at the segment after `after`'s.
    fn start_after(&mut self, state: &OpState, after: IntersectionHelper) -> bool {
        self.segment = after.segment.and_then(|s| state.seg_next(s));
        self.segment.is_some()
    }

    fn seg(&self) -> SegId {
        self.segment.expect("the helper walks a segment")
    }

    /// `bounds()`.
    fn bounds(&self, state: &OpState) -> Bounds {
        state.seg_bounds(self.seg())
    }

    fn left(&self, state: &OpState) -> f32 {
        self.bounds(state).left
    }

    fn right(&self, state: &OpState) -> f32 {
        self.bounds(state).right
    }

    fn top(&self, state: &OpState) -> f32 {
        self.bounds(state).top
    }

    fn bottom(&self, state: &OpState) -> f32 {
        self.bounds(state).bottom
    }

    /// `x()`: the left edge of the bounds.
    fn x(&self, state: &OpState) -> f32 {
        self.bounds(state).left
    }

    /// `y()`: the top edge of the bounds.
    fn y(&self, state: &OpState) -> f32 {
        self.bounds(state).top
    }

    /// `xFlipped()`: the curve starts away from its bounds' left edge.
    fn x_flipped(&self, state: &OpState) -> bool {
        self.x(state) != state.seg_pts(self.seg())[0].x
    }

    /// `yFlipped()`.
    fn y_flipped(&self, state: &OpState) -> bool {
        self.y(state) != state.seg_pts(self.seg())[0].y
    }

    fn pts(&self, state: &OpState) -> [Point; 4] {
        state.seg_pts(self.seg())
    }

    fn weight(&self, state: &OpState) -> f32 {
        state.seg_weight(self.seg())
    }

    /// `segmentType()`: a line is horizontal or vertical when its bounds are flat.
    // Port of: src/pathops/SkIntersectionHelper.h#L34-L45 (chrome/m156)
    fn segment_type(&self, state: &OpState) -> SegmentType {
        match state.seg_verb(self.seg()) {
            skia_rust_core::path::Verb::Line => {
                if state.seg_is_horizontal(self.seg()) {
                    SegmentType::HorizontalLine
                } else if state.seg_is_vertical_bounds(self.seg()) {
                    SegmentType::VerticalLine
                } else {
                    SegmentType::Line
                }
            }
            skia_rust_core::path::Verb::Quad => SegmentType::Quad,
            skia_rust_core::path::Verb::Conic => SegmentType::Conic,
            _ => SegmentType::Cubic,
        }
    }
}

/// `SkIntersections::lineHorizontal(a, left, right, y, flipped)`: the line `a`, given as its
/// first two points.
// Port of: src/pathops/SkIntersections.h#L145-L151 (chrome/m156)
fn line_horizontal(i: &mut Intersections, a: [Point; 4], left: f32, right: f32, y: f32, flipped: bool) -> usize {
    let line = line_from(a);
    i.set_max(2);
    i.horizontal_line(&line, f64::from(left), f64::from(right), f64::from(y), flipped)
}

/// `SkIntersections::lineVertical`.
// Port of: src/pathops/SkIntersections.h#L153-L158 (chrome/m156)
fn line_vertical(i: &mut Intersections, a: [Point; 4], top: f32, bottom: f32, x: f32, flipped: bool) -> usize {
    let line = line_from(a);
    i.set_max(2);
    i.vertical_line(&line, f64::from(top), f64::from(bottom), f64::from(x), flipped)
}

/// `SkIntersections::lineLine(a, b)`.
// Port of: src/pathops/SkIntersections.h#L159-L166 (chrome/m156)
fn line_line(i: &mut Intersections, a: [Point; 4], b: [Point; 4]) -> usize {
    i.set_max(2);
    i.intersect_line_line(&line_from(a), &line_from(b))
}

/// `SkIntersections::quadHorizontal(a, left, right, y, flipped)`.
// Port of: src/pathops/SkIntersections.h#L181-L188 (chrome/m156)
fn quad_horizontal(i: &mut Intersections, a: [Point; 4], left: f32, right: f32, y: f32, flipped: bool) -> usize {
    let quad = quad_from(&a);
    i.set_max(2);
    i.horizontal_quad(&quad, f64::from(left), f64::from(right), f64::from(y), flipped)
}

/// `SkIntersections::quadVertical`.
// Port of: src/pathops/SkIntersections.h#L189-L195 (chrome/m156)
fn quad_vertical(i: &mut Intersections, a: [Point; 4], top: f32, bottom: f32, x: f32, flipped: bool) -> usize {
    let quad = quad_from(&a);
    i.set_max(2);
    i.vertical_quad(&quad, f64::from(top), f64::from(bottom), f64::from(x), flipped)
}

/// `SkIntersections::quadLine(a, b)`: no `fMax` change.
// Port of: src/pathops/SkIntersections.h#L196-L203 (chrome/m156)
fn quad_line(i: &mut Intersections, a: [Point; 4], b: [Point; 4]) -> usize {
    i.intersect_quad_line(&quad_from(&a), &line_from(b))
}

/// `SkIntersections::conicHorizontal(a, weight, left, right, y, flipped)`.
// Port of: src/pathops/SkIntersections.h#L70-L76 (chrome/m156)
fn conic_horizontal(
    i: &mut Intersections,
    a: [Point; 4],
    weight: f32,
    left: f32,
    right: f32,
    y: f32,
    flipped: bool,
) -> usize {
    let conic = conic_from(&a[..3], weight);
    i.set_max(2);
    i.horizontal_conic(&conic, f64::from(left), f64::from(right), f64::from(y), flipped)
}

/// `SkIntersections::conicVertical`.
// Port of: src/pathops/SkIntersections.h#L78-L84 (chrome/m156)
fn conic_vertical(
    i: &mut Intersections,
    a: [Point; 4],
    weight: f32,
    top: f32,
    bottom: f32,
    x: f32,
    flipped: bool,
) -> usize {
    let conic = conic_from(&a[..3], weight);
    i.set_max(2);
    i.vertical_conic(&conic, f64::from(top), f64::from(bottom), f64::from(x), flipped)
}

/// `SkIntersections::conicLine(a, weight, b)`.
// Port of: src/pathops/SkIntersections.h#L86-L93 (chrome/m156)
fn conic_line(i: &mut Intersections, a: [Point; 4], weight: f32, b: [Point; 4]) -> usize {
    let conic = conic_from(&a[..3], weight);
    i.set_max(3);
    i.intersect_conic_line(&conic, &line_from(b))
}

/// `SkIntersections::cubicHorizontal(a, left, right, y, flipped)`.
// Port of: src/pathops/SkIntersections.h#L95-L101 (chrome/m156)
fn cubic_horizontal(i: &mut Intersections, a: [Point; 4], left: f32, right: f32, y: f32, flipped: bool) -> usize {
    let cubic = cubic_from(&a);
    i.set_max(3);
    i.horizontal_cubic(&cubic, f64::from(left), f64::from(right), f64::from(y), flipped)
}

/// `SkIntersections::cubicVertical`.
// Port of: src/pathops/SkIntersections.h#L103-L108 (chrome/m156)
fn cubic_vertical(i: &mut Intersections, a: [Point; 4], top: f32, bottom: f32, x: f32, flipped: bool) -> usize {
    let cubic = cubic_from(&a);
    i.set_max(3);
    i.vertical_cubic(&cubic, f64::from(top), f64::from(bottom), f64::from(x), flipped)
}

/// `SkIntersections::cubicLine(a, b)`.
// Port of: src/pathops/SkIntersections.h#L110-L117 (chrome/m156)
fn cubic_line(i: &mut Intersections, a: [Point; 4], b: [Point; 4]) -> usize {
    i.set_max(3);
    i.intersect_cubic_line(&cubic_from(&a), &line_from(b))
}

/// `SkDLine::set(pts)`: the first two points of `a`, widened to double.
fn line_from(a: [Point; 4]) -> DLine {
    DLine::new([
        DPoint::new(f64::from(a[0].x), f64::from(a[0].y)),
        DPoint::new(f64::from(a[1].x), f64::from(a[1].y)),
    ])
}

/// `AddIntersectTs(test, next, coincidence)`: records every intersection of `test`'s segments
/// with `next`'s segments in the op graph. Returns false if the contours cannot intersect at
/// all, and true otherwise (including when they are apart).
// Port of: src/pathops/SkAddIntersections.cpp#L286-L360 (chrome/m156)
pub(crate) fn add_intersect_ts(state: &mut OpState, test: ContourId, next: ContourId, coincidence: CoinSetId) -> bool {
    if test != next {
        if almost_less_ulps(state.contour_bounds(test).bottom, state.contour_bounds(next).top) {
            return false;
        }
        if !Bounds::intersects(&state.contour_bounds(test), &state.contour_bounds(next)) {
            return true;
        }
    }
    let mut wt = IntersectionHelper::init(state, test);
    if wt.segment.is_none() {
        return true;
    }
    loop {
        let mut wn = IntersectionHelper::init(state, next);
        'wt_body: {
            if test == next && !wn.start_after(state, wt) {
                break 'wt_body;
            }
            loop {
                'wn_body: {
                    if !Bounds::intersects(&wt.bounds(state), &wn.bounds(state)) {
                        break 'wn_body;
                    }
                    if !add_intersections_for_pair(state, wt, wn, coincidence) {
                        return false;
                    }
                }
                if !wn.advance(state) {
                    break;
                }
            }
        }
        if !wt.advance(state) {
            break;
        }
    }
    true
}

/// The body of the innermost loop of `AddIntersectTs`: intersects one pair of segments and
/// records the points. Returns false when a point cannot be added.
// Port of: src/pathops/SkAddIntersections.cpp#L303-L536 (chrome/m156)
fn add_intersections_for_pair(
    state: &mut OpState,
    wt: IntersectionHelper,
    wn: IntersectionHelper,
    coincidence: CoinSetId,
) -> bool {
    let mut ts = Intersections::default();
    let mut swap = false;
    let pts: usize;
    let wt_pts = wt.pts(state);
    let wn_pts = wn.pts(state);
    let wt_weight = wt.weight(state);
    let wn_weight = wn.weight(state);
    match wt.segment_type(state) {
        SegmentType::HorizontalLine => {
            swap = true;
            match wn.segment_type(state) {
                SegmentType::HorizontalLine | SegmentType::VerticalLine | SegmentType::Line => {
                    pts = line_horizontal(&mut ts, wn_pts, wt.left(state), wt.right(state), wt.y(state), wt.x_flipped(state));
                }
                SegmentType::Quad => {
                    pts = quad_horizontal(&mut ts, wn_pts, wt.left(state), wt.right(state), wt.y(state), wt.x_flipped(state));
                }
                SegmentType::Conic => {
                    pts = conic_horizontal(
                        &mut ts,
                        wn_pts,
                        wn_weight,
                        wt.left(state),
                        wt.right(state),
                        wt.y(state),
                        wt.x_flipped(state),
                    );
                }
                SegmentType::Cubic => {
                    pts = cubic_horizontal(&mut ts, wn_pts, wt.left(state), wt.right(state), wt.y(state), wt.x_flipped(state));
                }
            }
        }
        SegmentType::VerticalLine => {
            swap = true;
            match wn.segment_type(state) {
                SegmentType::HorizontalLine | SegmentType::VerticalLine | SegmentType::Line => {
                    pts = line_vertical(&mut ts, wn_pts, wt.top(state), wt.bottom(state), wt.x(state), wt.y_flipped(state));
                }
                SegmentType::Quad => {
                    pts = quad_vertical(&mut ts, wn_pts, wt.top(state), wt.bottom(state), wt.x(state), wt.y_flipped(state));
                }
                SegmentType::Conic => {
                    pts = conic_vertical(
                        &mut ts,
                        wn_pts,
                        wn_weight,
                        wt.top(state),
                        wt.bottom(state),
                        wt.x(state),
                        wt.y_flipped(state),
                    );
                }
                SegmentType::Cubic => {
                    pts = cubic_vertical(&mut ts, wn_pts, wt.top(state), wt.bottom(state), wt.x(state), wt.y_flipped(state));
                }
            }
        }
        SegmentType::Line => match wn.segment_type(state) {
            SegmentType::HorizontalLine => {
                pts = line_horizontal(&mut ts, wt_pts, wn.left(state), wn.right(state), wn.y(state), wn.x_flipped(state));
            }
            SegmentType::VerticalLine => {
                pts = line_vertical(&mut ts, wt_pts, wn.top(state), wn.bottom(state), wn.x(state), wn.y_flipped(state));
            }
            SegmentType::Line => {
                pts = line_line(&mut ts, wt_pts, wn_pts);
            }
            SegmentType::Quad => {
                swap = true;
                pts = quad_line(&mut ts, wn_pts, wt_pts);
            }
            SegmentType::Conic => {
                swap = true;
                pts = conic_line(&mut ts, wn_pts, wn_weight, wt_pts);
            }
            SegmentType::Cubic => {
                swap = true;
                pts = cubic_line(&mut ts, wn_pts, wt_pts);
            }
        },
        SegmentType::Quad => match wn.segment_type(state) {
            SegmentType::HorizontalLine => {
                pts = quad_horizontal(&mut ts, wt_pts, wn.left(state), wn.right(state), wn.y(state), wn.x_flipped(state));
            }
            SegmentType::VerticalLine => {
                pts = quad_vertical(&mut ts, wt_pts, wn.top(state), wn.bottom(state), wn.x(state), wn.y_flipped(state));
            }
            SegmentType::Line => {
                pts = quad_line(&mut ts, wt_pts, wn_pts);
            }
            SegmentType::Quad => {
                pts = ts.intersect_quad_quad(&quad_from(&wt_pts), &quad_from(&wn_pts));
            }
            SegmentType::Conic => {
                swap = true;
                pts = ts.intersect_conic_quad(&conic_from(&wn_pts[..3], wn_weight), &quad_from(&wt_pts));
            }
            SegmentType::Cubic => {
                swap = true;
                pts = ts.intersect_cubic_quad(&cubic_from(&wn_pts), &quad_from(&wt_pts));
            }
        },
        SegmentType::Conic => match wn.segment_type(state) {
            SegmentType::HorizontalLine => {
                pts = conic_horizontal(
                    &mut ts,
                    wt_pts,
                    wt_weight,
                    wn.left(state),
                    wn.right(state),
                    wn.y(state),
                    wn.x_flipped(state),
                );
            }
            SegmentType::VerticalLine => {
                pts = conic_vertical(
                    &mut ts,
                    wt_pts,
                    wt_weight,
                    wn.top(state),
                    wn.bottom(state),
                    wn.x(state),
                    wn.y_flipped(state),
                );
            }
            SegmentType::Line => {
                pts = conic_line(&mut ts, wt_pts, wt_weight, wn_pts);
            }
            SegmentType::Quad => {
                pts = ts.intersect_conic_quad(&conic_from(&wt_pts[..3], wt_weight), &quad_from(&wn_pts));
            }
            SegmentType::Conic => {
                pts = ts.intersect_conic_conic(
                    &conic_from(&wt_pts[..3], wt_weight),
                    &conic_from(&wn_pts[..3], wn_weight),
                );
            }
            SegmentType::Cubic => {
                swap = true;
                pts = ts.intersect_cubic_conic(&cubic_from(&wn_pts), &conic_from(&wt_pts[..3], wt_weight));
            }
        },
        SegmentType::Cubic => match wn.segment_type(state) {
            SegmentType::HorizontalLine => {
                pts = cubic_horizontal(&mut ts, wt_pts, wn.left(state), wn.right(state), wn.y(state), wn.x_flipped(state));
            }
            SegmentType::VerticalLine => {
                pts = cubic_vertical(&mut ts, wt_pts, wn.top(state), wn.bottom(state), wn.x(state), wn.y_flipped(state));
            }
            SegmentType::Line => {
                pts = cubic_line(&mut ts, wt_pts, wn_pts);
            }
            SegmentType::Quad => {
                pts = ts.intersect_cubic_quad(&cubic_from(&wt_pts), &quad_from(&wn_pts));
            }
            SegmentType::Conic => {
                pts = ts.intersect_cubic_conic(&cubic_from(&wt_pts), &conic_from(&wn_pts[..3], wn_weight));
            }
            SegmentType::Cubic => {
                pts = ts.intersect_cubic_cubic(&cubic_from(&wt_pts), &cubic_from(&wn_pts));
            }
        },
    }
    let mut coin_index: i32 = -1;
    let mut coin_ptt: [PtTId; 2] = [PtTId(usize::MAX); 2];
    for index in 0..pts {
        let i_pt: Point = ts.pt(index).as_sk_point();
        let i_pt_is_integral = i_pt.x == i_pt.x.floor() && i_pt.y == i_pt.y.floor();
        let swap_index = usize::from(swap);
        let Some(mut test_t_at) = (if i_pt_is_integral {
            state.seg_add_t_pt(wt.seg(), ts.t(swap_index, index), i_pt)
        } else {
            state.seg_add_t(wt.seg(), ts.t(swap_index, index))
        }) else {
            return false;
        };
        let Some(mut next_t_at) = (if i_pt_is_integral {
            state.seg_add_t_pt(wn.seg(), ts.t(1 - swap_index, index), i_pt)
        } else {
            state.seg_add_t(wn.seg(), ts.t(1 - swap_index, index))
        }) else {
            return false;
        };
        if !state.ptt_contains_ptt(test_t_at, next_t_at) {
            // Returns None if the pair already shares a pt-t loop.
            if let Some(opp_prev) = state.ptt_opp_prev(test_t_at, next_t_at) {
                let test_span = state.ptt_span(test_t_at);
                let next_span = state.ptt_span(next_t_at);
                let _ = state.span_merge_matches(test_span, next_span);
                state.ptt_add_opp(test_t_at, next_t_at, opp_prev);
            }
            if state.ptt_pt(test_t_at) != state.ptt_pt(next_t_at) {
                let test_span = state.ptt_span(test_t_at);
                let next_span = state.ptt_span(next_t_at);
                state.span_set_aligned(test_span, false);
                state.span_set_aligned(next_span, false);
            }
        }
        if !ts.is_coincident(index) {
            continue;
        }
        if coin_index < 0 {
            coin_ptt = [test_t_at, next_t_at];
            coin_index = index as i32;
            continue;
        }
        if state.ptt_span(coin_ptt[0]) == state.ptt_span(test_t_at) {
            coin_index = -1;
            continue;
        }
        if state.ptt_span(coin_ptt[1]) == state.ptt_span(next_t_at) {
            // Coincidence span collapsed.
            coin_index = -1;
            continue;
        }
        if swap {
            coin_ptt.swap(0, 1);
            std::mem::swap(&mut test_t_at, &mut next_t_at);
        }
        if state.span_deleted(state.ptt_span(coin_ptt[0])) {
            coin_index = -1;
            continue;
        }
        if state.span_deleted(state.ptt_span(test_t_at)) {
            coin_index = -1;
            continue;
        }
        state.cs_add(coincidence, coin_ptt[0], test_t_at, coin_ptt[1], next_t_at);
        coin_index = -1;
    }
    true
}
