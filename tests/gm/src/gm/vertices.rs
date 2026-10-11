// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/vertices.cpp (chrome/m156)
#![allow(clippy::many_single_char_names)]
// skbug_13047 mirrors the C++ names w and h
//
// Only `VerticesGM(1)`, `vertices_batching`, `vertices_collapsed`, `vertices_perspective` and
// `vertices_strip` and `skbug_13047` are ported here. `VerticesGM(1 / kShaderSize)` is not in the
// manifest's raster list.

// The int/float mixing and index casts mirror the C++ arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use crate::tool_utils::create_checkerboard_shader;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::colors;
use skia_rust_core::color_filters;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders as core_shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/vertices.cpp#L20-L20 (chrome/m156), kShaderSize
const K_SHADER_SIZE: scalar = 40.0;

// Port of: gm/vertices.cpp#L21-L37 (chrome/m156), make_shader1
#[allow(clippy::float_cmp)] // the C++ compares the scale with 1 exactly
fn make_shader1(shader_scale: scalar) -> Option<Shader> {
    let grad_colors = [
        colors::RED,
        colors::CYAN,
        colors::GREEN,
        colors::WHITE,
        colors::MAGENTA,
        colors::BLUE,
        colors::YELLOW,
    ];
    let pts = [
        Point::new(K_SHADER_SIZE / 4.0, 0.0),
        Point::new(3.0 * K_SHADER_SIZE / 4.0, K_SHADER_SIZE),
    ];
    let local_matrix = Matrix::scale((shader_scale, shader_scale));

    let grad = Gradient::new(
        Colors::new(&grad_colors, None, TileMode::Mirror, None),
        Interpolation::default(),
    );
    let grad_shader = shaders::linear_gradient((pts[0], pts[1]), &grad, &local_matrix)?;
    // Throw in a couple of local matrix wrappers for good measure.
    if shader_scale == 1.0 {
        Some(grad_shader)
    } else {
        Some(
            grad_shader
                .with_local_matrix(&Matrix::translate((-10.0, 0.0)))
                .with_local_matrix(&Matrix::translate((10.0, 0.0))),
        )
    }
}

// Port of: gm/vertices.cpp#L55-L57 (chrome/m156), make_shader2
fn make_shader2() -> Shader {
    core_shaders::color(Color::BLUE)
}

// Port of: gm/vertices.cpp#L59-L61 (chrome/m156), make_color_filter
fn make_color_filter() -> Option<skia_rust_core::color_filter::ColorFilter> {
    color_filters::blend_color(Color::new(0xFFAA_BBCC), BlendMode::Darken)
}

// Port of: gm/vertices.cpp#L63-L65 (chrome/m156), kMeshSize
const K_MESH_SIZE: scalar = 30.0;

// start with the center of a 3x3 grid of vertices.
// Port of: gm/vertices.cpp#L67-L70 (chrome/m156), kMeshFan
const K_MESH_FAN: [u16; 10] = [4, 0, 1, 2, 5, 8, 7, 6, 3, 0];

// Port of: gm/vertices.cpp#L72-L72 (chrome/m156), kMeshVertexCnt
const K_MESH_VERTEX_CNT: usize = 9;

// Port of: gm/vertices.cpp#L74-L101 (chrome/m156), fill_mesh
fn fill_mesh(shader_scale: scalar) -> ([Point; 9], [Point; 9], [Color; 9]) {
    let mut pts = [Point::new(0.0, 0.0); K_MESH_VERTEX_CNT];
    pts[0] = Point::new(0.0, 0.0);
    pts[1] = Point::new(K_MESH_SIZE / 2.0, 3.0);
    pts[2] = Point::new(K_MESH_SIZE, 0.0);
    pts[3] = Point::new(3.0, K_MESH_SIZE / 2.0);
    pts[4] = Point::new(K_MESH_SIZE / 2.0, K_MESH_SIZE / 2.0);
    pts[5] = Point::new(K_MESH_SIZE - 3.0, K_MESH_SIZE / 2.0);
    pts[6] = Point::new(0.0, K_MESH_SIZE);
    pts[7] = Point::new(K_MESH_SIZE / 2.0, K_MESH_SIZE - 3.0);
    pts[8] = Point::new(K_MESH_SIZE, K_MESH_SIZE);

    let shader_size = K_SHADER_SIZE * shader_scale;
    let mut texs = [Point::new(0.0, 0.0); K_MESH_VERTEX_CNT];
    texs[0] = Point::new(0.0, 0.0);
    texs[1] = Point::new(shader_size / 2.0, 0.0);
    texs[2] = Point::new(shader_size, 0.0);
    texs[3] = Point::new(0.0, shader_size / 2.0);
    texs[4] = Point::new(shader_size / 2.0, shader_size / 2.0);
    texs[5] = Point::new(shader_size, shader_size / 2.0);
    texs[6] = Point::new(0.0, shader_size);
    texs[7] = Point::new(shader_size / 2.0, shader_size);
    texs[8] = Point::new(shader_size, shader_size);

    let mut rand = Random::default();
    let mut colors = [Color::BLACK; K_MESH_VERTEX_CNT];
    for color in &mut colors {
        *color = Color::new(rand.next_u() | 0xFF00_0000);
    }
    (pts, texs, colors)
}

// Port of: gm/vertices.cpp#L103-L204 (chrome/m156), class VerticesGM
#[derive(Debug)]
pub struct VerticesGm {
    pts: [Point; K_MESH_VERTEX_CNT],
    texs: [Point; K_MESH_VERTEX_CNT],
    colors: [Color; K_MESH_VERTEX_CNT],
    shader1: Option<Shader>,
    shader2: Option<Shader>,
    color_filter: Option<skia_rust_core::color_filter::ColorFilter>,
    shader_scale: scalar,
}

impl VerticesGm {
    // Port of: gm/vertices.cpp#L111-L111 (chrome/m156), VerticesGM(SkScalar)
    #[must_use]
    pub fn new(shader_scale: scalar) -> Self {
        Self {
            pts: [Point::new(0.0, 0.0); K_MESH_VERTEX_CNT],
            texs: [Point::new(0.0, 0.0); K_MESH_VERTEX_CNT],
            colors: [Color::BLACK; K_MESH_VERTEX_CNT],
            shader1: None,
            shader2: None,
            color_filter: None,
            shader_scale,
        }
    }
}

impl GM for VerticesGm {
    // Port of: gm/vertices.cpp (chrome/m156), getName
    #[allow(clippy::float_cmp)] // the C++ compares the scale with 1 exactly
    fn name(&self) -> String {
        let mut name = "vertices".to_owned();
        if self.shader_scale != 1.0 {
            name.push_str("_scaled_shader");
        }
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(975, 1175)
    }

    // Port of: gm/vertices.cpp#L115-L122 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let (pts, texs, colors) = fill_mesh(self.shader_scale);
        self.pts = pts;
        self.texs = texs;
        self.colors = colors;
        self.shader1 = make_shader1(self.shader_scale);
        self.shader2 = Some(make_shader2());
        self.color_filter = make_color_filter();
    }

    // Port of: gm/vertices.cpp#L128-L185 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let modes = [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::DstATop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];

        let mut paint = Paint::default();

        canvas.translate((4.0, 4.0));
        for mode in modes {
            canvas.save();
            for alpha in [1.0_f32, 0.5] {
                for cf_is_some in [false, true] {
                    let cf = if cf_is_some {
                        self.color_filter.clone()
                    } else {
                        None
                    };
                    for shader_index in 0..2 {
                        let shader = if shader_index == 0 {
                            self.shader1.clone()
                        } else {
                            self.shader2.clone()
                        };
                        // (has colors, has texs)
                        for (has_colors, has_texs) in [(true, false), (false, true), (true, true)] {
                            paint.set_shader(shader.clone());
                            paint.set_color_filter(cf.clone());
                            paint.set_alpha_f(alpha);

                            let colors = has_colors.then_some(&self.colors[..]);
                            let texs = has_texs.then_some(&self.texs[..]);
                            let v = Vertices::new_copy(
                                VertexMode::TriangleFan,
                                &self.pts,
                                texs,
                                colors,
                                Some(&K_MESH_FAN[..]),
                            )
                            .expect("a vertices object");
                            canvas.draw_vertices(&v, mode, &paint);
                            canvas.translate((40.0, 0.0));
                        }
                    }
                }
            }
            canvas.restore();
            canvas.translate((0.0, 40.0));
        }
    }
}

// Port of: gm/vertices.cpp#L206-L206 (chrome/m156), DEF_GM(return new VerticesGM(1);)
crate::def_gm!(VerticesGM_1 = "VerticesGM(1)", VerticesGm::new(1.0));
// Port of: gm/vertices.cpp#L207 (chrome/m156), DEF_GM(return new VerticesGM(1 / kShaderSize);),
// with kShaderSize = 40 (an SkScalar, so the quotient is a float division).
crate::def_gm!(
    VerticesGM_1_over_kShaderSize = "VerticesGM(1 / kShaderSize)",
    VerticesGm::new(1.0_f32 / 40.0_f32)
);

// Port of: gm/vertices.cpp#L209-L227 (chrome/m156), draw_batching
fn draw_batching(canvas: &Canvas) {
    // Triangle fans can't batch so we convert to regular triangles,
    const NUM_TRIS: usize = K_MESH_FAN.len() - 2;
    let (pts, texs, colors) = fill_mesh(1.0);

    let mut matrices = Vec::new();
    matrices.push(Matrix::new_identity());
    matrices.push(Matrix::translate((0.0, 40.0)));
    {
        let mut m = Matrix::rotate_deg_pivot(45.0, (K_MESH_SIZE / 2.0, K_MESH_SIZE / 2.0));
        m.post_scale((1.2, 0.8), Point::new(K_MESH_SIZE / 2.0, K_MESH_SIZE / 2.0));
        m.post_translate((0.0, 80.0));
        matrices.push(m);
    }

    let shader = make_shader1(1.0);

    let mut indices = [0_u16; 3 * NUM_TRIS];
    for i in 0..NUM_TRIS {
        indices[3 * i] = K_MESH_FAN[0];
        indices[3 * i + 1] = K_MESH_FAN[i + 1];
        indices[3 * i + 2] = K_MESH_FAN[i + 2];
    }

    canvas.save();
    canvas.translate((10.0, 10.0));
    for use_shader in [false, true] {
        for use_tex in [false, true] {
            for m in &matrices {
                canvas.save();
                canvas.concat(m);
                let mut paint = Paint::default();
                paint.set_shader(if use_shader { shader.clone() } else { None });
                paint.set_color(Color::WHITE);

                let t = if use_tex { Some(&texs[..]) } else { None };
                let v = Vertices::new_copy(
                    VertexMode::Triangles,
                    &pts,
                    t,
                    Some(&colors[..]),
                    Some(&indices[..]),
                )
                .expect("a vertices object");
                canvas.draw_vertices(&v, BlendMode::Modulate, &paint);
                canvas.restore();
            }
            canvas.translate((0.0, 120.0));
        }
    }
    canvas.restore();
}

// Port of: gm/vertices.cpp#L287-L293 (chrome/m156), DEF_SIMPLE_GM(vertices_batching)
crate::def_simple_gm!(vertices_batching, canvas, 100, 500, {
    draw_batching(canvas);
    canvas.translate((50.0, 0.0));
    draw_batching(canvas);
});

// Test to ensure SkVertices::kTriangleStrip_VertexMode works properly.
// Port of: gm/vertices.cpp#L263-L285 (chrome/m156), DEF_SIMPLE_GM(vertices_strip)
crate::def_simple_gm!(vertices_strip, canvas, 600, 200, {
    // Create a quad respecting triangle strip ordering.
    let r = Rect::from_wh(128.0, 128.0);
    let pos = [
        Point::new(r.left, r.top),
        Point::new(r.left, r.bottom),
        Point::new(r.right, r.top),
        Point::new(r.right, r.bottom),
    ];

    let shader = make_shader1(1.0);
    let verts = Vertices::new_copy(VertexMode::TriangleStrip, &pos, None, None, None)
        .expect("a vertices object");

    let mut paint = Paint::default();
    for i in 0..4 {
        let color = if i % 2 == 0 { Color::RED } else { Color::BLUE };
        paint.set_color(color);
        paint.set_shader(if i >= 2 { shader.clone() } else { None });
        canvas.draw_vertices(&verts, BlendMode::SrcOver, &paint);
        canvas.translate((150.0, 0.0));
    }
});

// Test case for skbug.com/40041407. We need to draw the vertices twice (with different matrices) to
// trigger the bug.
// Port of: gm/vertices.cpp#L295-L330 (chrome/m156), DEF_SIMPLE_GM(vertices_perspective)
crate::def_simple_gm!(vertices_perspective, canvas, 256, 256, {
    let mut paint = Paint::default();
    paint.set_shader(create_checkerboard_shader(Color::BLACK, Color::WHITE, 32));

    let r = Rect::from_wh(128.0, 128.0);

    let pos = r.to_quad(None);
    let verts = Vertices::new_copy(VertexMode::TriangleFan, &pos, Some(&pos), None, None)
        .expect("a vertices object");

    let mut persp = Matrix::new_identity();
    persp.set_persp_y(1.0 / 100.0);

    canvas.save();
    canvas.concat(&persp);
    canvas.draw_rect(r, &paint);
    canvas.restore();

    canvas.save();
    canvas.translate((r.width(), 0.0));
    canvas.concat(&persp);
    canvas.draw_rect(r, &paint);
    canvas.restore();

    canvas.save();
    canvas.translate((0.0, r.height()));
    canvas.concat(&persp);
    canvas.draw_vertices(&verts, BlendMode::Modulate, &paint);
    canvas.restore();

    canvas.save();
    canvas.translate((r.width(), r.height()));
    canvas.concat(&persp);
    canvas.draw_vertices(&verts, BlendMode::Modulate, &paint);
    canvas.restore();
});

// Makes sure that drawVertices allows for triangles with "collapsed" UVs, where all three vertices
// have the same texture coordinate. b/40044794
// Port of: gm/vertices.cpp#L356-L370 (chrome/m156), DEF_SIMPLE_GM_BG(vertices_collapsed)
crate::def_simple_gm_bg!(vertices_collapsed, canvas, 50, 50, Color::WHITE, {
    let verts = [
        Point::new(5.0, 5.0),
        Point::new(45.0, 5.0),
        Point::new(45.0, 45.0),
        Point::new(5.0, 45.0),
    ];
    let texs = [Point::new(0.0, 0.0); 4];
    let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

    let v = Vertices::new_copy(
        VertexMode::Triangles,
        &verts,
        Some(&texs),
        None,
        Some(&indices),
    )
    .expect("a vertices object");

    let mut surf = skia_rust_raster::surfaces::raster_n32_premul((1, 1)).expect("a raster surface");
    surf.canvas().clear(Color::GREEN);
    let shader = surf.image_snapshot().expect("a snapshot").to_shader(
        None,
        skia_rust_core::sampling_options::SamplingOptions::default(),
        None,
    );
    let mut paint = Paint::default();
    paint.set_shader(shader);

    canvas.draw_vertices(&v, BlendMode::Dst, &paint);
});

// Port of: gm/vertices.cpp#L332-L352 (chrome/m156), DEF_SIMPLE_GM(skbug_13047)
crate::def_simple_gm!(skbug_13047, canvas, 200, 200, {
    let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");

    let w = image.width() as f32;
    let h = image.height() as f32;

    let verts = [
        Point::new(0.0, 0.0),
        Point::new(200.0, 0.0),
        Point::new(200.0, 200.0),
        Point::new(0.0, 200.0),
    ];
    let texs = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

    let v = Vertices::new_copy(
        VertexMode::Triangles,
        &verts,
        Some(&texs),
        None,
        Some(&indices),
    )
    .expect("a triangle list");

    let m = Matrix::scale((2.0, 2.0)); // ignored in CPU ???
    let s = image.to_shader(None, SamplingOptions::from(FilterMode::Linear), &m);

    let mut p = Paint::default();
    p.set_shader(s);

    canvas.draw_vertices(&v, BlendMode::Modulate, &p);
});
