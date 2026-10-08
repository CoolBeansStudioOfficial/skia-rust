// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkStrikeCacheTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

use crate::{def_test, reporter_assert};

// Port of: tests/SkStrikeCacheTest.cpp#L23-L63 (chrome/m156)
def_test!(SkStrikeCache_CachePurge, |reporter| {
    let cache = StrikeCache::new();

    let typeface = create_portable_typeface(Some("serif"), FontStyle::italic());

    let mut font = Font::default();
    font.set_edging(Edging::AntiAlias);
    font.set_subpixel(true);
    font.set_typeface(Some(typeface));

    let default_paint = Paint::default();
    let strike_spec = StrikeSpec::make_mask(
        &font,
        &default_paint,
        &SurfaceProps::new(SurfacePropsFlags::empty(), PixelGeometry::Unknown),
        ScalerContextBuildFlags::NONE,
        Matrix::i(),
    );

    // Initially empty cache
    reporter_assert!(reporter, cache.total_memory_used() == 0);

    {
        let _strike = cache.find_or_create_strike(&strike_spec);
    }

    // Stuff in cache.
    reporter_assert!(reporter, cache.total_memory_used() > 0);

    cache.purge_all();

    // Purged cache.
    reporter_assert!(reporter, cache.total_memory_used() == 0);

    // Smallest cache.
    // The previous limit is not used (C++ ignores the result too).
    let _ = cache.set_cache_size_limit(0);
    {
        let _strike = cache.find_or_create_strike(&strike_spec);
        reporter_assert!(reporter, cache.total_memory_used() == 0);
    }
    reporter_assert!(reporter, cache.total_memory_used() == 0);
});
