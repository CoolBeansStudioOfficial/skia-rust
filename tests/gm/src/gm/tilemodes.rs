// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tilemodes.cpp (chrome/m156)
//
// Ported here: `TilingGM(true)`, `TilingGM(false)`, `Tiling2GM(make_bm, "tilemode_bitmap")` and
// `tilemode_decal`.

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
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

use crate::tool_utils::{color_to_565, int_to_scalar};

// Port of: gm/tilemodes.cpp#L8-L26 (chrome/m156), makebm
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

// Port of: gm/tilemodes.cpp#L28-L35 (chrome/m156), setup
fn setup(paint: &mut Paint, bm: &Bitmap, fm: FilterMode, tmx: TileMode, tmy: TileMode) {
    if let Some(img) = images::raster_from_bitmap(bm) {
        // img can be null if the GPU context has been abandoned.
        paint.set_shader(img.to_shader((tmx, tmy), SamplingOptions::from(fm), None));
    }
}

// Port of: gm/tilemodes.cpp#L37-L38 (chrome/m156), gColorTypes
const COLOR_TYPES: [ColorType; 2] = [ColorType::N32, ColorType::RGB565];

const POT_SIZE: i32 = 32;
const NPOT_SIZE: i32 = 21;

// Port of: gm/tilemodes.cpp#L44-L101 (chrome/m156), class TilingGM
#[derive(Debug)]
pub struct TilingGm {
    power_of_two_size: bool,
    texture: [Bitmap; 2],
}

impl TilingGm {
    // Port of: gm/tilemodes.cpp#L44-L101 (chrome/m156), TilingGM(bool)
    #[must_use]
    pub fn new(power_of_two_size: bool) -> Self {
        Self {
            power_of_two_size,
            texture: [Bitmap::new(), Bitmap::new()],
        }
    }

    fn tile_size(&self) -> i32 {
        if self.power_of_two_size {
            POT_SIZE
        } else {
            NPOT_SIZE
        }
    }
}

impl GM for TilingGm {
    // Port of: gm/tilemodes.cpp (chrome/m156), getName
    fn name(&self) -> String {
        if self.power_of_two_size {
            "tilemodes".to_owned()
        } else {
            "tilemodes_npot".to_owned()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(880, 560)
    }

    // Port of: gm/tilemodes.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let size = self.tile_size();
        for (i, &ct) in COLOR_TYPES.iter().enumerate() {
            makebm(&mut self.texture[i], ct, size, size);
        }
    }

    // Port of: gm/tilemodes.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let text_paint = Paint::default();
        let font = default_portable_font();

        let size = self.tile_size();

        let r = Rect::from_ltrb(0.0, 0.0, (size * 2) as scalar, (size * 2) as scalar);

        let config_names = ["8888", "565"];

        let filters = [FilterMode::Nearest, FilterMode::Linear];
        let filter_names = ["point", "bilinear"];

        let modes = [TileMode::Clamp, TileMode::Repeat, TileMode::Mirror];
        let mode_names = ["C", "R", "M"];

        let mut y: scalar = 24.0;
        let mut x: scalar = 10.0;

        for kx in 0..modes.len() {
            for ky in 0..modes.len() {
                let mut p = Paint::default();
                p.set_dither(true);
                let s = format!("[{},{}]", mode_names[kx], mode_names[ky]);

                text_utils::draw_string(
                    canvas,
                    &s,
                    x + r.width() / 2.0,
                    y,
                    &font,
                    &p,
                    Align::Center,
                );

                x += r.width() * 4.0 / 3.0;
            }
        }

        y += 16.0;

        for i in 0..COLOR_TYPES.len() {
            for j in 0..filters.len() {
                x = 10.0;
                for kx in 0..modes.len() {
                    for ky in 0..modes.len() {
                        let mut paint = Paint::default();
                        if !self.power_of_two_size {
                            makebm(&mut self.texture[i], COLOR_TYPES[i], size, size);
                        }
                        setup(
                            &mut paint,
                            &self.texture[i],
                            filters[j],
                            modes[kx],
                            modes[ky],
                        );
                        paint.set_dither(true);

                        canvas.save();
                        canvas.translate((x, y));
                        canvas.draw_rect(r, &paint);
                        canvas.restore();

                        x += r.width() * 4.0 / 3.0;
                    }
                }
                canvas.draw_str(
                    format!("{}, {}", config_names[i], filter_names[j]),
                    (x, y + r.height() * 2.0 / 3.0),
                    &font,
                    &text_paint,
                );

                y += r.height() * 4.0 / 3.0;
            }
        }
    }
}

// Port of: gm/tilemodes.cpp#L177-L180 (chrome/m156), gWidth / gHeight
const G_WIDTH: i32 = 32;
const G_HEIGHT: i32 = 32;

// Port of: gm/tilemodes.cpp#L182-L187 (chrome/m156), make_bm
fn make_bm(tx: TileMode, ty: TileMode) -> Option<Shader> {
    let mut bm = Bitmap::new();
    makebm(&mut bm, ColorType::N32, G_WIDTH, G_HEIGHT);
    bm.to_shader((tx, ty), SamplingOptions::default(), None)
}

// Port of: gm/tilemodes.cpp#L186-L203 (chrome/m156), make_grad
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
// Port of: gm/tilemodes.cpp#L205 (chrome/m156), typedef ShaderProc
type ShaderProc = fn(TileMode, TileMode) -> Option<Shader>;

// Port of: gm/tilemodes.cpp#L207-L273 (chrome/m156), class Tiling2GM
struct Tiling2Gm {
    proc: ShaderProc,
    name: &'static str,
}

impl Tiling2Gm {
    // Port of: gm/tilemodes.cpp#L212 (chrome/m156), Tiling2GM(ShaderProc proc, const char name[])
    fn new(proc: ShaderProc, name: &'static str) -> Self {
        Self { proc, name }
    }
}

impl GM for Tiling2Gm {
    // Port of: gm/tilemodes.cpp (chrome/m156), getName
    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(650, 610)
    }

    // Port of: gm/tilemodes.cpp (chrome/m156), onDraw
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

// Port of: gm/tilemodes.cpp#L275-L275 (chrome/m156), DEF_GM( return new Tiling2GM(make_bm, ...); )
crate::def_gm!(
    Tiling2GM_bitmap = "Tiling2GM(make_bm, \"tilemode_bitmap\")",
    Tiling2Gm::new(make_bm, "tilemode_bitmap")
);

// Port of: gm/tilemodes.cpp#L276-L276 (chrome/m156), DEF_GM( return new Tiling2GM(make_grad, ...); )
crate::def_gm!(
    Tiling2GM_gradient = "Tiling2GM(make_grad, \"tilemode_gradient\")",
    Tiling2Gm::new(make_grad, "tilemode_gradient")
);

// Port of: gm/tilemodes.cpp#L273-L273 (chrome/m156), DEF_GM( return new TilingGM(true); )
crate::def_gm!(TilingGM_true = "TilingGM(true)", TilingGm::new(true));
// Port of: gm/tilemodes.cpp#L274-L274 (chrome/m156), DEF_GM( return new TilingGM(false); )
crate::def_gm!(TilingGM_false = "TilingGM(false)", TilingGm::new(false));

// Port of: gm/tilemodes.cpp#L280-L322 (chrome/m156), DEF_SIMPLE_GM tilemode_decal
crate::def_simple_gm!(tilemode_decal, canvas, 720, 1100, {
    let img = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");
    let mut bgpaint = Paint::default();
    bgpaint.set_color(Color::YELLOW);

    let r = Rect::from_ltrb(
        -20.0,
        -20.0,
        img.width() as f32 + 20.0,
        img.height() as f32 + 20.0,
    );
    canvas.translate((45.0, 45.0));

    let pairs = [
        (TileMode::Clamp, TileMode::Clamp),
        (TileMode::Clamp, TileMode::Decal),
        (TileMode::Decal, TileMode::Clamp),
        (TileMode::Decal, TileMode::Decal),
    ];
    for (tx, ty) in pairs {
        let mut paint = Paint::default();
        canvas.save();
        for proc_index in 0..5 {
            canvas.save();
            // Apply a slight rotation to highlight the differences between filtered and unfiltered
            // decal edges
            canvas.rotate(4.0, None);
            canvas.draw_rect(r, &bgpaint);
            tilemode_decal_shader(proc_index, &mut paint, &img, tx, ty);
            canvas.draw_rect(r, &paint);
            canvas.restore();
            canvas.translate((0.0, r.height() + 20.0));
        }
        canvas.restore();
        canvas.translate((r.width() + 10.0, 0.0));
    }
});

// The five `shader_procs` lambdas of `tilemode_decal`, selected by index.
// Port of: gm/tilemodes.cpp#L288-L311 (chrome/m156), shader_procs
fn tilemode_decal_shader(
    proc_index: usize,
    paint: &mut Paint,
    img: &skia_rust_core::image::Image,
    tx: TileMode,
    ty: TileMode,
) {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let grad_colors = [colors::RED, colors::BLUE];
    match proc_index {
        // Test no filtering with decal mode
        0 => {
            paint.set_shader(img.to_shader(
                (tx, ty),
                SamplingOptions::from(FilterMode::Nearest),
                None,
            ));
        }
        // Test bilerp approximation for decal mode (or clamp to border HW)
        1 => {
            paint.set_shader(img.to_shader(
                (tx, ty),
                SamplingOptions::from(FilterMode::Linear),
                None,
            ));
        }
        // Test bicubic filter with decal mode
        2 => {
            paint.set_shader(img.to_shader(
                (tx, ty),
                SamplingOptions::from(CubicResampler::mitchell()),
                None,
            ));
        }
        3 => {
            let grad = Gradient::new(
                Colors::new(&grad_colors, None, tx, None),
                Interpolation::default(),
            );
            paint.set_shader(shaders::linear_gradient(
                (Point::new(0.0, 0.0), Point::new(w * 1.0, h * 1.0)),
                &grad,
                None,
            ));
        }
        _ => {
            let grad = Gradient::new(
                Colors::new(&grad_colors, None, tx, None),
                Interpolation::default(),
            );
            paint.set_shader(shaders::radial_gradient(
                (Point::new(w * 0.5, w * 0.5), w * 0.5),
                &grad,
                None,
            ));
        }
    }
}
