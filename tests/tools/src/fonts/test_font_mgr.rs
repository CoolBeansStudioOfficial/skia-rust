// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/TestFontMgr.cpp (chrome/m156)

//! The portable font manager (`ToolUtils::MakePortableFontMgr`): the test typefaces, grouped by
//! family, with no other fonts.
//!
//! The `Emoji` and `Planet` families are the `TestSVGTypeface`s (`SK_ENABLE_SVG`).

use std::sync::Arc;

use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{
    FontMgr, FontMgrBase, FontStyleSet, FontStyleSetBase, TypefaceDecoder,
};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;
use skia_rust_text::utils::custom_typeface;

use super::test_svg_typeface::TestSvgTypeface;
use super::test_typeface::{TestTypeface, typefaces};

/// One face of a [`TestFontStyleSet`] (`FontStyleSet::TypefaceEntry`).
// Port of: tools/fonts/TestFontMgr.cpp#L21-L28 (chrome/m156)
#[derive(Debug)]
struct TypefaceEntry {
    typeface: Typeface,
    style: FontStyle,
    style_name: &'static str,
}

/// The styles of one family (`FontStyleSet` in `TestFontMgr.cpp`).
// Port of: tools/fonts/TestFontMgr.cpp#L17-L51 (chrome/m156)
#[derive(Debug)]
struct TestFontStyleSet {
    family_name: &'static str,
    typefaces: Vec<TypefaceEntry>,
}

impl FontStyleSetBase for TestFontStyleSet {
    // Port of: tools/fonts/TestFontMgr.cpp#L30 (chrome/m156)
    fn count(&self) -> usize {
        self.typefaces.len()
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L31-L38 (chrome/m156)
    fn get_style(&self, index: usize) -> (FontStyle, String) {
        let entry = &self.typefaces[index];
        (entry.style, entry.style_name.to_owned())
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L39-L41 (chrome/m156)
    fn create_typeface(&self, index: usize) -> Option<Typeface> {
        Some(self.typefaces[index].typeface.clone())
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L42-L44 (chrome/m156)
    fn match_style(&self, pattern: &FontStyle) -> Option<Typeface> {
        self.match_style_css3(pattern)
    }
}

/// `TestFontMgr`'s manager: families in `TestTypeface::Typefaces()` order.
// Port of: tools/fonts/TestFontMgr.cpp#L53-L131 (chrome/m156)
#[derive(Debug)]
struct TestFontMgr {
    families: Vec<Arc<TestFontStyleSet>>,
    default_family: Arc<TestFontStyleSet>,
    default_typeface: Typeface,
}

impl TestFontMgr {
    /// `FontMgr::FontMgr()`: builds the families and finds the default face.
    // Port of: tools/fonts/TestFontMgr.cpp#L55-L77 (chrome/m156)
    fn new() -> Self {
        let mut families = Vec::new();
        let mut default = None;
        for family in &typefaces().families {
            let set = Arc::new(TestFontStyleSet {
                family_name: family.name,
                typefaces: family
                    .faces
                    .iter()
                    .map(|face| TypefaceEntry {
                        typeface: face.typeface.clone(),
                        style: face.typeface.font_style(),
                        style_name: face.name,
                    })
                    .collect(),
            });
            for face in &family.faces {
                if face.is_default {
                    default = Some((Arc::clone(&set), face.typeface.clone()));
                }
            }
            families.push(set);
        }
        // C++ asserts that the test data has a default and falls back to the first family.
        let (default_family, default_typeface) =
            default.expect("TestTypeface must have a default typeface");

        // `#if defined(SK_ENABLE_SVG)`
        let svg_family = |family_name: &'static str, typeface: Typeface| {
            Arc::new(TestFontStyleSet {
                family_name,
                typefaces: vec![TypefaceEntry {
                    typeface,
                    style: FontStyle::normal(),
                    style_name: "Normal",
                }],
            })
        };
        families.push(svg_family("Emoji", TestSvgTypeface::default_typeface()));
        families.push(svg_family("Planet", TestSvgTypeface::planets()));
        Self {
            families,
            default_family,
            default_typeface,
        }
    }

    /// `FontMgr::matchFamily`, with `emptyOnNull`: the family's style set, or an empty set.
    // Port of: src/core/SkFontMgr.cpp#L74-L76 (chrome/m156), as used by onMatchFamilyStyle
    fn match_family(&self, family_name: Option<&str>) -> FontStyleSet {
        self.on_match_family(family_name)
            .unwrap_or_else(FontStyleSet::create_empty)
    }
}

impl FontMgrBase for TestFontMgr {
    // Port of: tools/fonts/TestFontMgr.cpp#L79 (chrome/m156)
    fn on_count_families(&self) -> usize {
        self.families.len()
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L84-L86 (chrome/m156)
    fn on_get_family_name(&self, index: usize) -> String {
        self.families[index].family_name.to_owned()
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L88-L91 (chrome/m156)
    fn on_create_style_set(&self, index: usize) -> Option<FontStyleSet> {
        Some(FontStyleSet::new(self.families[index].clone()))
    }

    /// `onMatchFamily`: the family whose name contains the key ("ono" monospace, "ans"
    /// sans-serif, "erif" serif), by the first key that matches, as Skia does.
    // Port of: tools/fonts/TestFontMgr.cpp#L93-L116 (chrome/m156)
    fn on_match_family(&self, family_name: Option<&str>) -> Option<FontStyleSet> {
        let name = family_name?;
        let index = if name.contains("ono") {
            0
        } else if name.contains("ans") {
            1
        } else if name.contains("erif") {
            2
        } else if name.contains("oji") {
            6
        } else if name.contains("Planet") {
            7
        } else {
            return None;
        };
        self.on_create_style_set(index)
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L118-L122 (chrome/m156)
    fn on_match_family_style(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
    ) -> Option<Typeface> {
        self.match_family(family_name).match_style(style)
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L124-L136 (chrome/m156): the bcp47 tags and the
    // character are ignored.
    fn on_match_family_style_character(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        self.on_match_family_style(family_name, style)
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L138-L139 (chrome/m156): no data is accepted.
    fn on_make_from_data(
        &self,
        _data: &skia_rust_core::data::Data,
        _tt_index: i32,
    ) -> Option<Typeface> {
        None
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L141-L144 (chrome/m156)
    fn on_make_from_stream_index(
        &self,
        _stream: Box<dyn StreamAsset>,
        _tt_index: i32,
    ) -> Option<Typeface> {
        None
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L146-L149 (chrome/m156)
    fn on_make_from_stream_args(
        &self,
        _stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        None
    }

    // Port of: tools/fonts/TestFontMgr.cpp#L151-L154 (chrome/m156)
    fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
        None
    }

    /// `onLegacyMakeTypeface`: the default family for a null name, else the named family, else
    /// the default typeface.
    // Port of: tools/fonts/TestFontMgr.cpp#L156-L168 (chrome/m156)
    fn on_legacy_make_typeface(
        &self,
        family_name: Option<&str>,
        style: FontStyle,
    ) -> Option<Typeface> {
        if family_name.is_none() {
            return self.default_family.match_style(&style);
        }
        self.on_match_family_style(family_name, &style)
            .or_else(|| Some(self.default_typeface.clone()))
    }

    /// The factory that reads the test typefaces back (`TestTypeface::Register`).
    // Port of: tools/fonts/TestTypeface.cpp#L232-L237 (chrome/m156)
    fn typeface_decoders(&self) -> Vec<TypefaceDecoder> {
        vec![
            TypefaceDecoder {
                factory_id: TestTypeface::FACTORY_ID,
                make_from_stream: TestTypeface::make_from_stream,
            },
            TypefaceDecoder {
                factory_id: TestSvgTypeface::DEFAULT_FACTORY_ID,
                make_from_stream: TestSvgTypeface::make_default_from_stream,
            },
            TypefaceDecoder {
                factory_id: TestSvgTypeface::PLANETS_FACTORY_ID,
                make_from_stream: TestSvgTypeface::make_planets_from_stream,
            },
            // `SkCustomTypefaceBuilder`'s decoder, which C++ registers for every configuration.
            TypefaceDecoder {
                factory_id: custom_typeface::FACTORY_ID,
                make_from_stream: custom_typeface::make_from_stream,
            },
        ]
    }
}

/// `ToolUtils::MakePortableFontMgr()`: a new portable manager.
// Port of: tools/fonts/TestFontMgr.cpp#L178-L180 (chrome/m156)
#[doc(alias = "MakePortableFontMgr")]
#[must_use]
pub fn make_portable_font_mgr() -> FontMgr {
    FontMgr::new(Arc::new(TestFontMgr::new()))
}
