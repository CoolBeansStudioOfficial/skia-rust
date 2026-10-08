// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis and src/sksl/SkSLAnalysis.cpp.

//! Read-only analyses of the IR. So far: the [`ProgramVisitor`] traversal (task S5); the
//! analyses themselves come with tasks S9a and S9b.

mod can_exit_without_returning_value;
mod check_program_structure;
mod finalization_checks;
mod loop_info;
mod program_visitor;
mod return_complexity;
mod s9b_shims;
#[cfg(test)]
mod s9b_tests;
mod specialization;
mod switch_case_exit;
mod symbol_table_checks;
mod validate_indexing;
mod var_declaration_scope;

pub use can_exit_without_returning_value::can_exit_without_returning_value;
pub use check_program_structure::check_program_structure;
pub use finalization_checks::do_finalization_checks;
pub use loop_info::{LoopControlFlowInfo, get_loop_control_flow_info, get_loop_unroll_info};
pub use program_visitor::{ProgramVisitor, walk_expression, walk_program_element, walk_statement};
pub use return_complexity::{ReturnComplexity, get_return_complexity};
pub use s9b_shims::WriteCounts;
pub use specialization::{
    ParameterMatchesFn, SpecializationIndex, SpecializationInfo, SpecializationMap,
    Specializations, SpecializedCallKey, SpecializedCallMap, SpecializedFunctionKey,
    SpecializedParameters, UNSPECIALIZED, find_functions_to_specialize,
    find_specialization_index_for_call, find_specialized_parameters_for_function,
    get_parameter_mappings_for_function,
};
pub use switch_case_exit::{
    switch_case_contains_conditional_exit, switch_case_contains_unconditional_exit,
};
pub use symbol_table_checks::{SymbolTableStackBuilder, check_symbol_table_correctness};
pub use validate_indexing::validate_indexing_for_es2;
pub use var_declaration_scope::detect_var_declaration_without_scope;
