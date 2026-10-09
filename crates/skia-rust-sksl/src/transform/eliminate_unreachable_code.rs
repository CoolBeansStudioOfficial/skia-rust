// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateUnreachableCode.cpp (chrome/m156).

//! [`eliminate_unreachable_code`]: replaces the statements after a function exit or a loop exit
//! with `Nop`s.

use super::{ProgramWriter, replace_with_nop};
use crate::analysis::ProgramUsage;
use crate::context::Context;
use crate::ir::{ElemId, ExprId, ProgramElementKind, StatementKind, StmtId};

/// `UnreachableCodeEliminator`. Each stack holds one flag per open control-flow region.
struct UnreachableCodeEliminator<'a> {
    usage: &'a mut ProgramUsage,
    /// `fFoundFunctionExit`.
    found_function_exit: Vec<bool>,
    /// `fFoundBlockExit`.
    found_block_exit: Vec<bool>,
}

impl<'a> UnreachableCodeEliminator<'a> {
    // Port of: src/sksl/transform/SkSLEliminateUnreachableCode.cpp#L39-L42 (chrome/m156)
    fn new(usage: &'a mut ProgramUsage) -> Self {
        Self {
            usage,
            found_function_exit: vec![false],
            found_block_exit: vec![false],
        }
    }

    fn top_function_exit(&self) -> bool {
        *self
            .found_function_exit
            .last()
            .expect("the function-exit stack is never empty")
    }

    fn top_block_exit(&self) -> bool {
        *self
            .found_block_exit
            .last()
            .expect("the block-exit stack is never empty")
    }

    fn set_top_function_exit(&mut self, value: bool) {
        *self
            .found_function_exit
            .last_mut()
            .expect("the function-exit stack is never empty") = value;
    }

    fn set_top_block_exit(&mut self, value: bool) {
        *self
            .found_block_exit
            .last_mut()
            .expect("the block-exit stack is never empty") = value;
    }
}

impl ProgramWriter for UnreachableCodeEliminator<'_> {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        // We don't need to look inside expressions at all.
        false
    }

    // Port of: src/sksl/transform/SkSLEliminateUnreachableCode.cpp#L49-L190 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        if self.top_function_exit() || self.top_block_exit() {
            // If we already found an exit in this section, anything beyond it is dead code.
            if !matches!(ctx.pool.statement(stmt).kind, StatementKind::Nop(_)) {
                // Eliminate the dead statement by substituting a Nop.
                self.usage.remove_statement(&ctx.pool, stmt);
                replace_with_nop(&mut ctx.pool, stmt);
            }
            return false;
        }

        // Copies the node's kind: its children are ids, and a block's list is short.
        let kind = ctx.pool.statement(stmt).kind.clone();
        match kind {
            StatementKind::Return(_) | StatementKind::Discard(_) => {
                // We found a function exit on this path.
                self.set_top_function_exit(true);
            }
            // A `break` statement can either be breaking out of a loop or terminating an
            // individual switch case. We treat both cases the same way: they only apply to the
            // statements associated with the parent statement (i.e. enclosing loop block /
            // preceding case label).
            StatementKind::Break(_) | StatementKind::Continue(_) => {
                self.set_top_block_exit(true);
            }
            // These statements don't affect control flow.
            StatementKind::Expression(_)
            | StatementKind::Nop(_)
            | StatementKind::VarDeclaration(_) => {}
            // Blocks are on the straight-line path and don't affect control flow.
            StatementKind::Block(_) => return self.visit_statement(ctx, stmt),
            StatementKind::Do(_) => {
                // Function-exits are allowed to propagate outside of a do-loop, because it always
                // executes its body at least once.
                self.found_block_exit.push(false);
                let result = self.visit_statement(ctx, stmt);
                self.found_block_exit.pop();
                return result;
            }
            StatementKind::For(_) => {
                // Function-exits are not allowed to propagate out, because a for-loop or while-loop
                // could potentially run zero times.
                self.found_function_exit.push(false);
                self.found_block_exit.push(false);
                let result = self.visit_statement(ctx, stmt);
                self.found_block_exit.pop();
                self.found_function_exit.pop();
                return result;
            }
            StatementKind::If(if_stmt) => {
                // This statement is conditional and encloses two inner sections of code. If both
                // sides contain a function-exit or loop-exit, that exit is allowed to propagate
                // out.
                self.found_function_exit.push(false);
                self.found_block_exit.push(false);
                let mut result = self.visit_statement_ptr(ctx, if_stmt.if_true);
                let found_function_exit_on_true = self.found_function_exit.pop().unwrap_or(false);
                let found_loop_exit_on_true = self.found_block_exit.pop().unwrap_or(false);

                self.found_function_exit.push(false);
                self.found_block_exit.push(false);
                if let Some(if_false) = if_stmt.if_false {
                    result |= self.visit_statement_ptr(ctx, if_false);
                }
                let found_function_exit_on_false = self.found_function_exit.pop().unwrap_or(false);
                let found_loop_exit_on_false = self.found_block_exit.pop().unwrap_or(false);

                let function_exit = self.top_function_exit()
                    || (found_function_exit_on_true && found_function_exit_on_false);
                self.set_top_function_exit(function_exit);
                let loop_exit =
                    self.top_block_exit() || (found_loop_exit_on_true && found_loop_exit_on_false);
                self.set_top_block_exit(loop_exit);
                return result;
            }
            StatementKind::Switch(sw) => {
                // In switch statements we consider unreachable code on a per-case basis.
                let mut result = false;

                // Tracks whether we found at least one case that doesn't lead to a return
                // statement (potentially via fallthrough).
                let mut found_case_without_return = false;
                let mut has_default = false;
                for case in sw.cases(&ctx.pool).to_vec() {
                    // We eliminate unreachable code within the statements of the individual case.
                    // Breaks are not allowed to propagate outside the case statement itself.
                    // Function returns are allowed to propagate out only if all cases have a
                    // return AND one of the cases is default (so that we know at least one of the
                    // branches will be taken). This is similar to how we handle if statements
                    // above.
                    self.found_function_exit.push(false);
                    self.found_block_exit.push(false);

                    let case_statement = match &ctx.pool.statement(case).kind {
                        StatementKind::SwitchCase(case_statement) => case_statement.clone(),
                        _ => unreachable!("a switch's cases are switch-case statements"),
                    };
                    result |= self.visit_statement_ptr(ctx, case_statement.statement);

                    // When considering whether a case has a return we can propagate, we assume the
                    // following:
                    //     1. The default case is always placed last in a switch statement and it
                    //        is the last possible label reachable via fallthrough. Thus if it does
                    //        not contain a return statement, then we don't propagate a function
                    //        return.
                    //     2. In all other cases we prevent the return from propagating only if we
                    //        encounter a break statement. If no return or break is found, we defer
                    //        the decision to the fallthrough case. We won't propagate a return
                    //        unless we eventually encounter a default label.
                    //
                    // See resources/sksl/shared/SwitchWithEarlyReturn.sksl for test cases that
                    // exercise this.
                    let function_exit = self.top_function_exit();
                    let block_exit = self.top_block_exit();
                    if case_statement.is_default {
                        found_case_without_return |= !function_exit;
                        has_default = true;
                    } else {
                        // We can only be sure that a case does not lead to a return if it doesn't
                        // fallthrough.
                        found_case_without_return |= !function_exit && block_exit;
                    }

                    self.found_function_exit.pop();
                    self.found_block_exit.pop();
                }

                let function_exit =
                    self.top_function_exit() || (!found_case_without_return && has_default);
                self.set_top_function_exit(function_exit);
                return result;
            }
            StatementKind::SwitchCase(_) => {
                // We should never hit this case as switch cases are handled in the previous case.
                unreachable!("switch cases are handled by their switch statement");
            }
        }

        false
    }
}

/// `Transform::EliminateUnreachableCode(Module&, ProgramUsage*)` and
/// `Transform::EliminateUnreachableCode(Program&)`: the function bodies among `elements` lose their
/// unreachable statements. `usage` is updated for every statement removed.
// Port of: src/sksl/transform/SkSLEliminateUnreachableCode.cpp#L35-L214 (chrome/m156)
#[doc(alias = "SkSL::Transform::EliminateUnreachableCode")]
pub fn eliminate_unreachable_code(
    ctx: &mut Context,
    elements: &[ElemId],
    usage: &mut ProgramUsage,
) {
    for &element in elements {
        let body = match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => def.body,
            _ => continue,
        };
        let mut eliminator = UnreachableCodeEliminator::new(usage);
        eliminator.visit_statement_ptr(ctx, body);
    }
}
