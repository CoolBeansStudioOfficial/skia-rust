// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLAnalysis.cpp, `Analysis::DetectVarDeclarationWithoutScope`
// (chrome/m156).

//! [`detect_var_declaration_without_scope`]: a variable declared as the body of an `if`, `for`,
//! `do` or `while`, where it needs its own scope.

use crate::error_reporter::ErrorReporter;
use crate::ir::{IrPool, StatementKind, StmtId, VarId};

/// `Analysis::DetectVarDeclarationWithoutScope`: true if `stmt` declares a variable without a
/// scope of its own. When `errors` is given, it reports the variable.
///
/// A declaration can be a lone `VarDeclaration`, or an unscoped `Block` of several of them; both
/// are detected from the first declaration.
// Port of: src/sksl/SkSLAnalysis.cpp#L510-L544 (chrome/m156)
#[must_use]
pub fn detect_var_declaration_without_scope(
    pool: &IrPool,
    stmt: StmtId,
    errors: Option<&mut ErrorReporter>,
) -> bool {
    let var: VarId = match &pool.statement(stmt).kind {
        // The single-variable case. No blocks at all.
        StatementKind::VarDeclaration(decl) => decl.var,
        // The multiple-variable case: an unscoped, non-empty block ...
        StatementKind::Block(block) => {
            if block.is_scope() || block.children.is_empty() {
                return false;
            }
            // ... whose first statement is a variable declaration.
            match &pool.statement(block.children[0]).kind {
                StatementKind::VarDeclaration(decl) => decl.var,
                _ => return false,
            }
        }
        // This statement is not a variable declaration.
        _ => return false,
    };

    let variable = pool.variable(var);
    if let Some(errors) = errors {
        errors.error(
            variable.position,
            &format!("variable '{}' must be created in a scope", variable.name),
        );
    }
    true
}
