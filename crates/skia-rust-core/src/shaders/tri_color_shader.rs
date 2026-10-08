// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkTriColorShader.h, src/shaders/SkTriColorShader.cpp

//! `SkTriColorShader`: the shader `drawVertices` uses to interpolate per-vertex colors across
//! one triangle.
//!
//! skia-rust: Skia updates the shader's matrices between triangles without re-appending its
//! stages (the pipeline points at the shader's members). Pipeline contexts are immutable here,
//! so `Draw::draw_vertices` makes a new shader (and pipeline) per triangle; [`update`]
//! (`Self::update`) computes the same matrices.

use skia_rust_simd::vx::Float4;

use crate::color::PMColor4f;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::point::Point;
use crate::raster_pipeline::Stage;
use crate::scalar::scalar;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// `SkTriColorShader::Matrix43`: a 4x3 color matrix, column major.
// Port of: src/shaders/SkTriColorShader.h#L42-L68 (chrome/m156)
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Matrix43 {
    mat: [scalar; 12],
}

impl Matrix43 {
    // Port of: src/shaders/SkTriColorShader.h#L64-L66 (chrome/m156)
    fn dot(&self, index: usize, x: scalar, y: scalar) -> scalar {
        self.mat[index] * x + self.mat[index + 4] * y
    }

    /// `setConcat(a, b)`. (`a` is passed by value in Skia so that there is no aliasing with
    /// `this`; here it is a copy, too.)
    // Port of: src/shaders/SkTriColorShader.h#L45-L62 (chrome/m156)
    fn set_concat(&mut self, a: Matrix43, b: &Matrix) {
        debug_assert!(!b.has_perspective());

        self.mat[0] = a.dot(0, b.scale_x(), b.skew_y());
        self.mat[1] = a.dot(1, b.scale_x(), b.skew_y());
        self.mat[2] = a.dot(2, b.scale_x(), b.skew_y());
        self.mat[3] = a.dot(3, b.scale_x(), b.skew_y());

        self.mat[4] = a.dot(0, b.skew_x(), b.scale_y());
        self.mat[5] = a.dot(1, b.skew_x(), b.scale_y());
        self.mat[6] = a.dot(2, b.skew_x(), b.scale_y());
        self.mat[7] = a.dot(3, b.skew_x(), b.scale_y());

        self.mat[8] = a.dot(0, b.translate_x(), b.translate_y()) + a.mat[8];
        self.mat[9] = a.dot(1, b.translate_x(), b.translate_y()) + a.mat[9];
        self.mat[10] = a.dot(2, b.translate_x(), b.translate_y()) + a.mat[10];
        self.mat[11] = a.dot(3, b.translate_x(), b.translate_y()) + a.mat[11];
    }
}

/// A shader that interpolates the colors of one triangle's vertices (`SkTriColorShader`).
// Port of: src/shaders/SkTriColorShader.h#L19-L78 (chrome/m156)
#[doc(alias = "SkTriColorShader")]
#[derive(Clone, Debug)]
pub struct TriColorShader {
    /// If `use_persp`, we need both of these matrices, otherwise we can combine them, and only
    /// use `m43`.
    m43: Matrix43,
    m33: Matrix,
    is_opaque: bool,
    /// Controls our stages, and what we do in `update()`.
    use_persp: bool,
}

impl TriColorShader {
    /// `SkTriColorShader(isOpaque, usePersp)`. Call [`update`](Self::update) before use.
    // Port of: src/shaders/SkTriColorShader.h#L21 (chrome/m156)
    #[must_use]
    pub fn new(is_opaque: bool, use_persp: bool) -> TriColorShader {
        TriColorShader {
            m43: Matrix43::default(),
            m33: Matrix::new_identity(),
            is_opaque,
            use_persp,
        }
    }

    /// Computes the matrices for the triangle `index0`, `index1`, `index2` of `pts`, whose
    /// vertex colors are `colors`, drawn with the inverse of the CTM `ctm_inv`. False if the
    /// triangle is degenerate (`update`).
    // Port of: src/shaders/SkTriColorShader.cpp#L28-L61 (chrome/m156)
    pub fn update(
        &mut self,
        ctm_inv: &Matrix,
        pts: &[Point],
        colors: &[PMColor4f],
        index0: usize,
        index1: usize,
        index2: usize,
    ) -> bool {
        let mut m = Matrix::new_identity();
        m.set(0usize, pts[index1].x - pts[index0].x);
        m.set(1usize, pts[index2].x - pts[index0].x);
        m.set(2usize, pts[index0].x);
        m.set(3usize, pts[index1].y - pts[index0].y);
        m.set(4usize, pts[index2].y - pts[index0].y);
        m.set(5usize, pts[index0].y);
        let Some(im) = m.invert() else {
            return false;
        };

        self.m33 = Matrix::concat(&im, ctm_inv);

        let load = |c: &PMColor4f| Float4::load(&[c.r, c.g, c.b, c.a]);
        let c0 = load(&colors[index0]);
        let c1 = load(&colors[index1]);
        let c2 = load(&colors[index2]);

        (c1 - c0).store(&mut self.m43.mat[0..4]);
        (c2 - c0).store(&mut self.m43.mat[4..8]);
        c0.store(&mut self.m43.mat[8..12]);

        if !self.use_persp {
            let m43 = self.m43;
            self.m43.set_concat(m43, &self.m33);
        }
        true
    }
}

impl ShaderBase for TriColorShader {
    fn is_opaque(&self) -> bool {
        self.is_opaque
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::TriColor
    }

    // Port of: src/shaders/SkTriColorShader.cpp#L19-L26 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _m_rec: &MatrixRec) -> bool {
        rec.pipeline.append(Stage::SeedShader);
        if self.use_persp {
            let mut storage = [0.0; 9];
            self.m33.get_9(&mut storage);
            rec.pipeline
                .append(Stage::MatrixPerspective(rec.alloc.make(storage)));
        }
        rec.pipeline
            .append(Stage::Matrix4x3(rec.alloc.make(self.m43.mat)));
        true
    }
}
