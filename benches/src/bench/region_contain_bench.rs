// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RegionContainBench.cpp

//! `SkRegion` intersection of a striped region with a rectangle (`bench/RegionContainBench.cpp`).

use skia_rust_core::random::Random;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Op, Region};

use crate::def_bench;
use crate::prelude::*;

/// `typedef bool (*Proc)(SkRegion& a, SkRegion& b)`.
type Proc = fn(&mut Region, &mut Region) -> bool;

// Port of: bench/RegionContainBench.cpp#L10-L14 (chrome/m156)
fn sect_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut result = Region::new();
    result.op_region_region(a, b, Op::Intersect)
}

const W: i32 = 200;
const H: i32 = 200;
const COUNT: i32 = 10;

/// `class RegionContainBench`.
// Port of: bench/RegionContainBench.cpp#L16-L60 (chrome/m156)
struct RegionContainBench {
    a: Region,
    b: Region,
    proc_: Proc,
    name: String,
}

impl RegionContainBench {
    // Port of: bench/RegionContainBench.cpp#L27-L31 (chrome/m156)
    fn randrect(rand: &mut Random, i: i32) -> IRect {
        let w = (rand.next_u() % W as u32).cast_signed();
        IRect::from_xywh(0, i * H / COUNT, w, H / COUNT)
    }

    // Port of: bench/RegionContainBench.cpp#L33-L44 (chrome/m156)
    fn new(proc_: Proc, name: &str) -> Self {
        let mut rand = Random::default();
        let mut a = Region::new();
        for i in 0..COUNT {
            a.op_rect(Self::randrect(&mut rand, i), Op::XOR);
        }
        let mut b = Region::new();
        b.set_rect(IRect::new(0, 0, H, W));
        Self {
            a,
            b,
            proc_,
            name: format!("region_contains_{name}"),
        }
    }
}

impl Benchmark for RegionContainBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let proc_ = self.proc_;
        for _ in 0..loops {
            proc_(&mut self.a, &mut self.b);
        }
    }
}

// Port of: bench/RegionContainBench.cpp#L67-L67 (chrome/m156)
def_bench!(
    region_contain_bench_sect = "RegionContainBench(sect_proc, \"sect\")",
    RegionContainBench::new(sect_proc, "sect")
);
