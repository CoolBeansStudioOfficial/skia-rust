// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/composeshader.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{int_to_scalar, make_texture_image};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/composeshader.cpp#L9-L23 (chrome/m156), make_shader
fn make_shader(mode: BlendMode) -> Option<Shader> {
    let pts_a = [Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
    let colors_a = [Color4f::from(Color::RED), Color4f::from(Color::BLUE)];
    let shader_a = gradient_shaders::linear_gradient(
        (pts_a[0], pts_a[1]),
        &Gradient::new(
            Colors::new(&colors_a, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )?;
    let pts_b = [Point::new(0.0, 0.0), Point::new(0.0, 100.0)];
    let colors_b = [
        Color4f::new(0.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 0.0, 128.0 / 255.0),
    ];
    let shader_b = gradient_shaders::linear_gradient(
        (pts_b[0], pts_b[1]),
        &Gradient::new(
            Colors::new(&colors_b, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )?;
    Some(shaders::blend(mode, shader_a, shader_b))
}

// Port of: gm/composeshader.cpp#L25-L44 (chrome/m156), ComposeShaderGM
struct ComposeShaderGm {
    shader: Option<Shader>,
}

impl GM for ComposeShaderGm {
    fn name(&self) -> String {
        "composeshader".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(120, 120)
    }

    fn on_once_before_draw(&mut self) {
        self.shader = make_shader(BlendMode::DstIn);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &paint);
        paint.set_shader(self.shader.clone());
        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &paint);
    }
}

// Port of: gm/composeshader.cpp#L25-L44 (chrome/m156), DEF_GM( return new ComposeShaderGM; )
crate::def_gm!(ComposeShaderGM, ComposeShaderGm { shader: None });

// Port of: gm/composeshader.cpp#L46-L72 (chrome/m156), ComposeShaderAlphaGM
struct ComposeShaderAlphaGm;

impl GM for ComposeShaderAlphaGm {
    fn name(&self) -> String {
        "composeshader_alpha".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(750, 220)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let shaders_list = [
            make_shader(BlendMode::DstIn),
            make_shader(BlendMode::SrcOver),
        ];
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        let r = Rect::new(5.0, 5.0, 105.0, 105.0);
        for shader in &shaders_list {
            canvas.save();
            let mut alpha: i32 = 0xFF;
            while alpha > 0 {
                paint.set_alpha_f(1.0);
                paint.set_shader(None);
                canvas.draw_rect(r, &paint);
                paint.set_alpha(u8::try_from(alpha).expect("an 8-bit alpha"));
                paint.set_shader(shader.clone());
                canvas.draw_rect(r, &paint);
                canvas.translate((r.width() + 5.0, 0.0));
                alpha -= 0x28;
            }
            canvas.restore();
            canvas.translate((0.0, r.height() + 5.0));
        }
    }
}

// Port of: gm/composeshader.cpp#L74 (chrome/m156), DEF_GM( return new ComposeShaderAlphaGM; )
crate::def_gm!(ComposeShaderAlphaGM, ComposeShaderAlphaGm);

// This determines the length and width of the bitmaps used in the ComposeShaders.
// Port of: gm/composeshader.cpp#L78-L130 (chrome/m156), squareLength
const SQUARE_LENGTH: i32 = 20;

// Port of: gm/composeshader.cpp#L78-L90 (chrome/m156), draw_color_bm
fn draw_color_bm(bm: &mut Bitmap, length: i32) {
    let mut paint = Paint::default();
    paint.set_color(Color::GREEN);
    bm.alloc_n32_pixels((length, length), None);
    bm.erase_color(Color::RED);
    let canvas = CoreCanvas::from_bitmap(bm, None).expect("a canvas on the bitmap");
    canvas.draw_circle(
        (int_to_scalar(length / 2), int_to_scalar(length / 2)),
        int_to_scalar(length / 2),
        &paint,
    );
}

// Port of: gm/composeshader.cpp#L92-L103 (chrome/m156), draw_alpha8_bm
fn draw_alpha8_bm(bm: &mut Bitmap, length: i32) {
    let mut circle_paint = Paint::default();
    circle_paint.set_color(Color::BLACK);
    bm.alloc_pixels_info(&ImageInfo::new_a8((length, length)), None);
    bm.erase_color(Color::TRANSPARENT);
    let canvas = CoreCanvas::from_bitmap(bm, None).expect("a canvas on the bitmap");
    canvas.draw_circle(
        (int_to_scalar(length / 2), int_to_scalar(length / 2)),
        int_to_scalar(length / 4),
        &circle_paint,
    );
}

// Port of: gm/composeshader.cpp#L105-L112 (chrome/m156), make_linear_gradient_shader
fn make_linear_gradient_shader(length: i32) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(int_to_scalar(length), 0.0)];
    let colors = [Color4f::from(Color::BLUE), Color4f::new(0.0, 0.0, 1.0, 0.0)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/composeshader.cpp#L114-L180 (chrome/m156), ComposeShaderBitmapGM
struct ComposeShaderBitmapGm {
    use_local_matrix: bool,
    initialized: bool,
    color_bitmap: Bitmap,
    alpha8_bitmap: Bitmap,
    color_bitmap_shader: Option<Shader>,
    alpha8_bitmap_shader: Option<Shader>,
    linear_gradient_shader: Option<Shader>,
}

impl ComposeShaderBitmapGm {
    fn new(use_local_matrix: bool) -> Self {
        ComposeShaderBitmapGm {
            use_local_matrix,
            initialized: false,
            color_bitmap: Bitmap::new(),
            alpha8_bitmap: Bitmap::new(),
            color_bitmap_shader: None,
            alpha8_bitmap_shader: None,
            linear_gradient_shader: None,
        }
    }
}

impl GM for ComposeShaderBitmapGm {
    fn name(&self) -> String {
        if self.use_local_matrix {
            "composeshader_bitmap_lm".to_string()
        } else {
            "composeshader_bitmap".to_string()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(7 * (SQUARE_LENGTH + 5), 2 * (SQUARE_LENGTH + 5))
    }

    // Port of: gm/composeshader.cpp#L127-L177 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        if !self.initialized {
            draw_color_bm(&mut self.color_bitmap, SQUARE_LENGTH);
            let img = skia_rust_core::images::raster_from_bitmap(&self.color_bitmap);
            if let Some(img) = make_texture_image(canvas, img) {
                self.color_bitmap_shader = img.to_shader(
                    (TileMode::Repeat, TileMode::Repeat),
                    SamplingOptions::default(),
                    &Matrix::new_identity(),
                );
            }

            draw_alpha8_bm(&mut self.alpha8_bitmap, SQUARE_LENGTH);
            let img = skia_rust_core::images::raster_from_bitmap(&self.alpha8_bitmap);
            if make_texture_image(canvas, img).is_some() {
                self.alpha8_bitmap_shader = self.alpha8_bitmap.to_shader(
                    (TileMode::Repeat, TileMode::Repeat),
                    SamplingOptions::default(),
                    &Matrix::new_identity(),
                );
            }
            self.linear_gradient_shader = make_linear_gradient_shader(SQUARE_LENGTH);
            self.initialized = true;
        }

        let mode = BlendMode::DstOver;
        let lm = Matrix::translate((0.0, int_to_scalar(SQUARE_LENGTH) * 0.5));
        let blend_or_none = |dst: &Option<Shader>, src: &Option<Shader>| -> Option<Shader> {
            match (dst, src) {
                (Some(dst), Some(src)) => Some(shaders::blend(mode, dst.clone(), src.clone())),
                _ => None,
            }
        };
        // gradient should appear over color bitmap
        // gradient should appear over alpha8 bitmap colorized by the paint color
        let mut shaders_list = [
            blend_or_none(&self.linear_gradient_shader, &self.color_bitmap_shader),
            blend_or_none(&self.linear_gradient_shader, &self.alpha8_bitmap_shader),
        ];
        if self.use_local_matrix {
            for shader in &mut shaders_list {
                *shader = shader.as_ref().map(|s| s.with_local_matrix(&lm));
            }
        }

        let mut paint = Paint::default();
        paint.set_color(Color::YELLOW);
        let r = Rect::new(
            0.0,
            0.0,
            int_to_scalar(SQUARE_LENGTH),
            int_to_scalar(SQUARE_LENGTH),
        );
        for shader in &shaders_list {
            canvas.save();
            let mut alpha: i32 = 0xFF;
            while alpha > 0 {
                paint.set_alpha(u8::try_from(alpha).expect("an 8-bit alpha"));
                paint.set_shader(shader.clone());
                canvas.draw_rect(r, &paint);
                canvas.translate((r.width() + 5.0, 0.0));
                alpha -= 0x28;
            }
            canvas.restore();
            canvas.translate((0.0, r.height() + 5.0));
        }
    }
}

// Port of: gm/composeshader.cpp#L181-L182 (chrome/m156), DEF_GM( return new ComposeShaderBitmapGM(false); )
crate::def_gm!(
    ComposeShaderBitmapGM_false = "ComposeShaderBitmapGM(false)",
    ComposeShaderBitmapGm::new(false)
);
// Port of: gm/composeshader.cpp#L183 (chrome/m156), DEF_GM( return new ComposeShaderBitmapGM(true); )
crate::def_gm!(
    ComposeShaderBitmapGM_true = "ComposeShaderBitmapGM(true)",
    ComposeShaderBitmapGm::new(true)
);

// The pixel storage of `composeshader_bitmap2`: the 8-bit mask `(y + x) / 2` and the packed
// colours `SkPackARGB32(0xFF, x, y, 0)`, with the host's N32 shifts as in the C++.
// Port of: gm/composeshader.cpp#L188-L196 (chrome/m156)
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_midpoint
)] // mirrors the C++ integer casts and (y + x) / 2
fn bitmap2_storage(width: i32, height: i32) -> (Vec<u8>, Vec<u32>) {
    let count = (width * height) as usize;
    let mut dst8_storage = vec![0u8; count];
    let mut dst32_storage = vec![0u32; count];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            dst8_storage[i] = ((y + x) / 2) as u8;
            dst32_storage[i] = pack_argb32(0xFF, x as u32, y as u32, 0);
        }
    }
    (dst8_storage, dst32_storage)
}

// Port of: gm/composeshader.cpp#L184-L213 (chrome/m156), composeshader_bitmap2
crate::def_simple_gm!(composeshader_bitmap2, canvas, 200, 200, {
    let width: i32 = 255;
    let height: i32 = 255;
    let (dst8_storage, dst32_storage) = bitmap2_storage(width, height);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLUE);
    let r = Rect::new(0.0, 0.0, int_to_scalar(width), int_to_scalar(height));
    canvas.draw_rect(r, &paint);

    let src_pixels: Vec<u8> = dst32_storage.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let mut sk_bitmap = Bitmap::new();
    assert!(sk_bitmap.install_pixels(
        &ImageInfo::new_n32_premul((width, height), None),
        Some(src_pixels),
        usize::try_from(width * 4).expect("a row"),
    ));
    let mut sk_mask = Bitmap::new();
    assert!(sk_mask.install_pixels(
        &ImageInfo::new_a8((width, height)),
        Some(dst8_storage),
        usize::try_from(width).expect("a width"),
    ));
    let sk_src: Image = sk_bitmap.as_image().expect("an image");
    let sk_mask_image: Image = sk_mask.as_image().expect("an image");
    paint.set_shader(shaders::blend(
        BlendMode::SrcIn,
        sk_mask_image
            .to_shader(None, SamplingOptions::default(), None)
            .expect("a shader"),
        sk_src
            .to_shader(None, SamplingOptions::default(), None)
            .expect("a shader"),
    ));
    canvas.draw_rect(r, &paint);
});

// Port of: gm/composeshader.cpp#L215-L220 (chrome/m156), make_src_shader
fn make_src_shader(size: f32) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(0.0, size)];
    let colors = [
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 0.0),
    ];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/composeshader.cpp#L222-L227 (chrome/m156), make_dst_shader
fn make_dst_shader(size: f32) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(size, 0.0)];
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(1.0, 0.0, 0.0, 0.0),
    ];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/composeshader.cpp#L229 (chrome/m156), gCellSize
const G_CELL_SIZE: f32 = 100.0;

// Port of: gm/composeshader.cpp#L231-L246 (chrome/m156), draw_cell
fn draw_cell(
    canvas: &Canvas,
    src: Option<Shader>,
    dst: Option<Shader>,
    mode: BlendMode,
    alpha: u8,
) {
    let r = Rect::new(0.0, 0.0, G_CELL_SIZE, G_CELL_SIZE);
    let mut p = Paint::default();
    p.set_alpha(alpha);
    let _acr = AutoCanvasRestore::guard(canvas, false);
    canvas.save_layer(&SaveLayerRec::default().bounds(&r).paint(&p));
    p.set_alpha(0xFF);
    p.set_shader(dst);
    p.set_blend_mode(BlendMode::Src);
    canvas.draw_rect(r, &p);
    p.set_shader(src);
    p.set_blend_mode(mode);
    canvas.draw_rect(r, &p);
}

// Port of: gm/composeshader.cpp#L248-L256 (chrome/m156), draw_composed
fn draw_composed(
    canvas: &Canvas,
    src: Option<Shader>,
    dst: Option<Shader>,
    mode: BlendMode,
    alpha: u8,
) {
    let mut p = Paint::default();
    p.set_alpha(alpha);
    let composed = match (dst, src) {
        (Some(dst), Some(src)) => Some(shaders::blend(mode, dst, src)),
        _ => None,
    };
    p.set_shader(composed);
    canvas.draw_rect(Rect::new(0.0, 0.0, G_CELL_SIZE, G_CELL_SIZE), &p);
}

// Port of: gm/composeshader.cpp#L258-L278 (chrome/m156), draw_pair
fn draw_pair(canvas: &Canvas, src: Option<&Shader>, dst: Option<&Shader>, mode: BlendMode) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    let gap: f32 = 4.0;
    let mut r = Rect::new(0.0, 0.0, 2.0 * G_CELL_SIZE + gap, 2.0 * G_CELL_SIZE + gap);
    r.outset((gap + 1.5, gap + 1.5));
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    canvas.draw_rect(r, &p); // border
    let mut alpha: u8 = 0xFF;
    for _ in 0..2 {
        draw_cell(canvas, src.cloned(), dst.cloned(), mode, alpha);
        canvas.save();
        canvas.translate((G_CELL_SIZE + gap, 0.0));
        draw_composed(canvas, src.cloned(), dst.cloned(), mode, alpha);
        canvas.restore();
        canvas.translate((0.0, G_CELL_SIZE + gap));
        alpha = 0x80;
    }
}

// Port of: gm/composeshader.cpp#L280-L300 (chrome/m156), composeshader_grid
crate::def_simple_gm!(composeshader_grid, canvas, 882, 882, {
    let src = make_src_shader(G_CELL_SIZE);
    let dst = make_dst_shader(G_CELL_SIZE);
    let margin: f32 = 15.0;
    let dx = 2.0 * G_CELL_SIZE + margin;
    let dy = 2.0 * G_CELL_SIZE + margin;
    canvas.translate((margin, margin));
    canvas.save();
    let modes = [
        BlendMode::Clear,
        BlendMode::Src,
        BlendMode::Dst,
        BlendMode::SrcOver,
        BlendMode::DstOver,
        BlendMode::SrcIn,
        BlendMode::DstIn,
        BlendMode::SrcOut,
        BlendMode::DstOut,
        BlendMode::SrcATop,
        BlendMode::DstATop,
        BlendMode::Xor,
        BlendMode::Plus,
        BlendMode::Modulate,
        BlendMode::Screen,
        BlendMode::Overlay,
    ];
    for (m, mode) in modes.into_iter().enumerate() {
        draw_pair(canvas, src.as_ref(), dst.as_ref(), mode);
        if (m % 4) == 3 {
            canvas.restore();
            canvas.translate((0.0, dy));
            canvas.save();
        } else {
            canvas.translate((dx, 0.0));
        }
    }
    canvas.restore();
});
