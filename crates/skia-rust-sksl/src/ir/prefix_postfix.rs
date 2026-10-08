// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLPrefixExpression.{h,cpp} and
// src/sksl/ir/SkSLPostfixExpression.{h,cpp} (data, `description`, `Convert` and `Make`).

//! [`PrefixExpression`] (`-x`, `!b`, `++i`) and [`PostfixExpression`] (`i++`).

use super::{
    BinaryExpression, ExprId, Expression, ExpressionKind, IrPool, TypeId, VariableRefKind,
    constant_folder_stub, s7b_shims,
};
use crate::context::Context;
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::position::Position;

/// `SkSL::PrefixExpression`. Its type is the operand's.
// Port of: src/sksl/ir/SkSLPrefixExpression.h#L25-L73 (chrome/m156)
#[doc(alias = "SkSL::PrefixExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct PrefixExpression {
    /// `getOperator()`.
    pub operator: Operator,
    /// `operand()`.
    pub operand: ExprId,
}

impl PrefixExpression {
    /// `Convert(context, pos, op, base)`: checks that `op` accepts the operand, and builds the
    /// expression. Reports errors and returns `None` on failure.
    // Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L233-L301 (chrome/m156)
    ///
    /// # Panics
    ///
    /// If `op` is not a prefix operator (Skia's `SK_ABORT`).
    pub fn convert(ctx: &mut Context, pos: Position, op: Operator, base: ExprId) -> Option<ExprId> {
        let base_ty = ctx.pool.expression(base).ty;
        let (is_array, is_number, is_boolean, is_integer) = {
            let t = ctx.pool.ty(base_ty);
            let component = t.component_type();
            (
                t.is_array(),
                component.is_number(),
                t.is_boolean(),
                component.is_integer(),
            )
        };
        let tight = op.tight_operator_name();
        let display = ctx.pool.ty(base_ty).display_name().to_owned();
        let cannot_operate = |ctx: &mut Context| {
            ctx.errors
                .error(pos, &format!("'{tight}' cannot operate on '{display}'"));
        };
        match op.kind() {
            OperatorKind::Plus | OperatorKind::Minus => {
                if is_array || !is_number {
                    cannot_operate(ctx);
                    return None;
                }
            }
            OperatorKind::PlusPlus | OperatorKind::MinusMinus => {
                if is_array || !is_number {
                    cannot_operate(ctx);
                    return None;
                }
                if !s7b_shims::update_variable_ref_kind(ctx, base, VariableRefKind::ReadWrite) {
                    return None;
                }
            }
            OperatorKind::LogicalNot => {
                if !is_boolean {
                    cannot_operate(ctx);
                    return None;
                }
            }
            OperatorKind::BitwiseNot => {
                if ctx.config().strict_es2_mode() {
                    // GLSL ES 1.00, Section 5.1
                    ctx.errors
                        .error(pos, &format!("operator '{tight}' is not allowed"));
                    return None;
                }
                if is_array || !is_integer {
                    cannot_operate(ctx);
                    return None;
                }
            }
            _ => unreachable!("unsupported prefix operator"),
        }

        Some(Self::make(ctx, pos, op, base))
    }

    /// `Make(context, pos, op, base)`: builds the expression, simplifying it when possible. The
    /// operand must already be valid for `op`.
    // Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L303-L347 (chrome/m156)
    ///
    /// # Panics
    ///
    /// If a `BitwiseNot` operand is a literal that does not coerce to its scalar type, which
    /// `Convert` rules out.
    pub fn make(ctx: &mut Context, pos: Position, op: Operator, base: ExprId) -> ExprId {
        let mut base = base;
        match op.kind() {
            OperatorKind::Plus => {
                ctx.pool.expression_mut(base).position = pos;
                return base;
            }
            OperatorKind::Minus => return negate_operand(ctx, pos, base),
            OperatorKind::LogicalNot => return logical_not_operand(ctx, pos, base),
            OperatorKind::PlusPlus | OperatorKind::MinusMinus => {}
            OperatorKind::BitwiseNot => {
                let base_ty = ctx.pool.expression(base).ty;
                if ctx.pool.ty(base_ty).is_literal() {
                    // The expression `~123` is no longer a literal; coerce to the actual type.
                    let scalar = ctx.pool.ty(base_ty).scalar_type_for_literal().id();
                    base = s7b_shims::coerce_expression(ctx, scalar, base)
                        .expect("a literal coerces to its scalar type");
                }
                return bitwise_not_operand(ctx, pos, base);
            }
            _ => debug_assert!(false, "unsupported prefix operator: {}", op.operator_name()),
        }

        let operand_ty = ctx.pool.expression(base).ty;
        ctx.pool.add_expression(Expression::new(
            pos,
            operand_ty,
            ExpressionKind::Prefix(Self {
                operator: op,
                operand: base,
            }),
        ))
    }

    /// `description(parentPrecedence)`.
    // Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L349-L355 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool, parent_precedence: OperatorPrecedence) -> String {
        let needs_parens = OperatorPrecedence::Prefix >= parent_precedence;
        format!(
            "{}{}{}{}",
            if needs_parens { "(" } else { "" },
            self.operator.tight_operator_name(),
            pool.expression_description_with(self.operand, OperatorPrecedence::Prefix),
            if needs_parens { ")" } else { "" },
        )
    }
}

/// `simplify_negation`: the simplified form of `-original`, or `None`.
///
/// Stand-in in part. Folding a negated constant (`-1.0`, `-vec(...)`, `-array(...)`,
/// `-matrix(...)`) needs S7a's `ConstructorCompound::MakeFromConstants` and `getConstantValue`, and
/// S8. Those cases return `None`, so the node stays `-(…)`. The double negation `-(-x)` is
/// Skia's and is kept.
// Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L71-L120 (chrome/m156), partly
fn simplify_negation(ctx: &mut Context, pos: Position, original: ExprId) -> Option<ExprId> {
    let value = constant_folder_stub::get_constant_value_for_variable(&ctx.pool, original);
    if let ExpressionKind::Prefix(prefix) = &ctx.pool.expression(value).kind
        && prefix.operator.kind() == OperatorKind::Minus
    {
        // Convert `-(-expression)` into `expression`.
        let operand = prefix.operand;
        return Some(ctx.pool.clone_expression_at(operand, pos));
    }
    None
}

/// `negate_operand`: `-value`, simplified when possible.
// Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L139-L149 (chrome/m156)
fn negate_operand(ctx: &mut Context, pos: Position, value: ExprId) -> ExprId {
    if let Some(simplified) = simplify_negation(ctx, pos, value) {
        return simplified;
    }
    let ty = ctx.pool.expression(value).ty;
    ctx.pool.add_expression(Expression::new(
        pos,
        ty,
        ExpressionKind::Prefix(PrefixExpression {
            operator: Operator::from(OperatorKind::Minus),
            operand: value,
        }),
    ))
}

/// `logical_not_operand`: `!operand`, simplified when possible.
// Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L151-L196 (chrome/m156)
fn logical_not_operand(ctx: &mut Context, pos: Position, operand: ExprId) -> ExprId {
    let value = constant_folder_stub::get_constant_value_for_variable(&ctx.pool, operand);
    let operand_ty = ctx.pool.expression(operand).ty;
    match ctx.pool.expression(value).kind.clone() {
        ExpressionKind::Literal(b) => {
            // Convert !boolLiteral(true) to boolLiteral(false).
            return s7b_shims::make_bool_literal(ctx, pos, !b.bool_value(), operand_ty);
        }
        ExpressionKind::Prefix(prefix) => {
            // Convert `!(!expression)` into `expression`.
            if prefix.operator.kind() == OperatorKind::LogicalNot {
                ctx.pool.expression_mut(prefix.operand).position = pos;
                return prefix.operand;
            }
        }
        ExpressionKind::Binary(binary) => {
            let replacement = match binary.operator.kind() {
                OperatorKind::EqEq => Some(OperatorKind::Neq),
                OperatorKind::Neq => Some(OperatorKind::EqEq),
                OperatorKind::Lt => Some(OperatorKind::GtEq),
                OperatorKind::LtEq => Some(OperatorKind::Gt),
                OperatorKind::Gt => Some(OperatorKind::LtEq),
                OperatorKind::GtEq => Some(OperatorKind::Lt),
                _ => None,
            };
            if let Some(replacement) = replacement {
                let binary_ty = ctx.pool.expression(value).ty;
                return BinaryExpression::make_with_result_type(
                    ctx,
                    pos,
                    binary.left,
                    Operator::from(replacement),
                    binary.right,
                    binary_ty,
                );
            }
        }
        _ => {}
    }

    // No simplified form; convert expression to Prefix(LOGICALNOT, expression).
    ctx.pool.add_expression(Expression::new(
        pos,
        operand_ty,
        ExpressionKind::Prefix(PrefixExpression {
            operator: Operator::from(OperatorKind::LogicalNot),
            operand,
        }),
    ))
}

/// `bitwise_not_operand`: `~operand`, simplified when possible.
///
/// Stand-in in part: folding a constant operand needs S7a and S8, as in `simplify_negation`.
// Port of: src/sksl/ir/SkSLPrefixExpression.cpp#L198-L231 (chrome/m156), partly
fn bitwise_not_operand(ctx: &mut Context, pos: Position, operand: ExprId) -> ExprId {
    let value = constant_folder_stub::get_constant_value_for_variable(&ctx.pool, operand);
    if let ExpressionKind::Prefix(prefix) = ctx.pool.expression(value).kind.clone()
        && prefix.operator.kind() == OperatorKind::BitwiseNot
    {
        // Convert `~(~expression)` into `expression`.
        ctx.pool.expression_mut(prefix.operand).position = pos;
        return prefix.operand;
    }
    let operand_ty = ctx.pool.expression(operand).ty;
    ctx.pool.add_expression(Expression::new(
        pos,
        operand_ty,
        ExpressionKind::Prefix(PrefixExpression {
            operator: Operator::from(OperatorKind::BitwiseNot),
            operand,
        }),
    ))
}

/// `SkSL::PostfixExpression`. Its type is the operand's.
// Port of: src/sksl/ir/SkSLPostfixExpression.h#L25-L75 (chrome/m156)
#[doc(alias = "SkSL::PostfixExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct PostfixExpression {
    /// `operand()`.
    pub operand: ExprId,
    /// `getOperator()`.
    pub operator: Operator,
}

impl PostfixExpression {
    /// `Convert(context, pos, base, op)`: checks that the operand can be incremented or
    /// decremented, and builds the expression. Reports errors and returns `None` on failure.
    // Port of: src/sksl/ir/SkSLPostfixExpression.cpp#L20-L34 (chrome/m156)
    pub fn convert(ctx: &mut Context, pos: Position, base: ExprId, op: Operator) -> Option<ExprId> {
        let base_ty = ctx.pool.expression(base).ty;
        let (is_array, is_number, display) = {
            let t = ctx.pool.ty(base_ty);
            (
                t.is_array(),
                t.component_type().is_number(),
                t.display_name().to_owned(),
            )
        };
        if is_array || !is_number {
            ctx.errors.error(
                pos,
                &format!(
                    "'{}' cannot operate on '{display}'",
                    op.tight_operator_name()
                ),
            );
            return None;
        }
        if !s7b_shims::update_variable_ref_kind(ctx, base, VariableRefKind::ReadWrite) {
            return None;
        }
        Some(Self::make(ctx, pos, base, op))
    }

    /// `Make(context, pos, base, op)`: builds the expression. The operand must be assignable.
    // Port of: src/sksl/ir/SkSLPostfixExpression.cpp#L36-L43 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position, base: ExprId, op: Operator) -> ExprId {
        let operand_ty: TypeId = ctx.pool.expression(base).ty;
        ctx.pool.add_expression(Expression::new(
            pos,
            operand_ty,
            ExpressionKind::Postfix(Self {
                operand: base,
                operator: op,
            }),
        ))
    }

    /// `description(parentPrecedence)`.
    // Port of: src/sksl/ir/SkSLPostfixExpression.cpp#L45-L51 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool, parent_precedence: OperatorPrecedence) -> String {
        let needs_parens = OperatorPrecedence::Postfix >= parent_precedence;
        format!(
            "{}{}{}{}",
            if needs_parens { "(" } else { "" },
            pool.expression_description_with(self.operand, OperatorPrecedence::Postfix),
            self.operator.tight_operator_name(),
            if needs_parens { ")" } else { "" },
        )
    }
}
