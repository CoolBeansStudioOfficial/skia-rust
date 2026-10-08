// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionCall.{h,cpp} (data and `description`). `Convert`,
// `Make`, `FindBestFunctionForCall` and intrinsic constant folding come with task S7c.

//! [`FunctionCall`]: `function(args…)`.

use super::{
    IrPool,
    ids::{ExprId, FnId},
};
use crate::operator::OperatorPrecedence;
use crate::string::Separator;

/// `SkSL::FunctionCall`.
// Port of: src/sksl/ir/SkSLFunctionCall.h#L29-L98 (chrome/m156)
#[doc(alias = "SkSL::FunctionCall")]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCall {
    /// `function()`.
    pub function: FnId,
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
    /// `stablePointer()`: identifies the call across `clone()`s (Specialization keys calls by
    /// it). It is the id the original call was allocated at
    /// ([`IrPool::next_expression_id`] before `add_expression`), and clones copy it.
    pub stable_pointer: ExprId,
}

impl FunctionCall {
    /// `description()`: `name(a, b)`.
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1012-L1021 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = format!("{}(", pool.function(self.function).name);
        let mut separator = Separator::new();
        for &arg in &self.arguments {
            result.push_str(separator.next_str());
            result.push_str(&pool.expression_description_with(arg, OperatorPrecedence::Sequence));
        }
        result.push(')');
        result
    }
}
