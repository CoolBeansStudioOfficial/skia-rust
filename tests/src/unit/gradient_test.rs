// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GradientTest.cpp (chrome/m156)
//
// Not ported:
// * `test_unsorted_degenerate`: compiled only with `SK_GANESH` (it makes a Ganesh fragment
//   processor), which is not in the oracle builds.
// * `TestSweepGradientZeroXGanesh`, `TestManyStopLinearHardstopsGanesh`: Ganesh tests, excluded.

#![cfg(test)]
// literals, exact comparisons and local items mirror the C++
#![allow(
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::items_after_statements
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas};
use skia_rust_core::color::{Color, Color4f, PMColor, colors};
use skia_rust_core::color_priv::get_packed_r32;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{self, GradientInfo, GradientType};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, reporter_assert};

/// `SkShaders::LinearGradient(pts, {{colors, pos, mode}, {}})`.
fn linear(
    pts: &[Point; 2],
    colors: &[Color4f],
    pos: Option<&[scalar]>,
    mode: TileMode,
    lm: Option<&Matrix>,
) -> Option<Shader> {
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(colors, pos, mode, None),
            Interpolation::default(),
        ),
        lm,
    )
}

// https://code.google.com/p/chromium/issues/detail?id=448299
// Giant (inverse) matrix causes overflow when converting/computing using 32.32
// Before the fix, we would assert (and then crash).
// Port of: tests/GradientTest.cpp#L62-L82 (chrome/m156)
fn test_big_grad(_reporter: &mut Reporter) {
    let colors = [colors::RED, colors::BLUE];
    let pts = [
        Point::new(15.0, 14.711_268_4),
        Point::new(0.709_064_007, 12.610_811_2),
    ];
    let mut paint = Paint::default();
    paint.set_shader(linear(&pts, &colors, None, TileMode::Clamp, None));

    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2000, 1), None);
    let c = Canvas::from_bitmap(&mut bm, None).expect("canvas");

    let affine = [
        1.066_086_27e-06,
        4.264_345_25e-07,
        6.2855,
        2.6611,
        273.4393,
        244.0046,
    ];
    let mut matrix = Matrix::new_identity();
    matrix.set_affine(&affine);
    c.concat(&matrix);

    c.draw_paint(&paint);
}

// Port of: tests/GradientTest.cpp#L84-L125 (chrome/m156)
struct GradRec<'a> {
    color_count: usize,
    colors: &'a [Color4f],
    pos: Option<&'a [scalar]>,
    point: &'a [Point; 2],
    radius: &'a [scalar; 2],
    tile_mode: TileMode,
}

impl GradRec<'_> {
    fn grad(&self) -> Gradient<'_> {
        Gradient::new(
            Colors::new(
                &self.colors[..self.color_count],
                self.pos.map(|p| &p[..self.color_count]),
                self.tile_mode,
                None,
            ),
            Interpolation::default(),
        )
    }

    /// Fills a `GradientInfo` from the shader (with `colors`/`color_offsets` storage of the
    /// right size, which are not part of the returned info) and checks it against this rec.
    fn grad_check(
        &self,
        reporter: &mut Reporter,
        shader: &Shader,
        gt: GradientType,
        local_matrix: &Matrix,
    ) -> GradientInfo<'static> {
        let mut color_storage = vec![Color4f::default(); self.color_count];
        let mut pos_storage = vec![0.0 as scalar; self.color_count];

        let mut info = GradientInfo {
            color_count: self.color_count,
            colors: Some(&mut color_storage),
            color_offsets: Some(&mut pos_storage),
            ..GradientInfo::default()
        };
        let mut shader_local_matrix = Matrix::new_identity();
        reporter_assert!(
            reporter,
            shader
                .as_base()
                .as_gradient(Some(&mut info), Some(&mut shader_local_matrix))
                == gt
        );
        reporter_assert!(reporter, shader_local_matrix == *local_matrix);

        reporter_assert!(reporter, info.color_count == self.color_count);
        let GradientInfo {
            color_count,
            point,
            radius,
            tile_mode,
            premul_interp,
            ..
        } = info;
        // `memcmp`
        reporter_assert!(
            reporter,
            color_storage
                .iter()
                .zip(self.colors)
                .all(|(a, b)| a.as_array().map(f32::to_bits) == b.as_array().map(f32::to_bits))
        );
        let fpos = self.pos.expect("positions");
        reporter_assert!(
            reporter,
            pos_storage
                .iter()
                .zip(fpos)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        reporter_assert!(reporter, self.tile_mode == tile_mode);
        GradientInfo {
            color_count,
            colors: None,
            color_offsets: None,
            point,
            radius,
            tile_mode,
            premul_interp,
        }
    }
}

/// `!memcmp(info.fPoint, checkRec.fPoint, 2 * sizeof(SkPoint))`.
fn points_memcmp(a: &[Point; 2], b: &[Point; 2]) -> bool {
    a.iter()
        .zip(b)
        .all(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits())
}

// Port of: tests/GradientTest.cpp#L127-L130 (chrome/m156)
fn none_gradproc(reporter: &mut Reporter, _build: &GradRec<'_>, _check: &GradRec<'_>) {
    let s = shaders::empty();
    reporter_assert!(
        reporter,
        GradientType::None == s.as_base().as_gradient(None, None)
    );
}

// Port of: tests/GradientTest.cpp#L132-L135 (chrome/m156)
fn color_gradproc(reporter: &mut Reporter, rec: &GradRec<'_>, _check: &GradRec<'_>) {
    let s = shaders::color_in_space(rec.colors[0], ColorSpace::new_srgb()).expect("shader");
    reporter_assert!(
        reporter,
        GradientType::None == s.as_base().as_gradient(None, None)
    );
}

// Port of: tests/GradientTest.cpp#L137-L144 (chrome/m156)
fn linear_gradproc(reporter: &mut Reporter, build_rec: &GradRec<'_>, check_rec: &GradRec<'_>) {
    let s = gradient_shaders::linear_gradient(
        (build_rec.point[0], build_rec.point[1]),
        &build_rec.grad(),
        None,
    )
    .expect("shader");

    let info = check_rec.grad_check(reporter, &s, GradientType::Linear, Matrix::i());
    reporter_assert!(reporter, points_memcmp(&info.point, check_rec.point));
}

// Port of: tests/GradientTest.cpp#L146-L155 (chrome/m156)
fn radial_gradproc(reporter: &mut Reporter, build_rec: &GradRec<'_>, check_rec: &GradRec<'_>) {
    let s = gradient_shaders::radial_gradient(
        (build_rec.point[0], build_rec.radius[0]),
        &build_rec.grad(),
        None,
    )
    .expect("shader");

    let info = check_rec.grad_check(reporter, &s, GradientType::Radial, Matrix::i());
    reporter_assert!(reporter, info.point[0] == check_rec.point[0]);
    reporter_assert!(reporter, info.radius[0] == check_rec.radius[0]);
}

// Port of: tests/GradientTest.cpp#L157-L164 (chrome/m156)
fn sweep_gradproc(reporter: &mut Reporter, build_rec: &GradRec<'_>, check_rec: &GradRec<'_>) {
    let s =
        gradient_shaders::sweep_gradient(build_rec.point[0], (0.0, 360.0), &build_rec.grad(), None)
            .expect("shader");

    let info = check_rec.grad_check(reporter, &s, GradientType::Sweep, Matrix::i());
    reporter_assert!(reporter, info.point[0] == check_rec.point[0]);
}

// Port of: tests/GradientTest.cpp#L166-L176 (chrome/m156)
fn conical_gradproc(reporter: &mut Reporter, build_rec: &GradRec<'_>, check_rec: &GradRec<'_>) {
    let s = gradient_shaders::two_point_conical_gradient(
        (build_rec.point[0], build_rec.radius[0]),
        (build_rec.point[1], build_rec.radius[1]),
        &build_rec.grad(),
        None,
    )
    .expect("shader");

    let info = check_rec.grad_check(reporter, &s, GradientType::Conical, Matrix::i());
    reporter_assert!(reporter, points_memcmp(&info.point, check_rec.point));
    reporter_assert!(
        reporter,
        info.radius
            .iter()
            .zip(check_rec.radius)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}

// Port of: tests/GradientTest.cpp#L178-L193 (chrome/m156)
fn linear_gradproc_matrix(
    reporter: &mut Reporter,
    build_rec: &GradRec<'_>,
    check_rec: &GradRec<'_>,
) {
    let local_matrix = Matrix::rotate_deg_pivot(45.0, (100.0, 100.0));
    let mut s = gradient_shaders::linear_gradient(
        (build_rec.point[0], build_rec.point[1]),
        &build_rec.grad(),
        Some(&local_matrix),
    )
    .expect("shader");

    let info = check_rec.grad_check(reporter, &s, GradientType::Linear, &local_matrix);
    reporter_assert!(reporter, points_memcmp(&info.point, check_rec.point));

    // Same but using a local matrix wrapper.
    s = gradient_shaders::linear_gradient(
        (build_rec.point[0], build_rec.point[1]),
        &build_rec.grad(),
        None,
    )
    .expect("shader");
    s = s.with_local_matrix(&local_matrix);
    let info = check_rec.grad_check(reporter, &s, GradientType::Linear, &local_matrix);
    reporter_assert!(reporter, points_memcmp(&info.point, check_rec.point));
}

// Ensure that repeated color gradients behave like drawing a single color
// Port of: tests/GradientTest.cpp#L195-L216 (chrome/m156)
fn test_constant_gradient(_reporter: &mut Reporter) {
    let pts = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
    let colors = [colors::BLUE, colors::BLUE];
    let pos = [0.0, 1.0];
    let mut paint = Paint::default();
    paint.set_shader(linear(&pts, &colors, Some(&pos), TileMode::Clamp, None));
    let mut out_bitmap = Bitmap::new();
    out_bitmap.alloc_n32_pixels((10, 1), None);
    let canvas = Canvas::from_bitmap(&mut out_bitmap, None).expect("canvas");
    canvas.draw_paint(&paint);
    // for i in 0..10 { REPORTER_ASSERT(reporter, SkColors::kBlue == outBitmap.getColor(i, 0)); }
    // The assertion is commented out in Skia because it currently fails
    // (https://code.google.com/p/skia/issues/detail?id=1098).
}

type GradProc = fn(&mut Reporter, &GradRec<'_>, &GradRec<'_>);

// Port of: tests/GradientTest.cpp#L218-L250 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ names gPos/gPts
fn test_gradient_shaders(reporter: &mut Reporter) {
    let g_colors = [colors::RED, colors::GREEN, colors::BLUE];
    let g_pos = [0.0, 0.5, 1.0];
    let g_pts = [Point::new(0.0, 0.0), Point::new(10.0, 20.0)];
    let g_rad = [1.0, 2.0];

    let rec = GradRec {
        color_count: g_colors.len(),
        colors: &g_colors,
        pos: Some(&g_pos),
        point: &g_pts,
        radius: &g_rad,
        tile_mode: TileMode::Clamp,
    };

    let g_procs: [GradProc; 7] = [
        none_gradproc,
        color_gradproc,
        linear_gradproc,
        linear_gradproc_matrix,
        radial_gradproc,
        sweep_gradproc,
        conical_gradproc,
    ];

    for proc in g_procs {
        proc(reporter, &rec, &rec);
    }
}

// Port of: tests/GradientTest.cpp#L252-L262 (chrome/m156)
fn test_nearly_vertical(_reporter: &mut Reporter) {
    let mut surface = surfaces::raster_n32_premul((200, 200)).expect("surface");

    let pts = [Point::new(100.0, 50.0), Point::new(100.0001, 50000.0)];
    let colors = [colors::BLACK, colors::WHITE];
    let pos = [0.0, 1.0];
    let mut paint = Paint::default();
    paint.set_shader(linear(&pts, &colors, Some(&pos), TileMode::Clamp, None));

    surface.canvas().draw_paint(&paint);
}

// Port of: tests/GradientTest.cpp#L264-L274 (chrome/m156)
fn test_vertical(_reporter: &mut Reporter) {
    let mut surface = surfaces::raster_n32_premul((200, 200)).expect("surface");

    let pts = [Point::new(100.0, 50.0), Point::new(100.0, 50.0)];
    let colors = [colors::BLACK, colors::WHITE];
    let pos = [0.0, 1.0];
    let mut paint = Paint::default();
    paint.set_shader(linear(&pts, &colors, Some(&pos), TileMode::Clamp, None));

    surface.canvas().draw_paint(&paint);
}

// A linear gradient interval can, due to numerical imprecision (likely in the divide)
// finish an interval with the final fx not landing outside of [p0...p1].
// The old code had an assert which this test triggered.
// We now explicitly clamp the resulting fx value.
// Port of: tests/GradientTest.cpp#L276-L290 (chrome/m156)
fn test_linear_fuzz(_reporter: &mut Reporter) {
    let mut surface = surfaces::raster_n32_premul((1300, 630)).expect("surface");

    let pts = [Point::new(179.5, -179.5), Point::new(1074.5, 715.5)];
    let colors = [colors::BLACK, colors::WHITE, colors::BLACK, colors::WHITE];
    let pos = [0.0, 0.200_000_003, 0.800_000_012, 1.0];

    let mut paint = Paint::default();
    paint.set_shader(linear(&pts, &colors, Some(&pos), TileMode::Clamp, None));

    let r = Rect::new(0.0, 83.0, 1254.0, 620.0);
    surface.canvas().draw_rect(r, &paint);
}

// https://bugs.chromium.org/p/skia/issues/detail?id=5023
// We should still shade pixels for which the radius is exactly 0.
// Port of: tests/GradientTest.cpp#L292-L312 (chrome/m156)
fn test_two_point_conical_zero_radius(reporter: &mut Reporter) {
    let mut surface = surfaces::raster_n32_premul((5, 5)).expect("surface");
    surface.canvas().clear(colors::RED);

    let colors = [colors::GREEN, colors::BLUE];
    let mut p = Paint::default();
    p.set_shader(gradient_shaders::two_point_conical_gradient(
        (Point::new(2.5, 2.5), 0.0),
        (Point::new(3.0, 3.0), 10.0),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    surface.canvas().draw_paint(&p);

    // r == 0 for the center pixel.
    // verify that we draw it (no red bleed)
    let mut center_pm_color_bytes = [0u8; 4];
    let ok = surface.read_pixels(
        &ImageInfo::new_n32_premul((1, 1), None),
        &mut center_pm_color_bytes,
        core::mem::size_of::<PMColor>(),
        (2, 2),
    );
    reporter_assert!(reporter, ok);
    let center_pm_color: PMColor = u32::from_ne_bytes(center_pm_color_bytes);
    reporter_assert!(reporter, get_packed_r32(center_pm_color) == 0);
}

// http://crbug.com/599458
// Port of: tests/GradientTest.cpp#L314-L331 (chrome/m156)
fn test_clamping_overflow(_reporter: &mut Reporter) {
    let mut p = Paint::default();
    let colors = [colors::RED, colors::GREEN];
    let pts1 = [
        Point::new(1001.0, 1_000_001.0),
        Point::new(1000.99, 1_000_000.0),
    ];

    p.set_shader(linear(&pts1, &colors, None, TileMode::Clamp, None));

    let mut surface = surfaces::raster_n32_premul((50, 50)).expect("surface");
    surface.canvas().scale((100.0, 100.0));
    surface.canvas().draw_paint(&p);

    let pts2 = [
        Point::new(10000.99, 1_000_000.0),
        Point::new(10001.0, 1_000_001.0),
    ];
    p.set_shader(linear(&pts2, &colors, None, TileMode::Clamp, None));
    surface.canvas().draw_paint(&p);

    // Passes if we don't trigger asserts.
}

// http://crbug.com/636194
// Port of: tests/GradientTest.cpp#L333-L346 (chrome/m156)
fn test_degenerate_linear(_reporter: &mut Reporter) {
    let mut p = Paint::default();
    let colors = [colors::RED, colors::GREEN];
    let pts = [
        Point::new(-46_058_024_627_067_344_430_605_278_824_628_224.0, 0.0),
        Point::new(f32::MAX, 0.0),
    ];

    p.set_shader(linear(&pts, &colors, None, TileMode::Clamp, None));
    let mut surface = surfaces::raster_n32_premul((50, 50)).expect("surface");
    surface.canvas().draw_paint(&p);

    // Passes if we don't trigger asserts.
}

// test_unsorted_degenerate (Port of: tests/GradientTest.cpp#L350-L396) is compiled only with
// SK_GANESH.

// "Interesting" fuzzer values.
// Port of: tests/GradientTest.cpp#L398-L517 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ test
fn test_linear_fuzzer(_reporter: &mut Reporter) {
    let g_colors0 = [
        Color4f::from_color(Color::new(0x3030_3030)),
        Color4f::from_color(Color::new(0x3030_3030)),
    ];
    let g_colors1 = [
        Color4f::from_color(Color::new(0x3030_3030)),
        Color4f::from_color(Color::new(0x3030_3030)),
        Color4f::from_color(Color::new(0x3030_3030)),
    ];

    let g_pos1 = [0.0, 0.0, 1.0];

    let g_matrix0: [f32; 9] = [
        6.409_690_56e-10,
        0.0,
        6.409_690_56e-10,
        0.0,
        4.425_390_23e-39,
        6.409_690_56e-10,
        0.0,
        0.0,
        1.0,
    ];
    let g_matrix1: [f32; 9] = [
        -2.752_941_13,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        -3.328_101_61e+24,
        6.409_690_56e-10,
        6.409_690_56e-10,
        0.0,
    ];
    let g_matrix2: [f32; 9] = [
        7.934_812_58e+17,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        0.688_235_283,
    ];
    let g_matrix3: [f32; 9] = [
        1.891_806_74e+11,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        6.409_690_56e-10,
        11276.0469,
        8.125_248_08e+20,
    ];

    struct Config<'a> {
        pts: [Point; 2],
        colors: &'a [Color4f],
        pos: Option<&'a [f32]>,
        tile_mode: TileMode,
        local_matrix: Option<&'a [f32; 9]>,
        global_matrix: Option<&'a [f32; 9]>,
    }

    let g_configs = [
        Config {
            pts: [Point::new(0.0, -2.752_941), Point::new(0.0, 0.0)],
            colors: &g_colors0,
            pos: None,
            tile_mode: TileMode::Clamp,
            local_matrix: Some(&g_matrix0),
            global_matrix: None,
        },
        Config {
            pts: [
                Point::new(4.425_390_23e-39, -4.425_390_23e-39),
                Point::new(9.780_411_62e-15, 4.425_390_23e-39),
            ],
            colors: &g_colors1,
            pos: Some(&g_pos1),
            tile_mode: TileMode::Clamp,
            local_matrix: None,
            global_matrix: Some(&g_matrix1),
        },
        Config {
            pts: [
                Point::new(4.425_390_23e-39, 6.409_690_56e-10),
                Point::new(6.409_690_56e-10, 1.492_372_38e-19),
            ],
            colors: &g_colors1,
            pos: Some(&g_pos1),
            tile_mode: TileMode::Clamp,
            local_matrix: None,
            global_matrix: Some(&g_matrix2),
        },
        Config {
            pts: [
                Point::new(6.409_690_56e-10, 6.409_690_56e-10),
                Point::new(6.409_690_56e-10, -0.688_235_283),
            ],
            colors: &g_colors0,
            pos: None,
            tile_mode: TileMode::Clamp,
            local_matrix: Some(&g_matrix3),
            global_matrix: None,
        },
    ];

    let srgb = ColorSpace::new_srgb();
    let color_spaces: [Option<&ColorSpace>; 2] = [
        None,        // hits the legacy gradient impl
        Some(&srgb), // triggers 4f/raster-pipeline
    ];

    let mut paint = Paint::default();

    for color_space in color_spaces {
        let mut surface = surfaces::raster(
            &ImageInfo::new(
                (100, 100),
                ColorType::N32,
                AlphaType::Premul,
                color_space.cloned(),
            ),
            None,
            None,
        )
        .expect("surface");
        let canvas = surface.canvas();

        for config in &g_configs {
            let _acr = AutoCanvasRestore::guard(canvas, false);
            let mut local_matrix: Option<Matrix> = None;
            if let Some(lm) = config.local_matrix {
                let mut m = Matrix::new_identity();
                m.set_9(lm);
                local_matrix = Some(m);
            }

            paint.set_shader(linear(
                &config.pts,
                config.colors,
                config.pos,
                config.tile_mode,
                local_matrix.as_ref(),
            ));
            if let Some(gm) = config.global_matrix {
                let mut m = Matrix::new_identity();
                m.set_9(gm);
                canvas.save();
                canvas.concat(&m);
            }

            canvas.draw_paint(&paint);
        }
    }
}

// Port of: tests/GradientTest.cpp#L519-L561 (chrome/m156)
fn test_sweep_fuzzer(_reporter: &mut Reporter) {
    let g_colors0 = [
        Color4f::from_color(Color::new(0x3030_3030)),
        Color4f::from_color(Color::new(0x3030_3030)),
        Color4f::from_color(Color::new(0x3030_3030)),
    ];
    let g_pos0: [f32; 3] = [-47_919_293_023_455_565_225_163_489_280.0, 0.0, 1.0];
    let g_matrix0: [f32; 9] = [
        1.121_167_16e-13,
        0.0,
        8.504_896_82e+16,
        4.191_704_1e-41,
        3.513_698_81e-23,
        -2.543_442_71e-26,
        9.611_119_07e+17,
        -3.352_638_08e-29,
        -1.356_594_03e+14,
    ];
    struct Config<'a> {
        center: Point,
        colors: &'a [Color4f],
        pos: Option<&'a [f32]>,
        global_matrix: Option<&'a [f32; 9]>,
    }

    let g_configs = [Config {
        center: Point::new(0.0, 0.0),
        colors: &g_colors0,
        pos: Some(&g_pos0),
        global_matrix: Some(&g_matrix0),
    }];

    let mut surface = surfaces::raster_n32_premul((100, 100)).expect("surface");
    let canvas = surface.canvas();
    let mut paint = Paint::default();

    for config in &g_configs {
        paint.set_shader(gradient_shaders::sweep_gradient(
            config.center,
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(config.colors, config.pos, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));

        let _acr = AutoCanvasRestore::guard(canvas, false);
        if let Some(gm) = config.global_matrix {
            let mut m = Matrix::new_identity();
            m.set_9(gm);
            canvas.save();
            canvas.concat(&m);
        }
        canvas.draw_paint(&paint);
    }
}

// Port of: tests/GradientTest.cpp#L709-L724 (chrome/m156)
def_tier_test!(Gradient, |reporter| {
    test_gradient_shaders(reporter);
    test_constant_gradient(reporter);
    test_big_grad(reporter);
    test_nearly_vertical(reporter);
    test_vertical(reporter);
    test_linear_fuzz(reporter);
    test_two_point_conical_zero_radius(reporter);
    test_clamping_overflow(reporter);
    test_degenerate_linear(reporter);
    test_linear_fuzzer(reporter);
    test_sweep_fuzzer(reporter);
    // test_unsorted_degenerate(reporter) is compiled only with SK_GANESH.
});
