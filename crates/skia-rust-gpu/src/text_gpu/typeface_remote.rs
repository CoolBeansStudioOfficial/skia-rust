// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTypeface_remote.h, src/core/SkTypeface_remote.cpp

//! [`TypefaceProxy`] and [`ScalerContextProxy`]: the typeface and scaler context a strike client
//! makes for a typeface that lives in the strike server's process. Their glyphs come from the
//! strike data the server sent. Anything that the strike does not have is a cache miss, which
//! is reported to the client's [`DiscardableHandleManager`] and answered with an empty glyph.
//!
//! skia-rust: Skia keeps these in `src/core`. They are here, beside the strike server and
//! client, because they refer to the client's `DiscardableHandleManager` (declared in
//! `SkChromeRemoteGlyphCache.h`), and the client needs the GPU text code.

use std::fmt;
use std::sync::Arc;

use skia_rust_core::advanced_typeface_metrics::AdvancedTypefaceMetrics;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::drawable::Drawable;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FourByteTag, GlyphId};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, ScalerContext, ScalerContextBase, ScalerContextEffects,
    ScalerContextImpl, ScalerContextRec,
};
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, TypefaceId,
};
use skia_rust_core::utf::Unichar;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER;

use crate::text_gpu::chrome_remote_glyph_cache::{
    CacheMissType, StrikeClientDiscardableHandleManager,
};

/// What a [`ScalerContextProxy`] reports a cache miss with: `(int)fRec.fTextSize`.
fn text_size_of(base: &ScalerContextBase) -> i32 {
    // The notification takes the size as an `int`, as `notifyCacheMiss(type, fRec.fTextSize)`
    // converts the float.
    #[allow(clippy::cast_possible_truncation)] // mirrors the implicit float to int conversion
    {
        base.rec().text_size as i32
    }
}

/// Whether the proxy typeface of a scaler context logs its cache misses
/// (`getProxyTypeface()->isLogging()`).
fn is_logging(base: &ScalerContextBase) -> bool {
    base.typeface()
        .downcast_ref::<TypefaceProxy>()
        .is_some_and(TypefaceProxy::is_logging)
}

/// `SkScalerContextProxy`: a scaler context with nothing to scale. Every glyph request is a
/// cache miss.
// Port of: src/core/SkTypeface_remote.h#L36-L55 (chrome/m156)
#[doc(alias = "SkScalerContextProxy")]
struct ScalerContextProxy {
    /// `fDiscardableManager`.
    discardable_manager: Arc<dyn StrikeClientDiscardableHandleManager>,
}

impl fmt::Debug for ScalerContextProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScalerContextProxy").finish_non_exhaustive()
    }
}

impl ScalerContextImpl for ScalerContextProxy {
    // Port of: src/core/SkTypeface_remote.cpp#L31-L45 (chrome/m156)
    fn generate_metrics(&mut self, glyph: &Glyph, base: &ScalerContextBase) -> GlyphMetrics {
        if is_logging(base) {
            eprintln!(
                "GlyphCacheMiss generateMetrics looking for glyph: {:x}\n  generateMetrics: {:?}",
                glyph.packed_id().value(),
                base.rec()
            );
        }

        self.discardable_manager
            .notify_cache_miss(CacheMissType::GlyphMetrics, text_size_of(base));

        GlyphMetrics::new(glyph.mask_format())
    }

    // Port of: src/core/SkTypeface_remote.cpp#L47-L60 (chrome/m156)
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], base: &ScalerContextBase) {
        if is_logging(base) {
            eprintln!("GlyphCacheMiss generateImage: {:?}", base.rec());
        }

        // There is no desperation search here, because if there was an image to be found it was
        // copied over with the metrics search.
        self.discardable_manager
            .notify_cache_miss(CacheMissType::GlyphImage, text_size_of(base));
        // Fill the glyph image with zeros so the missing glyph doesn't display unitialized
        // memory.
        let size = glyph.image_size().min(image.len());
        image[..size].fill(0);
    }

    // Port of: src/core/SkTypeface_remote.cpp#L62-L71 (chrome/m156)
    fn generate_path(&mut self, _glyph: &Glyph, base: &ScalerContextBase) -> Option<GeneratedPath> {
        if is_logging(base) {
            eprintln!("GlyphCacheMiss generatePath: {:?}", base.rec());
        }

        self.discardable_manager
            .notify_cache_miss(CacheMissType::GlyphPath, text_size_of(base));
        None
    }

    // Port of: src/core/SkTypeface_remote.cpp#L73-L82 (chrome/m156)
    fn generate_drawable(&mut self, _glyph: &Glyph, base: &ScalerContextBase) -> Option<Drawable> {
        if is_logging(base) {
            eprintln!("GlyphCacheMiss generateDrawable: {:?}", base.rec());
        }

        self.discardable_manager
            .notify_cache_miss(CacheMissType::GlyphDrawable, text_size_of(base));
        None
    }

    // Port of: src/core/SkTypeface_remote.cpp#L84-L94 (chrome/m156)
    fn generate_font_metrics(&mut self, base: &ScalerContextBase) -> FontMetrics {
        if is_logging(base) {
            eprintln!("GlyphCacheMiss generateFontMetrics: {:?}", base.rec());
        }

        // Font metrics aren't really used for render, so just zero out the data and return.
        self.discardable_manager
            .notify_cache_miss(CacheMissType::FontMetrics, text_size_of(base));
        FontMetrics::default()
    }
}

/// `SkTypefaceProxyPrototype`: the serialization format for [`TypefaceProxy`].
// Port of: src/core/SkTypeface_remote.h#L57-L89 (chrome/m156)
#[doc(alias = "SkTypefaceProxyPrototype")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypefaceProxyPrototype {
    /// `fServerTypefaceID`.
    server_typeface_id: TypefaceId,
    /// `fGlyphCount`.
    glyph_count: i32,
    /// `fStyleValue`.
    style_value: i32,
    /// `fIsFixedPitch`.
    is_fixed_pitch: bool,
    /// `fGlyphMaskNeedsCurrentColor`: used for COLRv0 or COLRv1 fonts that may need the 0xFFFF
    /// special palette index to represent foreground color. This information needs to be on here
    /// to determine how this typeface can be cached.
    glyph_mask_needs_current_color: bool,
}

impl TypefaceProxyPrototype {
    /// `SkTypefaceProxyPrototype(const SkTypeface&)`.
    // Port of: src/core/SkTypeface_remote.cpp#L119-L124 (chrome/m156)
    #[must_use]
    pub fn new(typeface: &Typeface) -> Self {
        Self {
            server_typeface_id: typeface.unique_id(),
            glyph_count: typeface.count_glyphs(),
            style_value: typeface.font_style().raw_value(),
            is_fixed_pitch: typeface.is_fixed_pitch(),
            glyph_mask_needs_current_color: typeface.glyph_mask_needs_current_color(),
        }
    }

    /// `SkTypefaceProxyPrototype(typefaceID, glyphCount, styleValue, isFixedPitch,
    /// glyphMaskNeedsCurrentColor)`.
    // Port of: src/core/SkTypeface_remote.cpp#L126-L134 (chrome/m156)
    #[must_use]
    pub fn from_parts(
        typeface_id: TypefaceId,
        glyph_count: i32,
        style_value: i32,
        is_fixed_pitch: bool,
        glyph_mask_needs_current_color: bool,
    ) -> Self {
        Self {
            server_typeface_id: typeface_id,
            glyph_count,
            style_value,
            is_fixed_pitch,
            glyph_mask_needs_current_color,
        }
    }

    /// `MakeFromBuffer(buffer)`: `None` if the buffer is invalid.
    // Port of: src/core/SkTypeface_remote.cpp#L101-L117 (chrome/m156)
    #[doc(alias = "MakeFromBuffer")]
    #[must_use]
    pub fn make_from_buffer(buffer: &mut ReadBuffer<'_>) -> Option<Self> {
        debug_assert!(buffer.is_valid());
        let typeface_id = buffer.read_uint();
        let glyph_count = buffer.read_int();
        let style_value = buffer.read32();
        let is_fixed_pitch = buffer.read_bool();
        let glyph_mask_needs_current_color = buffer.read_bool();

        if buffer.is_valid() {
            return Some(Self::from_parts(
                typeface_id,
                glyph_count,
                style_value,
                is_fixed_pitch,
                glyph_mask_needs_current_color,
            ));
        }

        None
    }

    /// `flatten(buffer)`.
    // Port of: src/core/SkTypeface_remote.cpp#L136-L142 (chrome/m156)
    pub fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_uint(self.server_typeface_id);
        buffer.write_int(self.glyph_count);
        buffer.write_int(self.style_value);
        buffer.write_bool(self.is_fixed_pitch);
        buffer.write_bool(self.glyph_mask_needs_current_color);
    }

    /// `serverTypefaceID()`.
    // Port of: src/core/SkTypeface_remote.h#L76 (chrome/m156)
    #[must_use]
    pub fn server_typeface_id(&self) -> TypefaceId {
        self.server_typeface_id
    }

    /// `style()`.
    // Port of: src/core/SkTypeface_remote.h#L79-L83 (chrome/m156)
    fn style(&self) -> FontStyle {
        FontStyle::from_raw_value(self.style_value)
    }
}

/// `SkTypefaceProxy`: a typeface that stands for one in the strike server's process.
// Port of: src/core/SkTypeface_remote.h#L91-L187 (chrome/m156)
#[doc(alias = "SkTypefaceProxy")]
pub struct TypefaceProxy {
    core: TypefaceCore,
    /// `fTypefaceID`: the id of the typeface in the server's process.
    typeface_id: TypefaceId,
    /// `fGlyphCount`.
    glyph_count: i32,
    /// `fIsLogging`.
    is_logging: bool,
    /// `fGlyphMaskNeedsCurrentColor`.
    glyph_mask_needs_current_color: bool,
    /// `fDiscardableManager`.
    discardable_manager: Arc<dyn StrikeClientDiscardableHandleManager>,
}

impl fmt::Debug for TypefaceProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypefaceProxy")
            .field("typeface_id", &self.typeface_id)
            .field("glyph_count", &self.glyph_count)
            .finish_non_exhaustive()
    }
}

impl TypefaceProxy {
    /// `SkTypefaceProxy(prototype, manager, isLogging)`.
    // Port of: src/core/SkTypeface_remote.cpp#L145-L154 (chrome/m156)
    #[must_use]
    pub fn from_prototype(
        prototype: &TypefaceProxyPrototype,
        manager: Arc<dyn StrikeClientDiscardableHandleManager>,
        is_logging: bool,
    ) -> Self {
        Self {
            core: TypefaceCore::new(prototype.style(), prototype.is_fixed_pitch),
            typeface_id: prototype.server_typeface_id,
            glyph_count: prototype.glyph_count,
            is_logging,
            glyph_mask_needs_current_color: prototype.glyph_mask_needs_current_color,
            discardable_manager: manager,
        }
    }

    /// `SkTypefaceProxy(typefaceID, glyphCount, style, isFixedPitch, glyphMaskNeedsCurrentColor,
    /// manager, isLogging)`.
    // Port of: src/core/SkTypeface_remote.cpp#L156-L170 (chrome/m156)
    #[must_use]
    pub fn new(
        typeface_id: TypefaceId,
        glyph_count: i32,
        style: FontStyle,
        is_fixed_pitch: bool,
        glyph_mask_needs_current_color: bool,
        manager: Arc<dyn StrikeClientDiscardableHandleManager>,
        is_logging: bool,
    ) -> Self {
        Self {
            core: TypefaceCore::new(style, is_fixed_pitch),
            typeface_id,
            glyph_count,
            is_logging,
            glyph_mask_needs_current_color,
            discardable_manager: manager,
        }
    }

    /// `remoteTypefaceID()`: the id of the typeface in the server's process.
    // Port of: src/core/SkTypeface_remote.h#L106 (chrome/m156)
    #[doc(alias = "remoteTypefaceID")]
    #[must_use]
    pub fn remote_typeface_id(&self) -> TypefaceId {
        self.typeface_id
    }

    /// `glyphCount()`.
    // Port of: src/core/SkTypeface_remote.h#L108 (chrome/m156)
    #[must_use]
    pub fn glyph_count(&self) -> i32 {
        self.glyph_count
    }

    /// `isLogging()`.
    // Port of: src/core/SkTypeface_remote.h#L110 (chrome/m156)
    #[must_use]
    pub fn is_logging(&self) -> bool {
        self.is_logging
    }
}

impl TypefaceBase for TypefaceProxy {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    // Port of: src/core/SkTypeface_remote.h#L172-L174 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L116-L118 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L137-L140 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        // Used by SkStrikeCache::DumpMemoryStatistics.
        String::new()
    }

    // Port of: src/core/SkTypeface_remote.h#L126-L129 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L130-L132 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L115 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L141-L143 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L144-L146 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L147-L149 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L150-L152 (chrome/m156)
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L168-L171 (chrome/m156)
    fn on_filter_rec(&self, _rec: &mut ScalerContextRec) {
        // The rec filtering is already applied by the server when generating the glyphs.
    }

    // Port of: src/core/SkTypeface_remote.h#L119-L121 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        self.glyph_mask_needs_current_color
    }

    // Port of: src/core/SkTypeface_remote.h#L122-L124 (chrome/m156)
    fn on_make_clone(&self, _this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L159-L166 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        let discardable_manager = Arc::clone(&self.discardable_manager);
        ScalerContext::new(this, effects, desc, &GLYPH_PATH_RASTERIZER, move |_| {
            Box::new(ScalerContextProxy {
                discardable_manager,
            })
        })
    }

    // Port of: src/core/SkTypeface_remote.h#L178-L180 (chrome/m156)
    fn on_chars_to_glyphs(&self, _unichars: &[Unichar], _glyphs: &mut [GlyphId]) {
        unreachable!("Should never be called.")
    }

    // Port of: src/core/SkTypeface_remote.h#L181-L183 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        self.glyph_count()
    }

    // Port of: src/core/SkTypeface_remote.h#L176-L177 (chrome/m156)
    fn on_get_advanced_metrics(&self) -> Option<AdvancedTypefaceMetrics> {
        unreachable!("Should never be called.")
    }
}
