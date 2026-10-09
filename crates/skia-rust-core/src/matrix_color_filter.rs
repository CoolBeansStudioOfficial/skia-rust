// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkMatrixColorFilter.{h,cpp}

//! `SkMatrixColorFilter`: a 5x4 matrix applied to the unpremultiplied RGBA color, or to its
//! HSLA form. The factories are in [`color_filters`](crate::color_filters).

use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::color_filters::{Clamp, hsla_matrix, matrix_row_major};
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::floating_point::is_finite_array;
use crate::picture_priv::VERSION_UNCLAMPED_MATRIX_COLOR_FILTER;
use crate::raster_pipeline::Stage;
use crate::read_buffer::ReadBuffer;
use crate::scalar::Scalar;
use crate::write_buffer::BinaryWriteBuffer;

/// Whether the matrix works on RGBA or on HSLA (`SkMatrixColorFilter::Domain`).
// Port of: src/effects/colorfilters/SkMatrixColorFilter.h#L23-L23 (chrome/m156)
#[doc(alias = "SkMatrixColorFilter::Domain")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    /// `kRGBA`.
    Rgba,
    /// `kHSLA`.
    Hsla,
}

/// A color filter that multiplies the color by a 5x4 matrix (`SkMatrixColorFilter`).
// Port of: src/effects/colorfilters/SkMatrixColorFilter.h#L21-L45 (chrome/m156)
#[doc(alias = "SkMatrixColorFilter")]
#[derive(Clone, Debug)]
pub struct MatrixColorFilter {
    matrix: [f32; 20],
    alpha_is_unchanged: bool,
    domain: Domain,
    clamp: Clamp,
}

impl MatrixColorFilter {
    /// `SkMatrixColorFilter(array, domain, clamp)`.
    // Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L35-L38 (chrome/m156)
    #[must_use]
    pub fn new(array: &[f32; 20], domain: Domain, clamp: Clamp) -> Self {
        Self {
            matrix: *array,
            alpha_is_unchanged: is_alpha_unchanged(array),
            domain,
            clamp,
        }
    }

    /// The matrix's domain (`domain`).
    #[must_use]
    pub fn domain(&self) -> Domain {
        self.domain
    }

    /// The clamp option (`clamp`).
    #[must_use]
    pub fn clamp(&self) -> Clamp {
        self.clamp
    }

    /// The row-major coefficients (`matrix`).
    #[must_use]
    pub fn matrix(&self) -> &[f32; 20] {
        &self.matrix
    }
}

/// True if the matrix never changes alpha: `srcA` is `[0, 0, 0, 1, 0]` (nearly).
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L27-L33 (chrome/m156)
fn is_alpha_unchanged(matrix: &[f32; 20]) -> bool {
    let src_a = &matrix[15..];
    src_a[0].nearly_zero(None)
        && src_a[1].nearly_zero(None)
        && src_a[2].nearly_zero(None)
        && <f32 as Scalar>::nearly_equal(src_a[3], 1.0, None)
        && src_a[4].nearly_zero(None)
}

impl ColorFilterBase for MatrixColorFilter {
    // Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L130 (chrome/m156),
    // SkFlattenable::Register
    fn type_name(&self) -> &'static str {
        "SkColorFilter_Matrix"
    }

    // Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L40-L47 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_scalar_array(&self.matrix);
        // RGBA flag
        buffer.write_bool(self.domain == Domain::Rgba);
        buffer.write_bool(self.clamp == Clamp::Yes);
    }

    // Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L70-L98 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        let will_stay_opaque = shader_is_opaque && self.alpha_is_unchanged;
        let hsla = self.domain == Domain::Hsla;
        let clamp = self.clamp == Clamp::Yes;

        if !shader_is_opaque {
            rec.pipeline.append(Stage::Unpremul);
        }
        if hsla {
            rec.pipeline.append(Stage::RgbToHsl);
        }
        rec.pipeline
            .append(Stage::Matrix4x5(rec.alloc.make(self.matrix)));
        if hsla {
            rec.pipeline.append(Stage::HslToRgb);
        }
        if clamp {
            rec.pipeline.append(Stage::Clamp01);
        } else {
            // We still need to clamp alpha, regardless
            rec.pipeline.append(Stage::ClampA01);
        }
        if !will_stay_opaque {
            rec.pipeline.append(Stage::Premul);
        }
        true
    }

    // Port of: src/effects/colorfilters/SkMatrixColorFilter.h#L30-L30 (chrome/m156)
    // (the C++ `onIsAlphaUnchanged` above, which returns the cached flag)
    fn on_is_alpha_unchanged(&self) -> bool {
        self.alpha_is_unchanged
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Matrix
    }

    // Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L63-L68 (chrome/m156)
    // skia-rust: like Skia, this reports the matrix for the HSLA domain too; the HSLA matrix
    // is not the RGBA matrix a caller would expect, so this is kept for parity only.
    fn on_as_a_color_matrix(&self) -> Option<[f32; 20]> {
        Some(self.matrix)
    }
}

/// `SkMatrixColorFilter::CreateProc`: the matrix, its domain flag and (from
/// `kUnclampedMatrixColorFilter`) its clamp flag. The clamp is ignored for HSLA filters.
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L49-L61 (chrome/m156)
#[doc(alias = "CreateProc")]
#[must_use]
pub fn matrix_create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<ColorFilter> {
    let mut matrix = [0.0_f32; 20];
    if !buffer.read_scalar_array(&mut matrix) {
        return None;
    }
    let is_rgba = buffer.read_bool();
    // The clamp flag is not written before `kUnclampedMatrixColorFilter`, and is then Yes.
    let clamp = if buffer.is_version_lt(VERSION_UNCLAMPED_MATRIX_COLOR_FILTER) || buffer.read_bool()
    {
        Clamp::Yes
    } else {
        Clamp::No
    };
    // clamp option is ignored for HSL-domain filters
    if is_rgba {
        matrix_row_major(&matrix, clamp)
    } else {
        hsla_matrix(&matrix)
    }
}

/// `MakeMatrix`: the filter for `array` in `domain`, or `None` if any coefficient is not finite.
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L100-L109 (chrome/m156)
pub(crate) fn make_matrix(
    array: &[f32; 20],
    domain: Domain,
    clamp: Clamp,
) -> Option<MatrixColorFilter> {
    if !is_finite_array(array) {
        return None;
    }
    Some(MatrixColorFilter::new(array, domain, clamp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_unchanged_needs_the_alpha_row() {
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        assert!(is_alpha_unchanged(&identity));
        let mut changed = identity;
        changed[18] = 0.5;
        assert!(!is_alpha_unchanged(&changed));
    }
}
