// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkShaderBase.{h,cpp}

//! `SkShaderBase`: the virtual interface of shaders ([`ShaderBase`]) and `SkShaders::MatrixRec`,
//! the matrix state passed down a shader tree while appending raster pipeline stages.

use core::any::Any;
use core::fmt;

use crate::color::Color4f;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::raster_pipeline::Stage;

/// Accumulates matrices, starting with the CTM, when building up a raster pipeline by walking
/// the shader tree (`SkShaders::MatrixRec`). It avoids adding a matrix multiply for each
/// individual matrix.
///
/// It also tracks the "total matrix": all the matrices encountered during traversal to the
/// current shader, including ones that have already been applied (the transformation from the
/// current shader's coordinate space to device space).
// Port of: src/shaders/SkShaderBase.h#L57-L158 (chrome/m156)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatrixRec {
    ctm: Matrix,
    /// Concatenation of all local matrices, including those already applied.
    total_local_matrix: Matrix,
    /// The accumulated local matrices from walking down the shader hierarchy that have NOT yet
    /// been incorporated into the raster pipeline.
    pending_local_matrix: Matrix,
    total_matrix_is_valid: bool,
    /// Tracks whether the CTM has already been applied (and in raster pipeline whether the
    /// device coords have been seeded.)
    ctm_applied: bool,
}

impl MatrixRec {
    /// The state at the root of a shader tree drawn with `ctm`.
    // Port of: src/shaders/SkShaderBase.cpp#L24 (chrome/m156)
    #[must_use]
    pub fn new(ctm: &Matrix) -> MatrixRec {
        MatrixRec {
            ctm: ctm.clone(),
            total_local_matrix: Matrix::new_identity(),
            pending_local_matrix: Matrix::new_identity(),
            total_matrix_is_valid: true,
            ctm_applied: false,
        }
    }

    /// A new `MatrixRec` that represents the existing total and pending matrix pre-concat'ed
    /// with `m` (`concat`).
    // Port of: src/shaders/SkShaderBase.cpp#L67-L73 (chrome/m156)
    #[must_use]
    pub fn concat(&self, m: &Matrix) -> MatrixRec {
        MatrixRec {
            ctm: self.ctm.clone(),
            total_local_matrix: concat_local_matrices(&self.total_local_matrix, m),
            pending_local_matrix: concat_local_matrices(&self.pending_local_matrix, m),
            total_matrix_is_valid: self.total_matrix_is_valid,
            ctm_applied: self.ctm_applied,
        }
    }

    /// Appends a multiply by the inverse of the pending local matrix to the pipeline (seeding
    /// the device coordinates first if the CTM was not applied). `post_inv` is an additional
    /// matrix to post-apply to the inverted pending matrix (Skia's default is the identity). If
    /// the pending matrix is not invertible, returns `None` and leaves the pipeline unmodified
    /// (`apply`).
    // Port of: src/shaders/SkShaderBase.cpp#L26-L46 (chrome/m156)
    #[must_use]
    pub fn apply(&self, rec: &mut StageRec<'_, '_>, post_inv: &Matrix) -> Option<MatrixRec> {
        let mut total = self.pending_local_matrix.clone();
        if !self.ctm_applied {
            total = Matrix::concat(&self.ctm, &total);
        }
        let inv = total.invert()?;
        total = Matrix::concat(post_inv, &inv);
        if !self.ctm_applied {
            rec.pipeline.append(Stage::SeedShader);
        }
        // appendMatrix is a no-op if total worked out to identity.
        rec.pipeline.append_matrix(rec.alloc, &total);
        Some(MatrixRec {
            ctm: self.ctm.clone(),
            total_local_matrix: self.total_local_matrix.clone(),
            pending_local_matrix: Matrix::new_identity(),
            total_matrix_is_valid: self.total_matrix_is_valid,
            ctm_applied: true,
        })
    }

    /// The state after this `MatrixRec` has been applied, without applying it (`applied`). The
    /// CTM is marked "not applied", as for fragment processors (which never apply the CTM).
    // Port of: src/shaders/SkShaderBase.cpp#L57-L65 (chrome/m156)
    #[must_use]
    pub fn applied(&self) -> MatrixRec {
        // We mark the CTM as "not applied" because we *never* apply the CTM for FPs. Their starting
        // coords are local, not device, coords.
        MatrixRec {
            ctm: self.ctm.clone(),
            total_local_matrix: self.total_local_matrix.clone(),
            pending_local_matrix: Matrix::new_identity(),
            total_matrix_is_valid: self.total_matrix_is_valid,
            ctm_applied: false,
        }
    }

    /// Indicates that the mapping from shader to device space is not known
    /// (`markTotalMatrixInvalid`).
    #[doc(alias = "markTotalMatrixInvalid")]
    pub fn mark_total_matrix_invalid(&mut self) {
        self.total_matrix_is_valid = false;
    }

    /// Marks the CTM as already applied; can avoid re-seeding the shader unnecessarily
    /// (`markCTMApplied`).
    #[doc(alias = "markCTMApplied")]
    pub fn mark_ctm_applied(&mut self) {
        self.ctm_applied = true;
    }

    /// Whether the total matrix represents the full transform between this shader's coordinate
    /// space and device space (`totalMatrixIsValid`).
    #[doc(alias = "totalMatrixIsValid")]
    #[must_use]
    pub fn total_matrix_is_valid(&self) -> bool {
        self.total_matrix_is_valid
    }

    /// The total transform from the current shader's space to device space (`totalMatrix`). It
    /// may not be valid: see [`total_matrix_is_valid`](Self::total_matrix_is_valid).
    // Port of: src/shaders/SkShaderBase.h#L117 (chrome/m156)
    #[doc(alias = "totalMatrix")]
    #[must_use]
    pub fn total_matrix(&self) -> Matrix {
        Matrix::concat(&self.ctm, &self.total_local_matrix)
    }

    /// The inverse of [`total_matrix`](Self::total_matrix), if invertible (`totalInverse`).
    #[doc(alias = "totalInverse")]
    #[must_use]
    pub fn total_inverse(&self) -> Option<Matrix> {
        self.total_matrix().invert()
    }

    /// Is there a transform that has not yet been applied by a parent shader
    /// (`hasPendingMatrix`)?
    // Port of: src/shaders/SkShaderBase.h#L125-L127 (chrome/m156)
    #[doc(alias = "hasPendingMatrix")]
    #[must_use]
    pub fn has_pending_matrix(&self) -> bool {
        (!self.ctm_applied && !self.ctm.is_identity()) || !self.pending_local_matrix.is_identity()
    }

    /// When generating a raster pipeline, have the device coordinates been seeded
    /// (`rasterPipelineCoordsAreSeeded`)?
    #[doc(alias = "rasterPipelineCoordsAreSeeded")]
    #[must_use]
    pub fn raster_pipeline_coords_are_seeded(&self) -> bool {
        self.ctm_applied
    }
}

/// Concatenates a parent and a child local matrix (`SkShaderBase::ConcatLocalMatrices`; Skia's
/// Android-framework order is not used).
// Port of: src/shaders/SkShaderBase.h#L383-L388 (chrome/m156)
#[doc(alias = "ConcatLocalMatrices")]
#[must_use]
pub fn concat_local_matrices(parent_lm: &Matrix, child_lm: &Matrix) -> Matrix {
    Matrix::concat(parent_lm, child_lm)
}

/// The kinds of shaders (`SkShaderBase::ShaderType`, from `SK_ALL_SHADERS`).
// Port of: src/shaders/SkShaderBase.h#L162-L177 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderType {
    /// `kBlend`.
    Blend,
    /// `kCTM`.
    CTM,
    /// `kColor`.
    Color,
    /// `kColorFilter`.
    ColorFilter,
    /// `kCoordClamp`.
    CoordClamp,
    /// `kEmpty`.
    Empty,
    /// `kGradientBase`.
    GradientBase,
    /// `kImage`.
    Image,
    /// `kLocalMatrix`.
    LocalMatrix,
    /// `kPerlinNoise`.
    PerlinNoise,
    /// `kPicture`.
    Picture,
    /// `kRuntime`.
    Runtime,
    /// `kTransform`.
    Transform,
    /// `kTriColor`.
    TriColor,
    /// `kWorkingColorSpace`.
    WorkingColorSpace,
}

/// The virtual interface of a shader (`SkShaderBase`, with `SkShader::isOpaque`).
///
/// Implementations are wrapped in a [`Shader`](crate::shader::Shader) with
/// [`Shader::from_base`](crate::shader::Shader::from_base). The non-virtual members of
/// `SkShaderBase` are inherent methods of `dyn ShaderBase`. Implementations are `Any`, so a
/// `&dyn ShaderBase` whose [`shader_type`](Self::shader_type) is known can be downcast to its
/// concrete type (Skia's `static_cast`s).
///
/// skia-rust: the legacy shader context (`makeContext`, `SK_ENABLE_LEGACY_SHADERCONTEXT`, not
/// defined in the oracle builds), `asGradient` (gradients are Phase 3), `onIsAImage`,
/// `asRuntimeEffect`, `makeAsALocalMatrixShader` (deprecated) and the flattening hooks are not
/// ported yet.
// Port of: src/shaders/SkShaderBase.h#L185-L411 (chrome/m156)
#[doc(alias = "SkShaderBase")]
pub trait ShaderBase: Any + fmt::Debug + Send + Sync {
    /// True if the shader is guaranteed to produce only opaque colors (`SkShader::isOpaque`).
    #[doc(alias = "isOpaque")]
    fn is_opaque(&self) -> bool;

    /// The single color the shader produces, if it is guaranteed to produce only one
    /// (`isConstant`).
    #[doc(alias = "isConstant")]
    fn is_constant(&self) -> Option<Color4f> {
        None
    }

    /// The kind of shader (`type`).
    #[doc(alias = "type")]
    fn shader_type(&self) -> ShaderType;

    /// Adds stages to implement this shader. To ensure that the correct input coords are
    /// present in `r, g`, [`MatrixRec::apply`] must be called (unless the shader doesn't require
    /// its input coords). False if the shader draws nothing (`appendStages`).
    #[doc(alias = "appendStages")]
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool;

    /// The color whose luminance represents the shader, if there is one
    /// (`onAsLuminanceColor`).
    #[doc(alias = "onAsLuminanceColor")]
    fn on_as_luminance_color(&self) -> Option<Color4f> {
        None
    }
}

impl dyn ShaderBase {
    /// If the shader can represent its "average" luminance in a single color, that color, made
    /// opaque (only the RGB components are used to compute luminance) (`asLuminanceColor`).
    // Port of: src/shaders/SkShaderBase.cpp#L85-L95 (chrome/m156)
    #[doc(alias = "asLuminanceColor")]
    #[must_use]
    pub fn as_luminance_color(&self) -> Option<Color4f> {
        let mut color = self.on_as_luminance_color()?;
        color.a = 1.0; // we only return opaque
        Some(color)
    }

    /// Appends the stages of a root shader drawn with `ctm`; if false, draw nothing (do not fall
    /// back to a shader context). Assumes the device coordinates have not been seeded
    /// (`appendRootStages`).
    // Port of: src/shaders/SkShaderBase.cpp#L131-L133 (chrome/m156)
    #[doc(alias = "appendRootStages")]
    #[must_use]
    pub fn append_root_stages(&self, rec: &mut StageRec<'_, '_>, ctm: &Matrix) -> bool {
        self.append_stages(rec, &MatrixRec::new(ctm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena_alloc::ArenaAlloc;
    use crate::color::colors;
    use crate::color_type::ColorType;
    use crate::raster_pipeline::RasterPipeline;
    use crate::rect::Rect;

    fn rec<'r, 'a>(p: &'r mut RasterPipeline<'a>, alloc: &'a ArenaAlloc) -> StageRec<'r, 'a> {
        StageRec {
            pipeline: p,
            alloc,
            dst_color_type: ColorType::RGBA8888,
            dst_cs: None,
            paint_color: colors::BLACK,
            dst_bounds: Rect::new_empty(),
        }
    }

    #[test]
    fn matrix_rec_apply() {
        let alloc = ArenaAlloc::new();
        let mut pipeline = RasterPipeline::new();

        // Identity CTM, nothing pending: seed only.
        let mrec = MatrixRec::default();
        assert!(!mrec.has_pending_matrix());
        let applied = mrec
            .apply(&mut rec(&mut pipeline, &alloc), Matrix::i())
            .unwrap();
        assert!(applied.raster_pipeline_coords_are_seeded());
        assert_eq!(
            pipeline.to_string(),
            "SkRasterPipeline, 1 stages\n\tseed_shader\n\n"
        );

        // Applying again does not re-seed; a pending translate becomes its inverse.
        pipeline.reset();
        let mrec = applied.concat(&Matrix::translate((2.0, 3.0)));
        assert!(mrec.has_pending_matrix());
        let applied = mrec
            .apply(&mut rec(&mut pipeline, &alloc), Matrix::i())
            .unwrap();
        assert!(!applied.has_pending_matrix());
        assert_eq!(pipeline.stages().len(), 1);
        let Stage::MatrixTranslate(t) = pipeline.stages()[0] else {
            panic!("expected matrix_translate");
        };
        assert_eq!(t, [-2.0, -3.0]);
        assert_eq!(applied.total_matrix(), Matrix::translate((2.0, 3.0)));

        // A singular pending matrix leaves the pipeline alone.
        pipeline.reset();
        let mrec = MatrixRec::new(&Matrix::scale((0.0, 1.0)));
        assert!(
            mrec.apply(&mut rec(&mut pipeline, &alloc), Matrix::i())
                .is_none()
        );
        assert!(pipeline.empty());
        assert!(mrec.total_inverse().is_none());

        // applied() clears the pending matrix and marks the CTM "not applied".
        let a = MatrixRec::new(&Matrix::scale((2.0, 2.0)))
            .concat(&Matrix::translate((1.0, 0.0)))
            .applied();
        assert!(a.has_pending_matrix()); // the (non-identity) CTM is pending again
        assert!(!a.raster_pipeline_coords_are_seeded());
        let mut b = a.clone();
        b.mark_ctm_applied();
        assert!(!b.has_pending_matrix());
        b.mark_total_matrix_invalid();
        assert!(!b.total_matrix_is_valid());
        assert!(a.total_matrix_is_valid());
    }
}
