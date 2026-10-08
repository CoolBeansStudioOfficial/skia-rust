// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontMgr.h, src/core/SkFontMgr.cpp (chrome/m156)

//! [`FontMgr`] (`SkFontMgr`): a source of typefaces by family and style, and [`FontStyleSet`]
//! (`SkFontStyleSet`), the styles of one family.
//!
//! Both are cheap-clone handles over `Arc<dyn ...Base>`, as [`Typeface`] is. A backend
//! implements [`FontMgrBase`] or [`FontStyleSetBase`]; the methods here are the non-virtual
//! half of Skia's classes, with the `on*` virtuals as the trait methods.

use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::data::Data;
use crate::font_arguments::FontArguments;
use crate::font_arguments::variation_position::Coordinate;
use crate::font_descriptor::FontDescriptor;
use crate::font_style::{FontStyle, Slant, Weight, Width};
use crate::scalar::{scalar, scalar_round_to_int};
use crate::stream::StreamAsset;
use crate::typeface::Typeface;
use crate::utf::Unichar;

/// The styles of one family (`SkFontStyleSet`'s virtuals).
// Port of: include/core/SkFontMgr.h#L19-L30 (chrome/m156)
#[doc(alias = "SkFontStyleSet")]
pub trait FontStyleSetBase: Send + Sync + fmt::Debug {
    /// `count`: the number of styles.
    // Port of: include/core/SkFontMgr.h#L22 (chrome/m156)
    fn count(&self) -> usize;

    /// `getStyle`: the style and name of the style at `index`.
    // Port of: include/core/SkFontMgr.h#L23 (chrome/m156)
    fn get_style(&self, index: usize) -> (FontStyle, String);

    /// `createTypeface`: the typeface at `index`.
    // Port of: include/core/SkFontMgr.h#L24 (chrome/m156)
    fn create_typeface(&self, index: usize) -> Option<Typeface>;

    /// `matchStyle`: the typeface that best matches `pattern`.
    // Port of: include/core/SkFontMgr.h#L25 (chrome/m156)
    fn match_style(&self, pattern: &FontStyle) -> Option<Typeface>;

    /// `SkFontStyleSet::matchStyleCSS3`: the CSS3 font matching algorithm, over this set's
    /// styles.
    // Port of: src/core/SkFontMgr.cpp#L335-L434 (chrome/m156)
    fn match_style_css3(&self, pattern: &FontStyle) -> Option<Typeface> {
        match_style_css3(self, pattern)
    }
}

/// A handle to a set of styles of one family (`sk_sp<SkFontStyleSet>`).
// Port of: include/core/SkFontMgr.h#L19 (chrome/m156)
#[doc(alias = "SkFontStyleSet")]
#[derive(Clone, Debug)]
pub struct FontStyleSet(Arc<dyn FontStyleSetBase>);

impl FontStyleSet {
    /// Wraps a backend's style set in a handle.
    #[must_use]
    pub fn new(base: Arc<dyn FontStyleSetBase>) -> Self {
        Self(base)
    }

    /// `SkFontStyleSet::CreateEmpty()`: a set with no styles.
    // Port of: src/core/SkFontMgr.cpp#L19-L22 (chrome/m156)
    #[doc(alias = "CreateEmpty")]
    #[must_use]
    pub fn create_empty() -> Self {
        Self::new(Arc::new(EmptyFontStyleSet))
    }

    /// `count`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.0.count()
    }

    /// `getStyle`: the style and name of the style at `index`.
    #[must_use]
    pub fn get_style(&self, index: usize) -> (FontStyle, String) {
        self.0.get_style(index)
    }

    /// `createTypeface`.
    #[must_use]
    pub fn create_typeface(&self, index: usize) -> Option<Typeface> {
        self.0.create_typeface(index)
    }

    /// `matchStyle`.
    #[must_use]
    pub fn match_style(&self, pattern: &FontStyle) -> Option<Typeface> {
        self.0.match_style(pattern)
    }

    /// `matchStyleCSS3`.
    #[must_use]
    pub fn match_style_css3(&self, pattern: &FontStyle) -> Option<Typeface> {
        self.0.match_style_css3(pattern)
    }
}

/// `SkEmptyFontStyleSet`: a family with no styles.
// Port of: src/core/SkFontMgr.cpp#L24-L39 (chrome/m156)
#[derive(Debug)]
struct EmptyFontStyleSet;

impl FontStyleSetBase for EmptyFontStyleSet {
    fn count(&self) -> usize {
        0
    }

    fn get_style(&self, _index: usize) -> (FontStyle, String) {
        unreachable!("SkFontStyleSet::getStyle called on empty set")
    }

    fn create_typeface(&self, _index: usize) -> Option<Typeface> {
        unreachable!("SkFontStyleSet::createTypeface called on empty set")
    }

    fn match_style(&self, _pattern: &FontStyle) -> Option<Typeface> {
        None
    }
}

/// `score[pattern.slant][current.slant]` of `matchStyleCSS3`, in the order Upright, Italic,
/// Oblique.
// Port of: src/core/SkFontMgr.cpp#L381-L388 (chrome/m156)
const SLANT_SCORE: [[i32; 3]; 3] = [
    // Upright  [pattern]
    [3, 1, 2],
    // Italic
    [1, 3, 2],
    // Oblique
    [1, 2, 3],
];

/// The score of one style in CSS3 matching (`Score` in `matchStyleCSS3`).
#[derive(Clone, Copy)]
struct Score {
    score: i32,
    index: usize,
}

/// `SkFontStyleSet::matchStyleCSS3`. Width has the greatest priority, then the slant, then the
/// weight. The comments below are Skia's.
// Port of: src/core/SkFontMgr.cpp#L335-L434 (chrome/m156)
// `pattern` is a reference as in the C++ signature (`const SkFontStyle&`).
#[allow(clippy::trivially_copy_pass_by_ref)]
fn match_style_css3<S: FontStyleSetBase + ?Sized>(
    set: &S,
    pattern: &FontStyle,
) -> Option<Typeface> {
    let count = set.count();
    if count == 0 {
        return None;
    }

    let pattern_width = *pattern.width();
    let pattern_weight = *pattern.weight();
    let mut max_score = Score { score: 0, index: 0 };
    for i in 0..count {
        let (current, _) = set.get_style(i);
        let current_width = *current.width();
        let current_weight = *current.weight();
        let mut current_score = Score { score: 0, index: i };

        // CSS stretch / SkFontStyle::Width. Takes priority over everything else.
        // If the value of pattern.width is 5 (normal) or less, narrower width values are
        // checked first, then wider values. If it is greater than 5, wider values first.
        if pattern_width <= *Width::NORMAL {
            if current_width <= pattern_width {
                current_score.score += 10 - pattern_width + current_width;
            } else {
                current_score.score += 10 - current_width;
            }
        } else if current_width > pattern_width {
            current_score.score += 10 + pattern_width - current_width;
        } else {
            current_score.score += current_width;
        }
        current_score.score <<= 8;

        // CSS style (normal, italic, oblique) / SkFontStyle::Slant. Takes priority over all
        // valid weights.
        current_score.score += SLANT_SCORE[pattern.slant() as usize][current.slant() as usize];
        current_score.score <<= 8;

        // Synthetics (weight, style). CSS weight / SkFontStyle::Weight: the 'closer' to the
        // target weight, the higher the score. 1000 is the 'heaviest' recognized weight.
        if pattern_weight == current_weight {
            current_score.score += 1000;
        } else if pattern_weight < 400 {
            // Less than 400 prefer lighter weights.
            if current_weight <= pattern_weight {
                current_score.score += 1000 - pattern_weight + current_weight;
            } else {
                current_score.score += 1000 - current_weight;
            }
        } else if pattern_weight <= 500 {
            // Between 400 and 500 prefer heavier up to 500, then lighter weights.
            if current_weight >= pattern_weight && current_weight <= 500 {
                current_score.score += 1000 + pattern_weight - current_weight;
            } else if current_weight <= pattern_weight {
                current_score.score += 500 + current_weight;
            } else {
                current_score.score += 1000 - current_weight;
            }
        } else if pattern_weight > 500 {
            // Greater than 500 prefer heavier weights.
            if current_weight > pattern_weight {
                current_score.score += 1000 + pattern_weight - current_weight;
            } else {
                current_score.score += current_weight;
            }
        }

        if max_score.score < current_score.score {
            max_score = current_score;
        }
    }
    set.create_typeface(max_score.index)
}

/// `SkFontMgr::Request`: what a font is asked for by [`FontMgr::match_request`] and
/// [`FontMgr::fallback`].
// Port of: include/core/SkFontMgr.h#L60-L77 (chrome/m156)
#[doc(alias = "SkFontMgr::Request")]
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// `cmapEntries`: the characters to find a font for (the fallback uses the first).
    pub cmap_entries: &'a [CMapEntry],
    /// `bcp47`: the language tags.
    pub bcp47: &'a [&'a str],
    /// `familyName`, or `None` for C++'s `nullptr`.
    pub family_name: Option<&'a str>,
    /// `model`: the variation coordinates of the wanted style.
    pub model: &'a [Coordinate],
    /// `syntheticBold`.
    pub synthetic_bold: Option<bool>,
    /// `syntheticOblique`.
    pub synthetic_oblique: Option<bool>,
}

/// `SkFontMgr::Request::CMapEntry`.
// Port of: include/core/SkFontMgr.h#L62-L65 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CMapEntry {
    /// `character`.
    pub character: Unichar,
    /// `variation`: zero for the default variation.
    pub variation: Unichar,
}

impl Request<'_> {
    /// `SkFontMgr::Request::fontStyleFromModel`: the style of the model's coordinates. Missing
    /// axes take the defaults (weight 400, width 100, upright).
    // Port of: src/core/SkFontMgr.cpp#L212-L236 (chrome/m156)
    #[doc(alias = "fontStyleFromModel")]
    #[must_use]
    pub fn font_style_from_model(&self) -> FontStyle {
        let mut weight: scalar = 400.0;
        let mut width: scalar = 100.0;
        let mut slant: scalar = 0.0;
        let mut italic: scalar = 0.0;
        for coord in self.model {
            match coord.axis {
                Coordinate::WGHT => weight = coord.value,
                Coordinate::WDTH => width = coord.value,
                Coordinate::SLNT => slant = coord.value,
                Coordinate::ITAL => italic = coord.value,
                _ => {}
            }
        }
        let mut slant_enum = Slant::Upright;
        if slant != 0.0 {
            slant_enum = Slant::Oblique;
        }
        if 0.0 < italic {
            slant_enum = Slant::Italic;
        }
        let width_enum = FontDescriptor::style_width_for_width_axis_value(width);
        FontStyle::new(
            Weight::from(scalar_round_to_int(weight)),
            width_enum,
            slant_enum,
        )
    }

    /// `SkFontMgr::Request::SetModel`: the model coordinates of `style`.
    // Port of: src/core/SkFontMgr.cpp#L238-L250 (chrome/m156)
    #[doc(alias = "SetModel")]
    #[must_use]
    pub fn set_model(style: FontStyle) -> [Coordinate; 4] {
        [
            Coordinate {
                axis: Coordinate::WGHT,
                // Weights are small integers, exact in a float, as C++'s int-to-float.
                #[allow(clippy::cast_precision_loss)]
                value: *style.weight() as f32,
            },
            Coordinate {
                axis: Coordinate::WDTH,
                value: FontDescriptor::font_width_axis_value_for_style_width(*style.width()),
            },
            Coordinate {
                axis: Coordinate::SLNT,
                value: if style.slant() == Slant::Oblique {
                    -20.0
                } else {
                    0.0
                },
            },
            Coordinate {
                axis: Coordinate::ITAL,
                value: if style.slant() == Slant::Italic {
                    1.0
                } else {
                    0.0
                },
            },
        ]
    }
}

/// A source of typefaces (`SkFontMgr`'s virtuals).
// Port of: include/core/SkFontMgr.h#L86-L118 (chrome/m156)
#[doc(alias = "SkFontMgr")]
pub trait FontMgrBase: Send + Sync + fmt::Debug {
    /// `onCountFamilies`.
    // Port of: include/core/SkFontMgr.h#L89 (chrome/m156)
    fn on_count_families(&self) -> usize;

    /// `onGetFamilyName`.
    // Port of: include/core/SkFontMgr.h#L90 (chrome/m156)
    fn on_get_family_name(&self, index: usize) -> String;

    /// `onCreateStyleSet`: `None` is C++'s null, which the handle turns into an empty set.
    // Port of: include/core/SkFontMgr.h#L91 (chrome/m156)
    fn on_create_style_set(&self, index: usize) -> Option<FontStyleSet>;

    /// `onMatchFamily`: `None` is C++'s null, which the handle turns into an empty set.
    // Port of: include/core/SkFontMgr.h#L92 (chrome/m156)
    fn on_match_family(&self, family_name: Option<&str>) -> Option<FontStyleSet>;

    /// `onMatchFamilyStyle`.
    // Port of: include/core/SkFontMgr.h#L93 (chrome/m156)
    fn on_match_family_style(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
    ) -> Option<Typeface>;

    /// `onMatchFamilyStyleCharacter`.
    // Port of: include/core/SkFontMgr.h#L94-L97 (chrome/m156)
    fn on_match_family_style_character(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
        bcp47: &[&str],
        character: Unichar,
    ) -> Option<Typeface>;

    /// `onMakeFromData`.
    // Port of: include/core/SkFontMgr.h#L100 (chrome/m156)
    fn on_make_from_data(&self, data: &Data, tt_index: i32) -> Option<Typeface>;

    /// `onMakeFromStreamIndex`.
    // Port of: include/core/SkFontMgr.h#L101 (chrome/m156)
    fn on_make_from_stream_index(
        &self,
        stream: Box<dyn StreamAsset>,
        tt_index: i32,
    ) -> Option<Typeface>;

    /// `onMakeFromStreamArgs`.
    // Port of: include/core/SkFontMgr.h#L103 (chrome/m156)
    fn on_make_from_stream_args(
        &self,
        stream: Box<dyn StreamAsset>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface>;

    /// `onMakeFromFile`.
    // Port of: include/core/SkFontMgr.h#L105 (chrome/m156)
    fn on_make_from_file(&self, path: &str, tt_index: i32) -> Option<Typeface>;

    /// `onLegacyMakeTypeface`.
    // Port of: include/core/SkFontMgr.h#L107 (chrome/m156)
    fn on_legacy_make_typeface(
        &self,
        family_name: Option<&str>,
        style: FontStyle,
    ) -> Option<Typeface>;

    /// `onMatch`: matches a request by its family name and the style of its model.
    // Port of: src/core/SkFontMgr.cpp#L252-L256 (chrome/m156)
    fn on_match(&self, request: &Request<'_>) -> Option<Typeface> {
        let style = request.font_style_from_model();
        self.on_match_family_style(request.family_name, &style)
    }

    /// `onFallback`: a typeface for the first character of the request, in its family.
    // Port of: src/core/SkFontMgr.cpp#L258-L266 (chrome/m156)
    fn on_fallback(&self, request: &Request<'_>) -> Option<Typeface> {
        let style = request.font_style_from_model();
        let character = request.cmap_entries.first().map_or(0x20, |e| e.character);
        self.on_match_family_style_character(request.family_name, &style, request.bcp47, character)
    }

    /// The decoders of the typefaces this manager can make from a stream, for
    /// [`Typeface::make_deserialize`](crate::typeface::Typeface). This replaces the static
    /// registry of `SkTypeface::Register` (docs/design/text.md §5.2). The default is none.
    // Port of: include/core/SkTypeface.h (Register, chrome/m156), replaced by the manager
    fn typeface_decoders(&self) -> Vec<TypefaceDecoder> {
        Vec::new()
    }
}

/// `SkTypeface::Register`'s entry: a factory id and the function that reads its typefaces.
#[derive(Clone, Copy)]
pub struct TypefaceDecoder {
    /// The `FactoryId` that the typeface's descriptor carries.
    pub factory_id: crate::font_descriptor::FactoryId,
    /// `MakeFromStream`.
    pub make_from_stream: fn(Box<dyn StreamAsset>, &FontArguments<'_, '_>) -> Option<Typeface>,
}

impl fmt::Debug for TypefaceDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypefaceDecoder")
            .field("factory_id", &self.factory_id)
            .finish_non_exhaustive()
    }
}

/// A source of typefaces (`sk_sp<SkFontMgr>`). Cloning it shares the manager.
// Port of: include/core/SkFontMgr.h#L44 (chrome/m156)
#[doc(alias = "SkFontMgr")]
#[derive(Clone, Debug)]
pub struct FontMgr(Arc<dyn FontMgrBase>);

impl FontMgr {
    /// Wraps a backend in a handle.
    #[must_use]
    pub fn new(base: Arc<dyn FontMgrBase>) -> Self {
        Self(base)
    }

    /// `SkFontMgr::RefEmpty()`: a manager with no families and no typeface dependencies. It is
    /// the same instance every time.
    // Port of: src/core/SkFontMgr.cpp#L291-L294 (chrome/m156)
    #[doc(alias = "RefEmpty")]
    #[must_use]
    pub fn empty() -> Self {
        static EMPTY: OnceLock<FontMgr> = OnceLock::new();
        EMPTY
            .get_or_init(|| Self::new(Arc::new(EmptyFontMgr)))
            .clone()
    }

    /// `countFamilies`.
    #[doc(alias = "countFamilies")]
    #[must_use]
    pub fn count_families(&self) -> usize {
        self.0.on_count_families()
    }

    /// `getFamilyName`.
    #[doc(alias = "getFamilyName")]
    #[must_use]
    pub fn family_name(&self, index: usize) -> String {
        self.0.on_get_family_name(index)
    }

    /// `createStyleSet`: the styles of the family at `index`, or an empty set.
    // Port of: src/core/SkFontMgr.cpp#L70-L72 (chrome/m156)
    #[doc(alias = "createStyleSet")]
    #[must_use]
    pub fn create_style_set(&self, index: usize) -> FontStyleSet {
        self.0
            .on_create_style_set(index)
            .unwrap_or_else(FontStyleSet::create_empty)
    }

    /// `matchFamily`: the styles of the named family, or an empty set (`emptyOnNull`).
    // Port of: src/core/SkFontMgr.cpp#L74-L76 (chrome/m156)
    #[doc(alias = "matchFamily")]
    #[must_use]
    pub fn match_family(&self, family_name: Option<&str>) -> FontStyleSet {
        self.0
            .on_match_family(family_name)
            .unwrap_or_else(FontStyleSet::create_empty)
    }

    /// `matchFamilyStyle`.
    #[doc(alias = "matchFamilyStyle")]
    #[must_use]
    pub fn match_family_style(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
    ) -> Option<Typeface> {
        self.0.on_match_family_style(family_name, style)
    }

    /// `matchFamilyStyleCharacter`.
    #[doc(alias = "matchFamilyStyleCharacter")]
    #[must_use]
    pub fn match_family_style_character(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
        bcp47: &[&str],
        character: Unichar,
    ) -> Option<Typeface> {
        self.0
            .on_match_family_style_character(family_name, style, bcp47, character)
    }

    /// `SkFontMgr::match`: the typeface that best matches the request.
    // Port of: src/core/SkFontMgr.cpp#L278-L280 (chrome/m156)
    #[must_use]
    pub fn match_request(&self, request: &Request<'_>) -> Option<Typeface> {
        self.0.on_match(request)
    }

    /// `SkFontMgr::fallback`: a typeface that can draw the request's first character.
    // Port of: src/core/SkFontMgr.cpp#L282-L284 (chrome/m156)
    #[must_use]
    pub fn fallback(&self, request: &Request<'_>) -> Option<Typeface> {
        self.0.on_fallback(request)
    }

    /// `makeFromData`: a typeface from data, or `None` for null data.
    // Port of: src/core/SkFontMgr.cpp#L186-L191 (chrome/m156)
    #[doc(alias = "makeFromData")]
    #[must_use]
    pub fn make_from_data(&self, data: Option<&Data>, tt_index: i32) -> Option<Typeface> {
        self.0.on_make_from_data(data?, tt_index)
    }

    /// `makeFromStream` with a collection index. `None` is C++'s null stream.
    // Port of: src/core/SkFontMgr.cpp#L193-L199 (chrome/m156)
    #[doc(alias = "makeFromStream")]
    #[must_use]
    pub fn make_from_stream(
        &self,
        stream: Option<Box<dyn StreamAsset>>,
        tt_index: i32,
    ) -> Option<Typeface> {
        self.0.on_make_from_stream_index(stream?, tt_index)
    }

    /// `makeFromStream` with font arguments. `None` is C++'s null stream.
    // Port of: src/core/SkFontMgr.cpp#L201-L207 (chrome/m156)
    #[doc(alias = "makeFromStream")]
    #[must_use]
    pub fn make_from_stream_args(
        &self,
        stream: Option<Box<dyn StreamAsset>>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        self.0.on_make_from_stream_args(stream?, args)
    }

    /// `makeFromFile`.
    // Port of: src/core/SkFontMgr.cpp#L209-L214 (chrome/m156)
    #[doc(alias = "makeFromFile")]
    #[must_use]
    pub fn make_from_file(&self, path: Option<&str>, tt_index: i32) -> Option<Typeface> {
        self.0.on_make_from_file(path?, tt_index)
    }

    /// `legacyMakeTypeface`.
    #[doc(alias = "legacyMakeTypeface")]
    #[must_use]
    pub fn legacy_make_typeface(
        &self,
        family_name: Option<&str>,
        style: FontStyle,
    ) -> Option<Typeface> {
        self.0.on_legacy_make_typeface(family_name, style)
    }

    /// The decoders of this manager, see [`FontMgrBase::typeface_decoders`].
    #[must_use]
    pub fn typeface_decoders(&self) -> Vec<TypefaceDecoder> {
        self.0.typeface_decoders()
    }
}

/// `SkEmptyFontMgr`: no families, and every lookup fails.
// Port of: src/core/SkFontMgr.cpp#L41-L86 (chrome/m156)
#[derive(Debug)]
struct EmptyFontMgr;

impl FontMgrBase for EmptyFontMgr {
    fn on_count_families(&self) -> usize {
        0
    }

    fn on_get_family_name(&self, _index: usize) -> String {
        // C++ SkDEBUGFAILs here and leaves the name untouched.
        String::new()
    }

    fn on_create_style_set(&self, _index: usize) -> Option<FontStyleSet> {
        // C++ SkDEBUGFAILs here and returns null, which the handle turns into an empty set.
        None
    }

    fn on_match_family(&self, _family_name: Option<&str>) -> Option<FontStyleSet> {
        Some(FontStyleSet::create_empty())
    }

    fn on_match_family_style(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
    ) -> Option<Typeface> {
        None
    }

    fn on_match_family_style_character(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        None
    }

    fn on_make_from_data(&self, _data: &Data, _tt_index: i32) -> Option<Typeface> {
        None
    }

    fn on_make_from_stream_index(
        &self,
        _stream: Box<dyn StreamAsset>,
        _tt_index: i32,
    ) -> Option<Typeface> {
        None
    }

    fn on_make_from_stream_args(
        &self,
        _stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        None
    }

    fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
        None
    }

    fn on_legacy_make_typeface(
        &self,
        _family_name: Option<&str>,
        _style: FontStyle,
    ) -> Option<Typeface> {
        None
    }
}
