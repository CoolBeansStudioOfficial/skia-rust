// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.{h,cpp}.

//! The Raster Pipeline back end: the [`builder::Builder`] that assembles instructions, the
//! [`program::Program`] it finishes and lowers to stages, and the [`dumper`] that prints them.

pub mod builder;
pub mod ops;
pub mod program;

pub use builder::{Builder, Instruction, NA, Slot, SlotRange};
pub use ops::{BuilderOp, ProgramOp};
pub use program::{Addr, Program, Stage, StageCtx};
