// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper.h (the RunIterator classes,
// TrivialRunIterator), modules/skshaper/src/SkShaper.cpp (FontMgrRunIterator,
// MakeStdLanguageRunIterator).

//! Run iterators: the sequences of runs of a text that drive a shaper.
//!
//! A run iterator covers the UTF-8 bytes of a text, one run at a time: [`RunIterator::consume`]
//! moves to the next run, [`RunIterator::end_of_current_run`] gives the byte offset past it.

use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::FourByteTag;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::{self, Unichar};

// Port of: modules/skshaper/src/SkShaper.cpp#L27-L30 (chrome/m156), `utf8_next`: replaces
// invalid UTF-8 sequences with REPLACEMENT CHARACTER U+FFFD.
fn utf8_next(utf8: &[u8], at: usize) -> (Unichar, usize) {
    let mut rest = &utf8[at..];
    let val = utf::next_utf8(&mut rest);
    let next = utf8.len() - rest.len();
    (if val < 0 { 0xFFFD } else { val }, next)
}

// Port of: modules/skshaper/include/SkShaper.h#L97-L115 (chrome/m156), `SkShaper::RunIterator`.
/// A sequence of runs over a UTF-8 text.
#[doc(alias = "SkShaper::RunIterator")]
pub trait RunIterator {
    /// Sets the state to that of the current run and moves the iterator to the end of that run.
    fn consume(&mut self);

    /// The offset of one past the last UTF-8 byte of the current run.
    #[doc(alias = "endOfCurrentRun")]
    #[must_use]
    fn end_of_current_run(&self) -> usize;

    /// `true` when [`RunIterator::consume`] must no longer be called.
    #[doc(alias = "atEnd")]
    #[must_use]
    fn at_end(&self) -> bool;
}

/// `SkShaper::FontRunIterator`: the font of each run.
#[doc(alias = "SkShaper::FontRunIterator")]
pub trait FontRunIterator: RunIterator {
    /// The font of the current run.
    #[doc(alias = "currentFont")]
    #[must_use]
    fn current_font(&self) -> &Font;
}

/// `SkShaper::BiDiRunIterator`: the bidi embedding level of each run.
#[doc(alias = "SkShaper::BiDiRunIterator")]
pub trait BiDiRunIterator: RunIterator {
    /// The Unicode bidi embedding level: even is LTR, odd is RTL.
    #[doc(alias = "currentLevel")]
    #[must_use]
    fn current_level(&self) -> u8;
}

/// `SkShaper::ScriptRunIterator`: the script of each run.
#[doc(alias = "SkShaper::ScriptRunIterator")]
pub trait ScriptRunIterator: RunIterator {
    /// The script of the current run, as an ISO 15924 tag.
    #[doc(alias = "currentScript")]
    #[must_use]
    fn current_script(&self) -> FourByteTag;
}

/// `SkShaper::LanguageRunIterator`: the language of each run.
#[doc(alias = "SkShaper::LanguageRunIterator")]
pub trait LanguageRunIterator: RunIterator {
    /// The language of the current run: BCP-47, or a C locale name.
    #[doc(alias = "currentLanguage")]
    #[must_use]
    fn current_language(&self) -> &str;
}

// Port of: modules/skshaper/include/SkShaper.h#L117-L132 (chrome/m156), the private
// `SkShaper::TrivialRunIterator`: one run covering the whole text.
#[derive(Debug, Clone, Copy)]
struct TrivialRun {
    end: usize,
    at_end: bool,
}

impl TrivialRun {
    // Port of: modules/skshaper/include/SkShaper.h#L120 (chrome/m156), the constructor.
    fn new(utf8_bytes: usize) -> Self {
        Self {
            end: utf8_bytes,
            at_end: utf8_bytes == 0,
        }
    }

    // Port of: modules/skshaper/include/SkShaper.h#L121 (chrome/m156), `consume`.
    fn consume(&mut self) {
        debug_assert!(!self.at_end);
        self.at_end = true;
    }

    // Port of: modules/skshaper/include/SkShaper.h#L122 (chrome/m156), `endOfCurrentRun`.
    fn end_of_current_run(&self) -> usize {
        if self.at_end { self.end } else { 0 }
    }
}

// Port of: modules/skshaper/include/SkShaper.h#L140-L153 (chrome/m156),
// `SkShaper::TrivialFontRunIterator`.
/// `SkShaper::TrivialFontRunIterator`: a single font for the whole text.
#[derive(Debug)]
#[doc(alias = "SkShaper::TrivialFontRunIterator")]
pub struct TrivialFontRunIterator {
    font: Font,
    run: TrivialRun,
}

impl TrivialFontRunIterator {
    /// `TrivialFontRunIterator(const SkFont& font, size_t utf8Bytes)`.
    #[must_use]
    pub fn new(font: &Font, utf8_bytes: usize) -> Self {
        Self {
            font: font.clone(),
            run: TrivialRun::new(utf8_bytes),
        }
    }
}

impl RunIterator for TrivialFontRunIterator {
    fn consume(&mut self) {
        self.run.consume();
    }
    fn end_of_current_run(&self) -> usize {
        self.run.end_of_current_run()
    }
    fn at_end(&self) -> bool {
        self.run.at_end
    }
}

impl FontRunIterator for TrivialFontRunIterator {
    fn current_font(&self) -> &Font {
        &self.font
    }
}

// Port of: modules/skshaper/include/SkShaper.h#L186-L197 (chrome/m156),
// `SkShaper::TrivialBiDiRunIterator`.
/// `SkShaper::TrivialBiDiRunIterator`: one bidi level for the whole text.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "SkShaper::TrivialBiDiRunIterator")]
pub struct TrivialBiDiRunIterator {
    level: u8,
    run: TrivialRun,
}

impl TrivialBiDiRunIterator {
    /// `TrivialBiDiRunIterator(uint8_t bidiLevel, size_t utf8Bytes)`.
    #[must_use]
    pub fn new(bidi_level: u8, utf8_bytes: usize) -> Self {
        Self {
            level: bidi_level,
            run: TrivialRun::new(utf8_bytes),
        }
    }
}

impl RunIterator for TrivialBiDiRunIterator {
    fn consume(&mut self) {
        self.run.consume();
    }
    fn end_of_current_run(&self) -> usize {
        self.run.end_of_current_run()
    }
    fn at_end(&self) -> bool {
        self.run.at_end
    }
}

impl BiDiRunIterator for TrivialBiDiRunIterator {
    fn current_level(&self) -> u8 {
        self.level
    }
}

// Port of: modules/skshaper/include/SkShaper.h#L224-L235 (chrome/m156),
// `SkShaper::TrivialScriptRunIterator`.
/// `SkShaper::TrivialScriptRunIterator`: one script for the whole text.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "SkShaper::TrivialScriptRunIterator")]
pub struct TrivialScriptRunIterator {
    script: FourByteTag,
    run: TrivialRun,
}

impl TrivialScriptRunIterator {
    /// `TrivialScriptRunIterator(SkFourByteTag script, size_t utf8Bytes)`.
    #[must_use]
    pub fn new(script: FourByteTag, utf8_bytes: usize) -> Self {
        Self {
            script,
            run: TrivialRun::new(utf8_bytes),
        }
    }
}

impl RunIterator for TrivialScriptRunIterator {
    fn consume(&mut self) {
        self.run.consume();
    }
    fn end_of_current_run(&self) -> usize {
        self.run.end_of_current_run()
    }
    fn at_end(&self) -> bool {
        self.run.at_end
    }
}

impl ScriptRunIterator for TrivialScriptRunIterator {
    fn current_script(&self) -> FourByteTag {
        self.script
    }
}

// Port of: modules/skshaper/include/SkShaper.h#L258-L269 (chrome/m156),
// `SkShaper::TrivialLanguageRunIterator`.
/// `SkShaper::TrivialLanguageRunIterator`: one language for the whole text.
#[derive(Debug, Clone)]
#[doc(alias = "SkShaper::TrivialLanguageRunIterator")]
pub struct TrivialLanguageRunIterator {
    language: String,
    run: TrivialRun,
}

impl TrivialLanguageRunIterator {
    /// `TrivialLanguageRunIterator(const char* language, size_t utf8Bytes)`. Skia's `nullptr`
    /// language is the empty string, as `SkString(nullptr)` is.
    #[must_use]
    pub fn new(language: &str, utf8_bytes: usize) -> Self {
        Self {
            language: language.to_owned(),
            run: TrivialRun::new(utf8_bytes),
        }
    }
}

impl RunIterator for TrivialLanguageRunIterator {
    fn consume(&mut self) {
        self.run.consume();
    }
    fn end_of_current_run(&self) -> usize {
        self.run.end_of_current_run()
    }
    fn at_end(&self) -> bool {
        self.run.at_end
    }
}

impl LanguageRunIterator for TrivialLanguageRunIterator {
    fn current_language(&self) -> &str {
        &self.language
    }
}

// Port of: modules/skshaper/src/SkShaper.cpp#L183-L186 (chrome/m156),
// `SkShaper::MakeStdLanguageRunIterator`.
/// `SkShaper::MakeStdLanguageRunIterator`: the language of the global C++ locale, which is `"C"`
/// unless the program changes it with `std::locale::global`. Rust programs have no such global
/// locale, so the name is the classic one.
#[must_use]
#[doc(alias = "MakeStdLanguageRunIterator")]
pub fn make_std_language_run_iterator(utf8_bytes: usize) -> TrivialLanguageRunIterator {
    TrivialLanguageRunIterator::new("C", utf8_bytes)
}

// Port of: modules/skshaper/src/SkShaper.cpp#L53-L131 (chrome/m156), the private
// `FontMgrRunIterator`.
//
// `fFallbackMgr` is nullable in Skia, and Skia dereferences it when a character is missing from
// the primary font. Here a missing fallback manager means "no fallback candidate": the primary
// font is used, as it is when the manager finds nothing.
//
// The overload of `SkShaper::MakeFontMgrRunIterator` that takes a `LanguageRunIterator` is not
// ported (rust-skia has the same TODO): the language is read while the font iterator is consumed,
// which a shared borrow of a second `&mut` iterator cannot express.
/// `SkShaper::FontRunIterator` from a font manager: each run uses the first font that has its
/// characters, falling back to a typeface the font manager finds for them.
#[derive(Debug)]
pub struct FontMgrRunIterator<'a> {
    current: usize,
    begin: usize,
    end: usize,
    utf8: &'a [u8],
    fallback_mgr: Option<FontMgr>,
    font: Font,
    /// `fFallbackFont`: `None` is Skia's null typeface.
    fallback_typeface: Option<Typeface>,
    fallback_font: Font,
    current_is_fallback: bool,
    request_style: FontStyle,
}

impl<'a> FontMgrRunIterator<'a> {
    // Port of: modules/skshaper/src/SkShaper.cpp#L53-L70 (chrome/m156), the constructor with
    // `requestName` nullptr, `requestStyle` = the font's typeface style and no language.
    /// `SkShaper::MakeFontMgrRunIterator(utf8, utf8Bytes, font, fallback)`.
    #[must_use]
    pub fn new(utf8: &'a [u8], font: &Font, fallback_mgr: Option<FontMgr>) -> Self {
        let request_style = font.typeface().font_style();
        // `fFont.setTypeface(font.refTypeface())`: a copy of `font` with its own typeface.
        let font = font.clone();
        // `fFallbackFont(fFont)` with `fFallbackFont.setTypeface(nullptr)`.
        let fallback_font = font.clone();
        Self {
            current: 0,
            begin: 0,
            end: utf8.len(),
            utf8,
            fallback_mgr,
            font,
            fallback_typeface: None,
            fallback_font,
            current_is_fallback: false,
            request_style,
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L92-L95 (chrome/m156), the current font
    // pointer `fCurrentFont`.
    fn current_font_ref(&self) -> &Font {
        if self.current_is_fallback {
            &self.fallback_font
        } else {
            &self.font
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L81-L84 (chrome/m156): `fFallbackFont.getTypeface()
    // && fFallbackFont.unicharToGlyph(u)`.
    fn fallback_has_glyph(&self, unichar: Unichar) -> bool {
        self.fallback_typeface.is_some() && self.fallback_font.unichar_to_glyph(unichar) != 0
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L104-L106 (chrome/m156): the
    // `matchFamilyStyleCharacter` call with no request name and no language.
    fn match_fallback(&self, unichar: Unichar) -> Option<Typeface> {
        self.fallback_mgr.as_ref()?.match_family_style_character(
            None,
            &self.request_style,
            &[],
            unichar,
        )
    }
}

impl RunIterator for FontMgrRunIterator<'_> {
    // Port of: modules/skshaper/src/SkShaper.cpp#L79-L129 (chrome/m156), `consume`.
    fn consume(&mut self) {
        debug_assert!(self.current < self.end);
        let (unichar, next) = utf8_next(self.utf8, self.current);
        self.current = next;

        // If the starting typeface can handle this character, use it.
        if self.font.unichar_to_glyph(unichar) != 0 {
            self.current_is_fallback = false;
        // If the current fallback can handle this character, use it.
        } else if self.fallback_has_glyph(unichar) {
            self.current_is_fallback = true;
        // If not, try to find a fallback typeface.
        } else if let Some(candidate) = self.match_fallback(unichar) {
            self.fallback_typeface = Some(candidate.clone());
            self.fallback_font.set_typeface(Some(candidate));
            self.current_is_fallback = true;
        } else {
            self.current_is_fallback = false;
        }

        while self.current < self.end {
            let prev = self.current;
            let (unichar, next) = utf8_next(self.utf8, self.current);
            self.current = next;

            // End the run if not using the initial typeface and the initial typeface has this
            // character.
            let current_typeface = self.current_font_ref().typeface();
            if *current_typeface != *self.font.typeface()
                && self.font.unichar_to_glyph(unichar) != 0
            {
                self.current = prev;
                return;
            }

            // End the run if the current typeface does not have this character and some other
            // font does.
            if self.current_font_ref().unichar_to_glyph(unichar) == 0
                && self.match_fallback(unichar).is_some()
            {
                self.current = prev;
                return;
            }
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L131-L133 (chrome/m156).
    fn end_of_current_run(&self) -> usize {
        self.current - self.begin
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L135-L137 (chrome/m156).
    fn at_end(&self) -> bool {
        self.current == self.end
    }
}

impl FontRunIterator for FontMgrRunIterator<'_> {
    // Port of: modules/skshaper/src/SkShaper.cpp#L139-L141 (chrome/m156), `currentFont`.
    fn current_font(&self) -> &Font {
        self.current_font_ref()
    }
}

// Port of: modules/skshaper/src/SkShaper.cpp#L63-L70 (chrome/m156), the 4-argument
// `MakeFontMgrRunIterator`.
/// `SkShaper::MakeFontMgrRunIterator(utf8, utf8Bytes, font, fallback)`.
#[must_use]
#[doc(alias = "MakeFontMgrRunIterator")]
pub fn make_font_mgr_run_iterator<'a>(
    utf8: &'a [u8],
    font: &Font,
    fallback: Option<FontMgr>,
) -> FontMgrRunIterator<'a> {
    FontMgrRunIterator::new(utf8, font, fallback)
}
