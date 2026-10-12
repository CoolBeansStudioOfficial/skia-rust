// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/StripGenerator.h, src/gpu/graphite/sparse_strips/StripGenerator.cpp

//! [`StripGenerator`]: flattens, tiles and rasterizes a path into `EndCaps` and `WideTiles`.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;

use crate::graphite::geom::end_caps::EndCaps;
use crate::graphite::geom::wide_tiles::WideTiles;
use crate::graphite::recorder::Recorder;
use crate::graphite::sparse_strips::config::SparseStripConfig;
use crate::graphite::sparse_strips::flatten::Flatten;
use crate::graphite::sparse_strips::make_strips::MakeStrips;
use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::tiler::Tiles;
use crate::graphite::sparse_strips::types::FlattenMode;

/// `StripGenerator`: generates the strips of one path at a time. The strips of the last
/// [`process_geometry`](Self::process_geometry) are in [`ends`](Self::ends) and
/// [`wides`](Self::wides).
// Port of: src/gpu/graphite/sparse_strips/StripGenerator.h#L23-L46 (chrome/m156)
#[doc(alias = "skgpu::graphite::StripGenerator")]
#[derive(Debug)]
pub struct StripGenerator<'a> {
    width: i32,
    height: i32,
    mask_lut: &'a [u8],
    ends: EndCaps,
    wides: WideTiles,
}

impl<'a> StripGenerator<'a> {
    /// `StripGenerator(width, height, maskLUT, recorder)`. The recorder is passed to
    /// [`process_geometry`](Self::process_geometry) and
    /// [`cleanup_null_caps`](Self::cleanup_null_caps) instead.
    // Port of: src/gpu/graphite/sparse_strips/StripGenerator.cpp#L23-L32 (chrome/m156)
    #[must_use]
    pub fn new(width: i32, height: i32, mask_lut: &'a [u8]) -> Self {
        Self {
            width,
            height,
            mask_lut,
            ends: EndCaps::new(),
            wides: WideTiles::new(),
        }
    }

    /// `processGeometry(path, ctm)`: false if the strips could not be generated, e.g. because the
    /// alpha atlas ran out of space.
    ///
    /// # Panics
    /// If `width` or `height` do not fit a `u16`, which the C++ truncates.
    // Port of: src/gpu/graphite/sparse_strips/StripGenerator.cpp#L39-L84 (chrome/m156)
    #[doc(alias = "processGeometry")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // mirrors the C++ conversions of `int` dimensions
    pub fn process_geometry(&mut self, recorder: &Recorder, path: &Path, ctm: &Matrix) -> bool {
        self.ends.clear();
        self.wides.clear();

        let rp = recorder.priv_();
        let mut atlas_provider = rp.atlas_provider().borrow_mut();
        let atlas_manager = atlas_provider.alpha_atlas_manager();

        let mut flattener = Flatten::new();
        let mut polyline = Polyline::new();
        flattener.process_paths(
            FlattenMode::Simd,
            path,
            ctm,
            self.width as f32,
            self.height as f32,
            &mut polyline,
        );

        if polyline.count() == 0 {
            return true;
        }

        let mut tiler = Tiles::<
            { SparseStripConfig::K_TILE_WIDTH },
            { SparseStripConfig::K_TILE_HEIGHT },
        >::new();
        tiler.make_tiles_msaa(&polyline, self.width as u16, self.height as u16);
        tiler.sort_tiles();

        if tiler.get_tiles().is_empty() {
            return true;
        }

        let success = MakeStrips::msaa_simd::<
            { SparseStripConfig::K_TILE_WIDTH },
            { SparseStripConfig::K_TILE_HEIGHT },
        >(
            &tiler,
            &mut self.wides,
            &mut self.ends,
            atlas_manager,
            recorder,
            path.fill_type(),
            &polyline,
            self.mask_lut,
            self.width as u16,
            self.height as u16,
            None,
        );

        if !success {
            return false;
        }

        atlas_manager.populate_proxies(&mut self.ends);
        true
    }

    /// `ends()`.
    #[must_use]
    pub fn ends(&self) -> &EndCaps {
        &self.ends
    }

    /// `wides()`.
    #[must_use]
    pub fn wides(&self) -> &WideTiles {
        &self.wides
    }

    /// `hasNullCaps()`.
    #[doc(alias = "hasNullCaps")]
    #[must_use]
    pub fn has_null_caps(&self) -> bool {
        self.ends.has_null_caps()
    }

    /// `cleanupNullCaps()`: moves the alphas of the caps that overflowed the atlas into the pages
    /// freed by the last flush.
    // Port of: src/gpu/graphite/sparse_strips/StripGenerator.cpp#L83-L87 (chrome/m156)
    #[doc(alias = "cleanupNullCaps")]
    pub fn cleanup_null_caps(&mut self, recorder: &Recorder) -> bool {
        let rp = recorder.priv_();
        let mut atlas_provider = rp.atlas_provider().borrow_mut();
        atlas_provider
            .alpha_atlas_manager()
            .resolve_null_caps(recorder, &mut self.ends)
    }
}
