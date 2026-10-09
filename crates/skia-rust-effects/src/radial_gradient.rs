// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/gradients/SkRadialGradient.{h,cpp}

//! `SkRadialGradient`: a gradient from a center out to a radius.

use skia_rust_core::color::Color4f;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::Stage;
use skia_rust_core::scalar::{Scalar, scalar, scalar_invert};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{GradientInfo, GradientType, MatrixRec, ShaderBase, ShaderType};

use crate::gradient::Gradient;
use crate::gradient_base_shader::{DEGENERATE_THRESHOLD, GradientBaseShader};

/// `rad_to_unit_matrix`.
// Port of: src/shaders/gradients/SkRadialGradient.cpp#L26-L33 (chrome/m156)
fn rad_to_unit_matrix(center: Point, radius: scalar) -> Matrix {
    let inv = scalar_invert(radius);

    let mut matrix = Matrix::new_identity();
    matrix.set_translate((-center.x, -center.y));
    matrix.post_scale((inv, inv), None);
    matrix
}

/// A gradient from a center out to a radius (`SkRadialGradient`).
// Port of: src/shaders/gradients/SkRadialGradient.h#L19-L46 (chrome/m156)
#[doc(alias = "SkRadialGradient")]
#[derive(Clone, Debug)]
pub struct RadialGradient {
    base: GradientBaseShader,
    center: Point,
    radius: scalar,
}

impl RadialGradient {
    /// `SkRadialGradient(center, radius, desc)`.
    // Port of: src/shaders/gradients/SkRadialGradient.cpp#L35-L39 (chrome/m156)
    #[must_use]
    pub fn new(center: Point, radius: scalar, desc: &Gradient<'_>) -> RadialGradient {
        RadialGradient {
            base: GradientBaseShader::new(desc, rad_to_unit_matrix(center, radius)),
            center,
            radius,
        }
    }

    /// The center (`center()`).
    // Port of: src/shaders/gradients/SkRadialGradient.h#L27-L30 (chrome/m156), `center()`
    #[must_use]
    pub fn center(&self) -> Point {
        self.center
    }

    /// The radius (`radius()`).
    // Port of: src/shaders/gradients/SkRadialGradient.h#L27-L30 (chrome/m156), `radius()`
    #[must_use]
    pub fn radius(&self) -> scalar {
        self.radius
    }

    /// The state shared by all gradients.
    #[must_use]
    pub fn base(&self) -> &GradientBaseShader {
        &self.base
    }
}

impl ShaderBase for RadialGradient {
    fn is_opaque(&self) -> bool {
        self.base.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::GradientBase
    }

    fn on_as_luminance_color(&self) -> Option<Color4f> {
        Some(self.base.on_as_luminance_color())
    }

    // Port of: src/shaders/gradients/SkRadialGradient.cpp#L72-L76 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.base
            .append_stages(rec, m_rec, |_alloc, p, _post| p.append(Stage::XyToRadius))
    }

    // Port of: src/shaders/gradients/SkRadialGradient.cpp#L41-L52 (chrome/m156)
    fn as_gradient(
        &self,
        info: Option<&mut GradientInfo<'_>>,
        local_matrix: Option<&mut Matrix>,
    ) -> GradientType {
        if let Some(info) = info {
            self.base.common_as_a_gradient(Some(&mut *info));
            info.point[0] = self.center;
            info.radius[0] = self.radius;
        }
        if let Some(local_matrix) = local_matrix {
            *local_matrix = Matrix::new_identity();
        }
        GradientType::Radial
    }
}

/// A shader that generates a radial gradient (`SkShaders::RadialGradient`).
// Port of: src/shaders/gradients/SkRadialGradient.cpp#L78-L96 (chrome/m156)
pub(crate) fn radial_gradient(
    center: Point,
    radius: scalar,
    grad: &Gradient<'_>,
    lm: Option<&Matrix>,
) -> Option<Shader> {
    if radius < 0.0 {
        return None;
    }

    if let Err(result) = grad.factory_early_exit(lm) {
        return result;
    }

    if radius.nearly_zero(DEGENERATE_THRESHOLD) {
        // Degenerate gradient optimization, and no special logic needed for clamped radial
        // gradient
        return GradientBaseShader::make_degenerate_gradient(grad.colors());
    }

    let s = Shader::from_base(RadialGradient::new(center, radius, grad));
    Some(s.with_local_matrix(lm.unwrap_or_else(|| Matrix::i())))
}
