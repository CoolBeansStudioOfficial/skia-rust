//! SIMD primitives and per-CPU-tier kernels for skia-rust.
//!
//! This is the only crate in the workspace allowed to contain `unsafe` code.
//! Rules (see `docs/UNSAFE.md`): one operation per `unsafe` block, each with a
//! `// SAFETY:` comment; every SIMD kernel has a scalar twin that must produce
//! bit-identical results; the public API is entirely safe.
//!
//! - [`tier`]: the CPU tiers ([`Tier`]), detection and [`Selection`] (design §2.2–§2.3).
//! - [`cpu`]: the port of `SkCpu` and the feature tokens that prove a tier can run.
//! - [`estimates`]: the host's `rcp`/`rsqrt` estimate instructions, fingerprints, tables and
//!   bit-exact software models (`estimates::amd_zen4`, `estimates::arm`).
//! - `testing` (feature `testing`): `force_tier`.
//! - [`rp`]: the raster pipeline's CPU code: lane types and per-tier primitives ([`rp::lanes`]).
//! - [`vx`](mod@vx): `skvx`.
//! - [`blit_row`], [`blit_mask`], [`memset`]: the standalone `SkOpts` kernels
//!   (`blit_row_s32a_opaque`, `blit_row_color32`, `blit_mask_d32_a8`, `memset16/32/64`,
//!   `rect_memset16/32/64`), per tier with scalar twins; [`color_util`] has the few
//!   `SkColorPriv.h` helpers they need.
//! - [`swizzle`]: the 8888 premul/unpremul/swap-RB swizzles of `SkOpts` (`SkSwizzler_opts.inc`).
#![allow(unsafe_code)]

pub mod blit_mask;
pub mod blit_row;
pub mod color_util;
pub mod cpu;
pub mod estimates;
pub mod half;
pub mod memset;
pub mod rp;
pub mod swizzle;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod tier;
pub mod vx;

pub use tier::{Backend, Estimates, Selection, Tier, Unsupported, selection, selection_is_forced};
