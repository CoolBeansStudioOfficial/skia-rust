// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ImageFilterCollapse.cpp

//! Chains of colour filters as image filters, drawn over a 400×400 gradient bitmap. The two
//! benches are `TableCollapseBench` (three table filters) and `MatrixCollapseBench` (two
//! brightness matrices around a grayscale matrix). Rendering benches.

use skia_rust_core::color::colors;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader::{self, GradientShaderColors};
use skia_rust_effects::image_filters;
use skia_rust_raster::surfaces;

use crate::def_bench;
use crate::prelude::*;

/// `BaseImageFilterCollapseBench`: the chain of image filters built in `doPreDraw`, and the
/// bitmap drawn through it.
// Port of: bench/ImageFilterCollapse.cpp#L14-L47 (chrome/m156)
struct BaseImageFilterCollapseBench {
    image_filter: Option<ImageFilter>,
    image: Option<Image>,
}

impl BaseImageFilterCollapseBench {
    fn new() -> Self {
        Self {
            image_filter: None,
            image: None,
        }
    }

    /// `doPreDraw(colorFilters, nFilters)`: `fImageFilter = ColorFilter(colorFilters[i],
    /// fImageFilter)` for `i` from the last filter to the first.
    // Port of: bench/ImageFilterCollapse.cpp#L20-L27 (chrome/m156)
    fn do_pre_draw(&mut self, color_filters: Vec<ColorFilter>) {
        assert!(self.image_filter.is_none());
        for color_filter in color_filters.into_iter().rev() {
            self.image_filter =
                image_filters::color_filter(Some(color_filter), self.image_filter.take(), None);
        }
    }

    /// `onDraw`: makes the bitmap, then draws it through the filter chain `loops` times.
    // Port of: bench/ImageFilterCollapse.cpp#L28-L36 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: &Canvas) {
        self.make_bitmap();
        for _ in 0..loops {
            // SkPaint paint; paint.setImageFilter(fImageFilter);
            let mut paint = Paint::default();
            paint.set_image_filter(self.image_filter.clone());
            // canvas->drawImage(fImage, 0, 0, SkSamplingOptions(), &paint);
            canvas.draw_image_with_sampling_options(
                self.image.as_ref().expect("makeBitmap ran"),
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
        }
    }

    /// `makeBitmap()`: a seven-colour linear gradient drawn over a 400×400 premultiplied raster.
    // Port of: bench/ImageFilterCollapse.cpp#L38-L50 (chrome/m156)
    fn make_bitmap(&mut self) {
        let w = 400;
        let h = 400;
        let mut surf = surfaces::raster_n32_premul((w, h)).expect("400x400 N32 raster surface");
        // SkPoint pts[] = { {0, 0}, {SkIntToScalar(W), SkIntToScalar(H)} };
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(int_to_scalar(w), int_to_scalar(h)),
        ];
        // SkColor4f colors[] = { kBlack, kGreen, kCyan, kRed, kTransparent, kBlue, kWhite };
        let grad_colors = [
            colors::BLACK,
            colors::GREEN,
            colors::CYAN,
            colors::RED,
            colors::TRANSPARENT,
            colors::BLUE,
            colors::WHITE,
        ];
        let mut paint = Paint::default();
        // paint.setShader(SkShaders::LinearGradient(pts, {{colors, {}, kClamp}, {}}));
        let shader: Option<Shader> = gradient_shader::linear(
            (pts[0], pts[1]),
            GradientShaderColors::from(grad_colors.as_slice()),
            None,
            TileMode::Clamp,
            None,
            None,
        );
        paint.set_shader(shader);
        surf.canvas().draw_paint(&paint);
        // fImage = surf->makeImageSnapshot();
        self.image = surf.image_snapshot();
    }
}

/// `class TableCollapseBench`.
// Port of: bench/ImageFilterCollapse.cpp#L53-L76 (chrome/m156)
struct TableCollapseBench {
    base: BaseImageFilterCollapseBench,
}

impl Benchmark for TableCollapseBench {
    fn name(&self) -> String {
        "image_filter_collapse_table".to_owned()
    }

    // Port of: bench/ImageFilterCollapse.cpp#L58-L72 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let mut table1 = [0u8; 256];
        let mut table2 = [0u8; 256];
        let mut table3 = [0u8; 256];
        // The index drives three tables and the arithmetic, as in the C++ loop.
        #[allow(clippy::needless_range_loop)]
        for i in 0..256usize {
            let n = u8::try_from(i >> 5).expect("i >> 5 is below 8");
            // table1[i] = (n << 5) | (n << 2) | (n >> 1);
            table1[i] = (n << 5) | (n << 2) | (n >> 1);
            // table2[i] = i * i / 255;
            table2[i] = u8::try_from(i * i / 255).expect("i * i / 255 fits in a byte");
            // float fi = i / 255.0f; table3[i] = static_cast<uint8_t>(sqrtf(fi) * 255);
            #[allow(clippy::cast_precision_loss)] // mirrors the int-to-float conversion in C++
            let fi = i as scalar / 255.0;
            // The C++ static_cast truncates the non-negative product to uint8_t.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let scaled = (fi.sqrt() * 255.0) as u8;
            table3[i] = scaled;
        }
        let color_filters_list = vec![
            color_filters::table(&table1),
            color_filters::table(&table2),
            color_filters::table(&table3),
        ];
        self.base.do_pre_draw(color_filters_list);
    }

    // Port of: bench/ImageFilterCollapse.cpp#L53-L76 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TableCollapseBench is a rendering bench");
        self.base.on_draw(loops, canvas);
    }
}

/// `make_brightness(amount)`: `SkColorFilters::Matrix` with the amount in the offset column.
// Port of: bench/ImageFilterCollapse.cpp#L78-L84 (chrome/m156)
fn make_brightness(amount: scalar) -> ColorFilter {
    let matrix: [scalar; 20] = [
        1.0, 0.0, 0.0, 0.0, amount, //
        0.0, 1.0, 0.0, 0.0, amount, //
        0.0, 0.0, 1.0, 0.0, amount, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    color_filters::matrix_row_major(&matrix, Clamp::Yes).expect("finite brightness matrix")
}

/// `make_grayscale()`: the luminance matrix.
// Port of: bench/ImageFilterCollapse.cpp#L86-L95 (chrome/m156)
fn make_grayscale() -> ColorFilter {
    let mut matrix: [scalar; 20] = [0.0; 20];
    matrix[0] = 0.2126;
    matrix[5] = 0.2126;
    matrix[10] = 0.2126;
    matrix[1] = 0.7152;
    matrix[6] = 0.7152;
    matrix[11] = 0.7152;
    matrix[2] = 0.0722;
    matrix[7] = 0.0722;
    matrix[12] = 0.0722;
    matrix[18] = 1.0;
    color_filters::matrix_row_major(&matrix, Clamp::Yes).expect("finite grayscale matrix")
}

/// `class MatrixCollapseBench`.
// Port of: bench/ImageFilterCollapse.cpp#L97-L113 (chrome/m156)
struct MatrixCollapseBench {
    base: BaseImageFilterCollapseBench,
}

impl Benchmark for MatrixCollapseBench {
    fn name(&self) -> String {
        "image_filter_collapse_matrix".to_owned()
    }

    // Port of: bench/ImageFilterCollapse.cpp#L102-L109 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let color_filters_list = vec![
            make_brightness(0.1),
            make_grayscale(),
            make_brightness(-0.1),
        ];
        self.base.do_pre_draw(color_filters_list);
    }

    // Port of: bench/ImageFilterCollapse.cpp#L97-L113 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("MatrixCollapseBench is a rendering bench");
        self.base.on_draw(loops, canvas);
    }
}

// Port of: bench/ImageFilterCollapse.cpp#L133-L133 (chrome/m156)
def_bench!(
    table_collapse_bench = "TableCollapseBench",
    TableCollapseBench {
        base: BaseImageFilterCollapseBench::new(),
    }
);
// Port of: bench/ImageFilterCollapse.cpp#L134-L134 (chrome/m156)
def_bench!(
    matrix_collapse_bench = "MatrixCollapseBench",
    MatrixCollapseBench {
        base: BaseImageFilterCollapseBench::new(),
    }
);
