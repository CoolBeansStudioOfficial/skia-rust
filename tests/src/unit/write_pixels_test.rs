// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/WritePixelsTest.cpp (chrome/m156)
//
// Not ported: the Ganesh tests (`WritePixels_Gpu`, `WritePixelsMSAA_Gpu`,
// `WritePixelsNonTexture_Gpu`, `WritePixelsNonTextureMSAA_Gpu`, `WritePixelsPendingIO`) and
// `WritePixels_Graphite` (Graphite recorder; GPU).

#![cfg(test)]
// Mirrors the C++ (single-letter channel names; one switch arm per case value).
#![allow(clippy::many_single_char_names, clippy::match_same_arms)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_data::{swizzle_bgra_to_pm_color, swizzle_rgba_to_pm_color};
use skia_rust_core::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
    premultiply_argb_inline,
};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::{color_type_is_alpha_only, image_info_valid_conversion};
use skia_rust_core::malloc_pixel_ref;
use skia_rust_core::math_priv::mul_div_255_ceiling;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::safe32::abs32;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, errorf, reporter_assert};

const DEV_W: i32 = 100;
const DEV_H: i32 = 100;
const DEV_PAD: u8 = 0xee;

// Port of: tests/WritePixelsTest.cpp#L69-L92 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // x, y are in [0, 100)
fn get_canvas_color(x: i32, y: i32) -> u32 {
    debug_assert!((0..DEV_W).contains(&x));
    debug_assert!((0..DEV_H).contains(&y));

    let (r, g, b) = (x as u32, y as u32, 0xc_u32);

    let a: u32 = match (x + y) % 5 {
        0 => 0xff,
        1 => 0x80,
        2 => 0xCC,
        3 => 0x00,
        4 => 0x01,
        _ => 0x0,
    };
    premultiply_argb_inline(a, r, g, b)
}

/// assumes any premu/.unpremul has been applied
// Port of: tests/WritePixelsTest.cpp#L94-L116 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // U8CPU channel values, as the C++ uint8_t stores
fn pack_color_type(ct: ColorType, a: u32, r: u32, g: u32, b: u32) -> u32 {
    let (a, r, g, b) = (a as u8, r as u8, g as u8, b as u8);
    match ct {
        ColorType::BGRA8888 => u32::from_ne_bytes([b, g, r, a]),
        ColorType::RGBA8888 | ColorType::RGB888x => u32::from_ne_bytes([r, g, b, a]),
        _ => {
            unreachable!("SkASSERT(0)");
        }
    }
}

// Port of: tests/WritePixelsTest.cpp#L118-L148 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // masked to 8 bits
fn get_bitmap_color(x: i32, y: i32, w: i32, ct: ColorType, at: AlphaType) -> u32 {
    let n = y * w + x;
    let mut b = (n & 0xff) as u32;
    let mut g = ((n >> 8) & 0xff) as u32;
    let mut r = ((n >> 16) & 0xff) as u32;
    let a: u32 = match (x + y) % 5 {
        4 => 0xff,
        3 => 0x80,
        2 => 0xCC,
        1 => 0x01,
        0 => 0x00,
        _ => 0,
    };
    if AlphaType::Premul == at {
        r = mul_div_255_ceiling(r, a);
        g = mul_div_255_ceiling(g, a);
        b = mul_div_255_ceiling(b, a);
    }
    pack_color_type(ct, a, r, g, b)
}

// Port of: tests/WritePixelsTest.cpp#L150-L158 (chrome/m156)
fn fill_surface(surface: &mut Surface<'_>) {
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((DEV_W, DEV_H), None);
    for y in 0..DEV_H {
        for x in 0..DEV_W {
            bmp.set_addr32(x, y, get_canvas_color(x, y));
        }
    }
    surface.write_pixels_from_bitmap(&bmp, (0, 0));
}

/// Lucky for us, alpha is always in the same spot (`SK_A32_SHIFT`), for both RGBA and BGRA.
/// Thus this routine doesn't need to know the exact colortype
// Port of: tests/WritePixelsTest.cpp#L160-L172 (chrome/m156)
fn premul(color: u32) -> u32 {
    let a = get_packed_a32(color);
    // these next three are not necessarily r,g,b in that order, but they are r,g,b in some order.
    let mut c0 = get_packed_r32(color);
    let mut c1 = get_packed_g32(color);
    let mut c2 = get_packed_b32(color);
    c0 = mul_div_255_ceiling(c0, a);
    c1 = mul_div_255_ceiling(c1, a);
    c2 = mul_div_255_ceiling(c2, a);
    pack_argb32(a, c0, c1, c2)
}

// Port of: tests/WritePixelsTest.cpp#L174-L192 (chrome/m156)
fn convert_to_pm_color(ct: ColorType, at: AlphaType, mut color: u32) -> u32 {
    if AlphaType::Unpremul == at {
        color = premul(color);
    }
    match ct {
        ColorType::RGBA8888 | ColorType::RGB888x => {
            color = swizzle_rgba_to_pm_color(color);
        }
        ColorType::BGRA8888 => {
            color = swizzle_bgra_to_pm_color(color);
        }
        _ => {
            unreachable!("SkASSERT(0)");
        }
    }
    color
}

// Port of: tests/WritePixelsTest.cpp#L194-L212 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // packed channels are 8-bit
fn check_pixel(a: u32, b: u32, did_premul_conversion: bool) -> bool {
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

    a_a == b_a && abs32(a_r - b_r) <= 1 && abs32(a_g - b_g) <= 1 && abs32(a_b - b_b) <= 1
}

// Port of: tests/WritePixelsTest.cpp#L214-L236 (chrome/m156)
fn write_should_succeed(dst_info: &ImageInfo, src_info: &ImageInfo, is_gpu: bool) -> bool {
    if !image_info_valid_conversion(dst_info, src_info) {
        return false;
    }
    if !is_gpu {
        return true;
    }
    // The GPU backend supports writing unpremul data to a premul dst but not vice versa.
    if src_info.alpha_type() == AlphaType::Premul && dst_info.alpha_type() == AlphaType::Unpremul {
        return false;
    }
    if !src_info.color_type().is_always_opaque() && dst_info.color_type().is_always_opaque() {
        return false;
    }
    // The source has no alpha value and the dst is only alpha
    if src_info.color_type().is_always_opaque() && color_type_is_alpha_only(dst_info.color_type()) {
        return false;
    }
    true
}

// Port of: tests/WritePixelsTest.cpp#L238-L316 (chrome/m156)
fn check_write(
    reporter: &mut Reporter,
    surf: &mut Surface<'_>,
    surface_alpha_type: AlphaType,
    bitmap: &Bitmap,
    write_x: i32,
    write_y: i32,
) -> bool {
    // Can't use canvas->peekPixels(), as we are trying to look at GPU pixels sometimes as well.
    // At some point this will be unsupported, as we won't allow accessBitmap() to magically call
    // readPixels for the client.
    let mut secret_dev_bitmap = Bitmap::new();
    secret_dev_bitmap.alloc_n32_pixels((surf.width(), surf.height()), None);
    if !surf.read_pixels_to_bitmap(&mut secret_dev_bitmap, (0, 0)) {
        return false;
    }

    let canvas_row_bytes = secret_dev_bitmap.row_bytes();
    let Some(canvas_pm) = secret_dev_bitmap.peek_pixels() else {
        // nullptr == canvasPixels
        return false;
    };
    let Some(canvas_bytes) = canvas_pm.addr() else {
        return false;
    };

    if surf.width() != DEV_W || surf.height() != DEV_H {
        return false;
    }

    let bm_info = bitmap.info();

    let write_rect = IRect::from_xywh(write_x, write_y, bitmap.width(), bitmap.height());
    // `canvasPixels`, advanced by rowBytes per row.
    let mut row_start = 0_usize;
    for cy in 0..DEV_H {
        for cx in 0..DEV_W {
            #[allow(clippy::cast_sign_loss)] // cx is in [0, DEV_W)
            let o = row_start + 4 * cx as usize;
            let canvas_pixel = u32::from_ne_bytes([
                canvas_bytes[o],
                canvas_bytes[o + 1],
                canvas_bytes[o + 2],
                canvas_bytes[o + 3],
            ]);
            if write_rect.contains(IPoint::new(cx, cy)) {
                let bx = cx - write_x;
                let by = cy - write_y;
                let bmp_color8888 = get_bitmap_color(
                    bx,
                    by,
                    bitmap.width(),
                    bm_info.color_type(),
                    bm_info.alpha_type(),
                );
                let mul = AlphaType::Unpremul == bm_info.alpha_type();
                let mut bmp_pm_color =
                    convert_to_pm_color(bm_info.color_type(), bm_info.alpha_type(), bmp_color8888);
                if bm_info.alpha_type() == AlphaType::Opaque
                    || surface_alpha_type == AlphaType::Opaque
                {
                    bmp_pm_color |= 0xFF00_0000;
                }
                if !check_pixel(bmp_pm_color, canvas_pixel, mul) {
                    errorf!(
                        reporter,
                        "Expected canvas pixel at {}, {} to be 0x{:08x}, got 0x{:08x}. Write performed premul: {}",
                        cx,
                        cy,
                        bmp_pm_color,
                        canvas_pixel,
                        i32::from(mul)
                    );
                    return false;
                }
            } else {
                let test_color = get_canvas_color(cx, cy);
                if canvas_pixel != test_color {
                    errorf!(
                        reporter,
                        "Canvas pixel outside write rect at {}, {} changed. Should be 0x{:08x}, got 0x{:08x}. ",
                        cx,
                        cy,
                        test_color,
                        canvas_pixel
                    );
                    return false;
                }
            }
        }
        if cy != DEV_H - 1 {
            // `pad = canvasPixels + DEV_W`, as bytes.
            #[allow(clippy::cast_sign_loss)] // DEV_W is positive
            let pad_start = row_start + 4 * DEV_W as usize;
            #[allow(clippy::cast_sign_loss)] // DEV_W is positive
            for px in 0..canvas_row_bytes - 4 * DEV_W as usize {
                let check = canvas_bytes[pad_start + px] == DEV_PAD;
                reporter_assert!(reporter, check);
                if !check {
                    return false;
                }
            }
        }
        row_start += canvas_row_bytes;
    }

    true
}

// This is a tricky pattern, because we have to setConfig+rowBytes AND specify
// a custom pixelRef (which also has to specify its rowBytes), so we have to be
// sure that the two rowBytes match (and the infos match).
//
// Port of: tests/WritePixelsTest.cpp#L318-L327 (chrome/m156)
fn alloc_row_bytes(bm: &mut Bitmap, info: &ImageInfo, row_bytes: usize) -> bool {
    if !bm.set_info(info, row_bytes) {
        return false;
    }
    let pr = malloc_pixel_ref::make_allocate(info, row_bytes);
    bm.set_pixel_ref(pr, (0, 0));
    true
}

// Port of: tests/WritePixelsTest.cpp#L329-L342 (chrome/m156)
fn setup_bitmap(
    bm: &mut Bitmap,
    ct: ColorType,
    at: AlphaType,
    w: i32,
    h: i32,
    tight_rb: bool,
) -> bool {
    #[allow(clippy::cast_sign_loss)] // w is non-negative
    let row_bytes = if tight_rb { 0 } else { 4 * w as usize + 60 };
    let info = ImageInfo::new((w, h), ct, at, None);
    if !alloc_row_bytes(bm, &info, row_bytes) {
        return false;
    }
    for y in 0..h {
        for x in 0..w {
            bm.set_addr32(x, y, get_bitmap_color(x, y, w, ct, at));
        }
    }
    true
}

// Port of: tests/WritePixelsTest.cpp#L344-L349 (chrome/m156)
fn call_writepixels(surface: &mut Surface<'_>) {
    let info = ImageInfo::new_n32_premul((1, 1), None);
    let pixel = [0_u8; 4];
    let src = Pixmap::new_readonly(&info, &pixel, 4).expect("pixmap");
    surface.write_pixels_from_pixmap(&src, (0, 0));
}

// Port of: tests/WritePixelsTest.cpp#L351-L360 (chrome/m156)
def_tier_test!(WritePixelsSurfaceGenID, |reporter| {
    let info = ImageInfo::new_n32_premul((100, 100), None);
    let mut surface = surfaces::raster(&info, None, None).expect("surface");
    let gen_id1 = surface.generation_id();
    call_writepixels(&mut surface);
    let gen_id2 = surface.generation_id();
    reporter_assert!(reporter, gen_id1 != gen_id2);
});

// Port of: tests/WritePixelsTest.cpp#L362-L447 (chrome/m156)
fn test_write_pixels(reporter: &mut Reporter, surface: &mut Surface<'_>, surface_info: &ImageInfo) {
    let test_rects = [
        // entire thing
        IRect::from_xywh(0, 0, DEV_W, DEV_H),
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

    let g_src_configs = [
        (ColorType::RGBA8888, AlphaType::Premul),
        (ColorType::RGBA8888, AlphaType::Unpremul),
        (ColorType::RGB888x, AlphaType::Opaque),
        (ColorType::BGRA8888, AlphaType::Premul),
        (ColorType::BGRA8888, AlphaType::Unpremul),
    ];
    for rect in &test_rects {
        for tight_bmp in 0..2 {
            for &(ct, at) in &g_src_configs {
                // `canvas->recordingContext()` and `canvas->recorder()` are null for a raster
                // canvas.
                let is_gpu = false;
                fill_surface(surface);
                let mut bmp = Bitmap::new();
                reporter_assert!(
                    reporter,
                    setup_bitmap(
                        &mut bmp,
                        ct,
                        at,
                        rect.width(),
                        rect.height(),
                        tight_bmp != 0
                    )
                );
                let id_before = surface.generation_id();

                surface.write_pixels_from_bitmap(&bmp, (rect.left, rect.top));

                let id_after = surface.generation_id();
                let ok = check_write(
                    reporter,
                    surface,
                    surface_info.alpha_type(),
                    &bmp,
                    rect.left,
                    rect.top,
                );
                reporter_assert!(reporter, ok);

                // we should change the genID iff pixels were actually written.
                let canvas_rect = IRect::from_size(surface.canvas().base_layer_size());
                let write_rect = IRect::from_xywh(rect.left, rect.top, bmp.width(), bmp.height());
                let expect_success = IRect::intersects(&canvas_rect, &write_rect)
                    && write_should_succeed(surface_info, bmp.info(), is_gpu);
                reporter_assert!(reporter, expect_success == (id_before != id_after));
            }
        }
    }
}

// Port of: tests/WritePixelsTest.cpp#L449-L476 (chrome/m156)
def_tier_test!(WritePixels, |reporter| {
    let info = ImageInfo::new_n32_premul((DEV_W, DEV_H), None);
    for tight_row_bytes in [true, false] {
        #[allow(clippy::cast_sign_loss)] // DEV_W is positive
        let row_bytes = if tight_row_bytes {
            info.min_row_bytes()
        } else {
            4 * DEV_W as usize + 100
        };
        let size = info.compute_byte_size(row_bytes);
        // sk_malloc_throw; if rowBytes isn't tight then the padding is set to a known value
        let mut pixels = if tight_row_bytes {
            vec![0_u8; size]
        } else {
            vec![DEV_PAD; size]
        };
        // `free_pixels` is the buffer going out of scope; the surface works on a copy of the
        // bytes (docs/design/pixels.md).
        let mut surface =
            surfaces::wrap_pixels(&info, &mut pixels, row_bytes, None).expect("surface");
        test_write_pixels(reporter, &mut surface, &info);
    }
});

// Port of: tests/WritePixelsTest.cpp#L647-L665 (chrome/m156)
def_tier_test!(WritePixels_InvalidRowBytes, |reporter| {
    let dst_ii = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surf = surfaces::raster(&dst_ii, None, None).expect("surface");
    #[allow(clippy::cast_possible_wrap)] // LAST_ENUM is small
    for ct in 0..=(ColorType::LAST_ENUM as i32) {
        let color_type = ColorType::from_i32(ct).expect("valid color type");

        let bpp = color_type.bytes_per_pixel();
        if bpp <= 1 {
            continue;
        }
        let src_ii = dst_ii.with_color_type(color_type);
        #[allow(clippy::cast_sign_loss)] // width and height are positive
        let (bad_row_bytes, height) = (
            (surf.width() + 1) as usize * bpp - 1,
            surf.height() as usize,
        );
        let storage = vec![0_u8; bad_row_bytes * height];
        // SkSurface::writePixels doesn't report bool, SkCanvas's does.
        reporter_assert!(
            reporter,
            !surf
                .canvas()
                .write_pixels(&src_ii, &storage, bad_row_bytes, (0, 0))
        );
    }
});
