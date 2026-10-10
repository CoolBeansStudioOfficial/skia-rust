// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The surface a test draws on and reads back. The C++ tests take an `SkSurface*` and run the same
//! body on a raster, a Ganesh or a Graphite surface; the ports do the same through this trait, so
//! one body serves the raster and Graphite variants.
#![cfg(not(target_arch = "wasm32"))]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_raster::surface::Surface;

/// `SkSurface`, as the tests use it: its image info and canvas, and `readPixels` into a bitmap.
pub trait TestSurface {
    /// `imageInfo()`.
    fn image_info(&self) -> ImageInfo;
    /// `getCanvas()`.
    fn canvas(&mut self) -> &Canvas;
    /// `readPixels(bitmap.info(), bitmap.getPixels(), bitmap.rowBytes(), 0, 0)`, and whether it
    /// succeeded.
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool;
}

impl TestSurface for Surface<'_> {
    fn image_info(&self) -> ImageInfo {
        Surface::image_info(self)
    }
    fn canvas(&mut self) -> &Canvas {
        Surface::canvas(self)
    }
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool {
        self.read_pixels_to_bitmap(bitmap, (0, 0))
    }
}

/// A Graphite surface with the context that reads it back (`context->readPixels` of
/// `Device::onReadPixels`).
pub struct GraphiteTestSurface<'a> {
    /// The context the surface is read back through.
    pub context: &'a mut WgpuContext,
    /// The surface.
    pub surface: &'a GraphiteSurface,
}

impl std::fmt::Debug for GraphiteTestSurface<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The context has no `Debug`; the surface identifies the test surface.
        f.debug_struct("GraphiteTestSurface")
            .field("surface", self.surface)
            .finish_non_exhaustive()
    }
}

impl TestSurface for GraphiteTestSurface<'_> {
    fn image_info(&self) -> ImageInfo {
        self.surface.image_info().clone()
    }
    fn canvas(&mut self) -> &Canvas {
        self.surface.canvas()
    }
    fn read_pixels(&mut self, bitmap: &mut Bitmap) -> bool {
        let Some(mut pm) = bitmap.peek_pixels_mut() else {
            return false;
        };
        self.context
            .read_surface_pixels(self.surface, &mut pm, 0, 0)
    }
}
