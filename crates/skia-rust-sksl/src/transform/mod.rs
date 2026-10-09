// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform.

//! IR rewrites. The [`ProgramWriter`] traversal (task S5), the switch transform that
//! `SwitchStatement::Convert` uses (task S7d), the built-in declarations that finalization copies
//! into a program (task S11), and the optimizer transforms (task S13).

mod add_const_to_var_modifiers;
mod eliminate_dead_functions;
mod eliminate_dead_global_variables;
mod eliminate_dead_local_variables;
mod eliminate_empty_statements;
mod eliminate_unnecessary_braces;
mod eliminate_unreachable_code;
mod find_and_declare;
mod hoist_switch_var_declarations;
mod program_writer;
mod rename_private_symbols;
mod replace_const_vars_with_literals;
mod replace_splat_casts_with_swizzles;
mod rewrite_indexed_swizzle;
#[cfg(test)]
mod tests;

use crate::ir::{IrPool, Nop, Statement, StatementKind, StmtId};
use crate::position::Position;

pub use add_const_to_var_modifiers::add_const_to_var_modifiers;
pub use eliminate_dead_functions::{eliminate_dead_functions, eliminate_dead_functions_in_program};
pub use eliminate_dead_global_variables::{
    eliminate_dead_global_variables, eliminate_dead_global_variables_in_program,
};
pub use eliminate_dead_local_variables::{
    eliminate_dead_local_variables, eliminate_dead_local_variables_in_program,
};
pub use eliminate_empty_statements::eliminate_empty_statements;
pub use eliminate_unnecessary_braces::eliminate_unnecessary_braces;
pub use eliminate_unreachable_code::eliminate_unreachable_code;
pub use find_and_declare::{
    find_and_declare_builtin_functions, find_and_declare_builtin_structs,
    find_and_declare_builtin_variables,
};
pub use hoist_switch_var_declarations::hoist_switch_var_declarations_at_top_level;
pub use program_writer::{
    ProgramWriter, walk_expression_mut, walk_program_element_mut, walk_statement_mut,
};
pub use rename_private_symbols::rename_private_symbols;
pub use replace_const_vars_with_literals::replace_const_vars_with_literals;
pub use replace_splat_casts_with_swizzles::replace_splat_casts_with_swizzles;
pub use rewrite_indexed_swizzle::rewrite_indexed_swizzle;

/// Overwrites the statement at `stmt` with a `Nop` (Skia: `stmt = Nop::Make()` on the owning
/// `unique_ptr`). The `Nop` is at an invalid position, as `Nop::Make` leaves it.
// Port of: src/sksl/ir/SkSLNop.h#L26-L28 (chrome/m156), in the slot form the transforms use.
pub(crate) fn replace_with_nop(pool: &mut IrPool, stmt: StmtId) {
    pool.replace_statement(
        stmt,
        Statement::new(Position::default(), StatementKind::Nop(Nop)),
    );
}
