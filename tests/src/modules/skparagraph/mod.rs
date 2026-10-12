//! Ports of `modules/skparagraph/tests`.
//!
//! The macros stand for the `SKIP_IF_FONTS_NOT_FOUND` and `NEED_SYSTEM_FONTS` macros of
//! `SkParagraphTest.cpp`, and are in scope in the test modules below them.

/// Port of `SKIP_IF_FONTS_NOT_FOUND(r, fontCollection)`: skip tests which do not find the
/// fonts, unless the user set `--paragraph_fonts` (`SKIA_RUST_PARAGRAPH_FONTS`), in which case
/// we should make a loud error.
// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L289-L296 (chrome/m156)
macro_rules! skip_if_fonts_not_found {
    ($r:expr, $font_collection:expr) => {
        if !$crate::modules::skparagraph::FontsFound::any_fonts_found(&$font_collection) {
            if ::skia_rust_tools::paragraph_fonts::paragraph_fonts_flag_set() {
                $crate::errorf!($r, "SkParagraphTests Fonts not found!\n");
            }
            return;
        }
    };
}

/// Port of `NEED_SYSTEM_FONTS(fontCollection)`: some tests are finicky and need certain system
/// fonts. `SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS=0` skips them.
// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L298-L303 (chrome/m156)
#[allow(unused_macros)]
macro_rules! need_system_fonts {
    ($font_collection:expr) => {
        if !::skia_rust_tools::paragraph_fonts::run_paragraph_tests_needing_system_fonts() {
            return;
        }
        $font_collection
            .set_default_font_manager(::skia_rust_tools::font_tool_utils::test_font_mgr());
        $font_collection.enable_font_fallback();
    };
}

/// `fontCollection->fontsFound()` of the two kinds of test font collections (a `bool` for
/// `ResourceFontCollection` and a count for `TestFontCollection`) as a condition.
#[cfg(test)]
pub trait FontsFound {
    /// True if the collection found fonts.
    fn any_fonts_found(&self) -> bool;
}

#[cfg(test)]
impl FontsFound for ::skia_rust_tools::paragraph_fonts::ResourceFontCollection {
    fn any_fonts_found(&self) -> bool {
        self.fonts_found()
    }
}

#[cfg(test)]
impl FontsFound for ::skia_rust_paragraph::utils::TestFontCollection {
    fn any_fonts_found(&self) -> bool {
        self.fonts_found() != 0
    }
}

#[cfg(test)]
pub mod sk_paragraph_test;
#[cfg(test)]
pub mod sk_shaper_json_writer_test;
