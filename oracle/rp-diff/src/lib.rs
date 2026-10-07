// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `oracle/rp-diff`: the per-stage raster pipeline oracle (`docs/design/raster-pipeline.md`
//! §4.2).
//!
//! GMs exercise raster pipeline stages only in combination and rarely hit edge cases; rp-diff runs
//! single stages and short stage sequences on chosen inputs through real Skia (every x86 code
//! path) and through skia-rust (every [`Selection`](skia_rust_simd::tier::Selection) that can
//! stand in for that path), and compares the bytes.
//!
//! - [`case`]: a case (buffers, stages with serialized contexts, run rects) and its text form,
//!   read by the C++ driver (`cpp/driver.cpp`, which builds the pipeline with Skia's own
//!   `SkRasterPipeline` on the tier's instantiation of `SkRasterPipeline_opts.h`, `cpp/tier.cpp`).
//! - [`cases`]: the case list ([`cases::all`]), generated deterministically; Wave B adds its
//!   stages' cases there.
//! - [`replay`]: runs a case through `skia_rust_simd::rp`.
//! - [`expected`]: Skia's results, stored per tier as hashes in `expected/<tier>.txt`, and the
//!   check against them that `cargo test` runs on every host (no C++ needed).
//!
//! `cargo xtask oracle rp-diff` (oracle host) builds the driver, runs the cases through Skia,
//! compares byte for byte with skia-rust (reporting the first differing bytes), and with
//! `--update` rewrites the stored results.

pub mod case;
pub mod cases;
pub mod expected;
pub mod replay;

pub use case::{Buffer, Case, Ctx, Rect, StageSpec};
