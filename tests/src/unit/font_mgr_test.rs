// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontMgrTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::font::Font;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_mgr::{FontStyleSet, FontStyleSetBase, Request};
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::{FourByteTag, TextEncoding};
use skia_rust_core::scaler_context::{ScalerContext, ScalerContextEffects, ScalerContextRec};
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_tools::font_tool_utils::{create_test_typeface, default_typeface, test_font_mgr};

use crate::{def_font_test, def_test, errorf, reporter_assert};

// Port of: tests/FontMgrTest.cpp#L30-L59 (chrome/m156)
// The sizes are compared exactly, as the C++ test compares them.
def_font_test!(
    #[allow(clippy::float_cmp)]
    FontMgr_Font,
    |reporter| {
        let font = Font::from_size(default_typeface(), 24.0);
        reporter_assert!(reporter, 24.0 == font.size());
        reporter_assert!(reporter, 1.0 == font.scale_x());
        reporter_assert!(reporter, 0.0 == font.skew_x());

        let mut glyphs = [0; 5];
        // Check that no glyphs are copied with insufficient storage.
        let count = font.text_to_glyphs(b"Hello", TextEncoding::UTF8, &mut glyphs[..2]);
        reporter_assert!(reporter, 5 == count);
        for glyph in glyphs {
            reporter_assert!(reporter, glyph == 0);
        }
        reporter_assert!(
            reporter,
            font.text_to_glyphs(b"Hello", TextEncoding::UTF8, &mut glyphs) == count
        );
        for &glyph in &glyphs[..count] {
            reporter_assert!(reporter, 0 != glyph);
        }
        reporter_assert!(reporter, glyphs[0] != glyphs[1]); // 'h' != 'e'
        reporter_assert!(reporter, glyphs[2] == glyphs[3]); // 'l' == 'l'

        let new_font = font.make_with_size(36.0);
        reporter_assert!(reporter, font.typeface() == new_font.typeface());
        reporter_assert!(reporter, 36.0 == new_font.size()); // double check we haven't changed
        reporter_assert!(reporter, 24.0 == font.size()); // double check we haven't changed
    }
);

// Port of: tests/FontMgrTest.cpp#L75-L102 (chrome/m156)
// If the font backend is going to "alias" some font names to other fonts (e.g. sans -> Arial)
// then we want to at least get the same typeface back if we request the alias name multiple
// times.
def_font_test!(FontMgr_AliasNames, |reporter| {
    const IN_NAMES: [&str; 6] = [
        "sans",
        "sans-serif",
        "serif",
        "monospace",
        "times",
        "helvetica",
    ];
    for name in IN_NAMES {
        // C++ skips a null typeface here; CreateTestTypeface never returns one.
        let first = create_test_typeface(Some(name), FontStyle::default());
        let first_name = first.family_name();
        for _ in 0..10 {
            let face = create_test_typeface(Some(name), FontStyle::default());
            let face_name = face.family_name();
            reporter_assert!(
                reporter,
                first.unique_id() == face.unique_id(),
                "Request \"{}\" First Name: \"{}\" Id: {:?} Received Name \"{}\" Id {:?}",
                name,
                first_name,
                first.unique_id(),
                face_name,
                face.unique_id()
            );
        }
    }
});

// Port of: tests/FontMgrTest.cpp#L104-L212 (chrome/m156)
def_font_test!(FontMgr_Iter, |reporter| {
    let fm = test_font_mgr();
    let count = fm.count_families();
    for i in 0..count {
        let fname = fm.family_name(i);
        let fnset = fm.match_family(Some(fname.as_str()));
        let set = fm.create_style_set(i);
        reporter_assert!(reporter, fnset.count() == set.count());
        if reporter.verbose() {
            eprintln!("[{i:2}] {fname}");
        }
        for j in 0..set.count() {
            let (fs, sname) = set.get_style(j);
            if reporter.verbose() {
                eprintln!(
                    "\t[{j}] \"{sname}\" [{:3} {} {:?}]",
                    *fs.weight(),
                    *fs.width(),
                    fs.slant()
                );
            }
            let Some(face1) = set.create_typeface(j) else {
                errorf!(reporter, "Could not create {} {}.", fname, sname);
                continue;
            };
            let name1 = face1.family_name();
            let s1 = face1.font_style();
            for other_name in face1.new_family_name_iterator() {
                if reporter.verbose() {
                    eprintln!("\t\"{}\" aka \"{}\"", name1, other_name.string);
                }
            }
            let resource1 = face1.get_resource_name();
            if reporter.verbose() {
                eprintln!("\t\"{name1}\" from resource \"{resource1:?}\"");
            }
            // Note that fs != s1 is fine, though probably rare.
            match fm.match_family_style(Some(name1.as_str()), &s1) {
                None => {
                    // Some fonts cannot be looked up by name on our test machines
                    if name1 == "Noto Emoji" || name1 == "Noto Sans Phags Pa" {
                        continue;
                    }
                    reporter_assert!(reporter, false, "Could not find {}", name1);
                }
                Some(face2) => {
                    let name2 = face2.family_name();
                    reporter_assert!(reporter, name1 == name2, "{} == {}", name1, name2);
                    let s2 = face2.font_style();
                    reporter_assert!(
                        reporter,
                        s1 == s2,
                        "{} [{:3} {} {:?}] != {} [{:3} {} {:?}]",
                        name1,
                        *s1.weight(),
                        *s1.width(),
                        s1.slant(),
                        name2,
                        *s2.weight(),
                        *s2.width(),
                        s2.slant()
                    );
                }
            }
            let model = Request::set_model(s1);
            let request = Request {
                cmap_entries: &[],
                bcp47: &[],
                family_name: Some(name1.as_str()),
                model: &model,
                synthetic_bold: None,
                synthetic_oblique: None,
            };
            match fm.match_request(&request) {
                None => {
                    // Some fonts cannot be looked up by name on our test machines
                    if name1 == "Noto Emoji" || name1 == "Noto Sans Phags Pa" {
                        continue;
                    }
                    reporter_assert!(reporter, false, "Could not find {}", name1);
                }
                Some(face3) => {
                    let name3 = face3.family_name();
                    reporter_assert!(reporter, name1 == name3, "{} == {}", name1, name3);
                    let mut s3 = face3.font_style();
                    // With DirectWrite it is possible to have a synthetic-oblique in the
                    // collection but looking up by oblique will just give the actual oblique font.
                    if face1.is_synthetic_oblique()
                        && s1.slant() == Slant::Oblique
                        && s3.slant() == Slant::Upright
                    {
                        s3 = FontStyle::new(s3.weight(), s3.width(), s1.slant());
                    }
                    reporter_assert!(
                        reporter,
                        s1 == s3,
                        "{} [{:3} {} {:?}]{}{} != {} [{:3} {} {:?}]{}{}",
                        name1,
                        *s1.weight(),
                        *s1.width(),
                        s1.slant(),
                        if face1.is_synthetic_bold() { "(B)" } else { "" },
                        if face1.is_synthetic_oblique() {
                            "(O)"
                        } else {
                            ""
                        },
                        name3,
                        *s3.weight(),
                        *s3.width(),
                        s3.slant(),
                        if face3.is_synthetic_bold() { "(B)" } else { "" },
                        if face3.is_synthetic_oblique() {
                            "(O)"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
    }
});

// Port of: tests/FontMgrTest.cpp#L214-L218 (chrome/m156)
def_font_test!(FontMgr_Match, |_reporter| {
    let fm = test_font_mgr();
    // C++ asserts that the set is non-null. `match_family` never returns null (it returns an
    // empty set where C++ would return nullptr), so there is nothing to assert here.
    let _style_set = fm.match_family(None);
});

// Port of: tests/FontMgrTest.cpp#L220-L242 (chrome/m156)
def_font_test!(FontMgr_MatchFamilyStyle, |reporter| {
    let fm = test_font_mgr();
    let style_set = fm.match_family(Some("Non Existing Family Name"));
    reporter_assert!(reporter, style_set.count() == 0);

    let typeface = fm.match_family_style(Some("Non Existing Family Name"), &FontStyle::normal());
    reporter_assert!(reporter, typeface.is_none());

    // Test a long name with many interesting case folding code points.
    let typeface1 = fm.match_family_style(
        Some("ῢ ΰ ῤ ῦ ῧ Ῠ Ῡ Ὺ Ύ Ῥ ῲ ῳ ῴ ῶ ῷ Ὸ Ό Ὼ Ώ ῼ"),
        &FontStyle::normal(),
    );
    reporter_assert!(reporter, typeface1.is_none());
    // TODO: enable after determining if a default font should be required.
    // if ((false)) {
    //     sk_sp<SkTypeface> def(fm->matchFamilyStyle(nullptr, FS::Normal()));
    //     REPORTER_ASSERT(reporter, def);
    // }
});

/// The `invalid_font_style` of `FontMgr_MatchStyleCSS3`: the weight 101 is out of range.
fn invalid_font_style() -> FontStyle {
    FontStyle::new(Weight::from(101), Width::NORMAL, Slant::Upright)
}

/// The local `TestTypeface` of `FontMgr_MatchStyleCSS3`: a typeface that only has a style. Each
/// override is the C++ override of the same name.
// Port of: tests/FontMgrTest.cpp#L247-L301 (chrome/m156)
#[derive(Debug)]
struct TestTypeface {
    core: TypefaceCore,
}

impl TestTypeface {
    fn make(style: FontStyle) -> Typeface {
        Typeface::new(Arc::new(Self {
            core: TypefaceCore::new(style, false),
        }))
    }
}

impl TypefaceBase for TestTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    // `onGetFontDescriptor`: nothing.
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        (FontDescriptor::new(), false)
    }

    // `onOpenStream`: null.
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        None
    }

    // `onGetFamilyName`: empty.
    fn on_get_family_name(&self) -> String {
        String::new()
    }

    // `onGetVariationDesignPosition`: no axes.
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    // `onGetVariationDesignParameters`: -1, the number of axes is unknown.
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        None
    }

    // `onGetUPEM`: 0.
    fn on_get_upem(&self) -> i32 {
        0
    }

    // `onGetPostScriptName`: false.
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    // `onCreateFamilyNameIterator`: no names.
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::default())
    }

    // `onGetTableTags`: none.
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    // `onGetTableData`: none.
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
    }

    // `onFilterRec`: nothing.
    fn on_filter_rec(&self, _rec: &mut ScalerContextRec) {}

    // `onGlyphMaskNeedsCurrentColor`: false.
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }

    // `onMakeClone`: the same typeface.
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    // `onCreateScalerContext`: `SkScalerContext::MakeEmpty`.
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        ScalerContext::make_empty(this, effects, desc)
    }
}

/// The local `TestFontStyleSet` of `FontMgr_MatchStyleCSS3`.
// Port of: tests/FontMgrTest.cpp#L302-L323 (chrome/m156)
#[derive(Debug)]
struct TestFontStyleSet {
    styles: Vec<FontStyle>,
}

impl FontStyleSetBase for TestFontStyleSet {
    fn count(&self) -> usize {
        self.styles.len()
    }

    fn get_style(&self, index: usize) -> (FontStyle, String) {
        (self.styles[index], String::new())
    }

    fn create_typeface(&self, index: usize) -> Option<Typeface> {
        if self.styles.len() <= index {
            return Some(TestTypeface::make(invalid_font_style()));
        }
        Some(TestTypeface::make(self.styles[index]))
    }

    fn match_style(&self, pattern: &FontStyle) -> Option<Typeface> {
        self.match_style_css3(pattern)
    }
}

/// A `TestFontStyleSet` holding `styles`, as the C++ initializer list does.
fn style_set(styles: Vec<FontStyle>) -> FontStyleSet {
    FontStyleSet::new(Arc::new(TestFontStyleSet { styles }))
}

/// One row of the table: a style set and its (pattern, expected result) cases.
struct StyleSetTest {
    style_set: FontStyleSet,
    cases: Vec<(FontStyle, FontStyle)>,
}

// Port of: tests/FontMgrTest.cpp#L244-L856 (chrome/m156)
def_test!(FontMgr_MatchStyleCSS3, |reporter| {
    let invalid_font_style = invalid_font_style();

    let condensed_normal_100 = FontStyle::new(Weight::THIN, Width::CONDENSED, Slant::Upright);
    let condensed_normal_900 = FontStyle::new(Weight::BLACK, Width::CONDENSED, Slant::Upright);
    let condensed_italic_100 = FontStyle::new(Weight::THIN, Width::CONDENSED, Slant::Italic);
    let condensed_italic_900 = FontStyle::new(Weight::BLACK, Width::CONDENSED, Slant::Italic);
    let condensed_obliqu_100 = FontStyle::new(Weight::THIN, Width::CONDENSED, Slant::Oblique);
    let condensed_obliqu_900 = FontStyle::new(Weight::BLACK, Width::CONDENSED, Slant::Oblique);
    let expanded_normal_100 = FontStyle::new(Weight::THIN, Width::EXPANDED, Slant::Upright);
    let expanded_normal_900 = FontStyle::new(Weight::BLACK, Width::EXPANDED, Slant::Upright);
    let expanded_italic_100 = FontStyle::new(Weight::THIN, Width::EXPANDED, Slant::Italic);
    let expanded_italic_900 = FontStyle::new(Weight::BLACK, Width::EXPANDED, Slant::Italic);
    let expanded_obliqu_100 = FontStyle::new(Weight::THIN, Width::EXPANDED, Slant::Oblique);
    let expanded_obliqu_900 = FontStyle::new(Weight::BLACK, Width::EXPANDED, Slant::Oblique);
    let normal_normal_100 = FontStyle::new(Weight::THIN, Width::NORMAL, Slant::Upright);
    let normal_normal_300 = FontStyle::new(Weight::LIGHT, Width::NORMAL, Slant::Upright);
    let normal_normal_400 = FontStyle::new(Weight::NORMAL, Width::NORMAL, Slant::Upright);
    let normal_normal_500 = FontStyle::new(Weight::MEDIUM, Width::NORMAL, Slant::Upright);
    let normal_normal_600 = FontStyle::new(Weight::SEMI_BOLD, Width::NORMAL, Slant::Upright);
    let normal_normal_900 = FontStyle::new(Weight::BLACK, Width::NORMAL, Slant::Upright);

    let tests = [
        StyleSetTest {
            style_set: style_set(vec![normal_normal_500, normal_normal_400]),
            cases: vec![
                (normal_normal_400, normal_normal_400),
                (normal_normal_500, normal_normal_500),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![normal_normal_500, normal_normal_300]),
            cases: vec![
                (normal_normal_300, normal_normal_300),
                (normal_normal_400, normal_normal_500),
                (normal_normal_500, normal_normal_500),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_normal_100,
                condensed_normal_900,
                condensed_italic_100,
                condensed_italic_900,
                expanded_normal_100,
                expanded_normal_900,
                expanded_italic_100,
                expanded_italic_900,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_normal_100,
                condensed_italic_100,
                expanded_normal_100,
                expanded_italic_100,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_100),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_100),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_100),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_normal_900,
                condensed_italic_900,
                expanded_normal_900,
                expanded_italic_900,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_normal_900),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_italic_900),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, expanded_normal_900),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_italic_900),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_normal_100,
                condensed_normal_900,
                expanded_normal_100,
                expanded_normal_900,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_normal_100),
                (condensed_italic_900, condensed_normal_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_normal_100),
                (expanded_italic_900, expanded_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_100, expanded_normal_100]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_100),
                (condensed_italic_100, condensed_normal_100),
                (condensed_italic_900, condensed_normal_100),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_100),
                (expanded_italic_100, expanded_normal_100),
                (expanded_italic_900, expanded_normal_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_900, expanded_normal_900]),
            cases: vec![
                (condensed_normal_100, condensed_normal_900),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_normal_900),
                (condensed_italic_900, condensed_normal_900),
                (expanded_normal_100, expanded_normal_900),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_normal_900),
                (expanded_italic_900, expanded_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_italic_100,
                condensed_italic_900,
                expanded_italic_100,
                expanded_italic_900,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_italic_100),
                (condensed_normal_900, condensed_italic_900),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, expanded_italic_100),
                (expanded_normal_900, expanded_italic_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_italic_100, expanded_italic_100]),
            cases: vec![
                (condensed_normal_100, condensed_italic_100),
                (condensed_normal_900, condensed_italic_100),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_100),
                (expanded_normal_100, expanded_italic_100),
                (expanded_normal_900, expanded_italic_100),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_italic_900, expanded_italic_900]),
            cases: vec![
                (condensed_normal_100, condensed_italic_900),
                (condensed_normal_900, condensed_italic_900),
                (condensed_italic_100, condensed_italic_900),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, expanded_italic_900),
                (expanded_normal_900, expanded_italic_900),
                (expanded_italic_100, expanded_italic_900),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                condensed_normal_100,
                condensed_normal_900,
                condensed_italic_100,
                condensed_italic_900,
            ]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, condensed_normal_100),
                (expanded_normal_900, condensed_normal_900),
                (expanded_italic_100, condensed_italic_100),
                (expanded_italic_900, condensed_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_100, condensed_italic_100]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_100),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_100),
                (expanded_normal_100, condensed_normal_100),
                (expanded_normal_900, condensed_normal_100),
                (expanded_italic_100, condensed_italic_100),
                (expanded_italic_900, condensed_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_900, condensed_italic_900]),
            cases: vec![
                (condensed_normal_100, condensed_normal_900),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_italic_900),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, condensed_normal_900),
                (expanded_normal_900, condensed_normal_900),
                (expanded_italic_100, condensed_italic_900),
                (expanded_italic_900, condensed_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_100, condensed_normal_900]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_normal_100),
                (condensed_italic_900, condensed_normal_900),
                (expanded_normal_100, condensed_normal_100),
                (expanded_normal_900, condensed_normal_900),
                (expanded_italic_100, condensed_normal_100),
                (expanded_italic_900, condensed_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_100]),
            cases: vec![
                (condensed_normal_100, condensed_normal_100),
                (condensed_normal_900, condensed_normal_100),
                (condensed_italic_100, condensed_normal_100),
                (condensed_italic_900, condensed_normal_100),
                (expanded_normal_100, condensed_normal_100),
                (expanded_normal_900, condensed_normal_100),
                (expanded_italic_100, condensed_normal_100),
                (expanded_italic_900, condensed_normal_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_normal_900]),
            cases: vec![
                (condensed_normal_100, condensed_normal_900),
                (condensed_normal_900, condensed_normal_900),
                (condensed_italic_100, condensed_normal_900),
                (condensed_italic_900, condensed_normal_900),
                (expanded_normal_100, condensed_normal_900),
                (expanded_normal_900, condensed_normal_900),
                (expanded_italic_100, condensed_normal_900),
                (expanded_italic_900, condensed_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_italic_100, condensed_italic_900]),
            cases: vec![
                (condensed_normal_100, condensed_italic_100),
                (condensed_normal_900, condensed_italic_900),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, condensed_italic_100),
                (expanded_normal_900, condensed_italic_900),
                (expanded_italic_100, condensed_italic_100),
                (expanded_italic_900, condensed_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_italic_100]),
            cases: vec![
                (condensed_normal_100, condensed_italic_100),
                (condensed_normal_900, condensed_italic_100),
                (condensed_italic_100, condensed_italic_100),
                (condensed_italic_900, condensed_italic_100),
                (expanded_normal_100, condensed_italic_100),
                (expanded_normal_900, condensed_italic_100),
                (expanded_italic_100, condensed_italic_100),
                (expanded_italic_900, condensed_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![condensed_italic_900]),
            cases: vec![
                (condensed_normal_100, condensed_italic_900),
                (condensed_normal_900, condensed_italic_900),
                (condensed_italic_100, condensed_italic_900),
                (condensed_italic_900, condensed_italic_900),
                (expanded_normal_100, condensed_italic_900),
                (expanded_normal_900, condensed_italic_900),
                (expanded_italic_100, condensed_italic_900),
                (expanded_italic_900, condensed_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                expanded_normal_100,
                expanded_normal_900,
                expanded_italic_100,
                expanded_italic_900,
            ]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_900),
                (condensed_obliqu_100, expanded_italic_100),
                (condensed_obliqu_900, expanded_italic_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
                (expanded_obliqu_100, expanded_italic_100),
                (expanded_obliqu_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_normal_100, expanded_italic_100]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_100),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_100),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_100),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_normal_900, expanded_italic_900]),
            cases: vec![
                (condensed_normal_100, expanded_normal_900),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_italic_900),
                (condensed_italic_900, expanded_italic_900),
                (expanded_normal_100, expanded_normal_900),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_italic_900),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_normal_100, expanded_normal_900]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_normal_100),
                (condensed_italic_900, expanded_normal_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_normal_100),
                (expanded_italic_900, expanded_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_normal_100]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_100),
                (condensed_italic_100, expanded_normal_100),
                (condensed_italic_900, expanded_normal_100),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_100),
                (expanded_italic_100, expanded_normal_100),
                (expanded_italic_900, expanded_normal_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_normal_900]),
            cases: vec![
                (condensed_normal_100, expanded_normal_900),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_normal_900),
                (condensed_italic_900, expanded_normal_900),
                (expanded_normal_100, expanded_normal_900),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_normal_900),
                (expanded_italic_900, expanded_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_italic_100, expanded_italic_900]),
            cases: vec![
                (condensed_normal_100, expanded_italic_100),
                (condensed_normal_900, expanded_italic_900),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_900),
                (expanded_normal_100, expanded_italic_100),
                (expanded_normal_900, expanded_italic_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_italic_100]),
            cases: vec![
                (condensed_normal_100, expanded_italic_100),
                (condensed_normal_900, expanded_italic_100),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_100),
                (expanded_normal_100, expanded_italic_100),
                (expanded_normal_900, expanded_italic_100),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_100),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![expanded_italic_900]),
            cases: vec![
                (condensed_normal_100, expanded_italic_900),
                (condensed_normal_900, expanded_italic_900),
                (condensed_italic_100, expanded_italic_900),
                (condensed_italic_900, expanded_italic_900),
                (expanded_normal_100, expanded_italic_900),
                (expanded_normal_900, expanded_italic_900),
                (expanded_italic_100, expanded_italic_900),
                (expanded_italic_900, expanded_italic_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![normal_normal_100, normal_normal_900]),
            cases: vec![
                (normal_normal_300, normal_normal_100),
                (normal_normal_400, normal_normal_100),
                (normal_normal_500, normal_normal_100),
                (normal_normal_600, normal_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                normal_normal_100,
                normal_normal_400,
                normal_normal_900,
            ]),
            cases: vec![
                (normal_normal_300, normal_normal_100),
                (normal_normal_400, normal_normal_400),
                (normal_normal_500, normal_normal_400),
                (normal_normal_600, normal_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                normal_normal_100,
                normal_normal_500,
                normal_normal_900,
            ]),
            cases: vec![
                (normal_normal_300, normal_normal_100),
                (normal_normal_400, normal_normal_500),
                (normal_normal_500, normal_normal_500),
                (normal_normal_600, normal_normal_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![]),
            cases: vec![
                (normal_normal_300, invalid_font_style),
                (normal_normal_400, invalid_font_style),
                (normal_normal_500, invalid_font_style),
                (normal_normal_600, invalid_font_style),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                expanded_normal_100,
                expanded_normal_900,
                expanded_italic_100,
                expanded_italic_900,
                expanded_obliqu_100,
                expanded_obliqu_900,
            ]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_900),
                (condensed_obliqu_100, expanded_obliqu_100),
                (condensed_obliqu_900, expanded_obliqu_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
                (expanded_obliqu_100, expanded_obliqu_100),
                (expanded_obliqu_900, expanded_obliqu_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                expanded_normal_100,
                expanded_normal_900,
                expanded_obliqu_100,
                expanded_obliqu_900,
            ]),
            cases: vec![
                (condensed_normal_100, expanded_normal_100),
                (condensed_normal_900, expanded_normal_900),
                (condensed_italic_100, expanded_obliqu_100),
                (condensed_italic_900, expanded_obliqu_900),
                (condensed_obliqu_100, expanded_obliqu_100),
                (condensed_obliqu_900, expanded_obliqu_900),
                (expanded_normal_100, expanded_normal_100),
                (expanded_normal_900, expanded_normal_900),
                (expanded_italic_100, expanded_obliqu_100),
                (expanded_italic_900, expanded_obliqu_900),
                (expanded_obliqu_100, expanded_obliqu_100),
                (expanded_obliqu_900, expanded_obliqu_900),
            ],
        },
        StyleSetTest {
            style_set: style_set(vec![
                expanded_italic_100,
                expanded_italic_900,
                expanded_obliqu_100,
                expanded_obliqu_900,
            ]),
            cases: vec![
                (condensed_normal_100, expanded_obliqu_100),
                (condensed_normal_900, expanded_obliqu_900),
                (condensed_italic_100, expanded_italic_100),
                (condensed_italic_900, expanded_italic_900),
                (condensed_obliqu_100, expanded_obliqu_100),
                (condensed_obliqu_900, expanded_obliqu_900),
                (expanded_normal_100, expanded_obliqu_100),
                (expanded_normal_900, expanded_obliqu_900),
                (expanded_italic_100, expanded_italic_100),
                (expanded_italic_900, expanded_italic_900),
                (expanded_obliqu_100, expanded_obliqu_100),
                (expanded_obliqu_900, expanded_obliqu_900),
            ],
        },
    ];
    for test in &tests {
        for &(pattern, expected) in &test.cases {
            if let Some(typeface) = test.style_set.match_style(&pattern) {
                reporter_assert!(reporter, typeface.font_style() == expected);
            } else {
                reporter_assert!(reporter, invalid_font_style == expected);
            }
        }
    }
});

// Port of: tests/FontMgrTest.cpp#L858-L867 (chrome/m156)
def_font_test!(FontMgr_MatchCharacter, |_reporter| {
    let fm = test_font_mgr();
    // 0xD800 <= codepoint <= 0xDFFF || 0x10FFFF < codepoint are invalid
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], 0x0);
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], 0xD800);
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], 0xDFFF);
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], 0x0011_0000);
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], 0x001F_FFFF);
    let _ = fm.match_family_style_character(Some("Blah"), &FontStyle::normal(), &[], -1);
});
