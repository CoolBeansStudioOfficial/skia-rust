// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The C library math functions, computed exactly as the oracle's C runtime computes them.
//!
//! Skia calls `sinf`, `cosf`, `atan2f`, `sin`, … from the platform C library, which on the oracle host
//! (Windows 11 x64, clang-cl) is the Universal CRT (`ucrtbase.dll`). Those functions are not
//! correctly rounded, so `f32::sin` & co. (glibc, Apple's libm, wasm's musl port) give different
//! last bits, and a one-ulp difference in a conic control point moves anti-aliased coverage. Every
//! ported libm call goes through this module instead: pure Rust, identical on every host, and bit for
//! bit what the UCRT's x64 FMA3 code path returns (CI checks this against the real UCRT on Windows).
//!
//! `sqrt`, `floor`, `ceil`, `trunc`, `fabs`, `copysign` and `fmod` are exact in IEEE 754 and stay on
//! `f32`/`f64`. See `docs/design/math.md` and `docs/PORTING.md` §5.

mod atan2f;
mod exp_log_f32;
mod exp_log_f64;
mod inv_trig_f32;
mod misc_f64;
mod pow_f64;
mod powf;
mod reduce;
mod tables;
mod trig_f32;
mod trig_f64;

pub use atan2f::atan2f;
pub use exp_log_f32::{expf, log2f, logf};
pub use exp_log_f64::{exp, log};
pub use inv_trig_f32::{acosf, asinf, atanf};
pub use misc_f64::{acos, cbrt, cbrtf};
pub use pow_f64::{exp2, pow};
pub use powf::powf;
pub use trig_f32::{cosf, sinf, tanf};
pub use trig_f64::{cos, sin};
