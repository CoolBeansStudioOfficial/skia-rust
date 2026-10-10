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
use std::sync::{Arc, Once, OnceLock};

use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet, TypefaceDecoder};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;

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

/// `ToolUtils::CreateStringBitmap`: a `w` by `h` premultiplied N32 bitmap with `text` drawn in
/// `color` at `(x, y)` in the default portable typeface at `text_size`. The background is
/// transparent.
///
/// # Panics
/// If the bitmap's canvas cannot be made.
// Port of: tools/fonts/FontToolUtils.cpp#L239-L263 (chrome/m156)
#[must_use]
#[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar: the coordinates are small integers
pub fn create_string_bitmap(
    w: i32,
    h: i32,
    color: skia_rust_core::color::Color,
    x: i32,
    y: i32,
    text_size: i32,
    text: &str,
) -> skia_rust_core::bitmap::Bitmap {
    use skia_rust_core::bitmap::Bitmap;
    use skia_rust_core::canvas::Canvas;
    use skia_rust_core::color::Color;
    use skia_rust_core::font::Font;
    use skia_rust_core::paint::Paint;
    use skia_rust_core::point::Point;
    use skia_rust_raster::raster_canvas::RasterCanvas;

    // `bitmap.allocN32Pixels(w, h)` and the final `setInfo(MakeS32(w, h, kPremul))` describe the
    // same pixels, so the bitmap is returned as it was drawn.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((w, h), false);
    {
        let canvas = Canvas::from_bitmap(&mut bitmap, None).expect("a canvas for the bitmap");
        canvas.clear(Color::new(0x0000_0000));
        let mut paint = Paint::default();
        paint.set_color(color);
        let font = Font::from_size(default_portable_typeface(), text_size as f32);
        canvas.draw_simple_text(
            text.as_bytes(),
            TextEncoding::UTF8,
            Point::new(x as f32, y as f32),
            &font,
            &paint,
        );
    }
    bitmap
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
    if config == FontConfig::NativeFontations {
        register_native_decoders();
    }
    let _restore = RestoreFontConfig(FONT_CONFIG.with(|c| c.replace(config)));
    body()
}

/// Restores the previous [`FontConfig`] of the thread when dropped (see [`with_font_config`]).
struct RestoreFontConfig(FontConfig);

impl Drop for RestoreFontConfig {
    fn drop(&mut self) {
        FONT_CONFIG.with(|c| c.set(self.0));
    }
}

/// The Fontations decoder, registered once for the process as Skia's test setup does for
/// `SK_TYPEFACE_FACTORY_FONTATIONS` (`SkTypeface::Register`, docs/design/text.md §5.2). The
/// registry is process-wide, so after the first [`FontConfig::NativeFontations`] run every later
/// `Typeface::make_deserialize` in the test binary decodes Fontations descriptors, in any
/// configuration. Only this configuration registers it.
// Port of: src/ports/SkTypeface_fontations.cpp#L27-L43 (chrome/m156), `SkTypeface::Register` call
fn register_native_decoders() {
    static REGISTERED: Once = Once::new();
    REGISTERED.call_once(|| {
        Typeface::register_decoder(
            skia_rust_text::ports::fontations::typeface::FACTORY_ID,
            skia_rust_text::ports::fontations::typeface::make_from_stream,
        );
    });
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
        // C++ registers the test typefaces globally (`TestTypeface::Register`), so they decode in
        // every configuration. The Fontations manager cannot list them, so this wrapper does.
        FontConfig::NativeFontations => with_typeface_decoders(
            skia_rust_text::ports::fontations::font_mgr::new_fontations_empty(),
            vec![
                TypefaceDecoder {
                    factory_id: crate::fonts::test_typeface::TestTypeface::FACTORY_ID,
                    make_from_stream: crate::fonts::test_typeface::TestTypeface::make_from_stream,
                },
                TypefaceDecoder {
                    factory_id:
                        crate::fonts::test_svg_typeface::TestSvgTypeface::DEFAULT_FACTORY_ID,
                    make_from_stream:
                        crate::fonts::test_svg_typeface::TestSvgTypeface::make_default_from_stream,
                },
                TypefaceDecoder {
                    factory_id:
                        crate::fonts::test_svg_typeface::TestSvgTypeface::PLANETS_FACTORY_ID,
                    make_from_stream:
                        crate::fonts::test_svg_typeface::TestSvgTypeface::make_planets_from_stream,
                },
                TypefaceDecoder {
                    factory_id: skia_rust_text::utils::custom_typeface::FACTORY_ID,
                    make_from_stream: skia_rust_text::utils::custom_typeface::make_from_stream,
                },
            ],
        ),
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
/// [`EmojiFontFormat::Test`] is `CreatePortableTypeface("Emoji")`, the `TestSVGTypeface`.
// Port of: tools/fonts/FontToolUtils.cpp#L143-L165 (chrome/m156)
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
        EmojiFontFormat::Test => EmojiTestSample {
            typeface: Some(create_portable_typeface(
                Some("Emoji"),
                FontStyle::default(),
            )),
            sample_text,
        },
    }
}

/// `ToolUtils::EmojiSample()`: the platform's emoji font (a resource, so missing in the portable
/// configuration), or the test emoji font.
// Port of: tools/fonts/FontToolUtils.cpp#L122-L141 (chrome/m156)
#[must_use]
pub fn emoji_sample_default() -> EmojiTestSample {
    static SAMPLE: OnceLock<EmojiTestSample> = OnceLock::new();
    SAMPLE
        .get_or_init(|| {
            // Linux: `EmojiSample(EmojiFontFormat::Cbdt)` (Windows: ColrV0, macOS: Sbix), which
            // reads a resource.
            let sample = emoji_sample(EmojiFontFormat::Cbdt);
            if sample.typeface.is_some() {
                return sample;
            }
            emoji_sample(EmojiFontFormat::Test)
        })
        .clone()
}

/// `ToolUtils::PlanetTypeface()`: the planets font resource (missing in the portable
/// configuration), or the test typeface of the `Planet` family.
// Port of: tools/fonts/FontToolUtils.cpp#L106-L120 (chrome/m156)
#[must_use]
pub fn planet_typeface() -> Typeface {
    static PLANET_TYPEFACE: OnceLock<Typeface> = OnceLock::new();
    PLANET_TYPEFACE
        .get_or_init(|| {
            if let Some(typeface) = create_typeface_from_resource(None, 0) {
                return typeface;
            }
            create_test_typeface(Some("Planet"), FontStyle::default())
        })
        .clone()
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

/// A manager that answers like `inner` and also decodes with `extra` (`SkTypeface::Register`
/// adds such decoders to C++'s global list; here the test that needs one wraps the manager).
/// Every lookup is delegated unchanged.
#[derive(Debug)]
struct WithTypefaceDecoders {
    inner: FontMgr,
    extra: Vec<TypefaceDecoder>,
}

impl FontMgrBase for WithTypefaceDecoders {
    fn on_count_families(&self) -> usize {
        self.inner.count_families()
    }

    fn on_get_family_name(&self, index: usize) -> String {
        self.inner.family_name(index)
    }

    fn on_create_style_set(&self, index: usize) -> Option<FontStyleSet> {
        Some(self.inner.create_style_set(index))
    }

    fn on_match_family(&self, family_name: Option<&str>) -> Option<FontStyleSet> {
        Some(self.inner.match_family(family_name))
    }

    fn on_match_family_style(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
    ) -> Option<Typeface> {
        self.inner.match_family_style(family_name, style)
    }

    fn on_match_family_style_character(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
        bcp47: &[&str],
        character: Unichar,
    ) -> Option<Typeface> {
        self.inner
            .match_family_style_character(family_name, style, bcp47, character)
    }

    fn on_make_from_data(&self, data: &Data, tt_index: i32) -> Option<Typeface> {
        self.inner.make_from_data(Some(data), tt_index)
    }

    fn on_make_from_stream_index(
        &self,
        stream: Box<dyn StreamAsset>,
        tt_index: i32,
    ) -> Option<Typeface> {
        self.inner.make_from_stream(Some(stream), tt_index)
    }

    fn on_make_from_stream_args(
        &self,
        stream: Box<dyn StreamAsset>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        self.inner.make_from_stream_args(Some(stream), args)
    }

    fn on_make_from_file(&self, path: &str, tt_index: i32) -> Option<Typeface> {
        self.inner.make_from_file(Some(path), tt_index)
    }

    fn on_legacy_make_typeface(
        &self,
        family_name: Option<&str>,
        style: FontStyle,
    ) -> Option<Typeface> {
        self.inner.legacy_make_typeface(family_name, style)
    }

    fn typeface_decoders(&self) -> Vec<TypefaceDecoder> {
        let mut decoders = self.inner.typeface_decoders();
        decoders.extend(self.extra.iter().copied());
        decoders
    }
}

/// `inner` with `extra` added to its typeface decoders.
#[must_use]
pub fn with_typeface_decoders(inner: FontMgr, extra: Vec<TypefaceDecoder>) -> FontMgr {
    FontMgr::new(Arc::new(WithTypefaceDecoders { inner, extra }))
}

/// `ToolUtils::SampleUserTypeface()`: a custom typeface of 68 circle glyphs, drawn at 1/200 em,
/// with metrics and an oblique style.
///
/// # Panics
///
/// Never: the builder always has glyphs.
// Port of: tools/fonts/FontToolUtils.cpp#L182-L218 (chrome/m156)
#[must_use]
pub fn sample_user_typeface() -> Typeface {
    use skia_rust_core::font_metrics::{Flags, FontMetrics};
    use skia_rust_core::font_style::{Slant, Weight, Width};
    use skia_rust_core::matrix::Matrix;
    use skia_rust_core::path::Path;
    use skia_rust_core::path_types::PathDirection;
    use skia_rust_text::utils::custom_typeface::CustomTypefaceBuilder;

    let mut builder = CustomTypefaceBuilder::new();
    let upem: scalar = 200.0;
    {
        let metrics = FontMetrics {
            flags: Flags::empty(),
            top: -200.0,
            ascent: -150.0,
            descent: 50.0,
            bottom: -75.0,
            leading: 10.0,
            avg_char_width: 150.0,
            max_char_width: 300.0,
            x_min: -20.0,
            x_max: 290.0,
            x_height: -100.0,
            cap_height: 0.0,
            underline_thickness: 5.0,
            underline_position: 2.0,
            strikeout_thickness: 5.0,
            strikeout_position: -50.0,
        };
        builder.set_metrics(&metrics, 1.0 / upem);
    }
    builder.set_font_style(FontStyle::new(
        Weight::from(367),
        Width::from(3),
        Slant::Oblique,
    ));

    let scale = Matrix::scale((1.0 / upem, 1.0 / upem));
    for index in 0..=67 {
        let width: scalar = 100.0;
        let circle = Path::circle((50.0, -50.0), 75.0, PathDirection::CW).make_transform(&scale);
        builder.set_glyph(index, width / upem, &circle);
    }

    builder.detach().expect("the sample has glyphs")
}
