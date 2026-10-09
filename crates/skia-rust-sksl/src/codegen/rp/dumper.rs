// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp (`Program::Dumper` and
// `Program::dump`).

//! The `.skrp` text dump of a Raster Pipeline program: one line per stage, with slot names taken
//! from the debug trace when there is one. The text is byte-exact with `Skia`'s.

// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L2490-L3850 (chrome/m156)

use std::collections::HashMap;

use super::ops::BuilderOp;
use super::program::{Addr, Program, SlotData, Stage, StageCtx};
use crate::skstd::to_string_f32;

/// `SkSL::String::printf("%-30.*s %s")`-style padding: the op name, left-justified in 30 columns.
fn pad_name(name: &str) -> String {
    format!("{name:<30}")
}

/// `SkSL::String::Separator`: an empty string first, then `", "`.
struct Separator(bool);

impl Separator {
    fn new() -> Self {
        Separator(false)
    }

    fn next(&mut self) -> &'static str {
        if self.0 {
            ", "
        } else {
            self.0 = true;
            ""
        }
    }
}

/// `Program::Dumper`: holds the lowered stages and the names a dump needs.
struct Dumper<'a> {
    program: &'a Program,
    /// `N`: the highp lane count.
    lanes: usize,
    /// `fStages`.
    stages: Vec<Stage>,
    /// `fSlotNameList`: unique names for each value slot (empty without a debug trace).
    slot_name_list: Vec<String>,
    /// `fLabelToStageMap`: label ID to stage index.
    label_to_stage: HashMap<i32, usize>,
    /// `fSlots`.
    slots: SlotData,
    /// `fUniforms`: the number of uniform slots (all zero).
    uniform_slots: usize,
}

impl Dumper<'_> {
    fn n(&self) -> isize {
        isize::try_from(self.lanes).expect("lanes fit in isize")
    }

    /// `values.size()` in slots: the number of value-slot floats (`N * numValueSlots`).
    fn values_end_slot(&self) -> isize {
        isize::try_from(self.slots.num_values).expect("slot count fits")
    }

    /// `buildLabelToStageMap`: finds the labels in the program, and keeps track of their offsets.
    fn build_label_to_stage_map(&mut self) {
        for (index, stage) in self.stages.iter().enumerate() {
            if stage.op == BuilderOp::Label {
                let StageCtx::Int(label_id) = stage.ctx else {
                    unreachable!("label stages carry their ID");
                };
                // Labels are unique; the map is only queried by ID.
                self.label_to_stage.insert(label_id, index);
            }
        }
    }

    /// `buildUniqueSlotNameList`: gives each variable slot a unique name, disambiguating
    /// variables that share a name and source position with subscripts `₀`, `₁`, ….
    fn build_unique_slot_name_list(&mut self) {
        let program = self.program;
        let Some(trace) = program.debug_trace.as_deref() else {
            return;
        };
        self.slot_name_list.reserve(trace.slot_info.len());

        // The map consists of <variable name, <source position, unique name>>.
        let mut unique_name_map: HashMap<&str, HashMap<usize, String>> = HashMap::new();
        for slot_info in &trace.slot_info {
            // Look up this variable by its name and source position.
            let pos = if slot_info.pos.valid() {
                usize::try_from(slot_info.pos.start_offset()).unwrap_or(0)
            } else {
                0
            };
            let position_map = unique_name_map.entry(slot_info.name.as_str()).or_default();

            // Have we seen this variable name/position combination before?
            if position_map.get(&pos).is_none_or(String::is_empty) {
                // This is a unique name/position pair. The map has to count the new entry too.
                let count = position_map.len() + usize::from(!position_map.contains_key(&pos));
                let mut unique_name = slot_info.name.clone();

                // But if it's not a unique _name_, it deserves a subscript to disambiguate it.
                let subscript = count - 1;
                if subscript > 0 {
                    for digit in subscript.to_string().chars() {
                        // U+2080 through U+2089 (₀₁₂₃₄₅₆₇₈₉).
                        let code = 0x2080 + (u32::from(digit) - u32::from('0'));
                        unique_name.push(char::from_u32(code).expect("subscript digit"));
                    }
                }
                position_map.insert(pos, unique_name);
            }

            let name = position_map[&pos].clone();
            self.slot_name_list.push(name);
        }
    }

    /// `branchOffset`: interprets the context as a branch to a label.
    fn branch_offset(&self, label_id: i32, index: usize) -> String {
        let target_index = *self
            .label_to_stage
            .get(&label_id)
            .expect("branch targets a label in the program");
        let relative =
            isize::try_from(target_index).expect("index") - isize::try_from(index).expect("index");
        format!(
            "{:+} (label {} at #{})",
            relative,
            label_id,
            target_index + 1
        )
    }

    /// `imm`: prints a 32-bit immediate of unknown type (int or float).
    fn imm_bits(bits: i32, show_as_float: bool) -> String {
        // Special case exact zero as "0" for readability (vs `0x00000000 (0.0)`).
        if bits == 0 {
            return "0".to_owned();
        }
        // Start with `0x3F800000` as a baseline.
        let mut text = format!("0x{:08X}", bits.cast_unsigned());

        // Extend it to `0x3F800000 (1.0)` for finite floating point values.
        let value = f32::from_bits(bits.cast_unsigned());
        if show_as_float && value.is_finite() {
            text.push_str(" (");
            text.push_str(&to_string_f32(value));
            text.push(')');
        }
        text
    }

    /// `immCtx`: interprets the context as a 32-bit immediate (labels and child indices).
    fn imm_ctx(ctx: StageCtx, show_as_float: bool) -> String {
        let bits = match ctx {
            StageCtx::Int(value) => value,
            StageCtx::Null => 0,
            other => unreachable!("immediate context {other:?}"),
        };
        Self::imm_bits(bits, show_as_float)
    }

    /// `asRange`: `1` for one slot, `1..3` for a range.
    fn as_range(first: isize, count: isize) -> String {
        let mut text = first.to_string();
        if count > 1 {
            text.push_str("..");
            text.push_str(&(first + count - 1).to_string());
        }
        text
    }

    /// `slotOrUniformName`: a readable name for a range of slots, e.g. `val`, `val(0..1)`,
    /// `foo, bar`, `foo(3), bar(0)`.
    fn slot_or_uniform_name(
        debug_info: &[crate::tracing::SlotDebugInfo],
        names: &[String],
        mut range: (isize, isize),
    ) -> String {
        let mut text = String::new();
        let mut separator = Separator::new();
        while range.1 > 0 {
            let slot_info = &debug_info[usize::try_from(range.0).expect("slot index")];
            text.push_str(separator.next());
            let name = if names.is_empty() {
                slot_info.name.as_str()
            } else {
                names[usize::try_from(range.0).expect("slot index")].as_str()
            };
            text.push_str(name);

            // Figure out how many slots we can chomp in this iteration.
            let entire_variable = isize::from(slot_info.columns) * isize::from(slot_info.rows);
            let slots_to_chomp = range
                .1
                .min(entire_variable - isize::from(slot_info.component_index));
            // If we aren't consuming an entire variable, from first slot to last...
            if slots_to_chomp != entire_variable {
                // ... decorate it with a range suffix.
                text.push('(');
                text.push_str(&Self::as_range(
                    isize::from(slot_info.component_index),
                    slots_to_chomp,
                ));
                text.push(')');
            }
            range.0 += slots_to_chomp;
            range.1 -= slots_to_chomp;
        }
        text
    }

    /// `uniformPtrCtx`: a uniform range, by name if the program has debug info.
    fn uniform_ptr_ctx(&self, addr: Addr, num_slots: isize) -> Option<String> {
        let Addr::Uniform(offset) = addr else {
            return None;
        };
        let uniform_idx = offset / 4;
        let end = uniform_idx + num_slots;
        if uniform_idx >= 0 && end <= isize::try_from(self.uniform_slots).expect("count") {
            if let Some(trace) = self.program.debug_trace.as_deref() {
                // Handle pointers to named uniform slots.
                let name =
                    Self::slot_or_uniform_name(&trace.uniform_info, &[], (uniform_idx, num_slots));
                if !name.is_empty() {
                    return Some(name);
                }
            }
            // Handle pointers to uniforms (when no debug info exists).
            return Some(format!("u{}", Self::as_range(uniform_idx, num_slots)));
        }
        None
    }

    /// `valuePtrCtx`: a value-slot range, by name if the program has debug info.
    fn value_ptr_ctx(&self, addr: Addr, num_slots: isize) -> Option<String> {
        let Addr::Slab(offset) = addr else {
            return None;
        };
        let n = self.n();
        // Floats, as in the C++: the pointer and its end are `N * numSlots` floats apart.
        let float_index = offset / 4;
        let end = float_index + n * num_slots;
        let values_end = n * self.values_end_slot();
        if float_index >= 0 && end <= values_end {
            debug_assert_eq!(float_index % n, 0);
            let value_idx = float_index / n;
            if let Some(trace) = self.program.debug_trace.as_deref() {
                // Handle pointers to named value slots.
                let name = Self::slot_or_uniform_name(
                    &trace.slot_info,
                    &self.slot_name_list,
                    (value_idx, num_slots),
                );
                if !name.is_empty() {
                    return Some(name);
                }
            }
            // Handle pointers to value slots (when no debug info exists).
            return Some(format!("v{}", Self::as_range(value_idx, num_slots)));
        }
        None
    }

    /// `immutablePtrCtx`: an immutable-slot range, with its values.
    fn immutable_ptr_ctx(&self, addr: Addr, num_slots: isize) -> Option<String> {
        let Addr::Slab(offset) = addr else {
            return None;
        };
        let float_index = offset / 4;
        let base = self.n()
            * (self.values_end_slot() + isize::try_from(self.slots.num_stack).expect("count"));
        let count = isize::try_from(self.slots.immutable.len()).expect("count");
        if float_index >= base && float_index + num_slots <= base + count {
            let index = float_index - base;
            return Some(format!(
                "i{} {}",
                Self::as_range(index, num_slots),
                self.multi_imm_ctx(addr, num_slots)
            ));
        }
        None
    }

    /// `multiImmCtx`: a pointer to `count` immediates, printed by name if it is a uniform.
    fn multi_imm_ctx(&self, addr: Addr, count: isize) -> String {
        // If this is a uniform, print it by name.
        if let Some(text) = self.uniform_ptr_ctx(addr, count) {
            return text;
        }
        let bits_at = |k: isize| {
            let step = 4 * k;
            let at = match addr {
                Addr::Slab(o) => Addr::Slab(o + step),
                Addr::Uniform(o) => Addr::Uniform(o + step),
                Addr::Null => Addr::Null,
            };
            self.slots.read_bits(at)
        };
        // Emit a single bracketed immediate.
        if count == 1 {
            return format!("[{}]", Self::imm_bits(bits_at(0), true));
        }
        // Emit a list like `[0x00000000 (0.0), 0x3F800000 (1.0)]`.
        let mut text = String::from("[");
        let mut separator = Separator::new();
        for k in 0..count {
            text.push_str(separator.next());
            text.push_str(&Self::imm_bits(bits_at(k), true));
        }
        text.push(']');
        text
    }

    /// `ptrCtx`: interprets the context value as a generic pointer.
    fn ptr_ctx(&self, addr: Addr, num_slots: isize) -> String {
        // Check for uniform, value, and immutable pointers.
        if let Some(text) = self.uniform_ptr_ctx(addr, num_slots) {
            return text;
        }
        if let Some(text) = self.value_ptr_ctx(addr, num_slots) {
            return text;
        }
        if let Some(text) = self.immutable_ptr_ctx(addr, num_slots) {
            return text;
        }
        // Handle pointers to temporary stack slots.
        if let Addr::Slab(offset) = addr {
            let float_index = offset / 4;
            let stack_start = self.n() * self.values_end_slot();
            let stack_end =
                stack_start + self.n() * isize::try_from(self.slots.num_stack).expect("count");
            if float_index >= stack_start && float_index < stack_end {
                let stack_idx = float_index - stack_start;
                debug_assert_eq!(stack_idx % self.n(), 0);
                return format!("${}", Self::as_range(stack_idx / self.n(), num_slots));
            }
        }
        // This pointer is out of our expected bounds; this generally isn't expected to happen.
        format!("ExternalPtr({})", Self::as_range(0, num_slots))
    }

    /// `offsetCtx`: a slab offset interpreted as a slot range.
    fn offset_ctx(&self, offset: u32, num_slots: isize) -> String {
        self.ptr_ctx(
            Addr::Slab(isize::try_from(offset).expect("offset")),
            num_slots,
        )
    }

    /// `constantCtx`: a `ConstantCtx`, as (destination, value).
    fn constant_ctx(&self, ctx: StageCtx, slots: isize, show_as_float: bool) -> (String, String) {
        let StageCtx::Constant { dst, value } = ctx else {
            unreachable!("constant context {ctx:?}");
        };
        (
            self.offset_ctx(dst, slots),
            Self::imm_bits(value, show_as_float),
        )
    }

    /// `binaryOpCtx`: a `BinaryOpCtx` for `copy_n_slots` (the slot count comes from the op).
    fn binary_op_ctx(&self, ctx: StageCtx, num_slots: isize) -> (String, String) {
        let StageCtx::BinaryOp { dst, src } = ctx else {
            unreachable!("binary-op context {ctx:?}");
        };
        (
            self.offset_ctx(dst, num_slots),
            self.offset_ctx(src, num_slots),
        )
    }

    /// `copyUniformCtx`: a `UniformCtx` for `copy_n_uniforms`.
    fn copy_uniform_ctx(&self, ctx: StageCtx, num_slots: isize) -> (String, String) {
        let StageCtx::Uniform { dst, src } = ctx else {
            unreachable!("uniform context {ctx:?}");
        };
        (
            self.ptr_ctx(dst, num_slots),
            self.multi_imm_ctx(src, num_slots),
        )
    }

    /// `adjacentPtrCtx`: a pointer to two adjacent values.
    fn adjacent_ptr_ctx(&self, addr: Addr, num_slots: isize) -> (String, String) {
        let next = addr.plus(4 * self.n() * num_slots);
        (self.ptr_ctx(addr, num_slots), self.ptr_ctx(next, num_slots))
    }

    /// `adjacentOffsetCtx`.
    fn adjacent_offset_ctx(&self, offset: u32, num_slots: isize) -> (String, String) {
        self.adjacent_ptr_ctx(
            Addr::Slab(isize::try_from(offset).expect("offset")),
            num_slots,
        )
    }

    /// `adjacentBinaryOpCtx`: a `BinaryOpCtx` whose slot count is the distance between pointers.
    fn adjacent_binary_op_ctx(&self, ctx: StageCtx) -> (String, String) {
        let StageCtx::BinaryOp { dst, src } = ctx else {
            unreachable!("binary-op context {ctx:?}");
        };
        let num_slots = (isize::try_from(src).expect("offset")
            - isize::try_from(dst).expect("offset"))
            / (self.n() * 4);
        self.adjacent_offset_ctx(dst, num_slots)
    }

    /// `adjacent3PtrCtx`: a pointer to three adjacent values.
    fn adjacent3_ptr_ctx(&self, addr: Addr, num_slots: isize) -> (String, String, String) {
        let n = self.n();
        (
            self.ptr_ctx(addr, num_slots),
            self.ptr_ctx(addr.plus(4 * n * num_slots), num_slots),
            self.ptr_ctx(addr.plus(4 * 2 * n * num_slots), num_slots),
        )
    }

    /// `adjacent3OffsetCtx`.
    fn adjacent3_offset_ctx(&self, offset: u32, num_slots: isize) -> (String, String, String) {
        self.adjacent3_ptr_ctx(
            Addr::Slab(isize::try_from(offset).expect("offset")),
            num_slots,
        )
    }

    /// `adjacentTernaryOpCtx`: a `TernaryOpCtx`, with the slot count inferred from `delta`.
    fn adjacent_ternary_op_ctx(&self, ctx: StageCtx) -> (String, String, String) {
        let StageCtx::TernaryOp { dst, delta } = ctx else {
            unreachable!("ternary-op context {ctx:?}");
        };
        let num_slots = isize::try_from(delta).expect("offset") / (4 * self.n());
        self.adjacent3_offset_ctx(dst, num_slots)
    }

    /// `swizzleOffsetSpan`: the swizzle text (`xyzw`) for a span of byte offsets.
    fn swizzle_offset_span(&self, offsets: &[u32]) -> String {
        let n4 = u32::try_from(4 * self.lanes).expect("lanes fit");
        offsets
            .iter()
            .map(|&offset| {
                if offset == 0 {
                    'x'
                } else if offset == n4 {
                    'y'
                } else if offset == 2 * n4 {
                    'z'
                } else if offset == 3 * n4 {
                    'w'
                } else {
                    '?'
                }
            })
            .collect()
    }

    /// `swizzleWidth`: the effective width of a swizzle, from its components.
    fn swizzle_width(&self, offsets: &[u32]) -> isize {
        let n4 = u32::try_from(4 * self.lanes).expect("lanes fit");
        let highest_component =
            isize::try_from(offsets.iter().copied().max().unwrap_or(0) / n4).expect("component");
        let swizzle_width = isize::try_from(offsets.len()).expect("count");
        swizzle_width.max(highest_component + 1)
    }

    /// `swizzlePtr`: a swizzled pointer, e.g. `($0).xy`.
    fn swizzle_ptr(&self, addr: Addr, offsets: &[u32]) -> String {
        format!(
            "({}).{}",
            self.ptr_ctx(addr, self.swizzle_width(offsets)),
            self.swizzle_offset_span(offsets)
        )
    }

    /// `swizzleCtx`: a `SwizzleCtx` for `swizzle_1`..4.
    fn swizzle_ctx(&self, op: BuilderOp, ctx: StageCtx) -> (String, String) {
        let StageCtx::Swizzle { dst, offsets } = ctx else {
            unreachable!("swizzle context {ctx:?}");
        };
        let dest_slots = isize::try_from(op as usize).expect("op")
            - isize::try_from(BuilderOp::Swizzle1 as usize).expect("op")
            + 1;
        let slots = usize::try_from(dest_slots).expect("slot count");
        (
            self.offset_ctx(dst, dest_slots),
            self.swizzle_ptr(
                Addr::Slab(isize::try_from(dst).expect("offset")),
                &offsets[..slots],
            ),
        )
    }

    /// `swizzleCopyCtx`: a `SwizzleCopyCtx` for `swizzle_copy_slot_masked`..4.
    fn swizzle_copy_ctx(&self, op: BuilderOp, ctx: StageCtx) -> (String, String) {
        let StageCtx::SwizzleCopy { dst, src, offsets } = ctx else {
            unreachable!("swizzle-copy context {ctx:?}");
        };
        let dest_slots = isize::try_from(op as usize).expect("op")
            - isize::try_from(BuilderOp::SwizzleCopySlotMasked as usize).expect("op")
            + 1;
        let slots = usize::try_from(dest_slots).expect("slot count");
        (
            self.swizzle_ptr(dst, &offsets[..slots]),
            self.ptr_ctx(src, dest_slots),
        )
    }

    /// `shuffleCtx`: a `ShuffleCtx`, as (destination, source list).
    fn shuffle_ctx(&self, ctx: StageCtx) -> (String, String) {
        let StageCtx::Shuffle {
            ptr,
            count,
            offsets,
        } = ctx
        else {
            unreachable!("shuffle context {ctx:?}");
        };
        let dst = self.ptr_ctx(ptr, isize::try_from(count).expect("count"));
        let mut src = format!("({dst})[");
        let slot_bytes = u32::try_from(4 * self.lanes).expect("lanes fit");
        for &offset in &offsets[..usize::try_from(count).expect("count")] {
            if offset % slot_bytes != 0 {
                src.push('?');
            } else {
                src.push_str(&(offset / slot_bytes).to_string());
            }
            src.push(' ');
        }
        // `src.back() = ']'`: replaces the trailing space (or the opening bracket).
        src.pop();
        src.push(']');
        (dst, src)
    }

    /// `matrixMultiply`: a `MatrixMultiplyCtx`, as (result, left, right) text.
    fn matrix_multiply(&self, ctx: StageCtx) -> (String, String, String) {
        let StageCtx::MatrixMultiply {
            dst,
            left_columns,
            left_rows,
            right_columns,
            right_rows,
        } = ctx
        else {
            unreachable!("matrix-multiply context {ctx:?}");
        };
        let n4 = u32::try_from(4 * self.lanes).expect("lanes fit");
        let left_matrix = left_columns * left_rows;
        let right_matrix = right_columns * right_rows;
        let result_matrix = right_columns * left_rows;
        let left_offset = dst + u32::try_from(right_columns * left_rows).expect("count") * n4;
        let right_offset =
            left_offset + u32::try_from(left_columns * left_rows).expect("count") * n4;
        (
            format!(
                "mat{}x{}({})",
                right_columns,
                left_rows,
                self.offset_ctx(dst, isize::try_from(result_matrix).expect("count"))
            ),
            format!(
                "mat{}x{}({})",
                left_columns,
                left_rows,
                self.offset_ctx(left_offset, isize::try_from(left_matrix).expect("count"))
            ),
            format!(
                "mat{}x{}({})",
                right_columns,
                right_rows,
                self.offset_ctx(right_offset, isize::try_from(right_matrix).expect("count"))
            ),
        )
    }

    /// The instruction count and the invocation count, as the dump's first line.
    fn instruction_summary(&self) -> String {
        let mut invocation_count = 0;
        let mut instruction_count = 0;
        for stage in &self.stages {
            match stage.op {
                // consumes zero instructions
                BuilderOp::Label => {}
                BuilderOp::InvokeShader
                | BuilderOp::InvokeColorFilter
                | BuilderOp::InvokeBlender
                | BuilderOp::InvokeToLinearSrgb
                | BuilderOp::InvokeFromLinearSrgb => invocation_count += 1,
                _ => instruction_count += 1,
            }
        }
        let mut text = format!("{instruction_count} instructions");
        if invocation_count > 0 {
            text.push_str(", ");
            text.push_str(&invocation_count.to_string());
            text.push_str(" invocations");
        }
        text.push_str("\n\n");
        text
    }
}

impl Program {
    /// `Program::dump`: the `.skrp` text of this program. `write_instruction_count` prepends the
    /// instruction and invocation counts, as `skslc` does.
    #[must_use]
    pub fn dump(&self, write_instruction_count: bool) -> String {
        self.dump_with(write_instruction_count, false)
    }

    /// `Program::dump`, with the build configuration that decides whether `stack_rewind` stages
    /// appear: `non_tail_rewinds` is `SKSL_STANDALONE || !SK_HAS_MUSTTAIL`. The `skslc` tool is
    /// `SKSL_STANDALONE`, so its goldens have the rewinds; Skia's unit tests do not.
    #[must_use]
    pub fn dump_with(&self, write_instruction_count: bool, non_tail_rewinds: bool) -> String {
        let lanes = skia_rust_simd::selection().tier.highp_stride();
        let mut slots = self.slot_data(lanes);
        let uniforms = vec![0_i32; usize::try_from(self.num_uniform_slots).unwrap_or(0)];

        // Turn this program into an array of Raster Pipeline stages.
        let stages = self.make_stages(&uniforms, &mut slots, non_tail_rewinds);
        let uniform_slots = uniforms.len();
        let mut dumper = Dumper {
            program: self,
            lanes,
            stages,
            slot_name_list: Vec::new(),
            label_to_stage: HashMap::new(),
            slots,
            uniform_slots,
        };

        // Assemble lookup tables for program labels and slot names.
        dumper.build_label_to_stage_map();
        dumper.build_unique_slot_name_list();

        let mut out = String::new();
        if write_instruction_count {
            out.push_str(&dumper.instruction_summary());
        }
        dumper.dump_immutables(&mut out);
        dumper.dump_stages(&mut out);
        out
    }
}

impl Dumper<'_> {
    /// Emits the program's immutable data.
    fn dump_immutables(&self, out: &mut String) {
        let mut header = "[immutable slots]\n";
        let mut footer = "";
        for inst in &self.program.instructions {
            if inst.op == BuilderOp::StoreImmutableValue {
                out.push_str(header);
                out.push('i');
                out.push_str(&inst.slot_a.to_string());
                out.push_str(" = ");
                out.push_str(&Self::imm_bits(inst.imm_a, true));
                out.push('\n');

                header = "";
                footer = "\n";
            }
        }
        out.push_str(footer);
    }

    /// Emits the program's instruction list, one line per stage.
    #[allow(clippy::too_many_lines)] // one arm per op, as in Skia's dumper
    fn dump_stages(&self, out: &mut String) {
        for index in 0..self.stages.len() {
            let stage = self.stages[index];
            let op = stage.op;
            let ctx = stage.ctx;

            let mut arg1 = String::new();
            let mut arg2 = String::new();
            let mut arg3 = String::new();
            let mut swizzle = String::new();

            match op {
                BuilderOp::Label
                | BuilderOp::InvokeShader
                | BuilderOp::InvokeColorFilter
                | BuilderOp::InvokeBlender => {
                    arg1 = Self::imm_ctx(ctx, false);
                }
                BuilderOp::CaseOp => {
                    let StageCtx::CaseOp {
                        expected_value,
                        offset,
                    } = ctx
                    else {
                        unreachable!("case_op context");
                    };
                    arg1 = self.offset_ctx(offset, 1);
                    arg2 =
                        self.offset_ctx(offset + 4 * u32::try_from(self.lanes).expect("lanes"), 1);
                    arg3 = Self::imm_bits(expected_value, false);
                }
                BuilderOp::Swizzle1
                | BuilderOp::Swizzle2
                | BuilderOp::Swizzle3
                | BuilderOp::Swizzle4 => {
                    (arg1, arg2) = self.swizzle_ctx(op, ctx);
                }
                BuilderOp::SwizzleCopySlotMasked
                | BuilderOp::SwizzleCopy2SlotsMasked
                | BuilderOp::SwizzleCopy3SlotsMasked
                | BuilderOp::SwizzleCopy4SlotsMasked => {
                    (arg1, arg2) = self.swizzle_copy_ctx(op, ctx);
                }
                BuilderOp::Refract4Floats => {
                    let StageCtx::Ptr(addr) = ctx else {
                        unreachable!("refract context");
                    };
                    (arg1, arg2) = self.adjacent_ptr_ctx(addr, 4);
                    arg3 = self.ptr_ctx(addr.plus(32 * self.n()), 1);
                }
                BuilderOp::Dot2Floats | BuilderOp::Dot3Floats | BuilderOp::Dot4Floats => {
                    let StageCtx::Ptr(addr) = ctx else {
                        unreachable!("dot context");
                    };
                    let slots = match op {
                        BuilderOp::Dot2Floats => 2,
                        BuilderOp::Dot3Floats => 3,
                        _ => 4,
                    };
                    arg1 = self.ptr_ctx(addr, 1);
                    (arg2, arg3) = self.adjacent_ptr_ctx(addr, slots);
                }
                BuilderOp::Shuffle => {
                    (arg1, arg2) = self.shuffle_ctx(ctx);
                }
                BuilderOp::MatrixMultiply2
                | BuilderOp::MatrixMultiply3
                | BuilderOp::MatrixMultiply4 => {
                    (arg1, arg2, arg3) = self.matrix_multiply(ctx);
                }
                BuilderOp::LoadConditionMask
                | BuilderOp::StoreConditionMask
                | BuilderOp::LoadLoopMask
                | BuilderOp::StoreLoopMask
                | BuilderOp::MergeLoopMask
                | BuilderOp::ReenableLoopMask
                | BuilderOp::LoadReturnMask
                | BuilderOp::StoreReturnMask
                | BuilderOp::ContinueOp
                | BuilderOp::CastToFloatFromInt
                | BuilderOp::CastToFloatFromUint
                | BuilderOp::CastToIntFromFloat
                | BuilderOp::CastToUintFromFloat
                | BuilderOp::AbsInt
                | BuilderOp::AcosFloat
                | BuilderOp::AsinFloat
                | BuilderOp::AtanFloat
                | BuilderOp::CeilFloat
                | BuilderOp::CosFloat
                | BuilderOp::ExpFloat
                | BuilderOp::Exp2Float
                | BuilderOp::LogFloat
                | BuilderOp::Log2Float
                | BuilderOp::FloorFloat
                | BuilderOp::InvsqrtFloat
                | BuilderOp::SinFloat
                | BuilderOp::SqrtFloat
                | BuilderOp::TanFloat => {
                    arg1 = self.ptr_ctx(ptr_of(ctx), 1);
                }
                BuilderOp::StoreSrcRg
                | BuilderOp::CastToFloatFrom2Ints
                | BuilderOp::CastToFloatFrom2Uints
                | BuilderOp::CastToIntFrom2Floats
                | BuilderOp::CastToUintFrom2Floats
                | BuilderOp::Abs2Ints
                | BuilderOp::Ceil2Floats
                | BuilderOp::Floor2Floats
                | BuilderOp::Invsqrt2Floats => {
                    arg1 = self.ptr_ctx(ptr_of(ctx), 2);
                }
                BuilderOp::CastToFloatFrom3Ints
                | BuilderOp::CastToFloatFrom3Uints
                | BuilderOp::CastToIntFrom3Floats
                | BuilderOp::CastToUintFrom3Floats
                | BuilderOp::Abs3Ints
                | BuilderOp::Ceil3Floats
                | BuilderOp::Floor3Floats
                | BuilderOp::Invsqrt3Floats => {
                    arg1 = self.ptr_ctx(ptr_of(ctx), 3);
                }
                BuilderOp::LoadSrc
                | BuilderOp::LoadDst
                | BuilderOp::ExchangeSrc
                | BuilderOp::StoreSrc
                | BuilderOp::StoreDst
                | BuilderOp::StoreDeviceXy01
                | BuilderOp::InvokeToLinearSrgb
                | BuilderOp::InvokeFromLinearSrgb
                | BuilderOp::CastToFloatFrom4Ints
                | BuilderOp::CastToFloatFrom4Uints
                | BuilderOp::CastToIntFrom4Floats
                | BuilderOp::CastToUintFrom4Floats
                | BuilderOp::Abs4Ints
                | BuilderOp::Ceil4Floats
                | BuilderOp::Floor4Floats
                | BuilderOp::Invsqrt4Floats
                | BuilderOp::InverseMat2 => {
                    arg1 = self.ptr_ctx(ptr_of(ctx), 4);
                }
                BuilderOp::InverseMat3 => arg1 = self.ptr_ctx(ptr_of(ctx), 9),
                BuilderOp::InverseMat4 => arg1 = self.ptr_ctx(ptr_of(ctx), 16),
                BuilderOp::CopyConstant
                | BuilderOp::AddImmFloat
                | BuilderOp::MulImmFloat
                | BuilderOp::CmpleImmFloat
                | BuilderOp::CmpltImmFloat
                | BuilderOp::CmpeqImmFloat
                | BuilderOp::CmpneImmFloat
                | BuilderOp::MinImmFloat
                | BuilderOp::MaxImmFloat => {
                    (arg1, arg2) = self.constant_ctx(ctx, 1, true);
                }
                BuilderOp::AddImmInt
                | BuilderOp::MulImmInt
                | BuilderOp::BitwiseAndImmInt
                | BuilderOp::BitwiseXorImmInt
                | BuilderOp::CmpleImmInt
                | BuilderOp::CmpleImmUint
                | BuilderOp::CmpltImmInt
                | BuilderOp::CmpltImmUint
                | BuilderOp::CmpeqImmInt
                | BuilderOp::CmpneImmInt => {
                    (arg1, arg2) = self.constant_ctx(ctx, 1, false);
                }
                BuilderOp::Splat2Constants | BuilderOp::BitwiseAndImm2Ints => {
                    (arg1, arg2) = self.constant_ctx(ctx, 2, true);
                }
                BuilderOp::Splat3Constants | BuilderOp::BitwiseAndImm3Ints => {
                    (arg1, arg2) = self.constant_ctx(ctx, 3, true);
                }
                BuilderOp::Splat4Constants | BuilderOp::BitwiseAndImm4Ints => {
                    (arg1, arg2) = self.constant_ctx(ctx, 4, true);
                }
                BuilderOp::CopyUniform => (arg1, arg2) = self.copy_uniform_ctx(ctx, 1),
                BuilderOp::Copy2Uniforms => (arg1, arg2) = self.copy_uniform_ctx(ctx, 2),
                BuilderOp::Copy3Uniforms => (arg1, arg2) = self.copy_uniform_ctx(ctx, 3),
                BuilderOp::Copy4Uniforms => (arg1, arg2) = self.copy_uniform_ctx(ctx, 4),
                BuilderOp::CopySlotMasked
                | BuilderOp::CopySlotUnmasked
                | BuilderOp::CopyImmutableUnmasked => (arg1, arg2) = self.binary_op_ctx(ctx, 1),
                BuilderOp::Copy2SlotsMasked
                | BuilderOp::Copy2SlotsUnmasked
                | BuilderOp::Copy2ImmutablesUnmasked => (arg1, arg2) = self.binary_op_ctx(ctx, 2),
                BuilderOp::Copy3SlotsMasked
                | BuilderOp::Copy3SlotsUnmasked
                | BuilderOp::Copy3ImmutablesUnmasked => (arg1, arg2) = self.binary_op_ctx(ctx, 3),
                BuilderOp::Copy4SlotsMasked
                | BuilderOp::Copy4SlotsUnmasked
                | BuilderOp::Copy4ImmutablesUnmasked => (arg1, arg2) = self.binary_op_ctx(ctx, 4),
                BuilderOp::CopyFromIndirectUniformUnmasked
                | BuilderOp::CopyFromIndirectUnmasked
                | BuilderOp::CopyToIndirectMasked => {
                    // We don't incorporate the indirect-limit in the output.
                    let StageCtx::CopyIndirect {
                        dst,
                        src,
                        indirect_offset,
                        slots,
                        ..
                    } = ctx
                    else {
                        unreachable!("indirect copy context");
                    };
                    arg1 = self.ptr_ctx(dst, isize::try_from(slots).expect("slots"));
                    arg2 = self.ptr_ctx(src, isize::try_from(slots).expect("slots"));
                    arg3 = self.ptr_ctx(indirect_offset, 1);
                }
                BuilderOp::SwizzleCopyToIndirectMasked => {
                    let StageCtx::SwizzleCopyIndirect {
                        dst,
                        src,
                        indirect_offset,
                        slots,
                        offsets,
                        ..
                    } = ctx
                    else {
                        unreachable!("swizzle indirect copy context");
                    };
                    let slots_usize = usize::try_from(slots).expect("slots");
                    arg1 = self.ptr_ctx(dst, self.swizzle_width(&offsets[..slots_usize]));
                    arg2 = self.ptr_ctx(src, isize::try_from(slots).expect("slots"));
                    arg3 = self.ptr_ctx(indirect_offset, 1);
                    swizzle = self.swizzle_offset_span(&offsets[..slots_usize]);
                }
                BuilderOp::MergeConditionMask
                | BuilderOp::MergeInvConditionMask
                | BuilderOp::AddFloat
                | BuilderOp::AddInt
                | BuilderOp::SubFloat
                | BuilderOp::SubInt
                | BuilderOp::MulFloat
                | BuilderOp::MulInt
                | BuilderOp::DivFloat
                | BuilderOp::DivInt
                | BuilderOp::DivUint
                | BuilderOp::BitwiseAndInt
                | BuilderOp::BitwiseOrInt
                | BuilderOp::BitwiseXorInt
                | BuilderOp::ModFloat
                | BuilderOp::MinFloat
                | BuilderOp::MinInt
                | BuilderOp::MinUint
                | BuilderOp::MaxFloat
                | BuilderOp::MaxInt
                | BuilderOp::MaxUint
                | BuilderOp::CmpltFloat
                | BuilderOp::CmpltInt
                | BuilderOp::CmpltUint
                | BuilderOp::CmpleFloat
                | BuilderOp::CmpleInt
                | BuilderOp::CmpleUint
                | BuilderOp::CmpeqFloat
                | BuilderOp::CmpeqInt
                | BuilderOp::CmpneFloat
                | BuilderOp::CmpneInt => {
                    (arg1, arg2) = self.adjacent_ptr_ctx(ptr_of(ctx), 1);
                }
                BuilderOp::MixFloat | BuilderOp::MixInt => {
                    (arg1, arg2, arg3) = self.adjacent3_ptr_ctx(ptr_of(ctx), 1);
                }
                BuilderOp::Add2Floats
                | BuilderOp::Add2Ints
                | BuilderOp::Sub2Floats
                | BuilderOp::Sub2Ints
                | BuilderOp::Mul2Floats
                | BuilderOp::Mul2Ints
                | BuilderOp::Div2Floats
                | BuilderOp::Div2Ints
                | BuilderOp::Div2Uints
                | BuilderOp::BitwiseAnd2Ints
                | BuilderOp::BitwiseOr2Ints
                | BuilderOp::BitwiseXor2Ints
                | BuilderOp::Mod2Floats
                | BuilderOp::Min2Floats
                | BuilderOp::Min2Ints
                | BuilderOp::Min2Uints
                | BuilderOp::Max2Floats
                | BuilderOp::Max2Ints
                | BuilderOp::Max2Uints
                | BuilderOp::Cmplt2Floats
                | BuilderOp::Cmplt2Ints
                | BuilderOp::Cmplt2Uints
                | BuilderOp::Cmple2Floats
                | BuilderOp::Cmple2Ints
                | BuilderOp::Cmple2Uints
                | BuilderOp::Cmpeq2Floats
                | BuilderOp::Cmpeq2Ints
                | BuilderOp::Cmpne2Floats
                | BuilderOp::Cmpne2Ints => {
                    (arg1, arg2) = self.adjacent_ptr_ctx(ptr_of(ctx), 2);
                }
                BuilderOp::Mix2Floats | BuilderOp::Mix2Ints => {
                    (arg1, arg2, arg3) = self.adjacent3_ptr_ctx(ptr_of(ctx), 2);
                }
                BuilderOp::Add3Floats
                | BuilderOp::Add3Ints
                | BuilderOp::Sub3Floats
                | BuilderOp::Sub3Ints
                | BuilderOp::Mul3Floats
                | BuilderOp::Mul3Ints
                | BuilderOp::Div3Floats
                | BuilderOp::Div3Ints
                | BuilderOp::Div3Uints
                | BuilderOp::BitwiseAnd3Ints
                | BuilderOp::BitwiseOr3Ints
                | BuilderOp::BitwiseXor3Ints
                | BuilderOp::Mod3Floats
                | BuilderOp::Min3Floats
                | BuilderOp::Min3Ints
                | BuilderOp::Min3Uints
                | BuilderOp::Max3Floats
                | BuilderOp::Max3Ints
                | BuilderOp::Max3Uints
                | BuilderOp::Cmplt3Floats
                | BuilderOp::Cmplt3Ints
                | BuilderOp::Cmplt3Uints
                | BuilderOp::Cmple3Floats
                | BuilderOp::Cmple3Ints
                | BuilderOp::Cmple3Uints
                | BuilderOp::Cmpeq3Floats
                | BuilderOp::Cmpeq3Ints
                | BuilderOp::Cmpne3Floats
                | BuilderOp::Cmpne3Ints => {
                    (arg1, arg2) = self.adjacent_ptr_ctx(ptr_of(ctx), 3);
                }
                BuilderOp::Mix3Floats | BuilderOp::Mix3Ints => {
                    (arg1, arg2, arg3) = self.adjacent3_ptr_ctx(ptr_of(ctx), 3);
                }
                BuilderOp::Add4Floats
                | BuilderOp::Add4Ints
                | BuilderOp::Sub4Floats
                | BuilderOp::Sub4Ints
                | BuilderOp::Mul4Floats
                | BuilderOp::Mul4Ints
                | BuilderOp::Div4Floats
                | BuilderOp::Div4Ints
                | BuilderOp::Div4Uints
                | BuilderOp::BitwiseAnd4Ints
                | BuilderOp::BitwiseOr4Ints
                | BuilderOp::BitwiseXor4Ints
                | BuilderOp::Mod4Floats
                | BuilderOp::Min4Floats
                | BuilderOp::Min4Ints
                | BuilderOp::Min4Uints
                | BuilderOp::Max4Floats
                | BuilderOp::Max4Ints
                | BuilderOp::Max4Uints
                | BuilderOp::Cmplt4Floats
                | BuilderOp::Cmplt4Ints
                | BuilderOp::Cmplt4Uints
                | BuilderOp::Cmple4Floats
                | BuilderOp::Cmple4Ints
                | BuilderOp::Cmple4Uints
                | BuilderOp::Cmpeq4Floats
                | BuilderOp::Cmpeq4Ints
                | BuilderOp::Cmpne4Floats
                | BuilderOp::Cmpne4Ints => {
                    (arg1, arg2) = self.adjacent_ptr_ctx(ptr_of(ctx), 4);
                }
                BuilderOp::Mix4Floats | BuilderOp::Mix4Ints => {
                    (arg1, arg2, arg3) = self.adjacent3_ptr_ctx(ptr_of(ctx), 4);
                }
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
                | BuilderOp::CmpltNFloats
                | BuilderOp::CmpltNInts
                | BuilderOp::CmpltNUints
                | BuilderOp::CmpleNFloats
                | BuilderOp::CmpleNInts
                | BuilderOp::CmpleNUints
                | BuilderOp::CmpeqNFloats
                | BuilderOp::CmpeqNInts
                | BuilderOp::CmpneNFloats
                | BuilderOp::CmpneNInts
                | BuilderOp::Atan2NFloats
                | BuilderOp::PowNFloats => {
                    (arg1, arg2) = self.adjacent_binary_op_ctx(ctx);
                }
                BuilderOp::MixNFloats | BuilderOp::MixNInts | BuilderOp::SmoothstepNFloats => {
                    (arg1, arg2, arg3) = self.adjacent_ternary_op_ctx(ctx);
                }
                BuilderOp::Jump
                | BuilderOp::BranchIfAllLanesActive
                | BuilderOp::BranchIfAnyLanesActive
                | BuilderOp::BranchIfNoLanesActive => {
                    let StageCtx::Branch { offset } = ctx else {
                        unreachable!("branch context");
                    };
                    arg1 = self.branch_offset(offset, index);
                }
                BuilderOp::BranchIfNoActiveLanesEq => {
                    let StageCtx::BranchIfEqual { offset, value, ptr } = ctx else {
                        unreachable!("branch-if-equal context");
                    };
                    arg1 = self.branch_offset(offset, index);
                    arg2 = self.ptr_ctx(ptr, 1);
                    arg3 = Self::imm_bits(value, true);
                }
                BuilderOp::TraceVar => {
                    let StageCtx::TraceVar {
                        trace_mask,
                        num_slots,
                        data,
                        indirect_offset,
                        ..
                    } = ctx
                    else {
                        unreachable!("trace-var context");
                    };
                    arg1 = self.ptr_ctx(trace_mask, 1);
                    arg2 = self.ptr_ctx(data, isize::try_from(num_slots).expect("count"));
                    if let Some(indirect) = indirect_offset {
                        arg3 = format!(" + {}", self.ptr_ctx(indirect, 1));
                    }
                }
                BuilderOp::TraceLine => {
                    let StageCtx::TraceLine { trace_mask, line } = ctx else {
                        unreachable!("trace-line context");
                    };
                    arg1 = self.ptr_ctx(trace_mask, 1);
                    arg2 = line.to_string();
                }
                BuilderOp::TraceEnter | BuilderOp::TraceExit => {
                    let StageCtx::TraceFunc {
                        trace_mask,
                        func_idx,
                    } = ctx
                    else {
                        unreachable!("trace-function context");
                    };
                    arg1 = self.ptr_ctx(trace_mask, 1);
                    arg2 = self
                        .program
                        .debug_trace
                        .as_deref()
                        .and_then(|trace| {
                            usize::try_from(func_idx)
                                .ok()
                                .and_then(|i| trace.func_info.get(i))
                        })
                        .map_or_else(|| "???".to_owned(), |info| info.name.clone());
                }
                BuilderOp::TraceScope => {
                    let StageCtx::TraceScope { trace_mask, delta } = ctx else {
                        unreachable!("trace-scope context");
                    };
                    arg1 = self.ptr_ctx(trace_mask, 1);
                    arg2 = format!("{delta:+}");
                }
                _ => {}
            }

            let op_name = op.name();
            let op_text = op_text(op, &arg1, &arg2, &arg3, &swizzle);
            let name: String = op_name.chars().take(30).collect();
            if op_text.is_empty() {
                out.push_str(&name);
                out.push('\n');
            } else {
                out.push_str(&pad_name(&name));
                out.push(' ');
                out.push_str(&op_text);
                out.push('\n');
            }
        }
    }
}

/// The text after the op name, per op (empty for ops printed by name alone).
#[allow(clippy::too_many_lines)] // one arm per op, as in Skia's dumper
fn op_text(op: BuilderOp, arg1: &str, arg2: &str, arg3: &str, swizzle: &str) -> String {
    {
        let a1 = arg1;
        let a2 = arg2;
        let a3 = arg3;
        match op {
            BuilderOp::TraceVar => format!("TraceVar({a2}{a3}) when {a1} is true"),
            BuilderOp::TraceLine => format!("TraceLine({a2}) when {a1} is true"),
            BuilderOp::TraceEnter => format!("TraceEnter({a2}) when {a1} is true"),
            BuilderOp::TraceExit => format!("TraceExit({a2}) when {a1} is true"),
            BuilderOp::TraceScope => format!("TraceScope({a2}) when {a1} is true"),
            BuilderOp::InitLaneMasks => "CondMask = LoopMask = RetMask = true".to_owned(),
            BuilderOp::LoadConditionMask => format!("CondMask = {a1}"),
            BuilderOp::StoreConditionMask => format!("{a1} = CondMask"),
            BuilderOp::MergeConditionMask => format!("CondMask = {a1} & {a2}"),
            BuilderOp::MergeInvConditionMask => format!("CondMask = {a1} & ~{a2}"),
            BuilderOp::LoadLoopMask => format!("LoopMask = {a1}"),
            BuilderOp::StoreLoopMask => format!("{a1} = LoopMask"),
            BuilderOp::MaskOffLoopMask => "LoopMask &= ~(CondMask & LoopMask & RetMask)".to_owned(),
            BuilderOp::ReenableLoopMask => format!("LoopMask |= {a1}"),
            BuilderOp::MergeLoopMask => format!("LoopMask &= {a1}"),
            BuilderOp::LoadReturnMask => format!("RetMask = {a1}"),
            BuilderOp::StoreReturnMask => format!("{a1} = RetMask"),
            BuilderOp::MaskOffReturnMask => {
                "RetMask &= ~(CondMask & LoopMask & RetMask)".to_owned()
            }
            BuilderOp::StoreSrcRg => format!("{a1} = src.rg"),
            BuilderOp::ExchangeSrc => format!("swap(src.rgba, {a1})"),
            BuilderOp::StoreSrc => format!("{a1} = src.rgba"),
            BuilderOp::StoreDst => format!("{a1} = dst.rgba"),
            BuilderOp::StoreDeviceXy01 => format!("{a1} = DeviceCoords.xy01"),
            BuilderOp::LoadSrc => format!("src.rgba = {a1}"),
            BuilderOp::LoadDst => format!("dst.rgba = {a1}"),
            BuilderOp::BitwiseAndInt
            | BuilderOp::BitwiseAnd2Ints
            | BuilderOp::BitwiseAnd3Ints
            | BuilderOp::BitwiseAnd4Ints
            | BuilderOp::BitwiseAndNInts
            | BuilderOp::BitwiseAndImmInt
            | BuilderOp::BitwiseAndImm2Ints
            | BuilderOp::BitwiseAndImm3Ints
            | BuilderOp::BitwiseAndImm4Ints => format!("{a1} &= {a2}"),
            BuilderOp::BitwiseOrInt
            | BuilderOp::BitwiseOr2Ints
            | BuilderOp::BitwiseOr3Ints
            | BuilderOp::BitwiseOr4Ints
            | BuilderOp::BitwiseOrNInts => format!("{a1} |= {a2}"),
            BuilderOp::BitwiseXorInt
            | BuilderOp::BitwiseXor2Ints
            | BuilderOp::BitwiseXor3Ints
            | BuilderOp::BitwiseXor4Ints
            | BuilderOp::BitwiseXorNInts
            | BuilderOp::BitwiseXorImmInt => format!("{a1} ^= {a2}"),
            BuilderOp::CastToFloatFromInt
            | BuilderOp::CastToFloatFrom2Ints
            | BuilderOp::CastToFloatFrom3Ints
            | BuilderOp::CastToFloatFrom4Ints => format!("{a1} = IntToFloat({a1})"),
            BuilderOp::CastToFloatFromUint
            | BuilderOp::CastToFloatFrom2Uints
            | BuilderOp::CastToFloatFrom3Uints
            | BuilderOp::CastToFloatFrom4Uints => format!("{a1} = UintToFloat({a1})"),
            BuilderOp::CastToIntFromFloat
            | BuilderOp::CastToIntFrom2Floats
            | BuilderOp::CastToIntFrom3Floats
            | BuilderOp::CastToIntFrom4Floats => format!("{a1} = FloatToInt({a1})"),
            BuilderOp::CastToUintFromFloat
            | BuilderOp::CastToUintFrom2Floats
            | BuilderOp::CastToUintFrom3Floats
            | BuilderOp::CastToUintFrom4Floats => format!("{a1} = FloatToUint({a1})"),
            BuilderOp::CopySlotMasked
            | BuilderOp::Copy2SlotsMasked
            | BuilderOp::Copy3SlotsMasked
            | BuilderOp::Copy4SlotsMasked
            | BuilderOp::SwizzleCopySlotMasked
            | BuilderOp::SwizzleCopy2SlotsMasked
            | BuilderOp::SwizzleCopy3SlotsMasked
            | BuilderOp::SwizzleCopy4SlotsMasked => format!("{a1} = Mask({a2})"),
            BuilderOp::CopyUniform
            | BuilderOp::Copy2Uniforms
            | BuilderOp::Copy3Uniforms
            | BuilderOp::Copy4Uniforms
            | BuilderOp::CopySlotUnmasked
            | BuilderOp::Copy2SlotsUnmasked
            | BuilderOp::Copy3SlotsUnmasked
            | BuilderOp::Copy4SlotsUnmasked
            | BuilderOp::CopyImmutableUnmasked
            | BuilderOp::Copy2ImmutablesUnmasked
            | BuilderOp::Copy3ImmutablesUnmasked
            | BuilderOp::Copy4ImmutablesUnmasked
            | BuilderOp::CopyConstant
            | BuilderOp::Splat2Constants
            | BuilderOp::Splat3Constants
            | BuilderOp::Splat4Constants
            | BuilderOp::Swizzle1
            | BuilderOp::Swizzle2
            | BuilderOp::Swizzle3
            | BuilderOp::Swizzle4
            | BuilderOp::Shuffle => format!("{a1} = {a2}"),
            BuilderOp::CopyFromIndirectUnmasked | BuilderOp::CopyFromIndirectUniformUnmasked => {
                format!("{a1} = Indirect({a2} + {a3})")
            }
            BuilderOp::CopyToIndirectMasked => format!("Indirect({a1} + {a3}) = Mask({a2})"),
            BuilderOp::SwizzleCopyToIndirectMasked => {
                format!("Indirect({a1} + {a3}).{swizzle} = Mask({a2})")
            }
            BuilderOp::AbsInt | BuilderOp::Abs2Ints | BuilderOp::Abs3Ints | BuilderOp::Abs4Ints => {
                format!("{a1} = abs({a1})")
            }
            BuilderOp::AcosFloat => format!("{a1} = acos({a1})"),
            BuilderOp::AsinFloat => format!("{a1} = asin({a1})"),
            BuilderOp::AtanFloat => format!("{a1} = atan({a1})"),
            BuilderOp::Atan2NFloats => format!("{a1} = atan2({a1}, {a2})"),
            BuilderOp::CeilFloat
            | BuilderOp::Ceil2Floats
            | BuilderOp::Ceil3Floats
            | BuilderOp::Ceil4Floats => {
                format!("{a1} = ceil({a1})")
            }
            BuilderOp::CosFloat => format!("{a1} = cos({a1})"),
            BuilderOp::Refract4Floats => format!("{a1} = refract({a1}, {a2}, {a3})"),
            BuilderOp::Dot2Floats | BuilderOp::Dot3Floats | BuilderOp::Dot4Floats => {
                format!("{a1} = dot({a2}, {a3})")
            }
            BuilderOp::ExpFloat => format!("{a1} = exp({a1})"),
            BuilderOp::Exp2Float => format!("{a1} = exp2({a1})"),
            BuilderOp::LogFloat => format!("{a1} = log({a1})"),
            BuilderOp::Log2Float => format!("{a1} = log2({a1})"),
            BuilderOp::PowNFloats => format!("{a1} = pow({a1}, {a2})"),
            BuilderOp::SinFloat => format!("{a1} = sin({a1})"),
            BuilderOp::SqrtFloat => format!("{a1} = sqrt({a1})"),
            BuilderOp::TanFloat => format!("{a1} = tan({a1})"),
            BuilderOp::FloorFloat
            | BuilderOp::Floor2Floats
            | BuilderOp::Floor3Floats
            | BuilderOp::Floor4Floats => {
                format!("{a1} = floor({a1})")
            }
            BuilderOp::InvsqrtFloat
            | BuilderOp::Invsqrt2Floats
            | BuilderOp::Invsqrt3Floats
            | BuilderOp::Invsqrt4Floats => format!("{a1} = inversesqrt({a1})"),
            BuilderOp::InverseMat2 | BuilderOp::InverseMat3 | BuilderOp::InverseMat4 => {
                format!("{a1} = inverse({a1})")
            }
            BuilderOp::AddFloat
            | BuilderOp::AddInt
            | BuilderOp::Add2Floats
            | BuilderOp::Add2Ints
            | BuilderOp::Add3Floats
            | BuilderOp::Add3Ints
            | BuilderOp::Add4Floats
            | BuilderOp::Add4Ints
            | BuilderOp::AddNFloats
            | BuilderOp::AddNInts
            | BuilderOp::AddImmFloat
            | BuilderOp::AddImmInt => format!("{a1} += {a2}"),
            BuilderOp::SubFloat
            | BuilderOp::SubInt
            | BuilderOp::Sub2Floats
            | BuilderOp::Sub2Ints
            | BuilderOp::Sub3Floats
            | BuilderOp::Sub3Ints
            | BuilderOp::Sub4Floats
            | BuilderOp::Sub4Ints
            | BuilderOp::SubNFloats
            | BuilderOp::SubNInts => format!("{a1} -= {a2}"),
            BuilderOp::MulFloat
            | BuilderOp::MulInt
            | BuilderOp::Mul2Floats
            | BuilderOp::Mul2Ints
            | BuilderOp::Mul3Floats
            | BuilderOp::Mul3Ints
            | BuilderOp::Mul4Floats
            | BuilderOp::Mul4Ints
            | BuilderOp::MulNFloats
            | BuilderOp::MulNInts
            | BuilderOp::MulImmFloat
            | BuilderOp::MulImmInt => format!("{a1} *= {a2}"),
            BuilderOp::DivFloat
            | BuilderOp::DivInt
            | BuilderOp::DivUint
            | BuilderOp::Div2Floats
            | BuilderOp::Div2Ints
            | BuilderOp::Div2Uints
            | BuilderOp::Div3Floats
            | BuilderOp::Div3Ints
            | BuilderOp::Div3Uints
            | BuilderOp::Div4Floats
            | BuilderOp::Div4Ints
            | BuilderOp::Div4Uints
            | BuilderOp::DivNFloats
            | BuilderOp::DivNInts
            | BuilderOp::DivNUints => format!("{a1} /= {a2}"),
            BuilderOp::MatrixMultiply2
            | BuilderOp::MatrixMultiply3
            | BuilderOp::MatrixMultiply4 => {
                format!("{a1} = {a2} * {a3}")
            }
            BuilderOp::ModFloat
            | BuilderOp::Mod2Floats
            | BuilderOp::Mod3Floats
            | BuilderOp::Mod4Floats
            | BuilderOp::ModNFloats => format!("{a1} = mod({a1}, {a2})"),
            BuilderOp::MinFloat
            | BuilderOp::MinInt
            | BuilderOp::MinUint
            | BuilderOp::Min2Floats
            | BuilderOp::Min2Ints
            | BuilderOp::Min2Uints
            | BuilderOp::Min3Floats
            | BuilderOp::Min3Ints
            | BuilderOp::Min3Uints
            | BuilderOp::Min4Floats
            | BuilderOp::Min4Ints
            | BuilderOp::Min4Uints
            | BuilderOp::MinNFloats
            | BuilderOp::MinNInts
            | BuilderOp::MinNUints
            | BuilderOp::MinImmFloat => format!("{a1} = min({a1}, {a2})"),
            BuilderOp::MaxFloat
            | BuilderOp::MaxInt
            | BuilderOp::MaxUint
            | BuilderOp::Max2Floats
            | BuilderOp::Max2Ints
            | BuilderOp::Max2Uints
            | BuilderOp::Max3Floats
            | BuilderOp::Max3Ints
            | BuilderOp::Max3Uints
            | BuilderOp::Max4Floats
            | BuilderOp::Max4Ints
            | BuilderOp::Max4Uints
            | BuilderOp::MaxNFloats
            | BuilderOp::MaxNInts
            | BuilderOp::MaxNUints
            | BuilderOp::MaxImmFloat => format!("{a1} = max({a1}, {a2})"),
            BuilderOp::CmpltFloat
            | BuilderOp::CmpltInt
            | BuilderOp::CmpltUint
            | BuilderOp::Cmplt2Floats
            | BuilderOp::Cmplt2Ints
            | BuilderOp::Cmplt2Uints
            | BuilderOp::Cmplt3Floats
            | BuilderOp::Cmplt3Ints
            | BuilderOp::Cmplt3Uints
            | BuilderOp::Cmplt4Floats
            | BuilderOp::Cmplt4Ints
            | BuilderOp::Cmplt4Uints
            | BuilderOp::CmpltNFloats
            | BuilderOp::CmpltNInts
            | BuilderOp::CmpltNUints
            | BuilderOp::CmpltImmFloat
            | BuilderOp::CmpltImmInt
            | BuilderOp::CmpltImmUint => format!("{a1} = lessThan({a1}, {a2})"),
            BuilderOp::CmpleFloat
            | BuilderOp::CmpleInt
            | BuilderOp::CmpleUint
            | BuilderOp::Cmple2Floats
            | BuilderOp::Cmple2Ints
            | BuilderOp::Cmple2Uints
            | BuilderOp::Cmple3Floats
            | BuilderOp::Cmple3Ints
            | BuilderOp::Cmple3Uints
            | BuilderOp::Cmple4Floats
            | BuilderOp::Cmple4Ints
            | BuilderOp::Cmple4Uints
            | BuilderOp::CmpleNFloats
            | BuilderOp::CmpleNInts
            | BuilderOp::CmpleNUints
            | BuilderOp::CmpleImmFloat
            | BuilderOp::CmpleImmInt
            | BuilderOp::CmpleImmUint => format!("{a1} = lessThanEqual({a1}, {a2})"),
            BuilderOp::CmpeqFloat
            | BuilderOp::CmpeqInt
            | BuilderOp::Cmpeq2Floats
            | BuilderOp::Cmpeq2Ints
            | BuilderOp::Cmpeq3Floats
            | BuilderOp::Cmpeq3Ints
            | BuilderOp::Cmpeq4Floats
            | BuilderOp::Cmpeq4Ints
            | BuilderOp::CmpeqNFloats
            | BuilderOp::CmpeqNInts
            | BuilderOp::CmpeqImmFloat
            | BuilderOp::CmpeqImmInt => format!("{a1} = equal({a1}, {a2})"),
            BuilderOp::CmpneFloat
            | BuilderOp::CmpneInt
            | BuilderOp::Cmpne2Floats
            | BuilderOp::Cmpne2Ints
            | BuilderOp::Cmpne3Floats
            | BuilderOp::Cmpne3Ints
            | BuilderOp::Cmpne4Floats
            | BuilderOp::Cmpne4Ints
            | BuilderOp::CmpneNFloats
            | BuilderOp::CmpneNInts
            | BuilderOp::CmpneImmFloat
            | BuilderOp::CmpneImmInt => format!("{a1} = notEqual({a1}, {a2})"),
            BuilderOp::MixFloat
            | BuilderOp::MixInt
            | BuilderOp::Mix2Floats
            | BuilderOp::Mix2Ints
            | BuilderOp::Mix3Floats
            | BuilderOp::Mix3Ints
            | BuilderOp::Mix4Floats
            | BuilderOp::Mix4Ints
            | BuilderOp::MixNFloats
            | BuilderOp::MixNInts => format!("{a1} = mix({a2}, {a3}, {a1})"),
            BuilderOp::SmoothstepNFloats => format!("{a1} = smoothstep({a1}, {a2}, {a3})"),
            BuilderOp::Jump
            | BuilderOp::BranchIfAllLanesActive
            | BuilderOp::BranchIfAnyLanesActive
            | BuilderOp::BranchIfNoLanesActive
            | BuilderOp::InvokeShader
            | BuilderOp::InvokeColorFilter
            | BuilderOp::InvokeBlender => format!("{} {a1}", op.name()),
            BuilderOp::InvokeToLinearSrgb => format!("{a1} = toLinearSrgb({a1})"),
            BuilderOp::InvokeFromLinearSrgb => format!("{a1} = fromLinearSrgb({a1})"),
            BuilderOp::BranchIfNoActiveLanesEq => {
                format!("branch {a1} if no lanes of {a2} == {a3}")
            }
            BuilderOp::Label => format!("label {a1}"),
            BuilderOp::CaseOp => format!("if ({a1} == {a3}) {{ LoopMask = true; {a2} = false; }}"),
            BuilderOp::ContinueOp => {
                format!("{a1} |= Mask(0xFFFFFFFF); LoopMask &= ~(CondMask & LoopMask & RetMask)")
            }
            _ => String::new(),
        }
    }
}

/// The pointer a pointer-context op carries (`Ptr`, or `Null` for `nullptr`).
fn ptr_of(ctx: StageCtx) -> Addr {
    match ctx {
        StageCtx::Ptr(addr) => addr,
        StageCtx::Null => Addr::Null,
        other => unreachable!("pointer context {other:?}"),
    }
}
