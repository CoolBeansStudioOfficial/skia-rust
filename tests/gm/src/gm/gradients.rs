// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradients.cpp (chrome/m156)
//
// Not ported (the manifest entries stay `todo`):
// * `gradients_color_space`, `gradients_hue_method`: they label each gradient with text
//   (`SkFont`, Phase 5).
// * `LCH`, `OKLCH`, `HSL`, `HWB` (`DEF_POWERLESS_HUE_GM`): `ToolUtils::draw_checkerboard` is a
//   bitmap shader (image shaders, Phase 3).

// structure and names mirror the C++
#![allow(
    clippy::items_after_statements,
    clippy::match_same_arms,
    clippy::similar_names
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::color::colors;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::{scalar, scalar_interp};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::interpolation::{ColorSpace as InterpColorSpace, InPremul};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_tools::font_tool_utils::default_portable_font;

const G_COLORS: [Color4f; 5] = [
    colors::RED,
    colors::GREEN,
    colors::BLUE,
    colors::WHITE,
    colors::BLACK,
];
const G_POS0: [f32; 2] = [0.0, 1.0];
const G_POS1: [f32; 2] = [1.0 / 4.0, 1.0 * 3.0 / 4.0];
const G_POS2: [f32; 5] = [0.0, 1.0 / 8.0, 1.0 / 2.0, 1.0 * 7.0 / 8.0, 1.0];

const G_POS_CLAMP: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const G_COLOR4F_CLAMP: [Color4f; 4] = [colors::RED, colors::GREEN, colors::GREEN, colors::BLUE];

// Port of: gm/gradients.cpp#L24-L27 (chrome/m156)
struct GradData {
    colors: &'static [Color4f],
    count: usize,
    pos: &'static [f32],
}

impl GradData {
    /// `{{data.fColors, data.fPos, tm}, {}}`.
    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(
                &self.colors[..self.count],
                if self.pos.is_empty() {
                    None
                } else {
                    Some(self.pos)
                },
                tm,
                None,
            ),
            Interpolation::default(),
        )
    }
}

// Port of: gm/gradients.cpp#L51-L58 (chrome/m156)
static G_GRAD_DATA: [GradData; 6] = [
    GradData {
        colors: &G_COLORS,
        count: 2,
        pos: &[],
    },
    GradData {
        colors: &G_COLORS,
        count: 2,
        pos: &G_POS0,
    },
    GradData {
        colors: &G_COLORS,
        count: 2,
        pos: &G_POS1,
    },
    GradData {
        colors: &G_COLORS,
        count: 5,
        pos: &[],
    },
    GradData {
        colors: &G_COLORS,
        count: 5,
        pos: &G_POS2,
    },
    GradData {
        colors: &G_COLOR4F_CLAMP,
        count: 4,
        pos: &G_POS_CLAMP,
    },
];

// Port of: gm/gradients.cpp#L60-L63 (chrome/m156)
fn make_linear(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    gradient_shaders::linear_gradient((pts[0], pts[1]), &data.grad(tm), Some(local_matrix))
}

// Port of: gm/gradients.cpp#L65-L72 (chrome/m156)
fn make_radial(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    gradient_shaders::radial_gradient((center, center.x), &data.grad(tm), Some(local_matrix))
}

// Port of: gm/gradients.cpp#L74-L81 (chrome/m156)
fn make_sweep(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    gradient_shaders::sweep_gradient(center, (0.0, 360.0), &data.grad(tm), Some(local_matrix))
}

// Port of: gm/gradients.cpp#L83-L94 (chrome/m156)
fn make_2_radial(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    let center1 = Point::new(
        scalar_interp(pts[0].x, pts[1].x, 3.0 / 5.0),
        scalar_interp(pts[0].y, pts[1].y, 1.0 / 4.0),
    );
    gradient_shaders::two_point_conical_gradient(
        (center1, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        &data.grad(tm),
        Some(local_matrix),
    )
}

// Port of: gm/gradients.cpp#L96-L107 (chrome/m156)
fn make_2_conical(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 10.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    gradient_shaders::two_point_conical_gradient(
        (center1, radius1),
        (center0, radius0),
        &data.grad(tm),
        Some(local_matrix),
    )
}

type GradMaker = fn(&[Point; 2], &GradData, TileMode, &Matrix) -> Option<Shader>;
// Port of: gm/gradients.cpp#L109-L117 (chrome/m156)
const G_GRAD_MAKERS: [GradMaker; 5] = [
    make_linear,
    make_radial,
    make_sweep,
    make_2_radial,
    make_2_conical,
];

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients.cpp#L121-L174 (chrome/m156)
struct GradientsGm {
    dither: bool,
    bg: Color,
}

impl GradientsGm {
    fn new(dither: bool) -> Self {
        Self {
            dither,
            bg: Color::WHITE,
        }
    }

    fn draw(&self, canvas: &Canvas) {
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        let tm = TileMode::Clamp;
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);

        canvas.translate((20.0, 20.0));
        for (i, data) in G_GRAD_DATA.iter().enumerate() {
            canvas.save();
            for maker in &G_GRAD_MAKERS {
                let mut scale = Matrix::new_identity();

                if i == 5 {
                    // if the clamp case
                    scale.set_scale((0.5, 0.5), None);
                    scale.post_translate((25.0, 25.0));
                }

                paint.set_shader(maker(&pts, data, tm, &scale));
                canvas.draw_rect(r, &paint);
                canvas.translate((0.0, 120.0));
            }
            canvas.restore();
            canvas.translate((120.0, 0.0));
        }
    }
}

impl GM for GradientsGm {
    fn name(&self) -> String {
        if self.dither {
            "gradients"
        } else {
            "gradients_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(840, 815)
    }

    fn bg_color(&self) -> Color {
        self.bg
    }

    fn on_once_before_draw(&mut self) {
        self.bg = Color::new(0xFFDD_DDDD);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw(canvas);
    }
}

// Port of: gm/gradients.cpp#L175-L176 (chrome/m156)
crate::def_gm!(
    GradientsGM_true = "GradientsGM(true)",
    GradientsGm::new(true)
);
crate::def_gm!(
    GradientsGM_false = "GradientsGM(false)",
    GradientsGm::new(false)
);

// Based on the original gradient slide, but with perspective applied to the
// gradient shaders' local matrices
// Port of: gm/gradients.cpp#L178-L227 (chrome/m156)
struct GradientsLocalPerspectiveGm {
    dither: bool,
}

impl GM for GradientsLocalPerspectiveGm {
    fn name(&self) -> String {
        if self.dither {
            "gradients_local_perspective"
        } else {
            "gradients_local_perspective_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(840, 815)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDD_DDDD)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
    fn on_draw(&mut self, canvas: &Canvas) {
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        let tm = TileMode::Clamp;
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);

        canvas.translate((20.0, 20.0));
        for (i, data) in G_GRAD_DATA.iter().enumerate() {
            canvas.save();
            for maker in &G_GRAD_MAKERS {
                // apply an increasing y perspective as we move to the right
                let mut perspective = Matrix::new_identity();
                perspective.set_persp_y((i + 1) as f32 / 500.0);
                perspective.set_skew_x((i + 1) as f32 / 10.0);

                paint.set_shader(maker(&pts, data, tm, &perspective));
                canvas.draw_rect(r, &paint);
                canvas.translate((0.0, 120.0));
            }
            canvas.restore();
            canvas.translate((120.0, 0.0));
        }
    }
}

// Port of: gm/gradients.cpp#L228-L229 (chrome/m156)
crate::def_gm!(
    GradientsLocalPerspectiveGM_true = "GradientsLocalPerspectiveGM(true)",
    GradientsLocalPerspectiveGm { dither: true }
);
crate::def_gm!(
    GradientsLocalPerspectiveGM_false = "GradientsLocalPerspectiveGM(false)",
    GradientsLocalPerspectiveGm { dither: false }
);

// Based on the original gradient slide, but with perspective applied to
// the view matrix
// Port of: gm/gradients.cpp#L231-L258 (chrome/m156)
struct GradientsViewPerspectiveGm {
    inner: GradientsGm,
}

impl GM for GradientsViewPerspectiveGm {
    fn name(&self) -> String {
        if self.inner.dither {
            "gradients_view_perspective"
        } else {
            "gradients_view_perspective_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(840, 500)
    }

    fn bg_color(&self) -> Color {
        self.inner.bg_color()
    }

    fn on_once_before_draw(&mut self) {
        self.inner.on_once_before_draw();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut perspective = Matrix::new_identity();
        perspective.set_persp_y(0.001);
        perspective.set_skew_x(8.0 / 25.0);
        canvas.concat(&perspective);
        self.inner.on_draw(canvas);
    }
}

// Port of: gm/gradients.cpp#L259-L260 (chrome/m156)
crate::def_gm!(
    GradientsViewPerspectiveGM_true = "GradientsViewPerspectiveGM(true)",
    GradientsViewPerspectiveGm {
        inner: GradientsGm::new(true)
    }
);
crate::def_gm!(
    GradientsViewPerspectiveGM_false = "GradientsViewPerspectiveGM(false)",
    GradientsViewPerspectiveGm {
        inner: GradientsGm::new(false)
    }
);

// Inspired by this <canvas> javascript, where we need to detect that we are not
// solving a quadratic equation, but must instead solve a linear (since our X^2
// coefficient is 0)
//
// ctx.fillStyle = '#f00';
// ctx.fillRect(0, 0, 100, 50);
//
// var g = ctx.createRadialGradient(-80, 25, 70, 0, 25, 150);
// g.addColorStop(0, '#f00');
// g.addColorStop(0.01, '#0f0');
// g.addColorStop(0.99, '#0f0');
// g.addColorStop(1, '#f00');
// ctx.fillStyle = g;
// ctx.fillRect(0, 0, 100, 50);
// Port of: gm/gradients.cpp#L262-L309 (chrome/m156)
struct GradientsDegenrate2PointGm {
    dither: bool,
}

impl GM for GradientsDegenrate2PointGm {
    fn name(&self) -> String {
        if self.dither {
            "gradients_degenerate_2pt"
        } else {
            "gradients_degenerate_2pt_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(320, 320)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::BLUE, None);

        let colors = [colors::RED, colors::GREEN, colors::GREEN, colors::RED];
        let pos = [0.0, 0.01, 0.99, 1.0];
        let c0 = Point::new(-80.0, 25.0);
        let r0 = 70.0;
        let c1 = Point::new(0.0, 25.0);
        let r1 = 150.0;
        let mut paint = Paint::default();
        paint.set_shader(gradient_shaders::two_point_conical_gradient(
            (c0, r0),
            (c1, r1),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        paint.set_dither(self.dither);
        canvas.draw_paint(&paint);
    }
}

// Port of: gm/gradients.cpp#L310-L311 (chrome/m156)
crate::def_gm!(
    GradientsDegenrate2PointGM_true = "GradientsDegenrate2PointGM(true)",
    GradientsDegenrate2PointGm { dither: true }
);
crate::def_gm!(
    GradientsDegenrate2PointGM_false = "GradientsDegenrate2PointGM(false)",
    GradientsDegenrate2PointGm { dither: false }
);

// skbug.com/40031542
// <canvas id="canvas"></canvas>
// <script>
// var c = document.getElementById("canvas");
// var ctx = c.getContext("2d");
// ctx.fillStyle = '#ff0';
// ctx.fillRect(0, 0, 100, 50);
//
// var g = ctx.createRadialGradient(200, 25, 20, 200, 25, 10);
// g.addColorStop(0, '#0f0');
// g.addColorStop(0.003, '#f00');  // 0.004 makes this work
// g.addColorStop(1, '#ff0');
// ctx.fillStyle = g;
// ctx.fillRect(0, 0, 100, 50);
// </script>

// should draw only green
// Port of: gm/gradients.cpp#L328-L346 (chrome/m156)
crate::def_simple_gm!(small_color_stop, canvas, 100, 150, {
    let colors = [colors::GREEN, colors::RED, colors::YELLOW];
    let pos = [0.0, 0.003, 1.0]; // 0.004f makes this work
    let c0 = Point::new(200.0, 25.0);
    let r0 = 20.0;
    let c1 = Point::new(200.0, 25.0);
    let r1 = 10.0;

    let mut paint = Paint::default();
    paint.set_color(Color::YELLOW);
    canvas.draw_rect(Rect::from_wh(100.0, 150.0), &paint);
    paint.set_shader(gradient_shaders::two_point_conical_gradient(
        (c0, r0),
        (c1, r1),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_rect(Rect::from_wh(100.0, 150.0), &paint);
});

/// Tests correctness of *optimized* codepaths in gradients.
// Port of: gm/gradients.cpp#L349-L378 (chrome/m156)
struct ClampedGradientsGm {
    dither: bool,
}

impl GM for ClampedGradientsGm {
    fn name(&self) -> String {
        if self.dither {
            "clamped_gradients"
        } else {
            "clamped_gradients_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 510)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::new(0xFFDD_DDDD), None);

        let r = Rect::new(0.0, 0.0, 100.0, 300.0);
        let mut paint = Paint::default();
        paint.set_dither(self.dither);
        paint.set_anti_alias(true);

        let center = Point::new(0.0, 300.0);
        canvas.translate((20.0, 20.0));
        paint.set_shader(gradient_shaders::radial_gradient(
            (center, 200.0),
            &Gradient::new(
                Colors::new(&G_COLORS, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_rect(r, &paint);
    }
}

// Port of: gm/gradients.cpp#L379-L380 (chrome/m156)
crate::def_gm!(
    ClampedGradientsGM_true = "ClampedGradientsGM(true)",
    ClampedGradientsGm { dither: true }
);
crate::def_gm!(
    ClampedGradientsGM_false = "ClampedGradientsGM(false)",
    ClampedGradientsGm { dither: false }
);

/// Checks quality of large radial gradients, which may display
/// some banding.
// Port of: gm/gradients.cpp#L385-L416 (chrome/m156)
struct RadialGradientGm;

impl GM for RadialGradientGm {
    fn name(&self) -> String {
        "radial_gradient".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1280, 1280)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
    fn on_draw(&mut self, canvas: &Canvas) {
        let dim = self.size();

        canvas.draw_color(Color::new(0xFF00_0000), None);

        let mut paint = Paint::default();
        paint.set_dither(true);
        let center = Point::new(dim.width as f32 / 2.0, dim.height as f32 / 2.0);
        let radius = dim.width as f32 / 2.0;
        let pos = [0.0, 0.35, 1.0];
        // SkColorConverter conv({ 0x7f7f7f7f, 0x7f7f7f7f, 0xb2000000 });
        let conv =
            [0x7f7f_7f7f_u32, 0x7f7f_7f7f, 0xb200_0000].map(|c| Color4f::from_color(Color::new(c)));
        paint.set_shader(gradient_shaders::radial_gradient(
            (center, radius),
            &Gradient::new(
                Colors::new(&conv, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        let r = Rect::new(0.0, 0.0, dim.width as f32, dim.height as f32);
        canvas.draw_rect(r, &paint);
    }
}

// Port of: gm/gradients.cpp#L417 (chrome/m156)
crate::def_gm!(RadialGradientGM, RadialGradientGm);

// Port of: gm/gradients.cpp#L419-L469 (chrome/m156)
struct RadialGradient2Gm {
    dither: bool,
}

impl GM for RadialGradient2Gm {
    fn name(&self) -> String {
        if self.dither {
            "radial_gradient2"
        } else {
            "radial_gradient2_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 400)
    }

    // Reproduces the example given in b/7671058.
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint1 = Paint::default();
        let mut paint2 = Paint::default();
        let mut paint3 = Paint::default();
        paint1.set_style(Style::Fill);
        paint2.set_style(Style::Fill);
        paint3.set_style(Style::Fill);

        let c4 = Color4f::new;
        let sweep_colors = [
            c4(1.0, 0.0, 0.0, 1.0),
            c4(1.0, 1.0, 0.0, 1.0),
            c4(0.0, 1.0, 0.0, 1.0),
            c4(0.0, 1.0, 1.0, 1.0),
            c4(0.0, 0.0, 1.0, 1.0),
            c4(1.0, 0.0, 1.0, 1.0),
            c4(1.0, 0.0, 0.0, 1.0),
        ];
        let colors1 = [c4(1.0, 1.0, 1.0, 1.0), c4(0.0, 0.0, 0.0, 0.0)];
        let colors2 = [c4(0.0, 0.0, 0.0, 1.0), c4(0.0, 0.0, 0.0, 0.0)];

        let (cx, cy, radius) = (200.0, 200.0, 150.0);
        let center = Point::new(cx, cy);

        // We can either interpolate endpoints and premultiply each point (default, more
        // precision), or premultiply the endpoints first, avoiding the need to premultiply each
        // point (cheap).
        let flags = [InPremul::No, InPremul::Yes];
        let tm = TileMode::Clamp;

        for flag in flags {
            let terp = Interpolation {
                in_premul: flag,
                ..Interpolation::default()
            };
            paint1.set_shader(gradient_shaders::sweep_gradient(
                center,
                (0.0, 360.0),
                &Gradient::new(Colors::new(&sweep_colors, None, tm, None), terp),
                None,
            ));
            paint2.set_shader(gradient_shaders::radial_gradient(
                (center, radius),
                &Gradient::new(Colors::new(&colors1, None, tm, None), terp),
                None,
            ));
            paint3.set_shader(gradient_shaders::radial_gradient(
                (center, radius),
                &Gradient::new(Colors::new(&colors2, None, tm, None), terp),
                None,
            ));
            paint1.set_dither(self.dither);
            paint2.set_dither(self.dither);
            paint3.set_dither(self.dither);

            canvas.draw_circle((cx, cy), radius, &paint1);
            canvas.draw_circle((cx, cy), radius, &paint3);
            canvas.draw_circle((cx, cy), radius, &paint2);

            canvas.translate((400.0, 0.0));
        }
    }
}

// Port of: gm/gradients.cpp#L470-L471 (chrome/m156)
crate::def_gm!(
    RadialGradient2GM_true = "RadialGradient2GM(true)",
    RadialGradient2Gm { dither: true }
);
crate::def_gm!(
    RadialGradient2GM_false = "RadialGradient2GM(false)",
    RadialGradient2Gm { dither: false }
);

// Shallow radial (shows banding on raster)
// Port of: gm/gradients.cpp#L473-L507 (chrome/m156)
struct RadialGradient3Gm {
    shader: Option<Shader>,
    dither: bool,
}

impl GM for RadialGradient3Gm {
    fn name(&self) -> String {
        if self.dither {
            "radial_gradient3"
        } else {
            "radial_gradient3_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_once_before_draw(&mut self) {
        let center = Point::new(0.0, 0.0);
        let k_radius = 3000.0;
        let k_colors = [
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
        ];
        self.shader = gradient_shaders::radial_gradient(
            (center, k_radius),
            &Gradient::new(
                Colors::new(&k_colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_shader(self.shader.clone());
        paint.set_dither(self.dither);
        canvas.draw_rect(Rect::from_wh(500.0, 500.0), &paint);
    }
}

// Port of: gm/gradients.cpp#L508-L509 (chrome/m156)
crate::def_gm!(
    RadialGradient3GM_true = "RadialGradient3GM(true)",
    RadialGradient3Gm {
        shader: None,
        dither: true
    }
);
crate::def_gm!(
    RadialGradient3GM_false = "RadialGradient3GM(false)",
    RadialGradient3Gm {
        shader: None,
        dither: false
    }
);

// Port of: gm/gradients.cpp#L511-L546 (chrome/m156)
struct RadialGradient4Gm {
    shader: Option<Shader>,
    dither: bool,
}

impl GM for RadialGradient4Gm {
    fn name(&self) -> String {
        if self.dither {
            "radial_gradient4"
        } else {
            "radial_gradient4_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_once_before_draw(&mut self) {
        let center = Point::new(250.0, 250.0);
        let k_radius = 250.0;
        let colors = [
            colors::RED,
            colors::RED,
            colors::WHITE,
            colors::WHITE,
            colors::RED,
        ];
        let pos = [0.0, 0.4, 0.4, 0.8, 0.8];
        self.shader = gradient_shaders::radial_gradient(
            (center, k_radius),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);
        paint.set_shader(self.shader.clone());
        canvas.draw_rect(Rect::from_wh(500.0, 500.0), &paint);
    }
}

// Port of: gm/gradients.cpp#L547-L548 (chrome/m156)
crate::def_gm!(
    RadialGradient4GM_true = "RadialGradient4GM(true)",
    RadialGradient4Gm {
        shader: None,
        dither: true
    }
);
crate::def_gm!(
    RadialGradient4GM_false = "RadialGradient4GM(false)",
    RadialGradient4Gm {
        shader: None,
        dither: false
    }
);

// Port of: gm/gradients.cpp#L550-L598 (chrome/m156)
struct LinearGradientGm {
    shader: Vec<Option<Shader>>,
    dither: bool,
}

impl LinearGradientGm {
    const K_WIDTH_BUMP: f32 = 30.0;
    const K_HEIGHT: f32 = 5.0;
    const K_MIN_WIDTH: f32 = 540.0;

    fn new(dither: bool) -> Self {
        Self {
            shader: Vec::new(),
            dither,
        }
    }
}

impl GM for LinearGradientGm {
    fn name(&self) -> String {
        if self.dither {
            "linear_gradient"
        } else {
            "linear_gradient_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    #[allow(clippy::cast_precision_loss)] // int * SkScalar arithmetic as in C++
    fn on_once_before_draw(&mut self) {
        let mut pts = [Point::new(0.0, 0.0), Point::new(0.0, 0.0)];
        let colors = [
            colors::WHITE,
            colors::WHITE,
            Color4f::from_color(Color::new(0xFF00_8200)),
            Color4f::from_color(Color::new(0xFF00_8200)),
            colors::WHITE,
            colors::WHITE,
        ];
        let unit_pos: [f32; 5] = [0.0, 50.0, 70.0, 500.0, 540.0];
        let mut pos = [0.0f32; 6];
        pos[5] = 1.0;
        self.shader = Vec::with_capacity(100);
        for index in 0..100 {
            pts[1].x = 500.0 + index as f32 * Self::K_WIDTH_BUMP;
            for (inner, unit) in unit_pos.iter().enumerate() {
                pos[inner] = unit / (Self::K_MIN_WIDTH + index as f32 * Self::K_WIDTH_BUMP);
            }
            self.shader.push(gradient_shaders::linear_gradient(
                (pts[0], pts[1]),
                &Gradient::new(
                    Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                    Interpolation::default(),
                ),
                None,
            ));
        }
    }

    #[allow(clippy::cast_precision_loss)] // int * SkScalar arithmetic as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);
        for (index, shader) in self.shader.iter().enumerate() {
            paint.set_shader(shader.clone());
            canvas.draw_rect(
                Rect::new(
                    0.0,
                    index as f32 * Self::K_HEIGHT,
                    Self::K_MIN_WIDTH + index as f32 * Self::K_WIDTH_BUMP,
                    (index as f32 + 1.0) * Self::K_HEIGHT,
                ),
                &paint,
            );
        }
    }
}

// Port of: gm/gradients.cpp#L599-L600 (chrome/m156)
crate::def_gm!(
    LinearGradientGM_true = "LinearGradientGM(true)",
    LinearGradientGm::new(true)
);
crate::def_gm!(
    LinearGradientGM_false = "LinearGradientGM(false)",
    LinearGradientGm::new(false)
);

// Port of: gm/gradients.cpp#L602-L645 (chrome/m156)
struct LinearGradientTinyGm;

impl GM for LinearGradientTinyGm {
    fn name(&self) -> String {
        "linear_gradient_tiny".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 500)
    }

    #[allow(clippy::cast_precision_loss)] // unsigned * SkScalar arithmetic as in C++
    #[allow(clippy::excessive_precision)] // Skia's literals kept verbatim
    fn on_draw(&mut self, canvas: &Canvas) {
        const K_RECT_SIZE: f32 = 100.0;
        const K_STOP_COUNT: usize = 3;
        let colors: [Color4f; K_STOP_COUNT] = [colors::GREEN, colors::RED, colors::GREEN];
        struct Configs {
            pts: [Point; 2],
            pos: [f32; K_STOP_COUNT],
        }
        let p = Point::new;
        let configs = [
            Configs {
                pts: [p(0.0, 0.0), p(10.0, 0.0)],
                pos: [0.0, 0.999_999, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(10.0, 0.0)],
                pos: [0.0, 0.000_001, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(10.0, 0.0)],
                pos: [0.0, 0.999_999_999, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(10.0, 0.0)],
                pos: [0.0, 0.000_000_001, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.0, 10.0)],
                pos: [0.0, 0.999_999, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.0, 10.0)],
                pos: [0.0, 0.000_001, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.0, 10.0)],
                pos: [0.0, 0.999_999_999, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.0, 10.0)],
                pos: [0.0, 0.000_000_001, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.000_01, 0.0)],
                pos: [0.0, 0.5, 1.0],
            },
            Configs {
                pts: [p(9.999_99, 0.0), p(10.0, 0.0)],
                pos: [0.0, 0.5, 1.0],
            },
            Configs {
                pts: [p(0.0, 0.0), p(0.0, 0.000_01)],
                pos: [0.0, 0.5, 1.0],
            },
            Configs {
                pts: [p(0.0, 9.999_99), p(0.0, 10.0)],
                pos: [0.0, 0.5, 1.0],
            },
        ];

        let mut paint = Paint::default();
        for (i, config) in configs.iter().enumerate() {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            paint.set_shader(gradient_shaders::linear_gradient(
                (config.pts[0], config.pts[1]),
                &Gradient::new(
                    Colors::new(&colors, Some(&config.pos), TileMode::Clamp, None),
                    Interpolation::default(),
                ),
                None,
            ));
            canvas.translate((
                K_RECT_SIZE * (((i % 4) as f32) * 1.5 + 0.25),
                K_RECT_SIZE * (((i / 4) as f32) * 1.5 + 0.25),
            ));

            canvas.draw_rect(Rect::from_wh(K_RECT_SIZE, K_RECT_SIZE), &paint);
        }
    }
}

// Port of: gm/gradients.cpp#L647 (chrome/m156)
crate::def_gm!(LinearGradientTinyGM, LinearGradientTinyGm);

///////////////////////////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients.cpp#L651-L660 (chrome/m156)
struct GradRun {
    colors: [Color4f; 4],
    pos: [f32; 4],
    count: usize,
}

impl GradRun {
    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(
                &self.colors[..self.count],
                Some(&self.pos[..self.count]),
                tm,
                None,
            ),
            Interpolation::default(),
        )
    }
}

const SIZE: f32 = 121.0;

// Port of: gm/gradients.cpp#L664-L667 (chrome/m156)
fn make_linear_run(run: &GradRun, mode: TileMode) -> Option<Shader> {
    let pts = [Point::new(30.0, 30.0), Point::new(SIZE - 30.0, SIZE - 30.0)];
    gradient_shaders::linear_gradient((pts[0], pts[1]), &run.grad(mode), None)
}

// Port of: gm/gradients.cpp#L669-L672 (chrome/m156)
fn make_radial_run(run: &GradRun, mode: TileMode) -> Option<Shader> {
    let half = SIZE * 0.5;
    gradient_shaders::radial_gradient((Point::new(half, half), half - 10.0), &run.grad(mode), None)
}

// Port of: gm/gradients.cpp#L674-L678 (chrome/m156)
fn make_conical_run(run: &GradRun, mode: TileMode) -> Option<Shader> {
    let half = SIZE * 0.5;
    let center = Point::new(half, half);
    gradient_shaders::two_point_conical_gradient(
        (center, 20.0),
        (center, half - 10.0),
        &run.grad(mode),
        None,
    )
}

// Port of: gm/gradients.cpp#L680-L683 (chrome/m156)
fn make_sweep_run(run: &GradRun, mode: TileMode) -> Option<Shader> {
    let half = SIZE * 0.5;
    gradient_shaders::sweep_gradient(Point::new(half, half), (0.0, 360.0), &run.grad(mode), None)
}

// Exercise duplicate color-stops, at the ends, and in the middle
//
// At the time of this writing, only Linear correctly deals with duplicates at the ends,
// and then only correctly on CPU backend.
// Port of: gm/gradients.cpp#L685-L735 (chrome/m156)
crate::def_simple_gm!(gradients_dup_color_stops, canvas, 704, 564, {
    let pre_color = colors::RED; // clamp color before start
    let post_color = colors::BLUE; // clamp color after end
    let color0 = colors::BLACK;
    let color1 = colors::GREEN;
    // should never be seen, fills out fixed-size array
    let bad_color = Color4f::from_color(Color::new(0xFF33_88BB));

    let runs = [
        GradRun {
            colors: [color0, color1, bad_color, bad_color],
            pos: [0.0, 1.0, -1.0, -1.0],
            count: 2,
        },
        GradRun {
            colors: [pre_color, color0, color1, bad_color],
            pos: [0.0, 0.0, 1.0, -1.0],
            count: 3,
        },
        GradRun {
            colors: [color0, color1, post_color, bad_color],
            pos: [0.0, 1.0, 1.0, -1.0],
            count: 3,
        },
        GradRun {
            colors: [pre_color, color0, color1, post_color],
            pos: [0.0, 0.0, 1.0, 1.0],
            count: 4,
        },
        GradRun {
            colors: [color0, color0, color1, color1],
            pos: [0.0, 0.5, 0.5, 1.0],
            count: 4,
        },
    ];
    let factories: [fn(&GradRun, TileMode) -> Option<Shader>; 4] = [
        make_linear_run,
        make_radial_run,
        make_conical_run,
        make_sweep_run,
    ];

    let rect = Rect::from_wh(SIZE, SIZE);
    let dx = SIZE + 20.0;
    let dy = SIZE + 20.0;
    let mode = TileMode::Clamp;

    let mut paint = Paint::default();
    canvas.translate((10.0, 10.0 - dy));
    for factory in factories {
        canvas.translate((0.0, dy));
        let _acr = AutoCanvasRestore::guard(canvas, true);
        for run in &runs {
            paint.set_shader(factory(run, mode));
            canvas.draw_rect(rect, &paint);
            canvas.translate((dx, 0.0));
        }
    }
});

// Port of: gm/gradients.cpp#L737-L760 (chrome/m156)
fn draw_many_stops(canvas: &Canvas) {
    const K_STOP_COUNT: usize = 200;
    let pts = [Point::new(50.0, 50.0), Point::new(450.0, 450.0)];

    let mut colors = [colors::RED; K_STOP_COUNT];
    for (i, color) in colors.iter_mut().enumerate() {
        *color = match i % 5 {
            0 => colors::RED,
            1 | 2 => colors::GREEN,
            3 => colors::BLUE,
            _ => colors::RED,
        };
    }

    let mut p = Paint::default();
    p.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));

    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 500.0, 500.0), &p);
}

// Port of: gm/gradients.cpp#L762-L764 (chrome/m156)
crate::def_simple_gm!(gradient_many_stops, canvas, 500, 500, {
    draw_many_stops(canvas);
});

// Port of: gm/gradients.cpp#L766-L792 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors the unsigned -> float conversions
fn draw_many_hard_stops(canvas: &Canvas) {
    const K_STOP_COUNT: usize = 300;
    let pts = [Point::new(50.0, 50.0), Point::new(450.0, 450.0)];

    let mut colors = [colors::RED; K_STOP_COUNT];
    let mut pos = [0.0f32; K_STOP_COUNT];
    for i in 0..K_STOP_COUNT {
        colors[i] = match i % 6 {
            0 => colors::RED,
            1 | 2 => colors::GREEN,
            3 | 4 => colors::BLUE,
            _ => colors::RED,
        };
        pos[i] = (2.0 * ((i / 2) as f32)) / K_STOP_COUNT as f32;
    }

    let mut p = Paint::default();
    p.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));

    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 500.0, 500.0), &p);
}

// Port of: gm/gradients.cpp#L794-L796 (chrome/m156)
crate::def_simple_gm!(gradient_many_hard_stops, canvas, 500, 500, {
    draw_many_hard_stops(canvas);
});

// Port of: gm/gradients.cpp#L936-L963 (chrome/m156)
crate::def_simple_gm!(sweep_tiling, canvas, 690, 512, {
    const SIZE: f32 = 160.0;
    let colors = [colors::BLUE, colors::YELLOW, colors::GREEN];
    let pos = [0.0, 0.25, 0.50];

    let modes = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];

    // { start, end }
    let angles: [(f32, f32); 4] = [
        (-330.0, -270.0),
        (30.0, 90.0),
        (390.0, 450.0),
        (-30.0, 800.0),
    ];

    let mut p = Paint::default();
    let r = Rect::from_wh(SIZE, SIZE);

    for mode in modes {
        {
            let _acr = AutoCanvasRestore::guard(canvas, true);

            for angle in angles {
                p.set_shader(gradient_shaders::sweep_gradient(
                    Point::new(SIZE / 2.0, SIZE / 2.0),
                    (angle.0, angle.1),
                    &Gradient::new(
                        Colors::new(&colors, Some(&pos), mode, None),
                        Interpolation::default(),
                    ),
                    None,
                ));

                canvas.draw_rect(r, &p);
                canvas.translate((SIZE * 1.1, 0.0));
            }
        }
        canvas.translate((0.0, SIZE * 1.1));
    }
});

// Port of: gm/gradients.cpp#L965-L981 (chrome/m156)
crate::def_simple_gm!(rgbw_sweep_gradient, canvas, 100, 100, {
    const SIZE: f32 = 100.0;
    let colors = [
        colors::WHITE,
        colors::WHITE,
        colors::BLUE,
        colors::BLUE,
        colors::RED,
        colors::RED,
        colors::GREEN,
        colors::GREEN,
    ];
    let pos = [0.0, 0.25, 0.25, 0.50, 0.50, 0.75, 0.75, 1.0];

    let mut p = Paint::default();
    p.set_shader(gradient_shaders::sweep_gradient(
        Point::new(SIZE / 2.0, SIZE / 2.0),
        (0.0, 360.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_rect(Rect::from_wh(SIZE, SIZE), &p);
});

// Exercises the special-case Ganesh gradient effects.
// Port of: gm/gradients.cpp#L983-L1033 (chrome/m156)
crate::def_simple_gm!(gradients_interesting, canvas, 640, 1300, {
    let colors2 = [colors::RED, colors::BLUE];
    let colors3 = [colors::RED, colors::YELLOW, colors::BLUE];
    let colors4 = [colors::RED, colors::YELLOW, colors::YELLOW, colors::BLUE];

    let soft_right = [0.0, 0.999, 1.0]; // Based on Android launcher "clipping"
    let hard_left = [0.0, 0.0, 1.0];
    let hard_right = [0.0, 1.0, 1.0];
    let hard_center = [0.0, 0.5, 0.5, 1.0];

    let configs: [(&[Color4f], Option<&[f32]>); 6] = [
        (&colors2, None),               // kTwo_ColorType
        (&colors3, None),               // kThree_ColorType (simple)
        (&colors3, Some(&soft_right)),  // kThree_ColorType (tricky)
        (&colors3, Some(&hard_left)),   // kHardStopLeftEdged_ColorType
        (&colors3, Some(&hard_right)),  // kHardStopRightEdged_ColorType
        (&colors4, Some(&hard_center)), // kSingleHardStop_ColorType
    ];

    let modes = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];

    const SIZE: f32 = 200.0;
    let pts = [
        Point::new(SIZE / 3.0, SIZE / 3.0),
        Point::new(SIZE * 2.0 / 3.0, SIZE * 2.0 / 3.0),
    ];

    let mut p = Paint::default();
    for cfg in &configs {
        {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            for mode in modes {
                let grad = Gradient::new(
                    Colors::new(cfg.0, cfg.1, mode, None),
                    Interpolation::default(),
                );
                p.set_shader(gradient_shaders::linear_gradient(
                    (pts[0], pts[1]),
                    &grad,
                    None,
                ));
                canvas.draw_rect(Rect::from_wh(SIZE, SIZE), &p);
                canvas.translate((SIZE * 1.1, 0.0));
            }
        }
        canvas.translate((0.0, SIZE * 1.1));
    }
});

// Port of: gm/gradients.cpp#L1100-L1124 (chrome/m156)
crate::def_simple_gm_bg!(
    #[ignore = "see notes/gm-gradients.cpp-OKLCH-libm.md"]
    gradients_color_space_tilemode,
    canvas,
    360,
    105,
    Color::new(0xFF88_8888),
    {
        // Test exotic (CSS) gradient color spaces in conjunction with tile modes. Rather than test
        // every combination, we pick one color space that has a sufficiently strange interpolated
        // representation (OKLCH) and just use that. We're mostly interested in making sure that
        // things like decal mode are implemented at the correct time in the pipeline, relative to
        // hue conversion, re-premultiplication, etc.
        let pts = [Point::new(20.0, 0.0), Point::new(120.0, 0.0)];
        let colors = [colors::BLUE, colors::YELLOW];

        let mut p = Paint::default();
        let interpolation = Interpolation {
            color_space: InterpColorSpace::OKLCH,
            ..Interpolation::default()
        };

        canvas.translate((5.0, 5.0));

        for tm in [
            TileMode::Clamp,
            TileMode::Repeat,
            TileMode::Mirror,
            TileMode::Decal,
        ] {
            let g = Gradient::new(Colors::new(&colors, None, tm, None), interpolation);
            p.set_shader(gradient_shaders::linear_gradient(
                (pts[0], pts[1]),
                &g,
                None,
            ));
            canvas.draw_rect(Rect::new(0.0, 0.0, 350.0, 20.0), &p);
            canvas.translate((0.0, 25.0));
        }
    }
);

// Port of: gm/gradients.cpp#L1126-L1163 (chrome/m156)
crate::def_simple_gm_bg!(
    #[ignore = "see notes/gm-gradients.cpp-OKLCH-libm.md"]
    gradients_color_space_many_stops,
    canvas,
    500,
    500,
    Color::new(0xFF88_8888),
    {
        // Test exotic (CSS) gradient color spaces with many stops. Rather than test every
        // combination, we pick one color space that has a sufficiently strange interpolated
        // representation (OKLCH) and just use that. We're mostly interested in making sure that the
        // texture fallback on GPU works correctly.
        let pts = [Point::new(50.0, 50.0), Point::new(450.0, 465.0)];

        const K_STOP_COUNT: usize = 200;
        let mut colors = [colors::RED; K_STOP_COUNT];
        for (i, color) in colors.iter_mut().enumerate() {
            *color = match i % 5 {
                0 => colors::RED,
                1 | 2 => colors::GREEN,
                3 => colors::BLUE,
                _ => colors::RED,
            };
        }

        let mut p = Paint::default();

        let interpolation = Interpolation {
            color_space: InterpColorSpace::OKLCH,
            ..Interpolation::default()
        };
        p.set_shader(gradient_shaders::linear_gradient(
            (pts[0], pts[1]),
            &Gradient::new(
                Colors::new(
                    &colors,
                    None,
                    TileMode::Clamp,
                    skia_rust_core::color_space::ColorSpace::new_srgb(),
                ),
                interpolation,
            ),
            None,
        ));

        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 500.0, 500.0), &p);
    }
);

// Port of: gm/gradients.cpp#L1165-L1196 (chrome/m156)
crate::def_simple_gm!(gradients_alpha_many_stops, canvas, 100, 100, {
    let k_pts = [Point::new(0.0, 0.0), Point::new(0.0, 100.0)];

    // From https://issues.chromium.org/issues/401546700, this encounters Graphite's
    // storage buffer option for storing gradient buffers AND uses colors that emphasize
    // premul vs. unpremul handling of the color data.
    let k_pos = [
        0.0, 0.19, 0.34, 0.47, 0.565, 0.65, 0.73, 0.802, 0.861, 0.91, 0.952, 0.982, 1.0,
    ];

    const K_G: f32 = 34.0 / 255.0;
    let k_colors = [
        Color4f::new(K_G, K_G, K_G, 1.0),
        Color4f::new(K_G, K_G, K_G, 0.738),
        Color4f::new(K_G, K_G, K_G, 0.541),
        Color4f::new(K_G, K_G, K_G, 0.382),
        Color4f::new(K_G, K_G, K_G, 0.278),
        Color4f::new(K_G, K_G, K_G, 0.194),
        Color4f::new(K_G, K_G, K_G, 0.126),
        Color4f::new(K_G, K_G, K_G, 0.075),
        Color4f::new(K_G, K_G, K_G, 0.042),
        Color4f::new(K_G, K_G, K_G, 0.021),
        Color4f::new(K_G, K_G, K_G, 0.008),
        Color4f::new(K_G, K_G, K_G, 0.002),
        Color4f::new(K_G, K_G, K_G, 0.0),
    ];

    canvas.clear(Color4f::new(0.5, 0.5, 0.5, 1.0));

    let mut paint = Paint::default();
    paint.set_shader(gradient_shaders::linear_gradient(
        (k_pts[0], k_pts[1]),
        &Gradient::new(
            Colors::new(&k_colors, Some(&k_pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_paint(&paint);
});

// Port of: gm/gradients.cpp#L997-L1039 (chrome/m156), gradients_color_space
crate::def_simple_gm_bg!(
    #[ignore = "see notes/gm-gradients.cpp-OKLCH-libm.md"]
    gradients_color_space,
    canvas,
    265,
    355,
    Color::GRAY,
    {
        use skia_rust_effects::gradient::interpolation::ColorSpace as CS;
        let configs: [(CS, &str); 14] = [
            (CS::SRGB, "sRGB"),
            (CS::SRGBLinear, "Linear"),
            (CS::Lab, "Lab"),
            (CS::OKLab, "OKLab"),
            (CS::OKLabGamutMap, "OKLabGamutMap"),
            (CS::LCH, "LCH"),
            (CS::OKLCH, "OKLCH"),
            (CS::OKLCHGamutMap, "OKLCHGamutMap"),
            (CS::HSL, "HSL"),
            (CS::HWB, "HWB"),
            (CS::A98RGB, "a98RGB"),
            (CS::ProphotoRGB, "ProPhotoRGB"),
            (CS::DisplayP3, "DisplayP3"),
            (CS::Rec2020, "Rec2020"),
        ];
        let pts = [Point::new(0.0, 0.0), Point::new(200.0, 0.0)];
        let colors = [Color4f::from(Color::BLUE), Color4f::from(Color::YELLOW)];
        let label_paint = Paint::default();
        let mut p = Paint::default();
        let font = default_portable_font();
        canvas.translate((5.0, 5.0));
        for (color_space, label) in configs {
            let interpolation = Interpolation {
                color_space,
                ..Interpolation::default()
            };
            let g = Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, ColorSpace::new_srgb()),
                interpolation,
            );
            p.set_shader(gradient_shaders::linear_gradient(
                (pts[0], pts[1]),
                &g,
                None,
            ));
            canvas.draw_rect(Rect::new(0.0, 0.0, 200.0, 20.0), &p);
            canvas.draw_simple_text(
                label.as_bytes(),
                TextEncoding::UTF8,
                (210.0, 15.0),
                &font,
                &label_paint,
            );
            canvas.translate((0.0, 25.0));
        }
    }
);

// Port of: gm/gradients.cpp#L1041-L1098 (chrome/m156), gradients_hue_method
crate::def_simple_gm_bg!(gradients_hue_method, canvas, 285, 155, Color::GRAY, {
    use skia_rust_effects::gradient::interpolation::HueMethod as HM;
    let configs: [(HM, &str); 4] = [
        (HM::Shorter, "Shorter"),
        (HM::Longer, "Longer"),
        (HM::Increasing, "Increasing"),
        (HM::Decreasing, "Decreasing"),
    ];
    let pts = [Point::new(0.0, 0.0), Point::new(200.0, 0.0)];
    let mut colors = [
        Color4f::from(Color::RED),
        Color4f::from(Color::GREEN),
        Color4f::from(Color::RED),
        Color4f::from(Color::RED),
    ];
    let label_paint = Paint::default();
    let mut p = Paint::default();
    let mut interpolation = Interpolation {
        color_space: skia_rust_effects::gradient::interpolation::ColorSpace::HSL,
        ..Interpolation::default()
    };
    canvas.translate((5.0, 5.0));
    let font = default_portable_font();
    for (hue_method, label) in configs {
        interpolation.hue_method = hue_method;
        let g = Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, ColorSpace::new_srgb()),
            interpolation,
        );
        p.set_shader(gradient_shaders::linear_gradient(
            (pts[0], pts[1]),
            &g,
            None,
        ));
        canvas.draw_rect(Rect::new(0.0, 0.0, 200.0, 20.0), &p);
        canvas.draw_simple_text(
            label.as_bytes(),
            TextEncoding::UTF8,
            (210.0, 15.0),
            &font,
            &label_paint,
        );
        canvas.translate((0.0, 25.0));
    }
    // Test a bug (skbug.com/40044215) with how gradient shaders handle explicit positions.
    // If there are no explicit positions at 0 or 1, those are automatically added, with copies of
    // the first/last color. When using kLonger, this can produce extra gradient that should
    // actually be solid. This gradient *should* be:
    //   |- solid red -|- red to green, the long way -|- solid green -|
    interpolation.hue_method = HM::Longer;
    let middle_pos = [0.3_f32, 0.7];
    let g = Gradient::new(
        Colors::new(
            &colors[..2],
            Some(&middle_pos),
            TileMode::Clamp,
            ColorSpace::new_srgb(),
        ),
        interpolation,
    );
    p.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &g,
        None,
    ));
    canvas.draw_rect(Rect::new(0.0, 0.0, 200.0, 20.0), &p);
    canvas.translate((0.0, 25.0));
    // However... if the user explicitly includes those duplicate color stops in kLonger mode,
    // we expect the gradient to do a full rotation in those regions:
    //  |- full circle, red to red -|- red to green -|- full circle, green to green -|
    colors[0] = Color4f::from(Color::RED);
    colors[1] = Color4f::from(Color::RED);
    colors[2] = Color4f::from(Color::GREEN);
    colors[3] = Color4f::from(Color::GREEN);
    let all_pos = [0.0_f32, 0.3, 0.7, 1.0];
    let g = Gradient::new(
        Colors::new(
            &colors,
            Some(&all_pos),
            TileMode::Clamp,
            ColorSpace::new_srgb(),
        ),
        interpolation,
    );
    p.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &g,
        None,
    ));
    canvas.draw_rect(Rect::new(0.0, 0.0, 200.0, 20.0), &p);
    canvas.translate((0.0, 25.0));
});

// Port of: gm/gradients.cpp#L1190-L1296 (chrome/m156), draw_powerless_hue_gradients
#[allow(clippy::too_many_lines)] // mirrors the long C++ draw_powerless_hue_gradients body
fn draw_powerless_hue_gradients(
    canvas: &Canvas,
    color_space: skia_rust_effects::gradient::interpolation::ColorSpace,
) {
    use skia_rust_effects::gradient::interpolation::{HueMethod, InPremul};
    let rgba = |r: f32, g: f32, b: f32, a: f32| Color4f::new(r, g, b, a);
    let white = Color4f::from(Color::WHITE);
    let black = Color4f::from(Color::BLACK);
    let blue = Color4f::from(Color::BLUE);
    let red = Color4f::from(Color::RED);
    let transparent = rgba(0.0, 0.0, 0.0, 0.0);

    // ToolUtils::draw_checkerboard(canvas): 0xFF999999, 0xFF666666, check size 8
    crate::tool_utils::draw_checkerboard(
        canvas,
        Color::from(0xFF99_9999),
        Color::from(0xFF66_6666),
        8,
    );

    let next_row = |canvas: &Canvas| {
        canvas.restore();
        canvas.translate((0.0, 25.0));
        canvas.save();
    };
    // gradient(colors, pos, inPremul): one column of the table
    let gradient = |canvas: &Canvas, colors: &[Color4f], pos: Option<&[f32]>, in_premul: bool| {
        let mut paint = Paint::default();
        let pts = [Point::new(0.0, 0.0), Point::new(200.0, 0.0)];
        let interpolation = Interpolation {
            color_space,
            in_premul: if in_premul {
                InPremul::Yes
            } else {
                InPremul::No
            },
            ..Interpolation::default()
        };
        paint.set_shader(gradient_shaders::linear_gradient(
            (pts[0], pts[1]),
            &Gradient::new(
                Colors::new(colors, pos, TileMode::Clamp, None),
                interpolation,
            ),
            None,
        ));
        canvas.draw_rect(Rect::new(0.0, 0.0, 200.0, 20.0), &paint);
        canvas.translate((205.0, 0.0)); // next column
    };

    canvas.translate((5.0, 5.0));
    canvas.save();
    // For each test case, the first gradient (first column) has an under-specified result due to a
    // powerless component after conversion to LCH. The second gradient (second column) "hints" the
    // correct result, by slightly tinting the otherwise powerless color.
    gradient(canvas, &[white, blue], None, false);
    gradient(canvas, &[rgba(0.99, 0.99, 1.00, 1.0), blue], None, false); // white, with blue hue
    next_row(canvas);
    gradient(canvas, &[black, blue], None, false);
    gradient(canvas, &[rgba(0.00, 0.00, 0.01, 1.0), blue], None, false); // black, with blue hue
    next_row(canvas);
    // Transparent cases are done in both premul and unpremul interpolation:
    gradient(canvas, &[transparent, blue], None, false);
    gradient(canvas, &[rgba(0.00, 0.00, 0.01, 0.0), blue], None, false);
    next_row(canvas);
    gradient(canvas, &[transparent, blue], None, true);
    gradient(canvas, &[rgba(0.00, 0.00, 0.01, 0.0), blue], None, true);
    next_row(canvas);
    gradient(canvas, &[rgba(1.00, 1.00, 1.00, 0.0), blue], None, false);
    gradient(canvas, &[rgba(0.99, 0.99, 1.00, 0.0), blue], None, false);
    next_row(canvas);
    gradient(canvas, &[rgba(1.00, 1.00, 1.00, 0.0), blue], None, true);
    gradient(canvas, &[rgba(0.99, 0.99, 1.00, 0.0), blue], None, true);
    next_row(canvas);
    // Now we test three-stop gradients, where the middle stop needs to be "split" to handle the
    // different hues on either side. Again, the second column explicitly injects those to produce
    // a reference result. See: https://github.com/w3c/csswg-drafts/issues/9295
    gradient(canvas, &[red, white, blue], None, false);
    gradient(
        canvas,
        &[
            red,
            rgba(1.00, 0.99, 0.99, 1.0),
            rgba(0.99, 0.99, 1.00, 1.0),
            blue,
        ],
        Some(&[0.0, 0.5, 0.5, 1.0]),
        false,
    );
    next_row(canvas);
    gradient(canvas, &[red, black, blue], None, false);
    gradient(
        canvas,
        &[
            red,
            rgba(0.01, 0.00, 0.00, 1.0),
            rgba(0.00, 0.00, 0.01, 1.0),
            blue,
        ],
        Some(&[0.0, 0.5, 0.5, 1.0]),
        false,
    );
    next_row(canvas);
    gradient(canvas, &[red, transparent, blue], None, false);
    gradient(
        canvas,
        &[
            red,
            rgba(0.01, 0.00, 0.00, 0.0),
            rgba(0.00, 0.00, 0.01, 0.0),
            blue,
        ],
        Some(&[0.0, 0.5, 0.5, 1.0]),
        false,
    );
    next_row(canvas);
    // Now do a few black-white tests, to ensure that the hue propagation works correctly, even
    // when there isn't any hue in the adjacent stops.
    let black_white_gradient = |canvas: &Canvas, hue_method: HueMethod| {
        let mut paint = Paint::default();
        let pts = [Point::new(0.0, 0.0), Point::new(405.0, 0.0)];
        let interpolation = Interpolation {
            color_space,
            hue_method,
            ..Interpolation::default()
        };
        let colors = [
            white,
            Color4f::new(0.5, 0.5, 0.5, 1.0), // kGray
            white,
            Color4f::new(0.25, 0.25, 0.25, 1.0), // kDkGray
            white,
            black,
        ];
        paint.set_shader(gradient_shaders::linear_gradient(
            (pts[0], pts[1]),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                interpolation,
            ),
            None,
        ));
        canvas.draw_rect(Rect::new(0.0, 0.0, 405.0, 20.0), &paint);
        next_row(canvas);
    };
    black_white_gradient(canvas, HueMethod::Shorter);
    black_white_gradient(canvas, HueMethod::Increasing);
    black_white_gradient(canvas, HueMethod::Decreasing);
    black_white_gradient(canvas, HueMethod::Longer);
}

// Port of: gm/gradients.cpp#L1295-L1304 (chrome/m156), DEF_POWERLESS_HUE_GM(LCH), (OKLCH),
// (HSL), (HWB)
crate::def_simple_gm_bg_name!(
    #[ignore = "see notes/gm-gradients.cpp-OKLCH-libm.md"]
    LCH,
    canvas,
    415,
    330,
    Color::WHITE,
    "gradients_powerless_hue_LCH",
    {
        draw_powerless_hue_gradients(
            canvas,
            skia_rust_effects::gradient::interpolation::ColorSpace::LCH,
        );
    }
);
crate::def_simple_gm_bg_name!(
    #[ignore = "see notes/gm-gradients.cpp-OKLCH-libm.md"]
    OKLCH,
    canvas,
    415,
    330,
    Color::WHITE,
    "gradients_powerless_hue_OKLCH",
    {
        draw_powerless_hue_gradients(
            canvas,
            skia_rust_effects::gradient::interpolation::ColorSpace::OKLCH,
        );
    }
);
crate::def_simple_gm_bg_name!(
    HSL,
    canvas,
    415,
    330,
    Color::WHITE,
    "gradients_powerless_hue_HSL",
    {
        draw_powerless_hue_gradients(
            canvas,
            skia_rust_effects::gradient::interpolation::ColorSpace::HSL,
        );
    }
);
crate::def_simple_gm_bg_name!(
    HWB,
    canvas,
    415,
    330,
    Color::WHITE,
    "gradients_powerless_hue_HWB",
    {
        draw_powerless_hue_gradients(
            canvas,
            skia_rust_effects::gradient::interpolation::ColorSpace::HWB,
        );
    }
);

// Port of: gm/gradients.cpp#L796-L808 (chrome/m156), draw_circle_shader
fn draw_circle_shader(
    canvas: &Canvas,
    (cx, cy, r): (scalar, scalar, scalar),
    shader_func: impl FnOnce() -> Option<Shader>,
) {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_shader(shader_func());
    canvas.draw_circle((cx, cy), r, &p);

    p.set_shader(None);
    p.set_color(Color::new(0xFF88_8888)); // SK_ColorGRAY
    p.set_style(Style::Stroke);
    p.set_stroke_width(2.0);
    canvas.draw_circle((cx, cy), r, &p);
}

// Port of: gm/gradients.cpp#L810-L893 (chrome/m156), the body of fancy_gradients
#[allow(clippy::too_many_lines)] // mirrors the long C++ DEF_SIMPLE_GM body
fn fancy_gradients_draw(canvas: &Canvas) {
    draw_circle_shader(canvas, (150.0, 150.0, 100.0), || {
        // Checkerboard using two linear gradients + picture shader.
        let k_tile_size: scalar = 80.0 / 2.0_f32.sqrt();
        let colors1 = [
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
        ];
        let colors2 = [
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
        ];
        let pos: [scalar; 6] = [0.0, 0.25, 0.25, 0.75, 0.75, 1.0];

        let mut recorder = PictureRecorder::new();
        let rc = recorder.begin_recording(Rect::from_wh(k_tile_size, k_tile_size), false);

        let mut p = Paint::default();

        let pts1 = (Point::new(0.0, 0.0), Point::new(k_tile_size, k_tile_size));
        p.set_shader(gradient_shaders::linear_gradient(
            pts1,
            &Gradient::new(
                Colors::new(&colors1, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        rc.draw_paint(&p);

        let pts2 = (Point::new(0.0, k_tile_size), Point::new(k_tile_size, 0.0));
        p.set_shader(gradient_shaders::linear_gradient(
            pts2,
            &Gradient::new(
                Colors::new(&colors2, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        rc.draw_paint(&p);

        let mut m = Matrix::new_identity();
        m.pre_rotate(45.0, None);
        recorder
            .finish_recording_as_picture(None)
            .and_then(|picture| {
                picture.to_shader(
                    (TileMode::Repeat, TileMode::Repeat),
                    FilterMode::Nearest,
                    &m,
                    None,
                )
            })
    });

    draw_circle_shader(canvas, (400.0, 150.0, 100.0), || {
        // Checkerboard using a sweep gradient + picture shader.
        let k_tile_size: scalar = 80.0;
        let colors = [
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
        ];
        let pos: [scalar; 8] = [0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0];

        let mut p = Paint::default();
        p.set_shader(gradient_shaders::sweep_gradient(
            Point::new(k_tile_size / 2.0, k_tile_size / 2.0),
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        let mut recorder = PictureRecorder::new();
        recorder
            .begin_recording(Rect::from_wh(k_tile_size, k_tile_size), false)
            .draw_paint(&p);

        recorder
            .finish_recording_as_picture(None)
            .and_then(|picture| {
                picture.to_shader(
                    (TileMode::Repeat, TileMode::Repeat),
                    FilterMode::Nearest,
                    None,
                    None,
                )
            })
    });

    draw_circle_shader(canvas, (650.0, 150.0, 100.0), || {
        // Dartboard using sweep + radial.
        let a = Color4f::new(1.0, 1.0, 1.0, 1.0);
        let b = Color4f::new(0.0, 0.0, 0.0, 1.0);
        let colors = [a, a, b, b, a, a, b, b, a, a, b, b, a, a, b, b];
        let pos: [scalar; 16] = [
            0.0, 0.125, 0.125, 0.25, 0.25, 0.375, 0.375, 0.5, 0.5, 0.625, 0.625, 0.75, 0.75, 0.875,
            0.875, 1.0,
        ];

        let center = Point::new(650.0, 150.0);
        let sweep1 = gradient_shaders::sweep_gradient(
            center,
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        )?;
        let mut m = Matrix::new_identity();
        m.pre_rotate(22.5, center);
        let sweep2 = gradient_shaders::sweep_gradient(
            center,
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            &m,
        )?;

        let sweep = skia_rust_core::shaders::blend(BlendMode::Exclusion, sweep1, sweep2);

        let radial_pos: [scalar; 16] = [
            0.0, 0.02, 0.02, 0.04, 0.04, 0.08, 0.08, 0.16, 0.16, 0.31, 0.31, 0.62, 0.62, 1.0, 1.0,
            1.0,
        ];
        let radial = gradient_shaders::radial_gradient(
            (center, 100.0),
            &Gradient::new(
                Colors::new(&colors, Some(&radial_pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        )?;
        Some(skia_rust_core::shaders::blend(
            BlendMode::Exclusion,
            sweep,
            radial,
        ))
    });
}

// Port of: gm/gradients.cpp#L810 (chrome/m156), DEF_SIMPLE_GM(fancy_gradients, canvas, 800, 300)
crate::def_simple_gm!(fancy_gradients, canvas, 800, 300, {
    fancy_gradients_draw(canvas);
});
