// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLAnalysis.cpp (`VariableWriteVisitor`, `NodeCountVisitor`,
// `Analysis::DetectVarDeclarationWithoutScope`, `Analysis::NodeCountUpToLimit`,
// `Analysis::StatementWritesToVariable`).

//! Statement-level helpers: write detection, node counting, and the scope check.

use super::{ProgramVisitor, walk_expression, walk_statement};
use crate::error_reporter::ErrorReporter;
use crate::ir::{
    ExprId, ExpressionKind, FunctionDefinition, IrPool, StatementKind, StmtId, VarId,
    VariableRefKind,
};

/// `VariableWriteVisitor`: true if a write to `var` appears anywhere in the statement.
// Port of: src/sksl/SkSLAnalysis.cpp#L230-L259 (chrome/m156)
struct VariableWriteVisitor {
    variable: VarId,
}

impl ProgramVisitor for VariableWriteVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if matches!(
            &pool.expression(expr).kind,
            ExpressionKind::VariableReference(reference)
                if reference.variable == self.variable
                    && matches!(
                        reference.ref_kind,
                        VariableRefKind::Write | VariableRefKind::ReadWrite | VariableRefKind::Pointer
                    )
        ) {
            return true;
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::StatementWritesToVariable`: true if the statement might alter `var`.
// Port of: src/sksl/SkSLAnalysis.cpp#L547-L549 (chrome/m156)
#[must_use]
pub fn statement_writes_to_variable(pool: &IrPool, stmt: StmtId, var: VarId) -> bool {
    VariableWriteVisitor { variable: var }.visit_statement(pool, stmt)
}

/// `NodeCountVisitor`: counts the nodes it visits, and stops once the count reaches `limit`.
// Port of: src/sksl/SkSLAnalysis.cpp#L199-L228 (chrome/m156)
struct NodeCountVisitor {
    count: i32,
    limit: i32,
}

impl ProgramVisitor for NodeCountVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        self.count += 1;
        self.count >= self.limit || walk_expression(self, pool, expr)
    }

    fn visit_program_element(&mut self, pool: &IrPool, element: crate::ir::ElemId) -> bool {
        self.count += 1;
        self.count >= self.limit || super::walk_program_element(self, pool, element)
    }

    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        self.count += 1;
        self.count >= self.limit || walk_statement(self, pool, stmt)
    }
}

/// `Analysis::NodeCountUpToLimit`: the number of nodes in the body of `function`, counted up to
/// `limit`.
// Port of: src/sksl/SkSLAnalysis.cpp#L543-L545 (chrome/m156)
#[must_use]
pub fn node_count_up_to_limit(pool: &IrPool, function: &FunctionDefinition, limit: i32) -> i32 {
    let mut visitor = NodeCountVisitor { count: 0, limit };
    visitor.visit_statement(pool, function.body);
    visitor.count
}

/// `Analysis::DetectVarDeclarationWithoutScope`: detects an orphaned variable declaration outside
/// of a scope, as in `if (true) int a;`. Returns true if one was found, and reports it to
/// `errors` if given.
// Port of: src/sksl/SkSLAnalysis.cpp#L510-L541 (chrome/m156)
#[must_use]
pub fn detect_var_declaration_without_scope(
    pool: &IrPool,
    stmt: StmtId,
    errors: Option<&mut ErrorReporter>,
) -> bool {
    // A variable declaration can create either a lone VarDeclaration or an unscoped Block
    // containing multiple VarDeclaration statements. We need to detect either case.
    let var = match &pool.statement(stmt).kind {
        // The single-variable case. No blocks at all.
        StatementKind::VarDeclaration(decl) => decl.var,
        // The multiple-variable case: an unscoped, non-empty block...
        StatementKind::Block(block) => {
            if block.is_scope() || block.children.is_empty() {
                return false;
            }
            // ... holding a variable declaration.
            let inner = block.children[0];
            match &pool.statement(inner).kind {
                StatementKind::VarDeclaration(decl) => decl.var,
                _ => return false,
            }
        }
        // This statement wasn't a variable declaration. No problem.
        _ => return false,
    };

    // Report an error.
    if let Some(errors) = errors {
        let variable = pool.variable(var);
        errors.error(
            variable.position,
            &format!("variable '{}' must be created in a scope", variable.name),
        );
    }
    true
}
