// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp (`Generator`'s
// `push*Expression` members).

//! Expressions: everything that pushes a value onto the stack.

// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2186-L3290 (chrome/m156)

use super::lvalue::LvId;
use super::{Generator, Res, TypedOps, unsupported};
use crate::analysis;
use crate::codegen::rp::builder::SlotRange;
use crate::codegen::rp::ops::BuilderOp;
use crate::constant_folder;
use crate::ir::{
    ChildCall, ExprId, ExpressionKind, FunctionCall, Literal, NumberKind, PostfixExpression,
    Swizzle, TypeId, TypeKind, VariableReference,
};
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;

impl Generator<'_> {
    /// `kind` of the elements of a type: `type.componentType().numberKind()`.
    pub(super) fn component_number_kind(&self, ty: TypeId) -> NumberKind {
        self.ctx.pool.ty(ty).component_type().number_kind()
    }

    /// `pushExpression`: pushes an expression to the value stack.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2186-L2250 (chrome/m156)
    pub(super) fn push_expression(&mut self, e: ExprId, uses_result: bool) -> Res {
        let kind = self.ctx.pool.expression(e).kind.clone();
        match kind {
            ExpressionKind::Binary(b) => self.push_binary_expression(b.left, b.operator, b.right),

            ExpressionKind::ChildCall(c) => self.push_child_call(&c),

            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorArrayCast(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorStruct(_) => self.push_constructor_compound(e),

            ExpressionKind::ConstructorCompoundCast(c) => self.push_constructor_cast(e, c.argument),
            ExpressionKind::ConstructorScalarCast(c) => self.push_constructor_cast(e, c.argument),

            ExpressionKind::ConstructorDiagonalMatrix(c) => {
                self.push_constructor_diagonal_matrix(e, c.argument)
            }

            ExpressionKind::ConstructorMatrixResize(c) => {
                self.push_constructor_matrix_resize(e, c.argument)
            }

            ExpressionKind::ConstructorSplat(c) => self.push_constructor_splat(e, c.argument),

            ExpressionKind::Empty(_) => Ok(()),

            ExpressionKind::FieldAccess(_) => self.push_field_access(e),

            ExpressionKind::FunctionCall(c) => self.push_function_call(e, &c),

            ExpressionKind::Index(_) => self.push_index_expression(e),

            ExpressionKind::Literal(l) => self.push_literal(e, l),

            ExpressionKind::Prefix(p) => self.push_prefix_expression(p.operator, p.operand),

            ExpressionKind::Postfix(p) => self.push_postfix_expression(e, &p, uses_result),

            ExpressionKind::Swizzle(s) => self.push_swizzle(&s),

            ExpressionKind::Ternary(t) => {
                self.push_ternary_expression(t.test, t.if_true, t.if_false)
            }

            ExpressionKind::VariableReference(v) => self.push_variable_reference(e, &v),

            _ => unsupported(),
        }
    }

    /// `GetTypedOp`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2252-L2261 (chrome/m156)
    pub(super) fn get_typed_op(&self, ty: TypeId, ops: TypedOps) -> BuilderOp {
        match self.component_number_kind(ty) {
            NumberKind::Float => ops.float_op,
            NumberKind::Signed => ops.signed_op,
            NumberKind::Unsigned => ops.unsigned_op,
            NumberKind::Boolean => ops.boolean_op,
            NumberKind::Nonnumeric => BuilderOp::Unsupported,
        }
    }

    /// `unaryOp`.
    pub(super) fn unary_op(&mut self, ty: TypeId, ops: TypedOps) -> Res {
        let op = self.get_typed_op(ty, ops);
        if op == BuilderOp::Unsupported {
            return unsupported();
        }
        let slots = self.type_slots(ty);
        self.builder.unary_op(op, slots);
        Ok(())
    }

    /// `binaryOp`.
    pub(super) fn binary_op(&mut self, ty: TypeId, ops: TypedOps) -> Res {
        let op = self.get_typed_op(ty, ops);
        if op == BuilderOp::Unsupported {
            return unsupported();
        }
        let slots = self.type_slots(ty);
        self.builder.binary_op(op, slots);
        Ok(())
    }

    /// `ternaryOp`.
    pub(super) fn ternary_op(&mut self, ty: TypeId, ops: TypedOps) -> Res {
        let op = self.get_typed_op(ty, ops);
        if op == BuilderOp::Unsupported {
            return unsupported();
        }
        let slots = self.type_slots(ty);
        self.builder.ternary_op(op, slots);
        Ok(())
    }

    /// `foldWithMultiOp`: folds the top N elements on the stack using an op that supports
    /// multiple slots, e.g.:
    /// `(A + B + C + D) -> add_2_floats $0..1 += $2..3; add_float $0 += $1`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2286-L2302 (chrome/m156)
    pub(super) fn fold_with_multi_op(&mut self, op: BuilderOp, mut elements: i32) {
        while elements >= 8 {
            self.builder.binary_op(op, 4);
            elements -= 4;
        }
        while elements >= 6 {
            self.builder.binary_op(op, 3);
            elements -= 3;
        }
        while elements >= 4 {
            self.builder.binary_op(op, 2);
            elements -= 2;
        }
        while elements >= 2 {
            self.builder.binary_op(op, 1);
            elements -= 1;
        }
    }

    /// `pushLValueOrExpression`.
    fn push_lvalue_or_expression(&mut self, lvalue: Option<LvId>, expr: ExprId) -> Res {
        match lvalue {
            Some(lvalue) => self.push_lvalue(lvalue),
            None => self.push_expression(expr, true),
        }
    }

    /// `pushMatrixMultiply`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2310-L2339 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors the C++ signature.
    fn push_matrix_multiply(
        &mut self,
        lvalue: Option<LvId>,
        left: ExprId,
        right: ExprId,
        left_columns: i32,
        left_rows: i32,
        right_columns: i32,
        right_rows: i32,
    ) -> Res {
        // Insert padding space on the stack to hold the result.
        self.builder.pad_stack(right_columns * left_rows);

        // Push the left and right matrices onto the stack.
        self.push_lvalue_or_expression(lvalue, left)?;
        self.push_expression(right, true)?;

        self.builder
            .matrix_multiply(left_columns, left_rows, right_columns, right_rows);

        // If this multiply was actually an assignment (via *=), write the result back to the
        // lvalue.
        match lvalue {
            Some(lvalue) => self.store_lvalue(lvalue),
            None => Ok(()),
        }
    }

    /// `foldComparisonOp`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2341-L2356 (chrome/m156)
    fn fold_comparison_op(&mut self, op: Operator, elements: i32) {
        match op.kind() {
            OperatorKind::EqEq => {
                // equal(x,y) returns a vector; use & to fold into a scalar.
                self.fold_with_multi_op(BuilderOp::BitwiseAndNInts, elements);
            }
            OperatorKind::Neq => {
                // notEqual(x,y) returns a vector; use | to fold into a scalar.
                self.fold_with_multi_op(BuilderOp::BitwiseOrNInts, elements);
            }
            _ => debug_assert!(false, "comparison only allows == and !="),
        }
    }

    /// `pushStructuredComparison`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2358-L2437 (chrome/m156)
    fn push_structured_comparison(
        &mut self,
        left: LvId,
        op: Operator,
        right: LvId,
        ty: TypeId,
    ) -> Res {
        let (is_struct, is_array) = {
            let t = self.ctx.pool.ty(ty);
            (t.is_struct(), t.is_array())
        };
        if is_struct {
            // Compare every field in the struct.
            let field_types: Vec<TypeId> =
                self.ctx.pool.ty(ty).fields().iter().map(|f| f.ty).collect();
            let mut current_slot = 0;
            for &field_type in &field_types {
                let field_slot_count = self.type_slots(field_type);
                let field_left = self.make_unowned_slice(left, current_slot, field_slot_count);
                let field_right = self.make_unowned_slice(right, current_slot, field_slot_count);
                self.push_structured_comparison(field_left, op, field_right, field_type)?;
                current_slot += field_slot_count;
            }

            self.fold_comparison_op(
                op,
                i32::try_from(field_types.len()).expect("field count fits i32"),
            );
            return Ok(());
        }

        if is_array {
            let indexed_type = self.ctx.pool.ty(ty).component_type().id();
            if self.ctx.pool.ty(indexed_type).number_kind() == NumberKind::Nonnumeric {
                // Compare every element in the array.
                let indexed_slot_count = self.type_slots(indexed_type);
                let columns = self.ctx.pool.ty(ty).columns();
                let mut current_slot = 0;
                for _ in 0..columns {
                    let indexed_left =
                        self.make_unowned_slice(left, current_slot, indexed_slot_count);
                    let indexed_right =
                        self.make_unowned_slice(right, current_slot, indexed_slot_count);
                    self.push_structured_comparison(indexed_left, op, indexed_right, indexed_type)?;
                    current_slot += indexed_slot_count;
                }

                self.fold_comparison_op(op, columns);
                return Ok(());
            }
        }

        // We've winnowed down to a single element, or an array of homogeneous numeric elements.
        // Push the elements onto the stack, then compare them.
        self.push_lvalue(left)?;
        self.push_lvalue(right)?;
        match op.kind() {
            OperatorKind::EqEq => self.binary_op(ty, TypedOps::EQUAL)?,
            OperatorKind::Neq => self.binary_op(ty, TypedOps::NOT_EQUAL)?,
            _ => debug_assert!(false, "comparison only allows == and !="),
        }

        let slots = self.type_slots(ty);
        self.fold_comparison_op(op, slots);
        Ok(())
    }

    /// `pushBinaryExpression(left, op, right)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2443-L2724 (chrome/m156)
    pub(super) fn push_binary_expression(
        &mut self,
        left: ExprId,
        mut op: Operator,
        right: ExprId,
    ) -> Res {
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);

        match op.kind() {
            // Rewrite greater-than ops as their less-than equivalents.
            OperatorKind::Gt => {
                return self.push_binary_expression(right, OperatorKind::Lt.into(), left);
            }
            OperatorKind::GtEq => {
                return self.push_binary_expression(right, OperatorKind::LtEq.into(), left);
            }

            // Handle struct and array comparisons, then (falling through) rewrite commutative ops
            // so that the literal is on the right-hand side. This gives the Builder more
            // opportunities to use immediate-mode ops.
            OperatorKind::EqEq
            | OperatorKind::Neq
            | OperatorKind::Plus
            | OperatorKind::Star
            | OperatorKind::BitwiseAnd
            | OperatorKind::BitwiseXor
            | OperatorKind::LogicalXor => {
                if matches!(op.kind(), OperatorKind::EqEq | OperatorKind::Neq) {
                    let (is_struct, is_array) = {
                        let t = self.ctx.pool.ty(left_type);
                        (t.is_struct(), t.is_array())
                    };
                    if is_struct || is_array {
                        debug_assert!(self.ctx.pool.ty(left_type).matches(right_type));
                        let lv_left = self.make_lvalue(left, true);
                        let lv_right = self.make_lvalue(right, true);
                        let (Some(lv_left), Some(lv_right)) = (lv_left, lv_right) else {
                            return unsupported();
                        };
                        let result =
                            self.push_structured_comparison(lv_left, op, lv_right, left_type);
                        // The locals are destroyed in reverse order.
                        self.free_lvalue(lv_right);
                        self.free_lvalue(lv_left);
                        return result;
                    }
                }
                if constant_folder::get_constant_value(&self.ctx.pool, left).is_some()
                    && constant_folder::get_constant_value(&self.ctx.pool, right).is_none()
                {
                    return self.push_binary_expression(right, op, left);
                }
            }

            // Emit comma expressions.
            OperatorKind::Comma => {
                if analysis::has_side_effects(&self.ctx.pool, left) {
                    self.push_expression(left, false)?;
                    let slots = self.expr_slots(left);
                    self.discard_expression(slots);
                }
                return self.push_expression(right, true);
            }

            _ => {}
        }

        // Handle binary expressions with mismatched types.
        let mut vectorize_left = false;
        let mut vectorize_right = false;
        if !self.ctx.pool.ty(left_type).matches(right_type) {
            if self.component_number_kind(left_type) != self.component_number_kind(right_type) {
                return unsupported();
            }
            let (left_scalar, left_vector, left_matrix) = {
                let t = self.ctx.pool.ty(left_type);
                (t.is_scalar(), t.is_vector(), t.is_matrix())
            };
            let (right_scalar, right_vector, right_matrix) = {
                let t = self.ctx.pool.ty(right_type);
                (t.is_scalar(), t.is_vector(), t.is_matrix())
            };
            if left_scalar && (right_vector || right_matrix) {
                vectorize_left = true;
            } else if (left_vector || left_matrix) && right_scalar {
                vectorize_right = true;
            }
        }

        let ty = if vectorize_left {
            right_type
        } else {
            left_type
        };

        // If this is an assignment...
        let mut lvalue: Option<LvId> = None;
        if op.is_assignment() {
            // ... turn the left side into an lvalue.
            lvalue = self.make_lvalue(left, false);
            let Some(lv) = lvalue else {
                return unsupported();
            };

            // Handle simple assignment (`var = expr`).
            if op.kind() == OperatorKind::Eq {
                let result = match self.push_expression(right, true) {
                    Ok(()) => self.store_lvalue(lv),
                    Err(e) => Err(e),
                };
                self.free_lvalue(lv);
                return result;
            }

            // Strip off the assignment from the op (turning += into +).
            op = op.remove_assignment();
        }

        let result = self.push_binary_expression_rest(
            lvalue,
            left,
            op,
            right,
            ty,
            vectorize_left,
            vectorize_right,
        );
        // `lvalue` is destroyed when the function returns.
        if let Some(lv) = lvalue {
            self.free_lvalue(lv);
        }
        result
    }

    /// The rest of `pushBinaryExpression(left, op, right)`, after the assignment is turned into
    /// an lvalue: the part during which `lvalue` is alive.
    #[allow(clippy::too_many_arguments)] // Shares the locals of the C++ function.
    fn push_binary_expression_rest(
        &mut self,
        lvalue: Option<LvId>,
        left: ExprId,
        op: Operator,
        right: ExprId,
        ty: TypeId,
        vectorize_left: bool,
        vectorize_right: bool,
    ) -> Res {
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);

        // Handle matrix multiplication (MxM/MxV/VxM).
        if op.kind() == OperatorKind::Star {
            let (left_matrix, left_vector) = {
                let t = self.ctx.pool.ty(left_type);
                (t.is_matrix(), t.is_vector())
            };
            let (right_matrix, right_vector) = {
                let t = self.ctx.pool.ty(right_type);
                (t.is_matrix(), t.is_vector())
            };
            let (left_columns, left_rows) = {
                let t = self.ctx.pool.ty(left_type);
                (t.columns(), t.rows())
            };
            let (right_columns, right_rows) = {
                let t = self.ctx.pool.ty(right_type);
                (t.columns(), t.rows())
            };
            // Matrix * matrix:
            if left_matrix && right_matrix {
                return self.push_matrix_multiply(
                    lvalue,
                    left,
                    right,
                    left_columns,
                    left_rows,
                    right_columns,
                    right_rows,
                );
            }

            // Vector * matrix:
            if left_vector && right_matrix {
                return self.push_matrix_multiply(
                    lvalue,
                    left,
                    right,
                    left_columns,
                    1,
                    right_columns,
                    right_rows,
                );
            }

            // Matrix * vector:
            if left_matrix && right_vector {
                return self.push_matrix_multiply(
                    lvalue,
                    left,
                    right,
                    left_columns,
                    left_rows,
                    1,
                    right_columns,
                );
            }
        }

        if !vectorize_left && !vectorize_right && !self.ctx.pool.ty(ty).matches(right_type) {
            // We have mismatched types but don't know how to handle them.
            return unsupported();
        }

        // Handle binary ops which require short-circuiting.
        match op.kind() {
            OperatorKind::LogicalAnd => {
                if analysis::has_side_effects(&self.ctx.pool, right) {
                    // If the RHS has side effects, we rewrite `a && b` as `a ? b : false`. This
                    // generates pretty solid code and gives us the required short-circuit
                    // behavior.
                    debug_assert!(!op.is_assignment());
                    debug_assert!(self.ctx.pool.ty(ty).component_type().is_boolean());
                    debug_assert_eq!(self.type_slots(ty), 1);
                    let false_literal = self.make_literal(Position::default(), 0.0, right_type);
                    return self.push_ternary_expression(left, right, false_literal);
                }
            }

            OperatorKind::LogicalOr => {
                if analysis::has_side_effects(&self.ctx.pool, right) {
                    // If the RHS has side effects, we rewrite `a || b` as `a ? true : b`.
                    debug_assert!(!op.is_assignment());
                    debug_assert!(self.ctx.pool.ty(ty).component_type().is_boolean());
                    debug_assert_eq!(self.type_slots(ty), 1);
                    let true_literal = self.make_literal(Position::default(), 1.0, right_type);
                    return self.push_ternary_expression(left, true_literal, right);
                }
            }

            _ => {}
        }

        // Push the left- and right-expressions onto the stack.
        self.push_lvalue_or_expression(lvalue, left)?;
        if vectorize_left {
            let slots = self.type_slots(right_type);
            self.builder.push_duplicates(slots - 1);
        }
        self.push_expression(right, true)?;
        if vectorize_right {
            let slots = self.type_slots(left_type);
            self.builder.push_duplicates(slots - 1);
        }

        match op.kind() {
            OperatorKind::Plus => self.binary_op(ty, TypedOps::ADD)?,

            OperatorKind::Minus => self.binary_op(ty, TypedOps::SUBTRACT)?,

            OperatorKind::Star => self.binary_op(ty, TypedOps::MULTIPLY)?,

            OperatorKind::Slash => self.binary_op(ty, TypedOps::DIVIDE)?,

            OperatorKind::Lt | OperatorKind::Gt => {
                self.binary_op(ty, TypedOps::LESS_THAN)?;
                debug_assert_eq!(self.type_slots(ty), 1); // operator< only works with scalar types
            }

            OperatorKind::LtEq | OperatorKind::GtEq => {
                self.binary_op(ty, TypedOps::LESS_THAN_EQUAL)?;
                debug_assert_eq!(self.type_slots(ty), 1); // operator<= only works with scalar types
            }

            OperatorKind::EqEq => {
                self.binary_op(ty, TypedOps::EQUAL)?;
                let slots = self.type_slots(ty);
                self.fold_comparison_op(op, slots);
            }

            OperatorKind::Neq => {
                self.binary_op(ty, TypedOps::NOT_EQUAL)?;
                let slots = self.type_slots(ty);
                self.fold_comparison_op(op, slots);
            }

            OperatorKind::LogicalAnd | OperatorKind::BitwiseAnd => {
                // For logical-and, we verified above that the RHS does not have side effects, so
                // we don't need to worry about short-circuiting side effects.
                let slots = self.type_slots(ty);
                self.builder.binary_op(BuilderOp::BitwiseAndNInts, slots);
            }

            OperatorKind::LogicalOr | OperatorKind::BitwiseOr => {
                // For logical-or, we verified above that the RHS does not have side effects.
                let slots = self.type_slots(ty);
                self.builder.binary_op(BuilderOp::BitwiseOrNInts, slots);
            }

            OperatorKind::LogicalXor | OperatorKind::BitwiseXor => {
                // Logical-xor does not short circuit.
                let slots = self.type_slots(ty);
                self.builder.binary_op(BuilderOp::BitwiseXorNInts, slots);
            }

            _ => return unsupported(),
        }

        // If we have an lvalue, we need to write the result back into it.
        match lvalue {
            Some(lvalue) => self.store_lvalue(lvalue),
            None => Ok(()),
        }
    }

    /// `pushConstructorCompound` (also for array and struct constructors and array casts).
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2776-L2787 (chrome/m156)
    fn push_constructor_compound(&mut self, c: ExprId) -> Res {
        let ty = self.expr_type(c);
        if self.type_slots(ty) > 1 && self.push_immutable_data(c) {
            return Ok(());
        }
        let arguments: Vec<ExprId> = self
            .ctx
            .pool
            .expression(c)
            .any_constructor_arguments()
            .expect("an AnyConstructor")
            .to_vec();
        for arg in arguments {
            self.push_expression(arg, true)?;
        }
        Ok(())
    }

    /// `pushChildCall`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2789-L2857 (chrome/m156)
    fn push_child_call(&mut self, c: &ChildCall) -> Res {
        // Save this in case pushExpression changes fChildEffectMap.
        let Some(&child_idx) = self.child_effect_map.get(&c.child) else {
            debug_assert!(false, "a child call refers to a child effect");
            return unsupported();
        };

        // All child calls have at least one argument.
        let arg = c.arguments[0];
        self.push_expression(arg, true)?;

        // Copy arguments from the stack into src/dst as required by this particular child-call.
        let child_type = self.ctx.pool.variable(c.child).ty;
        match self.ctx.pool.ty(child_type).type_kind {
            TypeKind::Shader => {
                // The argument must be a float2.
                debug_assert_eq!(c.arguments.len(), 1);
                debug_assert!(
                    self.ctx
                        .pool
                        .ty(self.expr_type(arg))
                        .matches(TypeId::FLOAT2)
                );

                // `exchange_src` will use the top four values on the stack, but we don't care
                // what goes into the blue/alpha components. We inject padding here to balance
                // the stack.
                self.builder.pad_stack(2);

                // Move the argument into src.rgba while also preserving the execution mask.
                self.builder.exchange_src();
                self.builder.invoke_shader(child_idx);
            }
            TypeKind::ColorFilter => {
                // The argument must be a half4/float4.
                debug_assert_eq!(c.arguments.len(), 1);

                // Move the argument into src.rgba while also preserving the execution mask.
                self.builder.exchange_src();
                self.builder.invoke_color_filter(child_idx);
            }
            TypeKind::Blender => {
                // Both arguments must be half4/float4.
                debug_assert_eq!(c.arguments.len(), 2);

                // Move the second argument into dst.rgba, and the first argument into src.rgba,
                // while simultaneously preserving the execution mask.
                self.push_expression(c.arguments[1], true)?;
                self.builder.pop_dst_rgba();
                self.builder.exchange_src();
                self.builder.invoke_blender(child_idx);
            }
            _ => {
                debug_assert!(false, "cannot sample from this type");
                return unsupported();
            }
        }

        // The child call has returned the result color via src.rgba, and the SkRP execution mask
        // is on top of the stack. Swapping the two puts the result color on top of the stack, and
        // also restores our execution masks.
        self.builder.exchange_src();
        Ok(())
    }

    /// `pushConstructorCast`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2859-L2958 (chrome/m156)
    fn push_constructor_cast(&mut self, c: ExprId, inner: ExprId) -> Res {
        let c_type = self.expr_type(c);
        let inner_type = self.expr_type(inner);
        debug_assert_eq!(self.type_slots(inner_type), self.type_slots(c_type));

        self.push_expression(inner, true)?;
        let inner_kind = self.component_number_kind(inner_type);
        let outer_kind = self.component_number_kind(c_type);

        if inner_kind == outer_kind {
            // Since we ignore type precision, this cast is effectively a no-op.
            return Ok(());
        }

        let slots = self.type_slots(c_type);
        match inner_kind {
            NumberKind::Signed => {
                if outer_kind == NumberKind::Unsigned {
                    // Treat uint(int) as a no-op.
                    return Ok(());
                }
                if outer_kind == NumberKind::Float {
                    self.builder.unary_op(BuilderOp::CastToFloatFromInt, slots);
                    return Ok(());
                }
            }

            NumberKind::Unsigned => {
                if outer_kind == NumberKind::Signed {
                    // Treat int(uint) as a no-op.
                    return Ok(());
                }
                if outer_kind == NumberKind::Float {
                    self.builder.unary_op(BuilderOp::CastToFloatFromUint, slots);
                    return Ok(());
                }
            }

            NumberKind::Boolean => {
                // Converting boolean to int or float can be accomplished via bitwise-and.
                if outer_kind == NumberKind::Float {
                    self.builder.push_constant_f(1.0);
                } else if outer_kind == NumberKind::Signed || outer_kind == NumberKind::Unsigned {
                    self.builder.push_constant_i(1, 1);
                } else {
                    debug_assert!(false, "unexpected cast from bool");
                    return unsupported();
                }
                self.builder.push_duplicates(slots - 1);
                self.builder.binary_op(BuilderOp::BitwiseAndNInts, slots);
                return Ok(());
            }

            NumberKind::Float => {
                if outer_kind == NumberKind::Signed {
                    self.builder.unary_op(BuilderOp::CastToIntFromFloat, slots);
                    return Ok(());
                }
                if outer_kind == NumberKind::Unsigned {
                    self.builder.unary_op(BuilderOp::CastToUintFromFloat, slots);
                    return Ok(());
                }
            }

            NumberKind::Nonnumeric => {}
        }

        if outer_kind == NumberKind::Boolean {
            // Converting int or float to boolean can be accomplished via `notEqual(x, 0)`.
            self.builder.push_zeros(slots);
            return self.binary_op(inner_type, TypedOps::NOT_EQUAL);
        }

        debug_assert!(false, "unexpected cast");
        unsupported()
    }

    /// `pushConstructorDiagonalMatrix`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2960-L2970 (chrome/m156)
    fn push_constructor_diagonal_matrix(&mut self, c: ExprId, argument: ExprId) -> Res {
        if self.push_immutable_data(c) {
            return Ok(());
        }
        self.builder.push_zeros(1);
        self.push_expression(argument, true)?;
        let (columns, rows) = {
            let t = self.ctx.pool.ty(self.expr_type(c));
            (t.columns(), t.rows())
        };
        self.builder.diagonal_matrix(columns, rows);
        Ok(())
    }

    /// `pushConstructorMatrixResize`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2972-L2981 (chrome/m156)
    fn push_constructor_matrix_resize(&mut self, c: ExprId, argument: ExprId) -> Res {
        self.push_expression(argument, true)?;
        let (orig_columns, orig_rows) = {
            let t = self.ctx.pool.ty(self.expr_type(argument));
            (t.columns(), t.rows())
        };
        let (columns, rows) = {
            let t = self.ctx.pool.ty(self.expr_type(c));
            (t.columns(), t.rows())
        };
        self.builder
            .matrix_resize(orig_columns, orig_rows, columns, rows);
        Ok(())
    }

    /// `pushConstructorSplat`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2983-L2989 (chrome/m156)
    fn push_constructor_splat(&mut self, c: ExprId, argument: ExprId) -> Res {
        self.push_expression(argument, true)?;
        let slots = self.expr_slots(c);
        self.builder.push_duplicates(slots - 1);
        Ok(())
    }

    /// `pushFieldAccess`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2991-L2995 (chrome/m156)
    fn push_field_access(&mut self, f: ExprId) -> Res {
        // If possible, get direct field access via the lvalue.
        let Some(lvalue) = self.make_lvalue(f, true) else {
            return unsupported();
        };
        let result = self.push_lvalue(lvalue);
        self.free_lvalue(lvalue);
        result
    }

    /// `pushFunctionCall`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2997-L3032 (chrome/m156)
    fn push_function_call(&mut self, e: ExprId, c: &FunctionCall) -> Res {
        if self.ctx.pool.function(c.function).is_intrinsic() {
            return self.push_intrinsic_call(c);
        }

        // Keep track of the current function.
        let last_function = self.current_function;
        let callee = self
            .ctx
            .pool
            .function(c.function)
            .definition
            .expect("a called function has a definition");
        self.current_function = Some(callee);

        // Skip over the function body entirely if there are no active lanes. (If the function
        // call was trivial, it would likely have been inlined in the frontend, so we assume here
        // that function calls generally represent a significant amount of work.)
        let skip_label_id = self.builder.next_label_id();
        self.builder.branch_if_no_lanes_active(skip_label_id);

        // Emit the function body.
        let r = self.write_function(super::SlotKey::CallExpr(e), callee, &c.arguments)?;

        // If the function uses result slots, move its result from slots onto the stack.
        if self.needs_function_result_slots(callee) {
            self.builder.push_slots(r);
        }

        // We've returned back to the last function.
        self.current_function = last_function;

        // Copy the function result from its slots onto the stack.
        self.builder.label(skip_label_id);
        Ok(())
    }

    /// `pushIndexExpression`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3034-L3038 (chrome/m156)
    fn push_index_expression(&mut self, i: ExprId) -> Res {
        let Some(lvalue) = self.make_lvalue(i, true) else {
            return unsupported();
        };
        let result = self.push_lvalue(lvalue);
        self.free_lvalue(lvalue);
        result
    }

    /// `pushVectorizedExpression`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3085-L3095 (chrome/m156)
    pub(super) fn push_vectorized_expression(&mut self, expr: ExprId, vector_type: TypeId) -> Res {
        self.push_expression(expr, true)?;
        let vector_slots = self.type_slots(vector_type);
        let expr_slots = self.expr_slots(expr);
        if vector_slots > expr_slots {
            debug_assert_eq!(expr_slots, 1);
            self.builder.push_duplicates(vector_slots - expr_slots);
        }
        Ok(())
    }

    /// `pushLiteral`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3500-L3524 (chrome/m156)
    fn push_literal(&mut self, e: ExprId, l: Literal) -> Res {
        let ty = self.expr_type(e);
        match self.ctx.pool.ty(ty).number_kind() {
            NumberKind::Float => {
                self.builder.push_constant_f(l.float_value());
                Ok(())
            }
            // The `SKSL_INT` value narrows to the 32-bit op argument.
            #[allow(clippy::cast_possible_truncation)]
            NumberKind::Signed => {
                self.builder.push_constant_i(l.int_value() as i32, 1);
                Ok(())
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            NumberKind::Unsigned => {
                self.builder.push_constant_u(l.int_value() as u32, 1);
                Ok(())
            }
            NumberKind::Boolean => {
                self.builder
                    .push_constant_i(if l.bool_value() { !0 } else { 0 }, 1);
                Ok(())
            }
            NumberKind::Nonnumeric => unreachable!("a literal has a numeric type"),
        }
    }

    /// `pushPostfixExpression`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3526-L3578 (chrome/m156)
    fn push_postfix_expression(
        &mut self,
        e: ExprId,
        p: &PostfixExpression,
        uses_result: bool,
    ) -> Res {
        // If the result is ignored...
        if !uses_result {
            // ... just emit a prefix expression instead.
            return self.push_prefix_expression(p.operator, p.operand);
        }
        // Get the operand as an lvalue, and push it onto the stack as-is.
        let Some(lvalue) = self.make_lvalue(p.operand, false) else {
            return unsupported();
        };
        let result = self.push_postfix_expression_rest(e, p, lvalue);
        // `lvalue` is destroyed when the function returns.
        self.free_lvalue(lvalue);
        result
    }

    /// The part of `pushPostfixExpression` during which the operand's lvalue is alive.
    fn push_postfix_expression_rest(
        &mut self,
        e: ExprId,
        p: &PostfixExpression,
        lvalue: LvId,
    ) -> Res {
        self.push_lvalue(lvalue)?;

        // Push a scratch copy of the operand.
        let ty = self.expr_type(e);
        let slots = self.type_slots(ty);
        self.builder.push_clone(slots, 0);

        // Increment or decrement the scratch copy by one.
        let component_type = self.ctx.pool.ty(ty).component_type().id();
        let one_literal = self.make_literal(Position::default(), 1.0, component_type);
        self.push_vectorized_expression(one_literal, ty)?;

        match p.operator.kind() {
            OperatorKind::PlusPlus => self.binary_op(ty, TypedOps::ADD)?,
            OperatorKind::MinusMinus => self.binary_op(ty, TypedOps::SUBTRACT)?,
            _ => unreachable!("a postfix expression is ++ or --"),
        }

        // Write the new value back to the operand.
        self.store_lvalue(lvalue)?;

        // Discard the scratch copy, leaving only the original value as-is.
        self.discard_expression(slots);
        Ok(())
    }

    /// `pushPrefixExpression(op, expr)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3584-L3636 (chrome/m156)
    pub(super) fn push_prefix_expression(&mut self, op: Operator, expr: ExprId) -> Res {
        let expr_type = self.expr_type(expr);
        match op.kind() {
            OperatorKind::BitwiseNot | OperatorKind::LogicalNot => {
                // Handle operators ! and ~.
                self.push_expression(expr, true)?;
                let slots = self.type_slots(expr_type);
                self.builder.push_constant_u(!0, slots);
                self.builder.binary_op(BuilderOp::BitwiseXorNInts, slots);
                Ok(())
            }

            OperatorKind::Minus => {
                self.push_expression(expr, true)?;
                let slots = self.type_slots(expr_type);
                if self.ctx.pool.ty(expr_type).component_type().is_float() {
                    // Handle float negation as an integer `x ^ 0x80000000`. This toggles the
                    // sign bit.
                    self.builder.push_constant_u(0x8000_0000, slots);
                    self.builder.binary_op(BuilderOp::BitwiseXorNInts, slots);
                } else {
                    // Handle integer negation as a componentwise `expr * -1`.
                    self.builder.push_constant_i(-1, slots);
                    self.builder.binary_op(BuilderOp::MulNInts, slots);
                }
                Ok(())
            }

            OperatorKind::PlusPlus => {
                // Rewrite as `expr += 1`.
                let component_type = self.ctx.pool.ty(expr_type).component_type().id();
                let one_literal = self.make_literal(Position::default(), 1.0, component_type);
                self.push_binary_expression(expr, OperatorKind::PlusEq.into(), one_literal)
            }

            OperatorKind::MinusMinus => {
                // Rewrite as `expr += -1`.
                let component_type = self.ctx.pool.ty(expr_type).component_type().id();
                let position = self.ctx.pool.expression(expr).position;
                let minus_one_literal = self.make_literal(position, -1.0, component_type);
                self.push_binary_expression(expr, OperatorKind::PlusEq.into(), minus_one_literal)
            }

            _ => unsupported(),
        }
    }

    /// `pushSwizzle`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3638-L3669 (chrome/m156)
    fn push_swizzle(&mut self, s: &Swizzle) -> Res {
        debug_assert!(!s.components.is_empty() && s.components.len() <= 4);

        // If this is a simple subset of a variable's slots...
        let is_simple_subset = super::lvalue::is_sliceable_swizzle(&s.components);
        if is_simple_subset {
            if let ExpressionKind::VariableReference(var_ref) =
                self.ctx.pool.expression(s.base).kind.clone()
            {
                // ... we can just push part of the variable directly onto the stack, rather than
                // pushing the whole expression and then immediately cutting it down. (Either way
                // works, but this saves a step.)
                return self.push_variable_reference_partial(
                    &var_ref,
                    SlotRange {
                        index: i32::from(s.components[0]),
                        count: i32::try_from(s.components.len()).expect("at most 4"),
                    },
                );
            }
        }
        // Push the base expression.
        self.push_expression(s.base, true)?;
        // An identity swizzle doesn't rearrange the data; it just (potentially) discards tail
        // elements.
        let base_slots = self.expr_slots(s.base);
        if is_simple_subset && s.components[0] == 0 {
            let discarded_elements =
                base_slots - i32::try_from(s.components.len()).expect("at most 4");
            debug_assert!(discarded_elements >= 0);
            self.builder.discard_stack(discarded_elements);
            return Ok(());
        }
        // Perform the swizzle.
        self.builder.swizzle(base_slots, &s.components);
        Ok(())
    }

    /// `pushDynamicallyUniformTernaryExpression`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3675-L3723 (chrome/m156)
    fn push_dynamically_uniform_ternary_expression(
        &mut self,
        test: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    ) -> Res {
        debug_assert!(analysis::is_dynamically_uniform_expression(
            &self.ctx.pool,
            test
        ));

        let false_label_id = self.builder.next_label_id();
        let exit_label_id = self.builder.next_label_id();

        // First, push the test-expression into a separate stack.
        let test_stack = self.new_auto_stack();
        self.stack_enter(test_stack);
        self.push_expression(test, true)?;

        // Branch to the true- or false-expression based on the test-expression. We can skip the
        // non-true path entirely since the test is known to be uniform.
        self.builder
            .branch_if_no_active_lanes_on_stack_top_equal(!0, false_label_id);
        self.stack_exit(test_stack);

        self.push_expression(if_true, true)?;

        self.builder.jump(exit_label_id);

        // The builder doesn't understand control flow, and assumes that every push moves the
        // stack-top forwards. We need to manually balance out the `pushExpression` from the
        // if-true path by moving the stack position backwards, so that the if-false path pushes
        // its expression into the same as the if-true result.
        let slots = self.expr_slots(if_true);
        self.discard_expression(slots);

        self.builder.label(false_label_id);

        self.push_expression(if_false, true)?;

        self.builder.label(exit_label_id);

        // Jettison the text-expression from the separate stack.
        self.stack_enter(test_stack);
        self.discard_expression(1);
        self.stack_exit(test_stack);
        self.drop_auto_stack(test_stack);
        Ok(())
    }

    /// `pushTernaryExpression(test, ifTrue, ifFalse)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3725-L3856 (chrome/m156)
    pub(super) fn push_ternary_expression(
        &mut self,
        test: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    ) -> Res {
        // If the test-expression is dynamically-uniform, we can skip over the non-true
        // expressions entirely, and not need to involve the condition mask.
        if analysis::is_dynamically_uniform_expression(&self.ctx.pool, test) {
            return self.push_dynamically_uniform_ternary_expression(test, if_true, if_false);
        }

        // Analyze the ternary to see which corners we can safely cut.
        let if_false_has_side_effects = analysis::has_side_effects(&self.ctx.pool, if_false);
        let if_true_has_side_effects = analysis::has_side_effects(&self.ctx.pool, if_true);
        let if_true_is_trivial = analysis::is_trivial_expression(&self.ctx.pool, if_true);
        let cleanup_label_id = self.builder.next_label_id();
        let true_type = self.expr_type(if_true);

        // If the true- and false-expressions both lack side effects, we evaluate both of them
        // safely without masking off their effects. In that case, we can emit both sides and use
        // boolean mix to select the correct result without using the condition mask at all.
        if !if_false_has_side_effects && !if_true_has_side_effects && if_true_is_trivial {
            // Push all of the arguments to mix.
            self.push_vectorized_expression(test, true_type)?;
            self.push_expression(if_false, true)?;
            self.push_expression(if_true, true)?;
            // Use boolean mix to select the true- or false-expression via the test-expression.
            let slots = self.type_slots(true_type);
            self.builder.ternary_op(BuilderOp::MixNInts, slots);
            return Ok(());
        }

        // First, push the current condition-mask and the test-expression into a separate stack.
        self.builder.enable_execution_mask_writes();
        let test_stack = self.new_auto_stack();
        self.stack_enter(test_stack);
        self.builder.push_condition_mask();
        self.push_expression(test, true)?;
        self.stack_exit(test_stack);

        // We can take some shortcuts with condition-mask handling if the false-expression is
        // entirely side-effect free. (We can evaluate it without masking off its effects.) We
        // always handle the condition mask properly for the test-expression and true-expression
        // properly.
        let slots = self.type_slots(true_type);
        if if_false_has_side_effects {
            // Merge the condition mask (on the separate stack) with the test expression.
            self.stack_enter(test_stack);
            self.builder.merge_condition_mask();
            self.stack_exit(test_stack);

            // Push the true-expression onto the primary stack.
            self.push_expression(if_true, true)?;

            // Switch back to the test-expression stack and apply the inverted test condition.
            self.stack_enter(test_stack);
            self.builder.merge_inv_condition_mask();
            self.stack_exit(test_stack);

            // Push the false-expression onto the primary stack, immediately after the
            // true-expression.
            self.push_expression(if_false, true)?;

            // Use a select to conditionally mask-merge the true-expression and false-expression
            // lanes; the mask is already set up for this.
            self.builder.select(slots);
        } else {
            // Push the false-expression onto the primary stack.
            self.push_expression(if_false, true)?;

            // Next, merge the condition mask (on the separate stack) with the test expression.
            self.stack_enter(test_stack);
            self.builder.merge_condition_mask();
            self.stack_exit(test_stack);

            // If no lanes are active, we can skip the true-expression entirely. This isn't super
            // likely to happen, so it's probably only a win for non-trivial true-expressions.
            if !if_true_is_trivial {
                self.builder.branch_if_no_lanes_active(cleanup_label_id);
            }

            // Push the true-expression onto the primary stack, immediately after the
            // false-expression.
            self.push_expression(if_true, true)?;

            // Use a select to conditionally mask-merge the true-expression and false-expression
            // lanes.
            self.builder.select(slots);
            self.builder.label(cleanup_label_id);
        }

        // Restore the condition-mask to its original state and jettison the test-expression.
        self.stack_enter(test_stack);
        self.discard_expression(1);
        self.builder.pop_condition_mask();
        self.stack_exit(test_stack);

        self.builder.disable_execution_mask_writes();
        self.drop_auto_stack(test_stack);
        Ok(())
    }

    /// `pushVariableReference`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3858-L3874 (chrome/m156)
    fn push_variable_reference(&mut self, e: ExprId, var: &VariableReference) -> Res {
        // If we are pushing a constant-value variable, push the value directly; literal values
        // are more amenable to optimization.
        let ty = self.expr_type(e);
        let (is_scalar, is_vector) = {
            let t = self.ctx.pool.ty(ty);
            (t.is_scalar(), t.is_vector())
        };
        if is_scalar || is_vector {
            if let Some(expr) = constant_folder::get_constant_value_or_null(&self.ctx.pool, e) {
                return self.push_expression(expr, true);
            }
            if self.immutable_variables.contains(&var.variable) {
                let initial_value = self
                    .ctx
                    .pool
                    .variable(var.variable)
                    .initial_value(&self.ctx.pool)
                    .expect("an immutable variable has an initial value");
                return self.push_expression(initial_value, true);
            }
        }
        let slots = self.type_slots(ty);
        self.push_variable_reference_partial(
            var,
            SlotRange {
                index: 0,
                count: slots,
            },
        )
    }

    /// `pushVariableReferencePartial`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3876-L3921 (chrome/m156)
    fn push_variable_reference_partial(&mut self, v: &VariableReference, subset: SlotRange) -> Res {
        let var = v.variable;
        let var_slots = self.type_slots(self.ctx.pool.variable(var).ty);
        let mut r;
        if self.is_uniform(var) {
            // Push a uniform.
            r = self.get_uniform_slots(var);
            debug_assert_eq!(r.count, var_slots);
            r.index += subset.index;
            r.count = subset.count;
            self.builder.push_uniform(r);
        } else if self.immutable_variables.contains(&var) {
            // If we only need a single slot, we can push a constant. This saves a lookup, and can
            // occasionally permit the use of an immediate-mode op.
            if subset.count == 1 {
                let expr = self
                    .ctx
                    .pool
                    .variable(var)
                    .initial_value(&self.ctx.pool)
                    .expect("an immutable variable has an initial value");
                let bits = self.get_immutable_bits_for_slot(
                    expr,
                    usize::try_from(subset.index).expect("slot index is non-negative"),
                );
                if let Some(bits) = bits {
                    self.builder.push_constant_i(bits, 1);
                    return Ok(());
                }
            }
            // Push the immutable slot range.
            r = self.get_immutable_slots(var);
            debug_assert_eq!(r.count, var_slots);
            r.index += subset.index;
            r.count = subset.count;
            self.builder.push_immutable(r);
        } else {
            // Push the variable.
            r = self.get_variable_slots(var);
            debug_assert_eq!(r.count, var_slots);
            r.index += subset.index;
            r.count = subset.count;
            self.builder.push_slots(r);
        }
        Ok(())
    }
}
