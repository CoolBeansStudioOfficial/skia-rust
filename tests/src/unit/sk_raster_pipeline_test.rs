// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (its arena, where needed, an
// `ArenaAlloc`); `p.run(x, y, w, h)` takes the run's writable memory as a `MemoryBindings`
// (empty here). `SkArenaAllocWithReset(storage, 128, 500)` is an `ArenaAlloc`.

// The ported tests compare floats exactly (as the C++ does), keep the C++ declaration order and
// the C++ int/float casts.
#![allow(
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::erasing_op,
    clippy::identity_op,
    clippy::needless_range_loop,
    clippy::explicit_counter_loop,
    clippy::similar_names
)]

use std::cell::RefCell;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::raster_pipeline_context_utils::{Packed, pack, unpack};
use skia_rust_simd::rp::contexts::{
    BranchCtx, BranchIfEqualCtx, CaseOpCtx, ConstantCtx, CopyIndirectCtx, CopyIndirectUniformCtx,
    ShuffleCtx, SwizzleCopyCtx, SwizzleCopyIndirectCtx, SwizzleCtx, TraceFuncCtx, TraceHook,
    TraceLineCtx, TraceScopeCtx, TraceVarCtx, UniformCtx,
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

// ~~~ SkSL arithmetic stages (B6b): matrix multiply, arithmetic, comparisons, unary ops, mix ~~~
//
// Mapping notes: `alignas(64) float slots[N * kMaxStride_highp]` is a `Slots` of 32-bit words
// bound as the run's memory (slot 0); `p.append(op, &slots[0])` is `Stage::Op(MemPtr)` and
// `set_base_pointer` takes the same pointer. `SkOpts::raster_pipeline_highp_stride` is the
// current tier's highp stride. The packed contexts (`SkRPCtxUtils::Pack`) are plain values.

use skia_rust_simd::rp::contexts::{BinaryOpCtx, MatrixMultiplyCtx, TernaryOpCtx};

/// `SkRasterPipelineContexts::kMaxStride_highp`.
const MAX_STRIDE_HIGHP: usize = 16;

/// `slots`: `count * kMaxStride_highp` 32-bit words, read as `float` or `int`.
struct Slots(Vec<u32>);

impl Slots {
    fn new(count: usize) -> Slots {
        Slots(vec![0; count * MAX_STRIDE_HIGHP])
    }

    fn float(&self, index: usize) -> f32 {
        f32::from_bits(self.0[index])
    }

    fn set_float(&mut self, index: usize, value: f32) {
        self.0[index] = value.to_bits();
    }

    fn int(&self, index: usize) -> i32 {
        i32::from_ne_bytes(self.0[index].to_ne_bytes())
    }

    fn set_int(&mut self, index: usize, value: i32) {
        self.0[index] = u32::from_ne_bytes(value.to_ne_bytes());
    }

    /// `&slots[0]`.
    fn ptr() -> MemPtr {
        MemPtr::new(MemSlot(0), 0)
    }

    /// `p.run(0,0,1,1)` over `stages`, with `slots` bound as the pipeline's memory.
    fn run(&mut self, stages: &[Stage<'_>]) {
        let mut p = RasterPipeline::new();
        for stage in stages {
            p.append(*stage);
        }
        let mut bytes: Vec<u8> = self.0.iter().flat_map(|w| w.to_ne_bytes()).collect();
        {
            let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut bytes));
            p.run(0, 0, 1, 1, &mut mem);
        }
        for (word, chunk) in self.0.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *word = u32::from_ne_bytes(*chunk);
        }
    }
}

/// `kLastSignalingNaN` (the largest positive signaling NaN's bit pattern).
const LAST_SIGNALING_NAN: i32 = 0x7fbf_ffff;

/// Fills `slots[from..to]` with `start, start + 1, ...` (`std::iota` over floats).
fn iota_float(slots: &mut Slots, from: usize, to: usize, start: f32) {
    let mut value = start;
    for index in from..to {
        slots.set_float(index, value);
        value += 1.0;
    }
}

/// `std::iota` over ints.
fn iota_int(slots: &mut Slots, from: usize, to: usize, start: i32) {
    let mut value = start;
    for index in from..to {
        slots.set_int(index, value);
        value = value.wrapping_add(1);
    }
}

/// Checks every element of a `dim x dim` matrix multiply's result against the dot product
/// `dot += left[n][r] * right[c][n]` (the result, left and right matrices are adjacent slots).
fn check_matrix_multiply(reporter: &mut crate::Reporter, slots: &Slots, dim: usize, n: usize) {
    // The slot index of destPtr[c][r], leftMtx[c][r] and rightMtx[c][r].
    let dest = |c: usize, r: usize| (c * dim + r) * n;
    let left = |c: usize, r: usize| (dim * dim + c * dim + r) * n;
    let right = |c: usize, r: usize| (2 * dim * dim + c * dim + r) * n;
    for c in 0..dim {
        for r in 0..dim {
            for lane in 0..n {
                // Dot a vector from leftMtx[*][r] with rightMtx[c][*].
                let mut dot = 0.0f32;
                for k in 0..dim {
                    dot += slots.float(left(k, r) + lane) * slots.float(right(c, k) + lane);
                }
                reporter_assert!(reporter, slots.float(dest(c, r) + lane) == dot);
            }
        }
    }
}

// Port of: tests/SkRasterPipelineTest.cpp#L1516-L1559 (chrome/m156)
def_test!(SkRasterPipeline_MatrixMultiply2x2, |reporter| {
    let mut slots = Slots::new(12);
    let n = highp_stride();

    // Populate the left- and right-matrix data. Slots 0-3 hold the result and are left as-is.
    iota_float(&mut slots, 4 * n, 12 * n, 1.0);

    // Perform a 2x2 matrix multiply.
    let ctx = MatrixMultiplyCtx {
        dst: 0,
        left_columns: 2,
        left_rows: 2,
        right_columns: 2,
        right_rows: 2,
    };
    slots.run(&[
        Stage::SetBasePointer(Slots::ptr()),
        Stage::MatrixMultiply2(ctx),
    ]);

    // Verify that the result slots hold a 2x2 matrix multiply.
    check_matrix_multiply(reporter, &slots, 2, n);
});

// Port of: tests/SkRasterPipelineTest.cpp#L1561-L1612 (chrome/m156)
def_test!(SkRasterPipeline_MatrixMultiply3x3, |reporter| {
    let mut slots = Slots::new(27);
    let n = highp_stride();

    // Populate the left- and right-matrix data. Slots 0-8 hold the result and are left as-is.
    // To keep results in full-precision float range, we only set values between 0 and 25.
    let mut value = 0.0f32;
    for idx in 9 * n..27 * n {
        slots.set_float(idx, value);
        value = (value + 1.0) % 25.0;
    }

    // Perform a 3x3 matrix multiply.
    let ctx = MatrixMultiplyCtx {
        dst: 0,
        left_columns: 3,
        left_rows: 3,
        right_columns: 3,
        right_rows: 3,
    };
    slots.run(&[
        Stage::SetBasePointer(Slots::ptr()),
        Stage::MatrixMultiply3(ctx),
    ]);

    // Verify that the result slots hold a 3x3 matrix multiply.
    check_matrix_multiply(reporter, &slots, 3, n);
});

// Port of: tests/SkRasterPipelineTest.cpp#L1614-L1668 (chrome/m156)
def_test!(SkRasterPipeline_MatrixMultiply4x4, |reporter| {
    let mut slots = Slots::new(48);
    let n = highp_stride();

    // Populate the left- and right-matrix data. Slots 0-8 hold the result and are left as-is.
    // To keep results in full-precision float range, we only set values between 0 and 25.
    let mut value = 0.0f32;
    for idx in 16 * n..48 * n {
        slots.set_float(idx, value);
        value = (value + 1.0) % 25.0;
    }

    // Perform a 4x4 matrix multiply.
    let ctx = MatrixMultiplyCtx {
        dst: 0,
        left_columns: 4,
        left_rows: 4,
        right_columns: 4,
        right_rows: 4,
    };
    slots.run(&[
        Stage::SetBasePointer(Slots::ptr()),
        Stage::MatrixMultiply4(ctx),
    ]);

    // Verify that the result slots hold a 4x4 matrix multiply.
    check_matrix_multiply(reporter, &slots, 4, n);
});

/// A stage taking a pointer to the slots (`p.append(op, &slots[0])`).
type PtrStage = fn(MemPtr) -> Stage<'static>;
/// A stage taking a packed `BinaryOpCtx`.
type BinaryStage = fn(BinaryOpCtx) -> Stage<'static>;

// Port of: tests/SkRasterPipelineTest.cpp#L1670-L1721 (chrome/m156)
def_test!(SkRasterPipeline_FloatArithmeticWithNSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct ArithmeticOp {
        stage: BinaryStage,
        verify: fn(f32, f32) -> f32,
    }
    let op = |stage: BinaryStage, verify: fn(f32, f32) -> f32| ArithmeticOp { stage, verify };

    let k_arithmetic_ops = [
        op(Stage::AddNFloats, |a, b| a + b),
        op(Stage::SubNFloats, |a, b| a - b),
        op(Stage::MulNFloats, |a, b| a * b),
        op(Stage::DivNFloats, |a, b| a / b),
    ];

    for op in &k_arithmetic_ops {
        for num_slots_affected in 1..=5usize {
            // Initialize the slot values to 1,2,3...
            iota_float(&mut slots, 0, 10 * n, 1.0);

            // Run the arithmetic op over our data.
            let ctx = BinaryOpCtx {
                dst: 0,
                src: u32::try_from(num_slots_affected * n * size_of::<f32>()).unwrap(),
            };
            slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.stage)(ctx)]);

            // Verify that the affected slots now equal (1,2,3...) op (4,5,6...).
            let mut left_value = 1.0f32;
            let mut right_value = (num_slots_affected * n) as f32 + 1.0;
            let mut dest = 0;
            for check_slot in 0..10 {
                for _check_lane in 0..n {
                    if check_slot < num_slots_affected {
                        reporter_assert!(
                            r,
                            slots.float(dest) == (op.verify)(left_value, right_value)
                        );
                    } else {
                        reporter_assert!(r, slots.float(dest) == left_value);
                    }

                    dest += 1;
                    left_value += 1.0;
                    right_value += 1.0;
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1723-L1784 (chrome/m156)
def_test!(SkRasterPipeline_FloatArithmeticWithHardcodedSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct ArithmeticOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(f32, f32) -> f32,
    }
    let op =
        |stage: PtrStage, num_slots_affected: usize, verify: fn(f32, f32) -> f32| ArithmeticOp {
            stage,
            num_slots_affected,
            verify,
        };
    let add: fn(f32, f32) -> f32 = |a, b| a + b;
    let sub: fn(f32, f32) -> f32 = |a, b| a - b;
    let mul: fn(f32, f32) -> f32 = |a, b| a * b;
    let div: fn(f32, f32) -> f32 = |a, b| a / b;

    let k_arithmetic_ops = [
        op(Stage::AddFloat, 1, add),
        op(Stage::SubFloat, 1, sub),
        op(Stage::MulFloat, 1, mul),
        op(Stage::DivFloat, 1, div),
        op(Stage::Add2Floats, 2, add),
        op(Stage::Sub2Floats, 2, sub),
        op(Stage::Mul2Floats, 2, mul),
        op(Stage::Div2Floats, 2, div),
        op(Stage::Add3Floats, 3, add),
        op(Stage::Sub3Floats, 3, sub),
        op(Stage::Mul3Floats, 3, mul),
        op(Stage::Div3Floats, 3, div),
        op(Stage::Add4Floats, 4, add),
        op(Stage::Sub4Floats, 4, sub),
        op(Stage::Mul4Floats, 4, mul),
        op(Stage::Div4Floats, 4, div),
    ];

    for op in &k_arithmetic_ops {
        // Initialize the slot values to 1,2,3...
        iota_float(&mut slots, 0, 10 * n, 1.0);

        // Run the arithmetic op over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the affected slots now equal (1,2,3...) op (4,5,6...).
        let mut left_value = 1.0f32;
        let mut right_value = (op.num_slots_affected * n) as f32 + 1.0;
        let mut dest = 0;
        for check_slot in 0..10 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    reporter_assert!(r, slots.float(dest) == (op.verify)(left_value, right_value));
                } else {
                    reporter_assert!(r, slots.float(dest) == left_value);
                }

                dest += 1;
                left_value += 1.0;
                right_value += 1.0;
            }
        }
    }
});

#[allow(clippy::cast_sign_loss)] // mirrors the uint32_t casts
fn divide_unsigned(a: i32, b: i32) -> i32 {
    ((a as u32) / (b as u32)) as i32
}
#[allow(clippy::cast_sign_loss)] // mirrors the uint32_t casts
fn min_unsigned(a: i32, b: i32) -> i32 {
    if (a as u32) < (b as u32) { a } else { b }
}
#[allow(clippy::cast_sign_loss)] // mirrors the uint32_t casts
fn max_unsigned(a: i32, b: i32) -> i32 {
    if (a as u32) > (b as u32) { a } else { b }
}

// Port of: tests/SkRasterPipelineTest.cpp#L1790-L1849 (chrome/m156)
def_test!(SkRasterPipeline_IntArithmeticWithNSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct ArithmeticOp {
        stage: BinaryStage,
        verify: fn(i32, i32) -> i32,
    }
    let op = |stage: BinaryStage, verify: fn(i32, i32) -> i32| ArithmeticOp { stage, verify };

    let k_arithmetic_ops = [
        op(Stage::AddNInts, i32::wrapping_add),
        op(Stage::SubNInts, i32::wrapping_sub),
        op(Stage::MulNInts, i32::wrapping_mul),
        op(Stage::DivNInts, |a, b| a / b),
        op(Stage::DivNUints, divide_unsigned),
        op(Stage::BitwiseAndNInts, |a, b| a & b),
        op(Stage::BitwiseOrNInts, |a, b| a | b),
        op(Stage::BitwiseXorNInts, |a, b| a ^ b),
        op(Stage::MinNInts, |a, b| if a < b { a } else { b }),
        op(Stage::MinNUints, min_unsigned),
        op(Stage::MaxNInts, |a, b| if a > b { a } else { b }),
        op(Stage::MaxNUints, max_unsigned),
    ];

    for op in &k_arithmetic_ops {
        for num_slots_affected in 1..=5usize {
            // Initialize the slot values to 1,2,3...
            iota_int(&mut slots, 0, 10 * n, 1);
            let mut left_value = slots.int(0);
            let mut right_value = slots.int(num_slots_affected * n);

            // Run the op (e.g. `add_n_ints`) over our data.
            let ctx = BinaryOpCtx {
                dst: 0,
                src: u32::try_from(num_slots_affected * n * size_of::<f32>()).unwrap(),
            };
            slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.stage)(ctx)]);

            // Verify that the affected slots now equal (1,2,3...) op (4,5,6...).
            let mut dest = 0;
            for check_slot in 0..10 {
                for _check_lane in 0..n {
                    if check_slot < num_slots_affected {
                        reporter_assert!(
                            r,
                            slots.int(dest) == (op.verify)(left_value, right_value)
                        );
                    } else {
                        reporter_assert!(r, slots.int(dest) == left_value);
                    }

                    dest += 1;
                    left_value += 1;
                    right_value += 1;
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1851-L1944 (chrome/m156)
def_test!(SkRasterPipeline_IntArithmeticWithHardcodedSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct ArithmeticOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(i32, i32) -> i32,
    }
    let op =
        |stage: PtrStage, num_slots_affected: usize, verify: fn(i32, i32) -> i32| ArithmeticOp {
            stage,
            num_slots_affected,
            verify,
        };
    let add: fn(i32, i32) -> i32 = |a, b| a.wrapping_add(b);
    let sub: fn(i32, i32) -> i32 = |a, b| a.wrapping_sub(b);
    let mul: fn(i32, i32) -> i32 = |a, b| a.wrapping_mul(b);
    let div: fn(i32, i32) -> i32 = |a, b| a / b;
    let and: fn(i32, i32) -> i32 = |a, b| a & b;
    let or: fn(i32, i32) -> i32 = |a, b| a | b;
    let xor: fn(i32, i32) -> i32 = |a, b| a ^ b;
    let min: fn(i32, i32) -> i32 = |a, b| if a < b { a } else { b };
    let max: fn(i32, i32) -> i32 = |a, b| if a > b { a } else { b };

    let k_arithmetic_ops = [
        op(Stage::AddInt, 1, add),
        op(Stage::SubInt, 1, sub),
        op(Stage::MulInt, 1, mul),
        op(Stage::DivInt, 1, div),
        op(Stage::DivUint, 1, divide_unsigned),
        op(Stage::BitwiseAndInt, 1, and),
        op(Stage::BitwiseOrInt, 1, or),
        op(Stage::BitwiseXorInt, 1, xor),
        op(Stage::MinInt, 1, min),
        op(Stage::MinUint, 1, min_unsigned),
        op(Stage::MaxInt, 1, max),
        op(Stage::MaxUint, 1, max_unsigned),
        op(Stage::Add2Ints, 2, add),
        op(Stage::Sub2Ints, 2, sub),
        op(Stage::Mul2Ints, 2, mul),
        op(Stage::Div2Ints, 2, div),
        op(Stage::Div2Uints, 2, divide_unsigned),
        op(Stage::BitwiseAnd2Ints, 2, and),
        op(Stage::BitwiseOr2Ints, 2, or),
        op(Stage::BitwiseXor2Ints, 2, xor),
        op(Stage::Min2Ints, 2, min),
        op(Stage::Min2Uints, 2, min_unsigned),
        op(Stage::Max2Ints, 2, max),
        op(Stage::Max2Uints, 2, max_unsigned),
        op(Stage::Add3Ints, 3, add),
        op(Stage::Sub3Ints, 3, sub),
        op(Stage::Mul3Ints, 3, mul),
        op(Stage::Div3Ints, 3, div),
        op(Stage::Div3Uints, 3, divide_unsigned),
        op(Stage::BitwiseAnd3Ints, 3, and),
        op(Stage::BitwiseOr3Ints, 3, or),
        op(Stage::BitwiseXor3Ints, 3, xor),
        op(Stage::Min3Ints, 3, min),
        op(Stage::Min3Uints, 3, min_unsigned),
        op(Stage::Max3Ints, 3, max),
        op(Stage::Max3Uints, 3, max_unsigned),
        op(Stage::Add4Ints, 4, add),
        op(Stage::Sub4Ints, 4, sub),
        op(Stage::Mul4Ints, 4, mul),
        op(Stage::Div4Ints, 4, div),
        op(Stage::Div4Uints, 4, divide_unsigned),
        op(Stage::BitwiseAnd4Ints, 4, and),
        op(Stage::BitwiseOr4Ints, 4, or),
        op(Stage::BitwiseXor4Ints, 4, xor),
        op(Stage::Min4Ints, 4, min),
        op(Stage::Min4Uints, 4, min_unsigned),
        op(Stage::Max4Ints, 4, max),
        op(Stage::Max4Uints, 4, max_unsigned),
    ];

    for op in &k_arithmetic_ops {
        // Initialize the slot values to 1,2,3...
        iota_int(&mut slots, 0, 10 * n, 1);
        let mut left_value = slots.int(0);
        let mut right_value = slots.int(op.num_slots_affected * n);

        // Run the op (e.g. `add_2_ints`) over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the affected slots now equal (1,2,3...) op (4,5,6...).
        let mut dest = 0;
        for check_slot in 0..10 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    reporter_assert!(r, slots.int(dest) == (op.verify)(left_value, right_value));
                } else {
                    reporter_assert!(r, slots.int(dest) == left_value);
                }

                dest += 1;
                left_value += 1;
                right_value += 1;
            }
        }
    }
});

/// `compareIsTrue ? ~0 : 0`.
fn compare_mask(compare_is_true: bool) -> i32 {
    if compare_is_true { !0 } else { 0 }
}

// Port of: tests/SkRasterPipelineTest.cpp#L1946-L2001 (chrome/m156)
def_test!(SkRasterPipeline_CompareFloatsWithNSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct CompareOp {
        stage: BinaryStage,
        verify: fn(f32, f32) -> bool,
    }
    let op = |stage: BinaryStage, verify: fn(f32, f32) -> bool| CompareOp { stage, verify };

    let k_compare_ops = [
        op(Stage::CmpeqNFloats, |a, b| a == b),
        op(Stage::CmpneNFloats, |a, b| a != b),
        op(Stage::CmpltNFloats, |a, b| a < b),
        op(Stage::CmpleNFloats, |a, b| a <= b),
    ];

    for op in &k_compare_ops {
        for num_slots_affected in 1..=5usize {
            // Initialize the slot values to 0,1,2,0,1,2,0,1,2...
            for index in 0..10 * n {
                slots.set_float(index, (index as f32) % 3.0);
            }

            let mut left_value = slots.float(0);
            let mut right_value = slots.float(num_slots_affected * n);

            // Run the comparison op over our data.
            let ctx = BinaryOpCtx {
                dst: 0,
                src: u32::try_from(num_slots_affected * n * size_of::<f32>()).unwrap(),
            };
            slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.stage)(ctx)]);

            // Verify that the affected slots now contain "(0,1,2,0...) op (1,2,0,1...)".
            let mut dest = 0;
            for check_slot in 0..10 {
                for _check_lane in 0..n {
                    if check_slot < num_slots_affected {
                        let compare_is_true = (op.verify)(left_value, right_value);
                        reporter_assert!(r, slots.int(dest) == compare_mask(compare_is_true));
                    } else {
                        reporter_assert!(r, slots.float(dest) == left_value);
                    }

                    dest += 1;
                    left_value = (left_value + 1.0) % 3.0;
                    right_value = (right_value + 1.0) % 3.0;
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2003-L2068 (chrome/m156)
def_test!(SkRasterPipeline_CompareFloatsWithHardcodedSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct CompareOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(f32, f32) -> bool,
    }
    let op = |stage: PtrStage, num_slots_affected: usize, verify: fn(f32, f32) -> bool| CompareOp {
        stage,
        num_slots_affected,
        verify,
    };
    let eq: fn(f32, f32) -> bool = |a, b| a == b;
    let ne: fn(f32, f32) -> bool = |a, b| a != b;
    let lt: fn(f32, f32) -> bool = |a, b| a < b;
    let le: fn(f32, f32) -> bool = |a, b| a <= b;

    let k_compare_ops = [
        op(Stage::CmpeqFloat, 1, eq),
        op(Stage::CmpneFloat, 1, ne),
        op(Stage::CmpltFloat, 1, lt),
        op(Stage::CmpleFloat, 1, le),
        op(Stage::Cmpeq2Floats, 2, eq),
        op(Stage::Cmpne2Floats, 2, ne),
        op(Stage::Cmplt2Floats, 2, lt),
        op(Stage::Cmple2Floats, 2, le),
        op(Stage::Cmpeq3Floats, 3, eq),
        op(Stage::Cmpne3Floats, 3, ne),
        op(Stage::Cmplt3Floats, 3, lt),
        op(Stage::Cmple3Floats, 3, le),
        op(Stage::Cmpeq4Floats, 4, eq),
        op(Stage::Cmpne4Floats, 4, ne),
        op(Stage::Cmplt4Floats, 4, lt),
        op(Stage::Cmple4Floats, 4, le),
    ];

    for op in &k_compare_ops {
        // Initialize the slot values to 0,1,2,0,1,2,0,1,2...
        for index in 0..10 * n {
            slots.set_float(index, (index as f32) % 3.0);
        }

        let mut left_value = slots.float(0);
        let mut right_value = slots.float(op.num_slots_affected * n);

        // Run the comparison op over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the affected slots now contain "(0,1,2,0...) op (1,2,0,1...)".
        let mut dest = 0;
        for check_slot in 0..10 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    let compare_is_true = (op.verify)(left_value, right_value);
                    reporter_assert!(r, slots.int(dest) == compare_mask(compare_is_true));
                } else {
                    reporter_assert!(r, slots.float(dest) == left_value);
                }

                dest += 1;
                left_value = (left_value + 1.0) % 3.0;
                right_value = (right_value + 1.0) % 3.0;
            }
        }
    }
});

#[allow(clippy::cast_sign_loss)] // mirrors the uint32_t casts
fn compare_lt_uint(a: i32, b: i32) -> bool {
    (a as u32) < (b as u32)
}
#[allow(clippy::cast_sign_loss)] // mirrors the uint32_t casts
fn compare_lteq_uint(a: i32, b: i32) -> bool {
    (a as u32) <= (b as u32)
}

// Port of: tests/SkRasterPipelineTest.cpp#L2073-L2134 (chrome/m156)
def_test!(SkRasterPipeline_CompareIntsWithNSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct CompareOp {
        stage: BinaryStage,
        verify: fn(i32, i32) -> bool,
    }
    let op = |stage: BinaryStage, verify: fn(i32, i32) -> bool| CompareOp { stage, verify };

    let k_compare_ops = [
        op(Stage::CmpeqNInts, |a, b| a == b),
        op(Stage::CmpneNInts, |a, b| a != b),
        op(Stage::CmpltNInts, |a, b| a < b),
        op(Stage::CmpleNInts, |a, b| a <= b),
        op(Stage::CmpltNUints, compare_lt_uint),
        op(Stage::CmpleNUints, compare_lteq_uint),
    ];

    for op in &k_compare_ops {
        for num_slots_affected in 1..=5usize {
            // Initialize the slot values to -1,0,1,-1,0,1,-1,0,1,-1...
            for index in 0..10 * n {
                slots.set_int(index, (index % 3) as i32 - 1);
            }

            let mut left_value = slots.int(0);
            let mut right_value = slots.int(num_slots_affected * n);

            // Run the comparison op over our data.
            let ctx = BinaryOpCtx {
                dst: 0,
                src: u32::try_from(size_of::<f32>() * num_slots_affected * n).unwrap(),
            };
            slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.stage)(ctx)]);

            // Verify that the affected slots now contain "(-1,0,1,-1...) op (0,1,-1,0...)".
            let mut dest = 0;
            for check_slot in 0..10 {
                for _check_lane in 0..n {
                    if check_slot < num_slots_affected {
                        let compare_is_true = (op.verify)(left_value, right_value);
                        reporter_assert!(r, slots.int(dest) == compare_mask(compare_is_true));
                    } else {
                        reporter_assert!(r, slots.int(dest) == left_value);
                    }

                    dest += 1;
                    left_value += 1;
                    if left_value == 2 {
                        left_value = -1;
                    }
                    right_value += 1;
                    if right_value == 2 {
                        right_value = -1;
                    }
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2136-L2213 (chrome/m156)
def_test!(SkRasterPipeline_CompareIntsWithHardcodedSlots, |r| {
    // Allocate space for 5 dest and 5 source slots.
    let mut slots = Slots::new(10);
    let n = highp_stride();

    struct CompareOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(i32, i32) -> bool,
    }
    let op = |stage: PtrStage, num_slots_affected: usize, verify: fn(i32, i32) -> bool| CompareOp {
        stage,
        num_slots_affected,
        verify,
    };
    let eq: fn(i32, i32) -> bool = |a, b| a == b;
    let ne: fn(i32, i32) -> bool = |a, b| a != b;
    let lt: fn(i32, i32) -> bool = |a, b| a < b;
    let le: fn(i32, i32) -> bool = |a, b| a <= b;

    let k_compare_ops = [
        op(Stage::CmpeqInt, 1, eq),
        op(Stage::CmpneInt, 1, ne),
        op(Stage::CmpltInt, 1, lt),
        op(Stage::CmpleInt, 1, le),
        op(Stage::CmpltUint, 1, compare_lt_uint),
        op(Stage::CmpleUint, 1, compare_lteq_uint),
        op(Stage::Cmpeq2Ints, 2, eq),
        op(Stage::Cmpne2Ints, 2, ne),
        op(Stage::Cmplt2Ints, 2, lt),
        op(Stage::Cmple2Ints, 2, le),
        op(Stage::Cmplt2Uints, 2, compare_lt_uint),
        op(Stage::Cmple2Uints, 2, compare_lteq_uint),
        op(Stage::Cmpeq3Ints, 3, eq),
        op(Stage::Cmpne3Ints, 3, ne),
        op(Stage::Cmplt3Ints, 3, lt),
        op(Stage::Cmple3Ints, 3, le),
        op(Stage::Cmplt3Uints, 3, compare_lt_uint),
        op(Stage::Cmple3Uints, 3, compare_lteq_uint),
        op(Stage::Cmpeq4Ints, 4, eq),
        op(Stage::Cmpne4Ints, 4, ne),
        op(Stage::Cmplt4Ints, 4, lt),
        op(Stage::Cmple4Ints, 4, le),
        op(Stage::Cmplt4Uints, 4, compare_lt_uint),
        op(Stage::Cmple4Uints, 4, compare_lteq_uint),
    ];

    for op in &k_compare_ops {
        // Initialize the slot values to -1,0,1,-1,0,1,-1,0,1,-1...
        for index in 0..10 * n {
            slots.set_int(index, (index % 3) as i32 - 1);
        }

        let mut left_value = slots.int(0);
        let mut right_value = slots.int(op.num_slots_affected * n);

        // Run the comparison op over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the affected slots now contain "(0,1,2,0...) op (1,2,0,1...)".
        let mut dest = 0;
        for check_slot in 0..10 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    let compare_is_true = (op.verify)(left_value, right_value);
                    reporter_assert!(r, slots.int(dest) == compare_mask(compare_is_true));
                } else {
                    reporter_assert!(r, slots.int(dest) == left_value);
                }

                dest += 1;
                left_value += 1;
                if left_value == 2 {
                    left_value = -1;
                }
                right_value += 1;
                if right_value == 2 {
                    right_value = -1;
                }
            }
        }
    }
});

#[allow(clippy::cast_possible_wrap)] // sk_bit_cast<int>(float)
fn to_float(a: i32) -> i32 {
    (a as f32).to_bits() as i32
}

// Port of: tests/SkRasterPipelineTest.cpp#L2217-L2267 (chrome/m156)
def_test!(SkRasterPipeline_UnaryIntOps, |r| {
    // Allocate space for 5 slots.
    let mut slots = Slots::new(5);
    let n = highp_stride();

    struct UnaryOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(i32) -> i32,
    }
    let op = |stage: PtrStage, num_slots_affected: usize, verify: fn(i32) -> i32| UnaryOp {
        stage,
        num_slots_affected,
        verify,
    };
    let abs: fn(i32) -> i32 = |a| if a < 0 { -a } else { a };

    let k_unary_ops = [
        op(Stage::CastToFloatFromInt, 1, to_float),
        op(Stage::CastToFloatFrom2Ints, 2, to_float),
        op(Stage::CastToFloatFrom3Ints, 3, to_float),
        op(Stage::CastToFloatFrom4Ints, 4, to_float),
        op(Stage::AbsInt, 1, abs),
        op(Stage::Abs2Ints, 2, abs),
        op(Stage::Abs3Ints, 3, abs),
        op(Stage::Abs4Ints, 4, abs),
    ];

    for op in &k_unary_ops {
        // Initialize the slot values to -10,-9,-8...
        iota_int(&mut slots, 0, 5 * n, -10);
        let mut input_value = slots.int(0);

        // Run the unary op over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the destination slots have been updated.
        let mut dest = 0;
        for check_slot in 0..5 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    let expected = (op.verify)(input_value);
                    reporter_assert!(r, slots.int(dest) == expected);
                } else {
                    reporter_assert!(r, slots.int(dest) == input_value);
                }

                dest += 1;
                input_value += 1;
            }
        }
    }
});

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors (int)a
fn to_int(a: f32) -> f32 {
    f32::from_bits(i32::cast_unsigned(a as i32))
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors (unsigned int)a
fn to_uint(a: f32) -> f32 {
    f32::from_bits(a as u32)
}

// Port of: tests/SkRasterPipelineTest.cpp#L2272-L2345 (chrome/m156)
def_test!(SkRasterPipeline_UnaryFloatOps, |r| {
    // Allocate space for 5 slots.
    let mut slots = Slots::new(5);
    let n = highp_stride();

    struct UnaryOp {
        stage: PtrStage,
        num_slots_affected: usize,
        verify: fn(f32) -> f32,
    }
    let op = |stage: PtrStage, num_slots_affected: usize, verify: fn(f32) -> f32| UnaryOp {
        stage,
        num_slots_affected,
        verify,
    };
    let floor: fn(f32) -> f32 = f32::floor;
    let ceil: fn(f32) -> f32 = f32::ceil;

    let k_unary_ops = [
        op(Stage::CastToIntFromFloat, 1, to_int),
        op(Stage::CastToIntFrom2Floats, 2, to_int),
        op(Stage::CastToIntFrom3Floats, 3, to_int),
        op(Stage::CastToIntFrom4Floats, 4, to_int),
        op(Stage::CastToUintFromFloat, 1, to_uint),
        op(Stage::CastToUintFrom2Floats, 2, to_uint),
        op(Stage::CastToUintFrom3Floats, 3, to_uint),
        op(Stage::CastToUintFrom4Floats, 4, to_uint),
        op(Stage::FloorFloat, 1, floor),
        op(Stage::Floor2Floats, 2, floor),
        op(Stage::Floor3Floats, 3, floor),
        op(Stage::Floor4Floats, 4, floor),
        op(Stage::CeilFloat, 1, ceil),
        op(Stage::Ceil2Floats, 2, ceil),
        op(Stage::Ceil3Floats, 3, ceil),
        op(Stage::Ceil4Floats, 4, ceil),
    ];

    for op in &k_unary_ops {
        // The result of some ops are undefined with negative inputs, so only test positive values.
        let positive_only = matches!(
            (op.stage)(Slots::ptr()),
            Stage::CastToUintFromFloat(_)
                | Stage::CastToUintFrom2Floats(_)
                | Stage::CastToUintFrom3Floats(_)
                | Stage::CastToUintFrom4Floats(_)
        );

        let iota_start = if positive_only { 1.0f32 } else { -9.75f32 };
        iota_float(&mut slots, 0, 5 * n, iota_start);
        let mut input_value = slots.float(0);

        // Run the unary op over our data.
        slots.run(&[(op.stage)(Slots::ptr())]);

        // Verify that the destination slots have been updated.
        let mut dest = 0;
        for check_slot in 0..5 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    let expected = (op.verify)(input_value);
                    // The casting tests can generate NaN, depending on the input value, so a value
                    // match (via ==) might not succeed.
                    // The ceil tests can generate negative zeros _sometimes_, depending on the
                    // exact implementation of ceil(), so a bitwise match might not succeed.
                    // Because of this, we allow either a value match or a bitwise match.
                    let bitwise_match = slots.0[dest] == expected.to_bits();
                    let value_match = slots.float(dest) == expected;
                    reporter_assert!(r, value_match || bitwise_match);
                } else {
                    reporter_assert!(r, slots.float(dest) == input_value);
                }

                dest += 1;
                input_value += 1.0;
            }
        }
    }
});

fn to_mix_weight(value: f32) -> f32 {
    // Convert a positive value to a mix-weight (a number between 0 and 1).
    let value = value / 16.0;
    value - value.floor()
}

/// One of `kMixOps`: the stage appended for `num_slots_affected` slots on a tier with `n` lanes
/// (the n-way stage's `delta` is `5 * N * sizeof(float)`).
struct MixOp {
    num_slots_affected: usize,
    append: fn(n: usize) -> Stage<'static>,
}

// Port of: tests/SkRasterPipelineTest.cpp#L2353-L2418 (chrome/m156)
def_test!(SkRasterPipeline_MixTest, |r| {
    // Allocate space for 5 dest and 10 source slots.
    let mut slots = Slots::new(15);
    let n = highp_stride();

    let k_mix_ops = [
        MixOp {
            num_slots_affected: 1,
            append: |_| Stage::MixFloat(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 2,
            append: |_| Stage::Mix2Floats(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 3,
            append: |_| Stage::Mix3Floats(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 4,
            append: |_| Stage::Mix4Floats(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 5,
            append: |n| {
                Stage::MixNFloats(TernaryOpCtx {
                    dst: 0,
                    delta: u32::try_from(5 * n * size_of::<f32>()).unwrap(),
                })
            },
        },
    ];

    for op in &k_mix_ops {
        // Initialize the values to 1,2,3...
        iota_float(&mut slots, 0, 15 * n, 1.0);

        let mut weight_value = slots.float(0);
        let mut from_value = slots.float(op.num_slots_affected * n);
        let mut to_value = slots.float(2 * op.num_slots_affected * n);

        // The first group of values (the weights) must be between zero and one.
        for idx in 0..op.num_slots_affected * n {
            let weight = to_mix_weight(slots.float(idx));
            slots.set_float(idx, weight);
        }

        // Run the mix op over our data.
        slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.append)(n)]);

        // Verify that the affected slots now equal mix({0.25, 0.3125...}, {3,4...}, {5,6...}, ).
        let mut dest = 0;
        for _check_slot in 0..op.num_slots_affected {
            for _check_lane in 0..n {
                let check_value =
                    (to_value - from_value) * to_mix_weight(weight_value) + from_value;
                reporter_assert!(r, slots.float(dest) == check_value);

                dest += 1;
                from_value += 1.0;
                to_value += 1.0;
                weight_value += 1.0;
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2420-L2485 (chrome/m156)
def_test!(SkRasterPipeline_MixIntTest, |r| {
    // Allocate space for 5 dest and 10 source slots.
    let mut slots = Slots::new(15);
    let n = highp_stride();

    let k_mix_ops = [
        MixOp {
            num_slots_affected: 1,
            append: |_| Stage::MixInt(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 2,
            append: |_| Stage::Mix2Ints(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 3,
            append: |_| Stage::Mix3Ints(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 4,
            append: |_| Stage::Mix4Ints(Slots::ptr()),
        },
        MixOp {
            num_slots_affected: 5,
            append: |n| {
                Stage::MixNInts(TernaryOpCtx {
                    dst: 0,
                    delta: u32::try_from(5 * n * size_of::<i32>()).unwrap(),
                })
            },
        },
    ];

    for op in &k_mix_ops {
        // Initialize the selector ("weight") values to alternating masks
        for idx in 0..op.num_slots_affected * n {
            slots.set_int(idx, if idx & 1 != 0 { !0 } else { 0 });
        }

        // Initialize the other values to various NaNs
        iota_int(
            &mut slots,
            op.num_slots_affected * n,
            15 * n,
            LAST_SIGNALING_NAN,
        );

        let mut weight_value = slots.int(0);
        let mut from_value = slots.int(op.num_slots_affected * n);
        let mut to_value = slots.int(2 * op.num_slots_affected * n);

        // Run the mix op over our data.
        slots.run(&[Stage::SetBasePointer(Slots::ptr()), (op.append)(n)]);

        // Verify that the affected slots now equal either fromValue or toValue, correctly
        let mut dest = 0;
        for _check_slot in 0..op.num_slots_affected {
            for _check_lane in 0..n {
                let check_value = if weight_value != 0 {
                    to_value
                } else {
                    from_value
                };
                reporter_assert!(r, slots.int(dest) == check_value);

                dest += 1;
                from_value += 1;
                to_value += 1;
                weight_value = !weight_value;
            }
        }
    }
});

// ~~~ B6a: SkSL masks, branches and copies ~~~
//
// Mapping notes: `SkOpts::raster_pipeline_highp_stride` is the selected tier's highp stride.
// The C++ `alignas(64) int32_t x[]` arrays are `Ints`, byte buffers bound to a `MemSlot` (the
// pipeline never borrows writable memory); a context pointer into such an array is a `MemPtr`.
// `SkRPCtxUtils::Pack` and the arena disappear: contexts are values, or references that outlive
// the pipeline. `InitLaneMasksCtx` has no counterpart (the tail is interpreter state).

/// The bit pattern of the "largest" signaling NaN. The next integer is a quiet NaN. We use this as
/// the starting point for various memory-shuffling tests below, to ensure that our code doesn't
/// interpret values as float when they might be integral.
const K_LAST_SIGNALING_NAN: i32 = 0x7fbf_ffff;

/// Similarly, this is the "smallest" (in magnitude) negative signaling NaN. The next integer is a
/// quiet negative NaN.
#[allow(clippy::cast_possible_wrap)] // 0xffbfffff, as the C++ `int` constant
const K_LAST_SIGNALING_NEG_NAN: i32 = 0xffbf_ffff_u32 as i32;

/// A C++ `alignas(64) int32_t x[]`: native-endian bytes that can be bound to a pipeline slot.
struct Ints(Vec<u8>);

impl Ints {
    fn new(values: &[i32]) -> Ints {
        Ints(values.iter().flat_map(|v| v.to_ne_bytes()).collect())
    }

    fn from_u32(values: &[u32]) -> Ints {
        Ints(values.iter().flat_map(|v| v.to_ne_bytes()).collect())
    }

    fn zeroed(len: usize) -> Ints {
        Ints::new(&vec![0; len])
    }

    fn get(&self) -> Vec<i32> {
        self.0
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| i32::from_ne_bytes(*c))
            .collect()
    }
}

/// `std::iota(&v[from], &v[to], start)`.
fn iota(v: &mut [i32], from: usize, to: usize, start: i32) {
    let mut value = start;
    for slot in &mut v[from..to] {
        *slot = value;
        value = value.wrapping_add(1);
    }
}

/// A pointer to the start of slot `index`.
fn slot(index: u16) -> MemPtr {
    MemPtr::new(MemSlot(index), 0)
}

/// `MemView`s for the bindings of slots `0..` (read-only first, then writable ones).
fn bind<'m>(read: &'m [&'m Ints], write: &'m mut [&'m mut Ints]) -> MemoryBindings<'m> {
    let mut mem = MemoryBindings::new();
    let mut index = 0;
    for ints in read {
        mem.bind(MemSlot(index), MemView::read(&ints.0));
        index += 1;
    }
    for ints in write {
        mem.bind(MemSlot(index), MemView::write(&mut ints.0));
        index += 1;
    }
    mem
}

// Port of: tests/SkRasterPipelineTest.cpp#L107-L149 (chrome/m156)
def_test!(SkRasterPipeline_LoadStoreConditionMask, |reporter| {
    let mask = Ints::new(&[!0, 0, !0, 0, !0, !0, !0, 0, !0, 0, !0, 0, !0, !0, !0, 0]);
    let mut mask_copy = Ints::zeroed(MAX_STRIDE_HIGHP);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let mask_values = mask.get();
    let n = highp_stride();

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadConditionMask(slot(0)));
    p.append(Stage::StoreConditionMask(slot(1)));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(
        0,
        0,
        n,
        1,
        &mut bind(&[&mask], &mut [&mut mask_copy, &mut src]),
    );
    let (mask_copy, src) = (mask_copy.get(), src.get());

    {
        // `maskCopy` should be populated with `mask` in the frontmost positions
        // (depending on the architecture that SkRasterPipeline is targeting).
        let mut index = 0;
        while index < n {
            reporter_assert!(reporter, mask_copy[index] == mask_values[index]);
            index += 1;
        }

        // The remaining slots should have been left alone.
        while index < mask_copy.len() {
            reporter_assert!(reporter, mask_copy[index] == 0);
            index += 1;
        }
    }
    {
        // `r` and `a` should be populated with `mask`.
        // `g` and `b` should remain initialized to true.
        let r = 0 * n;
        let g = n;
        let b = 2 * n;
        let a = 3 * n;
        for index in 0..n {
            reporter_assert!(reporter, src[r + index] == mask_values[index]);
            reporter_assert!(reporter, src[g + index] == !0);
            reporter_assert!(reporter, src[b + index] == !0);
            reporter_assert!(reporter, src[a + index] == mask_values[index]);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L151-L193 (chrome/m156)
def_test!(SkRasterPipeline_LoadStoreLoopMask, |reporter| {
    let mask = Ints::new(&[!0, 0, !0, 0, !0, !0, !0, 0, !0, 0, !0, 0, !0, !0, !0, 0]);
    let mut mask_copy = Ints::zeroed(MAX_STRIDE_HIGHP);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let mask_values = mask.get();
    let n = highp_stride();

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadLoopMask(slot(0)));
    p.append(Stage::StoreLoopMask(slot(1)));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(
        0,
        0,
        n,
        1,
        &mut bind(&[&mask], &mut [&mut mask_copy, &mut src]),
    );
    let (mask_copy, src) = (mask_copy.get(), src.get());

    {
        // `maskCopy` should be populated with `mask` in the frontmost positions
        // (depending on the architecture that SkRasterPipeline is targeting).
        let mut index = 0;
        while index < n {
            reporter_assert!(reporter, mask_copy[index] == mask_values[index]);
            index += 1;
        }

        // The remaining slots should have been left alone.
        while index < mask_copy.len() {
            reporter_assert!(reporter, mask_copy[index] == 0);
            index += 1;
        }
    }
    {
        // `g` and `a` should be populated with `mask`.
        // `r` and `b` should remain initialized to true.
        let r = 0 * n;
        let g = n;
        let b = 2 * n;
        let a = 3 * n;
        for index in 0..n {
            reporter_assert!(reporter, src[r + index] == !0);
            reporter_assert!(reporter, src[g + index] == mask_values[index]);
            reporter_assert!(reporter, src[b + index] == !0);
            reporter_assert!(reporter, src[a + index] == mask_values[index]);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L195-L237 (chrome/m156)
def_test!(SkRasterPipeline_LoadStoreReturnMask, |reporter| {
    let mask = Ints::new(&[!0, 0, !0, 0, !0, !0, !0, 0, !0, 0, !0, 0, !0, !0, !0, 0]);
    let mut mask_copy = Ints::zeroed(MAX_STRIDE_HIGHP);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let mask_values = mask.get();
    let n = highp_stride();

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::LoadReturnMask(slot(0)));
    p.append(Stage::StoreReturnMask(slot(1)));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(
        0,
        0,
        n,
        1,
        &mut bind(&[&mask], &mut [&mut mask_copy, &mut src]),
    );
    let (mask_copy, src) = (mask_copy.get(), src.get());

    {
        // `maskCopy` should be populated with `mask` in the frontmost positions
        // (depending on the architecture that SkRasterPipeline is targeting).
        let mut index = 0;
        while index < n {
            reporter_assert!(reporter, mask_copy[index] == mask_values[index]);
            index += 1;
        }

        // The remaining slots should have been left alone.
        while index < mask_copy.len() {
            reporter_assert!(reporter, mask_copy[index] == 0);
            index += 1;
        }
    }
    {
        // `b` and `a` should be populated with `mask`.
        // `r` and `g` should remain initialized to true.
        let r = 0 * n;
        let g = n;
        let b = 2 * n;
        let a = 3 * n;
        for index in 0..n {
            reporter_assert!(reporter, src[r + index] == !0);
            reporter_assert!(reporter, src[g + index] == !0);
            reporter_assert!(reporter, src[b + index] == mask_values[index]);
            reporter_assert!(reporter, src[a + index] == mask_values[index]);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L239-L267 (chrome/m156)
def_test!(SkRasterPipeline_MergeConditionMask, |reporter| {
    let mask = Ints::new(&[
        0, 0, !0, !0, 0, !0, 0, !0, !0, !0, !0, !0, 0, 0, 0, 0, 0, 0, !0, !0, 0, !0, 0, !0, !0, !0,
        !0, !0, 0, 0, 0, 0,
    ]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let mask_values = mask.get();
    let n = highp_stride();
    assert_eq!(mask_values.len(), 2 * MAX_STRIDE_HIGHP);

    let mut p = RasterPipeline::new();
    p.append(Stage::InitLaneMasks);
    p.append(Stage::MergeConditionMask(slot(0)));
    p.append(Stage::StoreSrc(slot(1)));
    p.run(0, 0, n, 1, &mut bind(&[&mask], &mut [&mut src]));
    let src = src.get();

    // `r` and `a` should be populated with `mask[x] & mask[y]` in the frontmost positions.
    // `g` and `b` should remain initialized to true.
    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    for index in 0..n {
        let expected = mask_values[index] & mask_values[index + n];
        reporter_assert!(reporter, src[r + index] == expected);
        reporter_assert!(reporter, src[g + index] == !0);
        reporter_assert!(reporter, src[b + index] == !0);
        reporter_assert!(reporter, src[a + index] == expected);
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L269-L303 (chrome/m156)
def_test!(SkRasterPipeline_MergeLoopMask, |reporter| {
    let initial = Ints::new(&[
        !0, !0, !0, !0, !0, 0, !0, !0, // r (condition)
        !0, 0, !0, 0, !0, !0, !0, !0, //
        !0, !0, !0, !0, !0, !0, 0, !0, // g (loop)
        !0, !0, !0, !0, !0, !0, !0, !0, //
        !0, !0, !0, !0, !0, 0, !0, !0, // b (return)
        !0, 0, !0, 0, !0, !0, !0, !0, //
        !0, !0, !0, !0, !0, !0, 0, !0, // a (combined)
        !0, !0, !0, !0, !0, !0, !0, !0,
    ]);
    let mask = Ints::new(&[0, !0, !0, 0, !0, !0, !0, !0, 0, !0, !0, 0, !0, !0, !0, !0]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let (initial_values, mask_values) = (initial.get(), mask.get());
    let n = highp_stride();
    assert_eq!(initial_values.len(), 4 * MAX_STRIDE_HIGHP);

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::MergeLoopMask(slot(1)));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(0, 0, n, 1, &mut bind(&[&initial, &mask], &mut [&mut src]));
    let src = src.get();

    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    for index in 0..n {
        // `g` should contain `g & mask` in each lane.
        reporter_assert!(
            reporter,
            src[g + index] == (initial_values[g + index] & mask_values[index])
        );

        // `r` and `b` should be unchanged.
        reporter_assert!(reporter, src[r + index] == initial_values[r + index]);
        reporter_assert!(reporter, src[b + index] == initial_values[b + index]);

        // `a` should contain `r & g & b`.
        reporter_assert!(
            reporter,
            src[a + index] == (src[r + index] & src[g + index] & src[b + index])
        );
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L305-L339 (chrome/m156)
def_test!(SkRasterPipeline_ReenableLoopMask, |reporter| {
    let initial = Ints::new(&[
        !0, !0, !0, !0, !0, 0, !0, !0, // r (condition)
        !0, 0, !0, 0, !0, !0, 0, !0, //
        0, !0, !0, !0, 0, 0, 0, !0, // g (loop)
        0, 0, !0, 0, 0, 0, 0, !0, //
        !0, !0, !0, !0, !0, 0, !0, !0, // b (return)
        !0, 0, !0, 0, !0, !0, 0, !0, //
        0, !0, !0, !0, 0, 0, 0, !0, // a (combined)
        0, 0, !0, 0, 0, 0, 0, !0,
    ]);
    let mask = Ints::new(&[0, !0, 0, 0, 0, 0, !0, 0, 0, !0, 0, 0, 0, 0, !0, 0]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let (initial_values, mask_values) = (initial.get(), mask.get());
    let n = highp_stride();
    assert_eq!(initial_values.len(), 4 * MAX_STRIDE_HIGHP);

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::ReenableLoopMask(slot(1)));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(0, 0, n, 1, &mut bind(&[&initial, &mask], &mut [&mut src]));
    let src = src.get();

    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    for index in 0..n {
        // `g` should contain `g | mask` in each lane.
        reporter_assert!(
            reporter,
            src[g + index] == (initial_values[g + index] | mask_values[index])
        );

        // `r` and `b` should be unchanged.
        reporter_assert!(reporter, src[r + index] == initial_values[r + index]);
        reporter_assert!(reporter, src[b + index] == initial_values[b + index]);

        // `a` should contain `r & g & b`.
        reporter_assert!(
            reporter,
            src[a + index] == (src[r + index] & src[g + index] & src[b + index])
        );
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L341-L400 (chrome/m156)
def_test!(SkRasterPipeline_CaseOp, |reporter| {
    let initial = Ints::new(&[
        !0, !0, !0, !0, !0, 0, !0, !0, // r (condition)
        0, !0, !0, 0, !0, !0, 0, !0, //
        !0, 0, !0, !0, 0, 0, 0, !0, // g (loop)
        0, 0, !0, 0, 0, 0, 0, !0, //
        !0, !0, !0, !0, !0, 0, !0, !0, // b (return)
        0, !0, !0, 0, !0, !0, 0, !0, //
        !0, 0, !0, !0, 0, 0, 0, !0, // a (combined)
        0, 0, !0, 0, 0, 0, 0, !0,
    ]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let initial_values = initial.get();
    let n = highp_stride();
    assert_eq!(initial_values.len(), 4 * MAX_STRIDE_HIGHP);

    let actual_values: [i32; 16] = [2, 1, 2, 4, 5, 2, 2, 8, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(actual_values.len(), MAX_STRIDE_HIGHP);

    let mut case_op_data = vec![0; 2 * MAX_STRIDE_HIGHP];
    for index in 0..n {
        case_op_data[index] = actual_values[index];
        case_op_data[n + index] = !0;
    }
    let mut case_op_data = Ints::new(&case_op_data);

    let ctx = CaseOpCtx {
        offset: 0,
        expected_value: 2,
    };

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::SetBasePointer(slot(1)));
    p.append(Stage::CaseOp(ctx));
    p.append(Stage::StoreSrc(slot(2)));
    p.run(
        0,
        0,
        n,
        1,
        &mut bind(&[&initial], &mut [&mut case_op_data, &mut src]),
    );
    let (src, case_op_data) = (src.get(), case_op_data.get());

    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    let actual_value_idx = 0 * n;
    let default_mask_idx = n;

    for index in 0..n {
        // `g` should have been set to true for each lane containing 2.
        let mut expected = if actual_values[index] == 2 {
            !0
        } else {
            initial_values[g + index]
        };
        reporter_assert!(reporter, src[g + index] == expected);

        // `r` and `b` should be unchanged.
        reporter_assert!(reporter, src[r + index] == initial_values[r + index]);
        reporter_assert!(reporter, src[b + index] == initial_values[b + index]);

        // `a` should contain `r & g & b`.
        reporter_assert!(
            reporter,
            src[a + index] == (src[r + index] & src[g + index] & src[b + index])
        );

        // The actual-value part of `caseOpData` should be unchanged from the inputs.
        reporter_assert!(
            reporter,
            case_op_data[actual_value_idx + index] == actual_values[index]
        );

        // The default-mask part of `caseOpData` should have been zeroed where the values matched.
        expected = if actual_values[index] == 2 { 0 } else { !0 };
        reporter_assert!(reporter, case_op_data[default_mask_idx + index] == expected);
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L402-L433 (chrome/m156)
def_test!(SkRasterPipeline_MaskOffLoopMask, |reporter| {
    let initial = Ints::new(&[
        !0, !0, !0, !0, !0, 0, !0, !0, // r (condition)
        !0, 0, !0, !0, 0, 0, 0, !0, //
        !0, !0, 0, !0, 0, 0, !0, !0, // g (loop)
        !0, 0, 0, !0, 0, 0, 0, !0, //
        !0, !0, !0, !0, !0, 0, !0, !0, // b (return)
        !0, 0, !0, !0, 0, 0, 0, !0, //
        !0, !0, 0, !0, 0, 0, !0, !0, // a (combined)
        !0, 0, 0, !0, 0, 0, 0, !0,
    ]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let initial_values = initial.get();
    let n = highp_stride();
    assert_eq!(initial_values.len(), 4 * MAX_STRIDE_HIGHP);

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::MaskOffLoopMask);
    p.append(Stage::StoreSrc(slot(1)));
    p.run(0, 0, n, 1, &mut bind(&[&initial], &mut [&mut src]));
    let src = src.get();

    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    for index in 0..n {
        // `g` should have masked off any lanes that are currently executing.
        let mut expected = initial_values[g + index] & !initial_values[a + index];
        reporter_assert!(reporter, src[g + index] == expected);

        // `a` should contain `r & g & b`.
        expected = src[r + index] & src[g + index] & src[b + index];
        reporter_assert!(reporter, src[a + index] == expected);
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L435-L466 (chrome/m156)
def_test!(SkRasterPipeline_MaskOffReturnMask, |reporter| {
    let initial = Ints::new(&[
        !0, !0, !0, !0, !0, 0, !0, !0, // r (condition)
        !0, 0, !0, !0, 0, 0, 0, !0, //
        !0, !0, 0, !0, 0, 0, !0, !0, // g (loop)
        !0, 0, 0, !0, 0, 0, 0, !0, //
        !0, !0, !0, !0, !0, 0, !0, !0, // b (return)
        !0, 0, !0, !0, 0, 0, 0, !0, //
        !0, !0, 0, !0, 0, 0, !0, !0, // a (combined)
        !0, 0, 0, !0, 0, 0, 0, !0,
    ]);
    let mut src = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let initial_values = initial.get();
    let n = highp_stride();
    assert_eq!(initial_values.len(), 4 * MAX_STRIDE_HIGHP);

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::MaskOffReturnMask);
    p.append(Stage::StoreSrc(slot(1)));
    p.run(0, 0, n, 1, &mut bind(&[&initial], &mut [&mut src]));
    let src = src.get();

    let r = 0 * n;
    let g = n;
    let b = 2 * n;
    let a = 3 * n;
    for index in 0..n {
        // `b` should have masked off any lanes that are currently executing.
        let mut expected = initial_values[b + index] & !initial_values[a + index];
        reporter_assert!(reporter, src[b + index] == expected);

        // `a` should contain `r & g & b`.
        expected = src[r + index] & src[g + index] & src[b + index];
        reporter_assert!(reporter, src[a + index] == expected);
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L468-L510 (chrome/m156)
def_test!(SkRasterPipeline_InitLaneMasks, |reporter| {
    let n = highp_stride();
    for width in 1..=n {
        let alloc = ArenaAlloc::new();
        let mut p = RasterPipeline::new();

        // Initialize RGBA to unrelated values.
        const K_ARBITRARY_COLOR: [f32; 4] = [0.0, 0.25, 0.50, 0.75];
        p.append_constant_color(&alloc, &K_ARBITRARY_COLOR);

        // Overwrite RGBA with lane masks up to the tail width.
        p.append(Stage::InitLaneMasks);

        // Use the store_src command to write out RGBA for inspection.
        let mut rgba = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
        p.append(Stage::StoreSrc(slot(0)));

        // Execute our program.
        p.run(0, 0, width, 1, &mut bind(&[], &mut [&mut rgba]));
        let rgba = rgba.get();

        // Initialized data should look like on/on/on/on (RGBA are all set) and is
        // striped by the raster pipeline stride because we wrote it using store_src.
        let mut index = 0;
        let channel_r = 0;
        let channel_g = channel_r + n;
        let channel_b = channel_g + n;
        let channel_a = channel_b + n;
        while index < width {
            reporter_assert!(reporter, rgba[channel_r + index] == !0);
            reporter_assert!(reporter, rgba[channel_g + index] == !0);
            reporter_assert!(reporter, rgba[channel_b + index] == !0);
            reporter_assert!(reporter, rgba[channel_a + index] == !0);
            index += 1;
        }

        // The rest of the output array should be untouched (all zero).
        while index < n {
            reporter_assert!(reporter, rgba[channel_r + index] == 0);
            reporter_assert!(reporter, rgba[channel_g + index] == 0);
            reporter_assert!(reporter, rgba[channel_b + index] == 0);
            reporter_assert!(reporter, rgba[channel_a + index] == 0);
            index += 1;
        }
    }
});

/// The indirect-offset mixes of the indirect copy tests.
const K_INDIRECT_OFFSETS: [[u32; 16]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2],
    [99, 99, 0, 0, 99, 99, 0, 0, 99, 99, 0, 0, 99, 99, 0, 0],
];

/// The indirect-offset mixes of the tests that use `~99u`.
const K_INDIRECT_OFFSETS_NOT_99: [[u32; 16]; 4] = [
    K_INDIRECT_OFFSETS[0],
    K_INDIRECT_OFFSETS[1],
    K_INDIRECT_OFFSETS[2],
    [99, !99, 0, 0, !99, 99, 0, 0, 99, !99, 0, 0, !99, 99, 0, 0],
];

/// The execution masks of the masked indirect copy tests.
const K_COPY_MASKS: [[i32; 16]; 4] = [
    [!0, !0, !0, !0, !0, 0, !0, !0, !0, !0, !0, !0, !0, 0, !0, !0],
    [!0, 0, !0, !0, 0, 0, 0, !0, !0, 0, !0, !0, 0, 0, 0, !0],
    [!0, !0, 0, !0, 0, 0, !0, !0, !0, !0, 0, !0, 0, 0, !0, !0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

// Port of: tests/SkRasterPipelineTest.cpp#L524-L594 (chrome/m156)
def_test!(SkRasterPipeline_CopyFromIndirectUnmasked, |reporter| {
    let n = highp_stride();

    for offsets in &K_INDIRECT_OFFSETS {
        for copy_size in 1..=5_usize {
            // Initialize the destination slots to 0,1,2.. and the source slots to various NaNs
            let mut dst_values = vec![0; 5 * MAX_STRIDE_HIGHP];
            let mut src_values = vec![0; 5 * MAX_STRIDE_HIGHP];
            iota(&mut dst_values, 0, 5 * n, 0);
            iota(&mut src_values, 0, 5 * n, K_LAST_SIGNALING_NAN);
            let src = Ints::new(&src_values);
            let mut dst = Ints::new(&dst_values);
            let offsets_ints = Ints::from_u32(offsets);

            // Run `copy_from_indirect_unmasked` over our data.
            let ctx = CopyIndirectCtx {
                dst: slot(2),
                src: slot(0),
                indirect_offset: slot(1),
                indirect_limit: u32::try_from(5 - copy_size).unwrap(),
                slots: u32::try_from(copy_size).unwrap(),
            };
            let mut p = RasterPipeline::new();
            p.append(Stage::CopyFromIndirectUnmasked(&ctx));
            p.run(
                0,
                0,
                n,
                1,
                &mut bind(&[&src, &offsets_ints], &mut [&mut dst]),
            );
            let dst = dst.get();

            // If the offset plus copy-size would overflow the source data, the results don't
            // matter; indexing off the end of the buffer is UB, and we don't make any promises
            // about the values you get. If we didn't crash, that's success. (In practice, we
            // will have clamped the source pointer so that we don't read past the end.)
            let max_offset = *offsets[..n].iter().max().unwrap();
            if copy_size + max_offset as usize > 5 {
                continue;
            }

            // Verify that the destination has been overwritten in the mask-on fields, and has
            // not been overwritten in the mask-off fields, for each destination slot.
            let mut expected_unchanged = 0;
            let mut expected_from_zero = src_values[0];
            let mut expected_from_two = src_values[2 * n];
            let mut dest_ptr = 0;
            for check_slot in 0..5 {
                for check_lane in 0..n {
                    if check_slot < copy_size {
                        if offsets[check_lane] == 0 {
                            reporter_assert!(reporter, dst[dest_ptr] == expected_from_zero);
                        } else if offsets[check_lane] == 2 {
                            reporter_assert!(reporter, dst[dest_ptr] == expected_from_two);
                        } else {
                            errorf!(reporter, "unexpected offset value");
                        }
                    } else {
                        reporter_assert!(reporter, dst[dest_ptr] == expected_unchanged);
                    }

                    dest_ptr += 1;
                    expected_unchanged += 1;
                    expected_from_zero += 1;
                    expected_from_two += 1;
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L596-L666 (chrome/m156)
def_test!(
    SkRasterPipeline_CopyFromIndirectUniformUnmasked,
    |reporter| {
        let n = highp_stride();

        for offsets in &K_INDIRECT_OFFSETS_NOT_99 {
            for copy_size in 1..=5_usize {
                // Initialize the destination slots to 0,1,2.. and the source uniforms to various NaNs
                let mut dst_values = vec![0; 5 * MAX_STRIDE_HIGHP];
                iota(&mut dst_values, 0, 5 * n, 0);
                let mut src = [0; 5];
                iota(&mut src, 0, 5, K_LAST_SIGNALING_NAN);
                let mut dst = Ints::new(&dst_values);
                let offsets_ints = Ints::from_u32(offsets);

                // Run `copy_from_indirect_unmasked` over our data.
                let ctx = CopyIndirectUniformCtx {
                    dst: slot(1),
                    src: &src,
                    indirect_offset: slot(0),
                    indirect_limit: u32::try_from(5 - copy_size).unwrap(),
                    slots: u32::try_from(copy_size).unwrap(),
                };
                let mut p = RasterPipeline::new();
                p.append(Stage::CopyFromIndirectUniformUnmasked(&ctx));
                p.run(0, 0, n, 1, &mut bind(&[&offsets_ints], &mut [&mut dst]));
                let dst = dst.get();

                // If the offset plus copy-size would overflow the source data, the results don't
                // matter; indexing off the end of the buffer is UB, and we don't make any promises
                // about the values you get. If we didn't crash, that's success. (In practice, we
                // will have clamped the source pointer so that we don't read past the end.)
                let max_offset = *offsets[..n].iter().max().unwrap();
                if copy_size + max_offset as usize > 5 {
                    continue;
                }

                // Verify that the destination has been overwritten in each slot.
                let mut expected_unchanged = 0;
                let mut expected_from_zero = src[0];
                let mut expected_from_two = src[2];
                let mut dest_ptr = 0;
                for check_slot in 0..5 {
                    for check_lane in 0..n {
                        if check_slot < copy_size {
                            if offsets[check_lane] == 0 {
                                reporter_assert!(reporter, dst[dest_ptr] == expected_from_zero);
                            } else if offsets[check_lane] == 2 {
                                reporter_assert!(reporter, dst[dest_ptr] == expected_from_two);
                            } else {
                                errorf!(reporter, "unexpected offset value");
                            }
                        } else {
                            reporter_assert!(reporter, dst[dest_ptr] == expected_unchanged);
                        }

                        dest_ptr += 1;
                        expected_unchanged += 1;
                    }
                    expected_from_zero += 1;
                    expected_from_two += 1;
                }
            }
        }
    }
);

// Port of: tests/SkRasterPipelineTest.cpp#L668-L757 (chrome/m156)
def_test!(SkRasterPipeline_CopyToIndirectMasked, |reporter| {
    let n = highp_stride();

    for mask in &K_COPY_MASKS {
        for offsets in &K_INDIRECT_OFFSETS_NOT_99 {
            for copy_size in 1..=5_usize {
                // Initialize the destination slots to 0,1,2.. and the source slots to various
                // NaNs
                let mut dst_values = vec![0; 5 * MAX_STRIDE_HIGHP];
                let mut src_values = vec![0; 5 * MAX_STRIDE_HIGHP];
                iota(&mut dst_values, 0, 5 * n, 0);
                iota(&mut src_values, 0, 5 * n, K_LAST_SIGNALING_NAN);
                let src = Ints::new(&src_values);
                let mut dst = Ints::new(&dst_values);
                let offsets_ints = Ints::from_u32(offsets);
                let mask_ints = Ints::new(mask);

                // Run `copy_to_indirect_masked` over our data.
                let ctx = CopyIndirectCtx {
                    dst: slot(3),
                    src: slot(0),
                    indirect_offset: slot(1),
                    indirect_limit: u32::try_from(5 - copy_size).unwrap(),
                    slots: u32::try_from(copy_size).unwrap(),
                };
                let mut p = RasterPipeline::new();
                p.append(Stage::InitLaneMasks);
                p.append(Stage::LoadConditionMask(slot(2)));
                p.append(Stage::CopyToIndirectMasked(&ctx));
                p.run(
                    0,
                    0,
                    n,
                    1,
                    &mut bind(&[&src, &offsets_ints, &mask_ints], &mut [&mut dst]),
                );
                let dst = dst.get();

                // If the offset plus copy-size would overflow the destination, the results don't
                // matter; indexing off the end of the buffer is UB, and we don't make any
                // promises about the values you get. If we didn't crash, that's success. (In
                // practice, we will have clamped the destination pointer so that we don't read
                // past the end.)
                let max_offset = *offsets[..n].iter().max().unwrap();
                if copy_size + max_offset as usize > 5 {
                    continue;
                }

                // Verify that the destination has been overwritten in the mask-on fields, and
                // has not been overwritten in the mask-off fields, for each destination slot.
                let mut expected_unchanged = 0;
                let mut expected_from_zero = src_values[0];
                let mut expected_from_two = src_values[0] - (2 * n) as i32;
                let mut dest_ptr = 0;
                let mut pos = 0;
                for _check_slot in 0..5 {
                    for check_lane in 0..n {
                        let range_start = offsets[check_lane] as usize * n;
                        let range_end = (offsets[check_lane] as usize + copy_size) * n;
                        if mask[check_lane] != 0 && pos >= range_start && pos < range_end {
                            if offsets[check_lane] == 0 {
                                reporter_assert!(reporter, dst[dest_ptr] == expected_from_zero);
                            } else if offsets[check_lane] == 2 {
                                reporter_assert!(reporter, dst[dest_ptr] == expected_from_two);
                            } else {
                                errorf!(reporter, "unexpected offset value");
                            }
                        } else {
                            reporter_assert!(reporter, dst[dest_ptr] == expected_unchanged);
                        }

                        pos += 1;
                        dest_ptr += 1;
                        expected_unchanged += 1;
                        expected_from_zero += 1;
                        expected_from_two += 1;
                    }
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L759-L904 (chrome/m156)
def_test!(SkRasterPipeline_SwizzleCopyToIndirectMasked, |reporter| {
    // Test with various swizzle permutations.
    struct TestPattern {
        swizzle_size: usize,
        swizzle_upper_bound: usize,
        swizzle: [u16; 4],
    }

    const K_PATTERNS: [TestPattern; 4] = [
        TestPattern {
            swizzle_size: 1,
            swizzle_upper_bound: 4,
            swizzle: [3, 0, 0, 0],
        }, // v.w    = (1)
        TestPattern {
            swizzle_size: 2,
            swizzle_upper_bound: 2,
            swizzle: [1, 0, 0, 0],
        }, // v.yx   = (1,2)
        TestPattern {
            swizzle_size: 3,
            swizzle_upper_bound: 3,
            swizzle: [2, 1, 0, 0],
        }, // v.zyx  = (1,2,3)
        TestPattern {
            swizzle_size: 4,
            swizzle_upper_bound: 4,
            swizzle: [3, 0, 1, 2],
        }, // v.wxyz = (1,2,3,4)
    ];

    #[derive(Clone, Copy)]
    enum Expect {
        OutOfBounds,
        Unchanged,
        S0,
        S1,
        S2,
        S3,
        #[allow(dead_code)] // as in the C++ enum
        S4,
    }
    use Expect::{OutOfBounds as XX, S0, S1, S2, S3, Unchanged as __};

    const K_EXPECTATIONS_AT_ZERO: [[Expect; 5]; 4] = [
        //  d[0].w = 1        d[0].yx = (1,2)   d[0].zyx = (1,2,3) d[0].wxyz = (1,2,3,4)
        [__, __, __, S0, __],
        [S1, S0, __, __, __],
        [S2, S1, S0, __, __],
        [S1, S2, S3, S0, __],
    ];
    const K_EXPECTATIONS_AT_TWO: [[Expect; 5]; 4] = [
        //  d[2].w = 1        d[2].yx = (1,2)   d[2].zyx = (1,2,3) d[2].wxyz = (1,2,3,4)
        [XX, XX, XX, XX, XX],
        [__, __, S1, S0, __],
        [__, __, S2, S1, S0],
        [XX, XX, XX, XX, XX],
    ];

    let n = highp_stride();

    for mask in &K_COPY_MASKS {
        for offsets in &K_INDIRECT_OFFSETS_NOT_99 {
            for (pattern_index, pattern) in K_PATTERNS.iter().enumerate() {
                // Initialize the destination slots to 0,1,2.. and the source slots to various
                // NaNs
                let mut dst_values = vec![0; 5 * MAX_STRIDE_HIGHP];
                let mut src_values = vec![0; 5 * MAX_STRIDE_HIGHP];
                iota(&mut dst_values, 0, 5 * n, 0);
                iota(&mut src_values, 0, 5 * n, K_LAST_SIGNALING_NAN);
                let src = Ints::new(&src_values);
                let mut dst = Ints::new(&dst_values);
                let offsets_ints = Ints::from_u32(offsets);
                let mask_ints = Ints::new(mask);

                // Run `swizzle_copy_to_indirect_masked` over our data.
                let stride_bytes = u16::try_from(n * 4).unwrap();
                let ctx = SwizzleCopyIndirectCtx {
                    copy: CopyIndirectCtx {
                        dst: slot(3),
                        src: slot(0),
                        indirect_offset: slot(1),
                        indirect_limit: u32::try_from(5 - pattern.swizzle_upper_bound).unwrap(),
                        slots: u32::try_from(pattern.swizzle_size).unwrap(),
                    },
                    offsets: pattern.swizzle.map(|s| s * stride_bytes),
                };
                let mut p = RasterPipeline::new();
                p.append(Stage::InitLaneMasks);
                p.append(Stage::LoadConditionMask(slot(2)));
                p.append(Stage::SwizzleCopyToIndirectMasked(&ctx));
                p.run(
                    0,
                    0,
                    n,
                    1,
                    &mut bind(&[&src, &offsets_ints, &mask_ints], &mut [&mut dst]),
                );
                let dst = dst.get();

                // If the offset plus copy-size would overflow the destination, the results don't
                // matter; indexing off the end of the buffer is UB, and we don't make any
                // promises about the values you get. If we didn't crash, that's success. (In
                // practice, we will have clamped the destination pointer so that we don't read
                // past the end.)
                let max_offset = *offsets[..n].iter().max().unwrap();
                if pattern.swizzle_upper_bound + max_offset as usize > 5 {
                    continue;
                }

                // Verify that the destination has been overwritten in the mask-on fields, and
                // has not been overwritten in the mask-off fields, for each destination slot.
                let mut expected_unchanged = 0;
                let mut dest_ptr = 0;
                for check_slot in 0..5 {
                    for check_lane in 0..n {
                        let mut expected_type = __;
                        if offsets[check_lane] == 0 {
                            expected_type = K_EXPECTATIONS_AT_ZERO[pattern_index][check_slot];
                        } else if offsets[check_lane] == 2 {
                            expected_type = K_EXPECTATIONS_AT_TWO[pattern_index][check_slot];
                        }
                        if mask[check_lane] == 0 {
                            expected_type = __;
                        }
                        match expected_type {
                            Expect::OutOfBounds => {} // out of bounds; ignore result
                            Expect::Unchanged => {
                                reporter_assert!(reporter, dst[dest_ptr] == expected_unchanged);
                            }
                            Expect::S0 => {
                                // destination should match source 0
                                reporter_assert!(reporter, dst[dest_ptr] == src_values[check_lane]);
                            }
                            Expect::S1 => {
                                // destination should match source 1
                                reporter_assert!(
                                    reporter,
                                    dst[dest_ptr] == src_values[n + check_lane]
                                );
                            }
                            Expect::S2 => {
                                // destination should match source 2
                                reporter_assert!(
                                    reporter,
                                    dst[dest_ptr] == src_values[2 * n + check_lane]
                                );
                            }
                            Expect::S3 => {
                                // destination should match source 3
                                reporter_assert!(
                                    reporter,
                                    dst[dest_ptr] == src_values[3 * n + check_lane]
                                );
                            }
                            Expect::S4 => {
                                // destination should match source 4
                                reporter_assert!(
                                    reporter,
                                    dst[dest_ptr] == src_values[4 * n + check_lane]
                                );
                            }
                        }

                        dest_ptr += 1;
                        expected_unchanged += 1;
                    }
                }
            }
        }
    }
});

/// The execution masks of `SkRasterPipeline_CopySlotsMasked`.
const K_SLOT_MASKS: [[i32; 16]; 4] = [
    [
        !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0,
    ],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [!0, 0, !0, !0, !0, !0, 0, !0, !0, 0, !0, !0, !0, !0, 0, !0],
    [0, !0, 0, 0, 0, !0, !0, 0, 0, !0, 0, 0, 0, !0, !0, 0],
];

// Port of: tests/SkRasterPipelineTest.cpp#L1140-L1208 (chrome/m156)
def_test!(SkRasterPipeline_CopySlotsMasked, |reporter| {
    let src_index = 0;
    let dst_index = 5;

    struct CopySlotsOp {
        stage: fn(BinaryOpCtx) -> Stage<'static>,
        num_slots_affected: usize,
    }

    let k_copy_ops = [
        CopySlotsOp {
            stage: Stage::CopySlotMasked,
            num_slots_affected: 1,
        },
        CopySlotsOp {
            stage: Stage::Copy2SlotsMasked,
            num_slots_affected: 2,
        },
        CopySlotsOp {
            stage: Stage::Copy3SlotsMasked,
            num_slots_affected: 3,
        },
        CopySlotsOp {
            stage: Stage::Copy4SlotsMasked,
            num_slots_affected: 4,
        },
    ];

    let n = highp_stride();

    for op in &k_copy_ops {
        for mask in &K_SLOT_MASKS {
            // Initialize the destination slots to 0,1,2.. and the source slots to various NaNs
            let mut slot_values = vec![0; 10 * MAX_STRIDE_HIGHP];
            iota(&mut slot_values, n * dst_index, n * (dst_index + 5), 0);
            iota(
                &mut slot_values,
                n * src_index,
                n * (src_index + 5),
                K_LAST_SIGNALING_NAN,
            );
            let mut slots = Ints::new(&slot_values);
            let mask_ints = Ints::new(mask);

            // Run `copy_slots_masked` over our data.
            let ctx = BinaryOpCtx {
                dst: u32::try_from(n * dst_index * 4).unwrap(),
                src: u32::try_from(n * src_index * 4).unwrap(),
            };

            let mut p = RasterPipeline::new();
            p.append(Stage::InitLaneMasks);
            p.append(Stage::SetBasePointer(slot(1)));
            p.append(Stage::LoadConditionMask(slot(0)));
            p.append((op.stage)(ctx));
            p.run(0, 0, n, 1, &mut bind(&[&mask_ints], &mut [&mut slots]));
            let slots = slots.get();

            // Verify that the destination has been overwritten in the mask-on fields, and has
            // not been overwritten in the mask-off fields, for each destination slot.
            let mut expected_unchanged = 0;
            let mut expected_changed = K_LAST_SIGNALING_NAN;
            let mut dest_ptr = n * dst_index;
            for check_slot in 0..5 {
                for check_mask in 0..n {
                    if check_slot < op.num_slots_affected && mask[check_mask] != 0 {
                        reporter_assert!(reporter, slots[dest_ptr] == expected_changed);
                    } else {
                        reporter_assert!(reporter, slots[dest_ptr] == expected_unchanged);
                    }

                    dest_ptr += 1;
                    expected_unchanged += 1;
                    expected_changed += 1;
                }
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1210-L1260 (chrome/m156)
def_test!(SkRasterPipeline_CopySlotsUnmasked, |reporter| {
    let src_index = 0;
    let dst_index = 5;
    let n = highp_stride();

    struct CopySlotsOp {
        stage: fn(BinaryOpCtx) -> Stage<'static>,
        num_slots_affected: usize,
    }

    let k_copy_ops = [
        CopySlotsOp {
            stage: Stage::CopySlotUnmasked,
            num_slots_affected: 1,
        },
        CopySlotsOp {
            stage: Stage::Copy2SlotsUnmasked,
            num_slots_affected: 2,
        },
        CopySlotsOp {
            stage: Stage::Copy3SlotsUnmasked,
            num_slots_affected: 3,
        },
        CopySlotsOp {
            stage: Stage::Copy4SlotsUnmasked,
            num_slots_affected: 4,
        },
    ];

    for op in &k_copy_ops {
        // Initialize the destination slots to 0,1,2.. and the source slots to various NaNs
        let mut slot_values = vec![0; 10 * MAX_STRIDE_HIGHP];
        iota(&mut slot_values, n * dst_index, n * (dst_index + 5), 0);
        iota(
            &mut slot_values,
            n * src_index,
            n * (src_index + 5),
            K_LAST_SIGNALING_NAN,
        );
        let mut slots = Ints::new(&slot_values);

        // Run `copy_slots_unmasked` over our data.
        let ctx = BinaryOpCtx {
            dst: u32::try_from(n * dst_index * 4).unwrap(),
            src: u32::try_from(n * src_index * 4).unwrap(),
        };
        let mut p = RasterPipeline::new();
        p.append(Stage::SetBasePointer(slot(0)));
        p.append((op.stage)(ctx));
        p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
        let slots = slots.get();

        // Verify that the destination has been overwritten in each slot.
        let mut expected_unchanged = 0;
        let mut expected_changed = K_LAST_SIGNALING_NAN;
        let mut dest_ptr = n * dst_index;
        for check_slot in 0..5 {
            for _check_lane in 0..n {
                if check_slot < op.num_slots_affected {
                    reporter_assert!(reporter, slots[dest_ptr] == expected_changed);
                } else {
                    reporter_assert!(reporter, slots[dest_ptr] == expected_unchanged);
                }

                dest_ptr += 1;
                expected_unchanged += 1;
                expected_changed += 1;
            }
        }
    }
});

/// The `copy_[n_]uniform[s]` stage with `count` uniforms.
fn copy_uniforms_stage<'a>(count: usize, ctx: &'a UniformCtx<'a>) -> Stage<'a> {
    match count {
        1 => Stage::CopyUniform(ctx),
        2 => Stage::Copy2Uniforms(ctx),
        3 => Stage::Copy3Uniforms(ctx),
        4 => Stage::Copy4Uniforms(ctx),
        _ => unreachable!(),
    }
}

// Port of: tests/SkRasterPipelineTest.cpp#L1262-L1313 (chrome/m156)
def_test!(SkRasterPipeline_CopyUniforms, |reporter| {
    let n = highp_stride();

    for num_slots_affected in 1..=4_usize {
        // Initialize the destination slots to 1,2,3...
        let mut slot_values = vec![0; 5 * MAX_STRIDE_HIGHP];
        iota(&mut slot_values, 0, 5 * n, 1);
        let mut slots = Ints::new(&slot_values);
        // Initialize the uniform buffer to various NaNs
        let mut uniforms = [0; 5];
        iota(&mut uniforms, 0, 5, K_LAST_SIGNALING_NAN);

        // Run `copy_n_uniforms` over our data.
        let ctx = UniformCtx {
            dst: slot(0),
            src: &uniforms,
        };
        let mut p = RasterPipeline::new();
        p.append(copy_uniforms_stage(num_slots_affected, &ctx));
        p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
        let slots = slots.get();

        // Verify that our uniforms have been broadcast into each slot.
        let mut expected_unchanged = 1;
        let mut expected_changed = K_LAST_SIGNALING_NAN;
        let mut dest_ptr = 0;
        for check_slot in 0..5 {
            for _check_lane in 0..n {
                if check_slot < num_slots_affected {
                    reporter_assert!(reporter, slots[dest_ptr] == expected_changed);
                } else {
                    reporter_assert!(reporter, slots[dest_ptr] == expected_unchanged);
                }

                dest_ptr += 1;
                expected_unchanged += 1;
            }
            expected_changed += 1;
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1315-L1350 (chrome/m156)
def_test!(SkRasterPipeline_CopyConstant, |reporter| {
    let n = highp_stride();

    for index in 0..5_usize {
        // Initialize the destination slots to 1,2,3...
        let mut slot_values = vec![0; 5 * MAX_STRIDE_HIGHP];
        iota(&mut slot_values, 0, 5 * n, 1);
        let mut slots = Ints::new(&slot_values);

        // Overwrite one destination slot with a constant (some NaN based on slot number).
        let ctx = ConstantCtx {
            dst: u32::try_from(n * index * 4).unwrap(),
            value: K_LAST_SIGNALING_NAN + i32::try_from(index).unwrap(),
        };
        let mut p = RasterPipeline::new();
        p.append(Stage::SetBasePointer(slot(0)));
        p.append(Stage::CopyConstant(ctx));
        p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
        let slots = slots.get();

        // Verify that our constant value has been broadcast into exactly one slot.
        let mut expected_unchanged = 1;
        let mut dest_ptr = 0;
        for check_slot in 0..5 {
            for _check_lane in 0..n {
                if check_slot == index {
                    reporter_assert!(reporter, slots[dest_ptr] == ctx.value);
                } else {
                    reporter_assert!(reporter, slots[dest_ptr] == expected_unchanged);
                }

                dest_ptr += 1;
                expected_unchanged += 1;
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1352-L1398 (chrome/m156)
def_test!(SkRasterPipeline_Swizzle, |reporter| {
    let n = highp_stride();

    struct TestPattern {
        stage: fn(SwizzleCtx) -> Stage<'static>,
        swizzle: [u8; 4],
        expectation: [u8; 4],
    }
    let k_patterns = [
        TestPattern {
            stage: Stage::Swizzle1,
            swizzle: [3, 0, 0, 0],
            expectation: [3, 1, 2, 3],
        }, // (1,2,3,4).w    = (4)
        TestPattern {
            stage: Stage::Swizzle2,
            swizzle: [1, 0, 0, 0],
            expectation: [1, 0, 2, 3],
        }, // (1,2,3,4).yx   = (2,1)
        TestPattern {
            stage: Stage::Swizzle3,
            swizzle: [2, 2, 2, 0],
            expectation: [2, 2, 2, 3],
        }, // (1,2,3,4).zzz  = (3,3,3)
        TestPattern {
            stage: Stage::Swizzle4,
            swizzle: [0, 0, 1, 2],
            expectation: [0, 0, 1, 2],
        }, // (1,2,3,4).xxyz = (1,1,2,3)
    ];

    for pattern in &k_patterns {
        // Initialize the destination slots to various NaNs
        let mut slot_values = vec![0; 4 * MAX_STRIDE_HIGHP];
        iota(&mut slot_values, 0, 4 * n, K_LAST_SIGNALING_NAN);
        let mut slots = Ints::new(&slot_values);

        // Apply the test-pattern swizzle.
        let mut ctx = SwizzleCtx {
            dst: 0,
            offsets: [0; 4],
        };
        for index in 0..ctx.offsets.len() {
            ctx.offsets[index] = pattern.swizzle[index] * u8::try_from(n * 4).unwrap();
        }
        let mut p = RasterPipeline::new();
        p.append(Stage::SetBasePointer(slot(0)));
        p.append((pattern.stage)(ctx));
        p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
        let slots = slots.get();

        // Verify that the swizzle has been applied in each slot.
        let mut dest_ptr = 0;
        for check_slot in 0..4 {
            let mut expected = i32::from(pattern.expectation[check_slot])
                * i32::try_from(n).unwrap()
                + K_LAST_SIGNALING_NAN;
            for _check_lane in 0..n {
                reporter_assert!(reporter, slots[dest_ptr] == expected);

                dest_ptr += 1;
                expected += 1;
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1400-L1456 (chrome/m156)
def_test!(SkRasterPipeline_SwizzleCopy, |reporter| {
    let n = highp_stride();

    struct TestPattern {
        slots: usize,
        swizzle: [u16; 4],
        expectation: [u16; 4],
    }
    const NA: u16 = !0;
    let k_patterns = [
        TestPattern {
            slots: 1,
            swizzle: [3, NA, NA, NA],
            expectation: [NA, NA, NA, 0],
        }, // v.w    = (1)
        TestPattern {
            slots: 2,
            swizzle: [1, 0, NA, NA],
            expectation: [1, 0, NA, NA],
        }, // v.yx   = (1,2)
        TestPattern {
            slots: 3,
            swizzle: [2, 3, 0, NA],
            expectation: [2, NA, 0, 1],
        }, // v.zwy  = (1,2,3)
        TestPattern {
            slots: 4,
            swizzle: [3, 0, 1, 2],
            expectation: [1, 2, 3, 0],
        }, // v.wxyz = (1,2,3,4)
    ];

    for pattern in &k_patterns {
        // Allocate space for 4 dest slots, and initialize them to zero.
        let mut dest = Ints::zeroed(4 * MAX_STRIDE_HIGHP);

        // Allocate 4 source slots and initialize them to various NaNs
        let mut source_values = vec![0; 4 * MAX_STRIDE_HIGHP];
        iota(&mut source_values, 0, 4 * n, K_LAST_SIGNALING_NAN);
        let source = Ints::new(&source_values);

        // Apply the dest-swizzle pattern.
        let mut ctx = SwizzleCopyCtx {
            src: slot(0),
            dst: slot(1),
            offsets: [0; 4],
        };
        for index in 0..ctx.offsets.len() {
            if pattern.swizzle[index] != NA {
                ctx.offsets[index] = pattern.swizzle[index] * u16::try_from(n * 4).unwrap();
            }
        }
        let mut p = RasterPipeline::new();
        p.append(Stage::InitLaneMasks);
        p.append(swizzle_copy_stage(pattern.slots, &ctx));
        p.run(0, 0, n, 1, &mut bind(&[&source], &mut [&mut dest]));
        let dest = dest.get();

        // Verify that the swizzle has been applied in each slot.
        let mut dest_ptr = 0;
        for check_slot in 0..4 {
            for check_lane in 0..n {
                if pattern.expectation[check_slot] == NA {
                    reporter_assert!(reporter, dest[dest_ptr] == 0);
                } else {
                    let expected_idx =
                        usize::from(pattern.expectation[check_slot]) * n + check_lane;
                    reporter_assert!(reporter, dest[dest_ptr] == source_values[expected_idx]);
                }

                dest_ptr += 1;
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L1458-L1514 (chrome/m156)
def_test!(SkRasterPipeline_Shuffle, |reporter| {
    let n = highp_stride();

    struct TestPattern {
        count: i32,
        shuffle: [u16; 16],
        expectation: [u16; 16],
    }
    let k_patterns = [
        TestPattern {
            count: 9,
            shuffle: [
                0, 3, 6, 1, 4, 7, 2, 5, 8, /* past end: */ 0, 0, 0, 0, 0, 0, 0,
            ],
            expectation: [
                0, 3, 6, 1, 4, 7, 2, 5, 8, /* unchanged: */ 9, 10, 11, 12, 13, 14, 15,
            ],
        },
        TestPattern {
            count: 16,
            shuffle: [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15],
            expectation: [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15],
        },
    ];

    for pattern in &k_patterns {
        // Initialize the destination slots to various NaNs
        let mut slot_values = vec![0; 16 * MAX_STRIDE_HIGHP];
        iota(&mut slot_values, 0, 16 * n, K_LAST_SIGNALING_NAN);
        let mut slots = Ints::new(&slot_values);

        // Apply the shuffle.
        let mut ctx = ShuffleCtx {
            ptr: slot(0),
            count: pattern.count,
            offsets: [0; 16],
        };
        for index in 0..ctx.offsets.len() {
            ctx.offsets[index] = pattern.shuffle[index] * u16::try_from(n * 4).unwrap();
        }
        let mut p = RasterPipeline::new();
        p.append(Stage::Shuffle(&ctx));
        p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
        let slots = slots.get();

        // Verify that the shuffle has been applied in each slot.
        let mut dest_ptr = 0;
        for check_slot in 0..16 {
            let mut expected = i32::from(pattern.expectation[check_slot])
                * i32::try_from(n).unwrap()
                + K_LAST_SIGNALING_NAN;
            for _check_lane in 0..n {
                reporter_assert!(reporter, slots[dest_ptr] == expected);

                dest_ptr += 1;
                expected += 1;
            }
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2487-L2513 (chrome/m156)
def_test!(SkRasterPipeline_Jump, |reporter| {
    // Allocate space for 4 slots.
    let mut slots = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
    let n = highp_stride();

    const K_COLOR_DARK_RED: [f32; 4] = [0.5, 0.0, 0.0, 0.75];
    const K_COLOR_GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    let ctx = BranchCtx { offset: 2 };

    // Make a program which jumps over an appendConstantColor op.
    let alloc = ArenaAlloc::new();
    let mut p = RasterPipeline::new();
    p.append_constant_color(&alloc, &K_COLOR_GREEN); // assign green
    p.append(Stage::Jump(ctx)); // jump over the dark-red color assignment
    p.append_constant_color(&alloc, &K_COLOR_DARK_RED); // (not executed)
    p.append(Stage::StoreSrc(slot(0))); // store the result so we can check it
    p.run(0, 0, 1, 1, &mut bind(&[], &mut [&mut slots]));
    let slots = slots.get();

    // Verify that the slots contain green.
    let mut dest_ptr = 0;
    for check_slot in 0..4 {
        for _check_lane in 0..n {
            reporter_assert!(
                reporter,
                slots[dest_ptr] == K_COLOR_GREEN[check_slot].to_bits() as i32
            );
            dest_ptr += 1;
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2515-L2543 (chrome/m156)
def_test!(SkRasterPipeline_ExchangeSrc, |reporter| {
    let n = highp_stride();

    let mut register_values = vec![0; 4 * MAX_STRIDE_HIGHP];
    let mut exchange_values = vec![0; 4 * MAX_STRIDE_HIGHP];
    iota(&mut register_values, 0, 4 * n, K_LAST_SIGNALING_NAN);
    iota(&mut exchange_values, 0, 4 * n, K_LAST_SIGNALING_NEG_NAN);
    let mut register_value = Ints::new(&register_values);
    let mut exchange_value = Ints::new(&exchange_values);

    // This program should swap the contents of `registerValue` and `exchangeValue`.
    let mut p = RasterPipeline::new();
    p.append(Stage::LoadSrc(slot(0)));
    p.append(Stage::ExchangeSrc(slot(1)));
    p.append(Stage::StoreSrc(slot(0)));
    p.run(
        0,
        0,
        n,
        1,
        &mut bind(&[], &mut [&mut register_value, &mut exchange_value]),
    );
    let (register_value, exchange_value) = (register_value.get(), exchange_value.get());

    let mut register_ptr = 0;
    let mut exchange_ptr = 0;
    let mut expected_register = K_LAST_SIGNALING_NEG_NAN;
    let mut expected_exchange = K_LAST_SIGNALING_NAN;
    for _check_slot in 0..4 {
        for _check_lane in 0..n {
            reporter_assert!(reporter, register_value[register_ptr] == expected_register);
            register_ptr += 1;
            reporter_assert!(reporter, exchange_value[exchange_ptr] == expected_exchange);
            exchange_ptr += 1;
            expected_register += 1;
            expected_exchange += 1;
        }
    }
});

/// `first`/`second` start as `0x12345678`; the branch program stores `a` into both.
const K_MARKER: i32 = 0x1234_5678;

/// Runs `init; branch; store_src_a first; store_src_a second` over `n` lanes (the `a` register
/// comes from `init`, a program prefix) and returns `(first, second)` (`N` lanes each).
fn run_branch(
    init: &[Stage<'_>],
    branch: Stage<'_>,
    extra: &[&Ints],
    n: usize,
) -> (Vec<i32>, Vec<i32>) {
    let mut first = Ints::new(&[K_MARKER; MAX_STRIDE_HIGHP]);
    let mut second = Ints::new(&[K_MARKER; MAX_STRIDE_HIGHP]);
    let mut p = RasterPipeline::new();
    for stage in init {
        p.append(*stage);
    }
    p.append(branch);
    // `first` and `second` are bound after the read-only `extra` inputs.
    let first_slot = u16::try_from(extra.len()).unwrap();
    p.append(Stage::StoreSrcA(slot(first_slot)));
    p.append(Stage::StoreSrcA(slot(first_slot + 1)));
    p.run(0, 0, n, 1, &mut bind(extra, &mut [&mut first, &mut second]));
    (first.get(), second.get())
}

/// A color for `load_src` with `a` in lane `n - 1` set to `a_last`, all other channels `~0`
/// when `fill` is true (all zero otherwise).
fn regs(n: usize, fill: bool, a_last: i32) -> Ints {
    let mut values = vec![0; 4 * MAX_STRIDE_HIGHP];
    if fill {
        values[..4 * n].fill(!0);
    }
    values[4 * n - 1] = a_last;
    Ints::new(&values)
}

// Port of: tests/SkRasterPipelineTest.cpp#L2545-L2625 (chrome/m156)
def_test!(SkRasterPipeline_BranchIfAllLanesActive, |reporter| {
    let n = highp_stride();

    let ctx = BranchCtx { offset: 2 };

    // The branch should be taken when lane masks are all-on.
    {
        let (first, second) = run_branch(
            &[Stage::InitLaneMasks],
            Stage::BranchIfAllLanesActive(ctx),
            &[],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should not be taken when lane masks are all-off.
    {
        let no_lanes_active = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfAllLanesActive(ctx),
            &[&no_lanes_active],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should not be taken when lane masks are partially-on.
    if n > 1 {
        // An array of ~0s, except for a single zero in the last A slot.
        let one_lane_inactive = regs(n, true, 0);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfAllLanesActive(ctx),
            &[&one_lane_inactive],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2627-L2706 (chrome/m156)
def_test!(SkRasterPipeline_BranchIfAnyLanesActive, |reporter| {
    let n = highp_stride();

    let ctx = BranchCtx { offset: 2 };

    // The branch should be taken when lane masks are all-on.
    {
        let (first, second) = run_branch(
            &[Stage::InitLaneMasks],
            Stage::BranchIfAnyLanesActive(ctx),
            &[],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should not be taken when lane masks are all-off.
    {
        let no_lanes_active = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfAnyLanesActive(ctx),
            &[&no_lanes_active],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should be taken when lane masks are partially-on.
    if n > 1 {
        // An array of all zeros, except for a single ~0 in the last A slot.
        let one_lane_active = regs(n, false, !0);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfAnyLanesActive(ctx),
            &[&one_lane_active],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2708-L2787 (chrome/m156)
def_test!(SkRasterPipeline_BranchIfNoLanesActive, |reporter| {
    let n = highp_stride();

    let ctx = BranchCtx { offset: 2 };

    // The branch should not be taken when lane masks are all-on.
    {
        let (first, second) = run_branch(
            &[Stage::InitLaneMasks],
            Stage::BranchIfNoLanesActive(ctx),
            &[],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should be taken when lane masks are all-off.
    {
        let no_lanes_active = Ints::zeroed(4 * MAX_STRIDE_HIGHP);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfNoLanesActive(ctx),
            &[&no_lanes_active],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should not be taken when lane masks are partially-on.
    if n > 1 {
        // An array of all zeros, except for a single ~0 in the last A slot.
        let one_lane_active = regs(n, false, !0);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(0))],
            Stage::BranchIfNoLanesActive(ctx),
            &[&one_lane_active],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
});

// Port of: tests/SkRasterPipelineTest.cpp#L2789-L2885 (chrome/m156)
def_test!(SkRasterPipeline_BranchIfActiveLanesEqual, |reporter| {
    let n = highp_stride();

    // An array of all 6s.
    let all_sixes = Ints::new(&[6; MAX_STRIDE_HIGHP]);

    // An array of all 6s, except for a single 5 in one lane.
    let mut mostly_sixes_with_one_five = [6; MAX_STRIDE_HIGHP];
    mostly_sixes_with_one_five[n - 1] = 5;
    let mostly_sixes_with_one_five = Ints::new(&mostly_sixes_with_one_five);

    // comparing all-six vs five will match
    let matching = BranchIfEqualCtx {
        offset: 2,
        value: 5,
        ptr: slot(0),
    };

    // comparing mostly-six vs five won't match
    let nonmatching = BranchIfEqualCtx {
        offset: 2,
        value: 5,
        ptr: slot(0),
    };

    // The branch should be taken when lane masks are all-on and we're checking 6 ≠ 5.
    {
        let (first, second) = run_branch(
            &[Stage::InitLaneMasks],
            Stage::BranchIfNoActiveLanesEq(&matching),
            &[&all_sixes],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should not be taken when lane masks are all-on and we're checking 5 ≠ 5
    {
        let (first, second) = run_branch(
            &[Stage::InitLaneMasks],
            Stage::BranchIfNoActiveLanesEq(&nonmatching),
            &[&mostly_sixes_with_one_five],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] != K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
    // The branch should be taken when the 5 = 5 lane is dead.
    if n > 1 {
        // An execution mask with all lanes on except for the five-lane.
        let mask = regs(n, true, 0);
        let (first, second) = run_branch(
            &[Stage::LoadSrc(slot(1))],
            Stage::BranchIfNoActiveLanesEq(&nonmatching),
            &[&mostly_sixes_with_one_five, &mask],
            n,
        );
        for check_lane in 0..n {
            reporter_assert!(reporter, first[check_lane] == K_MARKER);
            reporter_assert!(reporter, second[check_lane] != K_MARKER);
        }
    }
});

/// The `swizzle_copy_[n_]slot[s]_masked` stage copying `count` slots.
fn swizzle_copy_stage(count: usize, ctx: &SwizzleCopyCtx) -> Stage<'_> {
    match count {
        1 => Stage::SwizzleCopySlotMasked(ctx),
        2 => Stage::SwizzleCopy2SlotsMasked(ctx),
        3 => Stage::SwizzleCopy3SlotsMasked(ctx),
        4 => Stage::SwizzleCopy4SlotsMasked(ctx),
        _ => unreachable!(),
    }
}
