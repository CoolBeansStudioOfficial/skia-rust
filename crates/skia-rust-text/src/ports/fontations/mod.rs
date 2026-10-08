//! The Fontations backend: a Rust port of `src/ports/fontations/src/*.rs`, the bridge that
//! Skia calls through `cxx`, with the FFI replaced by a plain Rust API (`docs/design/text.md` §2.2),
//! and the typeface, scanner and empty font manager built on it.

pub mod base;
pub mod colr;
pub mod font_mgr;
pub mod font_scanner;
pub mod hinting;
pub mod names;
pub mod pen;
pub mod scaler_context;
pub mod typeface;
