// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkTransformShader.h, src/shaders/SkTransformShader.cpp

//! `SkTransformShader`: applies a matrix transform to the shader coordinates, like a local
//! matrix shader, for `drawVertices` and `drawAtlas`.
//!
//! The difference with a typical local matrix shader is that this shader's matrix is not
//! combined with the inverse CTM or other local matrices in order to facilitate modifying the
//! matrix between uses of the raster pipeline. In `drawVertices` (when explicit texture coords
//! are used) and `drawAtlas` the mapping from each triangle or atlas quad to shader space is
//! different.
//!
//! skia-rust: Skia updates the matrix in place between uses of one pipeline. Pipeline contexts
//! are immutable here, so the callers make a new shader (and pipeline) per triangle or quad;
//! [`update`](TransformShader::update) sets the same matrix before the stages are appended.

use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::raster_pipeline::Stage;
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// A shader that applies a matrix to the coordinates its child shader sees
/// (`SkTransformShader`).
// Port of: src/shaders/SkTransformShader.h#L23-L52 (chrome/m156)
#[doc(alias = "SkTransformShader")]
#[derive(Clone, Debug)]
pub struct TransformShader {
    shader: Shader,
    /// The matrix, as `SkMatrix::get9` stores it (`fMatrixStorage`).
    matrix_storage: [scalar; 9],
    allow_perspective: bool,
}

impl TransformShader {
    /// `SkTransformShader(shader, allowPerspective)`, with an identity matrix.
    // Port of: src/shaders/SkTransformShader.cpp#L17-L20 (chrome/m156)
    #[must_use]
    pub fn new(shader: Shader, allow_perspective: bool) -> TransformShader {
        let mut matrix_storage = [0.0; 9];
        Matrix::new_identity().get_9(&mut matrix_storage);
        TransformShader {
            shader,
            matrix_storage,
            allow_perspective,
        }
    }

    /// Changes the matrix used by the generated pipeline; false (leaving the matrix) if it has
    /// perspective and the shader does not allow it (`update`).
    // Port of: src/shaders/SkTransformShader.cpp#L22-L29 (chrome/m156)
    pub fn update(&mut self, matrix: &Matrix) -> bool {
        if !self.allow_perspective && matrix.has_perspective() {
            return false;
        }

        matrix.get_9(&mut self.matrix_storage);
        true
    }
}

impl ShaderBase for TransformShader {
    fn is_opaque(&self) -> bool {
        self.shader.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::Transform
    }

    /// Adds a pipestage to multiply the incoming coords in `r` and `g` by the matrix. The child
    /// shader is called with no pending local matrix and the total transform as unknowable.
    // Port of: src/shaders/SkTransformShader.cpp#L31-L56 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        // We have to seed and apply any constant matrices before appending our matrix that may
        // mutate. We could try to add one matrix stage and then incorporate the parent matrix
        // with the variable matrix in each call to update(). However, in practice our callers
        // fold the CTM into the update() matrix and don't wrap the transform shader in local
        // matrix shaders so the call to apply below should just seed the coordinates. If this
        // assert fires it just indicates an optimization opportunity, not a correctness bug.
        debug_assert!(!m_rec.has_pending_matrix());
        let Some(mut child_m_rec) = m_rec.apply(rec, Matrix::i()) else {
            return false;
        };
        // The matrix we're about to insert gets updated between uses of the pipeline so our
        // children can't know the total transform when they add their stages. We don't even
        // incorporate this matrix into the MatrixRec at all.
        child_m_rec.mark_total_matrix_invalid();

        if self.allow_perspective {
            rec.pipeline.append(Stage::MatrixPerspective(
                rec.alloc.make(self.matrix_storage),
            ));
        } else {
            // matrix_2x3 reads the first six entries.
            let affine: [scalar; 6] = core::array::from_fn(|i| self.matrix_storage[i]);
            rec.pipeline
                .append(Stage::Matrix2x3(rec.alloc.make(affine)));
        }

        // (Skia ignores the child's result.)
        let _ = self.shader.as_base().append_stages(rec, &child_m_rec);
        true
    }
}
