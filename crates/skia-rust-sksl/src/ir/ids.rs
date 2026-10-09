// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// skia-rust: the typed ids that replace Skia's IR pointers (`docs/design/sksl.md` §4.1 and
// "As implemented in S5").

//! Typed ids of IR nodes. Every node lives in an [`IrPool`](super::IrPool) and is named by one of
//! these `Copy` ids. An id is the identity of a node's *slot*: Skia's `std::unique_ptr<T>&` (a
//! place that owns a node) and its `const T*` (the node) both become the same id, and replacing a
//! node overwrites the pool entry at that id.

/// Declares one id newtype.
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub(crate) u32);

        impl $name {
            /// The raw index of this id across its pool chain (for diagnostics and hashing).
            #[must_use]
            pub const fn index(self) -> u32 {
                self.0
            }
        }
    };
}

define_id!(
    /// An [`Expression`](super::Expression) (Skia: `std::unique_ptr<Expression>` / `Expression*`).
    ExprId
);
define_id!(
    /// A [`Statement`](super::Statement) (Skia: `std::unique_ptr<Statement>` / `Statement*`).
    StmtId
);
define_id!(
    /// A [`ProgramElement`](super::ProgramElement).
    ElemId
);
define_id!(
    /// A [`Type`](super::Type). Built-in types have fixed ids (`TypeId::FLOAT`, …), see
    /// [`builtin_types`](crate::builtin_types).
    TypeId
);
define_id!(
    /// A [`Variable`](super::Variable).
    VarId
);
define_id!(
    /// A [`FunctionDeclaration`](super::FunctionDeclaration).
    FnId
);
define_id!(
    /// A [`FieldSymbol`](super::FieldSymbol) (a field of an anonymous interface block).
    FieldId
);
define_id!(
    /// A [`SymbolTable`](super::SymbolTable).
    SymTabId
);

/// `SkSL::Symbol*`: any symbol, by kind. Skia's `SymbolKind::kExternal` has no node at m156 and
/// is not represented.
#[doc(alias = "SkSL::Symbol")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolId {
    /// `Symbol::Kind::kType`.
    Type(TypeId),
    /// `Symbol::Kind::kVariable`.
    Variable(VarId),
    /// `Symbol::Kind::kFunctionDeclaration`.
    FunctionDeclaration(FnId),
    /// `Symbol::Kind::kField`.
    Field(FieldId),
}

impl From<TypeId> for SymbolId {
    fn from(id: TypeId) -> Self {
        Self::Type(id)
    }
}

impl From<VarId> for SymbolId {
    fn from(id: VarId) -> Self {
        Self::Variable(id)
    }
}

impl From<FnId> for SymbolId {
    fn from(id: FnId) -> Self {
        Self::FunctionDeclaration(id)
    }
}

impl From<FieldId> for SymbolId {
    fn from(id: FieldId) -> Self {
        Self::Field(id)
    }
}
