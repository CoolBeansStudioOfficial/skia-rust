// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Unit tests of `drawVertices`, `drawPatch` and `drawAtlas` over the raster device: what the
//! ported Skia tests do not cover (they only draw to check that nothing crashes). Every expected
//! picture is made by an independent route: a rect drawn with the color or shader the
//! vertices should produce, or a value worked out by hand.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_simd::Tier;
use skia_rust_simd::testing::{force_tier, oracle_selection};

use crate::surface::Surface;
use crate::surfaces;

const SIZE: i32 = 64;

/// A surface of half floats: the pipelines that draw to it are highp, like the ones of vertices
/// with colors (`matrix_4x3` is highp only), so that the expected pictures drawn with color
/// shaders (lowp otherwise) round as they do.
fn surface() -> Surface<'static> {
    let info = ImageInfo::new((SIZE, SIZE), ColorType::RGBAF16, AlphaType::Premul, None);
    surfaces::raster(&info, None, None).expect("surface")
}

/// The raw pixels (8 bytes each), row by row.
fn pixels(s: &mut Surface<'_>) -> Vec<u64> {
    let peek = s.peek_pixels().expect("pixels");
    let pm = peek.pixmap();
    (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .map(|(x, y)| pm.addr64(x, y))
        .collect()
}

/// The color at (x, y) of a surface (unpremultiplied).
fn color_at(s: &mut Surface<'_>, x: i32, y: i32) -> Color {
    let peek = s.peek_pixels().expect("pixels");
    peek.pixmap().get_color((x, y))
}

/// Asserts that two pictures are the same, reporting the first pixel that is not.
#[track_caller]
fn assert_same(got: &[u64], expected: &[u64], what: &str) {
    assert_eq!(got.len(), expected.len());
    let different = got.iter().zip(expected).filter(|(g, e)| g != e).count();
    if let Some(i) = got.iter().zip(expected).position(|(g, e)| g != e) {
        let (x, y) = (i % SIZE as usize, i / SIZE as usize);
        panic!(
            "{what}: {different} pixels differ; first at ({x}, {y}): got {:#018x}, expected {:#018x}",
            got[i], expected[i]
        );
    }
}

/// Runs `body` on every CPU tier, as the oracle selects them.
fn each_tier(body: impl Fn()) {
    for tier in Tier::ALL {
        let _guard = force_tier(oracle_selection(tier)).expect("a checkable tier");
        body();
    }
}

/// A square of side `side` at (0, 0) as a triangle strip.
fn square_strip(side: f32, colors: Option<&[Color]>, texs: Option<&[Point]>) -> Vertices {
    let pos = [
        Point::new(0.0, 0.0),
        Point::new(0.0, side),
        Point::new(side, 0.0),
        Point::new(side, side),
    ];
    Vertices::new_copy(VertexMode::TriangleStrip, &pos, texs, colors, None).expect("vertices")
}

/// What `draw_rect` makes of the rect (5, 5, 25, 25) with `paint`.
fn reference_rect(paint: &Paint) -> Vec<u64> {
    let mut s = surface();
    s.canvas().draw_rect(Rect::new(5.0, 5.0, 25.0, 25.0), paint);
    pixels(&mut s)
}

/// Draws `vertices` translated to (5, 5).
fn draw_translated(vertices: &Vertices, mode: BlendMode, paint: &Paint) -> Vec<u64> {
    let mut s = surface();
    let canvas = s.canvas();
    canvas.translate((5.0, 5.0));
    canvas.draw_vertices(vertices, mode, paint);
    pixels(&mut s)
}

fn paint_with_shader(shader: Shader) -> Paint {
    let mut p = Paint::default();
    p.set_shader(shader);
    p
}

fn color_paint(color: Color) -> Paint {
    let mut p = Paint::default();
    p.set_color(color);
    p
}

const COLORS: [Color; 4] = [
    Color::new(0xFF30_80C0),
    Color::new(0x8040_FF10),
    Color::new(0x0100_0000),
    Color::new(0xC0FF_FFFF),
];

#[test]
fn per_vertex_colors_of_one_color_fill_like_a_rect() {
    each_tier(|| {
        for color in COLORS {
            let expected = reference_rect(&color_paint(color));
            let v = square_strip(20.0, Some(&[color; 4]), None);
            // With `Dst` the colors are the vertex colors, whatever the paint.
            let got = draw_translated(&v, BlendMode::Dst, &Paint::default());
            assert_same(&got, &expected, &format!("{color:?}"));
        }
    });
}

#[test]
fn vertices_without_colors_and_shader_use_the_paint_color() {
    each_tier(|| {
        for color in COLORS {
            let expected = reference_rect(&color_paint(color));
            let v = square_strip(20.0, None, None);
            // The blend mode is ignored without vertex colors.
            for mode in [BlendMode::SrcOver, BlendMode::Dst, BlendMode::Xor] {
                let got = draw_translated(&v, mode, &color_paint(color));
                assert_same(&got, &expected, &format!("{color:?} {mode:?}"));
            }
        }
    });
}

#[test]
fn colors_are_interpolated_barycentrically() {
    each_tier(|| {
        // Red at (0, 0), green at (32, 0), blue at (0, 32): at the center of pixel (x, y) the
        // weights are u = (x + 0.5) / 32 for green and v = (y + 0.5) / 32 for blue (exact in
        // binary for the pixels below).
        let pos = [
            Point::new(0.0, 0.0),
            Point::new(32.0, 0.0),
            Point::new(0.0, 32.0),
        ];
        let colors = [Color::RED, Color::GREEN, Color::BLUE];
        let v = Vertices::new_copy(VertexMode::Triangles, &pos, None, Some(&colors), None)
            .expect("vertices");
        let mut s = surface();
        s.canvas()
            .draw_vertices(&v, BlendMode::Dst, &Paint::default());

        // (x, y): r = 1 - u - v, g = u, b = v, times 255, rounded.
        let cases = [
            ((0, 0), (247, 4, 4)),
            ((8, 8), (120, 68, 68)),
            ((16, 8), (56, 131, 68)),
            ((8, 16), (56, 68, 131)),
        ];
        for ((x, y), (r, g, b)) in cases {
            let c = color_at(&mut s, x, y);
            assert_eq!(
                (c.r(), c.g(), c.b(), c.a()),
                (r, g, b, 255),
                "pixel ({x}, {y})"
            );
        }
        // Outside the triangle nothing is drawn.
        assert_eq!(color_at(&mut s, 40, 40), Color::new(0));
        assert_eq!(color_at(&mut s, 31, 31), Color::new(0));
    });
}

/// The blend modes whose vertices are blended with the paint's shader or opaque color.
const MODES: [BlendMode; 10] = [
    BlendMode::SrcOver,
    BlendMode::Modulate,
    BlendMode::Plus,
    BlendMode::Screen,
    BlendMode::Multiply,
    BlendMode::DstIn,
    BlendMode::SrcIn,
    BlendMode::Xor,
    BlendMode::Darken,
    BlendMode::Luminosity,
];

#[test]
fn vertex_colors_are_blended_with_the_shader() {
    each_tier(|| {
        let vertex_color = Color::new(0xB040_C0E0);
        let shader_color = Color::new(0x80E0_6020);
        for mode in MODES {
            // The vertex colors are the blend's destination, the shader is its source.
            let blended = shaders::blend(
                mode,
                shaders::color(vertex_color),
                shaders::color(shader_color),
            );
            let expected = reference_rect(&paint_with_shader(blended));

            let v = square_strip(20.0, Some(&[vertex_color; 4]), None);
            let got = draw_translated(&v, mode, &paint_with_shader(shaders::color(shader_color)));
            assert_same(&got, &expected, &format!("{mode:?}"));
        }
    });
}

#[test]
fn vertex_colors_are_blended_with_the_opaque_paint_color() {
    each_tier(|| {
        let vertex_color = Color::new(0xB040_C0E0);
        let paint_color = Color::new(0x80E0_6020);
        for mode in MODES {
            // Without a shader the source is the paint's color made opaque.
            let blended = shaders::blend(
                mode,
                shaders::color(vertex_color),
                shaders::color(Color::new(0xFFE0_6020)),
            );
            // (The paint's alpha still scales the result.)
            let mut paint = paint_with_shader(blended);
            paint.set_alpha(0x80);
            let expected = reference_rect(&paint);

            let v = square_strip(20.0, Some(&[vertex_color; 4]), None);
            let got = draw_translated(&v, mode, &color_paint(paint_color));
            assert_same(&got, &expected, &format!("{mode:?}"));
        }
    });
}

#[test]
fn src_and_dst_modes_drop_the_colors_or_the_shader() {
    each_tier(|| {
        let vertex_color = Color::new(0xB040_C0E0);
        let shader_color = Color::new(0x80E0_6020);
        let colors = [vertex_color; 4];
        let v = square_strip(20.0, Some(&colors), None);
        let paint = paint_with_shader(shaders::color(shader_color));

        // `Src`: only the shader.
        let got = draw_translated(&v, BlendMode::Src, &paint);
        assert_same(&got, &reference_rect(&paint), "Src");

        // `Dst`: only the vertex colors.
        let got = draw_translated(&v, BlendMode::Dst, &paint);
        assert_same(&got, &reference_rect(&color_paint(vertex_color)), "Dst");
    });
}

#[test]
fn texture_coordinates_map_the_shader_per_triangle() {
    each_tier(|| {
        // A color shader does not depend on its coordinates, so the draw is the same with
        // texture coordinates (which make a transform shader per triangle) as without.
        let vertex_color = Color::new(0xB040_C0E0);
        let shader_color = Color::new(0x80E0_6020);
        let texs = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 8.0),
            Point::new(8.0, 0.0),
            Point::new(8.0, 8.0),
        ];
        let paint = paint_with_shader(shaders::color(shader_color));
        for colors in [None, Some([vertex_color; 4])] {
            for mode in [BlendMode::SrcOver, BlendMode::Modulate, BlendMode::Src] {
                let plain = square_strip(20.0, colors.as_ref().map(|c| &c[..]), None);
                let textured = square_strip(20.0, colors.as_ref().map(|c| &c[..]), Some(&texs));
                assert_same(
                    &draw_translated(&textured, mode, &paint),
                    &draw_translated(&plain, mode, &paint),
                    &format!("{colors:?} {mode:?}"),
                );
            }
        }

        // Collapsed texture coordinates make a singular matrix, which is allowed: the shader is
        // sampled at one point (`vertices_collapsed`).
        let collapsed = [Point::new(3.0, 3.0); 4];
        let v = square_strip(20.0, None, Some(&collapsed));
        let plain = square_strip(20.0, None, None);
        assert_same(
            &draw_translated(&v, BlendMode::SrcOver, &paint),
            &draw_translated(&plain, BlendMode::SrcOver, &paint),
            "collapsed",
        );
    });
}

#[test]
fn alpha_of_the_paint_scales_the_shader() {
    each_tier(|| {
        let shader_color = Color::new(0xFFE0_6020);
        let mut paint = paint_with_shader(shaders::color(shader_color));
        paint.set_alpha_f(0.5);
        let expected = reference_rect(&paint);
        let v = square_strip(20.0, None, None);
        assert_same(
            &draw_translated(&v, BlendMode::SrcOver, &paint),
            &expected,
            "alpha",
        );
    });
}

#[test]
fn degenerate_or_empty_vertices_draw_nothing() {
    each_tier(|| {
        let two = [Point::new(0.0, 0.0), Point::new(10.0, 10.0)];
        let colors = [Color::RED, Color::RED];
        let v = Vertices::new_copy(VertexMode::Triangles, &two, None, Some(&colors), None);
        let mut s = surface();
        s.canvas()
            .draw_vertices(&v.expect("two vertices"), BlendMode::Dst, &Paint::default());
        assert!(pixels(&mut s).iter().all(|&p| p == 0));

        // A triangle with all vertices on a line, and one with all vertices equal.
        for pos in [
            [
                Point::new(0.0, 0.0),
                Point::new(5.0, 5.0),
                Point::new(10.0, 10.0),
            ],
            [Point::new(3.0, 3.0); 3],
        ] {
            let colors = [Color::RED, Color::GREEN, Color::BLUE];
            let v = Vertices::new_copy(VertexMode::Triangles, &pos, None, Some(&colors), None)
                .expect("vertices");
            let mut s = surface();
            s.canvas()
                .draw_vertices(&v, BlendMode::Dst, &Paint::default());
            assert!(pixels(&mut s).iter().all(|&p| p == 0), "{pos:?}");
        }
    });
}

#[test]
fn a_perspective_ctm_projects_the_vertices() {
    each_tier(|| {
        let mut persp = Matrix::new_identity();
        persp.set_persp_y(1.0 / 100.0);
        let v = square_strip(20.0, Some(&[Color::BLUE; 4]), None);
        let mut s = surface();
        let canvas = s.canvas();
        canvas.translate((5.0, 5.0));
        canvas.concat(&persp);
        canvas.draw_vertices(&v, BlendMode::Dst, &Paint::default());

        // The top edge is at y = 0 and the bottom one at 20 / (1 + 20 / 100) = 16.67, so the
        // square is drawn down to row 5 + 16 and the pixels below it are untouched.
        assert_eq!(color_at(&mut s, 10, 8), Color::BLUE);
        assert_eq!(color_at(&mut s, 10, 20), Color::BLUE);
        assert_eq!(color_at(&mut s, 10, 24), Color::new(0));
        assert_eq!(color_at(&mut s, 40, 8), Color::new(0));
    });
}

#[test]
fn skipping_the_color_transform_keeps_the_vertex_colors() {
    each_tier(|| {
        // A linear sRGB destination: the vertex colors are converted into it before they are
        // interpolated, unless the device is told that they are in its space already.
        let info = ImageInfo::new_n32_premul((SIZE, SIZE), ColorSpace::new_srgb_linear());
        let color = Color::new(0xFF80_8080);
        let v = square_strip(20.0, Some(&[color; 4]), None);
        let mut drawn = Vec::new();
        for skip in [false, true] {
            let mut s = surfaces::raster(&info, None, None).expect("surface");
            s.canvas().with_top_device(|device| {
                device.draw_vertices(&v, Blender::mode(BlendMode::Dst), &Paint::default(), skip);
            });
            drawn.push(color_at(&mut s, 10, 10));
        }
        // 0x80 / 255 is 0.2158 in linear sRGB: 55 of 255.
        assert_eq!(drawn[0], Color::new(0xFF37_3737));
        assert_eq!(drawn[1], color);
    });
}

fn atlas_xforms() -> [RSXform; 2] {
    [
        // translate by (10, 10)
        RSXform::new(1.0, 0.0, (10.0, 10.0)),
        // scale by 2, translate by (40, 10)
        RSXform::new(2.0, 0.0, (40.0, 10.0)),
    ]
}

#[test]
fn atlas_sprites_are_filled_with_the_shader() {
    each_tier(|| {
        let shader_color = Color::new(0xC0E0_6020);
        let shader = shaders::color(shader_color);
        // The rects are (2, 2) to (12, 12) of the atlas: 10 x 10 pixels.
        let tex = [Rect::new(2.0, 2.0, 12.0, 12.0); 2];

        let mut s = surface();
        s.canvas().draw_atlas_with_shader(
            &shader,
            &atlas_xforms(),
            &tex,
            None,
            BlendMode::Dst,
            None,
            None,
        );
        let got = pixels(&mut s);

        let mut expected = surface();
        for rect in [
            Rect::new(10.0, 10.0, 20.0, 20.0),
            Rect::new(40.0, 10.0, 60.0, 30.0),
        ] {
            expected
                .canvas()
                .draw_rect(rect, &paint_with_shader(shader.clone()));
        }
        assert_same(&got, &pixels(&mut expected), "atlas");
    });
}

#[test]
fn atlas_colors_are_blended_with_the_shader() {
    each_tier(|| {
        let shader_color = Color::new(0xC0E0_6020);
        let colors = [Color::new(0xFF10_A0F0), Color::new(0x6080_8080)];
        let tex = [Rect::new(0.0, 0.0, 10.0, 10.0); 2];
        for mode in MODES {
            let mut s = surface();
            s.canvas().draw_atlas_with_shader(
                &shaders::color(shader_color),
                &atlas_xforms(),
                &tex,
                &colors[..],
                mode,
                None,
                None,
            );
            let got = pixels(&mut s);

            let mut expected = surface();
            for (rect, color) in [
                (Rect::new(10.0, 10.0, 20.0, 20.0), colors[0]),
                (Rect::new(40.0, 10.0, 60.0, 30.0), colors[1]),
            ] {
                // The colors are the blend's destination, the shader its source.
                let blended =
                    shaders::blend(mode, shaders::color(color), shaders::color(shader_color));
                expected
                    .canvas()
                    .draw_rect(rect, &paint_with_shader(blended));
            }
            assert_same(&got, &pixels(&mut expected), &format!("{mode:?}"));
        }
    });
}

#[test]
fn atlas_uses_the_alpha_of_the_paint_and_the_ctm() {
    each_tier(|| {
        let shader = shaders::color(Color::new(0xFFE0_6020));
        let tex = [Rect::new(0.0, 0.0, 10.0, 10.0)];
        let xform = [RSXform::new(1.0, 0.0, (10.0, 10.0))];
        let mut paint = Paint::default();
        paint.set_alpha_f(0.5);

        let mut s = surface();
        let canvas = s.canvas();
        canvas.scale((2.0, 2.0));
        canvas.draw_atlas_with_shader(&shader, &xform, &tex, None, BlendMode::Dst, None, &paint);
        let got = pixels(&mut s);

        let mut expected = surface();
        let mut rect_paint = paint_with_shader(shader);
        rect_paint.set_alpha_f(0.5);
        expected
            .canvas()
            .draw_rect(Rect::new(20.0, 20.0, 40.0, 40.0), &rect_paint);
        assert_same(&got, &pixels(&mut expected), "atlas");
    });
}

#[test]
fn rotated_atlas_sprites_cover_the_rotated_rect() {
    each_tier(|| {
        let shader = shaders::color(Color::BLUE);
        let tex = [Rect::new(0.0, 0.0, 10.0, 10.0)];
        // A quarter turn about the origin, then translate: the sprite covers (20, 10)..(30, 20)...
        // (scos = 0, ssin = 1 maps (x, y) to (-y, x)): so x in [-10, 0] + 30, y in [0, 10] + 10.
        let xform = [RSXform::new(0.0, 1.0, (30.0, 10.0))];
        let mut s = surface();
        s.canvas()
            .draw_atlas_with_shader(&shader, &xform, &tex, None, BlendMode::Dst, None, None);

        let mut expected = surface();
        expected.canvas().draw_rect(
            Rect::new(20.0, 10.0, 30.0, 20.0),
            &paint_with_shader(shader.clone()),
        );
        assert_same(&pixels(&mut s), &pixels(&mut expected), "quarter turn");

        // A 45 degree turn fills a diamond: the center is blue, the corner of the bounds is not.
        let sin_cos = std::f32::consts::FRAC_1_SQRT_2;
        let xform = [RSXform::new(sin_cos, sin_cos, (30.0, 10.0))];
        let mut s = surface();
        s.canvas()
            .draw_atlas_with_shader(&shader, &xform, &tex, None, BlendMode::Dst, None, None);
        assert_eq!(color_at(&mut s, 30, 17), Color::BLUE);
        assert_eq!(color_at(&mut s, 24, 11), Color::new(0));
    });
}
