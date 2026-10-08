// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/FontToolUtils.{h,cpp} (chrome/m156), the portable and test
// typeface subset. Emoji samples, the sample user typeface and the native managers are not here.

//! `ToolUtils`' font helpers: the typefaces and fonts that the tests and GMs draw with.
//!
//! GMs always use the portable configuration. Unit tests that make typefaces from the test
//! manager run under both configurations ([`FontConfig`]; docs/design/text.md §8).

use std::cell::Cell;
use std::sync::OnceLock;

use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::text_blob::TextBlobBuilder;
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

/// The font configurations a unit test can run under (docs/design/text.md §8). GMs always run
/// under [`FontConfig::Portable`], the GM oracle's configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontConfig {
    /// `--nativeFonts false`: the portable test manager.
    Portable,
    /// `--fontations`: the test manager is the empty Fontations manager, as in Skia's
    /// `NativeFonts_Fontations` bots.
    NativeFontations,
}

thread_local! {
    /// The configuration of the test running on this thread (set by [`with_font_config`]).
    static FONT_CONFIG: Cell<FontConfig> = const { Cell::new(FontConfig::Portable) };
}

/// The configuration that [`test_font_mgr`] uses on this thread.
#[must_use]
pub fn font_config() -> FontConfig {
    FONT_CONFIG.with(Cell::get)
}

/// Runs `body` with `config` as the configuration of this thread, then restores the previous
/// one (also when `body` panics).
pub fn with_font_config<R>(config: FontConfig, body: impl FnOnce() -> R) -> R {
    struct Restore(FontConfig);
    impl Drop for Restore {
        fn drop(&mut self) {
            FONT_CONFIG.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(FONT_CONFIG.with(|c| c.replace(config)));
    body()
}

/// `ToolUtils::TestFontMgr()`: the portable manager (`--nativeFonts false`, the GM oracle's
/// configuration), or in [`FontConfig::NativeFontations`] the empty Fontations manager.
// Port of: tools/fonts/FontToolUtils.cpp#L274-L283 (chrome/m156), the nativeFonts=false branch,
// and the Fontations branch of the NativeFonts_Fontations bots
#[doc(alias = "TestFontMgr")]
#[must_use]
pub fn test_font_mgr() -> FontMgr {
    match font_config() {
        FontConfig::Portable => portable_font_mgr().clone(),
        FontConfig::NativeFontations => {
            skia_rust_text::ports::fontations::font_mgr::new_fontations_empty()
        }
    }
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

/// `ToolUtils::EmojiFontFormat`: the emoji test fonts a GM can draw with.
// Port of: tools/fonts/FontToolUtils.h#L40-L46 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiFontFormat {
    /// `kCbdt`: embedded color bitmaps (`fonts/cbdt.ttf`).
    Cbdt,
    /// `kSbix`: Apple's bitmap format (`fonts/sbix.ttf`).
    Sbix,
    /// `kColrV0`: COLR version 0 (`fonts/colr.ttf`).
    ColrV0,
    /// `kTest`: the portable `Emoji` family (`TestSVGTypeface`).
    Test,
    /// `kSvg`: OpenType SVG (`fonts/SampleSVG.ttf`).
    Svg,
}

/// `ToolUtils::EmojiTestSample`: an emoji typeface and the text to draw with it.
// Port of: tools/fonts/FontToolUtils.h#L48-L51 (chrome/m156)
#[derive(Clone, Debug)]
pub struct EmojiTestSample {
    /// `typeface`: `None` when the resource is missing (every resource is, in the portable
    /// configuration).
    pub typeface: Option<Typeface>,
    /// `sampleText`.
    pub sample_text: &'static str,
}

/// `ToolUtils::NameForFontFormat`.
// Port of: tools/fonts/FontToolUtils.cpp#L166-L180 (chrome/m156)
#[must_use]
pub fn name_for_font_format(format: EmojiFontFormat) -> &'static str {
    match format {
        EmojiFontFormat::Cbdt => "cbdt",
        EmojiFontFormat::Sbix => "sbix",
        EmojiFontFormat::ColrV0 => "colrv0",
        EmojiFontFormat::Test => "test",
        EmojiFontFormat::Svg => "svg",
    }
}

/// `ToolUtils::EmojiSample(format)` for the resource formats. Each is `CreateTypefaceFromResource`
/// of a file under `fonts/`, which the portable manager cannot read, so the typeface is `None`
/// exactly as in the goldens (docs/design/text.md §1.2). The GMs that draw with them skip.
///
/// # Panics
///
/// For [`EmojiFontFormat::Test`], which is `CreatePortableTypeface("Emoji")` and needs
/// `TestSVGTypeface` (docs/design/text.md T20), not ported yet.
// Port of: tools/fonts/FontToolUtils.cpp#L143-L165 (chrome/m156), the resource branches
#[must_use]
pub fn emoji_sample(format: EmojiFontFormat) -> EmojiTestSample {
    let sample_text = "\u{1F600} \u{2662}";
    match format {
        EmojiFontFormat::Cbdt | EmojiFontFormat::Sbix | EmojiFontFormat::ColrV0 => {
            EmojiTestSample {
                typeface: create_typeface_from_resource(None, 0),
                sample_text,
            }
        }
        EmojiFontFormat::Svg => EmojiTestSample {
            typeface: create_typeface_from_resource(None, 0),
            sample_text: "abcdefghij",
        },
        EmojiFontFormat::Test => {
            panic!("EmojiSample(Test) needs TestSVGTypeface, which is not ported yet (T20)")
        }
    }
}

/// `ToolUtils::add_to_text_blob_w_len(builder, text, encoding, font, x, y)`: adds a run of the
/// glyphs of `text` at `(x, y)`, unless there are none.
// Port of: tools/ToolUtils.cpp#L216-L229 (chrome/m156)
pub fn add_to_text_blob_w_len(
    builder: &mut TextBlobBuilder,
    text: &[u8],
    encoding: TextEncoding,
    font: &Font,
    x: scalar,
    y: scalar,
) {
    let count = font.count_text(text, encoding);
    if count < 1 {
        return;
    }
    let glyphs = builder.alloc_run(font, count, x, y, None);
    font.text_to_glyphs(text, encoding, glyphs);
}

/// `ToolUtils::add_to_text_blob(builder, text, font, x, y)` for a UTF-8 string.
// Port of: tools/ToolUtils.cpp#L231-L237 (chrome/m156)
pub fn add_to_text_blob(
    builder: &mut TextBlobBuilder,
    text: &str,
    font: &Font,
    x: scalar,
    y: scalar,
) {
    add_to_text_blob_w_len(builder, text.as_bytes(), TextEncoding::UTF8, font, x, y);
}

/// `ToolUtils::get_text_path(font, text, encoding, pos)`: the outline of the glyphs of `text`,
/// each at its position in `pos` (or at the font's own positions when `pos` is `None`).
// Port of: tools/ToolUtils.cpp#L239-L268 (chrome/m156)
#[must_use]
pub fn get_text_path(
    font: &Font,
    text: &[u8],
    encoding: TextEncoding,
    pos: Option<&[skia_rust_core::point::Point]>,
) -> skia_rust_core::path::Path {
    let count = font.count_text(text, encoding);
    let mut glyphs = vec![0; count];
    font.text_to_glyphs(text, encoding, &mut glyphs);
    let computed: Vec<skia_rust_core::point::Point>;
    let pos = if let Some(pos) = pos {
        pos
    } else {
        let mut positions = vec![skia_rust_core::point::Point::default(); count];
        font.get_pos(
            &glyphs,
            &mut positions,
            skia_rust_core::point::Point::default(),
        );
        computed = positions;
        &computed
    };

    let mut builder = skia_rust_core::path_builder::PathBuilder::new();
    let mut index = 0;
    font.get_paths(&glyphs, |src, mx| {
        if let Some(src) = src {
            let mut tmp = mx.clone();
            tmp.post_translate(pos[index]);
            builder.add_path_with_transform(src, &tmp, None);
        }
        index += 1;
    });
    builder.detach()
}
