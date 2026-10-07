// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (its arena, where needed, an
// `ArenaAlloc`); `p.run(x, y, w, h)` takes the run's writable memory as a `MemoryBindings`
// (empty here). `SkArenaAllocWithReset(storage, 128, 500)` is an `ArenaAlloc`.

use std::cell::RefCell;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::raster_pipeline_context_utils::{Packed, pack, unpack};
use skia_rust_simd::rp::contexts::{
    TraceFuncCtx, TraceHook, TraceLineCtx, TraceScopeCtx, TraceVarCtx,
};

use crate::{def_test, errorf, reporter_assert};

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

// Mapping notes for the `SkRasterPipeline_Trace*` tests: `SkSL::TraceHook` is
// `skia_rust_simd::rp::contexts::TraceHook` (a temporary stand-in until an `SkSL` crate exists)
// and takes `&self`, so `fBuffer` is a `RefCell`. The `static constexpr` mask and data arrays are
// read-only memory bound to slots of the run's `MemoryBindings` and addressed by `MemPtr`s; the
// tests' per-class `TestTraceHook`s are one struct that records the calls named in `records`
// (the others push -9999999, as in Skia).

/// `SkOpts::raster_pipeline_highp_stride`.
fn highp_stride() -> usize {
    skia_rust_simd::selection().tier.highp_stride()
}

/// `size` 32-bit words as bytes, with the words of register `i` set to `registers[i]`
/// (`std::fill(k + i*N, k + (i+1)*N, value)`) and the rest zero.
fn filled(registers: &[i32], n: usize, size: usize) -> Vec<u8> {
    let mut words = vec![0i32; size];
    for (i, v) in registers.iter().enumerate() {
        words[i * n..(i + 1) * n].fill(*v);
    }
    words.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

/// `TestTraceHook`.
struct TestTraceHook {
    /// `fBuffer`.
    buffer: RefCell<Vec<i32>>,
    /// The call kinds that record their arguments: `v`ar, `l`ine, `e`nter, e`x`it, `s`cope.
    records: &'static str,
}

impl TestTraceHook {
    fn new(records: &'static str) -> TestTraceHook {
        TestTraceHook {
            buffer: RefCell::new(Vec::new()),
            records,
        }
    }

    fn push(&self, kind: char, values: &[i32]) {
        let mut buffer = self.buffer.borrow_mut();
        if self.records.contains(kind) {
            buffer.extend_from_slice(values);
        } else {
            buffer.push(-9_999_999);
        }
    }
}

impl TraceHook for TestTraceHook {
    fn line(&self, line_num: i32) {
        self.push('l', &[line_num]);
    }
    fn enter(&self, fn_idx: i32) {
        self.push('e', &[fn_idx, 1]);
    }
    fn exit(&self, fn_idx: i32) {
        self.push('x', &[fn_idx, 0]);
    }
    fn scope(&self, delta: i32) {
        self.push('s', &[delta]);
    }
    fn var(&self, slot: i32, val: i32) {
        self.push('v', &[slot, val]);
    }
}

/// `kMaskOn`, in slot 0.
const K_MASK_ON: MemPtr = MemPtr::new(MemSlot(0), 0);
/// `kMaskOff`, in slot 1.
const K_MASK_OFF: MemPtr = MemPtr::new(MemSlot(1), 0);

/// `kMaskOn` and `kMaskOff`: 16 words of `~0` and of 0.
fn mask_arrays(n: usize) -> (Vec<u8>, Vec<u8>) {
    (filled(&[!0], n, 16), filled(&[0], n, 16))
}

// Port of: tests/SkRasterPipelineTest.cpp#L906-L996 (chrome/m156)
def_test!(SkRasterPipeline_TraceVar, |r| {
    let n = highp_stride();

    let (mask_on, mask_off) = mask_arrays(n);
    let indirect0 = filled(&[0], n, 16);
    let indirect1 = filled(&[1], n, 16);
    let data333 = filled(&[333], n, 16);
    let data555 = filled(&[555], n, 16);
    let data666 = filled(&[666], n, 16);
    let data777 = filled(&[777, 707], n, 32);
    let data999 = filled(&[999, 909], n, 32);
    let k_indirect0 = MemPtr::new(MemSlot(2), 0);
    let k_indirect1 = MemPtr::new(MemSlot(3), 0);
    let k_data333 = MemPtr::new(MemSlot(4), 0);
    let k_data555 = MemPtr::new(MemSlot(5), 0);
    let k_data666 = MemPtr::new(MemSlot(6), 0);
    let k_data777 = MemPtr::new(MemSlot(7), 0);
    let k_data999 = MemPtr::new(MemSlot(8), 0);

    let trace = TestTraceHook::new("v");
    let k_trace_var1 = TraceVarCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        slot_idx: 2,
        num_slots: 1,
        data: k_data333,
        indirect_offset: None,
        indirect_limit: 0,
    };
    let k_trace_var2 = TraceVarCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        slot_idx: 4,
        num_slots: 1,
        data: k_data555,
        indirect_offset: None,
        indirect_limit: 0,
    };
    let k_trace_var3 = TraceVarCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        slot_idx: 5,
        num_slots: 1,
        data: k_data666,
        indirect_offset: None,
        indirect_limit: 0,
    };
    let k_trace_var4 = TraceVarCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        slot_idx: 6,
        num_slots: 2,
        data: k_data777,
        indirect_offset: None,
        indirect_limit: 0,
    };
    let k_trace_var5 = TraceVarCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        slot_idx: 8,
        num_slots: 2,
        data: k_data999,
        indirect_offset: None,
        indirect_limit: 0,
    };
    let k_trace_var6 = TraceVarCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        slot_idx: 9,
        num_slots: 1,
        data: k_data999,
        indirect_offset: Some(k_indirect0),
        indirect_limit: 1,
    };
    let k_trace_var7 = TraceVarCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        slot_idx: 9,
        num_slots: 1,
        data: k_data999,
        indirect_offset: Some(k_indirect1),
        indirect_limit: 1,
    };

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceVar(&k_trace_var1));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceVar(&k_trace_var2));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceVar(&k_trace_var3));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceVar(&k_trace_var4));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceVar(&k_trace_var5));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceVar(&k_trace_var6));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceVar(&k_trace_var7));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&mask_on))
        .with(MemSlot(1), MemView::read(&mask_off))
        .with(MemSlot(2), MemView::read(&indirect0))
        .with(MemSlot(3), MemView::read(&indirect1))
        .with(MemSlot(4), MemView::read(&data333))
        .with(MemSlot(5), MemView::read(&data555))
        .with(MemSlot(6), MemView::read(&data666))
        .with(MemSlot(7), MemView::read(&data777))
        .with(MemSlot(8), MemView::read(&data999));
    p.run(0, 0, n, 1, &mut mem);

    reporter_assert!(
        r,
        *trace.buffer.borrow() == vec![4, 555, 6, 777, 7, 707, 9, 999, 10, 909]
    );
});

// Port of: tests/SkRasterPipelineTest.cpp#L998-L1042 (chrome/m156)
def_test!(SkRasterPipeline_TraceLine, |r| {
    let n = highp_stride();

    let (mask_on, mask_off) = mask_arrays(n);

    let trace = TestTraceHook::new("l");
    let k_trace_line1 = TraceLineCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        line_number: 123,
    };
    let k_trace_line2 = TraceLineCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        line_number: 456,
    };
    let k_trace_line3 = TraceLineCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        line_number: 567,
    };
    let k_trace_line4 = TraceLineCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        line_number: 678,
    };
    let k_trace_line5 = TraceLineCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        line_number: 789,
    };

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceLine(&k_trace_line1));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceLine(&k_trace_line2));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceLine(&k_trace_line3));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceLine(&k_trace_line4));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceLine(&k_trace_line5));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&mask_on))
        .with(MemSlot(1), MemView::read(&mask_off));
    p.run(0, 0, n, 1, &mut mem);

    reporter_assert!(r, *trace.buffer.borrow() == vec![123, 789]);
});

// Port of: tests/SkRasterPipelineTest.cpp#L1044-L1094 (chrome/m156)
def_test!(SkRasterPipeline_TraceEnterExit, |r| {
    let n = highp_stride();

    let (mask_on, mask_off) = mask_arrays(n);

    let trace = TestTraceHook::new("ex");
    let k_trace_func1 = TraceFuncCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        func_idx: 99,
    };
    let k_trace_func2 = TraceFuncCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        func_idx: 12,
    };
    let k_trace_func3 = TraceFuncCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        func_idx: 34,
    };
    let k_trace_func4 = TraceFuncCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        func_idx: 56,
    };
    let k_trace_func5 = TraceFuncCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        func_idx: 78,
    };
    let k_trace_func6 = TraceFuncCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        func_idx: 90,
    };

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceEnter(&k_trace_func1));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceEnter(&k_trace_func2));
    p.append(Stage::TraceEnter(&k_trace_func3));
    p.append(Stage::TraceExit(&k_trace_func4));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceExit(&k_trace_func5));
    p.append(Stage::TraceExit(&k_trace_func6));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&mask_on))
        .with(MemSlot(1), MemView::read(&mask_off));
    p.run(0, 0, n, 1, &mut mem);

    reporter_assert!(r, *trace.buffer.borrow() == vec![12, 1, 56, 0]);
});

// Port of: tests/SkRasterPipelineTest.cpp#L1096-L1138 (chrome/m156)
def_test!(SkRasterPipeline_TraceScope, |r| {
    let n = highp_stride();

    let (mask_on, mask_off) = mask_arrays(n);

    let trace = TestTraceHook::new("s");
    let k_trace_scope1 = TraceScopeCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        delta: 1,
    };
    let k_trace_scope2 = TraceScopeCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        delta: -2,
    };
    let k_trace_scope3 = TraceScopeCtx {
        trace_mask: K_MASK_OFF,
        trace_hook: &trace,
        delta: 3,
    };
    let k_trace_scope4 = TraceScopeCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        delta: 4,
    };
    let k_trace_scope5 = TraceScopeCtx {
        trace_mask: K_MASK_ON,
        trace_hook: &trace,
        delta: -5,
    };

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceScope(&k_trace_scope1));
    p.append(Stage::TraceScope(&k_trace_scope2));
    p.append(Stage::LoadConditionMask(K_MASK_OFF));
    p.append(Stage::TraceScope(&k_trace_scope3));
    p.append(Stage::TraceScope(&k_trace_scope4));
    p.append(Stage::LoadConditionMask(K_MASK_ON));
    p.append(Stage::TraceScope(&k_trace_scope5));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&mask_on))
        .with(MemSlot(1), MemView::read(&mask_off));
    p.run(0, 0, n, 1, &mut mem);

    reporter_assert!(r, *trace.buffer.borrow() == vec![1, 4, -5]);
});

// Port of: tests/SkRasterPipelineTest.cpp#L23-L46 (chrome/m156)
def_test!(SkRasterPipeline, |r| {
    // Build and run a simple pipeline to exercise SkRasterPipeline,
    // drawing 50% transparent blue over opaque red in half-floats.
    let red: u64 = 0x3c00_0000_0000_3c00;
    let blue: u64 = 0x3800_3800_0000_0000;
    let (red, blue) = (red.to_ne_bytes(), blue.to_ne_bytes());
    let mut result = [0u8; 8];

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadF16(MemoryCtx::new(MemSlot(0))));
    p.append(Stage::LoadF16Dst(MemoryCtx::new(MemSlot(1))));
    p.append(Stage::Srcover);
    p.append(Stage::StoreF16(MemoryCtx::new(MemSlot(2))));
    p.run(
        0,
        0,
        1,
        1,
        &mut MemoryBindings::new()
            .with(MemSlot(0), MemView::read(&blue))
            .with(MemSlot(1), MemView::read(&red))
            .with(MemSlot(2), MemView::write(&mut result)),
    );
    let result = u64::from_ne_bytes(result);

    // We should see half-intensity magenta.
    #[allow(clippy::verbose_bit_mask)] // mirrors the C++ assertion
    {
        reporter_assert!(r, ((result) & 0xffff) == 0x3800);
        reporter_assert!(r, ((result >> 16) & 0xffff) == 0x0000);
        reporter_assert!(r, ((result >> 32) & 0xffff) == 0x3800);
        reporter_assert!(r, ((result >> 48) & 0xffff) == 0x3c00);
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L3123-L3242 (chrome/m156)
def_test!(SkRasterPipeline_u16, |r| {
    {
        let data: [[u16; 2]; 4] = [
            [0x0000, 0x0111],
            [0x1010, 0x1111],
            [0x2020, 0x2121],
            [0x3030, 0x3131],
        ];
        let src: Vec<u8> = data
            .iter()
            .flatten()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        for i in 1..=4usize {
            let mut buffer = [0xabu8; 16];
            run_tail(Stage::LoadRg1616, Stage::Store8888, &src, &mut buffer, i);
            for j in 0..i {
                let expected = [(data[j][0] >> 8) as u8, (data[j][1] >> 8) as u8, 0, 0xff];
                reporter_assert!(r, buffer[4 * j..4 * j + 4] == expected);
            }
            for b in &buffer[4 * i..] {
                reporter_assert!(r, *b == 0xab);
            }
        }
    }

    {
        let data: [u16; 4] = [0x0000, 0x1010, 0x2020, 0x3030];
        let src = halves_to_bytes(&data);
        for i in 1..=4usize {
            let mut buffer = [0xffu8; 16];
            run_tail(Stage::LoadA16, Stage::Store8888, &src, &mut buffer, i);
            for j in 0..i {
                let expected = [0x00, 0x00, 0x00, (data[j] >> 8) as u8];
                reporter_assert!(r, buffer[4 * j..4 * j + 4] == expected);
            }
            for b in &buffer[4 * i..] {
                reporter_assert!(r, *b == 0xff);
            }
        }
    }

    {
        let data: [[u8; 4]; 4] = [
            [0x00, 0x01, 0x02, 0x03],
            [0x10, 0x11, 0x12, 0x13],
            [0x20, 0x21, 0x22, 0x23],
            [0x30, 0x31, 0x32, 0x33],
        ];
        let src: Vec<u8> = data.iter().flatten().copied().collect();
        for i in 1..=4usize {
            let mut buffer = [0xffu8; 8];
            run_tail(Stage::Load8888, Stage::StoreA16, &src, &mut buffer, i);
            let buffer = bytes_to_halves(&buffer);
            for j in 0..i {
                let expected = (u16::from(data[j][3]) << 8) | u16::from(data[j][3]);
                reporter_assert!(r, buffer[j] == expected);
            }
            for v in &buffer[i..] {
                reporter_assert!(r, *v == 0xffff);
            }
        }
    }

    {
        let data: [[u16; 4]; 4] = [
            [0x0000, 0x1000, 0x2000, 0x3000],
            [0x0001, 0x1001, 0x2001, 0x3001],
            [0x0002, 0x1002, 0x2002, 0x3002],
            [0x0003, 0x1003, 0x2003, 0x3003],
        ];
        let src = halves_to_bytes(&data.iter().flatten().copied().collect::<Vec<_>>());
        for i in 1..=4usize {
            let mut buffer = [0xffu8; 32];
            let mut p = RasterPipeline::new();
            p.append(Stage::Load16161616(MemoryCtx::new(MemSlot(0))));
            p.append(Stage::SwapRb);
            p.append(Stage::Store16161616(MemoryCtx::new(MemSlot(1))));
            p.run(
                0,
                0,
                i,
                1,
                &mut MemoryBindings::new()
                    .with(MemSlot(0), MemView::read(&src))
                    .with(MemSlot(1), MemView::write(&mut buffer)),
            );
            let buffer = bytes_to_halves(&buffer);
            for j in 0..i {
                let expected = [data[j][2], data[j][1], data[j][0], data[j][3]];
                reporter_assert!(r, expected == buffer[4 * j..4 * j + 4]);
            }
            for u in &buffer[4 * i..] {
                reporter_assert!(r, *u == 0xffff);
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L3244-L3270 (chrome/m156)
def_test!(SkRasterPipeline_lowp, |r| {
    let mut rgba = [0u32; 64];
    for (i, px) in (0u32..).zip(rgba.iter_mut()) {
        *px = (4 * i) | ((4 * i + 1) << 8) | ((4 * i + 2) << 16) | ((4 * i + 3) << 24);
    }
    // `MemoryCtx ptr = { rgba, 0 }`: the pixels are bound to the context's slot for the run.
    let mut bytes: Vec<u8> = rgba.iter().flat_map(|px| px.to_ne_bytes()).collect();
    let ptr = MemoryCtx::new(MemSlot(0));

    let mut p = RasterPipeline::new();
    p.append(Stage::Load8888(ptr));
    p.append(Stage::SwapRb);
    p.append(Stage::Store8888(ptr));
    let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut bytes));
    p.run(0, 0, 64, 1, &mut mem);
    drop(mem);

    for (i, c) in (0u32..).zip(bytes.as_chunks::<4>().0) {
        let want = ((4 * i) << 16) | ((4 * i + 1) << 8) | (4 * i + 2) | ((4 * i + 3) << 24);
        let got = u32::from_ne_bytes(*c);
        if got != want {
            errorf!(r, "got {got:08x}, want {want:08x}\n");
        }
    }
});
