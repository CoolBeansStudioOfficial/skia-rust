// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PremulAlphaRoundTripTest.cpp (chrome/m156)
//
// Not ported: `PremulAlphaRoundTrip` (needs a raster `SkSurface`, task D6),
// `PremulAlphaRoundTrip_Gpu` and `PremulAlphaRoundTripGrConvertPixels` (Ganesh).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::image_info::ImageInfo;

use crate::{def_tier_test, reporter_assert};

// Port of: tests/PremulAlphaRoundTripTest.cpp#L32-L40 (chrome/m156)
fn pack_unpremul_rgba(c: Color) -> u32 {
    u32::from_ne_bytes([c.r(), c.g(), c.b(), c.a()])
}

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
