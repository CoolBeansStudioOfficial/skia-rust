// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: the header-only expression classes src/sksl/ir/SkSLEmptyExpression.h,
// SkSLFunctionReference.h, SkSLMethodReference.h, SkSLPoison.h and SkSLTypeReference.h (data and
// `description`). Their `Make`/`Convert` (and `TypeReference::VerifyType`) come with task S7b.

//! The leaf expressions that only refer to something: [`EmptyExpression`],
//! [`FunctionReference`], [`MethodReference`], [`Poison`] and [`TypeReference`].

use super::{
    IrPool,
    ids::{ExprId, FnId, TypeId},
};
use crate::compiler::Compiler;

/// `SkSL::EmptyExpression`: a `void` expression that stands for nothing (an omitted `for`
/// clause). Its type is `void`.
// Port of: src/sksl/ir/SkSLEmptyExpression.h#L21-L48 (chrome/m156)
#[doc(alias = "SkSL::EmptyExpression")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyExpression;

impl EmptyExpression {
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
    /// `description()`: the type's name.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        pool.ty(self.value).name().to_owned()
    }
}
