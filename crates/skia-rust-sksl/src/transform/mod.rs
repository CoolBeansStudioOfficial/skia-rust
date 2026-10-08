// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform.

//! IR rewrites. The [`ProgramWriter`] traversal (task S5), the switch transform that
//! `SwitchStatement::Convert` uses (task S7d), and the built-in declarations that finalization
//! copies into a program (task S11). The optimizer transforms come with task S13.

mod find_and_declare;
mod hoist_switch_var_declarations;
mod program_writer;

pub use find_and_declare::{
    find_and_declare_builtin_functions, find_and_declare_builtin_structs,
    find_and_declare_builtin_variables,
};
pub use hoist_switch_var_declarations::hoist_switch_var_declarations_at_top_level;
pub use program_writer::{
    ProgramWriter, walk_expression_mut, walk_program_element_mut, walk_statement_mut,
};
