// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.{h,cpp} (the `Builder` and
// `Instruction` half; `Program` is in `program.rs`).

//! The Raster Pipeline builder: a stream of [`Instruction`]s with peephole rewrites, which
//! [`Builder::finish`] turns into a [`Program`].
//!
//! Instructions are indexed, not pointed at: where `Skia` keeps an `Instruction*` and then pops or
//! edits the instruction list, this port holds an index into `instructions` and re-reads it.

// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L46-L1398 (chrome/m156)

use std::sync::Arc;

use super::ops::BuilderOp;
use super::program::Program;
use crate::tracing::DebugTracePriv;

/// A single scalar in our program consumes one slot.
#[doc(alias = "SkSL::RP::Slot")]
pub type Slot = i32;

/// `SkSL::RP::NA`: "no slot".
#[doc(alias = "SkSL::RP::NA")]
pub const NA: Slot = -1;

/// Scalars, vectors, and matrices can be represented as a range of slot indices.
#[doc(alias = "SkSL::RP::SlotRange")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotRange {
    /// `index`: the first slot.
    pub index: Slot,
    /// `count`: the number of slots.
    pub count: i32,
}

/// `SkSL::RP::Instruction`: one raster-pipeline `SkSL` instruction.
#[doc(alias = "SkSL::RP::Instruction")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// `fOp`.
    pub op: BuilderOp,
    /// `fSlotA`.
    pub slot_a: Slot,
    /// `fSlotB`.
    pub slot_b: Slot,
    /// `fImmA`.
    pub imm_a: i32,
    /// `fImmB`.
    pub imm_b: i32,
    /// `fImmC`.
    pub imm_c: i32,
    /// `fImmD`.
    pub imm_d: i32,
    /// `fStackID`.
    pub stack_id: i32,
}

/// `Builder::SlotList`: up to two slots an instruction refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SlotList {
    a: Slot,
    b: Slot,
}

impl SlotList {
    const NONE: SlotList = SlotList { a: NA, b: NA };

    const fn one(a: Slot) -> SlotList {
        SlotList { a, b: NA }
    }

    const fn two(a: Slot, b: Slot) -> SlotList {
        SlotList { a, b }
    }
}

/// `is_immediate_op`: ops with an immediate-mode form (`ALL_IMMEDIATE_BINARY_OP_CASES`).
fn is_immediate_op(op: BuilderOp) -> bool {
    matches!(
        op,
        BuilderOp::AddImmFloat
            | BuilderOp::AddImmInt
            | BuilderOp::MulImmFloat
            | BuilderOp::MulImmInt
            | BuilderOp::BitwiseAndImmInt
            | BuilderOp::BitwiseXorImmInt
            | BuilderOp::MinImmFloat
            | BuilderOp::MaxImmFloat
            | BuilderOp::CmpleImmFloat
            | BuilderOp::CmpleImmInt
            | BuilderOp::CmpleImmUint
            | BuilderOp::CmpltImmFloat
            | BuilderOp::CmpltImmInt
            | BuilderOp::CmpltImmUint
            | BuilderOp::CmpeqImmFloat
            | BuilderOp::CmpeqImmInt
            | BuilderOp::CmpneImmFloat
            | BuilderOp::CmpneImmInt
    )
}

/// `is_multi_slot_immediate_op`: immediate ops that take up to four slots.
fn is_multi_slot_immediate_op(op: BuilderOp) -> bool {
    matches!(op, BuilderOp::BitwiseAndImmInt)
}

/// `convert_n_way_op_to_immediate`: rewrites an n-way op with a constant operand into its
/// immediate-mode twin, if one exists. `constant_value` may be changed (for `sub` as `add`).
fn convert_n_way_op_to_immediate(op: BuilderOp, slots: i32, constant_value: &mut i32) -> BuilderOp {
    // We rely on the exact ordering of SkRP ops here; the immediate-mode op must always come
    // directly before the n-way op. (If we have more than one, the increasing-slot variations
    // continue backwards from there.)
    let imm_op = BuilderOp::ALL[op as usize - 1];

    // Some immediate ops support multiple slots.
    if is_multi_slot_immediate_op(imm_op) {
        return imm_op;
    }

    // Most immediate ops only directly support a single slot. However, it's still faster to execute
    // `add_imm_int, add_imm_int` instead of `splat_2_ints, add_2_ints`, so we allow those
    // conversions as well.
    if slots <= 2 {
        if is_immediate_op(imm_op) {
            return imm_op;
        }

        // We also allow for immediate-mode subtraction, by adding a negative value.
        match op {
            BuilderOp::SubNInts => {
                if *constant_value == i32::MIN {
                    return op;
                }
                *constant_value *= -1;
                return BuilderOp::AddImmInt;
            }
            BuilderOp::SubNFloats => {
                // This negates the floating-point value by inverting its sign bit.
                *constant_value ^= i32::MIN;
                return BuilderOp::AddImmFloat;
            }
            _ => {}
        }
    }

    // We don't have an immediate-mode version of this op.
    op
}

/// `slot_ranges_overlap`.
fn slot_ranges_overlap(x: SlotRange, y: SlotRange) -> bool {
    x.index < y.index + y.count && y.index < x.index + x.count
}

/// A swizzle element (a slot index within the swizzle's 16 elements).
fn element(index: i32) -> i8 {
    i8::try_from(index).expect("a swizzle element is at most 15")
}

/// The number of components of a swizzle, as the instruction's immediate.
fn component_count(components: &[i8]) -> i32 {
    i32::try_from(components.len()).expect("a swizzle has at most 16 components")
}

/// `pack_nybbles`: packs up to 8 elements (each 0-15) into nybbles, in reverse order.
fn pack_nybbles(components: &[i8]) -> i32 {
    let mut packed = 0_i32;
    for &component in components.iter().rev() {
        debug_assert!((0..=0xF).contains(&component));
        packed <<= 4;
        packed |= i32::from(component);
    }
    packed
}

/// `Builder`: assembles a Raster Pipeline program from `SkSL`-level operations.
#[doc(alias = "SkSL::RP::Builder")]
#[derive(Debug, Default)]
pub struct Builder {
    instructions: Vec<Instruction>,
    num_labels: i32,
    execution_mask_writes_enabled: i32,
    current_stack_id: i32,
}

impl Builder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Builder::finish`: finalizes and returns a completed program.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1276-L1285 (chrome/m156)
    #[must_use]
    pub fn finish(
        self,
        num_value_slots: i32,
        num_uniform_slots: i32,
        num_immutable_slots: i32,
        debug_trace: Option<Arc<DebugTracePriv>>,
    ) -> Program {
        // Verify that calls to enableExecutionMaskWrites and disableExecutionMaskWrites are balanced.
        debug_assert_eq!(self.execution_mask_writes_enabled, 0);
        Program::new(
            self.instructions,
            num_value_slots,
            num_uniform_slots,
            num_immutable_slots,
            self.num_labels,
            debug_trace,
        )
    }

    /// `nextLabelID`: peels off a label ID for use in the program.
    #[doc(alias = "nextLabelID")]
    pub fn next_label_id(&mut self) -> i32 {
        let id = self.num_labels;
        self.num_labels += 1;
        id
    }

    /// `enableExecutionMaskWrites`.
    #[doc(alias = "enableExecutionMaskWrites")]
    pub fn enable_execution_mask_writes(&mut self) {
        self.execution_mask_writes_enabled += 1;
    }

    /// `disableExecutionMaskWrites`.
    #[doc(alias = "disableExecutionMaskWrites")]
    pub fn disable_execution_mask_writes(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.execution_mask_writes_enabled -= 1;
    }

    /// `executionMaskWritesAreEnabled`.
    #[doc(alias = "executionMaskWritesAreEnabled")]
    #[must_use]
    pub fn execution_mask_writes_are_enabled(&self) -> bool {
        self.execution_mask_writes_enabled > 0
    }

    /// `set_current_stack`.
    pub fn set_current_stack(&mut self, stack_id: i32) {
        self.current_stack_id = stack_id;
    }

    // ---- instruction helpers --------------------------------------------------------------

    /// `appendInstruction`.
    fn append_instruction(&mut self, op: BuilderOp, slots: SlotList, imm: [i32; 4]) {
        self.instructions.push(Instruction {
            op,
            slot_a: slots.a,
            slot_b: slots.b,
            imm_a: imm[0],
            imm_b: imm[1],
            imm_c: imm[2],
            imm_d: imm[3],
            stack_id: self.current_stack_id,
        });
    }

    /// `lastInstruction`: the index of the instruction `from_back` places before the end, if it
    /// is on the current stack.
    fn last_instruction(&self, from_back: usize) -> Option<usize> {
        let idx = self.last_instruction_on_any_stack(from_back)?;
        if self.instructions[idx].stack_id != self.current_stack_id {
            return None;
        }
        Some(idx)
    }

    /// `lastInstructionOnAnyStack`.
    fn last_instruction_on_any_stack(&self, from_back: usize) -> Option<usize> {
        self.instructions.len().checked_sub(from_back + 1)
    }

    /// `Instruction* lastInstruction` as a copy, for reads.
    fn last(&self, from_back: usize) -> Option<Instruction> {
        self.last_instruction(from_back)
            .map(|i| self.instructions[i])
    }

    /// `fInstructions.pop_back()`.
    fn pop_back(&mut self) {
        self.instructions.pop();
    }

    // ---- simple instructions --------------------------------------------------------------

    /// `init_lane_masks`.
    pub fn init_lane_masks(&mut self) {
        self.append_instruction(BuilderOp::InitLaneMasks, SlotList::NONE, [0; 4]);
    }

    /// `store_src_rg`.
    pub fn store_src_rg(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 2);
        self.append_instruction(BuilderOp::StoreSrcRg, SlotList::one(slots.index), [0; 4]);
    }

    /// `store_src`.
    pub fn store_src(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 4);
        self.append_instruction(BuilderOp::StoreSrc, SlotList::one(slots.index), [0; 4]);
    }

    /// `store_dst`.
    pub fn store_dst(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 4);
        self.append_instruction(BuilderOp::StoreDst, SlotList::one(slots.index), [0; 4]);
    }

    /// `store_device_xy01`.
    pub fn store_device_xy01(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 4);
        self.append_instruction(
            BuilderOp::StoreDeviceXy01,
            SlotList::one(slots.index),
            [0; 4],
        );
    }

    /// `load_src`.
    pub fn load_src(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 4);
        self.append_instruction(BuilderOp::LoadSrc, SlotList::one(slots.index), [0; 4]);
    }

    /// `load_dst`.
    pub fn load_dst(&mut self, slots: SlotRange) {
        debug_assert_eq!(slots.count, 4);
        self.append_instruction(BuilderOp::LoadDst, SlotList::one(slots.index), [0; 4]);
    }

    /// `label`: inserts a label into the instruction stream.
    pub fn label(&mut self, label_id: i32) {
        debug_assert!(label_id >= 0 && label_id < self.num_labels);

        // If the previous instruction was a branch to this label, it's a no-op; jumping to the very
        // next instruction is effectively meaningless.
        while let Some(idx) = self.last_instruction_on_any_stack(0) {
            let last = self.instructions[idx];
            let branches_to_label = matches!(
                last.op,
                BuilderOp::Jump
                    | BuilderOp::BranchIfAllLanesActive
                    | BuilderOp::BranchIfAnyLanesActive
                    | BuilderOp::BranchIfNoLanesActive
                    | BuilderOp::BranchIfNoActiveLanesOnStackTopEqual
            ) && last.imm_a == label_id;
            if !branches_to_label {
                break;
            }
            self.pop_back();
        }
        self.append_instruction(BuilderOp::Label, SlotList::NONE, [label_id, 0, 0, 0]);
    }

    /// `jump`: unconditionally branches to a label.
    pub fn jump(&mut self, label_id: i32) {
        debug_assert!(label_id >= 0 && label_id < self.num_labels);
        // The previous instruction was also `jump`, so this branch could never possibly occur.
        if self
            .last_instruction_on_any_stack(0)
            .is_some_and(|last| self.instructions[last].op == BuilderOp::Jump)
        {
            return;
        }
        self.append_instruction(BuilderOp::Jump, SlotList::NONE, [label_id, 0, 0, 0]);
    }

    /// `branch_if_all_lanes_active`: branches to a label if the execution mask is active in every
    /// lane.
    pub fn branch_if_all_lanes_active(&mut self, label_id: i32) {
        if !self.execution_mask_writes_are_enabled() {
            self.jump(label_id);
            return;
        }

        debug_assert!(label_id >= 0 && label_id < self.num_labels);
        if let Some(last) = self.last_instruction_on_any_stack(0) {
            let op = self.instructions[last].op;
            if op == BuilderOp::BranchIfAllLanesActive || op == BuilderOp::Jump {
                // The previous instruction was `jump` or `branch_if_all_lanes_active`, so this
                // branch could never possibly occur.
                return;
            }
        }
        self.append_instruction(
            BuilderOp::BranchIfAllLanesActive,
            SlotList::NONE,
            [label_id, 0, 0, 0],
        );
    }

    /// `branch_if_any_lanes_active`: branches to a label if the execution mask is active in any
    /// lane.
    pub fn branch_if_any_lanes_active(&mut self, label_id: i32) {
        if !self.execution_mask_writes_are_enabled() {
            self.jump(label_id);
            return;
        }

        debug_assert!(label_id >= 0 && label_id < self.num_labels);
        if let Some(last) = self.last_instruction_on_any_stack(0) {
            let op = self.instructions[last].op;
            if op == BuilderOp::BranchIfAnyLanesActive || op == BuilderOp::Jump {
                // The previous instruction was `jump` or `branch_if_any_lanes_active`, so this
                // branch could never possibly occur.
                return;
            }
        }
        self.append_instruction(
            BuilderOp::BranchIfAnyLanesActive,
            SlotList::NONE,
            [label_id, 0, 0, 0],
        );
    }

    /// `branch_if_no_lanes_active`: branches to a label if the execution mask is inactive across
    /// all lanes.
    pub fn branch_if_no_lanes_active(&mut self, label_id: i32) {
        if !self.execution_mask_writes_are_enabled() {
            return;
        }

        debug_assert!(label_id >= 0 && label_id < self.num_labels);
        if let Some(last) = self.last_instruction_on_any_stack(0) {
            let op = self.instructions[last].op;
            if op == BuilderOp::BranchIfNoLanesActive || op == BuilderOp::Jump {
                // The previous instruction was `jump` or `branch_if_no_lanes_active`, so this
                // branch could never possibly occur.
                return;
            }
        }
        self.append_instruction(
            BuilderOp::BranchIfNoLanesActive,
            SlotList::NONE,
            [label_id, 0, 0, 0],
        );
    }

    /// `branch_if_no_active_lanes_on_stack_top_equal`: branches to a label if the top value on the
    /// stack is _not_ equal to `value` in any lane.
    pub fn branch_if_no_active_lanes_on_stack_top_equal(&mut self, value: i32, label_id: i32) {
        debug_assert!(label_id >= 0 && label_id < self.num_labels);
        if let Some(last) = self.last_instruction_on_any_stack(0) {
            let last = self.instructions[last];
            if last.op == BuilderOp::Jump
                || (last.op == BuilderOp::BranchIfNoActiveLanesOnStackTopEqual
                    && last.imm_b == value)
            {
                // The previous instruction was `jump` or
                // `branch_if_no_active_lanes_on_stack_top_equal` (checking against the same
                // value), so this branch could never possibly occur.
                return;
            }
        }
        self.append_instruction(
            BuilderOp::BranchIfNoActiveLanesOnStackTopEqual,
            SlotList::NONE,
            [label_id, value, 0, 0],
        );
    }

    /// `push_constant_i`: pushes `count` copies of a 32-bit value (bitcast from int, float or
    /// uint).
    pub fn push_constant_i(&mut self, val: i32, count: i32) {
        debug_assert!(count >= 0);
        if count > 0 {
            if let Some(idx) = self.last_instruction(0) {
                // If the previous op is pushing the same value, we can just push more of them.
                let last = self.instructions[idx];
                if last.op == BuilderOp::PushConstant && last.imm_b == val {
                    self.instructions[idx].imm_a += count;
                    return;
                }
            }
            self.append_instruction(BuilderOp::PushConstant, SlotList::NONE, [count, val, 0, 0]);
        }
    }

    /// `push_zeros`.
    pub fn push_zeros(&mut self, count: i32) {
        self.push_constant_i(0, count);
    }

    /// `push_constant_f`.
    pub fn push_constant_f(&mut self, val: f32) {
        self.push_constant_i(val.to_bits().cast_signed(), 1);
    }

    /// `push_constant_u`.
    pub fn push_constant_u(&mut self, val: u32, count: i32) {
        self.push_constant_i(val.cast_signed(), count);
    }

    /// `store_immutable_value_i`: initializes an immutable slot with a constant when the program
    /// is first created. Adds no instruction to the stream.
    pub fn store_immutable_value_i(&mut self, slot: Slot, val: i32) {
        self.append_instruction(
            BuilderOp::StoreImmutableValue,
            SlotList::one(slot),
            [val, 0, 0, 0],
        );
    }

    /// `push_uniform`: translates into `copy_uniforms` (from uniforms into temp stack).
    pub fn push_uniform(&mut self, src: SlotRange) {
        debug_assert!(src.count >= 0);
        if let Some(idx) = self.last_instruction(0) {
            // If the previous instruction was pushing uniforms contiguous to this range, we can
            // collapse the two pushes into one larger push.
            let last = self.instructions[idx];
            if last.op == BuilderOp::PushUniform && last.slot_a + last.imm_a == src.index {
                self.instructions[idx].imm_a += src.count;
                return;
            }
        }

        if src.count > 0 {
            self.append_instruction(
                BuilderOp::PushUniform,
                SlotList::one(src.index),
                [src.count, 0, 0, 0],
            );
        }
    }

    /// `push_uniform_indirect`: translates into `copy_uniforms_indirect` (from uniforms into temp
    /// stack). `fixed_range` is pushed forward by the value at the top of `dynamic_stack`, capped
    /// by `limit_range`.
    pub fn push_uniform_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack: i32,
        limit_range: SlotRange,
    ) {
        self.append_instruction(
            BuilderOp::PushUniformIndirect,
            SlotList::two(fixed_range.index, limit_range.index + limit_range.count),
            [fixed_range.count, dynamic_stack, 0, 0],
        );
    }

    /// `copy_uniform_to_slots_unmasked`: translates into `copy_uniforms` (from uniforms into value
    /// slots).
    pub fn copy_uniform_to_slots_unmasked(&mut self, dst: SlotRange, src: SlotRange) {
        // If the last instruction copied adjacent uniforms, just extend it.
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            // If the last op is copy-constant...
            if last.op == BuilderOp::CopyUniformToSlotsUnmasked
                // and this op's destination is immediately after the last copy-constant's
                // destination...
                && last.slot_b + last.imm_a == dst.index
                // and this op's source is immediately after the last copy-constant's source...
                && last.slot_a + last.imm_a == src.index
            {
                // then we can just extend the copy!
                self.instructions[idx].imm_a += dst.count;
                return;
            }
        }

        debug_assert_eq!(dst.count, src.count);
        self.append_instruction(
            BuilderOp::CopyUniformToSlotsUnmasked,
            SlotList::two(src.index, dst.index),
            [dst.count, 0, 0, 0],
        );
    }

    /// `push_slots_or_immutable`: pushes a range of value or immutable slots.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L541-L583 (chrome/m156)
    pub fn push_slots_or_immutable(&mut self, mut src: SlotRange, op: BuilderOp) {
        debug_assert!(src.count >= 0);
        if let Some(idx) = self.last_instruction(0) {
            // If the previous instruction was pushing slots contiguous to this range, we can
            // collapse the two pushes into one larger push.
            let last = self.instructions[idx];
            if last.op == op && last.slot_a + last.imm_a == src.index {
                self.instructions[idx].imm_a += src.count;
                src.count = 0;
            }
        }

        if src.count > 0 {
            self.append_instruction(op, SlotList::one(src.index), [src.count, 0, 0, 0]);
        }

        // Look for a sequence of "copy stack to X, discard stack, copy X to stack". This is a
        // common pattern when multiple operations in a row affect the same variable. When we see
        // this, we can eliminate both the discard and the push.
        if let (Some(push), Some(discard), Some(copy)) = (self.last(0), self.last(1), self.last(2))
        {
            // Look for a `discard_stack` matching our push count, and a `copy_stack_to_slots`
            // matching our push.
            let matches_sequence = push.op == BuilderOp::PushSlots
                && discard.op == BuilderOp::DiscardStack
                && discard.imm_a == push.imm_a
                && (copy.op == BuilderOp::CopyStackToSlots
                    || copy.op == BuilderOp::CopyStackToSlotsUnmasked)
                && copy.slot_a == push.slot_a
                && copy.imm_a == push.imm_a;
            if matches_sequence {
                // We found a matching sequence. Remove the discard and push.
                self.pop_back();
                self.pop_back();
            }
        }
    }

    /// `push_slots`: translates into `copy_slots_unmasked` (from values into temp stack).
    pub fn push_slots(&mut self, src: SlotRange) {
        self.push_slots_or_immutable(src, BuilderOp::PushSlots);
    }

    /// `push_immutable`: translates into `copy_immutable_unmasked` (from immutables into temp stack).
    pub fn push_immutable(&mut self, src: SlotRange) {
        self.push_slots_or_immutable(src, BuilderOp::PushImmutable);
    }

    /// `push_slots_or_immutable_indirect`.
    pub fn push_slots_or_immutable_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack: i32,
        limit_range: SlotRange,
        op: BuilderOp,
    ) {
        // SlotA: fixed-range start. SlotB: limit-range end. immA: number of slots.
        // immB: dynamic stack ID.
        self.append_instruction(
            op,
            SlotList::two(fixed_range.index, limit_range.index + limit_range.count),
            [fixed_range.count, dynamic_stack, 0, 0],
        );
    }

    /// `push_slots_indirect`.
    pub fn push_slots_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack: i32,
        limit_range: SlotRange,
    ) {
        self.push_slots_or_immutable_indirect(
            fixed_range,
            dynamic_stack,
            limit_range,
            BuilderOp::PushSlotsIndirect,
        );
    }

    /// `push_immutable_indirect`.
    pub fn push_immutable_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack: i32,
        limit_range: SlotRange,
    ) {
        self.push_slots_or_immutable_indirect(
            fixed_range,
            dynamic_stack,
            limit_range,
            BuilderOp::PushImmutableIndirect,
        );
    }

    /// `copy_stack_to_slots_indirect`: copies from the temp stack to values, at an indirect offset.
    pub fn copy_stack_to_slots_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack_id: i32,
        limit_range: SlotRange,
    ) {
        self.append_instruction(
            BuilderOp::CopyStackToSlotsIndirect,
            SlotList::two(fixed_range.index, limit_range.index + limit_range.count),
            [fixed_range.count, dynamic_stack_id, 0, 0],
        );
    }

    /// `pop_slots_indirect`: copies from the temp stack to slots (with an indirect offset), then
    /// shrinks the temp stack.
    pub fn pop_slots_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack_id: i32,
        limit_range: SlotRange,
    ) {
        self.copy_stack_to_slots_indirect(fixed_range, dynamic_stack_id, limit_range);
        self.discard_stack(fixed_range.count);
    }

    /// `copy_stack_to_slots`: translates into `copy_slots_masked` (from temp stack to values).
    /// Does not discard any values on the temp stack.
    pub fn copy_stack_to_slots(&mut self, dst: SlotRange, offset_from_stack_top: i32) {
        // If the execution mask is known to be all-true, then we can ignore the write mask.
        if !self.execution_mask_writes_are_enabled() {
            self.copy_stack_to_slots_unmasked(dst, offset_from_stack_top);
            return;
        }

        // If the last instruction copied the previous stack slots, just extend it.
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            // If the last op is copy-stack-to-slots...
            if last.op == BuilderOp::CopyStackToSlots
                // and this op's destination is immediately after the last copy-slots-op's
                // destination...
                && last.slot_a + last.imm_a == dst.index
                // and this op's source is immediately after the last copy-slots-op's source...
                && last.imm_b - last.imm_a == offset_from_stack_top
            {
                // then we can just extend the copy!
                self.instructions[idx].imm_a += dst.count;
                return;
            }
        }

        self.append_instruction(
            BuilderOp::CopyStackToSlots,
            SlotList::one(dst.index),
            [dst.count, offset_from_stack_top, 0, 0],
        );
    }

    /// `swizzle_copy_stack_to_slots`: translates into `swizzle_copy_slots_masked` (from temp stack
    /// to values). Does not discard any values on the temp stack.
    pub fn swizzle_copy_stack_to_slots(
        &mut self,
        dst: SlotRange,
        components: &[i8],
        offset_from_stack_top: i32,
    ) {
        // SlotA: fixed-range start. immA: number of swizzle components. immB: swizzle components.
        // immC: offset from stack top.
        self.append_instruction(
            BuilderOp::SwizzleCopyStackToSlots,
            SlotList::one(dst.index),
            [
                component_count(components),
                pack_nybbles(components),
                offset_from_stack_top,
                0,
            ],
        );
    }

    /// `swizzle_copy_stack_to_slots_indirect`: translates into `swizzle_copy_to_indirect_masked`
    /// (from temp stack to values). Does not discard any values on the temp stack.
    pub fn swizzle_copy_stack_to_slots_indirect(
        &mut self,
        fixed_range: SlotRange,
        dynamic_stack_id: i32,
        limit_range: SlotRange,
        components: &[i8],
        offset_from_stack_top: i32,
    ) {
        // SlotA: fixed-range start. SlotB: limit-range end. immA: number of swizzle components.
        // immB: swizzle components. immC: offset from stack top. immD: dynamic stack ID.
        self.append_instruction(
            BuilderOp::SwizzleCopyStackToSlotsIndirect,
            SlotList::two(fixed_range.index, limit_range.index + limit_range.count),
            [
                component_count(components),
                pack_nybbles(components),
                offset_from_stack_top,
                dynamic_stack_id,
            ],
        );
    }

    /// `copy_stack_to_slots_unmasked`: translates into `copy_slots_unmasked` (from temp stack to
    /// values). Does not discard any values on the temp stack.
    pub fn copy_stack_to_slots_unmasked(&mut self, dst: SlotRange, offset_from_stack_top: i32) {
        // If the last instruction copied the previous stack slots, just extend it.
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            // If the last op is copy-stack-to-slots-unmasked...
            if last.op == BuilderOp::CopyStackToSlotsUnmasked
                // and this op's destination is immediately after the last copy-slots-op's
                // destination...
                && last.slot_a + last.imm_a == dst.index
                // and this op's source is immediately after the last copy-slots-op's source...
                && last.imm_b - last.imm_a == offset_from_stack_top
            {
                // then we can just extend the copy!
                self.instructions[idx].imm_a += dst.count;
                return;
            }
        }

        self.append_instruction(
            BuilderOp::CopyStackToSlotsUnmasked,
            SlotList::one(dst.index),
            [dst.count, offset_from_stack_top, 0, 0],
        );
    }

    /// `pop_slots_unmasked`: the opposite of `push_slots`; copies values from the temp stack into
    /// value slots, then shrinks the temp stack.
    pub fn pop_slots_unmasked(&mut self, dst: SlotRange) {
        debug_assert!(dst.count >= 0);
        self.copy_stack_to_slots_unmasked(dst, dst.count);
        self.discard_stack(dst.count);
    }

    /// `pop_slots`: copies values from the temp stack into slots, then shrinks the temp stack.
    pub fn pop_slots(&mut self, dst: SlotRange) {
        if !self.execution_mask_writes_are_enabled() {
            self.pop_slots_unmasked(dst);
            return;
        }

        self.copy_stack_to_slots(dst, dst.count);
        self.discard_stack(dst.count);
    }

    /// `unary_op`: performs a unary op (like `bitwise_not`), given a slot count of `slots`. The
    /// stack top is replaced with the result.
    pub fn unary_op(&mut self, op: BuilderOp, slots: i32) {
        match op {
            BuilderOp::AcosFloat
            | BuilderOp::AsinFloat
            | BuilderOp::AtanFloat
            | BuilderOp::CosFloat
            | BuilderOp::ExpFloat
            | BuilderOp::Exp2Float
            | BuilderOp::LogFloat
            | BuilderOp::Log2Float
            | BuilderOp::SinFloat
            | BuilderOp::SqrtFloat
            | BuilderOp::TanFloat
            | BuilderOp::AbsInt
            | BuilderOp::CastToFloatFromInt
            | BuilderOp::CastToFloatFromUint
            | BuilderOp::CastToIntFromFloat
            | BuilderOp::CastToUintFromFloat
            | BuilderOp::CeilFloat
            | BuilderOp::FloorFloat
            | BuilderOp::InvsqrtFloat => {
                self.append_instruction(op, SlotList::NONE, [slots, 0, 0, 0]);
            }
            _ => debug_assert!(false, "not a unary op"),
        }
    }

    /// `binary_op`: performs a binary op (like `add_n_floats` or `cmpeq_n_ints`), given a slot
    /// count of `slots`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L229-L256 (chrome/m156)
    pub fn binary_op(&mut self, op: BuilderOp, slots: i32) {
        if let Some(idx) = self.last_instruction(0) {
            // If we just pushed or splatted a constant onto the stack...
            let last = self.instructions[idx];
            if last.op == BuilderOp::PushConstant && last.imm_a >= slots {
                // ... and this op has an immediate-mode equivalent...
                let mut constant_value = last.imm_b;
                let imm_op = convert_n_way_op_to_immediate(op, slots, &mut constant_value);
                if imm_op != op {
                    // ... discard the constants from the stack, and use an immediate-mode op.
                    self.discard_stack(slots);
                    self.append_instruction(imm_op, SlotList::NONE, [slots, constant_value, 0, 0]);
                    return;
                }
            }
        }

        match op {
            BuilderOp::Atan2NFloats
            | BuilderOp::PowNFloats
            | BuilderOp::AddNFloats
            | BuilderOp::AddNInts
            | BuilderOp::SubNFloats
            | BuilderOp::SubNInts
            | BuilderOp::MulNFloats
            | BuilderOp::MulNInts
            | BuilderOp::DivNFloats
            | BuilderOp::DivNInts
            | BuilderOp::DivNUints
            | BuilderOp::BitwiseAndNInts
            | BuilderOp::BitwiseOrNInts
            | BuilderOp::BitwiseXorNInts
            | BuilderOp::ModNFloats
            | BuilderOp::MinNFloats
            | BuilderOp::MinNInts
            | BuilderOp::MinNUints
            | BuilderOp::MaxNFloats
            | BuilderOp::MaxNInts
            | BuilderOp::MaxNUints
            | BuilderOp::CmpleNFloats
            | BuilderOp::CmpleNInts
            | BuilderOp::CmpleNUints
            | BuilderOp::CmpltNFloats
            | BuilderOp::CmpltNInts
            | BuilderOp::CmpltNUints
            | BuilderOp::CmpeqNFloats
            | BuilderOp::CmpeqNInts
            | BuilderOp::CmpneNFloats
            | BuilderOp::CmpneNInts => {
                self.append_instruction(op, SlotList::NONE, [slots, 0, 0, 0]);
            }
            _ => debug_assert!(false, "not a binary op"),
        }
    }

    /// `ternary_op`: performs a ternary op (like `mix` or `smoothstep`), given a slot count of
    /// `slots`.
    pub fn ternary_op(&mut self, op: BuilderOp, slots: i32) {
        match op {
            BuilderOp::SmoothstepNFloats | BuilderOp::MixNFloats | BuilderOp::MixNInts => {
                self.append_instruction(op, SlotList::NONE, [slots, 0, 0, 0]);
            }
            _ => debug_assert!(false, "not a ternary op"),
        }
    }

    /// `dot_floats`: computes a dot product on the stack (1 to 4 slots).
    pub fn dot_floats(&mut self, slots: i32) {
        match slots {
            1 => self.append_instruction(BuilderOp::MulNFloats, SlotList::NONE, [slots, 0, 0, 0]),
            2 => self.append_instruction(BuilderOp::Dot2Floats, SlotList::NONE, [slots, 0, 0, 0]),
            3 => self.append_instruction(BuilderOp::Dot3Floats, SlotList::NONE, [slots, 0, 0, 0]),
            4 => self.append_instruction(BuilderOp::Dot4Floats, SlotList::NONE, [slots, 0, 0, 0]),
            _ => debug_assert!(false, "invalid number of slots"),
        }
    }

    /// `refract_floats`: computes refract(N, I, eta) on the stack.
    pub fn refract_floats(&mut self) {
        self.append_instruction(BuilderOp::Refract4Floats, SlotList::NONE, [0; 4]);
    }

    /// `inverse_matrix`: computes inverse(`matN`) on the stack, for n of 2, 3 or 4.
    pub fn inverse_matrix(&mut self, n: i32) {
        match n {
            2 => self.append_instruction(BuilderOp::InverseMat2, SlotList::NONE, [4, 0, 0, 0]),
            3 => self.append_instruction(BuilderOp::InverseMat3, SlotList::NONE, [9, 0, 0, 0]),
            4 => self.append_instruction(BuilderOp::InverseMat4, SlotList::NONE, [16, 0, 0, 0]),
            _ => unreachable!("inverse_matrix: unsupported matrix size"),
        }
    }

    /// `pad_stack`: grows the temp stack, leaving any preexisting values in place.
    pub fn pad_stack(&mut self, count: i32) {
        if count > 0 {
            self.append_instruction(BuilderOp::PadStack, SlotList::NONE, [count, 0, 0, 0]);
        }
    }

    /// `simplifyImmediateUnmaskedOp`: a `push, immediate-op, unmasked pop` pattern becomes an
    /// immediate-op directly on the value slots. Returns whether it simplified.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L303-L345 (chrome/m156)
    fn simplify_immediate_unmasked_op(&mut self) -> bool {
        let len = self.instructions.len();
        if len < 3 {
            return false;
        }

        // If the last instruction is an unmasked pop, and the prior instruction was an
        // immediate-mode op, and the one before that pushed slots...
        let pop = self.instructions[len - 1];
        let imm = self.instructions[len - 2];
        let push = self.instructions[len - 3];
        if pop.op != BuilderOp::CopyStackToSlotsUnmasked {
            return false;
        }
        // ... with the same number of slots...
        if !is_immediate_op(imm.op) || imm.imm_a != pop.imm_a {
            return false;
        }
        // ... and we support multiple-slot immediates (if this op calls for it)...
        if imm.imm_a != 1 && !is_multi_slot_immediate_op(imm.op) {
            return false;
        }
        // ... and the prior instruction was `push_slots` or `push_immutable` of at least that
        // many slots...
        if !((push.op == BuilderOp::PushSlots || push.op == BuilderOp::PushImmutable)
            && push.imm_a >= pop.imm_a)
        {
            return false;
        }
        // ... onto the same slot range...
        let imm_slot = pop.slot_a + pop.imm_a;
        let push_slot = push.slot_a + push.imm_a;
        if imm_slot != push_slot {
            return false;
        }

        // ... we can shrink the push, eliminate the pop, and perform the immediate op in-place
        // instead.
        self.instructions[len - 3].imm_a -= imm.imm_a;
        self.instructions[len - 2].slot_a = imm_slot - imm.imm_a;
        self.pop_back();
        true
    }

    /// `discard_stack`: shrinks the temp stack, discarding values on top, on `stack_id`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L347-L434 (chrome/m156)
    pub fn discard_stack_on_stack(&mut self, mut count: i32, stack_id: i32) {
        // If we pushed something onto the stack and then immediately discarded part of it, we can
        // shrink or eliminate the push.
        while count > 0 {
            let Some(idx) = self.last_instruction_on_any_stack(0) else {
                break;
            };
            let last = self.instructions[idx];
            if last.stack_id != stack_id {
                break;
            }

            match last.op {
                BuilderOp::DiscardStack => {
                    // Our last op was actually a separate discard_stack; combine the discards.
                    self.instructions[idx].imm_a += count;
                    return;
                }

                BuilderOp::PushClone
                | BuilderOp::PushCloneFromStack
                | BuilderOp::PushCloneIndirectFromStack
                | BuilderOp::PushConstant
                | BuilderOp::PushImmutable
                | BuilderOp::PushImmutableIndirect
                | BuilderOp::PushSlots
                | BuilderOp::PushSlotsIndirect
                | BuilderOp::PushUniform
                | BuilderOp::PushUniformIndirect
                | BuilderOp::PadStack => {
                    // Our last op was a multi-slot push; these cancel out. Eliminate the op if its
                    // count reached zero.
                    let cancel_out = count.min(last.imm_a);
                    count -= cancel_out;
                    self.instructions[idx].imm_a -= cancel_out;
                    if self.instructions[idx].imm_a == 0 {
                        self.pop_back();
                    }
                }

                BuilderOp::PushConditionMask
                | BuilderOp::PushLoopMask
                | BuilderOp::PushReturnMask => {
                    // Our last op was a single-slot push; cancel out one discard and eliminate
                    // the op.
                    count -= 1;
                    self.pop_back();
                }

                BuilderOp::CopyStackToSlotsUnmasked => {
                    // Look for a pattern of `push, immediate-ops, pop` and simplify it down to an
                    // immediate-op directly to the value slot.
                    if count == last.imm_a && self.simplify_immediate_unmasked_op() {
                        return;
                    }

                    // A `copy_stack_to_slots_unmasked` op, followed immediately by a
                    // `discard_stack` op with an equal number of slots, is interpreted as an
                    // unmasked stack pop. We can simplify pops in a variety of ways. First,
                    // temporarily get rid of `copy_stack_to_slots_unmasked`.
                    if count == last.imm_a {
                        let mut dst = SlotRange {
                            index: last.slot_a,
                            count: last.imm_a,
                        };
                        self.pop_back();

                        // See if we can write this pop in a simpler way.
                        self.simplify_pop_slots_unmasked(&mut dst);

                        // If simplification consumed the entire range, we're done!
                        if dst.count == 0 {
                            return;
                        }

                        // Simplification did not consume the entire range. We are still
                        // responsible for copying-back and discarding any remaining slots.
                        self.copy_stack_to_slots_unmasked(dst, dst.count);
                        count = dst.count;
                    }
                    break;
                }

                // This instruction wasn't a push.
                _ => break,
            }
        }

        if count > 0 {
            self.append_instruction(BuilderOp::DiscardStack, SlotList::NONE, [count, 0, 0, 0]);
        }
    }

    /// `discard_stack`: shrinks the current temp stack, discarding values on top.
    pub fn discard_stack(&mut self, count: i32) {
        self.discard_stack_on_stack(count, self.current_stack_id);
    }

    /// `label`/`jump`/`branch_*` are above; this is `simplifyPopSlotsUnmasked`: consumes slots
    /// from the top of the stack that can be written in a simpler way.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L748-L833 (chrome/m156)
    fn simplify_pop_slots_unmasked(&mut self, dst: &mut SlotRange) {
        if dst.count == 0 {
            // There's nothing left to simplify.
            return;
        }
        let Some(idx) = self.last_instruction(0) else {
            // There's nothing left to simplify.
            return;
        };
        let last = self.instructions[idx];
        let last_op = last.op;

        // If the last instruction is pushing a constant, we can simplify it by copying the
        // constant directly into the destination slot.
        if last_op == BuilderOp::PushConstant {
            // Get the last slot.
            let value = last.imm_b;
            self.instructions[idx].imm_a -= 1;
            if self.instructions[idx].imm_a == 0 {
                self.pop_back();
            }

            // Consume one destination slot.
            dst.count -= 1;
            let destination_slot = dst.index + dst.count;

            // Continue simplifying if possible.
            self.simplify_pop_slots_unmasked(dst);

            // Write the constant directly to the destination slot.
            self.copy_constant(destination_slot, value);
            return;
        }

        // If the last instruction is pushing a uniform, we can simplify it by copying the uniform
        // directly into the destination slot.
        if last_op == BuilderOp::PushUniform {
            // Get the last slot.
            let source_slot = last.slot_a + last.imm_a - 1;
            self.instructions[idx].imm_a -= 1;
            if self.instructions[idx].imm_a == 0 {
                self.pop_back();
            }

            // Consume one destination slot.
            dst.count -= 1;
            let destination_slot = dst.index + dst.count;

            // Continue simplifying if possible.
            self.simplify_pop_slots_unmasked(dst);

            // Write the uniform directly to the destination slot.
            self.copy_uniform_to_slots_unmasked(
                SlotRange {
                    index: destination_slot,
                    count: 1,
                },
                SlotRange {
                    index: source_slot,
                    count: 1,
                },
            );
            return;
        }

        // If the last instruction is pushing a slot or immutable, we can just copy that slot.
        if last_op == BuilderOp::PushSlots || last_op == BuilderOp::PushImmutable {
            // Get the last slot.
            let source_slot = last.slot_a + last.imm_a - 1;
            self.instructions[idx].imm_a -= 1;
            if self.instructions[idx].imm_a == 0 {
                self.pop_back();
            }

            // Consume one destination slot.
            dst.count -= 1;
            let destination_slot = dst.index + dst.count;

            // Try once more.
            self.simplify_pop_slots_unmasked(dst);

            // Copy the slot directly.
            if last_op == BuilderOp::PushSlots {
                if destination_slot != source_slot {
                    self.copy_slots_unmasked(
                        SlotRange {
                            index: destination_slot,
                            count: 1,
                        },
                        SlotRange {
                            index: source_slot,
                            count: 1,
                        },
                    );
                }
                // Copying from a value-slot into the same value-slot is a no-op.
            } else {
                // Copy from immutable data directly to the destination slot.
                self.copy_immutable_unmasked(
                    SlotRange {
                        index: destination_slot,
                        count: 1,
                    },
                    SlotRange {
                        index: source_slot,
                        count: 1,
                    },
                );
            }
        }
    }

    /// `push_duplicates`: creates many clones of the top single-slot item on the temp stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L659-L684 (chrome/m156)
    pub fn push_duplicates(&mut self, mut count: i32) {
        if let Some(idx) = self.last_instruction(0) {
            // If the previous op is pushing a constant, we can just push more of them.
            if self.instructions[idx].op == BuilderOp::PushConstant {
                self.instructions[idx].imm_a += count;
                return;
            }
        }
        debug_assert!(count >= 0);
        if count >= 3 {
            // Use a swizzle to splat the input into a 4-slot value.
            self.swizzle(1, &[0, 0, 0, 0]);
            count -= 3;
        }
        while count >= 4 {
            // Clone the splatted value four slots at a time.
            self.push_clone(4, 0);
            count -= 4;
        }
        // Use a swizzle or clone to handle the trailing items.
        match count {
            3 => self.swizzle(1, &[0, 0, 0, 0]),
            2 => self.swizzle(1, &[0, 0, 0]),
            1 => self.push_clone(1, 0),
            _ => {}
        }
    }

    /// `push_clone`: creates a single clone of an item on the current temp stack, `num_slots`
    /// wide, taken `offset_from_stack_top` slots below the top.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L686-L699 (chrome/m156)
    pub fn push_clone(&mut self, num_slots: i32, offset_from_stack_top: i32) {
        // If we are cloning the stack top, and the previous op is pushing a constant, we can
        // just push more of them.
        if let Some(idx) = self.last_instruction(0).filter(|&idx| {
            num_slots == 1
                && offset_from_stack_top == 0
                && self.instructions[idx].op == BuilderOp::PushConstant
        }) {
            self.instructions[idx].imm_a += 1;
            return;
        }
        self.append_instruction(
            BuilderOp::PushClone,
            SlotList::NONE,
            [num_slots, num_slots + offset_from_stack_top, 0, 0],
        );
    }

    /// `push_clone_from_stack`: clones a range of slots from another stack onto this stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L701-L722 (chrome/m156)
    pub fn push_clone_from_stack(
        &mut self,
        range: SlotRange,
        other_stack_id: i32,
        offset_from_stack_top: i32,
    ) {
        let offset_from_stack_top = offset_from_stack_top - range.index;

        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            // If the previous op is also pushing a clone from the same stack, and this clone
            // starts at the same place that the last clone ends, just extend the existing clone.
            if last.op == BuilderOp::PushCloneFromStack
                && last.imm_b == other_stack_id
                && last.imm_c - last.imm_a == offset_from_stack_top
            {
                self.instructions[idx].imm_a += range.count;
                return;
            }
        }

        self.append_instruction(
            BuilderOp::PushCloneFromStack,
            SlotList::NONE,
            [range.count, other_stack_id, offset_from_stack_top, 0],
        );
    }

    /// `push_clone_indirect_from_stack`: translates into `copy_from_indirect_unmasked` (from one
    /// temp stack to another).
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L724-L736 (chrome/m156)
    pub fn push_clone_indirect_from_stack(
        &mut self,
        fixed_offset: SlotRange,
        dynamic_stack_id: i32,
        other_stack_id: i32,
        offset_from_stack_top: i32,
    ) {
        let offset_from_stack_top = offset_from_stack_top - fixed_offset.index;
        self.append_instruction(
            BuilderOp::PushCloneIndirectFromStack,
            SlotList::NONE,
            [
                fixed_offset.count,
                other_stack_id,
                offset_from_stack_top,
                dynamic_stack_id,
            ],
        );
    }

    /// `select`: overlays the top two entries on the stack, making one hybrid entry. The
    /// execution mask selects which lanes are preserved.
    pub fn select(&mut self, slots: i32) {
        debug_assert!(slots > 0);
        self.append_instruction(BuilderOp::Select, SlotList::NONE, [slots, 0, 0, 0]);
    }

    /// `case_op`: compares the stack top with `value`; if it matches, enables the loop mask.
    pub fn case_op(&mut self, value: i32) {
        self.append_instruction(BuilderOp::CaseOp, SlotList::NONE, [value, 0, 0, 0]);
    }

    /// `continue_op`: performs a `continue` in a loop.
    pub fn continue_op(&mut self, continue_mask_stack_id: i32) {
        self.append_instruction(
            BuilderOp::ContinueOp,
            SlotList::NONE,
            [continue_mask_stack_id, 0, 0, 0],
        );
    }

    /// `exchange_src`: exchanges src.rgba with the four values at the top of the stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L841-L852 (chrome/m156)
    pub fn exchange_src(&mut self) {
        if let Some(idx) = self.last_instruction(0) {
            // If the previous op is also an exchange-src, both ops can be eliminated. A double
            // swap is a no-op.
            if self.instructions[idx].op == BuilderOp::ExchangeSrc {
                self.pop_back();
                return;
            }
        }
        self.append_instruction(BuilderOp::ExchangeSrc, SlotList::NONE, [0; 4]);
    }

    /// `pop_src_rgba`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L854-L866 (chrome/m156)
    pub fn pop_src_rgba(&mut self) {
        if let Some(idx) = self.last_instruction(0) {
            // If the previous op is exchanging src.rgba with the stack, both ops can be
            // eliminated. It's just sliding the color back and forth.
            if self.instructions[idx].op == BuilderOp::ExchangeSrc {
                self.pop_back();
                self.discard_stack(4);
                return;
            }
        }
        self.append_instruction(BuilderOp::PopSrcRgba, SlotList::NONE, [0; 4]);
    }

    /// `pop_dst_rgba`.
    pub fn pop_dst_rgba(&mut self) {
        self.append_instruction(BuilderOp::PopDstRgba, SlotList::NONE, [0; 4]);
    }

    /// `push_src_rgba`.
    pub fn push_src_rgba(&mut self) {
        self.append_instruction(BuilderOp::PushSrcRgba, SlotList::NONE, [0; 4]);
    }

    /// `push_dst_rgba`.
    pub fn push_dst_rgba(&mut self) {
        self.append_instruction(BuilderOp::PushDstRgba, SlotList::NONE, [0; 4]);
    }

    /// `push_device_xy01`.
    pub fn push_device_xy01(&mut self) {
        self.append_instruction(BuilderOp::PushDeviceXy01, SlotList::NONE, [0; 4]);
    }

    /// `copy_constant`: directly writes a constant value into a slot.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L911-L927 (chrome/m156)
    pub fn copy_constant(&mut self, slot: Slot, constant_value: i32) {
        // If the last op is copy-constant with the same value, and the slot is immediately after
        // its destination, then we can extend the copy.
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            if last.op == BuilderOp::CopyConstant
                && last.imm_b == constant_value
                && last.slot_a + last.imm_a == slot
            {
                self.instructions[idx].imm_a += 1;
                return;
            }
        }

        self.append_instruction(
            BuilderOp::CopyConstant,
            SlotList::one(slot),
            [1, constant_value, 0, 0],
        );
    }

    /// `copy_slots_masked`.
    pub fn copy_slots_masked(&mut self, dst: SlotRange, src: SlotRange) {
        debug_assert_eq!(dst.count, src.count);
        self.append_instruction(
            BuilderOp::CopySlotMasked,
            SlotList::two(dst.index, src.index),
            [dst.count, 0, 0, 0],
        );
    }

    /// `copy_slots_unmasked`: copies adjacent slots, extending the previous copy when possible.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L929-L949 (chrome/m156)
    pub fn copy_slots_unmasked(&mut self, dst: SlotRange, src: SlotRange) {
        // If the last instruction copied adjacent slots, just extend it.
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            // If the last op is a match, and this op's destination and source both immediately
            // follow the last copy's, and the source/dest ranges will not overlap...
            if last.op == BuilderOp::CopySlotUnmasked
                && last.slot_a + last.imm_a == dst.index
                && last.slot_b + last.imm_a == src.index
                && !slot_ranges_overlap(
                    SlotRange {
                        index: last.slot_b,
                        count: last.imm_a + dst.count,
                    },
                    SlotRange {
                        index: last.slot_a,
                        count: last.imm_a + dst.count,
                    },
                )
            {
                // then we can just extend the copy!
                self.instructions[idx].imm_a += dst.count;
                return;
            }
        }

        debug_assert_eq!(dst.count, src.count);
        self.append_instruction(
            BuilderOp::CopySlotUnmasked,
            SlotList::two(dst.index, src.index),
            [dst.count, 0, 0, 0],
        );
    }

    /// `copy_immutable_unmasked`: copies adjacent immutable data, extending the previous copy
    /// when possible.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L951-L968 (chrome/m156)
    pub fn copy_immutable_unmasked(&mut self, dst: SlotRange, src: SlotRange) {
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            if last.op == BuilderOp::CopyImmutableUnmasked
                && last.slot_a + last.imm_a == dst.index
                && last.slot_b + last.imm_a == src.index
            {
                self.instructions[idx].imm_a += dst.count;
                return;
            }
        }

        debug_assert_eq!(dst.count, src.count);
        self.append_instruction(
            BuilderOp::CopyImmutableUnmasked,
            SlotList::two(dst.index, src.index),
            [dst.count, 0, 0, 0],
        );
    }

    /// `zero_slots_unmasked`: stores zeros across the entire slot range.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1053-L1074 (chrome/m156)
    pub fn zero_slots_unmasked(&mut self, dst: SlotRange) {
        if let Some(idx) = self.last_instruction(0) {
            let last = self.instructions[idx];
            if last.op == BuilderOp::CopyConstant && last.imm_b == 0 {
                if last.slot_a + last.imm_a == dst.index {
                    // The previous instruction was zeroing the range immediately before this
                    // range. Combine the ranges.
                    self.instructions[idx].imm_a += dst.count;
                    return;
                }

                if last.slot_a == dst.index + dst.count {
                    // The previous instruction was zeroing the range immediately after this
                    // range. Combine the ranges.
                    self.instructions[idx].slot_a = dst.index;
                    self.instructions[idx].imm_a += dst.count;
                    return;
                }
            }
        }

        self.append_instruction(
            BuilderOp::CopyConstant,
            SlotList::one(dst.index),
            [dst.count, 0, 0, 0],
        );
    }

    /// `swizzle`: consumes `consumed_slots` elements on the stack, then generates
    /// `components.len()` elements.
    ///
    /// # Panics
    ///
    /// Panics if there are more than 16 components.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1143-L1202 (chrome/m156)
    pub fn swizzle(&mut self, mut consumed_slots: i32, components: &[i8]) {
        debug_assert!(consumed_slots >= 0);

        // We only allow up to 16 elements, and they can only reach 0-15 slots, due to nybble
        // packing.
        let mut num_elements = components.len();
        assert!(num_elements <= 16, "swizzle: too many elements");
        debug_assert!(components.iter().all(|&e| (0..=0xF).contains(&e)));

        // Make a local copy of the element array.
        let mut elements = [0_i8; 16];
        elements[..num_elements].copy_from_slice(components);

        while num_elements > 0 {
            // If the first element of the swizzle is zero, and zero isn't used elsewhere in the
            // swizzle, we can omit the first slot from the swizzle entirely.
            if elements[0] != 0 {
                break;
            }
            if elements[1..num_elements].contains(&0) {
                break;
            }
            // Slide everything forward by one slot, and reduce the element index by one.
            for index in 1..num_elements {
                elements[index - 1] = elements[index] - 1;
            }
            elements[num_elements - 1] = 0;
            consumed_slots -= 1;
            num_elements -= 1;
        }

        // A completely empty swizzle is a discard.
        if num_elements == 0 {
            self.discard_stack(consumed_slots);
            return;
        }

        if consumed_slots <= 4 && num_elements <= 4 {
            // We can fit everything into a little swizzle.
            let op = BuilderOp::ALL[BuilderOp::Swizzle1 as usize + num_elements - 1];
            self.append_instruction(
                op,
                SlotList::NONE,
                [
                    consumed_slots,
                    pack_nybbles(&elements[..num_elements]),
                    0,
                    0,
                ],
            );
            return;
        }

        // This is a big swizzle. We use the `shuffle` op to handle these. immA counts the consumed
        // slots. immB counts the generated slots. immC and immD hold packed-nybble shuffle values.
        self.append_instruction(
            BuilderOp::Shuffle,
            SlotList::NONE,
            [
                consumed_slots,
                component_count(&elements[..num_elements]),
                pack_nybbles(&elements[..8]),
                pack_nybbles(&elements[8..]),
            ],
        );
    }

    /// `transpose`: transposes a matrix of size `CxR` on the stack (into a matrix of size `RxC`).
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1204-L1214 (chrome/m156)
    pub fn transpose(&mut self, columns: i32, rows: i32) {
        let mut elements = [0_i8; 16];
        let mut index = 0;
        for r in 0..rows {
            for c in 0..columns {
                elements[index] = element(c * rows + r);
                index += 1;
            }
        }
        self.swizzle(columns * rows, &elements[..index]);
    }

    /// `diagonal_matrix`: generates a `CxR` diagonal matrix from the top two scalars on the stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1216-L1226 (chrome/m156)
    pub fn diagonal_matrix(&mut self, columns: i32, rows: i32) {
        let mut elements = [0_i8; 16];
        let mut index = 0;
        for c in 0..columns {
            for r in 0..rows {
                elements[index] = i8::from(c == r);
                index += 1;
            }
        }
        self.swizzle(2, &elements[..index]);
    }

    /// `matrix_resize`: resizes a `CxR` matrix at the top of the stack to C'`xR`'.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1228-L1262 (chrome/m156)
    pub fn matrix_resize(
        &mut self,
        orig_columns: i32,
        orig_rows: i32,
        new_columns: i32,
        new_rows: i32,
    ) {
        let mut elements = [0_i8; 16];
        let mut index = 0;

        let mut consumed_slots = orig_columns * orig_rows;
        let mut zero_offset = 0;
        let mut one_offset = 0;

        for c in 0..new_columns {
            for r in 0..new_rows {
                if c < orig_columns && r < orig_rows {
                    // Push an element from the original matrix.
                    elements[index] = element(c * orig_rows + r);
                } else if c == r {
                    // This element is outside the original matrix; push 1. We need to synthesize
                    // a literal 1.
                    if one_offset == 0 {
                        self.push_constant_f(1.0);
                        one_offset = consumed_slots;
                        consumed_slots += 1;
                    }
                    elements[index] = element(one_offset);
                } else {
                    // This element is outside the original matrix; push 0. We need to synthesize
                    // a literal 0.
                    if zero_offset == 0 {
                        self.push_constant_f(0.0);
                        zero_offset = consumed_slots;
                        consumed_slots += 1;
                    }
                    elements[index] = element(zero_offset);
                }
                index += 1;
            }
        }
        self.swizzle(consumed_slots, &elements[..index]);
    }

    /// `matrix_multiply`: multiplies a `CxR` matrix/vector against an adjacent `CxR` matrix/vector on
    /// the stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1264-L1274 (chrome/m156)
    pub fn matrix_multiply(
        &mut self,
        left_columns: i32,
        left_rows: i32,
        right_columns: i32,
        right_rows: i32,
    ) {
        let op = match left_columns {
            2 => BuilderOp::MatrixMultiply2,
            3 => BuilderOp::MatrixMultiply3,
            4 => BuilderOp::MatrixMultiply4,
            _ => {
                debug_assert!(false, "unsupported matrix dimensions");
                return;
            }
        };
        self.append_instruction(
            op,
            SlotList::NONE,
            [left_columns, left_rows, right_columns, right_rows],
        );
    }

    /// `invoke_shader`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1643-L1645 (chrome/m156)
    pub fn invoke_shader(&mut self, child_idx: i32) {
        self.append_instruction(
            BuilderOp::InvokeShader,
            SlotList::NONE,
            [child_idx, 0, 0, 0],
        );
    }

    /// `invoke_color_filter`.
    pub fn invoke_color_filter(&mut self, child_idx: i32) {
        self.append_instruction(
            BuilderOp::InvokeColorFilter,
            SlotList::NONE,
            [child_idx, 0, 0, 0],
        );
    }

    /// `invoke_blender`.
    pub fn invoke_blender(&mut self, child_idx: i32) {
        self.append_instruction(
            BuilderOp::InvokeBlender,
            SlotList::NONE,
            [child_idx, 0, 0, 0],
        );
    }

    /// `invoke_to_linear_srgb`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1655-L1661 (chrome/m156)
    pub fn invoke_to_linear_srgb(&mut self) {
        // The intrinsics accept a three-component value; add a fourth padding element (which will
        // be ignored) since our RP ops deal in RGBA colors.
        self.pad_stack(1);
        self.append_instruction(BuilderOp::InvokeToLinearSrgb, SlotList::NONE, [0; 4]);
        self.discard_stack(1);
    }

    /// `invoke_from_linear_srgb`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1663-L1669 (chrome/m156)
    pub fn invoke_from_linear_srgb(&mut self) {
        self.pad_stack(1);
        self.append_instruction(BuilderOp::InvokeFromLinearSrgb, SlotList::NONE, [0; 4]);
        self.discard_stack(1);
    }

    /// `pop_condition_mask`.
    pub fn pop_condition_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::PopConditionMask, SlotList::NONE, [0; 4]);
    }

    /// `push_condition_mask`: if the previous op popped the condition mask, restores it onto the
    /// stack "for free" instead of copying it.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1023-L1035 (chrome/m156)
    pub fn push_condition_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        if self
            .last_instruction(0)
            .is_some_and(|last| self.instructions[last].op == BuilderOp::PopConditionMask)
        {
            self.pad_stack(1);
            return;
        }
        self.append_instruction(BuilderOp::PushConditionMask, SlotList::NONE, [0; 4]);
    }

    /// `merge_condition_mask`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1037-L1051 (chrome/m156)
    pub fn merge_condition_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());

        // This instruction is going to overwrite the condition mask. If the previous instruction
        // was loading the condition mask, that's wasted work and it can be eliminated.
        if let Some(idx) = self
            .last_instruction_on_any_stack(0)
            .filter(|&idx| self.instructions[idx].op == BuilderOp::PopConditionMask)
        {
            let stack_id = self.instructions[idx].stack_id;
            self.pop_back();
            self.discard_stack_on_stack(1, stack_id);
        }

        self.append_instruction(BuilderOp::MergeConditionMask, SlotList::NONE, [0; 4]);
    }

    /// `merge_inv_condition_mask`.
    pub fn merge_inv_condition_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::MergeInvConditionMask, SlotList::NONE, [0; 4]);
    }

    /// `push_loop_mask`.
    pub fn push_loop_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::PushLoopMask, SlotList::NONE, [0; 4]);
    }

    /// `pop_loop_mask`.
    pub fn pop_loop_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::PopLoopMask, SlotList::NONE, [0; 4]);
    }

    /// `mask_off_loop_mask`.
    pub fn mask_off_loop_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::MaskOffLoopMask, SlotList::NONE, [0; 4]);
    }

    /// `reenable_loop_mask`.
    pub fn reenable_loop_mask(&mut self, src: SlotRange) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        debug_assert_eq!(src.count, 1);
        self.append_instruction(
            BuilderOp::ReenableLoopMask,
            SlotList::one(src.index),
            [0; 4],
        );
    }

    /// `pop_and_reenable_loop_mask`.
    pub fn pop_and_reenable_loop_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::PopAndReenableLoopMask, SlotList::NONE, [0; 4]);
    }

    /// `merge_loop_mask`.
    pub fn merge_loop_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::MergeLoopMask, SlotList::NONE, [0; 4]);
    }

    /// `push_return_mask`.
    pub fn push_return_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::PushReturnMask, SlotList::NONE, [0; 4]);
    }

    /// `pop_return_mask`: a preceding `mask_off_return_mask` is wasted work, because this
    /// overwrites the return mask, so it is eliminated.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1009-L1021 (chrome/m156)
    pub fn pop_return_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        if self
            .last_instruction_on_any_stack(0)
            .is_some_and(|idx| self.instructions[idx].op == BuilderOp::MaskOffReturnMask)
        {
            self.pop_back();
        }
        self.append_instruction(BuilderOp::PopReturnMask, SlotList::NONE, [0; 4]);
    }

    /// `mask_off_return_mask`.
    pub fn mask_off_return_mask(&mut self) {
        debug_assert!(self.execution_mask_writes_are_enabled());
        self.append_instruction(BuilderOp::MaskOffReturnMask, SlotList::NONE, [0; 4]);
    }

    /// `trace_line`: writes the current line number to the debug trace.
    pub fn trace_line(&mut self, trace_mask_stack_id: i32, line: i32) {
        self.append_instruction(
            BuilderOp::TraceLine,
            SlotList::NONE,
            [trace_mask_stack_id, line, 0, 0],
        );
    }

    /// `trace_var`: writes a variable update to the debug trace.
    pub fn trace_var(&mut self, trace_mask_stack_id: i32, r: SlotRange) {
        self.append_instruction(
            BuilderOp::TraceVar,
            SlotList::one(r.index),
            [trace_mask_stack_id, r.count, 0, 0],
        );
    }

    /// `trace_var_indirect`: writes a variable update (via indirection) to the debug trace.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L629-L643 (chrome/m156)
    pub fn trace_var_indirect(
        &mut self,
        trace_mask_stack_id: i32,
        fixed_range: SlotRange,
        dynamic_stack_id: i32,
        limit_range: SlotRange,
    ) {
        // SlotA: fixed-range start. SlotB: limit-range end. immA: trace-mask stack ID.
        // immB: number of slots. immC: dynamic stack ID.
        self.append_instruction(
            BuilderOp::TraceVarIndirect,
            SlotList::two(fixed_range.index, limit_range.index + limit_range.count),
            [trace_mask_stack_id, fixed_range.count, dynamic_stack_id, 0],
        );
    }

    /// `trace_enter`: writes a function-entrance to the debug trace.
    pub fn trace_enter(&mut self, trace_mask_stack_id: i32, func_id: i32) {
        self.append_instruction(
            BuilderOp::TraceEnter,
            SlotList::NONE,
            [trace_mask_stack_id, func_id, 0, 0],
        );
    }

    /// `trace_exit`: writes a function-exit to the debug trace.
    pub fn trace_exit(&mut self, trace_mask_stack_id: i32, func_id: i32) {
        self.append_instruction(
            BuilderOp::TraceExit,
            SlotList::NONE,
            [trace_mask_stack_id, func_id, 0, 0],
        );
    }

    /// `trace_scope`: writes a scope-level change to the debug trace.
    pub fn trace_scope(&mut self, trace_mask_stack_id: i32, delta: i32) {
        self.append_instruction(
            BuilderOp::TraceScope,
            SlotList::NONE,
            [trace_mask_stack_id, delta, 0, 0],
        );
    }
}
