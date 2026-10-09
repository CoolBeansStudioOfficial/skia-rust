// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/DrawBitmapAABench.cpp

//! `DrawBitmapAABench`: a 200×200 green raster image drawn with `drawImage` (linear sampling) under
//! one transform, with and without anti-aliasing on the paint (a rendering bench).

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_raster::surfaces;

use crate::def_bench;
use crate::prelude::*;

/// `class DrawBitmapAABench`.
// Port of: bench/DrawBitmapAABench.cpp#L20-L56 (chrome/m156)
struct DrawBitmapAABench {
    paint: Paint,
    matrix: Matrix,
    name: String,
    image: Option<Image>,
}

impl DrawBitmapAABench {
    // DrawBitmapAABench(bool doAA, const SkMatrix& matrix, const char name[])
    // Port of: bench/DrawBitmapAABench.cpp#L15-L21 (chrome/m156)
    fn new(do_aa: bool, matrix: Matrix, name: &str) -> Self {
        // fPaint.setAntiAlias(doAA);
        let mut paint = Paint::default();
        paint.set_anti_alias(do_aa);
        // fName.appendf("%s_%s", doAA ? "aa" : "noaa", name);
        let name = format!("draw_bitmap_{}_{name}", if do_aa { "aa" } else { "noaa" });
        Self {
            paint,
            matrix,
            name,
            image: None,
        }
    }
}

impl Benchmark for DrawBitmapAABench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/DrawBitmapAABench.cpp#L27-L31 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // auto surf = SkSurfaces::Raster(SkImageInfo::MakeN32Premul(200, 200));
        let mut surf = surfaces::raster_n32_premul((200, 200)).expect("200x200 N32 raster surface");
        // surf->getCanvas()->clear(0xFF00FF00);
        surf.canvas()
            .clear(Color4f::from_color(Color::new(0xFF00_FF00)));
        // fImage = surf->makeImageSnapshot();
        self.image = surf.image_snapshot();
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/DrawBitmapAABench.cpp#L33-L42 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("DrawBitmapAABench is a rendering bench");
        // SkSamplingOptions sampling(SkFilterMode::kLinear);
        let sampling = SamplingOptions::from(FilterMode::Linear);
        // canvas->concat(fMatrix);
        canvas.concat(&self.matrix);
        let image = self.image.as_ref().expect("set in onDelayedSetup");
        for _ in 0..loops {
            // canvas->drawImage(fImage.get(), 0, 0, sampling, &fPaint);
            canvas.draw_image_with_sampling_options(image, (0.0, 0.0), sampling, Some(&self.paint));
        }
    }
}

// Port of: bench/DrawBitmapAABench.cpp#L58-L58 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_false_ident = "DrawBitmapAABench(false, SkMatrix::I(), \"ident\")",
    DrawBitmapAABench::new(false, Matrix::default(), "ident")
);
// Port of: bench/DrawBitmapAABench.cpp#L60-L60 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_false_scale =
        "DrawBitmapAABench(false, SkMatrix::Scale(1.17f, 1.17f), \"scale\")",
    DrawBitmapAABench::new(false, Matrix::scale((1.17, 1.17)), "scale")
);
// Port of: bench/DrawBitmapAABench.cpp#L62-L62 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_false_translate =
        "DrawBitmapAABench(false, SkMatrix::Translate(17.5f, 17.5f), \"translate\")",
    DrawBitmapAABench::new(false, Matrix::translate((17.5, 17.5)), "translate")
);
// Port of: bench/DrawBitmapAABench.cpp#L64-L69 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_false_rotate = "SkMatrix m; m.reset(); m.preRotate(15); return new DrawBitmapAABench(false, m, \"rotate\")",
    DrawBitmapAABench::new(false, rotate_15(), "rotate")
);
// Port of: bench/DrawBitmapAABench.cpp#L71-L71 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_true_ident = "DrawBitmapAABench(true, SkMatrix::I(), \"ident\")",
    DrawBitmapAABench::new(true, Matrix::default(), "ident")
);
// Port of: bench/DrawBitmapAABench.cpp#L73-L73 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_true_scale =
        "DrawBitmapAABench(true, SkMatrix::Scale(1.17f, 1.17f), \"scale\")",
    DrawBitmapAABench::new(true, Matrix::scale((1.17, 1.17)), "scale")
);
// Port of: bench/DrawBitmapAABench.cpp#L75-L75 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_true_translate =
        "DrawBitmapAABench(true, SkMatrix::Translate(17.5f, 17.5f), \"translate\")",
    DrawBitmapAABench::new(true, Matrix::translate((17.5, 17.5)), "translate")
);
// Port of: bench/DrawBitmapAABench.cpp#L77-L82 (chrome/m156)
def_bench!(
    draw_bitmap_aa_bench_true_rotate =
        "SkMatrix m; m.reset(); m.preRotate(15); return new DrawBitmapAABench(true, m, \"rotate\")",
    DrawBitmapAABench::new(true, rotate_15(), "rotate")
);

/// `SkMatrix m; m.reset(); m.preRotate(15);` (the rotate registrations).
// Port of: bench/DrawBitmapAABench.cpp#L64-L69 (chrome/m156)
fn rotate_15() -> Matrix {
    let mut m = Matrix::default();
    m.reset();
    m.pre_rotate(15.0, None);
    m
}
