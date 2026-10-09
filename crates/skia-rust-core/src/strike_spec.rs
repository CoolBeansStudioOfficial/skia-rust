// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStrikeSpec.{h,cpp}, and the descriptor half of
// SkScalerContext::CreateDescriptorAndEffectsUsingPaint (src/core/SkScalerContext.cpp#L1314-L1366)

//! `SkStrikeSpec`: what a strike is made for (a descriptor, the typeface and effects), and the
//! `SkBulkGlyphMetrics` family that reads a strike's glyphs in bulk.
//!
//! The descriptor holds the record, and, if the paint has a path effect or a mask filter, the
//! flattened effects in a `kEffects_SkDescriptorTag` entry, so that a strike is keyed by them.

use std::sync::Arc;

use crate::descriptor::{AutoDescriptor, Descriptor, EFFECTS_TAG, REC_TAG};
use crate::font::Font;
use crate::font_types::GlyphId;
use crate::glyph::Glyph;
use crate::matrix::Matrix;
use crate::packed_glyph_id::PackedGlyphId;
use crate::paint::{Paint, Style};
use crate::scalar::scalar;
use crate::scaler_context::{
    SCALER_CONTEXT_REC_SIZE, ScalerContext, ScalerContextBuildFlags, ScalerContextEffects,
    ScalerContextRec, make_text_matrix,
};
use crate::strike::Strike;
use crate::strike_cache::StrikeCache;
use crate::surface_props::SurfaceProps;
use crate::typeface::Typeface;
use crate::write_buffer::BinaryWriteBuffer;

/// What a strike is made for (`SkStrikeSpec`): the descriptor (record and effects), the typeface
/// and the effects themselves.
// Port of: src/core/SkStrikeSpec.h#L19-L70 (chrome/m156)
#[doc(alias = "SkStrikeSpec")]
#[derive(Clone, Debug)]
pub struct StrikeSpec {
    /// `fAutoDescriptor`.
    auto_descriptor: AutoDescriptor,
    /// `fTypeface`.
    typeface: Typeface,
    /// `fMaskFilter` and `fPathEffect`.
    effects: ScalerContextEffects,
}

impl StrikeSpec {
    /// `SkStrikeSpec(font, paint, surfaceProps, flags, deviceMatrix)`: the descriptor and effects
    /// of a font drawn with a paint.
    // Port of: src/core/SkStrikeSpec.cpp#L135-L146 (chrome/m156)
    fn from_font_and_paint(
        font: &Font,
        paint: &Paint,
        surface_props: &SurfaceProps,
        flags: ScalerContextBuildFlags,
        device_matrix: &Matrix,
    ) -> Self {
        let (rec, effects) =
            ScalerContext::make_rec_and_effects(font, paint, surface_props, flags, device_matrix);
        let auto_descriptor = auto_descriptor_given_rec_and_effects(&rec, &effects);
        Self {
            auto_descriptor,
            typeface: font.typeface().clone(),
            effects,
        }
    }

    /// `SkStrikeSpec::MakeMask`.
    // Port of: src/core/SkStrikeSpec.cpp#L31-L37 (chrome/m156)
    #[doc(alias = "MakeMask")]
    #[must_use]
    pub fn make_mask(
        font: &Font,
        paint: &Paint,
        surface_props: &SurfaceProps,
        flags: ScalerContextBuildFlags,
        device_matrix: &Matrix,
    ) -> Self {
        Self::from_font_and_paint(font, paint, surface_props, flags, device_matrix)
    }

    /// `SkStrikeSpec::MakeTransformMask`: a mask strike without subpixel positioning.
    // Port of: src/core/SkStrikeSpec.cpp#L39-L47 (chrome/m156)
    #[doc(alias = "MakeTransformMask")]
    #[must_use]
    pub fn make_transform_mask(
        font: &Font,
        paint: &Paint,
        surface_props: &SurfaceProps,
        flags: ScalerContextBuildFlags,
        device_matrix: &Matrix,
    ) -> Self {
        let mut source = font.clone();
        source.set_subpixel(false);
        Self::from_font_and_paint(&source, paint, surface_props, flags, device_matrix)
    }

    /// `SkStrikeSpec::MakePath`: the font and paint set up to be drawn as paths, with the scale
    /// from the canonical path size back to the font's size.
    // Port of: src/core/SkStrikeSpec.cpp#L49-L67 (chrome/m156)
    #[doc(alias = "MakePath")]
    #[must_use]
    pub fn make_path(
        font: &Font,
        paint: &Paint,
        surface_props: &SurfaceProps,
        flags: ScalerContextBuildFlags,
    ) -> (Self, scalar) {
        let mut path_paint = paint.clone();
        let mut path_font = font.clone();
        path_font.set_subpixel(false);
        let strike_to_source_scale = path_font.setup_for_as_paths(Some(&mut path_paint));
        let spec =
            Self::from_font_and_paint(&path_font, &path_paint, surface_props, flags, Matrix::i());
        (spec, strike_to_source_scale)
    }

    /// `SkStrikeSpec::MakeCanonicalized`: the spec for `font` and `paint` with the size and
    /// effects that `should_draw_as_path` decides on. Returns the scale from the strike to the
    /// source font.
    // Port of: src/core/SkStrikeSpec.cpp#L69-L89 (chrome/m156)
    #[doc(alias = "MakeCanonicalized")]
    #[must_use]
    pub fn make_canonicalized(font: &Font, paint: Option<&Paint>) -> (Self, scalar) {
        let mut canonicalized_paint = paint.cloned().unwrap_or_default();
        let mut canonicalized_font = font.clone();
        let mut strike_to_source_scale = 1.0;
        if Self::should_draw_as_path(&canonicalized_paint, font, Matrix::i()) {
            strike_to_source_scale = canonicalized_font.setup_for_as_paths(None);
            canonicalized_paint.reset();
        }
        let spec = Self::from_font_and_paint(
            &canonicalized_font,
            &canonicalized_paint,
            &SurfaceProps::default(),
            ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
            Matrix::i(),
        );
        (spec, strike_to_source_scale)
    }

    /// `SkStrikeSpec::MakeWithNoDevice`: the spec of a font and paint with no device matrix.
    // Port of: src/core/SkStrikeSpec.cpp#L91-L100 (chrome/m156)
    #[doc(alias = "MakeWithNoDevice")]
    #[must_use]
    pub fn make_with_no_device(
        font: &Font,
        paint: Option<&Paint>,
        flags: ScalerContextBuildFlags,
    ) -> Self {
        let setup_paint = paint.cloned().unwrap_or_default();
        Self::from_font_and_paint(
            font,
            &setup_paint,
            &SurfaceProps::default(),
            flags,
            Matrix::i(),
        )
    }

    /// `SkStrikeSpec::ShouldDrawAsPath`: whether text this big, this skewed, in perspective, or
    /// stroked with zero width must be drawn as paths.
    // Port of: src/core/SkStrikeSpec.cpp#L101-L127 (chrome/m156)
    #[doc(alias = "ShouldDrawAsPath")]
    #[must_use]
    pub fn should_draw_as_path(paint: &Paint, font: &Font, view_matrix: &Matrix) -> bool {
        const MEMORY_LIMIT: scalar = 256.0;
        const MAX_SIZE_SQUARED: scalar = MEMORY_LIMIT * MEMORY_LIMIT;
        if paint.style() == Style::Stroke && paint.stroke_width() == 0.0 {
            return true;
        }
        if view_matrix.has_perspective() {
            return true;
        }
        let mut text_matrix = make_text_matrix(font.size(), font.scale_x(), font.skew_x());
        text_matrix.post_concat(view_matrix);
        let distance = |x: scalar, y: scalar| x * x + y * y;
        distance(text_matrix.scale_x(), text_matrix.skew_y()) > MAX_SIZE_SQUARED
            || distance(text_matrix.skew_x(), text_matrix.scale_y()) > MAX_SIZE_SQUARED
    }

    /// `SkStrikeSpec::descriptor`.
    // Port of: src/core/SkStrikeSpec.h#L73 (chrome/m156)
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.auto_descriptor.get_desc()
    }

    /// `SkStrikeSpec::typeface`.
    #[must_use]
    pub fn typeface(&self) -> &Typeface {
        &self.typeface
    }

    /// `SkStrikeSpec::createScalerContext`: a new scaler context for this spec.
    // Port of: src/core/SkStrikeSpec.h#L80 (chrome/m156)
    #[must_use]
    pub fn create_scaler_context(&self) -> ScalerContext {
        self.typeface
            .create_scaler_context(&self.effects, self.descriptor())
    }

    /// `SkStrikeSpec::findOrCreateStrike()`: the strike in the global cache.
    // Port of: src/core/SkStrikeSpec.cpp#L153-L156 (chrome/m156)
    #[doc(alias = "findOrCreateStrike")]
    #[must_use]
    pub fn find_or_create_strike(&self) -> Arc<Strike> {
        StrikeCache::global().find_or_create_strike(self)
    }
}

/// `SkScalerContext::AutoDescriptorGivenRecAndEffects`: the descriptor of the record, and the
/// flattened effects (`kEffects_SkDescriptorTag`) if there are any.
// Port of: src/core/SkScalerContext.cpp#L1332-L1366 (chrome/m156), calculate_size_and_flatten and
// generate_descriptor
pub(crate) fn auto_descriptor_given_rec_and_effects(
    rec: &ScalerContextRec,
    effects: &ScalerContextEffects,
) -> AutoDescriptor {
    let effect_bytes = flatten_effects(effects);
    let entry_count = if effect_bytes.is_empty() { 1 } else { 2 };
    let mut auto_descriptor = AutoDescriptor::new();
    auto_descriptor.reset(
        SCALER_CONTEXT_REC_SIZE + effect_bytes.len() + Descriptor::compute_overhead(entry_count),
    );
    let desc = auto_descriptor.get_desc_mut();
    desc.add_entry(REC_TAG, SCALER_CONTEXT_REC_SIZE, Some(&rec.to_bytes()));
    if !effect_bytes.is_empty() {
        desc.add_entry(EFFECTS_TAG, effect_bytes.len(), Some(&effect_bytes));
    }
    desc.compute_checksum();
    auto_descriptor
}

/// The flattened path effect, then the mask filter, of `effects`: the body of the
/// `kEffects_SkDescriptorTag` entry. Empty if there are neither.
// Port of: src/core/SkScalerContext.cpp#L1332-L1343 (chrome/m156), calculate_size_and_flatten
fn flatten_effects(effects: &ScalerContextEffects) -> Vec<u8> {
    let mut buffer = BinaryWriteBuffer::new();
    if let Some(path_effect) = &effects.path_effect {
        buffer.write_path_effect(Some(path_effect));
    }
    if let Some(mask_filter) = &effects.mask_filter {
        buffer.write_mask_filter(Some(mask_filter));
    }
    let mut bytes = vec![0; buffer.bytes_written()];
    buffer.write_to_memory(&mut bytes);
    bytes
}

/// `SkBulkGlyphMetrics`: the metrics of glyphs in a strike.
// Port of: src/core/SkStrikeSpec.h#L122-L141 (chrome/m156)
#[doc(alias = "SkBulkGlyphMetrics")]
#[derive(Clone, Debug)]
pub struct BulkGlyphMetrics {
    strike: Arc<Strike>,
}

impl BulkGlyphMetrics {
    /// `SkBulkGlyphMetrics(const SkStrikeSpec&)`: the strike of `spec` in the global cache.
    // Port of: src/core/SkStrikeSpec.cpp#L161-L163 (chrome/m156)
    #[must_use]
    pub fn new(spec: &StrikeSpec) -> Self {
        Self {
            strike: spec.find_or_create_strike(),
        }
    }

    /// The metrics of each glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L166-L169 (chrome/m156)
    #[must_use]
    pub fn glyphs(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        self.strike.metrics(glyph_ids)
    }

    /// The metrics of one glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L171-L173 (chrome/m156)
    #[must_use]
    pub fn glyph(&self, glyph_id: GlyphId) -> Glyph {
        self.glyphs(&[glyph_id])
            .into_iter()
            .next()
            .expect("one glyph in, one glyph out")
    }
}

/// `SkBulkGlyphMetricsAndPaths`: the metrics and paths of glyphs in a strike.
// Port of: src/core/SkStrikeSpec.h#L143-L162 (chrome/m156)
#[doc(alias = "SkBulkGlyphMetricsAndPaths")]
#[derive(Clone, Debug)]
pub struct BulkGlyphMetricsAndPaths {
    strike: Arc<Strike>,
}

impl BulkGlyphMetricsAndPaths {
    /// `SkBulkGlyphMetricsAndPaths(const SkStrikeSpec&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L175-L177 (chrome/m156)
    #[must_use]
    pub fn new(spec: &StrikeSpec) -> Self {
        Self {
            strike: spec.find_or_create_strike(),
        }
    }

    /// `SkBulkGlyphMetricsAndPaths(sk_sp<SkStrike>&&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L178-L180 (chrome/m156)
    #[must_use]
    pub fn from_strike(strike: Arc<Strike>) -> Self {
        Self { strike }
    }

    /// The metrics and paths of each glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L183-L186 (chrome/m156)
    #[must_use]
    pub fn glyphs(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        self.strike.prepare_paths(glyph_ids)
    }

    /// The metrics and path of one glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L188-L190 (chrome/m156)
    #[must_use]
    pub fn glyph(&self, glyph_id: GlyphId) -> Glyph {
        self.glyphs(&[glyph_id])
            .into_iter()
            .next()
            .expect("one glyph in, one glyph out")
    }
}

/// `SkBulkGlyphMetricsAndDrawables`: the metrics and drawables of glyphs in a strike.
// Port of: src/core/SkStrikeSpec.h#L164-L183 (chrome/m156)
#[doc(alias = "SkBulkGlyphMetricsAndDrawables")]
#[derive(Clone, Debug)]
pub struct BulkGlyphMetricsAndDrawables {
    strike: Arc<Strike>,
}

impl BulkGlyphMetricsAndDrawables {
    /// `SkBulkGlyphMetricsAndDrawables(const SkStrikeSpec&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L213-L215 (chrome/m156)
    #[must_use]
    pub fn new(spec: &StrikeSpec) -> Self {
        Self {
            strike: spec.find_or_create_strike(),
        }
    }

    /// `SkBulkGlyphMetricsAndDrawables(sk_sp<SkStrike>&&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L203-L205 (chrome/m156)
    #[must_use]
    pub fn from_strike(strike: Arc<Strike>) -> Self {
        Self { strike }
    }

    /// The metrics and drawables of each glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L208-L211 (chrome/m156)
    #[must_use]
    pub fn glyphs(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        self.strike.prepare_drawables(glyph_ids)
    }

    /// The metrics and drawable of one glyph.
    // Port of: src/core/SkStrikeSpec.cpp#L213-L215 (chrome/m156)
    #[must_use]
    pub fn glyph(&self, glyph_id: GlyphId) -> Glyph {
        self.glyphs(&[glyph_id])
            .into_iter()
            .next()
            .expect("one glyph in, one glyph out")
    }
}

/// `SkBulkGlyphMetricsAndImages`: the metrics and mask images of packed glyph ids in a strike.
// Port of: src/core/SkStrikeSpec.h#L185-L205 (chrome/m156)
#[doc(alias = "SkBulkGlyphMetricsAndImages")]
#[derive(Clone, Debug)]
pub struct BulkGlyphMetricsAndImages {
    strike: Arc<Strike>,
}

impl BulkGlyphMetricsAndImages {
    /// `SkBulkGlyphMetricsAndImages(const SkStrikeSpec&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L217-L219 (chrome/m156)
    #[must_use]
    pub fn new(spec: &StrikeSpec) -> Self {
        Self {
            strike: spec.find_or_create_strike(),
        }
    }

    /// `SkBulkGlyphMetricsAndImages(sk_sp<SkStrike>&&)`.
    // Port of: src/core/SkStrikeSpec.cpp#L220-L222 (chrome/m156)
    #[must_use]
    pub fn from_strike(strike: Arc<Strike>) -> Self {
        Self { strike }
    }

    /// The metrics and images of each packed glyph id.
    // Port of: src/core/SkStrikeSpec.cpp#L225-L228 (chrome/m156)
    #[must_use]
    pub fn glyphs(&self, packed_ids: &[PackedGlyphId]) -> Vec<Glyph> {
        self.strike.prepare_images(packed_ids)
    }

    /// The metrics and image of one packed glyph id.
    // Port of: src/core/SkStrikeSpec.cpp#L230-L232 (chrome/m156)
    #[must_use]
    pub fn glyph(&self, packed_id: PackedGlyphId) -> Glyph {
        self.glyphs(&[packed_id])
            .into_iter()
            .next()
            .expect("one glyph in, one glyph out")
    }

    /// The descriptor of the strike.
    // Port of: src/core/SkStrikeSpec.cpp#L234-L236 (chrome/m156)
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.strike.descriptor()
    }
}
