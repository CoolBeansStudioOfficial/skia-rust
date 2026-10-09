// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BlurBench.cpp

//! Blurred ovals: a blur mask filter of each `BlurStyle` at a range of radii (`BlurBench`).
//! Rendering bench.

use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_fraction, scalar_round_to_int};

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/BlurBench.cpp#L11-L15 (chrome/m156)
/// `#define MINI 0.01f`
const MINI: scalar = 0.01;
/// `#define SMALL SkIntToScalar(2)`
const SMALL: scalar = 2.0;
/// `#define REAL 0.5f`
const REAL: scalar = 0.5;
/// `#define BIG SkIntToScalar(10)`
const BIG: scalar = 10.0;
/// `#define REALBIG 100.5f`
const REALBIG: scalar = 100.5;

/// `gStyleName`, indexed by the style.
// Port of: bench/BlurBench.cpp#L17-L22 (chrome/m156)
fn style_name(style: BlurStyle) -> &'static str {
    match style {
        BlurStyle::Normal => "normal",
        BlurStyle::Solid => "solid",
        BlurStyle::Outer => "outer",
        BlurStyle::Inner => "inner",
    }
}

/// `class BlurBench`.
// Port of: bench/BlurBench.cpp#L24-L70 (chrome/m156)
struct BlurBench {
    radius: scalar,
    style: BlurStyle,
    name: String,
}

impl BlurBench {
    // Port of: bench/BlurBench.cpp#L30-L43 (chrome/m156)
    fn new(rad: scalar, style: BlurStyle) -> Self {
        let name = if rad > 0.0 { style_name(style) } else { "none" };
        let quality = "high_quality";
        let name = if scalar_fraction(rad) == 0.0 {
            // fName.printf("blur_%d_%s_%s", SkScalarRoundToInt(rad), name, quality);
            format!("blur_{}_{name}_{quality}", scalar_round_to_int(rad))
        } else {
            // fName.printf("blur_%.2f_%s_%s", rad, name, quality);
            format!("blur_{rad:.2}_{name}_{quality}")
        };
        Self {
            radius: rad,
            style,
            name,
        }
    }
}

impl Benchmark for BlurBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/BlurBench.cpp#L45-L66 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("BlurBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        paint.set_anti_alias(true);
        // SkRandom rand;
        let mut rand = Random::default();
        for _ in 0..loops {
            // SkRect r = SkRect::MakeWH(rand.nextUScalar1() * 400, rand.nextUScalar1() * 400);
            let w = rand.next_u_scalar1() * 400.0;
            let h = rand.next_u_scalar1() * 400.0;
            let mut r = Rect::from_wh(w, h);
            // r.offset(fRadius, fRadius);
            r.offset((self.radius, self.radius));
            if self.radius > 0.0 {
                // SkMaskFilter::MakeBlur(fStyle, SkBlurMask::ConvertRadiusToSigma(fRadius))
                paint.set_mask_filter(MaskFilter::blur(
                    self.style,
                    BlurMask::convert_radius_to_sigma(self.radius),
                    None,
                ));
            }
            canvas.draw_oval(r, &paint);
        }
    }
}

// Port of: bench/BlurBench.cpp#L79-L79 (chrome/m156)
def_bench!(
    blur_bench_mini_normal = "BlurBench(MINI, kNormal_SkBlurStyle)",
    BlurBench::new(MINI, BlurStyle::Normal)
);
// Port of: bench/BlurBench.cpp#L80-L80 (chrome/m156)
def_bench!(
    blur_bench_mini_solid = "BlurBench(MINI, kSolid_SkBlurStyle)",
    BlurBench::new(MINI, BlurStyle::Solid)
);
// Port of: bench/BlurBench.cpp#L81-L81 (chrome/m156)
def_bench!(
    blur_bench_mini_outer = "BlurBench(MINI, kOuter_SkBlurStyle)",
    BlurBench::new(MINI, BlurStyle::Outer)
);
// Port of: bench/BlurBench.cpp#L82-L82 (chrome/m156)
def_bench!(
    blur_bench_mini_inner = "BlurBench(MINI, kInner_SkBlurStyle)",
    BlurBench::new(MINI, BlurStyle::Inner)
);
// Port of: bench/BlurBench.cpp#L84-L84 (chrome/m156)
def_bench!(
    blur_bench_small_normal = "BlurBench(SMALL, kNormal_SkBlurStyle)",
    BlurBench::new(SMALL, BlurStyle::Normal)
);
// Port of: bench/BlurBench.cpp#L85-L85 (chrome/m156)
def_bench!(
    blur_bench_small_solid = "BlurBench(SMALL, kSolid_SkBlurStyle)",
    BlurBench::new(SMALL, BlurStyle::Solid)
);
// Port of: bench/BlurBench.cpp#L86-L86 (chrome/m156)
def_bench!(
    blur_bench_small_outer = "BlurBench(SMALL, kOuter_SkBlurStyle)",
    BlurBench::new(SMALL, BlurStyle::Outer)
);
// Port of: bench/BlurBench.cpp#L87-L87 (chrome/m156)
def_bench!(
    blur_bench_small_inner = "BlurBench(SMALL, kInner_SkBlurStyle)",
    BlurBench::new(SMALL, BlurStyle::Inner)
);
// Port of: bench/BlurBench.cpp#L89-L89 (chrome/m156)
def_bench!(
    blur_bench_big_normal = "BlurBench(BIG, kNormal_SkBlurStyle)",
    BlurBench::new(BIG, BlurStyle::Normal)
);
// Port of: bench/BlurBench.cpp#L90-L90 (chrome/m156)
def_bench!(
    blur_bench_big_solid = "BlurBench(BIG, kSolid_SkBlurStyle)",
    BlurBench::new(BIG, BlurStyle::Solid)
);
// Port of: bench/BlurBench.cpp#L91-L91 (chrome/m156)
def_bench!(
    blur_bench_big_outer = "BlurBench(BIG, kOuter_SkBlurStyle)",
    BlurBench::new(BIG, BlurStyle::Outer)
);
// Port of: bench/BlurBench.cpp#L92-L92 (chrome/m156)
def_bench!(
    blur_bench_big_inner = "BlurBench(BIG, kInner_SkBlurStyle)",
    BlurBench::new(BIG, BlurStyle::Inner)
);
// Port of: bench/BlurBench.cpp#L94-L94 (chrome/m156)
def_bench!(
    blur_bench_realbig_normal = "BlurBench(REALBIG, kNormal_SkBlurStyle)",
    BlurBench::new(REALBIG, BlurStyle::Normal)
);
// Port of: bench/BlurBench.cpp#L95-L95 (chrome/m156)
def_bench!(
    blur_bench_realbig_solid = "BlurBench(REALBIG, kSolid_SkBlurStyle)",
    BlurBench::new(REALBIG, BlurStyle::Solid)
);
// Port of: bench/BlurBench.cpp#L96-L96 (chrome/m156)
def_bench!(
    blur_bench_realbig_outer = "BlurBench(REALBIG, kOuter_SkBlurStyle)",
    BlurBench::new(REALBIG, BlurStyle::Outer)
);
// Port of: bench/BlurBench.cpp#L97-L97 (chrome/m156)
def_bench!(
    blur_bench_realbig_inner = "BlurBench(REALBIG, kInner_SkBlurStyle)",
    BlurBench::new(REALBIG, BlurStyle::Inner)
);
// Port of: bench/BlurBench.cpp#L99-L99 (chrome/m156)
def_bench!(
    blur_bench_real_normal = "BlurBench(REAL, kNormal_SkBlurStyle)",
    BlurBench::new(REAL, BlurStyle::Normal)
);
// Port of: bench/BlurBench.cpp#L100-L100 (chrome/m156)
def_bench!(
    blur_bench_real_solid = "BlurBench(REAL, kSolid_SkBlurStyle)",
    BlurBench::new(REAL, BlurStyle::Solid)
);
// Port of: bench/BlurBench.cpp#L101-L101 (chrome/m156)
def_bench!(
    blur_bench_real_outer = "BlurBench(REAL, kOuter_SkBlurStyle)",
    BlurBench::new(REAL, BlurStyle::Outer)
);
// Port of: bench/BlurBench.cpp#L102-L102 (chrome/m156)
def_bench!(
    blur_bench_real_inner = "BlurBench(REAL, kInner_SkBlurStyle)",
    BlurBench::new(REAL, BlurStyle::Inner)
);
// Port of: bench/BlurBench.cpp#L104-L104 (chrome/m156)
def_bench!(
    blur_bench_zero_normal = "BlurBench(0, kNormal_SkBlurStyle)",
    BlurBench::new(0.0, BlurStyle::Normal)
);
