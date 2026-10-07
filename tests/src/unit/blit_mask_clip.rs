// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BlitMaskClip.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color::Alpha;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::rect::IRect;
use skia_rust_raster::blitter::{BlitMemory, Blitter};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/BlitMaskClip.cpp#L18-L41 (chrome/m156)
struct TestBlitter<'r> {
    bounds: IRect,
    reporter: &'r mut Reporter,
    memory: BlitMemory,
}

impl<'r> TestBlitter<'r> {
    fn new(bounds: IRect, reporter: &'r mut Reporter) -> Self {
        TestBlitter {
            bounds,
            reporter,
            memory: BlitMemory::default(),
        }
    }
}

impl Blitter for TestBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        reporter_assert!(
            self.reporter,
            x >= self.bounds.left && x < self.bounds.right
        );
        reporter_assert!(
            self.reporter,
            y >= self.bounds.top && y < self.bounds.bottom
        );
        let right = x + width;
        reporter_assert!(
            self.reporter,
            right > self.bounds.left && right <= self.bounds.right
        );
    }

    fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [Alpha], _runs: &mut [i16]) {
        // SkDEBUGFAIL("blitAntiH not implemented")
        #[allow(clippy::assertions_on_constants)] // SkDEBUGFAIL: fails in debug builds only
        {
            assert!(!cfg!(debug_assertions), "blitAntiH not implemented");
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

// Exercise all clips compared with different widths of bitMask. Make sure that no buffer
// overruns happen.
// Port of: tests/BlitMaskClip.cpp#L43-L69 (chrome/m156)
def_test!(BlitAndClip, |reporter| {
    let origin_x = 100;
    let origin_y = 100;
    for width in 1..=32 {
        let height = 2;
        let row_bytes = (width + 7) >> 3;
        let bits = vec![0xAAu8; usize::try_from(row_bytes * height).unwrap()];

        let b = IRect {
            left: origin_x,
            top: origin_y,
            right: origin_x + width,
            bottom: origin_y + height,
        };
        let mask = Mask::new(&bits, b, u32::try_from(row_bytes).unwrap(), MaskFormat::BW);
        let mut tb = TestBlitter::new(mask.bounds, reporter);

        for top in b.top..b.bottom {
            for bottom in (top + 1)..=b.bottom {
                for left in b.left..b.right {
                    for right in (left + 1)..=b.right {
                        let clip_rect = IRect {
                            left,
                            top,
                            right,
                            bottom,
                        };
                        tb.blit_mask(&mask, &clip_rect);
                    }
                }
            }
        }
    }
});
