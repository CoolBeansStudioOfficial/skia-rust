// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateUnnecessaryBraces.cpp (chrome/m156).

//! [`eliminate_unnecessary_braces`]: drops the braces around single-statement child blocks, and
//! puts back the braces an `else` needs to bind to the right `if`.

use super::ProgramWriter;
use crate::context::Context;
use crate::ir::{
    Block, BlockKind, ElemId, ExprId, IfStatement, Nop, ProgramElementKind, StatementKind, StmtId,
};

/// `eliminate_braces_from`: the statement that replaces `stmt` once its braces are removed. A block
/// with one useful statement becomes that statement; a block with none becomes a `Nop`; any other
/// block (and any other statement) is kept as it is.
// Port of: src/sksl/transform/SkSLEliminateUnnecessaryBraces.cpp#L69-L98 (chrome/m156)
fn eliminate_braces_from(ctx: &mut Context, stmt: StmtId) -> StmtId {
    let StatementKind::Block(block) = &ctx.pool.statement(stmt).kind else {
        return stmt;
    };
    let children = block.children.clone();
    let mut useful: Option<StmtId> = None;
    for child in children {
        if ctx.pool.statement(child).is_empty(&ctx.pool) {
            continue;
        }
        if useful.is_some() {
            // We found two non-empty statements. We can't eliminate braces from this block.
            return stmt;
        }
        // We found one non-empty statement.
        useful = Some(child);
    }

    match useful {
        // This block held zero useful statements. Replace the block with a nop.
        None => Nop::make(&mut ctx.pool),
        // This block held one useful statement. Replace the block with that statement.
        Some(child) => child,
    }
}

/// `UnnecessaryBraceEliminator`: the first pass, which works from the innermost blocks outward.
struct UnnecessaryBraceEliminator;

impl ProgramWriter for UnnecessaryBraceEliminator {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        // We don't need to look inside expressions at all.
        false
    }

    // Port of: src/sksl/transform/SkSLEliminateUnnecessaryBraces.cpp#L40-L68 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        // Work from the innermost blocks to the outermost.
        self.visit_statement(ctx, stmt);

        // The child slots are written back into the parent node, so that each child keeps its id.
        match ctx.pool.statement(stmt).kind {
            StatementKind::If(IfStatement {
                if_true, if_false, ..
            }) => {
                let if_true = eliminate_braces_from(ctx, if_true);
                let if_false = if_false.map(|child| eliminate_braces_from(ctx, child));
                if let StatementKind::If(if_stmt) = &mut ctx.pool.statement_mut(stmt).kind {
                    if_stmt.if_true = if_true;
                    if_stmt.if_false = if_false;
                }
            }
            StatementKind::For(_) => {
                let StatementKind::For(for_stmt) = &ctx.pool.statement(stmt).kind else {
                    unreachable!()
                };
                let body = eliminate_braces_from(ctx, for_stmt.statement);
                if let StatementKind::For(for_stmt) = &mut ctx.pool.statement_mut(stmt).kind {
                    for_stmt.statement = body;
                }
            }
            StatementKind::Do(_) => {
                let StatementKind::Do(do_stmt) = &ctx.pool.statement(stmt).kind else {
                    unreachable!()
                };
                let body = eliminate_braces_from(ctx, do_stmt.statement);
                if let StatementKind::Do(do_stmt) = &mut ctx.pool.statement_mut(stmt).kind {
                    do_stmt.statement = body;
                }
            }
            _ => {}
        }

        // We always check the entire program.
        false
    }
}

/// `RequiredBraceWriter`: the second pass. The first pass can remove so many braces that an `else`
/// binds to the wrong `if`, so this pass wraps the outer `if`'s true clause in braces when it holds
/// an `if` without an `else`.
struct RequiredBraceWriter;

impl ProgramWriter for RequiredBraceWriter {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        // We don't need to look inside expressions at all.
        false
    }

    // Port of: src/sksl/transform/SkSLEliminateUnnecessaryBraces.cpp#L109-L168 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        // Look for the following structure:
        //
        //    if (...)
        //      if (...)
        //        any statement;
        //    else
        //      any statement;
        //
        // This structure isn't correct if we emit it textually, because the else-clause would be
        // interpreted as if it were bound to the inner if-statement, like this:
        //
        //    if (...) {
        //      if (...)
        //        any statement;
        //      else
        //        any statement;
        //    }
        //
        // If we find such a structure, we must disambiguate the else-clause by adding braces:
        //    if (...) {
        //      if (...)
        //        any statement;
        //    } else
        //      any statement;

        // Work from the innermost blocks to the outermost.
        self.visit_statement(ctx, stmt);

        // We are looking for an if-statement that has an else clause, and that directly wraps
        // another if-statement (no Block) that has no else clause.
        let StatementKind::If(outer) = &ctx.pool.statement(stmt).kind else {
            return false;
        };
        let (test, if_true, if_false) = (outer.test, outer.if_true, outer.if_false);
        let outer_position = ctx.pool.statement(stmt).position;
        let Some(if_false) = if_false else {
            return false;
        };
        let StatementKind::If(inner) = &ctx.pool.statement(if_true).kind else {
            return false;
        };
        if inner.if_false.is_some() {
            return false;
        }

        // This structure is ambiguous; the else clause on the outer if-statement will bind to the
        // inner if-statement if we don't add braces. We must wrap the outer if-statement's
        // true-clause in braces.
        let stmt_position = ctx.pool.statement(if_true).position;
        let braced_if_true = Block::make_block(
            &mut ctx.pool,
            stmt_position,
            vec![if_true],
            BlockKind::BracedScope,
            None,
        );
        let replacement =
            IfStatement::make(ctx, outer_position, test, braced_if_true, Some(if_false));
        // `stmt = IfStatement::Make(...)`: the outer node takes the replacement's contents.
        ctx.pool.move_statement_into(stmt, replacement);

        // We always check the entire program.
        false
    }
}

/// `Transform::EliminateUnnecessaryBraces(const Context&, Module&)`: removes the braces around the
/// single-statement children of `if`, `for` and `do` statements in the function bodies of
/// `elements`, then restores the braces an `else` needs.
// Port of: src/sksl/transform/SkSLEliminateUnnecessaryBraces.cpp#L32-L186 (chrome/m156)
#[doc(alias = "SkSL::Transform::EliminateUnnecessaryBraces")]
pub fn eliminate_unnecessary_braces(ctx: &mut Context, elements: &[ElemId]) {
    for &element in elements {
        let body = match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => def.body,
            _ => continue,
        };
        // First, we eliminate braces around single-statement child blocks wherever possible.
        UnnecessaryBraceEliminator.visit_statement_ptr(ctx, body);

        // The first pass can be overzealous, since it can remove so many braces that else-clauses
        // are bound to the wrong if-statement. Search for this case and fix it up if we find it.
        RequiredBraceWriter.visit_statement_ptr(ctx, body);
    }
}
