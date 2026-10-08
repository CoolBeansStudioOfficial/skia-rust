// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: the header-only expression classes src/sksl/ir/SkSLEmptyExpression.h,
// SkSLFunctionReference.h, SkSLMethodReference.h, SkSLPoison.h and SkSLTypeReference.h (data and
// `description`, `Make` and `Convert`, and `TypeReference::VerifyType`).

//! The leaf expressions that only refer to something: [`EmptyExpression`],
//! [`FunctionReference`], [`MethodReference`], [`Poison`] and [`TypeReference`].

use super::{
    Expression, ExpressionKind, IrPool,
    ids::{ExprId, FnId, TypeId},
};
use crate::compiler::Compiler;
use crate::context::Context;
use crate::position::Position;

/// `SkSL::EmptyExpression`: a `void` expression that stands for nothing (an omitted `for`
/// clause). Its type is `void`.
// Port of: src/sksl/ir/SkSLEmptyExpression.h#L21-L48 (chrome/m156)
#[doc(alias = "SkSL::EmptyExpression")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyExpression;

impl EmptyExpression {
    /// `Make(pos, context)`: an empty `void` expression.
    // Port of: src/sksl/ir/SkSLEmptyExpression.h#L25-L27 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position) -> ExprId {
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::VOID,
            ExpressionKind::Empty(Self),
        ))
    }

    /// `description()`: an empty expression prints as `false`.
    #[must_use]
    pub fn description(self) -> String {
        "false".to_owned()
    }
}

/// `SkSL::FunctionReference`: a function name not yet called. Its type is `<INVALID>`.
// Port of: src/sksl/ir/SkSLFunctionReference.h#L22-L58 (chrome/m156)
#[doc(alias = "SkSL::FunctionReference")]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionReference {
    /// `overloadChain()`: the first function of that name.
    pub overload_chain: FnId,
}

impl FunctionReference {
    /// `FunctionReference(context, pos, overloadChain)`: a function name not yet called. Its type
    /// is `<INVALID>`.
    // Port of: src/sksl/ir/SkSLFunctionReference.h#L26-L30 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position, overload_chain: FnId) -> ExprId {
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::INVALID,
            ExpressionKind::FunctionReference(Self { overload_chain }),
        ))
    }

    /// `description()`.
    #[must_use]
    pub fn description(&self) -> String {
        "<function>".to_owned()
    }
}

/// `SkSL::MethodReference`: `self.method` not yet called. Its type is `<INVALID>`.
// Port of: src/sksl/ir/SkSLMethodReference.h#L22-L76 (chrome/m156)
#[doc(alias = "SkSL::MethodReference")]
#[derive(Clone, Debug, PartialEq)]
pub struct MethodReference {
    /// `self()`.
    pub self_: ExprId,
    /// `overloadChain()`.
    pub overload_chain: FnId,
}

impl MethodReference {
    /// `MethodReference(context, pos, self, overloadChain)`: `self.method` not yet called. Its
    /// type is `<INVALID>`.
    // Port of: src/sksl/ir/SkSLMethodReference.h#L30-L37 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position, self_: ExprId, overload_chain: FnId) -> ExprId {
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::INVALID,
            ExpressionKind::MethodReference(Self {
                self_,
                overload_chain,
            }),
        ))
    }

    /// `description()`.
    #[must_use]
    pub fn description(&self) -> String {
        "<method>".to_owned()
    }
}

/// `SkSL::Poison`: stands in for an expression that failed to convert. Errors that mention it
/// are not reported. Its type is `<POISON>`.
// Port of: src/sksl/ir/SkSLPoison.h#L17-L43 (chrome/m156)
#[doc(alias = "SkSL::Poison")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Poison;

impl Poison {
    /// `Make(pos, context)`: a stand-in for an expression that failed to convert. Its type is
    /// `<POISON>`.
    // Port of: src/sksl/ir/SkSLPoison.h#L21-L24 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position) -> ExprId {
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::POISON,
            ExpressionKind::Poison(Self),
        ))
    }

    /// `description()`: [`Compiler::POISON_TAG`].
    #[must_use]
    pub fn description(self) -> String {
        Compiler::POISON_TAG.to_owned()
    }
}

/// `SkSL::TypeReference`: a type name used as an expression (before a constructor call). Its
/// type is `<INVALID>`.
// Port of: src/sksl/ir/SkSLTypeReference.h#L23-L71 (chrome/m156)
#[doc(alias = "SkSL::TypeReference")]
#[derive(Clone, Debug, PartialEq)]
pub struct TypeReference {
    /// `value()`: the named type.
    pub value: TypeId,
}

impl TypeReference {
    /// `VerifyType(context, type, pos)`: reports and returns false for a generic or literal type,
    /// or, in a strict ES2 program, a type that is not allowed in ES2. Built-in code is not
    /// checked.
    // Port of: src/sksl/ir/SkSLTypeReference.cpp#L16-L28 (chrome/m156)
    pub fn verify_type(ctx: &mut Context, ty: TypeId, pos: Position) -> bool {
        if ctx.config().is_builtin_code() {
            return true;
        }
        let (name, generic_or_literal, allowed) = {
            let t = ctx.pool.ty(ty);
            (
                t.name().to_owned(),
                t.is_generic() || t.is_literal(),
                t.is_allowed_in_es2_for(ctx),
            )
        };
        if generic_or_literal {
            ctx.errors.error(pos, &format!("type '{name}' is generic"));
            return false;
        }
        if !allowed {
            ctx.errors
                .error(pos, &format!("type '{name}' is not supported"));
            return false;
        }
        true
    }

    /// `Convert(context, pos, type)`: a reference to `ty`, or `None` after an error.
    // Port of: src/sksl/ir/SkSLTypeReference.cpp#L30-L35 (chrome/m156)
    pub fn convert(ctx: &mut Context, pos: Position, ty: TypeId) -> Option<ExprId> {
        if Self::verify_type(ctx, ty, pos) {
            Some(Self::make(ctx, pos, ty))
        } else {
            None
        }
    }

    /// `Make(context, pos, type)`: a reference to `ty`, without checks. Its type is `<INVALID>`.
    // Port of: src/sksl/ir/SkSLTypeReference.cpp#L37-L42 (chrome/m156)
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::INVALID,
            ExpressionKind::TypeReference(Self { value: ty }),
        ))
    }

    /// `description()`: the type's name.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        pool.ty(self.value).name().to_owned()
    }
}
