// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSwitchStatement.{h,cpp} and src/sksl/ir/SkSLSwitchCase.{h,cpp}
// (`Convert`, `Make`, `MakeDefault`).

//! The factories of [`SwitchStatement`] and [`SwitchCase`].

use std::collections::HashSet;

use super::{
    Block, BlockKind, ExprId, IrPool, Nop, Statement, StatementKind, SwitchCase, SwitchStatement,
    TypeId,
    ids::{StmtId, SymTabId},
};
use crate::analysis;
use crate::constant_folder;
use crate::context::Context;
use crate::defines::SkslInt;
use crate::position::Position;
use crate::transform::{ProgramWriter, hoist_switch_var_declarations_at_top_level};

impl SwitchCase {
    /// `SwitchCase::Make`: `case value: statement`.
    // Port of: src/sksl/ir/SkSLSwitchCase.cpp#L12-L17 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position, value: SkslInt, statement: StmtId) -> StmtId {
        pool.add_statement(Statement::new(
            pos,
            StatementKind::SwitchCase(Self {
                is_default: false,
                value,
                statement,
            }),
        ))
    }

    /// `SwitchCase::MakeDefault`: `default: statement`.
    // Port of: src/sksl/ir/SkSLSwitchCase.cpp#L19-L24 (chrome/m156)
    #[must_use]
    pub fn make_default(pool: &mut IrPool, pos: Position, statement: StmtId) -> StmtId {
        pool.add_statement(Statement::new(
            pos,
            StatementKind::SwitchCase(Self {
                is_default: true,
                value: 0,
                statement,
            }),
        ))
    }
}

/// The `SwitchCase` payload of a statement that is one.
fn switch_case(pool: &IrPool, stmt: StmtId) -> &SwitchCase {
    match &pool.statement(stmt).kind {
        StatementKind::SwitchCase(case) => case,
        _ => panic!("a switch's case block holds only SwitchCase statements"),
    }
}

/// `find_duplicate_case_values`: the cases whose value (or default) repeats an earlier one, in
/// order.
// Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L43-L65 (chrome/m156)
fn find_duplicate_case_values(pool: &IrPool, cases: &[StmtId]) -> Vec<StmtId> {
    let mut duplicate_cases = Vec::new();
    // Only membership is tested here, so the iteration order of a `HashSet` never shows.
    let mut int_values = HashSet::new();
    let mut found_default = false;
    for &stmt in cases {
        let sc = switch_case(pool, stmt);
        if sc.is_default {
            if found_default {
                duplicate_cases.push(stmt);
                continue;
            }
            found_default = true;
        } else if !int_values.insert(sc.value) {
            duplicate_cases.push(stmt);
        }
    }
    duplicate_cases
}

/// `RemoveBreaksWriter`: replaces every `break` (outside of expressions) with a `Nop`.
// Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L67-L82 (chrome/m156)
struct RemoveBreaksWriter;

impl ProgramWriter for RemoveBreaksWriter {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        false
    }

    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        if matches!(ctx.pool.statement(stmt).kind, StatementKind::Break(_)) {
            ctx.pool.replace_statement(
                stmt,
                Statement::new(Position::default(), StatementKind::Nop(Nop)),
            );
            return false;
        }
        self.visit_statement(ctx, stmt)
    }
}

/// `remove_break_statements`.
// Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L67-L82 (chrome/m156)
fn remove_break_statements(ctx: &mut Context, stmt: StmtId) {
    RemoveBreaksWriter.visit_statement_ptr(ctx, stmt);
}

/// `block_for_case`: reduces a switch whose value is known to the cases that run for
/// `case_to_capture` (the matching case, and any cases it falls through to). Returns false, and
/// changes nothing, when a conditional break makes that impossible.
// Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L84-L140 (chrome/m156)
fn block_for_case(ctx: &mut Context, case_block: StmtId, case_to_capture: StmtId) -> bool {
    // This function reduces a switch to the matching case (or cases, if fallthrough occurs) when
    // the switch-value is known and no conditional breaks exist. If conversion is not possible,
    // false is returned and no changes are made. Conversion can fail if the switch contains
    // conditional breaks.
    //
    // We have to be careful to not move any of the pointers until after we're sure we're going to
    // succeed, so before we make any changes at all, we check the switch-cases to decide on a plan
    // of action.
    let cases = match &ctx.pool.statement(case_block).kind {
        StatementKind::Block(block) => block.children.clone(),
        _ => panic!("a switch's case block is a Block"),
    };

    // First, we identify the code that would be run if the switch's value matches
    // `caseToCapture`.
    let start = cases
        .iter()
        .position(|&case| case == case_to_capture)
        .unwrap_or(cases.len());

    // Next, walk forward through the rest of the switch. If we find a conditional break, we're
    // stuck and can't simplify at all. If we find an unconditional break, we have a range of
    // statements that we can use for simplification.
    let mut iter = start;
    let mut remove_break_statements_at_end = false;
    while iter < cases.len() {
        let stmt = switch_case(&ctx.pool, cases[iter]).statement;
        if analysis::switch_case_contains_conditional_exit(&ctx.pool, stmt) {
            // We can't reduce switch-cases to a block when they have conditional exits.
            return false;
        }
        iter += 1;
        if analysis::switch_case_contains_unconditional_exit(&ctx.pool, stmt) {
            // We found an unconditional exit. We can use this block, but we'll need to strip out
            // the break statement if there is one.
            remove_break_statements_at_end = true;
            break;
        }
    }

    // We fell off the bottom of the switch or encountered a break. Next, we must strip down
    // `caseBlock` to hold only the statements needed to execute `caseToCapture`. To do this, we
    // eliminate the SwitchCase elements. This converts each `case n: stmt;` element into just
    // `stmt;`. While doing this, we also move the elements to the front of the array if they
    // weren't already there.
    let num_elements = iter - start;
    let mut kept: Vec<StmtId> = (0..num_elements)
        .map(|index| switch_case(&ctx.pool, cases[start + index]).statement)
        .collect();

    // If we found an unconditional break at the end, we need to eliminate that break.
    if remove_break_statements_at_end && let Some(&last) = kept.last() {
        remove_break_statements(ctx, last);
    }
    // We've stripped down `caseBlock` to contain only the captured case.
    if let StatementKind::Block(block) = &mut ctx.pool.statement_mut(case_block).kind {
        block.children = std::mem::take(&mut kept);
    }
    true
}

impl SwitchStatement {
    /// `SwitchStatement::Convert`: coerces the value to `int` and the case values to its type,
    /// checks that each case value is a constant integer and that no value repeats, and hoists
    /// variable declarations at the top level of the cases into an enclosing block.
    ///
    /// `case_values[i]` is `None` for `default:`.
    // Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L142-L210 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        value: ExprId,
        case_values: Vec<Option<ExprId>>,
        case_statements: Vec<StmtId>,
        symbol_table: SymTabId,
    ) -> Option<StmtId> {
        debug_assert_eq!(case_values.len(), case_statements.len());
        let value = TypeId::INT.coerce_expression(ctx, value)?;
        let mut cases = Vec::with_capacity(case_values.len());
        for (case_value, statement) in case_values.into_iter().zip(case_statements) {
            if let Some(case_value) = case_value {
                let case_pos = ctx.pool.expression(case_value).position;
                // Case values must be constant integers of the same type as the switch value.
                let switch_type = ctx.pool.expression(value).ty;
                let case_value = switch_type.coerce_expression(ctx, case_value)?;
                let Some(int_value) = constant_folder::get_constant_int(&ctx.pool, case_value)
                else {
                    ctx.errors
                        .error(case_pos, "case value must be a constant integer");
                    return None;
                };
                cases.push(SwitchCase::make(
                    &mut ctx.pool,
                    case_pos,
                    int_value,
                    statement,
                ));
            } else {
                cases.push(SwitchCase::make_default(&mut ctx.pool, pos, statement));
            }
        }

        // Detect duplicate `case` labels and report an error.
        let duplicate_cases = find_duplicate_case_values(&ctx.pool, &cases);
        if !duplicate_cases.is_empty() {
            for stmt in duplicate_cases {
                let sc_pos = ctx.pool.statement(stmt).position;
                let sc = switch_case(&ctx.pool, stmt);
                if sc.is_default {
                    ctx.errors.error(sc_pos, "duplicate default case");
                } else {
                    let msg = format!("duplicate case value '{}'", sc.value);
                    ctx.errors.error(sc_pos, &msg);
                }
            }
            return None;
        }

        // If a switch-case has variable declarations at its top level, we want to create a scoped
        // block around the switch, then move the variable declarations out of the switch body and
        // into the outer scope. This prevents scoping issues in backends which don't offer a native
        // switch. (skbug.com/40045447) It also allows static-switch optimization to work properly
        // when variables are inherited from earlier fall-through cases. (oss-fuzz:70589)
        let block = hoist_switch_var_declarations_at_top_level(ctx, &cases, symbol_table, pos);
        let case_block = Block::make_block(
            &mut ctx.pool,
            pos,
            cases,
            BlockKind::BracedScope,
            Some(symbol_table),
        );
        let switch_stmt = Self::make(ctx, pos, value, case_block);
        if let Some(block) = block {
            // Add the switch statement to the end of the var-decl block.
            if let StatementKind::Block(block) = &mut ctx.pool.statement_mut(block).kind {
                block.children.push(switch_stmt);
            }
            Some(block)
        } else {
            // Return the switch statement directly.
            Some(switch_stmt)
        }
    }

    /// `SwitchStatement::Make`: a switch statement over the `SwitchCase` statements of
    /// `case_block`. When optimizing and the value is known, the switch is reduced to the matching
    /// case, or removed when no case matches and there is no default.
    ///
    /// # Panics
    ///
    /// If `case_block` is not a `Block`.
    // Port of: src/sksl/ir/SkSLSwitchStatement.cpp#L212-L266 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, value: ExprId, case_block: StmtId) -> StmtId {
        // Flatten switch statements if we're optimizing, and the value is known.
        if ctx.config().settings.optimize
            && let Some(switch_value) = constant_folder::get_constant_int(&ctx.pool, value)
        {
            let cases = match &ctx.pool.statement(case_block).kind {
                StatementKind::Block(block) => block.children.clone(),
                _ => panic!("a switch's case block is a Block"),
            };
            let mut default_case: Option<StmtId> = None;
            let mut matching_case: Option<StmtId> = None;
            for &stmt in &cases {
                let sc = switch_case(&ctx.pool, stmt);
                if sc.is_default {
                    default_case = Some(stmt);
                    continue;
                }
                if sc.value == switch_value {
                    matching_case = Some(stmt);
                    break;
                }
            }
            if matching_case.is_none() {
                // No case value matches the switch value.
                let Some(default_case) = default_case else {
                    // No default switch-case exists; the switch had no effect. We can eliminate
                    // the body of the switch entirely. There's still value in preserving the
                    // symbol table here, particularly when the input program is malformed, so we
                    // keep the Block itself.
                    if let StatementKind::Block(block) =
                        &mut ctx.pool.statement_mut(case_block).kind
                    {
                        block.children.clear();
                    }
                    return case_block;
                };
                // We had a default case; that's what we matched with.
                matching_case = Some(default_case);
            }
            // Strip down our case block to contain only the matching case, if we can.
            if let Some(matching_case) = matching_case
                && block_for_case(ctx, case_block, matching_case)
            {
                return case_block;
            }
        }
        // The switch couldn't be optimized away; emit it normally.
        ctx.pool.add_statement(Statement::new(
            pos,
            StatementKind::Switch(Self { value, case_block }),
        ))
    }
}
