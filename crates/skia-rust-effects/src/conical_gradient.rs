// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/gradients/SkConicalGradient.{h,cpp}

//! `SkConicalGradient`: a two-point conical gradient (see <https://skia.org/dev/design/conical>
//! for how the shader works).

use skia_rust_core::color::Color4f;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::Stage;
use skia_rust_core::raster_pipeline::contexts::Conical2PtCtx;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{GradientInfo, GradientType, MatrixRec, ShaderBase, ShaderType};
use skia_rust_core::tile_mode::TileMode;

use crate::gradient::{Colors, Gradient};
use crate::gradient_base_shader::{DEGENERATE_THRESHOLD, GradientBaseShader};
use crate::radial_gradient::radial_gradient;

/// What a two-point conical gradient is mapped to (`SkConicalGradient::Type`).
// Port of: src/shaders/gradients/SkConicalGradient.h#L55 (chrome/m156)
#[doc(alias = "SkConicalGradient::Type")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConicalType {
    /// `kRadial`.
    Radial,
    /// `kStrip`.
    Strip,
    /// `kFocal`.
    Focal,
}

/// See <https://skia.org/dev/design/conical> for what focal data means and how our shader works
/// (`SkConicalGradient::FocalData`).
// Port of: src/shaders/gradients/SkConicalGradient.h#L26-L53 (chrome/m156)
#[doc(alias = "SkConicalGradient::FocalData")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct FocalData {
    /// `fR1`: r1 after mapping focal point to (0, 0).
    pub r1: scalar,
    /// `fFocalX`: f.
    pub focal_x: scalar,
    /// `fIsSwapped`: whether we swapped r0, r1.
    pub is_swapped: bool,
}

impl FocalData {
    /// The input `r0`, `r1` are the radii when we map centers to {(0, 0), (1, 0)}. We'll post
    /// concat matrix with our transformation matrix that maps focal point to (0, 0). Returns
    /// true if the set succeeded (`set`).
    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L30-L67 (chrome/m156)
    pub fn set(&mut self, mut r0: scalar, mut r1: scalar, matrix: &mut Matrix) -> bool {
        self.is_swapped = false;
        self.focal_x = skia_rust_core::floating_point::ieee_float_divide(r0, r0 - r1);
        if (self.focal_x - 1.0).nearly_zero(None) {
            // swap r0, r1
            matrix.post_translate((-1.0, 0.0));
            matrix.post_scale((-1.0, 1.0), None);
            core::mem::swap(&mut r0, &mut r1);
            self.focal_x = 0.0; // because r0 is now 0
            self.is_swapped = true;
        }

        // Map {focal point, (1, 0)} to {(0, 0), (1, 0)}
        let from = [Point::new(self.focal_x, 0.0), Point::new(1.0, 0.0)];
        let to = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
        let Some(focal_matrix) = Matrix::poly_to_poly(&from, &to) else {
            return false;
        };
        matrix.post_concat(&focal_matrix);
        self.r1 = r1 / (1.0 - self.focal_x).abs(); // focalMatrix has a scale of 1/(1-f)

        // The following transformations are just to accelerate the shader computation by saving
        // some arithmatic operations.
        if self.is_focal_on_circle() {
            matrix.post_scale((0.5, 0.5), None);
        } else {
            // skia-rust: C++ `sqrt` of a float is `sqrtf` here.
            matrix.post_scale(
                (
                    self.r1 / (self.r1 * self.r1 - 1.0),
                    1.0 / (self.r1 * self.r1 - 1.0).abs().sqrt(),
                ),
                None,
            );
        }
        matrix.post_scale(
            ((1.0 - self.focal_x).abs(), (1.0 - self.focal_x).abs()),
            None,
        ); // scale |1 - f|
        true
    }

    /// Whether the focal point (0, 0) is on the end circle with center (1, 0) and radius r1. If
    /// this is true, it's as if an aircraft is flying at Mach 1 and all circles (soundwaves)
    /// will go through the focal point (aircraft). In our previous implementations, this was
    /// known as the edge case where the inside circle touches the outside circle (on the focal
    /// point). If we were to solve for t bruteforcely using a quadratic equation, this case
    /// implies that the quadratic equation degenerates to a linear equation
    /// (`isFocalOnCircle`).
    // Port of: src/shaders/gradients/SkConicalGradient.h#L41 (chrome/m156)
    #[doc(alias = "isFocalOnCircle")]
    #[must_use]
    pub fn is_focal_on_circle(&self) -> bool {
        (1.0 - self.r1).nearly_zero(None)
    }

    /// `isSwapped`.
    #[doc(alias = "isSwapped")]
    #[must_use]
    pub fn is_swapped(&self) -> bool {
        self.is_swapped
    }

    /// `isWellBehaved`.
    #[doc(alias = "isWellBehaved")]
    #[must_use]
    pub fn is_well_behaved(&self) -> bool {
        !self.is_focal_on_circle() && self.r1 > 1.0
    }

    /// `isNativelyFocal`.
    #[doc(alias = "isNativelyFocal")]
    #[must_use]
    pub fn is_natively_focal(&self) -> bool {
        self.focal_x.nearly_zero(None)
    }
}

/// A two-point conical gradient (`SkConicalGradient`).
// Port of: src/shaders/gradients/SkConicalGradient.h#L22-L104 (chrome/m156)
#[doc(alias = "SkConicalGradient")]
#[derive(Clone, Debug)]
pub struct ConicalGradient {
    base: GradientBaseShader,
    center1: Point,
    center2: Point,
    radius1: scalar,
    radius2: scalar,
    ty: ConicalType,
    focal_data: FocalData,
}

impl ConicalGradient {
    /// Maps the start center to (0, 0) and the end center to (1, 0) (`MapToUnitX`).
    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L69-L74 (chrome/m156)
    #[doc(alias = "MapToUnitX")]
    #[must_use]
    pub fn map_to_unit_x(start_center: Point, end_center: Point) -> Option<Matrix> {
        let centers = [start_center, end_center];
        let unitvec = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
        Matrix::poly_to_poly(&centers, &unitvec)
    }

    /// `SkConicalGradient::Create`.
    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L76-L121 (chrome/m156)
    #[must_use]
    pub fn create(
        c0: Point,
        r0: scalar,
        c1: Point,
        r1: scalar,
        desc: &Gradient<'_>,
        local_matrix: Option<&Matrix>,
    ) -> Option<Shader> {
        let mut gradient_matrix;
        let gradient_type;

        if (c0 - c1).length().nearly_zero(None) {
            if std_max(r0, r1).nearly_zero(None) || <scalar as Scalar>::nearly_equal(r0, r1, None) {
                // Degenerate case; avoid dividing by zero. Should have been caught by caller but
                // just in case, recheck here.
                return None;
            }
            // Concentric case: we can pretend we're radial (with a tiny twist).
            let scale = skia_rust_core::floating_point::ieee_float_divide(1.0, std_max(r0, r1));
            gradient_matrix = Matrix::translate((-c1.x, -c1.y));
            gradient_matrix.post_scale((scale, scale), None);

            gradient_type = ConicalType::Radial;
        } else {
            let mx = Self::map_to_unit_x(c0, c1)?;
            gradient_matrix = mx;

            gradient_type = if (r1 - r0).nearly_zero(None) {
                ConicalType::Strip
            } else {
                ConicalType::Focal
            };
        }

        let mut focal_data = FocalData::default();
        if gradient_type == ConicalType::Focal {
            let d_center = (c0 - c1).length();
            if !focal_data.set(r0 / d_center, r1 / d_center, &mut gradient_matrix) {
                return None;
            }
        }

        let s = Shader::from_base(ConicalGradient::new(
            c0,
            r0,
            c1,
            r1,
            desc,
            gradient_type,
            gradient_matrix,
            focal_data,
        ));
        Some(s.with_local_matrix(local_matrix.unwrap_or_else(|| Matrix::i())))
    }

    /// `SkConicalGradient(start, startRadius, end, endRadius, desc, type, gradientMatrix,
    /// data)`.
    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L123-L142 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    #[allow(clippy::float_cmp)] // Skia's debug assert compares exactly
    pub fn new(
        start: Point,
        start_radius: scalar,
        end: Point,
        end_radius: scalar,
        desc: &Gradient<'_>,
        ty: ConicalType,
        gradient_matrix: Matrix,
        data: FocalData,
    ) -> ConicalGradient {
        // this is degenerate, and should be caught by our caller
        debug_assert!(start != end || start_radius != end_radius);
        ConicalGradient {
            base: GradientBaseShader::new(desc, gradient_matrix),
            center1: start,
            center2: end,
            radius1: start_radius,
            radius2: end_radius,
            ty,
            focal_data: if ty == ConicalType::Focal {
                data
            } else {
                FocalData::default()
            },
        }
    }

    /// The distance between the centers (`getCenterX1`).
    #[doc(alias = "getCenterX1")]
    #[must_use]
    pub fn get_center_x1(&self) -> scalar {
        Point::distance(self.center1, self.center2)
    }

    /// `getStartRadius`.
    #[doc(alias = "getStartRadius")]
    #[must_use]
    pub fn get_start_radius(&self) -> scalar {
        self.radius1
    }

    /// `getDiffRadius`.
    #[doc(alias = "getDiffRadius")]
    #[must_use]
    pub fn get_diff_radius(&self) -> scalar {
        self.radius2 - self.radius1
    }

    /// `getStartCenter`.
    #[doc(alias = "getStartCenter")]
    #[must_use]
    pub fn get_start_center(&self) -> Point {
        self.center1
    }

    /// `getEndCenter`.
    #[doc(alias = "getEndCenter")]
    #[must_use]
    pub fn get_end_center(&self) -> Point {
        self.center2
    }

    /// `getEndRadius`.
    #[doc(alias = "getEndRadius")]
    #[must_use]
    pub fn get_end_radius(&self) -> scalar {
        self.radius2
    }

    /// `getType`.
    #[doc(alias = "getType")]
    #[must_use]
    pub fn get_type(&self) -> ConicalType {
        self.ty
    }

    /// `getFocalData`.
    #[doc(alias = "getFocalData")]
    #[must_use]
    pub fn get_focal_data(&self) -> &FocalData {
        &self.focal_data
    }

    /// The state shared by all gradients.
    #[must_use]
    pub fn base(&self) -> &GradientBaseShader {
        &self.base
    }
}

/// `std::max(a, b)`: `a < b ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

impl ShaderBase for ConicalGradient {
    // Because areas outside the cone are left untouched, we cannot treat the
    // shader as opaque even if the gradient itself is opaque.
    // TODO(junov): Compute whether the cone fills the plane crbug.com/222380
    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L144-L149 (chrome/m156)
    fn is_opaque(&self) -> bool {
        false
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::GradientBase
    }

    fn on_as_luminance_color(&self) -> Option<Color4f> {
        Some(self.base.on_as_luminance_color())
    }

    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L151-L167 (chrome/m156)
    fn as_gradient(
        &self,
        info: Option<&mut GradientInfo<'_>>,
        local_matrix: Option<&mut Matrix>,
    ) -> GradientType {
        if let Some(info) = info {
            self.base.common_as_a_gradient(Some(&mut *info));
            info.point[0] = self.center1;
            info.point[1] = self.center2;
            info.radius[0] = self.radius1;
            info.radius[1] = self.radius2;
        }
        if let Some(local_matrix) = local_matrix {
            *local_matrix = Matrix::new_identity();
        }
        GradientType::Conical
    }

    // Port of: src/shaders/gradients/SkConicalGradient.cpp#L196-L252 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.base
            .append_stages(rec, m_rec, |alloc, p, post_pipeline| {
                let d_radius = self.radius2 - self.radius1;

                if self.ty == ConicalType::Radial {
                    p.append(Stage::XyToRadius);

                    // Tiny twist: radial computes a t for [0, r2], but we want a t for [r1, r2].
                    let scale = std_max(self.radius1, self.radius2) / d_radius;
                    let bias = -self.radius1 / d_radius;

                    p.append_matrix(
                        alloc,
                        &Matrix::concat(
                            &Matrix::translate((bias, 0.0)),
                            &Matrix::scale((scale, 1.0)),
                        ),
                    );
                    return;
                }

                if self.ty == ConicalType::Strip {
                    let scaled_r0 = self.radius1 / self.get_center_x1();
                    let ctx = alloc.make(Conical2PtCtx {
                        p0: scaled_r0 * scaled_r0,
                        ..Conical2PtCtx::default()
                    });
                    p.append(Stage::XyTo2ptConicalStrip(ctx));
                    p.append(Stage::Mask2ptConicalNan(ctx));
                    post_pipeline.append(Stage::ApplyVectorMask(&ctx.mask));
                    return;
                }

                let ctx = alloc.make(Conical2PtCtx {
                    p0: 1.0 / self.focal_data.r1,
                    p1: self.focal_data.focal_x,
                    ..Conical2PtCtx::default()
                });

                if self.focal_data.is_focal_on_circle() {
                    p.append(Stage::XyTo2ptConicalFocalOnCircle);
                } else if self.focal_data.is_well_behaved() {
                    p.append(Stage::XyTo2ptConicalWellBehaved(ctx));
                } else if self.focal_data.is_swapped() || 1.0 - self.focal_data.focal_x < 0.0 {
                    p.append(Stage::XyTo2ptConicalSmaller(ctx));
                } else {
                    p.append(Stage::XyTo2ptConicalGreater(ctx));
                }

                if !self.focal_data.is_well_behaved() {
                    p.append(Stage::Mask2ptConicalDegenerates(ctx));
                }
                if 1.0 - self.focal_data.focal_x < 0.0 {
                    p.append(Stage::NegateX);
                }
                if !self.focal_data.is_natively_focal() {
                    p.append(Stage::Alter2ptConicalCompensateFocal(ctx));
                }
                if self.focal_data.is_swapped() {
                    p.append(Stage::Alter2ptConicalUnswap);
                }
                if !self.focal_data.is_well_behaved() {
                    post_pipeline.append(Stage::ApplyVectorMask(&ctx.mask));
                }
            })
    }
}

/// A shader that generates a two-point conical gradient (`SkShaders::TwoPointConicalGradient`).
// Port of: src/shaders/gradients/SkConicalGradient.cpp#L254-L308 (chrome/m156)
pub(crate) fn two_point_conical_gradient(
    start: Point,
    start_radius: scalar,
    end: Point,
    end_radius: scalar,
    grad: &Gradient<'_>,
    lm: Option<&Matrix>,
) -> Option<Shader> {
    if start_radius < 0.0 || end_radius < 0.0 {
        return None;
    }

    let colors = grad.colors();
    let interp = grad.interpolation();
    if !GradientBaseShader::valid_gradient(colors.colors(), colors.tile_mode(), interp) {
        return None;
    }
    if lm.is_some_and(|lm| lm.invert().is_none()) {
        return None;
    }

    if (start - end).length().nearly_zero(DEGENERATE_THRESHOLD) {
        // If the center positions are the same, then the gradient is the radial variant of a 2
        // pt conical gradient, an actual radial gradient (startRadius == 0), or it is fully
        // degenerate (startRadius == endRadius).
        if <scalar as Scalar>::nearly_equal(start_radius, end_radius, DEGENERATE_THRESHOLD) {
            // Degenerate case, where the interpolation region area approaches zero. The proper
            // behavior depends on the tile mode, which is consistent with the default degenerate
            // gradient behavior, except when mode = clamp and the radii > 0.
            if colors.tile_mode() == TileMode::Clamp && end_radius > DEGENERATE_THRESHOLD {
                // The interpolation region becomes an infinitely thin ring at the radius, so the
                // final gradient will be the first color repeated from p=0 to 1, and then a hard
                // stop switching to the last color at p=1.
                const CIRCLE_POS: [scalar; 3] = [0.0, 1.0, 1.0];
                let front = colors.colors()[0];
                let re_colors = [front, front, colors.colors()[colors.colors().len() - 1]];
                let new_colors = Colors::new(
                    &re_colors,
                    Some(&CIRCLE_POS),
                    colors.tile_mode(),
                    colors.color_space().cloned(),
                );
                return radial_gradient(start, end_radius, &Gradient::new(new_colors, *interp), lm);
            }
            return GradientBaseShader::make_degenerate_gradient(colors);
        } else if start_radius.nearly_zero(DEGENERATE_THRESHOLD) {
            // We can treat this gradient as radial, which is faster. If we got here, we know
            // that endRadius is not equal to 0, so this produces a meaningful gradient
            return radial_gradient(start, end_radius, grad, lm);
        }
        // Else it's the 2pt conical radial variant with no degenerate radii, so fall through to
        // the regular 2pt constructor.
    }

    let mut tmp2_colors = [Color4f::default(); 2];
    let mut c4 = colors.colors();
    let mut pos = colors.positions();
    if c4.len() == 1 {
        tmp2_colors[0] = c4[0];
        tmp2_colors[1] = c4[0];
        c4 = &tmp2_colors;
        pos = None;
    }
    let new_colors = Colors::new(c4, pos, colors.tile_mode(), colors.color_space().cloned());

    ConicalGradient::create(
        start,
        start_radius,
        end,
        end_radius,
        &Gradient::new(new_colors, *interp),
        lm,
    )
}
