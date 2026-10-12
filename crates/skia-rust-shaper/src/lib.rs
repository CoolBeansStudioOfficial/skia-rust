// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper.h, modules/skshaper/src/
// SkShaper.cpp, SkShaper_primitive.cpp, SkShaper_factory.cpp, modules/skshaper/utils/
// FactoryHelpers.h.
//
// Milestone M5 of docs/design/modules.md. The HarfBuzz shapers (`SkShaper_harfbuzz.cpp`,
// `SkShaper_skunicode.cpp`) are milestone M6: until they land, `Shaper::new` and
// `factory::best_available` return the primitive shaper, and the run iterators that M6 replaces
// (the bidi and script iterators of the unicode and HarfBuzz paths) return their trivial forms.
// Each such place says so where it is defined.

//! The `SkShaper` module: text shaping into runs of positioned glyphs.
//!
//! - [`Shaper`] is `SkShaper`. Its shapers are driven by [`RunIterator`]s (font, bidi, script,
//!   language) and report to a [`run_handler::RunHandler`].
//! - [`run_iterator`] holds the run iterators: the font manager fallback iterator, and the
//!   trivial ones that cover a whole text with one value.
//! - [`primitive`] is `SkShapers::Primitive`: one glyph per code point, no shaping, with
//!   line breaking at the advance widths.
//! - [`factory`] holds `SkShapers::Factory` and `SkShapers::BestAvailable`.

pub mod factory;
pub mod primitive;
pub mod run_handler;
pub mod run_iterator;
pub mod shaper;

pub use factory::{Factory, best_available};
pub use run_handler::{Buffer, RunHandler, RunInfo, TextBlobBuilderRunHandler};
pub use run_iterator::{
    BiDiRunIterator, FontRunIterator, LanguageRunIterator, RunIterator, ScriptRunIterator,
    TrivialBiDiRunIterator, TrivialFontRunIterator, TrivialLanguageRunIterator,
    TrivialScriptRunIterator,
};
pub use shaper::{Feature, Options, Shaper, ShaperBackend};
