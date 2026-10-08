// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSymbol.{h,cpp} (the shared `Symbol` accessors and
// `instantiate`).

//! The `Symbol` base-class accessors, over [`SymbolId`].

use super::{
    FieldAccess, FieldAccessOwnerKind, FunctionReference, IrPool, TypeReference, VariableRefKind,
    VariableReference,
    ids::{ExprId, SymbolId, TypeId},
};
use crate::context::Context;
use crate::position::Position;

/// `Symbol::Kind`.
#[doc(alias = "SkSL::SymbolKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Field,
    FunctionDeclaration,
    Type,
    Variable,
}

impl SymbolId {
    /// `kind()`.
    #[must_use]
    pub fn kind(self) -> SymbolKind {
        match self {
            Self::Type(_) => SymbolKind::Type,
            Self::Variable(_) => SymbolKind::Variable,
            Self::FunctionDeclaration(_) => SymbolKind::FunctionDeclaration,
            Self::Field(_) => SymbolKind::Field,
        }
    }

    /// `Symbol::instantiate(context, pos)`: an expression that refers to this symbol. A function
    /// gives a function reference, a variable a read of it, a field of an anonymous interface
    /// block the access to it, and a type a type reference.
    // Port of: src/sksl/ir/SkSLSymbol.cpp#L24-L52 (chrome/m156)
    pub fn instantiate(self, ctx: &mut Context, pos: Position) -> Option<ExprId> {
        match self {
            Self::FunctionDeclaration(function) => {
                Some(FunctionReference::make(ctx, pos, function))
            }
            Self::Variable(variable) => Some(VariableReference::make(
                &mut ctx.pool,
                pos,
                variable,
                VariableRefKind::Read,
            )),
            Self::Field(field) => {
                let (owner, field_index) = {
                    let symbol = ctx.pool.field_symbol(field);
                    (symbol.owner, symbol.field_index)
                };
                let base =
                    VariableReference::make(&mut ctx.pool, pos, owner, VariableRefKind::Read);
                Some(FieldAccess::make(
                    ctx,
                    pos,
                    base,
                    field_index,
                    FieldAccessOwnerKind::AnonymousInterfaceBlock,
                ))
            }
            Self::Type(ty) => TypeReference::convert(ctx, pos, ty),
        }
    }
}

impl IrPool {
    /// `Symbol::name()`.
    #[must_use]
    pub fn symbol_name(&self, symbol: SymbolId) -> &str {
        match symbol {
            SymbolId::Type(id) => &self.type_node(id).name,
            SymbolId::Variable(id) => &self.variable(id).name,
            SymbolId::FunctionDeclaration(id) => &self.function(id).name,
            SymbolId::Field(id) => &self.field_symbol(id).name,
        }
    }

    /// `Symbol::fPosition`.
    #[must_use]
    pub fn symbol_position(&self, symbol: SymbolId) -> Position {
        match symbol {
            SymbolId::Type(id) => self.type_node(id).position,
            SymbolId::Variable(id) => self.variable(id).position,
            SymbolId::FunctionDeclaration(id) => self.function(id).position,
            SymbolId::Field(id) => self.field_symbol(id).position,
        }
    }

    /// `Symbol::type()`: the type of a variable or field. Types and functions have none (Skia
    /// asserts).
    #[must_use]
    pub fn symbol_type(&self, symbol: SymbolId) -> Option<TypeId> {
        match symbol {
            SymbolId::Variable(id) => Some(self.variable(id).ty),
            SymbolId::Field(id) => Some(self.field_symbol(id).ty),
            SymbolId::Type(_) | SymbolId::FunctionDeclaration(_) => None,
        }
    }

    /// `Symbol::description()`.
    #[must_use]
    pub fn symbol_description(&self, symbol: SymbolId) -> String {
        match symbol {
            SymbolId::Type(id) => self.ty(id).description(),
            SymbolId::Variable(id) => self.variable(id).description(self),
            SymbolId::FunctionDeclaration(id) => self.function(id).description(self),
            SymbolId::Field(id) => self.field_symbol(id).description(self),
        }
    }
}
