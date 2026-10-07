//! Core types of skia-rust, ported from Skia's `include/core` and `src/core`.

pub mod align;
pub mod checksum;
pub mod color;
pub mod color_data;
#[doc(hidden)]
pub mod color_priv;
pub mod endian;
pub mod fixed;
pub mod float_bits;
pub mod floating_point;
pub mod half;
pub mod math;
#[doc(hidden)]
pub mod math_priv;
pub mod point;
pub mod point3;
pub mod random;
pub mod rect;
pub mod rrect;
pub mod safe32;
pub mod safe_math;
pub mod scalar;
pub mod size;
pub mod t_fits_in;
pub mod t_pin;
pub mod to;
pub mod un_pre_multiply;
pub mod utf;
