// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLIsConstantExpression.cpp, `ES2IndexingVisitor` and
// `Analysis::ValidateIndexingForES2` (chrome/m156).

//! [`validate_indexing_for_es2`]: in strict ES2 code, every index expression must be a
//! constant-index-expression, where a `for` loop's index counts as a constant.

use std::collections::HashSet;

use super::expression_queries::is_constant_expression_with_loop_indices;
use super::{ProgramVisitor, walk_expression, walk_statement};
use crate::error_reporter::ErrorReporter;
use crate::ir::{ElemId, ExprId, ExpressionKind, IrPool, StatementKind, StmtId, VarId};

/// `ES2IndexingVisitor`: tracks the loop indices in scope and checks each index expression.
struct Es2IndexingVisitor<'a> {
    errors: &'a mut ErrorReporter,
    loop_indices: HashSet<VarId>,
}

impl ProgramVisitor for Es2IndexingVisitor<'_> {
    // Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L118-L131 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::For(for_stmt) = &pool.statement(stmt).kind {
            let var = match for_stmt.initializer.map(|init| &pool.statement(init).kind) {
                Some(StatementKind::VarDeclaration(decl)) => decl.var,
                _ => unreachable!("a strict-ES2 for loop has a variable declaration initializer"),
            };
            debug_assert!(!self.loop_indices.contains(&var));
            self.loop_indices.insert(var);
            let result = self.visit_statement(pool, for_stmt.statement);
            self.loop_indices.remove(&var);
            return result;
        }
        walk_statement(self, pool, stmt)
    }

    // Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L133-L148 (chrome/m156)
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if let ExpressionKind::Index(index) = &pool.expression(expr).kind
            && !is_constant_expression_with_loop_indices(pool, index.index, &self.loop_indices)
        {
            self.errors.error(
                pool.expression(expr).position,
                "index expression must be constant",
            );
            return true;
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::ValidateIndexingForES2`: reports every index expression of `element` that is not a
/// constant-index-expression.
// Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L161-L164 (chrome/m156)
pub fn validate_indexing_for_es2(pool: &IrPool, element: ElemId, errors: &mut ErrorReporter) {
    let mut visitor = Es2IndexingVisitor {
        errors,
        loop_indices: HashSet::new(),
    };
    visitor.visit_program_element(pool, element);
}
