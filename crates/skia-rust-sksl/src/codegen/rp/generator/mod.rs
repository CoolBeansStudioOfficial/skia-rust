// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.{h,cpp}.

//! The Raster Pipeline code generator: converts an `SkSL` program into a [`Program`] of
//! Raster Pipeline instructions (`SkSL::MakeRasterPipelineProgram`).
//!
//! Skia's `Generator` is one class with RAII helpers (`AutoStack`, `AutoContinueMask`,
//! `AutoLoopTarget`) and a hierarchy of `LValue`s whose destructors emit instructions. This port
//! keeps the same generator methods, in the same order, and replaces each destructor with an
//! explicit call at the point where C++ runs it:
//!
//! - an `AutoStack` is a [`Stack`] id; `~AutoStack` is [`Generator::recycle_stack`];
//! - `LValue`s live in an arena owned by the generator and are named by [`LvId`]; `~LValue` is
//!   [`Generator::free_lvalue`], which emits `ScratchLValue`'s and `DynamicIndexLValue`'s discards
//!   and recycles their stacks in the order the C++ destructors run.
//!
//! Every `unsupported()` in Skia is an `Err(Unsupported)` here, propagated with `?`. A failed
//! generation discards the whole program, so cleanup on the error paths is not performed.

// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp (chrome/m156)

// The ported functions keep the shape of the C++ ones: long switches over node kinds, one bool
// (here `Res`) result for every `push*` and `write*` member, and Skia's float literals.
#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::excessive_precision,
    clippy::collapsible_if,
    clippy::collapsible_match
)]

mod expressions;
mod intrinsics;
mod lvalue;
mod statements;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::builder::{Builder, Slot, SlotRange};
use super::ops::BuilderOp;
use super::program::Program as RpProgram;
use crate::analysis::{self, ProgramUsage, ReturnComplexity};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::ir::{
    ElemId, ExprId, Expression, ExpressionKind, FnId, IrPool, Literal, ModifierFlags, Program,
    ProgramElementKind, TypeId, TypeKind, VarId,
};
use crate::position::Position;
use crate::thash::{THashMap, THashSet};
use crate::tracing::{DebugTracePriv, FunctionDebugInfo, SlotDebugInfo};

use lvalue::LValueArena;

/// `SK_FRAGCOORD_BUILTIN`: the `layout(builtin=…)` of `sk_FragCoord`.
const SK_FRAGCOORD_BUILTIN: i32 = 15;

/// What `unsupported()` returns: the program cannot be expressed in Raster Pipeline stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Unsupported;

/// A generation step that may hit something the back end does not support.
type Res<T = ()> = Result<T, Unsupported>;

/// `unsupported()`: "If `MakeRasterPipelineProgram` returns false, set a breakpoint here."
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L88-L91 (chrome/m156)
fn unsupported<T>() -> Res<T> {
    Err(Unsupported)
}

/// What a slot range is keyed by: a variable, or the call site of a function (a `FunctionCall`
/// expression, or `main`'s definition) for its return value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SlotKey {
    Variable(VarId),
    CallExpr(ExprId),
    CallElement(ElemId),
}

/// `SlotManager`: hands out slot ranges and (optionally) records their debug names.
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L97-L141 (chrome/m156)
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // Skia's `fSlotMap`, `fSlotCount` and `fSlotDebugInfo`.
struct SlotManager {
    slot_map: HashMap<SlotKey, SlotRange>,
    slot_count: i32,
    slot_debug_info: Option<Vec<SlotDebugInfo>>,
}

impl SlotManager {
    /// `SlotManager(std::vector<SlotDebugInfo>*)`: `with_debug_info` is whether a debug trace
    /// wants the slots recorded.
    fn new(with_debug_info: bool) -> Self {
        Self {
            slot_map: HashMap::new(),
            slot_count: 0,
            slot_debug_info: with_debug_info.then(Vec::new),
        }
    }

    /// `addSlotDebugInfoForGroup`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1108-L1154 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Skia's signature, plus the pool and the output.
    fn add_slot_debug_info_for_group(
        info: &mut Vec<SlotDebugInfo>,
        pool: &IrPool,
        var_name: &str,
        ty: TypeId,
        pos: Position,
        group_index: &mut i32,
        is_function_return_value: bool,
    ) {
        let t = pool.ty(ty);
        match t.type_kind {
            TypeKind::Array => {
                let nslots = t.columns();
                let elem_type = t.component_type().id();
                for slot in 0..nslots {
                    Self::add_slot_debug_info_for_group(
                        info,
                        pool,
                        &format!("{var_name}[{slot}]"),
                        elem_type,
                        pos,
                        group_index,
                        is_function_return_value,
                    );
                }
            }
            TypeKind::Struct => {
                for field in t.fields() {
                    Self::add_slot_debug_info_for_group(
                        info,
                        pool,
                        &format!("{var_name}.{}", field.name),
                        field.ty,
                        pos,
                        group_index,
                        is_function_return_value,
                    );
                }
            }
            // `kScalar`, `kVector` and `kMatrix` (the default case asserts, then falls through).
            _ => {
                let number_kind = t.component_type().number_kind();
                let nslots = t.slot_count();
                for slot in 0..nslots {
                    info.push(SlotDebugInfo {
                        name: var_name.to_owned(),
                        columns: u8::try_from(t.columns()).unwrap_or(0),
                        rows: u8::try_from(t.rows()).unwrap_or(0),
                        component_index: u8::try_from(slot).unwrap_or(0),
                        group_index: *group_index,
                        number_kind,
                        line: 0,
                        pos,
                        fn_return_value: if is_function_return_value { 1 } else { -1 },
                    });
                    *group_index += 1;
                }
            }
        }
    }

    /// `createSlots`: creates slots associated with an `SkSL` variable or return value.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1033-L1056 (chrome/m156)
    fn create_slots(
        &mut self,
        pool: &IrPool,
        name: impl FnOnce() -> String,
        ty: TypeId,
        pos: Position,
        is_function_return_value: bool,
    ) -> SlotRange {
        let nslots = i32::try_from(pool.ty(ty).slot_count()).expect("slot count fits i32");
        if nslots == 0 {
            return SlotRange::default();
        }
        if let Some(info) = &mut self.slot_debug_info {
            // Our debug slot-info table should have the same length as the actual slot table.
            debug_assert_eq!(info.len(), usize::try_from(self.slot_count).unwrap_or(0));
            // Append slot names and types to our debug slot-info table.
            let mut group_index = 0;
            Self::add_slot_debug_info_for_group(
                info,
                pool,
                &name(),
                ty,
                pos,
                &mut group_index,
                is_function_return_value,
            );
            debug_assert_eq!(group_index, nslots);
        }
        let result = SlotRange {
            index: self.slot_count,
            count: nslots,
        };
        self.slot_count += nslots;
        result
    }

    /// `mapVariableToSlots`: associates previously-created slots with a variable; returns the
    /// previously associated range, if any.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1058-L1065 (chrome/m156)
    fn map_variable_to_slots(
        &mut self,
        pool: &IrPool,
        v: VarId,
        range: SlotRange,
    ) -> Option<SlotRange> {
        debug_assert_eq!(
            pool.ty(pool.variable(v).ty).slot_count(),
            usize::try_from(range.count).unwrap_or(0)
        );
        self.slot_map.insert(SlotKey::Variable(v), range)
    }

    /// `unmapVariableSlots`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1067-L1069 (chrome/m156)
    fn unmap_variable_slots(&mut self, v: VarId) {
        self.slot_map.remove(&SlotKey::Variable(v));
    }

    /// `getVariableSlots`: looks up the slots associated with a variable; creates them if
    /// necessary.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1071-L1083 (chrome/m156)
    fn get_variable_slots(&mut self, pool: &IrPool, v: VarId) -> SlotRange {
        if let Some(entry) = self.slot_map.get(&SlotKey::Variable(v)) {
            return *entry;
        }
        let var = pool.variable(v);
        let range = self.create_slots(pool, || var.name.to_string(), var.ty, var.position, false);
        self.map_variable_to_slots(pool, v, range);
        range
    }

    /// `getFunctionSlots`: looks up the slots for a function's return value at a call site;
    /// creates the range if necessary.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1085-L1095 (chrome/m156)
    fn get_function_slots(&mut self, pool: &IrPool, call_site: SlotKey, f: FnId) -> SlotRange {
        if let Some(entry) = self.slot_map.get(&call_site) {
            return *entry;
        }
        let decl = pool.function(f);
        let range = self.create_slots(
            pool,
            || format!("[{}].result", decl.name),
            decl.return_type,
            decl.position,
            true,
        );
        self.slot_map.insert(call_site, range);
        range
    }

    /// `slotCount()`: the total number of slots consumed.
    fn slot_count(&self) -> i32 {
        self.slot_count
    }
}

/// `AutoStack`: a temporary stack. Skia's destructor recycles the id; here the generator's
/// [`Generator::recycle_stack`] does, at the same point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stack(i32);

impl Stack {
    /// `stackID()`.
    fn stack_id(self) -> i32 {
        self.0
    }
}

/// `AutoContinueMask`.
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L571-L627 (chrome/m156)
#[derive(Debug, Default)]
struct ContinueMask {
    continue_mask_stack: Option<Stack>,
    previous_continue_mask: Option<Stack>,
}

/// `Generator::TypedOps`: one op per component kind.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)] // Skia's `fFloatOp`, `fSignedOp`, ...
struct TypedOps {
    float_op: BuilderOp,
    signed_op: BuilderOp,
    unsigned_op: BuilderOp,
    boolean_op: BuilderOp,
}

/// `ImmutableBits`.
type ImmutableBits = i32;

/// `SkSL::RP::Generator`.
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L143-L471 (chrome/m156)
struct Generator<'c> {
    /// `fContext`, with the program's pool and configuration lent to it.
    ctx: &'c mut Context,
    /// `fProgram.fUsage`.
    usage: ProgramUsage,
    /// `fProgram.elements()`.
    elements: Vec<ElemId>,
    /// `fProgram.fSource`.
    source: Arc<[u8]>,
    /// `fBuilder`.
    builder: Builder,
    /// `fDebugTrace`.
    debug_trace: Option<DebugTracePriv>,
    /// `fWriteTraceOps`.
    write_trace_ops: bool,
    /// `fChildEffectMap`.
    child_effect_map: HashMap<VarId, i32>,
    /// `fProgramSlots`.
    program_slots: SlotManager,
    /// `fUniformSlots`.
    uniform_slots: SlotManager,
    /// `fImmutableSlots`.
    immutable_slots: SlotManager,
    /// `fTraceMask`.
    trace_mask: Option<Stack>,
    /// `fCurrentFunction`: the definition element being written.
    current_function: Option<ElemId>,
    /// `fCurrentFunctionResult`.
    current_function_result: SlotRange,
    /// `fCurrentContinueMask`.
    current_continue_mask: Option<Stack>,
    /// `fCurrentBreakTarget`.
    current_break_target: i32,
    /// `fCurrentStack`.
    current_stack: i32,
    /// `fNextStackID`.
    next_stack_id: i32,
    /// `fRecycledStacks`.
    recycled_stacks: Vec<i32>,
    /// `AutoStack::fParentStackID`, by stack id.
    stack_parent: HashMap<i32, i32>,
    /// `fReturnComplexityMap`.
    return_complexity_map: HashMap<ElemId, ReturnComplexity>,
    /// `fImmutableSlotMap`.
    immutable_slot_map: THashMap<ImmutableBits, THashSet<Slot>>,
    /// `fImmutableVariables`.
    immutable_variables: HashSet<VarId>,
    /// `fInsideCompoundStatement`.
    inside_compound_statement: i32,
    /// `fLineOffsets`.
    line_offsets: Vec<i32>,
    /// The `LValue`s alive right now.
    lvalues: LValueArena,
}

impl TypedOps {
    const ADD: TypedOps = TypedOps {
        float_op: BuilderOp::AddNFloats,
        signed_op: BuilderOp::AddNInts,
        unsigned_op: BuilderOp::AddNInts,
        boolean_op: BuilderOp::Unsupported,
    };
    const SUBTRACT: TypedOps = TypedOps {
        float_op: BuilderOp::SubNFloats,
        signed_op: BuilderOp::SubNInts,
        unsigned_op: BuilderOp::SubNInts,
        boolean_op: BuilderOp::Unsupported,
    };
    const MULTIPLY: TypedOps = TypedOps {
        float_op: BuilderOp::MulNFloats,
        signed_op: BuilderOp::MulNInts,
        unsigned_op: BuilderOp::MulNInts,
        boolean_op: BuilderOp::Unsupported,
    };
    const DIVIDE: TypedOps = TypedOps {
        float_op: BuilderOp::DivNFloats,
        signed_op: BuilderOp::DivNInts,
        unsigned_op: BuilderOp::DivNUints,
        boolean_op: BuilderOp::Unsupported,
    };
    const LESS_THAN: TypedOps = TypedOps {
        float_op: BuilderOp::CmpltNFloats,
        signed_op: BuilderOp::CmpltNInts,
        unsigned_op: BuilderOp::CmpltNUints,
        boolean_op: BuilderOp::Unsupported,
    };
    const LESS_THAN_EQUAL: TypedOps = TypedOps {
        float_op: BuilderOp::CmpleNFloats,
        signed_op: BuilderOp::CmpleNInts,
        unsigned_op: BuilderOp::CmpleNUints,
        boolean_op: BuilderOp::Unsupported,
    };
    const EQUAL: TypedOps = TypedOps {
        float_op: BuilderOp::CmpeqNFloats,
        signed_op: BuilderOp::CmpeqNInts,
        unsigned_op: BuilderOp::CmpeqNInts,
        boolean_op: BuilderOp::CmpeqNInts,
    };
    const NOT_EQUAL: TypedOps = TypedOps {
        float_op: BuilderOp::CmpneNFloats,
        signed_op: BuilderOp::CmpneNInts,
        unsigned_op: BuilderOp::CmpneNInts,
        boolean_op: BuilderOp::CmpneNInts,
    };
    const MOD: TypedOps = TypedOps {
        float_op: BuilderOp::ModNFloats,
        signed_op: BuilderOp::Unsupported,
        unsigned_op: BuilderOp::Unsupported,
        boolean_op: BuilderOp::Unsupported,
    };
    const MIN: TypedOps = TypedOps {
        float_op: BuilderOp::MinNFloats,
        signed_op: BuilderOp::MinNInts,
        unsigned_op: BuilderOp::MinNUints,
        boolean_op: BuilderOp::MinNUints,
    };
    const MAX: TypedOps = TypedOps {
        float_op: BuilderOp::MaxNFloats,
        signed_op: BuilderOp::MaxNInts,
        unsigned_op: BuilderOp::MaxNUints,
        boolean_op: BuilderOp::MaxNUints,
    };
    const MIX: TypedOps = TypedOps {
        float_op: BuilderOp::MixNFloats,
        signed_op: BuilderOp::Unsupported,
        unsigned_op: BuilderOp::Unsupported,
        boolean_op: BuilderOp::Unsupported,
    };
    const INVERSE_SQRT: TypedOps = TypedOps {
        float_op: BuilderOp::InvsqrtFloat,
        signed_op: BuilderOp::Unsupported,
        unsigned_op: BuilderOp::Unsupported,
        boolean_op: BuilderOp::Unsupported,
    };
}

impl<'c> Generator<'c> {
    /// `Generator(program, debugTrace, writeTraceOps)`.
    fn new(
        ctx: &'c mut Context,
        usage: ProgramUsage,
        elements: Vec<ElemId>,
        source: Arc<[u8]>,
        debug_trace: Option<DebugTracePriv>,
        write_trace_ops: bool,
    ) -> Self {
        let with_debug = debug_trace.is_some();
        Self {
            ctx,
            usage,
            elements,
            source,
            builder: Builder::new(),
            debug_trace,
            write_trace_ops,
            child_effect_map: HashMap::new(),
            program_slots: SlotManager::new(with_debug),
            uniform_slots: SlotManager::new(with_debug),
            immutable_slots: SlotManager::new(false),
            trace_mask: None,
            current_function: None,
            current_function_result: SlotRange::default(),
            current_continue_mask: None,
            current_break_target: -1,
            current_stack: 0,
            next_stack_id: 0,
            recycled_stacks: Vec::new(),
            stack_parent: HashMap::new(),
            return_complexity_map: HashMap::new(),
            immutable_slot_map: THashMap::new(),
            immutable_variables: HashSet::new(),
            inside_compound_statement: 0,
            line_offsets: Vec::new(),
            lvalues: LValueArena::default(),
        }
    }

    // ---- small helpers -----------------------------------------------------------------

    /// `e.type()`.
    fn expr_type(&self, e: ExprId) -> TypeId {
        self.ctx.pool.expression(e).ty
    }

    /// `e.type().slotCount()`.
    fn expr_slots(&self, e: ExprId) -> i32 {
        self.type_slots(self.expr_type(e))
    }

    /// `type.slotCount()` as the builder's `int`.
    fn type_slots(&self, ty: TypeId) -> i32 {
        i32::try_from(self.ctx.pool.ty(ty).slot_count()).expect("slot count fits i32")
    }

    /// A temporary `Literal{Position{}, value, &ty}` (Skia builds these on the C++ stack; here
    /// they are pool nodes that nothing else refers to).
    fn make_literal(&mut self, pos: Position, value: f64, ty: TypeId) -> ExprId {
        self.ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Literal(Literal { value }),
        ))
    }

    // ---- variable classification ---------------------------------------------------------

    /// `IsUniform`.
    fn is_uniform(&self, var: VarId) -> bool {
        self.ctx.pool.variable(var).modifier_flags.is_uniform()
    }

    /// `IsOutParameter`.
    fn is_out_parameter(&self, var: VarId) -> bool {
        (self.ctx.pool.variable(var).modifier_flags & (ModifierFlags::IN | ModifierFlags::OUT))
            == ModifierFlags::OUT
    }

    /// `IsInoutParameter`.
    fn is_inout_parameter(&self, var: VarId) -> bool {
        (self.ctx.pool.variable(var).modifier_flags & (ModifierFlags::IN | ModifierFlags::OUT))
            == (ModifierFlags::IN | ModifierFlags::OUT)
    }

    /// `hasVariableSlots`: true for variables with slots in `fProgramSlots`; immutables and
    /// uniforms are false.
    fn has_variable_slots(&self, v: VarId) -> bool {
        !self.is_uniform(v) && !self.immutable_variables.contains(&v)
    }

    /// `getVariableSlots`.
    fn get_variable_slots(&mut self, v: VarId) -> SlotRange {
        debug_assert!(self.has_variable_slots(v));
        self.program_slots.get_variable_slots(&self.ctx.pool, v)
    }

    /// `getImmutableSlots`.
    fn get_immutable_slots(&mut self, v: VarId) -> SlotRange {
        debug_assert!(!self.is_uniform(v));
        debug_assert!(self.immutable_variables.contains(&v));
        self.immutable_slots.get_variable_slots(&self.ctx.pool, v)
    }

    /// `getUniformSlots`.
    fn get_uniform_slots(&mut self, v: VarId) -> SlotRange {
        debug_assert!(self.is_uniform(v));
        debug_assert!(!self.immutable_variables.contains(&v));
        self.uniform_slots.get_variable_slots(&self.ctx.pool, v)
    }

    /// `getFunctionSlots`.
    fn get_function_slots(&mut self, call_site: SlotKey, f: FnId) -> SlotRange {
        self.program_slots
            .get_function_slots(&self.ctx.pool, call_site, f)
    }

    // ---- stacks ---------------------------------------------------------------------------

    /// `createStack`: creates an additional stack for the program to push values onto. The stack
    /// is not in use until it is entered.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1235-L1242 (chrome/m156)
    fn create_stack(&mut self) -> i32 {
        if let Some(stack_id) = self.recycled_stacks.pop() {
            return stack_id;
        }
        self.next_stack_id += 1;
        self.next_stack_id
    }

    /// `recycleStack`: frees a stack made by `createStack`. The freed stack must be empty.
    fn recycle_stack(&mut self, stack_id: i32) {
        self.recycled_stacks.push(stack_id);
    }

    /// `setCurrentStack`: redirects builder ops to a different stack.
    fn set_current_stack(&mut self, stack_id: i32) {
        if self.current_stack != stack_id {
            self.current_stack = stack_id;
            self.builder.set_current_stack(stack_id);
        }
    }

    /// `currentStack`.
    fn current_stack(&self) -> i32 {
        self.current_stack
    }

    /// `AutoStack(Generator*)`.
    fn new_auto_stack(&mut self) -> Stack {
        Stack(self.create_stack())
    }

    /// `~AutoStack`.
    fn drop_auto_stack(&mut self, stack: Stack) {
        self.recycle_stack(stack.0);
    }

    /// `AutoStack::enter`: activates the stack.
    fn stack_enter(&mut self, stack: Stack) {
        self.stack_parent.insert(stack.0, self.current_stack);
        self.set_current_stack(stack.0);
    }

    /// `AutoStack::exit`: returns to the previously active stack.
    fn stack_exit(&mut self, stack: Stack) {
        debug_assert_eq!(self.current_stack, stack.0);
        let parent = self.stack_parent.get(&stack.0).copied().unwrap_or(0);
        self.set_current_stack(parent);
    }

    /// `AutoStack::pushClone(slots)`: clones values from the stack onto the active stack.
    fn stack_push_clone(&mut self, stack: Stack, slots: i32) {
        self.stack_push_clone_range(
            stack,
            SlotRange {
                index: 0,
                count: slots,
            },
            slots,
        );
    }

    /// `AutoStack::pushClone(range, offsetFromStackTop)`.
    fn stack_push_clone_range(
        &mut self,
        stack: Stack,
        range: SlotRange,
        offset_from_stack_top: i32,
    ) {
        self.builder
            .push_clone_from_stack(range, stack.0, offset_from_stack_top);
    }

    /// `AutoStack::pushCloneIndirect`.
    fn stack_push_clone_indirect(
        &mut self,
        stack: Stack,
        range: SlotRange,
        dynamic_stack_id: i32,
        offset_from_stack_top: i32,
    ) {
        self.builder.push_clone_indirect_from_stack(
            range,
            dynamic_stack_id,
            stack.0,
            offset_from_stack_top,
        );
    }

    // ---- AutoContinueMask / AutoLoopTarget ---------------------------------------------

    /// `AutoContinueMask::enable`.
    fn continue_mask_enable(&mut self, mask: &mut ContinueMask) {
        debug_assert!(mask.continue_mask_stack.is_none());
        mask.continue_mask_stack = Some(self.new_auto_stack());
        mask.previous_continue_mask = self.current_continue_mask;
        self.current_continue_mask = mask.continue_mask_stack;
    }

    /// `AutoContinueMask::enterLoopBody`.
    fn continue_mask_enter_loop_body(&mut self, mask: &ContinueMask) {
        if let Some(stack) = mask.continue_mask_stack {
            self.stack_enter(stack);
            self.builder.push_constant_i(0, 1);
            self.stack_exit(stack);
        }
    }

    /// `AutoContinueMask::exitLoopBody`.
    fn continue_mask_exit_loop_body(&mut self, mask: &ContinueMask) {
        if let Some(stack) = mask.continue_mask_stack {
            self.stack_enter(stack);
            self.builder.pop_and_reenable_loop_mask();
            self.stack_exit(stack);
        }
    }

    /// `~AutoContinueMask`: restores the previous continue mask (only if there was one), then the
    /// member `fContinueMaskStack` recycles its stack.
    #[allow(clippy::needless_pass_by_value)] // `mask` is destroyed here, as `~AutoContinueMask`.
    fn continue_mask_drop(&mut self, mask: ContinueMask) {
        if let Some(previous) = mask.previous_continue_mask {
            self.current_continue_mask = Some(previous);
        }
        if let Some(stack) = mask.continue_mask_stack {
            self.drop_auto_stack(stack);
        }
    }

    // ---- program analysis helpers --------------------------------------------------------

    /// `returnComplexity`.
    fn return_complexity(&mut self, func: ElemId) -> ReturnComplexity {
        if let Some(complexity) = self.return_complexity_map.get(&func) {
            return *complexity;
        }
        let ProgramElementKind::Function(def) = &self.ctx.pool.element(func).kind else {
            panic!("returnComplexity: not a function definition");
        };
        let complexity = analysis::get_return_complexity(&self.ctx.pool, def);
        self.return_complexity_map.insert(func, complexity);
        complexity
    }

    /// `needsReturnMask`.
    fn needs_return_mask(&mut self, func: ElemId) -> bool {
        self.return_complexity(func) >= ReturnComplexity::EarlyReturns
    }

    /// `needsFunctionResultSlots`.
    fn needs_function_result_slots(&mut self, func: ElemId) -> bool {
        self.should_write_trace_ops()
            || self.return_complexity(func) > ReturnComplexity::SingleSafeReturn
    }

    /// `shouldWriteTraceOps`.
    fn should_write_trace_ops(&self) -> bool {
        self.debug_trace.is_some() && self.write_trace_ops
    }

    /// `traceMaskStackID`.
    fn trace_mask_stack_id(&self) -> i32 {
        self.trace_mask.expect("a trace mask exists").stack_id()
    }

    // ---- slot helpers ---------------------------------------------------------------------

    /// `popToSlotRange`: pops an expression from the value stack and copies it into slots.
    fn pop_to_slot_range(&mut self, r: SlotRange) {
        self.builder.pop_slots(r);
        if self.should_write_trace_ops() {
            self.builder.trace_var(self.trace_mask_stack_id(), r);
        }
    }

    /// `popToSlotRangeUnmasked`.
    fn pop_to_slot_range_unmasked(&mut self, r: SlotRange) {
        self.builder.pop_slots_unmasked(r);
        if self.should_write_trace_ops() {
            self.builder.trace_var(self.trace_mask_stack_id(), r);
        }
    }

    /// `discardExpression`: pops an expression from the value stack and discards it.
    fn discard_expression(&mut self, slots: i32) {
        self.builder.discard_stack(slots);
    }

    /// `zeroSlotRangeUnmasked`.
    fn zero_slot_range_unmasked(&mut self, r: SlotRange) {
        self.builder.zero_slots_unmasked(r);
        if self.should_write_trace_ops() {
            self.builder.trace_var(self.trace_mask_stack_id(), r);
        }
    }

    // ---- tracing --------------------------------------------------------------------------

    /// `getFunctionDebugInfo`: the index of this function inside `FunctionDebugInfo`; created if
    /// it does not exist.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1213-L1233 (chrome/m156)
    fn get_function_debug_info(&mut self, decl: FnId) -> i32 {
        let trace = self.debug_trace.as_mut().expect("a debug trace exists");
        let mut name = self.ctx.pool.function(decl).description(&self.ctx.pool);

        // When generating the debug trace, we typically mark every function as `noinline`. This
        // makes the trace more confusing, since this isn't in the source program, so remove it.
        if name.starts_with("noinline ") {
            name.drain(.."noinline ".len());
        }

        // Look for a matching FunctionDebugInfo slot.
        if let Some(index) = trace.func_info.iter().position(|f| f.name == name) {
            return i32::try_from(index).expect("function index fits i32");
        }

        // We've never called this function before; create a new slot to hold its information.
        let slot = i32::try_from(trace.func_info.len()).expect("function index fits i32");
        trace.func_info.push(FunctionDebugInfo { name });
        slot
    }

    /// `emitTraceLine`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1500-L1514 (chrome/m156)
    fn emit_trace_line(&mut self, pos: Position) {
        if self.should_write_trace_ops() && pos.valid() && self.inside_compound_statement == 0 {
            // Binary search within fLineOffets to convert the position into a line number.
            debug_assert!(self.line_offsets.len() >= 2);
            debug_assert_eq!(self.line_offsets[0], 0);
            let start = pos.start_offset();
            let line_number = self.line_offsets.partition_point(|&offset| offset <= start);
            self.builder.trace_line(
                self.trace_mask_stack_id(),
                i32::try_from(line_number).expect("line number fits i32"),
            );
        }
    }

    /// `pushTraceScopeMask`.
    fn push_trace_scope_mask(&mut self) {
        if self.should_write_trace_ops() {
            // Take the intersection of the trace mask and the execution mask. To do this, start
            // with an all-zero mask, then use select to overwrite those zeros with the trace mask
            // across all executing lanes. We'll get the trace mask in executing lanes, and zero
            // in dead lanes.
            self.builder.push_constant_i(0, 1);
            let mask = self.trace_mask.expect("a trace mask exists");
            self.stack_push_clone(mask, 1);
            self.builder.select(1);
        }
    }

    /// `discardTraceScopeMask`.
    fn discard_trace_scope_mask(&mut self) {
        if self.should_write_trace_ops() {
            self.discard_expression(1);
        }
    }

    /// `emitTraceScope`.
    fn emit_trace_scope(&mut self, delta: i32) {
        if self.should_write_trace_ops() {
            self.builder.trace_scope(self.current_stack(), delta);
        }
    }

    /// `calculateLineOffsets`: prepares the position-to-line-offset table.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1526-L1535 (chrome/m156)
    fn calculate_line_offsets(&mut self) {
        debug_assert_eq!(self.line_offsets.len(), 0);
        self.line_offsets.push(0);
        for (i, &byte) in self.source.iter().enumerate() {
            if byte == b'\n' {
                self.line_offsets
                    .push(i32::try_from(i).expect("source offset fits i32"));
            }
        }
        self.line_offsets
            .push(i32::try_from(self.source.len()).expect("source length fits i32"));
    }

    // ---- entry points ---------------------------------------------------------------------

    /// `writeProgram`: converts the `SkSL` `main()` function into a set of instructions.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3975-L4075 (chrome/m156)
    fn write_program(&mut self, function: ElemId) -> Res {
        self.current_function = Some(function);
        let (decl_id, _) = self.function_parts(function);

        if self.debug_trace.is_some() {
            // Copy the program source into the debug info so that it will be written in the trace
            // file.
            let source = Arc::clone(&self.source);
            if let Some(trace) = &mut self.debug_trace {
                trace.set_source(&source);
            }

            if self.write_trace_ops {
                // The Raster Pipeline blitter generates centered pixel coordinates. (0.5, 1.5,
                // 2.5, etc.) Add 0.5 to the requested trace coordinate to match this, then
                // compare against src.rg, which contains the shader's coordinates. We keep this
                // result in a dedicated trace-mask stack.
                let (trace_x, trace_y) = self
                    .debug_trace
                    .as_ref()
                    .map_or((0, 0), |trace| trace.trace_coord);
                let mask = self.new_auto_stack();
                self.trace_mask = Some(mask);
                self.stack_enter(mask);
                self.builder.push_device_xy01();
                self.builder.discard_stack(2);
                #[allow(clippy::cast_precision_loss)] // Mirrors `fTraceCoord.fX + 0.5f`.
                {
                    self.builder.push_constant_f(trace_x as f32 + 0.5);
                    self.builder.push_constant_f(trace_y as f32 + 0.5);
                }
                self.builder.binary_op(BuilderOp::CmpeqNFloats, 2);
                self.builder.binary_op(BuilderOp::BitwiseAndNInts, 1);
                self.stack_exit(mask);

                // Assemble a position-to-line-number mapping for the debugger.
                self.calculate_line_offsets();
            }
        }

        // Assign slots to the parameters of main; copy src and dst into those slots as
        // appropriate.
        let decl = self.ctx.pool.function(decl_id);
        let main_coords_param = decl.main_coords_parameter();
        let main_input_color_param = decl.main_input_color_parameter();
        let main_dest_color_param = decl.main_dest_color_parameter();
        let parameters = decl.parameters.clone();

        for param in parameters {
            if Some(param) == main_coords_param {
                // Coordinates are passed via RG.
                let frag_coord = self.get_variable_slots(param);
                debug_assert_eq!(frag_coord.count, 2);
                self.builder.store_src_rg(frag_coord);
            } else if Some(param) == main_input_color_param {
                // Input colors are passed via RGBA.
                let src_color = self.get_variable_slots(param);
                debug_assert_eq!(src_color.count, 4);
                self.builder.store_src(src_color);
            } else if Some(param) == main_dest_color_param {
                // Dest colors are passed via dRGBA.
                let dest_color = self.get_variable_slots(param);
                debug_assert_eq!(dest_color.count, 4);
                self.builder.store_dst(dest_color);
            } else {
                debug_assert!(false, "Invalid parameter to main()");
                return unsupported();
            }
        }

        // Initialize the program.
        self.builder.init_lane_masks();

        // Emit global variables.
        self.write_globals()?;

        // Invoke main().
        let main_result = self.write_function(SlotKey::CallElement(function), function, &[])?;

        // Move the result of main() from slots into RGBA.
        debug_assert_eq!(main_result.count, 4);
        let current = self.current_function.expect("a current function");
        if self.needs_function_result_slots(current) {
            self.builder.load_src(main_result);
        } else {
            self.builder.pop_src_rgba();
        }

        // Discard the trace mask.
        if let Some(mask) = self.trace_mask {
            self.stack_enter(mask);
            self.builder.discard_stack(1);
            self.stack_exit(mask);
        }

        Ok(())
    }

    /// The declaration and body of a function definition element.
    fn function_parts(&self, function: ElemId) -> (FnId, crate::ir::StmtId) {
        match &self.ctx.pool.element(function).kind {
            ProgramElementKind::Function(def) => (def.declaration, def.body),
            _ => panic!("expected a function definition"),
        }
    }

    /// `finish`: returns the generated program.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L4077-L4082 (chrome/m156)
    fn finish(mut self) -> RpProgram {
        // The slot managers own the slot debug info that Skia keeps inside `DebugTracePriv`.
        let debug_trace = self.debug_trace.take().map(|mut trace| {
            trace.slot_info = self
                .program_slots
                .slot_debug_info
                .take()
                .unwrap_or_default();
            trace.uniform_info = self
                .uniform_slots
                .slot_debug_info
                .take()
                .unwrap_or_default();
            Arc::new(trace)
        });
        self.builder.finish(
            self.program_slots.slot_count(),
            self.uniform_slots.slot_count(),
            self.immutable_slots.slot_count(),
            debug_trace,
        )
    }
}

/// `SkSL::MakeRasterPipelineProgram`: converts `function` (the definition of `main`) of a
/// program into a Raster Pipeline [`RpProgram`]. Returns `None` when the program uses something
/// the Raster Pipeline back end does not support.
///
/// `debug_trace` is the debug information to fill in (Skia's `DebugTracePriv*`); the finished
/// program owns it. `write_trace_ops` emits the trace ops that make the debug trace useful.
///
/// # Panics
///
/// If `function` is not a function definition element.
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L4084-L4098 (chrome/m156)
#[doc(alias = "MakeRasterPipelineProgram")]
#[must_use]
pub fn make_raster_pipeline_program(
    program: &mut Program,
    function: ElemId,
    debug_trace: Option<DebugTracePriv>,
    write_trace_ops: bool,
) -> Option<RpProgram> {
    assert!(
        matches!(
            program.pool.element(function).kind,
            ProgramElementKind::Function(_)
        ),
        "MakeRasterPipelineProgram: not a function definition"
    );
    let usage = analysis::get_usage(program);
    let elements: Vec<ElemId> = program.elements().collect();
    let source = Arc::clone(&program.source);
    let mut ctx = Context::new(ErrorReporter::compiler());
    ctx.with_program(program, |ctx| {
        let mut generator =
            Generator::new(ctx, usage, elements, source, debug_trace, write_trace_ops);
        generator.write_program(function).ok()?;
        Some(generator.finish())
    })
}
