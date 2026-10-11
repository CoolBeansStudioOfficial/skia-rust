// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzRegionOp.cpp (chrome/m156)

use skia_rust_core::region::Region;

use crate::common::fuzz_nice_region;
use crate::def_fuzz;

// Port of: fuzz/FuzzRegionOp.cpp#L11-L19 (chrome/m156)
def_fuzz!(RegionOp, fuzz_RegionOp, |fuzz| {
    // `fuzz -t api -n RegionOp`
    let mut region = Region::new();
    // FuzzNiceRegion generates a random region by joining a random amount of regions
    // together. This fuzzer simply targets that directly. 300 was picked arbitrarily as
    // a number over 2^8.
    fuzz_nice_region(fuzz, &mut region, 300);
    // Do a computation to make sure region is not optimized out.
    std::hint::black_box(region.compute_region_complexity());
});
