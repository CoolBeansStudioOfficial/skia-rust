// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFieldSymbol.h.

//! [`FieldSymbol`]: a field of an anonymous interface block, visible as a global name.

use super::{
    IrPool,
    ids::{TypeId, VarId},
};
use crate::position::Position;

/// `SkSL::FieldSymbol`: names field `field_index` of the interface block variable `owner`.
// Port of: src/sksl/ir/SkSLFieldSymbol.h#L20-L57 (chrome/m156)
#[doc(alias = "SkSL::FieldSymbol")]
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSymbol {
    /// `fPosition`.
    pub position: Position,
    /// The symbol name: the field's name.
    pub name: Box<str>,
    /// The symbol type: the field's type.
    pub ty: TypeId,
    /// `fOwner`.
    pub owner: VarId,
    /// `fFieldIndex`.
    pub field_index: usize,
}

impl FieldSymbol {
    /// The `FieldSymbol` constructor: takes the name and type from field `field_index` of
    /// `owner`'s type.
    #[must_use]
    pub fn new(pool: &IrPool, position: Position, owner: VarId, field_index: usize) -> Self {
        let field = &pool.ty(pool.variable(owner).ty).fields()[field_index];
        Self {
            position,
            name: field.name.clone(),
            ty: field.ty,
            owner,
            field_index,
        }
    }

    /// `description()`: the field name, qualified by the owner when the owner has a name.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let owner = pool.variable(self.owner);
        if owner.name.is_empty() {
            self.name.to_string()
        } else {
            format!("{}.{}", owner.description(pool), self.name)
        }
    }
}
