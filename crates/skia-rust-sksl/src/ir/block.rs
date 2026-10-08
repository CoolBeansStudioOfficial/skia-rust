// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBlock.{h,cpp} (data, `isEmpty`, `description`, `Make`,
// `MakeBlock` and `MakeCompoundStatement`).

//! [`Block`]: a list of statements, braced or not.

use super::{
    IrPool, Nop, Statement, StatementKind,
    ids::{StmtId, SymTabId},
};
use crate::position::Position;

/// `Block::Kind`.
#[doc(alias = "Block::Kind")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// A group of statements without curly braces.
    UnbracedBlock,
    /// A language-level block, with curly braces.
    #[default]
    BracedScope,
    /// A block that stands for a single statement, such as `int a, b;`.
    CompoundStatement,
}

/// `SkSL::Block`.
// Port of: src/sksl/ir/SkSLBlock.h#L24-L113 (chrome/m156)
#[doc(alias = "SkSL::Block")]
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// `children()`.
    pub children: Vec<StmtId>,
    /// `blockKind()`.
    pub block_kind: BlockKind,
    /// `symbolTable()`: the scope this block opens, if any.
    pub symbol_table: Option<SymTabId>,
}

impl Block {
    /// `Block::Make`: a statement of `statements`. The block may be simplified: a scope or a block
    /// with symbols is kept, an empty unscoped block becomes a `Nop`, and an unscoped block with a
    /// single non-empty statement becomes that statement. The result is a statement id, which may
    /// name a child of `statements`.
    // Port of: src/sksl/ir/SkSLBlock.cpp#L14-L56 (chrome/m156)
    #[must_use]
    pub fn make(
        pool: &mut IrPool,
        pos: Position,
        statements: Vec<StmtId>,
        kind: BlockKind,
        symbol_table: Option<SymTabId>,
    ) -> StmtId {
        // We can't simplify away braces or populated symbol tables.
        let populated = symbol_table.is_some_and(|table| pool.symbol_table(table).count() > 0);
        if kind == BlockKind::BracedScope || populated {
            return Self::make_block(pool, pos, statements, kind, symbol_table);
        }
        // If the Block is completely empty, synthesize a Nop.
        if statements.is_empty() {
            return Nop::make(pool);
        }
        if statements.len() > 1 {
            // The statement array contains multiple statements, but some of those might be
            // no-ops. If the statement array only contains one real statement, we can return that
            // directly and avoid creating an additional Block node.
            let mut found: Option<StmtId> = None;
            for &stmt in &statements {
                if !pool.statement(stmt).is_empty(pool) {
                    if found.is_none() {
                        // We found a single non-empty statement. Remember it and keep looking.
                        found = Some(stmt);
                        continue;
                    }
                    // We found more than one non-empty statement. We actually do need a Block.
                    return Self::make_block(pool, pos, statements, kind, None);
                }
            }
            // The array wrapped one valid Statement. Avoid allocating a Block by returning it
            // directly.
            if let Some(found) = found {
                return found;
            }
            // The statement array contained nothing but empty statements! In this case, we don't
            // actually need to allocate a Block. We can just return one of those empty statements.
        }
        statements[0]
    }

    /// `Block::MakeBlock`: always a real `Block` statement (its id is returned as a statement).
    /// Many callers rely on the result being a `Block`, such as a function body.
    // Port of: src/sksl/ir/SkSLBlock.cpp#L58-L66 (chrome/m156)
    #[must_use]
    pub fn make_block(
        pool: &mut IrPool,
        pos: Position,
        children: Vec<StmtId>,
        kind: BlockKind,
        symbol_table: Option<SymTabId>,
    ) -> StmtId {
        // Nothing to optimize here--eliminating empty statements doesn't actually improve the
        // generated code, and we promise to return a Block.
        pool.add_statement(Statement::new(
            pos,
            StatementKind::Block(Block {
                children,
                block_kind: kind,
                symbol_table,
            }),
        ))
    }

    /// `Block::MakeCompoundStatement`: wraps two statements into one compound-statement block. An
    /// empty or missing statement is dropped, and a compound block as `existing` takes
    /// `additional` as a new child.
    // Port of: src/sksl/ir/SkSLBlock.cpp#L68-L95 (chrome/m156)
    #[must_use]
    pub fn make_compound_statement(
        pool: &mut IrPool,
        existing: Option<StmtId>,
        additional: Option<StmtId>,
    ) -> Option<StmtId> {
        // If either of the two Statements is empty, return the other.
        let existing = match existing {
            Some(existing) if !pool.statement(existing).is_empty(pool) => existing,
            _ => return additional,
        };
        let additional = match additional {
            Some(additional) if !pool.statement(additional).is_empty(pool) => additional,
            _ => return Some(existing),
        };
        // If the existing statement is a compound-statement Block, append the additional
        // statement.
        if let StatementKind::Block(block) = &mut pool.statement_mut(existing).kind
            && block.block_kind == BlockKind::CompoundStatement
        {
            block.children.push(additional);
            return Some(existing);
        }
        // The existing statement was not a compound-statement Block; create one, and put both
        // statements inside of it.
        let pos = pool
            .statement(existing)
            .position
            .range_through(pool.statement(additional).position);
        Some(Self::make(
            pool,
            pos,
            vec![existing, additional],
            BlockKind::CompoundStatement,
            None,
        ))
    }

    /// `isScope()`.
    #[must_use]
    pub fn is_scope(&self) -> bool {
        self.block_kind == BlockKind::BracedScope
    }

    /// `isEmpty()`: every child is empty.
    #[must_use]
    pub fn is_empty(&self, pool: &IrPool) -> bool {
        self.children
            .iter()
            .all(|&stmt| pool.statement(stmt).is_empty(pool))
    }

    /// `description()`: one child per line, inside braces for a scope (or an empty block).
    // Port of: src/sksl/ir/SkSLBlock.cpp#L97-L112 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = String::new();
        // Write scope markers if this block is a scope, or if the block is empty (since we need
        // to emit something here to make the code valid).
        let is_scope = self.is_scope() || self.is_empty(pool);
        if is_scope {
            result.push('{');
        }
        for &stmt in &self.children {
            result.push('\n');
            result.push_str(&pool.statement_description(stmt));
        }
        result.push_str(if is_scope { "\n}\n" } else { "\n" });
        result
    }
}
