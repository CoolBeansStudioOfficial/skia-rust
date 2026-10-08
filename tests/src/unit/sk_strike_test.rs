// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkStrikeTest.cpp (chrome/m156), the cases that need the portable typeface
//
// Not ported yet:
// - `SkStrike_FlattenByType`: remote strike flattening (T23).

#![cfg(test)]

use std::sync::atomic::{AtomicI32, Ordering};

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::glyph::skglyph::{empty_rect, rect_union};
use skia_rust_core::glyph::{ActionType, GlyphAction, GlyphRect};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike::Strike;
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

use skia_rust_core::utf::Unichar;

use crate::def_test;

/// The C++ `Barrier`, a spinning count of the threads that have arrived.
// Port of: tests/SkStrikeTest.cpp#L20-L31 (chrome/m156)
struct Barrier {
    thread_count: AtomicI32,
}

impl Barrier {
    // Port of: tests/SkStrikeTest.cpp#L22 (chrome/m156)
    fn new(thread_count: i32) -> Self {
        Self {
            thread_count: AtomicI32::new(thread_count),
        }
    }

    // Port of: tests/SkStrikeTest.cpp#L23-L26 (chrome/m156)
    fn wait_for_all(&self) {
        self.thread_count.fetch_sub(1, Ordering::SeqCst);
        while self.thread_count.load(Ordering::SeqCst) > 0 {
            std::hint::spin_loop();
        }
    }
}

/// `prepare_for_mask_drawing`: the accepted glyphs (packed id and position), the rejected ones
/// (glyph id and position), and the bounds of the accepted glyphs.
///
/// This should stay in sync with the implementation from `SubRunContainer`.
// Port of: tests/SkStrikeTest.cpp#L33-L64 (chrome/m156)
#[allow(clippy::type_complexity)] // the three results of the C++ tuple
fn prepare_for_mask_drawing(
    strike: &Strike,
    source: &[(GlyphId, Point)],
) -> (
    Vec<(PackedGlyphId, Point)>,
    Vec<(GlyphId, Point)>,
    GlyphRect,
) {
    let mut bounding_rect = empty_rect();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    // `StrikeMutationMonitor m{strike}`: the strike is locked for the whole loop.
    let mut guard = strike.lock();
    for &(glyph_id, pos) in source {
        if !(pos.x.is_finite() && pos.y.is_finite()) {
            continue;
        }
        let packed_id = PackedGlyphId::from_glyph_id(glyph_id);
        let digest = guard.digest_for(ActionType::DirectMask, packed_id);
        match digest.action_for(ActionType::DirectMask) {
            GlyphAction::Accept => {
                let glyph_bounds = digest.bounds().offset_point(pos);
                bounding_rect = rect_union(bounding_rect, glyph_bounds);
                accepted.push((packed_id, glyph_bounds.left_top()));
            }
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected, bounding_rect)
}

/// The number of threads of `SkStrikeMultiThread`.
const THREAD_COUNT: usize = 4;
/// `'z'`: the C++ arrays are indexed by character, up to 'z'.
const Z: usize = b'z' as usize;
/// The glyphs of the test: the printable characters from ' ' to 'z'.
const GLYPH_COUNT: usize = Z - b' ' as usize;

// Port of: tests/SkStrikeTest.cpp#L109-L175 (chrome/m156)
def_test!(SkStrikeMultiThread, |_reporter| {
    let typeface = create_portable_typeface(Some("serif"), FontStyle::italic());
    let barrier = Barrier::new(i32::try_from(THREAD_COUNT).expect("a small thread count"));

    let mut font = Font::default();
    font.set_edging(Edging::AntiAlias);
    font.set_subpixel(true);
    font.set_typeface(Some(typeface));

    let mut glyphs = [0 as GlyphId; Z];
    let mut pos = [Point::new(0.0, 0.0); Z];
    for c in (b' ' as usize)..Z {
        let unichar = Unichar::try_from(c).expect("a character fits in a unichar");
        glyphs[c] = font.unichar_to_glyph(unichar);
        // `30.0f * c + 30`: the characters are below 2^24, exact in a float.
        #[allow(clippy::cast_precision_loss)]
        {
            pos[c] = Point::new(30.0 * c as f32 + 30.0, 30.0);
        }
    }
    // `SkMakeZip(glyphs, pos).subspan(' ', glyphCount)`.
    let data: Vec<(GlyphId, Point)> = glyphs
        .iter()
        .copied()
        .zip(pos.iter().copied())
        .skip(b' ' as usize)
        .take(GLYPH_COUNT)
        .collect();

    let strike_spec = StrikeSpec::make_mask(
        &font,
        &Paint::default(),
        &SurfaceProps::new(SurfacePropsFlags::empty(), PixelGeometry::Unknown),
        ScalerContextBuildFlags::NONE,
        Matrix::i(),
    )
    .expect("a plain mask strike has no effects to reject");

    let strike_cache = StrikeCache::new();

    // The C++ test uses its own executor, with one task per thread on a FIFO pool. Scoped
    // threads run the same four tasks, each to completion, before the group ends.
    for _tries in 0..100 {
        let strike = strike_cache.new_detached_strike(&strike_spec);

        std::thread::scope(|scope| {
            for thread_index in 0..THREAD_COUNT {
                let strike = &strike;
                let barrier = &barrier;
                let data = &data;
                scope.spawn(move || {
                    barrier.wait_for_all();

                    let local =
                        &data[thread_index * 2..thread_index * 2 + data.len() - THREAD_COUNT * 2];
                    for _i in 0..100 {
                        // The C++ `source = rejected` assignment is dead: `source` is declared
                        // again on each iteration, from `local`.
                        let _ = prepare_for_mask_drawing(strike, local);
                    }
                });
            }
        });
    }
});
