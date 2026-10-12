//! Pure-Rust elementary functions whose results are bit-identical to the Windows x64 UCRT.
//!
//! Skia's expected images (the oracle goldens) were produced on Windows, whose C runtime does not
//! return correctly rounded results; Rust's `std` forwards to whatever libm the host has. This
//! crate gives the ported code one implementation that agrees with the oracle on every host.
//!
//! Provenance of every function (public algorithm it starts from and the black-box measurements
//! that fixed each choice) is in `docs/design/libm-cleanroom.md`.

// This crate is numerics: the integer/float casts below are the point (bit manipulation of IEEE
// values and the final double-to-float rounding), not accidents.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::float_cmp
)]

mod atan;
mod atan2;
mod reduce;
mod tan;
mod trig;

pub use atan::atanf;
pub use atan2::atan2f;
pub use tan::tanf;
#[doc(hidden)]
pub use tan::tanf_trace;
pub use trig::{cosf, sinf};
#[doc(hidden)]
pub use trig::{probe_cosf, probe_sinf};
