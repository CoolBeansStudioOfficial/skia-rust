// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (no arena); `p.run(x, y, w, h)`
// takes the run's writable memory as a `MemoryBindings` (empty here).

use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};

use crate::{def_test, errorf, reporter_assert};

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

// Port of: tests/SkRasterPipelineTest.cpp#L2931-L2943 (chrome/m156)
fn h(f: f32) -> u16 {
    // Remember, a float is 1-8-23 (sign-exponent-mantissa) with 127 exponent bias.
    let sem: u32 = f.to_bits();
    let s: u32 = sem & 0x8000_0000;
    let em: u32 = sem ^ s;

    // Convert to 1-5-10 half with 15 bias, flushing denorm halfs (including zero) to zero.
    #[allow(clippy::cast_possible_wrap)] // mirrors (int32_t)em
    let denorm = (em as i32) < 0x3880_0000; // I32 comparison is often quicker, and always safe here.
    if denorm {
        0
    } else {
        // SkTo<uint16_t>: checked.
        u16::try_from(
            (s >> 16)
                .wrapping_add(em >> 13)
                .wrapping_sub((127 - 15) << 10),
        )
        .unwrap()
    }
}

/// Runs `load(src)` then `store(dst)` over `w` pixels of one row (the `p.run(0,0, i,1)` of
/// `SkRasterPipeline_tail`); `src` and `dst` are the bytes of the two `MemoryCtx { ptr, 0 }`s.
fn run_tail(
    load: fn(MemoryCtx) -> Stage<'static>,
    store: fn(MemoryCtx) -> Stage<'static>,
    src: &[u8],
    dst: &mut [u8],
    w: usize,
) {
    let mut p = RasterPipeline::new();
    p.append(load(MemoryCtx::new(MemSlot(0))));
    p.append(store(MemoryCtx::new(MemSlot(1))));
    p.run(
        0,
        0,
        w,
        1,
        &mut MemoryBindings::new()
            .with(MemSlot(0), MemView::read(src))
            .with(MemSlot(1), MemView::write(dst)),
    );
}

fn halves_to_bytes(h: &[u16]) -> Vec<u8> {
    h.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn bytes_to_halves(b: &[u8]) -> Vec<u16> {
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes(*c))
        .collect()
}

/// `{h(a0), h(a1), h(a2), h(a3)}` for each of the rows 0, 10, 20, 30.
fn half_rows() -> [[u16; 4]; 4] {
    [
        [h(0.), h(1.), h(2.), h(3.)],
        [h(10.), h(11.), h(12.), h(13.)],
        [h(20.), h(21.), h(22.), h(23.)],
        [h(30.), h(31.), h(32.), h(33.)],
    ]
}

// Port of: tests/SkRasterPipelineTest.cpp#L2945-L3121 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    SkRasterPipeline_tail,
    |r| {
        {
            let data: [[f32; 4]; 4] = [
                [0., 1., 2., 3.],
                [10., 11., 12., 13.],
                [20., 21., 22., 23.],
                [30., 31., 32., 33.],
            ];
            let src: Vec<u8> = data
                .iter()
                .flatten()
                .flat_map(|f| f.to_ne_bytes())
                .collect();

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 4 * 4]; // memset(buffer, 0xff, sizeof(buffer));
                run_tail(Stage::LoadF32, Stage::StoreF32, &src, &mut buffer, i);
                let buffer: Vec<[f32; 4]> = buffer
                    .as_chunks::<16>()
                    .0
                    .iter()
                    .map(|px| {
                        core::array::from_fn(|k| {
                            f32::from_ne_bytes(px[4 * k..4 * k + 4].try_into().unwrap())
                        })
                    })
                    .collect();
                for j in 0..i {
                    for k in 0..4 {
                        if buffer[j][k] != data[j][k] {
                            errorf!(
                                r,
                                "({}, {}) - a: {} r: {}\n",
                                j,
                                k,
                                data[j][k],
                                buffer[j][k]
                            );
                        }
                    }
                }
                for row in &buffer[i..4] {
                    for f in row {
                        reporter_assert!(r, f.is_nan());
                    }
                }
            }
        }

        {
            let data = half_rows();
            let flat: Vec<u16> = data.iter().flatten().copied().collect();
            let src = halves_to_bytes(&flat);

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 4 * 2];
                run_tail(Stage::LoadF16, Stage::StoreF16, &src, &mut buffer, i);
                let buffer = bytes_to_halves(&buffer);
                for j in 0..i {
                    for k in 0..4 {
                        reporter_assert!(r, buffer[4 * j + k] == data[j][k]);
                    }
                }
                for f in &buffer[4 * i..] {
                    reporter_assert!(r, *f == 0xffff);
                }
            }
        }

        {
            let data: [u16; 4] = [h(0.), h(10.), h(20.), h(30.)];
            let src = halves_to_bytes(&data);

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 4 * 2];
                run_tail(Stage::LoadAf16, Stage::StoreF16, &src, &mut buffer, i);
                let buffer = bytes_to_halves(&buffer);
                for j in 0..i {
                    let expected: [u16; 4] = [0, 0, 0, data[j]];
                    reporter_assert!(r, expected == buffer[4 * j..4 * j + 4]);
                }
                for f in &buffer[4 * i..] {
                    reporter_assert!(r, *f == 0xffff);
                }
            }
        }

        {
            let data = half_rows();
            let flat: Vec<u16> = data.iter().flatten().copied().collect();
            let src = halves_to_bytes(&flat);

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 2];
                run_tail(Stage::LoadF16, Stage::StoreAf16, &src, &mut buffer, i);
                let buffer = bytes_to_halves(&buffer);
                for j in 0..i {
                    reporter_assert!(r, data[j][3] == buffer[j]);
                }
                for f in &buffer[i..] {
                    reporter_assert!(r, *f == 0xffff);
                }
            }
        }

        {
            let data = half_rows();
            let flat: Vec<u16> = data.iter().flatten().copied().collect();
            let src = halves_to_bytes(&flat);

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 2 * 2];
                run_tail(Stage::LoadF16, Stage::StoreRgf16, &src, &mut buffer, i);
                let buffer = bytes_to_halves(&buffer);
                for j in 0..i {
                    reporter_assert!(r, buffer[2 * j..2 * j + 2] == data[j][..2]);
                }
                for f in &buffer[2 * i..] {
                    reporter_assert!(r, *f == 0xffff);
                }
            }
        }

        {
            let data: [[u16; 2]; 4] = [
                [h(0.), h(1.)],
                [h(10.), h(11.)],
                [h(20.), h(21.)],
                [h(30.), h(31.)],
            ];
            let flat: Vec<u16> = data.iter().flatten().copied().collect();
            let src = halves_to_bytes(&flat);

            for i in 1..=4usize {
                let mut buffer = vec![0xffu8; 4 * 4 * 2];
                run_tail(Stage::LoadRgf16, Stage::StoreF16, &src, &mut buffer, i);
                let buffer = bytes_to_halves(&buffer);
                for j in 0..i {
                    let expected: [u16; 4] = [data[j][0], data[j][1], h(0.), h(1.)];
                    reporter_assert!(r, buffer[4 * j..4 * j + 4] == expected);
                }
                for f in &buffer[4 * i..] {
                    reporter_assert!(r, *f == 0xffff);
                }
            }
        }
    }
);
