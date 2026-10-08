// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ReadPixelsTest.cpp (chrome/m156)

#![cfg(test)]
// The C++ names (r, g, b, a, devx/devy, bmpAT/bmpCT) are kept.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use skia_rust_core::align::align4;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::PMColor;
use skia_rust_core::color_data::{B16_MASK_IN_PLACE, G16_MASK_IN_PLACE, R16_MASK_IN_PLACE};
use skia_rust_core::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
    premultiply_argb_inline,
};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::half::HALF_1;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::{
    color_type_is_alpha_only, image_info_is_valid, image_info_valid_conversion,
};
use skia_rust_core::images;
use skia_rust_core::m44::M44;
use skia_rust_core::math::U8CPU;
use skia_rust_core::math_priv::mul_div_255_ceiling;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, errorf, reporter_assert};

const DEV_W: i32 = 100;
const DEV_H: i32 = 100;
const DEV_RECT: IRect = IRect {
    left: 0,
    top: 0,
    right: DEV_W,
    bottom: DEV_H,
};
#[allow(clippy::cast_precision_loss)] // DEV_W * SK_Scalar1
const DEV_RECT_S: Rect = Rect {
    left: 0.0,
    top: 0.0,
    right: DEV_W as f32,
    bottom: DEV_H as f32,
};

// Port of: tests/ReadPixelsTest.cpp#L48-L75 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // x, y are in [0, DEV_W), [0, DEV_H)
fn get_src_color(x: i32, y: i32) -> PMColor {
    debug_assert!((0..DEV_W).contains(&x));
    debug_assert!((0..DEV_H).contains(&y));

    let r = x as U8CPU;
    let g = y as U8CPU;
    let b: U8CPU = 0xc;

    let mut a: U8CPU = 0xff;
    match (x + y) % 5 {
        0 => a = 0xff,
        1 => a = 0x80,
        2 => a = 0xCC,
        4 => a = 0x01,
        3 => a = 0x00,
        _ => {}
    }
    premultiply_argb_inline(a, r, g, b)
}

// Port of: tests/ReadPixelsTest.cpp#L77-L84 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // n is non-negative
fn get_dst_bmp_init_color(x: i32, y: i32, w: i32) -> PMColor {
    let n = (y * w + x) as U8CPU;

    let b = n & 0xff;
    let g = (n >> 8) & 0xff;
    let r = (n >> 16) & 0xff;
    pack_argb32(0xff, r, g, b)
}

// TODO: Make this consider both ATs
// Port of: tests/ReadPixelsTest.cpp#L86-L118 (chrome/m156)
fn convert_to_pmcolor(ct: ColorType, at: AlphaType, c: &[u8], do_unpremul: &mut bool) -> PMColor {
    *do_unpremul = AlphaType::Unpremul == at;

    let (a, mut r, mut g, mut b): (U8CPU, U8CPU, U8CPU, U8CPU);
    match ct {
        ColorType::BGRA8888 => {
            b = U8CPU::from(c[0]);
            g = U8CPU::from(c[1]);
            r = U8CPU::from(c[2]);
            a = U8CPU::from(c[3]);
        }
        ColorType::RGB888x | ColorType::RGBA8888 => {
            // (kRGB_888x falls through)
            r = U8CPU::from(c[0]);
            g = U8CPU::from(c[1]);
            b = U8CPU::from(c[2]);
            // We set this even for kRGB_888x because our caller will validate that it is 0xff.
            a = U8CPU::from(c[3]);
        }
        _ => {
            debug_assert!(false, "Unexpected colortype");
            return 0;
        }
    }

    if *do_unpremul {
        r = mul_div_255_ceiling(r, a);
        g = mul_div_255_ceiling(g, a);
        b = mul_div_255_ceiling(b, a);
    }
    pack_argb32(a, r, g, b)
}

// Port of: tests/ReadPixelsTest.cpp#L120-L135 (chrome/m156)
// (The C++ keeps the bitmap in a function-local static; it is rebuilt here.)
fn make_src_image() -> skia_rust_core::image::Image {
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((DEV_W, DEV_H), None);
    for y in 0..DEV_H {
        for x in 0..DEV_W {
            bmp.set_addr32(x, y, get_src_color(x, y));
        }
    }
    bmp.set_immutable();
    bmp.as_image().expect("an image")
}

// Port of: tests/ReadPixelsTest.cpp#L137-L146 (chrome/m156)
fn fill_src_canvas(canvas: &Canvas) {
    canvas.save();
    canvas.set_matrix(&M44::new_identity());
    canvas.clip_rect(DEV_RECT_S, ClipOp::Intersect, None);
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    canvas.draw_image_with_sampling_options(
        make_src_image(),
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    canvas.restore();
}

// Port of: tests/ReadPixelsTest.cpp#L148-L163 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // SkGetPackedA32 fits a byte
fn fill_dst_bmp_with_init_data(bitmap: &mut Bitmap) {
    let w = bitmap.width();
    let h = bitmap.height();
    for y in 0..h {
        for x in 0..w {
            let init_color = get_dst_bmp_init_color(x, y, w);
            if ColorType::Alpha8 == bitmap.color_type() {
                bitmap.set_addr8(x, y, get_packed_a32(init_color) as u8);
            } else {
                bitmap.set_addr32(x, y, init_color);
            }
        }
    }
}

// Port of: tests/ReadPixelsTest.cpp#L165-L184 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // packed channels fit an i32
fn check_read_pixel(a: PMColor, b: PMColor, did_premul_conversion: bool) -> bool {
    if !did_premul_conversion {
        return a == b;
    }
    let a_a = get_packed_a32(a) as i32;
    let a_r = get_packed_r32(a) as i32;
    let a_g = get_packed_g32(a) as i32;
    let a_b = get_packed_b32(a) as i32;

    let b_a = get_packed_a32(b) as i32;
    let b_r = get_packed_r32(b) as i32;
    let b_g = get_packed_g32(b) as i32;
    let b_b = get_packed_b32(b) as i32;

    a_a == b_a && (a_r - b_r).abs() <= 1 && (a_g - b_g).abs() <= 1 && (a_b - b_b).abs() <= 1
}

// checks the bitmap contains correct pixels after the readPixels
// if the bitmap was prefilled with pixels it checks that these weren't
// overwritten in the area outside the readPixels.
// Port of: tests/ReadPixelsTest.cpp#L186-L268 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::cast_sign_loss)] // bx, by are non-negative
fn check_read(
    reporter: &mut Reporter,
    bitmap: &Bitmap,
    x: i32,
    y: i32,
    check_surface_pixels: bool,
    check_bitmap_pixels: bool,
    surface_info: &ImageInfo,
) -> bool {
    let bmp_at = bitmap.alpha_type();
    let bmp_ct = bitmap.color_type();
    debug_assert!(!bitmap.is_null());
    debug_assert!(check_surface_pixels || check_bitmap_pixels);

    let bw = bitmap.width();
    let bh = bitmap.height();

    let src_rect = IRect::from_xywh(x, y, bw, bh);
    let clipped_src_rect = IRect::intersect(&DEV_RECT, &src_rect).unwrap_or_else(IRect::new_empty);
    if ColorType::Alpha8 == bmp_ct {
        for by in 0..bh {
            for bx in 0..bw {
                let devx = bx + src_rect.left;
                let devy = by + src_rect.top;
                let alpha = bitmap.get_addr8(bx, by);

                if clipped_src_rect.contains(IPoint::new(devx, devy)) {
                    if check_surface_pixels {
                        #[allow(clippy::cast_possible_truncation)] // SkGetPackedA32 is a byte
                        let surface_alpha = if surface_info.alpha_type() == AlphaType::Opaque {
                            0xFF
                        } else {
                            get_packed_a32(get_src_color(devx, devy)) as u8
                        };
                        if surface_alpha != alpha {
                            errorf!(
                                reporter,
                                "Expected readback alpha ({bx}, {by}) value 0x{surface_alpha:02x}, got 0x{alpha:02x}. "
                            );
                            return false;
                        }
                    }
                } else if check_bitmap_pixels {
                    let orig_dst_alpha = get_packed_a32(get_dst_bmp_init_color(bx, by, bw));
                    if orig_dst_alpha != u32::from(alpha) {
                        errorf!(
                            reporter,
                            "Expected clipped out area of readback to be unchanged. Expected 0x{orig_dst_alpha:02x}, got 0x{alpha:02x}"
                        );
                        return false;
                    }
                }
            }
        }
        return true;
    }
    let pixmap = bitmap.peek_pixels().expect("pixels");
    for by in 0..bh {
        for bx in 0..bw {
            let devx = bx + src_rect.left;
            let devy = by + src_rect.top;

            let pixel_bytes = &pixmap.addr_at((bx, by)).expect("a pixel")[..4];
            let pixel = bitmap.get_addr32(bx, by);

            if clipped_src_rect.contains(IPoint::new(devx, devy)) {
                if check_surface_pixels {
                    let mut surface_pm_color = get_src_color(devx, devy);
                    if color_type_is_alpha_only(surface_info.color_type()) {
                        surface_pm_color &= 0xFF00_0000;
                    }
                    if AlphaType::Opaque == surface_info.alpha_type() || AlphaType::Opaque == bmp_at
                    {
                        surface_pm_color |= 0xFF00_0000;
                    }
                    let mut did_premul = false;
                    let pm_pixel = convert_to_pmcolor(bmp_ct, bmp_at, pixel_bytes, &mut did_premul);
                    if !check_read_pixel(pm_pixel, surface_pm_color, did_premul) {
                        errorf!(
                            reporter,
                            "Expected readback pixel ({bx}, {by}) value 0x{surface_pm_color:08x}, got 0x{pm_pixel:08x}. Readback was unpremul: {}",
                            i32::from(did_premul)
                        );
                        return false;
                    }
                }
            } else if check_bitmap_pixels {
                let orig_dst_pixel = get_dst_bmp_init_color(bx, by, bw);
                if orig_dst_pixel != pixel {
                    errorf!(
                        reporter,
                        "Expected clipped out area of readback to be unchanged. Expected 0x{orig_dst_pixel:08x}, got 0x{pixel:08x}"
                    );
                    return false;
                }
            }
        }
    }
    true
}

/// `TightRowBytes`.
#[derive(Copy, Clone, PartialEq, Eq)]
enum TightRowBytes {
    No,
    Yes,
}

// Port of: tests/ReadPixelsTest.cpp#L272-L280 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // info.width() + 16 is positive
fn init_bitmap(
    bitmap: &mut Bitmap,
    rect: &IRect,
    tight_rb: TightRowBytes,
    ct: ColorType,
    at: AlphaType,
) {
    let info = ImageInfo::new(rect.size(), ct, at, None);
    let mut row_bytes = 0;
    if tight_rb == TightRowBytes::No {
        row_bytes = align4((info.width() + 16) as usize * info.bytes_per_pixel());
    }
    bitmap.alloc_pixels_info(&info, row_bytes);
}

// Port of: tests/ReadPixelsTest.cpp#L282-L293 (chrome/m156)
const G_READ_PIXELS_CONFIGS: [(ColorType, AlphaType); 6] = [
    (ColorType::RGBA8888, AlphaType::Premul),
    (ColorType::RGBA8888, AlphaType::Unpremul),
    (ColorType::RGB888x, AlphaType::Opaque),
    (ColorType::BGRA8888, AlphaType::Premul),
    (ColorType::BGRA8888, AlphaType::Unpremul),
    (ColorType::Alpha8, AlphaType::Premul),
];

// Port of: tests/ReadPixelsTest.cpp#L294-L339 (chrome/m156)
const G_READ_PIXELS_TEST_RECTS: [IRect; 22] = [
    // entire thing
    DEV_RECT,
    // larger on all sides
    IRect::new(-10, -10, DEV_W + 10, DEV_H + 10),
    // fully contained
    IRect::new(DEV_W / 4, DEV_H / 4, 3 * DEV_W / 4, 3 * DEV_H / 4),
    // outside top left
    IRect::new(-10, -10, -1, -1),
    // touching top left corner
    IRect::new(-10, -10, 0, 0),
    // overlapping top left corner
    IRect::new(-10, -10, DEV_W / 4, DEV_H / 4),
    // overlapping top left and top right corners
    IRect::new(-10, -10, DEV_W + 10, DEV_H / 4),
    // touching entire top edge
    IRect::new(-10, -10, DEV_W + 10, 0),
    // overlapping top right corner
    IRect::new(3 * DEV_W / 4, -10, DEV_W + 10, DEV_H / 4),
    // contained in x, overlapping top edge
    IRect::new(DEV_W / 4, -10, 3 * DEV_W / 4, DEV_H / 4),
    // outside top right corner
    IRect::new(DEV_W + 1, -10, DEV_W + 10, -1),
    // touching top right corner
    IRect::new(DEV_W, -10, DEV_W + 10, 0),
    // overlapping top left and bottom left corners
    IRect::new(-10, -10, DEV_W / 4, DEV_H + 10),
    // touching entire left edge
    IRect::new(-10, -10, 0, DEV_H + 10),
    // overlapping bottom left corner
    IRect::new(-10, 3 * DEV_H / 4, DEV_W / 4, DEV_H + 10),
    // contained in y, overlapping left edge
    IRect::new(-10, DEV_H / 4, DEV_W / 4, 3 * DEV_H / 4),
    // outside bottom left corner
    IRect::new(-10, DEV_H + 1, -1, DEV_H + 10),
    // touching bottom left corner
    IRect::new(-10, DEV_H, 0, DEV_H + 10),
    // overlapping bottom left and bottom right corners
    IRect::new(-10, 3 * DEV_H / 4, DEV_W + 10, DEV_H + 10),
    // touching entire left edge
    IRect::new(0, DEV_H, DEV_W, DEV_H + 10),
    // overlapping bottom right corner
    IRect::new(3 * DEV_W / 4, 3 * DEV_H / 4, DEV_W + 10, DEV_H + 10),
    // overlapping top right and bottom right corners
    IRect::new(3 * DEV_W / 4, -10, DEV_W + 10, DEV_H + 10),
];

// Port of: tests/ReadPixelsTest.cpp#L341-L344 (chrome/m156)
fn read_should_succeed(src_rect: &IRect, dst_info: &ImageInfo, src_info: &ImageInfo) -> bool {
    IRect::intersects(src_rect, &DEV_RECT) && image_info_valid_conversion(dst_info, src_info)
}

// Port of: tests/ReadPixelsTest.cpp#L346-L392 (chrome/m156)
fn test_readpixels(reporter: &mut Reporter, surface: &mut Surface<'_>, surface_info: &ImageInfo) {
    fill_src_canvas(surface.canvas());
    for src_rect in &G_READ_PIXELS_TEST_RECTS {
        for tight_rb in [TightRowBytes::Yes, TightRowBytes::No] {
            for &(ct, at) in &G_READ_PIXELS_CONFIGS {
                let mut bmp = Bitmap::new();
                init_bitmap(&mut bmp, src_rect, tight_rb, ct, at);

                // if the bitmap has pixels allocated before the readPixels,
                // note that and fill them with pattern
                let starts_with_pixels = !bmp.is_null();
                if starts_with_pixels {
                    fill_dst_bmp_with_init_data(&mut bmp);
                }
                let id_before = surface.generation_id();
                let success =
                    surface.read_pixels_to_bitmap(&mut bmp, (src_rect.left, src_rect.top));
                let id_after = surface.generation_id();

                // we expect to succeed when the read isn't fully clipped out and the infos are
                // compatible.
                let expect_success = read_should_succeed(src_rect, bmp.info(), surface_info);
                // determine whether we expected the read to succeed.
                reporter_assert!(
                    reporter,
                    expect_success == success,
                    "Read succeed={} unexpectedly, src ct/at: {:?}/{:?}, dst ct/at: {:?}/{:?}",
                    i32::from(success),
                    surface_info.color_type(),
                    surface_info.alpha_type(),
                    bmp.info().color_type(),
                    bmp.info().alpha_type()
                );
                // read pixels should never change the gen id
                reporter_assert!(reporter, id_before == id_after);

                if success || starts_with_pixels {
                    check_read(
                        reporter,
                        &bmp,
                        src_rect.left,
                        src_rect.top,
                        success,
                        starts_with_pixels,
                        surface_info,
                    );
                } else {
                    // if we had no pixels beforehand and the readPixels
                    // failed then our bitmap should still not have pixels
                    reporter_assert!(reporter, bmp.is_null());
                }
            }
        }
    }
}

// Port of: tests/ReadPixelsTest.cpp#L394-L398 (chrome/m156)
def_tier_test!(ReadPixels, |reporter| {
    let info = ImageInfo::new_n32_premul((DEV_W, DEV_H), None);
    let mut surface = surfaces::raster(&info, None, None).expect("surface");
    test_readpixels(reporter, &mut surface, &info);
});

///////////////////////////////////////////////////////////////////////////////////////////////////

const NUM_PIXELS: usize = 5;

// The five reference pixels are: red, green, blue, white, black.
// Five is an interesting number to test because we'll exercise a full 4-wide SIMD vector
// plus a tail pixel.
// Port of: tests/ReadPixelsTest.cpp#L402-L431 (chrome/m156)
const RGBA: [u32; NUM_PIXELS] = [
    0xFF00_00FF,
    0xFF00_FF00,
    0xFFFF_0000,
    0xFFFF_FFFF,
    0xFF00_0000,
];
const BGRA: [u32; NUM_PIXELS] = [
    0xFFFF_0000,
    0xFF00_FF00,
    0xFF00_00FF,
    0xFFFF_FFFF,
    0xFF00_0000,
];
#[allow(clippy::cast_possible_truncation)] // the 565 masks fit 16 bits
const RGB565: [u16; NUM_PIXELS] = [
    R16_MASK_IN_PLACE as u16,
    G16_MASK_IN_PLACE as u16,
    B16_MASK_IN_PLACE as u16,
    0xFFFF,
    0x0,
];

const RGBA4444: [u16; NUM_PIXELS] = [0xF00F, 0x0F0F, 0x00FF, 0xFFFF, 0x000F];

#[allow(clippy::identity_op)] // `(uint64_t) SK_Half1 << 0`
const RED: u64 = (HALF_1 as u64) << 0;
const GREEN: u64 = (HALF_1 as u64) << 16;
const BLUE: u64 = (HALF_1 as u64) << 32;
const ALPHA: u64 = (HALF_1 as u64) << 48;
const F16: [u64; NUM_PIXELS] = [
    ALPHA | RED,
    ALPHA | GREEN,
    ALPHA | BLUE,
    ALPHA | BLUE | GREEN | RED,
    ALPHA,
];

const ALPHA8: [u8; NUM_PIXELS] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
const GRAY8: [u8; NUM_PIXELS] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

// The reference pixels as bytes (`five_reference_pixels`).
// Port of: tests/ReadPixelsTest.cpp#L433-L458 (chrome/m156)
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
fn five_reference_pixels(color_type: ColorType) -> Option<Vec<u8>> {
    match color_type {
        ColorType::Unknown => None,
        ColorType::Alpha8 => Some(ALPHA8.to_vec()),
        ColorType::RGB565 => Some(RGB565.iter().flat_map(|p| p.to_ne_bytes()).collect()),
        ColorType::ARGB4444 => Some(RGBA4444.iter().flat_map(|p| p.to_ne_bytes()).collect()),
        ColorType::RGBA8888 => Some(RGBA.iter().flat_map(|p| p.to_ne_bytes()).collect()),
        ColorType::BGRA8888 => Some(BGRA.iter().flat_map(|p| p.to_ne_bytes()).collect()),
        ColorType::Gray8 => Some(GRAY8.to_vec()),
        ColorType::RGBAF16 => Some(F16.iter().flat_map(|p| p.to_ne_bytes()).collect()),
        _ => None,
    }
}

// Port of: tests/ReadPixelsTest.cpp#L460-L498 (chrome/m156)
fn test_conversion(r: &mut Reporter, dst_info: &ImageInfo, src_info: &ImageInfo) {
    if !image_info_is_valid(src_info) {
        return;
    }

    let src_pixels = five_reference_pixels(src_info.color_type()).unwrap_or_default();
    let src_pixmap = Pixmap::new_readonly(src_info, &src_pixels, src_info.min_row_bytes());
    let src = src_pixmap
        .as_ref()
        .and_then(|pm| images::raster_from_pixmap(pm, || {}));
    reporter_assert!(r, src.is_some());
    let Some(src) = src else {
        return;
    };

    // Enough space for 5 pixels when color type is F16, more than enough space in other cases.
    let mut dst_pixels = [0_u8; NUM_PIXELS * 8];
    let success = match Pixmap::new(dst_info, &mut dst_pixels, dst_info.min_row_bytes()) {
        Some(mut dst_pixmap) => src.read_pixels_to_pixmap(&mut dst_pixmap, (0, 0)),
        // (An `SkPixmap` over an unknown color type still holds the pointer; the read fails.)
        None => false,
    };
    reporter_assert!(
        r,
        success == image_info_valid_conversion(dst_info, src_info)
    );

    if success {
        if ColorType::Gray8 == src_info.color_type() && ColorType::Gray8 != dst_info.color_type() {
            // TODO: test (r,g,b) == (gray,gray,gray)?
            return;
        }

        if ColorType::Gray8 == dst_info.color_type() && ColorType::Gray8 != src_info.color_type() {
            // TODO: test gray = luminance?
            return;
        }

        if ColorType::Alpha8 == src_info.color_type() && ColorType::Alpha8 != dst_info.color_type()
        {
            // TODO: test output = black with this alpha?
            return;
        }

        let n = NUM_PIXELS * dst_info.color_type().bytes_per_pixel();
        let reference = five_reference_pixels(dst_info.color_type()).unwrap_or_default();
        reporter_assert!(r, dst_pixels[..n] == reference[..n]);
    }
}

// Port of: tests/ReadPixelsTest.cpp#L500-L539 (chrome/m156)
def_tier_test!(ReadPixels_ValidConversion, |reporter| {
    let color_types = [
        ColorType::Unknown,
        ColorType::Alpha8,
        ColorType::RGB565,
        ColorType::ARGB4444,
        ColorType::RGBA8888,
        ColorType::BGRA8888,
        ColorType::Gray8,
        ColorType::RGBAF16,
    ];

    let alpha_types = [
        AlphaType::Unknown,
        AlphaType::Opaque,
        AlphaType::Premul,
        AlphaType::Unpremul,
    ];

    let color_spaces = [None, Some(ColorSpace::new_srgb())];

    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)] // NUM_PIXELS is 5
    let n = NUM_PIXELS as i32;
    for dst_ct in color_types {
        for dst_at in alpha_types {
            for dst_cs in &color_spaces {
                for src_ct in color_types {
                    for src_at in alpha_types {
                        for src_cs in &color_spaces {
                            test_conversion(
                                reporter,
                                &ImageInfo::new((n, 1), dst_ct, dst_at, dst_cs.clone()),
                                &ImageInfo::new((n, 1), src_ct, src_at, src_cs.clone()),
                            );
                        }
                    }
                }
            }
        }
    }
});

// Port of: tests/ReadPixelsTest.cpp#L541-L555 (chrome/m156)
def_tier_test!(ReadPixels_InvalidRowBytes, |reporter| {
    let src_ii = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surf = surfaces::raster(&src_ii, None, None).expect("surface");
    #[allow(clippy::cast_possible_wrap)] // LAST_ENUM is small
    for ct in 0..=(ColorType::LAST_ENUM as i32) {
        let color_type = ColorType::from_i32(ct).expect("valid color type");
        let bpp = color_type.bytes_per_pixel();
        if bpp <= 1 {
            continue;
        }
        let dst_ii = src_ii.with_color_type(color_type);
        #[allow(clippy::cast_sign_loss)] // width and height are positive
        let (bad_row_bytes, height) = (
            (surf.width() + 1) as usize * bpp - 1,
            surf.height() as usize,
        );
        let mut storage = vec![0_u8; bad_row_bytes * height];
        reporter_assert!(
            reporter,
            !surf.read_pixels(&dst_ii, &mut storage, bad_row_bytes, (0, 0))
        );
    }
});
