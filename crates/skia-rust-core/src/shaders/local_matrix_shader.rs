// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkLocalMatrixShader.{h,cpp} (`SkLocalMatrixShader`),
// src/shaders/SkShader.cpp (`makeWithLocalMatrix`, `isAImage`)

//! `SkLocalMatrixShader`: a shader that draws another shader with a matrix concatenated to its
//! local space.
//!
//! skia-rust: `asGradient` is not ported (gradients are a parallel task); flattening is not
//! ported (no `SkWriteBuffer`).

use crate::color::Color4f;
use crate::effect_priv::StageRec;
use crate::image::Image;
use crate::matrix::Matrix;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType, concat_local_matrices};
use crate::tile_mode::TileMode;

/// A shader drawn with a local matrix (`SkLocalMatrixShader`).
// Port of: src/shaders/SkLocalMatrixShader.h#L22-L70 (chrome/m156)
#[doc(alias = "SkLocalMatrixShader")]
#[derive(Clone, Debug)]
pub struct LocalMatrixShader {
    local_matrix: Matrix,
    wrapped_shader: Shader,
}

impl LocalMatrixShader {
    /// `SkLocalMatrixShader(wrapped, localMatrix)`.
    // Port of: src/shaders/SkLocalMatrixShader.h#L36-L37 (chrome/m156)
    #[must_use]
    pub fn new(wrapped: Shader, local_matrix: Matrix) -> LocalMatrixShader {
        LocalMatrixShader {
            local_matrix,
            wrapped_shader: wrapped,
        }
    }

    /// The local matrix (`localMatrix`).
    #[doc(alias = "localMatrix")]
    #[must_use]
    pub fn local_matrix(&self) -> &Matrix {
        &self.local_matrix
    }

    /// The wrapped shader (`wrappedShader`).
    #[doc(alias = "wrappedShader")]
    #[must_use]
    pub fn wrapped_shader(&self) -> &Shader {
        &self.wrapped_shader
    }
}

impl ShaderBase for LocalMatrixShader {
    // Port of: src/shaders/SkLocalMatrixShader.h#L39 (chrome/m156)
    fn is_opaque(&self) -> bool {
        self.wrapped_shader.as_base().is_opaque()
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L18-L20 (chrome/m156)
    fn is_constant(&self) -> Option<Color4f> {
        self.wrapped_shader.as_base().is_constant()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::LocalMatrix
    }

    // Port of: src/shaders/SkLocalMatrixShader.h#L44-L49 (chrome/m156)
    fn make_as_a_local_matrix_shader(&self) -> Option<(Shader, Matrix)> {
        Some((self.wrapped_shader.clone(), self.local_matrix.clone()))
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L58-L66 (chrome/m156)
    fn on_is_a_image(&self) -> Option<(Image, Matrix, (TileMode, TileMode))> {
        let (image, image_matrix, mode) = self.wrapped_shader.as_base().on_is_a_image()?;
        Some((
            image,
            concat_local_matrices(&self.local_matrix, &image_matrix),
            mode,
        ))
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L68-L70 (chrome/m156)
    fn on_as_luminance_color(&self) -> Option<Color4f> {
        self.wrapped_shader.as_base().as_luminance_color()
    }

    // Port of: src/shaders/SkLocalMatrixShader.cpp#L72-L75 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.wrapped_shader
            .as_base()
            .append_stages(rec, &m_rec.concat(&self.local_matrix))
    }
}

impl Shader {
    /// A shader that draws this shader with `local_matrix` concatenated to its local space
    /// (`makeWithLocalMatrix`).
    // Port of: src/shaders/SkShader.cpp#L26-L40 (chrome/m156)
    #[doc(alias = "makeWithLocalMatrix")]
    #[must_use]
    pub fn with_local_matrix(&self, local_matrix: &Matrix) -> Shader {
        let (base_shader, lm) = match self.as_base().make_as_a_local_matrix_shader() {
            Some((proxy, other_local_matrix)) => (
                proxy,
                concat_local_matrices(local_matrix, &other_local_matrix),
            ),
            None => (self.clone(), local_matrix.clone()),
        };

        Shader::from_base(LocalMatrixShader::new(base_shader, lm))
    }

    /// If the shader is an image shader (possibly with local matrices), its image, the local
    /// matrix to the image and its tile modes (`isAImage`).
    // Port of: src/shaders/SkShader.cpp#L22-L24 (chrome/m156)
    #[doc(alias = "isAImage")]
    #[must_use]
    pub fn is_a_image(&self) -> Option<(Image, Matrix, (TileMode, TileMode))> {
        self.as_base().on_is_a_image()
    }
}
