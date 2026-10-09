// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkWorkingColorSpaceShader.{h,cpp}

//! `SkWorkingColorSpaceShader`: runs a shader in a working color space. The child shader's colors
//! are converted from the destination into the input space, and its output from the output space
//! back into the destination. Made by [`Shader::with_working_color_space`](crate::shader::Shader).

use crate::alpha_type::AlphaType;
use crate::color::Color4f;
use crate::color_space::ColorSpace;
use crate::color_space_xform_color_filter::read_color_space;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::picture_priv::VERSION_WORKING_COLOR_SPACE_OUTPUT;
use crate::read_buffer::ReadBuffer;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};
use crate::write_buffer::BinaryWriteBuffer;

/// A shader that is evaluated in a working color space (`SkWorkingColorSpaceShader`).
///
/// `input_space` is the space the child shader works in (`None` means the destination space),
/// `output_space` the space its output is in (`None` means the input space). With
/// `work_in_unpremul` the child works with unpremultiplied colors.
// Port of: src/shaders/SkWorkingColorSpaceShader.h#L15-L60 (chrome/m156)
#[doc(alias = "SkWorkingColorSpaceShader")]
#[derive(Clone, Debug)]
pub struct WorkingColorSpaceShader {
    shader: Shader,
    input_space: Option<ColorSpace>,
    output_space: Option<ColorSpace>,
    work_in_unpremul: bool,
}

impl WorkingColorSpaceShader {
    /// `SkWorkingColorSpaceShader::Make`: `shader` in the given spaces. With no conversion to
    /// make, `shader` itself.
    // Port of: src/shaders/SkWorkingColorSpaceShader.cpp#L19-L40 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        shader: Shader,
        input_cs: Option<ColorSpace>,
        output_cs: Option<ColorSpace>,
        work_in_unpremul: bool,
    ) -> Shader {
        // A null input is the final dst CS, and a null output is the input CS, so if both are
        // null then there's no additional conversion for children and no additional conversion
        // applied to the shader's output.
        if input_cs.is_none() && output_cs.is_none() && !work_in_unpremul {
            return shader;
        }
        Shader::from_base(WorkingColorSpaceShader {
            shader,
            input_space: input_cs,
            output_space: output_cs,
            work_in_unpremul,
        })
    }

    /// The child shader (`shader`).
    #[must_use]
    pub fn shader(&self) -> &Shader {
        &self.shader
    }

    /// The input and output spaces and the working alpha type for a destination `dst_cs` with
    /// alpha type `dst_at` (`workingSpace`). The input space is the destination space when none
    /// was given; the output space is the input space when none was given.
    // Port of: src/shaders/SkWorkingColorSpaceShader.h#L30-L36 (chrome/m156)
    #[doc(alias = "workingSpace")]
    #[must_use]
    pub fn working_space(
        &self,
        dst_cs: &ColorSpace,
        dst_at: AlphaType,
    ) -> (ColorSpace, ColorSpace, AlphaType) {
        let input_space = self.input_space.clone().unwrap_or_else(|| dst_cs.clone());
        let output_space = self
            .output_space
            .clone()
            .unwrap_or_else(|| input_space.clone());
        let working_at = if self.work_in_unpremul {
            AlphaType::Unpremul
        } else {
            dst_at
        };
        (input_space, output_space, working_at)
    }
}

impl ShaderBase for WorkingColorSpaceShader {
    // Port of: src/shaders/SkWorkingColorSpaceShader.cpp (chrome/m156), SK_REGISTER_FLATTENABLE
    fn type_name(&self) -> &'static str {
        "SkWorkingColorSpaceShader"
    }

    // Port of: src/shaders/SkWorkingColorSpaceShader.h#L24 (chrome/m156)
    fn is_opaque(&self) -> bool {
        self.shader.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::WorkingColorSpace
    }

    // Port of: src/shaders/SkWorkingColorSpaceShader.cpp#L57-L86 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        let dst_cs = rec.dst_cs.cloned().unwrap_or_else(ColorSpace::new_srgb);
        // TODO(b/431253455): Should get the dstAT from `rec`
        let dst_at = AlphaType::Premul;
        let (input_cs, output_cs, working_at) = self.working_space(&dst_cs, dst_at);

        let dst_to_input =
            ColorSpaceXformSteps::new(Some(&dst_cs), dst_at, Some(&input_cs), working_at);
        let output_to_dst =
            ColorSpaceXformSteps::new(Some(&output_cs), working_at, Some(&dst_cs), dst_at);
        // NOTE: There is no inputToOutput steps to apply because it is assumed that the child
        // shader is responsible for such conversion (or input == output and it's a no-op).

        // Alpha-only image shaders reference the paint color, which is already in the destination
        // color space. We need to transform it to the working space for consistency.
        let mut paint = rec.paint_color.to_opaque().as_array();
        dst_to_input.apply(&mut paint);

        // The working rec's destination space is the raw input space, which is `None` when no
        // input space was given (as `fInputSpace.get()` in Skia).
        let mut working_rec = StageRec {
            pipeline: &mut *rec.pipeline,
            alloc: rec.alloc,
            dst_color_type: rec.dst_color_type,
            dst_cs: self.input_space.as_ref(),
            paint_color: Color4f::new(paint[0], paint[1], paint[2], paint[3]),
            surface_props: rec.surface_props,
            dst_bounds: rec.dst_bounds,
        };
        if !self.shader.as_base().append_stages(&mut working_rec, m_rec) {
            return false;
        }

        output_to_dst.apply_to_pipeline(rec.pipeline, rec.alloc);
        true
    }

    // Port of: src/shaders/SkWorkingColorSpaceShader.cpp#L88-L97 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_shader(Some(&self.shader));
        buffer.write_bool(self.work_in_unpremul);

        buffer.write_bool(self.input_space.is_some());
        if let Some(input_space) = &self.input_space {
            buffer.write_byte_array(&input_space.serialize());
        }

        buffer.write_bool(self.output_space.is_some());
        if let Some(output_space) = &self.output_space {
            buffer.write_byte_array(&output_space.serialize());
        }
    }
}

/// `SkWorkingColorSpaceShader::CreateProc`: the child shader, whether it works unpremultiplied,
/// then the optional input and output spaces. Version-older pictures always have an input space
/// and no output space, and no unpremultiplied flag.
// Port of: src/shaders/SkWorkingColorSpaceShader.cpp#L99-L136 (chrome/m156)
#[doc(alias = "CreateProc")]
#[must_use]
pub fn create_proc(buffer: &mut ReadBuffer<'_>, registry: &FlattenableRegistry) -> Option<Shader> {
    let shader = buffer.read_shader(registry)?;

    // If true, will not work in unpremul and assume inputSpace will be non-null and the
    // outputSpace will be null.
    let legacy_working_cs = buffer.is_version_lt(VERSION_WORKING_COLOR_SPACE_OUTPUT);

    let work_in_unpremul = !legacy_working_cs && buffer.read_bool();

    // The input/output spaces are allowed to be null, but if we think we have a non-null CS, then
    // it better be deserializable.
    let input_space = if legacy_working_cs || buffer.read_bool() {
        Some(read_color_space(buffer)?)
    } else {
        None
    };

    let output_space = if !legacy_working_cs && buffer.read_bool() {
        Some(read_color_space(buffer)?)
    } else {
        None
    };

    Some(WorkingColorSpaceShader::make(
        shader,
        input_space,
        output_space,
        work_in_unpremul,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaders;

    #[test]
    fn no_conversion_is_the_shader() {
        let s = shaders::color(crate::color::Color::RED);
        assert_eq!(
            WorkingColorSpaceShader::make(s.clone(), None, None, false),
            s
        );
    }

    #[test]
    fn working_space_defaults() {
        let s = shaders::color(crate::color::Color::RED);
        let dst = ColorSpace::new_srgb();

        let wrapped = WorkingColorSpaceShader::make(s.clone(), None, None, true);
        let base: &dyn core::any::Any = wrapped.as_base();
        let base = base.downcast_ref::<WorkingColorSpaceShader>().unwrap();
        let (input, output, at) = base.working_space(&dst, AlphaType::Premul);
        assert!(input.ptr_eq(&dst));
        assert!(output.ptr_eq(&dst));
        assert_eq!(at, AlphaType::Unpremul);
        assert!(base.shader().ptr_eq(&s));
    }
}
