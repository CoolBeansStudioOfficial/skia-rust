// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl (see docs/design/sksl.md for the crate's scope).

//! The `SkSL` compiler of skia-rust (design: `docs/design/sksl.md`).
//!
//! So far: the Raster Pipeline builder ([`codegen::rp::builder`]), the program it finishes
//! ([`codegen::rp::program`]) and its `.skrp` dumper ([`codegen::rp::dumper`]), the compiler's
//! support code (number formatting and parsing, `printf`, the hash containers, output streams), and
//! the embedded built-in modules ([`modules`]). The lexer, parser and IR, the stage-binding
//! `appendStages`, and the other back ends come in later tasks.

mod base_shim;
pub mod codegen;
pub mod flavor;
pub mod modules;
pub mod output_stream;
pub mod skstd;
pub mod string;
pub mod thash;
pub mod tracing;

pub use flavor::{Flavor, ModuleSource};
