// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/fontations/src/names.rs (chrome/m156), with the `cxx` FFI replaced by a plain
// Rust API.

//! Localized names of a font: the family name iterator, and the family and PostScript names.

use read_fonts::{TableProvider, tables::os2::SelectionFlags};
use skia_rust_core::typeface::LocalizedString;
use skrifa::{MetadataProvider, string::StringId};

use super::base::BridgeFontRef;

/// Port of `get_localized_strings` followed by `localized_name_next` until it is exhausted: the
/// family names of the font, in skrifa's order. C++ streams them from a skrifa iterator that
/// borrows the font; the port collects them eagerly, which yields the same sequence without a
/// self-referential iterator.
// Port of: src/ports/fontations/src/names.rs#L13-L43 (chrome/m156)
#[must_use]
pub fn get_localized_strings(font_ref: &BridgeFontRef<'_>) -> Vec<LocalizedString> {
    font_ref
        .with_font(|f| {
            Some(
                f.localized_strings(StringId::FAMILY_NAME)
                    .map(|localized_string| LocalizedString {
                        string: localized_string.to_string(),
                        language: localized_string
                            .language()
                            .map(str::to_owned)
                            .unwrap_or_default(),
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

/// Port of `english_or_first_font_name`: the name with `name_id` in English, or the first one.
// Port of: src/ports/fontations/src/names.rs#L45-L51 (chrome/m156)
#[must_use]
pub fn english_or_first_font_name(
    font_ref: &BridgeFontRef<'_>,
    name_id: StringId,
) -> Option<String> {
    font_ref.with_font(|f| {
        f.localized_strings(name_id)
            .english_or_first()
            .map(|localized_string| localized_string.to_string())
    })
}

/// Port of `family_name`: the typographic family name, unless the WWS bit says otherwise.
// Port of: src/ports/fontations/src/names.rs#L53-L70 (chrome/m156)
#[must_use]
pub fn family_name(font_ref: &BridgeFontRef<'_>) -> String {
    font_ref
        .with_font(|f| {
            // https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fsselection
            // Bit 8 of the `fsSelection' field in the `OS/2' table indicates a WWS-only font face.
            // When this bit is set it means *do not* use the WWS strings.
            let use_wws = !f
                .os2()
                .is_ok_and(|t| t.fs_selection().contains(SelectionFlags::WWS));
            use_wws
                .then(|| english_or_first_font_name(font_ref, StringId::WWS_FAMILY_NAME))
                .flatten()
                .or_else(|| english_or_first_font_name(font_ref, StringId::TYPOGRAPHIC_FAMILY_NAME))
                .or_else(|| english_or_first_font_name(font_ref, StringId::FAMILY_NAME))
        })
        .unwrap_or_default()
}

/// Port of `postscript_name`: the PostScript name, or `None` if the font has none.
// Port of: src/ports/fontations/src/names.rs#L72-L81 (chrome/m156)
#[must_use]
pub fn postscript_name(font_ref: &BridgeFontRef<'_>) -> Option<String> {
    english_or_first_font_name(font_ref, StringId::POSTSCRIPT_NAME)
}
