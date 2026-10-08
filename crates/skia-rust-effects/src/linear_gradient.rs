// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/gradients/SkLinearGradient.{h,cpp}

//! `SkLinearGradient`: a gradient along a line.

use skia_rust_core::color::Color4f;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{Scalar, scalar_invert};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{GradientInfo, GradientType, MatrixRec, ShaderBase, ShaderType};

use crate::gradient::Gradient;
use crate::gradient_base_shader::{DEGENERATE_THRESHOLD, GradientBaseShader};

/// `pts_to_unit_matrix`.
// Port of: src/shaders/gradients/SkLinearGradient.cpp#L29-L38 (chrome/m156)
fn pts_to_unit_matrix(pts: &[Point; 2]) -> Matrix {
    let mut vec = pts[1] - pts[0];
    let mag = vec.length();
    #[allow(clippy::if_not_else)] // mirrors Skia's `mag ? SkScalarInvert(mag) : 0`
    let inv = if mag != 0.0 { scalar_invert(mag) } else { 0.0 };

    vec.scale(inv);
    let mut matrix = Matrix::new_identity();
    matrix.set_sin_cos((-vec.y, vec.x), Some(pts[0]));
    matrix.post_translate((-pts[0].x, -pts[0].y));
    matrix.post_scale((inv, inv), None);
    matrix
}

/// A gradient along a line (`SkLinearGradient`).
// Port of: src/shaders/gradients/SkLinearGradient.h#L22-L51 (chrome/m156)
#[doc(alias = "SkLinearGradient")]
#[derive(Clone, Debug)]
pub struct LinearGradient {
    base: GradientBaseShader,
    start: Point,
    end: Point,
}

impl LinearGradient {
    /// `SkLinearGradient(pts, desc)`.
    // Port of: src/shaders/gradients/SkLinearGradient.cpp#L42-L43 (chrome/m156)
    #[must_use]
    pub fn new(pts: &[Point; 2], desc: &Gradient<'_>) -> LinearGradient {
        LinearGradient {
            base: GradientBaseShader::new(desc, pts_to_unit_matrix(pts)),
            start: pts[0],
            end: pts[1],
        }
    }

    /// The start point (`start`).
    #[must_use]
    pub fn start(&self) -> Point {
        self.start
    }

    /// The end point (`end`).
    #[must_use]
    pub fn end(&self) -> Point {
        self.end
    }

    /// The state shared by all gradients.
    #[must_use]
    pub fn base(&self) -> &GradientBaseShader {
        &self.base
    }
}

impl ShaderBase for LinearGradient {
    fn is_opaque(&self) -> bool {
        self.base.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::GradientBase
    }

    fn on_as_luminance_color(&self) -> Option<Color4f> {
        Some(self.base.on_as_luminance_color())
    }

    // Port of: src/shaders/gradients/SkLinearGradient.cpp#L65-L68 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        // No extra stage needed for linear gradients.
        self.base.append_stages(rec, m_rec, |_alloc, _p, _post| {})
    }

    // Port of: src/shaders/gradients/SkLinearGradient.cpp#L70-L81 (chrome/m156)
    fn as_gradient(
        &self,
        info: Option<&mut GradientInfo<'_>>,
        local_matrix: Option<&mut Matrix>,
    ) -> GradientType {
        if let Some(info) = info {
            self.base.common_as_a_gradient(Some(&mut *info));
            info.point[0] = self.start;
            info.point[1] = self.end;
        }
        if let Some(local_matrix) = local_matrix {
            *local_matrix = Matrix::new_identity();
        }
        GradientType::Linear
    }
}

/// A shader that generates a linear gradient between the two points (`SkShaders::LinearGradient`).
// Port of: src/shaders/gradients/SkLinearGradient.cpp#L83-L106 (chrome/m156)
pub(crate) fn linear_gradient(
    pts: &[Point; 2],
    grad: &Gradient<'_>,
    lm: Option<&Matrix>,
) -> Option<Shader> {
    if !is_finite((pts[1] - pts[0]).length()) {
        return None;
    }

    if let Err(result) = grad.factory_early_exit(lm) {
        return result;
    }

    if (pts[1] - pts[0]).length().nearly_zero(DEGENERATE_THRESHOLD) {
        // Degenerate gradient, the only tricky complication is when in clamp mode, the limit of
        // the gradient approaches two half planes of solid color (first and last). However,
        // they are divided by the line perpendicular to the start and end point, which becomes
        // undefined once start and end are exactly the same, so just use the end color for a
        // stable solution.
        return GradientBaseShader::make_degenerate_gradient(grad.colors());
    }

    let s = Shader::from_base(LinearGradient::new(pts, grad));
    Some(s.with_local_matrix(lm.unwrap_or_else(|| Matrix::i())))
}
