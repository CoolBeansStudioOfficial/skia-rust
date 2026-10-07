// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkColorShader.{h,cpp}

//! `SkColorShader`: a shader of a single color. (m156 has one color shader for `SkColor` and
//! `SkColor4f` colors; the former `SkColor4Shader` was merged into it.)

use crate::alpha_type::AlphaType;
use crate::color::Color4f;
use crate::color_space_priv::srgb_singleton;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// A shader that represents a single color (`SkColorShader`). In general, this effect can be
/// accomplished by just using the color field on the paint, but if an actual shader object is
/// needed, this provides that feature. Like all shaders, at draw time the paint's alpha is
/// respected, and is applied to the specified color.
///
/// Create one with [`shaders::color`](crate::shaders::color) or
/// [`shaders::color_in_space`](crate::shaders::color_in_space).
// Port of: src/shaders/SkColorShader.h#L25-L59 (chrome/m156)
#[doc(alias = "SkColorShader")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ColorShader {
    /// The color is stored in extended sRGB, regardless of the original color space that was
    /// passed into `SkShaders::Color()`.
    color: Color4f,
}

impl ColorShader {
    /// A shader wrapping the given (unpremultiplied, extended sRGB) color.
    #[must_use]
    pub fn new(color: Color4f) -> ColorShader {
        ColorShader { color }
    }

    /// The color, unpremultiplied extended sRGB (`color`).
    #[must_use]
    pub fn color(&self) -> Color4f {
        self.color
    }
}

impl ShaderBase for ColorShader {
    fn is_opaque(&self) -> bool {
        self.color.is_opaque()
    }

    fn is_constant(&self) -> Option<Color4f> {
        Some(self.color)
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::Color
    }

    // Port of: src/shaders/SkColorShader.cpp#L64-L70 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _m_rec: &MatrixRec) -> bool {
        let mut color = self.color.as_array();
        ColorSpaceXformSteps::new(
            Some(srgb_singleton()),
            AlphaType::Unpremul,
            rec.dst_cs,
            AlphaType::Premul,
        )
        .apply(&mut color);
        rec.pipeline.append_constant_color(rec.alloc, &color);
        true
    }

    fn on_as_luminance_color(&self) -> Option<Color4f> {
        Some(self.color)
    }
}
