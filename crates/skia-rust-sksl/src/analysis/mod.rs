// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis and src/sksl/SkSLAnalysis.cpp.

//! Analyses of the IR. [`ProgramVisitor`] is the traversal (task S5). Task S9a adds
//! [`ProgramUsage`] and the queries that use it, the expression predicates (constant, trivial,
//! same-tree, side effects, dynamically uniform), the sample-usage analysis, the assignability
//! checks, and the helpers of `SkSLAnalysis.cpp`. Task S9b adds the finalization checks and the
//! loop analyses.

mod assignment;
mod can_exit_without_returning_value;
mod check_program_structure;
mod expression_queries;
mod finalization_checks;
mod loop_info;
mod program_usage;
mod program_visitor;
mod return_complexity;
mod returns_input_alpha;
mod sample_usage;
mod specialization;
mod statement_queries;
mod switch_case_exit;
mod symbol_table_checks;
mod symbol_table_stack;
mod validate_indexing;

#[cfg(test)]
mod s9b_tests;
#[cfg(test)]
mod tests;

pub use assignment::{AssignmentInfo, is_assignable, update_variable_ref_kind};
pub use can_exit_without_returning_value::can_exit_without_returning_value;
pub use check_program_structure::check_program_structure;
pub use expression_queries::{
    contains_rt_adjust, contains_variable, get_root_variable, has_side_effects,
    is_compile_time_constant, is_constant_expression, is_dynamically_uniform_expression,
    is_same_expression_tree, is_trivial_expression,
};
pub use finalization_checks::do_finalization_checks;
pub use loop_info::{LoopControlFlowInfo, get_loop_control_flow_info, get_loop_unroll_info};
pub use program_usage::{ProgramUsage, VariableCounts, get_module_usage, get_usage};
pub use program_visitor::{ProgramVisitor, walk_expression, walk_program_element, walk_statement};
pub use return_complexity::{ReturnComplexity, get_return_complexity};
pub use returns_input_alpha::returns_input_alpha;
pub use sample_usage::{
    SampleUsage, SampleUsageKind, calls_color_transform_intrinsics, calls_sample_outside_main,
    get_sample_usage, references_builtin, references_frag_coords, references_sample_coords,
};
pub use specialization::{
    ParameterMatchesFn, SpecializationIndex, SpecializationInfo, SpecializationMap,
    Specializations, SpecializedCallKey, SpecializedCallMap, SpecializedFunctionKey,
    SpecializedParameters, UNSPECIALIZED, find_functions_to_specialize,
    find_specialization_index_for_call, find_specialized_parameters_for_function,
    get_parameter_mappings_for_function,
};
pub use statement_queries::{
    detect_var_declaration_without_scope, node_count_up_to_limit, statement_writes_to_variable,
};
pub use switch_case_exit::{
    switch_case_contains_conditional_exit, switch_case_contains_unconditional_exit,
};
pub use symbol_table_checks::check_symbol_table_correctness;
pub use symbol_table_stack::SymbolTableStackBuilder;
pub use validate_indexing::validate_indexing_for_es2;
