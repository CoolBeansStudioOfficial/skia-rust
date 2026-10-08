// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLAddConstToVarModifiers.cpp (chrome/m156).

//! [`add_const_to_var_modifiers`]: the `const` the inliner may add to a variable.

use crate::analysis::{ProgramUsage, is_compile_time_constant};
use crate::ir::{ExprId, IrPool, ModifierFlags, VarId};

/// `Transform::AddConstToVarModifiers`: the modifier flags of `var` with `const` added when it is
/// safe to add it, so that the inliner can fold more values. `initial_value` is the variable's
/// initial-value expression, if it has one.
// Port of: src/sksl/transform/SkSLAddConstToVarModifiers.cpp#L19-L38 (chrome/m156)
#[doc(alias = "SkSL::Transform::AddConstToVarModifiers")]
#[must_use]
pub fn add_const_to_var_modifiers(
    pool: &IrPool,
    var: VarId,
    initial_value: Option<ExprId>,
    usage: &ProgramUsage,
) -> ModifierFlags {
    // If the variable is already marked as `const`, keep our existing modifiers.
    let flags = pool.variable(var).modifier_flags;
    if flags.is_const() {
        return flags;
    }
    // If the variable doesn't have a compile-time-constant initial value, we can't `const` it.
    let Some(initial_value) = initial_value else {
        return flags;
    };
    if !is_compile_time_constant(pool, initial_value) {
        return flags;
    }
    // This only works for variables that are written-to a single time.
    if usage.get_variable(var).write != 1 {
        return flags;
    }
    // Add `const` to our variable's modifier flags, making it eligible for constant-folding.
    flags | ModifierFlags::CONST
}
