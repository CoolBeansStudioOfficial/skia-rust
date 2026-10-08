//! The Fontations backend: a Rust port of `src/ports/fontations/src/*.rs`, the bridge that
//! Skia calls through `cxx`, with the FFI replaced by a plain Rust API (`docs/design/text.md` §2.2).

pub mod base;
pub mod hinting;
pub mod names;
pub mod pen;
