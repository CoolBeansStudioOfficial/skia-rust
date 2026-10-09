// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/LinearTolerances.h (chrome/m156)

//! `LinearTolerances` stores state to approximate the final device-space transform applied to
//! curves, and uses that to calculate segmentation levels for both the parametric curves and
//! radial components (when stroking, where you have to represent the offset of a curve).
//!
//! - segments = a linear subsection on the curve, either defined as parametric (linear in t) or
//!   radial (linear in curve's internal rotation).
//! - edges = orthogonal geometry to segments, used in stroking to offset from the central curve
//!   by half the stroke width, or to construct the join geometry.

use std::f32::consts::PI;

use skia_rust_core::scalar::scalar_ceil_to_int;

use crate::tessellate::tessellation::{
    StrokeParams, calc_num_radial_segments_per_radian, num_fixed_edges_in_join_params,
};
use crate::tessellate::wangs_formula;

/// Worst-case segmentation tolerances accumulated over a set of curves.
// Port of: src/gpu/tessellate/LinearTolerances.h#L26-L131 (chrome/m156), class `LinearTolerances`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::struct_field_names)] // the C++ members are all named f*
pub struct LinearTolerances {
    // Used for both fills and strokes, always at least one parametric segment.
    f_num_parametric_segments_p4: f32,
    // Used for strokes, adding additional segments along the curve to account for its rotation.
    f_num_radial_segments_per_radian: f32,
    // Used for strokes, tracking the number of additional vertices required to handle joins
    // based on the join type and stroke width.
    f_edges_in_joins: i32,
}

impl Default for LinearTolerances {
    // Member initializers of the C++ class.
    fn default() -> Self {
        Self {
            f_num_parametric_segments_p4: 1.0,
            f_num_radial_segments_per_radian: 0.0,
            f_edges_in_joins: 0,
        }
    }
}

impl LinearTolerances {
    /// `numParametricSegments_p4()`.
    #[must_use]
    pub fn num_parametric_segments_p4(&self) -> f32 {
        self.f_num_parametric_segments_p4
    }

    /// `numRadialSegmentsPerRadian()`.
    #[must_use]
    pub fn num_radial_segments_per_radian(&self) -> f32 {
        self.f_num_radial_segments_per_radian
    }

    /// `numEdgesInJoins()`.
    #[must_use]
    pub fn num_edges_in_joins(&self) -> i32 {
        self.f_edges_in_joins
    }

    /// Fast log2 of minimum required # of segments per tracked Wang's formula calculations.
    // Port of: src/gpu/tessellate/LinearTolerances.h#L36-L40 (chrome/m156), `requiredResolveLevel`.
    #[must_use]
    pub fn required_resolve_level(&self) -> i32 {
        // log16(n^4) == log2(n)
        wangs_formula::nextlog16(self.f_num_parametric_segments_p4)
    }

    /// The worst-case number of edges an instance needs in a stroke.
    // Port of: src/gpu/tessellate/LinearTolerances.h#L41-L76 (chrome/m156), `requiredStrokeEdges`.
    #[must_use]
    pub fn required_stroke_edges(&self) -> i32 {
        // The maximum rotation we can have in a stroke is 180 degrees (SK_ScalarPI radians).
        // NOTE: This is also sufficient to handle circular caps because the shader sweeps a
        // stroke width line 180 degrees about the center point.
        let max_radial_segments_in_stroke = std::cmp::max(
            scalar_ceil_to_int(self.f_num_radial_segments_per_radian * PI),
            1,
        );
        let max_parametric_segments_in_stroke =
            scalar_ceil_to_int(wangs_formula::root4(self.f_num_parametric_segments_p4));
        debug_assert!(max_parametric_segments_in_stroke >= 1);

        // Now calculate the maximum number of edges we will need in the stroke portion of the
        // instance. The first and last edges in a stroke are shared by both the parametric and
        // radial sets of edges, so the total number of edges is:
        //
        //   numCombinedEdges = numParametricEdges + numRadialEdges - 2
        //
        // It's important to differentiate between the number of edges and segments in a strip:
        //
        //   numSegments = numEdges - 1
        //
        // So the total number of combined edges in the stroke is:
        //
        //   numEdgesInStroke = numParametricSegments + 1 + numRadialSegments + 1 - 2
        //                    = numParametricSegments + numRadialSegments
        let max_edges_in_stroke = max_radial_segments_in_stroke + max_parametric_segments_in_stroke;

        // Each triangle strip has two sections: It starts with a join then transitions to a
        // stroke. The number of edges in an instance is the sum of edges from the join and stroke
        // sections both.
        // NOTE: The final join edge and the first stroke edge are co-located, however we still
        // need to emit both because the join's edge is half-width and the stroke is full-width.
        self.f_edges_in_joins + max_edges_in_stroke
    }

    /// `setParametricSegments(float n4)`.
    // Port of: src/gpu/tessellate/LinearTolerances.h#L78-L81 (chrome/m156), `setParametricSegments`.
    pub fn set_parametric_segments(&mut self, n4: f32) {
        debug_assert!(n4 >= 0.0);
        self.f_num_parametric_segments_p4 = n4;
    }

    /// `setStroke(const StrokeParams&, float maxScale)`.
    // Port of: src/gpu/tessellate/LinearTolerances.h#L83-L98 (chrome/m156), `setStroke`.
    pub fn set_stroke(&mut self, stroke_params: &StrokeParams, max_scale: f32) {
        #[allow(clippy::float_cmp)] // SkStrokeParams compares the radius with == 0
        let approx_dev_stroke_radius = if stroke_params.radius == 0.0 {
            // Hairlines are always 1 px wide.
            0.5
        } else {
            // Approximate max scale * local stroke width / 2.
            stroke_params.radius * max_scale
        };

        self.f_num_radial_segments_per_radian =
            calc_num_radial_segments_per_radian(approx_dev_stroke_radius);
        self.f_edges_in_joins = num_fixed_edges_in_join_params(stroke_params);
        if stroke_params.join_type < 0.0 && self.f_num_radial_segments_per_radian > 0.0 {
            // For round joins we need to count the radial edges on our own. Account for a
            // worst-case join of 180 degrees (SK_ScalarPI radians).
            self.f_edges_in_joins +=
                scalar_ceil_to_int(self.f_num_radial_segments_per_radian * PI) - 1;
        }
    }

    /// `accumulate(const LinearTolerances&)`: keeps the worst case of each tolerance.
    // Port of: src/gpu/tessellate/LinearTolerances.h#L100-L112 (chrome/m156), `accumulate`.
    pub fn accumulate(&mut self, tolerances: &LinearTolerances) {
        if tolerances.f_num_parametric_segments_p4 > self.f_num_parametric_segments_p4 {
            self.f_num_parametric_segments_p4 = tolerances.f_num_parametric_segments_p4;
        }
        if tolerances.f_num_radial_segments_per_radian > self.f_num_radial_segments_per_radian {
            self.f_num_radial_segments_per_radian = tolerances.f_num_radial_segments_per_radian;
        }
        if tolerances.f_edges_in_joins > self.f_edges_in_joins {
            self.f_edges_in_joins = tolerances.f_edges_in_joins;
        }
    }
}
