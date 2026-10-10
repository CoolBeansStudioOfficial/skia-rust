// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFShader.{h,cpp} (chrome/m156)

//! `SkPDFShader`: a shader as a PDF pattern. A gradient is a shading (see
//! [`gradient_shader`](crate::gradient_shader)); an image shader is a tiling pattern whose cell is
//! drawn with a PDF device; anything else is rasterized to a bitmap that is used as an image
//! shader.

use std::hash::{Hash, Hasher};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::{scalar_ceil_to_int, scalar_sqrt};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::shader_base::GradientType;
use skia_rust_core::size::ISize;
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surfaces::raster_n32_premul;

use crate::document::DocHandle;
use crate::device::PdfDevice;
use crate::gradient_shader::make_gradient_shader;
use crate::keyed_image::{BitmapKey, bitmap_key_from_image};
use crate::types::{PdfDict, PdfIndirectReference};
use crate::utils::{inverse_transform_bbox, populate_tiling_pattern_dict, to_bitmap};
use skia_rust_core::canvas::Canvas;
use skia_rust_core::sampling_options::SamplingOptions;

/// `SkPDFImageShaderKey`.
// Port of: src/pdf/SkPDFShader.h#L47-L66 (chrome/m156)
#[doc(alias = "SkPDFImageShaderKey")]
#[derive(Debug, Clone)]
pub struct ImageShaderKey {
    /// `fTransform`.
    pub transform: Matrix,
    /// `fBBox`.
    pub bbox: IRect,
    /// `fBitmapKey`.
    pub bitmap_key: BitmapKey,
    /// `fImageTileModes`.
    pub image_tile_modes: [TileMode; 2],
    /// `fPaintColor`.
    pub paint_color: Color4f,
}

impl ImageShaderKey {
    fn signature(&self) -> [u32; 24] {
        let mut s = [0u32; 24];
        for (i, v) in s.iter_mut().enumerate().take(9) {
            *v = self.transform.get(i).to_bits();
        }
        s[9] = self.bbox.left.cast_unsigned();
        s[10] = self.bbox.top.cast_unsigned();
        s[11] = self.bbox.right.cast_unsigned();
        s[12] = self.bbox.bottom.cast_unsigned();
        s[13] = self.bitmap_key.subset.left.cast_unsigned();
        s[14] = self.bitmap_key.subset.top.cast_unsigned();
        s[15] = self.bitmap_key.subset.right.cast_unsigned();
        s[16] = self.bitmap_key.subset.bottom.cast_unsigned();
        s[17] = self.bitmap_key.id;
        s[18] = self.image_tile_modes[0] as u32;
        s[19] = self.image_tile_modes[1] as u32;
        s[20] = self.paint_color.r.to_bits();
        s[21] = self.paint_color.g.to_bits();
        s[22] = self.paint_color.b.to_bits();
        s[23] = self.paint_color.a.to_bits();
        s
    }
}

impl PartialEq for ImageShaderKey {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(self.bitmap_key.id != 0);
        debug_assert!(other.bitmap_key.id != 0);
        self.signature() == other.signature()
    }
}

impl Eq for ImageShaderKey {}

impl Hash for ImageShaderKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.signature().hash(state);
    }
}

// Port of: src/pdf/SkPDFShader.cpp#L28-L31 (draw, chrome/m156)
fn draw(canvas: &Canvas, image: &Image, paint_color: &Color4f) {
    let paint = Paint::new(paint_color, None);
    canvas.draw_image_with_sampling_options(image, (0.0, 0.0), SamplingOptions::default(), Some(&paint));
}

// Port of: src/pdf/SkPDFShader.cpp#L33-L40 (to_bitmap, chrome/m156)
fn image_to_bitmap(image: &Image) -> Bitmap {
    if let Some(bitmap) = to_bitmap(image) {
        return bitmap;
    }
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((image.width(), image.height()), None);
    bitmap.erase_color(0x0000_0000u32);
    bitmap
}

// Port of: src/pdf/SkPDFShader.cpp#L42-L48 (draw_matrix, chrome/m156)
fn draw_matrix(canvas: &Canvas, image: &Image, matrix: &Matrix, paint_color: &Color4f) {
    canvas.save();
    canvas.concat(matrix);
    draw(canvas, image, paint_color);
    canvas.restore();
}

// Port of: src/pdf/SkPDFShader.cpp#L50-L57 (draw_bitmap_matrix, chrome/m156)
fn draw_bitmap_matrix(canvas: &Canvas, bm: &Bitmap, matrix: &Matrix, paint_color: &Color4f) {
    canvas.save();
    canvas.concat(matrix);
    let paint = Paint::new(paint_color, None);
    if let Some(image) = bm.as_image() {
        canvas.draw_image_with_sampling_options(
            &image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
    canvas.restore();
}

// Port of: src/pdf/SkPDFShader.cpp#L59-L68 (fill_color_from_bitmap, chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++
fn fill_color_from_bitmap(
    canvas: &Canvas,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    bitmap: &Bitmap,
    x: i32,
    y: i32,
    alpha: f32,
) {
    let rect = Rect::new(left, top, right, bottom);
    if !rect.is_empty() {
        let color = Color4f::from_color(bitmap.get_color((x, y)));
        let paint = Paint::new(Color4f::new(color.r, color.g, color.b, alpha * color.a), None);
        canvas.draw_rect(rect, &paint);
    }
}

// Port of: src/pdf/SkPDFShader.cpp#L70-L74 (scale_translate, chrome/m156)
fn scale_translate(sx: f32, sy: f32, tx: f32, ty: f32) -> Matrix {
    let mut m = Matrix::new_identity();
    m.set_scale_translate((sx, sy), (tx, ty));
    m
}

// Port of: src/pdf/SkPDFShader.cpp#L76 (is_tiled, chrome/m156)
fn is_tiled(m: TileMode) -> bool {
    TileMode::Mirror == m || TileMode::Repeat == m
}

// Port of: src/pdf/SkPDFShader.cpp#L78-L260 (make_image_shader, chrome/m156)
#[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one function in Skia
fn make_image_shader(
    doc: &DocHandle,
    mut final_matrix: Matrix,
    tile_modes_x: TileMode,
    tile_modes_y: TileMode,
    b_box: Rect,
    image: &Image,
    paint_color: &Color4f,
) -> PdfIndirectReference {
    // The image shader pattern cell will be drawn into a separate device
    // in pattern cell space (no scaling on the bitmap, though there may be
    // translations so that all content is in the device, coordinates > 0).

    // Map clip bounds to shader space to ensure the device is large enough
    // to handle fake clamping.

    let mut device_bounds = b_box;
    if !inverse_transform_bbox(&final_matrix, &mut device_bounds) {
        return PdfIndirectReference::default();
    }

    let bitmap_bounds = Rect::from_wh(image.width() as f32, image.height() as f32);

    // For tiling modes, the bounds should be extended to include the bitmap,
    // otherwise the bitmap gets clipped out and the shader is empty and awful.
    // For clamp modes, we're only interested in the clip region, whether
    // or not the main bitmap is in it.
    if is_tiled(tile_modes_x) || is_tiled(tile_modes_y) {
        device_bounds.join(bitmap_bounds);
    }

    let pattern_device_size = ISize::new(
        scalar_ceil_to_int(device_bounds.width()),
        scalar_ceil_to_int(device_bounds.height()),
    );
    let pattern_device = PdfDevice::new(pattern_device_size, doc, &Matrix::new_identity());
    let pattern_content = pattern_device.content_handle();
    let canvas = Canvas::from_device(Box::new(pattern_device));

    let mut pattern_b_box = Rect::from_wh(image.width() as f32, image.height() as f32);
    let width = pattern_b_box.width();
    let height = pattern_b_box.height();

    // Translate the canvas so that the bitmap origin is at (0, 0).
    canvas.translate((-device_bounds.left, -device_bounds.top));
    pattern_b_box.offset((-device_bounds.left, -device_bounds.top));
    // Undo the translation in the final matrix
    final_matrix.pre_translate((device_bounds.left, device_bounds.top));

    // If the bitmap is out of bounds (i.e. clamp mode where we only see the
    // stretched sides), canvas will clip this out and the extraneous data
    // won't be saved to the PDF.
    draw(&canvas, image, paint_color);

    // Tiling is implied.  First we handle mirroring.
    if tile_modes_x == TileMode::Mirror {
        draw_matrix(
            &canvas,
            image,
            &scale_translate(-1.0, 1.0, 2.0 * width, 0.0),
            paint_color,
        );
        pattern_b_box.right += width;
    }
    if tile_modes_y == TileMode::Mirror {
        draw_matrix(
            &canvas,
            image,
            &scale_translate(1.0, -1.0, 0.0, 2.0 * height),
            paint_color,
        );
        pattern_b_box.bottom += height;
    }
    if tile_modes_x == TileMode::Mirror && tile_modes_y == TileMode::Mirror {
        draw_matrix(
            &canvas,
            image,
            &scale_translate(-1.0, -1.0, 2.0 * width, 2.0 * height),
            paint_color,
        );
    }

    // Then handle Clamping, which requires expanding the pattern canvas to
    // cover the entire surfaceBBox.

    let mut bitmap = Bitmap::new();
    if tile_modes_x == TileMode::Clamp || tile_modes_y == TileMode::Clamp {
        // For now, the easiest way to access the colors in the corners and sides is
        // to just make a bitmap from the image.
        bitmap = image_to_bitmap(image);
    }

    // If both x and y are in clamp mode, we start by filling in the corners.
    // (Which are just a rectangles of the corner colors.)
    if tile_modes_x == TileMode::Clamp && tile_modes_y == TileMode::Clamp {
        debug_assert!(!bitmap.draws_nothing());

        fill_color_from_bitmap(
            &canvas,
            device_bounds.left,
            device_bounds.top,
            0.0,
            0.0,
            &bitmap,
            0,
            0,
            paint_color.a,
        );

        fill_color_from_bitmap(
            &canvas,
            width,
            device_bounds.top,
            device_bounds.right,
            0.0,
            &bitmap,
            bitmap.width() - 1,
            0,
            paint_color.a,
        );

        fill_color_from_bitmap(
            &canvas,
            width,
            height,
            device_bounds.right,
            device_bounds.bottom,
            &bitmap,
            bitmap.width() - 1,
            bitmap.height() - 1,
            paint_color.a,
        );

        fill_color_from_bitmap(
            &canvas,
            device_bounds.left,
            height,
            0.0,
            device_bounds.bottom,
            &bitmap,
            0,
            bitmap.height() - 1,
            paint_color.a,
        );
    }

    // Then expand the left, right, top, then bottom.
    if tile_modes_x == TileMode::Clamp {
        debug_assert!(!bitmap.draws_nothing());
        let mut subset = IRect::from_xywh(0, 0, 1, bitmap.height());
        if device_bounds.left < 0.0 {
            let mut left = Bitmap::new();
            let extracted = bitmap.extract_subset(&mut left, subset);
            debug_assert!(extracted);

            let mut left_matrix = scale_translate(-device_bounds.left, 1.0, device_bounds.left, 0.0);
            draw_bitmap_matrix(&canvas, &left, &left_matrix, paint_color);

            if tile_modes_y == TileMode::Mirror {
                left_matrix.post_scale((1.0, -1.0), None);
                left_matrix.post_translate((0.0, 2.0 * height));
                draw_bitmap_matrix(&canvas, &left, &left_matrix, paint_color);
            }
            pattern_b_box.left = 0.0;
        }

        if device_bounds.right > width {
            let mut right = Bitmap::new();
            subset.offset((bitmap.width() - 1, 0));
            let extracted = bitmap.extract_subset(&mut right, subset);
            debug_assert!(extracted);

            let mut right_matrix = scale_translate(device_bounds.right - width, 1.0, width, 0.0);
            draw_bitmap_matrix(&canvas, &right, &right_matrix, paint_color);

            if tile_modes_y == TileMode::Mirror {
                right_matrix.post_scale((1.0, -1.0), None);
                right_matrix.post_translate((0.0, 2.0 * height));
                draw_bitmap_matrix(&canvas, &right, &right_matrix, paint_color);
            }
            pattern_b_box.right = device_bounds.width();
        }
    }
    if tile_modes_x == TileMode::Decal {
        if device_bounds.left < 0.0 {
            pattern_b_box.left = 0.0;
        }
        if device_bounds.right > width {
            pattern_b_box.right = device_bounds.width();
        }
    }

    if tile_modes_y == TileMode::Clamp {
        debug_assert!(!bitmap.draws_nothing());
        let mut subset = IRect::from_xywh(0, 0, bitmap.width(), 1);
        if device_bounds.top < 0.0 {
            let mut top = Bitmap::new();
            let extracted = bitmap.extract_subset(&mut top, subset);
            debug_assert!(extracted);

            let mut top_matrix = scale_translate(1.0, -device_bounds.top, 0.0, device_bounds.top);
            draw_bitmap_matrix(&canvas, &top, &top_matrix, paint_color);

            if tile_modes_x == TileMode::Mirror {
                top_matrix.post_scale((-1.0, 1.0), None);
                top_matrix.post_translate((2.0 * width, 0.0));
                draw_bitmap_matrix(&canvas, &top, &top_matrix, paint_color);
            }
            pattern_b_box.top = 0.0;
        }

        if device_bounds.bottom > height {
            let mut bottom = Bitmap::new();
            subset.offset((0, bitmap.height() - 1));
            let extracted = bitmap.extract_subset(&mut bottom, subset);
            debug_assert!(extracted);

            let mut bottom_matrix = scale_translate(1.0, device_bounds.bottom - height, 0.0, height);
            draw_bitmap_matrix(&canvas, &bottom, &bottom_matrix, paint_color);

            if tile_modes_x == TileMode::Mirror {
                bottom_matrix.post_scale((-1.0, 1.0), None);
                bottom_matrix.post_translate((2.0 * width, 0.0));
                draw_bitmap_matrix(&canvas, &bottom, &bottom_matrix, paint_color);
            }
            pattern_b_box.bottom = device_bounds.height();
        }
    }
    if tile_modes_y == TileMode::Decal {
        if device_bounds.top < 0.0 {
            pattern_b_box.top = 0.0;
        }
        if device_bounds.bottom > height {
            pattern_b_box.bottom = device_bounds.height();
        }
    }

    let (image_shader, resource_dict) = {
        let mut content = pattern_content.borrow_mut();
        let image_shader = content.content();
        let resource_dict = content.make_resource_dict();
        (image_shader, resource_dict)
    };
    let mut dict = PdfDict::new(None);
    populate_tiling_pattern_dict(
        &mut dict,
        &pattern_b_box,
        is_tiled(tile_modes_x),
        is_tiled(tile_modes_y),
        resource_dict,
        &final_matrix,
    );
    doc.stream_out(Some(dict), &image_shader, true)
}

/// Generic fallback for unsupported shaders:
///  * allocate a surfaceBBox-sized bitmap
///  * shade the whole area
///  * use the result as a bitmap shader
// Port of: src/pdf/SkPDFShader.cpp#L262-L322 (make_fallback_shader, chrome/m156)
fn make_fallback_shader(
    doc: &DocHandle,
    shader: &Shader,
    canvas_transform: &Matrix,
    surface_b_box: &IRect,
    paint_color: &Color4f,
) -> PdfIndirectReference {
    // surfaceBBox is in device space. While that's exactly what we
    // want for sizing our bitmap, we need to map it into
    // shader space for adjustments (to match
    // MakeImageShader's behavior).
    let mut shader_rect = Rect::from_irect(surface_b_box);
    if !inverse_transform_bbox(canvas_transform, &mut shader_rect) {
        return PdfIndirectReference::default();
    }
    // Clamp the bitmap size to about 1M pixels
    const MAX_BITMAP_AREA: i32 = 1024 * 1024;
    let bitmap_area = surface_b_box.width() as f32 * surface_b_box.height() as f32;
    let mut raster_scale = 1.0f32;
    if bitmap_area > MAX_BITMAP_AREA as f32 {
        raster_scale *= scalar_sqrt(MAX_BITMAP_AREA as f32 / bitmap_area);
    }

    let size = ISize::new(
        t_pin(
            scalar_ceil_to_int(raster_scale * surface_b_box.width() as f32),
            1,
            MAX_BITMAP_AREA,
        ),
        t_pin(
            scalar_ceil_to_int(raster_scale * surface_b_box.height() as f32),
            1,
            MAX_BITMAP_AREA,
        ),
    );
    let scale = (
        size.width as f32 / shader_rect.width(),
        size.height as f32 / shader_rect.height(),
    );

    let Some(mut surface) = raster_n32_premul((size.width, size.height)) else {
        debug_assert!(false);
        return PdfIndirectReference::default();
    };
    {
        let canvas = surface.canvas();
        canvas.clear(skia_rust_core::color::Color4f::new(0.0, 0.0, 0.0, 0.0));

        let mut p = Paint::new(paint_color, None);
        p.set_shader(shader.clone());

        canvas.scale(scale);
        canvas.translate((-shader_rect.x(), -shader_rect.y()));
        canvas.draw_paint(&p);
    }

    let mut shader_transform = Matrix::translate((shader_rect.x(), shader_rect.y()));
    shader_transform.pre_scale((1.0 / scale.0, 1.0 / scale.1), None);

    let Some(image) = surface.image_snapshot() else {
        debug_assert!(false);
        return PdfIndirectReference::default();
    };
    make_image_shader(
        doc,
        Matrix::concat(canvas_transform, &shader_transform),
        TileMode::Clamp,
        TileMode::Clamp,
        Rect::from_irect(surface_b_box),
        &image,
        paint_color,
    )
}

// Port of: src/pdf/SkPDFShader.cpp#L324-L331 (adjust_color, chrome/m156)
fn adjust_color(shader: &Shader, paint_color: &Color4f) -> Color4f {
    if let Some((img, _, _)) = shader.is_a_image() {
        if img.is_alpha_only() {
            return *paint_color;
        }
    }
    Color4f::new(0.0, 0.0, 0.0, paint_color.a) // only preserve the alpha.
}

/// `SkPDFMakeShader`: makes a PDF shader for the passed shader. If the shader is invalid in some
/// way, returns none.
///
/// In PDF parlance, this is a pattern, used in place of a color when the pattern color space is
/// selected. May cache the shader in the document for later re-use. If this function is called
/// again with an equivalent shader, a new reference to the cached pdf shader may be returned.
///
/// `canvas_transform` is the current transform matrix (PDF shaders are absolutely positioned,
/// relative to where the page is drawn). `surface_b_box` is the bounding box of the drawing
/// surface (with matrix already applied). `paint_color` is the color and alpha of the paint;
/// color is usually ignored, unless it is an alpha shader.
// Port of: src/pdf/SkPDFShader.cpp#L333-L380 (chrome/m156)
#[doc(alias = "SkPDFMakeShader")]
#[must_use]
pub fn make_shader(
    doc: &DocHandle,
    shader: &Shader,
    canvas_transform: &Matrix,
    surface_b_box: &IRect,
    paint_color: &Color4f,
) -> PdfIndirectReference {
    if shader.as_base().as_gradient(None, None) != GradientType::None {
        let gradient_shader = make_gradient_shader(doc, shader, canvas_transform, surface_b_box);
        if gradient_shader.is_valid() {
            return gradient_shader;
        }
    }
    if surface_b_box.is_empty() {
        return PdfIndirectReference::default();
    }

    let paint_color = adjust_color(shader, paint_color);
    if let Some((skimg, shader_transform, (tile_x, tile_y))) = shader.is_a_image() {
        let final_matrix = Matrix::concat(canvas_transform, &shader_transform);
        let key = ImageShaderKey {
            transform: final_matrix.clone(),
            bbox: *surface_b_box,
            bitmap_key: bitmap_key_from_image(Some(&skimg)),
            image_tile_modes: [tile_x, tile_y],
            paint_color,
        };
        if let Some(reference) = doc.with(|d| d.image_shader_map.get(&key).copied()) {
            return reference;
        }
        let pdf_shader = make_image_shader(
            doc,
            final_matrix,
            tile_x,
            tile_y,
            Rect::from_irect(surface_b_box),
            &skimg,
            &paint_color,
        );
        doc.with(|d| d.image_shader_map.insert(key, pdf_shader));
        return pdf_shader;
    }
    // Don't bother to de-dup fallback shader.
    make_fallback_shader(doc, shader, canvas_transform, surface_b_box, &paint_color)
}
