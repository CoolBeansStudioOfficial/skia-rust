// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLSwitchCaseContainsExit.cpp (chrome/m156).

//! [`switch_case_contains_unconditional_exit`] and [`switch_case_contains_conditional_exit`]:
//! whether a `case` body exits the switch (or the function) on every path, or on some path.

use super::{ProgramVisitor, walk_statement};
use crate::ir::{ExprId, IrPool, StatementKind, StmtId};

/// `SwitchCaseContainsExit`.
struct SwitchCaseContainsExit {
    conditional_exits: bool,
    in_conditional: i32,
    in_loop: i32,
    in_switch: i32,
}

impl SwitchCaseContainsExit {
    fn new(conditional_exits: bool) -> Self {
        Self {
            conditional_exits,
            in_conditional: 0,
            in_loop: 0,
            in_switch: 0,
        }
    }

    /// `fConditionalExits ? fInConditional : !fInConditional`.
    fn exit_is_at_this_level(&self) -> bool {
        if self.conditional_exits {
            self.in_conditional != 0
        } else {
            self.in_conditional == 0
        }
    }
}

impl ProgramVisitor for SwitchCaseContainsExit {
    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        // We can avoid processing expressions entirely.
        false
    }

    // Port of: src/sksl/analysis/SkSLSwitchCaseContainsExit.cpp#L23-L75 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            StatementKind::Block(_) | StatementKind::SwitchCase(_) => {
                walk_statement(self, pool, stmt)
            }
            // Returns are an early exit regardless of the surrounding control structures.
            StatementKind::Return(_) => self.exit_is_at_this_level(),
            // Continues are an early exit from switches, but not loops.
            StatementKind::Continue(_) => self.in_loop == 0 && self.exit_is_at_this_level(),
            // Breaks cannot escape from switches or loops.
            StatementKind::Break(_) => {
                self.in_loop == 0 && self.in_switch == 0 && self.exit_is_at_this_level()
            }
            StatementKind::If(_) => {
                self.in_conditional += 1;
                let result = walk_statement(self, pool, stmt);
                self.in_conditional -= 1;
                result
            }
            // Loops are conditional, because a loop could run zero times. There is no
            // straightforward way to tell that a loop definitely runs at least once.
            StatementKind::For(_) | StatementKind::Do(_) => {
                self.in_conditional += 1;
                self.in_loop += 1;
                let result = walk_statement(self, pool, stmt);
                self.in_loop -= 1;
                self.in_conditional -= 1;
                result
            }
            StatementKind::Switch(_) => {
                self.in_switch += 1;
                let result = walk_statement(self, pool, stmt);
                self.in_switch -= 1;
                result
            }
            _ => false,
        }
    }
}

/// `Analysis::SwitchCaseContainsUnconditionalExit`: the statement exits on every path.
// Port of: src/sksl/analysis/SkSLSwitchCaseContainsExit.cpp#L90-L92 (chrome/m156)
#[must_use]
pub fn switch_case_contains_unconditional_exit(pool: &IrPool, stmt: StmtId) -> bool {
    let mut visitor = SwitchCaseContainsExit::new(false);
    visitor.visit_statement(pool, stmt)
}

/// `Analysis::SwitchCaseContainsConditionalExit`: the statement exits on some path.
// Port of: src/sksl/analysis/SkSLSwitchCaseContainsExit.cpp#L94-L96 (chrome/m156)
#[must_use]
pub fn switch_case_contains_conditional_exit(pool: &IrPool, stmt: StmtId) -> bool {
    let mut visitor = SwitchCaseContainsExit::new(true);
    visitor.visit_statement(pool, stmt)
}
