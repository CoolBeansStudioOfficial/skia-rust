// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ColorFilterBench.cpp

//! Filtered rectangles (`FilteredRectBench`: no filter, a colour filter, an image filter that
//! wraps the colour filter) and `FilterColorBench` (`filterColor4f` in a loop, non-rendering).
//!
//! Not ported (manifest status stays `todo`):
//! - `FilteredRectBench(kRuntimeColorFilter_Type)` and `FilterColorBench("runtime_filtercolor4f")`:
//!   they build a `SkRuntimeEffect` from `SkSL`, which the port does not have.
//! - The `ColorFilterBench` registrations: they draw `images/mandrill_256.png`, which needs a PNG
//!   decoder the port does not have, and some use colour filters the port lacks (`Lerp`,
//!   `HighContrast`, `Overdraw`, the Gaussian filter); the two `*_runtime` ones are `SkSL`.

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::rect::Rect;
use skia_rust_effects::image_filters;

use crate::def_bench;
use crate::prelude::*;

/// `static constexpr float kGrayscaleMatrix[]` (20 row-major coefficients).
// Port of: bench/ColorFilterBench.cpp#L28-L34 (chrome/m156)
const GRAYSCALE_MATRIX: [f32; 20] = [
    0.2126, 0.7152, 0.0722, 0.0, 0.0, //
    0.2126, 0.7152, 0.0722, 0.0, 0.0, //
    0.2126, 0.7152, 0.0722, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// `make_grayscale()`: `SkColorFilters::Matrix(kGrayscaleMatrix)`.
// Port of: bench/ColorFilterBench.cpp#L39-L41 (chrome/m156)
fn make_grayscale() -> Option<ColorFilter> {
    color_filters::matrix_row_major(&GRAYSCALE_MATRIX, Clamp::Yes)
}

/// `class FilteredRectBench`: a red 256×256 rectangle with no filter, a colour filter, or an
/// image filter wrapping the colour filter.
// Port of: bench/ColorFilterBench.cpp#L52-L104 (chrome/m156)
struct FilteredRectBench {
    kind: FilterKind,
    paint: Paint,
    name: String,
}

/// `FilteredRectBench::Type` (the runtime-colour-filter variant is not ported; see the module
/// docs).
// Port of: bench/ColorFilterBench.cpp#L55-L60 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// The variant names mirror the C++ enumerators (`kNoFilter_Type`, ...).
#[allow(clippy::enum_variant_names)]
enum FilterKind {
    NoFilter,
    ColorFilter,
    ImageFilter,
}

impl FilteredRectBench {
    // Port of: bench/ColorFilterBench.cpp#L62-L75 (chrome/m156)
    fn new(kind: FilterKind) -> Self {
        // static constexpr auto kSuffix = {"nofilter", "colorfilter", "imagefilter", ...};
        let suffix = match kind {
            FilterKind::NoFilter => "nofilter",
            FilterKind::ColorFilter => "colorfilter",
            FilterKind::ImageFilter => "imagefilter",
        };
        let mut paint = Paint::default();
        // fPaint.setColor(SK_ColorRED);
        paint.set_color(Color::RED);
        Self {
            kind,
            paint,
            // fName.printf("filteredrect_%s", kSuffix[t]);
            name: format!("filteredrect_{suffix}"),
        }
    }
}

impl Benchmark for FilteredRectBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ColorFilterBench.cpp#L77-L93 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        match self.kind {
            FilterKind::NoFilter => {}
            FilterKind::ColorFilter => {
                // fPaint.setColorFilter(make_grayscale());
                self.paint.set_color_filter(make_grayscale());
            }
            FilterKind::ImageFilter => {
                // fPaint.setImageFilter(SkImageFilters::ColorFilter(make_grayscale(), nullptr));
                self.paint.set_image_filter(image_filters::color_filter(
                    make_grayscale(),
                    None,
                    None,
                ));
            }
        }
    }

    // Port of: bench/ColorFilterBench.cpp#L95-L102 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("FilteredRectBench is a rendering bench");
        // const SkRect r = { 0, 0, 256, 256 };
        let r = Rect::from_iwh(256, 256);
        for _ in 0..loops {
            canvas.draw_rect(r, &self.paint);
        }
    }
}

/// `class FilterColorBench`: `filterColor4f` applied `loops` times, with no canvas.
// Port of: bench/ColorFilterBench.cpp#L268-L279 (chrome/m156)
struct FilterColorBench {
    name: &'static str,
    filter_fn: fn() -> Option<ColorFilter>,
    color_filter: Option<ColorFilter>,
}

impl Benchmark for FilterColorBench {
    fn name(&self) -> String {
        self.name.to_owned()
    }

    // Port of: bench/ColorFilterBench.cpp#L271-L271 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/ColorFilterBench.cpp#L274-L276 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.color_filter = (self.filter_fn)();
    }

    // Port of: bench/ColorFilterBench.cpp#L277-L286 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // SkColor4f c = { 1.f, 1.f, 0.f, 1.0f };
        let mut c = Color4f::new(1.0, 1.0, 0.0, 1.0);
        let filter = self
            .color_filter
            .as_ref()
            .expect("FilterColorBench has a colour filter after delayed setup");
        for _ in 0..loops {
            // c = fColorFilter->filterColor4f(c, /*srcCS=*/nullptr, /*dstCS=*/nullptr);
            c = filter.filter_color4f(c, None, None);
        }
    }
}

// Port of: bench/ColorFilterBench.cpp#L255-L255 (chrome/m156)
def_bench!(
    filtered_rect_bench_nofilter = "FilteredRectBench(FilteredRectBench::kNoFilter_Type)",
    FilteredRectBench::new(FilterKind::NoFilter)
);
// Port of: bench/ColorFilterBench.cpp#L256-L256 (chrome/m156)
def_bench!(
    filtered_rect_bench_colorfilter = "FilteredRectBench(FilteredRectBench::kColorFilter_Type)",
    FilteredRectBench::new(FilterKind::ColorFilter)
);
// Port of: bench/ColorFilterBench.cpp#L257-L257 (chrome/m156)
def_bench!(
    filtered_rect_bench_imagefilter = "FilteredRectBench(FilteredRectBench::kImageFilter_Type)",
    FilteredRectBench::new(FilterKind::ImageFilter)
);
// Port of: bench/ColorFilterBench.cpp#L267-L267 (chrome/m156)
def_bench!(
    filter_color_bench_matrix = "FilterColorBench(\"matrix_filtercolor4f\", &make_grayscale)",
    FilterColorBench {
        name: "matrix_filtercolor4f",
        filter_fn: make_grayscale,
        color_filter: None,
    }
);
