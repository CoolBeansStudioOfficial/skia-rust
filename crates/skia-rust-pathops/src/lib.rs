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
pub mod op_add_intersections;
pub mod op_angle;
pub mod op_coincidence;
pub mod op_common;
pub mod op_contour;
pub mod op_curve;
pub mod op_edge_builder;
mod op_op;
pub mod op_segment;
mod op_simplify;
pub mod op_span;
pub mod op_state;
pub mod op_winding;
pub mod path_op;
pub mod path_writer;
pub mod point;
pub mod quad;
pub mod rect;
pub mod reduce_order;
pub mod t_curve;
pub mod t_sect;
pub mod t_span;
pub mod types;

pub use op_op::op;
pub use op_simplify::simplify;
