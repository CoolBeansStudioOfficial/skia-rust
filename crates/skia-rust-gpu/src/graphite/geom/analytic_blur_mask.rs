// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/AnalyticBlurMask.cpp (CPU helpers only)

//! CPU helpers of `skgpu::graphite::AnalyticBlurMask`.
//!
//! `AnalyticBlurMask::Make` and its `MakeRect`, `MakeCircle` and `MakeRRect` cache their lookup
//! tables through `Recorder`, `ProxyCache` and `TextureProxy`, none of which is ported yet, so the
//! class itself is not here. The two helpers below are the parts of `AnalyticBlurMask.cpp` that
//! need only geometry and arithmetic.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::scalar::{scalar, scalar_round_to_int};

use crate::graphite::geom::rect::Rect;

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
