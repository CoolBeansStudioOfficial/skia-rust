// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBinaryExpression.{h,cpp} (data, `description`, `Convert`,
// `Make`, `CheckRef` and `isAssignmentIntoVariable`).

//! [`BinaryExpression`]: `left op right`.

use super::{ExprId, Expression, ExpressionKind, IrPool, TypeId, VariableRefKind};
use crate::analysis::{self, AssignmentInfo};
use crate::constant_folder;
use crate::context::Context;
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::position::Position;

/// `SkSL::BinaryExpression`.
// Port of: src/sksl/ir/SkSLBinaryExpression.h#L30-L115 (chrome/m156)
#[doc(alias = "SkSL::BinaryExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryExpression {
    /// `left()`.
    pub left: ExprId,
    /// `getOperator()`.
    pub operator: Operator,
    /// `right()`.
    pub right: ExprId,
}

impl BinaryExpression {
    /// `Convert(context, pos, left, op, right)`: checks the operands of `op`, coerces them to the
    /// operand types of `op`, and builds the expression. Reports errors and returns `None` on
    /// failure.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L24-L95 (chrome/m156)
    #[allow(clippy::too_many_lines)] // Skia's `Convert` is one function, and its checks run in order.
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        left: ExprId,
        op: Operator,
        right: ExprId,
    ) -> Option<ExprId> {
        let (left_ty, left_is_int) = {
            let e = ctx.pool.expression(left);
            (e.ty, e.is_int_literal(&ctx.pool))
        };
        let (right_ty, right_is_int) = {
            let e = ctx.pool.expression(right);
            (e.ty, e.is_int_literal(&ctx.pool))
        };
        // An integer literal takes the type of an integer operand on the other side.
        let raw_left_type = if left_is_int && ctx.pool.ty(right_ty).is_integer() {
            right_ty
        } else {
            left_ty
        };
        let raw_right_type = if right_is_int && ctx.pool.ty(left_ty).is_integer() {
            left_ty
        } else {
            right_ty
        };

        let is_assignment = op.is_assignment();
        if is_assignment {
            let kind = if op.kind() == OperatorKind::Eq {
                VariableRefKind::Write
            } else {
                VariableRefKind::ReadWrite
            };
            if !analysis::update_variable_ref_kind(&mut ctx.pool, left, kind, Some(&mut ctx.errors))
            {
                return None;
            }
        }

        let Some(types) = op.determine_binary_type(ctx, raw_left_type, raw_right_type) else {
            let msg = format!(
                "type mismatch: '{}' cannot operate on '{}', '{}'",
                op.tight_operator_name(),
                ctx.pool.ty(left_ty).display_name(),
                ctx.pool.ty(right_ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        };

        if is_assignment {
            let (opaque, atomic) = {
                let left_type = ctx.pool.ty(types.left);
                (
                    left_type.component_type().is_opaque(),
                    left_type.is_or_contains_atomic(),
                )
            };
            if opaque || atomic {
                let msg = format!(
                    "assignments to opaque type '{}' are not permitted",
                    ctx.pool.ty(left_ty).display_name()
                );
                ctx.errors.error(pos, &msg);
                return None;
            }
        }
        let strict = ctx.config().strict_es2_mode();
        if strict && !op.is_allowed_in_strict_es2_mode() {
            let msg = format!("operator '{}' is not allowed", op.tight_operator_name());
            ctx.errors.error(pos, &msg);
            return None;
        }
        if strict || op.kind() == OperatorKind::Comma {
            // Most operators are already rejected on arrays, but GLSL ES 1.0 is very explicit that
            // the *only* operator allowed on arrays is subscripting (and the rules against
            // assignment, comparison, and even sequence apply to structs containing arrays as
            // well). WebGL2 also restricts the usage of the sequence operator with arrays (section
            // 5.26, "Disallowed variants of GLSL ES 3.00 operators"). Since there is very little
            // practical application for sequenced array expressions, we disallow it in SkSL.
            let array_expr = if ctx.pool.ty(types.left).is_or_contains_array() {
                Some(left)
            } else if ctx.pool.ty(types.right).is_or_contains_array() {
                Some(right)
            } else {
                None
            };
            if let Some(array_expr) = array_expr {
                let array_pos = ctx.pool.expression(array_expr).position;
                let msg = format!(
                    "operator '{}' can not operate on arrays (or structs containing arrays)",
                    op.tight_operator_name()
                );
                ctx.errors.error(array_pos, &msg);
                return None;
            }
        }

        let new_left = types.left.coerce_expression(ctx, left);
        let new_right = types.right.coerce_expression(ctx, right);
        let (new_left, new_right) = (new_left?, new_right?);

        Some(Self::make_with_result_type(
            ctx,
            pos,
            new_left,
            op,
            new_right,
            types.result,
        ))
    }

    /// `Make(context, pos, left, op, right)`: builds the expression, with the result type that
    /// `op` gives the operand types. The operand types must already be valid for `op`.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L97-L110 (chrome/m156)
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        left: ExprId,
        op: Operator,
        right: ExprId,
    ) -> ExprId {
        let (left_ty, right_ty) = (ctx.pool.expression(left).ty, ctx.pool.expression(right).ty);
        let types = op.determine_binary_type(ctx, left_ty, right_ty);
        debug_assert!(
            types.is_some(),
            "BinaryExpression::Make: operands not valid for {op:?}"
        );
        let result = types.map_or(left_ty, |t| t.result);
        Self::make_with_result_type(ctx, pos, left, op, right, result)
    }

    /// `Make(context, pos, left, op, right, resultType)`: checks the invariants Skia asserts, folds
    /// the expression when it can, and otherwise builds the node.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L112-L139 (chrome/m156)
    pub fn make_with_result_type(
        ctx: &mut Context,
        pos: Position,
        left: ExprId,
        op: Operator,
        right: ExprId,
        result_type: TypeId,
    ) -> ExprId {
        // For simple assignments, detect and report out-of-range literal values.
        if op.kind() == OperatorKind::Eq {
            let left_ty = ctx.pool.expression(left).ty;
            left_ty.check_for_out_of_range_literal(ctx, right);
        }

        // Perform constant-folding on the expression.
        if let Some(result) = constant_folder::simplify(ctx, pos, left, op, right, result_type) {
            return result;
        }

        ctx.pool.add_expression(Expression::new(
            pos,
            result_type,
            ExpressionKind::Binary(Self {
                left,
                operator: op,
                right,
            }),
        ))
    }

    /// `CheckRef(expr)`: whether `expr` names something that can be written to through a
    /// `ReadWrite` or `Write` reference.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L141-L164 (chrome/m156)
    #[must_use]
    pub fn check_ref(pool: &IrPool, expr: ExprId) -> bool {
        match &pool.expression(expr).kind {
            ExpressionKind::FieldAccess(f) => Self::check_ref(pool, f.base),
            ExpressionKind::Index(i) => Self::check_ref(pool, i.base),
            ExpressionKind::Swizzle(s) => Self::check_ref(pool, s.base),
            ExpressionKind::Ternary(t) => {
                Self::check_ref(pool, t.if_true) && Self::check_ref(pool, t.if_false)
            }
            ExpressionKind::VariableReference(v) => {
                v.ref_kind == VariableRefKind::Write || v.ref_kind == VariableRefKind::ReadWrite
            }
            _ => false,
        }
    }

    /// `isAssignmentIntoVariable()`: for an assignment whose target is a variable, the
    /// `VariableReference` the assignment writes to.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L184-L194 (chrome/m156)
    #[must_use]
    pub fn is_assignment_into_variable(&self, pool: &IrPool) -> Option<ExprId> {
        if !self.operator.is_assignment() {
            return None;
        }
        let mut info = AssignmentInfo::default();
        if analysis::is_assignable(pool, self.left, Some(&mut info), None) {
            info.assigned_var
        } else {
            None
        }
    }

    /// `description(parentPrecedence)`.
    // Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L174-L182 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool, parent_precedence: OperatorPrecedence) -> String {
        let operator_precedence = self.operator.binary_precedence();
        let needs_parens = operator_precedence >= parent_precedence;
        format!(
            "{}{}{}{}{}",
            if needs_parens { "(" } else { "" },
            pool.expression_description_with(self.left, operator_precedence),
            self.operator.operator_name(),
            pool.expression_description_with(self.right, operator_precedence),
            if needs_parens { ")" } else { "" },
        )
    }
}
