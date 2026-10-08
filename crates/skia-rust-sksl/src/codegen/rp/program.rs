// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.{h,cpp} (the `Program` half:
// construction, `makeStages` and the `appendCopy*`/`appendAdjacent*` helpers).

//! A finished Raster Pipeline program ([`Program`]) and its lowering to a stage list
//! ([`Program::make_stages`]).
//!
//! `Skia`'s stages hold raw pointers into one slot slab. Here a pointer is an [`Addr`]: a byte
//! offset from the slab's value base (or from the uniform block). The slab holds, in order, the
//! value slots, the temp stacks and the immutable slots, so `OffsetFromBase` is the same
//! subtraction `Skia` does. The contexts `Skia` packs into `void*` are [`StageCtx`] variants.
//!
//! `Program::appendStages`, the bridge to the simd pipeline, is in [`super::append`]. `Skia`'s
//! `SkOpts::raster_pipeline_highp_stride` is the `lanes` argument of [`Program::slot_data`].

// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1399-L1640 (chrome/m156)

use std::sync::Arc;

use super::builder::{Instruction, NA, Slot};
use super::ops::BuilderOp;
use crate::tracing::DebugTracePriv;

/// A pointer into slot memory, as byte offsets (`Skia`'s `float*`, relative to a base).
///
/// `Slab` offsets are relative to the value base; the temp stacks and immutable slots follow the
/// value slots in the same slab, as in `SkSL::RP::Program::allocateSlotData`. `Uniform` offsets
/// are relative to the uniform block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Addr {
    /// A null pointer (a `nullptr` context).
    Null,
    /// A byte offset from the slab's value base.
    Slab(isize),
    /// A byte offset from the uniform block.
    Uniform(isize),
}

impl Addr {
    /// Pointer arithmetic: the address `bytes` further along.
    ///
    /// # Panics
    ///
    /// Panics on a null address.
    #[must_use]
    pub fn plus(self, bytes: isize) -> Addr {
        match self {
            Addr::Slab(o) => Addr::Slab(o + bytes),
            Addr::Uniform(o) => Addr::Uniform(o + bytes),
            Addr::Null => panic!("pointer arithmetic on a null context"),
        }
    }

    /// `OffsetFromBase`: the `SkRPOffset` (bytes from the value base) of a slab pointer.
    ///
    /// # Panics
    ///
    /// Panics if the pointer is null, is a uniform, or lies before the value base.
    #[must_use]
    pub fn offset_from_base(self) -> u32 {
        match self {
            Addr::Slab(o) => u32::try_from(o).expect("slab offsets are non-negative"),
            _ => panic!("OffsetFromBase of a pointer outside the slab"),
        }
    }
}

/// The per-stage context: what `Skia`'s `void* ctx` holds for the op.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageCtx {
    /// `nullptr`.
    Null,
    /// A pointer to slot or uniform memory (`float*`).
    Ptr(Addr),
    /// A 32-bit integer packed into the context pointer (`context_bit_pun`): a label ID, a child
    /// index, or a function index depending on the op.
    Int(i32),
    /// `ConstantCtx`.
    Constant { dst: u32, value: i32 },
    /// `BinaryOpCtx`.
    BinaryOp { dst: u32, src: u32 },
    /// `TernaryOpCtx`: `delta` is the distance from `dst` to the first source.
    TernaryOp { dst: u32, delta: u32 },
    /// `UniformCtx`.
    Uniform { dst: Addr, src: Addr },
    /// `CopyIndirectCtx`. The dumper does not print `indirect_limit`; it is kept for fidelity.
    CopyIndirect {
        dst: Addr,
        src: Addr,
        indirect_offset: Addr,
        indirect_limit: i32,
        slots: i32,
    },
    /// `SwizzleCopyCtx`.
    SwizzleCopy {
        dst: Addr,
        src: Addr,
        offsets: [u32; 4],
    },
    /// `SwizzleCopyIndirectCtx`.
    SwizzleCopyIndirect {
        dst: Addr,
        src: Addr,
        indirect_offset: Addr,
        indirect_limit: i32,
        slots: i32,
        offsets: [u32; 4],
    },
    /// `SwizzleCtx`.
    Swizzle { dst: u32, offsets: [u32; 4] },
    /// `ShuffleCtx`.
    Shuffle {
        ptr: Addr,
        count: i32,
        offsets: [u32; 16],
    },
    /// `MatrixMultiplyCtx`.
    MatrixMultiply {
        dst: u32,
        left_columns: i32,
        left_rows: i32,
        right_columns: i32,
        right_rows: i32,
    },
    /// `CaseOpCtx`.
    CaseOp { expected_value: i32, offset: u32 },
    /// `BranchCtx`: `offset` is the label ID until the branch is fixed up.
    Branch { offset: i32 },
    /// `BranchIfEqualCtx`.
    BranchIfEqual { offset: i32, value: i32, ptr: Addr },
    /// `TraceLineCtx`.
    TraceLine { trace_mask: Addr, line: i32 },
    /// `TraceScopeCtx`.
    TraceScope { trace_mask: Addr, delta: i32 },
    /// `TraceFuncCtx`.
    TraceFunc { trace_mask: Addr, func_idx: i32 },
    /// `TraceVarCtx`.
    TraceVar {
        trace_mask: Addr,
        slot_idx: Slot,
        num_slots: i32,
        data: Addr,
        indirect_offset: Option<Addr>,
        indirect_limit: i32,
    },
}

/// One stage of a lowered program: an op and its context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    /// The op (a native raster pipeline op, or one of the extended ops).
    pub op: BuilderOp,
    /// The op's context.
    pub ctx: StageCtx,
}

/// The slot layout of `SkSL::RP::Program::allocateSlotData`, without the arena, plus the
/// immutable values `make_stages` writes into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotData {
    /// `N`: the highp lane count (`SkOpts::raster_pipeline_highp_stride`).
    pub lanes: usize,
    /// The number of value slots.
    pub num_values: usize,
    /// The number of temp-stack slots.
    pub num_stack: usize,
    /// The immutable slots' bit patterns (one `i32` per slot).
    pub immutable: Vec<i32>,
}

impl SlotData {
    /// The byte offset of the first temp-stack slot (`slots.stack.data()` from the value base).
    fn stack_base(&self) -> isize {
        slab_bytes(4 * self.lanes * self.num_values)
    }

    /// The byte offset of the first immutable slot.
    fn immutable_base(&self) -> isize {
        self.stack_base() + slab_bytes(4 * self.lanes * self.num_stack)
    }

    /// The bit pattern at `addr`, as `Skia`'s `*ptr` read: immutable slots hold their constants,
    /// and value, stack and uniform memory reads as zero (the dumper never runs the program).
    #[must_use]
    pub fn read_bits(&self, addr: Addr) -> i32 {
        let Addr::Slab(o) = addr else {
            return 0;
        };
        let rel = o - self.immutable_base();
        match usize::try_from(rel / 4) {
            Ok(idx) if rel >= 0 => self.immutable.get(idx).copied().unwrap_or(0),
            _ => 0,
        }
    }
}

/// Converts a byte count to a slab offset.
fn slab_bytes(bytes: usize) -> isize {
    isize::try_from(bytes).expect("slot slab fits in isize")
}

/// `SkSL::RP::Program`: a finished instruction list plus its slot counts.
#[doc(alias = "SkSL::RP::Program")]
#[derive(Debug)]
pub struct Program {
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) num_value_slots: i32,
    pub(crate) num_uniform_slots: i32,
    pub(crate) num_immutable_slots: i32,
    pub(crate) num_temp_stack_slots: i32,
    pub(crate) num_labels: i32,
    pub(crate) temp_stack_max_depths: Vec<i32>,
    pub(crate) debug_trace: Option<Arc<DebugTracePriv>>,
}

impl Program {
    /// `Program::Program`: computes the temp-stack depths from the instructions.
    pub(crate) fn new(
        instructions: Vec<Instruction>,
        num_value_slots: i32,
        num_uniform_slots: i32,
        num_immutable_slots: i32,
        num_labels: i32,
        debug_trace: Option<Arc<DebugTracePriv>>,
    ) -> Program {
        let temp_stack_max_depths = temp_stack_max_depths(&instructions);
        let num_temp_stack_slots = temp_stack_max_depths.iter().sum();
        Program {
            instructions,
            num_value_slots,
            num_uniform_slots,
            num_immutable_slots,
            num_temp_stack_slots,
            num_labels,
            temp_stack_max_depths,
            debug_trace,
        }
    }

    /// The debug trace the program was finished with, if any.
    #[must_use]
    pub fn debug_trace(&self) -> Option<&DebugTracePriv> {
        self.debug_trace.as_deref()
    }

    /// `Program::numUniforms`.
    #[doc(alias = "numUniforms")]
    #[must_use]
    pub fn num_uniforms(&self) -> i32 {
        self.num_uniform_slots
    }

    /// The slot layout `allocateSlotData` gives this program with `lanes` lanes. The immutable
    /// slots start at zero; `make_stages` fills them.
    #[must_use]
    pub fn slot_data(&self, lanes: usize) -> SlotData {
        SlotData {
            lanes,
            num_values: usize::try_from(self.num_value_slots).unwrap_or(0),
            num_stack: usize::try_from(self.num_temp_stack_slots).unwrap_or(0),
            immutable: vec![0; usize::try_from(self.num_immutable_slots).unwrap_or(0)],
        }
    }

    /// `Program::makeStages`: lowers the instructions to a stage list.
    ///
    /// # Panics
    ///
    /// Panics on an instruction the lowering does not support, or on a program whose
    /// instructions are not balanced (checked when the program is built).
    ///
    /// `non_tail_rewinds` is `SKSL_STANDALONE || !SK_HAS_MUSTTAIL`: whether
    /// `appendStackRewindForNonTailcallers` emits `stack_rewind`. `uniforms` holds the uniform
    /// bit patterns (the dumper passes zeros). `slots.immutable` is filled from the
    /// `store_immutable_value` instructions.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1821-L2489 (chrome/m156)
    // one arm per op, and Skia's one-letter immA..immD and src/dst names, as in `makeStages`
    #[allow(clippy::too_many_lines, clippy::many_single_char_names)]
    pub fn make_stages(
        &self,
        uniforms: &[i32],
        slots: &mut SlotData,
        non_tail_rewinds: bool,
    ) -> Vec<Stage> {
        debug_assert_eq!(
            usize::try_from(self.num_uniform_slots).ok(),
            Some(uniforms.len())
        );

        let n = slots.lanes;
        let slot_bytes = slab_bytes(4 * n);
        let mut most_recent_rewind = 0_usize;
        let mut pipeline: Vec<Stage> = Vec::new();

        // Assemble a map holding the current stack-top for each temporary stack. Position each temp
        // stack immediately after the previous temp stack; temp stacks are never allowed to overlap.
        let mut pos = 0_isize;
        let mut temp_stack_map: Vec<Addr> = Vec::with_capacity(self.temp_stack_max_depths.len());
        for &depth in &self.temp_stack_max_depths {
            temp_stack_map.push(Addr::Slab(slots.stack_base() + pos * slot_bytes));
            pos += isize::try_from(depth).expect("depths are non-negative");
        }

        // Track labels that we have reached in processing.
        let mut label_to_instruction_index: Vec<Option<usize>> =
            vec![None; usize::try_from(self.num_labels).unwrap_or(0)];
        let mut most_recent_invocation_instruction_idx = 0_usize;

        // Copy all immutable values into the immutable slots.
        for inst in &self.instructions {
            if inst.op == BuilderOp::StoreImmutableValue {
                let idx = usize::try_from(inst.slot_a).expect("immutable slot");
                slots.immutable[idx] = inst.imm_a;
            }
        }
        let immutable_base = slots.immutable_base();

        // Write each BuilderOp to the pipeline array.
        for (instruction_idx, &inst) in self.instructions.iter().enumerate() {
            let stack_id = usize::try_from(inst.stack_id).expect("stack id");
            let mut temp_stack_ptr = temp_stack_map[stack_id];

            // Address helpers, mirroring the lambdas in `makeStages`.
            let slot_a = Addr::Slab(slot_bytes * slot_index(inst.slot_a));
            let slot_b = Addr::Slab(slot_bytes * slot_index(inst.slot_b));
            let immutable_a = Addr::Slab(immutable_base + 4 * slot_index(inst.slot_a));
            let immutable_b = Addr::Slab(immutable_base + 4 * slot_index(inst.slot_b));
            let uniform_a = Addr::Uniform(4 * slot_index(inst.slot_a));
            let a = imm(inst.imm_a);
            let b = imm(inst.imm_b);
            let c = imm(inst.imm_c);
            let d = imm(inst.imm_d);

            match inst.op {
                BuilderOp::Label => {
                    // Remember the instruction index of this label.
                    let label_id = usize::try_from(inst.imm_a).expect("label id");
                    debug_assert!(label_to_instruction_index[label_id].is_none());
                    label_to_instruction_index[label_id] = Some(instruction_idx);
                    push(&mut pipeline, BuilderOp::Label, StageCtx::Int(inst.imm_a));
                }
                BuilderOp::Jump
                | BuilderOp::BranchIfAnyLanesActive
                | BuilderOp::BranchIfNoLanesActive
                | BuilderOp::BranchIfAllLanesActive => {
                    emit_rewind_for_backwards_branch(
                        &mut pipeline,
                        &mut most_recent_rewind,
                        &label_to_instruction_index,
                        most_recent_invocation_instruction_idx,
                        inst.imm_a,
                        non_tail_rewinds,
                    );
                    push(
                        &mut pipeline,
                        inst.op,
                        StageCtx::Branch { offset: inst.imm_a },
                    );
                }
                BuilderOp::BranchIfNoActiveLanesOnStackTopEqual => {
                    emit_rewind_for_backwards_branch(
                        &mut pipeline,
                        &mut most_recent_rewind,
                        &label_to_instruction_index,
                        most_recent_invocation_instruction_idx,
                        inst.imm_a,
                        non_tail_rewinds,
                    );
                    push(
                        &mut pipeline,
                        BuilderOp::BranchIfNoActiveLanesEq,
                        StageCtx::BranchIfEqual {
                            offset: inst.imm_a,
                            value: inst.imm_b,
                            ptr: temp_stack_ptr.plus(-slot_bytes),
                        },
                    );
                }
                BuilderOp::InitLaneMasks => push(&mut pipeline, inst.op, StageCtx::Null),
                BuilderOp::StoreSrcRg
                | BuilderOp::StoreSrc
                | BuilderOp::StoreDst
                | BuilderOp::StoreDeviceXy01
                | BuilderOp::LoadSrc
                | BuilderOp::LoadDst => push(&mut pipeline, inst.op, StageCtx::Ptr(slot_a)),
                // The immutable slots were populated in the first pass.
                BuilderOp::StoreImmutableValue => {}
                op if is_single_slot_unary_op(op) => {
                    let dst = temp_stack_ptr.plus(-a * slot_bytes);
                    append_single_slot_unary_op(&mut pipeline, op, dst, inst.imm_a, n);
                }
                op if is_multi_slot_unary_op(op) => {
                    let dst = temp_stack_ptr.plus(-a * slot_bytes);
                    append_multi_slot_unary_op(&mut pipeline, op, dst, inst.imm_a, n);
                }
                op if is_immediate_binary_op(op) => {
                    let dst = if inst.slot_a == NA {
                        temp_stack_ptr.plus(-a * slot_bytes)
                    } else {
                        slot_a
                    };
                    append_immediate_binary_op(
                        &mut pipeline,
                        op,
                        dst.offset_from_base(),
                        inst.imm_b,
                        inst.imm_a,
                        n,
                    );
                }
                op if is_n_way_binary_op(op) => {
                    let src = temp_stack_ptr.plus(-a * slot_bytes);
                    let dst = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    append_adjacent_n_way_binary_op(
                        &mut pipeline,
                        op,
                        dst.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                op if is_multi_slot_binary_op(op) => {
                    let src = temp_stack_ptr.plus(-a * slot_bytes);
                    let dst = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    append_adjacent_multi_slot_binary_op(
                        &mut pipeline,
                        op,
                        dst.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                op if is_n_way_ternary_op(op) => {
                    let src1 = temp_stack_ptr.plus(-a * slot_bytes);
                    let src0 = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    let dst = temp_stack_ptr.plus(-a * 3 * slot_bytes);
                    append_adjacent_n_way_ternary_op(
                        &mut pipeline,
                        op,
                        dst.offset_from_base(),
                        src0.offset_from_base(),
                        src1.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                op if is_multi_slot_ternary_op(op) => {
                    let src1 = temp_stack_ptr.plus(-a * slot_bytes);
                    let src0 = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    let dst = temp_stack_ptr.plus(-a * 3 * slot_bytes);
                    append_adjacent_multi_slot_ternary_op(
                        &mut pipeline,
                        op,
                        dst.offset_from_base(),
                        src0.offset_from_base(),
                        src1.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::Select => {
                    let src = temp_stack_ptr.plus(-a * slot_bytes);
                    let dst = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    append_copy_slots_masked(
                        &mut pipeline,
                        dst.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::CopySlotMasked => append_copy_slots_masked(
                    &mut pipeline,
                    slot_a.offset_from_base(),
                    slot_b.offset_from_base(),
                    inst.imm_a,
                    n,
                ),
                BuilderOp::CopySlotUnmasked => append_copy_slots_unmasked(
                    &mut pipeline,
                    slot_a.offset_from_base(),
                    slot_b.offset_from_base(),
                    inst.imm_a,
                    n,
                ),
                BuilderOp::CopyImmutableUnmasked => append_copy_immutable_unmasked(
                    &mut pipeline,
                    slots,
                    slot_a.offset_from_base(),
                    immutable_b.offset_from_base(),
                    inst.imm_a,
                    n,
                ),
                BuilderOp::Refract4Floats => {
                    let dst = temp_stack_ptr.plus(-9 * slot_bytes);
                    push(&mut pipeline, inst.op, StageCtx::Ptr(dst));
                }
                BuilderOp::InverseMat2 | BuilderOp::InverseMat3 | BuilderOp::InverseMat4 => {
                    let dst = temp_stack_ptr.plus(-a * slot_bytes);
                    push(&mut pipeline, inst.op, StageCtx::Ptr(dst));
                }
                BuilderOp::Dot2Floats | BuilderOp::Dot3Floats | BuilderOp::Dot4Floats => {
                    let dst = temp_stack_ptr.plus(-a * 2 * slot_bytes);
                    push(&mut pipeline, inst.op, StageCtx::Ptr(dst));
                }
                BuilderOp::Swizzle1 => {
                    // A single-component swizzle just copies a slot and shrinks the stack; we can
                    // slightly improve codegen by making that simplification here.
                    let offset = b;
                    debug_assert!((0..=15).contains(&inst.imm_b));
                    let dst = temp_stack_ptr.plus(-a * slot_bytes);
                    let src = dst.plus(offset * slot_bytes);
                    if src != dst {
                        append_copy_slots_unmasked(
                            &mut pipeline,
                            dst.offset_from_base(),
                            src.offset_from_base(),
                            1,
                            n,
                        );
                    }
                }
                BuilderOp::Swizzle2 | BuilderOp::Swizzle3 | BuilderOp::Swizzle4 => {
                    let mut offsets = [0_u32; 4];
                    unpack_nybbles_to_offsets(inst.imm_b, &mut offsets, n);
                    push(
                        &mut pipeline,
                        inst.op,
                        StageCtx::Swizzle {
                            dst: temp_stack_ptr.plus(-a * slot_bytes).offset_from_base(),
                            offsets,
                        },
                    );
                }
                BuilderOp::Shuffle => {
                    let consumed = a;
                    let generated = inst.imm_b;
                    let mut offsets = [0_u32; 16];
                    unpack_nybbles_to_offsets(inst.imm_c, &mut offsets[..8], n);
                    unpack_nybbles_to_offsets(inst.imm_d, &mut offsets[8..], n);
                    push(
                        &mut pipeline,
                        BuilderOp::Shuffle,
                        StageCtx::Shuffle {
                            ptr: temp_stack_ptr.plus(-consumed * slot_bytes),
                            count: generated,
                            offsets,
                        },
                    );
                }
                BuilderOp::MatrixMultiply2
                | BuilderOp::MatrixMultiply3
                | BuilderOp::MatrixMultiply4 => {
                    // result + left-matrix + right-matrix
                    let consumed = (b * c) + (a * b) + (c * d);
                    push(
                        &mut pipeline,
                        inst.op,
                        StageCtx::MatrixMultiply {
                            dst: temp_stack_ptr
                                .plus(-consumed * slot_bytes)
                                .offset_from_base(),
                            left_columns: inst.imm_a,
                            left_rows: inst.imm_b,
                            right_columns: inst.imm_c,
                            right_rows: inst.imm_d,
                        },
                    );
                }
                BuilderOp::ExchangeSrc => {
                    let dst = temp_stack_ptr.plus(-4 * slot_bytes);
                    push(&mut pipeline, BuilderOp::ExchangeSrc, StageCtx::Ptr(dst));
                }
                BuilderOp::PushSrcRgba => {
                    push(
                        &mut pipeline,
                        BuilderOp::StoreSrc,
                        StageCtx::Ptr(temp_stack_ptr),
                    );
                }
                BuilderOp::PushDstRgba => {
                    push(
                        &mut pipeline,
                        BuilderOp::StoreDst,
                        StageCtx::Ptr(temp_stack_ptr),
                    );
                }
                BuilderOp::PushDeviceXy01 => {
                    push(
                        &mut pipeline,
                        BuilderOp::StoreDeviceXy01,
                        StageCtx::Ptr(temp_stack_ptr),
                    );
                }
                BuilderOp::PopSrcRgba => {
                    let src = temp_stack_ptr.plus(-4 * slot_bytes);
                    push(&mut pipeline, BuilderOp::LoadSrc, StageCtx::Ptr(src));
                }
                BuilderOp::PopDstRgba => {
                    let src = temp_stack_ptr.plus(-4 * slot_bytes);
                    push(&mut pipeline, BuilderOp::LoadDst, StageCtx::Ptr(src));
                }
                BuilderOp::PushSlots => append_copy_slots_unmasked(
                    &mut pipeline,
                    temp_stack_ptr.offset_from_base(),
                    slot_a.offset_from_base(),
                    inst.imm_a,
                    n,
                ),
                BuilderOp::PushImmutable => append_copy_immutable_unmasked(
                    &mut pipeline,
                    slots,
                    temp_stack_ptr.offset_from_base(),
                    immutable_a.offset_from_base(),
                    inst.imm_a,
                    n,
                ),
                BuilderOp::CopyStackToSlotsIndirect
                | BuilderOp::PushImmutableIndirect
                | BuilderOp::PushSlotsIndirect
                | BuilderOp::PushUniformIndirect => {
                    // SlotA: fixed-range start. SlotB: limit-range end. immA: number of slots to
                    // copy. immB: dynamic stack ID.
                    let indirect_offset = temp_stack_map
                        [usize::try_from(inst.imm_b).expect("stack")]
                    .plus(-slot_bytes);
                    let indirect_limit = inst.slot_b - inst.slot_a - inst.imm_a;
                    let (op, dst, src) = match inst.op {
                        BuilderOp::PushSlotsIndirect => {
                            (BuilderOp::CopyFromIndirectUnmasked, temp_stack_ptr, slot_a)
                        }
                        BuilderOp::PushImmutableIndirect => (
                            BuilderOp::CopyFromIndirectUniformUnmasked,
                            temp_stack_ptr,
                            immutable_a,
                        ),
                        BuilderOp::PushUniformIndirect => (
                            BuilderOp::CopyFromIndirectUniformUnmasked,
                            temp_stack_ptr,
                            uniform_a,
                        ),
                        _ => (
                            BuilderOp::CopyToIndirectMasked,
                            slot_a,
                            temp_stack_ptr.plus(-a * slot_bytes),
                        ),
                    };
                    push(
                        &mut pipeline,
                        op,
                        StageCtx::CopyIndirect {
                            dst,
                            src,
                            indirect_offset,
                            indirect_limit,
                            slots: inst.imm_a,
                        },
                    );
                }
                BuilderOp::PushUniform | BuilderOp::CopyUniformToSlotsUnmasked => {
                    let mut src = uniform_a;
                    let mut dst = if inst.op == BuilderOp::PushUniform {
                        temp_stack_ptr
                    } else {
                        slot_b
                    };
                    let mut remaining = inst.imm_a;
                    while remaining > 0 {
                        let op = match remaining {
                            1 => BuilderOp::CopyUniform,
                            2 => BuilderOp::Copy2Uniforms,
                            3 => BuilderOp::Copy3Uniforms,
                            _ => BuilderOp::Copy4Uniforms,
                        };
                        push(&mut pipeline, op, StageCtx::Uniform { dst, src });
                        dst = dst.plus(4 * slot_bytes);
                        src = src.plus(4 * 4);
                        remaining -= 4;
                    }
                }
                BuilderOp::PushConditionMask => push(
                    &mut pipeline,
                    BuilderOp::StoreConditionMask,
                    StageCtx::Ptr(temp_stack_ptr),
                ),
                BuilderOp::PopConditionMask => push(
                    &mut pipeline,
                    BuilderOp::LoadConditionMask,
                    StageCtx::Ptr(temp_stack_ptr.plus(-slot_bytes)),
                ),
                BuilderOp::MergeConditionMask | BuilderOp::MergeInvConditionMask => push(
                    &mut pipeline,
                    inst.op,
                    StageCtx::Ptr(temp_stack_ptr.plus(-2 * slot_bytes)),
                ),
                BuilderOp::PushLoopMask => push(
                    &mut pipeline,
                    BuilderOp::StoreLoopMask,
                    StageCtx::Ptr(temp_stack_ptr),
                ),
                BuilderOp::PopLoopMask => push(
                    &mut pipeline,
                    BuilderOp::LoadLoopMask,
                    StageCtx::Ptr(temp_stack_ptr.plus(-slot_bytes)),
                ),
                BuilderOp::PopAndReenableLoopMask => push(
                    &mut pipeline,
                    BuilderOp::ReenableLoopMask,
                    StageCtx::Ptr(temp_stack_ptr.plus(-slot_bytes)),
                ),
                BuilderOp::ReenableLoopMask => push(
                    &mut pipeline,
                    BuilderOp::ReenableLoopMask,
                    StageCtx::Ptr(slot_a),
                ),
                BuilderOp::MaskOffLoopMask => {
                    push(&mut pipeline, BuilderOp::MaskOffLoopMask, StageCtx::Null);
                }
                BuilderOp::MergeLoopMask => push(
                    &mut pipeline,
                    BuilderOp::MergeLoopMask,
                    StageCtx::Ptr(temp_stack_ptr.plus(-slot_bytes)),
                ),
                BuilderOp::PushReturnMask => push(
                    &mut pipeline,
                    BuilderOp::StoreReturnMask,
                    StageCtx::Ptr(temp_stack_ptr),
                ),
                BuilderOp::PopReturnMask => push(
                    &mut pipeline,
                    BuilderOp::LoadReturnMask,
                    StageCtx::Ptr(temp_stack_ptr.plus(-slot_bytes)),
                ),
                BuilderOp::MaskOffReturnMask => {
                    push(&mut pipeline, BuilderOp::MaskOffReturnMask, StageCtx::Null);
                }
                BuilderOp::CopyConstant | BuilderOp::PushConstant => {
                    let mut dst = if inst.op == BuilderOp::CopyConstant {
                        slot_a
                    } else {
                        temp_stack_ptr
                    };
                    // Splat constant values onto the stack.
                    let mut remaining = inst.imm_a;
                    while remaining > 0 {
                        let op = match remaining {
                            1 => BuilderOp::CopyConstant,
                            2 => BuilderOp::Splat2Constants,
                            3 => BuilderOp::Splat3Constants,
                            _ => BuilderOp::Splat4Constants,
                        };
                        push(
                            &mut pipeline,
                            op,
                            StageCtx::Constant {
                                dst: dst.offset_from_base(),
                                value: inst.imm_b,
                            },
                        );
                        dst = dst.plus(4 * slot_bytes);
                        remaining -= 4;
                    }
                }
                BuilderOp::CopyStackToSlots => {
                    let src = temp_stack_ptr.plus(-b * slot_bytes);
                    append_copy_slots_masked(
                        &mut pipeline,
                        slot_a.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::CopyStackToSlotsUnmasked => {
                    let src = temp_stack_ptr.plus(-b * slot_bytes);
                    append_copy_slots_unmasked(
                        &mut pipeline,
                        slot_a.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::SwizzleCopyStackToSlots => {
                    // SlotA: fixed-range start. immA: number of swizzle components. immB: swizzle
                    // components. immC: offset from stack top.
                    let stage = op_offset(
                        BuilderOp::SwizzleCopySlotMasked,
                        isize::try_from(inst.imm_a - 1).expect("component count"),
                    );
                    let mut offsets = [0_u32; 4];
                    unpack_nybbles_to_offsets(inst.imm_b, &mut offsets, n);
                    push(
                        &mut pipeline,
                        stage,
                        StageCtx::SwizzleCopy {
                            dst: slot_a,
                            src: temp_stack_ptr.plus(-c * slot_bytes),
                            offsets,
                        },
                    );
                }
                BuilderOp::PushClone => {
                    let src = temp_stack_ptr.plus(-b * slot_bytes);
                    append_copy_slots_unmasked(
                        &mut pipeline,
                        temp_stack_ptr.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::PushCloneFromStack => {
                    // immA: number of slots. immB: other stack ID. immC: offset from stack top.
                    let source_stack_ptr =
                        temp_stack_map[usize::try_from(inst.imm_b).expect("stack")];
                    let src = source_stack_ptr.plus(-c * slot_bytes);
                    append_copy_slots_unmasked(
                        &mut pipeline,
                        temp_stack_ptr.offset_from_base(),
                        src.offset_from_base(),
                        inst.imm_a,
                        n,
                    );
                }
                BuilderOp::PushCloneIndirectFromStack => {
                    // immA: number of slots. immB: other stack ID. immC: offset from stack top.
                    // immD: dynamic stack ID.
                    let source_stack_ptr =
                        temp_stack_map[usize::try_from(inst.imm_b).expect("stack")];
                    let indirect_offset = temp_stack_map
                        [usize::try_from(inst.imm_d).expect("stack")]
                    .plus(-slot_bytes);
                    push(
                        &mut pipeline,
                        BuilderOp::CopyFromIndirectUnmasked,
                        StageCtx::CopyIndirect {
                            dst: temp_stack_ptr,
                            src: source_stack_ptr.plus(-c * slot_bytes),
                            indirect_offset,
                            indirect_limit: inst.imm_c - inst.imm_a,
                            slots: inst.imm_a,
                        },
                    );
                }
                BuilderOp::SwizzleCopyStackToSlotsIndirect => {
                    // SlotA: fixed-range start. SlotB: limit-range end. immA: number of swizzle
                    // components. immB: swizzle components. immC: offset from stack top. immD:
                    // dynamic stack ID.
                    let indirect_offset = temp_stack_map
                        [usize::try_from(inst.imm_d).expect("stack")]
                    .plus(-slot_bytes);
                    let mut offsets = [0_u32; 4];
                    unpack_nybbles_to_offsets(inst.imm_b, &mut offsets, n);
                    let count = usize::try_from(inst.imm_a).expect("component count");
                    push(
                        &mut pipeline,
                        BuilderOp::SwizzleCopyToIndirectMasked,
                        StageCtx::SwizzleCopyIndirect {
                            dst: slot_a,
                            src: temp_stack_ptr.plus(-c * slot_bytes),
                            indirect_offset,
                            indirect_limit: inst.slot_b
                                - inst.slot_a
                                - (max_packed_nybble(inst.imm_b, count) + 1),
                            slots: inst.imm_a,
                            offsets,
                        },
                    );
                }
                BuilderOp::CaseOp => push(
                    &mut pipeline,
                    BuilderOp::CaseOp,
                    StageCtx::CaseOp {
                        expected_value: inst.imm_a,
                        offset: temp_stack_ptr.plus(-2 * slot_bytes).offset_from_base(),
                    },
                ),
                BuilderOp::ContinueOp => {
                    let idx = usize::try_from(inst.imm_a).expect("stack");
                    push(
                        &mut pipeline,
                        BuilderOp::ContinueOp,
                        StageCtx::Ptr(temp_stack_map[idx].plus(-slot_bytes)),
                    );
                }
                BuilderOp::PadStack | BuilderOp::DiscardStack => {}
                BuilderOp::InvokeShader
                | BuilderOp::InvokeColorFilter
                | BuilderOp::InvokeBlender => {
                    push(&mut pipeline, inst.op, StageCtx::Int(inst.imm_a));
                    most_recent_invocation_instruction_idx = instruction_idx;
                }
                BuilderOp::InvokeToLinearSrgb | BuilderOp::InvokeFromLinearSrgb => {
                    let idx = usize::try_from(inst.imm_a).expect("stack");
                    push(
                        &mut pipeline,
                        inst.op,
                        StageCtx::Ptr(temp_stack_map[idx].plus(-4 * slot_bytes)),
                    );
                    most_recent_invocation_instruction_idx = instruction_idx;
                }
                BuilderOp::TraceLine => {
                    let trace_mask = trace_mask_of(&temp_stack_map, inst.imm_a, slot_bytes);
                    push(
                        &mut pipeline,
                        BuilderOp::TraceLine,
                        StageCtx::TraceLine {
                            trace_mask,
                            line: inst.imm_b,
                        },
                    );
                }
                BuilderOp::TraceScope => {
                    let trace_mask = trace_mask_of(&temp_stack_map, inst.imm_a, slot_bytes);
                    push(
                        &mut pipeline,
                        BuilderOp::TraceScope,
                        StageCtx::TraceScope {
                            trace_mask,
                            delta: inst.imm_b,
                        },
                    );
                }
                BuilderOp::TraceEnter | BuilderOp::TraceExit => {
                    let trace_mask = trace_mask_of(&temp_stack_map, inst.imm_a, slot_bytes);
                    push(
                        &mut pipeline,
                        inst.op,
                        StageCtx::TraceFunc {
                            trace_mask,
                            func_idx: inst.imm_b,
                        },
                    );
                }
                BuilderOp::TraceVar | BuilderOp::TraceVarIndirect => {
                    // SlotA: fixed-range start. SlotB: limit-range end. immA: trace-mask stack ID.
                    // immB: number of slots. immC: dynamic stack ID.
                    let trace_mask = trace_mask_of(&temp_stack_map, inst.imm_a, slot_bytes);
                    let (indirect_offset, indirect_limit) =
                        if inst.op == BuilderOp::TraceVarIndirect {
                            (
                                Some(trace_mask_of(&temp_stack_map, inst.imm_c, slot_bytes)),
                                inst.slot_b - inst.slot_a - inst.imm_b,
                            )
                        } else {
                            (None, 0)
                        };
                    push(
                        &mut pipeline,
                        BuilderOp::TraceVar,
                        StageCtx::TraceVar {
                            trace_mask,
                            slot_idx: inst.slot_a,
                            num_slots: inst.imm_b,
                            data: slot_a,
                            indirect_offset,
                            indirect_limit,
                        },
                    );
                }
                other => panic!("Raster Pipeline: unsupported instruction {}", other.name()),
            }

            let usage = stack_usage(&inst);
            if usage != 0 {
                temp_stack_ptr = temp_stack_ptr.plus(imm(usage) * slot_bytes);
                temp_stack_map[stack_id] = temp_stack_ptr;
            }

            // Periodically rewind the stack every 500 instructions. When SK_HAS_MUSTTAIL is set,
            // rewinds are not actually used; the appendStackRewind call becomes a no-op. On
            // platforms that don't support SK_HAS_MUSTTAIL, rewinding the stack periodically can
            // prevent a potential stack overflow when running a long program.
            let num_pipeline_stages = pipeline.len();
            if num_pipeline_stages - most_recent_rewind > 500 {
                append_stack_rewind_for_non_tailcallers(&mut pipeline, non_tail_rewinds);
                most_recent_rewind = num_pipeline_stages;
            }
        }

        pipeline
    }
}

/// `EmitStackRewindForBackwardsBranch`: before a branch to an already-seen label, emits a stack
/// rewind so that long-running loops don't grow the stack without bound.
fn emit_rewind_for_backwards_branch(
    pipeline: &mut Vec<Stage>,
    most_recent_rewind: &mut usize,
    label_to_instruction_index: &[Option<usize>],
    most_recent_invocation_instruction_idx: usize,
    label_id: i32,
    non_tail_rewinds: bool,
) {
    // If we have already encountered the label associated with this branch, this is a backwards
    // branch.
    let label_instruction_idx =
        label_to_instruction_index[usize::try_from(label_id).expect("label")];
    if let Some(label_instruction_idx) = label_instruction_idx {
        if most_recent_invocation_instruction_idx > label_instruction_idx {
            // The backwards-branch range includes an external invocation to another shader, color
            // filter, blender, or colorspace conversion. In this case, we always emit a stack
            // rewind, since the non-tailcall stages may exist on the stack.
            append_stack_rewind(pipeline);
        } else {
            // The backwards-branch range only includes SkSL ops. If tailcalling is supported,
            // stack rewinding isn't needed. If the platform cannot tailcall, we need to rewind.
            append_stack_rewind_for_non_tailcallers(pipeline, non_tail_rewinds);
        }
        *most_recent_rewind = pipeline.len();
    }
}

/// The trace-mask address for stack `stack_id`: the stack top, one slot down.
fn trace_mask_of(temp_stack_map: &[Addr], stack_id: i32, slot_bytes: isize) -> Addr {
    temp_stack_map[usize::try_from(stack_id).expect("stack")].plus(-slot_bytes)
}

/// Converts a slot index (or immutable index) to a `isize` for address arithmetic.
fn slot_index(slot: Slot) -> isize {
    isize::try_from(slot).expect("slot index is non-negative")
}

/// An `isize` from an `i32` count or immediate.
fn imm(value: i32) -> isize {
    isize::try_from(value).expect("isize holds an i32")
}

/// Appends a stage.
fn push(pipeline: &mut Vec<Stage>, op: BuilderOp, ctx: StageCtx) {
    pipeline.push(Stage { op, ctx });
}

/// `(ProgramOp)((int)op + delta)`: the op `delta` places after `op` in the op list.
fn op_offset(op: BuilderOp, delta: isize) -> BuilderOp {
    let index = isize::try_from(op as usize).expect("op index") + delta;
    BuilderOp::ALL[usize::try_from(index).expect("op index in range")]
}

/// `unpack_nybbles_to_offsets`: unpacks component nybbles into byte offsets pointing at stack
/// slots (`component * N * sizeof(float)`).
fn unpack_nybbles_to_offsets(components: i32, offsets: &mut [u32], lanes: usize) {
    let mut components = components.cast_unsigned();
    for offset in offsets {
        *offset = (components & 0xF) * u32::try_from(lanes * 4).expect("lanes fit");
        components >>= 4;
    }
}

/// `max_packed_nybble`: the largest of the first `num_components` nybbles.
fn max_packed_nybble(components: i32, num_components: usize) -> i32 {
    let mut components = components.cast_unsigned();
    let mut largest = 0_i32;
    for _ in 0..num_components {
        largest = largest.max((components & 0xF).cast_signed());
        components >>= 4;
    }
    largest
}

/// `immutable_data_is_splattable`: whether every immutable value in the range is bit-identical.
fn immutable_data_is_splattable(slots: &SlotData, src: u32, num_slots: i32) -> Option<i32> {
    let first = slots.read_bits(Addr::Slab(isize::try_from(src).ok()?));
    for index in 1..num_slots {
        let at = isize::try_from(src).ok()? + 4 * isize::try_from(index).ok()?;
        if first != slots.read_bits(Addr::Slab(at)) {
            return None;
        }
    }
    Some(first)
}

/// `Program::appendCopy`: copies `num_slots` slots, splitting into groups of four. `slots` is
/// given only for immutable copies, which may become a splat.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn append_copy(
    pipeline: &mut Vec<Stage>,
    slots: Option<&SlotData>,
    base_stage: BuilderOp,
    mut dst: u32,
    dst_stride: usize,
    mut src: u32,
    src_stride: usize,
    mut num_slots: i32,
) {
    while num_slots > 4 {
        // If we are appending a large copy, split it up into groups of four at a time.
        append_copy(
            pipeline, slots, base_stage, dst, dst_stride, src, src_stride, 4,
        );
        let stride_bytes = u32::try_from(16 * dst_stride).expect("stride fits");
        let src_bytes = u32::try_from(16 * src_stride).expect("stride fits");
        dst += stride_bytes;
        src += src_bytes;
        num_slots -= 4;
    }

    if num_slots > 0 {
        // If we are copying immutable data, it might be representable by a splat; this is
        // preferable, since splats are a tiny bit faster than regular copies.
        if let Some(slots) = slots {
            debug_assert_eq!(src_stride, 1);
            if let Some(value) = immutable_data_is_splattable(slots, src, num_slots) {
                let stage = op_offset(
                    BuilderOp::CopyConstant,
                    isize::try_from(num_slots - 1).expect("count"),
                );
                push(pipeline, stage, StageCtx::Constant { dst, value });
                return;
            }
        }

        // We can't use a splat, so emit the requested copy op.
        let stage = op_offset(base_stage, isize::try_from(num_slots - 1).expect("count"));
        push(pipeline, stage, StageCtx::BinaryOp { dst, src });
    }
}

/// `appendCopySlotsUnmasked`.
fn append_copy_slots_unmasked(
    pipeline: &mut Vec<Stage>,
    dst: u32,
    src: u32,
    num_slots: i32,
    lanes: usize,
) {
    append_copy(
        pipeline,
        None,
        BuilderOp::CopySlotUnmasked,
        dst,
        lanes,
        src,
        lanes,
        num_slots,
    );
}

/// `appendCopySlotsMasked`.
fn append_copy_slots_masked(
    pipeline: &mut Vec<Stage>,
    dst: u32,
    src: u32,
    num_slots: i32,
    lanes: usize,
) {
    append_copy(
        pipeline,
        None,
        BuilderOp::CopySlotMasked,
        dst,
        lanes,
        src,
        lanes,
        num_slots,
    );
}

/// `appendCopyImmutableUnmasked`: immutable slots are one scalar apart, so the source stride is 1.
fn append_copy_immutable_unmasked(
    pipeline: &mut Vec<Stage>,
    slots: &SlotData,
    dst: u32,
    src: u32,
    num_slots: i32,
    lanes: usize,
) {
    append_copy(
        pipeline,
        Some(slots),
        BuilderOp::CopyImmutableUnmasked,
        dst,
        lanes,
        src,
        1,
        num_slots,
    );
}

/// `appendSingleSlotUnaryOp`: appends `num_slots` copies of a one-slot op.
fn append_single_slot_unary_op(
    pipeline: &mut Vec<Stage>,
    op: BuilderOp,
    mut dst: Addr,
    num_slots: i32,
    lanes: usize,
) {
    for _ in 0..num_slots {
        push(pipeline, op, StageCtx::Ptr(dst));
        dst = dst.plus(slab_bytes(4 * lanes));
    }
}

/// `appendMultiSlotUnaryOp`: appends the 1-to-4-slot specializations of a unary op.
fn append_multi_slot_unary_op(
    pipeline: &mut Vec<Stage>,
    base: BuilderOp,
    mut dst: Addr,
    mut num_slots: i32,
    lanes: usize,
) {
    while num_slots > 0 {
        let current_slots = num_slots.min(4);
        let stage = op_offset(base, isize::try_from(current_slots - 1).expect("count"));
        push(pipeline, stage, StageCtx::Ptr(dst));
        dst = dst.plus(slab_bytes(16 * lanes));
        num_slots -= 4;
    }
}

/// `appendImmediateBinaryOp`: appends an immediate-mode binary op, one or four slots at a time.
fn append_immediate_binary_op(
    pipeline: &mut Vec<Stage>,
    base: BuilderOp,
    mut dst: u32,
    value: i32,
    mut num_slots: i32,
    lanes: usize,
) {
    let slots_per_stage = if is_multi_slot_immediate_op(base) {
        4
    } else {
        1
    };
    while num_slots > 0 {
        let current_slots = num_slots.min(slots_per_stage);
        let stage = op_offset(base, -isize::try_from(current_slots - 1).expect("count"));
        push(pipeline, stage, StageCtx::Constant { dst, value });
        dst += u32::try_from(usize::try_from(slots_per_stage).unwrap_or(0) * 4 * lanes)
            .expect("offset fits");
        num_slots -= slots_per_stage;
    }
}

/// `appendAdjacentNWayBinaryOp`: `src` must be directly after `dst`.
fn append_adjacent_n_way_binary_op(
    pipeline: &mut Vec<Stage>,
    op: BuilderOp,
    dst: u32,
    src: u32,
    num_slots: i32,
    lanes: usize,
) {
    debug_assert_eq!(
        dst + u32::try_from(4 * lanes * usize::try_from(num_slots).unwrap_or(0)).unwrap_or(0),
        src
    );
    if num_slots > 0 {
        push(pipeline, op, StageCtx::BinaryOp { dst, src });
    }
}

/// `appendAdjacentMultiSlotBinaryOp`.
fn append_adjacent_multi_slot_binary_op(
    pipeline: &mut Vec<Stage>,
    base: BuilderOp,
    dst: u32,
    src: u32,
    num_slots: i32,
    lanes: usize,
) {
    if num_slots > 4 {
        append_adjacent_n_way_binary_op(pipeline, base, dst, src, num_slots, lanes);
        return;
    }
    if num_slots > 0 {
        let specialized = op_offset(base, isize::try_from(num_slots).expect("count"));
        push(
            pipeline,
            specialized,
            StageCtx::Ptr(Addr::Slab(isize::try_from(dst).expect("offset"))),
        );
    }
}

/// `appendAdjacentNWayTernaryOp`.
fn append_adjacent_n_way_ternary_op(
    pipeline: &mut Vec<Stage>,
    op: BuilderOp,
    dst: u32,
    src0: u32,
    src1: u32,
    num_slots: i32,
    lanes: usize,
) {
    let bytes = u32::try_from(4 * lanes * usize::try_from(num_slots).unwrap_or(0)).unwrap_or(0);
    debug_assert_eq!(dst + bytes, src0);
    debug_assert_eq!(src0 + bytes, src1);
    if num_slots > 0 {
        push(
            pipeline,
            op,
            StageCtx::TernaryOp {
                dst,
                delta: src0 - dst,
            },
        );
    }
}

/// `appendAdjacentMultiSlotTernaryOp`.
fn append_adjacent_multi_slot_ternary_op(
    pipeline: &mut Vec<Stage>,
    base: BuilderOp,
    dst: u32,
    src0: u32,
    src1: u32,
    num_slots: i32,
    lanes: usize,
) {
    if num_slots > 4 {
        append_adjacent_n_way_ternary_op(pipeline, base, dst, src0, src1, num_slots, lanes);
        return;
    }
    if num_slots > 0 {
        let specialized = op_offset(base, isize::try_from(num_slots).expect("count"));
        push(
            pipeline,
            specialized,
            StageCtx::Ptr(Addr::Slab(isize::try_from(dst).expect("offset"))),
        );
    }
}

/// `appendStackRewind`: a `stack_rewind` stage.
fn append_stack_rewind(pipeline: &mut Vec<Stage>) {
    push(pipeline, BuilderOp::StackRewind, StageCtx::Null);
}

/// `appendStackRewindForNonTailcallers`: a rewind only when the platform cannot tail-call.
fn append_stack_rewind_for_non_tailcallers(pipeline: &mut Vec<Stage>, non_tail_rewinds: bool) {
    if non_tail_rewinds {
        append_stack_rewind(pipeline);
    }
}

/// `is_immediate_op`-family: ops with a one-slot immediate form (`ALL_IMMEDIATE_BINARY_OP_CASES`).
fn is_immediate_binary_op(op: BuilderOp) -> bool {
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

/// `ALL_SINGLE_SLOT_UNARY_OP_CASES`.
fn is_single_slot_unary_op(op: BuilderOp) -> bool {
    matches!(
        op,
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
    )
}

/// `ALL_MULTI_SLOT_UNARY_OP_CASES`.
fn is_multi_slot_unary_op(op: BuilderOp) -> bool {
    matches!(
        op,
        BuilderOp::AbsInt
            | BuilderOp::CastToFloatFromInt
            | BuilderOp::CastToFloatFromUint
            | BuilderOp::CastToIntFromFloat
            | BuilderOp::CastToUintFromFloat
            | BuilderOp::CeilFloat
            | BuilderOp::FloorFloat
            | BuilderOp::InvsqrtFloat
    )
}

/// `ALL_N_WAY_BINARY_OP_CASES`.
fn is_n_way_binary_op(op: BuilderOp) -> bool {
    matches!(op, BuilderOp::Atan2NFloats | BuilderOp::PowNFloats)
}

/// `ALL_MULTI_SLOT_BINARY_OP_CASES`.
fn is_multi_slot_binary_op(op: BuilderOp) -> bool {
    matches!(
        op,
        BuilderOp::AddNFloats
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
            | BuilderOp::CmpneNInts
    )
}

/// `ALL_N_WAY_TERNARY_OP_CASES`.
fn is_n_way_ternary_op(op: BuilderOp) -> bool {
    matches!(op, BuilderOp::SmoothstepNFloats)
}

/// `ALL_MULTI_SLOT_TERNARY_OP_CASES`.
fn is_multi_slot_ternary_op(op: BuilderOp) -> bool {
    matches!(op, BuilderOp::MixNFloats | BuilderOp::MixNInts)
}

/// `stack_usage`: how far an instruction moves its temp stack's top, in slots.
// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1287-L1368 (chrome/m156)
fn stack_usage(inst: &Instruction) -> i32 {
    match inst.op {
        BuilderOp::PushConditionMask | BuilderOp::PushLoopMask | BuilderOp::PushReturnMask => 1,
        BuilderOp::PushSrcRgba | BuilderOp::PushDstRgba | BuilderOp::PushDeviceXy01 => 4,
        BuilderOp::PushImmutable
        | BuilderOp::PushImmutableIndirect
        | BuilderOp::PushConstant
        | BuilderOp::PushSlots
        | BuilderOp::PushSlotsIndirect
        | BuilderOp::PushUniform
        | BuilderOp::PushUniformIndirect
        | BuilderOp::PushClone
        | BuilderOp::PushCloneFromStack
        | BuilderOp::PushCloneIndirectFromStack
        | BuilderOp::PadStack => inst.imm_a,
        BuilderOp::PopConditionMask
        | BuilderOp::PopLoopMask
        | BuilderOp::PopAndReenableLoopMask
        | BuilderOp::PopReturnMask => -1,
        BuilderOp::PopSrcRgba | BuilderOp::PopDstRgba => -4,
        op if is_n_way_binary_op(op)
            || is_multi_slot_binary_op(op)
            || matches!(op, BuilderOp::DiscardStack | BuilderOp::Select) =>
        {
            -inst.imm_a
        }
        op if is_n_way_ternary_op(op) || is_multi_slot_ternary_op(op) => 2 * -inst.imm_a,
        BuilderOp::Swizzle1 => 1 - inst.imm_a, // consumes immA slots and emits a scalar
        BuilderOp::Swizzle2 => 2 - inst.imm_a, // consumes immA slots and emits a 2-slot vector
        BuilderOp::Swizzle3 => 3 - inst.imm_a, // consumes immA slots and emits a 3-slot vector
        BuilderOp::Swizzle4 => 4 - inst.imm_a, // consumes immA slots and emits a 4-slot vector
        BuilderOp::Dot2Floats => -3,           // consumes two 2-slot vectors, emits one scalar
        // consumes two 3-slot vectors and emits one scalar; or consumes nine slots (N + I + eta)
        // and emits a 4-slot vector (R)
        BuilderOp::Dot3Floats | BuilderOp::Refract4Floats => -5,
        BuilderOp::Dot4Floats => -7, // consumes two 4-slot vectors, emits one scalar
        BuilderOp::MatrixMultiply2 | BuilderOp::MatrixMultiply3 | BuilderOp::MatrixMultiply4 => {
            // consumes the left- and right-matrices; emits result over existing padding slots
            -(inst.imm_a * inst.imm_b + inst.imm_c * inst.imm_d)
        }
        BuilderOp::Shuffle => {
            let consumed = inst.imm_a;
            let generated = inst.imm_b;
            generated - consumed
        }
        _ => 0,
    }
}

/// `Program::tempStackMaxDepths`: how deep each temp stack can get, and checks that the stacks
/// are balanced.
// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1370-L1397 (chrome/m156)
fn temp_stack_max_depths(instructions: &[Instruction]) -> Vec<i32> {
    // Count the number of separate temp stacks that the program uses.
    let mut num_stacks = 1_usize;
    for inst in instructions {
        num_stacks = num_stacks.max(usize::try_from(inst.stack_id + 1).expect("stack id"));
    }

    // Walk the program and calculate how deep each stack can potentially get.
    let mut largest = vec![0_i32; num_stacks];
    let mut current = vec![0_i32; num_stacks];
    for inst in instructions {
        let stack_id = usize::try_from(inst.stack_id).expect("stack id");
        current[stack_id] += stack_usage(inst);
        largest[stack_id] = largest[stack_id].max(current[stack_id]);
        // If we assert here, the generated program has popped off the top of the stack.
        assert!(
            current[stack_id] >= 0,
            "unbalanced temp stack push/pop on stack"
        );
    }

    // Ensure that when the program is complete, our stacks are fully balanced.
    for (stack_id, depth) in current.iter().enumerate() {
        // If we assert here, the generated program has pushed more data than it has popped.
        debug_assert_eq!(
            *depth, 0,
            "unbalanced temp stack push/pop on stack {stack_id}"
        );
    }

    largest
}
