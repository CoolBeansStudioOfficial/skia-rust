// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PremulAlphaRoundTripTest.cpp (chrome/m156)
//
// Not ported: `PremulAlphaRoundTrip_Gpu` and `PremulAlphaRoundTripGrConvertPixels` (Ganesh).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{def_tier_test, reporter_assert};

// Port of: tests/PremulAlphaRoundTripTest.cpp#L32-L40 (chrome/m156)
fn pack_unpremul_rgba(c: Color) -> u32 {
    u32::from_ne_bytes([c.r(), c.g(), c.b(), c.a()])
}

// Port of: tests/PremulAlphaRoundTripTest.cpp#L42-L50 (chrome/m156)
fn pack_unpremul_bgra(c: Color) -> u32 {
    u32::from_ne_bytes([c.b(), c.g(), c.r(), c.a()])
}

type PackUnpremulProc = fn(Color) -> u32;

// Port of: tests/PremulAlphaRoundTripTest.cpp#L52-L60 (chrome/m156)
struct GUnpremul {
    color_type: ColorType,
    pack_proc: PackUnpremulProc,
}
const G_UNPREMUL: [GUnpremul; 2] = [
    GUnpremul {
        color_type: ColorType::RGBA8888,
        pack_proc: pack_unpremul_rgba,
    },
    GUnpremul {
        color_type: ColorType::BGRA8888,
        pack_proc: pack_unpremul_bgra,
    },
];

// Port of: tests/PremulAlphaRoundTripTest.cpp#L62-L77 (chrome/m156)
fn fill_surface(surf: &mut Surface<'_>, color_type: ColorType, proc: PackUnpremulProc) {
    // Don't strictly need a bitmap, but its a handy way to allocate the pixels
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((256, 256), None);

    for a in 0..256_i32 {
        for r in 0..256_i32 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a, r < 256
            let c = Color::from_argb(a as u8, r as u8, 0, 0);
            bmp.set_addr32(r, a, proc(c));
        }
    }

    let info = ImageInfo::new(bmp.dimensions(), color_type, AlphaType::Unpremul, None);
    let pm = bmp.peek_pixels().expect("allocated");
    let src = Pixmap::new_readonly(&info, pm.addr().expect("has pixels"), bmp.row_bytes())
        .expect("valid pixmap");
    surf.write_pixels_from_pixmap(&src, (0, 0));
}

// Port of: tests/PremulAlphaRoundTripTest.cpp#L79-L110 (chrome/m156)
fn test_premul_alpha_roundtrip(reporter: &mut crate::Reporter, surf: &mut Surface<'_>) {
    for upma in &G_UNPREMUL {
        fill_surface(surf, upma.color_type, upma.pack_proc);

        let info = ImageInfo::new((256, 256), upma.color_type, AlphaType::Unpremul, None);
        let mut read_bmp1 = Bitmap::new();
        read_bmp1.alloc_pixels_info(&info, None);
        let mut read_bmp2 = Bitmap::new();
        read_bmp2.alloc_pixels_info(&info, None);

        read_bmp1.erase_color(0);
        read_bmp2.erase_color(0);

        let _ = surf.read_pixels_to_bitmap(&mut read_bmp1, (0, 0));
        surf.write_pixels_from_bitmap(&read_bmp1, (0, 0));
        let _ = surf.read_pixels_to_bitmap(&mut read_bmp2, (0, 0));

        let mut success = true;
        let mut y = 0;
        while y < 256 && success {
            let mut x = 0;
            while x < 256 && success {
                let p1 = read_bmp1.get_addr32(x, y);
                let p2 = read_bmp2.get_addr32(x, y);
                // We see sporadic failures here. May help to see where it goes wrong.
                if p1 != p2 {
                    eprintln!("{p1:x} != {p2:x}, x = {x}, y = {y}");
                }
                success = p1 == p2;
                reporter_assert!(reporter, success);
                x += 1;
            }
            y += 1;
        }
    }
}

// Port of: tests/PremulAlphaRoundTripTest.cpp#L112-L118 (chrome/m156)
def_tier_test!(PremulAlphaRoundTrip, |reporter| {
    let info = ImageInfo::new_n32_premul((256, 256), None);

    let mut surf = surfaces::raster(&info, None, None).expect("surface");

    test_premul_alpha_roundtrip(reporter, &mut surf);
});

// `*bm.getAddr32(x, y)`.
fn get_addr32(bm: &Bitmap, x: i32, y: i32) -> u32 {
    bm.get_addr32(x, y)
}

// Port of: tests/PremulAlphaRoundTripTest.cpp#L191-L246 (chrome/m156)
def_tier_test!(PremulAlphaRoundTripSkConvertPixels, |reporter| {
    // ... and now using SkConvertPixels, just for completeness
    let upm_info = ImageInfo::new((256, 256), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let pm_info = ImageInfo::new((256, 256), ColorType::RGBA8888, AlphaType::Premul, None);

    let mut src = Bitmap::new();
    src.alloc_pixels_info(&upm_info, None);
    // uint32_t* srcPixels = src.getAddr32(0, 0);
    for y in 0..256 {
        for x in 0..256 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // x, y < 256
            let (xb, yb) = (x as u8, y as u8);
            src.set_addr32(x, y, pack_unpremul_rgba(Color::from_argb(yb, xb, xb, xb)));
        }
    }

    let convert = |dst: &mut Bitmap, src: &Bitmap| {
        let (dst_info, dst_rb) = (dst.info().clone(), dst.row_bytes());
        let mut dst_pm = dst.peek_pixels_mut().expect("allocated");
        let src_pm = src.peek_pixels().expect("allocated");
        // SkAssertResult(SkConvertPixels(...))
        assert!(convert_pixels(
            &dst_info,
            dst_pm.writable_addr().expect("writable"),
            dst_rb,
            src.info(),
            src_pm.addr().expect("has pixels"),
            src.row_bytes(),
        ));
    };

    let mut surf = Bitmap::new();
    surf.alloc_pixels_info(&pm_info, None);
    convert(&mut surf, &src);

    let mut read1 = Bitmap::new();
    read1.alloc_pixels_info(&upm_info, None);
    convert(&mut read1, &surf);

    let mut surf2 = Bitmap::new();
    surf2.alloc_pixels_info(&pm_info, None);
    convert(&mut surf2, &read1);

    let mut read2 = Bitmap::new();
    read2.alloc_pixels_info(&upm_info, None);
    convert(&mut read2, &surf2);

    let dump_pixel_history = |x: i32, y: i32| {
        eprintln!("Pixel history for ({x}, {y}):");
        eprintln!("Src : {:08x}", get_addr32(&src, x, y));
        eprintln!(" -> : {:08x}", get_addr32(&surf, x, y));
        eprintln!(" <- : {:08x}", get_addr32(&read1, x, y));
        eprintln!(" -> : {:08x}", get_addr32(&surf2, x, y));
        eprintln!(" <- : {:08x}", get_addr32(&read2, x, y));
    };

    let mut success = true;
    let mut y = 0;
    while y < 256 && success {
        let mut x = 0;
        while x < 256 && success {
            let c1 = get_addr32(&read1, x, y);
            let c2 = get_addr32(&read2, x, y);
            // If this ever fails, it's helpful to see where it goes wrong.
            if c1 != c2 {
                dump_pixel_history(x, y);
            }
            success = c1 == c2;
            reporter_assert!(reporter, success);
            x += 1;
        }
        y += 1;
    }
});
