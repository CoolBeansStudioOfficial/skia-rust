// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/TestSVGTypeface.{h,cpp} (chrome/m156), the typeface, its scaler
// context and the `Emoji` and `Planets` fonts. The generators of OpenType test fonts that follow
// them in the C++ file (`exportTtx*`, `buildGlyfOutlines`) are not ported.

//! [`TestSvgTypeface`]: a typeface whose glyphs are SVG documents (`TestSVGTypeface`). The
//! portable font manager's `Emoji` and `Planet` families.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::drawable::{Drawable, DrawableBase};
use skia_rust_core::fixed::fixed_to_scalar;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_metrics::{FontMetrics, Flags as FontMetricsFlags};
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_priv::scale_font_metrics;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, FourByteTag, GlyphId, set_four_byte_tag};
use skia_rust_core::glyph::Glyph as StrikeGlyph;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Rect, RoundOut};
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, ScalerContext, ScalerContextBase, ScalerContextEffects,
    ScalerContextImpl, ScalerContextRec,
};
use skia_rust_core::size::Size;
use skia_rust_core::stream::{DynamicMemoryWStream, StreamAsset, WStream};
use skia_rust_core::typeface::{
    LocalizedString, LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_core::utf::Unichar;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER;
use skia_rust_raster::surfaces;
use skia_rust_svg::Dom;

use crate::resources::get_resource_as_stream;

/// One glyph of a [`TestSvgTypeface`] (`SkSVGTestTypefaceGlyphData`).
// Port of: tools/fonts/TestSVGTypeface.h#L45-L50 (chrome/m156)
#[doc(alias = "SkSVGTestTypefaceGlyphData")]
#[derive(Debug, Clone, Copy)]
struct SvgTestTypefaceGlyphData {
    /// `fSvgResourcePath`.
    svg_resource_path: &'static str,
    /// `fOrigin`: y-down.
    origin: (scalar, scalar),
    /// `fAdvance`.
    advance: scalar,
    /// `fUnicode`. TODO in Skia: this limits to 1:1.
    unicode: Unichar,
}

/// The lazily parsed SVG of a glyph.
#[derive(Debug, Default)]
struct ParsedSvg {
    parsed: bool,
    svg: Option<Dom>,
}

/// `TestSVGTypeface::Glyph`.
// Port of: tools/fonts/TestSVGTypeface.h#L131-L148 (chrome/m156)
#[derive(Debug)]
struct Glyph {
    origin: Point,
    advance: scalar,
    resource_path: &'static str,
    // The mutex guards lazy parsing of the SVG.
    svg: Mutex<ParsedSvg>,
}

impl Glyph {
    // Port of: tools/fonts/TestSVGTypeface.cpp#L59-L60 (chrome/m156)
    fn new(datum: &SvgTestTypefaceGlyphData) -> Self {
        Self {
            origin: Point::new(datum.origin.0, datum.origin.1),
            advance: datum.advance,
            resource_path: datum.svg_resource_path,
            svg: Mutex::new(ParsedSvg::default()),
        }
    }

    /// Lazily parses the SVG from the resource path, and manages locking.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L67-L100 (chrome/m156)
    fn with_svg(&self, f: impl FnOnce(&Dom)) {
        let mut guard = self.svg.lock().expect("the SVG lock is not poisoned");

        if !guard.parsed {
            guard.parsed = true;

            let Some(mut stream) = get_resource_as_stream(self.resource_path) else {
                return;
            };

            // We expressly *do not want* to set a SkFontMgr when parsing these SVGs.
            // 1) The SVGs we are processing have no <text> tags in them.
            // 2) Trying to use ToolUtils::TestFontMgr() is a problem because the portable
            //    SkFontMgr *calls* this function as it creates the typefaces.
            let Ok(svg) = Dom::make_from_stream(&mut *stream) else {
                return;
            };

            if svg.container_size().is_empty() {
                return;
            }

            guard.svg = Some(svg);
        }

        if let Some(svg) = &guard.svg {
            f(svg);
        }
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L102-L108 (chrome/m156)
    fn size(&self) -> Size {
        let mut size = Size::new(0.0, 0.0);
        self.with_svg(|svg| {
            size = *svg.container_size();
        });
        size
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L110-L114 (chrome/m156)
    fn render(&self, canvas: &Canvas) {
        self.with_svg(|svg| {
            svg.render(canvas);
        });
    }
}

/// Which of the two fonts a typeface is (`DefaultTypeface` and `PlanetTypeface`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Default,
    Planets,
}

impl Kind {
    /// `FactoryId`.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L218-L219 and #L283-L284 (chrome/m156)
    const fn factory_id(self) -> FactoryId {
        match self {
            Kind::Default => set_four_byte_tag(b'd', b's', b'v', b'g'),
            Kind::Planets => set_four_byte_tag(b'p', b's', b'v', b'g'),
        }
    }

    /// `gHeaderString`, with its NUL (`sizeof`).
    // Port of: tools/fonts/TestSVGTypeface.cpp#L220 and #L285 (chrome/m156)
    const fn header(self) -> &'static [u8] {
        match self {
            Kind::Default => b"SkTestSVGTypefaceDefault01\0",
            Kind::Planets => b"SkTestSVGTypefacePlanet01\0",
        }
    }
}

/// What a typeface and its scaler contexts share.
#[derive(Debug)]
struct Data {
    kind: Kind,
    name: String,
    upem: i32,
    font_metrics: FontMetrics,
    glyphs: Vec<Glyph>,
    cmap: HashMap<Unichar, GlyphId>,
}

impl Data {
    fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// `glyphID < fGlyphCount ? glyphID : 0`.
    fn clamp(&self, glyph_id: GlyphId) -> usize {
        let index = usize::from(glyph_id);
        if index < self.glyph_count() { index } else { 0 }
    }
}

/// A typeface whose glyphs are SVG documents.
// Port of: tools/fonts/TestSVGTypeface.h#L52-L160 (chrome/m156)
#[doc(alias = "TestSVGTypeface")]
#[derive(Debug)]
pub struct TestSvgTypeface {
    core: TypefaceCore,
    /// The style, also held by `core` (which keeps its fields private to `typeface`).
    style: FontStyle,
    data: Arc<Data>,
}

impl TestSvgTypeface {
    // Port of: tools/fonts/TestSVGTypeface.cpp#L50-L62 (chrome/m156)
    fn make(
        kind: Kind,
        name: &str,
        style: FontStyle,
        upem: i32,
        font_metrics: FontMetrics,
        data: &[SvgTestTypefaceGlyphData],
    ) -> Typeface {
        let mut glyphs = Vec::with_capacity(data.len());
        let mut cmap = HashMap::new();
        for (i, datum) in data.iter().enumerate() {
            cmap.insert(
                datum.unicode,
                GlyphId::try_from(i).expect("test fonts have fewer than 65536 glyphs"),
            );
            glyphs.push(Glyph::new(datum));
        }
        Typeface::new(Arc::new(Self {
            core: TypefaceCore::new(style, false),
            style,
            data: Arc::new(Data {
                kind,
                name: name.to_owned(),
                upem,
                font_metrics,
                glyphs,
                cmap,
            }),
        }))
    }

    /// `TestSVGTypeface::Default()`: the `Emoji` font.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L316-L347 (chrome/m156)
    #[doc(alias = "Default")]
    #[must_use]
    pub fn default_typeface() -> Typeface {
        // Recommended that the first four be .notdef, .null, CR, space
        const GLYPHS: [SvgTestTypefaceGlyphData; 4] = [
            SvgTestTypefaceGlyphData {
                svg_resource_path: "fonts/svg/notdef.svg",
                origin: (100.0, 800.0),
                advance: 800.0,
                unicode: 0x0,
            }, // .notdef
            SvgTestTypefaceGlyphData {
                svg_resource_path: "fonts/svg/empty.svg",
                origin: (0.0, 0.0),
                advance: 800.0,
                unicode: 0x0020,
            }, // space
            SvgTestTypefaceGlyphData {
                svg_resource_path: "fonts/svg/diamond.svg",
                origin: (100.0, 800.0),
                advance: 800.0,
                unicode: 0x2662,
            }, // ♢
            SvgTestTypefaceGlyphData {
                svg_resource_path: "fonts/svg/smile.svg",
                origin: (0.0, 800.0),
                advance: 800.0,
                unicode: 0x1F600,
            }, // 😀
        ];
        let metrics = FontMetrics {
            flags: FontMetricsFlags::UNDERLINE_THICKNESS_IS_VALID
                | FontMetricsFlags::UNDERLINE_POSITION_IS_VALID
                | FontMetricsFlags::STRIKEOUT_THICKNESS_IS_VALID
                | FontMetricsFlags::STRIKEOUT_POSITION_IS_VALID,
            top: -800.0,
            ascent: -800.0,
            descent: 200.0,
            bottom: 200.0,
            leading: 100.0,
            avg_char_width: 1000.0,
            max_char_width: 1000.0,
            x_min: 0.0,
            x_max: 1000.0,
            x_height: 500.0,
            cap_height: 700.0,
            underline_thickness: 40.0,
            underline_position: 20.0,
            strikeout_thickness: 20.0,
            strikeout_position: -400.0,
        };

        Self::make(
            Kind::Default,
            "Emoji",
            FontStyle::normal(),
            1000,
            metrics,
            &GLYPHS,
        )
    }

    /// `TestSVGTypeface::Planets()`: the `Planets` font.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L387-L440 (chrome/m156)
    #[doc(alias = "Planets")]
    #[must_use]
    pub fn planets() -> Typeface {
        const fn glyph(
            svg_resource_path: &'static str,
            origin: (scalar, scalar),
            advance: scalar,
            unicode: Unichar,
        ) -> SvgTestTypefaceGlyphData {
            SvgTestTypefaceGlyphData {
                svg_resource_path,
                origin,
                advance,
                unicode,
            }
        }
        // Recommended that the first four be .notdef, .null, CR, space
        const GLYPHS: [SvgTestTypefaceGlyphData; 10] = [
            glyph("fonts/svg/planets/pluto.svg", (0.0, 20.0), 60.0, 0x0), // .notdef
            glyph("fonts/svg/empty.svg", (0.0, 0.0), 400.0, 0x0020),      // space
            glyph("fonts/svg/planets/mercury.svg", (0.0, 45.0), 120.0, 0x263F), // ☿
            glyph("fonts/svg/planets/venus.svg", (0.0, 100.0), 240.0, 0x2640), // ♀
            glyph("fonts/svg/planets/earth.svg", (0.0, 100.0), 240.0, 0x2641), // ♁
            glyph("fonts/svg/planets/mars.svg", (0.0, 50.0), 130.0, 0x2642), // ♂
            glyph("fonts/svg/planets/jupiter.svg", (0.0, 1000.0), 2200.0, 0x2643), // ♃
            glyph("fonts/svg/planets/saturn.svg", (-300.0, 1500.0), 2600.0, 0x2644), // ♄
            glyph("fonts/svg/planets/uranus.svg", (0.0, 375.0), 790.0, 0x2645), // ♅
            glyph("fonts/svg/planets/neptune.svg", (0.0, 350.0), 740.0, 0x2646), // ♆
        ];
        let metrics = FontMetrics {
            flags: FontMetricsFlags::UNDERLINE_THICKNESS_IS_VALID
                | FontMetricsFlags::UNDERLINE_POSITION_IS_VALID
                | FontMetricsFlags::STRIKEOUT_THICKNESS_IS_VALID
                | FontMetricsFlags::STRIKEOUT_POSITION_IS_VALID,
            top: -1500.0,
            ascent: -200.0,
            descent: 50.0,
            bottom: 1558.0,
            leading: 10.0,
            avg_char_width: 200.0,
            max_char_width: 200.0,
            x_min: -300.0,
            x_max: 2566.0,
            x_height: 100.0,
            cap_height: 180.0,
            underline_thickness: 8.0,
            underline_position: 2.0,
            strikeout_thickness: 2.0,
            strikeout_position: -80.0,
        };

        Self::make(
            Kind::Planets,
            "Planets",
            FontStyle::normal(),
            200,
            metrics,
            &GLYPHS,
        )
    }

    /// `TestSVGTypeface::getAdvance`: the advance of a glyph; glyphs out of range use glyph 0.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L116-L119 (chrome/m156)
    #[doc(alias = "getAdvance")]
    fn advance_of(data: &Data, glyph_id: GlyphId) -> Point {
        let glyph_id = data.clamp(glyph_id);
        Point::new(data.glyphs[glyph_id].advance, 0.0)
    }

    /// `TestSVGTypeface::getPathOp`: how to combine the layer paths of a glyph of this color
    /// (used by the OpenType test font generators).
    // Port of: tools/fonts/TestSVGTypeface.cpp#L205-L211 and #L270-L273 (chrome/m156)
    #[doc(alias = "getPathOp")]
    #[must_use]
    pub fn get_path_op(typeface_kind_is_default: bool, color: Color) -> PathOp {
        if typeface_kind_is_default {
            if (u32::from(color.r()) + u32::from(color.g()) + u32::from(color.b())) / 3 > 0x20 {
                PathOp::Difference
            } else {
                PathOp::Union
            }
        } else {
            PathOp::Union
        }
    }

    /// `DefaultTypeface::MakeFromStream` and `PlanetTypeface::MakeFromStream`.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L229-L238 and #L294-L303 (chrome/m156)
    fn make_from_stream_of(kind: Kind, mut stream: Box<dyn StreamAsset>) -> Option<Typeface> {
        let header_size = kind.header().len();
        let mut header = vec![0u8; header_size];
        if stream.read(&mut header) != header_size || header != kind.header() {
            return None;
        }
        Some(match kind {
            Kind::Default => Self::default_typeface(),
            Kind::Planets => Self::planets(),
        })
    }

    /// `DefaultTypeface::MakeFromStream`.
    #[must_use]
    pub fn make_default_from_stream(
        stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        Self::make_from_stream_of(Kind::Default, stream)
    }

    /// `PlanetTypeface::MakeFromStream`.
    #[must_use]
    pub fn make_planets_from_stream(
        stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        Self::make_from_stream_of(Kind::Planets, stream)
    }

    /// The factory id of the `Emoji` typeface (`DefaultTypeface::FactoryId`, `'dsvg'`).
    pub const DEFAULT_FACTORY_ID: FactoryId = Kind::Default.factory_id();

    /// The factory id of the `Planets` typeface (`PlanetTypeface::FactoryId`, `'psvg'`).
    pub const PLANETS_FACTORY_ID: FactoryId = Kind::Planets.factory_id();
}

impl TypefaceBase for TestSvgTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `onGetFontDescriptor`: the family name and style, and the data is always serialized.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L145-L149, #L238-L241 and #L303-L306 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_family_name(&self.data.name);
        desc.set_style(self.style);
        desc.set_factory_id(self.data.kind.factory_id());
        (desc, true)
    }

    /// `onOpenStream`: the header string.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L222-L227 and #L287-L292 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        let mut wstream = DynamicMemoryWStream::new();
        wstream.write(self.data.kind.header());
        Some((wstream.detach_as_stream(), 0))
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L171 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        self.data.name.clone()
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L120-L122 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L124-L126 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        Some(Vec::new())
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L116 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        self.data.upem
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L173 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L175-L179 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::new(vec![LocalizedString {
            string: self.data.name.clone(),
            language: "und".to_owned(), // undetermined
        }]))
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L130 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L132-L137 (chrome/m156)
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
    }

    /// `onFilterRec`: hinting is off.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L121-L123 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec) {
        rec.set_hinting(FontHinting::None);
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L128 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }

    /// `onMakeClone`: the same object.
    // Port of: tools/fonts/TestSVGTypeface.h#L103-L105 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L206-L210 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        let data = Arc::clone(&self.data);
        ScalerContext::new(this, effects, desc, &GLYPH_PATH_RASTERIZER, move |base| {
            Box::new(TestSvgScalerContext::new(data, base))
        })
    }

    /// `onCharsToGlyphs`: glyph 0 for characters the font does not map.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L162-L169 (chrome/m156)
    fn on_chars_to_glyphs(&self, unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        debug_assert_eq!(unichars.len(), glyphs.len());
        for (glyph, uni) in glyphs.iter_mut().zip(unichars) {
            *glyph = self.data.cmap.get(uni).copied().unwrap_or(0);
        }
    }

    // Port of: tools/fonts/TestSVGTypeface.h#L114 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        i32::try_from(self.data.glyph_count()).expect("test fonts have few glyphs")
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L125-L135 (chrome/m156)
    fn on_get_glyph_to_unicode_map(&self, glyph_to_unicode: &mut [Unichar]) {
        for (&c, &g) in &self.data.cmap {
            let g = usize::from(g);
            debug_assert!(g < self.data.glyph_count());
            debug_assert!(g < glyph_to_unicode.len());
            glyph_to_unicode[g] = c;
        }
    }
}

/// `SkTestSVGScalerContext`.
// Port of: tools/fonts/TestSVGTypeface.cpp#L181-L305 (chrome/m156)
#[derive(Debug)]
struct TestSvgScalerContext {
    data: Arc<Data>,
    matrix: Matrix,
}

impl TestSvgScalerContext {
    // Port of: tools/fonts/TestSVGTypeface.cpp#L183-L190 (chrome/m156)
    fn new(data: Arc<Data>, base: &ScalerContextBase) -> Self {
        let mut matrix = base.rec().get_single_matrix();
        #[allow(clippy::cast_precision_loss)] // SkScalar upem = fUpem
        let upem = data.upem as scalar;
        matrix.pre_scale((1.0 / upem, 1.0 / upem), None);
        Self { data, matrix }
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L198-L201 (chrome/m156)
    fn compute_advance(&self, glyph_id: GlyphId) -> Point {
        let advance = TestSvgTypeface::advance_of(&self.data, glyph_id);
        self.matrix.map_point(advance)
    }
}

impl ScalerContextImpl for TestSvgScalerContext {
    // Port of: tools/fonts/TestSVGTypeface.cpp#L203-L228 (chrome/m156)
    fn generate_metrics(&mut self, glyph: &StrikeGlyph, _base: &ScalerContextBase) -> GlyphMetrics {
        let glyph_id = self.data.clamp(glyph.glyph_id());

        let mut mx = GlyphMetrics::new(MaskFormat::Argb32);
        mx.never_request_path = true;
        mx.advance = self.compute_advance(glyph.glyph_id());

        let glyph_data = &self.data.glyphs[glyph_id];

        let container_size = glyph_data.size();
        let mut new_bounds = Rect::from_xywh(
            glyph_data.origin.x,
            -glyph_data.origin.y,
            container_size.width,
            container_size.height,
        );
        new_bounds = self.matrix.map_rect(new_bounds).0;
        let dx = fixed_to_scalar(glyph.sub_x_fixed());
        let dy = fixed_to_scalar(glyph.sub_y_fixed());
        new_bounds.offset((dx, dy));
        mx.bounds = RoundOut::<Rect>::round_out(&new_bounds);
        mx
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L230-L255 (chrome/m156)
    fn generate_image(&mut self, glyph: &StrikeGlyph, image: &mut [u8], _base: &ScalerContextBase) {
        let glyph_id = self.data.clamp(glyph.glyph_id());

        // TODO: this should be SkImageInfo::MakeS32 when that passes all the tests.
        let info = ImageInfo::new_n32_premul((i32::from(glyph.width()), i32::from(glyph.height())), None);
        let row_bytes = glyph.row_bytes();
        // bm.eraseColor(0)
        let width_bytes = usize::from(glyph.width()) * 4;
        for row in image.chunks_mut(row_bytes) {
            let n = width_bytes.min(row.len());
            row[..n].fill(0);
        }
        let Some(mut surface) = surfaces::wrap_pixels(&info, image, row_bytes, None) else {
            return;
        };

        let glyph_data = &self.data.glyphs[glyph_id];

        let dx = fixed_to_scalar(glyph.sub_x_fixed());
        let dy = fixed_to_scalar(glyph.sub_y_fixed());

        let canvas = surface.canvas();
        #[allow(clippy::cast_precision_loss)] // SkScalar from the glyph's integer origin
        canvas.translate((-(glyph.left() as scalar), -(glyph.top() as scalar)));
        canvas.translate((dx, dy));
        canvas.concat(&self.matrix);
        canvas.translate((glyph_data.origin.x, -glyph_data.origin.y));

        glyph_data.render(canvas);
    }

    /// Should never get here since `generateMetrics` always sets the path to not exist.
    // Port of: tools/fonts/TestSVGTypeface.cpp#L257-L261 (chrome/m156)
    fn generate_path(
        &mut self,
        _glyph: &StrikeGlyph,
        _base: &ScalerContextBase,
    ) -> Option<GeneratedPath> {
        panic!("Path requested, but it should have been indicated that there isn't one.");
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L263-L293 (chrome/m156)
    fn generate_drawable(
        &mut self,
        glyph: &StrikeGlyph,
        _base: &ScalerContextBase,
    ) -> Option<Drawable> {
        Some(Drawable::new(Arc::new(SvgGlyphDrawable {
            data: Arc::clone(&self.data),
            matrix: self.matrix.clone(),
            glyph_id: glyph.glyph_id(),
            sub_x_fixed: glyph.sub_x_fixed(),
            sub_y_fixed: glyph.sub_y_fixed(),
            rect: glyph.rect(),
        })))
    }

    // Port of: tools/fonts/TestSVGTypeface.cpp#L295-L299 (chrome/m156)
    fn generate_font_metrics(&mut self, _base: &ScalerContextBase) -> FontMetrics {
        let mut metrics = self.data.font_metrics;
        scale_font_metrics(&mut metrics, self.matrix.scale_y());
        metrics
    }
}

/// `SkTestSVGScalerContext::SVGGlyphDrawable`.
// Port of: tools/fonts/TestSVGTypeface.cpp#L263-L290 (chrome/m156)
#[derive(Debug)]
struct SvgGlyphDrawable {
    data: Arc<Data>,
    matrix: Matrix,
    glyph_id: GlyphId,
    sub_x_fixed: skia_rust_core::fixed::Fixed,
    sub_y_fixed: skia_rust_core::fixed::Fixed,
    rect: Rect,
}

impl DrawableBase for SvgGlyphDrawable {
    fn on_get_bounds(&self) -> Rect {
        self.rect
    }

    fn on_approximate_bytes_used(&self) -> usize {
        std::mem::size_of::<SvgGlyphDrawable>()
    }

    fn on_draw(&self, canvas: &Canvas) {
        let glyph_id = self.data.clamp(self.glyph_id);

        let glyph_data = &self.data.glyphs[glyph_id];

        let dx = fixed_to_scalar(self.sub_x_fixed);
        let dy = fixed_to_scalar(self.sub_y_fixed);

        canvas.translate((dx, dy));
        canvas.concat(&self.matrix);
        canvas.translate((glyph_data.origin.x, -glyph_data.origin.y));

        glyph_data.render(canvas);
    }
}

#[cfg(test)]
mod tests {
    use skia_rust_core::font::Font;
    use skia_rust_core::font_types::TextEncoding;
    use skia_rust_core::paint::Paint;

    use super::*;

    #[test]
    fn draws_the_emoji_glyphs() {
        let typeface = TestSvgTypeface::default_typeface();
        let mut glyphs = [0u16; 2];
        typeface.unichars_to_glyphs(&[0x1F600, 0x2662], &mut glyphs);
        assert_eq!(glyphs, [3, 2]);

        let font = Font::from_size(typeface, 40.0);
        let mut surface = surfaces::raster_n32_premul((120, 60)).unwrap();
        surface.canvas().clear(Color::WHITE);
        surface.canvas().draw_simple_text(
            "\u{1F600} \u{2662}".as_bytes(),
            TextEncoding::UTF8,
            Point::new(5.0, 45.0),
            &font,
            &Paint::default(),
        );
        let pixels = surface.peek_pixels().unwrap();
        let pixmap = pixels.pixmap();
        let mut non_white = 0;
        let mut yellowish = 0;
        for y in 0..60 {
            for x in 0..120 {
                let c = pixmap.get_color((x, y));
                if c != Color::WHITE {
                    non_white += 1;
                }
                if c.r() > 200 && c.g() > 150 && c.b() < 100 {
                    yellowish += 1;
                }
            }
        }
        assert!(non_white > 200, "{non_white}");
        assert!(yellowish > 20, "{yellowish}");
    }
}
