// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/anisotropic.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::scalar::{scalar_cos, scalar_floor_to_int, scalar_sin};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

const IMAGE_SIZE: i32 = 256;
const SPACER: i32 = 10;
const NUM_VERT_IMAGES: i32 = 5;

/// `AnisotropicGM::Mode`.
#[derive(Copy, Clone)]
enum Mode {
    Linear,
    Mip,
    Aniso,
}

// This GM exercises anisotropic image scaling.
// Port of: gm/anisotropic.cpp#L33-L164 (chrome/m156)
struct AnisotropicGm {
    image: Option<Image>,
    sampling: SamplingOptions,
    mode: Mode,
}

impl AnisotropicGm {
    // Port of: gm/anisotropic.cpp#L37-L50 (chrome/m156)
    fn new(mode: Mode) -> AnisotropicGm {
        let sampling = match mode {
            Mode::Linear => SamplingOptions::from(FilterMode::Linear),
            Mode::Mip => SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
            Mode::Aniso => SamplingOptions::from_aniso(16),
        };
        AnisotropicGm {
            image: None,
            sampling,
            mode,
        }
    }

    // Port of: gm/anisotropic.cpp#L100-L104 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn draw(&self, canvas: &Canvas, x: i32, y: i32, x_size: i32, y_size: i32) {
        let r = Rect::from_xywh(x as f32, y as f32, x_size as f32, y_size as f32);
        canvas.draw_image_rect_with_sampling_options(
            self.image.as_ref().expect("an image"),
            None,
            r,
            self.sampling,
            &Paint::default(),
        );
    }
}

impl GM for AnisotropicGm {
    fn bg_color(&self) -> Color {
        Color::new(0xFFCC_CCCC)
    }

    fn name(&self) -> String {
        let mut name = String::from("anisotropic_image_scale_");
        match self.mode {
            Mode::Linear => name += "linear",
            Mode::Mip => name += "mip",
            Mode::Aniso => name += "aniso",
        }
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            2 * IMAGE_SIZE + 3 * SPACER,
            NUM_VERT_IMAGES * IMAGE_SIZE + (NUM_VERT_IMAGES + 1) * SPACER,
        )
    }

    // Create an image consisting of lines radiating from its center
    // Port of: gm/anisotropic.cpp#L74-L98 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int * float arithmetic as in C++
    fn on_once_before_draw(&mut self) {
        const NUM_LINES: i32 = 100;
        const ANGLE_STEP: f32 = 360.0 / NUM_LINES as f32;
        const INNER_OFFSET: i32 = 10;

        let info = ImageInfo::new_n32((IMAGE_SIZE, IMAGE_SIZE), AlphaType::Opaque, None);
        let mut surf = surfaces::raster(&info, None, None).expect("a surface");
        {
            let canvas = surf.canvas();

            canvas.clear(Color::WHITE);

            let mut p = Paint::default();
            p.set_anti_alias(true);

            let mut angle = 0.0_f32;

            canvas.translate((IMAGE_SIZE as f32 / 2.0, IMAGE_SIZE as f32 / 2.0));
            for _ in 0..NUM_LINES {
                let sin = scalar_sin(angle);
                let cos = scalar_cos(angle);
                canvas.draw_line(
                    (cos * INNER_OFFSET as f32, sin * INNER_OFFSET as f32),
                    (cos * IMAGE_SIZE as f32 / 2.0, sin * IMAGE_SIZE as f32 / 2.0),
                    &p,
                );
                angle += ANGLE_STEP;
            }
        }
        self.image = surf.image_snapshot();
    }

    // Port of: gm/anisotropic.cpp#L106-L152 (chrome/m156)
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    // fImage->height() * gScales[i], size_t -> int
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // i in 0..9 (size_t casts)
    fn on_draw(&mut self, canvas: &Canvas) {
        const G_SCALES: [f32; 9] = [0.9, 0.8, 0.75, 0.6, 0.5, 0.4, 0.25, 0.2, 0.1];

        debug_assert_eq!(NUM_VERT_IMAGES - 1, G_SCALES.len() as i32 / 2);

        let (image_width, image_height) = {
            let image = self.image.as_ref().expect("an image");
            (image.width(), image.height())
        };
        let n = G_SCALES.len() as i32;

        // Minimize vertically
        for i in 0..n {
            let height = scalar_floor_to_int(image_height as f32 * G_SCALES[i as usize]);

            let y_off = if i <= n / 2 {
                SPACER + i * (image_height + SPACER)
            } else {
                // Position the more highly squashed images with their less squashed counterparts
                (n - i) * (image_height + SPACER) - height
            };

            self.draw(canvas, SPACER, y_off, image_width, height);
        }

        // Minimize horizontally
        for i in 0..n {
            let width = scalar_floor_to_int(image_width as f32 * G_SCALES[i as usize]);

            let (x_off, y_off) = if i <= n / 2 {
                (
                    image_width + 2 * SPACER,
                    SPACER + i * (image_height + SPACER),
                )
            } else {
                // Position the more highly squashed images with their less squashed counterparts
                (
                    image_width + 2 * SPACER + image_width - width,
                    SPACER + (n - i - 1) * (image_height + SPACER),
                )
            };

            self.draw(canvas, x_off, y_off, width, image_height);
        }
    }
}

// Port of: gm/anisotropic.cpp#L167-L169 (chrome/m156)
crate::def_gm!(
    AnisotropicGM_kLinear = "AnisotropicGM(AnisotropicGM::Mode::kLinear)",
    AnisotropicGm::new(Mode::Linear)
);
crate::def_gm!(
    AnisotropicGM_kMip = "AnisotropicGM(AnisotropicGM::Mode::kMip)",
    AnisotropicGm::new(Mode::Mip)
);
crate::def_gm!(
    AnisotropicGM_kAniso = "AnisotropicGM(AnisotropicGM::Mode::kAniso)",
    AnisotropicGm::new(Mode::Aniso)
);

//////////////////////////////////////////////////////////////////////////////

const ANISO_IMAGE_SIZE: i32 = 128;
const PAD: i32 = 5;

// Port of: gm/anisotropic.cpp#L173-L274 (chrome/m156)
struct AnisoMipsGm;

impl AnisoMipsGm {
    // Port of: gm/anisotropic.cpp#L182-L193 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // surf->width() * 2/5.f
    fn update_image(surf: &mut Surface<'_>, color: Color) -> Option<Image> {
        let (w, h) = (surf.width() as f32, surf.height() as f32);
        surf.canvas().clear(color);
        let mut paint = Paint::default();
        paint.set_color(Color::new(!u32::from(color) | 0xFF00_0000));
        surf.canvas().draw_rect(
            Rect::new(w * 2.0 / 5.0, h * 2.0 / 5.0, w * 3.0 / 5.0, h * 3.0 / 5.0),
            &paint,
        );
        surf.image_snapshot()?.with_default_mipmaps()
    }
}

impl GM for AnisoMipsGm {
    fn name(&self) -> String {
        "anisomips".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(520, 260)
    }

    // Port of: gm/anisotropic.cpp#L195-L266 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // ii.width() * sx
    fn on_draw(&mut self, canvas: &Canvas) {
        const SCALES: [f32; 4] = [1.0, 0.5, 0.25, 0.125];
        const COLORS: [Color; 4] = [
            Color::new(0xFFF0_F0F0),
            Color::BLUE,
            Color::GREEN,
            Color::RED,
        ];

        let canvas_info = canvas.image_info();
        let ct = if canvas_info.color_type() == ColorType::Unknown {
            ColorType::RGBA8888
        } else {
            canvas_info.color_type()
        };
        let ii = ImageInfo::new(
            (ANISO_IMAGE_SIZE, ANISO_IMAGE_SIZE),
            ct,
            AlphaType::Premul,
            canvas_info.color_space(),
        );
        // (The Ganesh surface with mipmaps is not made: there is no recording context.)
        let mut surface = match canvas.new_surface(&ii, None) {
            Some(surface) => surface,
            // could be a recording canvas.
            None => surfaces::raster(&ii, None, None).expect("a surface"),
        };

        let sampling = SamplingOptions::from_aniso(16);

        for shader in [false, true] {
            let mut c = 0;
            canvas.save();
            for sy in SCALES {
                canvas.save();
                for sx in SCALES {
                    canvas.save();
                    canvas.scale((sx, sy));
                    let image = Self::update_image(&mut surface, COLORS[c]).expect("an image");
                    if shader {
                        let mut paint = Paint::default();
                        paint.set_shader(image.to_shader(None, sampling, None));
                        canvas.draw_rect(Rect::from_isize(image.dimensions()), &paint);
                    } else {
                        canvas.draw_image_with_sampling_options(&image, (0.0, 0.0), sampling, None);
                    }
                    canvas.restore();
                    canvas.translate((ii.width() as f32 * sx + PAD as f32, 0.0));
                    c = (c + 1) % COLORS.len();
                }
                canvas.restore();
                canvas.translate((0.0, ii.width() as f32 * sy + PAD as f32));
            }
            canvas.restore();
            for sx in SCALES {
                canvas.translate((ii.width() as f32 * sx + PAD as f32, 0.0));
            }
        }
    }
}

// Port of: gm/anisotropic.cpp#L278 (chrome/m156)
crate::def_gm!(AnisoMipsGM_ = "AnisoMipsGM()", AnisoMipsGm);
