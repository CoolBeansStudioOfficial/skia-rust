// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkCustomTypeface.cpp, include/utils/SkCustomTypeface.h (chrome/m156)

//! [`CustomTypefaceBuilder`] (`SkCustomTypefaceBuilder`): a typeface whose glyphs are paths or
//! drawables that the caller gives. Its metrics and style are the caller's, and it serializes
//! itself with the `'user'` factory.

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::drawable::{Drawable, DrawableBase};
use skia_rust_core::fixed::fixed_to_float;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::{FourByteTag, GlyphId, set_four_byte_tag};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::MaskBuilder;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, ScalerContext, ScalerContextBase, ScalerContextEffects,
    ScalerContextImpl, ScalerContextRec,
};
use skia_rust_core::stream::{DynamicMemoryWStream, Stream, StreamAsset, WStream};
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_core::utf::Unichar;
use skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER;
use skia_rust_raster::raster_canvas::RasterCanvas;

/// `SkCustomTypefaceBuilder::FactoryId`: `'user'`.
// Port of: include/utils/SkCustomTypeface.h#L40 (chrome/m156)
pub const FACTORY_ID: FactoryId = set_four_byte_tag(b'u', b's', b'e', b'r');

/// `gHeaderString` without its terminator: `kHeaderSize` bytes.
// Port of: src/utils/SkCustomTypeface.cpp#L300-L310 (chrome/m156)
const HEADER: &[u8; 16] = b"SkUserTypeface01";

/// `kMaxGlyphCount`.
// Port of: src/utils/SkCustomTypeface.cpp#L299 (chrome/m156)
const MAX_GLYPH_COUNT: i32 = 65536;

/// `GlyphType::kPath`. Drawables are not deserialized, so only paths are written and read.
// Port of: src/utils/SkCustomTypeface.cpp#L311 (chrome/m156)
const GLYPH_TYPE_PATH: u32 = 0;

/// `scale_fontmetrics`: every field scaled by the x or the y scale.
// Port of: src/utils/SkCustomTypeface.cpp#L37-L63 (chrome/m156)
fn scale_fontmetrics(src: &FontMetrics, sx: scalar, sy: scalar) -> FontMetrics {
    let mut dst = *src;
    dst.avg_char_width *= sx;
    dst.max_char_width *= sx;
    dst.x_min *= sx;
    dst.x_max *= sx;
    dst.top *= sy;
    dst.ascent *= sy;
    dst.descent *= sy;
    dst.bottom *= sy;
    dst.leading *= sy;
    dst.x_height *= sy;
    dst.cap_height *= sy;
    dst.underline_thickness *= sy;
    dst.underline_position *= sy;
    dst.strikeout_thickness *= sy;
    dst.strikeout_position *= sy;
    dst
}

/// `SkCustomTypefaceBuilder::GlyphRec`: a glyph that is a path, or a drawable with its bounds.
// Port of: include/utils/SkCustomTypeface.h#L44-L58 (chrome/m156)
#[derive(Clone, Debug, Default)]
struct GlyphRec {
    advance: scalar,
    path: Path,
    drawable: Option<Drawable>,
    bounds: Rect,
}

impl GlyphRec {
    /// `isDrawable()`.
    // Port of: include/utils/SkCustomTypeface.h#L50 (chrome/m156)
    fn is_drawable(&self) -> bool {
        self.drawable.is_some()
    }
}

/// `SkCustomTypefaceBuilder`: collects the glyphs, metrics and style, then makes the typeface
/// with [`detach`](Self::detach).
// Port of: include/utils/SkCustomTypeface.h#L20-L42 (chrome/m156)
#[doc(alias = "SkCustomTypefaceBuilder")]
#[derive(Debug, Default)]
pub struct CustomTypefaceBuilder {
    glyph_recs: Vec<GlyphRec>,
    metrics: FontMetrics,
    style: FontStyle,
}

impl CustomTypefaceBuilder {
    /// `SkCustomTypefaceBuilder()`: no glyphs, zeroed metrics (`sk_bzero`), the default style.
    // Port of: src/utils/SkCustomTypeface.cpp#L124-L126 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `setMetrics(fm, scale)`: the metrics, with each field scaled by `scale`.
    // Port of: src/utils/SkCustomTypeface.cpp#L128-L130 (chrome/m156)
    #[doc(alias = "setMetrics")]
    pub fn set_metrics(&mut self, fm: &FontMetrics, scale: scalar) {
        self.metrics = scale_fontmetrics(fm, scale, scale);
    }

    /// `setFontStyle`.
    // Port of: src/utils/SkCustomTypeface.cpp#L132-L134 (chrome/m156)
    #[doc(alias = "setFontStyle")]
    pub fn set_font_style(&mut self, style: FontStyle) {
        self.style = style;
    }

    /// `setGlyph(index, advance, path)`: a path glyph. Glyphs before `index` are left empty.
    // Port of: src/utils/SkCustomTypeface.cpp#L144-L149 (chrome/m156)
    #[doc(alias = "setGlyph")]
    pub fn set_glyph(&mut self, index: GlyphId, advance: scalar, path: &Path) {
        let rec = self.ensure_storage(index);
        rec.advance = advance;
        rec.path = path.clone();
        rec.drawable = None;
    }

    /// `setGlyph(index, advance, drawable, bounds)`: a drawable glyph, drawn within `bounds`.
    // Port of: src/utils/SkCustomTypeface.cpp#L151-L158 (chrome/m156)
    #[doc(alias = "setGlyph")]
    pub fn set_glyph_drawable(
        &mut self,
        index: GlyphId,
        advance: scalar,
        drawable: Drawable,
        bounds: Rect,
    ) {
        let rec = self.ensure_storage(index);
        rec.advance = advance;
        rec.drawable = Some(drawable);
        rec.bounds = bounds;
        rec.path = Path::default();
    }

    /// `ensureStorage(index)`: grows the glyph list to hold `index`.
    // Port of: src/utils/SkCustomTypeface.cpp#L136-L142 (chrome/m156)
    fn ensure_storage(&mut self, index: GlyphId) -> &mut GlyphRec {
        let index = usize::from(index);
        if index >= self.glyph_recs.len() {
            self.glyph_recs.resize_with(index + 1, GlyphRec::default);
        }
        &mut self.glyph_recs[index]
    }

    /// `detach()`: the typeface, with the top, bottom, and x extremes of the glyph bounds as its
    /// metrics. `None` if there are no glyphs. The builder keeps its metrics and style, but its
    /// glyphs move into the typeface.
    // Port of: src/utils/SkCustomTypeface.cpp#L160-L176 (chrome/m156)
    #[must_use]
    pub fn detach(&mut self) -> Option<Typeface> {
        if self.glyph_recs.is_empty() {
            return None;
        }
        // initially inverted, so that any "union" will overwrite the first time
        let mut bounds = Rect {
            left: scalar::MAX,
            top: scalar::MAX,
            right: -scalar::MAX,
            bottom: -scalar::MAX,
        };
        for rec in &self.glyph_recs {
            let glyph_bounds = if rec.is_drawable() {
                rec.bounds
            } else {
                *rec.path.bounds()
            };
            bounds.join(glyph_bounds);
        }
        self.metrics.top = bounds.top;
        self.metrics.bottom = bounds.bottom;
        self.metrics.x_min = bounds.left;
        self.metrics.x_max = bounds.right;
        let glyph_recs = std::mem::take(&mut self.glyph_recs);
        let base = UserTypeface {
            core: TypefaceCore::new(self.style, false),
            style: self.style,
            data: Arc::new(UserTypefaceData {
                glyph_recs,
                metrics: self.metrics,
            }),
        };
        Some(Typeface::new(Arc::new(base)))
    }
}

/// What the typeface and its scaler contexts share: the glyphs and the metrics.
#[derive(Debug)]
struct UserTypefaceData {
    glyph_recs: Vec<GlyphRec>,
    metrics: FontMetrics,
}

/// `SkUserTypeface`.
// Port of: src/utils/SkCustomTypeface.cpp#L65-L106 (chrome/m156)
#[derive(Debug)]
struct UserTypeface {
    core: TypefaceCore,
    /// `fStyle`: the style the builder was given.
    style: FontStyle,
    data: Arc<UserTypefaceData>,
}

impl UserTypeface {
    /// `glyphCount()`.
    // Port of: src/utils/SkCustomTypeface.cpp#L105 (chrome/m156)
    fn glyph_count(&self) -> i32 {
        i32::try_from(self.data.glyph_recs.len()).unwrap_or(i32::MAX)
    }
}

impl TypefaceBase for UserTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `SkUserTypeface::onGetFontDescriptor`: the `'user'` factory, and the data is local.
    // Port of: src/utils/SkCustomTypeface.cpp#L273-L276 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_factory_id(FACTORY_ID);
        (desc, true)
    }

    /// `SkUserTypeface::onComputeBounds`: the metrics' extremes.
    // Port of: src/utils/SkCustomTypeface.cpp#L97-L101 (chrome/m156)
    fn on_compute_bounds(&self) -> Option<Rect> {
        let metrics = &self.data.metrics;
        Some(Rect::from_ltrb(
            metrics.x_min,
            metrics.top,
            metrics.x_max,
            metrics.bottom,
        ))
    }

    /// `SkUserTypeface::onOpenStream`: the header, the metrics, the style, the glyph count, and
    /// for each glyph its type, advance, bounds and path (a drawable's path is its bounds). The
    /// layout is these fields in this order, not the raw bytes of C++'s structs.
    // Port of: src/utils/SkCustomTypeface.cpp#L372-L406 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        let mut wstream = DynamicMemoryWStream::new();
        wstream.write(HEADER);
        write_metrics(&mut wstream, &self.data.metrics);
        write_style(&mut wstream, self.style);
        wstream.write32(u32::try_from(self.glyph_count()).unwrap_or(0));
        for rec in &self.data.glyph_recs {
            wstream.write32(GLYPH_TYPE_PATH);
            wstream.write_scalar(rec.advance);
            write_rect(&mut wstream, &rec.bounds);
            // We simplify drawables on serialization to make deserializing easier/safer.
            let data = if rec.is_drawable() {
                Path::rect(rec.bounds, None).serialize()
            } else {
                rec.path.serialize()
            };
            // `size_t`: eight bytes, as on the 64-bit hosts that write these streams.
            wstream.write(&u64::try_from(data.len()).unwrap_or(0).to_ne_bytes());
            wstream.write(&data);
        }
        // The index of a custom typeface's stream is 0.
        Some((wstream.detach_as_stream(), 0))
    }

    /// `SkUserTypeface::onGetFamilyName`: the empty name.
    // Port of: src/utils/SkCustomTypeface.cpp#L240-L242 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        String::new()
    }

    /// `onGetVariationDesignPosition` and `onGetVariationDesignParameters`: no axes.
    // Port of: src/utils/SkCustomTypeface.cpp#L107-L114 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        Some(Vec::new())
    }

    /// `SkUserTypeface::onGetUPEM`: 2048, as in C++ (`?? ` there).
    // Port of: src/utils/SkCustomTypeface.cpp#L104 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        2048
    }

    /// `SkUserTypeface::onGetPostScriptName`: none.
    // Port of: src/utils/SkCustomTypeface.cpp#L244-L246 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    /// `SkUserTypeface::onCreateFamilyNameIterator`: no names (C++ returns null).
    // Port of: src/utils/SkCustomTypeface.cpp#L248-L250 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::new(Vec::new()))
    }

    /// `onGetTableTags` and `onGetTableData`: no tables.
    // Port of: src/utils/SkCustomTypeface.cpp#L118-L120 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
    }

    /// `SkUserTypeface::onFilterRec`: fake bold strokes, and hinting is off.
    // Port of: src/utils/SkCustomTypeface.cpp#L226-L229 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec) {
        rec.use_stroke_for_fake_bold();
        rec.set_hinting(skia_rust_core::font_types::FontHinting::None);
    }

    /// `onGlyphMaskNeedsCurrentColor`: false.
    // Port of: src/utils/SkCustomTypeface.cpp#L115 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }

    /// `onMakeClone`: the same object.
    // Port of: src/utils/SkCustomTypeface.cpp#L109-L111 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    /// `SkUserTypeface::onCreateScalerContext`: a scaler context with the typeface's glyphs.
    // Port of: src/utils/SkCustomTypeface.cpp#L484-L487 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        let data = Arc::clone(&self.data);
        ScalerContext::new(this, effects, desc, &GLYPH_PATH_RASTERIZER, move |base| {
            // C++: `fMatrix(fRec.getSingleMatrix())`, read from the record already set up.
            let matrix = base.rec().get_single_matrix();
            Box::new(UserScalerContext { data, matrix })
        })
    }

    /// `SkUserTypeface::onCharsToGlyphs`: a character is the glyph with its own index, if the
    /// font has that many glyphs, and glyph 0 otherwise.
    // Port of: src/utils/SkCustomTypeface.cpp#L253-L261 (chrome/m156)
    fn on_chars_to_glyphs(&self, unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        let glyph_count = self.glyph_count();
        for (glyph, &uni) in glyphs.iter_mut().zip(unichars) {
            *glyph = if i64::from(uni) < i64::from(glyph_count) {
                GlyphId::try_from(uni).unwrap_or(0)
            } else {
                0
            };
        }
    }

    /// `onCountGlyphs`.
    // Port of: src/utils/SkCustomTypeface.cpp#L105 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        self.glyph_count()
    }

    /// `SkUserTypeface::getGlyphToUnicodeMap`: glyph `gid` is unichar `gid`.
    // Port of: src/utils/SkCustomTypeface.cpp#L213-L220 (chrome/m156)
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        let count = usize::try_from(self.glyph_count())
            .unwrap_or(0)
            .min(dst.len());
        for (gid, unichar) in dst.iter_mut().enumerate().take(count) {
            *unichar = Unichar::try_from(gid).unwrap_or(0);
        }
    }
}

/// `SkUserScalerContext`.
// Port of: src/utils/SkCustomTypeface.cpp#L222-L340 (chrome/m156)
#[derive(Debug)]
struct UserScalerContext {
    data: Arc<UserTypefaceData>,
    /// `fMatrix`: the single matrix of the record.
    matrix: Matrix,
}

impl UserScalerContext {
    /// The glyph's record, if the typeface has it.
    fn rec(&self, glyph: &Glyph) -> Option<&GlyphRec> {
        self.data.glyph_recs.get(usize::from(glyph.glyph_id()))
    }
}

impl ScalerContextImpl for UserScalerContext {
    /// `generateMetrics`: the transformed advance; a drawable's bounds are its transformed
    /// bounds at the subpixel offset; a path's bounds come from the path.
    // Port of: src/utils/SkCustomTypeface.cpp#L232-L257 (chrome/m156)
    fn generate_metrics(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> GlyphMetrics {
        let mut mx = GlyphMetrics::new(glyph.mask_format());
        let Some(rec) = self.rec(glyph).cloned() else {
            mx.never_request_path = true;
            return mx;
        };
        mx.advance = self.matrix.map_point(Point::new(rec.advance, 0.0));
        if rec.is_drawable() {
            mx.mask_format = skia_rust_core::mask::MaskFormat::Argb32;
            let (mut bounds, _) = self.matrix.map_rect(rec.bounds);
            bounds.offset((
                fixed_to_float(glyph.sub_x_fixed()),
                fixed_to_float(glyph.sub_y_fixed()),
            ));
            // `roundOut(&mx.bounds)`: the rectangle rounded out to whole pixels.
            let rounded: IRect = bounds.round_out();
            mx.bounds = Rect::from_irect(rounded);
            // These do not have an outline path.
            mx.never_request_path = true;
        } else {
            mx.compute_from_path = true;
        }
        mx
    }

    /// `generateImage`: a path glyph is `generateImageFromPath`. A drawable glyph is drawn into
    /// a transparent N32 raster canvas over the image, at the glyph's offset.
    // Port of: src/utils/SkCustomTypeface.cpp#L258-L276 (chrome/m156)
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], base: &ScalerContextBase) {
        let Some(rec) = self.rec(glyph).cloned() else {
            return;
        };
        let Some(drawable) = rec.drawable.as_ref().filter(|_| rec.is_drawable()) else {
            let Some(path) = glyph.path() else {
                return;
            };
            // The mask writes into an owned buffer, so the bytes are copied in and out.
            let mut mask = MaskBuilder::new(
                image.to_vec(),
                glyph.i_rect(),
                u32::try_from(glyph.row_bytes()).unwrap_or(u32::MAX),
                glyph.mask_format(),
            );
            base.generate_image_from_path(&mut mask, path, glyph.path_is_hairline());
            image.copy_from_slice(&mask.image);
            return;
        };
        let info =
            ImageInfo::new_n32_premul((i32::from(glyph.width()), i32::from(glyph.height())), None);
        let Some(canvas) = Canvas::from_raster_direct(&info, image, glyph.row_bytes(), None) else {
            return;
        };
        canvas.clear(Color::TRANSPARENT);
        // The glyph's offset is a small integer, exact in a scalar.
        #[allow(clippy::cast_precision_loss)]
        canvas.translate((-(glyph.left() as scalar), -(glyph.top() as scalar)));
        canvas.translate((
            fixed_to_float(glyph.sub_x_fixed()),
            fixed_to_float(glyph.sub_y_fixed()),
        ));
        drawable.draw(&canvas, Some(&self.matrix));
    }

    /// `generatePath`: the record's path, transformed. Drawables have none.
    // Port of: src/utils/SkCustomTypeface.cpp#L278-L282 (chrome/m156)
    fn generate_path(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> Option<GeneratedPath> {
        let rec = self.rec(glyph)?;
        if rec.is_drawable() {
            return None;
        }
        Some(GeneratedPath {
            path: rec.path.make_transform(&self.matrix),
            modified: false,
        })
    }

    /// `generateDrawable`: the record's drawable, drawn with the matrix.
    // Port of: src/utils/SkCustomTypeface.cpp#L284-L304 (chrome/m156)
    fn generate_drawable(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> Option<Drawable> {
        let drawable = self.rec(glyph)?.drawable.clone()?;
        Some(Drawable::new(Arc::new(DrawableMatrixWrapper {
            drawable,
            matrix: self.matrix.clone(),
        })))
    }

    /// `generateFontMetrics`: the metrics scaled by the matrix's mapped unit point.
    // Port of: src/utils/SkCustomTypeface.cpp#L332-L335 (chrome/m156)
    fn generate_font_metrics(&mut self, _base: &ScalerContextBase) -> FontMetrics {
        let scaled = self.matrix.map_point(Point::new(1.0, 1.0));
        scale_fontmetrics(&self.data.metrics, scaled.x, scaled.y)
    }
}

/// `DrawableMatrixWrapper`: a drawable drawn with a matrix.
// Port of: src/utils/SkCustomTypeface.cpp#L286-L306 (chrome/m156)
#[derive(Debug)]
struct DrawableMatrixWrapper {
    drawable: Drawable,
    matrix: Matrix,
}

impl DrawableBase for DrawableMatrixWrapper {
    /// `onGetBounds`: the drawable's bounds, mapped.
    fn on_get_bounds(&self) -> Rect {
        self.matrix.map_rect(self.drawable.bounds()).0
    }

    /// `onDraw`: the drawable, drawn with the matrix.
    fn on_draw(&self, canvas: &Canvas) {
        self.drawable.draw(canvas, Some(&self.matrix));
    }
}

/// `SkCustomTypefaceBuilder::MakeFromStream`: reads a typeface that `on_open_stream` wrote.
/// Returns `None` for a malformed stream. The stream is consumed, so C++'s restore of its
/// position on failure has nothing to restore.
// Port of: src/utils/SkCustomTypeface.cpp#L430-L466 and #L515-L519 (chrome/m156)
#[must_use]
#[doc(alias = "MakeFromStream")]
pub fn make_from_stream(
    mut stream: Box<dyn StreamAsset>,
    _args: &FontArguments<'_, '_>,
) -> Option<Typeface> {
    deserialize(stream.as_mut())
}

/// `SkCustomTypefaceBuilder::Deserialize`.
// Port of: src/utils/SkCustomTypeface.cpp#L430-L505 (chrome/m156)
fn deserialize(stream: &mut dyn StreamAsset) -> Option<Typeface> {
    let mut header = [0u8; HEADER.len()];
    if !read_exact(stream, &mut header) || &header != HEADER {
        return None;
    }
    let metrics = read_metrics(stream)?;
    let style = read_style(stream)?;
    let glyph_count = stream.read_s32()?;
    if !(0..=MAX_GLYPH_COUNT).contains(&glyph_count) {
        return None;
    }
    let mut builder = CustomTypefaceBuilder::new();
    builder.set_metrics(&metrics, 1.0);
    builder.set_font_style(style);
    for index in 0..glyph_count {
        // We don't support deserializing GlyphType::kDrawable because drawables can be *anything*.
        if stream.read_u32()? != GLYPH_TYPE_PATH {
            return None;
        }
        let advance = stream.read_scalar()?;
        let bounds = read_rect(stream)?;
        if !bounds.left.is_finite()
            || !bounds.top.is_finite()
            || !bounds.right.is_finite()
            || !bounds.bottom.is_finite()
        {
            return None;
        }
        // SkPath cannot be read from a stream, so we have to page them into ram.
        let mut size_bytes = [0u8; 8];
        if !read_exact(stream, &mut size_bytes) {
            return None;
        }
        let size = usize::try_from(u64::from_ne_bytes(size_bytes)).ok()?;
        // The amount of bytes in the stream must be at least as big as `size`.
        if skia_rust_core::stream_priv::remaining_length_is_below(stream, size) {
            return None;
        }
        let mut data = vec![0u8; size];
        if !read_exact(stream, &mut data) {
            return None;
        }
        // The path must use exactly the bytes of its record.
        match Path::read_from_memory(&data) {
            (Some(path), read) if read == data.len() => {
                let index = GlyphId::try_from(index).ok()?;
                builder.set_glyph(index, advance, &path);
            }
            _ => return None,
        }
    }
    builder.detach()
}

/// `stream->read(buf, n) == n`.
fn read_exact(stream: &mut dyn Stream, buf: &mut [u8]) -> bool {
    stream.read(buf) == buf.len()
}

/// The fields of `SkFontMetrics` in struct order, as `f32` (the flags as `u32`).
fn write_metrics(wstream: &mut DynamicMemoryWStream, metrics: &FontMetrics) {
    wstream.write32(metrics.flags.bits());
    for value in metrics_scalars(metrics) {
        wstream.write_scalar(value);
    }
}

/// The scalars of `SkFontMetrics` after its flags, in struct order.
fn metrics_scalars(m: &FontMetrics) -> [scalar; 15] {
    [
        m.top,
        m.ascent,
        m.descent,
        m.bottom,
        m.leading,
        m.avg_char_width,
        m.max_char_width,
        m.x_min,
        m.x_max,
        m.x_height,
        m.cap_height,
        m.underline_thickness,
        m.underline_position,
        m.strikeout_thickness,
        m.strikeout_position,
    ]
}

/// Reads what `write_metrics` wrote.
fn read_metrics(stream: &mut dyn StreamAsset) -> Option<FontMetrics> {
    let flags = stream.read_u32()?;
    let mut s = [0.0; 15];
    for value in &mut s {
        *value = stream.read_scalar()?;
    }
    Some(FontMetrics {
        flags: skia_rust_core::font_metrics::Flags::from_bits_retain(flags),
        top: s[0],
        ascent: s[1],
        descent: s[2],
        bottom: s[3],
        leading: s[4],
        avg_char_width: s[5],
        max_char_width: s[6],
        x_min: s[7],
        x_max: s[8],
        x_height: s[9],
        cap_height: s[10],
        underline_thickness: s[11],
        underline_position: s[12],
        strikeout_thickness: s[13],
        strikeout_position: s[14],
    })
}

/// The weight, width and slant of a style, as `u32`s.
fn write_style(wstream: &mut DynamicMemoryWStream, style: FontStyle) {
    wstream.write32(u32::try_from(*style.weight()).unwrap_or(0));
    wstream.write32(u32::try_from(*style.width()).unwrap_or(0));
    wstream.write32(style.slant() as u32);
}

/// Reads what `write_style` wrote.
fn read_style(stream: &mut dyn StreamAsset) -> Option<FontStyle> {
    let weight = i32::try_from(stream.read_u32()?).ok()?;
    let width = i32::try_from(stream.read_u32()?).ok()?;
    let slant = stream.read_u32()?;
    let slant = match slant {
        0 => Slant::Upright,
        1 => Slant::Italic,
        2 => Slant::Oblique,
        _ => return None,
    };
    Some(FontStyle::new(
        Weight::from(weight),
        Width::from(width),
        slant,
    ))
}

/// The four scalars of a rectangle.
fn write_rect(wstream: &mut DynamicMemoryWStream, rect: &Rect) {
    wstream.write_scalar(rect.left);
    wstream.write_scalar(rect.top);
    wstream.write_scalar(rect.right);
    wstream.write_scalar(rect.bottom);
}

/// Reads what `write_rect` wrote.
fn read_rect(stream: &mut dyn StreamAsset) -> Option<Rect> {
    Some(Rect::from_ltrb(
        stream.read_scalar()?,
        stream.read_scalar()?,
        stream.read_scalar()?,
        stream.read_scalar()?,
    ))
}
