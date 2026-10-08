// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MipMapTest.cpp (chrome/m156)
//
// Not ported: `image_mip_factory` and `image_mip_mismatch` decode `images/mandrill_128.png`
// (image decoding is not ported).
//
// `SkMipmap::Build` does not run raster pipelines (and `eraseColor` is a memset), so the tests
// do not depend on the CPU tier (`def_test!`).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_1, scalar};
use skia_rust_core::size::{ISize, Size};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/MipMapTest.cpp#L35-L38 (chrome/m156)
fn make_bitmap(bm: &mut Bitmap, width: i32, height: i32) {
    bm.alloc_n32_pixels((width, height), None);
    bm.erase_color(Color::WHITE);
}

// Port of: tests/MipMapTest.cpp#L40-L82 (chrome/m156)
def_test!(MipMap, |reporter| {
    let mut bm = Bitmap::new();
    let mut rand = Random::default();

    for _ in 0..500 {
        #[allow(clippy::cast_possible_wrap)] // at most 1000
        let width = 1 + (rand.next_u() % 1000) as i32;
        #[allow(clippy::cast_possible_wrap)] // at most 1000
        let height = 1 + (rand.next_u() % 1000) as i32;
        make_bitmap(&mut bm, width, height);
        let mm = Mipmap::build(&bm.peek_pixels().expect("pixels"), true);
        reporter_assert!(reporter, mm.is_some());
        let Some(mm) = mm else {
            return;
        };

        reporter_assert!(
            reporter,
            mm.count_levels() == Mipmap::compute_level_count(width, height)
        );
        reporter_assert!(
            reporter,
            mm.extract_level(Size::new(SCALAR_1, SCALAR_1)).is_none()
        );
        reporter_assert!(
            reporter,
            mm.extract_level(Size::new(SCALAR_1 * 2.0, SCALAR_1 * 2.0))
                .is_none()
        );

        // `prevLevel` is zeroed, so its pixmap has no addr until a level is extracted.
        let mut prev_level: Option<(i32, i32)> = None;

        let mut scale: scalar = SCALAR_1;
        for _ in 0..30 {
            scale = scale * 2.0 / 3.0;

            if let Some(level) = mm.extract_level(Size::new(scale, scale)) {
                reporter_assert!(reporter, level.pixmap.addr().is_some());
                reporter_assert!(reporter, level.pixmap.width() > 0);
                reporter_assert!(reporter, level.pixmap.height() > 0);
                #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                // mirrors (int)level.fPixmap.rowBytes()
                let row_bytes = level.pixmap.row_bytes() as i32;
                reporter_assert!(reporter, row_bytes >= level.pixmap.width() * 4);

                if let Some((prev_width, prev_height)) = prev_level {
                    reporter_assert!(reporter, level.pixmap.width() <= prev_width);
                    reporter_assert!(reporter, level.pixmap.height() <= prev_height);
                }
                prev_level = Some((level.pixmap.width(), level.pixmap.height()));
            }
        }
    }
});

// Port of: tests/MipMapTest.cpp#L84-L116 (chrome/m156)
fn test_mipmap_generation(
    width: i32,
    height: i32,
    expected_mip_level_count: i32,
    reporter: &mut Reporter,
) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((width, height), None);
    bm.erase_color(Color::WHITE);
    let mm = Mipmap::build(&bm.peek_pixels().expect("pixels"), true);
    reporter_assert!(reporter, mm.is_some());
    let Some(mm) = mm else {
        return;
    };

    let mip_level_count = mm.count_levels();
    reporter_assert!(reporter, mip_level_count == expected_mip_level_count);
    reporter_assert!(
        reporter,
        mip_level_count == Mipmap::compute_level_count(width, height)
    );
    for i in 0..mip_level_count {
        let level = mm.get_level(i);
        reporter_assert!(reporter, level.is_some());
        let Some(level) = level else {
            continue;
        };
        // Make sure the mipmaps contain valid data and that the sizes are correct
        reporter_assert!(reporter, level.pixmap.addr().is_some());
        let size = Mipmap::compute_level_size(width, height, i);
        reporter_assert!(reporter, level.pixmap.width() == size.width);
        reporter_assert!(reporter, level.pixmap.height() == size.height);

        // + 1 because SkMipmap does not include the base mipmap level.
        let two_to_the_mip_level = 1 << (i + 1);
        let current_width = width / two_to_the_mip_level;
        let current_height = height / two_to_the_mip_level;
        reporter_assert!(reporter, level.pixmap.width() == current_width);
        reporter_assert!(reporter, level.pixmap.height() == current_height);
    }
}

// Port of: tests/MipMapTest.cpp#L118-L142 (chrome/m156)
def_test!(MipMap_DirectLevelAccess, |reporter| {
    // create mipmap with invalid size
    {
        // SkMipmap current requires the dimensions be greater than 2x2
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((1, 1), None);
        bm.erase_color(Color::WHITE);
        let mm = Mipmap::build(&bm.peek_pixels().expect("pixels"), true);

        reporter_assert!(reporter, mm.is_none());
    }

    // check small mipmap's count and levels
    // There should be 5 mipmap levels generated:
    // 16x16, 8x8, 4x4, 2x2, 1x1
    test_mipmap_generation(32, 32, 5, reporter);

    // check large mipmap's count and levels
    // There should be 9 mipmap levels generated:
    // 500x500, 250x250, 125x125, 62x62, 31x31, 15x15, 7x7, 3x3, 1x1
    test_mipmap_generation(1000, 1000, 9, reporter);
});

// Port of: tests/MipMapTest.cpp#L144-L148 (chrome/m156)
struct LevelCountScenario {
    width: i32,
    height: i32,
    expected_level_count: i32,
}

// Port of: tests/MipMapTest.cpp#L150-L198 (chrome/m156)
def_test!(MipMap_ComputeLevelCount, |reporter| {
    const fn s(width: i32, height: i32, expected_level_count: i32) -> LevelCountScenario {
        LevelCountScenario {
            width,
            height,
            expected_level_count,
        }
    }
    let tests = [
        // Test mipmaps with negative sizes
        s(-100, 100, 0),
        s(100, -100, 0),
        s(-100, -100, 0),
        // Test mipmaps with 0, 1, 2 as dimensions
        // (SkMipmap::Build requires a min size of 1)
        //
        // 0
        s(0, 100, 0),
        s(100, 0, 0),
        s(0, 0, 0),
        // 1
        s(1, 100, 6),
        s(100, 1, 6),
        s(1, 1, 0),
        // 2
        s(2, 100, 6),
        s(100, 2, 6),
        s(2, 2, 1),
        // Test a handful of boundaries such as 63x63 and 64x64
        s(63, 63, 5),
        s(64, 64, 6),
        s(127, 127, 6),
        s(128, 128, 7),
        s(255, 255, 7),
        s(256, 256, 8),
        // Test different dimensions, such as 256x64
        s(64, 129, 7),
        s(255, 32, 7),
        s(500, 1000, 9),
    ];

    for current_test in &tests {
        let level_count = Mipmap::compute_level_count(current_test.width, current_test.height);
        reporter_assert!(reporter, current_test.expected_level_count == level_count);
    }
});

// Port of: tests/MipMapTest.cpp#L200-L205 (chrome/m156)
struct LevelSizeScenario {
    base_width: i32,
    base_height: i32,
    level: i32,
    expected_mip_map_level_size: ISize,
}

// Port of: tests/MipMapTest.cpp#L207-L248 (chrome/m156)
def_test!(MipMap_ComputeLevelSize, |reporter| {
    fn s(base_width: i32, base_height: i32, level: i32, w: i32, h: i32) -> LevelSizeScenario {
        LevelSizeScenario {
            base_width,
            base_height,
            level,
            expected_mip_map_level_size: ISize::new(w, h),
        }
    }
    let tests = [
        // Test mipmaps with negative sizes
        s(-100, 100, 0, 0, 0),
        s(100, -100, 0, 0, 0),
        s(-100, -100, 0, 0, 0),
        // Test mipmaps with 0, 1, 2 as dimensions
        // (SkMipmap::Build requires a min size of 1)
        //
        // 0
        s(0, 100, 0, 0, 0),
        s(100, 0, 0, 0, 0),
        s(0, 0, 0, 0, 0),
        // 1
        s(1, 100, 0, 1, 50),
        s(100, 1, 0, 50, 1),
        s(1, 1, 0, 0, 0),
        // 2
        s(2, 100, 0, 1, 50),
        s(100, 2, 1, 25, 1),
        s(2, 2, 0, 1, 1),
        // Test a handful of cases
        s(63, 63, 2, 7, 7),
        s(64, 64, 2, 8, 8),
        s(127, 127, 2, 15, 15),
        s(64, 129, 3, 4, 8),
        s(255, 32, 6, 1, 1),
        s(500, 1000, 1, 125, 250),
    ];

    for current_test in &tests {
        let level_size = Mipmap::compute_level_size(
            current_test.base_width,
            current_test.base_height,
            current_test.level,
        );
        reporter_assert!(
            reporter,
            current_test.expected_mip_map_level_size == level_size
        );
    }
});

// Port of: tests/MipMapTest.cpp#L250-L255 (chrome/m156)
def_test!(MipMap_F16, |_reporter| {
    let mut bmp = Bitmap::new();
    bmp.alloc_pixels_info(
        &ImageInfo::new((10, 10), ColorType::RGBAF16, AlphaType::Premul, None),
        None,
    );
    bmp.erase_color(Color::new(0));
    let _mipmap = Mipmap::build(&bmp.peek_pixels().expect("pixels"), true);
});
