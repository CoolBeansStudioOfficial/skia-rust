// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkGaussianColorFilter.{h,cpp}

//! `SkGaussianColorFilter`: maps a color's alpha to a grayscale Gaussian-shaped value. Shadows
//! use it to turn the vertex alpha of a tessellated shadow mesh into a blurred falloff (made by
//! [`make_gaussian`], `SkColorFilterPriv::MakeGaussian`).

use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::effect_priv::StageRec;
use crate::raster_pipeline::Stage;

/// The Gaussian alpha-to-grayscale color filter (`SkGaussianColorFilter`).
// Port of: src/effects/colorfilters/SkGaussianColorFilter.h#L15-L26 (chrome/m156)
#[doc(alias = "SkGaussianColorFilter")]
#[derive(Clone, Copy, Debug, Default)]
pub struct GaussianColorFilter;

impl ColorFilterBase for GaussianColorFilter {
    // Port of: src/effects/colorfilters/SkGaussianColorFilter.cpp#L18-L22 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
        rec.pipeline.append(Stage::GaussAToRgba);
        true
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Gaussian
    }
}

/// `SkColorFilterPriv::MakeGaussian`: the Gaussian color filter used by shadows.
// Port of: src/effects/colorfilters/SkGaussianColorFilter.cpp#L31-L33 (chrome/m156)
#[doc(alias = "MakeGaussian")]
#[must_use]
pub fn make_gaussian() -> ColorFilter {
    ColorFilter::from_base(GaussianColorFilter)
}
