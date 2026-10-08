// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLTernaryExpression.{h,cpp} (data and `description`).
// `Convert` and `Make` come with task S7b.

//! [`TernaryExpression`]: `test ? ifTrue : ifFalse`.

use super::{IrPool, ids::ExprId};
use crate::operator::OperatorPrecedence;

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
