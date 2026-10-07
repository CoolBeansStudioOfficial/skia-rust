//! SIMD primitives and per-CPU-tier kernels for skia-rust.
//!
//! This is the only crate in the workspace allowed to contain `unsafe` code.
//! Rules (see `docs/UNSAFE.md`): one operation per `unsafe` block, each with a
//! `// SAFETY:` comment; every SIMD kernel has a scalar twin that must produce
//! bit-identical results; the public API is entirely safe.
#![allow(unsafe_code)]

mod tier;
pub mod vx;

pub use tier::Tier;
