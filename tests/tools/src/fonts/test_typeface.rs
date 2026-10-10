// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/TestTypeface.{h,cpp} (chrome/m156)

//! [`TestTypeface`]: the portable test font. Its glyphs are outlines stored as Skia's verbs and
//! points (`test_font_data`), so every backend that reads it draws the same shapes.

use std::sync::{Arc, OnceLock};

use skia_rust_core::advanced_typeface_metrics::AdvancedTypefaceMetrics;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::fixed::fixed_to_float;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_priv::scale_font_metrics;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::font_types::{FontHinting, FourByteTag, GlyphId};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::mask::MaskBuilder;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, ScalerContext, ScalerContextBase, ScalerContextEffects,
    ScalerContextImpl, ScalerContextRec,
};
use skia_rust_core::stream::{DynamicMemoryWStream, StreamAsset, WStream};
use skia_rust_core::typeface::{
    LocalizedString, LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_core::utf::Unichar;
use skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER;

/// `gHeaderString`: the header of a serialized test typeface. `sizeof` includes the NUL.
// Port of: tools/fonts/TestTypeface.cpp#L149-L150 (chrome/m156)
const HEADER: &[u8] = b"SkTestTypeface01\0";

use super::test_font_data::index::{DEFAULT_FONT_INDEX, SUB_FONTS, TEST_FONTS};

/// One font of the test data (`SkTestFontData`).
// Port of: tools/fonts/TestTypeface.h#L20-L28 (chrome/m156)
#[doc(alias = "SkTestFontData")]
#[derive(Debug)]
pub struct TestFontData {
    /// `fPoints`: the outline points, x and y interleaved.
    pub points: &'static [f32],
    /// `fVerbs`: one `SkPath::Verb` byte per segment, with `kDone` (6) ending each glyph.
    pub verbs: &'static [u8],
    /// `fCharCodes`: the unichar of each glyph.
    pub char_codes: &'static [Unichar],
    /// `fWidths`: the advance of each glyph, as `SkFixed`.
    pub widths: &'static [i32],
    /// `fMetrics`.
    pub metrics: &'static FontMetrics,
    /// `fName`: the family name, `Toy Liberation ...`.
    pub name: &'static str,
    /// The weight of `fStyle`.
    pub weight: i32,
    /// The width of `fStyle`.
    pub width: i32,
    /// The slant of `fStyle`.
    pub slant: Slant,
}

/// One face of a family in the test data (`gSubFonts`).
// Port of: tools/fonts/test_font_index.inc (SubFont, chrome/m156)
#[derive(Debug)]
pub struct SubFont {
    /// `fFamilyName`.
    pub family: &'static str,
    /// `fStyleName`.
    pub style: &'static str,
    /// The weight of `fStyle`.
    pub weight: i32,
    /// The width of `fStyle`.
    pub width: i32,
    /// The slant of `fStyle`.
    pub slant: Slant,
    /// `fFont`: an index into `gTestFonts`.
    pub font: usize,
    /// `fFile`: the file the face came from in Skia's resources (informational).
    pub file: &'static str,
}

/// `SkTestFont`: a test font with its glyph paths built once.
// Port of: tools/fonts/TestTypeface.h#L30-L49 (chrome/m156)
#[doc(alias = "SkTestFont")]
pub struct TestFont {
    char_codes: &'static [Unichar],
    widths: &'static [i32],
    metrics: &'static FontMetrics,
    name: &'static str,
    paths: Vec<Path>,
}

impl std::fmt::Debug for TestFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestFont")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl TestFont {
    /// `SkTestFont::SkTestFont`: builds the glyph paths from the verbs and points.
    // Port of: tools/fonts/TestTypeface.cpp#L50-L65 (chrome/m156)
    #[must_use]
    pub fn new(data: &'static TestFontData) -> Self {
        let char_codes = data.char_codes;
        Self {
            char_codes,
            widths: data.widths,
            metrics: data.metrics,
            name: data.name,
            paths: init(data.points, data.verbs, char_codes.len()),
        }
    }

    /// `SkTestFont::glyphForUnichar`: the first glyph for `char_code`, or 0.
    ///
    /// # Panics
    ///
    /// Panics if the index of the glyph does not fit in a [`GlyphId`] (the test fonts have far
    /// fewer glyphs).
    // Port of: tools/fonts/TestTypeface.cpp#L67-L74 (chrome/m156)
    #[must_use]
    pub fn glyph_for_unichar(&self, char_code: Unichar) -> GlyphId {
        self.char_codes
            .iter()
            .position(|&c| c == char_code)
            .map_or(0, |index| {
                GlyphId::try_from(index).expect("test fonts have fewer than 65536 glyphs")
            })
    }
}

/// `SkTestFont::init`: one path per glyph, read from the verb and point streams. Each glyph's
/// verbs end with `kDone`, and the streams continue into the next glyph.
// Port of: tools/fonts/TestTypeface.cpp#L76-L107 (chrome/m156)
fn init(mut pts: &[f32], mut verbs: &[u8], glyph_count: usize) -> Vec<Path> {
    const K_MOVE: u8 = 0;
    const K_LINE: u8 = 1;
    const K_QUAD: u8 = 2;
    const K_CUBIC: u8 = 4;
    const K_CLOSE: u8 = 5;
    const K_DONE: u8 = 6;

    let mut paths = Vec::with_capacity(glyph_count);
    for _ in 0..glyph_count {
        let mut b = PathBuilder::new();
        loop {
            let (&verb, rest) = verbs.split_first().expect("the verbs end with kDone");
            verbs = rest;
            match verb {
                K_DONE => break,
                K_MOVE => {
                    b.move_to((pts[0], pts[1]));
                    pts = &pts[2..];
                }
                K_LINE => {
                    b.line_to((pts[0], pts[1]));
                    pts = &pts[2..];
                }
                K_QUAD => {
                    b.quad_to((pts[0], pts[1]), (pts[2], pts[3]));
                    pts = &pts[4..];
                }
                K_CUBIC => {
                    b.cubic_to((pts[0], pts[1]), (pts[2], pts[3]), (pts[4], pts[5]));
                    pts = &pts[6..];
                }
                K_CLOSE => {
                    b.close();
                }
                _ => panic!("bad verb"),
            }
        }
        paths.push(b.detach());
    }
    paths
}

/// `TestTypeface::getAdvance`: the advance of a glyph; glyphs out of range use glyph 0.
// Port of: tools/fonts/TestTypeface.cpp#L109-L115 (chrome/m156)
fn advance_of(font: &TestFont, glyph_id: GlyphId) -> Point {
    let glyph = clamp_glyph(font, glyph_id);
    Point::new(fixed_to_float(font.widths[glyph]), 0.0)
}

/// `TestTypeface::getPath`: the outline of a glyph, glyph 0 if out of range.
// Port of: tools/fonts/TestTypeface.cpp#L117-L120 (chrome/m156)
fn path_of(font: &TestFont, glyph_id: GlyphId) -> &Path {
    &font.paths[clamp_glyph(font, glyph_id)]
}

fn clamp_glyph(font: &TestFont, glyph_id: GlyphId) -> usize {
    let index = usize::from(glyph_id);
    if index < font.char_codes.len() {
        index
    } else {
        0
    }
}

/// A test typeface (`TestTypeface`): a [`TestFont`] at one of the test styles.
// Port of: tools/fonts/TestTypeface.h#L51-L101 (chrome/m156)
#[derive(Debug)]
pub struct TestTypeface {
    core: TypefaceCore,
    /// The style, also held by `core` (which keeps its fields private to `typeface`).
    style: FontStyle,
    test_font: Arc<TestFont>,
}

impl TestTypeface {
    /// The factory id of the test typeface (`TestTypeface::FactoryId`, `'test'`).
    // Port of: tools/fonts/TestTypeface.h#L92 (chrome/m156)
    pub const FACTORY_ID: FactoryId = set_four_byte_tag(b't', b'e', b's', b't');

    /// `TestTypeface::TestTypeface(testFont, style)`: `SkTypeface(style, false)`.
    // Port of: tools/fonts/TestTypeface.cpp#L122-L123 (chrome/m156)
    fn typeface_of(test_font: Arc<TestFont>, style: FontStyle) -> Typeface {
        Typeface::new(Arc::new(Self {
            core: TypefaceCore::new(style, false),
            style,
            test_font,
        }))
    }
}

/// `TestTypeface::List::Family::Face`.
// Port of: tools/fonts/TestTypeface.h#L53-L56 (chrome/m156)
#[derive(Clone, Debug)]
pub struct Face {
    /// `typeface`.
    pub typeface: Typeface,
    /// `name`: the style name, `Normal` or `Bold Italic`.
    pub name: &'static str,
    /// `isDefault`: the default face of the manager.
    pub is_default: bool,
}

/// `TestTypeface::List::Family`.
// Port of: tools/fonts/TestTypeface.h#L57-L60 (chrome/m156)
#[derive(Clone, Debug)]
pub struct Family {
    /// `name`: the family name.
    pub name: &'static str,
    /// `faces`, in the order of `gSubFonts`.
    pub faces: Vec<Face>,
}

/// `TestTypeface::List`: every family and face of the test data.
// Port of: tools/fonts/TestTypeface.h#L52-L62 (chrome/m156)
#[derive(Clone, Debug)]
pub struct TypefaceList {
    /// `families`, in order of first appearance in `gSubFonts`.
    pub families: Vec<Family>,
}

/// `TestTypeface::Typefaces()`: the list of test typefaces, built once.
// Port of: tools/fonts/TestTypeface.cpp#L25-L56 (chrome/m156)
#[must_use]
pub fn typefaces() -> &'static TypefaceList {
    static LIST: OnceLock<TypefaceList> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut fonts: Vec<Arc<TestFont>> = Vec::with_capacity(TEST_FONTS.len());
        for data in &TEST_FONTS {
            fonts.push(Arc::new(TestFont::new(data)));
        }
        let mut list = TypefaceList {
            families: Vec::new(),
        };
        for (index, sub) in SUB_FONTS.iter().enumerate() {
            let family_index = list
                .families
                .iter()
                .position(|f| f.name == sub.family)
                .unwrap_or_else(|| {
                    list.families.push(Family {
                        name: sub.family,
                        faces: Vec::new(),
                    });
                    list.families.len() - 1
                });
            let style = FontStyle::new(Weight::from(sub.weight), Width::from(sub.width), sub.slant);
            let typeface = TestTypeface::typeface_of(Arc::clone(&fonts[sub.font]), style);
            list.families[family_index].faces.push(Face {
                typeface,
                name: sub.style,
                is_default: index == DEFAULT_FONT_INDEX,
            });
        }
        list
    })
}

impl TypefaceBase for TestTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `TestTypeface::onGetFontDescriptor`: the family and style, the factory id, and the data
    /// is always serialized.
    // Port of: tools/fonts/TestTypeface.cpp#L219-L224 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_family_name(self.test_font.name);
        desc.set_style(self.style);
        desc.set_factory_id(Self::FACTORY_ID);
        (desc, true)
    }

    /// `TestTypeface::onOpenStream`: the header, family name and style, which
    /// `TestTypeface::MakeFromStream` reads back. The index is 0.
    // Port of: tools/fonts/TestTypeface.cpp#L155-L172 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        let mut wstream = DynamicMemoryWStream::new();
        wstream.write(HEADER);

        let name = self.family_name_str();
        let style = self.style;
        wstream.write_packed_uint(name.len());
        wstream.write(name.as_bytes());
        // Weights and widths are small integers, exact in a float, as C++'s int-to-SkScalar.
        #[allow(clippy::cast_precision_loss)]
        {
            wstream.write_scalar(*style.weight() as f32);
            wstream.write_scalar(*style.width() as f32);
        }
        wstream.write_packed_uint(style.slant() as usize);

        Some((wstream.detach_as_stream(), 0))
    }

    /// `TestTypeface::onGetFamilyName`.
    // Port of: tools/fonts/TestTypeface.cpp#L240-L242 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        self.family_name_str().to_owned()
    }

    /// `TestTypeface::onGetVariationDesignPosition`: no axes.
    // Port of: tools/fonts/TestTypeface.h#L108-L111 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    /// `TestTypeface::onGetVariationDesignParameters`: no axes.
    // Port of: tools/fonts/TestTypeface.h#L120-L122 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        Some(Vec::new())
    }

    /// `TestTypeface::onGetUPEM`.
    // Port of: tools/fonts/TestTypeface.h#L107 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        2048
    }

    /// `TestTypeface::onGetPostScriptName`: none.
    // Port of: tools/fonts/TestTypeface.cpp#L241 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    /// `TestTypeface::onCreateFamilyNameIterator`: the family name, in the language `und`.
    // Port of: tools/fonts/TestTypeface.cpp#L243-L247 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::new(vec![LocalizedString {
            string: self.family_name_str().to_owned(),
            language: "und".to_owned(), // undetermined
        }]))
    }

    /// `TestTypeface::onGetTableTags`: no tables.
    // Port of: tools/fonts/TestTypeface.h#L124 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    /// `TestTypeface::onGetTableData`: no data.
    // Port of: tools/fonts/TestTypeface.h#L126-L131 (chrome/m156)
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
    }

    /// `TestTypeface::onFilterRec`: fake bold strokes, and hinting is off.
    // Port of: tools/fonts/TestTypeface.cpp#L145-L148 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec) {
        rec.use_stroke_for_fake_bold();
        rec.set_hinting(FontHinting::None);
    }

    /// `TestTypeface::onGlyphMaskNeedsCurrentColor`: false.
    // Port of: tools/fonts/TestTypeface.h#L97 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }

    /// `TestTypeface::onMakeClone`: the same object.
    // Port of: tools/fonts/TestTypeface.h#L78-L80 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    /// `TestTypeface::onCreateScalerContext`: a scaler context that draws the path glyphs.
    // Port of: tools/fonts/TestTypeface.cpp#L290-L296 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        let test_font = Arc::clone(&self.test_font);
        ScalerContext::new(this, effects, desc, &GLYPH_PATH_RASTERIZER, move |base| {
            // C++: `fMatrix(fRec.getSingleMatrix())`, read from the record already set up.
            let matrix = base.rec().get_single_matrix();
            Box::new(TestScalerContext { test_font, matrix })
        })
    }

    /// `TestTypeface::onCharsToGlyphs`.
    // Port of: tools/fonts/TestTypeface.cpp#L262-L268 (chrome/m156)
    fn on_chars_to_glyphs(&self, unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        for (glyph, &uni) in glyphs.iter_mut().zip(unichars) {
            *glyph = self.test_font.glyph_for_unichar(uni);
        }
    }

    /// `TestTypeface::onCountGlyphs`: the number of characters in the font.
    // Port of: tools/fonts/TestTypeface.h#L72 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        i32::try_from(self.test_font.char_codes.len()).expect("test fonts have few glyphs")
    }

    /// `TestTypeface::getGlyphToUnicodeMap`: the unichar of each glyph, up to the shorter of the
    /// glyph count and the destination.
    // Port of: tools/fonts/TestTypeface.cpp#L226-L232 (chrome/m156)
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        let count = self.test_font.char_codes.len().min(dst.len());
        dst[..count].copy_from_slice(&self.test_font.char_codes[..count]);
    }

    /// `TestTypeface::onGetAdvancedMetrics` (pdf only): only the PostScript name is set, so the
    /// type is `Other` and a PDF draws the typeface as Type3.
    // Port of: tools/fonts/TestTypeface.cpp#L157-L161 (chrome/m156)
    fn on_get_advanced_metrics(&self) -> Option<AdvancedTypefaceMetrics> {
        Some(AdvancedTypefaceMetrics {
            post_script_name: self.test_font.name.to_owned(),
            ..AdvancedTypefaceMetrics::default()
        })
    }
}

impl TestTypeface {
    /// `TestTypeface::MakeFromStream`: reads back a typeface written by `onOpenStream`. The
    /// family and style must name one of [`typefaces`]; otherwise there is no typeface.
    // Port of: tools/fonts/TestTypeface.cpp#L174-L213 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // C++ converts the SkScalar weight/width to int
    #[must_use]
    pub fn make_from_stream(
        mut stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        let mut header = [0u8; HEADER.len()];
        if stream.read(&mut header) != HEADER.len() || header.as_slice() != HEADER {
            return None;
        }

        let family_name_size = stream.read_packed_uint()?;
        let mut family_name = vec![0u8; family_name_size];
        // C++ tests `!read(...)`, which is true for a read of zero bytes too.
        if stream.read(&mut family_name) == 0 {
            return None;
        }

        let weight = stream.read_scalar()?;
        let width = stream.read_scalar()?;
        let slant = match stream.read_packed_uint()? {
            0 => Slant::Upright,
            1 => Slant::Italic,
            2 => Slant::Oblique,
            // C++ casts any value to the enum; no typeface can match it.
            _ => return None,
        };
        let style = FontStyle::new(
            Weight::from(weight as i32),
            Width::from(width as i32),
            slant,
        );

        for family in &typefaces().families {
            if family.name.as_bytes() == family_name.as_slice() {
                for face in &family.faces {
                    if face.typeface.font_style() == style {
                        return Some(face.typeface.clone());
                    }
                }
            }
        }
        None
    }
}

impl TestTypeface {
    /// The family name as `&str`.
    fn family_name_str(&self) -> &'static str {
        self.test_font.name
    }
}

/// `SkTestScalerContext`: the glyph path scaler of [`TestTypeface`].
// Port of: tools/fonts/TestTypeface.cpp#L177-L218 (chrome/m156)
#[doc(alias = "SkTestScalerContext")]
#[derive(Debug)]
struct TestScalerContext {
    test_font: Arc<TestFont>,
    /// `fMatrix`: the record's single matrix (the 2x2 with the text size applied).
    matrix: Matrix,
}

impl ScalerContextImpl for TestScalerContext {
    /// `generateMetrics`: the transformed advance; the bounds come from the path.
    // Port of: tools/fonts/TestTypeface.cpp#L181-L192 (chrome/m156)
    fn generate_metrics(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> GlyphMetrics {
        let mut mx = GlyphMetrics::new(glyph.mask_format());
        let advance = advance_of(&self.test_font, glyph.glyph_id());
        mx.advance = self.matrix.map_point(advance);
        // Always generates from paths, so SkScalerContext::makeGlyph will figure the bounds.
        mx.compute_from_path = true;
        mx
    }

    /// `generateImage`: `generateImageFromPath` of the glyph's path into `image`.
    // Port of: tools/fonts/TestTypeface.cpp#L194-L196 (chrome/m156)
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], base: &ScalerContextBase) {
        let path = glyph
            .path()
            .expect("SkScalerContext::generateImageFromPath needs the glyph's path");
        // The mask writes into an owned buffer, so the bytes are copied in and out.
        let mut mask = MaskBuilder::new(
            image.to_vec(),
            glyph.i_rect(),
            u32::try_from(glyph.row_bytes()).expect("a glyph's row bytes fit in a mask's u32"),
            glyph.mask_format(),
        );
        base.generate_image_from_path(&mut mask, path, glyph.path_is_hairline());
        image.copy_from_slice(&mask.image);
    }

    /// `generatePath`: the outline transformed by the matrix; it is not modified.
    // Port of: tools/fonts/TestTypeface.cpp#L198-L204 (chrome/m156)
    fn generate_path(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> Option<GeneratedPath> {
        Some(GeneratedPath {
            path: path_of(&self.test_font, glyph.glyph_id()).make_transform(&self.matrix),
            modified: false,
        })
    }

    /// `generateFontMetrics`: the font's metrics, scaled by the matrix's y scale.
    // Port of: tools/fonts/TestTypeface.cpp#L206-L211 (chrome/m156)
    fn generate_font_metrics(&mut self, _base: &ScalerContextBase) -> FontMetrics {
        let mut metrics = *self.test_font.metrics;
        scale_font_metrics(&mut metrics, self.matrix.scale_y());
        metrics
    }
}
