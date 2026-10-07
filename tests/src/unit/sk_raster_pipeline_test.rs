// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (its arena, where needed, an
// `ArenaAlloc`); `p.run(x, y, w, h)` takes the run's writable memory as a `MemoryBindings`
// (empty here). `SkArenaAllocWithReset(storage, 128, 500)` is an `ArenaAlloc`.

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::raster_pipeline::{MemoryBindings, RasterPipeline, Stage};
use skia_rust_core::raster_pipeline_context_utils::{Packed, pack, unpack};

use crate::{def_test, reporter_assert};

// Port of: tests/SkRasterPipelineTest.cpp#L48-L79 (chrome/m156)
def_test!(SkRasterPipeline_PackSmallContext, |r| {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct PackableObject {
        data: [u8; size_of::<*const ()>()],
    }

    // Create an arena with storage.
    let alloc = ArenaAlloc::new();

    // Construct and pack one PackableObject.
    let object = PackableObject {
        data: [123; size_of::<*const ()>()],
    };

    let packed = pack(&object, &alloc);

    // The alloc should still be empty.
    reporter_assert!(r, alloc.is_empty());

    // `packed` should now contain a bitwise cast of the raw object data.
    // (skia-rust: the inline value is the object itself; its bytes are checked.)
    let Packed::Inline(bits) = packed else {
        panic!("a pointer-sized context is packed inline");
    };
    for byte in bits.data {
        reporter_assert!(r, byte == 123);
    }

    // Now unpack it.
    let unpacked = unpack(packed);

    // The data should be identical to the original.
    reporter_assert!(r, unpacked.data == object.data);
});

// Port of: tests/SkRasterPipelineTest.cpp#L81-L105 (chrome/m156)
def_test!(SkRasterPipeline_PackBigContext, |r| {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct BigObject {
        data: [u8; size_of::<*const ()>() + 1],
    }

    // Create an arena with storage.
    let alloc = ArenaAlloc::new();

    // Construct and pack one BigObject.
    let object = BigObject {
        data: [123; size_of::<*const ()>() + 1],
    };

    let packed = pack(&object, &alloc);

    // The alloc should not be empty any longer.
    reporter_assert!(r, !alloc.is_empty());

    // Now unpack it.
    let unpacked = unpack(packed);

    // The data should be identical to the original.
    reporter_assert!(r, unpacked.data == object.data);
});

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
