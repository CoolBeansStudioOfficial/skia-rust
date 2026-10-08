// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructor.{h,cpp} (`AnyConstructor::description`) and the
// data of the nine constructor classes (src/sksl/ir/SkSLConstructor*.h). `Constructor::Convert`,
// each class's `Convert`/`Make`, `getConstantValue` and `compareConstant` come with task S7a;
// put them in files named after the Skia classes (`constructor_array.rs`, …) next to this one.

//! The constructor expressions (`AnyConstructor` and its subclasses).

use super::{
    IrPool,
    ids::{ExprId, TypeId},
};
use crate::operator::OperatorPrecedence;
use crate::string::Separator;

/// `SkSL::ConstructorArray`: `float[3](a, b, c)`.
#[doc(alias = "SkSL::ConstructorArray")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorArray {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `SkSL::ConstructorArrayCast`: `half[2](floatArray)`.
#[doc(alias = "SkSL::ConstructorArrayCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorArrayCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorCompound`: `float4(xy, z, w)`, `float2x2(…)`.
#[doc(alias = "SkSL::ConstructorCompound")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorCompound {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `SkSL::ConstructorCompoundCast`: `half4(float4Value)`.
#[doc(alias = "SkSL::ConstructorCompoundCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorCompoundCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorDiagonalMatrix`: `float2x2(1)`.
#[doc(alias = "SkSL::ConstructorDiagonalMatrix")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorDiagonalMatrix {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorMatrixResize`: `float3x3(float2x2Value)`.
#[doc(alias = "SkSL::ConstructorMatrixResize")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorMatrixResize {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorScalarCast`: `int(floatValue)`.
#[doc(alias = "SkSL::ConstructorScalarCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorScalarCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorSplat`: `float4(1)`.
#[doc(alias = "SkSL::ConstructorSplat")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorSplat {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorStruct`: `S(a, b)`.
#[doc(alias = "SkSL::ConstructorStruct")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorStruct {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `AnyConstructor::description`: `type(arg, arg, …)`.
// Port of: src/sksl/ir/SkSLConstructor.cpp#L228-L237 (chrome/m156)
#[doc(alias = "AnyConstructor::description")]
#[must_use]
pub fn any_constructor_description(pool: &IrPool, ty: TypeId, arguments: &[ExprId]) -> String {
    let mut result = pool.ty(ty).description() + "(";
    let mut separator = Separator::new();
    for &arg in arguments {
        result.push_str(separator.next_str());
        result.push_str(&pool.expression_description_with(arg, OperatorPrecedence::Sequence));
    }
    result.push(')');
    result
}
