// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineTest.cpp

// Port of: tests/SkRasterPipelineTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline` (no arena); `p.run(x, y, w, h)`
// takes the run's writable memory as a `MemoryBindings` (empty here).

use std::cell::RefCell;

use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, RasterPipeline, Stage,
};
use skia_rust_simd::rp::contexts::{
    TraceFuncCtx, TraceHook, TraceLineCtx, TraceScopeCtx, TraceVarCtx,
};

use crate::{def_test, reporter_assert};

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
