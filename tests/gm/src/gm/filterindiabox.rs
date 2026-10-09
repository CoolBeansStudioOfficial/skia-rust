// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/filterindiabox.cpp (chrome/m156). The bitmap is decoded from box.gif, so it needs the
// GIF codec (skia-rust-codec's wuffs_codec) through the image generator, as ToolUtils does.

// The C++ converts between int and scalar as written: the image sizes are small, so the conversions
// are exact.
#![allow(clippy::cast_precision_loss)]

use skia_rust_codec::image_generator_from_encoded::make_from_encoded;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;

// Port of: tools/DecodeUtils.cpp#L23-L28 (ToolUtils::DecodeDataToBitmap), with the default
// colour space: the bitmap of the image generator's natural info, or `None` if it has none.
fn decode_data_to_bitmap(data: Vec<u8>) -> Option<Bitmap> {
    let mut generator = make_from_encoded(Some(Data::new_from_vec(data)), None)?;
    let info: ImageInfo = generator.info().with_color_space(None);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    if !generator.get_pixels(&info, &mut pixels, row_bytes) {
        return None;
    }
    let mut bm = Bitmap::new();
    bm.install_pixels(&info, pixels, row_bytes).then_some(bm)
}

// Port of: tools/DecodeUtils.h#L23-L25 (ToolUtils::GetResourceAsBitmap)
fn get_resource_as_bitmap(resource: &str) -> Option<Bitmap> {
    decode_data_to_bitmap(get_resource_as_data(resource)?)
}

// Port of: gm/filterindiabox.cpp#L23-L27 (computeSize)
fn compute_size(bm: &Bitmap, mat: &Matrix) -> (f32, f32) {
    let bounds = Rect::from_wh(bm.width() as f32, bm.height() as f32);
    let (bounds, _) = mat.map_rect(bounds);
    (bounds.width(), bounds.height())
}

// Port of: gm/filterindiabox.cpp#L29-L36 (draw_cell)
fn draw_cell(canvas: &Canvas, bm: &Bitmap, mat: &Matrix, dx: f32, sampling: SamplingOptions) {
    let saved = canvas.save();
    canvas.translate((dx, 0.0));
    canvas.concat(mat);
    if let Some(image) = bm.as_image() {
        canvas.draw_image_with_sampling_options(&image, (0.0, 0.0), sampling, None);
    }
    canvas.restore_to_count(saved);
}

// Port of: gm/filterindiabox.cpp#L38-L45 (draw_row)
fn draw_row(canvas: &Canvas, bm: &Bitmap, mat: &Matrix, dx: f32) {
    draw_cell(canvas, bm, mat, 0.0 * dx, SamplingOptions::default());
    draw_cell(
        canvas,
        bm,
        mat,
        1.0 * dx,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
    );
    draw_cell(
        canvas,
        bm,
        mat,
        2.0 * dx,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
    );
    draw_cell(
        canvas,
        bm,
        mat,
        3.0 * dx,
        SamplingOptions::from(CubicResampler::mitchell()),
    );
}

struct FilterIndiaBoxGm {
    bm: Bitmap,
    matrix: [Matrix; 2],
}

impl FilterIndiaBoxGm {
    fn new() -> Self {
        Self {
            bm: Bitmap::new(),
            matrix: [Matrix::new_identity(), Matrix::new_identity()],
        }
    }
}

impl GM for FilterIndiaBoxGm {
    fn name(&self) -> String {
        "filterindiabox".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(680, 130)
    }

    // Port of: gm/filterindiabox.cpp#L47-L63 (onOnceBeforeDraw)
    fn on_once_before_draw(&mut self) {
        let resource = "images/box.gif";
        self.bm = if let Some(bm) = get_resource_as_bitmap(resource) {
            bm
        } else {
            // red == bad
            let mut bm = Bitmap::new();
            let info =
                ImageInfo::new_n32_premul((1, 1), None::<skia_rust_core::color_space::ColorSpace>);
            let row_bytes = info.min_row_bytes();
            let pixels = vec![0u8; info.compute_byte_size(row_bytes)];
            let installed = bm.install_pixels(&info, pixels, row_bytes);
            debug_assert!(installed);
            bm.erase_color(Color::from_argb(255, 255, 0, 0));
            bm
        };
        self.bm.set_immutable();

        let cx = self.bm.width() as f32 / 2.0;
        let cy = self.bm.height() as f32 / 2.0;
        let vert_scale = 30.0_f32 / 55.0;
        let horiz_scale = 150.0_f32 / 200.0;
        self.matrix[0] = Matrix::scale((horiz_scale, vert_scale));
        self.matrix[1] = Matrix::rotate_deg_pivot(30.0, (cx, cy));
        self.matrix[1].post_scale((horiz_scale, vert_scale), None);
    }

    // Port of: gm/filterindiabox.cpp#L65-L78 (onDraw)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 10.0));
        for i in 0..self.matrix.len() {
            let (width, height) = compute_size(&self.bm, &self.matrix[i]);
            let size = (width + 20.0, height + 20.0);
            draw_row(canvas, &self.bm, &self.matrix[i], size.0);
            canvas.translate((0.0, size.1));
        }
    }
}

crate::def_gm!(
    FilterIndiaBoxGM_ = "FilterIndiaBoxGM()",
    FilterIndiaBoxGm::new()
);
