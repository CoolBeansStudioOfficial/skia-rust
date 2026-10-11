// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GainmapTestCommon.h (chrome/m156), the approximate comparisons of gainmap
// parameters that the gainmap tests share.

use skia_rust_core::color::Color4f;
use skia_rust_core::gainmap_info::GainmapInfo;

use crate::{Reporter, reporter_assert};

// Return true if the relative difference between x and y is less than epsilon.
// Port of: tests/GainmapTestCommon.h#L13-L19 (chrome/m156), `ApproxEq(float, float, float)`.
#[must_use]
pub fn approx_eq(x: f32, y: f32, epsilon: f32) -> bool {
    let numerator = (x - y).abs();
    // To avoid being too sensitive around zero, set the minimum denominator to epsilon.
    let denominator = x.abs().min(y.abs()).max(epsilon);
    (numerator / denominator) <= epsilon
}

// Port of: tests/GainmapTestCommon.h#L21-L24 (chrome/m156), `ApproxEq(SkColor4f, SkColor4f)`.
// Only the RGB channels are compared.
#[must_use]
pub fn approx_eq_color(x: Color4f, y: Color4f, epsilon: f32) -> bool {
    approx_eq(x.r, y.r, epsilon) && approx_eq(x.g, y.g, epsilon) && approx_eq(x.b, y.b, epsilon)
}

// Port of: tests/GainmapTestCommon.h#L26-L62 (chrome/m156), `ExpectApproxEqInfo`.
pub fn expect_approx_eq_info(r: &mut Reporter, a: &GainmapInfo, b: &GainmapInfo) {
    let k_epsilon = 1e-4f32;
    reporter_assert!(
        r,
        approx_eq_color(a.gainmap_ratio_min, b.gainmap_ratio_min, k_epsilon)
    );
    reporter_assert!(
        r,
        approx_eq_color(a.gainmap_ratio_max, b.gainmap_ratio_max, k_epsilon)
    );
    reporter_assert!(
        r,
        approx_eq_color(a.gainmap_gamma, b.gainmap_gamma, k_epsilon)
    );
    reporter_assert!(r, approx_eq_color(a.epsilon_sdr, b.epsilon_sdr, k_epsilon));
    reporter_assert!(r, approx_eq_color(a.epsilon_hdr, b.epsilon_hdr, k_epsilon));
    reporter_assert!(
        r,
        approx_eq(a.display_ratio_sdr, b.display_ratio_sdr, k_epsilon)
    );
    reporter_assert!(
        r,
        approx_eq(a.display_ratio_hdr, b.display_ratio_hdr, k_epsilon)
    );
    reporter_assert!(r, a.gainmap_type == b.gainmap_type);
    reporter_assert!(r, a.base_image_type == b.base_image_type);
    reporter_assert!(
        r,
        a.gainmap_math_color_space.is_some() == b.gainmap_math_color_space.is_some()
    );
    if let (Some(a_cs), Some(b_cs)) = (&a.gainmap_math_color_space, &b.gainmap_math_color_space) {
        let a_fn = a_cs.transfer_fn();
        let a_m = a_cs.to_xyzd50();
        let b_fn = b_cs.transfer_fn();
        let b_m = b_cs.to_xyzd50();
        reporter_assert!(r, approx_eq(a_fn.g, b_fn.g, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.a, b_fn.a, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.b, b_fn.b, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.c, b_fn.c, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.d, b_fn.d, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.e, b_fn.e, k_epsilon));
        reporter_assert!(r, approx_eq(a_fn.f, b_fn.f, k_epsilon));
        // The round-trip of the color space through the ICC profile loses significant precision.
        // Use a larger epsilon for it.
        let k_matrix_epsilon = 1e-2f32;
        for i in 0..3 {
            for j in 0..3 {
                reporter_assert!(
                    r,
                    approx_eq(a_m.vals[i][j], b_m.vals[i][j], k_matrix_epsilon)
                );
            }
        }
    }
}
