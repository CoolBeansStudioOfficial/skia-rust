// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTightBoundsTest.cpp (chrome/m156). The threaded runners of the Skia file
// run their runnables one after another here, with the same cases and the same per-runnable
// state (a fresh `SkRandom` and bitmap for each runnable).

#![cfg(test)]
// Skia compares these bounds exactly (no epsilon), and the literals are Skia's spelling.
#![allow(clippy::unreadable_literal, clippy::float_cmp)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_pathops::tight_bounds::tight_bounds;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_test, reporter_assert};

/// `testTightBoundsLines(PathOpsThreadState* data)`.
// Port of: tests/PathOpsTightBoundsTest.cpp#L24-L48 (chrome/m156)
fn test_tight_bounds_lines(reporter: &mut Reporter) {
    let mut ran = Random::default();
    for _index in 0..1000 {
        let mut builder = PathBuilder::new();
        let contour_count = ran.next_range_u(1, 10);
        for _c_index in 0..contour_count {
            let line_count = ran.next_range_u(1, 10);
            let x = ran.next_range_f(-1000.0, 1000.0);
            let y = ran.next_range_f(-1000.0, 1000.0);
            builder.move_to((x, y));
            for _l_index in 0..line_count {
                let x = ran.next_range_f(-1000.0, 1000.0);
                let y = ran.next_range_f(-1000.0, 1000.0);
                builder.line_to((x, y));
            }
            if ran.next_bool() {
                builder.close();
            }
        }
        let path = builder.detach();
        let classic_bounds = *path.bounds();
        let tight = tight_bounds(&path);
        reporter_assert!(reporter, tight.is_some());
        reporter_assert!(reporter, Some(classic_bounds) == tight);
    }
}

// Port of: tests/PathOpsTightBoundsTest.cpp#L24-L48 (chrome/m156), the ten runnables
def_test!(PathOpsTightBoundsLines, |reporter| {
    let outer_count = if reporter.allow_extended_test() {
        100
    } else {
        1
    };
    for _index in 0..outer_count {
        for _idx2 in 0..10 {
            test_tight_bounds_lines(reporter);
        }
    }
});

/// `testTightBoundsQuads(PathOpsThreadState* data)`.
// Port of: tests/PathOpsTightBoundsTest.cpp#L50-L108 (chrome/m156)
fn test_tight_bounds_quads(reporter: &mut Reporter) {
    let mut ran = Random::default();
    let bit_width: i32 = 32;
    let bit_height: i32 = 32;
    let path_min: f32 = 1.0;
    #[allow(clippy::cast_precision_loss)] // 30, exact in f32
    let path_max: f32 = (bit_height - 2) as f32;
    let mut bits = Bitmap::new();
    bits.alloc_n32_pixels((bit_width, bit_height), None);
    for _index in 0..100 {
        let mut bu = PathBuilder::new();
        let contour_count = ran.next_range_u(1, 10);
        for _c_index in 0..contour_count {
            let line_count = ran.next_range_u(1, 10);
            let x = ran.next_range_f(1.0, path_max);
            let y = ran.next_range_f(path_min, path_max);
            bu.move_to((x, y));
            for _l_index in 0..line_count {
                if ran.next_bool() {
                    let x = ran.next_range_f(path_min, path_max);
                    let y = ran.next_range_f(path_min, path_max);
                    bu.line_to((x, y));
                } else {
                    let x1 = ran.next_range_f(path_min, path_max);
                    let y1 = ran.next_range_f(path_min, path_max);
                    let x2 = ran.next_range_f(path_min, path_max);
                    let y2 = ran.next_range_f(path_min, path_max);
                    bu.quad_to((x1, y1), (x2, y2));
                }
            }
            if ran.next_bool() {
                bu.close();
            }
        }
        let path: Path = bu.detach();
        let classic_bounds = *path.bounds();
        let tight_bounds_result = tight_bounds(&path);
        reporter_assert!(reporter, tight_bounds_result.is_some());
        let tight_bounds_rect: Rect = tight_bounds_result.unwrap_or_default();
        reporter_assert!(reporter, classic_bounds.contains(tight_bounds_rect));
        {
            let canvas =
                Canvas::from_bitmap(&mut bits, None).expect("canvas over the tight bounds bitmap");
            canvas.draw_color(Color::WHITE, None);
            canvas.draw_path(&path, &Paint::default());
        }
        // SkIRect bitsWritten = {31, 31, 0, 0};
        let mut bits_written = IRect::new(31, 31, 0, 0);
        for y in 0..bit_height {
            let mut line_written = false;
            for x in 0..bit_width {
                if bits.get_addr32(x, y) == u32::MAX {
                    continue;
                }
                line_written = true;
                bits_written.left = bits_written.left.min(x);
                bits_written.right = bits_written.right.max(x);
            }
            if !line_written {
                continue;
            }
            bits_written.top = bits_written.top.min(y);
            bits_written.bottom = bits_written.bottom.max(y);
        }
        if !bits_written.is_empty() {
            let tight_out: IRect = tight_bounds_rect.round_out();
            reporter_assert!(reporter, tight_out.contains(bits_written));
        }
    }
}

// Port of: tests/PathOpsTightBoundsTest.cpp#L110-L127 (chrome/m156), the ten runnables
def_test!(PathOpsTightBoundsQuads, |reporter| {
    let outer_count = if reporter.allow_extended_test() {
        100
    } else {
        1
    };
    for _index in 0..outer_count {
        for _idx2 in 0..10 {
            test_tight_bounds_quads(reporter);
        }
    }
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L129-L141 (chrome/m156)
def_test!(PathOpsTightBoundsMove, |reporter| {
    let path = PathBuilder::new()
        .move_to((10.0, 10.0))
        .close()
        .move_to((20.0, 20.0))
        .line_to((20.0, 20.0))
        .close()
        .move_to((15.0, 15.0))
        .line_to((15.0, 15.0))
        .close()
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    reporter_assert!(reporter, Some(bounds) == tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L143-L151 (chrome/m156)
def_test!(PathOpsTightBoundsMoveOne, |reporter| {
    let path = PathBuilder::new().move_to((20.0, 20.0)).detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    reporter_assert!(reporter, Some(bounds) == tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L153-L162 (chrome/m156)
def_test!(PathOpsTightBoundsMoveTwo, |reporter| {
    let path = PathBuilder::new()
        .move_to((20.0, 20.0))
        .move_to((40.0, 40.0))
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    reporter_assert!(reporter, Some(bounds) == tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L164-L177 (chrome/m156)
def_test!(PathOpsTightBoundsTiny, |reporter| {
    let path = PathBuilder::new()
        .move_to((1.0, 1.0))
        .quad_to((1.000001, 1.0), (1.0, 1.0))
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    let move_bounds = Rect::new(1.0, 1.0, 1.0, 1.0);
    reporter_assert!(reporter, Some(bounds) != tight);
    reporter_assert!(reporter, Some(move_bounds) == tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L179-L189 (chrome/m156)
def_test!(PathOpsTightBoundsWellBehaved, |reporter| {
    let path = PathBuilder::new()
        .move_to((1.0, 1.0))
        .quad_to((2.0, 3.0), (4.0, 5.0))
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    reporter_assert!(reporter, Some(bounds) == tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L191-L201 (chrome/m156)
def_test!(PathOpsTightBoundsIllBehaved, |reporter| {
    let path = PathBuilder::new()
        .move_to((1.0, 1.0))
        .quad_to((4.0, 3.0), (2.0, 2.0))
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path);
    reporter_assert!(reporter, tight.is_some());
    reporter_assert!(reporter, Some(bounds) != tight);
});

// Port of: tests/PathOpsTightBoundsTest.cpp#L203-L221 (chrome/m156)
def_test!(PathOpsTightBoundsIllBehavedScaled, |reporter| {
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .quad_to((1048578.0, 1048577.0), (1048576.0, 1048576.0))
        .detach();
    let bounds = *path.bounds();
    let tight = tight_bounds(&path).unwrap_or_default();
    reporter_assert!(reporter, Some(bounds) != Some(tight));
    reporter_assert!(reporter, tight.right == 1048576.0);
    reporter_assert!(reporter, tight.bottom == 1048576.0);
});
