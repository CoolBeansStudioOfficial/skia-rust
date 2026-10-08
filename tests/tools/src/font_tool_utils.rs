// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/FontToolUtils.{h,cpp} (chrome/m156), the portable and test
// typeface subset. Emoji samples, the sample user typeface and the native managers are not here.

//! `ToolUtils`' font helpers: the typefaces and fonts that the tests and GMs draw with.
//!
//! The portable configuration is the only one: GMs always use it, and unit tests will run it
//! beside the native-Fontations configuration once T19b lands (docs/design/text.md §8).

use std::sync::OnceLock;

use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;

use crate::fonts::test_font_mgr::make_portable_font_mgr;

/// The point size of the default fonts (`SkFont(typeface, 12)`).
const DEFAULT_TEXT_SIZE: f32 = 12.0;

/// `ToolUtils::PortableFontMgr`'s cached instance: `CreatePortableTypeface` uses one manager for
/// the process (`static sk_sp<SkFontMgr> portableFontMgr`).
// Port of: tools/fonts/FontToolUtils.cpp#L163-L165 (chrome/m156)
fn portable_font_mgr() -> &'static FontMgr {
    static MGR: OnceLock<FontMgr> = OnceLock::new();
    MGR.get_or_init(make_portable_font_mgr)
}

/// `ToolUtils::CreatePortableTypeface(name, style)`: a typeface from the portable manager.
/// `None` is C++'s null name.
///
/// # Panics
///
/// Panics if the portable manager has no typeface for the name and style, which the test data
/// never lacks (its `legacyMakeTypeface` falls back to the default face).
// Port of: tools/fonts/FontToolUtils.cpp#L199-L206 (chrome/m156)
#[must_use]
pub fn create_portable_typeface(name: Option<&str>, style: FontStyle) -> Typeface {
    portable_font_mgr()
        .legacy_make_typeface(name, style)
        .expect("the portable manager always has a typeface for a style")
}

/// `ToolUtils::DefaultPortableTypeface()`: the default face of the portable manager (a sans-serif
/// in m156, from `gDefaultFontIndex`).
// Port of: tools/fonts/FontToolUtils.cpp#L208-L213 (chrome/m156)
#[must_use]
pub fn default_portable_typeface() -> Typeface {
    create_portable_typeface(None, FontStyle::default())
}

/// `ToolUtils::DefaultPortableFont()`: `SkFont(DefaultPortableTypeface(), 12)`.
// Port of: tools/fonts/FontToolUtils.cpp#L215-L217 (chrome/m156)
#[must_use]
pub fn default_portable_font() -> Font {
    Font::from_size(default_portable_typeface(), DEFAULT_TEXT_SIZE)
}

/// `ToolUtils::TestFontMgr()` in the portable configuration (`--nativeFonts false`, the GM
/// oracle's configuration): the portable manager.
// Port of: tools/fonts/FontToolUtils.cpp#L274-L283 (chrome/m156), the nativeFonts=false branch
#[doc(alias = "TestFontMgr")]
#[must_use]
pub fn test_font_mgr() -> FontMgr {
    portable_font_mgr().clone()
}

/// `ToolUtils::CreateTestTypeface(name, style)`: a typeface from the test manager, or the
/// portable one when it has none.
// Port of: tools/fonts/FontToolUtils.cpp#L338-L346 (chrome/m156)
#[must_use]
pub fn create_test_typeface(name: Option<&str>, style: FontStyle) -> Typeface {
    if let Some(face) = test_font_mgr().legacy_make_typeface(name, style) {
        return face;
    }
    create_portable_typeface(name, style)
}

/// `ToolUtils::DefaultTypeface()`: `CreateTestTypeface(nullptr, SkFontStyle())`.
// Port of: tools/fonts/FontToolUtils.cpp#L332-L334 (chrome/m156)
#[must_use]
pub fn default_typeface() -> Typeface {
    create_test_typeface(None, FontStyle::default())
}

/// `ToolUtils::DefaultFont()`: `SkFont(DefaultTypeface(), 12)`.
// Port of: tools/fonts/FontToolUtils.cpp#L348-L350 (chrome/m156)
#[must_use]
pub fn default_font() -> Font {
    Font::from_size(default_typeface(), DEFAULT_TEXT_SIZE)
}

/// `ToolUtils::CreateTypefaceFromResource(resource)`: the typeface of a resource, through the
/// test manager. `resource` is the stream of the named resource (`GetResourceAsStream`), and
/// `None` when it cannot be opened, as in C++. In the portable configuration the manager reads
/// no font data, so the result is always `None` (docs/design/text.md §1.2).
// Port of: tools/fonts/FontToolUtils.cpp#L352-L356 (chrome/m156)
#[must_use]
pub fn create_typeface_from_resource(
    resource: Option<Box<dyn StreamAsset>>,
    tt_index: i32,
) -> Option<Typeface> {
    test_font_mgr().make_from_stream(resource, tt_index)
}
