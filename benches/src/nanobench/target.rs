// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (Target, is_enabled)

//! `Target`: the surface a benchmark draws into for one config.

use skia_rust_core::canvas::Canvas;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use super::config::Config;
use crate::{Backend, Benchmark};

/// Port of `Target` (CPU only).
// Port of: bench/nanobench.cpp#L207-L238 (chrome/m156)
#[derive(Debug)]
pub struct Target {
    pub config: Config,
    surface: Option<Surface<'static>>,
}

impl Target {
    /// `Target::init()`: a raster surface for the raster backend, nothing for nonrendering.
    // Port of: bench/nanobench.cpp#L249-L256 (chrome/m156)
    fn init(config: Config, info: &ImageInfo) -> Option<Self> {
        let surface = if config.backend == Backend::Raster {
            Some(surfaces::raster(info, None, None)?)
        } else {
            None
        };
        Some(Self { config, surface })
    }

    /// `getCanvas()`: null (`None`) for nonrendering.
    pub fn canvas(&mut self) -> Option<&Canvas> {
        self.surface.as_mut().map(Surface::canvas)
    }

    /// The surface's pixels, row by row without padding, for `--writeRaw` (the layout of
    /// `OracleDump`).
    #[must_use]
    pub fn packed_bytes(&mut self) -> Option<Vec<u8>> {
        let surface = self.surface.as_mut()?;
        let info = surface.image_info();
        let height = usize::try_from(info.height()).ok()?;
        let peeked = surface.peek_pixels()?;
        let pixmap = peeked.pixmap();
        let row_bytes = pixmap.row_bytes();
        let row = info.min_row_bytes();
        let bytes = pixmap.bytes()?;
        let mut out = Vec::with_capacity(row * height);
        for y in 0..height {
            out.extend_from_slice(&bytes[y * row_bytes..y * row_bytes + row]);
        }
        Some(out)
    }
}

/// `is_enabled()`: a `Target` for `bench` on `config`, or `None` when the bench is not suitable
/// for the backend or the surface cannot be made.
// Port of: bench/nanobench.cpp#L752-L781 (chrome/m156)
#[must_use]
pub fn is_enabled(bench: &mut dyn Benchmark, config: Config) -> Option<Target> {
    if !bench.is_suitable_for(config.backend) {
        return None;
    }
    let info = ImageInfo::new(bench.size(), config.color, config.alpha, None);
    Target::init(config, &info)
}
