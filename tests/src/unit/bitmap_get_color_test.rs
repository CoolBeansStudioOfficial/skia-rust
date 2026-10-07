// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BitmapGetColorTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;

use crate::{def_test, reporter_assert};

// Port of: tests/BitmapGetColorTest.cpp#L21-L58 (chrome/m156)
def_test!(GetColor, |reporter| {
    struct Rec {
        color_type: ColorType,
        in_color: Color,
        out_color: Color,
    }

    let g_rec = [
        // todo: add some tests that involve alpha, so we exercise the
        // unpremultiply aspect of getColor()
        Rec {
            color_type: ColorType::Alpha8,
            in_color: Color::new(0xFF00_0000),
            out_color: Color::new(0xFF00_0000),
        },
        Rec {
            color_type: ColorType::Alpha8,
            in_color: Color::new(0),
            out_color: Color::new(0),
        },
        Rec {
            color_type: ColorType::RGB565,
            in_color: Color::new(0xFF00_FF00),
            out_color: Color::new(0xFF00_FF00),
        },
        Rec {
            color_type: ColorType::RGB565,
            in_color: Color::new(0xFFFF_00FF),
            out_color: Color::new(0xFFFF_00FF),
        },
        Rec {
            color_type: ColorType::N32,
            in_color: Color::new(0xFFFF_FFFF),
            out_color: Color::new(0xFFFF_FFFF),
        },
        Rec {
            color_type: ColorType::N32,
            in_color: Color::new(0),
            out_color: Color::new(0),
        },
        Rec {
            color_type: ColorType::N32,
            in_color: Color::new(0xFF22_4466),
            out_color: Color::new(0xFF22_4466),
        },
    ];

    // specify an area that doesn't touch (0,0) and may extend beyond the
    // bitmap bounds (to test that we catch that in eraseArea
    let init_color = Color::new(0xFF00_00FF);
    let area = IRect::new(1, 1, 3, 3);

    for rec in &g_rec {
        let info = ImageInfo::new((2, 2), rec.color_type, AlphaType::Premul, None);
        let mut bm = Bitmap::new();
        // `uint32_t storage[4]`
        let storage = vec![0u8; 4 * std::mem::size_of::<u32>()];
        let _ = bm.install_pixels(&info, storage, info.min_row_bytes());

        bm.erase_color(init_color);
        bm.erase_area(area, rec.in_color);

        let c = bm.get_color((1, 1));
        reporter_assert!(reporter, c == rec.out_color);
    }
});
