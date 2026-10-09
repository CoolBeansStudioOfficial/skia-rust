// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLExpression.{h,cpp}, src/sksl/ir/SkSLIRNode.h
// (`ExpressionKind`) and the `description`/`clone` overrides of every expression class.
// `compareConstant`/`getConstantValue` come with tasks S7a and S8.

//! [`Expression`]: an expression node, and [`ExpressionKind`], its subclass and data.

use super::{
    BinaryExpression, ChildCall, ConstructorArray, ConstructorArrayCast, ConstructorCompound,
    ConstructorCompoundCast, ConstructorDiagonalMatrix, ConstructorMatrixResize,
    ConstructorScalarCast, ConstructorSplat, ConstructorStruct, EmptyExpression, FieldAccess,
    FunctionCall, FunctionReference, IndexExpression, IrPool, Literal, MethodReference, Poison,
    PostfixExpression, PrefixExpression, Setting, Swizzle, TernaryExpression, TypeReference,
    VariableReference,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::operator::OperatorPrecedence;
use crate::position::Position;

/// `SkSL::Expression`: an expression of type `ty`.
///
/// The node is plain data. Children are [`ExprId`]s into the same pool chain; replacing a child
/// overwrites the pool entry at its id ([`IrPool::replace_expression`]).
// Port of: src/sksl/ir/SkSLExpression.h#L33-L140 (chrome/m156)
#[doc(alias = "SkSL::Expression")]
#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    /// `fPosition`.
    pub position: Position,
    /// `type()`.
    pub ty: TypeId,
    /// The subclass and its data (`kind()`).
    pub kind: ExpressionKind,
}

/// `Expression::Kind`, carrying each subclass's data. The variants are in Skia's
/// `ExpressionKind` order.
// Port of: src/sksl/ir/SkSLIRNode.h#L59-L87 (chrome/m156)
#[doc(alias = "SkSL::ExpressionKind")]
#[derive(Clone, Debug, PartialEq)]
pub enum ExpressionKind {
    Binary(BinaryExpression),
    ChildCall(ChildCall),
    ConstructorArray(ConstructorArray),
    ConstructorArrayCast(ConstructorArrayCast),
    ConstructorCompound(ConstructorCompound),
    ConstructorCompoundCast(ConstructorCompoundCast),
    ConstructorDiagonalMatrix(ConstructorDiagonalMatrix),
    ConstructorMatrixResize(ConstructorMatrixResize),
    ConstructorScalarCast(ConstructorScalarCast),
    ConstructorSplat(ConstructorSplat),
    ConstructorStruct(ConstructorStruct),
    Empty(EmptyExpression),
    FieldAccess(FieldAccess),
    FunctionReference(FunctionReference),
    FunctionCall(FunctionCall),
    Index(IndexExpression),
    Literal(Literal),
    MethodReference(MethodReference),
    Poison(Poison),
    Postfix(PostfixExpression),
    Prefix(PrefixExpression),
    Setting(Setting),
    Swizzle(Swizzle),
    Ternary(TernaryExpression),
    TypeReference(TypeReference),
    VariableReference(VariableReference),
}

/// `Expression::ComparisonResult`: the result of `compareConstant`.
#[doc(alias = "Expression::ComparisonResult")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonResult {
    Unknown = -1,
    NotEqual = 0,
    Equal = 1,
}

impl ExpressionKind {
    /// Calls `f` on every child expression slot, in the order the node's `clone()` copies them.
    /// This includes `MethodReference::self`, which `ProgramVisitor` does not visit.
    pub fn for_each_child_mut(&mut self, f: &mut impl FnMut(&mut ExprId)) {
        match self {
            Self::Binary(b) => {
                f(&mut b.left);
                f(&mut b.right);
            }
            Self::ChildCall(ChildCall { arguments, .. })
            | Self::ConstructorArray(ConstructorArray { arguments })
            | Self::ConstructorCompound(ConstructorCompound { arguments })
            | Self::ConstructorStruct(ConstructorStruct { arguments })
            | Self::FunctionCall(FunctionCall { arguments, .. }) => {
                arguments.iter_mut().for_each(f);
            }
            Self::ConstructorArrayCast(ConstructorArrayCast { argument })
            | Self::ConstructorCompoundCast(ConstructorCompoundCast { argument })
            | Self::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument })
            | Self::ConstructorMatrixResize(ConstructorMatrixResize { argument })
            | Self::ConstructorScalarCast(ConstructorScalarCast { argument })
            | Self::ConstructorSplat(ConstructorSplat { argument }) => f(argument),
            Self::FieldAccess(FieldAccess { base, .. })
            | Self::MethodReference(MethodReference { self_: base, .. })
            | Self::Swizzle(Swizzle { base, .. }) => f(base),
            Self::Index(i) => {
                f(&mut i.base);
                f(&mut i.index);
            }
            Self::Postfix(PostfixExpression { operand, .. })
            | Self::Prefix(PrefixExpression { operand, .. }) => f(operand),
            Self::Ternary(t) => {
                f(&mut t.test);
                f(&mut t.if_true);
                f(&mut t.if_false);
            }
            Self::Empty(_)
            | Self::FunctionReference(_)
            | Self::Literal(_)
            | Self::Poison(_)
            | Self::Setting(_)
            | Self::TypeReference(_)
            | Self::VariableReference(_) => {}
        }
    }
}

impl Expression {
    /// A node of `kind` with type `ty` at `position`.
    #[must_use]
    pub fn new(position: Position, ty: TypeId, kind: ExpressionKind) -> Self {
        Self { position, ty, kind }
    }

    /// `isAnyConstructor()`.
    #[must_use]
    pub fn is_any_constructor(&self) -> bool {
        self.any_constructor_arguments().is_some()
    }

    /// `asAnyConstructor().argumentSpan()`: the arguments of any constructor kind, or `None`.
    #[must_use]
    pub fn any_constructor_arguments(&self) -> Option<&[ExprId]> {
        match &self.kind {
            ExpressionKind::ConstructorArray(ConstructorArray { arguments })
            | ExpressionKind::ConstructorCompound(ConstructorCompound { arguments })
            | ExpressionKind::ConstructorStruct(ConstructorStruct { arguments }) => Some(arguments),
            ExpressionKind::ConstructorArrayCast(ConstructorArrayCast { argument })
            | ExpressionKind::ConstructorCompoundCast(ConstructorCompoundCast { argument })
            | ExpressionKind::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument })
            | ExpressionKind::ConstructorMatrixResize(ConstructorMatrixResize { argument })
            | ExpressionKind::ConstructorScalarCast(ConstructorScalarCast { argument })
            | ExpressionKind::ConstructorSplat(ConstructorSplat { argument }) => {
                Some(std::slice::from_ref(argument))
            }
            _ => None,
        }
    }

    /// `asAnyConstructor().argumentSpan()`, mutable.
    pub fn any_constructor_arguments_mut(&mut self) -> Option<&mut [ExprId]> {
        match &mut self.kind {
            ExpressionKind::ConstructorArray(ConstructorArray { arguments })
            | ExpressionKind::ConstructorCompound(ConstructorCompound { arguments })
            | ExpressionKind::ConstructorStruct(ConstructorStruct { arguments }) => Some(arguments),
            ExpressionKind::ConstructorArrayCast(ConstructorArrayCast { argument })
            | ExpressionKind::ConstructorCompoundCast(ConstructorCompoundCast { argument })
            | ExpressionKind::ConstructorDiagonalMatrix(ConstructorDiagonalMatrix { argument })
            | ExpressionKind::ConstructorMatrixResize(ConstructorMatrixResize { argument })
            | ExpressionKind::ConstructorScalarCast(ConstructorScalarCast { argument })
            | ExpressionKind::ConstructorSplat(ConstructorSplat { argument }) => {
                Some(std::slice::from_mut(argument))
            }
            _ => None,
        }
    }

    /// The literal payload, if this is a `Literal`.
    #[must_use]
    pub fn as_literal(&self) -> Option<&Literal> {
        match &self.kind {
            ExpressionKind::Literal(l) => Some(l),
            _ => None,
        }
    }

    /// `isIntLiteral()`.
    #[must_use]
    pub fn is_int_literal(&self, pool: &IrPool) -> bool {
        self.as_literal().is_some() && pool.ty(self.ty).is_integer()
    }

    /// `isFloatLiteral()`.
    #[must_use]
    pub fn is_float_literal(&self, pool: &IrPool) -> bool {
        self.as_literal().is_some() && pool.ty(self.ty).is_float()
    }

    /// `isBoolLiteral()`.
    #[must_use]
    pub fn is_bool_literal(&self, pool: &IrPool) -> bool {
        self.as_literal().is_some() && pool.ty(self.ty).is_boolean()
    }

    /// `description()`: the expression as `SkSL` text, at top-level precedence.
    // Port of: src/sksl/ir/SkSLExpression.cpp#L18-L20 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        self.description_with_precedence(pool, OperatorPrecedence::EXPRESSION)
    }

    /// `description(parentPrecedence)`: parenthesized where `parent_precedence` requires it.
    #[must_use]
    pub fn description_with_precedence(
        &self,
        pool: &IrPool,
        parent_precedence: OperatorPrecedence,
    ) -> String {
        match &self.kind {
            ExpressionKind::Binary(b) => b.description(pool, parent_precedence),
            ExpressionKind::ChildCall(c) => c.description(pool),
            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorArrayCast(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorCompoundCast(_)
            | ExpressionKind::ConstructorDiagonalMatrix(_)
            | ExpressionKind::ConstructorMatrixResize(_)
            | ExpressionKind::ConstructorScalarCast(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_) => {
                super::constructor::any_constructor_description(
                    pool,
                    self.ty,
                    self.any_constructor_arguments().unwrap_or_default(),
                )
            }
            ExpressionKind::Empty(e) => e.description(),
            ExpressionKind::FieldAccess(f) => f.description(pool),
            ExpressionKind::FunctionReference(f) => f.description(),
            ExpressionKind::FunctionCall(c) => c.description(pool),
            ExpressionKind::Index(i) => i.description(pool),
            ExpressionKind::Literal(l) => l.description(pool, self.ty),
            ExpressionKind::MethodReference(m) => m.description(),
            ExpressionKind::Poison(p) => p.description(),
            ExpressionKind::Postfix(p) => p.description(pool, parent_precedence),
            ExpressionKind::Prefix(p) => p.description(pool, parent_precedence),
            ExpressionKind::Setting(s) => s.description(),
            ExpressionKind::Swizzle(s) => s.description(pool),
            ExpressionKind::Ternary(t) => t.description(pool, parent_precedence),
            ExpressionKind::TypeReference(t) => t.description(pool),
            ExpressionKind::VariableReference(v) => v.description(pool),
        }
    }

    /// `isIncomplete(context)`: reports an error and returns true for a dangling function,
    /// method or type reference (never called or constructed), or a bare `sk_Caps`.
    // Port of: src/sksl/ir/SkSLExpression.cpp#L22-L46 (chrome/m156)
    pub fn is_incomplete(ctx: &mut Context, expr: ExprId) -> bool {
        let e = ctx.pool.expression(expr);
        let position = e.position;
        match &e.kind {
            ExpressionKind::FunctionReference(_) => {
                ctx.errors
                    .error(position.after(), "expected '(' to begin function call");
                true
            }
            ExpressionKind::MethodReference(_) => {
                ctx.errors
                    .error(position.after(), "expected '(' to begin method call");
                true
            }
            ExpressionKind::TypeReference(_) => {
                ctx.errors.error(
                    position.after(),
                    "expected '(' to begin constructor invocation",
                );
                true
            }
            ExpressionKind::VariableReference(_) => {
                if ctx.pool.ty(e.ty).matches(TypeId::SK_CAPS) {
                    ctx.errors.error(position, "invalid expression");
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

impl IrPool {
    /// `expr->description()` for the expression at `id`.
    #[must_use]
    pub fn expression_description(&self, id: ExprId) -> String {
        self.expression(id).description(self)
    }

    /// `expr->description(parentPrecedence)` for the expression at `id`.
    #[must_use]
    pub fn expression_description_with(
        &self,
        id: ExprId,
        parent_precedence: OperatorPrecedence,
    ) -> String {
        self.expression(id)
            .description_with_precedence(self, parent_precedence)
    }
}
