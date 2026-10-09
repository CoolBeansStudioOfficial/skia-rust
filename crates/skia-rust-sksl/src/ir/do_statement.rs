// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLDoStatement.{h,cpp} (`Convert` and `Make`).

//! The factories of [`DoStatement`].

use super::{
    DoStatement, IrPool, Statement, StatementKind, TypeId,
    ids::{ExprId, StmtId},
};
use crate::analysis;
use crate::context::Context;
use crate::position::Position;

impl DoStatement {
    /// `DoStatement::Convert`: checks the test (coerced to `bool`) and the body (which must be
    /// scoped), and rejects `do`-`while` in strict ES2.
    // Port of: src/sksl/ir/SkSLDoStatement.cpp#L14-L36 (chrome/m156)
    #[must_use]
    pub fn convert(ctx: &mut Context, pos: Position, stmt: StmtId, test: ExprId) -> Option<StmtId> {
        if ctx.config().strict_es2_mode() {
            ctx.errors.error(pos, "do-while loops are not supported");
            return None;
        }
        let test = TypeId::BOOL.coerce_expression(ctx, test)?;
        if analysis::detect_var_declaration_without_scope(&ctx.pool, stmt, Some(&mut ctx.errors)) {
            return None;
        }
        Some(Self::make(&mut ctx.pool, pos, stmt, test))
    }

    /// `DoStatement::Make`: the caller has checked the test and the body.
    // Port of: src/sksl/ir/SkSLDoStatement.cpp#L38-L46 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position, stmt: StmtId, test: ExprId) -> StmtId {
        pool.add_statement(Statement::new(
            pos,
            StatementKind::Do(Self {
                statement: stmt,
                test,
            }),
        ))
    }
}
