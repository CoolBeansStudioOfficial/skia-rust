// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLBlock.{h,cpp} (data, `isEmpty` and `description`). `Make`,
// `MakeBlock` and `MakeCompoundStatement` come with task S7d.

//! [`Block`]: a list of statements, braced or not.

use super::{
    IrPool,
    ids::{StmtId, SymTabId},
};

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
