// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PremulAndUnpremulAlphaOpsBench.cpp

//! `PremulAndUnpremulAlphaOpsBench`: writes a 256×256 unpremultiplied bitmap onto the canvas and
//! reads it back, which converts between premultiplied and unpremultiplied alpha (a rendering
//! bench).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::def_bench;
use crate::prelude::*;
use crate::tool_utils::colortype_name;

/// `enum { W = 256, H = 256 }`.
// Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L15-L15 (chrome/m156)
const W: i32 = 256;
/// `enum { H = 256 }`.
const H: i32 = 256;

/// `class PremulAndUnpremulAlphaOpsBench`.
// Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L14-L62 (chrome/m156)
struct PremulAndUnpremulAlphaOpsBench {
    bmp1: Bitmap,
    bmp2: Bitmap,
    color_type: ColorType,
    name: String,
}

impl PremulAndUnpremulAlphaOpsBench {
    // PremulAndUnpremulAlphaOpsBench(SkColorType ct)
    // Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L14-L62 (chrome/m156)
    fn new(ct: ColorType) -> Self {
        Self {
            bmp1: Bitmap::new(),
            bmp2: Bitmap::new(),
            color_type: ct,
            // fName.printf("premul_and_unpremul_alpha_%s", ToolUtils::colortype_name(ct));
            name: format!("premul_and_unpremul_alpha_{}", colortype_name(ct)),
        }
    }
}

impl Benchmark for PremulAndUnpremulAlphaOpsBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L14-L62 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // SkImageInfo info = SkImageInfo::Make(W, H, fColorType, kUnpremul_SkAlphaType);
        let info = ImageInfo::new((W, H), self.color_type, AlphaType::Unpremul, None);
        // fBmp1.allocPixels(info);   // used in writePixels
        self.bmp1.alloc_pixels_info(&info, None);
        for h in 0..H {
            for w in 0..W {
                // SkColor places A in the right slot for either RGBA or BGRA
                // *fBmp1.getAddr32(w, h) = SkColorSetARGB(h & 0xFF, w & 0xFF, w & 0xFF, w & 0xFF);
                // SkColorSetARGB(a, r, g, b) = (a << 24) | (r << 16) | (g << 8) | b.
                let a = (h & 0xFF).unsigned_abs();
                let v = (w & 0xFF).unsigned_abs();
                self.bmp1
                    .set_addr32(w, h, (a << 24) | (v << 16) | (v << 8) | v);
            }
        }
        // fBmp2.allocPixels(info);    // used in readPixels()
        self.bmp2.alloc_pixels_info(&info, None);
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L14-L62 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("PremulAndUnpremulAlphaOpsBench is a rendering bench");
        // canvas->clear(SK_ColorBLACK);
        canvas.clear(Color4f::from_color(Color::new(0xFF00_0000)));
        for _ in 0..loops {
            // Unpremul -> Premul
            // canvas->writePixels(fBmp1.info(), fBmp1.getPixels(), fBmp1.rowBytes(), 0, 0);
            let _ = canvas.write_pixels_from_bitmap(&self.bmp1, (0, 0));
            // Premul -> Unpremul
            // canvas->readPixels(fBmp2.info(), fBmp2.getPixels(), fBmp2.rowBytes(), 0, 0);
            let _ = canvas.read_pixels_to_bitmap(&mut self.bmp2, (0, 0));
        }
    }
}

// Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L65-L65 (chrome/m156)
def_bench!(
    premul_and_unpremul_alpha_ops_bench_rgba =
        "PremulAndUnpremulAlphaOpsBench(kRGBA_8888_SkColorType)",
    PremulAndUnpremulAlphaOpsBench::new(ColorType::RGBA8888)
);
// Port of: bench/PremulAndUnpremulAlphaOpsBench.cpp#L66-L66 (chrome/m156)
def_bench!(
    premul_and_unpremul_alpha_ops_bench_bgra =
        "PremulAndUnpremulAlphaOpsBench(kBGRA_8888_SkColorType)",
    PremulAndUnpremulAlphaOpsBench::new(ColorType::BGRA8888)
);
