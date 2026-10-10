// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkColorSpaceXformStepsTest.cpp (chrome/m156)
//
// The Ganesh variant (`SkColorSpaceXform_Ganesh`) needs that backend and is excluded.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_raster::surfaces;
use skia_rust_skcms::{Matrix3x3, TransferFunction};

use crate::tools::test_surface::{GraphiteTestSurface, TestSurface};
use crate::{Reporter, def_graphite_adapter_test, def_test, errorf, reporter_assert};

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L40-L44 (chrome/m156)
fn trfn_pq_100() -> TransferFunction {
    TransferFunction::make_pq(100.0)
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L46-L50 (chrome/m156)
fn trfn_pq_203() -> TransferFunction {
    TransferFunction::make_pq(203.0)
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L52-L56 (chrome/m156)
fn trfn_hlg_12x() -> TransferFunction {
    TransferFunction::make_hlg(1.0, 12.0, 1.0)
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L58-L62 (chrome/m156)
fn trfn_hlg_10a() -> TransferFunction {
    TransferFunction::make_hlg(100.0, 1000.0, 1.2)
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L64-L68 (chrome/m156)
fn trfn_hlg_10b() -> TransferFunction {
    TransferFunction::make_hlg(10.0, 100.0, 1.2)
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L70-L74 (chrome/m156)
fn trfn_hlg_203() -> TransferFunction {
    TransferFunction::make_hlg(203.0, 1000.0, 1.2)
}

// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L76-L86 (chrome/m156)
fn rgba_close(expected: &[f32; 4], actual: &[f32; 4]) -> bool {
    // Allow 1% relative error.
    const K_EPSILON: f32 = 0.01;
    const K_MIN_DENOM: f32 = 0.001;
    (expected[0] - actual[0]).abs() / std_max(expected[0], K_MIN_DENOM) < K_EPSILON
        && (expected[1] - actual[1]).abs() / std_max(expected[1], K_MIN_DENOM) < K_EPSILON
        && (expected[2] - actual[2]).abs() / std_max(expected[2], K_MIN_DENOM) < K_EPSILON
        && (expected[3] - actual[3]).abs() / std_max(expected[3], K_MIN_DENOM) < K_EPSILON
}

fn make_rgb(trfn: &TransferFunction, gamut: &Matrix3x3) -> ColorSpace {
    ColorSpace::new_rgb(trfn, gamut).expect("a valid color space")
}

// One row of the table in `SkColorSpaceXformSteps`: a source and destination, and the steps
// that are expected.
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ struct of expected flags
struct Test {
    src: ColorSpace,
    dst: ColorSpace,
    src_at: AlphaType,
    dst_at: AlphaType,

    unpremul: bool,
    linearize: bool,
    gamut_transform: bool,
    encode: bool,
    premul: bool,
    src_ootf: bool,
    dst_ootf: bool,
}

// A row that lists the five flags that don't involve the HLG OOTF (the OOTF flags default to
// false).
#[allow(clippy::too_many_arguments)] // mirrors the C++ aggregate initializer
#[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ aggregate initializer
fn t5(
    src: &ColorSpace,
    dst: &ColorSpace,
    src_at: AlphaType,
    dst_at: AlphaType,
    unpremul: bool,
    linearize: bool,
    gamut_transform: bool,
    encode: bool,
    premul: bool,
) -> Test {
    Test {
        src: src.clone(),
        dst: dst.clone(),
        src_at,
        dst_at,
        unpremul,
        linearize,
        gamut_transform,
        encode,
        premul,
        src_ootf: false,
        dst_ootf: false,
    }
}

// A row that lists all seven flags.
#[allow(clippy::too_many_arguments)] // mirrors the C++ aggregate initializer
#[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ aggregate initializer
fn t7(
    src: &ColorSpace,
    dst: &ColorSpace,
    src_at: AlphaType,
    dst_at: AlphaType,
    unpremul: bool,
    linearize: bool,
    gamut_transform: bool,
    encode: bool,
    premul: bool,
    src_ootf: bool,
    dst_ootf: bool,
) -> Test {
    Test {
        src_ootf,
        dst_ootf,
        ..t5(
            src,
            dst,
            src_at,
            dst_at,
            unpremul,
            linearize,
            gamut_transform,
            encode,
            premul,
        )
    }
}

// Port of: tests/SkColorSpaceXformStepsTest.cpp#L83-L266 (chrome/m156)
def_test!(
    #[allow(clippy::similar_names)] // mirrors the C++ variable names
    #[allow(clippy::too_many_lines)] // mirrors the C++ table
    SkColorSpaceXformSteps,
    |r| {
        let srgb = ColorSpace::new_srgb();
        let adobe = make_rgb(&named_transfer_fn::DOT22, &named_gamut::ADOBE_RGB);
        let srgb22 = make_rgb(&named_transfer_fn::DOT22, &named_gamut::SRGB);
        let srgb1 = srgb.with_linear_gamma();
        let adobe1 = adobe.with_linear_gamma();
        let rec2020_pq_203 = make_rgb(&trfn_pq_203(), &named_gamut::REC2020);
        let rec2020_pq_100 = make_rgb(&trfn_pq_100(), &named_gamut::REC2020);
        let p3_pq_203 = make_rgb(&trfn_pq_203(), &named_gamut::DISPLAY_P3);
        let rec2020_hlg_12x = make_rgb(&trfn_hlg_12x(), &named_gamut::REC2020);
        let rec2020_hlg_10a = make_rgb(&trfn_hlg_10a(), &named_gamut::REC2020);
        let rec2020_hlg_10b = make_rgb(&trfn_hlg_10b(), &named_gamut::REC2020);
        let rec2020_hlg_203 = make_rgb(&trfn_hlg_203(), &named_gamut::REC2020);

        let premul = AlphaType::Premul;
        let opaque = AlphaType::Opaque;
        let unpremul = AlphaType::Unpremul;

        let (s, a, s22, s1, a1) = (&srgb, &adobe, &srgb22, &srgb1, &adobe1);
        let tests = [
            // The general case is converting between two color spaces with different gamuts
            // and different transfer functions.  There's no optimization possible here.
            t5(
                a, s, premul, premul,
                true, // src is encoded as f(s)*a,a, so we unpremul to f(s),a before linearizing.
                true, // linearize to s,a
                true, // transform s to dst gamut, s'
                true, // encode with dst transfer function, g(s'), a
                true, // premul to g(s')*a, a
            ),
            // All the same going the other direction.
            t5(s, a, premul, premul, true, true, true, true, true),
            // If the src alpha type is unpremul, we'll not need that initial unpremul step.
            t5(a, s, unpremul, premul, false, true, true, true, true),
            t5(s, a, unpremul, premul, false, true, true, true, true),
            // If opaque, we need neither the initial unpremul, nor the premul later.
            t5(a, s, opaque, premul, false, true, true, true, false),
            t5(s, a, opaque, premul, false, true, true, true, false),
            // Now let's go between sRGB and sRGB with a 2.2 gamma, the gamut staying the same.
            t5(
                s, s22, premul, premul, true,  // we need to linearize, so we need to unpremul
                true,  // we need to encode to 2.2 gamma, so we need to get linear
                false, // no need to change gamut
                true,  // linear -> gamma 2.2
                true,  // premul going into the blend
            ),
            // Same sort of logic in the other direction.
            t5(s22, s, premul, premul, true, true, false, true, true),
            // As in the general case, when we change the alpha type unpremul and premul steps drop out.
            t5(s, s22, unpremul, premul, false, true, false, true, true),
            t5(s22, s, unpremul, premul, false, true, false, true, true),
            t5(s, s22, opaque, premul, false, true, false, true, false),
            t5(s22, s, opaque, premul, false, true, false, true, false),
            // Let's look at the special case of completely matching color spaces.
            // We should be ready to go into the blend without any fuss.
            t5(s, s, premul, premul, false, false, false, false, false),
            t5(s, s, unpremul, premul, false, false, false, false, true),
            t5(s, s, opaque, premul, false, false, false, false, false),
            // We can drop out the linearize step when the source is already linear.
            t5(s1, a, premul, premul, true, false, true, true, true),
            t5(s1, s, premul, premul, true, false, false, true, true),
            // And we can drop the encode step when the destination is linear.
            t5(a, s1, premul, premul, true, true, true, false, true),
            t5(s, s1, premul, premul, true, true, false, false, true),
            // Here's an interesting case where only gamut transform is needed.
            t5(a1, s1, premul, premul, false, false, true, false, false),
            t5(a1, s1, opaque, premul, false, false, true, false, false),
            t5(a1, s1, unpremul, premul, false, false, true, false, true),
            // Just finishing up with something to produce each other possible output.
            // Nothing terribly interesting in these eight.
            t5(s, s1, opaque, premul, false, true, false, false, false),
            t5(s, s1, unpremul, premul, false, true, false, false, true),
            t5(s, a1, opaque, premul, false, true, true, false, false),
            t5(s, a1, unpremul, premul, false, true, true, false, true),
            t5(s1, s, opaque, premul, false, false, false, true, false),
            t5(s1, s, unpremul, premul, false, false, false, true, true),
            t5(s1, a, opaque, premul, false, false, true, true, false),
            t5(s1, a, unpremul, premul, false, false, true, true, true),
            // Now test non-premul outputs.
            t5(s, s, premul, unpremul, true, false, false, false, false),
            t5(s, s1, premul, unpremul, true, true, false, false, false),
            t5(s1, a1, premul, unpremul, true, false, true, false, false),
            t5(s, a1, premul, unpremul, true, true, true, false, false),
            t5(s1, s, premul, unpremul, true, false, false, true, false),
            t5(s, s22, premul, unpremul, true, true, false, true, false),
            t5(s1, a, premul, unpremul, true, false, true, true, false),
            t5(s, a, premul, unpremul, true, true, true, true, false),
            // Opaque outputs are treated as the same alpha type as the source input.
            // TODO: we'd really like to have a good way of explaining why we think this is useful.
            t5(s, s, premul, opaque, false, false, false, false, false),
            t5(s, s1, premul, opaque, true, true, false, false, true),
            t5(s1, a1, premul, opaque, false, false, true, false, false),
            t5(s, a1, premul, opaque, true, true, true, false, true),
            t5(s1, s, premul, opaque, true, false, false, true, true),
            t5(s, s22, premul, opaque, true, true, false, true, true),
            t5(s1, a, premul, opaque, true, false, true, true, true),
            t5(s, a, premul, opaque, true, true, true, true, true),
            t5(s, s, unpremul, opaque, false, false, false, false, false),
            t5(s, s1, unpremul, opaque, false, true, false, false, false),
            t5(s1, a1, unpremul, opaque, false, false, true, false, false),
            t5(s, a1, unpremul, opaque, false, true, true, false, false),
            t5(s1, s, unpremul, opaque, false, false, false, true, false),
            t5(s, s22, unpremul, opaque, false, true, false, true, false),
            t5(s1, a, unpremul, opaque, false, false, true, true, false),
            t5(s, a, unpremul, opaque, false, true, true, true, false),
            t5(
                &rec2020_pq_203,
                s,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
            ),
            t5(
                &rec2020_pq_203,
                &rec2020_pq_203,
                premul,
                premul,
                false,
                false,
                false,
                false,
                false,
            ),
            t5(
                &rec2020_pq_203,
                &rec2020_pq_100,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
            ),
            t5(
                &rec2020_pq_203,
                &p3_pq_203,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
            ),
            t7(
                &rec2020_hlg_203,
                s,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
                true,
                false,
            ),
            t7(
                &rec2020_hlg_12x,
                s,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
                false,
                false,
            ),
            t7(
                s,
                &rec2020_hlg_12x,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
                false,
                false,
            ),
            t7(
                &rec2020_hlg_203,
                &rec2020_pq_203,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
                true,
                false,
            ),
            t7(
                &rec2020_hlg_10a,
                &rec2020_hlg_10b,
                premul,
                premul,
                true,
                true,
                false,
                true,
                true,
                false,
                false,
            ),
            t7(
                &rec2020_hlg_203,
                &rec2020_hlg_203,
                premul,
                premul,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
            ),
            t7(
                &rec2020_hlg_203,
                &rec2020_hlg_12x,
                premul,
                premul,
                true,
                true,
                true,
                true,
                true,
                true,
                false,
            ),
        ];

        let mut tested: u32 = 0x0000_0000;
        for t in &tests {
            let steps = ColorSpaceXformSteps::new(Some(&t.src), t.src_at, Some(&t.dst), t.dst_at);
            reporter_assert!(r, steps.flags.unpremul == t.unpremul);
            reporter_assert!(r, steps.flags.linearize == t.linearize);
            reporter_assert!(r, steps.flags.gamut_transform == t.gamut_transform);
            reporter_assert!(r, steps.flags.encode == t.encode);
            reporter_assert!(r, steps.flags.premul == t.premul);
            reporter_assert!(r, steps.flags.src_ootf == t.src_ootf);
            reporter_assert!(r, steps.flags.dst_ootf == t.dst_ootf);

            let bits = u32::from(t.unpremul)
                | (u32::from(t.linearize) << 1)
                | (u32::from(t.gamut_transform) << 2)
                | (u32::from(t.encode) << 3)
                | (u32::from(t.premul) << 4);
            tested |= 1 << bits;
        }

        // We'll check our test cases cover all 2^5 == 32 possible outputs (excluding interactions
        // with the HLG OOTF).
        for t in 0..32u32 {
            if tested & (1 << t) != 0 {
                continue;
            }

            // There are a couple impossible outputs, so consider those bits tested.
            //
            // Unpremul then premul should be optimized away to a noop, so 0b10001 isn't possible.
            // A gamut transform in the middle is fine too, so 0b10101 isn't possible either.
            if t == 0b10001 || t == 0b10101 {
                continue;
            }

            errorf!(
                r,
                "{{ xxx, yyy, at, {},{},{},{},{} }}, not covered",
                if t & 1 != 0 { " true" } else { "false" },
                if t & 2 != 0 { " true" } else { "false" },
                if t & 4 != 0 { " true" } else { "false" },
                if t & 8 != 0 { " true" } else { "false" },
                if t & 16 != 0 { " true" } else { "false" },
            );
        }
    }
);

/// The `make_surface` / `upload_image` pair of `run_color_space_xform_test`: none (the
/// `SkColorSpaceXformSteps::apply` path, `SkColorSpaceXform_Apply`), a raster surface
/// (`SkColorSpaceXform_Raster`), or Graphite with the recorder that makes the surfaces and uploads
/// the images, and the context they are read back through (`SkColorSpaceXform_Graphite`).
enum Backend<'a> {
    Apply,
    Raster,
    Graphite {
        context: &'a mut WgpuContext,
        recorder: &'a Recorder,
    },
}

// The body after the destination surface is made: `clear(SK_ColorWHITE)`, `drawImage(src, 0, 0)`,
// then the read back to an F32 target and its check.
// Port of: tests/SkColorSpaceXformStepsTest.cpp#L392-L410 (chrome/m156)
fn check_xform_result(
    reporter: &mut Reporter,
    dst_surface: &mut dyn TestSurface,
    src_image: &Image,
    dst_info: &ImageInfo,
    expected_rgba: &[f32; 4],
) {
    dst_surface.canvas().clear(Color::WHITE);
    dst_surface.canvas().draw_image(src_image, (0.0, 0.0), None);

    // Read back to an F32 target.
    let rb_info = dst_info.with_color_type(ColorType::RGBAF32);
    let mut rb_bm = Bitmap::new();
    rb_bm.alloc_pixels_info(&rb_info, None);
    let rb_result = dst_surface.read_pixels(&mut rb_bm);
    reporter_assert!(reporter, rb_result);

    let rb_rgba = first_pixel_f32(&rb_bm);
    reporter_assert!(reporter, rgba_close(&rb_rgba, expected_rgba));
}

// `reinterpret_cast<const float*>(rb_bm.pixmap().addr(0, 0))`: the first pixel's four floats.
fn first_pixel_f32(bitmap: &Bitmap) -> [f32; 4] {
    let mut rgba = [0.0_f32; 4];
    let Some(pixmap) = bitmap.peek_pixels() else {
        return rgba;
    };
    let Some(bytes) = pixmap.addr() else {
        return rgba;
    };
    for (channel, chunk) in rgba.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *channel = f32::from_ne_bytes(*chunk);
    }
    rgba
}

// Body of test to ensure that SkColorSpaceXformSteps::apply, raster, ganesh, and graphite all
// produce the same results for color space conversions.
// Port of: tests/SkColorSpaceXformStepsTest.cpp#L270-L411 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
#[allow(clippy::similar_names)] // mirrors the C++ variable names
#[allow(clippy::items_after_statements)] // mirrors the C++ local declarations
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
#[allow(clippy::eq_op)] // mirrors 203/203.f in the C++ expected values
fn run_color_space_xform_test(reporter: &mut Reporter, mut backend: Backend<'_>) {
    const K_WIDTH: i32 = 2;
    const K_HEIGHT: i32 = 2;
    const K_PQ100: f32 = 0.508_078_421_517_399;
    const K_PQ203: f32 = 0.580_688_881_041_610_9;
    const K_PQ1000: f32 = 0.751_827_096_247_041;

    let rec2020_pq_203 = make_rgb(&trfn_pq_203(), &named_gamut::REC2020);
    let rec2020_pq_100 = make_rgb(&trfn_pq_100(), &named_gamut::REC2020);
    let rec2020_hlg_12x = make_rgb(&trfn_hlg_12x(), &named_gamut::REC2020);
    let rec2020_hlg_203 = make_rgb(&trfn_hlg_203(), &named_gamut::REC2020);
    let rec2020_linear = make_rgb(&named_transfer_fn::LINEAR, &named_gamut::REC2020);
    let srgb_hlg_203 = make_rgb(&trfn_hlg_203(), &named_gamut::SRGB);
    let srgb_linear = make_rgb(&named_transfer_fn::LINEAR, &named_gamut::SRGB);

    struct Rec<'a> {
        src_cs: &'a ColorSpace,
        src_rgba: [f32; 4],
        dst_cs: &'a ColorSpace,
        expected_rgba: [f32; 4],
    }
    let recs = [
        Rec {
            src_cs: &rec2020_hlg_203,
            src_rgba: [0.75, 0.75, 0.75, 1.0],
            dst_cs: &rec2020_linear,
            expected_rgba: [1.0, 1.0, 1.0, 1.0],
        },
        Rec {
            src_cs: &rec2020_linear,
            src_rgba: [1.0, 1.0, 1.0, 1.0],
            dst_cs: &rec2020_hlg_203,
            expected_rgba: [0.75, 0.75, 0.75, 1.0],
        },
        Rec {
            src_cs: &rec2020_hlg_12x,
            src_rgba: [0.5, 0.5, 0.5, 1.0],
            dst_cs: &rec2020_linear,
            expected_rgba: [1.0, 1.0, 1.0, 1.0],
        },
        Rec {
            src_cs: &rec2020_linear,
            src_rgba: [1.0, 1.0, 1.0, 1.0],
            dst_cs: &rec2020_hlg_12x,
            expected_rgba: [0.5, 0.5, 0.5, 1.0],
        },
        Rec {
            src_cs: &srgb_hlg_203,
            src_rgba: [0.1, 0.5, 0.75, 1.0],
            dst_cs: &srgb_linear,
            expected_rgba: [0.009_894_11, 0.247_352_74, 0.786_470_59, 1.0],
        },
        Rec {
            src_cs: &srgb_linear,
            src_rgba: [0.009_894_11, 0.247_352_74, 0.786_470_59, 1.0],
            dst_cs: &srgb_hlg_203,
            expected_rgba: [0.1, 0.5, 0.75, 1.0],
        },
        Rec {
            src_cs: &rec2020_pq_203,
            src_rgba: [K_PQ100, K_PQ203, K_PQ1000, 1.0],
            // Note: the blue expected component should be 1000/203, but the skcms formulation
            // of PQ evaluates to this.
            // TODO(https://issues.skia.org/issues/420956739): Investiage this.
            dst_cs: &rec2020_linear,
            expected_rgba: [100.0 / 203.0, 203.0 / 203.0, 1003.0 / 203.0, 1.0],
        },
        Rec {
            src_cs: &rec2020_pq_203,
            src_rgba: [K_PQ203, K_PQ203, K_PQ203, 1.0],
            dst_cs: &rec2020_pq_100,
            expected_rgba: [K_PQ100, K_PQ100, K_PQ100, 1.0],
        },
        // Note: the next two tests use color values outside of [0,1], so this will fail if
        // there is clamping to [0,1].
        Rec {
            src_cs: &rec2020_linear,
            src_rgba: [1.0, 2.03, 10.0, 1.0],
            dst_cs: &rec2020_pq_100,
            expected_rgba: [K_PQ100, K_PQ203, K_PQ1000, 1.0],
        },
        Rec {
            src_cs: &rec2020_pq_100,
            src_rgba: [K_PQ100, K_PQ203, K_PQ1000, 1.0],
            dst_cs: &rec2020_linear,
            expected_rgba: [1.0, 2.03, 10.0, 1.0],
        },
    ];

    for rec in &recs {
        if matches!(backend, Backend::Apply) {
            let steps = ColorSpaceXformSteps::new(
                Some(rec.src_cs),
                AlphaType::Unpremul,
                Some(rec.dst_cs),
                AlphaType::Unpremul,
            );
            let mut xform_rgba = [
                rec.src_rgba[0],
                rec.src_rgba[1],
                rec.src_rgba[2],
                rec.src_rgba[3],
            ];
            steps.apply(&mut xform_rgba);
            reporter_assert!(reporter, rgba_close(&xform_rgba, &rec.expected_rgba));
            continue;
        }

        // Create an F16 image with the specified color. If we do not convert explicitly to
        // F16, then when the GPU based tests attempt to implicitly convert to F32 textures
        // and fail, they fall back to converting to 8888, which results in clamping and
        // ginormous error. Write the values directly (rather than ask SkColor4fs) to ensure
        // we are testing the full pipeline.
        let src_info = ImageInfo::new(
            (K_WIDTH, K_HEIGHT),
            ColorType::RGBAF32,
            AlphaType::Premul,
            rec.src_cs.clone(),
        );

        // Write the pixels as F32.
        let mut src_bm_f32 = Bitmap::new();
        src_bm_f32.alloc_pixels_info(&src_info, None);
        src_bm_f32.erase_color(Color::TRANSPARENT);
        {
            let mut pm = src_bm_f32
                .peek_pixels_mut()
                .expect("the F32 bitmap has pixels");
            for x in 0..K_WIDTH {
                for y in 0..K_HEIGHT {
                    let p = pm
                        .writable_addr_at((x, y))
                        .expect("a pixel of the F32 bitmap");
                    for c in 0..4 {
                        p[4 * c..4 * c + 4].copy_from_slice(&rec.src_rgba[c].to_ne_bytes());
                    }
                }
            }
        }
        let mut src_bm = Bitmap::new();
        src_bm.alloc_pixels_info(&src_info.with_color_type(ColorType::RGBAF16), None);
        let rp_result = match src_bm.peek_pixels_mut() {
            Some(mut dst) => src_bm_f32.read_pixels_to_pixmap(&mut dst, (0, 0)),
            None => false,
        };
        reporter_assert!(reporter, rp_result);
        src_bm.set_immutable();

        let Some(src_raster) = images::raster_from_bitmap(&src_bm) else {
            errorf!(reporter, "RasterFromBitmap failed");
            continue;
        };
        // `upload_image`: `SkImages::TextureFromImage(recorder, image, {false})` (Graphite only).
        let src_image = match &backend {
            Backend::Graphite { recorder, .. } => {
                let uploaded = texture_from_image(
                    recorder,
                    &src_raster,
                    RequiredProperties { mipmapped: false },
                );
                reporter_assert!(reporter, uploaded.is_some());
                let Some(uploaded) = uploaded else {
                    continue;
                };
                uploaded
            }
            Backend::Apply | Backend::Raster => src_raster,
        };

        // Render the image to an F16 target.
        let dst_info = ImageInfo::new(
            (K_WIDTH, K_HEIGHT),
            ColorType::RGBAF16,
            AlphaType::Premul,
            rec.dst_cs.clone(),
        );
        match &mut backend {
            Backend::Apply => {}
            Backend::Raster => {
                // `SkSurfaces::Raster(info)`; a surface that cannot be made is skipped.
                let Some(mut dst_surface) = surfaces::raster(&dst_info, None, None) else {
                    continue;
                };
                check_xform_result(
                    reporter,
                    &mut dst_surface,
                    &src_image,
                    &dst_info,
                    &rec.expected_rgba,
                );
            }
            Backend::Graphite { context, recorder } => {
                // `SkSurfaces::RenderTarget(recorder.get(), info)`; a surface that cannot be made
                // is skipped.
                let Some(dst_surface) =
                    GraphiteSurface::render_target(recorder, &dst_info, Mipmapped::No, None, "")
                else {
                    continue;
                };
                check_xform_result(
                    reporter,
                    &mut GraphiteTestSurface {
                        context,
                        surface: &dst_surface,
                    },
                    &src_image,
                    &dst_info,
                    &rec.expected_rgba,
                );
            }
        }
    }
}

// Test color space space conversion using SkColorSpaceXformSteps::apply.
// Port of: tests/SkColorSpaceXformStepsTest.cpp#L414-L416 (chrome/m156)
def_test!(SkColorSpaceXform_Apply, |reporter| {
    run_color_space_xform_test(reporter, Backend::Apply);
});

// Test color space space conversion using raster.
// Port of: tests/SkColorSpaceXformStepsTest.cpp#L419-L424 (chrome/m156)
def_test!(SkColorSpaceXform_Raster, |reporter| {
    run_color_space_xform_test(reporter, Backend::Raster);
});

// Test color space conversion using Graphite.
// Port of: tests/SkColorSpaceXformStepsTest.cpp#L444-L460 (chrome/m156)
def_graphite_adapter_test!(SkColorSpaceXform_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);
    run_color_space_xform_test(
        reporter,
        Backend::Graphite {
            context,
            recorder: &recorder,
        },
    );
});
