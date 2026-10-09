// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image_shader.cpp (chrome/m156)
//
// Not ported: `ImageShaderGM` (picture, encoded and texture images), `textureimage_and_shader`
// (`SkCanvas::getSurface`).

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/image_shader.cpp#L174-L178 (chrome/m156)
fn make_checker_img(w: i32, h: i32, c0: Color, c1: Color, size: i32) -> Option<Image> {
    let mut bm = tool_utils::create_checkerboard_bitmap(w, h, c0, c1, size);
    bm.set_immutable();
    bm.as_image()
}

// Port of: gm/image_shader.cpp#L180-L218 (chrome/m156)
crate::def_simple_gm!(drawimage_sampling, canvas, 500, 500, {
    const N: i32 = 256;
    const SCALE: f32 = 1.0 / 6.0;
    #[allow(clippy::cast_precision_loss)] // kScale*N
    let dst = Rect::new(0.0, 0.0, SCALE * N as f32, SCALE * N as f32);

    let img = make_checker_img(N, N, Color::BLACK, Color::WHITE, 7)
        .expect("an image")
        .with_default_mipmaps()
        .expect("an image");
    #[allow(clippy::cast_precision_loss)] // SkRect::MakeIWH
    let src = Rect::from_wh(img.width() as f32, img.height() as f32);

    let mx = Matrix::rect_to_rect_or_identity(src, dst, None);

    let mut paint = Paint::default();

    for mm in [MipmapMode::None, MipmapMode::Nearest, MipmapMode::Linear] {
        for fm in [FilterMode::Nearest, FilterMode::Linear] {
            let sampling = SamplingOptions::new(fm, mm);

            canvas.save();

            canvas.save();
            canvas.concat(&mx);
            canvas.draw_image_with_sampling_options(&img, (0.0, 0.0), sampling, None);
            canvas.restore();

            canvas.translate((dst.width() + 4.0, 0.0));

            paint.set_shader(img.to_shader((TileMode::Clamp, TileMode::Clamp), sampling, &mx));
            canvas.draw_rect(dst, &paint);

            canvas.translate((dst.width() + 4.0, 0.0));

            // (`paint = nullptr` is the default paint.)
            canvas.draw_image_rect_with_sampling_options(
                &img,
                Some((&src, SrcRectConstraint::Fast)),
                dst,
                sampling,
                &Paint::default(),
            );
            canvas.restore();

            canvas.translate((0.0, dst.height() + 8.0));
        }
    }
});

// Port of: gm/image_shader.cpp#L254-L271 (chrome/m156)
crate::def_simple_gm!(imageshader_tinyscale, canvas, 1000, 1000, {
    // A small scale amplifies the sampling coords in image space.
    const K_SCALE: f32 = 0.01;

    // 128x128px image with red/black/green/blue quadrants.
    let img = tool_utils::get_resource_as_image("images/gainmap_gcontainer_only.jpg")
        .expect("images/gainmap_gcontainer_only.jpg");

    let m =Matrix::translate((500.0, 500.0)) * Matrix::scale((K_SCALE, K_SCALE));

    // In clamp mode we should see no repeating patterns, just the viewport filled
    // with four-colored quadrants.
    let mut p = Paint::default();
    p.set_shader(img.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        &m,
    ));
    canvas.draw_paint(&p);
});
