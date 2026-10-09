// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ReadPixBench.cpp

//! `ReadPixBench`: `readPixels` from a canvas into a bitmap, per color type, alpha type and color
//! space (rendering). `GetAlphafBench`: `getAlphaf` over every pixel of a 1024 x 1024 bitmap
//! (non-rendering). `PixmapOrientBench` needs `SkPixmapUtils::Orient` (codec include), not
//! ported, so it is not registered.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::def_bench;
use crate::prelude::*;

/// `class ReadPixBench`.
// Port of: bench/ReadPixBench.cpp#L18-L54 (chrome/m156)
struct ReadPixBench {
    ct: ColorType,
    at: AlphaType,
    cs: Option<ColorSpace>,
    /// `fName`.
    name: String,
}

impl ReadPixBench {
    // Port of: bench/ReadPixBench.cpp#L18-L54 (chrome/m156)
    fn new(ct: ColorType, at: AlphaType, cs: Option<ColorSpace>) -> Self {
        // fName.printf("readpix_%s_%s_%s", ...);
        let name = format!(
            "readpix_{}_{}_{}",
            if at == AlphaType::Premul { "pm" } else { "um" },
            if ct == ColorType::RGBA8888 {
                "rgba"
            } else {
                "bgra"
            },
            if cs.is_some() { "srgb" } else { "null" },
        );
        Self { ct, at, cs, name }
    }
}

impl Benchmark for ReadPixBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        // Rendering: the default (every backend but kNonRendering).
        backend != Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ReadPixBench.cpp#L18-L54 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ReadPixBench is a rendering bench");
        // canvas->clear(0x80000000);
        canvas.clear(Color::new(0x8000_0000));
        // SkISize size = canvas->getBaseLayerSize();
        let size = canvas.base_layer_size();
        // auto info = SkImageInfo::Make(size, fCT, fAT, fCS);
        let info = ImageInfo::new(size, self.ct, self.at, self.cs.clone());
        // SkBitmap bitmap; bitmap.allocPixels(info);
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(&info, None);
        for _ in 0..loops {
            // canvas->readPixels(bitmap.info(), bitmap.getPixels(), bitmap.rowBytes(), 0, 0);
            let _ = canvas.read_pixels_to_bitmap(&mut bitmap, (0, 0));
        }
    }
}

/// `class GetAlphafBench`.
// Port of: bench/ReadPixBench.cpp#L104-L140 (chrome/m156)
struct GetAlphafBench {
    ct: ColorType,
    /// `fName`.
    name: String,
    /// `fBM`, allocated in `on_delayed_setup`.
    bm: Bitmap,
}

impl GetAlphafBench {
    // Port of: bench/ReadPixBench.cpp#L104-L140 (chrome/m156)
    fn new(ct: ColorType, label: &str) -> Self {
        Self {
            ct,
            // fName.printf("getalphaf_%s", label);
            name: format!("getalphaf_{label}"),
            bm: Bitmap::new(),
        }
    }
}

impl Benchmark for GetAlphafBench {
    // Port of: bench/ReadPixBench.cpp#L104-L140 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ReadPixBench.cpp#L104-L140 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // fBM.allocPixels(SkImageInfo::Make(1024, 1024, fCT, kPremul_SkAlphaType));
        self.bm.alloc_pixels_info(
            &ImageInfo::new((1024, 1024), self.ct, AlphaType::Premul, None),
            None,
        );
        // fBM.eraseColor(0x88112233);
        self.bm.erase_color(Color::new(0x8811_2233));
    }

    // Port of: bench/ReadPixBench.cpp#L104-L140 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for y in 0..self.bm.height() {
                for x in 0..self.bm.width() {
                    // fBM.getAlphaf(x, y);
                    let _ = self.bm.get_alpha_f((x, y));
                }
            }
        }
    }
}

/// `DEF_BENCH(return new ReadPixBench(ct, at, cs);)`, with `cs` as `SkColorSpace::MakeSRGB()` or
/// `nullptr`.
macro_rules! def_read_pix_bench {
    ($test:ident = $name:literal, $ct:expr, $at:expr, $cs:expr) => {
        def_bench!($test = $name, ReadPixBench::new($ct, $at, $cs));
    };
}

// Port of: bench/ReadPixBench.cpp#L55-L63 (chrome/m156)
def_read_pix_bench!(
    read_pix_rgba_premul_null =
        "ReadPixBench(kRGBA_8888_SkColorType, kPremul_SkAlphaType, nullptr)",
    ColorType::RGBA8888,
    AlphaType::Premul,
    None
);
def_read_pix_bench!(
    read_pix_rgba_unpremul_null =
        "ReadPixBench(kRGBA_8888_SkColorType, kUnpremul_SkAlphaType, nullptr)",
    ColorType::RGBA8888,
    AlphaType::Unpremul,
    None
);
def_read_pix_bench!(
    read_pix_rgba_premul_srgb =
        "ReadPixBench(kRGBA_8888_SkColorType, kPremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    ColorType::RGBA8888,
    AlphaType::Premul,
    Some(ColorSpace::new_srgb())
);
def_read_pix_bench!(
    read_pix_rgba_unpremul_srgb =
        "ReadPixBench(kRGBA_8888_SkColorType, kUnpremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    ColorType::RGBA8888,
    AlphaType::Unpremul,
    Some(ColorSpace::new_srgb())
);
def_read_pix_bench!(
    read_pix_bgra_premul_null =
        "ReadPixBench(kBGRA_8888_SkColorType, kPremul_SkAlphaType, nullptr)",
    ColorType::BGRA8888,
    AlphaType::Premul,
    None
);
def_read_pix_bench!(
    read_pix_bgra_unpremul_null =
        "ReadPixBench(kBGRA_8888_SkColorType, kUnpremul_SkAlphaType, nullptr)",
    ColorType::BGRA8888,
    AlphaType::Unpremul,
    None
);
def_read_pix_bench!(
    read_pix_bgra_premul_srgb =
        "ReadPixBench(kBGRA_8888_SkColorType, kPremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    ColorType::BGRA8888,
    AlphaType::Premul,
    Some(ColorSpace::new_srgb())
);
def_read_pix_bench!(
    read_pix_bgra_unpremul_srgb =
        "ReadPixBench(kBGRA_8888_SkColorType, kUnpremul_SkAlphaType, SkColorSpace::MakeSRGB())",
    ColorType::BGRA8888,
    AlphaType::Unpremul,
    Some(ColorSpace::new_srgb())
);

// Port of: bench/ReadPixBench.cpp#L141-L144 (chrome/m156)
def_bench!(
    get_alphaf_rgba = "GetAlphafBench(kN32_SkColorType, \"rgba\")",
    GetAlphafBench::new(ColorType::N32, "rgba")
);
// Port of: bench/ReadPixBench.cpp#L142 (chrome/m156)
def_bench!(
    get_alphaf_rgbx = "GetAlphafBench(kRGB_888x_SkColorType, \"rgbx\")",
    GetAlphafBench::new(ColorType::RGB888x, "rgbx")
);
// Port of: bench/ReadPixBench.cpp#L143 (chrome/m156)
def_bench!(
    get_alphaf_f16 = "GetAlphafBench(kRGBA_F16_SkColorType, \"f16\")",
    GetAlphafBench::new(ColorType::RGBAF16, "f16")
);
// Port of: bench/ReadPixBench.cpp#L144 (chrome/m156)
def_bench!(
    get_alphaf_f32 = "GetAlphafBench(kRGBA_F32_SkColorType, \"f32\")",
    GetAlphafBench::new(ColorType::RGBAF32, "f32")
);
