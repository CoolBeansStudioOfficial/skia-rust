// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLForStatement.{h,cpp} (`Convert`, `ConvertWhile` and `Make`).
// The `SK_BUILD_FOR_FUZZER` iteration limit is not ported: it is a fuzzer-only build option.

//! The factories of [`ForStatement`].

use super::{
    Block, BlockKind, Expression, ForStatement, IrPool, LoopUnrollInfo, Nop, Statement,
    StatementKind, TypeId,
    ids::{ExprId, StmtId, SymTabId, SymbolId},
    insert_new_parent, move_symbol_to,
};
use crate::analysis;
use crate::context::Context;
use crate::position::{ForLoopPositions, Position};

/// `is_vardecl_block_initializer`: an unscoped block that holds only variable declarations.
// Port of: src/sksl/ir/SkSLForStatement.cpp#L24-L36 (chrome/m156)
fn is_vardecl_block_initializer(pool: &IrPool, stmt: Option<StmtId>) -> bool {
    let Some(stmt) = stmt else {
        return false;
    };
    let StatementKind::Block(block) = &pool.statement(stmt).kind else {
        return false;
    };
    if block.is_scope() {
        return false;
    }
    block
        .children
        .iter()
        .all(|&child| matches!(pool.statement(child).kind, StatementKind::VarDeclaration(_)))
}

/// `is_simple_initializer`: no initializer, an empty one, a declaration or an expression.
// Port of: src/sksl/ir/SkSLForStatement.cpp#L38-L41 (chrome/m156)
fn is_simple_initializer(pool: &IrPool, stmt: Option<StmtId>) -> bool {
    stmt.is_none_or(|stmt| {
        let statement = pool.statement(stmt);
        statement.is_empty(pool)
            || matches!(
                statement.kind,
                StatementKind::VarDeclaration(_) | StatementKind::Expression(_)
            )
    })
}

impl ForStatement {
    /// `ForStatement::Convert`: checks the parts of a `for` loop, and in strict ES2 requires an
    /// unrollable loop. A block initializer of several declarations is hoisted into a scope around
    /// the loop.
    ///
    /// # Panics
    ///
    /// If a block initializer has no symbol table (the parser always gives one).
    // Port of: src/sksl/ir/SkSLForStatement.cpp#L113-L183 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Convert` parameter list, in order.
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        positions: ForLoopPositions,
        initializer: Option<StmtId>,
        test: Option<ExprId>,
        next: Option<ExprId>,
        statement: StmtId,
        symbol_table: Option<SymTabId>,
    ) -> Option<StmtId> {
        let is_simple = is_simple_initializer(&ctx.pool, initializer);
        let is_vardecl_block = !is_simple && is_vardecl_block_initializer(&ctx.pool, initializer);
        if !is_simple && !is_vardecl_block {
            // A non-simple initializer is always present.
            if let Some(initializer) = initializer {
                let init_pos = ctx.pool.statement(initializer).position;
                ctx.errors.error(init_pos, "invalid for loop initializer");
            }
            return None;
        }
        let test = match test {
            Some(test) => Some(TypeId::BOOL.coerce_expression(ctx, test)?),
            None => None,
        };
        // The type of the next-expression doesn't matter, but it needs to be a complete
        // expression. Report an error on intermediate expressions like FunctionReference or
        // TypeReference.
        if let Some(next) = next
            && Expression::is_incomplete(ctx, next)
        {
            return None;
        }
        let mut test = test;
        // In strict-ES2, loops must be unrollable or it's an error. In ES3, loops don't have to be
        // unrollable, but we can use the unroll information for optimization purposes, and its
        // errors are dropped.
        let strict = ctx.config().strict_es2_mode();
        let unroll_info = analysis::get_loop_unroll_info(
            ctx,
            pos,
            &positions,
            initializer,
            &mut test,
            next,
            statement,
            strict,
        );
        if strict && unroll_info.is_none() {
            return None;
        }
        if analysis::detect_var_declaration_without_scope(
            &ctx.pool,
            statement,
            Some(&mut ctx.errors),
        ) {
            return None;
        }
        if is_vardecl_block {
            // If the initializer statement of a for loop contains multiple variables, this causes
            // difficulties for several of our backends; e.g. Metal doesn't have a way to express
            // arrays of different size in the same decl-stmt, because the array-size is part of
            // the type. It's conceptually equivalent to synthesize a scope, declare the variables,
            // and then emit a for statement with an empty init-stmt. (Note that we can't just do
            // this transformation unilaterally for all for-statements, because the resulting for
            // loop isn't ES2-compliant.)
            let initializer = initializer.expect("a block initializer is present");
            let inner = symbol_table.expect("a for loop with a block initializer has symbols");
            let hoisted = insert_new_parent(&mut ctx.pool, inner);
            hoist_vardecl_symbols_into_outer_scope(ctx, initializer, inner, hoisted);
            let loop_stmt = Self::make(
                &mut ctx.pool,
                pos,
                positions,
                None,
                test,
                next,
                statement,
                unroll_info,
                Some(inner),
            );
            let scope = vec![initializer, loop_stmt];
            return Some(Block::make(
                &mut ctx.pool,
                pos,
                scope,
                BlockKind::BracedScope,
                Some(hoisted),
            ));
        }
        Some(Self::make(
            &mut ctx.pool,
            pos,
            positions,
            initializer,
            test,
            next,
            statement,
            unroll_info,
            symbol_table,
        ))
    }

    /// `ForStatement::ConvertWhile`: a `while` loop is a `for` loop without an initializer or
    /// next expression. Strict ES2 rejects it.
    // Port of: src/sksl/ir/SkSLForStatement.cpp#L185-L199 (chrome/m156)
    #[must_use]
    pub fn convert_while(
        ctx: &mut Context,
        pos: Position,
        test: ExprId,
        statement: StmtId,
    ) -> Option<StmtId> {
        if ctx.config().strict_es2_mode() {
            ctx.errors.error(pos, "while loops are not supported");
            return None;
        }
        Self::convert(
            ctx,
            pos,
            ForLoopPositions::default(),
            None,
            Some(test),
            None,
            statement,
            None,
        )
    }

    /// `ForStatement::Make`: a `for` statement. An unrollable loop that runs zero times, or has an
    /// empty body, becomes a `Nop`.
    // Port of: src/sksl/ir/SkSLForStatement.cpp#L201-L227 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Make` parameter list, in order.
    #[must_use]
    pub fn make(
        pool: &mut IrPool,
        pos: Position,
        positions: ForLoopPositions,
        initializer: Option<StmtId>,
        test: Option<ExprId>,
        next: Option<ExprId>,
        statement: StmtId,
        unroll_info: Option<LoopUnrollInfo>,
        symbol_table: Option<SymTabId>,
    ) -> StmtId {
        // Unrollable loops are easy to optimize because we know initializer, test and next don't
        // have interesting side effects.
        if let Some(info) = unroll_info
            && (info.count <= 0 || pool.statement(statement).is_empty(pool))
        {
            // A zero-iteration unrollable loop can be replaced with Nop. An unrollable loop with
            // an empty body can be replaced with Nop.
            return Nop::make(pool);
        }
        pool.add_statement(Statement::new(
            pos,
            StatementKind::For(Self {
                for_loop_positions: positions,
                symbol_table,
                initializer,
                test,
                next,
                statement,
                unroll_info,
            }),
        ))
    }
}

/// `hoist_vardecl_symbols_into_outer_scope`: moves the symbols of the declarations in the block
/// initializer from the loop's table into the hoisted table. The initializer is only declarations.
// Port of: src/sksl/ir/SkSLForStatement.cpp#L43-L70 (chrome/m156)
fn hoist_vardecl_symbols_into_outer_scope(
    ctx: &mut Context,
    init_block: StmtId,
    inner: SymTabId,
    hoisted: SymTabId,
) {
    let children = match &ctx.pool.statement(init_block).kind {
        StatementKind::Block(block) => block.children.clone(),
        _ => return,
    };
    for child in children {
        let var = match &ctx.pool.statement(child).kind {
            StatementKind::VarDeclaration(decl) => decl.var,
            _ => continue,
        };
        // Hoist the variable's symbol outside of the initializer block's symbol table, and into
        // the outer symbol table.
        move_symbol_to(ctx, inner, hoisted, SymbolId::Variable(var));
    }
}
