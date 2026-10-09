// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ColorSpaceBench.cpp

//! `SkcmsTransformBench`: `skcms_Transform` from sRGB to Adobe RGB over a 512 x 512 RGBA image,
//! once per image or once per scanline (non-rendering). `ColorSpaceTransformBench` (the
//! raster pipeline transform) is not ported yet.

use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_skcms::{AlphaFormat, IccProfile, PixelFormat, transform};

use crate::def_bench;
use crate::prelude::*;

/// `enum class Mode`.
// Port of: bench/ColorSpaceBench.cpp#L18-L22 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// `kSingleImage`: transforms a single image at a time.
    SingleImage,
    /// `kSingleScanline`: transforms an image scanline by scanline.
    SingleScanline,
}

impl Mode {
    /// `mode_name(m)`.
    // Port of: bench/ColorSpaceBench.cpp#L24-L31 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Mode::SingleImage => "SingleImage",
            Mode::SingleScanline => "SingleScanline",
        }
    }
}

/// `kWidth` and `kHeight` of both benches.
// Port of: bench/ColorSpaceBench.cpp#L120-L123 (chrome/m156)
const WIDTH: usize = 512;
const HEIGHT: usize = 512;
/// `kBytesPerPixel`.
const BYTES_PER_PIXEL: usize = 4;

/// `class SkcmsTransformBench`.
// Port of: bench/ColorSpaceBench.cpp#L120-L204 (chrome/m156)
struct SkcmsTransformBench {
    mode: Mode,
    /// `fName`.
    name: String,
    src_pixels: Vec<u8>,
    dst_pixels: Vec<u8>,
    srgb_profile: Option<IccProfile>,
    adobe_profile: Option<IccProfile>,
}

impl SkcmsTransformBench {
    // Port of: bench/ColorSpaceBench.cpp#L121-L123 (chrome/m156)
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            // fName = std::string("Skcms") + mode_name(fMode) + "Transform";
            name: format!("Skcms{}Transform", mode.name()),
            src_pixels: vec![0; WIDTH * HEIGHT * BYTES_PER_PIXEL],
            dst_pixels: vec![0; WIDTH * HEIGHT * BYTES_PER_PIXEL],
            srgb_profile: None,
            adobe_profile: None,
        }
    }
}

impl Benchmark for SkcmsTransformBench {
    // Port of: bench/ColorSpaceBench.cpp#L127-L129 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ColorSpaceBench.cpp#L131-L146 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // std::fill(std::begin(fSrcPixels), std::end(fSrcPixels), 0xFF);
        self.src_pixels.fill(0xFF);
        // SkColorSpace::MakeSRGB()->toProfile(&fSRGBProfile);
        self.srgb_profile = Some(ColorSpace::new_srgb().to_profile());
        // SkColorSpace::MakeRGB(SkNamedTransferFn::k2Dot2, SkNamedGamut::kAdobeRGB)->toProfile(...)
        self.adobe_profile = Some(
            ColorSpace::new_rgb(&named_transfer_fn::DOT22, &named_gamut::ADOBE_RGB)
                .expect("a valid color space")
                .to_profile(),
        );
    }

    // Port of: bench/ColorSpaceBench.cpp#L148-L187 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let srgb = self.srgb_profile.as_ref().expect("on_delayed_setup ran");
        let adobe = self.adobe_profile.as_ref().expect("on_delayed_setup ran");
        match self.mode {
            Mode::SingleImage => {
                for _ in 0..loops {
                    // skcms_Transform(fSrcPixels, RGBA_8888, PremulAsEncoded, &fSRGBProfile,
                    //                 fDstPixels, RGBA_8888, PremulAsEncoded, &fAdobeProfile,
                    //                 kWidth * kHeight);
                    let _ = transform(
                        &self.src_pixels,
                        PixelFormat::Rgba8888,
                        AlphaFormat::PremulAsEncoded,
                        Some(srgb),
                        &mut self.dst_pixels,
                        PixelFormat::Rgba8888,
                        AlphaFormat::PremulAsEncoded,
                        Some(adobe),
                        WIDTH * HEIGHT,
                    );
                }
            }
            Mode::SingleScanline => {
                for _ in 0..loops {
                    // uint8_t* src = fSrcPixels; uint8_t* dst = fDstPixels;
                    for y in 0..HEIGHT {
                        let row = y * WIDTH * BYTES_PER_PIXEL;
                        let _ = transform(
                            &self.src_pixels[row..row + WIDTH * BYTES_PER_PIXEL],
                            PixelFormat::Rgba8888,
                            AlphaFormat::PremulAsEncoded,
                            Some(srgb),
                            &mut self.dst_pixels[row..row + WIDTH * BYTES_PER_PIXEL],
                            PixelFormat::Rgba8888,
                            AlphaFormat::PremulAsEncoded,
                            Some(adobe),
                            WIDTH,
                        );
                    }
                }
            }
        }
    }
}

// Port of: bench/ColorSpaceBench.cpp#L209 (chrome/m156)
def_bench!(
    skcms_single_image = "SkcmsTransformBench(Mode::kSingleImage)",
    SkcmsTransformBench::new(Mode::SingleImage)
);
// Port of: bench/ColorSpaceBench.cpp#L210 (chrome/m156)
def_bench!(
    skcms_single_scanline = "SkcmsTransformBench(Mode::kSingleScanline)",
    SkcmsTransformBench::new(Mode::SingleScanline)
);
