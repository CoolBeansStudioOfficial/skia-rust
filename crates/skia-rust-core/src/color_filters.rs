// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkBlendModeColorFilter.cpp (the blend filter only)

//! `SkColorFilters`: the color filter factories ported so far. Only [`blend`] (the
//! `SkBlendModeColorFilter`) is here, because `SkDevice::clipShader` needs it to invert a clip
//! shader's alpha; the rest of the filters are Phase 3 and add themselves to this module.

use crate::alpha_type::AlphaType;
use crate::blend_mode::BlendMode;
use crate::blend_mode_priv;
use crate::color::{Color, Color4f};
use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::raster_pipeline::Stage;

/// A color filter that blends a constant color with each filtered color (`SkBlendModeColorFilter`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.h#L16-L45 (chrome/m156)
#[doc(alias = "SkBlendModeColorFilter")]
#[derive(Clone, Debug)]
pub struct BlendModeColorFilter {
    color: Color4f,
    mode: BlendMode,
}

impl ColorFilterBase for BlendModeColorFilter {
    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L71-L79 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
        rec.pipeline.append(Stage::MoveSrcDst);
        let mut color = self.color.as_array();
        ColorSpaceXformSteps::new(
            Some(srgb_singleton()),
            AlphaType::Unpremul,
            rec.dst_cs,
            AlphaType::Premul,
        )
        .apply(&mut color);
        rec.pipeline.append_constant_color(rec.alloc, &color);
        blend_mode_priv::append_stages(self.mode, rec.pipeline);
        true
    }

    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L35-L42 (chrome/m156)
    fn on_is_alpha_unchanged(&self) -> bool {
        // kDst: [Da, Dc]; kSrcATop: [Da, Sc * Da + (1 - Sa) * Dc]
        matches!(self.mode, BlendMode::Dst | BlendMode::SrcATop)
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::BlendMode
    }

    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L28-L33 (chrome/m156)
    fn on_as_a_color_mode(&self) -> Option<(Color, BlendMode)> {
        Some((self.color.to_color(), self.mode))
    }
}

/// A filter that blends `color` (in `color_space`, sRGB if `None`) with the filtered color using
/// `mode`, or `None` if the combination does nothing (`SkColorFilters::Blend(SkColor4f, ...)`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L83-L127 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend(
    color: impl AsRef<Color4f>,
    color_space: Option<&ColorSpace>,
    mut mode: BlendMode,
) -> Option<ColorFilter> {
    // First map to sRGB to simplify storage in the actual SkColorFilter instance, staying
    // unpremul until the final dst color space is known when actually filtering. Also pin the
    // alpha to [0,1]
    let mut srgb = color.as_ref().pin_alpha();
    let mut arr = srgb.as_array();
    ColorSpaceXformSteps::new(
        color_space,
        AlphaType::Unpremul,
        Some(srgb_singleton()),
        AlphaType::Unpremul,
    )
    .apply(&mut arr);
    srgb = Color4f::new(arr[0], arr[1], arr[2], arr[3]);

    // Next collapse some modes if possible
    let alpha = srgb.a;
    if BlendMode::Clear == mode {
        srgb = Color4f::new(0.0, 0.0, 0.0, 0.0);
        mode = BlendMode::Src;
    } else if BlendMode::SrcOver == mode {
        #[allow(clippy::float_cmp)] // Skia compares the alpha with 0 and 1 exactly
        if 0.0 == alpha {
            mode = BlendMode::Dst;
        } else if 1.0 == alpha {
            mode = BlendMode::Src;
        }
        // else just stay srcover
    }

    // Finally weed out combinations that are noops, and just return null
    #[allow(clippy::float_cmp)] // Skia compares the alpha with 0 and 1 exactly
    if BlendMode::Dst == mode
        || (0.0 == alpha
            && matches!(
                mode,
                BlendMode::SrcOver
                    | BlendMode::DstOver
                    | BlendMode::DstOut
                    | BlendMode::SrcATop
                    | BlendMode::Xor
                    | BlendMode::Darken
            ))
        || (1.0 == alpha && BlendMode::DstIn == mode)
    {
        return None;
    }

    Some(ColorFilter::from_base(BlendModeColorFilter {
        color: srgb,
        mode,
    }))
}

/// [`blend`] with an sRGB 8-bit color (`SkColorFilters::Blend(SkColor, SkBlendMode)`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L129-L131 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend_color(color: Color, mode: BlendMode) -> Option<ColorFilter> {
    blend(Color4f::from_color(color), None, mode)
}
