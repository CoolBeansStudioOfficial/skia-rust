// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/WritePixelsBench.cpp

//! `WritePixelsBench`: times `SkCanvas::writePixels` of a base-layer-sized black bitmap, for each
//! combination of colour type, alpha type and colour space (a rendering bench). The combinations
//! can take fast or slow paths in the implementation.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::def_bench;
use crate::prelude::*;

/// `class WritePixelsBench`.
// Port of: bench/WritePixelsBench.cpp#L18-L56 (chrome/m156)
struct WritePixelsBench {
    color_type: ColorType,
    alpha_type: AlphaType,
    cs: Option<ColorSpace>,
    name: String,
}

impl WritePixelsBench {
    // WritePixelsBench(SkColorType ct, SkAlphaType at, sk_sp<SkColorSpace> cs)
    // Port of: bench/WritePixelsBench.cpp#L18-L56 (chrome/m156)
    fn new(ct: ColorType, at: AlphaType, cs: Option<ColorSpace>) -> Self {
        // fName.printf("writepix_%s_%s_%s", at == kPremul ? "pm" : "um",
        //              ct == kRGBA_8888 ? "rgba" : "bgra", cs ? "srgb" : "null");
        let name = format!(
            "writepix_{}_{}_{}",
            if at == AlphaType::Premul { "pm" } else { "um" },
            if ct == ColorType::RGBA8888 {
                "rgba"
            } else {
                "bgra"
            },
            if cs.is_some() { "srgb" } else { "null" }
        );
        Self {
            color_type: ct,
            alpha_type: at,
            cs,
            name,
        }
    }
}

impl Benchmark for WritePixelsBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/WritePixelsBench.cpp#L18-L56 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("WritePixelsBench is a rendering bench");
        // SkISize size = canvas->getBaseLayerSize();
        let size = canvas.base_layer_size();
        // SkImageInfo info = SkImageInfo::Make(size, fColorType, fAlphaType, fCS);
        let info = ImageInfo::new(size, self.color_type, self.alpha_type, self.cs.clone());
        // SkBitmap bmp;
        let mut bmp = Bitmap::new();
        // bmp.allocPixels(info);
        bmp.alloc_pixels_info(&info, None);
        // bmp.eraseColor(SK_ColorBLACK);
        bmp.erase_color(Color::new(0xFF00_0000));
        // for (int loop = 0; loop < loops; ++loop) {
        for _ in 0..loops {
            // canvas->writePixels(info, bmp.getPixels(), bmp.rowBytes(), 0, 0);
            let _ = canvas.write_pixels_from_bitmap(&bmp, (0, 0));
        }
    }
}

// Port of: bench/WritePixelsBench.cpp#L60-L60 (chrome/m156)
def_bench!(
    write_pixels_bench_rgba_premul_null =
        "WritePixelsBench(kRGBA_8888_SkColorType, kPremul_SkAlphaType, nullptr)",
    WritePixelsBench::new(ColorType::RGBA8888, AlphaType::Premul, None)
);
// Port of: bench/WritePixelsBench.cpp#L61-L61 (chrome/m156)
def_bench!(
    write_pixels_bench_rgba_unpremul_null =
        "WritePixelsBench(kRGBA_8888_SkColorType, kUnpremul_SkAlphaType, nullptr)",
    WritePixelsBench::new(ColorType::RGBA8888, AlphaType::Unpremul, None)
);
// Port of: bench/WritePixelsBench.cpp#L62-L62 (chrome/m156)
def_bench!(
    write_pixels_bench_rgba_premul_srgb =
        "WritePixelsBench(kRGBA_8888_SkColorType, kPremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    WritePixelsBench::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb())
    )
);
// Port of: bench/WritePixelsBench.cpp#L63-L63 (chrome/m156)
def_bench!(
    write_pixels_bench_rgba_unpremul_srgb =
        "WritePixelsBench(kRGBA_8888_SkColorType, kUnpremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    WritePixelsBench::new(
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb())
    )
);
// Port of: bench/WritePixelsBench.cpp#L65-L65 (chrome/m156)
def_bench!(
    write_pixels_bench_bgra_premul_null =
        "WritePixelsBench(kBGRA_8888_SkColorType, kPremul_SkAlphaType, nullptr)",
    WritePixelsBench::new(ColorType::BGRA8888, AlphaType::Premul, None)
);
// Port of: bench/WritePixelsBench.cpp#L66-L66 (chrome/m156)
def_bench!(
    write_pixels_bench_bgra_unpremul_null =
        "WritePixelsBench(kBGRA_8888_SkColorType, kUnpremul_SkAlphaType, nullptr)",
    WritePixelsBench::new(ColorType::BGRA8888, AlphaType::Unpremul, None)
);
// Port of: bench/WritePixelsBench.cpp#L67-L67 (chrome/m156)
def_bench!(
    write_pixels_bench_bgra_premul_srgb =
        "WritePixelsBench(kBGRA_8888_SkColorType, kPremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    WritePixelsBench::new(
        ColorType::BGRA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb())
    )
);
// Port of: bench/WritePixelsBench.cpp#L68-L68 (chrome/m156)
def_bench!(
    write_pixels_bench_bgra_unpremul_srgb =
        "WritePixelsBench(kBGRA_8888_SkColorType, kUnpremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    WritePixelsBench::new(
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb())
    )
);
