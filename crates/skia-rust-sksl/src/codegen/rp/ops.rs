// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.h (`BuilderOp`, `ProgramOp`,
// `SKRP_EXTENDED_OPS`) and src/core/SkRasterPipelineOpList.h (the native ops, via `rp_ops!`).

//! The op list of the `SkSL` Raster Pipeline builder.
//!
//! `Skia`'s `BuilderOp` is one flat enum: every native raster pipeline op
//! (`SK_RASTER_PIPELINE_OPS_ALL`), then the extended ops (`SKRP_EXTENDED_OPS`), then the
//! builder-only ops. The builder relies on that order (`(BuilderOp)((int)op - 1)` finds the
//! immediate-mode twin of an n-way op, and `swizzle_1 + n - 1` picks a swizzle), so the enum here
//! is generated from the same table as [`skia_rust_simd::rp::Op`], and the extras follow it.
//!
//! `ProgramOp` is the subset of this list that can appear in a finished program. `Skia` keeps two
//! enums; here both are [`BuilderOp`], because every program stage is one of these ops.

// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.h#L52-L140 (chrome/m156)

macro_rules! builder_ops {
    ($($name:ident $variant:ident [$($ctx:tt)*] $hk:ident $lk:ident $task:ident;)*) => {
        /// `SkSL::RP::BuilderOp` (and `ProgramOp`): a native raster pipeline op, an extended
        /// op, or a builder-only op. The discriminants follow `Skia`'s order.
        #[doc(alias = "SkSL::RP::BuilderOp")]
        #[doc(alias = "SkSL::RP::ProgramOp")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum BuilderOp {
            $(
                #[doc = concat!("`SkRasterPipelineOp::", stringify!($name), "`")]
                $variant,
            )*
            // SKRP_EXTENDED_OPS: branch targets, child programs, color space transforms.
            Label,
            InvokeShader,
            InvokeColorFilter,
            InvokeBlender,
            InvokeToLinearSrgb,
            InvokeFromLinearSrgb,
            // Builder-specific ops. These interface with the stack and are converted into
            // program ops during `make_stages`.
            PushClone,
            PushCloneFromStack,
            PushCloneIndirectFromStack,
            PushConstant,
            PushImmutable,
            PushImmutableIndirect,
            PushSlots,
            PushSlotsIndirect,
            PushUniform,
            PushUniformIndirect,
            CopyStackToSlots,
            CopyStackToSlotsUnmasked,
            CopyStackToSlotsIndirect,
            CopyUniformToSlotsUnmasked,
            StoreImmutableValue,
            SwizzleCopyStackToSlots,
            SwizzleCopyStackToSlotsIndirect,
            DiscardStack,
            PadStack,
            Select,
            PushConditionMask,
            PopConditionMask,
            PushLoopMask,
            PopLoopMask,
            PopAndReenableLoopMask,
            PushReturnMask,
            PopReturnMask,
            PushSrcRgba,
            PushDstRgba,
            PushDeviceXy01,
            PopSrcRgba,
            PopDstRgba,
            TraceVarIndirect,
            BranchIfNoActiveLanesOnStackTopEqual,
            Unsupported,
        }

        impl BuilderOp {
            /// Every op, in discriminant order (`ALL[op as usize] == op`).
            pub const ALL: &'static [BuilderOp] = &[
                $(BuilderOp::$variant,)*
                BuilderOp::Label,
                BuilderOp::InvokeShader,
                BuilderOp::InvokeColorFilter,
                BuilderOp::InvokeBlender,
                BuilderOp::InvokeToLinearSrgb,
                BuilderOp::InvokeFromLinearSrgb,
                BuilderOp::PushClone,
                BuilderOp::PushCloneFromStack,
                BuilderOp::PushCloneIndirectFromStack,
                BuilderOp::PushConstant,
                BuilderOp::PushImmutable,
                BuilderOp::PushImmutableIndirect,
                BuilderOp::PushSlots,
                BuilderOp::PushSlotsIndirect,
                BuilderOp::PushUniform,
                BuilderOp::PushUniformIndirect,
                BuilderOp::CopyStackToSlots,
                BuilderOp::CopyStackToSlotsUnmasked,
                BuilderOp::CopyStackToSlotsIndirect,
                BuilderOp::CopyUniformToSlotsUnmasked,
                BuilderOp::StoreImmutableValue,
                BuilderOp::SwizzleCopyStackToSlots,
                BuilderOp::SwizzleCopyStackToSlotsIndirect,
                BuilderOp::DiscardStack,
                BuilderOp::PadStack,
                BuilderOp::Select,
                BuilderOp::PushConditionMask,
                BuilderOp::PopConditionMask,
                BuilderOp::PushLoopMask,
                BuilderOp::PopLoopMask,
                BuilderOp::PopAndReenableLoopMask,
                BuilderOp::PushReturnMask,
                BuilderOp::PopReturnMask,
                BuilderOp::PushSrcRgba,
                BuilderOp::PushDstRgba,
                BuilderOp::PushDeviceXy01,
                BuilderOp::PopSrcRgba,
                BuilderOp::PopDstRgba,
                BuilderOp::TraceVarIndirect,
                BuilderOp::BranchIfNoActiveLanesOnStackTopEqual,
                BuilderOp::Unsupported,
            ];

            /// The op's name as `Skia` prints it (`stringify!` of the op-list entry). The extended
            /// and builder-only ops use their `Skia` enum names.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(BuilderOp::$variant => stringify!($name),)*
                    BuilderOp::Label => "label",
                    BuilderOp::InvokeShader => "invoke_shader",
                    BuilderOp::InvokeColorFilter => "invoke_color_filter",
                    BuilderOp::InvokeBlender => "invoke_blender",
                    BuilderOp::InvokeToLinearSrgb => "invoke_to_linear_srgb",
                    BuilderOp::InvokeFromLinearSrgb => "invoke_from_linear_srgb",
                    BuilderOp::PushClone => "push_clone",
                    BuilderOp::PushCloneFromStack => "push_clone_from_stack",
                    BuilderOp::PushCloneIndirectFromStack => "push_clone_indirect_from_stack",
                    BuilderOp::PushConstant => "push_constant",
                    BuilderOp::PushImmutable => "push_immutable",
                    BuilderOp::PushImmutableIndirect => "push_immutable_indirect",
                    BuilderOp::PushSlots => "push_slots",
                    BuilderOp::PushSlotsIndirect => "push_slots_indirect",
                    BuilderOp::PushUniform => "push_uniform",
                    BuilderOp::PushUniformIndirect => "push_uniform_indirect",
                    BuilderOp::CopyStackToSlots => "copy_stack_to_slots",
                    BuilderOp::CopyStackToSlotsUnmasked => "copy_stack_to_slots_unmasked",
                    BuilderOp::CopyStackToSlotsIndirect => "copy_stack_to_slots_indirect",
                    BuilderOp::CopyUniformToSlotsUnmasked => "copy_uniform_to_slots_unmasked",
                    BuilderOp::StoreImmutableValue => "store_immutable_value",
                    BuilderOp::SwizzleCopyStackToSlots => "swizzle_copy_stack_to_slots",
                    BuilderOp::SwizzleCopyStackToSlotsIndirect => {
                        "swizzle_copy_stack_to_slots_indirect"
                    }
                    BuilderOp::DiscardStack => "discard_stack",
                    BuilderOp::PadStack => "pad_stack",
                    BuilderOp::Select => "select",
                    BuilderOp::PushConditionMask => "push_condition_mask",
                    BuilderOp::PopConditionMask => "pop_condition_mask",
                    BuilderOp::PushLoopMask => "push_loop_mask",
                    BuilderOp::PopLoopMask => "pop_loop_mask",
                    BuilderOp::PopAndReenableLoopMask => "pop_and_reenable_loop_mask",
                    BuilderOp::PushReturnMask => "push_return_mask",
                    BuilderOp::PopReturnMask => "pop_return_mask",
                    BuilderOp::PushSrcRgba => "push_src_rgba",
                    BuilderOp::PushDstRgba => "push_dst_rgba",
                    BuilderOp::PushDeviceXy01 => "push_device_xy01",
                    BuilderOp::PopSrcRgba => "pop_src_rgba",
                    BuilderOp::PopDstRgba => "pop_dst_rgba",
                    BuilderOp::TraceVarIndirect => "trace_var_indirect",
                    BuilderOp::BranchIfNoActiveLanesOnStackTopEqual => {
                        "branch_if_no_active_lanes_on_stack_top_equal"
                    }
                    BuilderOp::Unsupported => "unsupported",
                }
            }
        }
    };
}

skia_rust_simd::rp_op_table!(builder_ops);

/// `ProgramOp`: the ops that can appear in a finished program. See the module docs.
#[doc(alias = "SkSL::RP::ProgramOp")]
pub type ProgramOp = BuilderOp;
