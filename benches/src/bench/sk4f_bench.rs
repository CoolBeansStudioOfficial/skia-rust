// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/Sk4fBench.cpp

//! `skvx::float4` round-trip casts, floor and the gradient loop (non-rendering benches).

use std::marker::PhantomData;

use skia_rust_simd::vx::{CastFrom, Float4, Lane, floor};

use crate::def_bench;
use crate::prelude::*;

// Writing into this array prevents the loops from being compiled away.
// Port of: bench/Sk4fBench.cpp#L16-L17 (chrome/m156)
// The C++ `static volatile float blackhole[4]` is a process-global sink; each bench keeps its own
// copy here and passes it through `std::hint::black_box` in the same place.
struct Blackhole([f32; 4]);

impl Blackhole {
    fn new() -> Self {
        Self([0.0; 4])
    }
}

/// `Sk4fRoundtripBench<T>`: `fs = cast<float>(cast<T>(fs))` `loops` times.
// Port of: bench/Sk4fBench.cpp#L19-L37 (chrome/m156)
struct Sk4fRoundtripBench<T> {
    name: &'static str,
    blackhole: Blackhole,
    _t: PhantomData<T>,
}

impl<T> Sk4fRoundtripBench<T> {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            blackhole: Blackhole::new(),
            _t: PhantomData,
        }
    }
}

impl<T> Benchmark for Sk4fRoundtripBench<T>
where
    T: Lane + CastFrom<f32>,
    f32: CastFrom<T>,
{
    // onGetName(): switch (sizeof(T)) over the three instantiations.
    // Port of: bench/Sk4fBench.cpp#L23-L33 (chrome/m156)
    fn name(&self) -> String {
        self.name.to_owned()
    }

    // Port of: bench/Sk4fBench.cpp#L35-L37 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/Sk4fBench.cpp#L39-L47 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut fs = Float4::new(1.0, 2.0, 3.0, 4.0);
        for _ in 0..loops {
            fs = fs.cast::<T>().cast::<f32>();
        }
        fs.store(&mut self.blackhole.0);
        std::hint::black_box(&self.blackhole.0);
    }
}

/// `Sk4fFloorBench`.
// Port of: bench/Sk4fBench.cpp#L50-L66 (chrome/m156)
struct Sk4fFloorBench {
    blackhole: Blackhole,
}

impl Benchmark for Sk4fFloorBench {
    // Port of: bench/Sk4fBench.cpp#L53-L54 (chrome/m156)
    fn name(&self) -> String {
        "Sk4f_floor".to_owned()
    }

    // Port of: bench/Sk4fBench.cpp#L55 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/Sk4fBench.cpp#L57-L65 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut fs = Float4::new(1.0, 2.0, 3.0, 4.0);
        for _ in 0..loops {
            fs = floor(fs);
        }
        fs.store(&mut self.blackhole.0);
        std::hint::black_box(&self.blackhole.0);
    }
}

/// `Sk4fGradientBench`: writes 100 device pixels, four at a time.
// Port of: bench/Sk4fBench.cpp#L68-L110 (chrome/m156)
struct Sk4fGradientBench {
    // std::array<SkPMColor, 100> fDevice: each SkPMColor is four bytes, stored as the four u8
    // lanes in memory order.
    device: [[u8; 4]; 100],
}

impl Benchmark for Sk4fGradientBench {
    // Port of: bench/Sk4fBench.cpp#L70-L71 (chrome/m156)
    fn name(&self) -> String {
        "Sk4f_gradient".to_owned()
    }

    // Port of: bench/Sk4fBench.cpp#L72 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/Sk4fBench.cpp#L74-L108 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let c0 = Float4::new(0.0, 0.0, 255.0, 255.0);
        let c1 = Float4::new(255.0, 0.0, 0.0, 255.0);
        let dc = c1 - c0;
        let fx = Float4::splat(0.1);
        let dx = Float4::splat(0.002);
        let dcdx = dc * dx;
        let dcdx4 = dcdx + dcdx + dcdx + dcdx;

        for _ in 0..loops {
            // add an extra 0.5f to get rounding for free.
            let mut a = c0 + dc * fx + 0.5;
            let mut b = a + dcdx;
            let mut c = b + dcdx;
            let mut d = c + dcdx;
            for i in (0..self.device.len()).step_by(4) {
                a.cast::<u8>().store_bytes(&mut self.device[i]);
                b.cast::<u8>().store_bytes(&mut self.device[i + 1]);
                c.cast::<u8>().store_bytes(&mut self.device[i + 2]);
                d.cast::<u8>().store_bytes(&mut self.device[i + 3]);
                a += dcdx4;
                b += dcdx4;
                c += dcdx4;
                d += dcdx4;
            }
        }
        std::hint::black_box(&self.device);
    }
}

// Port of: bench/Sk4fBench.cpp#L42-L42 (chrome/m156)
def_bench!(
    sk4f_roundtrip_bench_uint16_t = "Sk4fRoundtripBench<uint16_t>",
    Sk4fRoundtripBench::<u16>::new("Sk4f_roundtrip_u16")
);
// Port of: bench/Sk4fBench.cpp#L41-L41 (chrome/m156)
def_bench!(
    sk4f_roundtrip_bench_uint8_t = "Sk4fRoundtripBench<uint8_t>",
    Sk4fRoundtripBench::<u8>::new("Sk4f_roundtrip_u8")
);
// Port of: bench/Sk4fBench.cpp#L43-L43 (chrome/m156)
def_bench!(
    sk4f_roundtrip_bench_int = "Sk4fRoundtripBench<int>",
    Sk4fRoundtripBench::<i32>::new("Sk4f_roundtrip_int")
);
// Port of: bench/Sk4fBench.cpp#L59-L59 (chrome/m156)
def_bench!(
    sk4f_floor_bench = "Sk4fFloorBench",
    Sk4fFloorBench {
        blackhole: Blackhole::new()
    }
);
// Port of: bench/Sk4fBench.cpp#L93-L93 (chrome/m156)
def_bench!(
    sk4f_gradient_bench = "Sk4fGradientBench",
    Sk4fGradientBench {
        device: [[0; 4]; 100]
    }
);
