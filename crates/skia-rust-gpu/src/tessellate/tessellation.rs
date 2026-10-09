// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/Tessellation.h (chrome/m156)

//! Shared constants and helpers of path tessellation: the precision and segment limits, the
//! patch attribute layout, and the stroke parameters that go into each patch.
//!
//! `PreChopPathCurves` and `FindCubicConvex180Chops` (`Tessellation.cpp`) are not ported yet.

use bitflags::bitflags;
use skia_rust_core::paint::Join;
use skia_rust_core::point::Point;
use skia_rust_core::stroke_rec::StrokeRec;

/// Don't allow linearized segments to be off by more than 1/4th of a pixel from the true curve.
// Port of: src/gpu/tessellate/Tessellation.h#L26-L27 (chrome/m156), `kPrecision`.
pub const K_PRECISION: f32 = 4.0;

/// This is the maximum number of subdivisions of a Bezier curve that can be represented in the
/// fixed count vertex and index buffers. If rendering a curve that requires more subdivisions, it
/// must be chopped.
// Port of: src/gpu/tessellate/Tessellation.h#L30-L31 (chrome/m156), `kMaxResolveLevel`.
pub const K_MAX_RESOLVE_LEVEL: i32 = 5;

/// This is the maximum number of parametric segments (linear sections) that a curve can be split
/// into. This is the same for path filling and stroking, although fixed-count stroking also uses
/// additional vertices to handle radial segments, joins, and caps.
// Port of: src/gpu/tessellate/Tessellation.h#L38-L40 (chrome/m156), `kMaxParametricSegments`.
pub const K_MAX_PARAMETRIC_SEGMENTS: i32 = 1 << K_MAX_RESOLVE_LEVEL;
// Port of: src/gpu/tessellate/Tessellation.h#L41 (chrome/m156), `kMaxParametricSegments_p2`.
pub const K_MAX_PARAMETRIC_SEGMENTS_P2: i32 = K_MAX_PARAMETRIC_SEGMENTS * K_MAX_PARAMETRIC_SEGMENTS;
// Port of: src/gpu/tessellate/Tessellation.h#L42-L43 (chrome/m156), `kMaxParametricSegments_p4`.
pub const K_MAX_PARAMETRIC_SEGMENTS_P4: i32 =
    K_MAX_PARAMETRIC_SEGMENTS_P2 * K_MAX_PARAMETRIC_SEGMENTS_P2;

/// Don't tessellate paths that might have an individual curve that requires more than 1024
/// segments. (See `wangs_formula::worst_case_cubic`.)
// Port of: src/gpu/tessellate/Tessellation.h#L49 (chrome/m156), `kMaxSegmentsPerCurve`.
pub const K_MAX_SEGMENTS_PER_CURVE: f32 = 1024.0;
// Port of: src/gpu/tessellate/Tessellation.h#L50 (chrome/m156), `kMaxSegmentsPerCurve_p2`.
pub const K_MAX_SEGMENTS_PER_CURVE_P2: f32 = K_MAX_SEGMENTS_PER_CURVE * K_MAX_SEGMENTS_PER_CURVE;
// Port of: src/gpu/tessellate/Tessellation.h#L51 (chrome/m156), `kMaxSegmentsPerCurve_p4`.
pub const K_MAX_SEGMENTS_PER_CURVE_P4: f32 =
    K_MAX_SEGMENTS_PER_CURVE_P2 * K_MAX_SEGMENTS_PER_CURVE_P2;

/// How many triangles are in a curve with `2^resolveLevel` line segments? Resolve level defines
/// the tessellation factor for filled paths drawn using curves or wedges.
// Port of: src/gpu/tessellate/Tessellation.h#L63-L70 (chrome/m156), `NumCurveTrianglesAtResolveLevel`.
#[must_use]
pub const fn num_curve_triangles_at_resolve_level(resolve_level: i32) -> i32 {
    // resolveLevel=0 -> 0 line segments -> 0 triangles
    // resolveLevel=1 -> 2 line segments -> 1 triangle
    // resolveLevel=2 -> 4 line segments -> 3 triangles
    // resolveLevel=3 -> 8 line segments -> 7 triangles
    (1 << resolve_level) - 1
}

bitflags! {
    /// Optional attribs that are included in tessellation patches, following the control points
    /// and in the same order as they appear here.
    // Port of: src/gpu/tessellate/Tessellation.h#L76-L92 (chrome/m156), `PatchAttribs`.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PatchAttribs: u32 {
        /// `kNone`.
        const NONE = 0;
        /// `[float2]` Used by strokes. This defines tangent direction.
        const JOIN_CONTROL_POINT = 1 << 0;
        /// `[float2]` Used by wedges. This is the center point the wedges fan around.
        const FAN_POINT = 1 << 1;
        /// `[float2]` Used when strokes have different widths or join types.
        const STROKE_PARAMS = 1 << 2;
        /// `[ubyte4 or float4]` Used when patches have different colors.
        const COLOR = 1 << 3;
        /// `[float]` Used in Graphite to specify depth attachment value for draw.
        const PAINT_DEPTH = 1 << 4;
        /// `[float]` Used when GPU can't infer curve type based on infinity.
        const EXPLICIT_CURVE_TYPE = 1 << 5;
        /// Extra flag: if `COLOR` is set, specifies it to be float4 wide color.
        const WIDE_COLOR_IF_ENABLED = 1 << 6;
        /// `[int]` Used to index into a shared storage buffer for this patch's uniform values.
        const SSBO_INDEX = 1 << 7;
    }
}

/// When `PatchAttribs::EXPLICIT_CURVE_TYPE` is set, these are the values that tell the GPU what
/// type of curve is being drawn.
// Port of: src/gpu/tessellate/Tessellation.h#L95-L97 (chrome/m156), `kCubicCurveType`.
pub const K_CUBIC_CURVE_TYPE: f32 = 0.0;
// Port of: src/gpu/tessellate/Tessellation.h#L95-L97 (chrome/m156), `kConicCurveType`.
pub const K_CONIC_CURVE_TYPE: f32 = 1.0;
// Port of: src/gpu/tessellate/Tessellation.h#L95-L97 (chrome/m156), `kTriangularConicCurveType`.
/// Conic curve with `w = Inf`.
pub const K_TRIANGULAR_CONIC_CURVE_TYPE: f32 = 2.0;

/// Returns the packed size in bytes of the attribs portion of tessellation patches (or instances)
/// in GPU buffers.
// Port of: src/gpu/tessellate/Tessellation.h#L101-L113 (chrome/m156), `PatchAttribsStride`.
#[must_use]
pub fn patch_attribs_stride(attribs: PatchAttribs) -> usize {
    let f32_size = size_of::<f32>();
    (if attribs.contains(PatchAttribs::JOIN_CONTROL_POINT) {
        f32_size * 2
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::FAN_POINT) {
        f32_size * 2
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::STROKE_PARAMS) {
        f32_size * 2
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::COLOR) {
        (if attribs.contains(PatchAttribs::WIDE_COLOR_IF_ENABLED) {
            f32_size
        } else {
            size_of::<u8>()
        }) * 4
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::PAINT_DEPTH) {
        f32_size
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::EXPLICIT_CURVE_TYPE) {
        f32_size
    } else {
        0
    }) + (if attribs.contains(PatchAttribs::SSBO_INDEX) {
        size_of::<u32>()
    } else {
        0
    })
}

/// Returns the packed size in bytes of one patch: four control points, then the attribs.
// Port of: src/gpu/tessellate/Tessellation.h#L115-L117 (chrome/m156), `PatchStride`.
#[must_use]
pub fn patch_stride(attribs: PatchAttribs) -> usize {
    4 * size_of::<Point>() + patch_attribs_stride(attribs)
}

/// Returns true if the given conic (or quadratic) has a cusp point. The `w` value is not necessary
/// in determining this. If there is a cusp, it can be found at the midtangent.
// Port of: src/gpu/tessellate/Tessellation.h#L129-L139 (chrome/m156), `ConicHasCusp`.
#[must_use]
pub fn conic_has_cusp(p: &[Point]) -> bool {
    let a = p[1] - p[0];
    let b = p[2] - p[1];
    // A conic of any class can only have a cusp if it is a degenerate flat line with a 180 degree
    // turnaround. To detect this, the beginning and ending tangents must be parallel
    // (a.cross(b) == 0) and pointing in opposite directions (a.dot(b) < 0).
    a.cross(b) == 0.0 && a.dot(b) < 0.0
}

/// We encode all of a join's information in a single float value:
///
/// - Negative: round join.
/// - Zero: bevel join.
/// - Positive: miter join, and the value is also the miter limit.
// Port of: src/gpu/tessellate/Tessellation.h#L145-L155 (chrome/m156), `GetJoinType`.
#[must_use]
pub fn get_join_type(stroke: &StrokeRec) -> f32 {
    match stroke.join() {
        Join::Round => -1.0,
        Join::Bevel => 0.0,
        Join::Miter => {
            debug_assert!(stroke.miter() >= 0.0);
            stroke.miter()
        }
    }
}

/// This float2 gets written out with each patch/instance if `PatchAttribs::STROKE_PARAMS` is
/// enabled.
// Port of: src/gpu/tessellate/Tessellation.h#L157-L175 (chrome/m156), `StrokeParams`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StrokeParams {
    pub radius: f32,
    /// See [`get_join_type`].
    pub join_type: f32,
}

impl StrokeParams {
    /// `StrokeParams(float radius, float joinType)`.
    #[must_use]
    pub fn new(radius: f32, join_type: f32) -> Self {
        Self { radius, join_type }
    }

    /// `StrokeParams(const SkStrokeRec& stroke)`.
    #[must_use]
    pub fn from_stroke(stroke: &StrokeRec) -> Self {
        let mut params = Self::default();
        params.set(stroke);
        params
    }

    /// `StrokeParams::set(const SkStrokeRec& stroke)`.
    pub fn set(&mut self, stroke: &StrokeRec) {
        self.radius = stroke.width() * 0.5;
        self.join_type = get_join_type(stroke);
    }
}

/// Returns true if both strokes have the same width, join and (for miter joins) miter limit.
#[allow(clippy::float_cmp)]
// SkStrokesHaveEqualParams compares the widths and limits exactly
// Port of: src/gpu/tessellate/Tessellation.h#L177-L180 (chrome/m156), `StrokesHaveEqualParams`.
#[must_use]
pub fn strokes_have_equal_params(a: &StrokeRec, b: &StrokeRec) -> bool {
    a.width() == b.width()
        && a.join() == b.join()
        && (a.join() != Join::Miter || a.miter() == b.miter())
}

/// Returns the fixed number of edges that are always emitted with the given join type. If the
/// join is round, the caller needs to account for the additional radial edges on their own.
///
/// Each join always emits:
///
/// - Two colocated edges at the beginning (a full-width edge to seam with the preceding stroke and
///   a half-width edge to begin the join).
/// - An extra edge in the middle for miter joins, or else a variable number of radial edges for
///   round joins (the caller is responsible for counting radial edges from round joins).
/// - A half-width edge at the end of the join that will be colocated with the first (full-width)
///   edge of the stroke.
// Port of: src/gpu/tessellate/Tessellation.h#L195-L206 (chrome/m156), `NumFixedEdgesInJoin(SkPaint::Join)`.
#[must_use]
pub const fn num_fixed_edges_in_join(join_type: Join) -> i32 {
    match join_type {
        Join::Miter => 4,
        // The caller is responsible for counting the variable number of middle, radial segments
        // on round joins.
        Join::Round | Join::Bevel => 3,
    }
}

/// `NumFixedEdgesInJoin(const StrokeParams&)`: the caller is responsible for counting the variable
/// number of segments for round joins.
// Port of: src/gpu/tessellate/Tessellation.h#L208-L211 (chrome/m156), `NumFixedEdgesInJoin(const StrokeParams&)`.
#[must_use]
pub fn num_fixed_edges_in_join_params(stroke_params: &StrokeParams) -> i32 {
    // miter, else round or bevel
    if stroke_params.join_type > 0.0 { 4 } else { 3 }
}

/// Decides the number of radial segments the tessellator adds for each curve. (Uniform steps in
/// tangent angle.) The tessellator will add this number of radial segments for each radian of
/// rotation in local path space.
// Port of: src/gpu/tessellate/Tessellation.h#L213-L217 (chrome/m156), `CalcNumRadialSegmentsPerRadian`.
#[must_use]
pub fn calc_num_radial_segments_per_radian(approx_dev_stroke_radius: f32) -> f32 {
    let cos_theta = 1.0 - (1.0 / K_PRECISION) / approx_dev_stroke_radius;
    // std::max(cosTheta, -1.f)
    let clamped = if cos_theta < -1.0 { -1.0 } else { cos_theta };
    0.5 / clamped.acos()
}
