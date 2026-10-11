// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/FuzzCanvas.cpp

//! The `FuzzCanvas` fuzzers (`fuzz/FuzzCanvas.cpp`): each one runs the random drawing commands of
//! [`crate::canvas_helpers::fuzz_canvas`] on a different kind of canvas.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, CanvasHooks, SaveLayerRec, SaveLayerStrategy};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::stream::NullWStream;
use skia_rust_pdf::document::new_document;
use skia_rust_pdf::jpeg::metadata_with_callbacks;
use skia_rust_raster::surfaces;

use crate::canvas_helpers::{canvas_on_bitmap, fuzz_canvas, make_fuzz_image_filter};
use crate::def_fuzz;

/// `kCanvasSize`.
const CANVAS_SIZE: (i32, i32) = (128, 160);
/// `kCanvasSize` as the `SkScalar`s the PDF page takes.
const CANVAS_SCALARS: (f32, f32) = (128.0, 160.0);

/// The canvas `SkMakeNullCanvas()` returns: `SkNWayCanvas(0, 0)`, which forwards each call to the
/// canvases it holds, and it holds none. It keeps the state of a canvas with no pixels (the
/// matrix, the clip and the save stack) and draws nothing. Its save layers have no layer, as
/// `SkNWayCanvas::getSaveLayerStrategy` returns `kNoLayer_SaveLayerStrategy`.
///
/// skia-rust: the `SkNWayCanvas` class itself is not ported. With no canvases to forward to, its
/// overrides reduce to these hooks, so the behaviour is the same.
// Port of: src/utils/SkNullCanvas.cpp#L11-L15 (chrome/m156), SkMakeNullCanvas
#[derive(Debug)]
struct NullCanvasHooks;

impl CanvasHooks for NullCanvasHooks {
    // Port of: src/utils/SkNWayCanvas.cpp#L120-L128 (chrome/m156), getSaveLayerStrategy
    fn get_save_layer_strategy(&mut self, _rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        SaveLayerStrategy::NoLayer
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L213-L255 (chrome/m156), the draw overrides: each
    // forwards to the (empty) list of canvases and does not draw itself.
    fn on_draw_paint(&mut self, _paint: &Paint) -> bool {
        true
    }
    fn on_draw_points(
        &mut self,
        _mode: skia_rust_core::canvas::PointMode,
        _pts: &[skia_rust_core::point::Point],
        _paint: &Paint,
    ) -> bool {
        true
    }
    fn on_draw_rect(&mut self, _rect: &Rect, _paint: &Paint) -> bool {
        true
    }
    fn on_draw_region(&mut self, _region: &skia_rust_core::region::Region, _paint: &Paint) -> bool {
        true
    }
    fn on_draw_oval(&mut self, _oval: &Rect, _paint: &Paint) -> bool {
        true
    }
    fn on_draw_rrect(&mut self, _rrect: &skia_rust_core::rrect::RRect, _paint: &Paint) -> bool {
        true
    }
    fn on_draw_drrect(
        &mut self,
        _outer: &skia_rust_core::rrect::RRect,
        _inner: &skia_rust_core::rrect::RRect,
        _paint: &Paint,
    ) -> bool {
        true
    }
    fn on_draw_path(&mut self, _path: &skia_rust_core::path::Path, _paint: &Paint) -> bool {
        true
    }
}

/// `SkMakeNullCanvas()`: a canvas with no pixels and the null canvas's hooks.
#[must_use]
fn make_null_canvas() -> Canvas {
    let canvas = Canvas::new_no_pixels_irect(&IRect::new_empty(), None);
    canvas.set_hooks(Some(Box::new(NullCanvasHooks)));
    canvas
}

// Port of: fuzz/FuzzCanvas.cpp#L47-L49 (chrome/m156)
def_fuzz!(NullCanvas, fuzz_NullCanvas, |fuzz| {
    let canvas = make_null_canvas();
    fuzz_canvas(fuzz, &canvas, 9);
});

// Port of: fuzz/FuzzCanvas.cpp#L53-L58 (chrome/m156)
def_fuzz!(RasterN32Canvas, fuzz_RasterN32Canvas, |fuzz| {
    let Some(mut surface) = surfaces::raster_n32_premul(CANVAS_SIZE) else {
        fuzz.signal_bug();
    };
    fuzz_canvas(fuzz, surface.canvas(), 9);
});

// Port of: fuzz/FuzzCanvas.cpp#L88-L101 (chrome/m156)
def_fuzz!(ImageFilter, fuzz_ImageFilter, |fuzz| {
    let mut bitmap = Bitmap::new();
    if !bitmap.try_alloc_n32_pixels((256, 256), false) {
        // skia-rust: SkDEBUGF("Could not allocate 256x256 bitmap in ImageFilter").
        return;
    }

    let fil = make_fuzz_image_filter(fuzz, 20);

    let mut paint = Paint::default();
    paint.set_image_filter(fil);
    let canvas = canvas_on_bitmap(bitmap);
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&Rect::new(0.0, 0.0, 500.0, 500.0))
            .paint(&paint),
    );
});

// Port of: fuzz/FuzzCanvas.cpp#L214-L219 (chrome/m156)
def_fuzz!(PDFCanvas, fuzz_PDFCanvas, |fuzz| {
    let mut stream = NullWStream::new();
    let metadata = metadata_with_callbacks();
    let mut doc = new_document(&mut stream, Some(&metadata));
    // A null canvas makes `FuzzCanvas` return before it reads anything.
    if let Some(canvas) = doc.begin_page(CANVAS_SCALARS.0, CANVAS_SCALARS.1, None) {
        fuzz_canvas(fuzz, canvas, 9);
    }
});
