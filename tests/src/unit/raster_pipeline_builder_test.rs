// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RasterPipelineBuilderTest.cpp (chrome/m156)

//! The `SkSL` Raster Pipeline builder's dumps, checked against the text Skia prints.
//!
//! Where Skia's test has `#if SK_HAS_MUSTTAIL`, this port takes the `SK_HAS_MUSTTAIL` branch:
//! the `Flavor::Library` build, which never emits `stack_rewind` for non-tail-callers.
//! (`Flavor` is not a type yet; the dumper always makes the library choice.)

use std::sync::Arc;

use skia_rust_sksl::codegen::rp::{Builder, BuilderOp, Program, SlotRange};
use skia_rust_sksl::tracing::{DebugTracePriv, FunctionDebugInfo, SlotDebugInfo};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/RasterPipelineBuilderTest.cpp#L13-L31 (chrome/m156)
fn check(r: &mut Reporter, program: &Program, expected: &str) {
    // Verify that the program matches expectations.
    let dump = program.dump(false);
    reporter_assert!(
        r,
        dump == expected,
        "Output did not match expectation:\n{}",
        dump
    );
}

// Port of: tests/RasterPipelineBuilderTest.cpp#L33-L51 (chrome/m156)
fn one_slot_at(index: i32) -> SlotRange {
    SlotRange { index, count: 1 }
}

fn two_slots_at(index: i32) -> SlotRange {
    SlotRange { index, count: 2 }
}

fn three_slots_at(index: i32) -> SlotRange {
    SlotRange { index, count: 3 }
}

fn four_slots_at(index: i32) -> SlotRange {
    SlotRange { index, count: 4 }
}

fn five_slots_at(index: i32) -> SlotRange {
    SlotRange { index, count: 5 }
}

fn ten_slots_at(index: i32) -> SlotRange {
    SlotRange { index, count: 10 }
}

// Port of: tests/RasterPipelineBuilderTest.cpp#L58-L88 (chrome/m156)
def_test!(RasterPipelineBuilder, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.store_src_rg(two_slots_at(0));
    builder.store_src(four_slots_at(2));
    builder.store_dst(four_slots_at(4));
    builder.store_device_xy01(four_slots_at(6));
    builder.init_lane_masks();
    builder.enable_execution_mask_writes();
    builder.mask_off_return_mask();
    builder.mask_off_loop_mask();
    builder.reenable_loop_mask(one_slot_at(4));
    builder.disable_execution_mask_writes();
    builder.load_src(four_slots_at(1));
    builder.load_dst(four_slots_at(3));
    let program = builder.finish(10, 0, 0, None);
    check(
        r,
        &program,
        "store_src_rg                   v0..1 = src.rg
store_src                      v2..5 = src.rgba
store_dst                      v4..7 = dst.rgba
store_device_xy01              v6..9 = DeviceCoords.xy01
init_lane_masks                CondMask = LoopMask = RetMask = true
mask_off_return_mask           RetMask &= ~(CondMask & LoopMask & RetMask)
mask_off_loop_mask             LoopMask &= ~(CondMask & LoopMask & RetMask)
reenable_loop_mask             LoopMask |= v4
load_src                       src.rgba = v1..4
load_dst                       dst.rgba = v3..6
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L90-L135 (chrome/m156)
def_test!(RasterPipelineBuilderPushPopMaskRegisters, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();

    reporter_assert!(r, !builder.execution_mask_writes_are_enabled());
    builder.enable_execution_mask_writes();
    reporter_assert!(r, builder.execution_mask_writes_are_enabled());

    builder.push_condition_mask(); // push into 0
    builder.push_loop_mask(); // push into 1
    builder.push_return_mask(); // push into 2
    builder.merge_condition_mask(); // set the condition-mask to 1 & 2
    builder.merge_inv_condition_mask(); // set the condition-mask to 1 & ~2
    builder.pop_condition_mask(); // pop from 2
    builder.merge_loop_mask(); // mask off the loop-mask against 1
    builder.push_condition_mask(); // push into 2
    builder.pop_condition_mask(); // pop from 2
    builder.pop_loop_mask(); // pop from 1
    builder.pop_return_mask(); // pop from 0
    builder.push_condition_mask(); // push into 0
    builder.pop_and_reenable_loop_mask(); // pop from 0

    reporter_assert!(r, builder.execution_mask_writes_are_enabled());
    builder.disable_execution_mask_writes();
    reporter_assert!(r, !builder.execution_mask_writes_are_enabled());

    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "store_condition_mask           $0 = CondMask
store_loop_mask                $1 = LoopMask
store_return_mask              $2 = RetMask
merge_condition_mask           CondMask = $1 & $2
merge_inv_condition_mask       CondMask = $1 & ~$2
load_condition_mask            CondMask = $2
merge_loop_mask                LoopMask &= $1
store_condition_mask           $2 = CondMask
load_condition_mask            CondMask = $2
load_loop_mask                 LoopMask = $1
load_return_mask               RetMask = $0
store_condition_mask           $0 = CondMask
reenable_loop_mask             LoopMask |= $0
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L137-L156 (chrome/m156)
def_test!(RasterPipelineBuilderCaseOp, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();

    builder.push_constant_i(123, 1); // push a test value
    builder.push_constant_i(!0, 1); // push an all-on default mask
    builder.case_op(123); // do `case 123:`
    builder.case_op(124); // do `case 124:`
    builder.discard_stack(2);

    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "copy_constant                  $0 = 0x0000007B (1.723597e-43)
copy_constant                  $1 = 0xFFFFFFFF
case_op                        if ($0 == 0x0000007B) { LoopMask = true; $1 = false; }
case_op                        if ($0 == 0x0000007C) { LoopMask = true; $1 = false; }
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L158-L180 (chrome/m156)
def_test!(RasterPipelineBuilderPushPopSrcDst, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();

    builder.push_src_rgba();
    builder.push_dst_rgba();
    builder.pop_src_rgba();
    builder.exchange_src();
    builder.exchange_src();
    builder.exchange_src();
    builder.pop_dst_rgba();

    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "store_src                      $0..3 = src.rgba
store_dst                      $4..7 = dst.rgba
load_src                       src.rgba = $4..7
exchange_src                   swap(src.rgba, $0..3)
load_dst                       dst.rgba = $0..3
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L182-L198 (chrome/m156)
def_test!(RasterPipelineBuilderInvokeChild, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();

    builder.invoke_shader(1);
    builder.invoke_color_filter(2);
    builder.invoke_blender(3);

    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "invoke_shader                  invoke_shader 0x00000001
invoke_color_filter            invoke_color_filter 0x00000002
invoke_blender                 invoke_blender 0x00000003
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L200-L228 (chrome/m156)
def_test!(RasterPipelineBuilderPushPopTempImmediates, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.set_current_stack(1);
    builder.push_constant_i(999, 1); // push into 2
    builder.set_current_stack(0);
    builder.push_constant_f(13.5); // push into 0
    builder.push_clone_from_stack(one_slot_at(0), 1, 1); // push into 1 from 2
    builder.discard_stack(1); // discard 2
    builder.push_constant_u(357, 1); // push into 2
    builder.set_current_stack(1);
    builder.push_clone_from_stack(one_slot_at(0), 0, 1); // push into 3 from 0
    builder.discard_stack(2); // discard 2 and 3
    builder.set_current_stack(0);
    builder.push_constant_f(1.2); // push into 2
    builder.pad_stack(3); // pad slots 3,4,5
    builder.push_constant_f(3.4); // push into 6
    builder.discard_stack(7); // discard 0 through 6
    let program = builder.finish(1, 0, 0, None);
    check(
        r,
        &program,
        "copy_constant                  $2 = 0x000003E7 (1.399897e-42)
copy_constant                  $0 = 0x41580000 (13.5)
copy_constant                  $1 = 0x00000165 (5.002636e-43)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L230-L262 (chrome/m156)
def_test!(RasterPipelineBuilderPushPopIndirect, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.set_current_stack(1);
    builder.push_constant_i(3, 1);
    builder.set_current_stack(0);
    builder.push_slots_indirect(two_slots_at(0), 1, ten_slots_at(0));
    builder.push_slots_indirect(four_slots_at(10), 1, ten_slots_at(10));
    builder.push_uniform_indirect(one_slot_at(0), 1, five_slots_at(0));
    builder.push_uniform_indirect(three_slots_at(5), 1, five_slots_at(5));
    builder.swizzle_copy_stack_to_slots_indirect(
        three_slots_at(6),
        1,
        ten_slots_at(0),
        &[2, 1, 0],
        3,
    );
    builder.copy_stack_to_slots_indirect(three_slots_at(4), 1, ten_slots_at(0));
    builder.pop_slots_indirect(five_slots_at(0), 1, ten_slots_at(0));
    builder.pop_slots_indirect(five_slots_at(10), 1, ten_slots_at(10));
    builder.set_current_stack(1);
    builder.discard_stack(1);
    let program = builder.finish(20, 10, 0, None);
    check(
        r,
        &program,
        "copy_constant                  $10 = 0x00000003 (4.203895e-45)
copy_from_indirect_unmasked    $0..1 = Indirect(v0..1 + $10)
copy_from_indirect_unmasked    $2..5 = Indirect(v10..13 + $10)
copy_from_indirect_uniform_unm $6 = Indirect(u0 + $10)
copy_from_indirect_uniform_unm $7..9 = Indirect(u5..7 + $10)
swizzle_copy_to_indirect_maske Indirect(v6..8 + $10).zyx = Mask($7..9)
copy_to_indirect_masked        Indirect(v4..6 + $10) = Mask($7..9)
copy_to_indirect_masked        Indirect(v0..4 + $10) = Mask($5..9)
copy_to_indirect_masked        Indirect(v10..14 + $10) = Mask($0..4)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L264-L276 (chrome/m156)
def_test!(RasterPipelineBuilderCopySlotsMasked, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.copy_slots_masked(two_slots_at(0), two_slots_at(2));
    builder.copy_slots_masked(four_slots_at(1), four_slots_at(5));
    let program = builder.finish(9, 0, 0, None);
    check(
        r,
        &program,
        "copy_2_slots_masked            v0..1 = Mask(v2..3)
copy_4_slots_masked            v1..4 = Mask(v5..8)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L278-L291 (chrome/m156)
def_test!(RasterPipelineBuilderCopySlotsUnmasked, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.copy_slots_unmasked(three_slots_at(0), three_slots_at(2));
    builder.copy_slots_unmasked(five_slots_at(1), five_slots_at(5));
    let program = builder.finish(10, 0, 0, None);
    check(
        r,
        &program,
        "copy_3_slots_unmasked          v0..2 = v2..4
copy_4_slots_unmasked          v1..4 = v5..8
copy_slot_unmasked             v5 = v9
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L293-L317 (chrome/m156)
def_test!(RasterPipelineBuilderPushPopSlots, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.push_slots(four_slots_at(10)); // push from 10~13 into $0~$3
    builder.copy_stack_to_slots(one_slot_at(5), 3); // copy from $1 into 5
    builder.pop_slots_unmasked(two_slots_at(20)); // pop from $2~$3 into 20~21 (unmasked)
    builder.enable_execution_mask_writes();
    builder.copy_stack_to_slots_unmasked(one_slot_at(4), 2); // copy from $0 into 4
    builder.push_slots(three_slots_at(30)); // push from 30~32 into $2~$4
    builder.pop_slots(five_slots_at(0)); // pop from $0~$4 into 0~4 (masked)
    builder.disable_execution_mask_writes();

    let program = builder.finish(50, 0, 0, None);
    check(
        r,
        &program,
        "copy_4_slots_unmasked          $0..3 = v10..13
copy_slot_unmasked             v5 = $1
copy_2_slots_unmasked          v20..21 = $2..3
copy_slot_unmasked             v4 = $0
copy_3_slots_unmasked          $2..4 = v30..32
copy_4_slots_masked            v0..3 = Mask($0..3)
copy_slot_masked               v4 = Mask($4)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L319-L351 (chrome/m156)
def_test!(RasterPipelineBuilderDuplicateSelectAndSwizzleSlots, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.push_constant_f(1.0); // push into 0
    builder.push_duplicates(1); // duplicate into 1
    builder.push_duplicates(2); // duplicate into 2~3
    builder.push_duplicates(3); // duplicate into 4~6
    builder.push_duplicates(5); // duplicate into 7~11
    builder.select(4); // select from 4~7 and 8~11 into 4~7
    builder.select(3); // select from 2~4 and 5~7 into 2~4
    builder.select(1); // select from 3 and 4 into 3
    builder.swizzle_copy_stack_to_slots(four_slots_at(1), &[3, 2, 1, 0], 4);
    builder.swizzle_copy_stack_to_slots(four_slots_at(0), &[0, 1, 3], 3);
    builder.swizzle(4, &[3, 2, 1, 0]); // reverse the order of 0~3 (value.wzyx)
    builder.swizzle(4, &[1, 2]); // eliminate elements 0 and 3 (value.yz)
    builder.swizzle(2, &[0]); // eliminate element 1 (value.x)
    builder.discard_stack(1); // balance stack
    let program = builder.finish(6, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x3F800000 (1.0)
splat_4_constants              $4..7 = 0x3F800000 (1.0)
splat_4_constants              $8..11 = 0x3F800000 (1.0)
copy_4_slots_masked            $4..7 = Mask($8..11)
copy_3_slots_masked            $2..4 = Mask($5..7)
copy_slot_masked               $3 = Mask($4)
swizzle_copy_4_slots_masked    (v1..4).wzyx = Mask($0..3)
swizzle_copy_3_slots_masked    (v0..3).xyw = Mask($1..3)
swizzle_4                      $0..3 = ($0..3).wzyx
swizzle_2                      $0..1 = ($0..2).yz
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L353-L378 (chrome/m156)
def_test!(RasterPipelineBuilderTransposeMatrix, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.push_constant_f(1.0); // push into 0
    builder.push_duplicates(15); // duplicate into 1~15
    builder.transpose(2, 2); // transpose a 2x2 matrix
    builder.transpose(3, 3); // transpose a 3x3 matrix
    builder.transpose(4, 4); // transpose a 4x4 matrix
    builder.transpose(2, 4); // transpose a 2x4 matrix
    builder.transpose(4, 3); // transpose a 4x3 matrix
    builder.discard_stack(16); // balance stack
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x3F800000 (1.0)
splat_4_constants              $4..7 = 0x3F800000 (1.0)
splat_4_constants              $8..11 = 0x3F800000 (1.0)
splat_4_constants              $12..15 = 0x3F800000 (1.0)
swizzle_3                      $13..15 = ($13..15).yxz
shuffle                        $8..15 = ($8..15)[2 5 0 3 6 1 4 7]
shuffle                        $1..15 = ($1..15)[3 7 11 0 4 8 12 1 5 9 13 2 6 10 14]
shuffle                        $9..15 = ($9..15)[3 0 4 1 5 2 6]
shuffle                        $5..15 = ($5..15)[2 5 8 0 3 6 9 1 4 7 10]
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L380-L409 (chrome/m156)
def_test!(RasterPipelineBuilderDiagonalMatrix, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.push_constant_f(0.0); // push into 0
    builder.push_constant_f(1.0); // push into 1
    builder.diagonal_matrix(2, 2); // generate a 2x2 diagonal matrix
    builder.discard_stack(4); // balance stack
    builder.push_constant_f(0.0); // push into 0
    builder.push_constant_f(2.0); // push into 1
    builder.diagonal_matrix(4, 4); // generate a 4x4 diagonal matrix
    builder.discard_stack(16); // balance stack
    builder.push_constant_f(0.0); // push into 0
    builder.push_constant_f(3.0); // push into 1
    builder.diagonal_matrix(2, 3); // generate a 2x3 diagonal matrix
    builder.discard_stack(6); // balance stack
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "copy_constant                  $0 = 0
copy_constant                  $1 = 0x3F800000 (1.0)
swizzle_4                      $0..3 = ($0..3).yxxy
copy_constant                  $0 = 0
copy_constant                  $1 = 0x40000000 (2.0)
shuffle                        $0..15 = ($0..15)[1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1]
copy_constant                  $0 = 0
copy_constant                  $1 = 0x40400000 (3.0)
shuffle                        $0..5 = ($0..5)[1 0 0 0 1 0]
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L411-L444 (chrome/m156)
def_test!(RasterPipelineBuilderMatrixResize, |r| {
    // Create a very simple nonsense program.
    let mut builder = Builder::new();
    builder.push_constant_f(1.0); // synthesize a 2x2 matrix
    builder.push_constant_f(2.0);
    builder.push_constant_f(3.0);
    builder.push_constant_f(4.0);
    builder.matrix_resize(2, 2, 4, 4); // resize 2x2 matrix into 4x4
    builder.matrix_resize(4, 4, 2, 2); // resize 4x4 matrix back into 2x2
    builder.matrix_resize(2, 2, 2, 4); // resize 2x2 matrix into 2x4
    builder.matrix_resize(2, 4, 4, 2); // resize 2x4 matrix into 4x2
    builder.matrix_resize(4, 2, 3, 3); // resize 4x2 matrix into 3x3
    builder.discard_stack(9); // balance stack
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "copy_constant                  $0 = 0x3F800000 (1.0)
copy_constant                  $1 = 0x40000000 (2.0)
copy_constant                  $2 = 0x40400000 (3.0)
copy_constant                  $3 = 0x40800000 (4.0)
copy_constant                  $4 = 0
copy_constant                  $5 = 0x3F800000 (1.0)
shuffle                        $2..15 = ($2..15)[2 2 0 1 2 2 2 2 3 2 2 2 2 3]
shuffle                        $2..3 = ($2..3)[2 3]
copy_constant                  $4 = 0
shuffle                        $2..7 = ($2..7)[2 2 0 1 2 2]
copy_constant                  $8 = 0
shuffle                        $2..7 = ($2..7)[2 3 6 6 6 6]
copy_constant                  $8 = 0
copy_constant                  $9 = 0x3F800000 (1.0)
shuffle                        $2..8 = ($2..8)[6 0 1 6 2 3 7]
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L446-L562 (chrome/m156)
// The `SK_HAS_MUSTTAIL` branch: no `stack_rewind` before backwards branches.
def_test!(RasterPipelineBuilderBranches, |r| {
    const EXPECTATION_WITH_KNOWN_EXECUTION_MASK: &str = "jump                           jump +9 (label 3 at #10)
label                          label 0
copy_constant                  v0 = 0
label                          label 0x00000001
copy_constant                  v1 = 0
jump                           jump -4 (label 0 at #2)
label                          label 0x00000002
copy_constant                  v2 = 0
jump                           jump -7 (label 0 at #2)
label                          label 0x00000003
branch_if_no_active_lanes_eq   branch -4 (label 2 at #7) if no lanes of v2 == 0
branch_if_no_active_lanes_eq   branch -10 (label 0 at #2) if no lanes of v2 == 0x00000001 (1.401298e-45)
";
    const EXPECTATION_WITH_EXECUTION_MASK_WRITES: &str = "jump                           jump +10 (label 3 at #11)
label                          label 0
copy_constant                  v0 = 0
label                          label 0x00000001
copy_constant                  v1 = 0
branch_if_no_lanes_active      branch_if_no_lanes_active -2 (label 1 at #4)
branch_if_all_lanes_active     branch_if_all_lanes_active -5 (label 0 at #2)
label                          label 0x00000002
copy_constant                  v2 = 0
branch_if_any_lanes_active     branch_if_any_lanes_active -8 (label 0 at #2)
label                          label 0x00000003
branch_if_no_active_lanes_eq   branch -4 (label 2 at #8) if no lanes of v2 == 0
branch_if_no_active_lanes_eq   branch -11 (label 0 at #2) if no lanes of v2 == 0x00000001 (1.401298e-45)
";

    for enable_execution_mask_writes in [false, true] {
        // Create a very simple nonsense program.
        let mut builder = Builder::new();
        let label1 = builder.next_label_id();
        let label2 = builder.next_label_id();
        let label3 = builder.next_label_id();
        let label4 = builder.next_label_id();

        if enable_execution_mask_writes {
            builder.enable_execution_mask_writes();
        }

        builder.jump(label4);
        builder.label(label1);
        builder.zero_slots_unmasked(one_slot_at(0));
        builder.label(label2);
        builder.zero_slots_unmasked(one_slot_at(1));
        builder.branch_if_no_lanes_active(label2);
        builder.branch_if_no_lanes_active(label3);
        builder.branch_if_all_lanes_active(label1);
        builder.label(label3);
        builder.zero_slots_unmasked(one_slot_at(2));
        builder.branch_if_any_lanes_active(label1);
        builder.branch_if_any_lanes_active(label1);
        builder.label(label4);
        builder.branch_if_no_active_lanes_on_stack_top_equal(0, label3);
        builder.branch_if_no_active_lanes_on_stack_top_equal(0, label2);
        builder.branch_if_no_active_lanes_on_stack_top_equal(1, label1);
        builder.branch_if_no_active_lanes_on_stack_top_equal(1, label4);

        if enable_execution_mask_writes {
            builder.disable_execution_mask_writes();
        }

        let program = builder.finish(3, 0, 0, None);
        check(
            r,
            &program,
            if enable_execution_mask_writes {
                EXPECTATION_WITH_EXECUTION_MASK_WRITES
            } else {
                EXPECTATION_WITH_KNOWN_EXECUTION_MASK
            },
        );
    }
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L564-L587 (chrome/m156)
def_test!(
    RasterPipelineBuilderBackwardsBranchOverInvocationShouldRewind,
    |r| {
        // Branching backward over a call to invoke_shader should always emit a stack_rewind op.
        let mut builder = Builder::new();
        let label1 = builder.next_label_id();
        builder.push_constant_f(10.0);
        builder.label(label1);
        builder.push_constant_f(20.0);
        builder.invoke_shader(9);
        builder.push_constant_f(30.0);
        builder.discard_stack(3);
        builder.branch_if_any_lanes_active(label1);

        let program = builder.finish(3, 0, 0, None);
        check(
            r,
            &program,
            "copy_constant                  $0 = 0x41200000 (10.0)
label                          label 0
copy_constant                  $1 = 0x41A00000 (20.0)
invoke_shader                  invoke_shader 0x00000009
stack_rewind
jump                           jump -4 (label 0 at #2)
",
        );
    }
);

// Port of: tests/RasterPipelineBuilderTest.cpp#L589-L621 (chrome/m156)
// The `SK_HAS_MUSTTAIL` branch.
def_test!(
    RasterPipelineBuilderBackwardsBranchWithoutInvocationMightNotRewind,
    |r| {
        let mut builder = Builder::new();
        let label1 = builder.next_label_id();
        builder.push_constant_f(10.0);
        builder.invoke_shader(9);
        builder.push_constant_f(20.0);
        builder.label(label1);
        builder.push_constant_f(30.0);
        builder.discard_stack(3);
        builder.branch_if_any_lanes_active(label1);

        let program = builder.finish(3, 0, 0, None);
        check(
            r,
            &program,
            "copy_constant                  $0 = 0x41200000 (10.0)
invoke_shader                  invoke_shader 0x00000009
copy_constant                  $1 = 0x41A00000 (20.0)
label                          label 0
jump                           jump -1 (label 0 at #4)
",
        );
    }
);

// Port of: tests/RasterPipelineBuilderTest.cpp#L623-L663 (chrome/m156)
def_test!(RasterPipelineBuilderBinaryFloatOps, |r| {
    let mut builder = Builder::new();
    builder.push_constant_f(10.0);
    builder.push_duplicates(30);
    builder.binary_op(BuilderOp::AddNFloats, 1);
    builder.binary_op(BuilderOp::SubNFloats, 2);
    builder.binary_op(BuilderOp::MulNFloats, 3);
    builder.binary_op(BuilderOp::DivNFloats, 4);
    builder.binary_op(BuilderOp::MaxNFloats, 3);
    builder.binary_op(BuilderOp::MinNFloats, 2);
    builder.binary_op(BuilderOp::CmpltNFloats, 5);
    builder.binary_op(BuilderOp::CmpleNFloats, 4);
    builder.binary_op(BuilderOp::CmpeqNFloats, 3);
    builder.binary_op(BuilderOp::CmpneNFloats, 2);
    builder.discard_stack(2);
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x41200000 (10.0)
splat_4_constants              $4..7 = 0x41200000 (10.0)
splat_4_constants              $8..11 = 0x41200000 (10.0)
splat_4_constants              $12..15 = 0x41200000 (10.0)
splat_4_constants              $16..19 = 0x41200000 (10.0)
splat_4_constants              $20..23 = 0x41200000 (10.0)
splat_4_constants              $24..27 = 0x41200000 (10.0)
splat_2_constants              $28..29 = 0x41200000 (10.0)
add_imm_float                  $29 += 0x41200000 (10.0)
sub_2_floats                   $26..27 -= $28..29
mul_3_floats                   $22..24 *= $25..27
div_4_floats                   $17..20 /= $21..24
max_3_floats                   $15..17 = max($15..17, $18..20)
min_2_floats                   $14..15 = min($14..15, $16..17)
cmplt_n_floats                 $6..10 = lessThan($6..10, $11..15)
cmple_4_floats                 $3..6 = lessThanEqual($3..6, $7..10)
cmpeq_3_floats                 $1..3 = equal($1..3, $4..6)
cmpne_2_floats                 $0..1 = notEqual($0..1, $2..3)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L665-L713 (chrome/m156)
def_test!(RasterPipelineBuilderBinaryIntOps, |r| {
    let mut builder = Builder::new();
    builder.push_constant_i(123, 1);
    builder.push_duplicates(40);
    builder.binary_op(BuilderOp::BitwiseAndNInts, 1);
    builder.binary_op(BuilderOp::BitwiseXorNInts, 2);
    builder.binary_op(BuilderOp::BitwiseOrNInts, 3);
    builder.binary_op(BuilderOp::AddNInts, 2);
    builder.binary_op(BuilderOp::SubNInts, 3);
    builder.binary_op(BuilderOp::MulNInts, 4);
    builder.binary_op(BuilderOp::DivNInts, 5);
    builder.binary_op(BuilderOp::MaxNInts, 4);
    builder.binary_op(BuilderOp::MinNInts, 3);
    builder.binary_op(BuilderOp::CmpltNInts, 1);
    builder.binary_op(BuilderOp::CmpleNInts, 2);
    builder.binary_op(BuilderOp::CmpeqNInts, 3);
    builder.binary_op(BuilderOp::CmpneNInts, 4);
    builder.discard_stack(4);
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x0000007B (1.723597e-43)
splat_4_constants              $4..7 = 0x0000007B (1.723597e-43)
splat_4_constants              $8..11 = 0x0000007B (1.723597e-43)
splat_4_constants              $12..15 = 0x0000007B (1.723597e-43)
splat_4_constants              $16..19 = 0x0000007B (1.723597e-43)
splat_4_constants              $20..23 = 0x0000007B (1.723597e-43)
splat_4_constants              $24..27 = 0x0000007B (1.723597e-43)
splat_4_constants              $28..31 = 0x0000007B (1.723597e-43)
splat_4_constants              $32..35 = 0x0000007B (1.723597e-43)
splat_4_constants              $36..39 = 0x0000007B (1.723597e-43)
bitwise_and_imm_int            $39 &= 0x0000007B
bitwise_xor_2_ints             $36..37 ^= $38..39
bitwise_or_3_ints              $32..34 |= $35..37
add_2_ints                     $31..32 += $33..34
sub_3_ints                     $27..29 -= $30..32
mul_4_ints                     $22..25 *= $26..29
div_n_ints                     $16..20 /= $21..25
max_4_ints                     $13..16 = max($13..16, $17..20)
min_3_ints                     $11..13 = min($11..13, $14..16)
cmplt_int                      $12 = lessThan($12, $13)
cmple_2_ints                   $9..10 = lessThanEqual($9..10, $11..12)
cmpeq_3_ints                   $5..7 = equal($5..7, $8..10)
cmpne_4_ints                   $0..3 = notEqual($0..3, $4..7)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L715-L743 (chrome/m156)
def_test!(RasterPipelineBuilderBinaryUIntOps, |r| {
    let mut builder = Builder::new();
    builder.push_constant_u(456, 1);
    builder.push_duplicates(21);
    builder.binary_op(BuilderOp::DivNUints, 6);
    builder.binary_op(BuilderOp::CmpltNUints, 5);
    builder.binary_op(BuilderOp::CmpleNUints, 4);
    builder.binary_op(BuilderOp::MaxNUints, 3);
    builder.binary_op(BuilderOp::MinNUints, 2);
    builder.discard_stack(2);
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x000001C8 (6.389921e-43)
splat_4_constants              $4..7 = 0x000001C8 (6.389921e-43)
splat_4_constants              $8..11 = 0x000001C8 (6.389921e-43)
splat_4_constants              $12..15 = 0x000001C8 (6.389921e-43)
splat_4_constants              $16..19 = 0x000001C8 (6.389921e-43)
splat_2_constants              $20..21 = 0x000001C8 (6.389921e-43)
div_n_uints                    $10..15 /= $16..21
cmplt_n_uints                  $6..10 = lessThan($6..10, $11..15)
cmple_4_uints                  $3..6 = lessThanEqual($3..6, $7..10)
max_3_uints                    $1..3 = max($1..3, $4..6)
min_2_uints                    $0..1 = min($0..1, $2..3)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L745-L787 (chrome/m156)
def_test!(RasterPipelineBuilderUnaryOps, |r| {
    let mut builder = Builder::new();
    builder.push_constant_i(456, 1);
    builder.push_duplicates(4);
    builder.unary_op(BuilderOp::CastToFloatFromInt, 1);
    builder.unary_op(BuilderOp::CastToFloatFromUint, 2);
    builder.unary_op(BuilderOp::CastToIntFromFloat, 3);
    builder.unary_op(BuilderOp::CastToUintFromFloat, 4);
    builder.unary_op(BuilderOp::CosFloat, 4);
    builder.unary_op(BuilderOp::TanFloat, 3);
    builder.unary_op(BuilderOp::SinFloat, 2);
    builder.unary_op(BuilderOp::SqrtFloat, 1);
    builder.unary_op(BuilderOp::AbsInt, 2);
    builder.unary_op(BuilderOp::FloorFloat, 3);
    builder.unary_op(BuilderOp::CeilFloat, 4);
    builder.discard_stack(5);
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x000001C8 (6.389921e-43)
copy_constant                  $4 = 0x000001C8 (6.389921e-43)
cast_to_float_from_int         $4 = IntToFloat($4)
cast_to_float_from_2_uints     $3..4 = UintToFloat($3..4)
cast_to_int_from_3_floats      $2..4 = FloatToInt($2..4)
cast_to_uint_from_4_floats     $1..4 = FloatToUint($1..4)
cos_float                      $1 = cos($1)
cos_float                      $2 = cos($2)
cos_float                      $3 = cos($3)
cos_float                      $4 = cos($4)
tan_float                      $2 = tan($2)
tan_float                      $3 = tan($3)
tan_float                      $4 = tan($4)
sin_float                      $3 = sin($3)
sin_float                      $4 = sin($4)
sqrt_float                     $4 = sqrt($4)
abs_2_ints                     $3..4 = abs($3..4)
floor_3_floats                 $2..4 = floor($2..4)
ceil_4_floats                  $1..4 = ceil($1..4)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L789-L812 (chrome/m156)
def_test!(RasterPipelineBuilderUniforms, |r| {
    let mut builder = Builder::new();
    builder.push_uniform(one_slot_at(0)); // push into 0
    builder.push_uniform(two_slots_at(1)); // push into 1~2
    builder.push_uniform(three_slots_at(3)); // push into 3~5
    builder.push_uniform(four_slots_at(6)); // push into 6~9
    builder.push_uniform(five_slots_at(0)); // push into 10~14
    builder.unary_op(BuilderOp::AbsInt, 1); // perform work so the program isn't eliminated
    builder.discard_stack(15); // balance stack
    let program = builder.finish(0, 10, 0, None);
    check(
        r,
        &program,
        "copy_4_uniforms                $0..3 = u0..3
copy_4_uniforms                $4..7 = u4..7
copy_2_uniforms                $8..9 = u8..9
copy_4_uniforms                $10..13 = u0..3
copy_uniform                   $14 = u4
abs_int                        $14 = abs($14)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L814-L836 (chrome/m156)
def_test!(RasterPipelineBuilderPushZeros, |r| {
    let mut builder = Builder::new();
    builder.push_zeros(1); // push into 0
    builder.push_zeros(2); // push into 1~2
    builder.push_zeros(3); // push into 3~5
    builder.push_zeros(4); // push into 6~9
    builder.push_zeros(5); // push into 10~14
    builder.unary_op(BuilderOp::AbsInt, 1); // perform work so the program isn't eliminated
    builder.discard_stack(15); // balance stack
    let program = builder.finish(0, 10, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0
splat_4_constants              $4..7 = 0
splat_4_constants              $8..11 = 0
splat_3_constants              $12..14 = 0
abs_int                        $14 = abs($14)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L838-L855 (chrome/m156)
def_test!(RasterPipelineBuilderTernaryFloatOps, |r| {
    let mut builder = Builder::new();
    builder.push_constant_f(0.75);
    builder.push_duplicates(8);
    builder.ternary_op(BuilderOp::MixNFloats, 3);
    builder.discard_stack(3);
    let program = builder.finish(0, 0, 0, None);
    check(
        r,
        &program,
        "splat_4_constants              $0..3 = 0x3F400000 (0.75)
splat_4_constants              $4..7 = 0x3F400000 (0.75)
copy_constant                  $8 = 0x3F400000 (0.75)
mix_3_floats                   $0..2 = mix($3..5, $6..8, $0..2)
",
    );
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L857-L878 (chrome/m156)
// The `SK_HAS_MUSTTAIL` branch: tail calls are guaranteed, so no `stack_rewind` is emitted.
def_test!(RasterPipelineBuilderAutomaticStackRewinding, |r| {
    let mut builder = Builder::new();
    builder.push_constant_i(1, 1);
    builder.push_duplicates(2000);
    builder.unary_op(BuilderOp::AbsInt, 1); // perform work so the program isn't eliminated
    builder.discard_stack(2001);
    let program = builder.finish(0, 0, 0, None);
    let dump = program.dump(false);

    // We have guaranteed tail-calling, so we never use `stack_rewind`.
    reporter_assert!(r, !dump.contains("stack_rewind"));
});

// Port of: tests/RasterPipelineBuilderTest.cpp#L880-L942 (chrome/m156)
def_test!(RasterPipelineBuilderTraceOps, |r| {
    for provide_debug_trace in [false, true] {
        let mut builder = Builder::new();
        // Create a trace mask stack on stack-ID 123.
        builder.set_current_stack(123);
        builder.push_constant_i(!0, 1);
        // Emit trace ops.
        builder.trace_enter(123, 2);
        builder.trace_scope(123, 1);
        builder.trace_line(123, 456);
        builder.trace_var(123, two_slots_at(3));
        builder.trace_scope(123, -1);
        builder.trace_exit(123, 2);
        // Discard the trace mask.
        builder.discard_stack(1);

        if provide_debug_trace {
            // Test the output when we supply a populated DebugTrace.
            let trace = DebugTracePriv {
                func_info: ["FunctionA", "FunctionB", "FunctionC", "FunctionD"]
                    .map(|name| FunctionDebugInfo {
                        name: name.to_owned(),
                    })
                    .into(),
                slot_info: ["Var0", "Var1", "Var2", "Var3", "Var4"]
                    .map(SlotDebugInfo::named)
                    .into(),
                ..DebugTracePriv::default()
            };

            let program = builder.finish(20, 0, 0, Some(Arc::new(trace)));
            check(
                r,
                &program,
                "copy_constant                  $0 = 0xFFFFFFFF
trace_enter                    TraceEnter(FunctionC) when $0 is true
trace_scope                    TraceScope(+1) when $0 is true
trace_line                     TraceLine(456) when $0 is true
trace_var                      TraceVar(Var3, Var4) when $0 is true
trace_scope                    TraceScope(-1) when $0 is true
trace_exit                     TraceExit(FunctionC) when $0 is true
",
            );
        } else {
            // Test the output when no DebugTrace info is provided.
            let program = builder.finish(20, 0, 0, None);
            check(
                r,
                &program,
                "copy_constant                  $0 = 0xFFFFFFFF
trace_enter                    TraceEnter(???) when $0 is true
trace_scope                    TraceScope(+1) when $0 is true
trace_line                     TraceLine(456) when $0 is true
trace_var                      TraceVar(v3..4) when $0 is true
trace_scope                    TraceScope(-1) when $0 is true
trace_exit                     TraceExit(???) when $0 is true
",
            );
        }
    }
});
