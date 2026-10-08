// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLIndexExpression.{h,cpp} (data and `description`).
// `Convert`, `Make` and `IndexType` come with task S7b.

//! [`IndexExpression`]: `base[index]`.

use super::{IrPool, ids::ExprId};
use crate::operator::OperatorPrecedence;

/// `SkSL::IndexExpression`.
// Port of: src/sksl/ir/SkSLIndexExpression.h#L25-L94 (chrome/m156)
#[doc(alias = "SkSL::IndexExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct IndexExpression {
    /// `base()`.
    pub base: ExprId,
    /// `index()`.
    pub index: ExprId,
}

impl IndexExpression {
    /// `description()`: `base[index]`.
    // Port of: src/sksl/ir/SkSLIndexExpression.cpp#L172-L175 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}[{}]",
            pool.expression_description_with(self.base, OperatorPrecedence::Postfix),
            pool.expression_description_with(self.index, OperatorPrecedence::EXPRESSION)
        )
    }
}
