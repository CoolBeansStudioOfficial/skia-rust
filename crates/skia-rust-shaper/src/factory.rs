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

use crate::harfbuzz::{script_run_iterator_with_script, shaper_driven_wrapper};
use crate::primitive::primitive_text;
use crate::run_iterator::{
    BiDiRunIterator, ScriptRunIterator, TrivialBiDiRunIterator, TrivialScriptRunIterator,
};
use crate::shaper::Shaper;
use crate::unicode_shaper::unicode_bidi_run_iterator;

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

// Port of: modules/skshaper/utils/FactoryHelpers.h#L37-L77 (chrome/m156), `BestAvailableUnicode`.
fn best_available_unicode() -> Option<Arc<dyn Unicode>> {
    // `SkUnicodes::ICU::Make()`, then ICU4X: both are the ICU4X-backed implementation here.
    skia_rust_unicode::unicode_icu::make().or_else(skia_rust_unicode::unicode_icu::make_icu4x)
}

// Port of: modules/skshaper/utils/FactoryHelpers.h#L79-L107 (chrome/m156), `HarfbuzzFactory`.
#[derive(Clone)]
struct HarfbuzzFactory {
    unicode: Arc<dyn Unicode>,
}

impl fmt::Debug for HarfbuzzFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarfbuzzFactory").finish_non_exhaustive()
    }
}

impl Factory for HarfbuzzFactory {
    fn make_shaper(&self, fallback: Option<FontMgr>) -> Shaper {
        shaper_driven_wrapper(self.unicode.clone(), fallback)
    }

    // `SkShapers::unicode::BidiRunIterator`, which is null for text that is not UTF-8 or when the
    // bidi iterator cannot be made. The trait cannot return null, so the text is then one run at
    // the given level.
    fn make_bidi_run_iterator(&self, utf8: &[u8], bidi_level: u8) -> Box<dyn BiDiRunIterator> {
        unicode_bidi_run_iterator(&*self.unicode, utf8, bidi_level)
            .unwrap_or_else(|| Box::new(TrivialBiDiRunIterator::new(bidi_level, utf8.len())))
    }

    fn make_script_run_iterator(
        &self,
        utf8: &[u8],
        script: FourByteTag,
    ) -> Box<dyn ScriptRunIterator> {
        script_run_iterator_with_script(utf8, script)
    }

    fn unicode(&self) -> Option<&dyn Unicode> {
        Some(&*self.unicode)
    }
}

/// `SkShapers::HarfbuzzFactory(unicode)`: the factory of the HarfBuzz shapers, with `unicode` or
/// the best available one. `None` if there is no Unicode.
#[must_use]
#[doc(alias = "HarfbuzzFactory")]
pub fn harfbuzz_factory(unicode: Option<Arc<dyn Unicode>>) -> Option<Arc<dyn Factory>> {
    let unicode = unicode.or_else(best_available_unicode)?;
    Some(Arc::new(HarfbuzzFactory { unicode }))
}

// Port of: modules/skshaper/utils/FactoryHelpers.h#L107-L125 (chrome/m156), `BestAvailable`: the
// HarfBuzz factory when SkUnicode is available (it always is here), else the primitive factory.
/// `SkShapers::BestAvailable()`.
#[must_use]
#[doc(alias = "BestAvailable")]
pub fn best_available() -> Arc<dyn Factory> {
    harfbuzz_factory(None).unwrap_or_else(primitive_factory)
}
