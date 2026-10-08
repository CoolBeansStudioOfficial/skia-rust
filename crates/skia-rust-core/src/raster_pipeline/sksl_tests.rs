// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Execution tests of `SkSL` programs appended to a raster pipeline (`Program::append_stages`).
//!
//! Each test builds a program with the builder (as `RasterPipelineBuilderTest` does), appends it
//! to a [`RasterPipeline`] on one tier, runs it over one pixel and reads the value slots back from
//! the scratch memory the program's slab lives in. The expected values are worked out from the
//! stage semantics: each slot is `N` lanes (the tier's highp stride), and every lane of a
//! splatted or unmasked result holds the same value.

// Values are small exact binary fractions, so the comparisons are exact by construction.
#![allow(clippy::float_cmp)]

use skia_rust_simd::Tier;
use skia_rust_simd::rp::Stage;
use skia_rust_simd::rp::contexts::ConstantCtx;
use skia_rust_simd::testing::{force_tier, oracle_selection};
use skia_rust_sksl::codegen::rp::{Builder, BuilderOp, Callbacks, Program, SlotRange};

use crate::arena_alloc::ArenaAlloc;
use crate::effect_priv::SHADER_SCRATCH;
use crate::raster_pipeline::{MemView, MemoryBindings, RasterPipeline};

/// Every tier the host can run (natively or modeled), as the oracle compares them.
const TIERS: [Tier; 6] = Tier::ALL;

fn one(index: i32) -> SlotRange {
    SlotRange { index, count: 1 }
}

fn two(index: i32) -> SlotRange {
    SlotRange { index, count: 2 }
}

/// The bit pattern of `f` as a stage or builder immediate.
fn bits(f: f32) -> i32 {
    f.to_bits().cast_signed()
}

/// Appends `program` to a pipeline on `tier`, runs it over one pixel with `uniforms`, and returns
/// the first `slots` value slots, each as its `lanes` lane values.
fn run<'a>(
    alloc: &'a ArenaAlloc,
    tier: Tier,
    program: &Program,
    slots: usize,
    uniforms: &[f32],
    children: Option<&mut dyn Callbacks<'a, RasterPipeline<'a>>>,
) -> Vec<Vec<f32>> {
    let _guard =
        force_tier(oracle_selection(tier)).expect("the tier runs here, natively or modeled");
    let lanes = tier.highp_stride();
    let mut pipeline = RasterPipeline::new();
    assert!(program.append_stages(&mut pipeline, alloc, children, uniforms));
    assert_eq!(pipeline.lane_count(), Some(lanes));

    // The slab is the arena's first allocation, so its values start at byte 0.
    let mut scratch = alloc.scratch_buffer();
    let mut mem = MemoryBindings::new();
    mem.bind(SHADER_SCRATCH, MemView::write(&mut scratch));
    pipeline.run(0, 0, 1, 1, &mut mem);

    (0..slots)
        .map(|slot| {
            (0..lanes)
                .map(|lane| {
                    let at = 4 * (slot * lanes + lane);
                    f32::from_ne_bytes(scratch[at..at + 4].try_into().expect("four bytes"))
                })
                .collect()
        })
        .collect()
}

/// `value` in each of the `lanes` lanes.
fn splat(value: f32, tier: Tier) -> Vec<f32> {
    vec![value; tier.highp_stride()]
}

// Constants and the n-way arithmetic ops, appended from a built program.
#[test]
fn constants_and_arithmetic_run_on_every_tier() {
    let mut b = Builder::new();
    b.push_constant_f(1.5);
    b.push_constant_f(2.25);
    b.binary_op(BuilderOp::AddNFloats, 1);
    b.pop_slots_unmasked(one(0));
    b.push_constant_f(6.0);
    b.push_constant_f(4.0);
    b.binary_op(BuilderOp::SubNFloats, 1);
    b.pop_slots_unmasked(one(1));
    b.push_constant_f(3.0);
    b.push_constant_f(-0.5);
    b.binary_op(BuilderOp::MulNFloats, 1);
    b.pop_slots_unmasked(one(2));
    let program = b.finish(3, 0, 0, None);

    for tier in TIERS {
        let alloc = ArenaAlloc::new();
        let slots = run(&alloc, tier, &program, 3, &[], None);
        assert_eq!(slots[0], splat(3.75, tier), "1.5 + 2.25 on {tier:?}");
        assert_eq!(slots[1], splat(2.0, tier), "6 - 4 on {tier:?}");
        assert_eq!(slots[2], splat(-1.5, tier), "3 * -0.5 on {tier:?}");
    }
}

// Immutable slots and uniform copies, read from the program's slab and the uniform block.
#[test]
fn immutable_and_uniform_data_run_on_every_tier() {
    let mut b = Builder::new();
    b.store_immutable_value_i(0, bits(7.5));
    b.push_immutable(one(0));
    b.pop_slots_unmasked(one(3));
    b.push_uniform(one(1));
    b.pop_slots_unmasked(one(4));
    b.push_uniform(two(0));
    b.pop_slots_unmasked(two(5));
    let program = b.finish(7, 2, 1, None);
    let uniforms = [0.25, -8.0];

    for tier in TIERS {
        let alloc = ArenaAlloc::new();
        let slots = run(&alloc, tier, &program, 7, &uniforms, None);
        assert_eq!(slots[3], splat(7.5, tier), "immutable 0 on {tier:?}");
        assert_eq!(slots[4], splat(-8.0, tier), "uniform 1 on {tier:?}");
        assert_eq!(
            slots[5],
            splat(0.25, tier),
            "uniforms 0..2, first on {tier:?}"
        );
        assert_eq!(
            slots[6],
            splat(-8.0, tier),
            "uniforms 0..2, second on {tier:?}"
        );
        // Slots nothing wrote stay zero.
        assert_eq!(slots[2], splat(0.0, tier), "unwritten slot on {tier:?}");
    }
}

// A forward jump: the store it skips does not run, the store at its label does.
#[test]
fn a_jump_skips_to_its_label_on_every_tier() {
    let mut b = Builder::new();
    let skip = b.next_label_id();
    b.jump(skip);
    b.push_constant_f(9.0);
    b.pop_slots_unmasked(one(0));
    b.label(skip);
    b.push_constant_f(1.0);
    b.pop_slots_unmasked(one(1));
    let program = b.finish(2, 0, 0, None);

    for tier in TIERS {
        let alloc = ArenaAlloc::new();
        let slots = run(&alloc, tier, &program, 2, &[], None);
        assert_eq!(slots[0], splat(0.0, tier), "the skipped store on {tier:?}");
        assert_eq!(slots[1], splat(1.0, tier), "the label's store on {tier:?}");
    }
}

// A branch on the stack top, whose target is known only after the stages are appended (its
// context is replaced at the end).
#[test]
fn a_branch_on_the_stack_top_depends_on_the_lanes_on_every_tier() {
    // Branch past the store unless some active lane holds 5 on the stack top.
    let program = |top: i32| {
        let mut b = Builder::new();
        b.init_lane_masks();
        b.push_constant_i(top, 1);
        let end = b.next_label_id();
        b.branch_if_no_active_lanes_on_stack_top_equal(5, end);
        b.push_constant_f(1.0);
        b.pop_slots_unmasked(one(0));
        b.label(end);
        b.discard_stack(1);
        b.finish(1, 0, 0, None)
    };
    let taken = program(7);
    let fallthrough = program(5);

    for tier in TIERS {
        let alloc = ArenaAlloc::new();
        let skipped = run(&alloc, tier, &taken, 1, &[], None);
        assert_eq!(
            skipped[0],
            splat(0.0, tier),
            "no lane equal: branch taken on {tier:?}"
        );
        let alloc = ArenaAlloc::new();
        let stored = run(&alloc, tier, &fallthrough, 1, &[], None);
        assert_eq!(
            stored[0],
            splat(1.0, tier),
            "a lane equal: no branch on {tier:?}"
        );
    }
}

/// A child shader that stores 2.0 into value slot 3.
struct StoreChild {
    lanes: usize,
}

impl<'a> Callbacks<'a, RasterPipeline<'a>> for StoreChild {
    fn append_shader(&mut self, p: &mut RasterPipeline<'a>, index: i32) -> bool {
        assert_eq!(index, 0, "the program invokes child 0");
        let dst = 3 * 4 * u32::try_from(self.lanes).expect("lanes fit in a u32");
        p.append(Stage::CopyConstant(ConstantCtx {
            value: bits(2.0),
            dst,
        }));
        true
    }

    fn append_color_filter(&mut self, _: &mut RasterPipeline<'a>, _: i32) -> bool {
        false
    }

    fn append_blender(&mut self, _: &mut RasterPipeline<'a>, _: i32) -> bool {
        false
    }

    fn to_linear_srgb(&mut self, _: &mut RasterPipeline<'a>, _: skia_rust_simd::rp::MemPtr) {
        unreachable!("the program has no color space transform")
    }

    fn from_linear_srgb(&mut self, _: &mut RasterPipeline<'a>, _: skia_rust_simd::rp::MemPtr) {
        unreachable!("the program has no color space transform")
    }
}

// A child invocation: the callback appends its stages, then the base pointer is reset.
#[test]
fn a_child_shader_appends_its_stages_on_every_tier() {
    let mut b = Builder::new();
    b.invoke_shader(0);
    let program = b.finish(4, 0, 0, None);

    for tier in TIERS {
        let alloc = ArenaAlloc::new();
        let mut child = StoreChild {
            lanes: tier.highp_stride(),
        };
        let slots = run(&alloc, tier, &program, 4, &[], Some(&mut child));
        assert_eq!(slots[3], splat(2.0, tier), "the child's store on {tier:?}");
        assert_eq!(slots[0], splat(0.0, tier), "the caller's slot on {tier:?}");
    }
}

#[test]
fn a_program_that_invokes_a_child_needs_callbacks() {
    let mut b = Builder::new();
    b.invoke_shader(0);
    let program = b.finish(4, 0, 0, None);

    let _guard = force_tier(oracle_selection(Tier::Scalar)).expect("scalar runs everywhere");
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    assert!(!program.append_stages(&mut pipeline, &alloc, None, &[]));
}

#[test]
fn trace_ops_are_not_appended_until_the_trace_hook_lands() {
    let mut b = Builder::new();
    b.trace_line(0, 1);
    let program = b.finish(0, 0, 0, None);

    let _guard = force_tier(oracle_selection(Tier::Scalar)).expect("scalar runs everywhere");
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    assert!(!program.append_stages(&mut pipeline, &alloc, None, &[]));
    assert_eq!(
        pipeline.num_stages(),
        0,
        "nothing is appended for a rejected program"
    );
}

#[test]
#[should_panic(expected = "were built for")]
fn a_pipeline_cannot_run_on_a_different_lane_count() {
    let mut b = Builder::new();
    b.push_constant_f(1.0);
    b.pop_slots_unmasked(one(0));
    let program = b.finish(1, 0, 0, None);

    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    {
        let _guard = force_tier(oracle_selection(Tier::Scalar)).expect("scalar runs everywhere");
        assert!(program.append_stages(&mut pipeline, &alloc, None, &[]));
    }
    // Built for one lane; run on four.
    let _guard = force_tier(oracle_selection(Tier::Sse2)).expect("sse2 runs everywhere");
    let mut scratch = alloc.scratch_buffer();
    let mut mem = MemoryBindings::new();
    mem.bind(SHADER_SCRATCH, MemView::write(&mut scratch));
    pipeline.run(0, 0, 1, 1, &mut mem);
}
