// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLReplaceConstVarsWithLiterals.cpp (chrome/m156).

//! [`replace_const_vars_with_literals`]: replaces the uses of a constant variable with its value,
//! when that makes the text no longer.

use std::collections::HashSet;

use super::ProgramWriter;
use crate::analysis::ProgramUsage;
use crate::constant_folder::{get_constant_value_for_variable, get_constant_value_or_null};
use crate::context::Context;
use crate::ir::{ElemId, ExprId, ExpressionKind, ProgramElementKind, VarId};

/// `ConstVarReplacer`.
struct ConstVarReplacer<'a> {
    usage: &'a mut ProgramUsage,
    /// `fCandidates`: the variables whose uses are replaced.
    candidates: HashSet<VarId>,
}

impl ProgramWriter for ConstVarReplacer<'_> {
    // Port of: src/sksl/transform/SkSLReplaceConstVarsWithLiterals.cpp#L39-L56 (chrome/m156)
    fn visit_expression_ptr(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        // If this is a variable...
        if let ExpressionKind::VariableReference(reference) = &ctx.pool.expression(expr).kind {
            // ... and it's a candidate for size reduction...
            if self.candidates.contains(&reference.variable) {
                // ... get its constant value...
                if let Some(value) = get_constant_value_or_null(&ctx.pool, expr) {
                    // ... and replace it with that value.
                    self.usage.remove_expression(&ctx.pool, expr);
                    let copy = ctx.pool.clone_expression(value);
                    ctx.pool.move_expression_into(expr, copy);
                    self.usage.add_expression(&ctx.pool, expr);
                    return false;
                }
            }
        }
        self.visit_expression(ctx, expr)
    }
}

/// `Transform::ReplaceConstVarsWithLiterals(Module&, ProgramUsage*)`: replaces the uses of each
/// constant variable in `elements` with its value, for the variables where that does not grow the
/// text. Only the function bodies are rewritten.
// Port of: src/sksl/transform/SkSLReplaceConstVarsWithLiterals.cpp#L32-L103 (chrome/m156)
#[doc(alias = "SkSL::Transform::ReplaceConstVarsWithLiterals")]
pub fn replace_const_vars_with_literals(
    ctx: &mut Context,
    elements: &[ElemId],
    usage: &mut ProgramUsage,
) {
    let mut candidates = HashSet::new();
    for (&var, &count) in &usage.variable_counts {
        // We can only replace const variables that still exist, and that have initial values.
        if count.var_exists == 0 || count.write != 1 {
            continue;
        }
        let variable = ctx.pool.variable(var);
        if !variable.modifier_flags.is_const() {
            continue;
        }
        let Some(initial_value) = variable.initial_value(&ctx.pool) else {
            continue;
        };
        // The current size is:
        //   strlen("const type varname=initialvalue;`") + count*strlen("varname").
        let constant = get_constant_value_for_variable(&ctx.pool, initial_value);
        let initial_value_size = ctx.pool.expression_description(constant).len();
        let reads = usize::try_from(count.read).unwrap_or(0);
        let total_old_size = variable.description(&ctx.pool).len() // const type varname
            + 1 // =
            + initial_value_size // initialvalue
            + 1 // ;
            + reads * variable.name.len(); // count * varname
        // If we replace varname with initialvalue everywhere, the new size would be:
        //   count*strlen("initialvalue")
        let total_new_size = reads * initial_value_size; // count * initialvalue

        if total_new_size <= total_old_size {
            candidates.insert(var);
        }
    }

    if candidates.is_empty() {
        return;
    }
    let mut visitor = ConstVarReplacer { usage, candidates };
    for &element in elements {
        if matches!(
            ctx.pool.element(element).kind,
            ProgramElementKind::Function(_)
        ) {
            visitor.visit_program_element(ctx, element);
        }
    }
}
