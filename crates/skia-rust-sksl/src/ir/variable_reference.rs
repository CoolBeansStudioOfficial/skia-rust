// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLVariableReference.{h,cpp}.

//! [`VariableReference`]: a use of a variable.

use super::{
    Expression, ExpressionKind, IrPool,
    ids::{ExprId, VarId},
};
use crate::position::Position;

/// `SkSL::VariableRefKind`: how a reference uses the variable.
#[doc(alias = "SkSL::VariableRefKind")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VariableRefKind {
    #[default]
    Read,
    Write,
    ReadWrite,
    /// Taking the variable's address (an `out` argument).
    Pointer,
}

/// `SkSL::VariableReference`. Its type is the variable's.
// Port of: src/sksl/ir/SkSLVariableReference.h#L36-L94 (chrome/m156)
#[doc(alias = "SkSL::VariableReference")]
#[derive(Clone, Debug, PartialEq)]
pub struct VariableReference {
    /// `variable()`.
    pub variable: VarId,
    /// `refKind()`.
    pub ref_kind: VariableRefKind,
}

impl VariableReference {
    /// `Make(pos, variable, refKind)`: a reference to `variable`. Its type is the variable's.
    // Port of: src/sksl/ir/SkSLVariableReference.h#L49-L56 (chrome/m156)
    #[must_use]
    pub fn make(
        pool: &mut IrPool,
        pos: Position,
        variable: VarId,
        ref_kind: VariableRefKind,
    ) -> ExprId {
        let ty = pool.variable(variable).ty;
        pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::VariableReference(Self { variable, ref_kind }),
        ))
    }

    /// `description()`: the variable's name.
    // Port of: src/sksl/ir/SkSLVariableReference.cpp#L21-L23 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        pool.variable(self.variable).name.to_string()
    }
}
