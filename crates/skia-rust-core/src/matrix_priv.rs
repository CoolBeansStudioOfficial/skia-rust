// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMatrixPriv.h, src/core/SkMatrix.cpp, src/core/SkM44.cpp

//! Private helpers of `SkMatrix` (`SkMatrixPriv.h`), used by Skia's own code and tests.
//!
//! # Not ported
//! * `SkMatrixPriv::GetMapPtsProc`, `MapPointsWithStride` and `MapHomogeneousPointsWithStride`:
//!   they walk raw point memory with a byte stride, which is not expressible without `unsafe`.
//!   Use [`Matrix::map_points`](crate::matrix::Matrix::map_points) and
//!   [`Matrix::map_homogeneous_points`](crate::matrix::Matrix::map_homogeneous_points).

use crate::floating_point::is_finite_all;
use crate::m44::M44;
use crate::matrix::{
    Matrix, Member, TypeMask, rect_from_float4, rect_to_float4, sk_determinant, to_quad_cw,
};
use crate::point::Point;
use crate::point3::Point3;
use crate::rect::Rect;
use crate::scalar::{
    SCALAR_INFINITY, SCALAR_NEARLY_ZERO, Scalar, double_to_scalar, scalar, scalar_abs,
};
use skia_rust_simd::vx::{Float2, Float4, shuffle};

/// `writeToMemory` / `readFromMemory` never use more than this many bytes
/// (`SkMatrixPriv::kMaxFlattenSize`).
// Port of: src/core/SkMatrixPriv.h#L23-L26 (chrome/m156)
#[doc(alias = "kMaxFlattenSize")]
pub const MAX_FLATTEN_SIZE: usize = 9 * size_of::<scalar>() + size_of::<u32>();

/// The distance of the plane `w = kW0PlaneDistance`, where perspective-mapped rectangles are
/// clipped (`SkPathPriv::kW0PlaneDistance`, defined in [`path_priv`](crate::path_priv)).
pub use crate::path_priv::W0_PLANE_DISTANCE;

/// Writes the nine members of `matrix` as native-endian floats into `buffer` (if any) and
/// returns the number of bytes needed (`SkMatrixPriv::WriteToMemory`).
///
/// # Panics
/// If `buffer` is shorter than the returned size.
// Port of: src/core/SkMatrix.cpp#L1504-L1511 (chrome/m156)
#[doc(alias = "WriteToMemory")]
pub fn write_to_memory(matrix: &Matrix, buffer: Option<&mut [u8]>) -> usize {
    // TODO write less for simple matrices
    let size_in_memory = 9 * size_of::<scalar>();
    if let Some(buffer) = buffer {
        let (chunks, _) = buffer[..size_in_memory].as_chunks_mut::<4>();
        for (chunk, value) in chunks.iter_mut().zip(matrix.members()) {
            *chunk = value.to_ne_bytes();
        }
    }
    size_in_memory
}

/// Reads the nine members of `matrix` from `buffer` and returns the number of bytes read, or 0
/// if `buffer` is too short (`SkMatrixPriv::ReadFromMemory`).
// Port of: src/core/SkMatrix.cpp#L1513-L1523 (chrome/m156)
#[doc(alias = "ReadFromMemory")]
pub fn read_from_memory(matrix: &mut Matrix, buffer: &[u8]) -> usize {
    let size_in_memory = 9 * size_of::<scalar>();
    if buffer.len() < size_in_memory {
        return 0;
    }
    let mut members = [0.0; 9];
    let (chunks, _) = buffer[..size_in_memory].as_chunks::<4>();
    for (value, chunk) in members.iter_mut().zip(chunks) {
        *value = scalar::from_ne_bytes(*chunk);
    }
    matrix.set_9(&members);
    // Figure out the type now so that we're thread-safe
    let _ = matrix.get_type();
    size_in_memory
}

/// Attempts to map `src` through the inverse of `mx`. If `mx` is not invertible, returns
/// `None` (`SkMatrixPriv::InverseMapRect`).
// Port of: src/core/SkMatrixPriv.h#L47-L87 (chrome/m156)
#[doc(alias = "InverseMapRect")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons
pub fn inverse_map_rect(mx: &Matrix, src: &Rect) -> Option<Rect> {
    if mx.is_scale_translate() {
        // A scale-translate matrix with a 0 scale factor is not invertible.
        if mx.scale_x() == 0.0 || mx.scale_y() == 0.0 {
            return None;
        }

        let tx = mx.translate_x();
        let ty = mx.translate_y();
        // mx maps coordinates as ((sx*x + tx), (sy*y + ty)) so the inverse is
        // ((x - tx)/sx), (y - ty)/sy). If sx or sy are negative, we have to swap the edge
        // values to maintain a sorted rect.
        let mut inverted = rect_to_float4(src);
        inverted -= Float4::new(tx, ty, tx, ty);

        if mx.get_type().bits() > TypeMask::TRANSLATE.bits() {
            let sx = 1.0 / mx.scale_x();
            let sy = 1.0 / mx.scale_y();
            inverted *= Float4::new(sx, sy, sx, sy);
            if sx < 0.0 && sy < 0.0 {
                inverted = shuffle(inverted, [2, 3, 0, 1]); // swap L|R and T|B
            } else if sx < 0.0 {
                inverted = shuffle(inverted, [2, 1, 0, 3]); // swap L|R
            } else if sy < 0.0 {
                inverted = shuffle(inverted, [0, 3, 2, 1]); // swap T|B
            }
        }
        return Some(rect_from_float4(inverted));
    }

    // general case
    mx.invert().map(|inverse| inverse.map_rect(src).0)
}

/// Divides the matrix's x and y members by `divx` and `divy`; returns false (leaving the
/// matrix unchanged) if either is zero (`SkMatrixPriv::PostIDiv`).
// Port of: src/core/SkMatrixPriv.h#L158-L160 (chrome/m156)
#[doc(alias = "PostIDiv")]
#[allow(clippy::similar_names)] // divx / divy follow the C++
pub fn post_i_div(matrix: &mut Matrix, divx: i32, divy: i32) -> bool {
    matrix.post_i_div(divx, divy)
}

/// Returns true if the nine members of `a` and `b` are bit-for-bit equal
/// (`SkMatrixPriv::CheapEqual`, a `memcmp`); unlike `==`, `-0.0 != 0.0` and `NaN == NaN`.
// Port of: src/core/SkMatrixPriv.h#L162-L164 (chrome/m156)
#[doc(alias = "CheapEqual")]
#[must_use]
pub fn cheap_equal(a: &Matrix, b: &Matrix) -> bool {
    std::ptr::eq(a, b)
        || a.members()
            .iter()
            .zip(b.members())
            .all(|(lhs, rhs)| lhs.to_bits() == rhs.to_bits())
}

/// Returns the members of `m` in column-major order (`SkMatrixPriv::M44ColMajor`).
// Port of: src/core/SkMatrixPriv.h#L166 (chrome/m156)
#[doc(alias = "M44ColMajor")]
#[must_use]
pub fn m44_col_major(m: &M44) -> [scalar; 16] {
    *m.members()
}

/// Returns true if the 3x3 portion of `m` is scale-translate. This is legacy functionality
/// that only checks the 3x3 portion; the matrix could have Z-based shear, or other complex
/// behavior (`SkMatrixPriv::IsScaleTranslateAsM33`).
// Port of: src/core/SkMatrixPriv.h#L168-L176 (chrome/m156)
#[doc(alias = "IsScaleTranslateAsM33")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons
pub fn is_scale_translate_as_m33(m: &M44) -> bool {
    m.rc(1, 0) == 0.0
        && m.rc(3, 0) == 0.0
        && m.rc(0, 1) == 0.0
        && m.rc(3, 1) == 0.0
        && m.rc(3, 3) == 1.0
}

// Port of: src/core/SkM44.cpp#L138-L162 (chrome/m156)
fn map_rect_affine(src: &Rect, mat: &[f32; 16]) -> Rect {
    // When multiplied against vectors of the form <x,y,x,y>, 'flip' allows a single min()
    // to compute both the min and "negated" max between the xy coordinates. Once finished, another
    // multiplication produces the original max.
    let flip = Float4::new(1.0, 1.0, -1.0, -1.0);

    // Since z = 0 and it's assumed ther's no perspective, only load the upper 2x2 and (tx,ty) in c3
    let c0 = Float2::new(mat[0], mat[1]).xyxy() * flip;
    let c1 = Float2::new(mat[4], mat[5]).xyxy() * flip;
    let c3 = Float2::new(mat[12], mat[13]).xyxy();

    // Compute the min and max of the four transformed corners pre-translation; then translate once
    // at the end.
    let min_max = c3
        + flip
            * (c0 * src.left + c1 * src.top)
                .min(c0 * src.right + c1 * src.top)
                .min((c0 * src.left + c1 * src.bottom).min(c0 * src.right + c1 * src.bottom));

    // minMax holds (min x, min y, max x, max y) so can be copied into an SkRect expecting l,t,r,b
    rect_from_float4(min_max)
}

// Port of: src/core/SkM44.cpp#L164-L215 (chrome/m156)
fn map_rect_perspective(src: &Rect, mat: &[f32; 16]) -> Rect {
    // Like map_rect_affine, z = 0 so we can skip the 3rd column, but we do need to compute w's
    // for each corner of the src rect.
    let c0 = Float4::load(&mat[0..4]);
    let c1 = Float4::load(&mat[4..8]);
    let c3 = Float4::load(&mat[12..16]);

    // Unlike map_rect_affine, we do not defer the 4th column since we may need to homogeneous
    // coordinates to clip against the w=0 plane
    let tl = c0 * src.left + c1 * src.top + c3;
    let tr = c0 * src.right + c1 * src.top + c3;
    let bl = c0 * src.left + c1 * src.bottom + c3;
    let br = c0 * src.right + c1 * src.bottom + c3;

    // After clipping to w>0 and projecting to 2d, 'project' employs the same negation trick to
    // compute min and max at the same time.
    let flip = Float4::new(1.0, 1.0, -1.0, -1.0);
    let project = |p0: Float4, p1: Float4, p2: Float4| -> Float4 {
        let w0 = p0[3];
        if w0 >= W0_PLANE_DISTANCE {
            // Unclipped, just divide by w
            flip * shuffle(p0, [0, 1, 0, 1]) / w0
        } else {
            let clip = |p: Float4| -> Float4 {
                let w = p[3];
                if w >= W0_PLANE_DISTANCE {
                    let t = (W0_PLANE_DISTANCE - w0) / (w - w0);
                    let c = (t * p.xy() + (1.0 - t) * p0.xy()) / W0_PLANE_DISTANCE;

                    flip * c.xyxy()
                } else {
                    Float4::splat(SCALAR_INFINITY)
                }
            };
            // Clip both edges leaving p0, and return the min/max of the two clipped points
            // (since clip returns infinity when both p0 and 2nd vertex have w<0, it'll
            // automatically be ignored).
            clip(p1).min(clip(p2))
        }
    };

    // Project all 4 corners, and pass in their adjacent vertices for clipping if it has w < 0,
    // then accumulate the min and max xy's.
    let min_max = flip
        * project(tl, tr, bl)
            .min(project(tr, br, tl))
            .min(project(br, bl, tr).min(project(bl, tl, br)));

    rect_from_float4(min_max)
}

/// Maps the four corners of `src` (with `z = 0` and `w = 1`) and returns the bounding box of
/// those points. If the matrix has perspective, the returned rectangle is the bounding box of
/// the projected points after being clipped to `w > 0` (`SkMatrixPriv::MapRect`).
// Port of: src/core/SkM44.cpp#L217-L225 (chrome/m156)
#[doc(alias = "MapRect")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons
pub fn map_rect(m: &M44, src: &Rect) -> Rect {
    let mat = m.members();
    let has_perspective = mat[3] != 0.0 || mat[7] != 0.0 || mat[11] != 0.0 || mat[15] != 1.0;
    if has_perspective {
        map_rect_perspective(src, mat)
    } else {
        map_rect_affine(src, mat)
    }
}

/// Returns the differential area scale factor for a local point `p` that will be transformed
/// by `m` (which may have perspective). If `m` does not have perspective, this scale factor is
/// constant regardless of `p`; when it does have perspective, it is specific to that point.
///
/// This can be crudely thought of as "device pixel area" / "local pixel area" at `p`.
///
/// Returns positive infinity if the transformed homogeneous point has `w <= 0`
/// (`SkMatrixPriv::DifferentialAreaScale`).
// Port of: src/core/SkMatrix.cpp#L1707-L1738 (chrome/m156)
#[doc(alias = "DifferentialAreaScale")]
#[must_use]
pub fn differential_area_scale(m: &Matrix, p: Point) -> scalar {
    //              [m00 m01 m02]                                 [f(u,v)]
    // Assuming M = [m10 m11 m12], define the projected p'(u,v) = [g(u,v)] where
    //              [m20 m12 m22]
    //
    //                                                        [x]     [u]
    // f(u,v) = x(u,v) / w(u,v), g(u,v) = y(u,v) / w(u,v) and [y] = M*[v]
    //                                                        [w]     [1]
    //
    // Then the differential scale factor between p = (u,v) and p' is |det J|,
    // where J is the Jacobian for p': [df/du dg/du]
    //                                 [df/dv dg/dv]
    // and df/du = (w*dx/du - x*dw/du)/w^2,   dg/du = (w*dy/du - y*dw/du)/w^2
    //     df/dv = (w*dx/dv - x*dw/dv)/w^2,   dg/dv = (w*dy/dv - y*dw/dv)/w^2
    //
    // From here, |det J| can be rewritten as |det J'/w^3|, where
    //      [x     y     w    ]   [x   y   w  ]
    // J' = [dx/du dy/du dw/du] = [m00 m10 m20]
    //      [dx/dv dy/dv dw/dv]   [m01 m11 m21]
    let xyw = m.map_point_to_homogeneous(p);

    if xyw.z < SCALAR_NEARLY_ZERO {
        // Reaching the discontinuity of xy/w and where the point would clip to w >= 0
        return SCALAR_INFINITY;
    }
    let jacobian = Matrix::new_all(
        xyw.x,
        xyw.y,
        xyw.z,
        m.scale_x(),
        m.skew_y(),
        m.persp_x(),
        m.skew_x(),
        m.scale_y(),
        m.persp_y(),
    );

    let mut denom = 1.0 / f64::from(xyw.z); // 1/w
    denom = denom * denom * denom; // 1/w^3
    scalar_abs(double_to_scalar(
        sk_determinant(jacobian.members(), true) * denom,
    ))
}

/// Determines if the transformation `m` applied to `bounds` can be approximated by an affine
/// transformation, i.e., the perspective part of the transformation has little visible effect
/// (`SkMatrixPriv::NearlyAffine`; use [`SCALAR_NEARLY_ZERO`] for the default `tolerance`).
// Port of: src/core/SkMatrix.cpp#L1740-L1793 (chrome/m156)
#[doc(alias = "NearlyAffine")]
#[must_use]
pub fn nearly_affine(m: &Matrix, bounds: &Rect, tolerance: scalar) -> bool {
    let mut tolerance = tolerance;
    if !m.has_perspective() {
        return true;
    }

    // The idea here is that we are computing the differential area scale at each corner,
    // and comparing them with some tolerance value. If they are similar, then we can say
    // that the transformation is nearly affine.

    // We can map the four points simultaneously.
    let mut xyw = [Point3::default(); 4];
    m.map_points_to_homogeneous(&mut xyw, &to_quad_cw(bounds));

    // Since the Jacobian is a 3x3 matrix, the determinant is a scalar triple product,
    // and the initial cross product is constant across all four points.
    let v1 = Point3::new(m.scale_x(), m.skew_y(), m.persp_x());
    let v2 = Point3::new(m.skew_x(), m.scale_y(), m.persp_y());
    let det_cross_prod = v1.cross(v2);

    // Start with the calculations at P0.
    if xyw[0].z < SCALAR_NEARLY_ZERO {
        // Reaching the discontinuity of xy/w and where the point would clip to w >= 0
        return false;
    }

    // Performing a dot product with the pre-w divide transformed point completes
    // the scalar triple product and the determinant calculation.
    let mut det = f64::from(det_cross_prod.dot(xyw[0]));
    // From that we can compute the differential area scale at P0.
    let mut denom = 1.0 / f64::from(xyw[0].z); // 1/w
    denom = denom * denom * denom; // 1/w^3
    let a0 = scalar_abs(double_to_scalar(det * denom));

    // Now we compare P0's scale with that at the other three points
    tolerance *= tolerance; // squared tolerance since we're comparing area
    for p in &xyw[1..] {
        if p.z < SCALAR_NEARLY_ZERO {
            // Reaching the discontinuity of xy/w and where the point would clip to w >= 0
            return false;
        }

        det = f64::from(det_cross_prod.dot(*p)); // completing scalar triple product
        denom = 1.0 / f64::from(p.z); // 1/w
        denom = denom * denom * denom; // 1/w^3
        let a = scalar_abs(double_to_scalar(det * denom));
        if !scalar::nearly_equal(a0, a, tolerance) {
            return false;
        }
    }

    true
}

/// Returns the scale to use when stroking with `matrix`: the larger of the lengths of its
/// first two columns, or 1 if that is not finite and positive
/// (`SkMatrixPriv::ComputeResScaleForStroking`).
// Port of: src/core/SkMatrix.cpp#L1795-L1806 (chrome/m156)
#[doc(alias = "ComputeResScaleForStroking")]
#[must_use]
pub fn compute_res_scale_for_stroking(matrix: &Matrix) -> scalar {
    // Not sure how to handle perspective differently, so we just don't try (yet)
    let sx = Point::length_xy(matrix[Member::ScaleX], matrix[Member::SkewY]);
    let sy = Point::length_xy(matrix[Member::SkewX], matrix[Member::ScaleY]);
    if is_finite_all(sx, &[sy]) {
        let scale = if sx > sy { sx } else { sy }; // std::max(sx, sy)
        if scale > 0.0 {
            return scale;
        }
    }
    1.0
}
