// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/SkGlyphCacheBench.cpp

//! `SkGlyphCacheBasic`: mask metrics and images of 90 printable glyphs at sizes 8 to 63, with
//! the font cache limited to the bench's size (non-rendering). The stress test
//! (`SkGlyphCacheStressTest`) runs its work on 16 `SkTaskGroup` threads, which are not ported
//! (see the manifest reason).

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::graphics::{font_cache_limit, set_font_cache_limit};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike_spec::{BulkGlyphMetricsAndImages, StrikeSpec};
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::utf::Unichar;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_font};

use crate::def_bench;
use crate::prelude::*;

/// `do_font_stuff(SkFont* font)`: mask metrics and images of `' '..'z'` at sizes 8 to 63.
// Port of: bench/SkGlyphCacheBench.cpp#L28-L47 (chrome/m156)
fn do_font_stuff(font: &mut Font) {
    // SkPaint defaultPaint;
    let default_paint = Paint::default();
    // for (SkScalar i = 8; i < 64; i++)
    let mut i: scalar = 8.0;
    while i < 64.0 {
        font.set_size(i);
        // auto strikeSpec = SkStrikeSpec::MakeMask(*font, defaultPaint,
        //         SkSurfaceProps(0, kUnknown_SkPixelGeometry), SkScalerContextFlags::kNone,
        //         SkMatrix::I());
        let strike_spec = StrikeSpec::make_mask(
            font,
            &default_paint,
            &SurfaceProps::new(SurfacePropsFlags::empty(), PixelGeometry::Unknown),
            ScalerContextBuildFlags::NONE,
            Matrix::i(),
        );
        // std::array<SkPackedGlyphID, 'z'> glyphs;
        let mut glyphs = [PackedGlyphId::default(); 'z' as usize];
        // for (int c = ' '; c < 'z'; c++) glyphs[c] = SkPackedGlyphID{font->unicharToGlyph(c)};
        for c in usize::from(b' ')..usize::from(b'z') {
            let uni = Unichar::try_from(c).expect("ASCII");
            glyphs[c] = PackedGlyphId::from_glyph_id(font.unichar_to_glyph(uni));
        }
        // constexpr size_t glyphCount = 'z' - ' ';
        // SkSpan<const SkPackedGlyphID> glyphIDs{&glyphs[SkTo<int>(' ')], glyphCount};
        let glyph_ids = &glyphs[usize::from(b' ')..usize::from(b'z')];
        // SkBulkGlyphMetricsAndImages images{strikeSpec};
        let images = BulkGlyphMetricsAndImages::new(&strike_spec);
        for _ in 0..10 {
            // (void)images.glyphs(glyphIDs);
            let _ = images.glyphs(glyph_ids);
        }
        i += 1.0;
    }
}

/// `class SkGlyphCacheBasic`.
// Port of: bench/SkGlyphCacheBench.cpp#L49-L81 (chrome/m156)
struct SkGlyphCacheBasic {
    cache_size: usize,
}

impl SkGlyphCacheBasic {
    fn new(cache_size: usize) -> Self {
        Self { cache_size }
    }
}

impl Benchmark for SkGlyphCacheBasic {
    // Port of: bench/SkGlyphCacheBench.cpp#L53-L57 (chrome/m156)
    fn name(&self) -> String {
        // fName.printf("SkGlyphCacheBasic%dK", (int)(fCacheSize >> 10));
        format!("SkGlyphCacheBasic{}K", self.cache_size >> 10)
    }

    // Port of: bench/SkGlyphCacheBench.cpp#L59-L61 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/SkGlyphCacheBench.cpp#L63-L80 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let old_cache_limit_size = font_cache_limit();
        let _ = set_font_cache_limit(self.cache_size);
        // SkFont font = ToolUtils::DefaultFont();
        let mut font = default_font();
        font.set_edging(Edging::AntiAlias);
        font.set_subpixel(true);
        font.set_typeface(Some(create_portable_typeface(
            Some("serif"),
            FontStyle::italic(),
        )));

        for _ in 0..loops {
            do_font_stuff(&mut font);
        }
        let _ = set_font_cache_limit(old_cache_limit_size);
    }
}

// Port of: bench/SkGlyphCacheBench.cpp#L127 (chrome/m156)
def_bench!(
    sk_glyph_cache_basic_256k = "SkGlyphCacheBasic(256 * 1024)",
    SkGlyphCacheBasic::new(256 * 1024)
);
// Port of: bench/SkGlyphCacheBench.cpp#L128 (chrome/m156)
def_bench!(
    sk_glyph_cache_basic_32m = "SkGlyphCacheBasic(32 * 1024 * 1024)",
    SkGlyphCacheBasic::new(32 * 1024 * 1024)
);
