//! Ports of `skia/tests/*.cpp`, one module per Skia file (see the crate docs for naming).

#[cfg(test)]
pub mod checksum_test;
#[cfg(test)]
pub mod color_priv_test;
#[cfg(test)]
pub mod color_test;
#[cfg(test)]
pub mod float16_test;
#[cfg(test)]
pub mod floating_point_test;
#[cfg(test)]
pub mod hsv_round_trip_test;
#[cfg(test)]
pub mod math_test;
#[cfg(test)]
pub mod random_test;
#[cfg(test)]
pub mod safe_math_test;
#[cfg(test)]
pub mod sk_color4f_test;
#[cfg(test)]
pub mod sk_utf_test;
#[cfg(test)]
pub mod sk_vx_test;
