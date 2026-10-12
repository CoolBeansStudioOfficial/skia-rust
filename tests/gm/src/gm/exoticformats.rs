// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/exoticformats.cpp (chrome/m156)

// This GM exercises our handling of some of the more exotic formats using externally generated
// content. Right now it only tests ETC1 and BC1. On a raster canvas the images are decoded on the
// CPU by `SkImages::RasterFromCompressedTextureData`; the Ganesh upload branch of `data_to_img`
// and the `isTextureBacked` check in `drawImage` do not run here.

use crate::prelude::*;
use skia_rust_core::compressed_data_utils::compressed_data_size;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::images::raster_from_compressed_texture_data;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;
use skia_rust_tools::resources::get_resource_bytes;

// Port of: gm/exoticformats.cpp#L32-L36 (chrome/m156), `struct ImageInfo`
struct ImageInfo {
    dim: ISize,
    mipmapped: bool,
    compression_type: TextureCompressionType,
}

// Port of: gm/exoticformats.cpp#L38-L46 (chrome/m156), `get_uint`: an unaligned native (little
// endian) 32-bit read. The caller checks the length.
fn get_uint(buffer: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(buffer[i..i + 4].try_into().expect("4 bytes"))
}

// Port of: gm/exoticformats.cpp#L58-L60 (chrome/m156), `kKTXIdentifierSize`
const KTX_IDENTIFIER_SIZE: usize = 12;
// Port of: gm/exoticformats.cpp#L62 (chrome/m156), `kKTXHeaderSize`
const KTX_HEADER_SIZE: usize = KTX_IDENTIFIER_SIZE + 13 * 4;
// Port of: gm/exoticformats.cpp#L70-L72 (chrome/m156), `kExpectedIdentifier`
const KTX_EXPECTED_IDENTIFIER: [u8; KTX_IDENTIFIER_SIZE] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

// The GL enums the KTX loader accepts (`GR_GL_COMPRESSED_*`, src/gpu/ganesh/gl/GrGLDefines.h).
const GR_GL_COMPRESSED_ETC1_RGB8: u32 = 0x8D64;
const GR_GL_COMPRESSED_RGB8_ETC2: u32 = 0x9274;
const GR_GL_COMPRESSED_RGB_S3TC_DXT1_EXT: u32 = 0x83F0;
const GR_GL_COMPRESSED_RGBA_S3TC_DXT1_EXT: u32 = 0x83F1;

// Port of: gm/exoticformats.cpp#L48-L152 (chrome/m156), `load_ktx`
//
// This KTX loader is barely sufficient to load the specific files this GM requires. Use at your
// own peril. Returns `None` where the C++ returns `nullptr`.
#[allow(clippy::cast_possible_wrap)] // `int numberOfMipmapLevels = get_uint(...)`, as in C++
fn load_ktx(path: &str) -> Option<(Data, ImageInfo)> {
    let file = get_resource_bytes(path)?;
    if file.len() < KTX_HEADER_SIZE {
        return None;
    }
    let header = &file[..KTX_HEADER_SIZE];

    if header[..KTX_IDENTIFIER_SIZE] != KTX_EXPECTED_IDENTIFIER {
        return None;
    }

    let endianness = get_uint(header, 12);
    if endianness != 0x0403_0201 {
        // TODO in Skia: need to swap rest of header and, if glTypeSize is > 1, all the texture data.
        return None;
    }

    let gl_type = get_uint(header, 16);
    let gl_format = get_uint(header, 24);
    let gl_internal_format = get_uint(header, 28);
    let pixel_width = get_uint(header, 36);
    let pixel_height = get_uint(header, 40);
    let pixel_depth = get_uint(header, 44);
    let number_of_faces = get_uint(header, 52);
    let number_of_mipmap_levels = get_uint(header, 56) as i32;
    let bytes_of_key_value_data = get_uint(header, 60);

    if gl_type != 0 || gl_format != 0 {
        // only care about compressed data for now
        return None;
    }

    // We only handle these four formats right now
    let compression_type = match gl_internal_format {
        GR_GL_COMPRESSED_ETC1_RGB8 | GR_GL_COMPRESSED_RGB8_ETC2 => {
            TextureCompressionType::ETC2_RGB8_UNORM
        }
        GR_GL_COMPRESSED_RGB_S3TC_DXT1_EXT => TextureCompressionType::BC1_RGB8_UNORM,
        GR_GL_COMPRESSED_RGBA_S3TC_DXT1_EXT => TextureCompressionType::BC1_RGBA8_UNORM,
        _ => return None,
    };

    let dim = ISize::new(pixel_width as i32, pixel_height as i32);

    if pixel_depth != 0 {
        return None; // pixel depth is always zero for 2D textures
    }

    if number_of_faces != 1 {
        return None; // we don't support cube maps right now
    }

    let mipmapped = if number_of_mipmap_levels == 1 {
        false
    } else {
        let num_required_mip_levels =
            Mipmap::compute_level_count(pixel_width as i32, pixel_height as i32) + 1;
        if number_of_mipmap_levels != num_required_mip_levels {
            return None;
        }
        true
    };

    if bytes_of_key_value_data != 0 {
        return None;
    }

    let mut individual_mip_offsets = Vec::new();
    let data_size = compressed_data_size(
        compression_type,
        dim,
        Some(&mut individual_mip_offsets),
        mipmapped,
    );

    let mut dest = vec![0u8; data_size];

    let mut pos = KTX_HEADER_SIZE;
    let mut offset = 0usize;
    for _ in 0..number_of_mipmap_levels {
        let image_size = usize::try_from(get_uint(file.get(pos..pos + 4)?, 0)).ok()?;
        pos += 4;

        // The C++ asserts that the level fits in the data; a file that doesn't is rejected.
        if offset + image_size > data_size {
            return None;
        }
        let src = file.get(pos..pos.checked_add(image_size)?)?;
        dest[offset..offset + image_size].copy_from_slice(src);

        pos += image_size;
        offset += image_size;
    }

    Some((
        Data::new_from_vec(dest),
        ImageInfo {
            dim,
            mipmapped,
            compression_type,
        },
    ))
}

// Port of: gm/exoticformats.cpp#L154-L172 (chrome/m156), `DWORD`, the `DDS_PIXELFORMAT` flags and
// the `DDS_HEADER` flags
const DDPF_FOURCC: u32 = 0x4;

const DDSD_CAPS: u32 = 0x1; // required
const DDSD_HEIGHT: u32 = 0x2; // required
const DDSD_WIDTH: u32 = 0x4; // required
const DDSD_PIXELFORMAT: u32 = 0x0000_1000; // required
const DDSD_MIPMAPCOUNT: u32 = 0x0002_0000;
// The C++ also names DDSD_PITCH (0x8), DDSD_LINEARSIZE (0x80000) and DDSD_DEPTH (0x800000) for a
// block that only has a TODO, so they are not checked here.

const DDSD_REQUIRED: u32 = DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PIXELFORMAT;

// `kMagic`: "DDS " read as a little-endian u32.
const DDS_MAGIC: u32 = 0x2053_4444;
// `kDDSHeaderSize` (`sizeof(DDS_HEADER)`) and `kDDSPixelFormatSize`.
const DDS_HEADER_SIZE: usize = 124;
const DDS_PIXEL_FORMAT_SIZE: u32 = 32;

// Byte offsets of the `DDS_HEADER` fields this loader reads, relative to the header (the file
// without its 4-byte magic): dwSize 0, dwFlags 4, dwHeight 8, dwWidth 12, dwMipMapCount 24,
// ddspf at 72 (dwSize 72, dwFlags 76, dwFourCC 80).
const DDS_HEADER_SIZE_FIELD: usize = 0;
const DDS_HEADER_FLAGS_FIELD: usize = 4;
const DDS_HEADER_HEIGHT_FIELD: usize = 8;
const DDS_HEADER_WIDTH_FIELD: usize = 12;
const DDS_HEADER_MIPMAP_COUNT_FIELD: usize = 24;
const DDS_PIXELFORMAT_SIZE_FIELD: usize = 72;
const DDS_PIXELFORMAT_FLAGS_FIELD: usize = 76;
const DDS_PIXELFORMAT_FOURCC_FIELD: usize = 80;

// Port of: gm/exoticformats.cpp#L173-L320 (chrome/m156), `load_dds`
//
// This DDS loader is barely sufficient to load the specific files this GM requires. Use at your
// own peril. Returns `None` where the C++ returns `nullptr`.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss, // dimensions and mip counts, as in C++
)]
fn load_dds(path: &str) -> Option<(Data, ImageInfo)> {
    let file = get_resource_bytes(path)?;

    if file.get(..4)? != DDS_MAGIC.to_le_bytes() {
        return None;
    }

    let header = file.get(4..4 + DDS_HEADER_SIZE)?;

    if get_uint(header, DDS_HEADER_SIZE_FIELD) != DDS_HEADER_SIZE as u32
        || get_uint(header, DDS_PIXELFORMAT_SIZE_FIELD) != DDS_PIXEL_FORMAT_SIZE
    {
        return None;
    }

    let flags = get_uint(header, DDS_HEADER_FLAGS_FIELD);
    if flags & DDSD_REQUIRED != DDSD_REQUIRED {
        return None;
    }

    // TODO in Skia: support DDSD_PITCH, DDSD_LINEARSIZE and DDSD_DEPTH.

    let width = get_uint(header, DDS_HEADER_WIDTH_FIELD);
    let height = get_uint(header, DDS_HEADER_HEIGHT_FIELD);
    let dim = ISize::new(width as i32, height as i32);

    let mut number_of_mipmap_levels = 1;
    let mipmapped = if flags & DDSD_MIPMAPCOUNT != 0 {
        let mip_map_count = get_uint(header, DDS_HEADER_MIPMAP_COUNT_FIELD);
        if mip_map_count == 1 {
            false
        } else {
            let num_required_levels = Mipmap::compute_level_count(width as i32, height as i32) + 1;
            if mip_map_count != num_required_levels as u32 {
                return None;
            }
            number_of_mipmap_levels = num_required_levels;
            true
        }
    } else {
        false
    };

    if get_uint(header, DDS_PIXELFORMAT_FLAGS_FIELD) & DDPF_FOURCC == 0 {
        return None;
    }

    // We only handle these one format right now
    let compression_type = match get_uint(header, DDS_PIXELFORMAT_FOURCC_FIELD) {
        0x3154_5844 => TextureCompressionType::BC1_RGB8_UNORM, // DXT1
        _ => return None,
    };

    let mut individual_mip_offsets = Vec::new();
    let data_size = compressed_data_size(
        compression_type,
        dim,
        Some(&mut individual_mip_offsets),
        mipmapped,
    );
    debug_assert_eq!(
        individual_mip_offsets.len(),
        number_of_mipmap_levels as usize
    );

    // The mip levels follow the header directly, one after another.
    let start = 4 + DDS_HEADER_SIZE;
    let src = file.get(start..start.checked_add(data_size)?)?;
    Some((
        Data::new_from_vec(src.to_vec()),
        ImageInfo {
            dim,
            mipmapped,
            compression_type,
        },
    ))
}

// Port of: gm/exoticformats.cpp#L322-L330 (chrome/m156), `data_to_img` (the raster branch: no
// direct GPU context)
fn data_to_img(data: Data, info: &ImageInfo) -> Option<Image> {
    raster_from_compressed_texture_data(
        data,
        info.dim.width,
        info.dim.height,
        info.compression_type,
    )
}

// Port of: gm/exoticformats.cpp#L332-L338 (chrome/m156), the image size constants
const K_IMG_WIDTH_HEIGHT: i32 = 128;
const K_PAD: i32 = 4;

// Port of: gm/exoticformats.cpp#L340-L384 (chrome/m156), `ExoticFormatsGM::drawImage`, with the
// raster path: the images are never texture backed, so the red outline is always drawn.
#[allow(clippy::cast_precision_loss)] // image offsets and sizes are small integers, exact in f32
fn draw_image(canvas: &Canvas, image: Option<&Image>, x: i32, y: i32) {
    let Some(image) = image else {
        return;
    };

    let is_compressed = image.is_texture_backed();

    canvas.draw_image(image, Point::new(x as f32, y as f32), None);

    if !is_compressed {
        // Make it obvious which drawImages used decompressed images
        let r = Rect::from_xywh(
            x as f32,
            y as f32,
            K_IMG_WIDTH_HEIGHT as f32,
            K_IMG_WIDTH_HEIGHT as f32,
        );
        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(2.0);
        canvas.draw_rect(r, &paint);
    }
}

// Port of: gm/exoticformats.cpp#L32-L36 (chrome/m156), class ExoticFormatsGM
#[derive(Debug, Default)]
pub struct ExoticFormatsGm {
    etc1_image: Option<Image>,
    bc1_image: Option<Image>,
}

impl ExoticFormatsGm {
    // Port of: gm/exoticformats.cpp#L340-L366 (chrome/m156), `loadImages` (raster branch)
    fn load_images(&mut self) -> bool {
        let Some((data, info)) = load_ktx("images/flower-etc1.ktx") else {
            // SkDebugf("failed to load flower-etc1.ktx\n") in the C++
            return false;
        };
        debug_assert_eq!(info.dim, ISize::new(K_IMG_WIDTH_HEIGHT, K_IMG_WIDTH_HEIGHT));
        debug_assert!(!info.mipmapped);
        debug_assert_eq!(
            info.compression_type,
            TextureCompressionType::ETC2_RGB8_UNORM
        );
        self.etc1_image = data_to_img(data, &info);

        let Some((data, info)) = load_dds("images/flower-bc1.dds") else {
            // SkDebugf("failed to load flower-bc1.dds\n") in the C++
            return false;
        };
        debug_assert_eq!(info.dim, ISize::new(K_IMG_WIDTH_HEIGHT, K_IMG_WIDTH_HEIGHT));
        debug_assert!(!info.mipmapped);
        debug_assert_eq!(
            info.compression_type,
            TextureCompressionType::BC1_RGB8_UNORM
        );
        self.bc1_image = data_to_img(data, &info);

        true
    }
}

impl GM for ExoticFormatsGm {
    // Port of: gm/exoticformats.cpp#L400-L402 (chrome/m156), `getName`
    fn name(&self) -> String {
        "exoticformats".to_string()
    }

    // Port of: gm/exoticformats.cpp#L404-L406 (chrome/m156), `getISize`
    fn size(&mut self) -> ISize {
        ISize::new(
            2 * K_IMG_WIDTH_HEIGHT + 3 * K_PAD,
            K_IMG_WIDTH_HEIGHT + 2 * K_PAD,
        )
    }

    // Port of: gm/exoticformats.cpp#L397-L398 (chrome/m156), `setBGColor(SK_ColorBLACK)`
    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/exoticformats.cpp#L420-L433 (chrome/m156), `onGpuSetup`
    fn on_gpu_setup(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if !self.load_images() {
            "Failed to create images.".clone_into(error_msg);
            return DrawResult::Fail;
        }

        DrawResult::Ok
    }

    // Port of: gm/exoticformats.cpp#L435-L440 (chrome/m156), `onDraw`
    fn on_draw(&mut self, canvas: &Canvas) {
        draw_image(canvas, self.etc1_image.as_ref(), K_PAD, K_PAD);
        draw_image(
            canvas,
            self.bc1_image.as_ref(),
            K_IMG_WIDTH_HEIGHT + 2 * K_PAD,
            K_PAD,
        );
    }
}

// Port of: gm/exoticformats.cpp#L444-L446 (chrome/m156), DEF_GM
crate::def_gm!(ExoticFormatsGM, ExoticFormatsGm::default());
