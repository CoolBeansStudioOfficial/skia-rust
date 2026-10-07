// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BitmapTest.cpp (chrome/m156)
//
// Not ported (they need `SkBitmap::readPixels` / `SkConvertPixels`, which are not ported yet):
// `Bitmap_setColorSpace`, `Bitmap_getColor_Swizzle` (`ToolUtils::copy_to`) and `getalphaf`.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::malloc_pixel_ref;
use skia_rust_core::random::Random;

use crate::{Reporter, def_test, infof, reporter_assert};

// Port of: tests/BitmapTest.cpp#L31-L53 (chrome/m156)
fn test_peekpixels(reporter: &mut Reporter) {
    let info = ImageInfo::new_n32_premul((10, 10), None);

    let mut bm = Bitmap::new();

    // empty should return false
    // (`peekPixels(nullptr)` and `peekPixels(&pmap)` are the same call here)
    reporter_assert!(reporter, bm.peek_pixels().is_none());
    reporter_assert!(reporter, bm.peek_pixels().is_none());

    // no pixels should return false
    let _ = bm.set_info(&ImageInfo::new_n32_premul((10, 10), None), None);
    reporter_assert!(reporter, bm.peek_pixels().is_none());
    reporter_assert!(reporter, bm.peek_pixels().is_none());

    // real pixels should return true
    bm.alloc_pixels_info(&info, None);
    reporter_assert!(reporter, bm.peek_pixels().is_some());
    let pmap = bm.peek_pixels();
    reporter_assert!(reporter, pmap.is_some());
    let pmap = pmap.unwrap();
    reporter_assert!(reporter, pmap.info() == bm.info());
    // `pmap.addr() == bm.getPixels()`: the same pixel memory.
    reporter_assert!(
        reporter,
        pmap.addr().unwrap().as_ptr() == bm.pixmap().addr().unwrap().as_ptr()
    );
    reporter_assert!(reporter, pmap.row_bytes() == bm.row_bytes());
}

// https://code.google.com/p/chromium/issues/detail?id=446164
// Port of: tests/BitmapTest.cpp#L55-L66 (chrome/m156)
fn test_bigalloc(reporter: &mut Reporter) {
    let width = 0x4000_0001;
    let height = 0x0000_0096;
    let info = ImageInfo::new_n32_premul((width, height), None);

    let mut bm = Bitmap::new();
    reporter_assert!(reporter, !bm.try_alloc_pixels_info(&info, None));

    let pr = malloc_pixel_ref::make_allocate(&info, info.min_row_bytes());
    reporter_assert!(reporter, pr.is_none());
}

// Port of: tests/BitmapTest.cpp#L68-L140 (chrome/m156)
fn test_allocpixels(reporter: &mut Reporter) {
    let width = 10;
    let height = 10;
    let info = ImageInfo::new_n32_premul((width, height), None);
    let explicit_row_bytes = info.min_row_bytes() + 24;

    let mut bm = Bitmap::new();
    let _ = bm.set_info(&info, None);
    reporter_assert!(reporter, info.min_row_bytes() == bm.row_bytes());
    bm.alloc_pixels();
    reporter_assert!(reporter, info.min_row_bytes() == bm.row_bytes());
    bm.reset();
    bm.alloc_pixels_info(&info, None);
    reporter_assert!(reporter, info.min_row_bytes() == bm.row_bytes());

    let _ = bm.set_info(&info, explicit_row_bytes);
    reporter_assert!(reporter, explicit_row_bytes == bm.row_bytes());
    bm.alloc_pixels();
    reporter_assert!(reporter, explicit_row_bytes == bm.row_bytes());
    bm.reset();
    bm.alloc_pixels_info(&info, explicit_row_bytes);
    reporter_assert!(reporter, explicit_row_bytes == bm.row_bytes());

    bm.reset();
    let _ = bm.set_info(&info, 0usize);
    reporter_assert!(reporter, info.min_row_bytes() == bm.row_bytes());
    bm.reset();
    bm.alloc_pixels_info(&info, 0usize);
    reporter_assert!(reporter, info.min_row_bytes() == bm.row_bytes());

    bm.reset();
    let mut success = bm.set_info(&info, info.min_row_bytes() - 1); // invalid for 32bit
    reporter_assert!(reporter, !success);
    reporter_assert!(reporter, bm.is_null());

    for ct in [
        ColorType::Alpha8,
        ColorType::RGB565,
        ColorType::ARGB4444,
        ColorType::RGBA8888,
        ColorType::BGRA8888,
        ColorType::RGB888x,
        ColorType::RGBA1010102,
        ColorType::RGB101010x,
        ColorType::Gray8,
        ColorType::RGBAF16Norm,
        ColorType::RGBAF16,
        ColorType::RGBAF32,
        ColorType::R8G8UNorm,
        ColorType::A16UNorm,
        ColorType::R16UNorm,
        ColorType::R16G16UNorm,
        ColorType::A16Float,
        ColorType::R16Float,
        ColorType::R16G16Float,
        ColorType::R16G16B16A16UNorm,
    ] {
        let image_info = info.with_color_type(ct);
        for row_bytes_padding in 1..=17usize {
            bm.reset();
            success = bm.set_info(&image_info, image_info.min_row_bytes() + row_bytes_padding);
            if row_bytes_padding % image_info.bytes_per_pixel() == 0 {
                reporter_assert!(reporter, success);
                success = bm.try_alloc_pixels();
                reporter_assert!(reporter, success);
            } else {
                // Not pixel aligned.
                reporter_assert!(reporter, !success);
                reporter_assert!(reporter, bm.is_null());
            }
        }
    }
}

// Port of: tests/BitmapTest.cpp#L142-L157 (chrome/m156)
fn test_bigwidth(reporter: &mut Reporter) {
    let mut bm = Bitmap::new();
    let width = 1 << 29; // *4 will be the high-bit of 32bit int

    let info = ImageInfo::new_a8((width, 1));
    reporter_assert!(reporter, bm.set_info(&info, None));
    reporter_assert!(
        reporter,
        bm.set_info(&info.with_color_type(ColorType::RGB565), None)
    );

    // for a 4-byte config, this width will compute a rowbytes of 0x80000000,
    // which does not fit in a int32_t. setConfig should detect this, and fail.

    // TODO: perhaps skia can relax this, and only require that rowBytes fit
    //       in a uint32_t (or larger), but for now this is the constraint.

    reporter_assert!(
        reporter,
        !bm.set_info(&info.with_color_type(ColorType::N32), None)
    );
}

// Port of: tests/BitmapTest.cpp#L159-L177 (chrome/m156)
def_test!(Bitmap, |reporter| {
    // Zero-sized bitmaps are allowed
    for width in 0..2 {
        for height in 0..2 {
            let mut bm = Bitmap::new();
            let set_conf = bm.set_info(&ImageInfo::new_n32_premul((width, height), None), None);
            reporter_assert!(reporter, set_conf);
            if set_conf {
                bm.alloc_pixels();
            }
            reporter_assert!(reporter, ((width & height) != 0) != bm.is_empty());
        }
    }

    test_bigwidth(reporter);
    test_allocpixels(reporter);
    test_bigalloc(reporter);
    test_peekpixels(reporter);
});

// Port of: tests/BitmapTest.cpp#L245-L252 (chrome/m156)
fn test_erasecolor_premul(reporter: &mut Reporter, ct: ColorType, input: Color, expected: Color) {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ImageInfo::new((1, 1), ct, AlphaType::Premul, None), None);
    bm.erase_color(input);
    infof!(
        reporter,
        "expected: {:x} actual: {:x}\n",
        u32::from(expected),
        u32::from(bm.get_color((0, 0)))
    );
    reporter_assert!(reporter, bm.get_color((0, 0)) == expected);
}

// This test checks that eraseColor premultiplies the color correctly.
// Port of: tests/BitmapTest.cpp#L254-L264 (chrome/m156)
def_test!(Bitmap_eraseColor_Premul, |r| {
    let color = Color::new(0x80FF_0080);
    test_erasecolor_premul(r, ColorType::Alpha8, color, Color::new(0x8000_0000));
    test_erasecolor_premul(r, ColorType::RGB565, color, Color::new(0xFF84_0042));
    test_erasecolor_premul(r, ColorType::ARGB4444, color, Color::new(0x88FF_0080));
    test_erasecolor_premul(r, ColorType::RGBA8888, color, color);
    test_erasecolor_premul(r, ColorType::BGRA8888, color, color);
});

// Test that SkBitmap::ComputeOpaque() is correct for various colortypes.
// Port of: tests/BitmapTest.cpp#L266-L282 (chrome/m156)
def_test!(Bitmap_compute_is_opaque, |r| {
    for i in 1..=ColorType::LAST_ENUM as i32 {
        let ct = ColorType::from_i32(i).unwrap();
        let mut bm = Bitmap::new();
        let at = if ct.is_always_opaque() {
            AlphaType::Opaque
        } else {
            AlphaType::Premul
        };
        bm.alloc_pixels_info(&ImageInfo::new((13, 17), ct, at, None), None);
        bm.erase_color(Color::from_argb(255, 10, 20, 30));
        reporter_assert!(r, Bitmap::compute_is_opaque(&bm));

        bm.erase_color(Color::from_argb(128, 255, 255, 255));
        let is_opaque = Bitmap::compute_is_opaque(&bm);
        let should_be_opaque = at == AlphaType::Opaque;
        reporter_assert!(r, is_opaque == should_be_opaque);
    }
});

// Test that erase+getColor round trips with RGBA_F16 pixels.
// Port of: tests/BitmapTest.cpp#L284-L305 (chrome/m156)
def_test!(Bitmap_erase_f16_erase_getColor, |r| {
    let mut random = Random::default();
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(
        &ImageInfo::new((1, 1), ColorType::RGBAF16, AlphaType::Premul, None),
        None,
    );
    // skia-rust: `SkPixmap::erase` writes through a `const_cast`; the pixmap that can write is
    // `peek_pixels_mut`.
    let pm = bm.peek_pixels_mut();
    reporter_assert!(r, pm.is_some());
    let mut pm = pm.unwrap();
    for i in 0..0x100u32 {
        // Test all possible values of blue component.
        let color1 = Color::new((random.next_u() & 0xFFFF_FF00) | i);
        // Test all possible values of alpha component.
        let color2 = Color::new((random.next_u() & 0x00FF_FFFF) | (i << 24));
        for color in [color1, color2] {
            pm.erase(color, None);
            if color.a() != 0 {
                reporter_assert!(r, color == pm.get_color((0, 0)));
            } else {
                reporter_assert!(r, 0 == pm.get_color((0, 0)).a());
            }
        }
    }
});

// Verify that SkBitmap::erase erases in SRGB, regardless of the SkColorSpace of the
// SkBitmap.
// Port of: tests/BitmapTest.cpp#L307-L319 (chrome/m156)
def_test!(Bitmap_erase_srgb, |r| {
    let mut bm = Bitmap::new();
    // Use a color spin from SRGB.
    bm.alloc_pixels_info(
        &ImageInfo::new(
            (1, 1),
            ColorType::N32,
            AlphaType::Premul,
            ColorSpace::new_srgb().with_color_spin(),
        ),
        None,
    );
    // RED will be converted into the spun color space.
    bm.erase_color(Color::new(0xFFFF_0000)); // SK_ColorRED
    // getColor doesn't take the color space into account, so the returned color
    // is different due to the color spin.
    reporter_assert!(r, bm.get_color((0, 0)) == Color::new(0xFF00_00FF)); // SK_ColorBLUE
});

// Make sure that the bitmap remains valid when pixelref is removed.
// Port of: tests/BitmapTest.cpp#L321-L327 (chrome/m156)
def_test!(Bitmap_clear_pixelref_keep_info, |r| {
    let _ = r;
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ImageInfo::new_n32_premul((100, 100), None), None);
    bm.set_pixel_ref(None, (0, 0));
    bm.validate();
});

// At the time of writing, SkBitmap::erase() works when the color is zero for all formats,
// but some formats failed when the color is non-zero!
// Port of: tests/BitmapTest.cpp#L329-L359 (chrome/m156)
def_test!(Bitmap_erase, |r| {
    let color_types = [
        ColorType::RGB565,
        ColorType::ARGB4444,
        ColorType::RGB888x,
        ColorType::RGBA8888,
        ColorType::BGRA8888,
        ColorType::RGB101010x,
        ColorType::RGBA1010102,
    ];

    for ct in color_types {
        let info = ImageInfo::new((1, 1), ct, AlphaType::Premul, None);

        let mut bm = Bitmap::new();
        bm.alloc_pixels_info(&info, None);

        bm.erase_color(Color::new(0x0000_0000));
        if ct.is_always_opaque() {
            reporter_assert!(r, bm.get_color((0, 0)) == Color::new(0xff00_0000));
        } else {
            reporter_assert!(r, bm.get_color((0, 0)) == Color::new(0x0000_0000));
        }

        bm.erase_color(Color::new(0xaabb_ccdd));
        reporter_assert!(r, bm.get_color((0, 0)) != Color::new(0xff00_0000));
        reporter_assert!(r, bm.get_color((0, 0)) != Color::new(0x0000_0000));
    }
});

/*  computeByteSize() is documented to return 0 if height is zero, but does not
 *  special-case width==0, so computeByteSize() can return non-zero for that
 *  (since it is defined to return (height-1)*rb + ...
 *
 *  Test that allocPixels() respects this, and allocates a buffer as large as
 *  computeByteSize()... even though the bitmap is logicallly empty.
 */
// Port of: tests/BitmapTest.cpp#L470-L503 (chrome/m156)
def_test!(bitmap_zerowidth_crbug_1103827, |reporter| {
    struct Rec {
        width: i32,
        height: i32,
        rowbytes: usize,
        expected_size: usize,
    }

    let big_rb: usize = 1 << 16;
    let rec = [
        Rec {
            width: 2,
            height: 0,
            rowbytes: big_rb,
            expected_size: 0,
        }, // zero-height means zero-size
        Rec {
            width: 0,
            height: 2,
            rowbytes: big_rb,
            expected_size: big_rb,
        }, // zero-width is computed normally
    ];

    for r in &rec {
        let info = ImageInfo::new(
            (r.width, r.height),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let size = info.compute_byte_size(r.rowbytes);
        reporter_assert!(reporter, size == r.expected_size);

        let mut bm = Bitmap::new();
        let _ = bm.set_info(&info, r.rowbytes);
        reporter_assert!(reporter, size == bm.compute_byte_size());

        // Be sure we can actually write to that much memory. If the bitmap underallocated
        // the buffer, this should trash memory and crash (we hope).
        bm.alloc_pixels();
        // sk_bzero(bm.getPixels(), size)
        let mut pixels = bm.peek_pixels_mut().unwrap();
        pixels.writable_addr().unwrap()[..size].fill(0);
    }
});
