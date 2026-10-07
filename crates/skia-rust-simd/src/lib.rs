//! SIMD primitives and per-CPU-tier kernels for skia-rust.
//!
//! This is the only crate in the workspace allowed to contain `unsafe` code.
//! Rules (see `docs/UNSAFE.md`): one operation per `unsafe` block, each with a
//! `// SAFETY:` comment; every SIMD kernel has a scalar twin that must produce
//! bit-identical results; the public API is entirely safe.
//!
//! - [`tier`]: the CPU tiers ([`Tier`]), detection and [`Selection`] (design §2.2–§2.3).
//! - [`cpu`]: the port of `SkCpu` and the feature tokens that prove a tier can run.
//! - [`estimates`]: the host's `rcp`/`rsqrt` estimate instructions, fingerprints and tables.
//! - `testing` (feature `testing`): `force_tier`.
//! - [`vx`](mod@vx): `skvx`.
#![allow(unsafe_code)]

pub mod cpu;
pub mod estimates;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod tier;
pub mod vx;

pub use tier::{Backend, Estimates, Selection, Tier, Unsupported, selection};
