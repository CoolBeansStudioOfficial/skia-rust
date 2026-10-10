// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/SubRunControl.h, src/text/gpu/SubRunControl.cpp

//! [`SubRunControl`]: the thresholds that pick how a run of glyphs is drawn (distance field
//! text, direct masks or paths), and [`SdftMatrixRange`], the range of scales one distance field
//! size can be reused for.
//!
//! Not ported: `SdftMatrixRange::flatten`/`MakeFromBuffer` (Slug serialization needs the remote
//! glyph cache, T23).

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_priv::approximate_transformed_text_size;
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::glyph::GlyphDigest;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{Scalar, scalar};

use crate::graphite::caps::Caps;

/// `kSmallDFFontLimit`: the DF sizes and thresholds for usage of the small and medium sizes. For
/// example, above `kSmallDFFontLimit` we will use the medium size.
// Port of: src/text/gpu/SubRunControl.cpp#L27-L29 (chrome/m156)
const SMALL_DF_FONT_LIMIT: scalar = 32.0;
/// `kMediumDFFontLimit`.
const MEDIUM_DF_FONT_LIMIT: scalar = 72.0;
/// `kLargeDFFontLimit`: the large size is used up until the size at which we switch over to
/// drawing as paths, as controlled by [`SubRunControl`].
const LARGE_DF_FONT_LIMIT: scalar = 162.0;

/// Two numbers `matrix_min` and `matrix_max` such that if `viewMatrix.getMaxScale()` is between
/// them then this SDFT size can be reused (`SdftMatrixRange`).
// Port of: src/text/gpu/SubRunControl.h#L33-L45 (chrome/m156)
#[doc(alias = "SDFTMatrixRange")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdftMatrixRange {
    matrix_min: scalar,
    matrix_max: scalar,
}

impl SdftMatrixRange {
    /// `SdftMatrixRange(min, max)`.
    #[must_use]
    pub fn new(min: scalar, max: scalar) -> Self {
        Self {
            matrix_min: min,
            matrix_max: max,
        }
    }

    /// `matrixInRange(matrix)`: whether the matrix' maximum scale is in `(min, max]`.
    // Port of: src/text/gpu/SubRunControl.cpp#L131-L134 (chrome/m156)
    #[must_use]
    pub fn matrix_in_range(&self, matrix: &Matrix) -> bool {
        let max_scale = matrix.max_scale();
        self.matrix_min < max_scale && max_scale <= self.matrix_max
    }
}

/// How a font drawn as distance field text is set up: the font, the scale from the nominal size
/// to the source space size, and the matrix range where the font can be reused.
pub type SDFFont = (Font, scalar, SdftMatrixRange);

/// The decisions about how to draw text on the GPU (`sktext::gpu::SubRunControl`).
// Port of: src/text/gpu/SubRunControl.h#L47-L87 (chrome/m156)
#[doc(alias = "sktext::gpu::SubRunControl")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubRunControl {
    /// `fMinDistanceFieldFontSize`: below this size (in device space) distance field text will
    /// not be used.
    min_distance_field_font_size: scalar,
    /// `fMaxDistanceFieldFontSize`: above this size (in device space) distance field text will
    /// not be used and glyphs will be rendered from outline as individual paths.
    max_distance_field_font_size: scalar,
    /// `fAbleToUseSDFT`.
    able_to_use_sdft: bool,
    /// `fAbleToUsePerspectiveSDFT`.
    able_to_use_perspective_sdft: bool,
    /// `fForcePathAA`: if true, glyphs drawn as paths are always anti-aliased regardless of any
    /// edge hinting.
    force_path_aa: bool,
    /// `fUseBilerp`.
    use_bilerp: bool,
}

impl SubRunControl {
    /// `MinSDFTRange(useSDFTForSmallText, min)`.
    // Port of: src/text/gpu/SubRunControl.cpp#L36-L41 (chrome/m156)
    fn min_sdft_range(use_sdft_for_small_text: bool, min: scalar) -> scalar {
        if !use_sdft_for_small_text {
            return LARGE_DF_FONT_LIMIT;
        }
        min
    }

    /// `SubRunControl(ableToUseSDFT, useSDFTForSmallText, useSDFTForPerspectiveText, min, max,
    /// forcePathAA, useBilerp)`.
    ///
    /// # Panics
    /// If `0 < min && min <= max` does not hold (`SkASSERT_RELEASE`).
    // Port of: src/text/gpu/SubRunControl.cpp#L43-L56 (chrome/m156)
    #[must_use]
    pub fn new(
        able_to_use_sdft: bool,
        use_sdft_for_small_text: bool,
        use_sdft_for_perspective_text: bool,
        min: scalar,
        max: scalar,
        force_path_aa: bool,
        use_bilerp: bool,
    ) -> Self {
        assert!(0.0 < min && min <= max);
        Self {
            min_distance_field_font_size: Self::min_sdft_range(use_sdft_for_small_text, min),
            max_distance_field_font_size: max,
            able_to_use_sdft,
            able_to_use_perspective_sdft: use_sdft_for_perspective_text,
            force_path_aa,
            use_bilerp,
        }
    }

    /// `Caps::getSubRunControl(useSDFTForSmallText)`.
    // Port of: src/gpu/graphite/Caps.cpp#L415-L427 (chrome/m156)
    #[must_use]
    pub fn from_caps(
        caps: &dyn Caps,
        min_distance_field_font_size: scalar,
        glyphs_as_paths_font_size: scalar,
        use_sdft_for_small_text: bool,
    ) -> Self {
        Self::new(
            // `supportsDistanceFieldText()` is `fShaderDerivativeSupport`.
            caps.shader_caps().shader_derivative_support,
            use_sdft_for_small_text,
            true, // ableToUsePerspectiveSDFT
            min_distance_field_font_size,
            glyphs_as_paths_font_size,
            true, // forcePathAA
            caps.support_bilerp_from_glyph_atlas(),
        )
    }

    /// `maxSize()`.
    #[must_use]
    pub fn max_size(&self) -> scalar {
        self.max_distance_field_font_size
    }

    /// `forcePathAA()`.
    #[must_use]
    pub fn force_path_aa(&self) -> bool {
        self.force_path_aa
    }

    /// `useBilerp()`.
    #[must_use]
    pub fn use_bilerp(&self) -> bool {
        self.use_bilerp
    }

    /// `isDirect(approximateDeviceTextSize, paint, matrix)`.
    // Port of: src/text/gpu/SubRunControl.cpp#L58-L72 (chrome/m156)
    #[must_use]
    pub fn is_direct(
        &self,
        approximate_device_text_size: scalar,
        paint: &Paint,
        matrix: &Matrix,
    ) -> bool {
        let is_sdft = self.is_sdft(approximate_device_text_size, paint, matrix);
        let max_atlas_dimension = if self.use_bilerp {
            scalar::from(GlyphDigest::SIDE_TOO_BIG_FOR_ATLAS - 2)
        } else {
            scalar::from(GlyphDigest::SIDE_TOO_BIG_FOR_ATLAS)
        };
        !is_sdft
            && !matrix.has_perspective()
            && 0.0 < approximate_device_text_size
            && approximate_device_text_size < max_atlas_dimension
    }

    /// `isSDFT(approximateDeviceTextSize, paint, matrix)`.
    // Port of: src/text/gpu/SubRunControl.cpp#L75-L86 (chrome/m156)
    #[must_use]
    pub fn is_sdft(
        &self,
        approximate_device_text_size: scalar,
        paint: &Paint,
        matrix: &Matrix,
    ) -> bool {
        let wide_stroke = paint.style() == Style::Stroke && paint.stroke_width() > 0.0;
        self.able_to_use_sdft
            && paint.mask_filter().is_none()
            && (paint.style() == Style::Fill || wide_stroke)
            && 0.0 < approximate_device_text_size
            && (self.able_to_use_perspective_sdft || !matrix.has_perspective())
            && (self.min_distance_field_font_size <= approximate_device_text_size
                || matrix.has_perspective())
            && approximate_device_text_size <= self.max_distance_field_font_size
    }

    /// `getSDFFont(font, viewMatrix, textLoc)`: produces a font, a scale factor from the nominal
    /// size to the source space size, and the matrix range where this font can be reused.
    // Port of: src/text/gpu/SubRunControl.cpp#L88-L129 (chrome/m156)
    #[must_use]
    pub fn get_sdf_font(
        &self,
        font: &Font,
        view_matrix: &Matrix,
        text_loc: Point,
    ) -> SDFFont {
        let text_size = font.size();
        let mut scaled_text_size = approximate_transformed_text_size(font, view_matrix, text_loc);
        if scaled_text_size <= 0.0
            || <scalar as Scalar>::nearly_equal(text_size, scaled_text_size, None)
        {
            scaled_text_size = text_size;
        }

        let mut df_font = font.clone();

        let (df_mask_scale_floor, df_mask_scale_ceil, df_mask_size);
        if scaled_text_size <= SMALL_DF_FONT_LIMIT {
            df_mask_scale_floor = self.min_distance_field_font_size;
            df_mask_scale_ceil = SMALL_DF_FONT_LIMIT;
            df_mask_size = SMALL_DF_FONT_LIMIT;
        } else if scaled_text_size <= MEDIUM_DF_FONT_LIMIT {
            df_mask_scale_floor = SMALL_DF_FONT_LIMIT;
            df_mask_scale_ceil = MEDIUM_DF_FONT_LIMIT;
            df_mask_size = MEDIUM_DF_FONT_LIMIT;
        } else {
            df_mask_scale_floor = MEDIUM_DF_FONT_LIMIT;
            df_mask_scale_ceil = self.max_distance_field_font_size;
            df_mask_size = LARGE_DF_FONT_LIMIT;
        }

        df_font.set_size(df_mask_size);
        df_font.set_force_auto_hinting(false);
        df_font.set_hinting(FontHinting::Normal);

        // The sub-pixel position will always happen when transforming to the screen, so we
        // effectively disable the LCD path in StrikeSpec creation by overwriting the edging
        // here.
        df_font.set_subpixel(false);
        df_font.set_edging(Edging::AntiAlias);

        let min_matrix_scale = df_mask_scale_floor / text_size;
        let max_matrix_scale = df_mask_scale_ceil / text_size;
        (
            df_font,
            text_size / df_mask_size,
            SdftMatrixRange::new(min_matrix_scale, max_matrix_scale),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> SubRunControl {
        // The default Graphite caps: SDFT on, 18 px minimum, 324 px as paths.
        SubRunControl::new(true, false, true, 18.0, 324.0, true, false)
    }

    #[test]
    fn small_text_is_direct_when_sdft_for_small_text_is_off() {
        let control = control();
        let paint = Paint::default();
        // Below the large-DF limit (162) and with sdft-for-small-text off, text is direct.
        assert!(!control.is_sdft(24.0, &paint, Matrix::i()));
        assert!(control.is_direct(24.0, &paint, Matrix::i()));
    }

    #[test]
    fn big_text_is_sdft() {
        let control = control();
        let paint = Paint::default();
        assert!(control.is_sdft(200.0, &paint, Matrix::i()));
        assert!(!control.is_direct(200.0, &paint, Matrix::i()));
        // Beyond the path threshold the glyphs are paths.
        assert!(!control.is_sdft(400.0, &paint, Matrix::i()));
        assert!(!control.is_direct(400.0, &paint, Matrix::i()));
    }

    #[test]
    fn hairline_stroke_is_never_sdft() {
        let control = control();
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(0.0);
        assert!(!control.is_sdft(200.0, &paint, Matrix::i()));
        paint.set_stroke_width(2.0);
        assert!(control.is_sdft(200.0, &paint, Matrix::i()));
    }

    #[test]
    fn perspective_text_uses_sdft_at_any_size() {
        let control = control();
        let paint = Paint::default();
        let mut m = Matrix::i().clone();
        m.set_persp_x(0.001);
        assert!(control.is_sdft(5.0, &paint, &m));
        assert!(!control.is_direct(5.0, &paint, &m));
    }

    #[test]
    fn sdf_font_picks_the_mask_size_bucket() {
        let control = control();
        let mut font = Font::default();
        font.set_size(100.0);
        let (df_font, to_source, range) = control.get_sdf_font(&font, Matrix::i(), Point::new(0.0, 0.0));
        assert_eq!(df_font.size(), 162.0);
        assert_eq!(to_source, 100.0 / 162.0);
        assert!(range.matrix_in_range(Matrix::i()));
        assert!(!range.matrix_in_range(&Matrix::scale((4.0, 4.0))));
    }

    #[test]
    fn matrix_range_excludes_the_minimum() {
        let range = SdftMatrixRange::new(1.0, 2.0);
        assert!(!range.matrix_in_range(Matrix::i())); // maxScale == min is excluded
        assert!(range.matrix_in_range(&Matrix::scale((2.0, 2.0)))); // max is included
    }
}
