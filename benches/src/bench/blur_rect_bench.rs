// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BlurRectBench.cpp

//! Blurred rectangles made by `SkBlurMask` directly, from a box-filter approximation of a source
//! mask, and from the ground-truth gaussian (`BlurRectBench`, `BlurRectDirectBench`,
//! `BlurRectBoxFilterBench`, `BlurRectGaussianBench`). These benches draw nothing: they time the
//! mask code, so they are non-rendering in effect but use the default (rendering) suitability.
//!
//! The C++ class hierarchy is a base struct plus a virtual `makeBlurryRect` and `preBenchSetup`.
//! Here the base is [`BlurRectBench`], generic over the kernel, and the three subclasses are
//! [`BlurRectKernel`] implementations.

use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask::{AllocType, CreateMode, MaskBuilder, MaskFormat};
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::scalar::{scalar, scalar_fraction, scalar_round_to_int};

use crate::def_bench;
use crate::prelude::*;

// #define SMALL SkIntToScalar(2)
// #define REAL 1.5f
// static const SkScalar kMedium = SkIntToScalar(5);
// #define BIG SkIntToScalar(10)
// static const SkScalar kMedBig = SkIntToScalar(20);
// #define REALBIG 30.5f
const SMALL: scalar = 2.0;
const REAL: scalar = 1.5;
const K_MEDIUM: scalar = 5.0;
const BIG: scalar = 10.0;
const K_MED_BIG: scalar = 20.0;
const REAL_BIG: scalar = 30.5;

/// The virtual methods of `BlurRectBench` that its subclasses override: `preBenchSetup` and
/// `makeBlurryRect`. The radius is passed in because the C++ subclasses read it from the base.
trait BlurRectKernel {
    /// `preBenchSetup(const SkRect&)`: once per `onDraw`, before the timed loop.
    // Port of: bench/BlurRectBench.cpp#L40 (chrome/m156)
    fn pre_bench_setup(&mut self, _r: &Rect) {}

    /// `makeBlurryRect(const SkRect&)`: one iteration of the timed loop.
    fn make_blurry_rect(&mut self, radius: scalar, r: &Rect);
}

/// `class BlurRectBench`: the name, the radius and the kernel. `fLoopCount` is computed by the
/// C++ constructor but never read by `onDraw`, so it is not ported.
// Port of: bench/BlurRectBench.cpp#L13-L49 (chrome/m156)
struct BlurRectBench<K: BlurRectKernel> {
    radius: scalar,
    name: String,
    kernel: K,
}

impl<K: BlurRectKernel> BlurRectBench<K> {
    /// `setName` with the `%.2f` / `%d` formatting the subclass constructors share.
    // Port of: bench/BlurRectBench.cpp#L24-L49 (chrome/m156), the per-subclass name formatting
    fn new(kernel: K, prefix: &str, rad: scalar) -> Self {
        let name = if scalar_fraction(rad) != 0.0 {
            format!("{prefix}{rad:.2}")
        } else {
            format!("{prefix}{}", scalar_round_to_int(rad))
        };
        Self {
            radius: rad,
            name,
            kernel,
        }
    }
}

impl<K: BlurRectKernel> Benchmark for BlurRectBench<K> {
    // Port of: bench/BlurRectBench.cpp#L19-L21 (chrome/m156)
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/BlurRectBench.cpp#L27-L41 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // SkPaint paint;
        let mut paint = Paint::default();
        // this->setupPaint(&paint);
        self.setup_paint(&mut paint);
        // paint.setAntiAlias(true);
        paint.set_anti_alias(true);
        // Skia builds the paint and never draws with it: the paint is only set up, not used.

        // SkScalar pad = fRadius*3/2 + SK_Scalar1;
        let pad = self.radius * 3.0 / 2.0 + 1.0;
        // SkRect r = SkRect::MakeWH(2 * pad + SK_Scalar1, 2 * pad + SK_Scalar1);
        let r = Rect::from_wh(2.0 * pad + 1.0, 2.0 * pad + 1.0);

        // preBenchSetup(r);
        self.kernel.pre_bench_setup(&r);

        // for (int i = 0; i < loops; i++) { this->makeBlurryRect(r); }
        for _ in 0..loops {
            self.kernel.make_blurry_rect(self.radius, &r);
        }
    }
}

/// `BlurRectDirectBench::makeBlurryRect`: `SkBlurMask::BlurRect` into a fresh mask.
// Port of: bench/BlurRectBench.cpp#L51-L66 (chrome/m156)
struct DirectKernel;

impl BlurRectKernel for DirectKernel {
    fn make_blurry_rect(&mut self, radius: scalar, r: &Rect) {
        let mut mask = MaskBuilder::default();
        if !BlurMask::blur_rect(
            BlurMask::convert_radius_to_sigma(radius),
            &mut mask,
            r,
            BlurStyle::Normal,
            None,
            CreateMode::ComputeBoundsAndRenderImage,
        ) {
            return;
        }
        // SkMaskBuilder::FreeImage(mask.image()); dropping `mask` frees the image.
    }
}

/// The source mask shared by `BlurRectSeparableBench` and its two subclasses (`fSrcMask`).
// Port of: bench/BlurRectBench.cpp#L68-L91 (chrome/m156)
#[derive(Default)]
struct SeparableSource {
    src_mask: MaskBuilder,
}

impl SeparableSource {
    /// `BlurRectSeparableBench::preBenchSetup`: an opaque A8 mask covering `r` rounded out.
    // Port of: bench/BlurRectBench.cpp#L72-L84 (chrome/m156)
    fn pre_bench_setup(&mut self, r: &Rect) {
        // SkMaskBuilder::FreeImage(fSrcMask.image()); the assignment below drops the old image.
        // r.roundOut(&fSrcMask.bounds());
        let bounds: IRect = r.round_out();
        self.src_mask.bounds = bounds;
        // fSrcMask.format() = SkMask::kA8_Format;
        self.src_mask.format = MaskFormat::A8;
        // fSrcMask.rowBytes() = fSrcMask.fBounds.width();
        // The width is positive for a rectangle that rounds out to a non-empty mask.
        #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> uint32_t conversion
        let row_bytes = self.src_mask.bounds.width() as u32;
        self.src_mask.row_bytes = row_bytes;
        // fSrcMask.image() = SkMaskBuilder::AllocImage(fSrcMask.computeTotalImageSize());
        let total = self.src_mask.compute_total_image_size();
        self.src_mask.image = MaskBuilder::alloc_image(total, AllocType::Uninit);
        // memset(fSrcMask.image(), 0xff, fSrcMask.computeTotalImageSize());
        self.src_mask.image[..total].fill(0xff);
    }
}

/// `BlurRectBoxFilterBench::makeBlurryRect`: `SkBlurMask::BoxBlur` of the source mask.
// Port of: bench/BlurRectBench.cpp#L113-L125 (chrome/m156)
#[derive(Default)]
struct BoxFilterKernel {
    source: SeparableSource,
}

impl BlurRectKernel for BoxFilterKernel {
    fn pre_bench_setup(&mut self, r: &Rect) {
        self.source.pre_bench_setup(r);
    }

    fn make_blurry_rect(&mut self, radius: scalar, _r: &Rect) {
        let mut mask = MaskBuilder::default();
        if !BlurMask::box_blur(
            &mut mask,
            &self.source.src_mask.as_mask(),
            BlurMask::convert_radius_to_sigma(radius),
            BlurStyle::Normal,
            None,
        ) {
            return;
        }
        // SkMaskBuilder::FreeImage(mask.image()); dropping `mask` frees the image.
    }
}

/// `BlurRectGaussianBench::makeBlurryRect`: `SkBlurMask::BlurGroundTruth` of the source mask.
// Port of: bench/BlurRectBench.cpp#L136-L148 (chrome/m156)
#[derive(Default)]
struct GaussianKernel {
    source: SeparableSource,
}

impl BlurRectKernel for GaussianKernel {
    fn pre_bench_setup(&mut self, r: &Rect) {
        self.source.pre_bench_setup(r);
    }

    fn make_blurry_rect(&mut self, radius: scalar, _r: &Rect) {
        let mut mask = MaskBuilder::default();
        if !BlurMask::blur_ground_truth(
            BlurMask::convert_radius_to_sigma(radius),
            &mut mask,
            &self.source.src_mask.as_mask(),
            BlurStyle::Normal,
            None,
        ) {
            return;
        }
        // SkMaskBuilder::FreeImage(mask.image()); dropping `mask` frees the image.
    }
}

/// `BlurRectDirectBench(rad)`: name `blurrect_direct_<rad>`.
// Port of: bench/BlurRectBench.cpp#L93-L110 (chrome/m156)
fn blur_rect_direct_bench(rad: scalar) -> BlurRectBench<DirectKernel> {
    BlurRectBench::new(DirectKernel, "blurrect_direct_", rad)
}

/// `BlurRectBoxFilterBench(rad)`: name `blurrect_boxfilter_<rad>`.
// Port of: bench/BlurRectBench.cpp#L113-L125 (chrome/m156)
fn blur_rect_box_filter_bench(rad: scalar) -> BlurRectBench<BoxFilterKernel> {
    BlurRectBench::new(BoxFilterKernel::default(), "blurrect_boxfilter_", rad)
}

/// `BlurRectGaussianBench(rad)`: name `blurrect_gaussian_<rad>`.
// Port of: bench/BlurRectBench.cpp#L127-L148 (chrome/m156)
fn blur_rect_gaussian_bench(rad: scalar) -> BlurRectBench<GaussianKernel> {
    BlurRectBench::new(GaussianKernel::default(), "blurrect_gaussian_", rad)
}

// Port of: bench/BlurRectBench.cpp#L186 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_small = "BlurRectBoxFilterBench(SMALL)",
    blur_rect_box_filter_bench(SMALL)
);
// Port of: bench/BlurRectBench.cpp#L187 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_big = "BlurRectBoxFilterBench(BIG)",
    blur_rect_box_filter_bench(BIG)
);
// Port of: bench/BlurRectBench.cpp#L188 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_real_big = "BlurRectBoxFilterBench(REALBIG)",
    blur_rect_box_filter_bench(REAL_BIG)
);
// Port of: bench/BlurRectBench.cpp#L189 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_real = "BlurRectBoxFilterBench(REAL)",
    blur_rect_box_filter_bench(REAL)
);
// Port of: bench/BlurRectBench.cpp#L190 (chrome/m156)
def_bench!(
    blur_rect_gaussian_bench_small = "BlurRectGaussianBench(SMALL)",
    blur_rect_gaussian_bench(SMALL)
);
// Port of: bench/BlurRectBench.cpp#L191 (chrome/m156)
def_bench!(
    blur_rect_gaussian_bench_big = "BlurRectGaussianBench(BIG)",
    blur_rect_gaussian_bench(BIG)
);
// Port of: bench/BlurRectBench.cpp#L192 (chrome/m156)
def_bench!(
    blur_rect_gaussian_bench_real_big = "BlurRectGaussianBench(REALBIG)",
    blur_rect_gaussian_bench(REAL_BIG)
);
// Port of: bench/BlurRectBench.cpp#L193 (chrome/m156)
def_bench!(
    blur_rect_gaussian_bench_real = "BlurRectGaussianBench(REAL)",
    blur_rect_gaussian_bench(REAL)
);
// Port of: bench/BlurRectBench.cpp#L194 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_small = "BlurRectDirectBench(SMALL)",
    blur_rect_direct_bench(SMALL)
);
// Port of: bench/BlurRectBench.cpp#L195 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_big = "BlurRectDirectBench(BIG)",
    blur_rect_direct_bench(BIG)
);
// Port of: bench/BlurRectBench.cpp#L196 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_real_big = "BlurRectDirectBench(REALBIG)",
    blur_rect_direct_bench(REAL_BIG)
);
// Port of: bench/BlurRectBench.cpp#L197 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_real = "BlurRectDirectBench(REAL)",
    blur_rect_direct_bench(REAL)
);
// Port of: bench/BlurRectBench.cpp#L199 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_k_medium = "BlurRectDirectBench(kMedium)",
    blur_rect_direct_bench(K_MEDIUM)
);
// Port of: bench/BlurRectBench.cpp#L200 (chrome/m156)
def_bench!(
    blur_rect_direct_bench_k_med_big = "BlurRectDirectBench(kMedBig)",
    blur_rect_direct_bench(K_MED_BIG)
);
// Port of: bench/BlurRectBench.cpp#L202 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_k_medium = "BlurRectBoxFilterBench(kMedium)",
    blur_rect_box_filter_bench(K_MEDIUM)
);
// Port of: bench/BlurRectBench.cpp#L203 (chrome/m156)
def_bench!(
    blur_rect_box_filter_bench_k_med_big = "BlurRectBoxFilterBench(kMedBig)",
    blur_rect_box_filter_bench(K_MED_BIG)
);
