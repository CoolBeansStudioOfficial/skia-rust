// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (no arena); `p.run(x, y, w, h)`
// takes the run's writable memory as a `MemoryBindings` (empty here).

use skia_rust_core::raster_pipeline::{MemoryBindings, RasterPipeline, Stage};

use crate::def_test;

// Port of: tests/SkRasterPipelineTest.cpp#L2887-L2891 (chrome/m156)
def_test!(SkRasterPipeline_empty, |_r| {
    // No asserts... just a test that this is safe to run.
    let p = RasterPipeline::new();
    p.run(0, 0, 20, 1, &mut MemoryBindings::new());
});

// Port of: tests/SkRasterPipelineTest.cpp#L2893-L2899 (chrome/m156)
def_test!(SkRasterPipeline_nonsense, |_r| {
    // No asserts... just a test that this is safe to run and terminates.
    // srcover() calls st->next(); this makes sure we've always got something there to call.
    let mut p = RasterPipeline::new();
    p.append(Stage::Srcover);
    p.run(0, 0, 20, 1, &mut MemoryBindings::new());
});
