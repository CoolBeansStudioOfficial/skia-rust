// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBreakStatement.h, SkSLContinueStatement.h,
// SkSLDiscardStatement.{h,cpp}, SkSLNop.h and SkSLReturnStatement.h (data and `description`).
// `DiscardStatement::Convert` comes with task S7d.

//! The small statements: [`BreakStatement`], [`ContinueStatement`], [`DiscardStatement`],
//! [`Nop`] and [`ReturnStatement`].

use super::{IrPool, ids::ExprId};

/// `SkSL::BreakStatement`.
#[doc(alias = "SkSL::BreakStatement")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BreakStatement;

impl BreakStatement {
    /// `description()`.
    #[must_use]
    pub fn description(self) -> String {
        "break;".to_owned()
    }
}

/// `SkSL::ContinueStatement`.
#[doc(alias = "SkSL::ContinueStatement")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContinueStatement;

impl ContinueStatement {
    /// `description()`.
    #[must_use]
    pub fn description(self) -> String {
        "continue;".to_owned()
    }
}

/// `SkSL::DiscardStatement`.
#[doc(alias = "SkSL::DiscardStatement")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscardStatement;

impl DiscardStatement {
    /// `description()`.
    #[must_use]
    pub fn description(self) -> String {
        "discard;".to_owned()
    }
}

/// `SkSL::Nop`: an empty statement. Its position is always invalid.
#[doc(alias = "SkSL::Nop")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nop;

impl Nop {
    /// `description()`.
    #[must_use]
    pub fn description(self) -> String {
        ";".to_owned()
    }
}

/// `SkSL::ReturnStatement`.
// Port of: src/sksl/ir/SkSLReturnStatement.h#L22-L58 (chrome/m156)
#[doc(alias = "SkSL::ReturnStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnStatement {
    /// `expression()`.
    pub expression: Option<ExprId>,
}

impl ReturnStatement {
    /// `description()`: `return x;` or `return;`.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        match self.expression {
            Some(e) => format!("return {};", pool.expression_description(e)),
            None => "return;".to_owned(),
        }
    }
}
