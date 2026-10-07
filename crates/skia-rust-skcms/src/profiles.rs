// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc

//! Canonical profiles and transfer functions, profile comparison, primaries and white-point
//! conversion, and making profiles usable as transform destinations.

use std::sync::LazyLock;

use crate::curve::{approximate_curve, are_approximate_inverses, max_roundtrip_error};
use crate::math::{Vector3, fmaxf_, is_zero_to_one, mv_mul};
use crate::public::{
    AlphaFormat, Curve, IccProfile, Matrix3x3, PixelFormat, TransferFunction, signature,
};
use crate::transform::{prep_for_destination, transform};

// We choose to represent sRGB with its canonical transfer function.
// Port of: modules/skcms/skcms.cc#L1544-L1642 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (float)(double expression) casts
const SRGB_TRANSFER_FUNCTION: TransferFunction = TransferFunction::new(
    2.4,
    (1.0f64 / 1.055) as f32,
    (0.055f64 / 1.055) as f32,
    (1.0f64 / 12.92) as f32,
    0.04045,
    0.0,
    0.0,
);

// Port of: modules/skcms/skcms.cc#L1748-L1752 (chrome/m156)
#[allow(clippy::excessive_precision)] // Skia's literals kept verbatim
const SRGB_INVERSE_TRANSFER_FUNCTION: TransferFunction = TransferFunction::new(
    0.416_666_657,
    1.137_283_325,
    -0.0,
    12.920_000_076,
    0.003_130_805,
    -0.054_969_788,
    -0.0,
);

const IDENTITY_TRANSFER_FUNCTION: TransferFunction =
    TransferFunction::new(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

// Port of: modules/skcms/skcms.cc#L1544-L1642 (chrome/m156)
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
static SRGB_PROFILE: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    // buffer, size, tag count: moot here
    data_color_space: signature::RGB,
    pcs: signature::XYZ,

    // We choose to represent sRGB with its canonical transfer function,
    // and with its canonical XYZD50 gamut matrix.
    trc: [
        Curve::Parametric(SRGB_TRANSFER_FUNCTION),
        Curve::Parametric(SRGB_TRANSFER_FUNCTION),
        Curve::Parametric(SRGB_TRANSFER_FUNCTION),
    ],

    // 3x3 toXYZD50 matrix
    to_xyzd50: Matrix3x3 {
        vals: [
            [0.436_065_674, 0.385_147_095, 0.143_066_406],
            [0.222_488_403, 0.716_873_169, 0.060_607_910],
            [0.013_916_016, 0.097_076_416, 0.714_096_069],
        ],
    },

    has_trc: true,
    has_to_xyzd50: true,
    ..IccProfile::default()
});

// Just like sRGB above, but with identity transfer functions and toXYZD50 matrix.
// Port of: modules/skcms/skcms.cc#L1644-L1742 (chrome/m156)
static XYZD50_PROFILE: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    data_color_space: signature::RGB,
    pcs: signature::XYZ,

    trc: [
        Curve::Parametric(IDENTITY_TRANSFER_FUNCTION),
        Curve::Parametric(IDENTITY_TRANSFER_FUNCTION),
        Curve::Parametric(IDENTITY_TRANSFER_FUNCTION),
    ],

    to_xyzd50: Matrix3x3 {
        vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    },

    has_trc: true,
    has_to_xyzd50: true,
    ..IccProfile::default()
});

/// The sRGB color profile is so commonly used that we offer a canonical [`IccProfile`] for it.
// Port of: modules/skcms/skcms.cc#L1544 (chrome/m156)
#[doc(alias = "skcms_sRGB_profile")]
#[must_use]
pub fn srgb_profile() -> &'static IccProfile {
    &SRGB_PROFILE
}

/// Ditto for XYZD50, the most common profile connection space.
// Port of: modules/skcms/skcms.cc#L1644 (chrome/m156)
#[doc(alias = "skcms_XYZD50_profile")]
#[must_use]
pub fn xyzd50_profile() -> &'static IccProfile {
    &XYZD50_PROFILE
}

/// sRGB's canonical transfer function.
// Port of: modules/skcms/skcms.cc#L1744-L1746 (chrome/m156)
#[doc(alias = "skcms_sRGB_TransferFunction")]
#[must_use]
pub fn srgb_transfer_function() -> &'static TransferFunction {
    &SRGB_TRANSFER_FUNCTION
}

/// The inverse of sRGB's canonical transfer function.
// Port of: modules/skcms/skcms.cc#L1748-L1752 (chrome/m156)
#[doc(alias = "skcms_sRGB_Inverse_TransferFunction")]
#[must_use]
pub fn srgb_inverse_transfer_function() -> &'static TransferFunction {
    &SRGB_INVERSE_TRANSFER_FUNCTION
}

/// The identity transfer function.
// Port of: modules/skcms/skcms.cc#L1754-L1757 (chrome/m156)
#[doc(alias = "skcms_Identity_TransferFunction")]
#[must_use]
pub fn identity_transfer_function() -> &'static TransferFunction {
    &IDENTITY_TRANSFER_FUNCTION
}

// 252 of a random shuffle of all possible bytes.
// 252 is evenly divisible by 3 and 4.  Only 192, 10, 241, and 43 are missing.
// Used for ICC profile equivalence testing.
// Port of: modules/skcms/skcms.cc#L1759-L1775 (chrome/m156)
#[doc(alias = "skcms_252_random_bytes")]
pub const RANDOM_BYTES_252: [u8; 252] = [
    8, 179, 128, 204, 253, 38, 134, 184, 68, 102, 32, 138, 99, 39, 169, 215, 119, 26, 3, 223, 95,
    239, 52, 132, 114, 74, 81, 234, 97, 116, 244, 205, 30, 154, 173, 12, 51, 159, 122, 153, 61,
    226, 236, 178, 229, 55, 181, 220, 191, 194, 160, 126, 168, 82, 131, 18, 180, 245, 163, 22, 246,
    69, 235, 252, 57, 108, 14, 6, 152, 240, 255, 171, 242, 20, 227, 177, 238, 96, 85, 16, 211, 70,
    200, 149, 155, 146, 127, 145, 100, 151, 109, 19, 165, 208, 195, 164, 137, 254, 182, 248, 64,
    201, 45, 209, 5, 147, 207, 210, 113, 162, 83, 225, 9, 31, 15, 231, 115, 37, 58, 53, 24, 49,
    197, 56, 120, 172, 48, 21, 214, 129, 111, 11, 50, 187, 196, 34, 60, 103, 71, 144, 47, 203, 77,
    80, 232, 140, 222, 250, 206, 166, 247, 139, 249, 221, 72, 106, 27, 199, 117, 54, 219, 135, 118,
    40, 79, 41, 251, 46, 93, 212, 92, 233, 148, 28, 121, 63, 123, 158, 105, 59, 29, 42, 143, 23, 0,
    107, 176, 87, 104, 183, 156, 193, 189, 90, 188, 65, 190, 17, 198, 7, 186, 161, 1, 124, 78, 125,
    170, 133, 174, 218, 67, 157, 75, 101, 89, 217, 62, 33, 141, 228, 25, 35, 91, 230, 4, 2, 13, 73,
    86, 167, 237, 84, 243, 44, 185, 66, 130, 110, 150, 142, 216, 88, 112, 36, 224, 136, 202, 76,
    94, 98, 175, 213,
];

/// Practical equality test for two [`IccProfile`]s.
/// The implementation is subject to change, but it will always try to answer
/// "can I substitute A for B?" and "can I skip transforming from A to B?".
// Port of: modules/skcms/skcms.cc#L1778-L1834 (chrome/m156)
#[doc(alias = "skcms_ApproximatelyEqualProfiles")]
#[must_use]
pub fn approximately_equal_profiles(a: &IccProfile, b: &IccProfile) -> bool {
    // Test for exactly equal profiles first.
    if std::ptr::eq(a, b) || a.bit_eq(b) {
        return true;
    }

    // For now this is the essentially the same strategy we use in test_only.c
    // for our skcms_Transform() smoke tests:
    //    1) transform A to XYZD50
    //    2) transform B to XYZD50
    //    3) return true if they're similar enough
    // Our current criterion in 3) is maximum 1 bit error per XYZD50 byte.

    // skcms_252_random_bytes are 252 of a random shuffle of all possible bytes.
    // 252 is evenly divisible by 3 and 4.  Only 192, 10, 241, and 43 are missing.

    // We want to allow otherwise equivalent profiles tagged as grayscale and RGB
    // to be treated as equal.  But CMYK profiles are a totally different ballgame.
    let cmyk = signature::CMYK;
    if (a.data_color_space == cmyk) != (b.data_color_space == cmyk) {
        return false;
    }

    // Interpret as RGB_888 if data color space is RGB or GRAY, RGBA_8888 if CMYK.
    // TODO: working with RGBA_8888 either way is probably fastest.
    let mut fmt = PixelFormat::Rgb888;
    let mut npixels = 84;
    if a.data_color_space == signature::CMYK {
        fmt = PixelFormat::Rgba8888;
        npixels = 63;
    }

    // TODO: if A or B is a known profile (skcms_sRGB_profile, skcms_XYZD50_profile),
    // use pre-canned results and skip that skcms_Transform() call?
    let mut dst_a = [0u8; 252];
    let mut dst_b = [0u8; 252];
    if !transform(
        &RANDOM_BYTES_252,
        fmt,
        AlphaFormat::Unpremul,
        Some(a),
        &mut dst_a,
        PixelFormat::Rgb888,
        AlphaFormat::Unpremul,
        Some(xyzd50_profile()),
        npixels,
    ) {
        return false;
    }
    if !transform(
        &RANDOM_BYTES_252,
        fmt,
        AlphaFormat::Unpremul,
        Some(b),
        &mut dst_b,
        PixelFormat::Rgb888,
        AlphaFormat::Unpremul,
        Some(xyzd50_profile()),
        npixels,
    ) {
        return false;
    }

    // TODO: make sure this final check has reasonable codegen.
    for i in 0..252 {
        if (i32::from(dst_a[i]) - i32::from(dst_b[i])).abs() > 1 {
            return false;
        }
    }
    true
}

/// Similar to [`are_approximate_inverses`], answering the question for all three TRC curves of
/// the given profile. Again, passing [`srgb_inverse_transfer_function`] as `inv_tf` will answer
/// the question: "Does this profile have a transfer function that is very close to sRGB?"
// Port of: modules/skcms/skcms.cc#L1836-L1845 (chrome/m156)
#[doc(alias = "skcms_TRCs_AreApproximateInverse")]
#[must_use]
pub fn trcs_are_approximate_inverse(profile: &IccProfile, inv_tf: &TransferFunction) -> bool {
    if !profile.has_trc {
        return false;
    }

    are_approximate_inverses(&profile.trc[0], inv_tf)
        && are_approximate_inverses(&profile.trc[1], inv_tf)
        && are_approximate_inverses(&profile.trc[2], inv_tf)
}

/// Returns a matrix to adapt XYZ color from the given whitepoint to D50.
// Port of: modules/skcms/skcms.cc#L1863-L1902 (chrome/m156)
#[doc(alias = "skcms_AdaptToXYZD50")]
#[must_use]
pub fn adapt_to_xyzd50(wx: f32, wy: f32) -> Option<Matrix3x3> {
    if !is_zero_to_one(wx) || !is_zero_to_one(wy) {
        return None;
    }

    // Assumes that Y is 1.0f.
    let w_xyz = Vector3 {
        vals: [wx / wy, 1.0, (1.0 - wx - wy) / wy],
    };

    // Now convert toXYZ matrix to toXYZD50.
    let w_xyzd50 = Vector3 {
        vals: [0.96422f32, 1.0f32, 0.82521f32],
    };

    // Calculate the chromatic adaptation matrix.  We will use the Bradford method, thus
    // the matrices below.  The Bradford method is used by Adobe and is widely considered
    // to be the best.
    let xyz_to_lms = Matrix3x3 {
        vals: [
            [0.8951, 0.2664, -0.1614],
            [-0.7502, 1.7135, 0.0367],
            [0.0389, -0.0685, 1.0296],
        ],
    };
    let lms_to_xyz = Matrix3x3 {
        vals: [
            [0.986_992_9, -0.147_054_3, 0.159_962_7],
            [0.432_305_3, 0.518_360_3, 0.049_291_2],
            [-0.008_528_7, 0.040_042_8, 0.968_486_7],
        ],
    };

    let src_cone = mv_mul(&xyz_to_lms, &w_xyz);
    let dst_cone = mv_mul(&xyz_to_lms, &w_xyzd50);

    let mut to_xyzd50 = Matrix3x3 {
        vals: [
            [dst_cone.vals[0] / src_cone.vals[0], 0.0, 0.0],
            [0.0, dst_cone.vals[1] / src_cone.vals[1], 0.0],
            [0.0, 0.0, dst_cone.vals[2] / src_cone.vals[2]],
        ],
    };
    to_xyzd50 = to_xyzd50.concat(&xyz_to_lms);
    to_xyzd50 = lms_to_xyz.concat(&to_xyzd50);

    Some(to_xyzd50)
}

/// Returns a matrix to convert RGB color into XYZ adapted to D50, given the primaries and
/// whitepoint of the RGB model.
// Port of: modules/skcms/skcms.cc#L1904-L1946 (chrome/m156)
#[doc(alias = "skcms_PrimariesToXYZD50")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
pub fn primaries_to_xyzd50(
    rx: f32,
    ry: f32,
    gx: f32,
    gy: f32,
    bx: f32,
    by: f32,
    wx: f32,
    wy: f32,
) -> Option<Matrix3x3> {
    if !is_zero_to_one(rx)
        || !is_zero_to_one(ry)
        || !is_zero_to_one(gx)
        || !is_zero_to_one(gy)
        || !is_zero_to_one(bx)
        || !is_zero_to_one(by)
        || !is_zero_to_one(wx)
        || !is_zero_to_one(wy)
    {
        return None;
    }

    // First, we need to convert xy values (primaries) to XYZ.
    let primaries = Matrix3x3 {
        vals: [
            [rx, gx, bx],
            [ry, gy, by],
            [1.0 - rx - ry, 1.0 - gx - gy, 1.0 - bx - by],
        ],
    };
    let primaries_inv = primaries.invert()?;

    // Assumes that Y is 1.0f.
    let w_xyz = Vector3 {
        vals: [wx / wy, 1.0, (1.0 - wx - wy) / wy],
    };
    let xyz = mv_mul(&primaries_inv, &w_xyz);

    let mut to_xyz = Matrix3x3 {
        vals: [
            [xyz.vals[0], 0.0, 0.0],
            [0.0, xyz.vals[1], 0.0],
            [0.0, 0.0, xyz.vals[2]],
        ],
    };
    to_xyz = primaries.concat(&to_xyz);

    let dx_to_d50 = adapt_to_xyzd50(wx, wy)?;

    Some(dx_to_d50.concat(&to_xyz))
}

/// If `profile` can be used as a destination in [`transform`], returns true. Otherwise, attempts
/// to rewrite it with approximations where reasonable. If successful, returns true. If no
/// reasonable approximation exists, leaves the profile unchanged and returns false.
// Port of: modules/skcms/skcms.cc#L3148-L3179 (chrome/m156)
#[doc(alias = "skcms_MakeUsableAsDestination")]
#[must_use]
#[allow(clippy::needless_range_loop)] // mirrors the C++ index loops
pub fn make_usable_as_destination(profile: &mut IccProfile) -> bool {
    if !profile.has_b2a {
        if !profile.has_trc || !profile.has_to_xyzd50 || profile.to_xyzd50.invert().is_none() {
            return false;
        }

        let mut tf = [TransferFunction::default(); 3];
        for i in 0..3 {
            if let Curve::Parametric(parametric) = &profile.trc[i]
                && parametric.invert().is_some()
            {
                tf[i] = *parametric;
                continue;
            }

            // Parametric curves from skcms_ApproximateCurve() are guaranteed to be invertible.
            let Some((approx, _max_error)) = approximate_curve(&profile.trc[i]) else {
                return false;
            };
            tf[i] = approx;
        }

        for i in 0..3 {
            profile.trc[i] = Curve::Parametric(tf[i]);
        }
    }
    debug_assert!(prep_for_destination(profile).is_some());
    true
}

/// If `profile` can be used as a destination with a single parametric transfer function (ie for
/// rasterization), returns true. Otherwise, attempts to rewrite it with approximations where
/// reasonable. If successful, returns true. If no reasonable approximation exists, leaves the
/// profile unchanged and returns false.
// Port of: modules/skcms/skcms.cc#L3181-L3216 (chrome/m156)
#[doc(alias = "skcms_MakeUsableAsDestinationWithSingleCurve")]
#[must_use]
pub fn make_usable_as_destination_with_single_curve(profile: &mut IccProfile) -> bool {
    // Call skcms_MakeUsableAsDestination() with B2A disabled;
    // on success that'll return a TRC/XYZ profile with three skcms_TransferFunctions.
    let mut result = profile.clone();
    result.has_b2a = false;
    if !make_usable_as_destination(&mut result) {
        return false;
    }

    // Of the three, pick the transfer function that best fits the other two.
    let mut best_tf = 0;
    let mut min_max_error = f32::INFINITY;
    for i in 0..3 {
        let Some(parametric) = result.trc[i].parametric() else {
            return false;
        };
        let Some(inv) = parametric.invert() else {
            return false;
        };

        let mut err = 0.0f32;
        for j in 0..3 {
            err = fmaxf_(err, max_roundtrip_error(&profile.trc[j], &inv));
        }
        if min_max_error > err {
            min_max_error = err;
            best_tf = i;
        }
    }

    let best = result.trc[best_tf].clone();
    for i in 0..3 {
        result.trc[i] = best.clone();
    }

    *profile = result;
    debug_assert!(prep_for_destination(profile).is_some());
    true
}

/// Call before your first call to [`transform`] to skip runtime CPU detection. A no-op: the
/// portable port does not detect CPU features.
// Port of: modules/skcms/skcms.cc#L46-L48 (chrome/m156)
#[doc(alias = "skcms_DisableRuntimeCPUDetection")]
pub fn disable_runtime_cpu_detection() {}
