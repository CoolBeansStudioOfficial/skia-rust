// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateDeadGlobalVariables.cpp (chrome/m156).

//! [`eliminate_dead_global_variables`]: removes the global variables that are never used.

use crate::analysis::ProgramUsage;
use crate::context::Context;
use crate::ir::{ElemId, IrPool, ProgramElementKind, StatementKind};

/// `is_dead_variable`: true when `element` is a global declaration whose variable is dead (and,
/// with `only_private_globals`, whose name starts with `$`). Its usage is removed, because it is
/// about to be eliminated.
// Port of: src/sksl/transform/SkSLEliminateDeadGlobalVariables.cpp#L26-L43 (chrome/m156)
fn is_dead_variable(
    pool: &IrPool,
    element: ElemId,
    usage: &mut ProgramUsage,
    only_private_globals: bool,
) -> bool {
    let ProgramElementKind::GlobalVar(global) = &pool.element(element).kind else {
        return false;
    };
    let declaration = global.declaration;
    let StatementKind::VarDeclaration(var_decl) = &pool.statement(declaration).kind else {
        unreachable!("a global variable's declaration is a variable declaration");
    };
    let var = var_decl.var;
    if only_private_globals && !pool.variable(var).name.starts_with('$') {
        return false;
    }
    if !usage.is_dead(pool, var) {
        return false;
    }
    // This declaration is about to be eliminated by remove_if; update ProgramUsage accordingly.
    usage.remove_statement(pool, declaration);
    true
}

/// `Transform::EliminateDeadGlobalVariables(const Context&, Module&, ProgramUsage*, bool)`: removes
/// the dead global declarations from `elements` when the settings allow it. With
/// `only_private_globals`, only the `$`-prefixed names are candidates. Returns true if any were
/// removed.
// Port of: src/sksl/transform/SkSLEliminateDeadGlobalVariables.cpp#L45-L63 (chrome/m156)
pub fn eliminate_dead_global_variables(
    ctx: &mut Context,
    elements: &mut Vec<ElemId>,
    usage: &mut ProgramUsage,
    only_private_globals: bool,
) -> bool {
    let num_elements = elements.len();
    if ctx.config().settings.remove_dead_variables {
        let pool = &ctx.pool;
        elements.retain(|&element| !is_dead_variable(pool, element, usage, only_private_globals));
    }
    elements.len() < num_elements
}

/// `Transform::EliminateDeadGlobalVariables(Program&)`: removes the dead global declarations from a
/// program's owned and shared elements (every name is a candidate). Returns true if any were
/// removed.
// Port of: src/sksl/transform/SkSLEliminateDeadGlobalVariables.cpp#L65-L88 (chrome/m156)
pub fn eliminate_dead_global_variables_in_program(
    ctx: &mut Context,
    owned: &mut Vec<ElemId>,
    shared: &mut Vec<ElemId>,
    usage: &mut ProgramUsage,
) -> bool {
    let owned_changed = eliminate_dead_global_variables(ctx, owned, usage, false);
    let shared_changed = eliminate_dead_global_variables(ctx, shared, usage, false);
    owned_changed || shared_changed
}
