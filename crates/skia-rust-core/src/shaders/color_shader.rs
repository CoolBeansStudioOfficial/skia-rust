// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkColorShader.{h,cpp}

//! `SkColorShader`: a shader of a single color. (m156 has one color shader for `SkColor` and
//! `SkColor4f` colors; the former `SkColor4Shader` was merged into it.)

use crate::alpha_type::AlphaType;
use crate::color::{Color, Color4f};
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::picture_priv::VERSION_COMBINE_COLOR_SHADERS;
use crate::read_buffer::ReadBuffer;
use crate::shader::Shader;
use crate::shaders::{
    self,
    shader_base::{MatrixRec, ShaderBase, ShaderType},
};
use crate::write_buffer::BinaryWriteBuffer;

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
    // Port of: src/shaders/SkColorShader.cpp#L74-L75 (chrome/m156), SK_REGISTER_FLATTENABLE
    fn type_name(&self) -> &'static str {
        "SkColorShader"
    }

    // Port of: src/shaders/SkColorShader.cpp#L60-L62 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_color4f(self.color);
    }

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

/// `SkColorShader::CreateProc`: a color shader from its color. Before `kCombineColorShaders` the
/// color is 8-bit sRGB.
// Port of: src/shaders/SkColorShader.cpp#L48-L58 (chrome/m156)
#[doc(alias = "CreateProc")]
#[must_use]
pub fn create_proc(buffer: &mut ReadBuffer<'_>, _registry: &FlattenableRegistry) -> Option<Shader> {
    if buffer.is_version_lt(VERSION_COMBINE_COLOR_SHADERS) {
        // Stored an 8-bit color only.
        return Some(shaders::color(Color::new(buffer.read32().cast_unsigned())));
    }
    // Stores a floating-point color in sRGB.
    let color = buffer.read_color4f();
    shaders::color_in_space(color, ColorSpace::new_srgb())
}

/// The legacy `SkColorShader4` flattenable (`legacy_color4_create_proc`): a color with its color
/// space, which only SKPs older than `kCombineColorShaders` refer to.
// Port of: src/shaders/SkColorShader.cpp#L30-L46 (chrome/m156)
#[doc(alias = "SkColorShader4")]
#[must_use]
pub fn legacy_color4_create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<Shader> {
    if !buffer.validate(buffer.is_version_lt(VERSION_COMBINE_COLOR_SHADERS)) {
        return None;
    }
    let color = buffer.read_color4f();
    let mut color_space = None;
    if buffer.read_bool() {
        color_space = buffer
            .read_byte_array_as_data()
            .and_then(|data| ColorSpace::deserialize(data.as_bytes()));
    }
    shaders::color_in_space(color, color_space)
}
