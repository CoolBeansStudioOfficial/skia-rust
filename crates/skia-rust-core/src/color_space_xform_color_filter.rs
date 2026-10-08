// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkColorSpaceXformColorFilter.{h,cpp}

//! `SkColorSpaceXformColorFilter`: converts the color from one color space to another. The
//! sRGB gamma filters (`LinearToSRGBGamma`, `SRGBToLinearGamma`) are instances of it, made by
//! [`color_filters`](crate::color_filters).

use crate::alpha_type::AlphaType;
use crate::color_filter::{ColorFilterBase, ColorFilterType};
use crate::color_space::ColorSpace;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::raster_pipeline::Stage;

/// Converts colors from `src` to `dst` (`SkColorSpaceXformColorFilter`).
// Port of: src/effects/colorfilters/SkColorSpaceXformColorFilter.h#L20-L49 (chrome/m156)
#[doc(alias = "SkColorSpaceXformColorFilter")]
#[derive(Clone, Debug)]
pub struct ColorSpaceXformColorFilter {
    src: ColorSpace,
    dst: ColorSpace,
    steps: ColorSpaceXformSteps,
}

impl ColorSpaceXformColorFilter {
    /// `SkColorSpaceXformColorFilter(src, dst)`.
    // Port of: src/effects/colorfilters/SkColorSpaceXformColorFilter.cpp#L28-L36 (chrome/m156)
    #[must_use]
    pub fn new(src: ColorSpace, dst: ColorSpace) -> Self {
        // We handle premul/unpremul separately, so here just always upm->upm.
        let steps = ColorSpaceXformSteps::new(
            Some(&src),
            AlphaType::Unpremul,
            Some(&dst),
            AlphaType::Unpremul,
        );
        Self { src, dst, steps }
    }

    /// The source color space.
    #[must_use]
    pub fn src(&self) -> &ColorSpace {
        &self.src
    }

    /// The destination color space.
    #[must_use]
    pub fn dst(&self) -> &ColorSpace {
        &self.dst
    }
}

impl ColorFilterBase for ColorSpaceXformColorFilter {
    // Port of: src/effects/colorfilters/SkColorSpaceXformColorFilter.cpp#L38-L49 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        if !shader_is_opaque {
            rec.pipeline.append(Stage::Unpremul);
        }
        self.steps.apply_to_pipeline(rec.pipeline, rec.alloc);
        if !shader_is_opaque {
            rec.pipeline.append(Stage::Premul);
        }
        true
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::ColorSpaceXform
    }
}
