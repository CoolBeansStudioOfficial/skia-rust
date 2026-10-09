// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLAnalysis.cpp, `ReturnsNonOpaqueColorVisitor` and
// `Analysis::ReturnsOpaqueColor` (chrome/m156).

//! [`returns_opaque_color`]: whether every return of a function is a `vec4` whose alpha is known
//! to be 1.

use super::{ProgramVisitor, walk_statement};
use crate::constant_folder;
use crate::ir::{ElemId, ExprId, IrPool, StatementKind, StmtId};

/// `ReturnsNonOpaqueColorVisitor`: stops at the first return that is not known to be opaque.
// Port of: src/sksl/SkSLAnalysis.cpp#L173-L196 (chrome/m156)
struct ReturnsNonOpaqueColorVisitor;

impl ProgramVisitor for ReturnsNonOpaqueColorVisitor {
    // Port of: src/sksl/SkSLAnalysis.cpp#L177-L187 (chrome/m156)
    #[allow(clippy::float_cmp)] // Skia compares the alpha slot with `== 1`.
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::Return(ret) = &pool.statement(stmt).kind {
            let known_opaque = ret.expression.is_some_and(|expr| {
                let slot_count = pool.ty(pool.expression(expr).ty).slot_count();
                let value = constant_folder::get_constant_value_for_variable(pool, expr);
                slot_count == 4
                    && pool
                        .expression(value)
                        .get_constant_value(pool, 3)
                        .unwrap_or(0.0)
                        == 1.0
            });
            return !known_opaque;
        }
        walk_statement(self, pool, stmt)
    }

    // No need to recurse into expressions, these can never contain return statements.
    // Port of: src/sksl/SkSLAnalysis.cpp#L189-L192 (chrome/m156)
    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        false
    }
}

/// `Analysis::ReturnsOpaqueColor(function)`: whether `function` always returns an opaque color (a
/// `vec4` whose last component is known to be 1). This is conservative, and based on constant
/// expression analysis.
// Port of: src/sksl/SkSLAnalysis.cpp#L406-L409 (chrome/m156)
#[must_use]
pub fn returns_opaque_color(pool: &IrPool, function: ElemId) -> bool {
    !ReturnsNonOpaqueColorVisitor.visit_program_element(pool, function)
}
