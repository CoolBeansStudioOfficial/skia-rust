// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/SkDashImpl.h, src/effects/SkDashPathEffect.cpp

//! `SkDashImpl`: the dash path effect.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{DashInfo, PathEffect, PathEffectBase, PointData, PointFlags};
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_1, Scalar, scalar, scalar_floor_to_int, scalar_invert, scalar_is_int, scalar_mod,
};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::dash_path::{
    MAX_DASH_COUNT, StrokeRecApplication, calc_dash_parameters, internal_filter,
};

/// The dash path effect (`SkDashImpl`).
// Port of: src/effects/SkDashImpl.h#L16-L49 (chrome/m156)
#[doc(alias = "SkDashImpl")]
#[derive(Clone, Debug)]
pub struct DashImpl {
    intervals: Vec<scalar>,
    phase: scalar,

    // computed from phase
    initial_dash_length: scalar,
    interval_length: scalar,
    initial_dash_index: usize,
}

impl DashImpl {
    /// `intervals` must have an even number (at least 2) of entries; see
    /// [`crate::dash_path::valid_dash_path`].
    // Port of: src/effects/SkDashPathEffect.cpp#L33-L47 (chrome/m156)
    #[must_use]
    pub fn new(intervals: &[scalar], phase: scalar) -> Self {
        debug_assert!(intervals.len() > 1 && intervals.len().is_multiple_of(2));

        let intervals = intervals.to_vec();

        // set the internal data members
        let params = calc_dash_parameters(phase, &intervals, true);
        Self {
            intervals,
            phase: params.adjusted_phase,
            initial_dash_length: params.initial_dash_length,
            interval_length: params.interval_length,
            initial_dash_index: params.initial_dash_index,
        }
    }
}

// Port of: src/effects/SkDashPathEffect.cpp#L56-L66 (chrome/m156)
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

// Attempt to trim the line to minimally cover the cull rect (currently
// only works for horizontal and vertical lines).
// Return true if processing should continue; false otherwise.
// Port of: src/effects/SkDashPathEffect.cpp#L68-L171 (chrome/m156)
#[allow(clippy::if_not_else)] // keeps the C++ branch order
fn cull_line(
    pts: &mut [Point; 2],
    rec: &StrokeRec,
    ctm: &Matrix,
    cull_rect: Option<&Rect>,
    interval_length: scalar,
) -> bool {
    let Some(cull_rect) = cull_rect else {
        debug_assert!(false); // Shouldn't ever occur in practice
        return false;
    };

    let dx = pts[1].x - pts[0].x;
    let dy = pts[1].y - pts[0].y;

    if (dx != 0.0 && dy != 0.0) || (dx == 0.0 && dy == 0.0) {
        return false;
    }

    let mut bounds = *cull_rect;
    outset_for_stroke(&mut bounds, rec);

    // cullRect is in device space while pts are in the local coordinate system
    // defined by the ctm. We want our answer in the local coordinate system.

    debug_assert!(ctm.rect_stays_rect());
    let Some(inv) = ctm.invert() else {
        return false;
    };

    bounds = inv.map_rect(bounds).0;

    if dx != 0.0 {
        debug_assert!(dx != 0.0 && dy == 0.0);
        let mut min_x = pts[0].x;
        let mut max_x = pts[1].x;

        if dx < 0.0 {
            std::mem::swap(&mut min_x, &mut max_x);
        }

        debug_assert!(min_x < max_x);
        if max_x <= bounds.left || min_x >= bounds.right {
            return false;
        }

        // Now we actually perform the chop, removing the excess to the left and
        // right of the bounds (keeping our new line "in phase" with the dash,
        // hence the (mod intervalLength).

        if min_x < bounds.left {
            min_x = bounds.left - scalar_mod(bounds.left - min_x, interval_length);
        }
        if max_x > bounds.right {
            max_x = bounds.right + scalar_mod(max_x - bounds.right, interval_length);
        }

        debug_assert!(max_x > min_x);
        if dx < 0.0 {
            std::mem::swap(&mut min_x, &mut max_x);
        }
        pts[0].x = min_x;
        pts[1].x = max_x;
    } else {
        debug_assert!(dy != 0.0 && dx == 0.0);
        let mut min_y = pts[0].y;
        let mut max_y = pts[1].y;

        if dy < 0.0 {
            std::mem::swap(&mut min_y, &mut max_y);
        }

        debug_assert!(min_y < max_y);
        if max_y <= bounds.top || min_y >= bounds.bottom {
            return false;
        }

        // Now we actually perform the chop, removing the excess to the top and
        // bottom of the bounds (keeping our new line "in phase" with the dash,
        // hence the (mod intervalLength).

        if min_y < bounds.top {
            min_y = bounds.top - scalar_mod(bounds.top - min_y, interval_length);
        }
        if max_y > bounds.bottom {
            max_y = bounds.bottom + scalar_mod(max_y - bounds.bottom, interval_length);
        }

        debug_assert!(max_y > min_y);
        if dy < 0.0 {
            std::mem::swap(&mut min_y, &mut max_y);
        }
        pts[0].y = min_y;
        pts[1].y = max_y;
    }

    true
}

/// `SkDashImpl::CreateProc`: the phase, then the intervals, which are validated as a dash.
// Port of: src/effects/SkDashPathEffect.cpp#L379-L389 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<PathEffect> {
    let phase = buffer.read_scalar();
    let count = usize::try_from(buffer.get_array_count()).ok()?;
    // Don't allocate gigantic buffers if there's not data for them.
    if !buffer.validate_can_read_n(count, size_of::<f32>()) {
        return None;
    }
    let mut intervals = vec![0.0; count];
    if !buffer.read_scalar_array(&mut intervals) {
        return None;
    }
    crate::dash_path_effect::new(&intervals, phase)
}

impl PathEffectBase for DashImpl {
    // Port of: src/effects/SkDashPathEffect.cpp#L379 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkDashImpl"
    }

    // Port of: src/effects/SkDashPathEffect.cpp#L374-L377 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_scalar(self.phase);
        buffer.write_scalar_array(&self.intervals);
    }

    // Port of: src/effects/SkDashPathEffect.cpp#L49-L54 (chrome/m156)
    fn on_filter_path(
        &self,
        builder: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // mirrors the size_t -> int32_t conversion of fInitialDashIndex
        let initial_dash_index = self.initial_dash_index as i32;
        internal_filter(
            builder,
            src,
            rec,
            cull_rect,
            &self.intervals,
            self.initial_dash_length,
            initial_dash_index,
            self.interval_length,
            self.phase,
            StrokeRecApplication::Allow,
        )
    }

    // Currently asPoints is more restrictive then it needs to be. In the future
    // we need to:
    //      allow kRound_Cap capping (could allow rotations in the matrix with this)
    //      allow paths to be returned
    // Port of: src/effects/SkDashPathEffect.cpp#L173-L345 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in C++
    #[allow(clippy::many_single_char_names)] // mirrors the C++ names
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    fn on_as_points(
        &self,
        results: &mut PointData,
        src: &Path,
        rec: &StrokeRec,
        matrix: &Matrix,
        cull_rect: Option<&Rect>,
    ) -> bool {
        // width < 0 -> fill && width == 0 -> hairline so requiring width > 0 rules both out
        if 0.0 >= rec.width() {
            return false;
        }

        // TODO: this next test could be eased up. We could allow any number of
        // intervals as long as all the ons match and all the offs match.
        // Additionally, they do not necessarily need to be integers.
        // We cannot allow arbitrary intervals since we want the returned points
        // to be uniformly sized.
        if self.intervals.len() != 2
            || !<scalar as Scalar>::nearly_equal(self.intervals[0], self.intervals[1], None)
            || !scalar_is_int(self.intervals[0])
            || !scalar_is_int(self.intervals[1])
        {
            return false;
        }

        let Some((p0, p1)) = src.is_line() else {
            return false;
        };
        let mut pts = [p0, p1];

        // TODO: this test could be eased up to allow circles
        if Cap::Butt != rec.cap() {
            return false;
        }

        // TODO: this test could be eased up for circles. Rotations could be allowed.
        if !matrix.rect_stays_rect() {
            return false;
        }

        // See if the line can be limited to something plausible.
        if !cull_line(&mut pts, rec, matrix, cull_rect, self.interval_length) {
            return false;
        }

        let length = Point::distance(pts[1], pts[0]);

        let mut tangent = pts[1] - pts[0];
        if tangent.is_zero() {
            return false;
        }

        tangent.scale(scalar_invert(length));

        // TODO: make this test for horizontal & vertical lines more robust
        let mut is_x_axis = true;
        if <scalar as Scalar>::nearly_equal(SCALAR_1, tangent.x, None)
            || <scalar as Scalar>::nearly_equal(-SCALAR_1, tangent.x, None)
        {
            results.size.set(self.intervals[0] / 2.0, rec.width() / 2.0);
        } else if <scalar as Scalar>::nearly_equal(SCALAR_1, tangent.y, None)
            || <scalar as Scalar>::nearly_equal(-SCALAR_1, tangent.y, None)
        {
            results.size.set(rec.width() / 2.0, self.intervals[0] / 2.0);
            is_x_axis = false;
        } else if Cap::Round != rec.cap() {
            // Angled lines don't have axis-aligned boxes.
            return false;
        }

        results.flags = PointFlags::empty();
        // std::min(length, fInitialDashLength)
        let clamped_initial_dash_length = if self.initial_dash_length < length {
            self.initial_dash_length
        } else {
            length
        };

        if Cap::Round == rec.cap() {
            results.flags |= PointFlags::CIRCLES;
        }

        let mut num_points: i32 = 0;
        let mut len2 = length;
        if clamped_initial_dash_length > 0.0 || 0 == self.initial_dash_index {
            debug_assert!(len2 >= clamped_initial_dash_length);
            if 0 == self.initial_dash_index {
                if clamped_initial_dash_length > 0.0 {
                    if clamped_initial_dash_length >= self.intervals[0] {
                        num_points += 1; // partial first dash
                    }
                    len2 -= clamped_initial_dash_length;
                }
                len2 -= self.intervals[1]; // also skip first space
                if len2 < 0.0 {
                    len2 = 0.0;
                }
            } else {
                len2 -= clamped_initial_dash_length; // skip initial partial empty
            }
        }
        // Too many midpoints can cause results->fNumPoints to overflow or
        // otherwise cause the results->fPoints allocation below to OOM.
        // Cap it to a sane value.
        let num_intervals = len2 / self.interval_length;
        if !is_finite(num_intervals) || num_intervals > MAX_DASH_COUNT {
            return false;
        }
        let mut num_mid_points = scalar_floor_to_int(num_intervals);
        num_points += num_mid_points;
        #[allow(clippy::cast_precision_loss)] // mirrors the int -> float conversion
        {
            len2 -= (num_mid_points as scalar) * self.interval_length;
        }
        let mut partial_last = false;
        if len2 > 0.0 {
            if len2 < self.intervals[0] {
                partial_last = true;
            } else {
                num_mid_points += 1;
                num_points += 1;
            }
        }

        results.points =
            vec![Point::default(); usize::try_from(num_points).expect("negative point count")];

        let mut distance: scalar = 0.0;
        let mut cur_pt: usize = 0;

        if clamped_initial_dash_length > 0.0 || 0 == self.initial_dash_index {
            debug_assert!(clamped_initial_dash_length <= length);

            if 0 == self.initial_dash_index {
                if clamped_initial_dash_length > 0.0 {
                    // partial first block
                    debug_assert_ne!(Cap::Round, rec.cap()); // can't handle partial circles
                    let x = pts[0].x + tangent.x * (clamped_initial_dash_length / 2.0);
                    let y = pts[0].y + tangent.y * (clamped_initial_dash_length / 2.0);
                    let (half_width, half_height) = if is_x_axis {
                        (clamped_initial_dash_length / 2.0, rec.width() / 2.0)
                    } else {
                        (rec.width() / 2.0, clamped_initial_dash_length / 2.0)
                    };
                    if clamped_initial_dash_length < self.intervals[0] {
                        // This one will not be like the others
                        results.first = Path::rect(
                            Rect::new(
                                x - half_width,
                                y - half_height,
                                x + half_width,
                                y + half_height,
                            ),
                            None,
                        );
                    } else {
                        debug_assert!(cur_pt < results.points.len());
                        results.points[cur_pt].set(x, y);
                        cur_pt += 1;
                    }

                    distance += clamped_initial_dash_length;
                }

                distance += self.intervals[1]; // skip over the next blank block too
            } else {
                distance += clamped_initial_dash_length;
            }
        }

        if 0 != num_mid_points {
            distance += self.intervals[0] / 2.0;

            for _ in 0..num_mid_points {
                let x = pts[0].x + tangent.x * distance;
                let y = pts[0].y + tangent.y * distance;

                debug_assert!(cur_pt < results.points.len());
                results.points[cur_pt].set(x, y);
                cur_pt += 1;

                distance += self.interval_length;
            }

            distance -= self.intervals[0] / 2.0;
        }

        if partial_last {
            // partial final block
            debug_assert_ne!(Cap::Round, rec.cap()); // can't handle partial circles
            let temp = length - distance;
            debug_assert!(temp < self.intervals[0]);
            let x = pts[0].x + tangent.x * (distance + (temp / 2.0));
            let y = pts[0].y + tangent.y * (distance + (temp / 2.0));
            let (half_width, half_height) = if is_x_axis {
                (temp / 2.0, rec.width() / 2.0)
            } else {
                (rec.width() / 2.0, temp / 2.0)
            };
            results.last = Path::rect(
                Rect::new(
                    x - half_width,
                    y - half_height,
                    x + half_width,
                    y + half_height,
                ),
                None,
            );
        }

        debug_assert_eq!(cur_pt, results.points.len());

        true
    }

    // Port of: src/effects/SkDashPathEffect.cpp#L347-L349 (chrome/m156)
    fn as_a_dash(&self) -> Option<DashInfo> {
        Some(DashInfo {
            intervals: self.intervals.clone(),
            phase: self.phase,
        })
    }

    // Port of: src/effects/SkDashImpl.h#L33-L37 (chrome/m156)
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        // Dashing a path returns a subset of the input path so just return true and leave
        // bounds unmodified
        true
    }
}
