// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradients_no_texture.cpp (chrome/m156)

// local items stay where the C++ declares them
#![allow(clippy::items_after_statements)]

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_interp;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/gradients_no_texture.cpp#L27-L40 (chrome/m156)
struct GradData {
    count: usize,
    colors: &'static [Color4f],
}

impl GradData {
    /// `operator()(tm)`: no positions.
    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(&self.colors[..self.count], None, tm, None),
            Interpolation::default(),
        )
    }
}

const G_COLORS: [Color4f; 4] = [colors::RED, colors::GREEN, colors::BLUE, colors::WHITE];

// Port of: gm/gradients_no_texture.cpp#L46-L51 (chrome/m156)
static G_GRAD_DATA: [GradData; 4] = [
    GradData {
        count: 1,
        colors: &G_COLORS,
    },
    GradData {
        count: 2,
        colors: &G_COLORS,
    },
    GradData {
        count: 3,
        colors: &G_COLORS,
    },
    GradData {
        count: 4,
        colors: &G_COLORS,
    },
];

// Port of: gm/gradients_no_texture.cpp#L53-L55 (chrome/m156)
fn make_linear(pts: &[Point; 2], grad: &GradData, tm: TileMode) -> Option<Shader> {
    shaders::linear_gradient((pts[0], pts[1]), &grad.grad(tm), None)
}

// Port of: gm/gradients_no_texture.cpp#L57-L63 (chrome/m156)
fn make_radial(pts: &[Point; 2], grad: &GradData, tm: TileMode) -> Option<Shader> {
    let center = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    shaders::radial_gradient((center, center.x), &grad.grad(tm), None)
}

// Port of: gm/gradients_no_texture.cpp#L65-L71 (chrome/m156)
fn make_sweep(pts: &[Point; 2], grad: &GradData, tm: TileMode) -> Option<Shader> {
    let center = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    shaders::sweep_gradient(center, (0.0, 360.0), &grad.grad(tm), None)
}

// Port of: gm/gradients_no_texture.cpp#L73-L84 (chrome/m156)
fn make_2_radial(pts: &[Point; 2], grad: &GradData, tm: TileMode) -> Option<Shader> {
    let center0 = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    let center1 = Point::new(
        scalar_interp(pts[0].x, pts[1].x, 3.0 / 5.0),
        scalar_interp(pts[0].y, pts[1].y, 1.0 / 4.0),
    );
    shaders::two_point_conical_gradient(
        (center1, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        &grad.grad(tm),
        None,
    )
}

// Port of: gm/gradients_no_texture.cpp#L86-L94 (chrome/m156)
fn make_2_conical(pts: &[Point; 2], grad: &GradData, tm: TileMode) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 10.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    shaders::two_point_conical_gradient(
        (center1, radius1),
        (center0, radius0),
        &grad.grad(tm),
        None,
    )
}

type GradMaker = fn(&[Point; 2], &GradData, TileMode) -> Option<Shader>;

// Port of: gm/gradients_no_texture.cpp#L98-L104 (chrome/m156)
const G_GRAD_MAKERS: [GradMaker; 5] = [
    make_linear,
    make_radial,
    make_sweep,
    make_2_radial,
    make_2_conical,
];

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients_no_texture.cpp#L108-L158 (chrome/m156)
struct GradientsNoTextureGm {
    dither: bool,
}

impl GM for GradientsNoTextureGm {
    fn name(&self) -> String {
        if self.dither {
            "gradients_no_texture"
        } else {
            "gradients_no_texture_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 615)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDD_DDDD)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const K_PTS: [Point; 2] = [Point::new(0.0, 0.0), Point::new(50.0, 50.0)];
        const K_TM: TileMode = TileMode::Clamp;
        let k_rect = Rect::new(0.0, 0.0, 50.0, 50.0);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);

        canvas.translate((20.0, 20.0));
        let k_alphas: [u8; 2] = [0xff, 0x40];
        for alpha in k_alphas {
            for data in &G_GRAD_DATA {
                canvas.save();
                for maker in &G_GRAD_MAKERS {
                    paint.set_shader(maker(&K_PTS, data, K_TM));
                    paint.set_alpha(alpha);
                    canvas.draw_rect(k_rect, &paint);
                    canvas.translate((0.0, k_rect.height() + 20.0));
                }
                canvas.restore();
                canvas.translate((k_rect.width() + 20.0, 0.0));
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients_no_texture.cpp#L162-L192 (chrome/m156)
#[derive(Default)]
struct ColorPos {
    colors: Vec<Color4f>,
    pos: Option<Vec<f32>>,
}

impl ColorPos {
    fn construct(colors: &[Color], pos: Option<&[f32]>) -> ColorPos {
        let count = colors.len();
        let f_colors: Vec<Color4f> = colors.iter().map(|c| Color4f::from_color(*c)).collect();
        let f_pos = pos.map(|pos| {
            let mut p = pos[..count].to_vec();
            p[0] = 0.0;
            p[count - 1] = 1.0;
            p
        });
        ColorPos {
            colors: f_colors,
            pos: f_pos,
        }
    }

    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(&self.colors, self.pos.as_deref(), tm, None),
            Interpolation::default(),
        )
    }
}

// Port of: gm/gradients_no_texture.cpp#L194-L225 (chrome/m156)
fn make0() -> ColorPos {
    // From http://jsfiddle.net/3fe2a/
    //
    // background-image: -webkit-linear-gradient(left, #22d1cd 1%, #22d1cd 0.9510157507590116%,
    // #df4b37 2.9510157507590113%, #df4b37 23.695886056604927%, #22d1cd 25.695886056604927%,
    // #22d1cd 25.39321881940624%, #e6de36 27.39321881940624%, #e6de36 31.849399922570655%,
    // #3267ff 33.849399922570655%, #3267ff 44.57735802921938%, #9d47d1 46.57735802921938%,
    // #9d47d1 53.27185850805876%, #3267ff 55.27185850805876%, #3267ff 61.95718972227316%,
    // #5cdd9d 63.95718972227316%, #5cdd9d 69.89166004442%, #3267ff 71.89166004442%,
    // #3267ff 74.45795382765857%, #9d47d1 76.45795382765857%, #9d47d1 82.78364610713776%,
    // #3267ff 84.78364610713776%, #3267ff 94.52743647737229%, #e3d082 96.52743647737229%,
    // #e3d082 96.03934633331295%);
    // height: 30px;

    let colors = [
        0xFF22_D1CD_u32,
        0xFF22_D1CD,
        0xFFDF_4B37,
        0xFFDF_4B37,
        0xFF22_D1CD,
        0xFF22_D1CD,
        0xFFE6_DE36,
        0xFFE6_DE36,
        0xFF32_67FF,
        0xFF32_67FF,
        0xFF9D_47D1,
        0xFF9D_47D1,
        0xFF32_67FF,
        0xFF32_67FF,
        0xFF5C_DD9D,
        0xFF5C_DD9D,
        0xFF32_67FF,
        0xFF32_67FF,
        0xFF9D_47D1,
        0xFF9D_47D1,
        0xFF32_67FF,
        0xFF32_67FF,
        0xFFE3_D082,
        0xFFE3_D082,
    ]
    .map(Color::new);
    #[allow(clippy::excessive_precision)] // Skia's literals kept verbatim
    let percent: [f64; 24] = [
        1.0,
        0.951_015_750_759_011_6,
        2.951_015_750_759_011_3,
        23.695_886_056_604_927,
        25.695_886_056_604_927,
        25.393_218_819_406_24,
        27.393_218_819_406_24,
        31.849_399_922_570_655,
        33.849_399_922_570_655,
        44.577_358_029_219_38,
        46.577_358_029_219_38,
        53.271_858_508_058_76,
        55.271_858_508_058_76,
        61.957_189_722_273_16,
        63.957_189_722_273_16,
        69.891_660_044_42,
        71.891_660_044_42,
        74.457_953_827_658_57,
        76.457_953_827_658_57,
        82.783_646_107_137_76,
        84.783_646_107_137_76,
        94.527_436_477_372_29,
        96.527_436_477_372_29,
        96.039_346_333_312_95,
    ];
    #[allow(clippy::cast_possible_truncation)] // SkDoubleToScalar
    let pos: Vec<f32> = percent.iter().map(|p| (p / 100.0) as f32).collect();
    ColorPos::construct(&colors, Some(&pos))
}

// Port of: gm/gradients_no_texture.cpp#L227-L234 (chrome/m156)
fn make1() -> ColorPos {
    let colors = [
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
    ];
    ColorPos::construct(&colors, None)
}

// Port of: gm/gradients_no_texture.cpp#L236-L248 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SK_Scalar1 * i / (N - 1)
fn make2() -> ColorPos {
    let colors = [
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
        Color::WHITE,
        Color::BLACK,
    ];
    const N: usize = 9;
    let mut pos = [0.0f32; N];
    for (i, p) in pos.iter_mut().enumerate() {
        *p = 1.0 * i as f32 / (N - 1) as f32;
    }
    ColorPos::construct(&colors, Some(&pos))
}

// Port of: gm/gradients_no_texture.cpp#L250-L257 (chrome/m156)
fn make3() -> ColorPos {
    let colors = [
        Color::RED,
        Color::BLUE,
        Color::BLUE,
        Color::GREEN,
        Color::GREEN,
        Color::BLACK,
    ];
    let pos = [0.0, 0.0, 0.5, 0.5, 1.0, 1.0];
    ColorPos::construct(&colors, Some(&pos))
}

// Port of: gm/gradients_no_texture.cpp#L259-L309 (chrome/m156)
struct GradientsManyColorsGm {
    dither: bool,
}

impl GradientsManyColorsGm {
    const W: i32 = 800;
}

impl GM for GradientsManyColorsGm {
    fn name(&self) -> String {
        if self.dither {
            "gradients_many"
        } else {
            "gradients_many_nodither"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(880, 400)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
    fn on_draw(&mut self, canvas: &Canvas) {
        let procs: [fn() -> ColorPos; 4] = [make0, make1, make2, make3];
        let pts = [Point::new(0.0, 0.0), Point::new(Self::W as f32, 0.0)];
        let r = Rect::from_wh(Self::W as f32, 30.0);

        let mut paint = Paint::default();
        paint.set_dither(self.dither);

        canvas.translate((40.0, 20.0));

        for i in 0..=8 {
            let x = r.width() * i as f32 / 8.0;
            canvas.draw_line((x, 0.0), (x, 10000.0), &paint);
        }

        // expand the drawing rect so we exercise clampping in the gradients
        let draw_r = r.with_outset((20.0, 0.0));
        for proc in procs {
            let rec = proc();
            paint.set_shader(shaders::linear_gradient(
                (pts[0], pts[1]),
                &rec.grad(TileMode::Clamp),
                None,
            ));
            canvas.draw_rect(draw_r, &paint);

            canvas.save();
            canvas.translate((r.center_x(), r.height() + 4.0));
            canvas.scale((-1.0, 1.0));
            canvas.translate((-r.center_x(), 0.0));
            canvas.draw_rect(draw_r, &paint);
            canvas.restore();

            canvas.translate((0.0, r.height() + 2.0 * r.height() + 8.0));
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients_no_texture.cpp#L315-L318 (chrome/m156)
crate::def_gm!(
    GradientsNoTextureGM_true = "GradientsNoTextureGM(true)",
    GradientsNoTextureGm { dither: true }
);
crate::def_gm!(
    GradientsNoTextureGM_false = "GradientsNoTextureGM(false)",
    GradientsNoTextureGm { dither: false }
);
crate::def_gm!(
    GradientsManyColorsGM_true = "GradientsManyColorsGM(true)",
    GradientsManyColorsGm { dither: true }
);
crate::def_gm!(
    GradientsManyColorsGM_false = "GradientsManyColorsGM(false)",
    GradientsManyColorsGm { dither: false }
);
