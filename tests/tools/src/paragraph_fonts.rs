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
