// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLStructDefinition.{h,cpp} (`Convert` and `Make`).

//! The factories of [`StructDefinition`].

use super::{
    Field, IrPool, ProgramElement, ProgramElementKind, StructDefinition, Type, add_symbol,
    ids::{ElemId, SymbolId, TypeId},
};
use crate::context::Context;
use crate::position::Position;

/// Adds a type to the current symbol table (`context.fSymbolTable->add(context, type)`), and
/// returns its id. A duplicate name is reported by the table.
pub(super) fn add_type_symbol(ctx: &mut Context, ty: Type) -> TypeId {
    let id = ctx.pool.add_type(ty);
    let table = ctx
        .symbol_table
        .expect("a type symbol is added to the current symbol table");
    add_symbol(ctx, table, SymbolId::Type(id));
    id
}

impl StructDefinition {
    /// `StructDefinition::Convert`: a struct type with `fields`, added to the current symbol table.
    // Port of: src/sksl/ir/SkSLStructDefinition.cpp#L15-L23 (chrome/m156)
    #[must_use]
    pub fn convert(ctx: &mut Context, pos: Position, name: &str, fields: Vec<Field>) -> ElemId {
        let owned_type = Type::make_struct_type(ctx, pos, name, fields, false);
        let ty = add_type_symbol(ctx, owned_type);
        Self::make(&mut ctx.pool, pos, ty)
    }

    /// `StructDefinition::Make`: the element that defines `ty`.
    // Port of: src/sksl/ir/SkSLStructDefinition.cpp#L25-L28 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position, ty: TypeId) -> ElemId {
        pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::StructDefinition(Self { ty }),
        ))
    }
}
