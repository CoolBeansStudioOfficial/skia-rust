// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkColorSpacePriv.h

//! Private color space helpers that Skia's own tests use.

use skia_rust_skcms::{Matrix3x3, TransferFunction};

use crate::color_space::{ColorSpace, named_transfer_fn};

/// A gamut narrower than sRGB, useful for testing.
// Port of: src/core/SkColorSpacePriv.h#L16-L20 (chrome/m156)
#[doc(alias = "gNarrow_toXYZD50")]
pub const NARROW_TO_XYZD50: Matrix3x3 = Matrix3x3 {
    vals: [
        [0.190_974, 0.404_865, 0.368_380],
        [0.114_746, 0.582_937, 0.302_318],
        [0.032_925, 0.153_615, 0.638_669],
    ],
};

// Port of: src/core/SkColorSpacePriv.h#L22-L24 (chrome/m156)
#[must_use]
pub fn color_space_almost_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

// Let's use a stricter version for transfer functions.  Worst case, these are encoded
// in ICC format, which offers 16-bits of fractional precision.
// Port of: src/core/SkColorSpacePriv.h#L28-L30 (chrome/m156)
#[must_use]
pub fn transfer_fn_almost_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

// Port of: src/core/SkColorSpacePriv.h#L32-L40 (chrome/m156)
#[must_use]
pub fn is_almost_srgb(coeffs: &TransferFunction) -> bool {
    transfer_fn_almost_equal(named_transfer_fn::SRGB.a, coeffs.a)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.b, coeffs.b)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.c, coeffs.c)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.d, coeffs.d)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.e, coeffs.e)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.f, coeffs.f)
        && transfer_fn_almost_equal(named_transfer_fn::SRGB.g, coeffs.g)
}

// Port of: src/core/SkColorSpacePriv.h#L42-L48 (chrome/m156)
#[must_use]
pub fn is_almost_2dot2(coeffs: &TransferFunction) -> bool {
    transfer_fn_almost_equal(1.0, coeffs.a)
        && transfer_fn_almost_equal(0.0, coeffs.b)
        && transfer_fn_almost_equal(0.0, coeffs.e)
        && transfer_fn_almost_equal(2.2, coeffs.g)
        && coeffs.d <= 0.0
}

// Port of: src/core/SkColorSpacePriv.h#L50-L65 (chrome/m156)
#[must_use]
pub fn is_almost_linear(coeffs: &TransferFunction) -> bool {
    // OutputVal = InputVal ^ 1.0f
    let linear_exp = transfer_fn_almost_equal(1.0, coeffs.a)
        && transfer_fn_almost_equal(0.0, coeffs.b)
        && transfer_fn_almost_equal(0.0, coeffs.e)
        && transfer_fn_almost_equal(1.0, coeffs.g)
        && coeffs.d <= 0.0;

    // OutputVal = 1.0f * InputVal
    let linear_fn = transfer_fn_almost_equal(1.0, coeffs.c)
        && transfer_fn_almost_equal(0.0, coeffs.f)
        && coeffs.d >= 1.0;

    linear_exp || linear_fn
}

/// The shared sRGB color space (`sk_srgb_singleton`).
// Port of: src/core/SkColorSpace.cpp#L186-L190 (chrome/m156)
#[doc(alias = "sk_srgb_singleton")]
#[must_use]
pub fn srgb_singleton() -> &'static ColorSpace {
    crate::color_space::srgb_singleton()
}

/// The shared sRGB-gamut, linear-transfer-function color space (`sk_srgb_linear_singleton`).
// Port of: src/core/SkColorSpace.cpp#L192-L196 (chrome/m156)
#[doc(alias = "sk_srgb_linear_singleton")]
#[must_use]
pub fn srgb_linear_singleton() -> &'static ColorSpace {
    crate::color_space::srgb_linear_singleton()
}
