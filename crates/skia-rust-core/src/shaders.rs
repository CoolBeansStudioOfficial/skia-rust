// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkShader.h (namespace SkShaders), src/shaders/SkColorShader.cpp,
// src/shaders/SkEmptyShader.cpp

//! `SkShaders`: the shader factories, and the shader implementations of `src/shaders` that
//! core needs (the shader base, color and empty shaders).

pub mod color_filter_shader;
pub mod color_shader;
pub mod empty_shader;
pub mod shader_base;

use crate::alpha_type::AlphaType;
use crate::color::{Color, Color4f};
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::floating_point::is_finite_array;
use crate::shader::Shader;

pub use color_filter_shader::ColorFilterShader;
pub use color_shader::ColorShader;
pub use empty_shader::EmptyShader;
pub use shader_base::{
    ContextRec, ENABLE_LEGACY_SHADER_CONTEXT, MatrixRec, OPAQUE_ALPHA_FLAG, ShaderBase,
    ShaderContext, ShaderType,
};

/// A shader that draws nothing (`SkShaders::Empty`).
// Port of: src/shaders/SkEmptyShader.cpp#L20 (chrome/m156)
#[doc(alias = "Empty")]
#[must_use]
pub fn empty() -> Shader {
    Shader::from_base(EmptyShader)
}

/// A shader of a single sRGB color (`SkShaders::Color(SkColor)`).
///
/// # Panics
/// Never: an 8-bit color is finite, so [`color_in_space`] always makes a shader.
// Port of: src/shaders/SkColorShader.cpp#L80-L82 (chrome/m156)
#[doc(alias = "Color")]
#[must_use]
pub fn color(color: impl Into<Color>) -> Shader {
    // An 8-bit color is always finite.
    color_in_space(Color4f::from_color(color.into()), ColorSpace::new_srgb())
        .expect("a Color is finite")
}

/// A shader of a single color in `space` (sRGB if `None`), or `None` if the color is not finite
/// (`SkShaders::Color(const SkColor4f&, sk_sp<SkColorSpace>)`). The alpha is pinned to `[0, 1]`
/// and the color stored unpremultiplied in extended sRGB.
// Port of: src/shaders/SkColorShader.cpp#L84-L96 (chrome/m156)
#[doc(alias = "Color")]
#[must_use]
pub fn color_in_space(
    color: impl AsRef<Color4f>,
    space: impl Into<Option<ColorSpace>>,
) -> Option<Shader> {
    let color = *color.as_ref();
    if !is_finite_array(&color.as_array()) {
        return None;
    }
    let space = space.into();

    // Convert to sRGB to simplify what must be stored, remaining unpremul until the final dst
    // color space is known during actual shading. Also pin the alpha to [0,1].
    let mut srgb = color.pin_alpha().as_array();
    ColorSpaceXformSteps::new(
        space.as_ref(),
        AlphaType::Unpremul,
        Some(srgb_singleton()),
        AlphaType::Unpremul,
    )
    .apply(&mut srgb);

    Some(Shader::from_base(ColorShader::new(Color4f::new(
        srgb[0], srgb[1], srgb[2], srgb[3],
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena_alloc::ArenaAlloc;
    use crate::color::colors;
    use crate::color_type::ColorType;
    use crate::effect_priv::StageRec;
    use crate::matrix::Matrix;
    use crate::raster_pipeline::RasterPipeline;
    use crate::rect::Rect;
    use core::any::Any;

    fn color_of(s: &Shader) -> Color4f {
        let base: &dyn Any = s.as_base();
        base.downcast_ref::<ColorShader>().unwrap().color()
    }

    #[test]
    fn color_shader() {
        let s = color(Color::from_argb(0x80, 0xFF, 0x00, 0x40));
        assert_eq!(s.as_base().shader_type(), ShaderType::Color);
        assert!(!s.is_opaque());
        let c = Color4f::from_color(Color::from_argb(0x80, 0xFF, 0x00, 0x40));
        assert_eq!(color_of(&s), c);
        assert_eq!(s.as_base().is_constant(), Some(c));
        assert_eq!(
            s.as_base().as_luminance_color(),
            Some(Color4f { a: 1.0, ..c })
        );
        assert!(color(Color::BLACK).is_opaque());

        // Alpha is pinned; non-finite colors make no shader.
        let s = color_in_space(Color4f::new(0.5, 2.0, -1.0, 1.5), None).unwrap();
        assert_eq!(color_of(&s), Color4f::new(0.5, 2.0, -1.0, 1.0));
        assert!(color_in_space(Color4f::new(f32::NAN, 0.0, 0.0, 1.0), None).is_none());
        assert!(color_in_space(Color4f::new(0.0, f32::INFINITY, 0.0, 1.0), None).is_none());

        // Converted to sRGB from the given space.
        let linear = ColorSpace::new_srgb_linear();
        let s = color_in_space(Color4f::new(0.5, 0.5, 0.5, 1.0), linear).unwrap();
        let mut expected = [0.5, 0.5, 0.5, 1.0];
        ColorSpaceXformSteps::new(
            Some(&ColorSpace::new_srgb_linear()),
            AlphaType::Unpremul,
            Some(srgb_singleton()),
            AlphaType::Unpremul,
        )
        .apply(&mut expected);
        assert_eq!(color_of(&s).as_array(), expected);
        assert_ne!(expected[0], 0.5);
    }

    #[test]
    fn empty_shader() {
        let s = empty();
        assert_eq!(s.as_base().shader_type(), ShaderType::Empty);
        assert!(!s.is_opaque());
        assert_eq!(s.as_base().is_constant(), None);
        assert_eq!(s.as_base().as_luminance_color(), None);
        let alloc = ArenaAlloc::new();
        let mut p = RasterPipeline::new();
        let mut rec = StageRec {
            pipeline: &mut p,
            alloc: &alloc,
            dst_color_type: ColorType::RGBA8888,
            dst_cs: None,
            paint_color: colors::BLACK,
            dst_bounds: Rect::new_empty(),
        };
        assert!(!s.as_base().append_root_stages(&mut rec, Matrix::i()));
        assert!(p.empty());
        // Each factory call makes a new shader.
        assert_ne!(empty(), empty());
        let s2 = s.clone();
        assert_eq!(s, s2);
    }
}
