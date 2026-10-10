// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/coordclampshader.cpp (chrome/m156)

use crate::tool_utils::{get_resource_as_image, int_to_scalar};
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::CoordClampShader;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/coordclampshader.cpp#L9-L60 (chrome/m156), coordclampshader
crate::def_simple_gm!(coordclampshader, canvas, 1074, 795, {
    let Some(image) = get_resource_as_image("images/mandrill_256.png") else {
        return;
    };
    // The mandrill_512 image has a bottom row of mostly black pixels. Remove it.
    let image = image
        .make_subset(
            IRect::from_wh(image.width(), image.height() - 1),
            RequiredProperties::default(),
        )
        .expect("a subset of the image");
    let image = image.with_default_mipmaps().expect("an image with mipmaps");
    let image_shader: Shader = image
        .to_shader(None, FilterMode::Linear, None)
        .expect("a shader");

    let mut paint = Paint::default();
    let draw_rect = Rect::new(
        0.0,
        0.0,
        int_to_scalar(image.width()),
        int_to_scalar(image.height()),
    );
    let rotate = Matrix::rotate_deg_pivot(45.0, draw_rect.center());
    let mut clamp_rect = draw_rect;
    clamp_rect.inset((20.0, 40.0));
    canvas.translate((10.0, 10.0));
    let mut shader = CoordClampShader::make(Some(image_shader.clone()), clamp_rect);
    paint.set_shader(shader.clone());
    canvas.draw_rect(draw_rect, &paint);
    canvas.save();
    canvas.translate((int_to_scalar(image.width()), 0.0));
    shader = CoordClampShader::make(Some(image_shader.with_local_matrix(&rotate)), clamp_rect);
    paint.set_shader(shader.clone());
    canvas.draw_rect(draw_rect, &paint);
    canvas.restore();
    canvas.save();
    canvas.translate((0.0, int_to_scalar(image.height())));
    shader = CoordClampShader::make(Some(image_shader.clone()), clamp_rect)
        .map(|s| s.with_local_matrix(&rotate));
    paint.set_shader(shader.clone());
    canvas.draw_rect(draw_rect, &paint);
    canvas.restore();
    canvas.save();
    canvas.translate((int_to_scalar(image.width()), int_to_scalar(image.height())));
    shader = CoordClampShader::make(Some(image_shader.with_local_matrix(&rotate)), clamp_rect)
        .map(|s| s.with_local_matrix(&rotate));
    paint.set_shader(shader);
    canvas.draw_rect(draw_rect, &paint);
    canvas.restore();
    canvas.translate((0.0, 2.0 * int_to_scalar(image.height()) + 10.0));
    let samplers = [
        SamplingOptions::from(FilterMode::Nearest),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        SamplingOptions::from_aniso(16),
    ];
    for sampler in samplers {
        let mut scale = Matrix::new_identity();
        scale.set_scale((0.3, 1.0), None);
        let image_shader = image
            .to_shader((TileMode::Mirror, TileMode::Mirror), sampler, &scale)
            .expect("a shader");
        shader = CoordClampShader::make(Some(image_shader), clamp_rect);
        paint.set_shader(shader.clone());
        canvas.draw_rect(draw_rect, &paint);
        canvas.translate((int_to_scalar(image.width()) + 10.0, 0.0));
    }
});
