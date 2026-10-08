// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform.

//! IR rewrites. The [`ProgramWriter`] traversal (task S5), and the switch transform that
//! `SwitchStatement::Convert` uses (task S7d). The other transforms come with tasks S11 and S13.

mod hoist_switch_var_declarations;
mod program_writer;

pub use hoist_switch_var_declarations::hoist_switch_var_declarations_at_top_level;
pub use program_writer::{
    ProgramWriter, walk_expression_mut, walk_program_element_mut, walk_statement_mut,
};
