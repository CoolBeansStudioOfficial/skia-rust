// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLHoistSwitchVarDeclarationsAtTopLevel.cpp (chrome/m156).

//! [`hoist_switch_var_declarations_at_top_level`]: moves the top-level variable declarations of a
//! switch's cases into a scoped block around the switch.

use super::ProgramWriter;
use crate::analysis;
use crate::context::Context;
use crate::ir::{
    BinaryExpression, Block, BlockKind, ExprId, ExpressionStatement, StatementKind, StmtId,
    SymTabId, SymbolId, VarId, VariableRefKind, VariableReference, insert_new_parent,
    move_symbol_to,
};
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;

/// `HoistSwitchVarDeclsVisitor`: collects the variable declarations at the top level of the cases.
#[derive(Default)]
struct HoistSwitchVarDeclsVisitor {
    /// `fVarDeclarations`: the statement slots of the declarations found, in order.
    var_declarations: Vec<StmtId>,
}

impl ProgramWriter for HoistSwitchVarDeclsVisitor {
    fn visit_expression_ptr(&mut self, _ctx: &mut Context, _expr: ExprId) -> bool {
        // We don't need to recurse into expressions.
        false
    }

    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        match &ctx.pool.statement(stmt).kind {
            // Recurse inward from the switch and its inner switch-cases.
            StatementKind::SwitchCase(_) => self.visit_statement(ctx, stmt),
            StatementKind::Block(block) => {
                if block.is_scope() {
                    // A scoped block keeps its declarations to itself.
                    false
                } else {
                    // Recurse inward from unscoped blocks.
                    self.visit_statement(ctx, stmt)
                }
            }
            // Keep track of variable declarations.
            StatementKind::VarDeclaration(_) => {
                self.var_declarations.push(stmt);
                false
            }
            // We don't need to recurse into other statement types; we're only interested in the
            // top level of the switch statement.
            _ => false,
        }
    }
}

/// `IRHelpers::Assign(Ref(var), value)`: the statement `var = value;`.
// Port of: src/sksl/ir/SkSLIRHelpers.h#L99-L104 (chrome/m156), with `Ref` of SkSLIRHelpers.h#L30-L33.
fn assign_variable(ctx: &mut Context, var: VarId, value: ExprId) -> StmtId {
    let target = VariableReference::make(
        &mut ctx.pool,
        Position::default(),
        var,
        VariableRefKind::Read,
    );
    let updated =
        analysis::update_variable_ref_kind(&mut ctx.pool, target, VariableRefKind::Write, None);
    debug_assert!(updated, "a variable declared in a switch is assignable");
    let pos = ctx
        .pool
        .expression(target)
        .position
        .range_through(ctx.pool.expression(value).position);
    let expr = BinaryExpression::make(ctx, pos, target, Operator::from(OperatorKind::Eq), value);
    ExpressionStatement::make(ctx, expr)
}

/// `Transform::HoistSwitchVarDeclarationsAtTopLevel`: moves the top-level variable declarations of
/// the switch's cases into a new scoped block, which the caller places around the switch. The
/// cases are rewritten in place: a declaration with an initial value becomes an assignment, and any
/// other declaration becomes a `Nop`. The variables' symbols move into the block's symbol table.
///
/// `cases` holds the `SwitchCase` statements. Returns `None` when there are no such declarations.
// Port of: src/sksl/transform/SkSLHoistSwitchVarDeclarationsAtTopLevel.cpp#L26-L129 (chrome/m156)
#[must_use]
pub fn hoist_switch_var_declarations_at_top_level(
    ctx: &mut Context,
    cases: &[StmtId],
    switch_symbols: SymTabId,
    pos: Position,
) -> Option<StmtId> {
    // Visit every switch-case in the switch, looking for hoistable var-declarations.
    let mut visitor = HoistSwitchVarDeclsVisitor::default();
    for &case in cases {
        visitor.visit_statement_ptr(ctx, case);
    }
    // If no declarations were found, the switch can stay as-is.
    if visitor.var_declarations.is_empty() {
        return None;
    }

    // Move all of the var-declaration statements into a separate block.
    let block_symbols = insert_new_parent(&mut ctx.pool, switch_symbols);
    let mut block_stmts = Vec::with_capacity(visitor.var_declarations.len() + 1);
    for slot in visitor.var_declarations {
        let (var, value) = match &ctx.pool.statement(slot).kind {
            StatementKind::VarDeclaration(decl) => (decl.var, decl.value),
            _ => unreachable!("the visitor only collects variable declarations"),
        };
        let is_const = ctx.pool.variable(var).modifier_flags.is_const();
        // The declaration moves into the block; its slot is left as a `Nop`.
        let moved = ctx.pool.relocate_statement(slot);
        match value {
            Some(value) if !is_const => {
                // The inner variable-declaration has an initial-value; we must replace the
                // declaration with an assignment to the variable. This also has the helpful
                // effect of stripping off the initial-value from the declaration.
                if let StatementKind::VarDeclaration(decl) = &mut ctx.pool.statement_mut(moved).kind
                {
                    decl.value = None;
                }
                let assignment = assign_variable(ctx, var, value);
                ctx.pool.move_statement_into(slot, assignment);
            }
            _ => {
                // The inner variable-declaration has no initial-value, or it's const and has a
                // constant value; we can move it upwards as-is and replace its statement with a
                // no-op.
                debug_assert!(
                    !is_const
                        || value.is_some_and(|v| analysis::is_constant_expression(&ctx.pool, v))
                );
            }
        }
        block_stmts.push(moved);
        // Hoist the variable's symbol outside of the switch's symbol table, and into the enclosing
        // block's symbol table.
        move_symbol_to(ctx, switch_symbols, block_symbols, SymbolId::Variable(var));
    }

    // Return a scoped Block holding the variable declarations.
    Some(Block::make_block(
        &mut ctx.pool,
        pos,
        block_stmts,
        BlockKind::BracedScope,
        Some(block_symbols),
    ))
}
