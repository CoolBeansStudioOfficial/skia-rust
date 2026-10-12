// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper.h, modules/skshaper/src/
// SkShaper.cpp, SkShaper_primitive.cpp, SkShaper_factory.cpp, modules/skshaper/utils/
// FactoryHelpers.h.
//
// Milestones M5 and M6 of docs/design/modules.md. The HarfBuzz shapers (`SkShaper_harfbuzz.cpp`,
// `SkShaper_skunicode.cpp`) run on HarfRust (third_party/harfrust); `Shaper::new` and
// `factory::best_available` use them whenever there is a Unicode implementation.

// `HarfBuzz` and `HarfRust` are proper names, not code.
#![allow(clippy::doc_markdown)]

//! The `SkShaper` module: text shaping into runs of positioned glyphs.
//!
//! - [`Shaper`] is `SkShaper`. Its shapers are driven by [`RunIterator`]s (font, bidi, script,
//!   language) and report to a [`run_handler::RunHandler`].
//! - [`run_iterator`] holds the run iterators: the font manager fallback iterator, and the
//!   trivial ones that cover a whole text with one value.
//! - [`primitive`] is `SkShapers::Primitive`: one glyph per code point, no shaping, with
//!   line breaking at the advance widths.
//! - [`harfbuzz`] is `SkShapers::HB`: the HarfBuzz shapers on HarfRust, the script run iterator and
//!   the cache of faces. [`unicode_shaper`] is `SkShapers::unicode`: the bidi run iterator.
//! - [`factory`] holds `SkShapers::Factory` and `SkShapers::BestAvailable`.

pub mod factory;
pub mod harfbuzz;
pub mod primitive;
pub mod run_handler;
pub mod run_iterator;
pub mod shaper;
pub mod unicode_shaper;

pub use factory::{Factory, best_available, harfbuzz_factory};
pub use run_handler::{Buffer, RunHandler, RunInfo, TextBlobBuilderRunHandler};
pub use run_iterator::{
    BiDiRunIterator, FontRunIterator, LanguageRunIterator, RunIterator, ScriptRunIterator,
    TrivialBiDiRunIterator, TrivialFontRunIterator, TrivialLanguageRunIterator,
    TrivialScriptRunIterator,
};
pub use shaper::{Feature, Options, Shaper, ShaperBackend};
