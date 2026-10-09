// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp.

//! [`SymbolTableStackBuilder`]: keeps a symbol-table stack in step with a visit.

use crate::ir::{IrPool, StatementKind, StmtId, SymTabId};

/// `Analysis::SymbolTableStackBuilder`: when a statement holds a symbol table (a block or a `for`
/// loop), pushes it onto the stack for the visit of that statement.
///
/// Skia pops in the destructor. The stack is also used by the visitor while the builder is alive,
/// so a Rust guard cannot borrow it. Call [`finish`](Self::finish) with the same stack when the
/// statement is done.
// Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L22-L50 (chrome/m156)
#[doc(alias = "SkSL::Analysis::SymbolTableStackBuilder")]
#[derive(Debug)]
#[must_use = "call `finish` with the same stack once the statement has been visited"]
pub struct SymbolTableStackBuilder {
    pushed: bool,
}

impl SymbolTableStackBuilder {
    /// If `stmt` holds a symbol table, pushes it onto `stack`. `stmt` may be absent.
    // Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L22-L44 (chrome/m156)
    pub fn new(pool: &IrPool, stmt: Option<StmtId>, stack: &mut Vec<SymTabId>) -> Self {
        let table = stmt.and_then(|stmt| match &pool.statement(stmt).kind {
            StatementKind::Block(block) => block.symbol_table,
            StatementKind::For(for_stmt) => for_stmt.symbol_table,
            _ => None,
        });
        let pushed = match table {
            Some(table) => {
                stack.push(table);
                true
            }
            None => false,
        };
        Self { pushed }
    }

    /// `foundSymbolTable()`: true if an entry was added to the stack.
    #[must_use]
    pub fn found_symbol_table(&self) -> bool {
        self.pushed
    }

    /// The destructor's pop: removes the entry this builder pushed, if any.
    // Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L46-L50 (chrome/m156)
    pub fn finish(self, stack: &mut Vec<SymTabId>) {
        if self.pushed {
            stack.pop();
        }
    }
}
