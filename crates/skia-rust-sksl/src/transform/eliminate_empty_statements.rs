// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateEmptyStatements.cpp (chrome/m156).

//! [`eliminate_empty_statements`]: removes the empty statements from the blocks of a module.

use super::ProgramWriter;
use crate::context::Context;
use crate::ir::{ElemId, ExprId, ProgramElementKind, StatementKind, StmtId};

/// `EmptyStatementEliminator`.
struct EmptyStatementEliminator;

impl ProgramWriter for EmptyStatementEliminator {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        // We don't need to look inside expressions at all.
        false
    }

    // Port of: src/sksl/transform/SkSLEliminateEmptyStatements.cpp#L35-L49 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        // Work from the innermost blocks to the outermost.
        self.visit_statement(ctx, stmt);

        if let StatementKind::Block(block) = &ctx.pool.statement(stmt).kind {
            let kept: Vec<StmtId> = block
                .children
                .iter()
                .copied()
                .filter(|&child| !ctx.pool.statement(child).is_empty(&ctx.pool))
                .collect();
            if let StatementKind::Block(block) = &mut ctx.pool.statement_mut(stmt).kind {
                block.children = kept;
            }
        }

        // We always check the entire program.
        false
    }
}

/// `Transform::EliminateEmptyStatements(Module&)`: removes the empty statements from the blocks of
/// the function bodies in `elements`.
// Port of: src/sksl/transform/SkSLEliminateEmptyStatements.cpp#L27-L65 (chrome/m156)
#[doc(alias = "SkSL::Transform::EliminateEmptyStatements")]
pub fn eliminate_empty_statements(ctx: &mut Context, elements: &[ElemId]) {
    for &element in elements {
        let body = match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => def.body,
            _ => continue,
        };
        let mut eliminator = EmptyStatementEliminator;
        eliminator.visit_statement_ptr(ctx, body);
    }
}
