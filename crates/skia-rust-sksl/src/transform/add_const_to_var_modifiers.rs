// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLAddConstToVarModifiers.cpp.

//! [`add_const_to_var_modifiers`]: the modifiers an inlined variable gets.

use crate::analysis::{ProgramUsage, is_compile_time_constant};
use crate::ir::{ExprId, IrPool, ModifierFlags, VarId};

/// `Transform::AddConstToVarModifiers`: the modifier flags of `var`, plus `const` when it has a
/// compile-time-constant initial value and is written to exactly once.
// Port of: src/sksl/transform/SkSLAddConstToVarModifiers.cpp#L17-L36 (chrome/m156)
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
    match initial_value {
        Some(value) if is_compile_time_constant(pool, value) => {}
        _ => return flags,
    }
    // This only works for variables that are written-to a single time.
    let counts = usage.get_variable(var);
    if counts.write != 1 {
        return flags;
    }
    // Add `const` to our variable's modifier flags, making it eligible for constant-folding.
    flags | ModifierFlags::CONST
}
