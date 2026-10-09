// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/localmatrixshader.cpp (chrome/m156)
//
// Not ported here: `localmatrixshader_persp` (it needs `SkImage::scalePixels`, which skia-rust
// does not have).

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]
use crate::prelude::*;
use crate::tool_utils::{get_resource_as_image, make_texture_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::shaders::ImageShader;
use skia_rust_core::tile_mode::TileMode;

const K_SIZE: f32 = 50.0;

// Port of: gm/localmatrixshader.cpp#L13-L29 (chrome/m156), make_image
fn make_image(root_canvas: &Canvas) -> Option<Image> {
    let info = ImageInfo::new((50, 50), ColorType::N32, AlphaType::Premul, None);
    let mut surface = skia_rust_raster::surfaces::raster(&info, None, None)?;
    {
        let canvas = surface.canvas();
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_color(Color::GREEN);
        canvas.draw_circle((K_SIZE / 2.0, K_SIZE / 2.0), K_SIZE / 2.0, &p);
        p.set_style(Style::Stroke);
        p.set_color(Color::RED);
        canvas.draw_line(
            (K_SIZE * 0.25, K_SIZE * 0.50),
            (K_SIZE * 0.75, K_SIZE * 0.50),
            &p,
        );
        canvas.draw_line(
            (K_SIZE * 0.50, K_SIZE * 0.25),
            (K_SIZE * 0.50, K_SIZE * 0.75),
            &p,
        );
    }
    let img = surface.image_snapshot();
    make_texture_image(root_canvas, img)
}

// Port of: gm/localmatrixshader.cpp#L38-L44 (chrome/m156), the `makeShader(SkSamplingOptions(), m)`
// helper used by the nested factories.
fn image_shader(img: &Image, local_matrix: Option<&Matrix>) -> Option<Shader> {
    ImageShader::make(
        Some(img.clone()),
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::default(),
        local_matrix,
        false,
    )
}

// Factories for the nested local-matrix shader arrangements of `localmatrixshader_nested`.
type NestedFactory = fn(&Image, &Matrix, &Matrix) -> Option<Shader>;

// Port of: gm/localmatrixshader.cpp#L58-L61 (chrome/m156), SkLocalMatrixShader(SkImageShader(inner), outer)
fn nested_factory_0(img: &Image, inner: &Matrix, outer: &Matrix) -> Option<Shader> {
    Some(image_shader(img, Some(inner))?.with_local_matrix(outer))
}

// Port of: gm/localmatrixshader.cpp#L63-L66 (chrome/m156), SkLocalMatrixShader(SkLocalMatrixShader(SkImageShader(I), inner), outer)
fn nested_factory_1(img: &Image, inner: &Matrix, outer: &Matrix) -> Option<Shader> {
    Some(
        image_shader(img, None)?
            .with_local_matrix(inner)
            .with_local_matrix(outer),
    )
}

// Port of: gm/localmatrixshader.cpp#L68-L73 (chrome/m156), SkLocalMatrixShader(SkComposeShader(SkImageShader(inner)), outer)
fn nested_factory_2(img: &Image, inner: &Matrix, outer: &Matrix) -> Option<Shader> {
    let blended = shaders::blend(
        BlendMode::SrcOver,
        shaders::color(Color::TRANSPARENT),
        image_shader(img, Some(inner))?,
    );
    Some(blended.with_local_matrix(outer))
}

// Port of: gm/localmatrixshader.cpp#L75-L81 (chrome/m156), SkLocalMatrixShader(SkComposeShader(SkLocalMatrixShader(SkImageShader(I), inner)), outer)
fn nested_factory_3(img: &Image, inner: &Matrix, outer: &Matrix) -> Option<Shader> {
    let blended = shaders::blend(
        BlendMode::SrcOver,
        shaders::color(Color::TRANSPARENT),
        image_shader(img, None)?.with_local_matrix(inner),
    );
    Some(blended.with_local_matrix(outer))
}

// Port of: gm/localmatrixshader.cpp#L50-L116 (chrome/m156), DEF_SIMPLE_GM(localmatrixshader_nested)
crate::def_simple_gm!(localmatrixshader_nested, canvas, 450, 1200, {
    let Some(image) = make_image(canvas) else {
        return;
    };
    let factories: [NestedFactory; 4] = [
        nested_factory_0,
        nested_factory_1,
        nested_factory_2,
        nested_factory_3,
    ];
    let outer = Matrix::scale((2.0, 2.0));
    let inner = Matrix::translate((20.0, 20.0));
    let mut border = Paint::default();
    border.set_anti_alias(true);
    border.set_style(Style::Stroke);
    let rect = Rect::from(image.bounds());
    let (rect, _) = Matrix::concat(&outer, &inner).map_rect(rect);

    let draw_column = |canvas: &Canvas| {
        canvas.save();
        for factory in factories {
            let mut p = Paint::default();
            if let Some(shader) = factory(&image, &inner, &outer) {
                p.set_shader(shader);
            }
            canvas.draw_rect(rect, &p);
            canvas.draw_rect(rect, &border);
            canvas.translate((0.0, rect.height() * 1.5));
        }
        canvas.restore();
    };

    draw_column(canvas);
    canvas.save();
    canvas.translate((0.0, rect.height() * factories.len() as f32 * 1.5));
    draw_column(canvas);
    canvas.restore();

    canvas.translate((rect.width() * 1.5, 0.0));
    canvas.scale((2.0, 2.0));
    draw_column(canvas);
});

// Port of: gm/localmatrixshader.cpp#L219-L270 (chrome/m156), LocalMatrixOrder
struct LocalMatrixOrderGm {
    shader: Option<Shader>,
}

impl GM for LocalMatrixOrderGm {
    fn name(&self) -> String {
        "localmatrix_order".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    // Port of: gm/localmatrixshader.cpp#L226-L240 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let Some(mandrill) = get_resource_as_image("images/mandrill_256.png") else {
            return;
        };
        let Some(example5) = get_resource_as_image("images/example_5.png") else {
            return;
        };
        let rotate_about_center = Matrix::rotate_deg_pivot(45.0, (128.0, 128.0));
        let Some(mshader) = ImageShader::make(
            Some(mandrill),
            TileMode::Repeat,
            TileMode::Repeat,
            &SamplingOptions::from(FilterMode::Nearest),
            Some(&rotate_about_center),
            false,
        ) else {
            return;
        };
        // make same size as mandrill and...
        let scale = Matrix::scale((2.0, 2.0));
        let Some(eshader) = ImageShader::make(
            Some(example5),
            TileMode::Repeat,
            TileMode::Repeat,
            &SamplingOptions::from(FilterMode::Nearest),
            Some(&scale),
            false,
        ) else {
            return;
        };
        // ... rotate about center
        let eshader = eshader.with_local_matrix(&rotate_about_center);
        // blend the two rotated and aligned images.
        self.shader = Some(shaders::blend(BlendMode::Modulate, mshader, eshader));
    }

    // Port of: gm/localmatrixshader.cpp#L242-L258 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // Rotate fShader about the canvas center
        let mut center = Rect::from(canvas.image_info().bounds()).center();
        // viewer can insert a dpi scaling matrix. Make the animation always rotate about the
        // device center.
        if let Some(ictm) = canvas.total_matrix().invert() {
            center = ictm.map_point(center);
        }
        // The animation angle starts at zero (fAngle = 0.f) and this harness does not animate.
        let angle: f32 = 0.0;
        let Some(fshader) = self.shader.clone() else {
            return;
        };
        let shader = fshader.with_local_matrix(&Matrix::rotate_deg_pivot(angle, center));
        let mut paint = Paint::default();
        paint.set_shader(shader);
        canvas.draw_paint(&paint);
    }
}

// Port of: gm/localmatrixshader.cpp#L270 (chrome/m156), DEF_GM(return new LocalMatrixOrder;)
crate::def_gm!(LocalMatrixOrder, LocalMatrixOrderGm { shader: None });
