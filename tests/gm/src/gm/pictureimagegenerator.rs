// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pictureimagegenerator.cpp (chrome/m156)

//! The picture image generator (`SkImageGenerators::MakeFromPicture`) drawn at a range of sizes,
//! scales and opacities, next to the vector logo it draws.

use crate::GM;
use crate::canvas::Canvas;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_generator::ImageGenerator;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::utils::text_utils::get_path;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::image_picture::{BitDepth, make_from_picture};
use skia_rust_tools::font_tool_utils::default_portable_font;

const K_PICTURE_WIDTH: f32 = 200.0;
const K_PICTURE_HEIGHT: f32 = 100.0;

/// `kGradientPad`, `kVerticalSpacing` and `kAccentScale` of `draw_vector_logo`.
// Port of: gm/pictureimagegenerator.cpp#L46-L48 (chrome/m156)
const K_GRADIENT_PAD: f32 = 0.1;
const K_VERTICAL_SPACING: f32 = 0.25;
const K_ACCENT_SCALE: f32 = 1.20;

/// `SkColorConverter::colors4f()`: each `SkColor` as an `SkColor4f`.
// Port of: tools/ToolUtils.h (SkColorConverter, chrome/m156)
fn colors4f(colors: &[Color]) -> Vec<skia_rust_core::color::Color4f> {
    colors
        .iter()
        .map(|&c| skia_rust_core::color::Color4f::from(c))
        .collect()
}

// Port of: gm/pictureimagegenerator.cpp#L44-L121 (chrome/m156), draw_vector_logo
#[allow(clippy::similar_names, clippy::too_many_lines)] // mirrors the C++ names (iBox, skiBox, pos1...) and body
#[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar of small sizes (exact in f32)
fn draw_vector_logo(canvas: &Canvas, view_box: Rect) {
    const SKIA_STR: &[u8] = b"SKIA";

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut font = default_portable_font();
    font.set_subpixel(true);
    font.set_embolden(true);

    let path = get_path(b"SKI", TextEncoding::UTF8, 0.0, 0.0, &font);
    let ski_box = path.compute_tight_bounds();

    let path = get_path(b"I", TextEncoding::UTF8, 0.0, 0.0, &font);
    let mut i_box = path.compute_tight_bounds();
    i_box.offset_to((ski_box.right - i_box.width(), i_box.top));

    let path = get_path(SKIA_STR, TextEncoding::UTF8, 0.0, 0.0, &font);
    let mut skia_box = path.compute_tight_bounds();
    skia_box.outset((0.0, 2.0 * i_box.width() * (K_VERTICAL_SPACING + 1.0)));

    let accent_size = i_box.width() * K_ACCENT_SCALE;
    let underline_y = i_box.bottom() + (K_VERTICAL_SPACING + 3.0_f32.sqrt() / 2.0) * accent_size;
    canvas.save();
    canvas.concat(&Matrix::rect_to_rect_or_identity(skia_box, view_box, None));

    canvas.draw_circle(
        (
            i_box.center_x(),
            i_box.y() - (0.5 + K_VERTICAL_SPACING) * accent_size,
        ),
        accent_size / 2.0,
        &paint,
    );

    let mut builder = PathBuilder::new();
    builder
        .move_to((
            i_box.center_x() - accent_size / 2.0,
            i_box.bottom() + K_VERTICAL_SPACING * accent_size,
        ))
        .r_line_to((accent_size, 0.0))
        .line_to((i_box.center_x(), underline_y));
    let path = builder.detach();
    canvas.draw_path(&path, &paint);

    let underline_rect = Rect::from_ltrb(
        i_box.center_x() - i_box.width() * accent_size * 3.0,
        underline_y,
        i_box.center_x(),
        underline_y + accent_size / 10.0,
    );
    let pts1 = [(underline_rect.x(), 0.0), (i_box.center_x(), 0.0)];
    let pos1 = [0.0, 0.75];
    let colors1 = colors4f(&[Color::TRANSPARENT, Color::BLACK]);
    paint.set_shader(gradient_linear(pts1, &colors1, &pos1));
    canvas.draw_rect(underline_rect, &paint);

    let pts2 = [
        (i_box.x() - i_box.width() * K_GRADIENT_PAD, 0.0),
        (i_box.right() + i_box.width() * K_GRADIENT_PAD, 0.0),
    ];
    let pos2 = [
        0.0,
        0.01,
        1.0 / 3.0,
        1.0 / 3.0,
        2.0 / 3.0,
        2.0 / 3.0,
        0.99,
        1.0,
    ];
    let colors2 = [
        Color::new(0xFF00_0000),
        Color::new(0xffca_5139),
        Color::new(0xffca_5139),
        Color::new(0xff8d_bd53),
        Color::new(0xff8d_bd53),
        Color::new(0xff54_60a5),
        Color::new(0xff54_60a5),
        Color::new(0xFF00_0000),
    ];
    paint.set_shader(gradient_linear(pts2, &colors4f(&colors2), &pos2));
    canvas.draw_simple_text(SKIA_STR, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);

    canvas.restore();
}

/// `SkShaders::LinearGradient(pts, {{colors, pos, SkTileMode::kClamp}, {}})`.
// Port of: gm/pictureimagegenerator.cpp#L101-L121 (chrome/m156), the gradients of draw_vector_logo
fn gradient_linear(
    points: [(f32, f32); 2],
    colors: &[skia_rust_core::color::Color4f],
    positions: &[f32],
) -> Option<Shader> {
    let gradient = Gradient::new(
        Colors::new(
            colors,
            Some(positions),
            skia_rust_core::tile_mode::TileMode::Clamp,
            None,
        ),
        Interpolation::default(),
    );
    gradient_shaders::linear_gradient((points[0], points[1]), &gradient, None)
}

/// One config of `PictureGeneratorGM::onDraw`: the size, the scales and the opacity.
struct Config {
    size: ISize,
    scale_x: f32,
    scale_y: f32,
    opacity: f32,
}

// Port of: gm/pictureimagegenerator.cpp#L123-L213 (chrome/m156), class PictureGeneratorGM
struct PictureGeneratorGm {
    picture: Option<Picture>,
}

impl GM for PictureGeneratorGm {
    // Port of: gm/pictureimagegenerator.cpp#L127-L127 (chrome/m156), getName
    fn name(&self) -> String {
        "pictureimagegenerator".to_string()
    }

    // Port of: gm/pictureimagegenerator.cpp#L129-L129 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1160, 860)
    }

    // Port of: gm/pictureimagegenerator.cpp#L131-L138 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let rect = Rect::from_wh(K_PICTURE_WIDTH, K_PICTURE_HEIGHT);
        let mut recorder = PictureRecorder::new();
        draw_vector_logo(recorder.begin_recording(rect, false), rect);
        self.picture = recorder.finish_recording_as_picture(None);
    }

    // Port of: gm/pictureimagegenerator.cpp#L140-L213 (chrome/m156), onDraw
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)] // the C++ config table is inline; SkIntToScalar of small sizes
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(picture) = self.picture.clone() else {
            return;
        };
        let configs = [
            Config {
                size: ISize::new(200, 100),
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 1.0,
                scale_y: 2.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(400, 200),
                scale_x: 2.0,
                scale_y: 2.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 100),
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 0.9,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 0.75,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 1.0,
                scale_y: 2.0,
                opacity: 0.5,
            },
            Config {
                size: ISize::new(400, 200),
                scale_x: 2.0,
                scale_y: 2.0,
                opacity: 0.25,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 0.5,
                scale_y: 1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 1.0,
                scale_y: 0.5,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 0.5,
                scale_y: 0.5,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 200),
                scale_x: 2.0,
                scale_y: 2.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 100),
                scale_x: -1.0,
                scale_y: 1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 100),
                scale_x: 1.0,
                scale_y: -1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 100),
                scale_x: -1.0,
                scale_y: -1.0,
                opacity: 1.0,
            },
            Config {
                size: ISize::new(200, 100),
                scale_x: -1.0,
                scale_y: -1.0,
                opacity: 0.5,
            },
        ];

        let srgb_color_space = ColorSpace::new_srgb();
        let draws_per_row = 4;
        let draw_size: f32 = 250.0;

        for (i, config) in configs.iter().enumerate() {
            let mut p = Paint::default();
            p.set_alpha_f(config.opacity);

            let mut m = Matrix::scale((config.scale_x, config.scale_y));
            if config.scale_x < 0.0 {
                m.post_translate((config.size.width as f32, 0.0));
            }
            if config.scale_y < 0.0 {
                m.post_translate((0.0, config.size.height as f32));
            }
            let paint = if p.alpha() == 255 { None } else { Some(&p) };
            let mut generator: Box<dyn ImageGenerator> = make_from_picture(
                config.size,
                picture.clone(),
                Some(&m),
                paint,
                BitDepth::U8,
                Some(srgb_color_space.clone()),
                SurfaceProps::default(),
            )
            .expect("a picture generator");

            // `gen->getInfo().makeColorSpace(canvas->imageInfo().refColorSpace())`
            let bm_info = generator
                .info()
                .with_color_space(canvas.image_info().color_space());

            let mut bm = Bitmap::new();
            bm.alloc_pixels_info(&bm_info, None);
            {
                let mut pixels = bm.peek_pixels_mut().expect("the bitmap's pixels");
                assert!(generator.get_pixels_into(&mut pixels));
            }

            let x = draw_size * (i % draws_per_row) as f32;
            let y = draw_size * (i / draws_per_row) as f32;

            p.set_color(Color::new(0xfff0_f0f0));
            p.set_alpha_f(1.0);
            canvas.draw_rect(
                Rect::from_xywh(x, y, bm.width() as f32, bm.height() as f32),
                &p,
            );
            if let Some(image) = bm.as_image() {
                canvas.draw_image(&image, (x, y), None);
            }
        }
    }
}

// Port of: gm/pictureimagegenerator.cpp#L214 (chrome/m156), DEF_GM(return new PictureGeneratorGM;)
crate::def_gm!(
    #[ignore = "see notes/pictureimagegenerator.md"]
    PictureGeneratorGM = "PictureGeneratorGM",
    PictureGeneratorGm { picture: None }
);
