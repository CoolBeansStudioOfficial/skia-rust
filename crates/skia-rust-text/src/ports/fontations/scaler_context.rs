// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/SkTypeface_fontations.cpp#L358-L985 (chrome/m156), the outline half of
// `SkFontationsScalerContext` (T19b, docs/design/text.md §9). The colour (COLRv0/v1, T21) and
// embedded bitmap (T22) halves are not ported yet: their glyphs are drawn from the outline.

//! The Fontations scaler context: the glyph metrics, paths and font metrics of a typeface.

use std::sync::Arc;

use skia_rust_core::data::Data;
use skia_rust_core::font_metrics::{Flags, FontMetrics};
use skia_rust_core::font_types::{FontHinting, set_four_byte_tag};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::mask::{MaskBuilder, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::{
    GeneratedPath, GlyphMetrics, PreMatrixScale, ScalerContextBase, ScalerContextFlags,
    ScalerContextImpl,
};

use super::base::{
    BridgeFontRef, BridgeMappingIndex, BridgeNormalizedCoords, get_outline_collection,
    get_skia_metrics, lookup_glyph_or_zero, make_font_ref, table_data,
    unhinted_advance_width_or_zero,
};
use super::hinting::{
    AutoHintingControl, BridgeGlyphStyles, BridgeHintingInstance, hinting_reliant,
    make_hinting_instance, make_mono_hinting_instance, no_hinting_instance,
};
use super::pen::{BridgeScalerMetrics, get_path_verbs_points};
use super::typeface::is_lcd;

/// `ScalerContextBits::PATH`: the extra bits of a glyph drawn from its outline.
// Port of: src/ports/SkTypeface_fontations.cpp#L519-L525 (chrome/m156), PATH
const PATH_BITS: u16 = 1;

/// `SkSetFourByteTag('f', 'v', 'a', 'r')`: the table whose presence makes the bounds invalid.
// Port of: src/ports/SkTypeface_fontations.cpp#L915-L916 (chrome/m156)
const FVAR_TAG: u32 = set_four_byte_tag(b'f', b'v', b'a', b'r');

/// What a scaler context reads from its typeface. C++ keeps references to the typeface's
/// members. Here the context holds handles to the same state: the font data is shared and the
/// bridge state is behind `Arc`s, so the context can outlive the typeface's borrow.
#[derive(Debug, Clone)]
pub struct FontationsSource {
    /// `fFontData`: the bytes of the font.
    pub font_data: Data,
    /// `fTtcIndex`: the font index within the data.
    pub ttc_index: u32,
    /// `fMappingIndex`: the character map.
    pub mapping_index: Arc<BridgeMappingIndex>,
    /// `fBridgeNormalizedCoords`: the normalized coordinates of the instance.
    pub normalized_coords: Arc<BridgeNormalizedCoords>,
    /// `fGlyphStyles`: the lazily computed autohinter styles.
    pub glyph_styles: Arc<BridgeGlyphStyles>,
}

/// The outline scaler context of a Fontations typeface (`SkFontationsScalerContext`).
// Port of: src/ports/SkTypeface_fontations.cpp#L358-L985 (chrome/m156)
#[derive(Debug)]
pub struct FontationsScalerContext {
    source: FontationsSource,
    /// `fScale`: the pre-scale of the record, from `computeMatrices(kVertical)`.
    scale: Vector,
    /// `fRemainingMatrix`: the matrix applied after the outline is scaled.
    remaining_matrix: Matrix,
    /// `fDoLinearMetrics`: the advances are unhinted.
    do_linear_metrics: bool,
    /// `fHintingInstance`: the hinting the outlines are drawn with.
    hinting_instance: BridgeHintingInstance,
}

impl FontationsScalerContext {
    /// The constructor of `SkFontationsScalerContext`: splits the record's matrix, then picks the
    /// hinting instance the record asks for.
    // Port of: src/ports/SkTypeface_fontations.cpp#L360-L455 (chrome/m156)
    #[must_use]
    pub fn new(source: FontationsSource, base: &ScalerContextBase) -> Self {
        let rec = base.rec();
        let mut scale = Vector::new(0.0, 0.0);
        let mut remaining_matrix = Matrix::i().clone();
        rec.compute_matrices(
            PreMatrixScale::Vertical,
            &mut scale,
            &mut remaining_matrix,
            None,
            None,
            None,
        );

        // `isLinearMetrics()`: the branches below keep it or override it.
        let is_linear_metrics = rec.flags.contains(ScalerContextFlags::LINEAR_METRICS);

        let font_ref = make_font_ref(source.font_data.as_bytes(), source.ttc_index);
        let outlines = get_outline_collection(&font_ref);

        // See below for the exception for SkFontHinting::kSlight.
        let force_autohinting = rec.flags.contains(ScalerContextFlags::FORCE_AUTOHINTING);
        let mut auto_hinting = if force_autohinting {
            AutoHintingControl::ForceForGlyfAndCff
        } else {
            AutoHintingControl::Fallback
        };
        // The C++ build for the Android framework (`SK_BUILD_FOR_ANDROID_FRAMEWORK`) forces
        // `ForceOff` unless the force flag is set. That build is not ported.

        let coords = &*source.normalized_coords;
        let size = scale.y;
        // Hinting-reliant fonts exist that display incorrect contours when not executing their
        // hinting instructions. Detect those and force-enable hinting for them.
        let (hinting_instance, do_linear_metrics) = if hinting_reliant(&outlines) {
            (make_mono_hinting_instance(&outlines, size, coords), false)
        } else if rec.format() == MaskFormat::BW {
            if rec.hinting() == FontHinting::None {
                (no_hinting_instance(), true)
            } else {
                (make_mono_hinting_instance(&outlines, size, coords), false)
            }
        } else {
            match rec.hinting() {
                FontHinting::None => (no_hinting_instance(), true),
                FontHinting::Slight => {
                    // Unhinted metrics.
                    if auto_hinting != AutoHintingControl::ForceForGlyfAndCff
                        && auto_hinting != AutoHintingControl::ForceOff
                    {
                        auto_hinting = AutoHintingControl::ForceForGlyf;
                    }
                    let instance = make_hinting_instance(
                        &outlines,
                        &source.glyph_styles,
                        size,
                        coords,
                        true,  // do_light_hinting
                        false, // do_lcd_antialiasing
                        false, // lcd_orientation_vertical
                        auto_hinting,
                    );
                    (instance, true)
                }
                FontHinting::Normal => {
                    // No hinting to subpixel coordinates.
                    let instance = make_hinting_instance(
                        &outlines,
                        &source.glyph_styles,
                        size,
                        coords,
                        false, // do_light_hinting
                        false, // do_lcd_antialiasing
                        false, // lcd_orientation_vertical
                        auto_hinting,
                    );
                    (instance, is_linear_metrics)
                }
                FontHinting::Full => {
                    // Attempt to make use of hinting to subpixel coordinates.
                    let instance = make_hinting_instance(
                        &outlines,
                        &source.glyph_styles,
                        size,
                        coords,
                        false, // do_light_hinting
                        is_lcd(rec),
                        rec.flags.contains(ScalerContextFlags::LCD_VERTICAL),
                        auto_hinting,
                    );
                    (instance, is_linear_metrics)
                }
            }
        };

        Self {
            source,
            scale,
            remaining_matrix,
            do_linear_metrics,
            hinting_instance,
        }
    }

    /// The parsed font, over this context's copy of the data.
    fn font_ref(&self) -> BridgeFontRef<'_> {
        make_font_ref(self.source.font_data.as_bytes(), self.source.ttc_index)
    }

    /// The character-to-glyph lookup of one code point, 0 for none, through the font's map.
    // Port of: src/ports/SkTypeface_fontations.cpp#L462-L465 (chrome/m156)
    fn glyph_for_unichar(&self, codepoint: u32) -> u16 {
        let mut glyph = [0_u16; 1];
        lookup_glyph_or_zero(
            &self.font_ref(),
            &self.source.mapping_index,
            &[codepoint],
            &mut glyph,
        );
        glyph[0]
    }

    /// `getContourHeightForLetter`: the height of the outline of a letter, or `None` when the
    /// font has no glyph for it or the glyph has no path.
    // Port of: src/ports/SkTypeface_fontations.cpp#L461-L480 (chrome/m156)
    fn get_contour_height_for_letter(&self, letter: char) -> Option<scalar> {
        let glyph_id = self.glyph_for_unichar(u32::from(letter));
        if glyph_id == 0 {
            return None;
        }
        let (glyph_path, _scaler_metrics) =
            self.generate_path_for_glyph_id(glyph_id, self.scale.y, &self.hinting_instance)?;
        Some(glyph_path.bounds().height())
    }

    /// `generatePathForGlyphId`: the path of a glyph at `y_scale`, drawn with `hinting_instance`,
    /// with the scaler metrics of the draw.
    // Port of: src/ports/SkTypeface_fontations.cpp#L482-L517 (chrome/m156)
    fn generate_path_for_glyph_id(
        &self,
        glyph_id: u16,
        y_scale: scalar,
        hinting_instance: &BridgeHintingInstance,
    ) -> Option<(Path, BridgeScalerMetrics)> {
        let font_ref = self.font_ref();
        // The outlines borrow the font bytes, so they are read from the data for each draw.
        // C++ keeps `fOutlines` in the typeface.
        let outlines = get_outline_collection(&font_ref);
        let mut builder = PathBuilder::new();
        let scaler_metrics = get_path_verbs_points(
            &outlines,
            glyph_id,
            y_scale,
            &self.source.normalized_coords,
            hinting_instance,
            &mut builder,
        )?;
        // See https://issues.skia.org/345178242: no path simplification here, as in C++.
        Some((builder.detach(), scaler_metrics))
    }

    /// `generatePathImpl`: the glyph's path with the remaining matrix applied.
    // Port of: src/ports/SkTypeface_fontations.cpp#L831-L845 (chrome/m156)
    fn generate_path_impl(&self, glyph_id: u16) -> Option<(GeneratedPath, BridgeScalerMetrics)> {
        let (path, scaler_metrics) =
            self.generate_path_for_glyph_id(glyph_id, self.scale.y, &self.hinting_instance)?;
        let new_path = path.try_make_transform(&self.remaining_matrix)?;
        Some((
            GeneratedPath {
                path: new_path,
                modified: !self.remaining_matrix.is_identity(),
            },
            scaler_metrics,
        ))
    }
}

/// `setMetric`: a NaN metric is zero and not valid; any other value is valid.
// Port of: src/ports/SkTypeface_fontations.cpp#L937-L944 (chrome/m156)
fn set_metric(dst: &mut scalar, src: scalar, flags: &mut Flags, flag: Flags) {
    if src.is_nan() {
        *dst = 0.0;
    } else {
        *dst = src;
        flags.insert(flag);
    }
}

impl ScalerContextImpl for FontationsScalerContext {
    /// `generateMetrics` for the outline glyphs: the advance, and the request for a path. Colour
    /// and bitmap glyphs take this path too, until T21 and T22 port them.
    // Port of: src/ports/SkTypeface_fontations.cpp#L527-L721 (chrome/m156), the path branch
    #[allow(clippy::float_cmp)] // exact comparison of the hinted advance, as in C++
    fn generate_metrics(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> GlyphMetrics {
        let mut mx = GlyphMetrics::new(glyph.mask_format());
        let glyph_id = glyph.glyph_id();

        // TODO(T21): COLRv1 and COLRv0 glyphs (`has_colrv1_glyph`, `has_colrv0_glyph`) are drawn
        // from their outline, as FreeType does without `FT_LOAD_COLOR`, until the COLR port lands.
        // TODO(T22): embedded bitmap glyphs (`has_bitmap_glyph`) are drawn from their outline
        // until the bitmap port lands.
        let do_linear = self.do_linear_metrics;

        let mut x_advance = unhinted_advance_width_or_zero(
            &self.font_ref(),
            self.scale.y,
            &self.source.normalized_coords,
            glyph_id,
        );
        if !do_linear && let Some((generated, scaler_metrics)) = self.generate_path_impl(glyph_id) {
            mx.generated_path = Some(generated);

            if scaler_metrics.has_adjusted_advance {
                // FreeType rounds the advance to full pixels when in hinting modes. Match
                // FreeType and round here.
                let hinted_advance = scaler_metrics.adjusted_advance.round();
                // TODO(drott): Remove this workaround for fontations returning 0 for a space glyph
                // without contours, compare https://github.com/googlefonts/fontations/issues/905
                if hinted_advance != x_advance && hinted_advance != 0.0 {
                    x_advance = hinted_advance;
                }
            }
        }
        mx.advance = self.remaining_matrix.map_point(Point::new(x_advance, 0.0));

        mx.extra_bits = PATH_BITS;
        mx.compute_from_path = true;
        mx
    }

    /// `generateImage` for a path glyph: `generateImageFromPath` of the glyph's path. The C++
    /// asserts that the path is set. Here a glyph without one keeps an empty image.
    // Port of: src/ports/SkTypeface_fontations.cpp#L802-L829 (chrome/m156), the PATH branch
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], base: &ScalerContextBase) {
        if glyph.extra_bits() != PATH_BITS {
            // TODO(T21, T22): COLR and bitmap glyphs are never requested from this context yet.
            return;
        }
        let Some(path) = glyph.path() else {
            return;
        };
        let row_bytes = u32::try_from(glyph.row_bytes()).expect("row bytes fit in a mask");
        let mut mask = MaskBuilder::new(
            image.to_vec(),
            glyph.i_rect(),
            row_bytes,
            glyph.mask_format(),
        );
        base.generate_image_from_path(&mut mask, path, glyph.path_is_hairline());
        let copied = image.len().min(mask.image.len());
        image[..copied].copy_from_slice(&mask.image[..copied]);
    }

    /// `generatePath`: the outline of the glyph, with the remaining matrix applied.
    // Port of: src/ports/SkTypeface_fontations.cpp#L847-L851 (chrome/m156)
    fn generate_path(&mut self, glyph: &Glyph, _base: &ScalerContextBase) -> Option<GeneratedPath> {
        self.generate_path_impl(glyph.glyph_id())
            .map(|(generated, _metrics)| generated)
    }

    /// `generateFontMetrics`: the font's metrics at the context's size, with the cap and x
    /// heights synthesized from the letters when the font does not give them.
    // Port of: src/ports/SkTypeface_fontations.cpp#L906-L965 (chrome/m156)
    #[allow(clippy::float_cmp)] // `!fCapHeight` in C++ is an exact zero test
    fn generate_font_metrics(&mut self, _base: &ScalerContextBase) -> FontMetrics {
        let metrics = get_skia_metrics(
            &self.font_ref(),
            self.scale.y,
            &self.source.normalized_coords,
        );
        let mut out = FontMetrics {
            top: -metrics.top,
            ascent: -metrics.ascent,
            descent: -metrics.descent,
            bottom: -metrics.bottom,
            leading: metrics.leading,
            avg_char_width: metrics.avg_char_width,
            max_char_width: metrics.max_char_width,
            x_min: metrics.x_min,
            x_max: metrics.x_max,
            x_height: -metrics.x_height,
            cap_height: -metrics.cap_height,
            flags: Flags::empty(),
            ..FontMetrics::default()
        };

        // Cap height synthesis.
        if out.cap_height == 0.0 {
            out.cap_height = self
                .get_contour_height_for_letter('H')
                .unwrap_or(metrics.ascent);
        }

        // X height synthesis.
        if out.x_height == 0.0 {
            out.x_height = self
                .get_contour_height_for_letter('x')
                .unwrap_or(metrics.ascent);
        }

        if table_data(&self.font_ref(), FVAR_TAG, 0, &mut []) != 0 {
            out.flags.insert(Flags::BOUNDS_INVALID);
        }

        set_metric(
            &mut out.underline_position,
            -metrics.underline_position,
            &mut out.flags,
            Flags::UNDERLINE_POSITION_IS_VALID,
        );
        set_metric(
            &mut out.underline_thickness,
            metrics.underline_thickness,
            &mut out.flags,
            Flags::UNDERLINE_THICKNESS_IS_VALID,
        );
        set_metric(
            &mut out.strikeout_position,
            -metrics.strikeout_position,
            &mut out.flags,
            Flags::STRIKEOUT_POSITION_IS_VALID,
        );
        set_metric(
            &mut out.strikeout_thickness,
            metrics.strikeout_thickness,
            &mut out.flags,
            Flags::STRIKEOUT_THICKNESS_IS_VALID,
        );
        out
    }
}

#[cfg(test)]
mod tests {
    //! Draws a real glyph through a Fontations typeface's scaler context. The font is read from
    //! Skia's `resources/fonts`; the test skips when that directory is missing.

    use std::path::PathBuf;

    use skia_rust_core::font::Font;
    use skia_rust_core::font_arguments::FontArguments;
    use skia_rust_core::stream::{MemoryStream, StreamAsset};

    use crate::ports::fontations::typeface::make_from_stream;

    #[test]
    fn draws_h_through_the_scaler_context() {
        let dir = match std::env::var_os("SKIA_RESOURCES") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("third_party")
                .join("skia")
                .join("resources"),
        };
        let file = dir.join("fonts").join("Roboto-Regular.ttf");
        let Ok(bytes) = std::fs::read(&file) else {
            eprintln!("todo: skipping, missing Skia resource {}", file.display());
            return;
        };

        let stream: Box<dyn StreamAsset> = MemoryStream::make_copy(&bytes);
        let typeface = make_from_stream(stream, &FontArguments::new()).expect("Roboto is a font");
        let glyph = typeface.unichar_to_glyph('H' as i32);
        assert_ne!(glyph, 0, "Roboto has an H");

        let font = Font::from_size(typeface, 100.0);
        let path = font.get_path(glyph).expect("the H has an outline");
        let bounds = path.bounds();
        // The pen flips y, so the glyph lies above the baseline, with a positive height.
        assert!(bounds.bottom <= 0.0, "the glyph is above the baseline");
        assert!(bounds.height() > 0.0, "the H has a height");

        let (_, metrics) = font.metrics();
        assert!(metrics.cap_height > 0.0, "Roboto has a cap height");
    }
}
