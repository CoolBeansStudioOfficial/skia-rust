// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The raster pipeline's CPU code (`src/opts/SkRasterPipeline_opts.h`), see
//! `docs/design/raster-pipeline.md`.
//!
//! - [`lanes`]: lane types and the per-tier primitives (design §2.4, tasks A2a, A2b, A2c).
//!
//! Ops, contexts, stages and the interpreters arrive with task A3.

pub mod lanes;
