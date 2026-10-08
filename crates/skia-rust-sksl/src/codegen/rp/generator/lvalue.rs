// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp (`LValue` and its
// subclasses, `Generator::makeLValue`, `Generator::push`, `Generator::store`).

//! `LValue`s: places that expressions can be read from and written to.
//!
//! Skia's `LValue` is a class hierarchy of owned (`std::unique_ptr`) and borrowed (`LValue*`)
//! parents. Here the nodes live in an arena and are named by [`LvId`]; an `UnownedLValueSlice`
//! is a [`LNode::Slice`] with `owns_parent: false`. Virtual dispatch is a `match` on the node.
//! Destruction is explicit: [`Generator::free_lvalue`] runs what the destructors would.

// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L630-L1005 (chrome/m156)

use super::{Generator, Res, Stack, unsupported};
use crate::codegen::rp::builder::SlotRange;
use crate::constant_folder;
use crate::ir::{ComponentArray, ExprId, ExpressionKind, VarId};
use crate::transform;

/// A handle to an `LValue` in the generator's arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LvId(usize);

/// The `LValue` subclasses.
#[derive(Clone, Copy, Debug)]
pub(super) enum LNode {
    /// `ScratchLValue`: a temporary expression, read-only.
    Scratch {
        expr: ExprId,
        num_slots: i32,
        dedicated_stack: Option<Stack>,
    },
    /// `VariableLValue`.
    Variable { var: VarId },
    /// `ImmutableLValue`.
    Immutable { var: VarId },
    /// `SwizzleLValue`.
    Swizzle {
        parent: LvId,
        components: ComponentArray,
    },
    /// `LValueSlice` (`owns_parent`) and `UnownedLValueSlice`.
    Slice {
        parent: LvId,
        initial_slot: i32,
        num_slots: i32,
        owns_parent: bool,
    },
    /// `DynamicIndexLValue`.
    DynamicIndex {
        parent: LvId,
        index_expr: ExprId,
        dedicated_stack: Option<Stack>,
    },
}

/// The `LValue`s that have been made.
#[derive(Debug, Default)]
pub(super) struct LValueArena {
    nodes: Vec<LNode>,
}

impl LValueArena {
    fn add(&mut self, node: LNode) -> LvId {
        self.nodes.push(node);
        LvId(self.nodes.len() - 1)
    }

    fn get(&self, id: LvId) -> LNode {
        self.nodes[id.0]
    }

    fn set(&mut self, id: LvId, node: LNode) {
        self.nodes[id.0] = node;
    }
}

/// `is_sliceable_swizzle`: determines if the swizzle rearranges its elements, or if it's a simple
/// subset of its elements. (A simple subset would be a sequential non-repeating range of
/// components, like `.xyz` or `.yzw` or `.z`, but not `.xx` or `.xz`, which can be accessed as a
/// slice of the variable.)
// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1097-L1106 (chrome/m156)
pub(super) fn is_sliceable_swizzle(components: &[i8]) -> bool {
    for (index, &component) in components.iter().enumerate().skip(1) {
        // `int8_t(components[0] + index)`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // Mirrors the C++ narrowing to int8_t.
        let expected = (i32::from(components[0]) + index as i32) as i8;
        if component != expected {
            return false;
        }
    }
    true
}

impl Generator<'_> {
    // ---- the LValue interface --------------------------------------------------------------

    /// `isWritable`: false for temporaries, immutables and uniforms.
    pub(super) fn lv_is_writable(&self, lv: LvId) -> bool {
        match self.lvalues.get(lv) {
            LNode::Scratch { .. } | LNode::Immutable { .. } => false,
            LNode::Variable { var } => !self.is_uniform(var),
            LNode::Swizzle { parent, .. }
            | LNode::Slice { parent, .. }
            | LNode::DynamicIndex { parent, .. } => self.lv_is_writable(parent),
        }
    }

    /// `fixedSlotRange`: the fixed slot range of the lvalue, after it is winnowed down to the
    /// selected field/index. The range is calculated assuming every dynamic index evaluates to
    /// zero.
    pub(super) fn lv_fixed_slot_range(&mut self, lv: LvId) -> SlotRange {
        match self.lvalues.get(lv) {
            LNode::Scratch { num_slots, .. } => SlotRange {
                index: 0,
                count: num_slots,
            },
            LNode::Variable { var } => {
                if self.is_uniform(var) {
                    self.get_uniform_slots(var)
                } else {
                    self.get_variable_slots(var)
                }
            }
            LNode::Immutable { var } => self.get_immutable_slots(var),
            LNode::Swizzle { parent, .. } => self.lv_fixed_slot_range(parent),
            LNode::Slice {
                parent,
                initial_slot,
                num_slots,
                ..
            } => {
                let range = self.lv_fixed_slot_range(parent);
                let mut adjusted = range;
                adjusted.index += initial_slot;
                adjusted.count = num_slots;
                debug_assert!(adjusted.index + adjusted.count <= range.index + range.count);
                adjusted
            }
            LNode::DynamicIndex {
                parent, index_expr, ..
            } => {
                // Compute the fixed slot range as if we are indexing into position zero.
                let mut range = self.lv_fixed_slot_range(parent);
                range.count = self.expr_slots(index_expr);
                range
            }
        }
    }

    /// `dynamicSlotRange`: a stack which holds a single integer, the dynamic offset of the
    /// lvalue. This value does not incorporate the fixed offset. `None` if the lvalue doesn't
    /// have a dynamic offset.
    pub(super) fn lv_dynamic_slot_range(&self, lv: LvId) -> Option<Stack> {
        match self.lvalues.get(lv) {
            LNode::Scratch { .. } | LNode::Variable { .. } | LNode::Immutable { .. } => None,
            LNode::Swizzle { parent, .. } | LNode::Slice { parent, .. } => {
                self.lv_dynamic_slot_range(parent)
            }
            LNode::DynamicIndex {
                dedicated_stack, ..
            } => {
                // We incorporated any parent dynamic offsets when `evaluateDynamicIndices` was
                // called.
                debug_assert!(dedicated_stack.is_some());
                dedicated_stack
            }
        }
    }

    /// `swizzle`: the swizzle components of the lvalue, or an empty array for non-swizzle
    /// `LValue`s.
    pub(super) fn lv_swizzle(&self, lv: LvId) -> ComponentArray {
        match self.lvalues.get(lv) {
            LNode::Swizzle { components, .. } => components,
            _ => ComponentArray::default(),
        }
    }

    /// `LValue::push`: pushes values directly onto the stack.
    pub(super) fn lv_push(
        &mut self,
        lv: LvId,
        fixed_offset: SlotRange,
        dynamic_offset: Option<Stack>,
        swizzle: ComponentArray,
    ) -> Res {
        match self.lvalues.get(lv) {
            LNode::Scratch {
                expr,
                num_slots,
                dedicated_stack,
            } => {
                let stack = if let Some(stack) = dedicated_stack {
                    stack
                } else {
                    // Push the scratch expression onto a dedicated stack.
                    let stack = self.new_auto_stack();
                    self.lvalues.set(
                        lv,
                        LNode::Scratch {
                            expr,
                            num_slots,
                            dedicated_stack: Some(stack),
                        },
                    );
                    self.stack_enter(stack);
                    self.push_expression(expr, true)?;
                    self.stack_exit(stack);
                    stack
                };

                if let Some(dynamic) = dynamic_offset {
                    self.stack_push_clone_indirect(
                        stack,
                        fixed_offset,
                        dynamic.stack_id(),
                        num_slots,
                    );
                } else {
                    self.stack_push_clone_range(stack, fixed_offset, num_slots);
                }
                if !swizzle.is_empty() {
                    self.builder.swizzle(fixed_offset.count, &swizzle);
                }
                Ok(())
            }
            LNode::Variable { var } => {
                if self.is_uniform(var) {
                    if let Some(dynamic) = dynamic_offset {
                        let limit = self.lv_fixed_slot_range(lv);
                        self.builder
                            .push_uniform_indirect(fixed_offset, dynamic.stack_id(), limit);
                    } else {
                        self.builder.push_uniform(fixed_offset);
                    }
                } else if let Some(dynamic) = dynamic_offset {
                    let limit = self.lv_fixed_slot_range(lv);
                    self.builder
                        .push_slots_indirect(fixed_offset, dynamic.stack_id(), limit);
                } else {
                    self.builder.push_slots(fixed_offset);
                }
                if !swizzle.is_empty() {
                    self.builder.swizzle(fixed_offset.count, &swizzle);
                }
                Ok(())
            }
            LNode::Immutable { .. } => {
                if let Some(dynamic) = dynamic_offset {
                    let limit = self.lv_fixed_slot_range(lv);
                    self.builder
                        .push_immutable_indirect(fixed_offset, dynamic.stack_id(), limit);
                } else {
                    self.builder.push_immutable(fixed_offset);
                }
                if !swizzle.is_empty() {
                    self.builder.swizzle(fixed_offset.count, &swizzle);
                }
                Ok(())
            }
            LNode::Swizzle { parent, components } => {
                if !swizzle.is_empty() {
                    debug_assert!(false, "swizzle-of-a-swizzle should have been folded out");
                    return unsupported();
                }
                self.lv_push(parent, fixed_offset, dynamic_offset, components)
            }
            LNode::Slice { parent, .. } | LNode::DynamicIndex { parent, .. } => {
                self.lv_push(parent, fixed_offset, dynamic_offset, swizzle)
            }
        }
    }

    /// `LValue::store`: stores the topmost values from the stack directly into the lvalue.
    pub(super) fn lv_store(
        &mut self,
        lv: LvId,
        fixed_offset: SlotRange,
        dynamic_offset: Option<Stack>,
        swizzle: ComponentArray,
    ) -> Res {
        match self.lvalues.get(lv) {
            LNode::Scratch { .. } => {
                debug_assert!(false, "scratch lvalues cannot be stored into");
                unsupported()
            }
            LNode::Immutable { .. } => {
                debug_assert!(false, "immutable values cannot be stored into");
                unsupported()
            }
            LNode::Variable { var } => {
                debug_assert!(!self.is_uniform(var));

                if swizzle.is_empty() {
                    if let Some(dynamic) = dynamic_offset {
                        let limit = self.lv_fixed_slot_range(lv);
                        self.builder.copy_stack_to_slots_indirect(
                            fixed_offset,
                            dynamic.stack_id(),
                            limit,
                        );
                    } else {
                        self.builder
                            .copy_stack_to_slots(fixed_offset, fixed_offset.count);
                    }
                } else if let Some(dynamic) = dynamic_offset {
                    let limit = self.lv_fixed_slot_range(lv);
                    self.builder.swizzle_copy_stack_to_slots_indirect(
                        fixed_offset,
                        dynamic.stack_id(),
                        limit,
                        &swizzle,
                        i32::try_from(swizzle.len()).expect("a swizzle has at most 4 components"),
                    );
                } else {
                    self.builder.swizzle_copy_stack_to_slots(
                        fixed_offset,
                        &swizzle,
                        i32::try_from(swizzle.len()).expect("a swizzle has at most 4 components"),
                    );
                }
                if self.should_write_trace_ops() {
                    if let Some(dynamic) = dynamic_offset {
                        let limit = self.lv_fixed_slot_range(lv);
                        self.builder.trace_var_indirect(
                            self.trace_mask_stack_id(),
                            fixed_offset,
                            dynamic.stack_id(),
                            limit,
                        );
                    } else {
                        self.builder
                            .trace_var(self.trace_mask_stack_id(), fixed_offset);
                    }
                }
                Ok(())
            }
            LNode::Swizzle { parent, components } => {
                if !swizzle.is_empty() {
                    debug_assert!(false, "swizzle-of-a-swizzle should have been folded out");
                    return unsupported();
                }
                self.lv_store(parent, fixed_offset, dynamic_offset, components)
            }
            LNode::Slice { parent, .. } | LNode::DynamicIndex { parent, .. } => {
                self.lv_store(parent, fixed_offset, dynamic_offset, swizzle)
            }
        }
    }

    /// `DynamicIndexLValue::evaluateDynamicIndices`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L936-L973 (chrome/m156)
    fn evaluate_dynamic_indices(&mut self, lv: LvId) -> Res {
        let LNode::DynamicIndex {
            parent, index_expr, ..
        } = self.lvalues.get(lv)
        else {
            panic!("evaluateDynamicIndices: not a dynamic index lvalue");
        };
        // The index must only be computed once; the index-expression could have side effects.
        // Once it has been computed, the offset lives on the dedicated stack.
        let stack = self.new_auto_stack();
        self.lvalues.set(
            lv,
            LNode::DynamicIndex {
                parent,
                index_expr,
                dedicated_stack: Some(stack),
            },
        );

        if !self.lv_swizzle(parent).is_empty() {
            debug_assert!(false, "an indexed-swizzle should have been handled");
            return unsupported();
        }

        // Push the index expression onto the dedicated stack.
        self.stack_enter(stack);
        let ExpressionKind::Index(index) = self.ctx.pool.expression(index_expr).kind.clone() else {
            panic!("DynamicIndexLValue: not an index expression");
        };
        self.push_expression(index.index, true)?;

        // Multiply the index-expression result by the per-value slot count.
        let slot_count = self.expr_slots(index_expr);
        if slot_count != 1 {
            self.builder.push_constant_i(slot_count, 1);
            self.builder.binary_op(super::BuilderOp::MulNInts, 1);
        }

        // Check to see if a parent LValue already has a dynamic index. If so, we need to
        // incorporate its value into our own.
        if let Some(parent_dynamic_index_stack) = self.lv_dynamic_slot_range(parent) {
            self.stack_push_clone(parent_dynamic_index_stack, 1);
            self.builder.binary_op(super::BuilderOp::AddNInts, 1);
        }
        self.stack_exit(stack);
        Ok(())
    }

    /// `Generator::push(LValue&)`: pushes the lvalue onto the top-of-stack.
    pub(super) fn push_lvalue(&mut self, lv: LvId) -> Res {
        let fixed = self.lv_fixed_slot_range(lv);
        let dynamic = self.lv_dynamic_slot_range(lv);
        self.lv_push(lv, fixed, dynamic, ComponentArray::default())
    }

    /// `Generator::store(LValue&)`: copies the top-of-stack value into this lvalue, without
    /// discarding it from the stack.
    pub(super) fn store_lvalue(&mut self, lv: LvId) -> Res {
        debug_assert!(self.lv_is_writable(lv));
        let fixed = self.lv_fixed_slot_range(lv);
        let dynamic = self.lv_dynamic_slot_range(lv);
        self.lv_store(lv, fixed, dynamic, ComponentArray::default())
    }

    /// `~LValue`: runs what the destructors of this lvalue and the lvalues it owns do, in the
    /// order they run.
    pub(super) fn free_lvalue(&mut self, lv: LvId) {
        match self.lvalues.get(lv) {
            LNode::Scratch {
                num_slots,
                dedicated_stack,
                ..
            } => {
                if let Some(stack) = dedicated_stack {
                    // Jettison the scratch expression.
                    self.stack_enter(stack);
                    self.discard_expression(num_slots);
                    self.stack_exit(stack);
                    // The `fDedicatedStack` member is destroyed after the destructor body.
                    self.drop_auto_stack(stack);
                }
            }
            LNode::Variable { .. } | LNode::Immutable { .. } => {}
            LNode::Swizzle { parent, .. } => self.free_lvalue(parent),
            LNode::Slice {
                parent,
                owns_parent,
                ..
            } => {
                if owns_parent {
                    self.free_lvalue(parent);
                }
            }
            LNode::DynamicIndex {
                parent,
                dedicated_stack,
                ..
            } => {
                if let Some(stack) = dedicated_stack {
                    // Jettison the index expression.
                    self.stack_enter(stack);
                    self.discard_expression(1);
                    self.stack_exit(stack);
                    // Members are destroyed in reverse order: `fDedicatedStack`, then `fParent`.
                    self.drop_auto_stack(stack);
                }
                self.free_lvalue(parent);
            }
        }
    }

    /// An `UnownedLValueSlice`.
    pub(super) fn make_unowned_slice(
        &mut self,
        parent: LvId,
        initial_slot: i32,
        num_slots: i32,
    ) -> LvId {
        debug_assert!(initial_slot >= 0);
        debug_assert!(num_slots > 0);
        self.lvalues.add(LNode::Slice {
            parent,
            initial_slot,
            num_slots,
            owns_parent: false,
        })
    }

    /// `Generator::makeLValue`: returns an `LValue` for the expression; if the expression isn't
    /// supported as an `LValue`, returns `None`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1108-L1189 (chrome/m156)
    pub(super) fn make_lvalue(&mut self, e: ExprId, allow_scratch: bool) -> Option<LvId> {
        let kind = self.ctx.pool.expression(e).kind.clone();
        let ty = self.ctx.pool.expression(e).ty;
        match kind {
            ExpressionKind::VariableReference(var_ref) => {
                let variable = var_ref.variable;
                if self.immutable_variables.contains(&variable) {
                    return Some(self.lvalues.add(LNode::Immutable { var: variable }));
                }
                Some(self.lvalues.add(LNode::Variable { var: variable }))
            }
            ExpressionKind::Swizzle(swizzle_expr) => {
                let base = self.make_lvalue(swizzle_expr.base, allow_scratch)?;
                let components = swizzle_expr.components;
                if is_sliceable_swizzle(&components) {
                    // If the swizzle is a contiguous subset, we can represent it with a fixed
                    // slice.
                    return Some(self.lvalues.add(LNode::Slice {
                        parent: base,
                        initial_slot: i32::from(components[0]),
                        num_slots: i32::try_from(components.len()).expect("at most 4"),
                        owns_parent: true,
                    }));
                }
                Some(self.lvalues.add(LNode::Swizzle {
                    parent: base,
                    components,
                }))
            }
            ExpressionKind::FieldAccess(field_expr) => {
                let base = self.make_lvalue(field_expr.base, allow_scratch)?;
                // Represent field access with a slice.
                let initial_slot = field_expr.initial_slot(&self.ctx.pool);
                Some(self.lvalues.add(LNode::Slice {
                    parent: base,
                    initial_slot: i32::try_from(initial_slot).expect("slot fits i32"),
                    num_slots: self.type_slots(ty),
                    owns_parent: true,
                }))
            }
            ExpressionKind::Index(index_expr) => {
                // If the index base is swizzled (`vec.zyx[idx]`), rewrite it into an equivalent
                // non-swizzled form (`vec[uint3(2,1,0)[idx]]`).
                if let Some(rewritten) = transform::rewrite_indexed_swizzle(self.ctx, e) {
                    // Convert the rewritten expression into an lvalue. (The rewritten expression
                    // lives in the pool, which stands in for `fScratchExpression`.)
                    return self.make_lvalue(rewritten, allow_scratch);
                }
                let base = self.make_lvalue(index_expr.base, allow_scratch)?;
                // If the index is a compile-time constant, we can represent it with a fixed
                // slice.
                if let Some(index_value) =
                    constant_folder::get_constant_int(&self.ctx.pool, index_expr.index)
                {
                    let num_slots = self.type_slots(ty);
                    // `numSlots * indexValue` is an `SKSL_INT` product narrowed to `int`.
                    #[allow(clippy::cast_possible_truncation)]
                    let initial_slot = (i64::from(num_slots) * index_value) as i32;
                    return Some(self.lvalues.add(LNode::Slice {
                        parent: base,
                        initial_slot,
                        num_slots,
                        owns_parent: true,
                    }));
                }

                // Represent non-constant indexing via a dynamic index.
                debug_assert!(
                    self.ctx
                        .pool
                        .ty(self.ctx.pool.expression(index_expr.index).ty)
                        .is_integer()
                );
                let dyn_lvalue = self.lvalues.add(LNode::DynamicIndex {
                    parent: base,
                    index_expr: e,
                    dedicated_stack: None,
                });
                self.evaluate_dynamic_indices(dyn_lvalue)
                    .ok()
                    .map(|()| dyn_lvalue)
            }
            _ => {
                if allow_scratch {
                    // This path allows us to perform field- and index-accesses on an expression
                    // as if it were an lvalue, but is a temporary and shouldn't be written back
                    // to.
                    let num_slots = self.type_slots(ty);
                    return Some(self.lvalues.add(LNode::Scratch {
                        expr: e,
                        num_slots,
                        dedicated_stack: None,
                    }));
                }
                None
            }
        }
    }
}
