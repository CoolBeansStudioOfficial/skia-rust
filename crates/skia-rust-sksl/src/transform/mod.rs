// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform.

//! IR rewrites. So far: the [`ProgramWriter`] traversal (task S5); the transforms come with tasks
//! S11 and S13.

mod program_writer;

pub use program_writer::{
    ProgramWriter, walk_expression_mut, walk_program_element_mut, walk_statement_mut,
};
