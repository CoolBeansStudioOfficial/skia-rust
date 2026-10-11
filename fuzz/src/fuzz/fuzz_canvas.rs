// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/FuzzCanvas.cpp

//! The `FuzzCanvas` fuzzers (`fuzz/FuzzCanvas.cpp`): each one runs the random drawing commands of
//! [`crate::canvas_helpers::fuzz_canvas`] on a different kind of canvas.

use skia_rust_codec::codecs;
use skia_rust_codec::encode::png_encoder;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, CanvasHooks, SaveLayerRec, SaveLayerStrategy};
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture_priv;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::serial_procs::{ImageSerializer, SerialProcs};
use skia_rust_core::stream::NullWStream;
use skia_rust_effects::flattenable::REGISTRY as EFFECTS_REGISTRY;
use skia_rust_pdf::document::new_document;
use skia_rust_pdf::jpeg::metadata_with_callbacks;
use skia_rust_raster::surfaces;
use skia_rust_svg::canvas::{SharedStream, make as svg_canvas};
use skia_rust_svg::device::Options as SvgOptions;

use crate::canvas_helpers::{canvas_on_bitmap, fuzz_canvas, make_fuzz_image_filter};
use crate::def_fuzz;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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

// Port of: fuzz/FuzzCanvas.cpp#L63-L84 (chrome/m156)
// skia-rust: the C++ fuzzer is compiled only with `SK_CODEC_DECODES_PNG_WITH_LIBPNG` and
// `SK_CODEC_ENCODES_PNG_WITH_LIBPNG`, which the oracle build defines; it is ported unconditionally.
//
// Skia signals a bug on every input here, so the replay is `#[ignore]`d and the entry `failing`.
// `SkPicture::serialize` writes the stream form (the info, the sentinel byte 1, then the
// `SkPictureData`), but `SkPicturePriv::MakeFromBuffer` reads a 32-bit sentinel of 1 straight
// after the info (SkPicture.cpp#L226-L252). The byte after the info is 1 followed by the first
// three bytes of the `SK_PICT_READER_TAG`, so the sentinel is not 1 and MakeFromBuffer returns
// null. The port follows Skia, so it does not diverge to make the replay pass.
def_fuzz!(
    RasterN32CanvasViaSerialization,
    fuzz_RasterN32CanvasViaSerialization,
    ignore = "Skia bug: SkPicture::serialize output is rejected by SkPicturePriv::MakeFromBuffer on every input (SkPicture.cpp#L226-L252 reads a sentinel of 1 where serialize wrote the stream form), so Skia's fuzzer signals a bug on every input",
    |fuzz| {
        let mut recorder = PictureRecorder::new();
        let canvas = recorder.begin_recording(
            Rect::new(0.0, 0.0, CANVAS_SCALARS.0, CANVAS_SCALARS.1),
            false,
        );
        fuzz_canvas(fuzz, canvas, 9);
        let Some(pic) = recorder.finish_recording_as_picture(None) else {
            fuzz.signal_bug();
        };
        // `sProcs.fImageProc`: `SkPngEncoder::Encode(nullptr, img, SkPngEncoder::Options{})`.
        let image_proc: ImageSerializer =
            Arc::new(|img| png_encoder::encode_image(img, &png_encoder::Options::default()));
        let procs = SerialProcs {
            image: Some(image_proc),
            ..SerialProcs::default()
        };
        let Some(data) = pic.serialize(Some(&procs)) else {
            fuzz.signal_bug();
        };
        let mut rb = ReadBuffer::new(data.as_bytes());
        // `SkCodecs::Register(SkPngDecoder::Decoder())`: the PNG decoder replaces its own entry.
        if let Some(png) = codecs::decoders().iter().find(|d| d.id == "png") {
            codecs::register(*png);
        }
        let Some(deserialized) = picture_priv::make_from_buffer(&mut rb, &EFFECTS_REGISTRY) else {
            fuzz.signal_bug();
        };
        let Some(mut surface) = surfaces::raster_n32_premul(CANVAS_SIZE) else {
            // SkASSERT(surface && surface->getCanvas()): debug only.
            debug_assert!(false, "a 128x160 N32 raster surface");
            return;
        };
        surface.canvas().draw_picture(&deserialized, None, None);
    }
);

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

// Port of: fuzz/FuzzCanvas.cpp#L103-L165 (chrome/m156)
def_fuzz!(SerializedImageFilter, fuzz_SerializedImageFilter, |fuzz| {
    let mut bitmap = Bitmap::new();
    if !bitmap.try_alloc_n32_pixels((256, 256), false) {
        // skia-rust: SkDEBUGF("Could not allocate 256x256 bitmap in SerializedImageFilter").
        return;
    }

    let Some(filter) = make_fuzz_image_filter(fuzz, 20) else {
        return;
    };
    let data = filter.serialize(None);
    let mut bytes = data.as_bytes().to_vec();
    // `SK_ADD_RANDOM_BIT_FLIPS`: the reads depend on the length of the serialized filter, so the
    // loop runs once per byte, as Skia's does.
    for byte in &mut bytes {
        let j: u8 = fuzz.next_range(1, 250);
        if j == 1 {
            // 0.4% of the time, flip a bit or byte
            let k: u8 = fuzz.next_range(1, 10);
            if k == 1 {
                // Then 10% of the time, change a whole byte
                let s: u8 = fuzz.next_range(0, 2);
                match s {
                    0 => *byte ^= 0xFF, // Flip entire byte
                    1 => *byte = 0xFF,  // Set all bits to 1
                    2 => *byte = 0x00,  // Set all bits to 0
                    _ => {}
                }
            } else {
                let _s: u8 = fuzz.next_range(0, 7);
                *byte ^= 1 << 7;
            }
        }
    }
    let deserialized_fil = ImageFilter::deserialize(&bytes, None, &EFFECTS_REGISTRY);

    let mut paint = Paint::default();
    paint.set_image_filter(deserialized_fil);

    let canvas = canvas_on_bitmap(bitmap);
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&Rect::new(0.0, 0.0, 256.0, 256.0))
            .paint(&paint),
    );
    canvas.restore();
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

// Port of: fuzz/FuzzCanvas.cpp#L237-L242 (chrome/m156)
def_fuzz!(SVGCanvas, fuzz_SVGCanvas, |fuzz| {
    let stream: SharedStream = Rc::new(RefCell::new(NullWStream::new()));
    let bounds = Rect::new(0.0, 0.0, 150.0, 150.0); // SkRect::MakeIWH(150, 150)
    let canvas = svg_canvas(&bounds, stream, SvgOptions::default());
    fuzz_canvas(fuzz, &canvas, 9);
});
