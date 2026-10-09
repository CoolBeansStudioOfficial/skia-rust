// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/perspimages.cpp (chrome/m156)

use crate::GM;
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::canvas::{Canvas, SrcRectConstraint};
use skia_rust_core::image::Image;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;

// Port of: gm/perspimages.cpp#L10-L12 (chrome/m156), make_image1
fn make_image1() -> Image {
    get_resource_as_image("images/mandrill_128.png").expect("images/mandrill_128.png")
}

// Port of: gm/perspimages.cpp#L14-L16 (chrome/m156), make_image2
fn make_image2() -> Image {
    get_resource_as_image("images/brickwork-texture.jpg")
        .expect("images/brickwork-texture.jpg")
        .make_subset(
            IRect::from_ltrb(0, 0, 128, 128),
            RequiredProperties::default(),
        )
        .expect("a subset of the brickwork texture")
}

// Port of: gm/perspimages.cpp#L23-L31 (chrome/m156), PerspImages
struct PerspImagesGm {
    images: Vec<Image>,
}

#[derive(Clone, Copy)]
enum DrawType {
    DrawImage,
    DrawImageRectStrict,
    DrawImageRectFast,
}

impl GM for PerspImagesGm {
    fn name(&self) -> String {
        "persp_images".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1150, 1280)
    }

    // Port of: gm/perspimages.cpp#L33-L36 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.images.push(make_image1());
        self.images.push(make_image2());
    }

    // Port of: gm/perspimages.cpp#L38-L107 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut matrices: Vec<Matrix> = Vec::new();
        let mut m0 = Matrix::new_identity();
        m0.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.005, 1.0);
        matrices.push(m0);
        let mut m1 = Matrix::new_identity();
        m1.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.007, -0.005, 1.0);
        matrices.push(m1);
        matrices[1].pre_skew((0.2, -0.1), None);
        matrices[1].pre_rotate(-65.0, None);
        matrices[1].pre_scale((1.2, 0.8), None);
        matrices[1].post_translate((0.0, 60.0));
        let mut paint = Paint::default();
        let mut n = 0;
        let mut bounds = Rect::new_empty();
        for img in &self.images {
            let img_b = Rect::from_wh(img.width() as f32, img.height() as f32);
            for m in &matrices {
                let (temp, _) = m.map_rect(img_b);
                bounds.join(temp);
            }
        }
        canvas.translate((-bounds.left() + 10.0, -bounds.top() + 10.0));
        canvas.save();
        for draw_type in [
            DrawType::DrawImage,
            DrawType::DrawImageRectStrict,
            DrawType::DrawImageRectFast,
        ] {
            for m in &matrices {
                for aa in [false, true] {
                    paint.set_anti_alias(aa);
                    for sampling in [
                        SamplingOptions::from(FilterMode::Nearest),
                        SamplingOptions::from(FilterMode::Linear),
                        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
                        SamplingOptions::from(CubicResampler::mitchell()),
                    ] {
                        for orig_image in &self.images {
                            // ToolUtils::MakeTextureImage is the identity without a GPU context.
                            let img = orig_image;
                            canvas.save();
                            canvas.concat(m);
                            let w = img.width() as f32;
                            let h = img.height() as f32;
                            let src =
                                Rect::from_ltrb(w / 4.0, h / 4.0, 3.0 * w / 4.0, 3.0 * h / 4.0);
                            let dst = Rect::from_ltrb(0.0, 0.0, 3.0 / 4.0 * w, 3.0 / 4.0 * h);
                            match draw_type {
                                DrawType::DrawImage => {
                                    canvas.draw_image_with_sampling_options(
                                        img,
                                        (0.0, 0.0),
                                        sampling,
                                        Some(&paint),
                                    );
                                }
                                DrawType::DrawImageRectStrict => {
                                    canvas.draw_image_rect_with_sampling_options(
                                        img,
                                        Some((&src, SrcRectConstraint::Strict)),
                                        dst,
                                        sampling,
                                        &paint,
                                    );
                                }
                                DrawType::DrawImageRectFast => {
                                    canvas.draw_image_rect_with_sampling_options(
                                        img,
                                        Some((&src, SrcRectConstraint::Fast)),
                                        dst,
                                        sampling,
                                        &paint,
                                    );
                                }
                            }
                            canvas.restore();
                            n += 1;
                            if n < 8 {
                                canvas.translate((bounds.width() + 10.0, 0.0));
                            } else {
                                canvas.restore();
                                canvas.translate((0.0, bounds.height() + 10.0));
                                canvas.save();
                                n = 0;
                            }
                        }
                    }
                }
            }
        }
        canvas.restore();
    }
}

// Port of: gm/perspimages.cpp#L121-L121 (chrome/m156), DEF_GM(return new PerspImages();)
crate::def_gm!(
    PerspImagesGM_ = "PerspImages()",
    PerspImagesGm { images: Vec::new() }
);
