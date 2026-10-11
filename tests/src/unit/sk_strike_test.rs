// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkStrikeTest.cpp (chrome/m156), the cases that need the portable typeface

#![cfg(test)]

use std::sync::atomic::{AtomicI32, Ordering};

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::drawable::{Drawable, DrawableBase};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::glyph::Glyph;
use skia_rust_core::glyph::skglyph::{empty_rect, rect_union};
use skia_rust_core::glyph::{ActionType, GlyphAction, GlyphRect};
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike::Strike;
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, create_test_typeface};

use skia_rust_core::utf::Unichar;

use crate::{def_test, reporter_assert};

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
    );

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

/// `SkGlyphTestPeer::SetGlyph`: tweaks the bounds to make them unique based on glyph id.
// Port of: tests/SkStrikeTest.cpp#L172-L183 (chrome/m156)
fn set_glyph(glyph: &mut Glyph) {
    let uniquify = i16::try_from(glyph.glyph_id()).expect("a small glyph id");
    glyph.set_metrics_for_testing(
        Point::new(10.0, 11.0),
        -1 - uniquify,
        -2,
        8,
        9,
        MaskFormat::A8,
    );
}

/// `TestDrawable`: draws a rectangle.
// Port of: tests/SkStrikeTest.cpp#L239-L253 (chrome/m156)
#[derive(Debug)]
struct TestDrawable {
    rect: Rect,
}

impl DrawableBase for TestDrawable {
    fn on_get_bounds(&self) -> Rect {
        self.rect
    }

    fn on_approximate_bytes_used(&self) -> usize {
        0
    }

    fn on_draw(&self, canvas: &Canvas) {
        let paint = Paint::default();
        canvas.draw_rect(self.rect, &paint);
    }
}

// Port of: tests/SkStrikeTest.cpp#L200-L295 (chrome/m156)
def_test!(SkStrike_FlattenByType, |reporter| {
    const X: u8 = 0xff;
    const O: u8 = 0x00;

    let mut images_to_send: Vec<Glyph> = Vec::new();
    let mut paths_to_send: Vec<Glyph> = Vec::new();
    let mut drawables_to_send: Vec<Glyph> = Vec::new();

    // Make a mask glyph and put it in the glyphs to send.
    let mask_packed_glyph_id = PackedGlyphId::from_glyph_id(10);
    let mut mask_glyph = Glyph::new(mask_packed_glyph_id);
    set_glyph(&mut mask_glyph);

    let image_data: [[u8; 8]; 9] = [
        [X, X, X, X, X, X, X, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, X, X, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, X, X, X, X, X, X, X],
    ];
    mask_glyph.set_image(image_data.concat().into_boxed_slice());
    images_to_send.push(mask_glyph.clone());

    // Make a path glyph and put it in the glyphs to send.
    let path_packed_glyph_id = PackedGlyphId::from_glyph_id(11);
    let mut path_glyph = Glyph::new(path_packed_glyph_id);
    set_glyph(&mut path_glyph);
    let path = Path::rect(path_glyph.rect(), None);
    path_glyph.set_path(Some(path), false, false);
    paths_to_send.push(path_glyph.clone());

    // Make a drawable glyph and put it in the glyphs to send.
    let drawable_packed_glyph_id = PackedGlyphId::from_glyph_id(12);
    let mut drawable_glyph = Glyph::new(drawable_packed_glyph_id);
    set_glyph(&mut drawable_glyph);

    let drawable = Drawable::new(Arc::new(TestDrawable {
        rect: drawable_glyph.rect(),
    }));
    reporter_assert!(reporter, !drawable_glyph.set_drawable_has_been_called());
    drawable_glyph.set_drawable(Some(drawable));
    reporter_assert!(reporter, drawable_glyph.set_drawable_has_been_called());
    reporter_assert!(reporter, drawable_glyph.drawable().is_some());
    drawables_to_send.push(drawable_glyph.clone());

    // Test the FlattenGlyphsByType method.
    let mut write_buffer = BinaryWriteBuffer::new();
    Strike::flatten_glyphs_by_type(
        &mut write_buffer,
        &images_to_send,
        &paths_to_send,
        &drawables_to_send,
    );
    let data = write_buffer.snapshot_as_data();

    // Make a strike to merge into.
    let strike_cache = StrikeCache::new();
    let dst_typeface = create_test_typeface(Some("monospace"), FontStyle::default());
    let font = Font::from_size(dst_typeface, 12.0);
    let spec = StrikeSpec::make_with_no_device(
        &font,
        None,
        ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
    );
    let strike = strike_cache.find_or_create_strike(&spec);

    // Test the mergeFromBuffer method.
    let mut read_buffer = ReadBuffer::new(data.as_bytes());
    strike.merge_from_buffer(&mut read_buffer);

    // Check mask glyph.
    let dst_mask_glyph = strike.glyph_for_testing(mask_packed_glyph_id);
    reporter_assert!(reporter, mask_glyph.rect() == dst_mask_glyph.rect());
    reporter_assert!(reporter, dst_mask_glyph.set_image_has_been_called());
    reporter_assert!(reporter, dst_mask_glyph.image().is_some());

    // Check path glyph.
    let dst_path_glyph = strike.glyph_for_testing(path_packed_glyph_id);
    reporter_assert!(reporter, path_glyph.rect() == dst_path_glyph.rect());
    reporter_assert!(reporter, dst_path_glyph.set_path_has_been_called());
    reporter_assert!(reporter, dst_path_glyph.path().is_some());

    // Check drawable glyph.
    let dst_drawable_glyph = strike.glyph_for_testing(drawable_packed_glyph_id);
    reporter_assert!(reporter, drawable_glyph.rect() == dst_drawable_glyph.rect());
    reporter_assert!(reporter, dst_drawable_glyph.set_drawable_has_been_called());
    reporter_assert!(reporter, dst_drawable_glyph.drawable().is_some());
});
