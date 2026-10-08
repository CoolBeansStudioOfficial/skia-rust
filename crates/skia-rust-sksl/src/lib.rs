// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl (see docs/design/sksl.md for the crate's scope).

//! The `SkSL` compiler of skia-rust (design: `docs/design/sksl.md`).
//!
//! So far: the Raster Pipeline builder ([`codegen::rp::builder`]), the program it finishes
//! ([`codegen::rp::program`]) and its `.skrp` dumper ([`codegen::rp::dumper`]), the compiler's
//! support code (number formatting and parsing, `printf`, the hash containers, output streams),
//! the embedded built-in modules ([`modules`]), the lexer, and the IR core ([`ir`]: pools and
//! ids, every node, `description()` and `clone()`, with [`context::Context`], the error
//! reporter, the mangler and the visitor/writer traversals). The parser, the IR conversions,
//! the optimizer, the stage-binding `appendStages`, and the other back ends come in later tasks.

pub mod analysis;
mod base_helpers;
pub mod builtin_types;
pub mod codegen;
pub mod compiler;
pub mod constant_folder;
pub mod context;
pub mod defines;
pub mod error_reporter;
pub mod flavor;
pub mod intrinsic_list;
pub mod ir;
pub mod lexer;
pub mod mangler;
mod matrix_invert;
pub mod memory_layout;
pub mod modules;
pub mod operator;
pub mod output_stream;
pub mod position;
pub mod program_settings;
pub mod skstd;
pub mod string;
pub mod thash;
pub mod tracing;
pub mod transform;
pub mod util;

pub use flavor::{Flavor, ModuleSource};
