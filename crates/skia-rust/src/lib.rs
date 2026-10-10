//! A faithful port of [Skia](https://skia.org) to safe, idiomatic Rust.
//!
//! This crate is the facade: it re-exports the internal `skia-rust-*` crates
//! behind feature flags. See `docs/PLAN.md` for the project plan.

pub use skia_rust_core as core;
pub use skia_rust_pathops as pathops;
pub use skia_rust_pathops::PathOpsExt;

/// The codec layer (`skia_safe::codec`), with its lazy-image generators.
#[cfg(feature = "codec")]
pub use skia_rust_codec as codec;

/// The PDF backend (`skia_safe::pdf`).
#[cfg(feature = "pdf")]
pub mod pdf;

/// `skia_safe::Document`: a sequence of pages drawn to canvases.
#[cfg(feature = "pdf")]
pub mod document;
#[cfg(feature = "pdf")]
pub use document::Document;

/// The image factories (`skia_safe::images`): the raster and lazy factories of core, and the
/// codec's `deferred_from_encoded_data`.
pub mod images {
    pub use skia_rust_core::images::*;

    #[cfg(feature = "codec")]
    pub use skia_rust_codec::images::*;
}
