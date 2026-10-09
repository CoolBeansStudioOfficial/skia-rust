// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLIfStatement.{h,cpp} (`Convert` and `Make`).

//! The factories of [`IfStatement`].

use super::{
    ExpressionStatement, IfStatement, IrPool, Nop, Statement, StatementKind, TypeId,
    ids::{ExprId, StmtId},
};
use crate::analysis;
use crate::constant_folder;
use crate::context::Context;
use crate::position::Position;

/// `replace_empty_with_nop`: keeps `stmt` unless it is missing, or empty and not already a `Nop`.
// Port of: src/sksl/ir/SkSLIfStatement.cpp#L40-L43 (chrome/m156)
fn replace_empty_with_nop(pool: &mut IrPool, stmt: Option<StmtId>, is_empty: bool) -> StmtId {
    match stmt {
        Some(stmt) if !is_empty || matches!(pool.statement(stmt).kind, StatementKind::Nop(_)) => {
            stmt
        }
        _ => Nop::make(pool),
    }
}

impl IfStatement {
    /// `IfStatement::Convert`: coerces the test to `bool`, and rejects a declaration that is not
    /// in a scope.
    // Port of: src/sksl/ir/SkSLIfStatement.cpp#L32-L50 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        if_true: StmtId,
        if_false: Option<StmtId>,
    ) -> Option<StmtId> {
        let test = TypeId::BOOL.coerce_expression(ctx, test)?;
        if analysis::detect_var_declaration_without_scope(&ctx.pool, if_true, Some(&mut ctx.errors))
        {
            return None;
        }
        if let Some(if_false) = if_false
            && analysis::detect_var_declaration_without_scope(
                &ctx.pool,
                if_false,
                Some(&mut ctx.errors),
            )
        {
            return None;
        }
        Some(Self::make(ctx, pos, test, if_true, if_false))
    }

    /// `IfStatement::Make`: a potentially simplified if-statement. When optimizing, an `if` with
    /// two empty branches becomes its test, a constant test selects one branch, and an empty
    /// `else` is dropped.
    // Port of: src/sksl/ir/SkSLIfStatement.cpp#L52-L95 (chrome/m156)
    #[must_use]
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        if_true: StmtId,
        if_false: Option<StmtId>,
    ) -> StmtId {
        let optimize = ctx.config().settings.optimize;
        let mut true_is_empty = false;
        let mut false_is_empty = false;
        if optimize {
            // If both sides are empty, the if statement can be reduced to its test expression.
            true_is_empty = ctx.pool.statement(if_true).is_empty(&ctx.pool);
            false_is_empty = if_false.is_none_or(|f| ctx.pool.statement(f).is_empty(&ctx.pool));
            if true_is_empty && false_is_empty {
                return ExpressionStatement::make(ctx, test);
            }
        }
        if optimize {
            // Static Boolean values can fold down to a single branch.
            let test_value = constant_folder::get_constant_value_for_variable(&ctx.pool, test);
            let constant = ctx
                .pool
                .expression(test_value)
                .as_literal()
                .filter(|_| ctx.pool.expression(test_value).is_bool_literal(&ctx.pool))
                .map(|literal| literal.bool_value());
            if let Some(value) = constant {
                return if value {
                    replace_empty_with_nop(&mut ctx.pool, Some(if_true), true_is_empty)
                } else {
                    replace_empty_with_nop(&mut ctx.pool, if_false, false_is_empty)
                };
            }
        }
        let mut if_true = if_true;
        let mut if_false = if_false;
        if optimize {
            // Replace an empty if-true branches with Nop; eliminate empty if-false branches
            // entirely.
            if_true = replace_empty_with_nop(&mut ctx.pool, Some(if_true), true_is_empty);
            if false_is_empty {
                if_false = None;
            }
        }
        ctx.pool.add_statement(Statement::new(
            pos,
            StatementKind::If(Self {
                test,
                if_true,
                if_false,
            }),
        ))
    }
}
