// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.{h,cpp} (the parts the ported tests use)

//! `ToolUtils`: helpers shared by Skia's tests.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;

/// `ToolUtils::equal_pixels(const SkPixmap&, const SkPixmap&)`: same size, same color type, and
/// the same bytes in every row (only the `width * bytesPerPixel` bytes of each row are compared).
// Port of: tools/ToolUtils.cpp#L457-L474 (chrome/m156)
#[must_use]
pub fn equal_pixels(a: &Pixmap<'_>, b: &Pixmap<'_>) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return false;
    }
    if a.color_type() != b.color_type() {
        return false;
    }
    // width * bytesPerPixel, which is minRowBytes() for a valid info.
    let row_len = a.info().min_row_bytes();
    for y in 0..a.height() {
        let (Some(a_row), Some(b_row)) = (a.addr_at((0, y)), b.addr_at((0, y))) else {
            return false;
        };
        if a_row[..row_len] != b_row[..row_len] {
            return false;
        }
    }
    true
}

/// `ToolUtils::equal_pixels(const SkImage*, const SkImage*)`: the two images, made raster, have the
/// same pixels (see [`equal_pixels`]). An image that cannot be made raster or peeked is not equal
/// (C++ aborts on a null image here).
// Port of: tools/ToolUtils.cpp#L487-L500 (chrome/m156)
#[must_use]
pub fn equal_pixels_image(a: &Image, b: &Image) -> bool {
    // ensure that peekPixels will succeed
    let (Some(raster_a), Some(raster_b)) = (a.to_raster_image(), b.to_raster_image()) else {
        return false;
    };
    let (Some(pm0), Some(pm1)) = (raster_a.peek_pixels(), raster_b.peek_pixels()) else {
        return false;
    };
    equal_pixels(&pm0, &pm1)
}

/// Port of `ToolUtils::DecodeDataToBitmap` (tools/DecodeUtils.cpp): decodes `data` into a bitmap
/// allocated with the generator's info, or `None` if no generator or no pixels.
// Port of: tools/DecodeUtils.cpp#L23-L28 (chrome/m156)
#[must_use]
pub fn decode_data_to_bitmap(data: Option<skia_rust_core::data::Data>) -> Option<Bitmap> {
    let mut generator = skia_rust_codec::image_generator_from_encoded::make_from_encoded(data, None)?;
    let gen_info = generator.info().clone();
    let row_bytes = gen_info.min_row_bytes();
    let mut pixels = vec![0u8; gen_info.compute_byte_size(row_bytes)];
    // `gen->getInfo().makeColorSpace(nullptr)`: the pixels are read without a colour space.
    if !generator.get_pixels(&gen_info.with_color_space(None), &mut pixels, row_bytes) {
        return None;
    }
    let mut bm = Bitmap::new();
    bm.install_pixels(&gen_info, pixels, row_bytes).then_some(bm)
}

/// Port of `ToolUtils::GetResourceAsBitmap` (tools/DecodeUtils.h): the bitmap decoded from the
/// resource at `resource`, or `None` if the resource is missing or cannot be decoded.
// Port of: tools/DecodeUtils.h#L23-L25 (chrome/m156)
#[must_use]
pub fn get_resource_as_bitmap(resource: &str) -> Option<Bitmap> {
    let data = crate::resources::get_resource_as_data(resource)?;
    decode_data_to_bitmap(Some(skia_rust_core::data::Data::new_from_vec(data)))
}

/// One mip level made by [`make_pixmaps`]: its info, row bytes and pixels. Each level owns its
/// pixels (Skia makes them one allocation, which nothing observes).
#[derive(Debug, Clone)]
pub struct PixmapLevel {
    pub info: ImageInfo,
    pub row_bytes: usize,
    pub pixels: Vec<u8>,
}

impl PixmapLevel {
    /// The level as a read-only pixmap.
    #[must_use]
    pub fn pixmap(&self) -> Pixmap<'_> {
        Pixmap::new_readonly(&self.info, &self.pixels, self.row_bytes)
            .expect("a level's pixels hold its info")
    }
}

/// `ToolUtils::make_pixmaps`: makes the (up to six) mip levels of a `ct` texture, each filled
/// with its entry of `colors`. Levels with an even index get row bytes that are not tight, so
/// the tests exercise row bytes that differ from the minimum.
// Port of: tools/ToolUtils.cpp#L184-L214 (chrome/m156)
#[must_use]
pub fn make_pixmaps(
    ct: ColorType,
    at: AlphaType,
    with_mips: bool,
    colors: &[Color4f; 6],
) -> Vec<PixmapLevel> {
    let mut level_size: i32 = 32;
    let num_mip_levels: usize = if with_mips { 6 } else { 1 };
    let mut levels = Vec::with_capacity(num_mip_levels);
    for (level, color) in colors.iter().enumerate().take(num_mip_levels) {
        let info = ImageInfo::new((level_size, level_size), ct, at, None);
        let mut row_bytes = info.min_row_bytes();
        // Make sure we test row bytes that aren't tight.
        if level % 2 == 0 {
            row_bytes += (level + 1) * ct.bytes_per_pixel();
        }
        let mut pixels = vec![0u8; row_bytes * level_size as usize];
        if let Some(mut pixmap) = Pixmap::new(&info, &mut pixels, row_bytes) {
            pixmap.erase_4f(color, None);
        }
        levels.push(PixmapLevel {
            info,
            row_bytes,
            pixels,
        });
        level_size /= 2;
    }
    levels
}

/// `ToolUtils::alphatype_name`.
// Port of: tools/ToolUtils.cpp#L52-L60 (chrome/m156)
#[must_use]
pub fn alphatype_name(at: AlphaType) -> &'static str {
    match at {
        AlphaType::Unknown => "Unknown",
        AlphaType::Opaque => "Opaque",
        AlphaType::Premul => "Premul",
        AlphaType::Unpremul => "Unpremul",
    }
}

/// `ToolUtils::colortype_name`.
// Port of: tools/ToolUtils.cpp#L62-L95 (chrome/m156)
#[must_use]
pub fn colortype_name(ct: ColorType) -> &'static str {
    match ct {
        ColorType::Unknown => "Unknown",
        ColorType::Alpha8 => "Alpha_8",
        ColorType::A16UNorm => "Alpha_16",
        ColorType::A16Float => "A16_float",
        ColorType::RGB565 => "RGB_565",
        ColorType::ARGB4444 => "ARGB_4444",
        ColorType::RGBA8888 => "RGBA_8888",
        ColorType::SRGBA8888 => "SRGBA_8888",
        ColorType::RGB888x => "RGB_888x",
        ColorType::BGRA8888 => "BGRA_8888",
        ColorType::RGBA1010102 => "RGBA_1010102",
        ColorType::BGRA1010102 => "BGRA_1010102",
        ColorType::RGB101010x => "RGB_101010x",
        ColorType::BGR101010x => "BGR_101010x",
        ColorType::BGR101010xXR => "BGR_101010x_XR",
        ColorType::RGBA10x6 => "RGBA_10x6",
        ColorType::Gray8 => "Gray_8",
        ColorType::RGBAF16Norm => "RGBA_F16Norm",
        ColorType::RGBF16F16F16x => "RGB_F16F16F16x",
        ColorType::RGBAF16 => "RGBA_F16",
        ColorType::RGBAF32 => "RGBA_F32",
        ColorType::R8G8UNorm => "R8G8_unorm",
        ColorType::R16UNorm => "R16_unorm",
        ColorType::R16Float => "R16_float",
        ColorType::R16G16UNorm => "R16G16_unorm",
        ColorType::R16G16Float => "R16G16_float",
        ColorType::R16G16B16A16UNorm => "R16G16B16A16_unorm",
        ColorType::R8UNorm => "R8_unorm",
        ColorType::BGRA10101010XR => "BGRA_10101010_XR",
    }
}

/// `ToolUtils::copy_to`: copies `src` into a new bitmap of color type `dst_color_type` (same
/// dimensions, alpha type and color space), converting the pixels; on success swaps it into
/// `dst` and returns true.
// Port of: tools/ToolUtils.cpp#L395-L422 (chrome/m156)
pub fn copy_to(dst: &mut Bitmap, dst_color_type: ColorType, src: &Bitmap) -> bool {
    let Some(src_pm) = src.peek_pixels() else {
        return false;
    };

    let mut tmp_dst = Bitmap::new();
    let dst_info = src_pm.info().with_color_type(dst_color_type);
    if !tmp_dst.set_info(&dst_info, None) {
        return false;
    }

    if !tmp_dst.try_alloc_pixels() {
        return false;
    }

    let Some(mut dst_pm) = tmp_dst.peek_pixels_mut() else {
        return false;
    };

    if !src_pm.read_pixels_to_pixmap(&mut dst_pm, (0, 0)) {
        return false;
    }

    dst.swap(&mut tmp_dst);
    true
}

/// `ToolUtils::PixelIter`: visits the locations of the pixels of a pixmap, row by row.
///
/// skia-rust: Skia's `next` returns the pixel's address and its location; the Rust version
/// returns the location, and the test reads the pixel from the pixmap it keeps.
// Port of: tools/ToolUtils.h#L267-L298 (chrome/m156)
#[derive(Debug)]
pub struct PixelIter {
    width: i32,
    height: i32,
    loc: IPoint,
    done: bool,
}

impl PixelIter {
    /// `PixelIter(SkSurface*)` / `reset(pm)`: starts before the first pixel of `pm`. A pixmap
    /// without pixels has no locations.
    #[must_use]
    pub fn new(pm: &Pixmap<'_>) -> PixelIter {
        PixelIter {
            width: pm.width(),
            height: pm.height(),
            loc: IPoint::new(-1, 0),
            done: pm.addr().is_none(),
        }
    }

    /// The location of the next pixel, or `None` when there are no more (`next`).
    // Port of: tools/ToolUtils.h#L281-L297 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors PixelIter::next, which takes no Iterator
    pub fn next(&mut self) -> Option<IPoint> {
        if self.done {
            return None;
        }
        self.loc.x += 1;
        if self.loc.x >= self.width {
            self.loc.x = 0;
            self.loc.y += 1;
            if self.loc.y >= self.height {
                self.done = true;
                return None;
            }
        }
        Some(self.loc)
    }
}
