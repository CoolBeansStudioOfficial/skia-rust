// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L74-L116 (DEFINE_string(paragraph_fonts),
// DEFINE_bool(run_paragraph_tests_needing_system_fonts) and the file listing of
// ResourceFontCollection::getTypefaces) (chrome/m156)

//! The fonts of the paragraph tests: Skia's `--paragraph_fonts` and
//! `--run_paragraph_tests_needing_system_fonts` flags as environment variables.
//!
//! | Skia flag | Here |
//! |---|---|
//! | `--paragraph_fonts <dir>` (a subdirectory of `//resources`; empty by default) | `SKIA_RUST_PARAGRAPH_FONTS=<dir>` (relative to Skia's `resources`, or absolute). `release` selects the fonts of `inventory/test-fonts.lock`, fetched and cached like the goldens (`oracle/text-deps`). Unset or empty: no fonts, and the tests that need them return early, as in Skia. |
//! | `--norun_paragraph_tests_needing_system_fonts` | `SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS=0` |
//!
//! Skia's `NativeFonts_Fontations` bot runs `--paragraph_fonts extra_fonts
//! --norun_paragraph_tests_needing_system_fonts`
//! (`infra/bots/gen_tasks_logic/dm_flags.go#L1820-L1834`); the counterpart here is
//! `SKIA_RUST_PARAGRAPH_FONTS=release SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS=0`.

use std::path::PathBuf;
use std::sync::OnceLock;

use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::stream;
use skia_rust_core::typeface::Typeface;
use skia_rust_paragraph::{FontCollection, TypefaceFontProvider};

use crate::font_tool_utils::test_font_mgr;
use crate::resources::resource_dir;

/// `FLAGS_run_paragraph_tests_needing_system_fonts` (default true).
#[must_use]
pub fn run_paragraph_tests_needing_system_fonts() -> bool {
    skia_rust_text_deps::run_paragraph_tests_needing_system_fonts()
}

/// `GetResourcePath(FLAGS_paragraph_fonts[0])`: the font directory, or `None` when
/// `FLAGS_paragraph_fonts` is empty.
///
/// # Errors
/// If `release` is selected and the fonts cannot be fetched or verified.
pub fn paragraph_fonts_dir() -> Result<Option<PathBuf>, String> {
    skia_rust_text_deps::paragraph_fonts_dir(resource_dir().as_deref())
}

/// The font files `ResourceFontCollection::getTypefaces` registers, in its order (sorted by
/// `strcmp`); empty when no fonts are selected.
///
/// # Errors
/// If the fonts cannot be fetched, or the directory cannot be read.
pub fn resource_font_paths() -> Result<Vec<PathBuf>, String> {
    match paragraph_fonts_dir()? {
        Some(dir) => skia_rust_text_deps::font_files_sorted(&dir),
        None => Ok(Vec::new()),
    }
}

/// `FLAGS_paragraph_fonts.size() != 0`: the user asked for the fonts, so a test that cannot find
/// them is an error and not a skip (`SKIP_IF_FONTS_NOT_FOUND`).
#[must_use]
pub fn paragraph_fonts_flag_set() -> bool {
    skia_rust_text_deps::ParagraphFonts::from_env() != skia_rust_text_deps::ParagraphFonts::None
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L118-L172 (chrome/m156), the static
// `getTypefaces` of `ResourceFontCollection`.
/// The typefaces of the `--paragraph_fonts` directory, made by the test font manager
/// (`ToolUtils::TestFontMgr()->makeFromStream`), in the order of the sorted file names. Empty
/// when no fonts are selected. Made once for the process, as the C++ `static` is.
///
/// # Panics
/// If fonts are selected but no typeface can be made from them (Skia: "`--paragraph_fonts` set but
/// no fonts found. Did you set `--nativeFonts`?"), or `Roboto-Italic.ttf` is missing.
fn get_typefaces() -> &'static Vec<Typeface> {
    static TYPEFACES: OnceLock<Vec<Typeface>> = OnceLock::new();
    TYPEFACES.get_or_init(|| {
        let paths = resource_font_paths().expect("the paragraph fonts are readable");
        if paths.is_empty() {
            return Vec::new();
        }

        let mgr = test_font_mgr();
        let mut typefaces: Vec<Typeface> = Vec::new();
        let mut fonts_found = false;
        for path in &paths {
            let stream = stream::make_from_file(path);
            // `SkASSERTF(stream, "%s not readable", path.c_str())`
            let stream = stream.unwrap_or_else(|| panic!("{} not readable", path.display()));
            let typeface = mgr.make_from_stream_args(Some(stream), &FontArguments::new());
            // Without --nativeFonts, DM will use the portable test font manager which does
            // not know how to read in fonts from bytes.
            if let Some(typeface) = typeface {
                if path.to_string_lossy().ends_with("Roboto-Italic.ttf") {
                    fonts_found = true;
                }
                typefaces.push(typeface);
            }
        }
        assert!(
            !typefaces.is_empty(),
            "--paragraph_fonts set but no fonts found. Did you set --nativeFonts?"
        );
        assert!(
            fonts_found,
            "--paragraph_fonts set but Roboto-Italic.ttf not found"
        );
        typefaces
    })
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L118-L203 (chrome/m156)
/// `ResourceFontCollection`: a `FontCollection` of the test fonts. It dereferences to its
/// [`FontCollection`].
#[derive(Debug, Clone)]
pub struct ResourceFontCollection {
    font_collection: FontCollection,
    fonts_found: bool,
}

impl std::ops::Deref for ResourceFontCollection {
    type Target = FontCollection;

    fn deref(&self) -> &FontCollection {
        &self.font_collection
    }
}

impl std::ops::DerefMut for ResourceFontCollection {
    fn deref_mut(&mut self) -> &mut FontCollection {
        &mut self.font_collection
    }
}

impl From<&ResourceFontCollection> for FontCollection {
    fn from(collection: &ResourceFontCollection) -> Self {
        collection.font_collection.clone()
    }
}

impl From<ResourceFontCollection> for FontCollection {
    fn from(collection: ResourceFontCollection) -> Self {
        collection.font_collection
    }
}

impl ResourceFontCollection {
    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L173-L190 (chrome/m156)
    /// `ResourceFontCollection(testOnly)`: registers the test fonts with a
    /// `TypefaceFontProvider` and makes it the asset font manager (or the test font manager),
    /// with the font fallback disabled.
    #[must_use]
    pub fn new(test_only: bool) -> Self {
        let typefaces = get_typefaces();
        let mut font_provider = TypefaceFontProvider::new();
        for typeface in typefaces {
            font_provider.register_typeface(typeface.clone(), None);
        }

        let mut font_collection = FontCollection::new();
        if test_only {
            font_collection.set_test_font_manager(font_provider.font_mgr());
        } else {
            font_collection.set_asset_font_manager(font_provider.font_mgr());
        }
        font_collection.disable_font_fallback();
        Self {
            font_collection,
            fonts_found: !typefaces.is_empty(),
        }
    }

    /// `fontsFound()`: false when no fonts are selected (a test skips itself then).
    #[must_use]
    #[doc(alias = "fontsFound")]
    pub fn fonts_found(&self) -> bool {
        self.fonts_found
    }

    /// `resolvedFonts()`: always zero, as in C++ (`fResolvedFonts` is never changed).
    #[must_use]
    #[doc(alias = "resolvedFonts")]
    pub fn resolved_fonts(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fonts_unless_selected() {
        if std::env::var_os("SKIA_RUST_PARAGRAPH_FONTS").is_none() {
            assert_eq!(paragraph_fonts_dir(), Ok(None));
            assert_eq!(resource_font_paths(), Ok(Vec::new()));
        }
    }
}
