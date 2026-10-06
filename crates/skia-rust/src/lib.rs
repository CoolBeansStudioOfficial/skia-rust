//! A faithful port of [Skia](https://skia.org) to safe, idiomatic Rust.
//!
//! This crate is the facade: it re-exports the internal `skia-rust-*` crates
//! behind feature flags. See `docs/PLAN.md` for the project plan.

pub use skia_rust_core as core;
