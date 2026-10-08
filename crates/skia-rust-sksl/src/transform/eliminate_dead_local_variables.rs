// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp (chrome/m156).

//! [`eliminate_dead_local_variables`]: removes the local variables that are never read.

use std::collections::HashSet;

use super::{ProgramWriter, replace_with_nop};
use crate::analysis::{ProgramUsage, VariableCounts, has_side_effects};
use crate::context::Context;
use crate::ir::{
    ElemId, ExprId, ExpressionKind, ExpressionStatement, IrPool, ProgramElementKind, StatementKind,
    StmtId, VarId, Variable, VariableStorage,
};

/// `DeadLocalVariableEliminator::CanEliminate`: the variable exists, is never read, and is local.
// Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L127-L129 (chrome/m156)
fn can_eliminate(variable: &Variable, counts: VariableCounts) -> bool {
    counts.var_exists != 0 && counts.read == 0 && variable.storage == VariableStorage::Local
}

/// `DeadLocalVariableEliminator`.
struct DeadLocalVariableEliminator<'a> {
    /// `fMadeChanges`.
    made_changes: bool,
    usage: &'a mut ProgramUsage,
    /// `fDeadVariables`.
    dead_variables: HashSet<VarId>,
    /// `fAssignmentWasEliminated`.
    assignment_was_eliminated: bool,
}

impl ProgramWriter for DeadLocalVariableEliminator<'_> {
    // Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L50-L76 (chrome/m156)
    fn visit_expression_ptr(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        // Search for expressions of the form `deadVar = anyExpression`.
        let assigned_dead = match &ctx.pool.expression(expr).kind {
            ExpressionKind::Binary(binary) => binary
                .is_assignment_into_variable(&ctx.pool)
                .and_then(|assigned| match &ctx.pool.expression(assigned).kind {
                    ExpressionKind::VariableReference(reference) => Some(reference.variable),
                    _ => None,
                })
                .filter(|variable| self.dead_variables.contains(variable))
                .map(|_| binary.right),
            _ => None,
        };
        if let Some(right) = assigned_dead {
            // Replace `deadVar = anyExpression` with `anyExpression`.
            self.usage.remove_expression(&ctx.pool, expr);
            ctx.pool.move_expression_into(expr, right);
            self.usage.add_expression(&ctx.pool, expr);

            // If `anyExpression` is now a lone ExpressionStatement, it's highly likely that we can
            // eliminate it entirely. This flag will let us know to check.
            self.assignment_was_eliminated = true;

            // Re-process the newly cleaned-up expression. This lets us fully clean up gnarly
            // assignments like `a = b = 123;` where both `a` and `b` are dead, or silly
            // double-assignments like `a = a = 123;`.
            return self.visit_expression_ptr(ctx, expr);
        }
        if let ExpressionKind::VariableReference(reference) = &ctx.pool.expression(expr).kind {
            debug_assert!(!self.dead_variables.contains(&reference.variable));
        }
        self.visit_expression(ctx, expr)
    }

    // Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L78-L129 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        if let StatementKind::VarDeclaration(decl) = &ctx.pool.statement(stmt).kind {
            let (var, value) = (decl.var, decl.value);
            let counts = self.usage.get_variable(var);
            debug_assert!(counts.var_exists != 0);
            if can_eliminate(ctx.pool.variable(var), counts) {
                self.dead_variables.insert(var);
                if let Some(value) = value {
                    // The variable has an initial-value expression, which might have side effects.
                    // ExpressionStatement::Make will preserve side effects, but replaces pure
                    // expressions with Nop.
                    self.usage.remove_statement(&ctx.pool, stmt);
                    let replacement = ExpressionStatement::make(ctx, value);
                    ctx.pool.move_statement_into(stmt, replacement);
                    self.usage.add_statement(&ctx.pool, stmt);
                } else {
                    // The variable has no initial-value and can be cleanly eliminated.
                    self.usage.remove_statement(&ctx.pool, stmt);
                    replace_with_nop(&mut ctx.pool, stmt);
                }
                self.made_changes = true;

                // Re-process the newly cleaned-up statement. This lets us fully clean up gnarly
                // assignments like `a = b = 123;` where both `a` and `b` are dead, or silly
                // double-assignments like `a = a = 123;`.
                return self.visit_statement_ptr(ctx, stmt);
            }
        }

        let result = self.visit_statement(ctx, stmt);

        // If we eliminated an assignment above, we may have left behind an inert
        // ExpressionStatement.
        if self.assignment_was_eliminated {
            self.assignment_was_eliminated = false;
            if let StatementKind::Expression(expr_stmt) = &ctx.pool.statement(stmt).kind {
                let expression = expr_stmt.expression;
                if !has_side_effects(&ctx.pool, expression) {
                    // The expression-statement was inert; eliminate it entirely.
                    self.usage.remove_statement(&ctx.pool, stmt);
                    replace_with_nop(&mut ctx.pool, stmt);
                }
            }
        }

        result
    }
}

/// Whether any of `usage`'s variables is a dead local, so that the functions need scanning.
fn any_dead_local(pool: &IrPool, usage: &ProgramUsage) -> bool {
    usage
        .variable_counts
        .iter()
        .any(|(&var, &counts)| can_eliminate(pool.variable(var), counts))
}

/// `eliminate_dead_local_variables(context, elements, usage)`: removes the dead local variables of
/// the function bodies in `elements`. Returns true if any change was made.
// Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L39-L156 (chrome/m156)
fn eliminate_dead_local_variables_in(
    ctx: &mut Context,
    elements: &[ElemId],
    usage: &mut ProgramUsage,
) -> bool {
    // Only scan the program when it holds at least one dead local variable.
    if !any_dead_local(&ctx.pool, usage) {
        return false;
    }
    let mut visitor = DeadLocalVariableEliminator {
        made_changes: false,
        usage,
        dead_variables: HashSet::new(),
        assignment_was_eliminated: false,
    };
    // Scan the program for any dead local variables and eliminate them all.
    for &element in elements {
        if matches!(
            ctx.pool.element(element).kind,
            ProgramElementKind::Function(_)
        ) {
            visitor.visit_program_element(ctx, element);
        }
    }
    visitor.made_changes
}

/// `Transform::EliminateDeadLocalVariables(const Context&, Module&, ProgramUsage*)`: the module
/// form, which does not consult the settings. Returns true if any change was made.
// Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L158-L162 (chrome/m156)
pub fn eliminate_dead_local_variables(
    ctx: &mut Context,
    elements: &[ElemId],
    usage: &mut ProgramUsage,
) -> bool {
    eliminate_dead_local_variables_in(ctx, elements, usage)
}

/// `Transform::EliminateDeadLocalVariables(Program&)`: the program form, which does nothing when
/// the settings disable dead-variable removal. Returns true if any change was made.
// Port of: src/sksl/transform/SkSLEliminateDeadLocalVariables.cpp#L164-L170 (chrome/m156)
pub fn eliminate_dead_local_variables_in_program(
    ctx: &mut Context,
    owned: &[ElemId],
    usage: &mut ProgramUsage,
) -> bool {
    ctx.config().settings.remove_dead_variables
        && eliminate_dead_local_variables_in(ctx, owned, usage)
}
