// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp and
// src/sksl/analysis/SkSLCheckSymbolTableCorrectness.cpp (chrome/m156).

//! [`SymbolTableStackBuilder`], which keeps the stack of scopes that encloses a statement, and
//! [`check_symbol_table_correctness`], which checks that each variable is declared in the scope
//! it belongs to (Skia runs it only in debug builds).

use super::{ProgramVisitor, walk_statement};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::ir::{ElemId, ExprId, IrPool, StatementKind, StmtId, SymTabId, SymbolId};

/// `SymbolTableStackBuilder`: pushes the symbol table a statement opens, if any, onto a stack of
/// scopes. Skia pops it in the destructor; Rust has no destructor that can borrow the stack, so
/// the caller calls [`SymbolTableStackBuilder::pop`] once the statement has been visited.
// Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L22-L51 (chrome/m156)
#[doc(alias = "SkSL::Analysis::SymbolTableStackBuilder")]
#[must_use = "call `pop` once the statement has been visited"]
#[derive(Debug)]
pub struct SymbolTableStackBuilder {
    pushed: bool,
}

impl SymbolTableStackBuilder {
    /// Pushes the scope that `stmt` opens (a `Block` or a `for` loop), if it opens one.
    // Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L22-L44 (chrome/m156)
    pub fn new(pool: &IrPool, stmt: Option<StmtId>, stack: &mut Vec<SymTabId>) -> Self {
        let scope = stmt.and_then(|stmt| match &pool.statement(stmt).kind {
            StatementKind::Block(block) => block.symbol_table,
            StatementKind::For(for_stmt) => for_stmt.symbol_table,
            _ => None,
        });
        match scope {
            Some(symbols) => {
                stack.push(symbols);
                Self { pushed: true }
            }
            None => Self { pushed: false },
        }
    }

    /// The destructor of the C++ class: pops the scope this builder pushed.
    // Port of: src/sksl/analysis/SkSLSymbolTableStackBuilder.cpp#L46-L50 (chrome/m156)
    pub fn pop(self, stack: &mut Vec<SymTabId>) {
        if self.pushed {
            stack.pop();
        }
    }
}

/// `SymbolTableCorrectnessVisitor`.
struct SymbolTableCorrectnessVisitor<'a> {
    errors: &'a mut ErrorReporter,
    symbol_table_stack: Vec<SymTabId>,
}

impl ProgramVisitor for SymbolTableCorrectnessVisitor<'_> {
    // Port of: src/sksl/analysis/SkSLCheckSymbolTableCorrectness.cpp#L33-L63 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        let builder = SymbolTableStackBuilder::new(pool, Some(stmt), &mut self.symbol_table_stack);
        if let StatementKind::VarDeclaration(vardecl) = &pool.statement(stmt).kind {
            // Check the top of the stack for this exact variable. Skia iterates the table's
            // contents and compares pointers, so this compares the symbol ids the table maps.
            let top = self
                .symbol_table_stack
                .last()
                .copied()
                .expect("the symbol table stack holds the program's table");
            let table = pool.symbol_table(top);
            let var_symbol = SymbolId::Variable(vardecl.var);
            let contains_symbol = table
                .names()
                .any(|name| table.find_local(name) == Some(var_symbol));
            if !contains_symbol {
                self.errors.error(
                    pool.statement(stmt).position,
                    &format!(
                        "internal error (variable '{}' is incorrectly scoped)",
                        pool.variable(vardecl.var).name
                    ),
                );
            }
        }
        let result = walk_statement(self, pool, stmt);
        builder.pop(&mut self.symbol_table_stack);
        result
    }

    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        false
    }
}

/// `Analysis::CheckSymbolTableCorrectness`: reports every variable declared outside the symbol
/// table of its scope. `symbols` is the program's top-level table and `owned_elements` are the
/// program's own elements. Skia calls this only in debug builds.
// Port of: src/sksl/analysis/SkSLCheckSymbolTableCorrectness.cpp#L33-L85 (chrome/m156)
pub fn check_symbol_table_correctness(
    ctx: &mut Context,
    symbols: SymTabId,
    owned_elements: &[ElemId],
) {
    let pool: &IrPool = &ctx.pool;
    let mut visitor = SymbolTableCorrectnessVisitor {
        errors: &mut ctx.errors,
        symbol_table_stack: vec![symbols],
    };
    for &element in owned_elements {
        visitor.visit_program_element(pool, element);
    }
}
