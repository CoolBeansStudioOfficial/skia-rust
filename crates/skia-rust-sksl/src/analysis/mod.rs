// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis and src/sksl/SkSLAnalysis.cpp.

//! Read-only analyses of the IR. So far: the [`ProgramVisitor`] traversal (task S5); the
//! analyses themselves come with tasks S9a and S9b.

mod program_visitor;

pub use program_visitor::{ProgramVisitor, walk_expression, walk_program_element, walk_statement};
