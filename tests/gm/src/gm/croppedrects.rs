// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/croppedrects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::{AutoCanvasRestore, SrcRectConstraint};
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_raster::surfaces;

const SRC_IMAGE_CLIP: Rect = Rect {
    left: 75.0,
    top: 75.0,
    right: 275.0,
    bottom: 275.0,
};

// Port of: gm/croppedrects.cpp#L31-L49 (chrome/m156)
fn create_image(_dest_canvas: &Canvas) -> Option<Image> {
    const STROKE_WIDTH: f32 = 10.0;

    let mut src_surface =
        surfaces::raster(&ImageInfo::new_n32_premul((500, 500), None), None, None)?;
    {
        let src_canvas = src_surface.canvas();

        src_canvas.clear(Color::RED);

        let mut paint = Paint::default();
        paint.set_color(Color::new(0xff00_ff00));
        src_canvas.draw_rect(SRC_IMAGE_CLIP, &paint);

        let mut stroke = Paint::default();
        stroke.set_style(Style::Stroke);
        stroke.set_stroke_width(STROKE_WIDTH);
        stroke.set_color(Color::new(0xff00_8800));
        src_canvas.draw_rect(
            SRC_IMAGE_CLIP.with_inset((STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0)),
            &stroke,
        );
    }

    // (`ToolUtils::MakeTextureImage` returns the image unchanged on a raster canvas.)
    src_surface.image_snapshot()
}

// Port of: gm/croppedrects.cpp#L51-L114 (chrome/m156)
struct CroppedRectsGm {
    src_image: Option<Image>,
    src_image_shader: Option<Shader>,
}

impl GM for CroppedRectsGm {
    fn name(&self) -> String {
        "croppedrects".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        if self.src_image.is_none() {
            self.src_image = create_image(canvas);
            if let Some(src_image) = &self.src_image {
                self.src_image_shader = src_image.to_shader(None, SamplingOptions::default(), None);
            }
        }

        canvas.clear(Color::WHITE);

        {
            // skgpu::ganesh::SurfaceDrawContext::drawFilledRect.
            let _acr = AutoCanvasRestore::guard(canvas, true);
            let mut paint = Paint::default();
            paint.set_shader(self.src_image_shader.clone());
            canvas.clip_rect(SRC_IMAGE_CLIP, None, None);
            canvas.draw_paint(&paint);
        }

        {
            // skgpu::ganesh::SurfaceDrawContext::fillRectToRect.
            let _acr = AutoCanvasRestore::guard(canvas, true);
            let draw_rect = Rect::from_xywh(350.0, 100.0, 100.0, 300.0);
            canvas.clip_rect(draw_rect, None, None);
            if let Some(src_image) = &self.src_image {
                canvas.draw_image_rect(
                    src_image,
                    Some((
                        &SRC_IMAGE_CLIP
                            .with_outset((0.5 * SRC_IMAGE_CLIP.width(), SRC_IMAGE_CLIP.height())),
                        SrcRectConstraint::Strict,
                    )),
                    draw_rect.with_outset((0.5 * draw_rect.width(), draw_rect.height())),
                    &Paint::default(),
                );
            }
        }

        {
            // skgpu::ganesh::SurfaceDrawContext::fillRectWithLocalMatrix.
            let _acr = AutoCanvasRestore::guard(canvas, true);
            let path = Path::line(
                (
                    SRC_IMAGE_CLIP.left - SRC_IMAGE_CLIP.width(),
                    SRC_IMAGE_CLIP.center_y(),
                ),
                (
                    SRC_IMAGE_CLIP.right + 3.0 * SRC_IMAGE_CLIP.width(),
                    SRC_IMAGE_CLIP.center_y(),
                ),
            );
            let mut paint = Paint::default();
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(2.0 * SRC_IMAGE_CLIP.height());
            paint.set_shader(self.src_image_shader.clone());
            canvas.translate((23.0, 301.0));
            canvas.scale((
                300.0 / SRC_IMAGE_CLIP.width(),
                100.0 / SRC_IMAGE_CLIP.height(),
            ));
            canvas.translate((-SRC_IMAGE_CLIP.left, -SRC_IMAGE_CLIP.top));
            canvas.clip_rect(SRC_IMAGE_CLIP, None, None);
            canvas.draw_path(&path, &paint);
        }

        // TODO: assert the draw target only has one op in the post-MDB world.
    }
}

// Port of: gm/croppedrects.cpp#L116 (chrome/m156)
crate::def_gm!(
    CroppedRectsGM_ = "CroppedRectsGM()",
    CroppedRectsGm {
        src_image: None,
        src_image_shader: None,
    }
);
