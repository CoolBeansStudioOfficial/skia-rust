// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/gradients/SkSweepGradient.{h,cpp}

//! `SkSweepGradient`: a gradient around a center.

use skia_rust_core::color::Color4f;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::floating_point::is_finite_all;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::Stage;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{GradientInfo, GradientType, MatrixRec, ShaderBase, ShaderType};
use skia_rust_core::tile_mode::TileMode;

use crate::gradient::{Colors, Gradient};
use crate::gradient_base_shader::{DEGENERATE_THRESHOLD, GradientBaseShader};

/// A gradient around a center (`SkSweepGradient`).
// Port of: src/shaders/gradients/SkSweepGradient.h#L19-L50 (chrome/m156)
#[doc(alias = "SkSweepGradient")]
#[derive(Clone, Debug)]
pub struct SweepGradient {
    base: GradientBaseShader,
    center: Point,
    t_bias: scalar,
    t_scale: scalar,
}

impl SweepGradient {
    /// `SkSweepGradient(center, t0, t1, desc)`.
    // Port of: src/shaders/gradients/SkSweepGradient.cpp#L21-L31 (chrome/m156)
    #[must_use]
    pub fn new(center: Point, t0: scalar, t1: scalar, desc: &Gradient<'_>) -> SweepGradient {
        debug_assert!(t0 < t1);
        SweepGradient {
            base: GradientBaseShader::new(desc, Matrix::translate((-center.x, -center.y))),
            center,
            t_bias: -t0,
            t_scale: 1.0 / (t1 - t0),
        }
    }

    /// The center (`center()`).
    // Port of: src/shaders/gradients/SkSweepGradient.h#L26-L33 (chrome/m156), `center()`
    #[must_use]
    pub fn center(&self) -> Point {
        self.center
    }

    /// The bias added to `t` (`tBias()`).
    // Port of: src/shaders/gradients/SkSweepGradient.h#L26-L33 (chrome/m156), `tBias()`
    #[must_use]
    pub fn t_bias(&self) -> scalar {
        self.t_bias
    }

    /// The scale applied to `t` (`tScale()`).
    // Port of: src/shaders/gradients/SkSweepGradient.h#L26-L33 (chrome/m156), `tScale()`
    #[must_use]
    pub fn t_scale(&self) -> scalar {
        self.t_scale
    }

    /// The state shared by all gradients.
    #[must_use]
    pub fn base(&self) -> &GradientBaseShader {
        &self.base
    }
}

impl ShaderBase for SweepGradient {
    fn is_opaque(&self) -> bool {
        self.base.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::GradientBase
    }

    fn on_as_luminance_color(&self) -> Option<Color4f> {
        Some(self.base.on_as_luminance_color())
    }

    // Port of: src/shaders/gradients/SkSweepGradient.cpp#L80-L86 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.base.append_stages(rec, m_rec, |alloc, p, _post| {
            p.append(Stage::XyToUnitAngle);
            p.append_matrix(
                alloc,
                &Matrix::concat(
                    &Matrix::scale((self.t_scale, 1.0)),
                    &Matrix::translate((self.t_bias, 0.0)),
                ),
            );
        })
    }

    // Port of: src/shaders/gradients/SkSweepGradient.cpp#L33-L46 (chrome/m156)
    fn as_gradient(
        &self,
        info: Option<&mut GradientInfo<'_>>,
        local_matrix: Option<&mut Matrix>,
    ) -> GradientType {
        if let Some(info) = info {
            self.base.common_as_a_gradient(Some(&mut *info));
            info.point[0] = self.center;
            info.point[1].x = self.t_scale;
            info.point[1].y = self.t_bias;
        }
        if let Some(local_matrix) = local_matrix {
            *local_matrix = Matrix::new_identity();
        }
        GradientType::Sweep
    }
}

/// A shader that generates a sweep gradient (`SkShaders::SweepGradient`).
// Port of: src/shaders/gradients/SkSweepGradient.cpp#L88-L141 (chrome/m156)
pub(crate) fn sweep_gradient(
    center: Point,
    start_angle: scalar,
    end_angle: scalar,
    grad: &Gradient<'_>,
    lm: Option<&Matrix>,
) -> Option<Shader> {
    if !is_finite_all(start_angle, &[end_angle]) || start_angle > end_angle {
        return None;
    }

    if let Err(result) = grad.factory_early_exit(lm) {
        return result;
    }

    let colors = grad.colors();
    let interp = grad.interpolation();
    let mut mode = colors.tile_mode();

    if <scalar as Scalar>::nearly_equal(start_angle, end_angle, DEGENERATE_THRESHOLD) {
        // Degenerate gradient, which should follow default degenerate behavior unless it is
        // clamped and the angle is greater than 0.
        if mode == TileMode::Clamp && end_angle > DEGENERATE_THRESHOLD {
            // In this case, the first color is repeated from 0 to the angle, then a hardstop
            // switches to the last color (all other colors are compressed to the infinitely thin
            // interpolation region).
            const CLAMP_POS: [scalar; 3] = [0.0, 1.0, 1.0];
            let src_colors = colors.colors();
            let re_colors = [
                src_colors[0],
                src_colors[0],
                src_colors[src_colors.len() - 1],
            ];
            let new_colors = Colors::new(
                &re_colors,
                Some(&CLAMP_POS),
                colors.tile_mode(),
                colors.color_space().cloned(),
            );
            return sweep_gradient(
                center,
                0.0,
                end_angle,
                &Gradient::new(new_colors, *interp),
                lm,
            );
        }
        return GradientBaseShader::make_degenerate_gradient(colors);
    }

    if start_angle <= 0.0 && end_angle >= 360.0 {
        // If the t-range includes [0,1], then we can always use clamping (presumably faster).
        mode = TileMode::Clamp;
    }

    let new_grad = Gradient::new(
        Colors::new(
            colors.colors(),
            colors.positions(),
            mode,
            colors.color_space().cloned(),
        ),
        *interp,
    );

    let t0 = start_angle / 360.0;
    let t1 = end_angle / 360.0;

    let s = Shader::from_base(SweepGradient::new(center, t0, t1, &new_grad));
    Some(s.with_local_matrix(lm.unwrap_or_else(|| Matrix::i())))
}
