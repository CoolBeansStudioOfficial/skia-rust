// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ImageBench.cpp

//! `Image2RasterBench`: draws an image made from the canvas's own surface type into a CPU raster
//! surface (`native_image_to_raster_surface`). Only the raster backend is in scope; the Ganesh
//! half of `isSuitableFor` is dropped with Ganesh (docs/design/bench.md §2.2).

use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::prelude::*;

/// `class Image2RasterBench`.
// Port of: bench/ImageBench.cpp#L13-L64 (chrome/m156)
#[derive(Default)]
struct Image2RasterBench {
    image: Option<Image>,
    raster_surface: Option<Surface<'static>>,
}

impl Benchmark for Image2RasterBench {
    // fName.set("native_image_to_raster_surface");
    fn name(&self) -> String {
        "native_image_to_raster_surface".to_owned()
    }

    // isSuitableFor(Backend backend): kGanesh || kRaster.
    // Port of: bench/ImageBench.cpp#L13-L64 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::Raster
    }

    // onPerCanvasPreDraw(SkCanvas* canvas)
    // Port of: bench/ImageBench.cpp#L13-L64 (chrome/m156)
    fn on_per_canvas_pre_draw(&mut self, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("Image2RasterBench is a rendering bench");
        // create an Image reflecting the canvas (gpu or cpu)
        // SkImageInfo info = canvas->imageInfo().makeWH(100, 100);
        let info: ImageInfo = canvas.image_info().with_wh(100, 100);
        // auto surface(canvas->makeSurface(info));
        let mut surface = canvas
            .new_surface(&info, None)
            .expect("canvas makes a surface of its own type");
        // canvas->drawColor(SK_ColorRED);
        canvas.draw_color(Color::new(0xFFFF_0000), None);
        // fImage = surface->makeImageSnapshot();
        self.image = Some(
            surface
                .image_snapshot()
                .expect("snapshot of a fresh surface"),
        );

        // create a cpu-backed Surface
        // SkImageInfo n32Info = SkImageInfo::MakeN32Premul(100, 100);
        // fRasterSurface = SkSurfaces::Raster(n32Info);
        self.raster_surface =
            Some(surfaces::raster_n32_premul((100, 100)).expect("100x100 N32 raster surface"));
    }

    // onPerCanvasPostDraw(SkCanvas*)
    // Port of: bench/ImageBench.cpp#L13-L64 (chrome/m156)
    fn on_per_canvas_post_draw(&mut self, _canvas: Option<&Canvas>) {
        // Release the image and raster surface here to prevent out of order destruction
        // between these and the gpu interface.
        // fRasterSurface.reset(nullptr);
        self.raster_surface = None;
        // fImage.reset(nullptr);
        self.image = None;
    }

    // onDraw(int loops, SkCanvas*)
    // Port of: bench/ImageBench.cpp#L13-L64 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let image = self.image.as_ref().expect("set in onPerCanvasPreDraw");
        let surface = self
            .raster_surface
            .as_mut()
            .expect("set in onPerCanvasPreDraw");
        for _ in 0..loops {
            for _ in 0..10 {
                // fRasterSurface->getCanvas()->drawImage(fImage.get(), 0, 0);
                surface.canvas().draw_image(image, (0.0, 0.0), None);
            }
        }
    }
}

// Port of: bench/ImageBench.cpp#L65-L65 (chrome/m156)
crate::def_bench!(
    image_bench_image2_raster_bench = "Image2RasterBench",
    Image2RasterBench::default()
);
