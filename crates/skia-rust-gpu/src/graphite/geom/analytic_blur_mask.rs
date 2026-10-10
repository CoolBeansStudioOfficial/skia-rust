// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/AnalyticBlurMask.cpp, AnalyticBlurMask.h

//! [`AnalyticBlurMask`]: the shader inputs of an analytic blur over a rect, a rounded rect or a
//! circle, and its CPU helpers `outset_bounds` and `quantize`.
//!
//! Not ported: `MakeRect` and `MakeRRect`. `MakeRect` builds its integral table with
//! `CreateIntegralTable`, which calls `std::erf`; the standard library has none, and an
//! approximation would not match. Until the libm decision (`docs/design/gpu.md`), `Make` returns
//! `None` for rects, so `Device::draw_blurred_rrect` falls back to a regular draw for them. The
//! circle case (`MakeCircle`) needs no `erf` and is ported. `MakeRRect` is only built under
//! `SK_SUPPORT_LEGACY_GRAPHITE_RRECT_BLUR`, which is off.

use std::sync::{Arc, OnceLock};

use skia_rust_core::m44::{M44, V2};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::rrect::{RRect, rrect_priv};
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, scalar, scalar_round_to_int};

use crate::gpu::blur_utils::{create_circle_profile, create_half_plane_profile};
use crate::gpu::resource_key::{UniqueKey, UniqueKeyBuilder, UniqueKeyDomain};
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::transform::Transform;
use crate::graphite::proxy_cache::find_or_create_cached_proxy_from_bitmap;
use crate::graphite::recorder::Recorder;
use crate::graphite::texture_proxy::TextureProxy;

/// `AnalyticBlurMask::ShapeType`. The blur shaders depend on the values.
// Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L24-L36 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticBlurMask::ShapeType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeType {
    /// `kRect`.
    Rect = 0,
    /// `kRRect`.
    RRect = 1,
    /// `kCircle`.
    Circle = 2,
}

/// `AnalyticBlurMask`: the shader inputs of an analytic blur over a rect, rounded rect or circle.
// Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L38-L121 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticBlurMask")]
#[derive(Clone, Debug)]
pub struct AnalyticBlurMask {
    /// Draw bounds in local space.
    draw_bounds: Rect,
    /// Transforms device-space coordinates to the shape data's space.
    dev_to_scaled_shape: M44,
    /// Shape data, in the local space of `local_to_device * dev_to_scaled_shape`.
    shape_data: Rect,
    /// Holds different data per shape type (see the C++ comment on `fBlurData`).
    blur_data: V2,
    shape_type: ShapeType,
    proxy: Arc<TextureProxy>,
}

impl AnalyticBlurMask {
    /// `AnalyticBlurMask::Make(recorder, localToDevice, deviceSigma, srcRRect)`. Returns `None`
    /// when the transform is not a similarity, and for the shapes whose integral table needs
    /// `std::erf` (rects; see the module docs).
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L69-L107 (chrome/m156)
    #[must_use]
    pub fn make(
        recorder: &Recorder,
        local_to_device_transform: &Transform,
        device_sigma: f32,
        src_rrect: &RRect,
    ) -> Option<Self> {
        // TODO: Implement SkMatrix functionality used below for Transform.
        let local_to_device = local_to_device_transform.to_matrix();
        if !local_to_device.is_similarity() {
            return None;
        }

        if src_rrect.is_rect() && local_to_device.preserves_right_angles() {
            // MakeRect: needs `CreateIntegralTable` (std::erf), not ported.
            return None;
        }

        let dev_rrect = src_rrect.transform(&local_to_device);
        if let Some(dev_rrect) = dev_rrect
            && rrect_priv::is_circle(&dev_rrect)
        {
            return Self::make_circle(
                recorder,
                &local_to_device,
                device_sigma,
                src_rrect.rect(),
                dev_rrect.rect(),
            );
        }

        // A local-space circle transformed by a rotation matrix will fail SkRRect::transform since
        // it only supports scale + translate matrices, but is still a valid circle that can be
        // blurred.
        if rrect_priv::is_circle(src_rrect) && local_to_device.is_similarity() {
            let src_rect = src_rrect.rect();
            let dev_center = local_to_device.map_point(src_rect.center());
            let dev_radius = local_to_device
                .map_vector((0.0, src_rect.width() / 2.0))
                .length();
            let dev_rect = SkRect {
                left: dev_center.x - dev_radius,
                top: dev_center.y - dev_radius,
                right: dev_center.x + dev_radius,
                bottom: dev_center.y + dev_radius,
            };
            return Self::make_circle(
                recorder,
                &local_to_device,
                device_sigma,
                src_rect,
                &dev_rect,
            );
        }

        // SK_SUPPORT_LEGACY_GRAPHITE_RRECT_BLUR is off: MakeRRect is not built.
        None
    }

    /// `AnalyticBlurMask::MakeCircle`: the blurred circle, with its profile texture.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L210-L281 (chrome/m156)
    fn make_circle(
        recorder: &Recorder,
        local_to_device: &Matrix,
        dev_sigma: f32,
        src_rect: &SkRect,
        dev_rect: &SkRect,
    ) -> Option<Self> {
        let radius = (dev_rect.right - dev_rect.left) / 2.0;
        if !radius.is_finite() || radius < SCALAR_NEARLY_ZERO {
            return None;
        }

        // Pack profile-dependent properties and derived values into a struct that can be passed
        // into `findOrCreateCachedProxy` to lazily invoke the profile creation bitmap factories.
        let params = DerivedParams::new(dev_sigma, radius);

        let mut key = UniqueKey::new();
        {
            static DOMAIN: OnceLock<UniqueKeyDomain> = OnceLock::new();
            let domain = *DOMAIN.get_or_init(UniqueKey::generate_domain);
            let mut builder =
                UniqueKeyBuilder::new(&mut key, domain, 2, Some("BlurredCircleIntegralTable"));
            if params.use_half_plane_approx {
                // There only ever needs to be one half plane approximation table, so store {0,0}
                // into the key, which never arises under normal use because we reject radius = 0.
                builder[0] = 0.0_f32.to_bits();
                builder[1] = 0.0_f32.to_bits();
            } else {
                builder[0] = params.quantized_dev_sigma.to_bits();
                builder[1] = params.quantized_radius.to_bits();
            }
        }
        let profile = find_or_create_cached_proxy_from_bitmap(
            recorder,
            &key,
            || {
                const K_PROFILE_TEXTURE_WIDTH: i32 = 512;
                if params.use_half_plane_approx {
                    create_half_plane_profile(K_PROFILE_TEXTURE_WIDTH)
                } else {
                    // Rescale params to the size of the texture we're creating.
                    #[allow(clippy::cast_precision_loss)] // the C++ divides an int by a float
                    let scale = K_PROFILE_TEXTURE_WIDTH as f32 / params.texture_radius;
                    create_circle_profile(
                        params.quantized_dev_sigma * scale,
                        params.quantized_radius * scale,
                        K_PROFILE_TEXTURE_WIDTH,
                    )
                }
            },
            "",
        )?;

        // In the shader we calculate an index into the blur profile
        // "i = (length(fragCoords - circleCenter) - solidRadius + 0.5) / textureRadius" as
        // "i = length((fragCoords - circleCenter) / textureRadius) -
        //      (solidRadius - 0.5) / textureRadius"
        // to avoid passing large values to length() that would overflow. We precalculate
        // "1 / textureRadius" and "(solidRadius - 0.5) / textureRadius" here.
        let shape_data = Rect::new(
            (dev_rect.left + dev_rect.right) * 0.5,
            (dev_rect.top + dev_rect.bottom) * 0.5,
            1.0 / params.texture_radius,
            (params.solid_radius - 0.5) / params.texture_radius,
        );

        // Determine how much to outset the draw bounds to ensure we hit pixels within 3*sigma.
        let draw_bounds = outset_bounds(local_to_device, params.quantized_dev_sigma, src_rect)?;

        Some(Self {
            draw_bounds,
            dev_to_scaled_shape: M44::new_identity(),
            shape_data,
            // kUnusedBlurData
            blur_data: V2::new(0.0, 0.0),
            shape_type: ShapeType::Circle,
            proxy: profile,
        })
    }

    /// `drawBounds()`: the draw bounds in local space.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L53 (chrome/m156)
    #[must_use]
    pub const fn draw_bounds(&self) -> &Rect {
        &self.draw_bounds
    }

    /// `deviceToScaledShape()`.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L54 (chrome/m156)
    #[must_use]
    pub const fn device_to_scaled_shape(&self) -> &M44 {
        &self.dev_to_scaled_shape
    }

    /// `shapeData()`.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L55 (chrome/m156)
    #[must_use]
    pub const fn shape_data(&self) -> &Rect {
        &self.shape_data
    }

    /// `shapeType()`.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L56 (chrome/m156)
    #[must_use]
    pub const fn shape_type(&self) -> ShapeType {
        self.shape_type
    }

    /// `blurData()`.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L57 (chrome/m156)
    #[must_use]
    pub const fn blur_data(&self) -> V2 {
        self.blur_data
    }

    /// `refProxy()`: the lookup texture of the blur.
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.h#L58 (chrome/m156)
    #[must_use]
    pub fn ref_proxy(&self) -> Arc<TextureProxy> {
        Arc::clone(&self.proxy)
    }
}

/// `DerivedParams` of `AnalyticBlurMask::MakeCircle`: the quantized radius and sigma, and the
/// profile's geometry.
// Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L223-L259 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct DerivedParams {
    quantized_radius: f32,
    quantized_dev_sigma: f32,
    solid_radius: f32,
    texture_radius: f32,
    use_half_plane_approx: bool,
}

impl DerivedParams {
    // Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L223-L259 (chrome/m156)
    fn new(dev_sigma: f32, radius: f32) -> Self {
        let quantized_radius = quantize(radius);
        let mut quantized_dev_sigma = quantize(dev_sigma);
        debug_assert!(quantized_radius > 0.0); // quantization shouldn't have rounded to 0

        // When sigma is really small this becomes a equivalent to convolving a Gaussian with a
        // half-plane. Similarly, in the extreme high ratio cases circle becomes a point WRT to the
        // Guassian and the profile texture is a just a Gaussian evaluation. However, we haven't yet
        // implemented this latter optimization.
        const K_HALF_PLANE_THRESHOLD: f32 = 0.1;
        // std::min(a, b) is `(b < a) ? b : a`.
        let ratio = quantized_dev_sigma / quantized_radius;
        let sigma_to_radius_ratio = if 8.0 < ratio { 8.0 } else { ratio };
        if sigma_to_radius_ratio <= K_HALF_PLANE_THRESHOLD {
            Self {
                quantized_radius,
                quantized_dev_sigma,
                solid_radius: quantized_radius - 3.0 * quantized_dev_sigma,
                texture_radius: 6.0 * quantized_dev_sigma,
                use_half_plane_approx: true,
            }
        } else {
            quantized_dev_sigma = quantized_radius * sigma_to_radius_ratio;
            Self {
                quantized_radius,
                quantized_dev_sigma,
                solid_radius: 0.0,
                texture_radius: quantized_radius + 3.0 * quantized_dev_sigma,
                use_half_plane_approx: false,
            }
        }
    }
}

/// `outset_bounds(localToDevice, devSigma, srcRect)`: the local-space rect that the blur of
/// `src_rect` can reach, which is three sigmas beyond the rect in device space. `None` when the
/// transform has no decomposable scale.
// Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L51-L68 (chrome/m156)
#[must_use]
pub fn outset_bounds(local_to_device: &Matrix, dev_sigma: f32, src_rect: &SkRect) -> Option<Rect> {
    let mut outset_x: f32 = 3.0 * dev_sigma;
    let mut outset_y: f32 = 3.0 * dev_sigma;
    if local_to_device.is_scale_translate() {
        outset_x /= local_to_device.scale_x().abs();
        outset_y /= local_to_device.scale_y().abs();
    } else {
        let scale = local_to_device.decompose_scale(None)?;
        outset_x /= scale.width;
        outset_y /= scale.height;
    }
    // SkRect::makeOutset(dx, dy): Rect(left - dx, top - dy, right + dx, bottom + dy).
    Some(Rect::from_sk_rect(&SkRect {
        left: src_rect.left - outset_x,
        top: src_rect.top - outset_y,
        right: src_rect.right + outset_x,
        bottom: src_rect.bottom + outset_y,
    }))
}

/// `quantize(deviceSpaceFloat)`: snaps a device-space value to the nearest 1/32, and clamps it to
/// at least 1/32.
// Port of: src/gpu/graphite/geom/AnalyticBlurMask.cpp#L208-L213 (chrome/m156)
#[must_use]
pub fn quantize(device_space_float: scalar) -> scalar {
    // Snap the device-space value to the nearest 1/32 to increase cache hits w/o impacting the
    // visible output since it should be hard to see a change limited to 1/32 of a pixel.
    // Clamp the value to 1/32 as identity blurs and points should be caught earlier.
    // std::max(a, b) is `(a < b) ? b : a`, which is not `f32::max` for NaN.
    // The C++ divides the int by a float, so the int is converted to float first.
    #[allow(clippy::cast_precision_loss)]
    let snapped = scalar_round_to_int(device_space_float * 32.0) as f32 / 32.0;
    let min = 1.0 / 32.0;
    if snapped < min { min } else { snapped }
}

#[cfg(test)]
mod tests {
    use super::quantize;

    #[test]
    fn quantize_snaps_to_one_thirty_second_and_clamps() {
        assert_eq!(quantize(0.0), 1.0 / 32.0);
        assert_eq!(quantize(0.5), 0.5);
        assert_eq!(quantize(0.51), 16.0 / 32.0);
        assert_eq!(quantize(-3.0), 1.0 / 32.0);
    }
}
