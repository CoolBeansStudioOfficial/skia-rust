// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLAnalysis.cpp (`IsAssignableVisitor`, `Analysis::IsAssignable`,
// `Analysis::UpdateVariableRefKind`).

//! Which expressions can be assigned into, and marking the variable they write.

use crate::error_reporter::ErrorReporter;
use crate::ir::{ExprId, ExpressionKind, IrPool, ModifierFlags, VariableRefKind, VariableStorage};

/// `Analysis::AssignmentInfo`: the variable reference an assignment writes to.
#[doc(alias = "SkSL::Analysis::AssignmentInfo")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssignmentInfo {
    /// `fAssignedVar`: the `VariableReference` expression, if one was found.
    pub assigned_var: Option<ExprId>,
}

/// `IsAssignableVisitor`. It is not a `ProgramVisitor`: for an index or a field access it looks at
/// the base only, and never at the index.
// Port of: src/sksl/SkSLAnalysis.cpp#L261-L339 (chrome/m156)
struct IsAssignableVisitor<'a> {
    pool: &'a IrPool,
    errors: &'a mut ErrorReporter,
    assigned_var: Option<ExprId>,
}

impl IsAssignableVisitor<'_> {
    /// `visit(expr, info)`: the error count is compared before and after, so a probe with no
    /// reporter still answers.
    fn visit(&mut self, expr: ExprId) {
        self.visit_expression(expr, None);
    }

    /// `visitExpression(expr, fieldAccess)`. `field_access` is the enclosing field access, if any,
    /// which names the variable in the error messages.
    fn visit_expression(&mut self, expr: ExprId, field_access: Option<ExprId>) {
        let pool = self.pool;
        let e = pool.expression(expr);
        match &e.kind {
            ExpressionKind::VariableReference(reference) => {
                let var = pool.variable(reference.variable);
                let field_name = || match field_access {
                    Some(access) => match &pool.expression(access).kind {
                        ExpressionKind::FieldAccess(f) => f.description(pool),
                        _ => var.name.to_string(),
                    },
                    None => var.name.to_string(),
                };
                if var.modifier_flags.is_const() || var.modifier_flags.is_uniform() {
                    self.errors.error(
                        e.position,
                        &format!("cannot modify immutable variable '{}'", field_name()),
                    );
                } else if var.storage == VariableStorage::Global
                    && var.modifier_flags.contains(ModifierFlags::IN)
                {
                    self.errors.error(
                        e.position,
                        &format!("cannot modify pipeline input variable '{}'", field_name()),
                    );
                } else {
                    debug_assert!(self.assigned_var.is_none());
                    self.assigned_var = Some(expr);
                }
            }
            ExpressionKind::FieldAccess(f) => {
                self.visit_expression(f.base, Some(expr));
            }
            ExpressionKind::Swizzle(swizzle) => {
                self.check_swizzle_write(e.position, swizzle.components.as_slice());
                self.visit_expression(swizzle.base, field_access);
            }
            ExpressionKind::Index(index) => {
                self.visit_expression(index.base, field_access);
            }
            ExpressionKind::Poison(_) => {}
            _ => {
                self.errors
                    .error(e.position, "cannot assign to this expression");
            }
        }
    }

    /// `checkSwizzleWrite`: no component may be written twice.
    fn check_swizzle_write(&mut self, position: crate::position::Position, components: &[i8]) {
        let mut bits = 0_i32;
        for &idx in components {
            let bit = 1_i32 << idx;
            if bits & bit != 0 {
                self.errors.error(
                    position,
                    "cannot write to the same swizzle field more than once",
                );
                break;
            }
            bits |= bit;
        }
    }
}

/// `Analysis::IsAssignable`: true if `expr` can be assigned into. `info`, when given, receives the
/// variable reference that is written. `errors`, when given, receives an error for each part of
/// `expr` that is not writable.
// Port of: src/sksl/SkSLAnalysis.cpp#L551-L554 (chrome/m156)
#[must_use]
pub fn is_assignable(
    pool: &IrPool,
    expr: ExprId,
    info: Option<&mut AssignmentInfo>,
    errors: Option<&mut ErrorReporter>,
) -> bool {
    let mut unused_errors = ErrorReporter::no_op();
    let errors = errors.unwrap_or(&mut unused_errors);
    let old_error_count = errors.error_count();
    let mut visitor = IsAssignableVisitor {
        pool,
        errors,
        assigned_var: None,
    };
    visitor.visit(expr);
    let assigned_var = visitor.assigned_var;
    if let Some(info) = info {
        info.assigned_var = assigned_var;
    }
    errors.error_count() == old_error_count
}

/// `Analysis::UpdateVariableRefKind`: sets the `refKind` of the variable reference that `expr`
/// writes to. Returns true if `expr` is assignable. Otherwise it returns false, and reports an
/// error if `errors` is given.
// Port of: src/sksl/SkSLAnalysis.cpp#L556-L575 (chrome/m156)
pub fn update_variable_ref_kind(
    pool: &mut IrPool,
    expr: ExprId,
    kind: VariableRefKind,
    mut errors: Option<&mut ErrorReporter>,
) -> bool {
    let mut info = AssignmentInfo::default();
    if !is_assignable(pool, expr, Some(&mut info), errors.as_deref_mut()) {
        return false;
    }
    let Some(assigned) = info.assigned_var else {
        if let Some(errors) = errors {
            let position = pool.expression(expr).position;
            let description = pool.expression_description(expr);
            errors.error(
                position,
                &format!("can't assign to expression '{description}'"),
            );
        }
        return false;
    };
    if let ExpressionKind::VariableReference(reference) = &mut pool.expression_mut(assigned).kind {
        reference.ref_kind = kind;
    }
    true
}
