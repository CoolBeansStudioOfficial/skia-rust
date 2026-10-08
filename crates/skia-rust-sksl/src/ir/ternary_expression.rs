// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLTernaryExpression.{h,cpp} (data, `description`, `Convert`
// and `Make`).

//! [`TernaryExpression`]: `test ? ifTrue : ifFalse`.

use super::{ExprId, Expression, ExpressionKind, IrPool, TypeId, constant_folder_stub, s7b_shims};
use crate::context::Context;
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::position::Position;

/// `SkSL::TernaryExpression`. Its type is `ifTrue`'s.
// Port of: src/sksl/ir/SkSLTernaryExpression.h#L24-L102 (chrome/m156)
#[doc(alias = "SkSL::TernaryExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct TernaryExpression {
    /// `test()`.
    pub test: ExprId,
    /// `ifTrue()`.
    pub if_true: ExprId,
    /// `ifFalse()`.
    pub if_false: ExprId,
}

impl TernaryExpression {
    /// `Convert(context, pos, test, ifTrue, ifFalse)`: checks the operands and their types, and
    /// builds the expression. Reports errors and returns `None` on failure.
    // Port of: src/sksl/ir/SkSLTernaryExpression.cpp#L24-L70 (chrome/m156)
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    ) -> Option<ExprId> {
        let test = s7b_shims::coerce_expression(ctx, TypeId::BOOL, test)?;

        let true_ty = ctx.pool.expression(if_true).ty;
        let false_ty = ctx.pool.expression(if_false).ty;
        if ctx.pool.ty(true_ty).component_type().is_opaque() {
            let msg = format!(
                "ternary expression of opaque type '{}' is not allowed",
                ctx.pool.ty(true_ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        let equality = Operator::from(OperatorKind::EqEq);
        let types = equality.determine_binary_type(ctx, true_ty, false_ty);
        let matched = types.is_some_and(|t| ctx.pool.ty(t.left).matches(t.right));
        let Some(types) = types.filter(|_| matched) else {
            let error_pos = {
                let (a, b) = (
                    ctx.pool.expression(if_true).position,
                    ctx.pool.expression(if_false).position,
                );
                a.range_through(b)
            };
            if ctx.pool.ty(true_ty).is_void() {
                ctx.errors.error(
                    error_pos,
                    "ternary expression of type 'void' is not allowed",
                );
            } else {
                let msg = format!(
                    "ternary operator result mismatch: '{}', '{}'",
                    ctx.pool.ty(true_ty).display_name(),
                    ctx.pool.ty(false_ty).display_name()
                );
                ctx.errors.error(error_pos, &msg);
            }
            return None;
        };

        if ctx.pool.ty(types.left).is_or_contains_array() {
            ctx.errors.error(
                pos,
                "ternary operator result may not be an array (or struct containing an array)",
            );
            return None;
        }

        let if_true = s7b_shims::coerce_expression(ctx, types.left, if_true)?;
        let if_false = s7b_shims::coerce_expression(ctx, types.right, if_false)?;

        Some(Self::make(ctx, pos, test, if_true, if_false))
    }

    /// `Make(context, pos, test, ifTrue, ifFalse)`: builds the expression, simplifying it when
    /// possible. The branches must already have the same type.
    ///
    /// Not ported: the `fOptimize` rewrites (`test ? x : x`, `test ? x : false`, `test ? true : x`,
    /// `test ? false : true`, `test ? 1 : 0`). They need `Analysis::IsSameExpressionTree` and
    /// `HasSideEffects` (S9a), and the constant folder (S8). The tree is therefore not reduced
    /// where Skia would reduce it.
    // Port of: src/sksl/ir/SkSLTernaryExpression.cpp#L72-L137 (chrome/m156), static-test part
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    ) -> ExprId {
        let test_expr = constant_folder_stub::get_constant_value_for_variable(&ctx.pool, test);
        let static_bool = ctx
            .pool
            .expression(test_expr)
            .as_literal()
            .filter(|_| ctx.pool.expression(test_expr).is_bool_literal(&ctx.pool))
            .map(|lit| lit.bool_value());
        if let Some(value) = static_bool {
            // A static boolean test: just return one of the branches.
            let chosen = if value { if_true } else { if_false };
            ctx.pool.expression_mut(chosen).position = pos;
            return chosen;
        }

        let ty = ctx.pool.expression(if_true).ty;
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Ternary(Self {
                test,
                if_true,
                if_false,
            }),
        ))
    }

    /// `description(parentPrecedence)`.
    // Port of: src/sksl/ir/SkSLTernaryExpression.cpp#L139-L146 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool, parent_precedence: OperatorPrecedence) -> String {
        let needs_parens = OperatorPrecedence::Ternary >= parent_precedence;
        format!(
            "{}{} ? {} : {}{}",
            if needs_parens { "(" } else { "" },
            pool.expression_description_with(self.test, OperatorPrecedence::Ternary),
            pool.expression_description_with(self.if_true, OperatorPrecedence::Ternary),
            pool.expression_description_with(self.if_false, OperatorPrecedence::Ternary),
            if needs_parens { ")" } else { "" },
        )
    }
}
