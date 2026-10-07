// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorSpaceTest.cpp (chrome/m156)
//
// Not ported (they need SkCodec, which is not ported yet; the manifest entry stays `todo`):
//   - ColorSpaceParseICCProfiles (and its `test_path` helper)

use skia_rust_core::color_space::{
    ColorSpace, ColorSpacePrimaries, named_gamut, named_transfer_fn,
};
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::floating_point::FLOAT_NAN;
use skia_rust_skcms::{self as skcms, Matrix3x3, TransferFunction};

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert, skip_missing_resource};

fn almost_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

// Port of: tests/ColorSpaceTest.cpp#L33-L50 (chrome/m156)
#[allow(clippy::needless_range_loop)] // mirrors the C++ index loop
fn test_space(
    r: &mut Reporter,
    space: Option<&ColorSpace>,
    red: &[f32; 3],
    green: &[f32; 3],
    blue: &[f32; 3],
    expect_srgb: bool,
) {
    reporter_assert!(r, space.is_some());
    let Some(space) = space else {
        return;
    };
    reporter_assert!(r, expect_srgb == space.gamma_close_to_srgb());

    let mat = space.to_xyzd50();
    let reference: [&[f32; 3]; 3] = [red, green, blue];
    for i in 0..3 {
        reporter_assert!(r, almost_equal(reference[i][0], mat.vals[0][i]));
        reporter_assert!(r, almost_equal(reference[i][1], mat.vals[1][i]));
        reporter_assert!(r, almost_equal(reference[i][2], mat.vals[2][i]));
    }
}

const G_SRGB_R: [f32; 3] = [0.4358, 0.2224, 0.0139];
const G_SRGB_G: [f32; 3] = [0.3853, 0.7170, 0.0971];
const G_SRGB_B: [f32; 3] = [0.1430, 0.0606, 0.7139];

// Port of: tests/ColorSpaceTest.cpp#L74-L77 (chrome/m156)
def_test!(ColorSpace_sRGB, |r| {
    test_space(
        r,
        Some(srgb_singleton()),
        &G_SRGB_R,
        &G_SRGB_G,
        &G_SRGB_B,
        true,
    );
});

// Port of: tests/ColorSpaceTest.cpp#L100-L127 (chrome/m156)
fn test_serialize(r: &mut Reporter, space: &ColorSpace, is_named: bool) {
    let data1 = space.serialize();

    let bytes = space.write_to_memory(None);
    let mut data2 = vec![0u8; bytes];
    let _ = space.write_to_memory(Some(&mut data2));

    // skia-rust: not expressible in Rust: the SK_DUMP_TO_DISK block (a debugging aid that is
    // compiled out in Skia's test builds).

    let new_space1 = ColorSpace::deserialize(&data1);
    let new_space2 = ColorSpace::deserialize(&data2);

    if is_named {
        reporter_assert!(r, new_space1.as_ref().is_some_and(|n| space.ptr_eq(n)));
        reporter_assert!(r, new_space2.as_ref().is_some_and(|n| space.ptr_eq(n)));
    } else {
        reporter_assert!(r, ColorSpace::equals(Some(space), new_space1.as_ref()));
        reporter_assert!(r, ColorSpace::equals(Some(space), new_space2.as_ref()));
    }
}

// Port of: tests/ColorSpaceTest.cpp#L129-L161 (chrome/m156)
def_test!(ColorSpace_Serialize, |r| {
    test_serialize(r, &ColorSpace::new_srgb(), true);
    test_serialize(r, &ColorSpace::new_srgb_linear(), true);

    let test = |r: &mut Reporter, path: &str| {
        let data = skip_missing_resource!(get_resource_as_data(path), path);

        let profile = skcms::parse(&data);
        reporter_assert!(r, profile.is_some());
        let Some(profile) = profile else {
            return;
        };

        let space = ColorSpace::make(&profile);
        reporter_assert!(r, space.is_some());
        let Some(space) = space else {
            return;
        };

        test_serialize(r, &space, false);
    };
    test(r, "icc_profiles/HP_ZR30w.icc");
    test(r, "icc_profiles/HP_Z32x.icc");

    let fn_ = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 1.0,
        d: 0.5,
        e: 0.0,
        f: 0.0,
        g: 1.0,
    };
    let to_xyz = Matrix3x3 {
        vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    let space = ColorSpace::new_rgb(&fn_, &to_xyz);
    reporter_assert!(r, space.is_some());
    if let Some(space) = space {
        test_serialize(r, &space, false);
    }
});

// Port of: tests/ColorSpaceTest.cpp#L163-L207 (chrome/m156)
def_test!(ColorSpace_Equals, |r| {
    let srgb = ColorSpace::new_srgb();

    let parse = |r: &mut Reporter, path: &str| -> Option<ColorSpace> {
        let data = skip_missing_resource!(get_resource_as_data(path), path, None);

        let profile = skcms::parse(&data);
        reporter_assert!(r, profile.is_some());
        let profile = profile?;

        let space = ColorSpace::make(&profile);
        reporter_assert!(r, space.is_some());

        space
    };
    let z30 = parse(r, "icc_profiles/HP_ZR30w.icc");
    let z32 = parse(r, "icc_profiles/HP_Z32x.icc");

    let fn_ = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 1.0,
        d: 0.5,
        e: 0.0,
        f: 0.0,
        g: 1.0,
    };
    let to_xyz = Matrix3x3 {
        vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    let rgb4 = ColorSpace::new_rgb(&fn_, &to_xyz);

    // The C++ `parse` lambda returns null for a missing resource and then crashes on use; skip
    // the rest of the test in that case.
    let (Some(z30), Some(z32)) = (z30, z32) else {
        eprintln!("todo: skipping, missing Skia resource icc_profiles/HP_ZR30w.icc or HP_Z32x.icc");
        return;
    };

    reporter_assert!(r, ColorSpace::equals(None, None));
    reporter_assert!(r, ColorSpace::equals(Some(&srgb), Some(&srgb)));
    reporter_assert!(r, ColorSpace::equals(Some(&z30), Some(&z30)));
    reporter_assert!(r, ColorSpace::equals(Some(&z32), Some(&z32)));
    reporter_assert!(r, ColorSpace::equals(rgb4.as_ref(), rgb4.as_ref()));

    reporter_assert!(r, !ColorSpace::equals(None, Some(&srgb)));
    reporter_assert!(r, !ColorSpace::equals(Some(&srgb), None));
    reporter_assert!(r, !ColorSpace::equals(Some(&z30), Some(&srgb)));
    reporter_assert!(r, !ColorSpace::equals(Some(&z32), Some(&z30)));
    reporter_assert!(r, !ColorSpace::equals(Some(&z30), rgb4.as_ref()));
    reporter_assert!(r, !ColorSpace::equals(Some(&srgb), rgb4.as_ref()));
});

// Port of: tests/ColorSpaceTest.cpp#L209-L218 (chrome/m156)
fn matrix_almost_equal(a: &Matrix3x3, b: &Matrix3x3) -> bool {
    for r in 0..3 {
        for c in 0..3 {
            if !almost_equal(a.vals[r][c], b.vals[r][c]) {
                return false;
            }
        }
    }
    true
}

// Port of: tests/ColorSpaceTest.cpp#L220-L226 (chrome/m156)
fn check_primaries(r: &mut Reporter, primaries: &ColorSpacePrimaries, reference: &Matrix3x3) {
    let to_xyz = primaries.to_xyzd50();
    reporter_assert!(r, to_xyz.is_some());
    let Some(to_xyz) = to_xyz else {
        return;
    };
    reporter_assert!(r, matrix_almost_equal(&to_xyz, reference));
}

// Port of: tests/ColorSpaceTest.cpp#L228-L304 (chrome/m156)
def_test!(
    #[allow(clippy::unreadable_literal)] // Skia's float literals kept verbatim
    #[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
    ColorSpace_Primaries,
    |r| {
        // sRGB primaries (D65)
        let srgb_to_xyz =
            skcms::primaries_to_xyzd50(0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290);
        reporter_assert!(r, srgb_to_xyz.is_some());
        let srgb_to_xyz = srgb_to_xyz.unwrap_or_default();

        let space = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &srgb_to_xyz);
        reporter_assert!(
            r,
            space
                .as_ref()
                .is_some_and(|s| ColorSpace::new_srgb().ptr_eq(s))
        );

        // ProPhoto (D50)
        let pro_photo = ColorSpacePrimaries {
            rx: 0.7347,
            ry: 0.2653,
            gx: 0.1596,
            gy: 0.8404,
            bx: 0.0366,
            by: 0.0001,
            wx: 0.34567,
            wy: 0.35850,
        };
        let pro_to_xyz = Matrix3x3 {
            vals: [
                [0.7976749, 0.1351917, 0.0313534],
                [0.2880402, 0.7118741, 0.0000857],
                [0.0000000, 0.0000000, 0.8252100],
            ],
        };
        check_primaries(r, &pro_photo, &pro_to_xyz);

        // NTSC (C)
        let ntsc = ColorSpacePrimaries {
            rx: 0.67,
            ry: 0.33,
            gx: 0.21,
            gy: 0.71,
            bx: 0.14,
            by: 0.08,
            wx: 0.31006,
            wy: 0.31616,
        };
        let ntsc_to_xyz = Matrix3x3 {
            vals: [
                [0.6343706, 0.1852204, 0.1446290],
                [0.3109496, 0.5915984, 0.0974520],
                [-0.0011817, 0.0555518, 0.7708399],
            ],
        };
        check_primaries(r, &ntsc, &ntsc_to_xyz);

        // DCI P3 (D65)
        let p3 = ColorSpacePrimaries {
            rx: 0.680,
            ry: 0.320,
            gx: 0.265,
            gy: 0.690,
            bx: 0.150,
            by: 0.060,
            wx: 0.3127,
            wy: 0.3290,
        };
        let space = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3);
        let reference = space.expect("a valid color space").to_xyzd50();
        check_primaries(r, &p3, &reference);

        // Rec 2020 (D65)
        let rec2020 = ColorSpacePrimaries {
            rx: 0.708,
            ry: 0.292,
            gx: 0.170,
            gy: 0.797,
            bx: 0.131,
            by: 0.046,
            wx: 0.3127,
            wy: 0.3290,
        };
        let space = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::REC2020);
        let reference = space.expect("a valid color space").to_xyzd50();
        check_primaries(r, &rec2020, &reference);
    }
);

// Port of: tests/ColorSpaceTest.cpp#L306-L321 (chrome/m156)
def_test!(ColorSpace_MatrixHash, |r| {
    let srgb = ColorSpace::new_srgb();

    let fn_ = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: 3.0,
    };

    let strange = ColorSpace::new_rgb(&fn_, &named_gamut::SRGB);
    reporter_assert!(r, strange.is_some());
    let Some(strange) = strange else {
        return;
    };

    reporter_assert!(r, srgb.to_xyzd50_hash() == strange.to_xyzd50_hash());
});

// Port of: tests/ColorSpaceTest.cpp#L323-L338 (chrome/m156)
def_test!(ColorSpace_IsSRGB, |r| {
    let srgb0 = ColorSpace::new_srgb();

    let fn_ = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: 2.2,
    };
    let two_dot_two = ColorSpace::new_rgb(&fn_, &named_gamut::SRGB);
    reporter_assert!(r, two_dot_two.is_some());
    let Some(two_dot_two) = two_dot_two else {
        return;
    };

    reporter_assert!(r, srgb0.is_srgb());
    reporter_assert!(r, !two_dot_two.is_srgb());
});

// Port of: tests/ColorSpaceTest.cpp#L340-L343 (chrome/m156)
def_test!(ColorSpace_skcms_IsSRGB, |r| {
    let srgb = ColorSpace::make(skcms::srgb_profile());
    reporter_assert!(r, srgb.is_some());
    let Some(srgb) = srgb else {
        return;
    };
    reporter_assert!(r, srgb.is_srgb());
});

// Port of: tests/ColorSpaceTest.cpp#L345-L350 (chrome/m156)
def_test!(ColorSpace_skcms_sRGB_exact, |r| {
    let profile = srgb_singleton().to_profile();

    // The C++ is `0 == memcmp(&profile, skcms_sRGB_profile(), sizeof(skcms_ICCProfile))`.
    reporter_assert!(r, profile.bit_eq(skcms::srgb_profile()));
});

// Port of: tests/ColorSpaceTest.cpp#L352-L364 (chrome/m156)
def_test!(ColorSpace_classifyUnderflow, |r| {
    // crbug.com/1016183
    #[allow(clippy::cast_precision_loss)] // mirrors fn.g = INT_MIN
    let fn_ = TransferFunction {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: i32::MIN as f32,
    };
    let bad = ColorSpace::new_rgb(&fn_, &named_gamut::SRGB);
    reporter_assert!(r, bad.is_none());
});

// Port of: tests/ColorSpaceTest.cpp#L366-L382 (chrome/m156)
def_test!(ColorSpace_equiv, |r| {
    let tf = named_transfer_fn::SRGB;
    let mut gamut = named_gamut::SRGB;

    // Previously a NaN anywhere in the tf or gamut would trip up Equals(),
    // making us think we'd hit a hash collision where we hadn't.
    gamut.vals[1][1] = FLOAT_NAN;

    // There's a quick pointer comparison in SkColorSpace::Equals() we want to get past.
    let x = ColorSpace::new_rgb(&tf, &gamut);
    let y = ColorSpace::new_rgb(&tf, &gamut);
    reporter_assert!(r, x.is_some() && y.is_some());
    let (Some(x), Some(y)) = (x, y) else {
        return;
    };
    reporter_assert!(r, !x.ptr_eq(&y));

    // Most important to test in debug mode that we don't SkASSERT().
    reporter_assert!(r, ColorSpace::equals(Some(&x), Some(&y)));
});
