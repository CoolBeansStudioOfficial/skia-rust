//! Core types of skia-rust, ported from Skia's `include/core` and `src/core`.

pub mod align;
pub mod alpha_type;
pub mod bezier_curves;
pub mod checksum;
pub mod color;
pub mod color_data;
#[doc(hidden)]
pub mod color_priv;
pub mod color_space;
#[doc(hidden)]
pub mod color_space_priv;
pub mod color_space_xform_steps;
pub mod cubic_clipper;
pub mod cubic_map;
pub mod cubics;
pub mod endian;
pub mod fixed;
pub mod float_bits;
pub mod floating_point;
pub mod geometry;
pub mod half;
pub mod m44;
pub mod math;
#[doc(hidden)]
pub mod math_priv;
pub mod matrix;
pub mod matrix_invert;
#[doc(hidden)]
pub mod matrix_priv;
#[doc(hidden)]
pub mod matrix_utils;
pub mod point;
pub mod point3;
pub mod quads;
pub mod random;
pub mod rect;
pub mod region;
pub mod rrect;
pub mod safe32;
pub mod safe_math;
pub mod scalar;
pub mod size;
pub mod t_fits_in;
pub mod t_pin;
pub mod tessellation;
pub mod to;
pub mod un_pre_multiply;
pub mod utf;
