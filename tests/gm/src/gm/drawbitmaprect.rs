// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawbitmaprect.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::Color4f;
use skia_rust_core::font::Font;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::math_priv::next_log2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/drawbitmaprect.cpp#L9-L19 (chrome/m156)
fn make_chessbm(w: i32, h: i32) -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((w, h), None);
    for y in 0..bm.height() {
        for x in 0..bm.width() {
            let c = if ((x + y) & 1) != 0 {
                0xFFFF_FFFF
            } else {
                0xFF00_0000
            };
            bm.set_addr32(x, y, c);
        }
    }
    bm.set_immutable();
    bm
}

// `ToolUtils::makeSurface`: the canvas's own surface kind, or a raster surface.
// Port of: tools/ToolUtils.cpp (makeSurface, chrome/m156)
fn make_surface(
    canvas: &Canvas,
    info: &ImageInfo,
) -> Option<skia_rust_raster::surface::Surface<'static>> {
    canvas
        .new_surface(info, None)
        .or_else(|| surfaces::raster(info, None, None))
}

// Port of: gm/drawbitmaprect.cpp#L21-L49 (chrome/m156)
fn makebm(orig_canvas: &Canvas, result_bm: &mut Bitmap, w: i32, h: i32) -> Option<Image> {
    let info = ImageInfo::new_n32_premul((w, h), None);
    let mut surface = make_surface(orig_canvas, &info)?;
    let w_scalar = w as f32;
    let h_scalar = h as f32;
    let pt = Point::new(w_scalar / 2.0, h_scalar / 2.0);
    let radius = 4.0 * w_scalar.max(h_scalar);
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(1.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(1.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 1.0, 1.0, 1.0),
        Color4f::new(1.0, 0.0, 0.0, 1.0),
    ];
    let pos: [f32; 7] = [
        0.0,
        1.0 / 6.0,
        2.0 * 1.0 / 6.0,
        3.0 * 1.0 / 6.0,
        4.0 * 1.0 / 6.0,
        5.0 * 1.0 / 6.0,
        1.0,
    ];
    let mut paint = Paint::default();
    let mut rect = Rect::from_wh(w_scalar, h_scalar);
    let mut mat = Matrix::new_identity();
    {
        let canvas = surface.canvas();
        canvas.clear(Color::TRANSPARENT);
        for _ in 0..4 {
            paint.set_shader(shaders::radial_gradient(
                (pt, radius),
                &Gradient::new(
                    Colors::new(&colors, Some(&pos), TileMode::Repeat, None),
                    Interpolation::default(),
                ),
                Some(&mat),
            ));
            canvas.draw_rect(rect, &paint);
            rect.inset((w_scalar / 8.0, h_scalar / 8.0));
            mat.post_scale((1.0 / 4.0, 1.0 / 4.0), None);
        }
    }
    let image = surface.image_snapshot()?;
    // `image->asLegacyBitmap(&tempBM)`: the result bitmap shares the image's pixels.
    let mut temp_bm = image.as_legacy_bitmap()?;
    temp_bm.set_immutable();
    *result_bm = temp_bm;
    Some(image)
}

// Port of: gm/drawbitmaprect.cpp#L51-L55 (chrome/m156)
type DrawRectRectProc = fn(&Canvas, &Image, &Bitmap, IRect, Rect, SamplingOptions, &Paint);

// Port of: gm/drawbitmaprect.cpp#L56-L63 (chrome/m156)
fn bitmapproc(
    canvas: &Canvas,
    image: &Image,
    _bm: &Bitmap,
    src_r: IRect,
    dst_r: Rect,
    sampling: SamplingOptions,
    paint: &Paint,
) {
    canvas.draw_image_rect_with_sampling_options(
        image,
        Some((&Rect::from_irect(src_r), SrcRectConstraint::Strict)),
        dst_r,
        sampling,
        paint,
    );
}

// Port of: gm/drawbitmaprect.cpp#L64-L78 (chrome/m156)
fn bitmapsubsetproc(
    canvas: &Canvas,
    image: &Image,
    bm: &Bitmap,
    src_r: IRect,
    dst_r: Rect,
    sampling: SamplingOptions,
    paint: &Paint,
) {
    if !bm.bounds().contains(src_r) {
        bitmapproc(canvas, image, bm, src_r, dst_r, sampling, paint);
        return;
    }
    let mut subset = Bitmap::new();
    if bm.extract_subset(&mut subset, src_r) {
        // `ToolUtils::MakeTextureImage` is the identity on a raster canvas.
        if let Some(subset_img) = subset.as_image() {
            canvas.draw_image_rect_with_sampling_options(&subset_img, None, dst_r, sampling, paint);
        }
    }
}

// Port of: gm/drawbitmaprect.cpp#L79-L86 (chrome/m156)
fn imageproc(
    canvas: &Canvas,
    image: &Image,
    _bm: &Bitmap,
    src_r: IRect,
    dst_r: Rect,
    sampling: SamplingOptions,
    paint: &Paint,
) {
    canvas.draw_image_rect_with_sampling_options(
        image,
        Some((&Rect::from_irect(src_r), SrcRectConstraint::Strict)),
        dst_r,
        sampling,
        paint,
    );
}

// Port of: gm/drawbitmaprect.cpp#L87-L99 (chrome/m156)
fn imagesubsetproc(
    canvas: &Canvas,
    image: &Image,
    bm: &Bitmap,
    src_r: IRect,
    dst_r: Rect,
    sampling: SamplingOptions,
    paint: &Paint,
) {
    if !image.bounds().contains(src_r) {
        imageproc(canvas, image, bm, src_r, dst_r, sampling, paint);
        return;
    }
    if let Some(subset) = image.make_subset(src_r, RequiredProperties::default()) {
        canvas.draw_image_rect_with_sampling_options(&subset, None, dst_r, sampling, paint);
    }
}

const G_SIZE: i32 = 1024;
const G_BMP_SIZE: i32 = 2048;

// Port of: gm/drawbitmaprect.cpp#L101-L180 (chrome/m156)
struct DrawBitmapRectGm {
    proc: DrawRectRectProc,
    name: String,
    large_bitmap: Bitmap,
    image: Option<Image>,
}

impl DrawBitmapRectGm {
    fn new(proc: DrawRectRectProc, suffix: Option<&str>) -> Self {
        let mut name = String::from("drawbitmaprect");
        if let Some(suffix) = suffix {
            name.push_str(suffix);
        }
        Self {
            proc,
            name,
            large_bitmap: Bitmap::new(),
            image: None,
        }
    }
}

impl GM for DrawBitmapRectGm {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn size(&mut self) -> ISize {
        ISize::new(G_SIZE, G_SIZE)
    }

    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if self.image.is_none() {
            // `ToolUtils::MakeTextureImage` is the identity on a raster canvas.
            self.image = makebm(canvas, &mut self.large_bitmap, G_BMP_SIZE, G_BMP_SIZE);
            if self.image.is_none() {
                *error_msg = String::from("Image creation failed");
                return DrawResult::Skip;
            }
        }
        let image = self.image.as_ref().expect("the image is made above");
        let dst_rect = Rect::from_ltrb(0.0, 0.0, 64.0, 64.0);
        let k_max_src_rect_size: i32 = 1 << (next_log2(G_BMP_SIZE as u32) + 2);
        const K_PAD_X: i32 = 30;
        const K_PAD_Y: i32 = 40;
        let mut alpha_paint = Paint::default();
        alpha_paint.set_alpha_f(0.125);
        canvas.draw_image_rect_with_sampling_options(
            image,
            None,
            Rect::from_iwh(G_SIZE, G_SIZE),
            SamplingOptions::default(),
            &alpha_paint,
        );
        canvas.translate((1.0 * K_PAD_X as f32 / 2.0, 1.0 * K_PAD_Y as f32 / 2.0));
        let mut black_paint = Paint::default();
        let title_height: f32 = 1.0 * 24.0;
        black_paint.set_color(Color::BLACK);
        black_paint.set_anti_alias(true);
        let mut font = Font::from_size(default_portable_typeface(), title_height);
        let title = format!("Bitmap size: {G_BMP_SIZE} x {G_BMP_SIZE}");
        draw_string(
            canvas,
            &title,
            0.0,
            title_height,
            &font,
            &black_paint,
            Align::Left,
        );
        canvas.translate((0.0, 1.0 * K_PAD_Y as f32 / 2.0 + title_height));
        let mut row_count = 0;
        canvas.save();
        let mut w = 1;
        while w <= k_max_src_rect_size {
            let mut h = 1;
            while h <= k_max_src_rect_size {
                let src_rect = IRect::from_xywh((G_BMP_SIZE - w) / 2, (G_BMP_SIZE - h) / 2, w, h);
                (self.proc)(
                    canvas,
                    image,
                    &self.large_bitmap,
                    src_rect,
                    dst_rect,
                    SamplingOptions::default(),
                    &Paint::default(),
                );
                let label = format!("{w} x {h}");
                black_paint.set_anti_alias(true);
                black_paint.set_style(Style::Fill);
                font.set_size(1.0 * 10.0);
                let baseline = dst_rect.height() + font.size() + 1.0 * 3.0;
                draw_string(
                    canvas,
                    &label,
                    0.0,
                    baseline,
                    &font,
                    &black_paint,
                    Align::Left,
                );
                black_paint.set_style(Style::Stroke);
                black_paint.set_stroke_width(1.0);
                black_paint.set_anti_alias(false);
                canvas.draw_rect(dst_rect, &black_paint);
                canvas.translate((dst_rect.width() + 1.0 * K_PAD_X as f32, 0.0));
                row_count += 1;
                if (dst_rect.width() + K_PAD_X as f32) * row_count as f32 > G_SIZE as f32 {
                    canvas.restore();
                    canvas.translate((0.0, dst_rect.height() + 1.0 * K_PAD_Y as f32));
                    canvas.save();
                    row_count = 0;
                }
                h *= 4;
            }
            w *= 4;
        }
        {
            let mut mask_paint = Paint::default();
            let bm = make_chessbm(5, 5);
            let img = bm.as_image().expect("an image");
            let src_rect = IRect::from_xywh(1, 1, 3, 3);
            mask_paint.set_mask_filter(
                MaskFilter::blur(
                    BlurStyle::Normal,
                    skia_rust_core::blur_mask::BlurMask::convert_radius_to_sigma(5.0),
                    None,
                )
                .expect("a mask filter"),
            );
            (self.proc)(
                canvas,
                &img,
                &bm,
                src_rect,
                dst_rect,
                SamplingOptions::from(FilterMode::Linear),
                &mask_paint,
            );
        }
        DrawResult::Ok
    }
}

// Port of: gm/drawbitmaprect.cpp#L182-L185 (chrome/m156)
crate::def_gm!(
    DrawBitmapRectGM_bitmapproc = "DrawBitmapRectGM(bitmapproc , nullptr)",
    DrawBitmapRectGm::new(bitmapproc, None)
);
crate::def_gm!(
    DrawBitmapRectGM_bitmapsubsetproc = "DrawBitmapRectGM(bitmapsubsetproc, \"-subset\")",
    DrawBitmapRectGm::new(bitmapsubsetproc, Some("-subset"))
);
crate::def_gm!(
    DrawBitmapRectGM_imageproc = "DrawBitmapRectGM(imageproc , \"-imagerect\")",
    DrawBitmapRectGm::new(imageproc, Some("-imagerect"))
);
crate::def_gm!(
    DrawBitmapRectGM_imagesubsetproc = "DrawBitmapRectGM(imagesubsetproc , \"-imagerect-subset\")",
    DrawBitmapRectGm::new(imagesubsetproc, Some("-imagerect-subset"))
);
