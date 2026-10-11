// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tilemodes_scaled.cpp (chrome/m156)
//
// Only `ScaledTilingGM(true)`, `ScaledTilingGM(false)` and `ScaledTiling2GM(make_bm, ...)` are
// ported here (the gradient variant is not in the manifest's raster list).

// The int-to-scalar casts of small sizes mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]
// Single-letter and similar names mirror the C++ GM (w, h).
#![allow(clippy::many_single_char_names)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color::colors;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

use crate::tool_utils::{color_to_565, int_to_scalar};

// Port of: gm/tilemodes_scaled.cpp#L8-L17 (chrome/m156), gSamplings
fn samplings() -> [SamplingOptions; 5] {
    [
        SamplingOptions::from(FilterMode::Nearest),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        SamplingOptions::from(CubicResampler::mitchell()),
        SamplingOptions::from_aniso(16),
    ]
}

// Port of: gm/tilemodes_scaled.cpp#L19-L33 (chrome/m156), makebm
fn makebm(bm: &mut Bitmap, ct: ColorType, w: i32, h: i32) {
    bm.alloc_pixels_info(&ImageInfo::new((w, h), ct, AlphaType::Premul, None), None);
    bm.erase_color(Color::TRANSPARENT);

    let canvas = CoreCanvas::from_bitmap(bm, None).expect("a canvas on the bitmap");
    let pts = [Point::new(0.0, 0.0), Point::new(w as scalar, h as scalar)];
    let grad_colors = [colors::RED, colors::GREEN, colors::BLUE];
    let pos = [0.0, 1.0 / 2.0, 1.0];
    let mut paint = Paint::default();

    paint.set_dither(true);
    let grad = Gradient::new(
        Colors::new(&grad_colors, Some(&pos), TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(shaders::linear_gradient((pts[0], pts[1]), &grad, None));
    canvas.draw_paint(&paint);
}

// Port of: gm/tilemodes_scaled.cpp#L35-L39 (chrome/m156), setup
fn setup(paint: &mut Paint, bm: &Bitmap, sampling: SamplingOptions, tmx: TileMode, tmy: TileMode) {
    paint.set_shader(bm.to_shader((tmx, tmy), sampling, None));
}

// Port of: gm/tilemodes_scaled.cpp#L41-L44 (chrome/m156), gColorTypes
const COLOR_TYPES: [ColorType; 2] = [ColorType::N32, ColorType::RGB565];

// Port of: gm/tilemodes_scaled.cpp#L65-L204 (chrome/m156), class ScaledTilingGM
#[derive(Debug)]
pub struct ScaledTilingGm {
    power_of_two_size: bool,
    texture: [Bitmap; 2],
}

impl ScaledTilingGm {
    // Port of: gm/tilemodes_scaled.cpp#L65-L69 (chrome/m156), ScaledTilingGM(bool)
    #[must_use]
    pub fn new(power_of_two_size: bool) -> Self {
        Self {
            power_of_two_size,
            texture: [Bitmap::new(), Bitmap::new()],
        }
    }

    // kPOTSize = 4, kNPOTSize = 3
    fn tile_size(&self) -> i32 {
        if self.power_of_two_size { 4 } else { 3 }
    }
}

impl GM for ScaledTilingGm {
    // Port of: gm/tilemodes_scaled.cpp (chrome/m156), getName
    fn name(&self) -> String {
        if self.power_of_two_size {
            "scaled_tilemodes".to_owned()
        } else {
            "scaled_tilemodes_npot".to_owned()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(880, 880)
    }

    // Port of: gm/tilemodes_scaled.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let size = self.tile_size();
        for (i, &ct) in COLOR_TYPES.iter().enumerate() {
            makebm(&mut self.texture[i], ct, size, size);
        }
    }

    // Port of: gm/tilemodes_scaled.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let text_paint = Paint::default();
        let font = skia_rust_core::font::Font::from_size(
            skia_rust_tools::font_tool_utils::default_portable_typeface(),
            12.0,
        );

        let scale: scalar = 32.0 / 4.0;

        let size = self.tile_size();

        let r = Rect::from_ltrb(0.0, 0.0, (size * 2) as scalar, (size * 2) as scalar);

        let color_type_names = ["8888", "565"];

        let filter_names = ["Nearest", "Linear", "Trilinear", "Mitchell", "Aniso"];

        let modes = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];
        let mode_names = ["C", "R", "M"];

        let mut y: scalar = 24.0;
        let mut x: scalar = 10.0 / scale;

        for kx in 0..modes.len() {
            for ky in 0..modes.len() {
                let s = format!("[{},{}]", mode_names[kx], mode_names[ky]);

                text_utils::draw_string(
                    canvas,
                    &s,
                    scale * (x + r.width() / 2.0),
                    y,
                    &font,
                    &Paint::default(),
                    Align::Center,
                );

                x += r.width() * 4.0 / 3.0;
            }
        }

        y = 40.0 / scale;

        let samplings = samplings();
        for i in 0..COLOR_TYPES.len() {
            for j in 0..samplings.len() {
                x = 10.0 / scale;
                for kx in 0..modes.len() {
                    for ky in 0..modes.len() {
                        let mut paint = Paint::default();
                        if !self.power_of_two_size {
                            makebm(&mut self.texture[i], COLOR_TYPES[i], size, size);
                        }
                        setup(
                            &mut paint,
                            &self.texture[i],
                            samplings[j],
                            modes[kx],
                            modes[ky],
                        );
                        paint.set_dither(true);

                        canvas.save();
                        canvas.scale((scale, scale));
                        canvas.translate((x, y));
                        canvas.draw_rect(r, &paint);
                        canvas.restore();

                        x += r.width() * 4.0 / 3.0;
                    }
                }
                canvas.draw_str(
                    format!("{}, {}", color_type_names[i], filter_names[j]),
                    (scale * x, scale * (y + r.height() * 2.0 / 3.0)),
                    &font,
                    &text_paint,
                );

                y += r.height() * 4.0 / 3.0;
            }
        }
    }
}

// Port of: gm/tilemodes_scaled.cpp#L177-L180 (chrome/m156), gWidth / gHeight
const G_WIDTH: i32 = 32;
const G_HEIGHT: i32 = 32;

// Port of: gm/tilemodes_scaled.cpp#L181-L186 (chrome/m156), make_bm
fn make_bm(tx: TileMode, ty: TileMode) -> Option<Shader> {
    let mut bm = Bitmap::new();
    makebm(&mut bm, ColorType::N32, G_WIDTH, G_HEIGHT);
    bm.to_shader((tx, ty), SamplingOptions::default(), None)
}

// Port of: gm/tilemodes_scaled.cpp#L184-L202 (chrome/m156), make_grad
fn make_grad(tx: TileMode, ty: TileMode) -> Option<Shader> {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(int_to_scalar(G_WIDTH), int_to_scalar(G_HEIGHT)),
    ];
    let center = Point::new(int_to_scalar(G_WIDTH) / 2.0, int_to_scalar(G_HEIGHT) / 2.0);
    let rad = int_to_scalar(G_WIDTH) / 2.0;
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::from_color(color_to_565(Color::from(0xFF00_44FF))),
    ];
    let grad = Gradient::new(
        Colors::new(&colors, None, tx, None),
        Interpolation::default(),
    );

    // `int index = (int)ty;` for the modes this GM uses (kClamp, kRepeat, kMirror).
    let index = match ty {
        TileMode::Clamp => 0,
        TileMode::Repeat => 1,
        TileMode::Mirror => 2,
        TileMode::Decal => 3,
    };
    match index % 3 {
        0 => shaders::linear_gradient((pts[0], pts[1]), &grad, None),
        1 => shaders::radial_gradient((center, rad), &grad, None),
        2 => shaders::sweep_gradient(center, (135.0, 225.0), &grad, None),
        _ => None,
    }
}

/// `ShaderProc`: builds the shader for a pair of tile modes.
// Port of: gm/tilemodes_scaled.cpp#L204 (chrome/m156), typedef ShaderProc
type ShaderProc = fn(TileMode, TileMode) -> Option<Shader>;

// Port of: gm/tilemodes_scaled.cpp#L206-L273 (chrome/m156), class ScaledTiling2GM
struct ScaledTiling2Gm {
    proc: ShaderProc,
    name: &'static str,
}

impl ScaledTiling2Gm {
    // Port of: gm/tilemodes_scaled.cpp#L208 (chrome/m156), ScaledTiling2GM(ShaderProc, const char*)
    #[must_use]
    fn new(proc: ShaderProc, name: &'static str) -> Self {
        Self { proc, name }
    }
}

impl GM for ScaledTiling2Gm {
    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(650, 610)
    }

    // Port of: gm/tilemodes_scaled.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((3.0 / 2.0, 3.0 / 2.0));

        let w = G_WIDTH as scalar;
        let h = G_HEIGHT as scalar;
        let r = Rect::from_ltrb(-w, -h, w * 2.0, h * 2.0);

        let modes = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];
        let mode_names = ["Clamp", "Repeat", "Mirror"];

        let mut y: scalar = 24.0;
        let mut x: scalar = 66.0;

        let font = default_portable_font();

        for name in mode_names {
            text_utils::draw_string(
                canvas,
                name,
                x + r.width() / 2.0,
                y,
                &font,
                &Paint::default(),
                Align::Center,
            );
            x += r.width() * 4.0 / 3.0;
        }

        y += 16.0 + h;

        for (ky, name) in mode_names.iter().enumerate() {
            x = 16.0 + w;

            text_utils::draw_string(
                canvas,
                name,
                x,
                y + h / 2.0,
                &font,
                &Paint::default(),
                Align::Right,
            );

            x += 50.0;
            for kx in 0..modes.len() {
                let mut paint = Paint::default();
                paint.set_shader((self.proc)(modes[kx], modes[ky]));

                canvas.save();
                canvas.translate((x, y));
                canvas.draw_rect(r, &paint);
                canvas.restore();

                x += r.width() * 4.0 / 3.0;
            }
            y += r.height() * 4.0 / 3.0;
        }
    }
}

// Port of: gm/tilemodes_scaled.cpp#L274-L276 (chrome/m156), DEF_GM( return new ScaledTilingGM(true); )
crate::def_gm!(
    ScaledTilingGM_true = "ScaledTilingGM(true)",
    ScaledTilingGm::new(true)
);
// Port of: gm/tilemodes_scaled.cpp#L275-L275 (chrome/m156), DEF_GM( return new ScaledTilingGM(false); )
crate::def_gm!(
    ScaledTilingGM_false = "ScaledTilingGM(false)",
    ScaledTilingGm::new(false)
);
// Port of: gm/tilemodes_scaled.cpp#L276-L276 (chrome/m156), DEF_GM( return new ScaledTiling2GM(make_bm, ...); )
crate::def_gm!(
    ScaledTiling2GM_bitmap = "ScaledTiling2GM(make_bm, \"scaled_tilemode_bitmap\")",
    ScaledTiling2Gm::new(make_bm, "scaled_tilemode_bitmap")
);
// Port of: gm/tilemodes_scaled.cpp#L277-L277 (chrome/m156), DEF_GM( return new ScaledTiling2GM(make_grad, ...); )
crate::def_gm!(
    ScaledTiling2GM_gradient = "ScaledTiling2GM(make_grad, \"scaled_tilemode_gradient\")",
    ScaledTiling2Gm::new(make_grad, "scaled_tilemode_gradient")
);
