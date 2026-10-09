// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/MipmapBench.cpp

//! `MipmapBench`: `SkMipmap::Build` of a white sRGB bitmap, in N32 or RGBA F16, with even and odd
//! sizes at each level (non-rendering).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mipmap::Mipmap;

use crate::def_bench;
use crate::prelude::*;

/// `class MipmapBench`.
// Port of: bench/MipmapBench.cpp#L9-L43 (chrome/m156)
struct MipmapBench {
    /// `fName`.
    name: String,
    w: i32,
    h: i32,
    half_float: bool,
    /// `fBitmap`, allocated in `on_delayed_setup`.
    bitmap: Bitmap,
}

impl MipmapBench {
    // Port of: bench/MipmapBench.cpp#L13-L22 (chrome/m156)
    fn new(w: i32, h: i32, half_float: bool) -> Self {
        // fName.printf("mipmap_build_%dx%d", w, h);
        let mut name = format!("mipmap_build_{w}x{h}");
        if half_float {
            // fName.append("_f16");
            name.push_str("_f16");
        }
        Self {
            name,
            w,
            h,
            half_float,
            bitmap: Bitmap::new(),
        }
    }
}

impl Benchmark for MipmapBench {
    // Port of: bench/MipmapBench.cpp#L24-L26 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/MipmapBench.cpp#L28-L35 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // SkColorType ct = fHalfFoat ? kRGBA_F16_SkColorType : kN32_SkColorType;
        let ct = if self.half_float {
            ColorType::RGBAF16
        } else {
            ColorType::N32
        };
        // SkImageInfo info = SkImageInfo::Make(fW, fH, ct, kPremul_SkAlphaType, SkColorSpace::MakeSRGB());
        let info = ImageInfo::new(
            (self.w, self.h),
            ct,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );
        // fBitmap.allocPixels(info);
        self.bitmap.alloc_pixels_info(&info, None);
        // fBitmap.eraseColor(SK_ColorWHITE);  // so we don't read uninitialized memory
        self.bitmap.erase_color(Color::new(0xFFFF_FFFF));
    }

    // Port of: bench/MipmapBench.cpp#L37-L41 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let pixmap = self
            .bitmap
            .peek_pixels()
            .expect("on_delayed_setup allocated the bitmap");
        for _ in 0..loops * 4 {
            // SkMipmap::Build(fBitmap, nullptr)->unref();
            let _ = Mipmap::build(&pixmap, true);
        }
    }
}

// Build variants that exercise the width and heights being even or odd at each level, as the
// impl specializes on each of these.
// Port of: bench/MipmapBench.cpp#L45-L54 (chrome/m156)
def_bench!(
    mipmap_511_511 = "MipmapBench(511, 511)",
    MipmapBench::new(511, 511, false)
);
// Port of: bench/MipmapBench.cpp#L46 (chrome/m156)
def_bench!(
    mipmap_512_511 = "MipmapBench(512, 511)",
    MipmapBench::new(512, 511, false)
);
// Port of: bench/MipmapBench.cpp#L47 (chrome/m156)
def_bench!(
    mipmap_511_512 = "MipmapBench(511, 512)",
    MipmapBench::new(511, 512, false)
);
// Port of: bench/MipmapBench.cpp#L48 (chrome/m156)
def_bench!(
    mipmap_512_512 = "MipmapBench(512, 512)",
    MipmapBench::new(512, 512, false)
);
// Port of: bench/MipmapBench.cpp#L49 (chrome/m156)
def_bench!(
    mipmap_512_512_f16 = "MipmapBench(512, 512, true)",
    MipmapBench::new(512, 512, true)
);
// Port of: bench/MipmapBench.cpp#L50 (chrome/m156)
def_bench!(
    mipmap_511_511_f16 = "MipmapBench(511, 511, true)",
    MipmapBench::new(511, 511, true)
);
// Port of: bench/MipmapBench.cpp#L51 (chrome/m156)
def_bench!(
    mipmap_2048_2048 = "MipmapBench(2048, 2048)",
    MipmapBench::new(2048, 2048, false)
);
// Port of: bench/MipmapBench.cpp#L52 (chrome/m156)
def_bench!(
    mipmap_2047_2047 = "MipmapBench(2047, 2047)",
    MipmapBench::new(2047, 2047, false)
);
// Port of: bench/MipmapBench.cpp#L53 (chrome/m156)
def_bench!(
    mipmap_2048_2047 = "MipmapBench(2048, 2047)",
    MipmapBench::new(2048, 2047, false)
);
// Port of: bench/MipmapBench.cpp#L54 (chrome/m156)
def_bench!(
    mipmap_2047_2048 = "MipmapBench(2047, 2048)",
    MipmapBench::new(2047, 2048, false)
);
