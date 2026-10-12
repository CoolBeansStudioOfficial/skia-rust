// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper.h (SkShaper, Feature,
// Options), modules/skshaper/src/SkShaper.cpp (SkShaper::Make, PurgeCaches).

//! `SkShaper`: the shaper, and the backend trait its implementations provide.

use std::fmt;

use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_types::FourByteTag;
use skia_rust_core::scalar::scalar;

use crate::primitive::PrimitiveShaper;
use crate::run_handler::RunHandler;
use crate::run_iterator::{
    BiDiRunIterator, FontRunIterator, LanguageRunIterator, ScriptRunIterator,
    TrivialBiDiRunIterator, TrivialLanguageRunIterator, TrivialScriptRunIterator,
    make_font_mgr_run_iterator,
};

// Port of: modules/skshaper/include/SkShaper.h#L240-L246 (chrome/m156), `SkShaper::Feature`.
/// `SkShaper::Feature`: an OpenType feature applied to the UTF-8 bytes `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkShaper::Feature")]
pub struct Feature {
    /// The four-byte feature tag.
    pub tag: FourByteTag,
    /// The feature value.
    pub value: u32,
    /// The offset of the first UTF-8 byte of the run the feature applies to.
    pub start: usize,
    /// The offset one past the last UTF-8 byte of the run the feature applies to.
    pub end: usize,
}

// Port of: modules/skshaper/include/SkShaper.h#L299-L302 (chrome/m156), `SkShaper::Options`.
/// `SkShaper::Options`: the line width and the tracking of a shape call.
#[derive(Debug, Clone, Copy, PartialEq)]
#[doc(alias = "SkShaper::Options")]
pub struct Options {
    /// The width of a line. Text wraps where a run would exceed it.
    pub width: f32,
    /// The tracking, in em units.
    pub tracking: f32,
}

impl Default for Options {
    // Port of: modules/skshaper/include/SkShaper.h#L300-L301 (chrome/m156): the default width is
    // `std::numeric_limits<float>::max()` and the tracking is 0.
    fn default() -> Self {
        Self {
            width: f32::MAX,
            tracking: 0.0,
        }
    }
}

/// The implementation of a shaper: the virtual `SkShaper::shape` of Skia.
pub trait ShaperBackend: fmt::Debug + Send + Sync {
    /// `SkShaper::shape(SkSpan<const char> utf8, FontRunIterator&, BiDiRunIterator&,
    /// ScriptRunIterator&, LanguageRunIterator&, SkSpan<const Feature>, const Options&,
    /// RunHandler*)`.
    #[allow(clippy::too_many_arguments)] // mirrors SkShaper::shape's parameters
    fn shape(
        &self,
        utf8: &[u8],
        font_runs: &mut dyn FontRunIterator,
        bidi_runs: &mut dyn BiDiRunIterator,
        script_runs: &mut dyn ScriptRunIterator,
        language_runs: &mut dyn LanguageRunIterator,
        features: &[Feature],
        options: &Options,
        handler: &mut dyn RunHandler,
    );
}

/// `SkShaper`: shapes UTF-8 text into runs of glyphs, reported to a [`RunHandler`].
#[doc(alias = "SkShaper")]
#[derive(Debug)]
pub struct Shaper {
    backend: Box<dyn ShaperBackend>,
}

impl Shaper {
    // Port of: modules/skshaper/src/SkShaper_primitive.cpp#L166-L168 (chrome/m156),
    // `SkShapers::Primitive::PrimitiveText`.
    /// `SkShapers::Primitive::PrimitiveText()`: the primitive shaper.
    #[must_use]
    #[doc(alias = "MakePrimitive")]
    pub fn new_primitive() -> Self {
        Self {
            backend: Box::new(PrimitiveShaper),
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L26-L38 (chrome/m156), `SkShaper::Make`. In the
    // oracle build the HarfBuzz shaper is tried first, with the font manager as fallback. The
    // HarfBuzz shapers are milestone M6, so the primitive shaper is the result for now (the same
    // result as Skia without HarfBuzz). `fallback` is kept for the HarfBuzz path.
    /// `SkShaper::Make(sk_sp<SkFontMgr> fallback)`.
    #[must_use]
    #[doc(alias = "Make")]
    #[allow(clippy::needless_pass_by_value)] // the HarfBuzz shaper (M6) takes the font manager
    pub fn new(fallback: Option<FontMgr>) -> Self {
        let _ = fallback; // Used by the HarfBuzz shaper (M6).
        Self::new_primitive()
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L40-L44 (chrome/m156), `SkShaper::PurgeCaches`.
    // The only caches are the HarfBuzz ones (M6), so nothing is purged yet.
    /// `SkShaper::PurgeCaches()`.
    #[doc(alias = "PurgeCaches")]
    pub fn purge_caches() {}

    /// `SkShaper::shape(utf8, FontRunIterator&, BiDiRunIterator&, ScriptRunIterator&,
    /// LanguageRunIterator&, features, options, handler)`.
    #[allow(clippy::too_many_arguments)] // mirrors SkShaper::shape's parameters
    pub fn shape(
        &self,
        utf8: &[u8],
        font_runs: &mut dyn FontRunIterator,
        bidi_runs: &mut dyn BiDiRunIterator,
        script_runs: &mut dyn ScriptRunIterator,
        language_runs: &mut dyn LanguageRunIterator,
        features: &[Feature],
        options: &Options,
        handler: &mut dyn RunHandler,
    ) {
        self.backend.shape(
            utf8,
            font_runs,
            bidi_runs,
            script_runs,
            language_runs,
            features,
            options,
            handler,
        );
    }

    // Port of: modules/skshaper/include/SkShaper.h#L301-L312 (chrome/m156), the legacy
    // `SkShaper::shape(utf8, FontRunIterator&, BiDiRunIterator&, ScriptRunIterator&,
    // LanguageRunIterator&, SkScalar width, RunHandler*)` (no features, width only).
    /// The legacy `shape` with iterators and a width, and no features.
    #[doc(alias = "shape")]
    #[allow(clippy::too_many_arguments)] // mirrors the legacy SkShaper::shape overload
    pub fn shape_with_iterators(
        &self,
        utf8: &[u8],
        font_runs: &mut dyn FontRunIterator,
        bidi_runs: &mut dyn BiDiRunIterator,
        script_runs: &mut dyn ScriptRunIterator,
        language_runs: &mut dyn LanguageRunIterator,
        width: scalar,
        handler: &mut dyn RunHandler,
    ) {
        let options = Options {
            width,
            ..Options::default()
        };
        self.shape(
            utf8,
            font_runs,
            bidi_runs,
            script_runs,
            language_runs,
            &[],
            &options,
            handler,
        );
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L142-L153 (chrome/m156), the legacy
    // `SkShaperPrimitive::shape(utf8, const SkFont&, leftToRight, width, handler)`, which has no
    // fallback manager: `leftToRight` is unused by the primitive shaper, and the font manager is
    // null, so a missing glyph keeps the primary font (see `run_iterator::FontMgrRunIterator`).
    /// The legacy `shape` with one font, a direction and a width.
    #[doc(alias = "shape")]
    pub fn shape_font(
        &self,
        utf8: &[u8],
        font: &Font,
        left_to_right: bool,
        width: scalar,
        handler: &mut dyn RunHandler,
    ) {
        let _ = left_to_right; // Unused by the primitive shaper, as in Skia.
        let mut font_runs = make_font_mgr_run_iterator(utf8, font, None);
        // bidi, script and language are unused, so they are built with empty data.
        let mut bidi_runs = TrivialBiDiRunIterator::new(0, 0);
        let mut script_runs = TrivialScriptRunIterator::new(0, 0);
        let mut language_runs = TrivialLanguageRunIterator::new("", 0);
        self.shape_with_iterators(
            utf8,
            &mut font_runs,
            &mut bidi_runs,
            &mut script_runs,
            &mut language_runs,
            width,
            handler,
        );
    }
}
