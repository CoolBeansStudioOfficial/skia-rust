// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/HdrMetadataTest.cpp (chrome/m156), the content light level and mastering display
// colour volume cases, and the adaptive global tone map (AGTM) cases, except the one that applies
// the tone map as a colour filter (see the manifest reason).
#![cfg(test)]
// The literals are kept verbatim from the C++ test so they can be checked against it, and the
// exact comparisons are the C++ `==` assertions.
#![allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::needless_range_loop,
    clippy::items_after_statements,
    clippy::struct_field_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::{
    ColorSpace, ColorSpacePrimaries, ColorSpaceTransferFn, named_gamut, named_primaries,
    named_transfer_fn,
};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::hdr_agtm::{
    AdaptiveGlobalToneMap, AlternateImage, ColorGainFunction, ComponentMixingFunction, GainCurve,
    GainCurveControlPoint, HeadroomAdaptiveToneMap, Metadata,
};
use skia_rust_core::hdr_agtm_helpers::{
    Weighting, apply_gain, compute_weighting, evaluate_gain_curve, get_gain_application_space,
    populate_slope_from_pchip, populate_tone_map_agtm_params, populate_using_rwtmo, validate,
};
use skia_rust_core::hdr_metadata::{ContentLightLevelInformation, MasteringDisplayColorVolume};
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_test, reporter_assert};

/// `SkScalarNearlyEqual(x, y, tolerance)`.
fn nearly_equal(x: scalar, y: scalar, tolerance: scalar) -> bool {
    <scalar as Scalar>::nearly_equal(x, y, tolerance)
}

/// `SkScalarNearlyEqual(x, y)` with the default tolerance.
fn nearly_equal_default(x: scalar, y: scalar) -> bool {
    <scalar as Scalar>::nearly_equal(x, y, None)
}

/// A control point with a zero slope, as in `{ .fX = x, .fY = y }`.
fn xy(x: f32, y: f32) -> GainCurveControlPoint {
    GainCurveControlPoint { x, y, m: 0.0 }
}

/// A control point, as in `{ .fX = x, .fY = y, .fM = m }`.
fn xym(x: f32, y: f32, m: f32) -> GainCurveControlPoint {
    GainCurveControlPoint { x, y, m }
}

/// A colour with the given components, as in `SkColor4f({r, g, b, a})`.
fn color4(r: f32, g: f32, b: f32, a: f32) -> Color4f {
    Color4f { r, g, b, a }
}

/// An alternate image with the given headroom and colour gain function.
fn alt(hdr_headroom: f32, color_gain_function: ColorGainFunction) -> AlternateImage {
    AlternateImage {
        hdr_headroom,
        color_gain_function,
    }
}

/// A colour gain function with the given component mixing and control points.
fn gain(
    component_mixing: ComponentMixingFunction,
    control_points: Vec<GainCurveControlPoint>,
) -> ColorGainFunction {
    ColorGainFunction {
        component_mixing,
        gain_curve: GainCurve { control_points },
    }
}

/// A component mixing function with only the given fields set (the others are zero).
fn mix(
    red: f32,
    green: f32,
    blue: f32,
    max: f32,
    min: f32,
    component: f32,
) -> ComponentMixingFunction {
    ComponentMixingFunction {
        red,
        green,
        blue,
        max,
        min,
        component,
    }
}

// Port of: tests/HdrMetadataTest.cpp#L20-L47 (chrome/m156), HdrMetadata_ParseSerialize_ContentLightLevelInformation
def_test!(
    HdrMetadata_ParseSerialize_ContentLightLevelInformation,
    |r| {
        let data = Data::new_copy(&[0x03, 0xE8, 0x00, 0xFA]);
        // Data taken from:
        // https://www.w3.org/TR/png-3/#example-13
        // https://www.w3.org/TR/png-3/#example-14
        let data_png = Data::new_copy(&[0x00, 0x98, 0x96, 0x80, 0x00, 0x26, 0x25, 0xA0]);
        let clli_expected = ContentLightLevelInformation {
            max_cll: 1000.0,
            max_fall: 250.0,
        };

        let clli = ContentLightLevelInformation::parse(&data);
        reporter_assert!(r, clli == Some(clli_expected));
        reporter_assert!(
            r,
            clli.is_some_and(|clli| data.equals(Some(&clli.serialize())))
        );

        let clli_png = ContentLightLevelInformation::parse_png_chunk(&data_png);
        reporter_assert!(r, clli_png == Some(clli_expected));
        reporter_assert!(
            r,
            clli.is_some_and(|clli| data_png.equals(Some(&clli.serialize_png_chunk())))
        );
    }
);

// Port of: tests/HdrMetadataTest.cpp#L49-L73 (chrome/m156), HdrMetadata_ParseSerialize_MasteringDisplayColorVolume
def_test!(
    HdrMetadata_ParseSerialize_MasteringDisplayColorVolume,
    |r| {
        // Data taken from:
        // https://www.w3.org/TR/png-3/#example-5
        // https://www.w3.org/TR/png-3/#example-6
        // https://www.w3.org/TR/png-3/#example-7
        // https://www.w3.org/TR/png-3/#example-8
        let data = Data::new_copy(&[
            0x8A, 0x48, 0x39, 0x08, // Red
            0x21, 0x34, 0x9B, 0xAA, // Green
            0x19, 0x96, 0x08, 0xFC, // Blue
            0x3D, 0x13, 0x40, 0x42, // White
            0x02, 0x62, 0x5A, 0x00, // Maximum luminance
            0x00, 0x00, 0x00, 0x05, // Minimum luminance
        ]);
        let mdcv_expected = MasteringDisplayColorVolume {
            display_primaries: ColorSpacePrimaries {
                rx: 0.708,
                ry: 0.292,
                gx: 0.17,
                gy: 0.797,
                bx: 0.131,
                by: 0.046,
                wx: 0.3127,
                wy: 0.329,
            },
            maximum_display_mastering_luminance: 4000.0,
            minimum_display_mastering_luminance: 0.0005,
        };

        let mdcv = MasteringDisplayColorVolume::parse(&data);
        reporter_assert!(r, mdcv.as_ref() == Some(&mdcv_expected));
        reporter_assert!(
            r,
            mdcv.is_some_and(|mdcv| data.equals(Some(&mdcv.serialize())))
        );
    }
);

// Port of: tests/HdrMetadataTest.cpp#L75-L110 (chrome/m156), HdrMetadata_Agtm_Cubic
def_test!(HdrMetadata_Agtm_Cubic, |r| {
    let mut cubic = GainCurve {
        control_points: vec![
            xy(0.10720647, 0.37384606),
            xy(0.76246667, 0.93143060),
            xy(1.39535723, 0.0),
            xy(2.17572099, 1.23009354),
            xy(2.47834070, 1.25542898),
            xy(3.14288223, 2.22460677),
            xy(3.35428070, 2.69226748),
            xy(4.24864607, 3.45838813),
            xy(4.59087493, 4.44597502),
            xy(4.80373641, 5.19196203),
        ],
    };
    populate_slope_from_pchip(&mut cubic);

    const M_EXPECTED: [f32; 10] = [
        2.03242568, 0.0, 0.0, 0.14042951, 0.14250506, 1.82245618, 1.35855757, 1.43703564,
        3.18918733, 3.74186390,
    ];
    for i in 0..10 {
        reporter_assert!(
            r,
            nearly_equal(cubic.control_points[i].m, M_EXPECTED[i], 0.0001)
        );
    }

    const Y_EXPECTED: [f32; 11] = [
        0.37384606, 0.86280187, 0.63630745, 0.05871820, 1.05625216, 1.26009455, 1.95243885,
        2.85680727, 3.19521825, 4.14318213, 5.13419092,
    ];
    for i in 0..11 {
        let x = i as f32 / 2.0;
        let y = evaluate_gain_curve(&cubic, x);
        reporter_assert!(r, nearly_equal(y, Y_EXPECTED[i], 0.0001));
    }
});

// Port of: tests/HdrMetadataTest.cpp#L112-L226 (chrome/m156), HdrMetadata_Agtm_PCHIP_EdgeCases
def_test!(HdrMetadata_Agtm_PCHIP_EdgeCases, |r| {
    struct SlopeTest {
        name: &'static str,
        curve: Vec<GainCurveControlPoint>,
        expected: Vec<(usize, f32)>,
    }

    let tests = [
        SlopeTest {
            name: "SinglePoint",
            curve: vec![xy(1.0, 1.0)],
            expected: vec![(0, 0.0)],
        },
        SlopeTest {
            name: "Left endpoint, three-point, sign change s0 vs m",
            curve: vec![xy(0.0, 0.0), xy(1.0, 1.0), xy(2.0, 5.0)],
            expected: vec![(0, 0.0)],
        },
        SlopeTest {
            name: "Left endpoint, three-point, sign change s1 vs m, |m| > 3|s0|",
            curve: vec![xy(0.0, 0.0), xy(1.0, 1.0), xy(2.0, -9.0)],
            expected: vec![(0, 3.0)],
        },
        SlopeTest {
            name: "Interior point, different signs",
            curve: vec![xy(0.0, 0.0), xy(1.0, 1.0), xy(2.0, 0.0)],
            expected: vec![(1, 0.0)],
        },
        SlopeTest {
            name: "Interior point, duplicate point left",
            curve: vec![xy(0.0, 0.0), xy(0.0, 0.0), xy(1.0, 2.0)],
            expected: vec![(1, 2.0)],
        },
        SlopeTest {
            name: "Interior point, duplicate point right",
            curve: vec![xy(0.0, 0.0), xy(1.0, 2.0), xy(1.0, 2.0)],
            expected: vec![(1, 2.0)],
        },
        SlopeTest {
            name: "Interior point, flat on both sides",
            curve: vec![xy(0.0, 2.0), xy(1.0, 2.0), xy(2.0, 2.0)],
            expected: vec![(1, 0.0)],
        },
        SlopeTest {
            name: "N=2, two-point difference",
            curve: vec![xy(0.0, 0.0), xy(2.0, 4.0)],
            expected: vec![(0, 2.0), (1, 2.0)],
        },
        SlopeTest {
            name: "h[i]=0 case",
            curve: vec![xy(0.0, 0.0), xy(0.0, 0.0), xy(1.0, 1.0)],
            expected: vec![(0, 0.0), (1, 1.0)],
        },
        SlopeTest {
            name: "Right endpoint, three-point, sign change s0 vs m",
            curve: vec![xy(0.0, 4.0), xy(1.0, 0.0), xy(2.0, -1.0)],
            expected: vec![(2, 0.0)],
        },
        SlopeTest {
            name: "Right endpoint, three-point, sign change s1 vs m, |m| > 3|s0|",
            curve: vec![xy(0.0, -4.0), xy(1.0, 0.0), xy(2.0, -1.0)],
            expected: vec![(2, -3.0)],
        },
    ];

    for t in tests {
        r.set_context(Some(t.name.to_owned()));
        let mut curve = GainCurve {
            control_points: t.curve,
        };
        populate_slope_from_pchip(&mut curve);
        for (index, expected_m) in t.expected {
            reporter_assert!(r, curve.control_points[index].m == expected_m);
        }
    }
    r.set_context(None);
});

// Port of: tests/HdrMetadataTest.cpp#L228-L281 (chrome/m156), HdrMetadata_Agtm_Mix
def_test!(HdrMetadata_Agtm_Mix, |r| {
    let test = |r: &mut Reporter,
                name: &str,
                mix: ComponentMixingFunction,
                input: Color4f,
                expected: Color4f| {
        r.set_context(Some(name.to_owned()));
        let actual =
            skia_rust_core::hdr_agtm_helpers::evaluate_component_mixing_function(&mix, &input);
        reporter_assert!(r, actual.r == expected.r);
        reporter_assert!(r, actual.g == expected.g);
        reporter_assert!(r, actual.b == expected.b);
        reporter_assert!(r, actual.a == expected.a);
        reporter_assert!(r, actual.a == input.a);
    };

    test(
        r,
        "Red only",
        mix(1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        color4(0.5, 0.75, 0.25, 1.0),
        color4(0.5, 0.5, 0.5, 1.0),
    );

    test(
        r,
        "Green only",
        mix(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        color4(0.75, 0.5, 0.25, 1.0),
        color4(0.5, 0.5, 0.5, 1.0),
    );

    test(
        r,
        "Blue only",
        mix(0.0, 0.0, 1.0, 0.0, 0.0, 0.0),
        color4(0.75, 0.25, 0.5, 1.0),
        color4(0.5, 0.5, 0.5, 1.0),
    );

    test(
        r,
        "Max only",
        mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        color4(0.75, 0.5, 0.25, 1.0),
        color4(0.75, 0.75, 0.75, 1.0),
    );

    test(
        r,
        "Min only",
        mix(0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        color4(0.75, 0.5, 0.25, 1.0),
        color4(0.25, 0.25, 0.25, 1.0),
    );

    test(
        r,
        "Component only",
        mix(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        color4(0.75, 0.5, 0.25, 1.0),
        color4(0.75, 0.5, 0.25, 1.0),
    );

    test(
        r,
        "CIE Y (luminance)",
        mix(0.2627, 0.6780, 0.0593, 0.0, 0.0, 0.0),
        color4(0.75, 0.5, 0.25, 0.125),
        color4(0.55085, 0.55085, 0.55085, 0.125),
    );

    test(
        r,
        "max-component",
        mix(0.0, 0.0, 0.0, 0.75, 0.0, 0.25),
        color4(0.75, 0.5, 0.25, 0.125),
        color4(0.75, 0.6875, 0.6250, 0.125),
    );
    r.set_context(None);
});

// Port of: tests/HdrMetadataTest.cpp#L283-L321 (chrome/m156), HdrMetadata_Agtm_RWTMO
def_test!(HdrMetadata_Agtm_RWTMO, |r| {
    let mut hatm = HeadroomAdaptiveToneMap {
        baseline_hdr_headroom: 1.0,
        ..Default::default()
    };
    populate_using_rwtmo(&mut hatm);

    reporter_assert!(
        r,
        hatm.gain_application_space_primaries == named_primaries::REC2020
    );
    reporter_assert!(r, hatm.alternate_images.len() == 2);
    reporter_assert!(r, hatm.alternate_images[0].hdr_headroom == 0.0);
    reporter_assert!(
        r,
        nearly_equal_default(hatm.alternate_images[1].hdr_headroom, 0.6151137835929048)
    );

    const X_EXPECTED: [[f32; 8]; 2] = [
        [
            1.00000, 1.06461, 1.15531, 1.27209, 1.41494, 1.58388, 1.77890, 2.00000,
        ],
        [
            1.00000, 1.10504, 1.22269, 1.35294, 1.49580, 1.65126, 1.81933, 2.00000,
        ],
    ];
    const Y_EXPECTED: [[f32; 8]; 2] = [
        [
            -0.35356, -0.37367, -0.42913, -0.51246, -0.61663, -0.73563, -0.86465, -1.00000,
        ],
        [
            0.00000, -0.01253, -0.04583, -0.09477, -0.15559, -0.22550, -0.30244, -0.38489,
        ],
    ];
    const M_EXPECTED: [[f32; 8]; 2] = [
        [
            0.00000, -0.50266, -0.68079, -0.73059, -0.72159, -0.68535, -0.63784, -0.58742,
        ],
        [
            0.00000, -0.21470, -0.33759, -0.40581, -0.44088, -0.45573, -0.45828, -0.45351,
        ],
    ];

    for a in 0..2 {
        let cubic = &hatm.alternate_images[a].color_gain_function.gain_curve;
        reporter_assert!(r, cubic.control_points.len() == 8);
        for c in 0..8 {
            reporter_assert!(
                r,
                nearly_equal_default(X_EXPECTED[a][c], cubic.control_points[c].x)
            );
            reporter_assert!(
                r,
                nearly_equal_default(Y_EXPECTED[a][c], cubic.control_points[c].y)
            );
            reporter_assert!(
                r,
                nearly_equal_default(M_EXPECTED[a][c], cubic.control_points[c].m)
            );
        }
    }
});

// Port of: tests/HdrMetadataTest.cpp#L323-L415 (chrome/m156), HdrMetadata_Agtm_Weighting
def_test!(HdrMetadata_Agtm_Weighting, |r| {
    const INV: u8 = Weighting::K_INVALID_INDEX;

    let test = |r: &mut Reporter,
                name: &str,
                hatm: &HeadroomAdaptiveToneMap,
                targeted_hdr_headroom: f32,
                w_expected: Weighting| {
        r.set_context(Some(name.to_owned()));
        let w = compute_weighting(hatm, targeted_hdr_headroom);
        reporter_assert!(r, w.weight[0] == w_expected.weight[0]);
        reporter_assert!(r, w.weight[1] == w_expected.weight[1]);
        reporter_assert!(
            r,
            w.alternate_image_index[0] == w_expected.alternate_image_index[0]
        );
        reporter_assert!(
            r,
            w.alternate_image_index[1] == w_expected.alternate_image_index[1]
        );
    };
    let w = |index: [u8; 2], weight: [f32; 2]| Weighting {
        alternate_image_index: index,
        weight,
    };

    // Tests with a single baseline representation.
    let mut hatm = HeadroomAdaptiveToneMap {
        baseline_hdr_headroom: 1.0,
        ..Default::default()
    };
    test(r, "base-1, target-0", &hatm, 0.0, w([INV, INV], [0.0, 0.0]));
    test(r, "base-1, target-1", &hatm, 1.0, w([INV, INV], [0.0, 0.0]));
    hatm.baseline_hdr_headroom = 2.0;
    test(r, "base-2, target-2", &hatm, 2.0, w([INV, INV], [0.0, 0.0]));

    // Tests with a baseline and an alternate representation.
    hatm.baseline_hdr_headroom = 1.0;
    hatm.alternate_images = vec![AlternateImage {
        hdr_headroom: 0.0,
        ..Default::default()
    }];

    test(
        r,
        "base-1-alt0, target-0",
        &hatm,
        0.0,
        w([0, INV], [1.0, 0.0]),
    );
    test(
        r,
        "base-1-alt0, target-0.25",
        &hatm,
        0.25,
        w([0, INV], [0.75, 0.0]),
    );
    test(
        r,
        "base-1-alt0, target-1",
        &hatm,
        1.0,
        w([INV, INV], [0.0, 0.0]),
    );
    test(
        r,
        "base-1-alt0, target-1.25",
        &hatm,
        1.25,
        w([INV, INV], [0.0, 0.0]),
    );

    // Two alternate representations.
    hatm.baseline_hdr_headroom = 1.0;
    hatm.alternate_images = vec![
        AlternateImage {
            hdr_headroom: 0.0,
            ..Default::default()
        },
        AlternateImage {
            hdr_headroom: 2.0,
            ..Default::default()
        },
    ];

    test(
        r,
        "base-1-alt0-alt2, target-0",
        &hatm,
        0.0,
        w([0, INV], [1.0, 0.0]),
    );
    test(
        r,
        "base-1-alt0-alt2, target-0.25",
        &hatm,
        0.25,
        w([0, INV], [0.75, 0.0]),
    );
    test(
        r,
        "base-1-alt0-alt2, target-1",
        &hatm,
        1.0,
        w([INV, INV], [0.0, 0.0]),
    );
    test(
        r,
        "base-1-alt0-alt2, target-1.25",
        &hatm,
        1.25,
        w([1, INV], [0.25, 0.0]),
    );
    test(
        r,
        "base-1-alt0-alt2, target-2",
        &hatm,
        2.0,
        w([1, INV], [1.0, 0.0]),
    );
    test(
        r,
        "base-1-alt0-alt2, target-3",
        &hatm,
        3.0,
        w([1, INV], [1.0, 0.0]),
    );

    // Two alternate representations again, now mix-able.
    hatm.baseline_hdr_headroom = 2.0;
    hatm.alternate_images = vec![
        AlternateImage {
            hdr_headroom: 0.0,
            ..Default::default()
        },
        AlternateImage {
            hdr_headroom: 1.0,
            ..Default::default()
        },
    ];

    test(
        r,
        "base-2-alt0-alt1, target-0.25",
        &hatm,
        0.25,
        w([0, 1], [0.75, 0.25]),
    );
    r.set_context(None);
});

/// Port of `assert_agtms_equal` in tests/HdrMetadataTest.cpp#L1205-L1329 (chrome/m156).
fn assert_agtms_equal(
    r: &mut Reporter,
    agtm_in: &AdaptiveGlobalToneMap,
    agtm_out: &AdaptiveGlobalToneMap,
) {
    // Allow error for headrooms, x, and y to twice the their encoding step.
    const HEADROOM_ERROR: f32 = 0.0002;
    const X_ERROR: f32 = 0.002;
    const Y_ERROR: f32 = 0.0002;
    // The slope is non-uniformly sampled. It has higher precision than the Y values near 0, but
    // lower precision at higher values.
    const M_ERROR: f32 = 0.0004;

    reporter_assert!(
        r,
        agtm_in.hdr_reference_white == agtm_out.hdr_reference_white
    );

    reporter_assert!(
        r,
        agtm_in.headroom_adaptive_tone_map.is_some()
            == agtm_out.headroom_adaptive_tone_map.is_some()
    );
    let (Some(hatm_in), Some(hatm_out)) = (
        &agtm_in.headroom_adaptive_tone_map,
        &agtm_out.headroom_adaptive_tone_map,
    ) else {
        return;
    };

    reporter_assert!(
        r,
        nearly_equal(
            hatm_in.baseline_hdr_headroom,
            hatm_out.baseline_hdr_headroom,
            HEADROOM_ERROR
        )
    );
    reporter_assert!(
        r,
        hatm_in.gain_application_space_primaries == hatm_out.gain_application_space_primaries
    );
    reporter_assert!(
        r,
        hatm_in.alternate_images.len() == hatm_out.alternate_images.len()
    );
    if hatm_in.alternate_images.len() != hatm_out.alternate_images.len() {
        return;
    }
    for (a, (alt_in, alt_out)) in hatm_in
        .alternate_images
        .iter()
        .zip(&hatm_out.alternate_images)
        .enumerate()
    {
        r.set_context(Some(format!("AlternateImage:a={a}")));

        reporter_assert!(
            r,
            nearly_equal(alt_in.hdr_headroom, alt_out.hdr_headroom, HEADROOM_ERROR)
        );

        let mix_in = &alt_in.color_gain_function.component_mixing;
        let mix_out = &alt_out.color_gain_function.component_mixing;

        reporter_assert!(r, mix_in.red == mix_out.red);
        reporter_assert!(r, mix_in.green == mix_out.green);
        reporter_assert!(r, mix_in.blue == mix_out.blue);
        reporter_assert!(r, mix_in.max == mix_out.max);
        reporter_assert!(r, mix_in.min == mix_out.min);
        reporter_assert!(r, mix_in.component == mix_out.component);

        let curve_in = &alt_in.color_gain_function.gain_curve;
        let curve_out = &alt_out.color_gain_function.gain_curve;
        reporter_assert!(
            r,
            curve_in.control_points.len() == curve_out.control_points.len()
        );
        if curve_in.control_points.len() != curve_out.control_points.len() {
            return;
        }
        for (c, (cp_in, cp_out)) in curve_in
            .control_points
            .iter()
            .zip(&curve_out.control_points)
            .enumerate()
        {
            r.set_context(Some(format!("ControlPoint:c={c}")));
            reporter_assert!(r, nearly_equal(cp_in.x, cp_out.x, X_ERROR));
            reporter_assert!(r, nearly_equal(cp_in.y, cp_out.y, Y_ERROR));
            reporter_assert!(r, nearly_equal(cp_in.m, cp_out.m, M_ERROR));
        }
    }
    r.set_context(None);
}

/// The RWTMO tone map with the given baseline headroom (`get_rwtmo` in the C++ test, which does
/// not use its reference white argument).
fn get_rwtmo(_hdr_reference_white: f32, headroom: f32) -> AdaptiveGlobalToneMap {
    let mut agtm = AdaptiveGlobalToneMap {
        headroom_adaptive_tone_map: Some(HeadroomAdaptiveToneMap {
            baseline_hdr_headroom: headroom,
            ..Default::default()
        }),
        ..Default::default()
    };
    if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
        populate_using_rwtmo(hatm);
    }
    agtm
}

/// One binary encoding of a test case, as `AdaptiveGlobalToneMap::Encoding` in the C++ test.
struct Encoding {
    name: Option<&'static str>,
    is_default_encoding: bool,
    data: Vec<u8>,
}

impl Encoding {
    /// The encoding that `serialize` produces.
    fn default_encoding(data: &[u8]) -> Self {
        Self {
            name: None,
            is_default_encoding: true,
            data: data.to_vec(),
        }
    }

    /// An encoding that parses to the same metadata but is not what `serialize` produces.
    fn other(name: &'static str, data: &[u8]) -> Self {
        Self {
            name: Some(name),
            is_default_encoding: false,
            data: data.to_vec(),
        }
    }
}

/// One round-trip case (`Test` in the C++ test).
struct RoundTripTest {
    name: &'static str,
    agtm: AdaptiveGlobalToneMap,
    encodings: Vec<Encoding>,
}

fn agtm_with(
    hdr_reference_white: f32,
    headroom: Option<HeadroomAdaptiveToneMap>,
) -> AdaptiveGlobalToneMap {
    AdaptiveGlobalToneMap {
        hdr_reference_white,
        headroom_adaptive_tone_map: headroom,
    }
}

// Port of: tests/HdrMetadataTest.cpp#L485-L932 (chrome/m156), HdrMetadata_Agtm_RoundTripSerialize
def_test!(HdrMetadata_Agtm_RoundTripSerialize, |r| {
    let tests = vec![
        RoundTripTest {
            name: "NoAdaptiveToneMap-DefaultWhite",
            agtm: AdaptiveGlobalToneMap::default(),
            encodings: vec![
                Encoding::default_encoding(&[0x00, 0x00]),
                // The application_version syntax element is 7. Because minimum_application_version
                // is 0, we should still parse this. See Clause C.2.1.
                Encoding::other("application_version=7", &[0xe0, 0x00]),
            ],
        },
        RoundTripTest {
            name: "NoAdaptiveToneMap-123-White",
            agtm: agtm_with(123.0, None),
            encodings: vec![Encoding::default_encoding(&[0x00, 0x80, 0x02, 0x67])],
        },
        RoundTripTest {
            name: "NoAdaptiveToneMap-MinWhite",
            agtm: agtm_with(0.2, None),
            encodings: vec![
                Encoding::default_encoding(&[0x00, 0x80, 0x00, 0x01]),
                // The hdr_reference_white syntax element is 0 but clamped to 1 by Clause C.3.3.
                Encoding::other("hdr_reference_white=0", &[0x00, 0x80, 0x00, 0x00]),
            ],
        },
        RoundTripTest {
            name: "NoAdaptiveToneMap-MaxWhite",
            agtm: agtm_with(10000.0, None),
            encodings: vec![
                Encoding::default_encoding(&[0x00, 0x80, 0xc3, 0x50]),
                // The hdr_reference_white syntax element is 65535 but clamped to 50000 by Clause
                // C.3.3.
                Encoding::other("hdr_reference_white=65535", &[0x00, 0x80, 0xff, 0xff]),
            ],
        },
        RoundTripTest {
            name: "RWTMO-min-headroom",
            agtm: get_rwtmo(203.0, 0.0),
            encodings: vec![Encoding::default_encoding(&[0x00, 0x40, 0x00, 0x00, 0x80])],
        },
        RoundTripTest {
            name: "RWTMO-mid-headroom",
            agtm: get_rwtmo(203.0, 3.0),
            encodings: vec![Encoding::default_encoding(&[0x00, 0x40, 0x75, 0x30, 0x80])],
        },
        RoundTripTest {
            name: "RWTMO-max-headroom",
            agtm: get_rwtmo(203.0, 6.0),
            encodings: vec![
                Encoding::default_encoding(&[0x00, 0x40, 0xea, 0x60, 0x80]),
                // The baseline_hdr_headroom syntax element is 65535 but clamped to 60000 by Clause
                // C.3.4.
                Encoding::other(
                    "baseline_hdr_headroom=65535",
                    &[0x00, 0x40, 0xff, 0xff, 0x80],
                ),
            ],
        },
        RoundTripTest {
            name: "ClampInRec601",
            agtm: agtm_with(
                100.0,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 2.0,
                    gain_application_space_primaries: named_primaries::REC601,
                    alternate_images: vec![],
                }),
            ),
            encodings: vec![Encoding::default_encoding(&[
                0x00, 0xc0, 0x01, 0xf4, 0x4e, 0x20, 0x0c, 0x7b, 0x0c, 0x42, 0x68, 0x3c, 0x8c, 0x74,
                0x36, 0x1e, 0x46, 0x0d, 0xac, 0x3d, 0x13, 0x40, 0x42,
            ])],
        },
        RoundTripTest {
            name: "OneAlternates",
            agtm: agtm_with(
                400.0,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 4.0,
                    gain_application_space_primaries: named_primaries::SMPTE_EG_432_1,
                    alternate_images: vec![alt(
                        0.0,
                        gain(
                            mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                            vec![xym(1.0, 0.0, 0.0), xym(16.0, -4.0, 0.0)],
                        ),
                    )],
                }),
            ),
            encodings: vec![Encoding::default_encoding(&[
                0x00, 0xc0, 0x07, 0xd0, 0x9c, 0x40, 0x14, 0x00, 0x00, 0x00, 0x08, 0x03, 0xe8, 0x3e,
                0x80, 0x00, 0x00, 0x9c, 0x40, 0x46, 0x50, 0x46, 0x50,
            ])],
        },
        RoundTripTest {
            name: "OneAlternates-MaxValues",
            agtm: agtm_with(
                AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 6.0,
                    gain_application_space_primaries: named_primaries::REC2020,
                    alternate_images: vec![alt(
                        0.0,
                        gain(
                            mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                            vec![xym(64.0, -6.0, 0.0)],
                        ),
                    )],
                }),
            ),
            encodings: vec![
                Encoding::default_encoding(&[
                    0x00, 0x40, 0xea, 0x60, 0x18, 0x00, 0x00, 0x00, 0x00, 0xfa, 0x00, 0xea, 0x60,
                    0x46, 0x50,
                ]),
                // The gain_curve_control_points_x and gain_curve_control_points_y syntax elements
                // are 65535 but are clamped to 64000 and 60000 by Clause C.3.7.
                Encoding::other(
                    "gain_curve_control_points_x/y=65535",
                    &[
                        0x00, 0x40, 0xea, 0x60, 0x18, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
                        0xff, 0x46, 0x50,
                    ],
                ),
            ],
        },
        RoundTripTest {
            name: "FourAlternates",
            agtm: agtm_with(
                400.0,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 2.0,
                    gain_application_space_primaries: named_primaries::SMPTE_EG_432_1,
                    alternate_images: vec![
                        alt(
                            0.0,
                            gain(
                                mix(0.0, 0.0, 0.0, 0.75, 0.25, 0.0),
                                vec![xym(0.0, -1.0, 0.0)],
                            ),
                        ),
                        alt(
                            1.0,
                            gain(
                                mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                                vec![
                                    xym(0.0, -1.0, 0.0),
                                    xym(1.0, -0.5, 0.1),
                                    xym(2.0, -0.4, 0.2),
                                    xym(3.0, -0.3, 0.3),
                                ],
                            ),
                        ),
                        alt(
                            3.0,
                            gain(
                                mix(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
                                vec![xym(0.0, 1.0, 0.0), xym(1.0, 0.5, 0.1)],
                            ),
                        ),
                        alt(
                            4.0,
                            gain(
                                mix(0.3, 0.6, 0.1, 0.0, 0.0, 0.0),
                                vec![
                                    xym(0.0, 1.0, 0.0),
                                    xym(1.0, 0.5, 0.1),
                                    xym(2.0, 0.4, 0.5),
                                    xym(2.0, 0.4, 0.7),
                                ],
                            ),
                        ),
                    ],
                }),
            ),
            encodings: vec![
                Encoding::default_encoding(&[
                    0x00, 0xc0, 0x07, 0xd0, 0x4e, 0x20, 0x44, 0x00, 0x00, 0xc6, 0x92, 0x7c, 0x30,
                    0xd4, 0x00, 0x00, 0x00, 0x27, 0x10, 0x46, 0x50, 0x27, 0x10, 0x00, 0x18, 0x00,
                    0x00, 0x03, 0xe8, 0x07, 0xd0, 0x0b, 0xb8, 0x27, 0x10, 0x13, 0x88, 0x0f, 0xa0,
                    0x0b, 0xb8, 0x46, 0x50, 0x4a, 0xc6, 0x4f, 0x26, 0x53, 0x5c, 0x75, 0x30, 0x40,
                    0x08, 0x00, 0x00, 0x03, 0xe8, 0x27, 0x10, 0x13, 0x88, 0x46, 0x50, 0x4a, 0xc6,
                    0x9c, 0x40, 0xf8, 0x3a, 0x98, 0x75, 0x30, 0x13, 0x88, 0x18, 0x00, 0x00, 0x03,
                    0xe8, 0x07, 0xd0, 0x07, 0xd0, 0x27, 0x10, 0x13, 0x88, 0x0f, 0xa0, 0x0f, 0xa0,
                    0x46, 0x50, 0x4a, 0xc6, 0x5b, 0x11, 0x61, 0xa6,
                ]),
                // The num_alternate_images syntax element is 7 but clamped to 4 by Clause C.3.4.
                // The difference from the above is the 0x74 element, which was previously 0x44.
                Encoding::other(
                    "num_alternate_images=7",
                    &[
                        0x00, 0xc0, 0x07, 0xd0, 0x4e, 0x20, 0x74, 0x00, 0x00, 0xc6, 0x92, 0x7c,
                        0x30, 0xd4, 0x00, 0x00, 0x00, 0x27, 0x10, 0x46, 0x50, 0x27, 0x10, 0x00,
                        0x18, 0x00, 0x00, 0x03, 0xe8, 0x07, 0xd0, 0x0b, 0xb8, 0x27, 0x10, 0x13,
                        0x88, 0x0f, 0xa0, 0x0b, 0xb8, 0x46, 0x50, 0x4a, 0xc6, 0x4f, 0x26, 0x53,
                        0x5c, 0x75, 0x30, 0x40, 0x08, 0x00, 0x00, 0x03, 0xe8, 0x27, 0x10, 0x13,
                        0x88, 0x46, 0x50, 0x4a, 0xc6, 0x9c, 0x40, 0xf8, 0x3a, 0x98, 0x75, 0x30,
                        0x13, 0x88, 0x18, 0x00, 0x00, 0x03, 0xe8, 0x07, 0xd0, 0x07, 0xd0, 0x27,
                        0x10, 0x13, 0x88, 0x0f, 0xa0, 0x0f, 0xa0, 0x46, 0x50, 0x4a, 0xc6, 0x5b,
                        0x11, 0x61, 0xa6,
                    ],
                ),
            ],
        },
        RoundTripTest {
            // This is an inverse tone map, computed using the script used to generate the figures
            // used in SMPTE ST 2094-50. This also tests using 32 control points (the maximum
            // allowable).
            name: "InverseToneMap",
            agtm: agtm_with(
                AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 0.0,
                    gain_application_space_primaries: named_primaries::SMPTE_EG_432_1,
                    alternate_images: vec![alt(
                        2.0,
                        gain(
                            mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                            vec![
                                xym(0.5, 0.9998566, 0.0015297),
                                xym(0.5208116, 1.0027409, 0.2734709),
                                xym(0.5413111, 1.0109166, 0.5211183),
                                xym(0.5614984, 1.0237336, 0.7456896),
                                xym(0.5813735, 1.0406077, 0.9495246),
                                xym(0.6009365, 1.0610152, 1.1343419),
                                xym(0.6201873, 1.0844893, 1.3021958),
                                xym(0.6391259, 1.1106143, 1.4547685),
                                xym(0.6577523, 1.1390220, 1.5937591),
                                xym(0.6760665, 1.1693869, 1.7207136),
                                xym(0.6940686, 1.2014223, 1.8370443),
                                xym(0.7117585, 1.2348758, 1.9440458),
                                xym(0.7291363, 1.2695263, 2.0429054),
                                xym(0.7462018, 1.3051801, 2.1347281),
                                xym(0.7629552, 1.3416678, 2.2204760),
                                xym(0.7793964, 1.3788421, 2.3010690),
                                xym(0.7955254, 1.4165743, 2.3773159),
                                xym(0.8113423, 1.4547529, 2.4499659),
                                xym(0.8268470, 1.4932810, 2.5197041),
                                xym(0.8420395, 1.5320748, 2.5871586),
                                xym(0.8569198, 1.5710618, 2.6529097),
                                xym(0.8714880, 1.6101797, 2.7174979),
                                xym(0.8857440, 1.6493748, 2.7814233),
                                xym(0.8996878, 1.6886011, 2.8451656),
                                xym(0.9133194, 1.7278194, 2.9091748),
                                xym(0.9266389, 1.7669963, 2.9738880),
                                xym(0.9396462, 1.8061036, 3.0397306),
                                xym(0.9523413, 1.8451175, 3.1071218),
                                xym(0.9647242, 1.8840181, 3.1764804),
                                xym(0.9767950, 1.9227892, 3.2482303),
                                xym(0.9885535, 1.9614173, 3.3228070),
                                xym(1.0000000, 1.9998918, 3.4006599),
                            ],
                        ),
                    )],
                }),
            ),
            encodings: vec![Encoding::default_encoding(&[
                0x00, 0x40, 0x00, 0x00, 0x14, 0x4e, 0x20, 0x00, 0xf8, 0x01, 0xf4, 0x02, 0x09, 0x02,
                0x1d, 0x02, 0x31, 0x02, 0x45, 0x02, 0x59, 0x02, 0x6c, 0x02, 0x7f, 0x02, 0x92, 0x02,
                0xa4, 0x02, 0xb6, 0x02, 0xc8, 0x02, 0xd9, 0x02, 0xea, 0x02, 0xfb, 0x03, 0x0b, 0x03,
                0x1c, 0x03, 0x2b, 0x03, 0x3b, 0x03, 0x4a, 0x03, 0x59, 0x03, 0x67, 0x03, 0x76, 0x03,
                0x84, 0x03, 0x91, 0x03, 0x9f, 0x03, 0xac, 0x03, 0xb8, 0x03, 0xc5, 0x03, 0xd1, 0x03,
                0xdd, 0x03, 0xe8, 0x27, 0x0f, 0x27, 0x2b, 0x27, 0x7d, 0x27, 0xfd, 0x28, 0xa6, 0x29,
                0x72, 0x2a, 0x5d, 0x2b, 0x62, 0x2c, 0x7e, 0x2d, 0xae, 0x2e, 0xee, 0x30, 0x3d, 0x31,
                0x97, 0x32, 0xfc, 0x34, 0x69, 0x35, 0xdc, 0x37, 0x56, 0x38, 0xd4, 0x3a, 0x55, 0x3b,
                0xd9, 0x3d, 0x5f, 0x3e, 0xe6, 0x40, 0x6e, 0x41, 0xf6, 0x43, 0x7e, 0x45, 0x06, 0x46,
                0x8d, 0x48, 0x13, 0x49, 0x98, 0x4b, 0x1c, 0x4c, 0x9e, 0x4e, 0x1f, 0x46, 0x62, 0x52,
                0x43, 0x5b, 0xd1, 0x62, 0xfe, 0x68, 0x4f, 0x6c, 0x48, 0x6f, 0x50, 0x71, 0xab, 0x73,
                0x8b, 0x75, 0x0f, 0x76, 0x50, 0x77, 0x5c, 0x78, 0x40, 0x79, 0x04, 0x79, 0xaf, 0x7a,
                0x46, 0x7a, 0xcd, 0x7b, 0x47, 0x7b, 0xb7, 0x7c, 0x1d, 0x7c, 0x7d, 0x7c, 0xd7, 0x7d,
                0x2d, 0x7d, 0x7f, 0x7d, 0xce, 0x7e, 0x1b, 0x7e, 0x66, 0x7e, 0xb0, 0x7e, 0xf9, 0x7f,
                0x42, 0x7f, 0x8a, 0x7f, 0xd3,
            ])],
        },
        RoundTripTest {
            name: "Mix-Normalization",
            agtm: agtm_with(
                AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
                Some(HeadroomAdaptiveToneMap {
                    baseline_hdr_headroom: 1.0,
                    gain_application_space_primaries: named_primaries::REC2020,
                    alternate_images: vec![alt(
                        0.0,
                        gain(
                            mix(0.0, 0.0, 0.0, 0.75, 0.0, 0.25),
                            vec![xym(1.0, -1.0, -0.5)],
                        ),
                    )],
                }),
            ),
            encodings: vec![Encoding::other(
                "Mix-Normalization",
                &[
                    0x00, 0x40, 0x27, 0x10, 0x18, 0x00, 0x00, 0xc5, 0x00, 0x03, 0x00, 0x01, 0x00,
                    0x03, 0xe8, 0x27, 0x10, 0x31, 0x8f,
                ],
            )],
        },
    ];

    for test in &tests {
        r.set_context(Some(test.name.to_owned()));

        // Serialize the `agtm` member, and verify the bits come out as expected.
        let serialized = test.agtm.serialize();
        reporter_assert!(r, serialized.is_some());

        for encoding in &test.encodings {
            r.set_context(Some(encoding.name.unwrap_or("default").to_owned()));

            // Parse `encoding.data`, and verify that parsing it matches `agtm`.
            let encoding_data = Data::new_copy(&encoding.data);
            let parsed = AdaptiveGlobalToneMap::parse(&encoding_data);
            reporter_assert!(r, parsed.is_some());
            assert_agtms_equal(r, &test.agtm, &parsed.unwrap_or_default());

            // If this is idempotent, then `encoding.data` should bit-equal `serialized`.
            reporter_assert!(
                r,
                encoding.is_default_encoding
                    == serialized
                        .as_ref()
                        .is_some_and(|s| s.equals(Some(&encoding_data)))
            );
        }
    }
    r.set_context(None);
});

// Port of: tests/HdrMetadataTest.cpp#L1205-L1329 (chrome/m156), HdrMetadata_Agtm_Invalid
def_test!(HdrMetadata_Agtm_Invalid, |r| {
    let agtm_baseline_0 = AdaptiveGlobalToneMap {
        headroom_adaptive_tone_map: Some(HeadroomAdaptiveToneMap {
            baseline_hdr_headroom: 0.0,
            gain_application_space_primaries: named_primaries::REC2020,
            alternate_images: vec![],
        }),
        ..Default::default()
    };
    let agtm_baseline_2 = AdaptiveGlobalToneMap {
        headroom_adaptive_tone_map: Some(HeadroomAdaptiveToneMap {
            baseline_hdr_headroom: 2.0,
            gain_application_space_primaries: named_primaries::REC2020,
            alternate_images: vec![],
        }),
        ..Default::default()
    };
    let max_one_gain = || gain(mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0), vec![xym(0.0, 0.0, 0.0)]);

    {
        r.set_context(Some("HdrReferenceWhite too low".to_owned()));
        let agtm = AdaptiveGlobalToneMap {
            hdr_reference_white: -1.0,
            ..Default::default()
        };
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("HdrReferenceWhite too high".to_owned()));
        let agtm = AdaptiveGlobalToneMap {
            hdr_reference_white: 10000.1,
            ..Default::default()
        };
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("BaselineHdrHeadroom too low".to_owned()));
        let agtm = agtm_with(
            AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
            Some(HeadroomAdaptiveToneMap {
                baseline_hdr_headroom: -0.01,
                ..Default::default()
            }),
        );
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("BaselineHdrHeadroom too high".to_owned()));
        let agtm = agtm_with(
            AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
            Some(HeadroomAdaptiveToneMap {
                baseline_hdr_headroom: 6.01,
                ..Default::default()
            }),
        );
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("Equal headrooms".to_owned()));
        let mut agtm = agtm_baseline_0.clone();
        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images = vec![alt(0.0, max_one_gain())];
        }
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("Non-monotone headrooms".to_owned()));
        let mut agtm = agtm_baseline_0.clone();
        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images = vec![alt(1.0, max_one_gain()), alt(2.0, max_one_gain())];
        }
        reporter_assert!(r, validate(&agtm));
        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images.swap(0, 1);
        }
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("Non-monotone X".to_owned()));
        let mut agtm = agtm_baseline_2.clone();
        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images = vec![alt(
                0.0,
                gain(
                    mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                    vec![xym(4.0, -2.0, 0.0), xym(0.0, 0.0, 0.0)],
                ),
            )];
        }
        reporter_assert!(r, !validate(&agtm));
    }
    {
        r.set_context(Some("Discontinuous Y".to_owned()));
        let mut agtm = agtm_baseline_2.clone();
        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images = vec![alt(
                0.0,
                gain(
                    mix(0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                    vec![
                        xym(0.0, 0.0, 0.0),
                        xym(2.0, -1.0, 0.0),
                        xym(2.0, -1.0, 0.0),
                        xym(4.0, -2.0, 0.0),
                    ],
                ),
            )];
        }
        reporter_assert!(r, validate(&agtm));

        if let Some(hatm) = agtm.headroom_adaptive_tone_map.as_mut() {
            hatm.alternate_images[0]
                .color_gain_function
                .gain_curve
                .control_points[2]
                .y = -2.0;
        }
        reporter_assert!(r, !validate(&agtm));
    }
    r.set_context(None);
});

// Port of: tests/HdrMetadataTest.cpp#L935-L1118 (chrome/m156), HdrMetadata_Agtm_Apply_and_Shader
def_test!(HdrMetadata_Agtm_Apply_and_Shader, |r| {
    // This will tone map several input colors to different targeted HDR headrooms using this
    // RWTMO metadata.
    let mut hatm = HeadroomAdaptiveToneMap {
        baseline_hdr_headroom: 2.0,
        ..Default::default()
    };
    populate_using_rwtmo(&mut hatm);

    // We will use the following input pixel values in gain application color space. These include
    // monochrome and non-monochrome values, as well as values that are less than white (less than
    // 1) and brighter than white (greater than 1).
    const NUM_TEST_COLORS: usize = 6;
    let input_test_colors: [Color4f; NUM_TEST_COLORS] = [
        color4(1.00, 1.00, 1.00, 1.0),
        color4(1.00, 0.50, 0.25, 1.0),
        color4(4.00, 4.00, 4.00, 1.0),
        color4(1.00, 2.00, 4.00, 1.0),
        color4(0.50, 0.50, 0.50, 1.0),
        color4(2.00, 2.00, 2.00, 1.0),
    ];

    // We will test applying the gain for the following targetd HDR headroom values.
    const NUM_TESTS: usize = 5;
    let test_targeted_hdr_headrooms: [f32; NUM_TESTS] = [
        0.0,
        1.0,
        hatm.alternate_images[1].hdr_headroom,
        3.0f32.log2(),
        2.0,
    ];

    // These are the expected output pixel values for each of the targted HDR headrooms.
    let expected_test_colors: [[Color4f; NUM_TEST_COLORS]; NUM_TESTS] = [
        [
            color4(0.565302, 0.565302, 0.565302, 1.0),
            color4(0.565302, 0.282651, 0.141326, 1.0),
            color4(1.000000, 1.000000, 1.000000, 1.0),
            color4(0.250000, 0.500000, 1.000000, 1.0),
            color4(0.282651, 0.282651, 0.282651, 1.0),
            color4(0.815278, 0.815278, 0.815278, 1.0),
        ],
        [
            color4(0.898755, 0.898755, 0.898755, 1.0),
            color4(0.898755, 0.449377, 0.224689, 1.0),
            color4(2.000000, 2.000000, 2.000000, 1.0),
            color4(0.500000, 1.000000, 2.000000, 1.0),
            color4(0.449377, 0.449377, 0.449377, 1.0),
            color4(1.471569, 1.471569, 1.471569, 1.0),
        ],
        [
            color4(1.000000, 1.000000, 1.000000, 1.0),
            color4(1.000000, 0.500000, 0.250000, 1.0),
            color4(2.346040, 2.346040, 2.346040, 1.0),
            color4(0.586510, 1.173020, 2.346040, 1.0),
            color4(0.500000, 0.500000, 0.500000, 1.0),
            color4(1.685886, 1.685886, 1.685886, 1.0),
        ],
        [
            color4(1.000000, 1.000000, 1.000000, 1.0),
            color4(1.000000, 0.500000, 0.250000, 1.0),
            color4(3.000000, 3.000000, 3.000000, 1.0),
            color4(0.750000, 1.500000, 3.000000, 1.0),
            color4(0.500000, 0.500000, 0.500000, 1.0),
            color4(1.823991, 1.823991, 1.823991, 1.0),
        ],
        [
            color4(1.00, 1.00, 1.00, 1.0),
            color4(1.00, 0.50, 0.25, 1.0),
            color4(4.00, 4.00, 4.00, 1.0),
            color4(1.00, 2.00, 4.00, 1.0),
            color4(0.50, 0.50, 0.50, 1.0),
            color4(2.00, 2.00, 2.00, 1.0),
        ],
    ];

    // All of the math is done with at least half-precision. Given the range of values we are in
    // (not far from 1), we should maintain at least ten bit precision.
    const EPSILON: f32 = 1.0 / 1024.0;

    // Test the AgtmHelpers::ApplyGain function.
    for t in 0..NUM_TESTS {
        let targeted_hdr_headroom = test_targeted_hdr_headrooms[t];
        r.set_context(Some(format!(
            "AgtmHelpers::ApplyGain, targetedHdrHeadroom:{targeted_hdr_headroom:.6}"
        )));

        // Copy the inputTextColors to outputTestColors (because ApplyGain works in-place).
        let mut output_test_colors = input_test_colors;

        // Apply the tone mapping gain in-place on outputTestColors.
        apply_gain(&hatm, &mut output_test_colors, targeted_hdr_headroom);

        // Verify the result matches expectations.
        for i in 0..NUM_TEST_COLORS {
            let output = &output_test_colors[i];
            let expected = &expected_test_colors[t][i];

            reporter_assert!(r, nearly_equal(output.r, expected.r, EPSILON));
            reporter_assert!(r, nearly_equal(output.g, expected.g, EPSILON));
            reporter_assert!(r, nearly_equal(output.b, expected.b, EPSILON));
            reporter_assert!(r, nearly_equal(output.a, expected.a, EPSILON));
        }
    }

    // Test using an SkColorFilter from skhdr::Metadata.
    let gain_space = get_gain_application_space(&hatm).expect("gain application space");
    for t in 0..NUM_TESTS {
        let targeted_hdr_headroom = test_targeted_hdr_headrooms[t];
        r.set_context(Some(format!(
            "skhdr::Metadata::makeToneMapColorFilter, targetedHdrHeadroom:{targeted_hdr_headroom:.6}"
        )));

        // The input and output images will be kNumTestColors-by-1.
        let info = ImageInfo::new(
            (NUM_TEST_COLORS as i32, 1),
            ColorType::RGBAF32,
            AlphaType::Premul,
            gain_space.clone(),
        );
        let row_bytes = info.min_row_bytes();

        // Create an SkImage that references the inputTestColors array directly.
        let input_bytes: Vec<u8> = input_test_colors
            .iter()
            .flat_map(|c| [c.r, c.g, c.b, c.a].into_iter().flat_map(f32::to_ne_bytes))
            .collect();
        let input_image =
            images::raster_from_data(&info, Data::new_from_vec(input_bytes), row_bytes)
                .expect("input image");

        const NUM_INPUT_IMAGES: usize = 3;
        let input_image_names: [&str; NUM_INPUT_IMAGES] = ["linear", "pq", "pq-100"];
        let pq = ColorSpace::new_rgb(&named_transfer_fn::PQ, &named_gamut::REC2020)
            .expect("PQ colour space");
        let pq100 =
            ColorSpace::new_rgb(&ColorSpaceTransferFn::make_pq(100.0), &named_gamut::REC2020)
                .expect("PQ 100 colour space");
        let image_pq = input_image
            .make_color_space(pq, RequiredProperties::default())
            .expect("pq image");
        let image_pq100 = image_pq
            .reinterpret_color_space(pq100)
            .expect("pq-100 image");
        let input_images = [input_image, image_pq, image_pq100];

        for s in 0..NUM_INPUT_IMAGES {
            r.set_context(Some(format!("inputImage:{}", input_image_names[s])));

            // Create an output bitmap to draw into.
            let mut storage = vec![0u8; info.compute_byte_size(row_bytes)];

            // Call drawImage, using the color filter created by Agtm::makeColorFilter.
            {
                let mut metadata = Metadata::make_empty();
                metadata.set_adaptive_global_tone_map(AdaptiveGlobalToneMap {
                    hdr_reference_white: AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE,
                    headroom_adaptive_tone_map: Some(hatm.clone()),
                });
                let color_filter = metadata.make_tone_map_color_filter(
                    targeted_hdr_headroom,
                    input_images[s].color_space().as_ref(),
                );
                reporter_assert!(r, color_filter.is_some());

                let mut paint = Paint::default();
                paint.set_color_filter(color_filter);
                let canvas = Canvas::from_raster_direct(&info, &mut storage, row_bytes, None);
                let canvas = canvas.expect("raster canvas");
                canvas.draw_image_with_sampling_options(
                    &input_images[s],
                    (0, 0),
                    SamplingOptions::default(),
                    Some(&paint),
                );
            }

            // Verify that the pixels written into the bitmap match the expected values.
            for i in 0..NUM_TEST_COLORS {
                // There is more error in the PQ transfer function.
                const LOOSER_EPSILON: f32 = 1.0 / 100.0;

                let offset = i * 16;
                let read = |k: usize| {
                    let at = offset + 4 * k;
                    f32::from_ne_bytes([
                        storage[at],
                        storage[at + 1],
                        storage[at + 2],
                        storage[at + 3],
                    ])
                };
                let output = color4(read(0), read(1), read(2), read(3));
                let expected = &expected_test_colors[t][i];
                reporter_assert!(r, nearly_equal(output.r, expected.r, LOOSER_EPSILON));
                reporter_assert!(r, nearly_equal(output.g, expected.g, LOOSER_EPSILON));
                reporter_assert!(r, nearly_equal(output.b, expected.b, LOOSER_EPSILON));
                reporter_assert!(r, nearly_equal(output.a, expected.a, LOOSER_EPSILON));
            }
        }
    }
    r.set_context(None);
});

// Port of: tests/HdrMetadataTest.cpp#L1120-L1203 (chrome/m156), HdrMetadata_ShaderParams
def_test!(HdrMetadata_ShaderParams, |r| {
    let cs_srgb = ColorSpace::new_srgb();
    let cs_pq = ColorSpace::new_rgb(&named_transfer_fn::PQ, &named_gamut::REC2020)
        .expect("PQ Rec2020 colour space");
    let cs_pq100 =
        ColorSpace::new_rgb(&ColorSpaceTransferFn::make_pq(100.0), &named_gamut::REC2020)
            .expect("PQ 100 nit Rec2020 colour space");

    // Start with CLLI and MDCV metadata.
    let mut metadata = Metadata::make_empty();
    {
        let clli = ContentLightLevelInformation {
            max_cll: 406.0,
            ..Default::default()
        };
        metadata.set_content_light_level_information(clli);

        let mdcv = MasteringDisplayColorVolume {
            maximum_display_mastering_luminance: 812.0,
            ..Default::default()
        };
        metadata.set_mastering_display_color_volume(mdcv);
    }

    // Because this has no AGTM metadata, SDR inputs get no tone mapping shader.
    reporter_assert!(
        r,
        populate_tone_map_agtm_params(&metadata, Some(&cs_srgb)).is_none()
    );

    // This will have headroom log2(406/203)=1 for PQ.
    let (tone_map_agtm, scale_factor) =
        populate_tone_map_agtm_params(&metadata, Some(&cs_pq)).expect("PQ tone maps");
    reporter_assert!(r, scale_factor == 1.0);
    reporter_assert!(
        r,
        tone_map_agtm
            .headroom_adaptive_tone_map
            .as_ref()
            .is_some_and(|h| h.baseline_hdr_headroom == 1.0)
    );

    // This will have headroom log2(406/100) for PQ with 100 nit white.
    let (tone_map_agtm, scale_factor) =
        populate_tone_map_agtm_params(&metadata, Some(&cs_pq100)).expect("PQ 100 tone maps");
    reporter_assert!(r, scale_factor == 1.0);
    reporter_assert!(
        r,
        tone_map_agtm
            .headroom_adaptive_tone_map
            .as_ref()
            .is_some_and(|h| h.baseline_hdr_headroom == (4.06f32).log2())
    );

    // Invalidate the CLLI metadata.
    {
        let clli = ContentLightLevelInformation {
            max_cll: 0.0,
            ..Default::default()
        };
        metadata.set_content_light_level_information(clli);
    }

    // This will now have headroom log2(812/203)=2 for PQ.
    let (tone_map_agtm, scale_factor) =
        populate_tone_map_agtm_params(&metadata, Some(&cs_pq)).expect("PQ tone maps");
    reporter_assert!(r, scale_factor == 1.0);
    reporter_assert!(
        r,
        tone_map_agtm
            .headroom_adaptive_tone_map
            .as_ref()
            .is_some_and(|h| h.baseline_hdr_headroom == 2.0)
    );

    // Set AGTM metadata with just white level set.
    {
        let agtm = AdaptiveGlobalToneMap {
            hdr_reference_white: 100.0,
            ..Default::default()
        };
        metadata.set_adaptive_global_tone_map(agtm);
    }

    // PQ input at 203 nits will be scaled now.
    let (tone_map_agtm, scale_factor) =
        populate_tone_map_agtm_params(&metadata, Some(&cs_pq)).expect("PQ tone maps");
    reporter_assert!(r, scale_factor == 203.0 / 100.0);
    reporter_assert!(
        r,
        tone_map_agtm
            .headroom_adaptive_tone_map
            .as_ref()
            .is_some_and(|h| h.baseline_hdr_headroom == (812.0f32 / 100.0).log2())
    );

    // PQ input at 100 nits will not be scaled now.
    let (tone_map_agtm, scale_factor) =
        populate_tone_map_agtm_params(&metadata, Some(&cs_pq100)).expect("PQ 100 tone maps");
    reporter_assert!(r, scale_factor == 1.0);
    reporter_assert!(
        r,
        tone_map_agtm
            .headroom_adaptive_tone_map
            .as_ref()
            .is_some_and(|h| h.baseline_hdr_headroom == (812.0f32 / 100.0).log2())
    );
});
