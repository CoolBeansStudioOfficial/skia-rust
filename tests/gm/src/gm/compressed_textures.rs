// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/compressed_textures.cpp (chrome/m156)

// Basic test of ETC1 and BC1 compressed textures. The GPU branches of the C++ (the Ganesh and
// Graphite uploads in `make_compressed_image` and the `isTextureBacked` check in `drawCell`) do
// not run on a raster canvas, so the raster path is ported: the images are decoded on the CPU by
// `SkImages::RasterFromCompressedTextureData`.

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::compressed_data_utils::compressed_data_size;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images::raster_from_compressed_texture_data;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::scalar::{scalar_cos, scalar_sin};
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::etc1::etc1_encode_image;
use skia_rust_tools::gpu::compressed_texture::two_color_bc1_compress;

// Port of: gm/compressed_textures.cpp#L50-L55 (chrome/m156), `gen_pt`
fn gen_pt(angle: f32, scale: Point) -> Point {
    let s = scalar_sin(angle);
    let c = scalar_cos(angle);

    Point::new(scale.x * c, scale.y * s)
}

// The resulting path will be centered at (0,0) and its size will match 'dimensions'
// Port of: gm/compressed_textures.cpp#L58-L84 (chrome/m156), `make_gear`
#[allow(clippy::cast_precision_loss)] // dimensions are small integers, exact in f32
fn make_gear(dimensions: ISize, num_teeth: i32) -> Path {
    let outer_rad = Point::new(
        dimensions.width as f32 / 2.0,
        dimensions.height as f32 / 2.0,
    );
    let inner_rad = Point::new(
        dimensions.width as f32 / 2.5,
        dimensions.height as f32 / 2.5,
    );
    // `SK_ScalarPI` is `3.14159265f`, which is `f32::consts::PI`.
    let angle_per_tooth: f32 = 2.0 * std::f32::consts::PI / (3 * num_teeth) as f32;

    let mut angle: f32 = 0.0;

    let mut tmp = PathBuilder::new_with_fill_type(PathFillType::Winding);

    tmp.move_to(gen_pt(angle, outer_rad));

    for _ in 0..num_teeth {
        tmp.line_to(gen_pt(angle + angle_per_tooth, outer_rad));
        tmp.line_to(gen_pt(angle + (1.5 * angle_per_tooth), inner_rad));
        tmp.line_to(gen_pt(angle + (2.5 * angle_per_tooth), inner_rad));
        tmp.line_to(gen_pt(angle + (3.0 * angle_per_tooth), outer_rad));
        angle += 3.0 * angle_per_tooth;
    }

    tmp.close();

    let f_inner_rad = 0.1 * dimensions.width.min(dimensions.height) as f32;
    if f_inner_rad > 0.5 {
        tmp.add_circle(Point::new(0.0, 0.0), f_inner_rad, PathDirection::CCW);
    }

    tmp.detach()
}

// Render one level of a mipmap
// Port of: gm/compressed_textures.cpp#L87-L108 (chrome/m156), `render_level`
#[allow(clippy::cast_precision_loss)] // dimensions are small integers, exact in f32
fn render_level(dimensions: ISize, color: u32, color_type: ColorType, opaque: bool) -> Bitmap {
    let path = make_gear(dimensions, 9);

    let at = if opaque {
        AlphaType::Opaque
    } else {
        AlphaType::Premul
    };
    let ii = ImageInfo::new(dimensions, color_type, at, None);
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ii, None);

    bm.erase_color(if opaque {
        Color::BLACK
    } else {
        Color::TRANSPARENT
    });

    let mut paint = Paint::default();
    paint.set_color(color | 0xFF00_0000);
    paint.set_anti_alias(false);

    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas");
        canvas.translate(Point::new(
            dimensions.width as f32 / 2.0,
            dimensions.height as f32 / 2.0,
        ));
        canvas.draw_path(&path, &paint);
    }

    bm
}

// Create the compressed data blob needed to represent a mipmapped 2-color texture of the specified
// compression format. In this case 2-color means either opaque black or transparent black plus
// one other color.
// Note that ETC1/ETC2_RGB8_UNORM only supports 565 opaque textures.
// Use a different color for each mipmap level so we can visually evaluate the draws
// Port of: gm/compressed_textures.cpp#L138-L145 (chrome/m156), `kColors`
const K_COLORS: [u32; 7] = [
    0xFFFF_0000, // SK_ColorRED
    0xFF00_FF00, // SK_ColorGREEN
    0xFF00_00FF, // SK_ColorBLUE
    0xFF00_FFFF, // SK_ColorCYAN
    0xFFFF_00FF, // SK_ColorMAGENTA
    0xFFFF_FF00, // SK_ColorYELLOW
    0xFFFF_FFFF, // SK_ColorWHITE
];

// Port of: gm/compressed_textures.cpp#L123-L206 (chrome/m156), `make_compressed_image` (the
// raster path, which returns the image or nothing)
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation, // row bytes of a small image fit in u32
)]
fn make_compressed_image(
    dimensions: ISize,
    color_type: ColorType,
    opaque: bool,
    compression: TextureCompressionType,
) -> Option<Image> {
    let total_size = compressed_data_size(compression, dimensions, None, true);

    let mut pixels = vec![0u8; total_size];

    let num_mip_levels = Mipmap::compute_level_count_size(dimensions) as usize + 1;

    let mut offset = 0usize;

    let mut level_dims = dimensions;
    for i in 0..num_mip_levels {
        let level_size = compressed_data_size(compression, level_dims, None, false);

        let color = K_COLORS[i % 7];
        let bm = render_level(level_dims, color, color_type, opaque);
        let pixmap = bm.pixmap();
        if compression == TextureCompressionType::ETC2_RGB8_UNORM {
            let src = pixmap.addr()?;
            if etc1_encode_image(
                src,
                level_dims.width as u32,
                level_dims.height as u32,
                2,
                bm.row_bytes() as u32,
                &mut pixels[offset..],
            ) != 0
            {
                return None;
            }
        } else {
            two_color_bc1_compress(&pixmap, Color::from(color), &mut pixels[offset..]);
        }

        offset += level_size;
        level_dims = ISize::new(1.max(level_dims.width / 2), 1.max(level_dims.height / 2));
    }

    raster_from_compressed_texture_data(
        Data::new_from_vec(pixels),
        dimensions.width,
        dimensions.height,
        compression,
    )
}

/// The three image variants: `kNormal`, `kNonPowerOfTwo` and `kNonMultipleOfFour`.
// Port of: gm/compressed_textures.cpp#L220-L224 (chrome/m156), `CompressedTexturesGM::Type`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressedTexturesType {
    Normal,
    NonPowerOfTwo,
    NonMultipleOfFour,
}

// Port of: gm/compressed_textures.cpp#L218-L225 (chrome/m156), `CompressedTexturesGM`
// (the constants of the class)
const K_PAD: i32 = 8;
const K_BASE_TEX_WIDTH: i32 = 64;
// `static const int kCellWidth = 1.5f * kBaseTexWidth;`
const K_CELL_WIDTH: i32 = 96;
const K_BASE_TEX_HEIGHT: i32 = 64;

// Port of: gm/compressed_textures.cpp#L218-L393 (chrome/m156), class CompressedTexturesGM
#[derive(Debug)]
pub struct CompressedTexturesGm {
    typ: CompressedTexturesType,
    img_dimensions: ISize,

    opaque_etc2_image: Option<Image>,
    opaque_bc1_image: Option<Image>,
    transparent_bc1_image: Option<Image>,
}

impl CompressedTexturesGm {
    // Port of: gm/compressed_textures.cpp#L226-L244 (chrome/m156), the constructor
    #[must_use]
    pub fn new(typ: CompressedTexturesType) -> Self {
        let img_dimensions = match typ {
            // These dimensions force the top two mip levels to be 1x3 and 1x1
            CompressedTexturesType::NonPowerOfTwo => ISize::new(20, 60),
            // These dimensions force the top three mip levels to be 1x7, 1x3 and 1x1
            // (prime numbers - just bc)
            CompressedTexturesType::NonMultipleOfFour => ISize::new(13, 61),
            CompressedTexturesType::Normal => ISize::new(K_BASE_TEX_WIDTH, K_BASE_TEX_HEIGHT),
        };

        Self {
            typ,
            img_dimensions,
            opaque_etc2_image: None,
            opaque_bc1_image: None,
            transparent_bc1_image: None,
        }
    }

    // Port of: gm/compressed_textures.cpp#L330-L392 (chrome/m156), `drawCell`
    #[allow(clippy::cast_precision_loss)] // dimensions are small integers, exact in f32
    fn draw_cell(&self, canvas: &Canvas, image: &Image, offset: (i32, i32)) {
        let (mut offset_x, mut offset_y) = offset;
        let mut level_dimensions = self.img_dimensions;
        let num_mip_levels = Mipmap::compute_level_count_size(level_dimensions) + 1;

        let sampling: SamplingOptions = CubicResampler::mitchell().into();

        // The images made here are raster, so `isTextureBacked()` is false and the C++ takes
        // the path that outlines every level in red. The GPU branch is not ported.
        let is_compressed = image.is_texture_backed();

        let mut red_stroke_paint = Paint::default();
        red_stroke_paint.set_color(Color::from(0xFFFF_0000));
        red_stroke_paint.set_style(Style::Stroke);

        for i in 0..num_mip_levels {
            let r = Rect::from_xywh(
                offset_x as f32,
                offset_y as f32,
                level_dimensions.width as f32,
                level_dimensions.height as f32,
            );

            canvas.draw_image_rect_with_sampling_options(
                image,
                None,
                r,
                sampling,
                &Paint::default(),
            );
            if !is_compressed {
                // Make it obvious which drawImages used decompressed images
                canvas.draw_rect(r, &red_stroke_paint);
            }

            if i == 0 {
                offset_x += level_dimensions.width + 1;
            } else {
                offset_y += level_dimensions.height + 1;
            }

            level_dimensions = ISize::new(
                1.max(level_dimensions.width / 2),
                1.max(level_dimensions.height / 2),
            );
        }
    }
}

impl GM for CompressedTexturesGm {
    // Port of: gm/compressed_textures.cpp#L246-L256 (chrome/m156), `getName`
    fn name(&self) -> String {
        let mut name = String::from("compressed_textures");

        if self.typ == CompressedTexturesType::NonPowerOfTwo {
            name.push_str("_npot");
        } else if self.typ == CompressedTexturesType::NonMultipleOfFour {
            name.push_str("_nmof");
        }

        name
    }

    // Port of: gm/compressed_textures.cpp#L258-L261 (chrome/m156), `getISize`
    fn size(&mut self) -> ISize {
        ISize::new(
            2 * K_CELL_WIDTH + 3 * K_PAD,
            2 * K_BASE_TEX_HEIGHT + 3 * K_PAD,
        )
    }

    // Port of: gm/compressed_textures.cpp#L227 (chrome/m156), `setBGColor(0xFFCCCCCC)`
    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/compressed_textures.cpp#L262-L309 (chrome/m156), `onGpuSetup`
    fn on_gpu_setup(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        self.opaque_etc2_image = make_compressed_image(
            self.img_dimensions,
            ColorType::RGB565,
            true,
            TextureCompressionType::ETC2_RGB8_UNORM,
        );

        self.opaque_bc1_image = make_compressed_image(
            self.img_dimensions,
            ColorType::RGBA8888,
            true,
            TextureCompressionType::BC1_RGB8_UNORM,
        );

        self.transparent_bc1_image = make_compressed_image(
            self.img_dimensions,
            ColorType::RGBA8888,
            false,
            TextureCompressionType::BC1_RGBA8_UNORM,
        );

        if self.opaque_etc2_image.is_none()
            || self.opaque_bc1_image.is_none()
            || self.transparent_bc1_image.is_none()
        {
            "Failed to create compressed images.".clone_into(error_msg);
            return DrawResult::Fail;
        }

        DrawResult::Ok
    }

    // Port of: gm/compressed_textures.cpp#L320-L328 (chrome/m156), `onDraw`
    fn on_draw(&mut self, canvas: &Canvas) {
        if let Some(image) = &self.opaque_etc2_image {
            self.draw_cell(canvas, image, (K_PAD, K_PAD));
        }
        if let Some(image) = &self.opaque_bc1_image {
            self.draw_cell(canvas, image, (2 * K_PAD + K_CELL_WIDTH, K_PAD));
        }
        if let Some(image) = &self.transparent_bc1_image {
            self.draw_cell(
                canvas,
                image,
                (2 * K_PAD + K_CELL_WIDTH, 2 * K_PAD + K_BASE_TEX_HEIGHT),
            );
        }
    }
}

// Port of: gm/compressed_textures.cpp#L397-L399 (chrome/m156), the DEF_GM registrations
crate::def_gm!(
    CompressedTexturesGM_kNormal = "CompressedTexturesGM(CompressedTexturesGM::Type::kNormal)",
    CompressedTexturesGm::new(CompressedTexturesType::Normal)
);
crate::def_gm!(
    CompressedTexturesGM_kNonPowerOfTwo =
        "CompressedTexturesGM(CompressedTexturesGM::Type::kNonPowerOfTwo)",
    CompressedTexturesGm::new(CompressedTexturesType::NonPowerOfTwo)
);
crate::def_gm!(
    CompressedTexturesGM_kNonMultipleOfFour =
        "CompressedTexturesGM(CompressedTexturesGM::Type::kNonMultipleOfFour)",
    CompressedTexturesGm::new(CompressedTexturesType::NonMultipleOfFour)
);
