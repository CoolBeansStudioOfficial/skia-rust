// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDrawShadowInfo.{h,cpp}

//! The shadow parameter record (`SkDrawShadowRec`) and the shadow metrics that map a light and
//! an occluder to blur radii, scales and offsets (`SkDrawShadowMetrics`).

use crate::color::Color;
use crate::floating_point::ieee_float_divide;
use crate::matrix::Matrix;
use crate::path::Path;
use crate::point::{Point, Vector};
use crate::point3::Point3;
use crate::rect::Rect;
use crate::scalar::{SCALAR_NEARLY_ZERO, scalar, scalar_invert};
use crate::shadow_utils::ShadowFlags;

/// Parameters of one shadow draw (`SkDrawShadowRec`).
// Port of: src/core/SkDrawShadowInfo.h#L23-L31 (chrome/m156)
#[doc(alias = "SkDrawShadowRec")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawShadowRec {
    /// Plane function params giving the occluder's Z offset from local x and y.
    pub z_plane_params: Point3,
    /// The light position (device space unless directional).
    pub light_pos: Point3,
    /// The light radius (or directional blur at elevation 1).
    pub light_radius: scalar,
    /// The ambient shadow color.
    pub ambient_color: Color,
    /// The spot shadow color.
    pub spot_color: Color,
    /// [`ShadowFlags`] bits.
    pub flags: u32,
}

// Port of: src/core/SkDrawShadowInfo.h#L34-L36 (chrome/m156)
pub(crate) const AMBIENT_HEIGHT_FACTOR: scalar = 1.0 / 128.0;
// Port of: src/core/SkDrawShadowInfo.h#L37 (chrome/m156)
pub(crate) const AMBIENT_GEOM_FACTOR: scalar = 64.0;
// Port of: src/core/SkDrawShadowInfo.h#L41 (chrome/m156)
pub(crate) const MAX_AMBIENT_RADIUS: scalar = 300.0 * AMBIENT_HEIGHT_FACTOR * AMBIENT_GEOM_FACTOR;

/// `std::min(a, b)`: returns `b` only if `b < a`.
#[inline]
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: returns `b` only if `a < b`.
#[inline]
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// `SkTPin(x, lo, hi)` as `std::max(lo, std::min(x, hi))`, so NaN inputs pin to `lo`.
#[inline]
fn sk_t_pin(x: scalar, lo: scalar, hi: scalar) -> scalar {
    std_max(lo, std_min(x, hi))
}

/// `divide_and_pin`: `SkTPin(sk_ieee_float_divide(numer, denom), min, max)`.
// Port of: src/core/SkDrawShadowInfo.h#L44-L50 (chrome/m156)
#[inline]
fn divide_and_pin(numer: f32, denom: f32, min: f32, max: f32) -> f32 {
    sk_t_pin(ieee_float_divide(numer, denom), min, max)
}

// Port of: src/core/SkDrawShadowInfo.h#L52-L54 (chrome/m156)
#[doc(alias = "AmbientBlurRadius")]
#[must_use]
pub fn ambient_blur_radius(height: scalar) -> scalar {
    std_min(
        height * AMBIENT_HEIGHT_FACTOR * AMBIENT_GEOM_FACTOR,
        MAX_AMBIENT_RADIUS,
    )
}

// Port of: src/core/SkDrawShadowInfo.h#L56-L58 (chrome/m156)
#[doc(alias = "AmbientRecipAlpha")]
#[must_use]
pub fn ambient_recip_alpha(height: scalar) -> scalar {
    1.0 + std_max(height * AMBIENT_HEIGHT_FACTOR, 0.0)
}

// Port of: src/core/SkDrawShadowInfo.h#L60-L63 (chrome/m156)
#[doc(alias = "SpotBlurRadius")]
#[must_use]
pub fn spot_blur_radius(occluder_z: scalar, light_z: scalar, light_radius: scalar) -> scalar {
    light_radius * divide_and_pin(occluder_z, light_z - occluder_z, 0.0, 0.95)
}

/// Spot shadow blur, scale and translation for a point light (`GetSpotParams`). The result is
/// `(blur_radius, scale, translate)`.
// Port of: src/core/SkDrawShadowInfo.h#L65-L74 (chrome/m156)
#[doc(alias = "GetSpotParams")]
#[must_use]
pub fn get_spot_params(
    occluder_z: scalar,
    light_x: scalar,
    light_y: scalar,
    light_z: scalar,
    light_radius: scalar,
) -> (scalar, scalar, Vector) {
    let z_ratio = divide_and_pin(occluder_z, light_z - occluder_z, 0.0, 0.95);
    let blur_radius = light_radius * z_ratio;
    let scale = divide_and_pin(light_z, light_z - occluder_z, 1.0, 1.95);
    let translate = Point::new(-z_ratio * light_x, -z_ratio * light_y);
    (blur_radius, scale, translate)
}

/// Spot shadow blur, scale and translation for a directional light (`GetDirectionalParams`).
/// The result is `(blur_radius, scale, translate)`.
// Port of: src/core/SkDrawShadowInfo.h#L76-L92 (chrome/m156)
#[doc(alias = "GetDirectionalParams")]
#[must_use]
pub fn get_directional_params(
    occluder_z: scalar,
    light_x: scalar,
    light_y: scalar,
    light_z: scalar,
    light_radius: scalar,
) -> (scalar, scalar, Vector) {
    let blur_radius = light_radius * occluder_z;
    let scale = 1.0;
    // Max z-ratio is "max expected elevation"/"min allowable z"
    let max_z_ratio: scalar = 64.0 / SCALAR_NEARLY_ZERO;
    let z_ratio = divide_and_pin(occluder_z, light_z, 0.0, max_z_ratio);
    let translate = Point::new(-z_ratio * light_x, -z_ratio * light_y);
    (blur_radius, scale, translate)
}

/// `compute_z`: the plane function evaluated at `(x, y)`.
// Port of: src/core/SkDrawShadowInfo.cpp#L15-L17 (chrome/m156)
fn compute_z(x: scalar, y: scalar, params: Point3) -> scalar {
    x * params.x + y * params.y + params.z
}

/// Creates the transformation to apply to a path to get its base shadow outline, given the light
/// parameters and the path's 3D transformation (given by `ctm` and `z_plane_params`). Also
/// computes the blur radius to apply the transformed outline. Returns `None` when the shadow
/// cannot be placed.
// Port of: src/core/SkDrawShadowInfo.cpp#L19-L120 (chrome/m156)
#[doc(alias = "GetSpotShadowTransform")]
#[must_use]
pub fn get_spot_shadow_transform(
    light_pos: Point3,
    light_radius: scalar,
    ctm: &Matrix,
    z_plane_params: Point3,
    path_bounds: &Rect,
    directional: bool,
) -> Option<(Matrix, scalar)> {
    let height_func = |x: scalar, y: scalar| -> scalar {
        z_plane_params.x * x + z_plane_params.y * y + z_plane_params.z
    };

    let occluder_height = height_func(path_bounds.center_x(), path_bounds.center_y());

    let mut shadow_transform = Matrix::default();
    let radius: scalar;
    // TODO: have directional lights support tilt via the zPlaneParams
    if !ctm.has_perspective() || directional {
        let scale;
        let translate;
        if directional {
            (radius, scale, translate) = get_directional_params(
                occluder_height,
                light_pos.x,
                light_pos.y,
                light_pos.z,
                light_radius,
            );
        } else {
            (radius, scale, translate) = get_spot_params(
                occluder_height,
                light_pos.x,
                light_pos.y,
                light_pos.z,
                light_radius,
            );
        }
        shadow_transform.set_scale_translate((scale, scale), translate);
        shadow_transform.pre_concat(ctm);
    } else {
        if scalar_nearly_zero(path_bounds.width()) || scalar_nearly_zero(path_bounds.height()) {
            return None;
        }

        // get rotated quad in 3D
        let pts = ctm.map_rect_to_quad(path_bounds);
        let mut pts3d = [Point3::default(); 4];
        let mut z = height_func(path_bounds.left, path_bounds.top);
        pts3d[0] = Point3::new(pts[0].x, pts[0].y, z);
        z = height_func(path_bounds.right, path_bounds.top);
        pts3d[1] = Point3::new(pts[1].x, pts[1].y, z);
        z = height_func(path_bounds.right, path_bounds.bottom);
        pts3d[2] = Point3::new(pts[2].x, pts[2].y, z);
        z = height_func(path_bounds.left, path_bounds.bottom);
        pts3d[3] = Point3::new(pts[3].x, pts[3].y, z);

        // project from light through corners to z=0 plane
        for p in &mut pts3d {
            let dz = light_pos.z - p.z;
            // light shouldn't be below or at a corner's z-location
            if dz <= SCALAR_NEARLY_ZERO {
                return None;
            }
            let z_ratio = p.z / dz;
            p.x -= (light_pos.x - p.x) * z_ratio;
            p.y -= (light_pos.y - p.y) * z_ratio;
            p.z = 1.0;
        }

        // Generate matrix that projects from [-1,1]x[-1,1] square to projected quad
        // Compute homogenous crossing point between top and bottom edges (gives new x-axis).
        let mut h0 = pts3d[1].cross(pts3d[0]).cross(pts3d[2].cross(pts3d[3]));
        // Compute homogenous crossing point between left and right edges (gives new y-axis).
        let mut h1 = pts3d[0].cross(pts3d[3]).cross(pts3d[1].cross(pts3d[2]));
        // Compute homogenous crossing point between diagonals (gives new origin).
        let h2 = pts3d[0].cross(pts3d[2]).cross(pts3d[1].cross(pts3d[3]));

        // If h2 is a vector (z=0 in 2D homogeneous space), that means that at least
        // two of the quad corners are coincident and we don't have a realistic projection
        if scalar_nearly_zero(h2.z) {
            return None;
        }

        // In some cases the crossing points are in the wrong direction
        // to map (-1,-1) to pts3D[0], so we need to correct for that.
        // Want h0 to be to the right of the left edge.
        let mut v = pts3d[3] - pts3d[0];
        let w = h0 - pts3d[0];
        let mut perp_dot = v.x * w.y - v.y * w.x;
        if perp_dot > 0.0 {
            h0 = -h0;
        }

        // Want h1 to be above the bottom edge.
        v = pts3d[1] - pts3d[0];
        perp_dot = v.x * w.y - v.y * w.x;
        if perp_dot < 0.0 {
            h1 = -h1;
        }

        shadow_transform.set_all(
            h0.x / h2.z,
            h1.x / h2.z,
            h2.x / h2.z,
            h0.y / h2.z,
            h1.y / h2.z,
            h2.y / h2.z,
            h0.z / h2.z,
            h1.z / h2.z,
            1.0,
        );

        // generate matrix that transforms from bounds to [-1,1]x[-1,1] square
        let x_scale = 2.0 / (path_bounds.right - path_bounds.left);
        let y_scale = 2.0 / (path_bounds.bottom - path_bounds.top);
        let mut to_homogeneous = Matrix::default();
        to_homogeneous.set_all(
            x_scale,
            0.0,
            -x_scale * path_bounds.left - 1.0,
            0.0,
            y_scale,
            -y_scale * path_bounds.top - 1.0,
            0.0,
            0.0,
            1.0,
        );
        shadow_transform.pre_concat(&to_homogeneous);

        radius = spot_blur_radius(occluder_height, light_pos.z, light_radius);
    }
    Some((shadow_transform, radius))
}

/// Get bounds prior to the `ctm` being applied (`GetLocalBounds`).
// Port of: src/core/SkDrawShadowInfo.cpp#L122-L216 (chrome/m156)
#[doc(alias = "GetLocalBounds")]
#[must_use]
pub fn get_local_bounds(path: &Path, rec: &DrawShadowRec, ctm: &Matrix) -> Rect {
    let mut ambient_bounds = *path.bounds();

    let mut occluder_z: scalar;
    if scalar_nearly_zero(rec.z_plane_params.x) && scalar_nearly_zero(rec.z_plane_params.y) {
        occluder_z = rec.z_plane_params.z;
    } else {
        occluder_z = compute_z(ambient_bounds.left, ambient_bounds.top, rec.z_plane_params);
        occluder_z = std_max(
            occluder_z,
            compute_z(ambient_bounds.right, ambient_bounds.top, rec.z_plane_params),
        );
        occluder_z = std_max(
            occluder_z,
            compute_z(
                ambient_bounds.left,
                ambient_bounds.bottom,
                rec.z_plane_params,
            ),
        );
        occluder_z = std_max(
            occluder_z,
            compute_z(
                ambient_bounds.right,
                ambient_bounds.bottom,
                rec.z_plane_params,
            ),
        );
    }

    let ambient_blur: scalar;
    let mut spot_blur: scalar;
    let spot_scale: scalar;
    let mut spot_offset: Vector;
    let directional = rec.flags & ShadowFlags::DIRECTIONAL_LIGHT.bits() != 0;
    if ctm.has_perspective() {
        // transform ambient and spot bounds into device space
        ambient_bounds = ctm.map_rect(ambient_bounds).0;

        // get ambient blur (in device space)
        ambient_blur = ambient_blur_radius(occluder_z);

        // get spot params (in device space)
        if directional {
            (spot_blur, spot_scale, spot_offset) = get_directional_params(
                occluder_z,
                rec.light_pos.x,
                rec.light_pos.y,
                rec.light_pos.z,
                rec.light_radius,
            );
        } else {
            let dev_light_pos = ctm.map_point(Point::new(rec.light_pos.x, rec.light_pos.y));
            (spot_blur, spot_scale, spot_offset) = get_spot_params(
                occluder_z,
                dev_light_pos.x,
                dev_light_pos.y,
                rec.light_pos.z,
                rec.light_radius,
            );
        }
    } else {
        let dev_to_src_scale = scalar_invert(ctm.min_scale());

        // get ambient blur (in local space)
        let dev_space_ambient_blur = ambient_blur_radius(occluder_z);
        ambient_blur = dev_space_ambient_blur * dev_to_src_scale;

        // get spot params (in local space)
        if directional {
            (spot_blur, spot_scale, spot_offset) = get_directional_params(
                occluder_z,
                rec.light_pos.x,
                rec.light_pos.y,
                rec.light_pos.z,
                rec.light_radius,
            );
            // light dir is in device space, so need to map spot offset back into local space
            if let Some(inverse) = ctm.invert() {
                spot_offset = inverse.map_vector(spot_offset);
            }
        } else {
            (spot_blur, spot_scale, spot_offset) = get_spot_params(
                occluder_z,
                rec.light_pos.x,
                rec.light_pos.y,
                rec.light_pos.z,
                rec.light_radius,
            );
        }
        // convert spot blur to local space
        spot_blur *= dev_to_src_scale;
    }

    // in both cases, adjust ambient and spot bounds
    let mut spot_bounds = ambient_bounds;
    ambient_bounds.outset((ambient_blur, ambient_blur));
    spot_bounds.left *= spot_scale;
    spot_bounds.top *= spot_scale;
    spot_bounds.right *= spot_scale;
    spot_bounds.bottom *= spot_scale;
    spot_bounds.offset(spot_offset);
    spot_bounds.outset((spot_blur, spot_blur));

    // merge bounds
    let mut bounds = ambient_bounds;
    bounds.join(spot_bounds);

    // outset a bit to account for floating point error
    bounds.outset((1.0, 1.0));

    // if perspective, transform back to src space
    if ctm.has_perspective() {
        // TODO: create tighter mapping from dev rect back to src rect
        if let Some(inverse) = ctm.invert() {
            bounds = inverse.map_rect(bounds).0;
        }
    }
    bounds
}

/// `SkScalarNearlyZero(x)`: `|x| <= SK_ScalarNearlyZero`.
#[inline]
fn scalar_nearly_zero(x: scalar) -> bool {
    x.abs() <= SCALAR_NEARLY_ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_radius_is_capped() {
        assert_eq!(ambient_blur_radius(0.0), 0.0);
        assert_eq!(ambient_blur_radius(1.0e9), MAX_AMBIENT_RADIUS);
    }
}
