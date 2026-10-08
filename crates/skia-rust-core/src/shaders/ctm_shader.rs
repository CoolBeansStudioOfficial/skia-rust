// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkLocalMatrixShader.h, src/shaders/SkLocalMatrixShader.cpp
// (`SkCTMShader`), src/shaders/SkShaderBase.cpp (`makeWithCTM`, `makeInvertAlpha`)

//! `SkCTMShader`: a shader that ignores the CTM it is drawn with and uses a fixed one instead.
//! `SkDevice::clipShader` wraps a clip shader in one so that it keeps the matrix it was clipped
//! with.

use crate::blend_mode::BlendMode;
use crate::color::Color;
use crate::color_filters;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::shader::Shader;
use crate::shaders::color_filter_shader::ColorFilterShader;
use crate::shaders::shader_base::{GradientInfo, GradientType, MatrixRec, ShaderBase, ShaderType};

/// A shader drawn with a fixed CTM (`SkCTMShader`).
// Port of: src/shaders/SkLocalMatrixShader.h#L65-L97 (chrome/m156)
#[doc(alias = "SkCTMShader")]
#[derive(Clone, Debug)]
pub struct CtmShader {
    proxy_shader: Shader,
    ctm: Matrix,
}

impl CtmShader {
    /// `SkCTMShader(proxy, ctm)`.
    // Port of: src/shaders/SkLocalMatrixShader.cpp#L72-L73 (chrome/m156)
    #[must_use]
    pub fn new(proxy_shader: Shader, ctm: Matrix) -> CtmShader {
        CtmShader { proxy_shader, ctm }
    }
}

impl ShaderBase for CtmShader {
    // `SkShader::isOpaque` is not overridden: false.
    fn is_opaque(&self) -> bool {
        false
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L75-L77 (chrome/m156)
    fn is_constant(&self) -> Option<crate::color::Color4f> {
        self.proxy_shader.as_base().is_constant()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::CTM
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L79-L82 (chrome/m156)
    fn as_gradient(
        &self,
        info: Option<&mut GradientInfo<'_>>,
        local_matrix: Option<&mut Matrix>,
    ) -> GradientType {
        self.proxy_shader.as_base().as_gradient(info, local_matrix)
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L84-L86 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _m_rec: &MatrixRec) -> bool {
        self.proxy_shader
            .as_base()
            .append_root_stages(rec, &self.ctm)
    }
}

impl Shader {
    /// A shader that draws this shader with `post_m` as its CTM, whatever it is drawn with
    /// (`SkShaderBase::makeWithCTM`).
    // Port of: src/shaders/SkShaderBase.cpp#L135-L137 (chrome/m156)
    #[doc(alias = "makeWithCTM")]
    #[must_use]
    pub fn make_with_ctm(&self, post_m: &Matrix) -> Shader {
        Shader::from_base(CtmShader::new(self.clone(), post_m.clone()))
    }

    /// A shader with `1 - alpha` of this one's alpha: a cheap way to invert the alpha channel
    /// (`SkShaderBase::makeInvertAlpha`).
    // Port of: src/shaders/SkShaderBase.cpp#L139-L142 (chrome/m156)
    #[doc(alias = "makeInvertAlpha")]
    #[must_use]
    pub fn make_invert_alpha(&self) -> Shader {
        // `makeWithColorFilter(filter)` is `SkColorFilterShader::Make(this, 1, filter)`; the
        // Blend(white, kSrcOut) filter is never null.
        ColorFilterShader::make(
            self.clone(),
            1.0,
            color_filters::blend_color(Color::WHITE, BlendMode::SrcOut),
        )
    }
}
