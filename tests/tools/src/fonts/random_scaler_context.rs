// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/RandomScalerContext.{h,cpp} (chrome/m156)

//! [`RandomTypeface`]: a debug typeface that wraps a proxy typeface and draws each glyph in a mask
//! format picked from its glyph id. The choice is deterministic, so a text blob mixes BW, A8, LCD
//! and ARGB masks and the same glyph always gets the same format.

use std::collections::HashMap;
use std::sync::Arc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_types::{FontHinting, FourByteTag, GlyphId};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{Rect, RoundOut};
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, ScalerContext, ScalerContextBase, ScalerContextEffects,
    ScalerContextImpl, ScalerContextRec,
};
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_core::utf::Unichar;
use skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER;
use skia_rust_raster::raster_canvas::RasterCanvas;

/// The state of a [`RandomTypeface`] that its scaler contexts read (`getRandomTypeface()`).
#[derive(Debug)]
struct RandomTypefaceData {
    /// `fProxy`: the typeface whose glyphs are drawn.
    proxy: Typeface,
    /// `fPaint`: the paint the glyph paths are drawn with.
    paint: Paint,
    /// `fFakeIt`: draw nothing, only the metrics.
    fake_it: bool,
}

/// `SkRandomTypeface`: wraps a proxy typeface and draws its glyphs in random but deterministic
/// mask formats. Debug only: it does not serialize.
// Port of: tools/fonts/RandomScalerContext.h#L17-L53 (chrome/m156)
#[doc(alias = "SkRandomTypeface")]
#[derive(Debug)]
pub struct RandomTypeface {
    core: TypefaceCore,
    data: Arc<RandomTypefaceData>,
}

impl RandomTypeface {
    /// `SkRandomTypeface::SkRandomTypeface(proxy, paint, fakeIt)`: a typeface with the proxy's
    /// style.
    // Port of: tools/fonts/RandomScalerContext.cpp#L161-L167 (chrome/m156)
    #[must_use]
    pub fn make(proxy: Typeface, paint: Paint, fake_it: bool) -> Typeface {
        let core = TypefaceCore::new(proxy.font_style(), false);
        let data = Arc::new(RandomTypefaceData {
            proxy,
            paint,
            fake_it,
        });
        Typeface::new(Arc::new(Self { core, data }))
    }

    /// `SkRandomTypeface::proxy()`.
    // Port of: tools/fonts/RandomScalerContext.h#L27 (chrome/m156)
    #[must_use]
    pub fn proxy(&self) -> &Typeface {
        &self.data.proxy
    }

    /// `SkRandomTypeface::paint()`.
    // Port of: tools/fonts/RandomScalerContext.h#L28 (chrome/m156)
    #[must_use]
    pub fn paint(&self) -> &Paint {
        &self.data.paint
    }
}

/// `RandomScalerContext`: draws each glyph of its proxy in the mask format its glyph id selects.
// Port of: tools/fonts/RandomScalerContext.cpp#L24-L47 (chrome/m156)
#[derive(Debug)]
struct RandomScalerContext {
    data: Arc<RandomTypefaceData>,
    /// `fProxy`: the scaler context of the proxy typeface.
    proxy: ScalerContext,
    /// `fProxyGlyphs`: the glyphs whose path the proxy has, when the random glyph does not. The
    /// originals are kept here, keyed by their packed id.
    proxy_glyphs: HashMap<PackedGlyphId, Glyph>,
}

impl RandomScalerContext {
    /// The constructor: `fProxy` is the proxy typeface's scaler context for the same descriptor.
    // Port of: tools/fonts/RandomScalerContext.cpp#L49-L55 (chrome/m156)
    fn new(data: Arc<RandomTypefaceData>, desc: &Descriptor) -> Self {
        let proxy = data
            .proxy
            .create_scaler_context(&ScalerContextEffects::default(), desc);
        Self {
            data,
            proxy,
            proxy_glyphs: HashMap::new(),
        }
    }

    /// Whether the proxy glyph of `glyph` exists and has a path (`shadowProxyGlyph->path()`).
    fn has_proxy_path(&self, glyph: &Glyph) -> bool {
        self.proxy_glyphs
            .get(&glyph.packed_id())
            .is_some_and(|proxy_glyph| proxy_glyph.path().is_some())
    }
}

impl ScalerContextImpl for RandomScalerContext {
    /// `RandomScalerContext::generateMetrics`: the format comes from the glyph id modulo 4. The
    /// glyph ids 2 mod 4 are ARGB glyphs, which are made from the proxy's path when it has one.
    // Port of: tools/fonts/RandomScalerContext.cpp#L57-L107 (chrome/m156)
    fn generate_metrics(&mut self, orig_glyph: &Glyph, _base: &ScalerContextBase) -> GlyphMetrics {
        // Here we will change the mask format of the glyph
        // NOTE: this may be overridden by the base class (e.g. if a mask filter is applied).
        let format = match orig_glyph.glyph_id() % 4 {
            0 => MaskFormat::Lcd16,
            1 => MaskFormat::A8,
            2 => MaskFormat::Argb32,
            // `% 4` is 3 here; the switch has no other case.
            _ => MaskFormat::BW,
        };

        let mut glyph = self
            .proxy
            .internal_make_glyph(orig_glyph.packed_id(), format);

        let mut mx = GlyphMetrics::new(MaskFormat::A8);
        mx.advance = glyph.advance_vector();
        mx.bounds = glyph.rect();
        mx.mask_format = glyph.mask_format();
        mx.extra_bits = glyph.extra_bits();

        if self.data.fake_it || glyph.glyph_id() % 4 != 2 {
            mx.never_request_path = glyph.set_path_has_been_called() && glyph.path().is_none();
            mx.compute_from_path = !mx.never_request_path;
            return mx;
        }

        self.proxy.get_path(&mut glyph);
        if glyph.path().is_none() {
            mx.never_request_path = true;
            return mx;
        }

        // The proxy glyph has a path, but this glyph does not.
        // Stash the proxy glyph so it can be used later.
        let packed_id = glyph.packed_id();
        self.proxy_glyphs.insert(packed_id, glyph);
        let proxy_glyph = &self.proxy_glyphs[&packed_id];
        let proxy_path = proxy_glyph
            .path()
            .expect("the stashed proxy glyph has a path");

        mx.never_request_path = true;
        mx.mask_format = MaskFormat::Argb32;
        mx.advance = proxy_glyph.advance_vector();
        mx.extra_bits = proxy_glyph.extra_bits();

        let new_bounds = self
            .data
            .paint
            .do_compute_fast_bounds(proxy_path.bounds(), Style::Fill);
        // `newBounds.roundOut(&mx.bounds)`
        mx.bounds = RoundOut::<Rect>::round_out(&new_bounds);

        mx
    }

    /// `RandomScalerContext::generateImage`: the proxy's image, or the stashed proxy path drawn
    /// with the random typeface's paint. A hairline path is stroked with a zero width.
    // Port of: tools/fonts/RandomScalerContext.cpp#L109-L138 (chrome/m156)
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], _base: &ScalerContextBase) {
        if self.data.fake_it {
            image.fill(0);
            return;
        }

        let stashed = self
            .proxy_glyphs
            .get(&glyph.packed_id())
            .filter(|proxy_glyph| proxy_glyph.path().is_some());
        let Some(proxy_glyph) = stashed else {
            // `fProxy->getImage(glyph)`: the proxy makes the image in its own copy of the glyph.
            let mut proxy_image_glyph = glyph.clone();
            self.proxy.get_image(&mut proxy_image_glyph);
            let made = proxy_image_glyph
                .image()
                .expect("the proxy makes the glyph's image");
            image.copy_from_slice(&made[..image.len()]);
            return;
        };
        let path = proxy_glyph
            .path()
            .expect("the stashed proxy glyph has a path");
        let hairline = proxy_glyph.path_is_hairline();

        let mut bm = Bitmap::new();
        let info =
            ImageInfo::new_n32_premul((i32::from(glyph.width()), i32::from(glyph.height())), None);
        // The C++ ignores the result: the info and the pixels always match.
        let _ = bm.install_pixels(&info, vec![0; image.len()], glyph.row_bytes());
        bm.erase_color(Color::new(0));

        {
            let canvas = Canvas::from_bitmap(&mut bm, None).expect("a canvas for the bitmap");
            canvas.translate((-int_to_scalar(glyph.left()), -int_to_scalar(glyph.top())));
            let mut paint = self.data.paint.clone();
            if hairline {
                // We have a device path with effects already applied which is normally a fill
                // path. However here we do not have a fill path and there is no area to fill.
                paint.set_style(Style::Stroke);
                paint.set_stroke_width(0.0);
                paint.set_stroke(false);
            }
            // Need to modify the paint if the devPath is hairline
            canvas.draw_path(path, &paint);
        }

        let pixels = bm
            .peek_pixels()
            .and_then(|pixmap| pixmap.bytes().map(<[u8]>::to_vec))
            .expect("the glyph bitmap has pixels");
        image.copy_from_slice(&pixels[..image.len()]);
    }

    /// `RandomScalerContext::generatePath`: no path when the stashed proxy glyph has one, else the
    /// proxy's path.
    // Port of: tools/fonts/RandomScalerContext.cpp#L140-L147 (chrome/m156)
    fn generate_path(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> Option<GeneratedPath> {
        if self.has_proxy_path(glyph) {
            return None;
        }
        self.proxy.generate_path(glyph)
    }

    /// `RandomScalerContext::generateDrawable`: no drawable when the stashed proxy glyph has a
    /// path, else the proxy's drawable.
    // Port of: tools/fonts/RandomScalerContext.cpp#L149-L155 (chrome/m156)
    fn generate_drawable(
        &mut self,
        glyph: &Glyph,
        _base: &ScalerContextBase,
    ) -> Option<skia_rust_core::drawable::Drawable> {
        if self.has_proxy_path(glyph) {
            return None;
        }
        self.proxy.get_drawable(glyph)
    }

    /// `RandomScalerContext::generateFontMetrics`.
    // Port of: tools/fonts/RandomScalerContext.cpp#L157-L159 (chrome/m156)
    fn generate_font_metrics(
        &mut self,
        _base: &ScalerContextBase,
    ) -> skia_rust_core::font_metrics::FontMetrics {
        self.proxy.get_font_metrics()
    }
}

impl TypefaceBase for RandomTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `SkRandomTypeface::onGetFontDescriptor`: the proxy's descriptor.
    // Port of: tools/fonts/RandomScalerContext.cpp#L202-L206 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        self.data.proxy.font_descriptor()
    }

    /// `SkRandomTypeface::onOpenStream`: the proxy's stream.
    // Port of: tools/fonts/RandomScalerContext.cpp#L190-L192 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn skia_rust_core::stream::StreamAsset>, i32)> {
        self.data.proxy.open_stream()
    }

    /// `SkRandomTypeface::onGetFamilyName`: the proxy's family name.
    // Port of: tools/fonts/RandomScalerContext.cpp#L217-L219 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        self.data.proxy.family_name()
    }

    /// `SkRandomTypeface::onGetVariationDesignPosition`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L237-L240 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        self.data.proxy.variation_design_position()
    }

    /// `SkRandomTypeface::onGetVariationDesignParameters`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L242-L245 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        self.data.proxy.variation_design_parameters()
    }

    /// `SkRandomTypeface::onGetUPEM`: the proxy's units per em (0 when it has none).
    // Port of: tools/fonts/RandomScalerContext.cpp#L215 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        self.data.proxy.units_per_em().unwrap_or(0)
    }

    /// `SkRandomTypeface::onGetPostScriptName`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L221-L223 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        self.data.proxy.post_script_name()
    }

    /// `SkRandomTypeface::onCreateFamilyNameIterator`: the proxy's names.
    // Port of: tools/fonts/RandomScalerContext.cpp#L225-L227 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::new(
            self.data.proxy.new_family_name_iterator().collect(),
        ))
    }

    /// `SkRandomTypeface::onGetTableTags`: the proxy's tables.
    // Port of: tools/fonts/RandomScalerContext.cpp#L247-L249 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        let mut tags = vec![0; self.data.proxy.count_tables()];
        self.data.proxy.read_table_tags(&mut tags);
        tags
    }

    /// `SkRandomTypeface::onGetTableData`: the proxy's table data. An empty `data` is a size
    /// query, which C++ makes with a null pointer.
    // Port of: tools/fonts/RandomScalerContext.cpp#L251-L260 (chrome/m156)
    fn on_get_table_data(
        &self,
        tag: FourByteTag,
        offset: usize,
        length: usize,
        data: &mut [u8],
    ) -> usize {
        let out = if data.is_empty() { None } else { Some(data) };
        self.data.proxy.get_table_data(tag, offset, length, out)
    }

    /// `SkRandomTypeface::onFilterRec`: the proxy's filter, then no hinting and ARGB32 masks.
    // Port of: tools/fonts/RandomScalerContext.cpp#L176-L180 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec) {
        self.data.proxy.filter_rec(rec);
        rec.set_hinting(FontHinting::None);
        rec.mask_format = MaskFormat::Argb32;
    }

    /// `SkRandomTypeface::onGlyphMaskNeedsCurrentColor`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L233-L235 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        self.data.proxy.glyph_mask_needs_current_color()
    }

    /// `SkRandomTypeface::onMakeClone`: a random typeface over the proxy's clone, with the same
    /// paint and fake-it flag.
    // Port of: tools/fonts/RandomScalerContext.cpp#L194-L200 (chrome/m156)
    fn on_make_clone(&self, _this: Typeface, args: &FontArguments<'_, '_>) -> Typeface {
        let proxy = self.data.proxy.make_clone(args);
        RandomTypeface::make(proxy, self.data.paint.clone(), self.data.fake_it)
    }

    /// `SkRandomTypeface::onCreateScalerContext`: a [`RandomScalerContext`] over the proxy's
    /// scaler context.
    // Port of: tools/fonts/RandomScalerContext.cpp#L169-L174 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        let data = Arc::clone(&self.data);
        ScalerContext::new(this, effects, desc, &GLYPH_PATH_RASTERIZER, move |_base| {
            Box::new(RandomScalerContext::new(data, desc))
        })
    }

    /// `SkRandomTypeface::onCharsToGlyphs`: the proxy's mapping.
    // Port of: tools/fonts/RandomScalerContext.cpp#L208-L211 (chrome/m156)
    fn on_chars_to_glyphs(&self, unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        self.data.proxy.unichars_to_glyphs(unichars, glyphs);
    }

    /// `SkRandomTypeface::onCountGlyphs`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L213 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        self.data.proxy.count_glyphs()
    }

    /// `SkRandomTypeface::getGlyphToUnicodeMap`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L182-L184 (chrome/m156)
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        self.data.proxy.glyph_to_unicode_map(dst);
    }

    /// `SkRandomTypeface::onGetAdvancedMetrics`: the proxy's.
    // Port of: tools/fonts/RandomScalerContext.cpp#L186-L188 (chrome/m156)
    fn on_get_advanced_metrics(
        &self,
    ) -> Option<skia_rust_core::advanced_typeface_metrics::AdvancedTypefaceMetrics> {
        self.data.proxy.advanced_metrics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_tool_utils::create_test_typeface;
    use skia_rust_core::font::Font;
    use skia_rust_core::font_style::FontStyle;
    use skia_rust_core::font_types::FontHinting;
    use skia_rust_core::packed_glyph_id::PackedGlyphId;
    use skia_rust_core::scaler_context::ScalerContextBuildFlags;
    use skia_rust_core::strike_spec::StrikeSpec;

    fn random_test_typeface() -> Typeface {
        RandomTypeface::make(
            create_test_typeface(Some("monospace"), FontStyle::default()),
            Paint::default(),
            false,
        )
    }

    /// `filterRec`: the proxy's filter, then no hinting and ARGB32 masks.
    #[test]
    fn filter_rec_makes_argb32_without_hinting() {
        let typeface = random_test_typeface();
        let mut rec = ScalerContextRec::default();
        rec.set_hinting(FontHinting::Full);
        typeface.filter_rec(&mut rec);
        assert_eq!(rec.mask_format, MaskFormat::Argb32);
        assert_eq!(rec.hinting(), FontHinting::None);
    }

    /// The mask format of a glyph follows its glyph id modulo 4 (`generateMetrics`): glyph ids
    /// 1 and 3 mod 4 are drawn as A8 and BW by the proxy.
    #[test]
    fn glyph_format_follows_glyph_id_mod_4() {
        let font = Font::from_size(random_test_typeface(), 32.0);
        let spec = StrikeSpec::make_with_no_device(&font, None, ScalerContextBuildFlags::NONE);
        let mut context = spec.create_scaler_context();
        for (glyph_id, expected) in [(5, MaskFormat::A8), (7, MaskFormat::BW)] {
            let glyph = context.make_glyph(PackedGlyphId::from_glyph_id(glyph_id));
            assert_eq!(glyph.mask_format(), expected, "glyph id {glyph_id}");
        }
    }
}
