// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SimplifyPaintTest.cpp (chrome/m156)
//
// The Ganesh variant needs that backend and is not ported.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arc::Arc;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, Color4f, colors};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shaders;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_raster::surfaces;
use skia_rust_skcms::{Matrix3x3, matrix3x3_concat};

use crate::tools::test_surface::{GraphiteTestSurface, TestSurface};
use crate::{Reporter, def_graphite_adapter_test, def_test, errorf, reporter_assert};

// Port of: tests/SimplifyPaintTest.cpp#L35-L35 (chrome/m156)
const K_SURFACE_SIZE: i32 = 32;

// Port of: tests/SimplifyPaintTest.cpp#L37-L39 (chrome/m156)
fn draw_paint(canvas: &Canvas, paint: &Paint) {
    canvas.draw_paint(paint);
}

// Port of: tests/SimplifyPaintTest.cpp#L41-L43 (chrome/m156)
fn draw_rect(canvas: &Canvas, paint: &Paint) {
    canvas.draw_rect(Rect::from_wh(K_SURFACE_SIZE as f32, K_SURFACE_SIZE as f32), paint);
}

// Port of: tests/SimplifyPaintTest.cpp#L45-L47 (chrome/m156)
fn draw_rrect(canvas: &Canvas, paint: &Paint) {
    let size = K_SURFACE_SIZE as f32;
    canvas.draw_rrect(
        RRect::new_rect_xy(Rect::new(0.0, 0.0, size, size), 4.0, 4.0),
        paint,
    );
}

// Port of: tests/SimplifyPaintTest.cpp#L49-L54 (chrome/m156)
fn draw_arc(canvas: &Canvas, paint: &Paint) {
    let size = K_SURFACE_SIZE as f32;
    // SkArc::Type::kWedge, which closes the arc through the oval's center.
    let arc = Arc::new(Rect::new(-size, -size, size, size), 0.0, 270.0, true);
    canvas.draw_arc_2(&arc, paint);
}

// Port of: tests/SimplifyPaintTest.cpp#L56-L65 (chrome/m156)
fn create_dented_rect() -> Path {
    let half = (K_SURFACE_SIZE / 2) as f32;
    let size = K_SURFACE_SIZE as f32;
    let mut b = PathBuilder::new();
    b.move_to((0.0, 0.0));
    b.line_to((half, 1.0));
    b.line_to((size, 0.0));
    b.line_to((size, size));
    b.line_to((0.0, size));
    b.close();
    b.detach()
}

// Port of: tests/SimplifyPaintTest.cpp#L67-L71 (chrome/m156)
fn draw_path(canvas: &Canvas, paint: &Paint) {
    let p = create_dented_rect();
    canvas.draw_path(&p, paint);
}

// Port of: tests/SimplifyPaintTest.cpp#L73-L80 (chrome/m156)
const DRAW_METHODS: [fn(&Canvas, &Paint); 5] = [draw_paint, draw_rect, draw_rrect, draw_arc, draw_path];

// Create an SkColorSpace that removes the specified color channel with the transfer
// function of the provided colorSpace
// Port of: tests/SimplifyPaintTest.cpp#L84-L101 (chrome/m156)
fn make_knockout(cs: &ColorSpace, index: usize) -> Option<ColorSpace> {
    let mut knockout_mat = Matrix3x3 {
        vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    knockout_mat.vals[index][0] = 0.0;
    knockout_mat.vals[index][1] = 0.0;
    knockout_mat.vals[index][2] = 0.0;
    let tmp_xyz_d50 = cs.to_xyzd50();
    let tmp_transfer_fn = cs.transfer_fn();
    let knocked_out = matrix3x3_concat(&tmp_xyz_d50, &knockout_mat);
    ColorSpace::new_rgb(&tmp_transfer_fn, &knocked_out)
}

// Port of: tests/SimplifyPaintTest.cpp#L103-L111 (chrome/m156)
fn get_surface_ii() -> ImageInfo {
    let spin_cs = ColorSpace::new_srgb().with_color_spin();
    ImageInfo::new(
        (K_SURFACE_SIZE, K_SURFACE_SIZE),
        ColorType::RGBA8888,
        AlphaType::Premul,
        spin_cs,
    )
}

// Port of: tests/SimplifyPaintTest.cpp#L113-L119 (chrome/m156)
fn almost_equals(a: Color, b: Color, tolerance: i32) -> bool {
    if (i32::from(a.r()) - i32::from(b.r())).abs() > tolerance {
        return false;
    }
    if (i32::from(a.g()) - i32::from(b.g())).abs() > tolerance {
        return false;
    }
    if (i32::from(a.b()) - i32::from(b.b())).abs() > tolerance {
        return false;
    }
    if (i32::from(a.a()) - i32::from(b.a())).abs() > tolerance {
        return false;
    }
    true
}

// Port of: tests/SimplifyPaintTest.cpp#L121-L197 (chrome/m156)
fn run_test(surface: &mut dyn TestSurface, reporter: &mut Reporter) {
    let info = ImageInfo::new(
        (K_SURFACE_SIZE, K_SURFACE_SIZE),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&info, None);

    let srgb_cs = ColorSpace::new_srgb();
    let knockout_g = make_knockout(&srgb_cs, 1);
    let knockout_b = make_knockout(&srgb_cs, 2);

    // The same as the color spin ColorSpace: R->B, G->R, B->R
    static K_SPIN_MATRIX: [f32; 20] = [
        0.0, 1.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, 0.0, //
        1.0, 0.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    let spin_matrix_cf = color_filters::matrix_row_major(&K_SPIN_MATRIX, Clamp::Yes);

    let k_trans_white = Color4f::new(1.0, 1.0, 1.0, 0.5);
    let white = colors::WHITE;
    let k_cyan100 = Color4f::new(0.0, 1.0, 1.0, 1.0);
    let k_cyan50 = Color4f::new(0.0, 1.0, 1.0, 0.5);
    let k_cyan25 = Color4f::new(0.0, 1.0, 1.0, 0.25);
    let k_yellow100 = Color4f::new(1.0, 1.0, 0.0, 1.0);
    let k_yellow50 = Color4f::new(1.0, 1.0, 0.0, 0.5);
    let k_yellow25 = Color4f::new(1.0, 1.0, 0.0, 0.25);

    // These will be used to create an SkPaint w/ the specified paint color, solid color shader,
    // and color filter.
    struct Case {
        paint: Color4f,
        shader: Color4f,
        color_filter: Option<skia_rust_core::color_filter::ColorFilter>,
        expected: Color,
    }
    let k_cases = [
        Case {
            paint: white,
            shader: white,
            color_filter: None,
            expected: k_cyan100.to_color(),
        },
        Case {
            paint: k_trans_white,
            shader: white,
            color_filter: None,
            expected: k_cyan50.to_color(),
        },
        Case {
            paint: white,
            shader: k_trans_white,
            color_filter: None,
            expected: k_cyan50.to_color(),
        },
        Case {
            paint: k_trans_white,
            shader: k_trans_white,
            color_filter: None,
            expected: k_cyan25.to_color(),
        },
        Case {
            paint: white,
            shader: white,
            color_filter: spin_matrix_cf.clone(),
            expected: k_yellow100.to_color(),
        },
        Case {
            paint: k_trans_white,
            shader: white,
            color_filter: spin_matrix_cf.clone(),
            expected: k_yellow50.to_color(),
        },
        Case {
            paint: white,
            shader: k_trans_white,
            color_filter: spin_matrix_cf.clone(),
            expected: k_yellow50.to_color(),
        },
        Case {
            paint: k_trans_white,
            shader: k_trans_white,
            color_filter: spin_matrix_cf.clone(),
            expected: k_yellow25.to_color(),
        },
    ];

    for c in &k_cases {
        let mut paint = Paint::default();
        paint.set_color4f(c.paint, knockout_b.as_ref()); // yellow
        paint.set_shader(shaders::color_in_space(c.shader, knockout_g.clone())); // magenta
        paint.set_color_filter(c.color_filter.clone()); // magenta -> cyan, if !nullptr
        paint.set_blend_mode(BlendMode::Src);
        for draw in DRAW_METHODS {
            surface.canvas().clear(Color::BLACK);
            // The spinCS on the surface spins:
            //    cyan    -> yellow
            //    magenta -> cyan
            //    yellow  -> magenta
            draw(surface.canvas(), &paint);
            if !surface.read_pixels(&mut bitmap) {
                errorf!(reporter, "readPixels failed");
                return;
            }
            let actual = bitmap.get_color((K_SURFACE_SIZE / 2, K_SURFACE_SIZE / 2));
            reporter_assert!(
                reporter,
                almost_equals(actual, c.expected, 2),
                "Wrong color, expected {:08x}, found {:08x}",
                u32::from(c.expected),
                u32::from(actual)
            );
        }
    }
}

// Port of: tests/SimplifyPaintTest.cpp#L230-L234 (chrome/m156)
def_test!(SimplifyPaintTest_Raster, |reporter| {
    let mut surface = surfaces::raster(&get_surface_ii(), None, None).expect("a raster surface");
    run_test(&mut surface, reporter);
});

// Port of: tests/SimplifyPaintTest.cpp#L201-L213 (chrome/m156)
def_graphite_adapter_test!(SimplifyPaintTest_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);
    let surface = GraphiteSurface::render_target(
        &recorder,
        &get_surface_ii(),
        Mipmapped::No,
        None,
        "",
    );
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };
    run_test(
        &mut GraphiteTestSurface {
            context,
            surface: &surface,
        },
        reporter,
    );
});
