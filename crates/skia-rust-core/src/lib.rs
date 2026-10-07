//! Core types of skia-rust, ported from Skia's `include/core` and `src/core`.

pub mod align;
pub mod checksum;
pub mod color;
pub mod color_data;
#[doc(hidden)]
pub mod color_priv;
pub mod endian;
pub mod fixed;
pub mod floating_point;
pub mod half;
pub mod math;
#[doc(hidden)]
pub mod math_priv;
pub mod random;
pub mod safe32;
pub mod safe_math;
pub mod scalar;
pub mod t_fits_in;
pub mod t_pin;
pub mod to;
pub mod un_pre_multiply;
pub mod utf;
