// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/TextBlob.h, src/text/gpu/TextBlob.cpp

//! [`TextBlob`]: a fully processed `SkTextBlob`, suitable for nearly immediate drawing on the
//! GPU. These are initially created with valid positions and colors, but with invalid texture
//! coordinates.
//!
//! A text blob contains a number of sub runs (a [`SubRunContainer`]). Each sub run tracks its own
//! glyph and position data.
//!
//! In these classes, the convention about matrices and origins is:
//! * drawMatrix and drawOrigin - describes transformations for the current draw command.
//! * positionMatrix - is equal to drawMatrix * [drawOrigin-as-translation-matrix]
//! * initial Matrix - describes the combined initial matrix and origin the `TextBlob` was created
//!   with.

use skia_rust_core::color::Color;
use skia_rust_core::color_data::compute_luminance;
use skia_rust_core::font_priv::approximate_transformed_text_size;
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::mask_filter::BlurRec;
use skia_rust_core::mask_gamma::MaskGamma;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::paint_priv::compute_luminance_color;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::surface_props::PixelGeometry;

use crate::text_gpu::sub_run_container::{
    StrikeDeviceInfo, SubRunContainer, SubRunCreationBehavior, SubRunTarget,
};

/// Check for integer translate with the same 2x2 matrix. Returns the translation, and true if the
/// change from initial matrix to the position matrix support using direct glyph masks.
// Port of: src/text/gpu/TextBlob.cpp#L33-L47 (chrome/m156)
// The matrix entries are compared exactly, as the C++ does.
#[allow(clippy::float_cmp)]
fn can_use_direct(initial_position_matrix: &Matrix, position_matrix: &Matrix) -> (bool, Vector) {
    // The existing direct glyph info can be used if the initialPositionMatrix, and the
    // positionMatrix have the same 2x2, and the translation between them is integer. Calculate
    // the translation in source space to a translation in device space by mapping (0, 0) through
    // both the initial position matrix and the position matrix; take the difference.
    let translation = position_matrix.map_origin() - initial_position_matrix.map_origin();
    (
        initial_position_matrix.scale_x() == position_matrix.scale_x()
            && initial_position_matrix.scale_y() == position_matrix.scale_y()
            && initial_position_matrix.skew_x() == position_matrix.skew_x()
            && initial_position_matrix.skew_y() == position_matrix.skew_y()
            && skia_rust_core::scalar::scalar_is_int(translation.x)
            && skia_rust_core::scalar::scalar_is_int(translation.y),
        translation,
    )
}

/// `compute_canonical_color(paint, lcd)`.
// Port of: src/text/gpu/TextBlob.cpp#L50-L74 (chrome/m156)
fn compute_canonical_color(paint: &Paint, lcd: bool) -> Color {
    let canonical_color = compute_luminance_color(paint);
    if lcd {
        // This is the correct computation for canonicalColor, but there are tons of cases where
        // LCD can be modified. For now we just regenerate if any run in a textblob has LCD.
        // TODO figure out where all of these modifications are and see if we can incorporate that
        //      logic at a higher level *OR* use sRGB
        //canonicalColor = SkMaskGamma::CanonicalColor(canonicalColor);

        // TODO we want to figure out a way to be able to use the canonical color on LCD text,
        // see the note above.  We pick a placeholder value for LCD text to ensure we always match
        // the same key
        Color::TRANSPARENT
    } else {
        // A8, though can have mixed BMP text but it shouldn't matter because BMP text won't have
        // gamma corrected masks anyways, nor color
        let lum = compute_luminance(
            u32::from(canonical_color.r()),
            u32::from(canonical_color.g()),
            u32::from(canonical_color.b()),
        );
        // reduce to our finite number of bits
        MaskGamma::canonical_color(Color::from_rgb(
            u8::try_from(lum).unwrap_or(u8::MAX),
            u8::try_from(lum).unwrap_or(u8::MAX),
            u8::try_from(lum).unwrap_or(u8::MAX),
        ))
    }
}

/// The key of a cached text blob: what a draw has to match for the cached blob to be used. The
/// key is not used as part of a hash map, so the hash is never taken. It's only used in a list
/// search using `==` (`TextBlob::Key`).
// Port of: src/text/gpu/TextBlob.h#L56-L83 (chrome/m156)
#[doc(alias = "TextBlob::Key")]
#[derive(Clone, Debug)]
pub struct TextBlobKey {
    /// `fUniqueID`.
    pub unique_id: u32,
    /// `fCanonicalColor`: color may affect the gamma of the mask we generate, but in a fairly
    /// limited way. Each color is assigned to on of a fixed number of buckets based on its
    /// luminance. For each luminance bucket there is a "canonical color" that represents the
    /// bucket. This functionality is currently only supported for A8.
    pub canonical_color: Color,
    /// `fFrameWidth`.
    pub frame_width: scalar,
    /// `fMiterLimit`.
    pub miter_limit: scalar,
    /// `fPixelGeometry`.
    pub pixel_geometry: PixelGeometry,
    /// `fBlurRec`.
    pub blur_rec: Option<BlurRec>,
    /// `fScalerContextFlags`.
    pub scaler_context_flags: ScalerContextBuildFlags,
    /// `fPositionMatrix`.
    pub position_matrix: Matrix,
    /// `fHasSomeDirectSubRuns`.
    pub has_some_direct_sub_runs: bool,
    /// `fHasBlur`.
    pub has_blur: bool,
    /// `fStyle`.
    pub style: Style,
    /// `fJoin`.
    pub join: Join,
}

impl Default for TextBlobKey {
    fn default() -> Self {
        Self {
            unique_id: 0,
            canonical_color: Color::TRANSPARENT,
            frame_width: 0.0,
            miter_limit: 0.0,
            pixel_geometry: PixelGeometry::Unknown,
            blur_rec: None,
            scaler_context_flags: ScalerContextBuildFlags::NONE,
            position_matrix: Matrix::i().clone(),
            has_some_direct_sub_runs: false,
            has_blur: false,
            style: Style::Fill,
            join: Join::Miter,
        }
    }
}

impl TextBlobKey {
    /// `Key::Make(glyphRunList, paint, drawMatrix, strikeDevice)`: whether the blob can be
    /// cached, and its key.
    // Port of: src/text/gpu/TextBlob.cpp#L79-L146 (chrome/m156)
    #[must_use]
    pub fn make(
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
        draw_matrix: &Matrix,
        strike_device: &StrikeDeviceInfo,
    ) -> (bool, TextBlobKey) {
        // It might be worth caching these things, but its not clear at this time
        // TODO for animated mask filters, this will fill up our cache. We need a safeguard here
        let mask_filter = paint.mask_filter();
        let blur_rec = mask_filter.as_ref().and_then(|mf| mf.as_base().as_a_blur());
        let can_cache = glyph_run_list.can_cache()
            && !(paint.path_effect().is_some() || (mask_filter.is_some() && blur_rec.is_none()));

        let mut key = TextBlobKey::default();
        if can_cache {
            let has_lcd = glyph_run_list.any_runs_lcd();

            // We canonicalize all non-lcd draws to use kUnknown_SkPixelGeometry
            let pixel_geometry = if has_lcd {
                strike_device.surface_props.pixel_geometry()
            } else {
                PixelGeometry::Unknown
            };

            let canonical_color = compute_canonical_color(paint, has_lcd);

            key.pixel_geometry = pixel_geometry;
            key.unique_id = glyph_run_list.unique_id();
            key.style = paint.style();
            if key.style != Style::Fill {
                key.frame_width = paint.stroke_width();
                key.miter_limit = paint.stroke_miter();
                key.join = paint.stroke_join();
            }
            key.has_blur = mask_filter.is_some();
            if key.has_blur {
                key.blur_rec = blur_rec;
            }
            key.canonical_color = canonical_color;
            key.scaler_context_flags = strike_device.scaler_context_flags;

            // Do any runs use direct drawing types?.
            key.has_some_direct_sub_runs = false;
            let glyph_run_list_location = glyph_run_list.source_bounds_with_origin().center();
            for run in glyph_run_list.runs() {
                let approximate_device_text_size = approximate_transformed_text_size(
                    run.font(),
                    draw_matrix,
                    glyph_run_list_location,
                );
                key.has_some_direct_sub_runs |= strike_device.sub_run_control.is_direct(
                    approximate_device_text_size,
                    paint,
                    draw_matrix,
                );
            }

            if key.has_some_direct_sub_runs {
                // Store the fractional offset of the position. We know that the matrix can't be
                // perspective at this point.
                let mapped_origin = draw_matrix.map_origin();
                key.position_matrix = draw_matrix.clone();
                key.position_matrix
                    .set_translate_x(mapped_origin.x - mapped_origin.x.floor());
                key.position_matrix
                    .set_translate_y(mapped_origin.y - mapped_origin.y.floor());
            } else {
                // For path and SDFT, the matrix doesn't matter.
                key.position_matrix = Matrix::i().clone();
            }
        }

        (can_cache, key)
    }
}

impl PartialEq for TextBlobKey {
    // Port of: src/text/gpu/TextBlob.cpp#L148-L184 (chrome/m156)
    // The scalars are compared exactly, as the C++ does.
    #[allow(clippy::float_cmp)]
    fn eq(&self, that: &TextBlobKey) -> bool {
        if self.unique_id != that.unique_id {
            return false;
        }
        if self.canonical_color != that.canonical_color {
            return false;
        }
        if self.style != that.style {
            return false;
        }
        if self.style != Style::Fill
            && (self.frame_width != that.frame_width
                || self.miter_limit != that.miter_limit
                || self.join != that.join)
        {
            return false;
        }
        if self.pixel_geometry != that.pixel_geometry {
            return false;
        }
        if self.has_blur != that.has_blur {
            return false;
        }
        if self.has_blur {
            let (Some(a), Some(b)) = (self.blur_rec, that.blur_rec) else {
                return self.blur_rec.is_none() && that.blur_rec.is_none();
            };
            if a.style != b.style || a.sigma != b.sigma {
                return false;
            }
        }

        if self.scaler_context_flags != that.scaler_context_flags {
            return false;
        }

        // DirectSubRuns do not support perspective when used with a TextBlob. SDFT, Transformed,
        // Path, and Drawable do support perspective.
        if self.position_matrix.has_perspective() && self.has_some_direct_sub_runs {
            return false;
        }

        if self.has_some_direct_sub_runs != that.has_some_direct_sub_runs {
            return false;
        }

        if self.has_some_direct_sub_runs {
            let (compatible, _) = can_use_direct(&self.position_matrix, &that.position_matrix);
            return compatible;
        }

        true
    }
}

/// A fully processed text blob (`sktext::gpu::TextBlob`).
// Port of: src/text/gpu/TextBlob.h#L50-L127 (chrome/m156)
#[doc(alias = "sktext::gpu::TextBlob")]
#[derive(Debug)]
pub struct TextBlob {
    /// `fSubRuns`.
    sub_runs: SubRunContainer,
    /// `fSize`: overall size of this struct plus vertices and glyphs.
    size: usize,
    /// `fInitialLuminance`.
    initial_luminance: Color,
    /// `fKey`.
    key: TextBlobKey,
}

impl TextBlob {
    /// `Make(glyphRunList, paint, positionMatrix, strikeDeviceInfo, strikeCache)`: makes a text
    /// blob and its sub runs.
    // Port of: src/text/gpu/TextBlob.cpp#L196-L215 (chrome/m156)
    #[must_use]
    pub fn make(
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
        position_matrix: &Matrix,
        strike_device_info: &StrikeDeviceInfo,
    ) -> TextBlob {
        let container = SubRunContainer::make(
            glyph_run_list,
            position_matrix,
            paint,
            strike_device_info,
            SubRunCreationBehavior::AddSubRuns,
        );

        let initial_luminance = compute_luminance_color(paint);
        let total_memory = std::mem::size_of::<TextBlob>() + container.unflatten_size_hint();
        TextBlob::new(container, total_memory, initial_luminance)
    }

    /// `TextBlob(alloc, subRuns, totalMemorySize, initialLuminance)`.
    // Port of: src/text/gpu/TextBlob.cpp#L246-L253 (chrome/m156)
    #[must_use]
    pub fn new(
        sub_runs: SubRunContainer,
        total_memory_size: usize,
        initial_luminance: Color,
    ) -> Self {
        Self {
            sub_runs,
            size: total_memory_size,
            initial_luminance,
            key: TextBlobKey::default(),
        }
    }

    /// `key()`.
    // Port of: src/text/gpu/TextBlob.h#L103 (chrome/m156)
    #[must_use]
    pub fn key(&self) -> &TextBlobKey {
        &self.key
    }

    /// `addKey(key)`.
    // Port of: src/text/gpu/TextBlob.cpp#L217-L219 (chrome/m156)
    pub fn add_key(&mut self, key: &TextBlobKey) {
        self.key = key.clone();
    }

    /// `size()`.
    // Port of: src/text/gpu/TextBlob.h#L110 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// The sub runs.
    #[must_use]
    pub fn sub_runs(&self) -> &SubRunContainer {
        &self.sub_runs
    }

    /// `canReuse(paint, positionMatrix)`.
    // Port of: src/text/gpu/TextBlob.cpp#L221-L236 (chrome/m156)
    #[must_use]
    pub fn can_reuse(&self, paint: &Paint, position_matrix: &Matrix) -> bool {
        // A singular matrix will create a TextBlob with no SubRuns, but unknown glyphs can also
        // cause empty runs. If there are no subRuns, then regenerate when the matrices don't
        // match.
        if self.sub_runs.is_empty() && self.sub_runs.initial_position() != position_matrix {
            return false;
        }

        // If we have LCD text then our canonical color will be set to transparent, in this case
        // we have to regenerate the blob on any color change. We use the grPaint to get any color
        // filter effects.
        if self.key.canonical_color == Color::TRANSPARENT
            && self.initial_luminance != compute_luminance_color(paint)
        {
            return false;
        }

        self.sub_runs.can_reuse(paint, position_matrix)
    }

    /// `draw(canvas, drawOrigin, paint, atlasDelegate)`.
    // Port of: src/text/gpu/TextBlob.cpp#L240-L245 (chrome/m156)
    pub fn draw(&self, target: &mut dyn SubRunTarget, draw_origin: Point, paint: &Paint) {
        self.sub_runs.draw(target, draw_origin, paint);
    }
}
