// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBinaryExpression.{h,cpp} (data and `description`).
// `Convert`, `Make`, `CheckRef` and `isAssignmentIntoVariable` come with task S7b.

//! [`BinaryExpression`]: `left op right`.

use super::{IrPool, ids::ExprId};
use crate::operator::{Operator, OperatorPrecedence};

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
