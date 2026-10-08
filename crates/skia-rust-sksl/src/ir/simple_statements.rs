// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBreakStatement.h, SkSLContinueStatement.h,
// SkSLDiscardStatement.{h,cpp}, SkSLNop.h and SkSLReturnStatement.h (data, `description` and the
// factories).

//! The small statements: [`BreakStatement`], [`ContinueStatement`], [`DiscardStatement`],
//! [`Nop`] and [`ReturnStatement`].

use super::{
    IrPool, Statement, StatementKind,
    ids::{ExprId, StmtId},
};
use crate::context::Context;
use crate::position::Position;
use crate::program_settings::ProgramConfig;

/// `SkSL::BreakStatement`.
#[doc(alias = "SkSL::BreakStatement")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BreakStatement;

impl BreakStatement {
    /// `BreakStatement::Make`.
    // Port of: src/sksl/ir/SkSLBreakStatement.h#L25-L27 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position) -> StmtId {
        pool.add_statement(Statement::new(pos, StatementKind::Break(Self)))
    }

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
    /// `ContinueStatement::Make`.
    // Port of: src/sksl/ir/SkSLContinueStatement.h#L25-L27 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position) -> StmtId {
        pool.add_statement(Statement::new(pos, StatementKind::Continue(Self)))
    }

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
    /// `DiscardStatement::Convert`: a `discard` is only permitted in fragment shaders.
    // Port of: src/sksl/ir/SkSLDiscardStatement.cpp#L14-L20 (chrome/m156)
    #[must_use]
    pub fn convert(ctx: &mut Context, pos: Position) -> Option<StmtId> {
        if !ProgramConfig::is_fragment(ctx.config().kind) {
            ctx.errors.error(
                pos,
                "discard statement is only permitted in fragment shaders",
            );
            return None;
        }
        Some(Self::make(ctx, pos))
    }

    /// `DiscardStatement::Make`: the caller has checked that this is a fragment program.
    // Port of: src/sksl/ir/SkSLDiscardStatement.cpp#L22-L26 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position) -> StmtId {
        debug_assert!(ProgramConfig::is_fragment(ctx.config().kind));
        ctx.pool
            .add_statement(Statement::new(pos, StatementKind::Discard(Self)))
    }

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
    /// `Nop::Make()`: an empty statement at an invalid position.
    // Port of: src/sksl/ir/SkSLNop.h#L20-L22 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool) -> StmtId {
        pool.add_statement(Statement::new(
            Position::default(),
            StatementKind::Nop(Self),
        ))
    }

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
    /// `ReturnStatement::Make`.
    // Port of: src/sksl/ir/SkSLReturnStatement.h#L22-L58 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position, expression: Option<ExprId>) -> StmtId {
        pool.add_statement(Statement::new(
            pos,
            StatementKind::Return(Self { expression }),
        ))
    }

    /// `description()`: `return x;` or `return;`.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        match self.expression {
            Some(e) => format!("return {};", pool.expression_description(e)),
            None => "return;".to_owned(),
        }
    }
}
