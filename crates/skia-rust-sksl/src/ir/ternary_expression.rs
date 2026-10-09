// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLTernaryExpression.{h,cpp} (data, `description`, `Convert`
// and `Make`).

//! [`TernaryExpression`]: `test ? ifTrue : ifFalse`.

use super::{
    BinaryExpression, ConstructorScalarCast, ExprId, Expression, ExpressionKind, IrPool,
    PrefixExpression, TypeId,
};
use crate::analysis;
use crate::constant_folder;
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

/// `isBoolLiteral()` and `as<Literal>().boolValue()`: the value of a Boolean literal.
fn bool_literal(pool: &IrPool, expr: ExprId) -> Option<bool> {
    let e = pool.expression(expr);
    if e.is_bool_literal(pool) {
        e.as_literal().map(|lit| lit.bool_value())
    } else {
        None
    }
}

/// `is<Literal>()` and `as<Literal>().value()`: the value of a literal.
fn literal_value(pool: &IrPool, expr: ExprId) -> Option<f64> {
    pool.expression(expr).as_literal().map(|lit| lit.value)
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
        let test = TypeId::BOOL.coerce_expression(ctx, test)?;

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

        let if_true = types.left.coerce_expression(ctx, if_true)?;
        let if_false = types.right.coerce_expression(ctx, if_false)?;

        Some(Self::make(ctx, pos, test, if_true, if_false))
    }

    /// `Make(context, pos, test, ifTrue, ifFalse)`: builds the expression, simplifying it when
    /// possible. The branches must already have the same type.
    ///
    // Port of: src/sksl/ir/SkSLTernaryExpression.cpp#L72-L137 (chrome/m156)
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        if_true: ExprId,
        if_false: ExprId,
    ) -> ExprId {
        let test_expr = constant_folder::get_constant_value_for_variable(&ctx.pool, test);
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

        if ctx.config().settings.optimize {
            let if_true_expr = constant_folder::get_constant_value_for_variable(&ctx.pool, if_true);
            let if_false_expr =
                constant_folder::get_constant_value_for_variable(&ctx.pool, if_false);

            // A ternary with matching true- and false-cases does not need to branch.
            if analysis::is_same_expression_tree(&ctx.pool, if_true_expr, if_false_expr) {
                // If `test` has no side-effects, we can eliminate it too, and just return
                // `ifTrue`.
                if !analysis::has_side_effects(&ctx.pool, test) {
                    ctx.pool.expression_mut(if_true).position = pos;
                    return if_true;
                }
                // Return a comma-expression containing `(test, ifTrue)`.
                return BinaryExpression::make(
                    ctx,
                    pos,
                    test,
                    Operator::from(OperatorKind::Comma),
                    if_true,
                );
            }

            // A ternary of the form `test ? expr : false` can be simplified to `test && expr`.
            if bool_literal(&ctx.pool, if_false_expr) == Some(false) {
                return BinaryExpression::make(
                    ctx,
                    pos,
                    test,
                    Operator::from(OperatorKind::LogicalAnd),
                    if_true,
                );
            }

            // A ternary of the form `test ? true : expr` can be simplified to `test || expr`.
            if bool_literal(&ctx.pool, if_true_expr) == Some(true) {
                return BinaryExpression::make(
                    ctx,
                    pos,
                    test,
                    Operator::from(OperatorKind::LogicalOr),
                    if_false,
                );
            }

            // A ternary of the form `test ? false : true` can be simplified to `!test`.
            if bool_literal(&ctx.pool, if_true_expr) == Some(false)
                && bool_literal(&ctx.pool, if_false_expr) == Some(true)
            {
                return PrefixExpression::make(
                    ctx,
                    pos,
                    Operator::from(OperatorKind::LogicalNot),
                    test,
                );
            }

            // A ternary of the form `test ? 1 : 0` can be simplified to `cast(test)`.
            if literal_value(&ctx.pool, if_true_expr) == Some(1.0)
                && literal_value(&ctx.pool, if_false_expr) == Some(0.0)
            {
                let ty = ctx.pool.expression(if_true).ty;
                return ConstructorScalarCast::make(ctx, pos, ty, test);
            }
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
