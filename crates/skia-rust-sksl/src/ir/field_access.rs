// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFieldAccess.{h,cpp} (data and `description`). `Convert`,
// `Make` and `initialSlot` come with task S7b.

//! [`FieldAccess`]: `base.field`.

use super::{IrPool, ids::ExprId};
use crate::operator::OperatorPrecedence;

/// `SkSL::FieldAccessOwnerKind`: whether the base is a named value or an anonymous interface
/// block.
#[doc(alias = "SkSL::FieldAccessOwnerKind")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FieldAccessOwnerKind {
    #[default]
    Default,
    /// The base is an anonymous interface block, so the field is accessed by its bare name.
    AnonymousInterfaceBlock,
}

/// `SkSL::FieldAccess`. Its type is the field's type.
// Port of: src/sksl/ir/SkSLFieldAccess.h#L33-L105 (chrome/m156)
#[doc(alias = "SkSL::FieldAccess")]
#[derive(Clone, Debug, PartialEq)]
pub struct FieldAccess {
    /// `base()`.
    pub base: ExprId,
    /// `fieldIndex()`.
    pub field_index: usize,
    /// `ownerKind()`.
    pub owner_kind: FieldAccessOwnerKind,
}

impl FieldAccess {
    /// `description()`: `base.name`, or the bare `name` when the base prints as nothing.
    // Port of: src/sksl/ir/SkSLFieldAccess.cpp#L116-L122 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut f = pool.expression_description_with(self.base, OperatorPrecedence::Postfix);
        if !f.is_empty() {
            f.push('.');
        }
        let base_type = pool.expression(self.base).ty;
        f + &pool.ty(base_type).fields()[self.field_index].name
    }
}
