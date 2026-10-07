// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The raster pipeline's CPU code (`src/opts/SkRasterPipeline_opts.h`,
//! `SkRasterPipelineOpList.h`, `SkRasterPipelineOpContexts.h`), see
//! `docs/design/raster-pipeline.md`.
//!
//! - [`lanes`]: lane types and the per-tier primitives (design §2.4, tasks A2a, A2b, A2c).
//! - [`Op`] and [`Stage`] ([`ops`]): the op list and the typed program, generated from one
//!   table in Skia's order; [`contexts`]: the op contexts.
//! - [`Program`]: a pipeline built for a tier (lowp or highp), run with [`MemoryBindings`].
//! - `tiers` (private): the stage code, written once and stamped per tier, and the
//!   interpreters (design §2.5, §2.6).
//!
//! # Running a pipeline
//! ```
//! use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage};
//!
//! // Two stages: seed_shader puts each pixel's center in (r, g); store_src writes r,g,b,a.
//! let out = MemPtr::new(MemSlot(0), 0);
//! let mut program =
//!     Program::new(&[Stage::SeedShader, Stage::StoreSrc(out)], skia_rust_simd::selection(), true);
//! let mut buf = [0u8; 4 * 4 * 16]; // room for 4 registers of up to 16 floats
//! program.run(3, 7, 1, 1, &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut buf)));
//! // Lane 0 of r is the pixel center x = 3.5.
//! assert_eq!(f32::from_ne_bytes(buf[..4].try_into().unwrap()), 3.5);
//! ```

pub mod contexts;
pub mod lanes;
mod memory;
pub mod ops;
mod program;
mod tiers;

pub use contexts::{MemPtr, MemSlot, MemoryCtx, MemoryCtxInfo};
pub use memory::{MemView, MemoryBindings, NO_TAIL, Params, memory_ctx_infos};
pub use ops::{NUM_HIGHP_OPS, NUM_LOWP_OPS, Op, Stage};
pub use program::{Program, has_lowp};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_wide;
