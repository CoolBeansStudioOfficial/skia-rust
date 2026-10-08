//! Boolean path operations of skia-rust, ported from Skia's `src/pathops`.
//!
//! So far: the numeric core (epsilons and ULPS comparisons, double-precision points, lines,
//! quads, conics, cubics, curves, rects and bounds) and the line/curve intersection routines.
//! The T-intersection code and the op graph follow.

pub mod conic;
pub mod cubic;
pub mod curve;
pub mod d_cubic_line_intersection;
pub mod d_line_intersection;
pub mod intersections;
pub mod line;
pub mod line_curve_intersection;
pub mod line_parameters;
pub mod point;
pub mod quad;
pub mod rect;
pub mod reduce_order;
pub mod t_curve;
pub mod types;
