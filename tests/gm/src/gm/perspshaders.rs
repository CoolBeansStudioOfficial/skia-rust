// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/perspshaders.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]
use crate::prelude::*;
use crate::tool_utils::{
    create_checkerboard_image, draw_checkerboard, get_resource_as_image, make_surface,
};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::ImageShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

const CELL_SIZE: i32 = 50;
const NUM_ROWS: i32 = 5;
const NUM_COLS: i32 = 6;

// Port of: gm/perspshaders.cpp#L14-L22 (chrome/m156), make_image
fn make_image(orig_canvas: &Canvas, w: i32, h: i32) -> Option<Image> {
    let info = ImageInfo::new((w, h), ColorType::N32, AlphaType::Premul, None);
    let mut surface = make_surface(orig_canvas, &info, None)?;
    draw_checkerboard(surface.canvas(), Color::RED, Color::GREEN, w / 10);
    surface.image_snapshot()
}

// Port of: gm/perspshaders.cpp#L24-L120 (chrome/m156), PerspShadersGM
struct PerspShadersGm {
    do_aa: bool,
    path: Option<Path>,
    linear_grad1: Option<Shader>,
    linear_grad2: Option<Shader>,
    persp_matrix: Matrix,
    image: Option<Image>,
    bitmap_image: Option<Image>,
}

impl PerspShadersGm {
    // Port of: gm/perspshaders.cpp#L62-L95 (chrome/m156), drawRow
    fn draw_row(&self, canvas: &Canvas, sampling: SamplingOptions) {
        let Some(bitmap_image) = self.bitmap_image.as_ref() else {
            return;
        };
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let cell = Rect::from_wh(CELL_SIZE as f32, CELL_SIZE as f32);

        let mut filter_paint = Paint::default();
        filter_paint.set_anti_alias(self.do_aa);
        let mut path_paint = Paint::default();
        path_paint.set_shader(ImageShader::make(
            Some(bitmap_image.clone()),
            TileMode::Clamp,
            TileMode::Clamp,
            &sampling,
            None,
            false,
        ));
        path_paint.set_anti_alias(self.do_aa);
        let mut grad_paint1 = Paint::default();
        grad_paint1.set_shader(self.linear_grad1.clone());
        grad_paint1.set_anti_alias(self.do_aa);
        let mut grad_paint2 = Paint::default();
        grad_paint2.set_shader(self.linear_grad2.clone());
        grad_paint2.set_anti_alias(self.do_aa);

        let cell_step = CELL_SIZE as f32;
        let Some(path) = self.path.as_ref() else {
            return;
        };

        canvas.save();
        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_image_rect_with_sampling_options(
            bitmap_image,
            None,
            cell,
            sampling,
            &filter_paint,
        );
        canvas.restore();
        canvas.translate((cell_step, 0.0));

        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_image_with_sampling_options(image, (0.0, 0.0), sampling, Some(&filter_paint));
        canvas.restore();
        canvas.translate((cell_step, 0.0));

        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_rect(cell, &path_paint);
        canvas.restore();
        canvas.translate((cell_step, 0.0));

        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_path(path, &path_paint);
        canvas.restore();
        canvas.translate((cell_step, 0.0));

        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_rect(cell, &grad_paint1);
        canvas.restore();
        canvas.translate((cell_step, 0.0));

        canvas.save();
        canvas.concat(&self.persp_matrix);
        canvas.draw_path(path, &grad_paint2);
        canvas.restore();

        canvas.restore();
    }
}

impl GM for PerspShadersGm {
    fn name(&self) -> String {
        if self.do_aa {
            "persp_shaders_aa".to_string()
        } else {
            "persp_shaders_bw".to_string()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(CELL_SIZE * NUM_COLS, CELL_SIZE * NUM_ROWS)
    }

    // Port of: gm/perspshaders.cpp#L36-L60 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.bitmap_image = Some(create_checkerboard_image(
            CELL_SIZE,
            CELL_SIZE,
            Color::BLUE,
            Color::YELLOW,
            CELL_SIZE / 10,
        ));
        let pts1 = [
            Point::new(0.0, 0.0),
            Point::new(CELL_SIZE as f32, CELL_SIZE as f32),
        ];
        let pts2 = [Point::new(0.0, 0.0), Point::new(0.0, CELL_SIZE as f32)];
        let colors = [
            Color4f::from(Color::RED),
            Color4f::from(Color::GREEN),
            Color4f::from(Color::RED),
            Color4f::from(Color::GREEN),
            Color4f::from(Color::RED),
        ];
        let pos: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
        let gradient = Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        );
        self.linear_grad1 = gradient_shaders::linear_gradient((pts1[0], pts1[1]), &gradient, None);
        self.linear_grad2 = gradient_shaders::linear_gradient((pts2[0], pts2[1]), &gradient, None);
        self.persp_matrix = Matrix::new_identity();
        self.persp_matrix.set_persp_y(1.0 / 50.0);
        let cell = CELL_SIZE as f32;
        let mut builder = PathBuilder::new();
        builder
            .move_to((0.0, 0.0))
            .line_to((0.0, cell))
            .line_to((cell / 2.0, cell / 2.0))
            .line_to((cell, cell))
            .line_to((cell, 0.0))
            .close();
        self.path = Some(builder.detach());
    }

    // Port of: gm/perspshaders.cpp#L97-L115 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        if self.image.is_none() {
            self.image = make_image(canvas, CELL_SIZE, CELL_SIZE);
        }
        let step = CELL_SIZE as f32;
        self.draw_row(canvas, SamplingOptions::from(FilterMode::Nearest));
        canvas.translate((0.0, step));
        self.draw_row(canvas, SamplingOptions::from(FilterMode::Linear));
        canvas.translate((0.0, step));
        self.draw_row(
            canvas,
            SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest),
        );
        canvas.translate((0.0, step));
        self.draw_row(canvas, SamplingOptions::from(CubicResampler::mitchell()));
        canvas.translate((0.0, step));
        self.draw_row(canvas, SamplingOptions::from_aniso(16));
        canvas.translate((0.0, step));
    }
}

// Port of: gm/perspshaders.cpp#L181 (chrome/m156), DEF_GM(return new PerspShadersGM(true);)
crate::def_gm!(
    PerspShadersGM_true = "PerspShadersGM(true)",
    PerspShadersGm {
        do_aa: true,
        path: None,
        linear_grad1: None,
        linear_grad2: None,
        persp_matrix: Matrix::new_identity(),
        image: None,
        bitmap_image: None,
    }
);

// Port of: gm/perspshaders.cpp#L182 (chrome/m156), DEF_GM(return new PerspShadersGM(false);)
crate::def_gm!(
    PerspShadersGM_false = "PerspShadersGM(false)",
    PerspShadersGm {
        do_aa: false,
        path: None,
        linear_grad1: None,
        linear_grad2: None,
        persp_matrix: Matrix::new_identity(),
        image: None,
        bitmap_image: None,
    }
);

// Port of: gm/perspshaders.cpp#L186-L199 (chrome/m156), make_path
fn make_path() -> Path {
    let mut rand = Random::default();
    let mut rand_pt = || {
        let x = rand.next_f();
        let y = rand.next_f();
        Point::new(x * 400.0, y * 400.0)
    };
    let mut builder = PathBuilder::new();
    for _ in 0..4 {
        let mut pts = [Point::new(0.0, 0.0); 6];
        for p in &mut pts {
            *p = rand_pt();
        }
        builder
            .move_to(pts[0])
            .quad_to(pts[1], pts[2])
            .quad_to(pts[3], pts[4])
            .line_to(pts[5]);
    }
    builder.detach()
}

// Port of: gm/perspshaders.cpp#L201-L224 (chrome/m156), DEF_SIMPLE_GM(perspective_clip)
crate::def_simple_gm!(perspective_clip, canvas, 800, 800, {
    let path = make_path();
    let Some(mandrill) = get_resource_as_image("images/mandrill_128.png") else {
        return;
    };
    let mut scale = Matrix::new_identity();
    scale.set_scale((3.0, 3.0), None);
    let shader = ImageShader::make(
        Some(mandrill),
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::default(),
        Some(&scale),
        false,
    );

    let mut paint = Paint::default();
    paint.set_color4f(Color4f::new(0.75, 0.75, 0.75, 1.0), None);
    canvas.draw_path(&path, &paint);

    // This is a crazy perspective matrix, derived from halfplanes3, to draw a shape where
    // part of it is "behind" the viewer, hence showing the need for "half-plane" clipping
    // when in perspective.
    let mut mx = Matrix::new_identity();
    mx.set_9(&[
        -1.7866, 1.3357, 273.0295, -1.0820, 1.3186, 135.5196, -0.0047, -0.0015, 2.1485,
    ]);
    paint.set_shader(shader);
    canvas.concat(&mx);
    canvas.draw_path(&path, &paint);
});
