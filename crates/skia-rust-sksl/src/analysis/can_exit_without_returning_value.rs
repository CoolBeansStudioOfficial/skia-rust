// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLCanExitWithoutReturningValue.cpp (chrome/m156).

//! [`can_exit_without_returning_value`]: whether a non-void function can reach its end without
//! a `return` on every path.

use super::{ProgramVisitor, walk_statement};
use crate::ir::{FnId, IrPool, StatementKind, StmtId};

/// `ReturnsOnAllPathsVisitor`: records which exits a statement definitely contains.
#[derive(Default)]
struct ReturnsOnAllPathsVisitor {
    found_return: bool,
    found_break: bool,
    found_continue: bool,
}

impl ProgramVisitor for ReturnsOnAllPathsVisitor {
    fn visit_expression(&mut self, _pool: &IrPool, _expr: crate::ir::ExprId) -> bool {
        // We can avoid processing expressions entirely.
        false
    }

    // Port of: src/sksl/analysis/SkSLCanExitWithoutReturningValue.cpp#L22-L115 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            // Returns, breaks, or continues stop the scan, so only one of these is ever true.
            StatementKind::Return(_) => {
                self.found_return = true;
                true
            }
            StatementKind::Break(_) => {
                self.found_break = true;
                true
            }
            StatementKind::Continue(_) => {
                self.found_continue = true;
                true
            }
            StatementKind::If(i) => {
                let mut true_visitor = Self::default();
                let mut false_visitor = Self::default();
                true_visitor.visit_statement(pool, i.if_true);
                if let Some(if_false) = i.if_false {
                    false_visitor.visit_statement(pool, if_false);
                }
                // If either branch leads to a break or continue, the whole `if` contains one,
                // since we don't know which side will be reached.
                self.found_break = true_visitor.found_break || false_visitor.found_break;
                self.found_continue = true_visitor.found_continue || false_visitor.found_continue;
                // Only returns that definitely happen count, so both sides must return.
                self.found_return = true_visitor.found_return && false_visitor.found_return;
                self.found_break || self.found_continue || self.found_return
            }
            StatementKind::For(f) => {
                // A for loop is assumed to run at least once. A break or continue inside it only
                // exits the loop, so only a return is kept.
                let mut for_visitor = Self::default();
                for_visitor.visit_statement(pool, f.statement);
                self.found_return = for_visitor.found_return;
                self.found_return
            }
            StatementKind::Do(d) => {
                // A do-while block is always entered at least once. As for `for`, only a return
                // is kept.
                let mut do_visitor = Self::default();
                do_visitor.visit_statement(pool, d.statement);
                self.found_return = do_visitor.found_return;
                self.found_return
            }
            // Blocks are definitely entered and add no control flow of their own.
            StatementKind::Block(_) | StatementKind::SwitchCase(_) => {
                walk_statement(self, pool, stmt)
            }
            StatementKind::Switch(s) => {
                // A switch needs a default case, and every case must either return
                // unconditionally or fall through to a case that does.
                let mut found_default = false;
                let mut fell_through = false;
                for &case in s.cases(pool) {
                    let StatementKind::SwitchCase(sc) = &pool.statement(case).kind else {
                        unreachable!("a switch's case block holds only SwitchCase statements");
                    };
                    // A default case is marked as such; a switch without one cannot definitely
                    // return, as its value might not be in the cases list.
                    if sc.is_default {
                        found_default = true;
                    }
                    // Scan this case for any exit (break, continue or return).
                    let mut case_visitor = Self::default();
                    case_visitor.visit_statement(pool, case);

                    // A break or continue, conditional or not, means this case cannot be an
                    // unconditional return. Switches absorb breaks but not continues.
                    if case_visitor.found_continue {
                        self.found_continue = true;
                        return false;
                    }
                    if case_visitor.found_break {
                        return false;
                    }
                    // Without a break or continue, the case has fallen through unless it returns
                    // unconditionally. (A conditional return does not count.)
                    fell_through = !case_visitor.found_return;
                }

                // Without a default case, or when the very last case falls through, the switch
                // does not meet the criteria.
                if fell_through || !found_default {
                    return false;
                }

                // Every section either falls through or returns unconditionally.
                self.found_return = true;
                true
            }
            // None of these can contain a return.
            StatementKind::Discard(_)
            | StatementKind::Expression(_)
            | StatementKind::Nop(_)
            | StatementKind::VarDeclaration(_) => false,
        }
    }
}

/// `Analysis::CanExitWithoutReturningValue`: true when `function` returns a value and `body` has
/// a path that reaches its end without a `return`.
// Port of: src/sksl/analysis/SkSLCanExitWithoutReturningValue.cpp#L163-L173 (chrome/m156)
#[must_use]
pub fn can_exit_without_returning_value(pool: &IrPool, function: FnId, body: StmtId) -> bool {
    if pool.ty(pool.function(function).return_type).is_void() {
        return false;
    }
    let mut visitor = ReturnsOnAllPathsVisitor::default();
    visitor.visit_statement(pool, body);
    !visitor.found_return
}
