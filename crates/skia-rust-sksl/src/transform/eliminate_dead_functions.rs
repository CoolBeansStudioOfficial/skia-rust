// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLEliminateDeadFunctions.cpp (chrome/m156).

//! [`eliminate_dead_functions`]: removes the function definitions that are never called.

use crate::analysis::ProgramUsage;
use crate::context::Context;
use crate::ir::{ElemId, IrPool, ProgramElementKind};

/// `dead_function_predicate`: true when `element` is a function that is neither `main` nor
/// called. Its usage is removed, because it is about to be eliminated.
// Port of: src/sksl/transform/SkSLEliminateDeadFunctions.cpp#L25-L36 (chrome/m156)
fn dead_function_predicate(pool: &IrPool, element: ElemId, usage: &mut ProgramUsage) -> bool {
    let ProgramElementKind::Function(def) = &pool.element(element).kind else {
        return false;
    };
    let declaration = def.declaration;
    if pool.function(declaration).is_main || usage.get_call_count(declaration) > 0 {
        return false;
    }
    // This function is about to be eliminated by remove_if; update ProgramUsage accordingly.
    usage.remove_element(pool, element);
    true
}

/// `Transform::EliminateDeadFunctions(const Context&, Module&, ProgramUsage*)`: removes the dead
/// functions from `elements` when the context's settings allow it. Returns true if any were removed.
// Port of: src/sksl/transform/SkSLEliminateDeadFunctions.cpp#L63-L77 (chrome/m156)
pub fn eliminate_dead_functions(
    ctx: &mut Context,
    elements: &mut Vec<ElemId>,
    usage: &mut ProgramUsage,
) -> bool {
    let num_elements = elements.len();
    if ctx.config().settings.remove_dead_functions {
        let pool = &ctx.pool;
        elements.retain(|&element| !dead_function_predicate(pool, element, usage));
    }
    elements.len() < num_elements
}

/// `Transform::EliminateDeadFunctions(Program&)`: removes the dead functions from a program's owned
/// and shared elements. Returns true if any were removed.
// Port of: src/sksl/transform/SkSLEliminateDeadFunctions.cpp#L38-L61 (chrome/m156)
pub fn eliminate_dead_functions_in_program(
    ctx: &mut Context,
    owned: &mut Vec<ElemId>,
    shared: &mut Vec<ElemId>,
    usage: &mut ProgramUsage,
) -> bool {
    // Owned elements first, then shared ones, as Skia's two `remove_if` calls run.
    let owned_changed = eliminate_dead_functions(ctx, owned, usage);
    let shared_changed = eliminate_dead_functions(ctx, shared, usage);
    owned_changed || shared_changed
}
