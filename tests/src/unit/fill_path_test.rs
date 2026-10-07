// Copyright 2010 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FillPathTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color::Alpha;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::Region;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_raster::blitter::{BlitMemory, Blitter};
use skia_rust_raster::scan;

use crate::{def_test, reporter_assert};

// Port of: tests/FillPathTest.cpp#L19-L32 (chrome/m156)
#[derive(Default)]
struct FakeBlitter {
    blit_count: i32,
    mem: BlitMemory,
}

impl Blitter for FakeBlitter {
    fn blit_h(&mut self, _x: i32, _y: i32, _width: i32) {
        self.blit_count += 1;
    }

    fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [Alpha], _runs: &mut [i16]) {
        panic!("blitAntiH not implemented");
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.mem
    }
}

// skbug.com/40031085
// Lines which is not clipped by boundary based clipping,
// but skipped after tessellation, should be cleared by the blitter.
// Port of: tests/FillPathTest.cpp#L34-L55 (chrome/m156)
def_test!(FillPathInverse, |reporter| {
    let mut blitter = FakeBlitter::default();
    let mut builder = PathBuilder::new_with_fill_type(PathFillType::InverseWinding);
    let height: i32 = 100;
    let width: i32 = 200;
    let expected_lines: i32 = 5;
    let clip = IRect::new(0, height - expected_lines, width, height);
    builder
        .move_to((0.0, 0.0))
        .quad_to(
            (int_to_scalar(width / 2), int_to_scalar(height)),
            (int_to_scalar(width), 0.0),
        )
        .close();
    let rgn = Region::from_rect(clip);
    let raw = path_priv::raw_builder(&builder, ResolveConvexity::Yes).unwrap();
    scan::fill_path(&raw, &rgn, &mut blitter);

    reporter_assert!(reporter, blitter.blit_count == expected_lines);
});
