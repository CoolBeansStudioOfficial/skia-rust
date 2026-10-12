// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper_factory.h,
// modules/skshaper/src/SkShaper_factory.cpp, modules/skshaper/utils/FactoryHelpers.h.

//! `SkShapers::Factory`: the callbacks a client gives to build shapers and their run iterators,
//! and `BestAvailable`, which picks the factory for the shapers compiled in.

use std::fmt;
use std::sync::Arc;

use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_types::FourByteTag;
use skia_rust_unicode::unicode::Unicode;

use crate::primitive::primitive_text;
use crate::run_iterator::{
    BiDiRunIterator, ScriptRunIterator, TrivialBiDiRunIterator, TrivialScriptRunIterator,
};
use crate::shaper::Shaper;

// Port of: modules/skshaper/include/SkShaper_factory.h#L19-L36 (chrome/m156), `SkShapers::Factory`.
/// `SkShapers::Factory`: makes shapers and the run iterators they use.
#[doc(alias = "SkShapers::Factory")]
pub trait Factory: fmt::Debug + Send + Sync {
    /// `makeShaper(sk_sp<SkFontMgr> fallback)`.
    #[doc(alias = "makeShaper")]
    fn make_shaper(&self, fallback: Option<FontMgr>) -> Shaper;

    /// `makeBidiRunIterator(utf8, utf8Bytes, bidiLevel)`.
    #[doc(alias = "makeBidiRunIterator")]
    fn make_bidi_run_iterator(&self, utf8: &[u8], bidi_level: u8) -> Box<dyn BiDiRunIterator>;

    /// `makeScriptRunIterator(utf8, utf8Bytes, script)`.
    #[doc(alias = "makeScriptRunIterator")]
    fn make_script_run_iterator(
        &self,
        utf8: &[u8],
        script: FourByteTag,
    ) -> Box<dyn ScriptRunIterator>;

    /// `getUnicode()`: the Unicode services of the factory, if it has any.
    #[doc(alias = "getUnicode")]
    fn unicode(&self) -> Option<&dyn Unicode>;
}

// Port of: modules/skshaper/src/SkShaper_factory.cpp#L10-L33 (chrome/m156), the anonymous
// `PrimitiveFactory`.
#[derive(Debug, Default)]
struct PrimitiveFactory;

impl Factory for PrimitiveFactory {
    fn make_shaper(&self, _fallback: Option<FontMgr>) -> Shaper {
        primitive_text()
    }

    // The primitive shaper ignores the bidi and script of the text, so the trivial iterators are
    // built with empty data.
    fn make_bidi_run_iterator(&self, _utf8: &[u8], _bidi_level: u8) -> Box<dyn BiDiRunIterator> {
        Box::new(TrivialBiDiRunIterator::new(0, 0))
    }

    fn make_script_run_iterator(
        &self,
        _utf8: &[u8],
        _script: FourByteTag,
    ) -> Box<dyn ScriptRunIterator> {
        Box::new(TrivialScriptRunIterator::new(0, 0))
    }

    fn unicode(&self) -> Option<&dyn Unicode> {
        None
    }
}

// Port of: modules/skshaper/src/SkShaper_factory.cpp#L35-L37 (chrome/m156),
// `SkShapers::Primitive::Factory`.
/// `SkShapers::Primitive::Factory()`: the factory of the primitive shaper.
#[must_use]
#[doc(alias = "Factory")]
pub fn primitive_factory() -> Arc<dyn Factory> {
    Arc::new(PrimitiveFactory)
}

// Port of: modules/skshaper/utils/FactoryHelpers.h#L107-L125 (chrome/m156), `BestAvailable`.
// Skia returns the HarfBuzz factory when HarfBuzz and SkUnicode are compiled in, the CoreText
// factory on Apple platforms, and the primitive factory otherwise. The HarfBuzz factory is
// milestone M6, so the primitive factory is the result for now. That is the one place where the
// choice of the shaper changes when M6 lands.
/// `SkShapers::BestAvailable()`.
#[must_use]
#[doc(alias = "BestAvailable")]
pub fn best_available() -> Arc<dyn Factory> {
    primitive_factory()
}
