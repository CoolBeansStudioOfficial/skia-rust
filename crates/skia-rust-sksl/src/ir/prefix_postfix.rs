// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLPrefixExpression.{h,cpp} and
// src/sksl/ir/SkSLPostfixExpression.{h,cpp} (data and `description`). Their `Convert`/`Make`
// come with task S7b (in `prefix_expression.rs` / `postfix_expression.rs`).

//! [`PrefixExpression`] (`-x`, `!b`, `++i`) and [`PostfixExpression`] (`i++`).

use super::{IrPool, ids::ExprId};
use crate::operator::{Operator, OperatorPrecedence};

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
