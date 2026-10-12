// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bc1_transparency.cpp (chrome/m156)

// BC1 has per-block transparency. This GM draws a 16x8 BC1 texture twice: as kBC1_RGB8_UNORM and
// as kBC1_RGBA8_UNORM. On a raster canvas the images are decoded on the CPU by
// `SkImages::RasterFromCompressedTextureData`; the Ganesh upload branch of `data_to_img` and the
// `isTextureBacked` check in `draw_image` do not run here.

use crate::prelude::*;
use skia_rust_core::compressed_data_utils::compressed_data_size;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::images::raster_from_compressed_texture_data;
use skia_rust_core::math::mul_div_255_round;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

// Port of: gm/bc1_transparency.cpp#L24-L25 (chrome/m156), `kImgWidth` and `kImgHeight`
const K_IMG_WIDTH: i32 = 16;
const K_IMG_HEIGHT: i32 = 8;
// Port of: gm/bc1_transparency.cpp#L26 (chrome/m156), `kPad`
const K_PAD: i32 = 4;

// Port of: gm/bc1_transparency.cpp#L28-L32 (chrome/m156), `struct BC1Block`
#[derive(Clone, Copy)]
struct BC1Block {
    color0: u16,
    color1: u16,
    indices: u32,
}

impl BC1Block {
    // Little-endian, the layout of the struct on every target Skia's data is read on.
    fn to_le_bytes(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..2].copy_from_slice(&self.color0.to_le_bytes());
        out[2..4].copy_from_slice(&self.color1.to_le_bytes());
        out[4..8].copy_from_slice(&self.indices.to_le_bytes());
        out
    }
}

// Port of: gm/bc1_transparency.cpp#L34-L37 (chrome/m156), `num_4x4_blocks`
fn num_4x4_blocks(size: i32) -> i32 {
    ((size + 3) & !3) >> 2
}

// Port of: gm/bc1_transparency.cpp#L39-L46 (chrome/m156), `to565`
fn to565(col: Color) -> u16 {
    let r5 = mul_div_255_round(31, u32::from(col.r()));
    let g6 = mul_div_255_round(63, u32::from(col.g()));
    let b5 = mul_div_255_round(31, u32::from(col.b()));

    // Each channel is at most 31 or 63, so the packed value fits in 16 bits.
    u16::try_from((r5 << 11) | (g6 << 5) | b5).expect("a 16-bit 565 value")
}

// Port of: gm/bc1_transparency.cpp#L61-L80 (chrome/m156), `create_BC1_block`
#[allow(clippy::identity_op)] // the shifts spell out the 2-bit index of each pixel, as in C++
fn create_bc1_block(transparent: bool) -> BC1Block {
    let (color0, color1, byte) = if transparent {
        // SkASSERT(fColor0 <= fColor1): this signals a transparent block.
        // opaque black (col0), medium grey (col2), transparent black (col3), white (col1).
        let byte: u32 = (0x0 << 0) | (0x2 << 2) | (0x3 << 4) | (0x1 << 6);
        (to565(Color::BLACK), to565(Color::WHITE), byte)
    } else {
        // SkASSERT(fColor0 > fColor1): this signals an opaque block.
        // opaque black (col1), dark grey (col3), light grey (col2), white (col0)
        let byte: u32 = (0x1 << 0) | (0x3 << 2) | (0x2 << 4) | (0x0 << 6);
        (to565(Color::WHITE), to565(Color::BLACK), byte)
    };

    BC1Block {
        color0,
        color1,
        indices: (byte << 24) | (byte << 16) | (byte << 8) | byte,
    }
}

// This makes a 16x8 BC1 texture which has the top 4 rows be officially transparent
// and the bottom 4 rows be officially opaque.
// Port of: gm/bc1_transparency.cpp#L83-L106 (chrome/m156), `make_compressed_data`
#[allow(clippy::similar_names, clippy::cast_sign_loss)] // the names are the C++ ones; indices >= 0
fn make_compressed_data() -> Data {
    let dim = ISize::new(K_IMG_WIDTH, K_IMG_HEIGHT);

    let total_size = compressed_data_size(TextureCompressionType::BC1_RGB8_UNORM, dim, None, false);

    let mut tmp = vec![0u8; total_size];

    let trans_block = create_bc1_block(true);
    let opaque_block = create_bc1_block(false);

    let num_x_blocks = num_4x4_blocks(K_IMG_WIDTH);
    let num_y_blocks = num_4x4_blocks(K_IMG_HEIGHT);

    for y in 0..num_y_blocks {
        for x in 0..num_x_blocks {
            let block = if y < num_y_blocks / 2 {
                trans_block
            } else {
                opaque_block
            };
            let at = (y * num_x_blocks + x) as usize * 8;
            tmp[at..at + 8].copy_from_slice(&block.to_le_bytes());
        }
    }

    Data::new_from_vec(tmp)
}

// Port of: gm/bc1_transparency.cpp#L108-L118 (chrome/m156), `data_to_img` (the raster branch)
fn data_to_img(data: Data, compression: TextureCompressionType) -> Option<Image> {
    raster_from_compressed_texture_data(data, K_IMG_WIDTH, K_IMG_HEIGHT, compression)
}

// Port of: gm/bc1_transparency.cpp#L120-L143 (chrome/m156), `draw_image` (the raster path: the
// images are never texture backed, so the red outline is always drawn)
#[allow(clippy::cast_precision_loss)] // image offsets and sizes are small integers, exact in f32
fn draw_image(canvas: &Canvas, image: Option<&Image>, x: i32, y: i32) {
    let Some(image) = image else {
        return;
    };

    let is_compressed = image.is_texture_backed();

    canvas.draw_image(image, Point::new(x as f32, y as f32), None);

    if !is_compressed {
        let mut r = Rect::from_xywh(x as f32, y as f32, K_IMG_WIDTH as f32, K_IMG_HEIGHT as f32);
        r.outset(Point::new(1.0, 1.0));

        let mut red_stroke = Paint::default();
        red_stroke.set_color(Color::RED);
        red_stroke.set_style(Style::Stroke);
        red_stroke.set_stroke_width(2.0);

        canvas.draw_rect(r, &red_stroke);
    }
}

// Port of: gm/bc1_transparency.cpp#L149-L223 (chrome/m156), class BC1TransparencyGM
#[derive(Debug, Default)]
pub struct BC1TransparencyGm {
    rgb_image: Option<Image>,
    rgba_image: Option<Image>,
}

impl GM for BC1TransparencyGm {
    // Port of: gm/bc1_transparency.cpp#L172 (chrome/m156), `getName`
    fn name(&self) -> String {
        "bc1_transparency".to_string()
    }

    // Port of: gm/bc1_transparency.cpp#L174-L176 (chrome/m156), `getISize`
    fn size(&mut self) -> ISize {
        ISize::new(K_IMG_WIDTH + 2 * K_PAD, 2 * K_IMG_HEIGHT + 3 * K_PAD)
    }

    // Port of: gm/bc1_transparency.cpp#L152-L154 (chrome/m156), `setBGColor(SK_ColorGREEN)`
    fn bg_color(&self) -> Color {
        Color::from(0xFF00_FF00)
    }

    // Port of: gm/bc1_transparency.cpp#L178-L202 (chrome/m156), `onGpuSetup`
    fn on_gpu_setup(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let bc1_data = make_compressed_data();

        self.rgb_image = data_to_img(bc1_data.clone(), TextureCompressionType::BC1_RGB8_UNORM);
        self.rgba_image = data_to_img(bc1_data, TextureCompressionType::BC1_RGBA8_UNORM);
        if self.rgb_image.is_none() || self.rgba_image.is_none() {
            "Failed to create BC1 images.".clone_into(error_msg);
            return DrawResult::Fail;
        }

        DrawResult::Ok
    }

    // Port of: gm/bc1_transparency.cpp#L208-L212 (chrome/m156), `onDraw`
    fn on_draw(&mut self, canvas: &Canvas) {
        draw_image(canvas, self.rgb_image.as_ref(), K_PAD, K_PAD);
        draw_image(
            canvas,
            self.rgba_image.as_ref(),
            K_PAD,
            2 * K_PAD + K_IMG_HEIGHT,
        );
    }
}

// Port of: gm/bc1_transparency.cpp#L222 (chrome/m156), DEF_GM
crate::def_gm!(BC1TransparencyGM, BC1TransparencyGm::default());
