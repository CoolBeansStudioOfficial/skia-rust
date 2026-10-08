// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLSpecialization.{h,cpp} (chrome/m156).

//! Function specialization: finds the calls that pass a uniform (or a parameter that a caller
//! specialized) for a parameter, and records one specialized copy of the callee per distinct set
//! of such arguments.

use std::collections::{HashMap, HashSet};

use super::s9b_shims::is_same_expression_tree;
use super::{ProgramVisitor, walk_expression};
use crate::ir::{
    ElemId, ExprId, ExpressionKind, FnId, FunctionCall, IrPool, ProgramElementKind, VarId,
    Variable, VariableStorage,
};

/// The index of the specialized copy of a function that is being walked, or of the copy a call
/// must use. `UNSPECIALIZED` is the generic copy.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L30-L38 (chrome/m156)
pub type SpecializationIndex = i32;

/// `kUnspecialized`.
pub const UNSPECIALIZED: SpecializationIndex = -1;

/// Global uniforms used by one specialization: maps a function parameter to the argument
/// expression that stands for it.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L41-L42 (chrome/m156)
pub type SpecializedParameters = HashMap<VarId, ExprId>;

/// The set of specialized implementations that a function needs.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L43-L44 (chrome/m156)
pub type Specializations = Vec<SpecializedParameters>;

/// The specializations required by the whole program.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L45-L46 (chrome/m156)
pub type SpecializationMap = HashMap<FnId, Specializations>;

/// A key for a specialized function: the declaration and the specialization.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L51-L62 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecializedFunctionKey {
    /// `fDeclaration`.
    pub declaration: FnId,
    /// `fSpecializationIndex`.
    pub specialization_index: SpecializationIndex,
}

/// A key into the [`SpecializedCallMap`]: a call, and the specialization of the function that
/// makes it.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L65-L79 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecializedCallKey {
    /// `fStablePointer`: the id of the call expression.
    pub stable_pointer: ExprId,
    /// `fParentSpecializationIndex`.
    pub parent_specialization_index: SpecializationIndex,
}

/// The specialization index that each call (in each specialized caller) must use.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L83-L84 (chrome/m156)
pub type SpecializedCallMap = HashMap<SpecializedCallKey, SpecializationIndex>;

/// `SpecializationInfo`: what [`find_functions_to_specialize`] found.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L85-L88 (chrome/m156)
#[derive(Debug, Default)]
pub struct SpecializationInfo {
    /// `fSpecializationMap`.
    pub specialization_map: SpecializationMap,
    /// `fSpecializedCallMap`.
    pub specialized_call_map: SpecializedCallMap,
}

/// The parameters that a specialization applies to: `true` for each parameter, given the
/// variable. Skia's `ParameterMatchesFn`.
// Port of: src/sksl/analysis/SkSLSpecialization.h#L94-L95 (chrome/m156)
pub type ParameterMatchesFn<'a> = &'a dyn Fn(&Variable) -> bool;

/// `Searcher`: walks the call graph from `main`, creating a specialization of each callee for
/// each distinct set of specialized arguments.
struct Searcher<'a> {
    info: &'a mut SpecializationInfo,
    parameter_matches: ParameterMatchesFn<'a>,
    visited_functions: HashSet<FnId>,
    inherited_specializations: SpecializedParameters,
    inherited_specialization_index: SpecializationIndex,
}

/// `parameter_mappings_are_equal`.
// Port of: src/sksl/analysis/SkSLSpecialization.cpp#L28-L43 (chrome/m156)
fn parameter_mappings_are_equal(
    pool: &IrPool,
    left: &SpecializedParameters,
    right: &SpecializedParameters,
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (key, &left_expr) in left {
        let Some(&right_expr) = right.get(key) else {
            return false;
        };
        if !is_same_expression_tree(pool, left_expr, right_expr) {
            return false;
        }
    }
    true
}

impl ProgramVisitor for Searcher<'_> {
    // Port of: src/sksl/analysis/SkSLSpecialization.cpp#L45-L134 (chrome/m156)
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if let ExpressionKind::FunctionCall(call) = &pool.expression(expr).kind {
            let decl_id = call.function;
            let decl = pool.function(decl_id);
            if !decl.is_intrinsic() {
                let specialization = self.specialize_call(pool, call);
                if !specialization.is_empty() {
                    let call_key = SpecializedCallKey {
                        stable_pointer: call.stable_pointer,
                        parent_specialization_index: self.inherited_specialization_index,
                    };
                    // Reuse an existing specialization if one has the same parameter mappings.
                    let specializations = self.info.specialization_map.entry(decl_id).or_default();
                    let existing = specializations.iter().position(|entry| {
                        parameter_mappings_are_equal(pool, &specialization, entry)
                    });
                    if let Some(index) = existing {
                        // This specialization has already been tracked.
                        let index = SpecializationIndex::try_from(index)
                            .expect("a specialization index fits in an int");
                        self.info.specialized_call_map.insert(call_key, index);
                        return walk_expression(self, pool, expr);
                    }

                    // Otherwise this is a new specialization: the call uses it, and the callee
                    // is walked with it as the inherited specialization.
                    let index = SpecializationIndex::try_from(specializations.len())
                        .expect("a specialization index fits in an int");
                    specializations.push(specialization.clone());
                    self.info.specialized_call_map.insert(call_key, index);

                    // Swap so that the inherited specializations of the caller are restored once
                    // the callee has been walked.
                    let outer_specializations =
                        std::mem::replace(&mut self.inherited_specializations, specialization);
                    let outer_index =
                        std::mem::replace(&mut self.inherited_specialization_index, index);
                    let definition = decl
                        .definition
                        .expect("a specialized function has a definition");
                    self.visit_program_element(pool, definition);
                    self.inherited_specialization_index = outer_index;
                    self.inherited_specializations = outer_specializations;
                } else if self.visited_functions.insert(decl_id) {
                    // The callee is not specialized, but its calls may be. Since nothing is
                    // specialized here, each callee needs to be walked once only.
                    let definition = decl.definition.expect("a called function has a definition");
                    self.visit_program_element(pool, definition);
                }
            }
        }
        walk_expression(self, pool, expr)
    }
}

impl Searcher<'_> {
    /// The parameter mappings of `call` in the current specialization: each parameter whose
    /// argument is a global or an inherited parameter, and which `parameter_matches` accepts.
    // Port of: src/sksl/analysis/SkSLSpecialization.cpp#L53-L97 (chrome/m156)
    fn specialize_call(&self, pool: &IrPool, call: &FunctionCall) -> SpecializedParameters {
        let mut specialization = SpecializedParameters::new();
        let decl = pool.function(call.function);
        let num_params = decl.parameters.len();
        debug_assert_eq!(call.arguments.len(), num_params);

        for (i, &arg_id) in call.arguments.iter().enumerate().take(num_params) {
            // Specializations can only be made on arguments that are variable references or
            // field accesses of variables, since these are inlined in the specialized functions.
            let arg = pool.expression(arg_id);
            let arg_base = match &arg.kind {
                ExpressionKind::VariableReference(var_ref) => var_ref.variable,
                ExpressionKind::FieldAccess(field)
                    if matches!(
                        pool.expression(field.base).kind,
                        ExpressionKind::VariableReference(_)
                    ) =>
                {
                    let ExpressionKind::VariableReference(var_ref) =
                        &pool.expression(field.base).kind
                    else {
                        unreachable!("the base was checked to be a variable reference");
                    };
                    var_ref.variable
                }
                _ => continue,
            };

            let param = decl.parameters[i];
            // Check that this parameter fits the criteria to create a specialization.
            if !(self.parameter_matches)(pool.variable(param)) {
                continue;
            }

            match pool.variable(arg_base).storage {
                VariableStorage::Global => {
                    specialization.insert(param, arg_id);
                }
                VariableStorage::Parameter => {
                    // The argument is a parameter of the caller, which an inherited
                    // specialization maps to an expression.
                    let uniform_expr = self
                        .inherited_specializations
                        .get(&arg_base)
                        .expect("a specialized parameter has an inherited specialization");
                    specialization.insert(param, *uniform_expr);
                }
                // TODO(b/353532475) in Skia: report an error instead of aborting.
                _ => panic!("Specialization requires a uniform or parameter variable"),
            }
        }
        specialization
    }
}

/// `FindFunctionsToSpecialize`: walks the program from `main`, and records the specializations
/// that each function needs in `info`. A function with a matching parameter gets an entry even
/// when no reachable call specializes it, so that the code generator does not emit it without
/// its specializations. `elements` are the program's elements (shared and owned).
// Port of: src/sksl/analysis/SkSLSpecialization.cpp#L136-L196 (chrome/m156)
pub fn find_functions_to_specialize(
    pool: &IrPool,
    elements: &[ElemId],
    info: &mut SpecializationInfo,
    parameter_matches: ParameterMatchesFn<'_>,
) {
    for &element in elements {
        let ProgramElementKind::Function(func_def) = &pool.element(element).kind else {
            continue;
        };
        let decl_id = func_def.declaration;
        let decl = pool.function(decl_id);

        if decl.is_main {
            // Visit through the program's call stack and aggregate the necessary
            // specializations.
            let mut searcher = Searcher {
                info: &mut *info,
                parameter_matches,
                visited_functions: HashSet::new(),
                inherited_specializations: SpecializedParameters::new(),
                inherited_specialization_index: UNSPECIALIZED,
            };
            searcher.visit_program_element(pool, element);
            continue;
        }

        // Look for any function parameter that needs specialization.
        if decl
            .parameters
            .iter()
            .any(|&param| parameter_matches(pool.variable(param)))
        {
            // Ensure that this function is in the map, whether or not main() reaches it, so
            // that unreachable specialized functions are discarded instead of emitted.
            info.specialization_map.entry(decl_id).or_default();
        }
    }
}

/// `FindSpecializationIndexForCall`: the specialization that `call` uses, given the specialization
/// of the function that contains it, or `UNSPECIALIZED`.
// Port of: src/sksl/analysis/SkSLSpecialization.cpp#L198-L204 (chrome/m156)
#[must_use]
pub fn find_specialization_index_for_call(
    call: &FunctionCall,
    info: &SpecializationInfo,
    parent_specialization_index: SpecializationIndex,
) -> SpecializationIndex {
    let call_key = SpecializedCallKey {
        stable_pointer: call.stable_pointer,
        parent_specialization_index,
    };
    info.specialized_call_map
        .get(&call_key)
        .copied()
        .unwrap_or(UNSPECIALIZED)
}

/// `FindSpecializedParametersForFunction`: one flag per parameter of `func`, set when the
/// parameter is specialized and so must be left out of the parameter list.
///
/// Skia reads the first specialization with `front()`. A function with no specializations (an
/// unreachable one) has none to read, so this returns no flags for it.
// Port of: src/sksl/analysis/SkSLSpecialization.cpp#L206-L221 (chrome/m156)
#[must_use]
pub fn find_specialized_parameters_for_function(
    pool: &IrPool,
    func: FnId,
    info: &SpecializationInfo,
) -> Vec<bool> {
    let decl = pool.function(func);
    let mut result = vec![false; decl.parameters.len()];
    if let Some(first) = info
        .specialization_map
        .get(&func)
        .and_then(|specializations| specializations.first())
    {
        for (index, param) in decl.parameters.iter().enumerate() {
            if first.contains_key(param) {
                result[index] = true;
            }
        }
    }
    result
}

/// `GetParameterMappingsForFunction`: calls `callback` once per specialized parameter of `func` in
/// the given specialization, with the parameter's index, the parameter, and the value it takes.
///
/// # Panics
///
/// If `specialization_index` is not `UNSPECIALIZED` and is not a specialization of `func`.
// Port of: src/sksl/analysis/SkSLSpecialization.cpp#L223-L241 (chrome/m156)
pub fn get_parameter_mappings_for_function(
    pool: &IrPool,
    func: FnId,
    info: &SpecializationInfo,
    specialization_index: SpecializationIndex,
    mut callback: impl FnMut(usize, VarId, ExprId),
) {
    if specialization_index == UNSPECIALIZED {
        return;
    }
    let Some(specializations) = info.specialization_map.get(&func) else {
        return;
    };
    let index = usize::try_from(specialization_index)
        .expect("a specialization index is not negative when it is specialized");
    let specialized_params = &specializations[index];
    for (index, &param) in pool.function(func).parameters.iter().enumerate() {
        if let Some(&expr) = specialized_params.get(&param) {
            callback(index, param, expr);
        }
    }
}
