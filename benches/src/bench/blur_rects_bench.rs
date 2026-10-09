// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BlurRectsBench.cpp

//! A blurred path made of an outer and an inner rectangle, drawn with `drawPath`
//! (`BlurRectsNinePatchBench`, `BlurRectsNonNinePatchBench`). Rendering benches.

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

/// `class BlurRectsBench`: the outer and inner rectangles as one path, with a normal blur of
/// `radius` (used as the sigma directly).
// Port of: bench/BlurRectsBench.cpp#L13-L49 (chrome/m156)
struct BlurRectsBench {
    name: &'static str,
    outer: Rect,
    inner: Rect,
    radius: scalar,
}

impl BlurRectsBench {
    // Port of: bench/BlurRectsBench.cpp#L16-L21 (chrome/m156)
    fn new(name: &'static str, outer: Rect, inner: Rect, radius: scalar) -> Self {
        Self {
            name,
            outer,
            inner,
            radius,
        }
    }
}

impl Benchmark for BlurRectsBench {
    fn name(&self) -> String {
        self.name.to_owned()
    }

    // Port of: bench/BlurRectsBench.cpp#L27-L41 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("BlurRectsBench is a rendering bench");
        // SkPaint paint; (no setupPaint, so anti-aliasing is off)
        let mut paint = Paint::default();
        // paint.setMaskFilter(SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, fRadius));
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, self.radius, None));
        // SkPath path = SkPathBuilder().addRect(fOuter, kCW).addRect(fInner, kCW).detach();
        let mut builder = PathBuilder::new();
        builder.add_rect(self.outer, PathDirection::CW, None::<usize>);
        builder.add_rect(self.inner, PathDirection::CW, None::<usize>);
        let path: Path = builder.detach();
        for _ in 0..loops {
            canvas.draw_path(&path, &paint);
        }
    }
}

// Port of: bench/BlurRectsBench.cpp#L78-L80 (chrome/m156)
def_bench!(
    blur_rects_nine_patch = "BlurRectsNinePatchBench(SkRect::MakeXYWH(10, 10, 100, 100), SkRect::MakeXYWH(20, 20, 60, 60), 2.3f)",
    BlurRectsBench::new(
        "blurrectsninepatch",
        Rect::from_ltrb(10.0, 10.0, 110.0, 110.0),
        Rect::from_ltrb(20.0, 20.0, 80.0, 80.0),
        2.3
    )
);
// Port of: bench/BlurRectsBench.cpp#L81-L83 (chrome/m156)
def_bench!(
    blur_rects_non_nine_patch = "BlurRectsNonNinePatchBench(SkRect::MakeXYWH(10, 10, 100, 100), SkRect::MakeXYWH(50, 50, 10, 10), 4.3f)",
    BlurRectsBench::new(
        "blurrectsnonninepatch",
        Rect::from_ltrb(10.0, 10.0, 110.0, 110.0),
        Rect::from_ltrb(50.0, 50.0, 60.0, 60.0),
        4.3
    )
);
