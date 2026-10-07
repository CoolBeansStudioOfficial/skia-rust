// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkDashPathPriv.h, src/utils/SkDashPath.cpp

//! `SkDashPath`: the dashing algorithm behind the dash path effect.

use skia_rust_core::floating_point::{ieee_float_divide, is_finite_all, is_nan};
use skia_rust_core::paint::Join;
use skia_rust_core::path::{Iter, Path};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::DashInfo;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{Point, Vector, point_priv};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_1, SCALAR_NEARLY_ZERO, scalar, scalar_abs, scalar_ceil_to_int, scalar_mod,
};
use skia_rust_core::stroke_rec::{StrokeRec, Style};

/// The most dash segments a path is dashed into (`SkDashPath::kMaxDashCount`).
// Port of: src/utils/SkDashPathPriv.h#L31-L35 (chrome/m156)
#[doc(alias = "kMaxDashCount")]
pub const MAX_DASH_COUNT: scalar = 1_000_000.0;

/// Whether [`internal_filter`] may stroke simple shapes itself
/// (`SkDashPath::StrokeRecApplication`).
// Port of: src/utils/SkDashPathPriv.h#L37-L40 (chrome/m156)
#[doc(alias = "SkDashPath::StrokeRecApplication")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StrokeRecApplication {
    /// `kDisallow`.
    Disallow,
    /// `kAllow`.
    Allow,
}

/// The values [`calc_dash_parameters`] computes from a phase and the intervals.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DashParameters {
    /// `initialDashLength`.
    pub initial_dash_length: scalar,
    /// `initialDashIndex`.
    pub initial_dash_index: usize,
    /// `intervalLength`.
    pub interval_length: scalar,
    /// `adjustedPhase`: the phase between 0 and `interval_length` (the input phase unchanged
    /// when the phase was not adjusted).
    pub adjusted_phase: scalar,
}

// Port of: src/utils/SkDashPath.cpp#L32-L34 (chrome/m156)
fn is_even(x: i32) -> bool {
    (x & 1) == 0
}

// Port of: src/utils/SkDashPath.cpp#L36-L55 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn find_first_interval(intervals: &[scalar], mut phase: scalar) -> (scalar, usize) {
    for (i, &gap) in intervals.iter().enumerate() {
        if phase > gap || (phase == gap && gap != 0.0) {
            phase -= gap;
        } else {
            return (gap - phase, i);
        }
    }
    // If we get here, phase "appears" to be larger than our length. This
    // shouldn't happen with perfect precision, but we can accumulate errors
    // during the initial length computation (rounding can make our sum be too
    // big or too small. In that event, we just have to eat the error here.
    (intervals[0], 0)
}

/// Calculates the initial dash length, initial dash index and interval length based on the
/// inputed phase and intervals. If `adjust_phase`, then the phase is adjusted to be between 0
/// and the interval length (and returned in [`DashParameters::adjusted_phase`]). Otherwise it
/// is assumed phase is already between 0 and the interval length.
///
/// Caller should have already used [`valid_dash_path`] to exclude invalid data.
// Port of: src/utils/SkDashPath.cpp#L57-L93 (chrome/m156)
#[doc(alias = "CalcDashParameters")]
#[must_use]
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
pub fn calc_dash_parameters(
    mut phase: scalar,
    intervals: &[scalar],
    adjust_phase: bool,
) -> DashParameters {
    let mut len: scalar = 0.0;
    for &interval in intervals {
        len += interval;
    }
    let interval_length = len;
    // Adjust phase to be between 0 and len, "flipping" phase if negative.
    // e.g., if len is 100, then phase of -20 (or -120) is equivalent to 80
    let mut adjusted_phase = phase;
    if adjust_phase {
        if phase < 0.0 {
            phase = -phase;
            if phase > len {
                phase = scalar_mod(phase, len);
            }
            phase = len - phase;

            // Due to finite precision, it's possible that phase == len,
            // even after the subtract (if len >>> phase), so fix that here.
            // This fixes http://crbug.com/124652 .
            debug_assert!(phase <= len);
            if phase == len {
                phase = 0.0;
            }
        } else if phase >= len {
            phase = scalar_mod(phase, len);
        }
        adjusted_phase = phase;
    }
    debug_assert!(phase >= 0.0 && phase < len);

    let (initial_dash_length, initial_dash_index) = find_first_interval(intervals, phase);

    debug_assert!(initial_dash_length >= 0.0);
    debug_assert!(initial_dash_index < intervals.len());
    DashParameters {
        initial_dash_length,
        initial_dash_index,
        interval_length,
        adjusted_phase,
    }
}

// Port of: src/utils/SkDashPath.cpp#L95-L104 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn outset_for_stroke(rect: &mut Rect, rec: &StrokeRec) {
    let mut radius = rec.width() / 2.0;
    if 0.0 == radius {
        radius = SCALAR_1; // hairlines
    }
    if Join::Miter == rec.join() {
        radius *= rec.miter();
    }
    rect.outset((radius, radius));
}

// If line is zero-length, bump out the end by a tiny amount
// to draw endcaps. The bump factor is sized so that
// SkPoint::Distance() computes a non-zero length.
// Offsets SK_ScalarNearlyZero or smaller create empty paths when Iter measures length.
// Large values are scaled by SK_ScalarNearlyZero so significant bits change.
// Port of: src/utils/SkDashPath.cpp#L106-L113 (chrome/m156)
fn adjust_zero_length_line(pts: &mut [Point; 2]) {
    debug_assert_eq!(pts[0], pts[1]);
    // std::max(1.001f, x)
    let x = pts[1].x;
    let m = if 1.001_f32 < x { x } else { 1.001_f32 };
    pts[1].x += m * SCALAR_NEARLY_ZERO;
}

// `(&pts[0].fX)[xyOffset]`
fn xy(pt: Point, xy_offset: usize) -> scalar {
    if xy_offset == 0 { pt.x } else { pt.y }
}

fn set_xy(pt: &mut Point, xy_offset: usize, v: scalar) {
    if xy_offset == 0 {
        pt.x = v;
    } else {
        pt.y = v;
    }
}

// Port of: src/utils/SkDashPath.cpp#L115-L177 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn clip_line(
    pts: &mut [Point; 2],
    bounds: &Rect,
    interval_length: scalar,
    prior_phase: scalar,
) -> bool {
    let dxy: Vector = pts[1] - pts[0];

    // only horizontal or vertical lines
    if dxy.x != 0.0 && dxy.y != 0.0 {
        return false;
    }
    let xy_offset = usize::from(dxy.y != 0.0); // 0 to adjust horizontal, 1 to adjust vertical

    let mut min_xy = xy(pts[0], xy_offset);
    let mut max_xy = xy(pts[1], xy_offset);
    let swapped = max_xy < min_xy;
    if swapped {
        std::mem::swap(&mut min_xy, &mut max_xy);
    }

    debug_assert!(min_xy <= max_xy);
    let (left_top, right_bottom) = if xy_offset == 0 {
        (bounds.left, bounds.right)
    } else {
        (bounds.top, bounds.bottom)
    };
    if max_xy < left_top || min_xy > right_bottom {
        return false;
    }

    // Now we actually perform the chop, removing the excess to the left/top and
    // right/bottom of the bounds (keeping our new line "in phase" with the dash,
    // hence the (mod intervalLength).

    if min_xy < left_top {
        min_xy = left_top - scalar_mod(left_top - min_xy, interval_length);
        if !swapped {
            min_xy -= prior_phase; // for rectangles, adjust by prior phase
        }
    }
    if max_xy > right_bottom {
        max_xy = right_bottom + scalar_mod(max_xy - right_bottom, interval_length);
        if swapped {
            max_xy += prior_phase; // for rectangles, adjust by prior phase
        }
    }

    debug_assert!(max_xy >= min_xy);
    if swapped {
        std::mem::swap(&mut min_xy, &mut max_xy);
    }
    set_xy(&mut pts[0], xy_offset, min_xy);
    set_xy(&mut pts[1], xy_offset, max_xy);

    if min_xy == max_xy {
        adjust_zero_length_line(pts);
    }
    true
}

// Handles only lines and rects.
// If cull_path() returns true, builder is the new smaller path,
// otherwise builder may have been changed but you should ignore it.
// Port of: src/utils/SkDashPath.cpp#L179-L246 (chrome/m156)
fn cull_path(
    src_path: &Path,
    rec: &StrokeRec,
    cull_rect: Option<&Rect>,
    interval_length: scalar,
    builder: &mut PathBuilder,
) -> bool {
    let Some(cull_rect) = cull_rect else {
        if let Some((p0, p1)) = src_path.is_line() {
            let mut pts = [p0, p1];
            if pts[0] == pts[1] {
                adjust_zero_length_line(&mut pts);
                builder.move_to(pts[0]);
                builder.line_to(pts[1]);
                return true;
            }
        }
        return false;
    };

    let mut bounds = *cull_rect;
    outset_for_stroke(&mut bounds, rec);

    if let Some((p0, p1)) = src_path.is_line() {
        let mut pts = [p0, p1];
        if clip_line(&mut pts, &bounds, interval_length, 0.0) {
            builder.move_to(pts[0]);
            builder.line_to(pts[1]);
            return true;
        }
        return false;
    }

    if src_path.is_rect().is_some() {
        // We'll break the rect into four lines, culling each separately.
        let mut iter = Iter::new(src_path, false);

        let it = iter.next_rec();
        debug_assert!(it.is_some_and(|it| it.verb() == PathVerb::Move));

        let mut accum: f64 = 0.0; // Sum of unculled edge lengths to keep the phase correct.
        // Intentionally a double to minimize the risk of overflow and drift.
        while let Some(it) = iter.next_rec() {
            if it.verb() != PathVerb::Line {
                break;
            }
            // Notice this vector v and accum work with the original unclipped length.
            let v: Vector = it.points()[1] - it.points()[0];

            let mut pts = [it.points()[0], it.points()[1]];
            // std::fmod(accum, intervalLength): the scalar widens to double, the result narrows
            // back to a scalar parameter.
            let prior_phase =
                skia_rust_core::scalar::double_to_scalar(accum % f64::from(interval_length));
            if clip_line(&mut pts, &bounds, interval_length, prior_phase) {
                // pts[0] may have just been changed by clip_line().
                // If that's not where we ended the previous lineTo(), we need to moveTo() there.
                let maybe_last = builder.get_last_pt();
                if maybe_last.is_none_or(|last| last != pts[0]) {
                    builder.move_to(pts[0]);
                }
                builder.line_to(pts[1]);
            }

            // We either just traveled v.fX horizontally or v.fY vertically.
            debug_assert!(v.x == 0.0 || v.y == 0.0);
            accum += f64::from(scalar_abs(v.x + v.y));
        }
        return !builder.is_empty();
    }

    false
}

// Port of: src/utils/SkDashPath.cpp#L248-L326 (chrome/m156)
struct SpecialLineRec {
    pts: [Point; 2],
    tangent: Vector,
    normal: Vector,
    path_length: scalar,
}

impl SpecialLineRec {
    // Port of: src/utils/SkDashPath.cpp#L250-L297 (chrome/m156)
    fn init(
        src: &Path,
        dst: &mut PathBuilder,
        rec: &mut StrokeRec,
        interval_count: i32,
        interval_length: scalar,
    ) -> Option<SpecialLineRec> {
        if rec.is_hairline_style() {
            return None;
        }
        let (p0, p1) = src.is_line()?;
        let pts = [p0, p1];

        // can relax this in the future, if we handle square and round caps
        if skia_rust_core::paint::Cap::Butt != rec.cap() {
            return None;
        }

        let path_length = Point::distance(pts[0], pts[1]);

        let mut tangent = pts[1] - pts[0];
        if tangent.is_zero() {
            return None;
        }

        tangent.scale(ieee_float_divide(1.0, path_length));
        if !is_finite_all(tangent.x, &[tangent.y]) {
            return None;
        }
        let mut normal = point_priv::rotate_ccw(tangent);
        normal.scale(rec.width() / 2.0);

        // now estimate how many quads will be added to the path
        //     resulting segments = pathLen * intervalCount / intervalLen
        //     resulting points = 4 * segments

        #[allow(clippy::cast_precision_loss)] // mirrors the int -> float conversion
        let mut pt_count: scalar = path_length * (interval_count as scalar) / interval_length;
        // std::min(ptCount, kMaxDashCount)
        if MAX_DASH_COUNT < pt_count {
            pt_count = MAX_DASH_COUNT;
        }
        if is_nan(pt_count) {
            return None;
        }
        let n = scalar_ceil_to_int(pt_count) << 2;
        dst.inc_reserve(n, n, 0);

        // we will take care of the stroking
        rec.set_fill_style();
        Some(SpecialLineRec {
            pts,
            tangent,
            normal,
            path_length,
        })
    }

    // Port of: src/utils/SkDashPath.cpp#L299-L320 (chrome/m156)
    fn add_segment(&self, d0: scalar, mut d1: scalar, path: &mut PathBuilder) {
        debug_assert!(d0 <= self.path_length);
        // clamp the segment to our length
        if d1 > self.path_length {
            d1 = self.path_length;
        }

        let x0 = self.pts[0].x + self.tangent.x * d0;
        let x1 = self.pts[0].x + self.tangent.x * d1;
        let y0 = self.pts[0].y + self.tangent.y * d0;
        let y1 = self.pts[0].y + self.tangent.y * d1;

        let pts = [
            Point::new(x0 + self.normal.x, y0 + self.normal.y), // moveTo
            Point::new(x1 + self.normal.x, y1 + self.normal.y), // lineTo
            Point::new(x1 - self.normal.x, y1 - self.normal.y), // lineTo
            Point::new(x0 - self.normal.x, y0 - self.normal.y), // lineTo
        ];

        path.add_polygon(&pts, false);
    }
}

/// Dashes `src` into `dst`. Caller should have already used [`valid_dash_path`] to exclude
/// invalid data. Typically, this leaves the stroke rec unmodified. However, for some simple
/// shapes (e.g. a line) it may directly evaluate the dash and stroke to produce a stroked output
/// path with a fill stroke rec. Passing [`StrokeRecApplication::Disallow`] turns this behavior
/// off.
// Port of: src/utils/SkDashPath.cpp#L329-L504 (chrome/m156)
#[doc(alias = "InternalFilter")]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // one function in C++
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
pub fn internal_filter(
    dst: &mut PathBuilder,
    src: &Path,
    rec: &mut StrokeRec,
    cull_rect: Option<&Rect>,
    intervals: &[scalar],
    initial_dash_length: scalar,
    initial_dash_index: i32,
    interval_length: scalar,
    start_phase: scalar,
    stroke_rec_application: StrokeRecApplication,
) -> bool {
    let src_pts = src.points();
    if src_pts.is_empty() {
        return true;
    }
    let count = intervals.len();
    // we must always have an even number of intervals
    debug_assert!(count.is_multiple_of(2));

    // we do nothing if the src wants to be filled
    let style = rec.style();
    if Style::Fill == style || Style::StrokeAndFill == style {
        return false;
    }

    let mut dash_count: scalar = 0.0;

    let mut builder = PathBuilder::new();
    let cull_path_storage: Path;
    let mut active_src = src;
    if cull_path(src, rec, cull_rect, interval_length, &mut builder) {
        // if rect is closed, starts in a dash, and ends in a dash, add the initial join
        // potentially a better fix is described here: skbug.com/40038693
        if src.is_rect().is_some() && src.is_last_contour_closed() && is_even(initial_dash_index) {
            let path_length = PathMeasure::new(src, false, rec.res_scale()).length();
            let mut end_phase = scalar_mod(path_length + start_phase, interval_length);
            let mut index: usize = 0;
            while end_phase > intervals[index] {
                end_phase -= intervals[index];
                index += 1;
                debug_assert!(index <= count);
                if index == count {
                    // We have run out of intervals. endPhase "should" never get to this point,
                    // but it could if the subtracts underflowed. Hence we will pin it as if it
                    // perfectly ran through the intervals.
                    // See crbug.com/875494 (and skbug.com/40039544)
                    end_phase = 0.0;
                    break;
                }
            }
            // if dash ends inside "on", or ends at beginning of "off"
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            // mirrors the size_t -> int conversion of is_even(index)
            let index_even = is_even(index as i32);
            if index_even == (end_phase > 0.0) {
                let mid_point = src_pts[0];
                // get vector at end of rect
                let mut last = src.count_points() - 1;
                while mid_point == src_pts[last] {
                    last -= 1;
                }
                // get vector at start of rect
                let mut next = 1;
                while mid_point == src_pts[next] {
                    next += 1;
                    debug_assert!(next < last);
                }
                let mut v: Vector = mid_point - src_pts[last];
                let k_tiny_offset = SCALAR_NEARLY_ZERO;
                // scale vector to make start of tiny right angle
                v *= k_tiny_offset;
                builder.move_to(mid_point - v);
                builder.line_to(mid_point);
                v = mid_point - src_pts[next];
                // scale vector to make end of tiny right angle
                v *= k_tiny_offset;
                builder.line_to(mid_point - v);
            }
        }

        cull_path_storage = builder.detach();
        active_src = &cull_path_storage;
    }

    let line_rec = if StrokeRecApplication::Allow == stroke_rec_application {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // mirrors the size_t -> int conversion of `count >> 1`
        SpecialLineRec::init(active_src, dst, rec, (count >> 1) as i32, interval_length)
    } else {
        None
    };

    let mut meas = PathMeasure::new(active_src, false, rec.res_scale());

    loop {
        let mut skip_first_segment = meas.is_closed();
        let mut added_segment = false;
        let length = meas.length();
        #[allow(clippy::cast_sign_loss)] // initial_dash_index is never negative
        let mut index = initial_dash_index as usize;

        // Since the path length / dash length ratio may be arbitrarily large, we can exert
        // significant memory pressure while attempting to build the filtered path. To avoid this,
        // we simply give up dashing beyond a certain threshold.
        //
        // The original bug report (http://crbug.com/165432) is based on a path yielding more than
        // 90 million dash segments and crashing the memory allocator. A limit of 1 million
        // segments seems reasonable: at 2 verbs per segment * 9 bytes per verb, this caps the
        // maximum dash memory overhead at roughly 17MB per path.
        #[allow(clippy::cast_precision_loss)] // mirrors the size_t -> float conversion
        {
            dash_count += length * ((count >> 1) as scalar) / interval_length;
        }
        if dash_count > MAX_DASH_COUNT {
            dst.reset();
            return false;
        }

        // Using double precision to avoid looping indefinitely due to single precision rounding
        // (for extreme path_length/dash_length ratios). See test_infinite_dash() unittest.
        let mut distance: f64 = 0.0;
        let mut dlen: f64 = f64::from(initial_dash_length);

        while distance < f64::from(length) {
            debug_assert!(dlen >= 0.0);
            added_segment = false;
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            // mirrors the size_t -> int conversion of is_even(index)
            let index_even = is_even(index as i32);
            if index_even && !skip_first_segment {
                added_segment = true;

                if let Some(line_rec) = &line_rec {
                    line_rec.add_segment(
                        skia_rust_core::scalar::double_to_scalar(distance),
                        skia_rust_core::scalar::double_to_scalar(distance + dlen),
                        dst,
                    );
                } else {
                    meas.get_segment(
                        skia_rust_core::scalar::double_to_scalar(distance),
                        skia_rust_core::scalar::double_to_scalar(distance + dlen),
                        dst,
                        true,
                    );
                }
            }
            distance += dlen;

            // clear this so we only respect it the first time around
            skip_first_segment = false;

            // wrap around our intervals array if necessary
            index += 1;
            debug_assert!(index <= count);
            if index == count {
                index = 0;
            }

            // fetch our next dlen
            dlen = f64::from(intervals[index]);
        }

        // extend if we ended on a segment and we need to join up with the (skipped) initial segment
        if meas.is_closed() && is_even(initial_dash_index) && initial_dash_length >= 0.0 {
            meas.get_segment(0.0, initial_dash_length, dst, !added_segment);
        }
        if !meas.next_contour() {
            break;
        }
    }

    true
}

/// Dashes `src` into `dst` with the dash pattern `info` (`SkDashPath::FilterDashPath`).
// Port of: src/utils/SkDashPath.cpp#L506-L519 (chrome/m156)
#[doc(alias = "FilterDashPath")]
pub fn filter_dash_path(
    dst: &mut PathBuilder,
    src: &Path,
    rec: &mut StrokeRec,
    cull_rect: Option<&Rect>,
    info: &DashInfo,
) -> bool {
    if !valid_dash_path(info.phase, &info.intervals) {
        return false;
    }
    let params = calc_dash_parameters(info.phase, &info.intervals, false);
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    // mirrors the size_t -> int32_t conversion of initialDashIndex
    let initial_dash_index = params.initial_dash_index as i32;
    internal_filter(
        dst,
        src,
        rec,
        cull_rect,
        &info.intervals,
        params.initial_dash_length,
        initial_dash_index,
        params.interval_length,
        info.phase,
        StrokeRecApplication::Allow,
    )
}

/// True if `phase` and `intervals` describe a dash pattern: an even number (at least 2) of
/// non-negative intervals with a positive, finite sum and a finite phase
/// (`SkDashPath::ValidDashPath`).
// Port of: src/utils/SkDashPath.cpp#L521-L535 (chrome/m156)
#[doc(alias = "ValidDashPath")]
#[must_use]
pub fn valid_dash_path(phase: scalar, intervals: &[scalar]) -> bool {
    if intervals.len() < 2 || !intervals.len().is_multiple_of(2) {
        return false;
    }
    let mut length: scalar = 0.0;
    for &interval in intervals {
        if interval < 0.0 {
            return false;
        }
        length += interval;
    }
    // watch out for values that might make us go out of bounds
    length > 0.0 && is_finite_all(phase, &[length])
}
