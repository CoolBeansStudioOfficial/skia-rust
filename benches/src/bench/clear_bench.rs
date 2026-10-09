// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ClearBench.cpp

//! `SkCanvas::clear` on the full canvas, on an axis-aligned clip, and on a rounded-rect clip, with
//! a small shaded rectangle drawn between clears (a rendering bench).
//!
//! The C++ `TopDeviceSurfaceDrawContext` / `testingOnly_SetPreserveOpsOnFullClear` lines only act
//! on a Ganesh `SurfaceDrawContext`; a raster canvas has none, so that guard is dropped.

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

use crate::def_bench;
use crate::prelude::*;

/// `ClearBench::ClearType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClearType {
    Full,
    Partial,
    Complex,
}

/// `static sk_sp<SkShader> make_shader()`: a blue-to-white linear gradient.
// Port of: bench/ClearBench.cpp#L26-L29 (chrome/m156)
fn make_shader() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(10.0, 10.0)];
    let colors = [
        Color4f::new(0.0, 0.0, 1.0, 1.0), // SkColors::kBlue
        Color4f::new(1.0, 1.0, 1.0, 1.0), // SkColors::kWhite
    ];
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    shaders::linear_gradient((pts[0], pts[1]), &gradient, None)
}

/// `class ClearBench`.
// Port of: bench/ClearBench.cpp#L38-L88 (chrome/m156)
struct ClearBench {
    kind: ClearType,
}

impl Benchmark for ClearBench {
    fn name(&self) -> String {
        match self.kind {
            ClearType::Full => "Clear-Full",
            ClearType::Partial => "Clear-Partial",
            ClearType::Complex => "Clear-Complex",
        }
        .to_owned()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ClearBench is a rendering bench");
        // static const SkRect kPartialClip = SkRect::MakeLTRB(50, 50, 400, 400);
        let partial_clip = Rect::from_ltrb(50.0, 50.0, 400.0, 400.0);
        // static const SkRRect kComplexClip = SkRRect::MakeRectXY(kPartialClip, 15, 15);
        let complex_clip = RRect::new_rect_xy(partial_clip, 15.0, 15.0);
        // Small to limit fill cost, but intersects the clips to confound batching.
        let interrupt_rect = Rect::from_xywh(200.0, 200.0, 3.0, 3.0);

        // For the draw that sits between consecutive clears, use a shader that is simple but
        // requires local coordinates (see the C++ comment in bench/ClearBench.cpp).
        let mut interrupt_paint = Paint::default();
        interrupt_paint.set_shader(make_shader());

        for _ in 0..loops {
            canvas.save();
            match self.kind {
                ClearType::Partial => {
                    canvas.clip_rect(partial_clip, None, None);
                }
                ClearType::Complex => {
                    canvas.clip_rrect(complex_clip, None, None);
                }
                // Don't add any extra clipping, since it defaults to the entire "device".
                ClearType::Full => {}
            }

            // The clear we care about measuring.
            canvas.clear(Color4f::from_color(Color::new(0xFF00_00FF))); // SkColors::kBlue
            canvas.restore();

            // Perform as minimal a draw as possible that intersects with the clear region in order
            // to prevent the clear ops from being batched together.
            canvas.draw_rect(interrupt_rect, &interrupt_paint);
        }
    }
}

// Port of: bench/ClearBench.cpp#L104-L104 (chrome/m156)
def_bench!(
    clear_bench_full = "ClearBench(ClearBench::kFull_ClearType)",
    ClearBench {
        kind: ClearType::Full
    }
);
// Port of: bench/ClearBench.cpp#L105-L105 (chrome/m156)
def_bench!(
    clear_bench_partial = "ClearBench(ClearBench::kPartial_ClearType)",
    ClearBench {
        kind: ClearType::Partial
    }
);
// Port of: bench/ClearBench.cpp#L106-L106 (chrome/m156)
def_bench!(
    clear_bench_complex = "ClearBench(ClearBench::kComplex_ClearType)",
    ClearBench {
        kind: ClearType::Complex
    }
);
