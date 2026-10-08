// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLDoStatement.{h,cpp}, SkSLExpressionStatement.{h,cpp},
// SkSLForStatement.{h,cpp}, SkSLIfStatement.{h,cpp}, SkSLSwitchStatement.{h,cpp} and
// SkSLSwitchCase.{h,cpp} (data, `description`, and the `ExpressionStatement` factories). The other
// factories live in files named after the Skia classes (`do_statement.rs`, `for_statement.rs`, …).

//! The statements that hold expressions or other statements: [`DoStatement`],
//! [`ExpressionStatement`], [`ForStatement`], [`IfStatement`], [`SwitchStatement`] and
//! [`SwitchCase`].

use super::{
    Expression, ExpressionKind, IrPool, Nop, Statement, StatementKind, VariableRefKind,
    ids::{ExprId, StmtId, SymTabId, VarId},
};
use crate::analysis;
use crate::context::Context;
use crate::defines::SkslInt;
use crate::operator::OperatorPrecedence;
use crate::position::{ForLoopPositions, Position};

/// `SkSL::DoStatement`: `do statement while (test);`.
// Port of: src/sksl/ir/SkSLDoStatement.h#L23-L74 (chrome/m156)
#[doc(alias = "SkSL::DoStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct DoStatement {
    /// `statement()`.
    pub statement: StmtId,
    /// `test()`.
    pub test: ExprId,
}

impl DoStatement {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLDoStatement.cpp#L48-L51 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "do {} while ({});",
            pool.statement_description(self.statement),
            pool.expression_description(self.test)
        )
    }
}

/// `SkSL::ExpressionStatement`: an expression evaluated for its side effects. Its position is
/// the expression's.
// Port of: src/sksl/ir/SkSLExpressionStatement.h#L24-L58 (chrome/m156)
#[doc(alias = "SkSL::ExpressionStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct ExpressionStatement {
    /// `expression()`.
    pub expression: ExprId,
}

impl ExpressionStatement {
    /// `ExpressionStatement::Convert`: a statement of `expr`, if `expr` is a complete expression.
    // Port of: src/sksl/ir/SkSLExpressionStatement.cpp#L14-L23 (chrome/m156)
    #[must_use]
    pub fn convert(ctx: &mut Context, expr: ExprId) -> Option<StmtId> {
        // Expression-statements need to represent a complete expression. Report an error on
        // intermediate expressions, like FunctionReference or TypeReference.
        if Expression::is_incomplete(ctx, expr) {
            return None;
        }
        Some(Self::make(ctx, expr))
    }

    /// `ExpressionStatement::Make`: a statement of `expr`. When optimizing, an expression with no
    /// side effects becomes a `Nop`, and an assignment whose target is read-write is demoted to a
    /// write, because the value of the assignment is discarded.
    // Port of: src/sksl/ir/SkSLExpressionStatement.cpp#L31-L53 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, expr: ExprId) -> StmtId {
        let pos = ctx.pool.expression(expr).position;
        if ctx.config().settings.optimize {
            // Expression-statements without any side effect can be replaced with a Nop.
            if !analysis::has_side_effects(&ctx.pool, expr) {
                return ctx
                    .pool
                    .add_statement(Statement::new(Position::default(), StatementKind::Nop(Nop)));
            }
            // If this is an assignment statement like `a += b;`, the ref-kind of `a` will be set
            // as read-write; `a` is written-to by the +=, and read-from by the consumer of the
            // expression. We can demote the ref-kind to "write" safely, because the result of the
            // expression is discarded; that is, `a` is never actually read-from.
            let assigned = match &ctx.pool.expression(expr).kind {
                ExpressionKind::Binary(binary) => binary.is_assignment_into_variable(&ctx.pool),
                _ => None,
            };
            if let Some(var_ref) = assigned
                && let ExpressionKind::VariableReference(reference) =
                    &mut ctx.pool.expression_mut(var_ref).kind
                && reference.ref_kind == VariableRefKind::ReadWrite
            {
                reference.ref_kind = VariableRefKind::Write;
            }
        }
        ctx.pool.add_statement(Statement::new(
            pos,
            StatementKind::Expression(ExpressionStatement { expression: expr }),
        ))
    }
}

impl ExpressionStatement {
    /// `description()`: the expression at statement precedence, then `;`.
    // Port of: src/sksl/ir/SkSLExpressionStatement.cpp#L58-L60 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        pool.expression_description_with(self.expression, OperatorPrecedence::Statement) + ";"
    }
}

/// `SkSL::LoopUnrollInfo`: how a strict-ES2 `for` loop iterates.
// Port of: src/sksl/ir/SkSLForStatement.h#L28-L33 (chrome/m156)
#[doc(alias = "SkSL::LoopUnrollInfo")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopUnrollInfo {
    /// `fIndex`: the loop variable.
    pub index: VarId,
    /// `fStart`.
    pub start: f64,
    /// `fDelta`.
    pub delta: f64,
    /// `fCount`.
    pub count: i32,
}

/// `SkSL::ForStatement`: `for (initializer; test; next) statement`.
// Port of: src/sksl/ir/SkSLForStatement.h#L38-L150 (chrome/m156)
#[doc(alias = "SkSL::ForStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct ForStatement {
    /// `forLoopPositions()`.
    pub for_loop_positions: ForLoopPositions,
    /// `symbols()`: the scope of the initializer's variables.
    pub symbol_table: Option<SymTabId>,
    /// `initializer()`.
    pub initializer: Option<StmtId>,
    /// `test()`.
    pub test: Option<ExprId>,
    /// `next()`.
    pub next: Option<ExprId>,
    /// `statement()`.
    pub statement: StmtId,
    /// `unrollInfo()`: only in strict-ES2 code.
    pub unroll_info: Option<LoopUnrollInfo>,
}

impl ForStatement {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLForStatement.cpp#L51-L68 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = String::from("for (");
        match self.initializer {
            Some(init) => result.push_str(&pool.statement_description(init)),
            None => result.push(';'),
        }
        result.push(' ');
        if let Some(test) = self.test {
            result.push_str(&pool.expression_description(test));
        }
        result.push_str("; ");
        if let Some(next) = self.next {
            result.push_str(&pool.expression_description(next));
        }
        result.push_str(") ");
        result.push_str(&pool.statement_description(self.statement));
        result
    }
}

/// `SkSL::IfStatement`: `if (test) ifTrue else ifFalse`.
// Port of: src/sksl/ir/SkSLIfStatement.h#L23-L87 (chrome/m156)
#[doc(alias = "SkSL::IfStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct IfStatement {
    /// `test()`.
    pub test: ExprId,
    /// `ifTrue()`.
    pub if_true: StmtId,
    /// `ifFalse()`.
    pub if_false: Option<StmtId>,
}

impl IfStatement {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLIfStatement.cpp#L23-L30 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = format!(
            "if ({}) {}",
            pool.expression_description(self.test),
            pool.statement_description(self.if_true)
        );
        if let Some(if_false) = self.if_false {
            result.push_str(" else ");
            result.push_str(&pool.statement_description(if_false));
        }
        result
    }
}

/// `SkSL::SwitchStatement`: `switch (value) caseBlock`. The case block is a `Block` that holds
/// only `SwitchCase` statements.
// Port of: src/sksl/ir/SkSLSwitchStatement.h#L28-L92 (chrome/m156)
#[doc(alias = "SkSL::SwitchStatement")]
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStatement {
    /// `value()`.
    pub value: ExprId,
    /// `caseBlock()`.
    pub case_block: StmtId,
}

impl SwitchStatement {
    /// `cases()`: the `SwitchCase` statements of the case block.
    ///
    /// # Panics
    ///
    /// If the case block is not a `Block`.
    #[must_use]
    pub fn cases<'a>(&self, pool: &'a IrPool) -> &'a [StmtId] {
        match &pool.statement(self.case_block).kind {
            StatementKind::Block(b) => &b.children,
            _ => panic!("SwitchStatement: the case block must be a Block"),
        }
    }

    /// `description()`.
    // Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L36-L38 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "switch ({}) {}",
            pool.expression_description(self.value),
            pool.statement_description(self.case_block)
        )
    }
}

/// `SkSL::SwitchCase`: `case value:` or `default:` followed by its statement.
// Port of: src/sksl/ir/SkSLSwitchCase.h#L23-L71 (chrome/m156)
#[doc(alias = "SkSL::SwitchCase")]
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchCase {
    /// `isDefault()`.
    pub is_default: bool,
    /// `value()` (meaningless for `default:`).
    pub value: SkslInt,
    /// `statement()`.
    pub statement: StmtId,
}

impl SwitchCase {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLSwitchCase.cpp#L25-L28 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let statement = pool.statement_description(self.statement);
        if self.is_default {
            format!("default: \n{statement}")
        } else {
            format!("case {}: \n{statement}", self.value)
        }
    }
}
