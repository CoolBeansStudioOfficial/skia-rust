// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSymbolTable.{h,cpp}: storage, lookup, `injectWithoutOwnership`,
// and (with task S6) adding with duplicate checks, renaming, removal, `moveSymbolTo`,
// `insertNewParent`, `addArrayDimension` and `wouldShadowSymbolsFrom`. `instantiateSymbolRef` needs
// `Symbol::instantiate` (S7b) and is not here yet.

//! [`SymbolTable`]: maps names to symbols, with a parent chain.

use std::borrow::Cow;
use std::collections::HashMap;

use super::{
    IrPool, Type,
    ids::{SymTabId, SymbolId, TypeId},
};
use crate::context::Context;

/// `SkSL::SymbolTable`. Symbols are owned by the pool, not by the table (Skia's
/// `fOwnedSymbols` and `takeOwnershipOfSymbol` reduce to allocating in the pool), so the table
/// only maps names to ids.
// Port of: src/sksl/ir/SkSLSymbolTable.h#L32-L207 (chrome/m156)
#[doc(alias = "SkSL::SymbolTable")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolTable {
    /// `fParent`.
    pub parent: Option<SymTabId>,
    /// `fBuiltin`: a table of a built-in module.
    pub builtin: bool,
    /// `fAtModuleBoundary`: the parent table belongs to a different module.
    pub at_module_boundary: bool,
    /// `fSymbols`. Skia's `THashMap` here is only iterated by the debug-only
    /// `CheckSymbolTableCorrectness`, which is not ported, so a `std` map is safe.
    symbols: HashMap<Box<str>, SymbolId>,
}

impl SymbolTable {
    /// `SymbolTable(parent, builtin)`.
    #[must_use]
    pub fn new(parent: Option<SymTabId>, builtin: bool) -> Self {
        Self {
            parent,
            builtin,
            at_module_boundary: false,
            symbols: HashMap::new(),
        }
    }

    /// The symbol `name` maps to in this table only (no parent lookup).
    #[must_use]
    pub fn find_local(&self, name: &str) -> Option<SymbolId> {
        self.symbols.get(name).copied()
    }

    /// Maps `name` to `symbol`, replacing any symbol of that name in this table, and returns the
    /// previous symbol. This is the raw `fSymbols[key] = symbol` that Skia's `add*` and
    /// `inject*` methods end in.
    pub fn set(&mut self, name: &str, symbol: SymbolId) -> Option<SymbolId> {
        self.symbols.insert(name.into(), symbol)
    }

    /// Removes `name` from this table.
    pub fn remove(&mut self, name: &str) -> Option<SymbolId> {
        self.symbols.remove(name)
    }

    /// `count()`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.symbols.len()
    }

    /// `isBuiltin()`.
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.builtin
    }

    /// `markModuleBoundary()`.
    pub fn mark_module_boundary(&mut self) {
        self.at_module_boundary = true;
    }
}

impl IrPool {
    /// `SymbolTable::find(name)` (`lookup`): the symbol `name` maps to in `table` or its
    /// parents.
    // Port of: src/sksl/ir/SkSLSymbolTable.cpp#L66-L75 (chrome/m156)
    #[must_use]
    pub fn find_symbol(&self, table: SymTabId, name: &str) -> Option<SymbolId> {
        let mut table = Some(table);
        while let Some(id) = table {
            let t = self.symbol_table(id);
            if let Some(symbol) = t.find_local(name) {
                return Some(symbol);
            }
            table = t.parent;
        }
        None
    }

    /// `SymbolTable::findBuiltinSymbol(name)`: looks `name` up starting at the nearest built-in
    /// table.
    // Port of: src/sksl/ir/SkSLSymbolTable.cpp#L59-L64 (chrome/m156)
    #[must_use]
    pub fn find_builtin_symbol(&self, table: SymTabId, name: &str) -> Option<SymbolId> {
        let t = self.symbol_table(table);
        if !t.is_builtin() {
            return t.parent.and_then(|p| self.find_builtin_symbol(p, name));
        }
        self.find_symbol(table, name)
    }

    /// `SymbolTable::isType(name)`.
    #[must_use]
    pub fn is_type(&self, table: SymTabId, name: &str) -> bool {
        matches!(self.find_symbol(table, name), Some(SymbolId::Type(_)))
    }

    /// `SymbolTable::isBuiltinType(name)`.
    #[must_use]
    pub fn is_builtin_type(&self, table: SymTabId, name: &str) -> bool {
        let t = self.symbol_table(table);
        if !t.is_builtin() {
            return t.parent.is_some_and(|p| self.is_builtin_type(p, name));
        }
        self.is_type(table, name)
    }

    /// `SymbolTable::injectWithoutOwnership(symbol)`: maps the symbol's name to it in `table`,
    /// replacing any existing symbol of that name.
    // Port of: src/sksl/ir/SkSLSymbolTable.cpp#L171-L174 (chrome/m156)
    pub fn inject_symbol(&mut self, table: SymTabId, symbol: SymbolId) {
        let name: Box<str> = self.symbol_name(symbol).into();
        self.symbol_table_mut(table).set(&name, symbol);
    }
}

impl SymbolTable {
    /// The names this table maps, not counting its parents.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.symbols.keys().map(Box::as_ref)
    }
}

/// `Symbol::setName`: gives `symbol` a new name. Overload chains are renamed by the caller.
fn set_symbol_name(pool: &mut IrPool, symbol: SymbolId, name: &str) {
    match symbol {
        SymbolId::Type(id) => pool.type_mut(id).name = Cow::Owned(name.to_owned()),
        SymbolId::Variable(id) => pool.variable_mut(id).name = name.into(),
        SymbolId::FunctionDeclaration(id) => pool.function_mut(id).name = name.into(),
        SymbolId::Field(id) => pool.field_symbol_mut(id).name = name.into(),
    }
}

/// `SymbolTable::addWithoutOwnership(symbol)`: maps the symbol's name in `table`. Returns false
/// when a global symbol of the name already exists in a parent module, and when the name was
/// already taken in `table` (the new symbol then replaces the old one, as in Skia).
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L143-L169 (chrome/m156)
fn insert_symbol(pool: &mut IrPool, table: SymTabId, symbol: SymbolId) -> bool {
    let name: Box<str> = pool.symbol_name(symbol).into();
    if name.is_empty() {
        // Anonymous symbols, such as unnamed function parameters, are not in the table.
        return true;
    }
    // A function joins the overload chain of an existing function of the same name.
    if let SymbolId::FunctionDeclaration(function) = symbol
        && let Some(SymbolId::FunctionDeclaration(existing)) = pool.find_symbol(table, &name)
    {
        pool.function_mut(function).next_overload = Some(existing);
        pool.symbol_table_mut(table).set(&name, symbol);
        return true;
    }
    let (at_boundary, parent) = {
        let t = pool.symbol_table(table);
        (t.at_module_boundary, t.parent)
    };
    if at_boundary
        && let Some(parent) = parent
        && pool.find_symbol(parent, &name).is_some()
    {
        // A global symbol that already exists in a parent module is a duplicate.
        return false;
    }
    // `std::swap(symbol, fSymbols[key])`: the table takes the new symbol, and the add succeeds
    // only if the name was free.
    pool.symbol_table_mut(table).set(&name, symbol).is_none()
}

/// `SymbolTable::add(context, symbol)` (and `addWithoutOwnership(context, symbol)`): maps the
/// symbol's name in `table`, reporting a duplicate. Ownership is the pool's, so `add` and
/// `addWithoutOwnership` are one function here.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L122-L130 (chrome/m156)
pub fn add_symbol(ctx: &mut Context, table: SymTabId, symbol: SymbolId) {
    if !insert_symbol(&mut ctx.pool, table, symbol) {
        let name = ctx.pool.symbol_name(symbol).to_owned();
        let pos = ctx.pool.symbol_position(symbol);
        ctx.errors
            .error(pos, &format!("symbol '{name}' was already defined"));
    }
}

/// `SymbolTable::renameSymbol(context, symbol, newName)`: renames `symbol` (every overload of a
/// function) and maps it under the new name in `table`.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L93-L107 (chrome/m156)
pub fn rename_symbol(ctx: &mut Context, table: SymTabId, symbol: SymbolId, new_name: &str) {
    if let SymbolId::FunctionDeclaration(first) = symbol {
        // This is a function declaration, so the entire overload set is renamed.
        let mut next = Some(first);
        while let Some(function) = next {
            ctx.pool.function_mut(function).name = new_name.into();
            next = ctx.pool.function(function).next_overload;
        }
    } else {
        // Other symbols don't allow two symbols with the same name.
        set_symbol_name(&mut ctx.pool, symbol, new_name);
    }
    add_symbol(ctx, table, symbol);
}

/// `SymbolTable::removeSymbol(symbol)`: drops the name of `symbol` from `table`. Skia also hands
/// back the owning pointer; ownership is the pool's here, so there is nothing to hand back.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L109-L121 (chrome/m156)
pub fn remove_symbol(pool: &mut IrPool, table: SymTabId, symbol: SymbolId) {
    let name: Box<str> = pool.symbol_name(symbol).into();
    pool.symbol_table_mut(table).remove(&name);
}

/// `SymbolTable::moveSymbolTo(otherTable, symbol, context)`: moves `symbol` from `from` to `to`.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L123-L131 (chrome/m156)
pub fn move_symbol_to(ctx: &mut Context, from: SymTabId, to: SymTabId, symbol: SymbolId) {
    remove_symbol(&mut ctx.pool, from, symbol);
    add_symbol(ctx, to, symbol);
}

/// `SymbolTable::insertNewParent()`: inserts a table between `table` and its parent, and returns
/// the new table.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L15-L20 (chrome/m156)
pub fn insert_new_parent(pool: &mut IrPool, table: SymTabId) -> SymTabId {
    let (parent, builtin) = {
        let t = pool.symbol_table(table);
        (t.parent, t.is_builtin())
    };
    let new_table = pool.add_symbol_table(SymbolTable::new(parent, builtin));
    pool.symbol_table_mut(table).parent = Some(new_table);
    new_table
}

/// `SymbolTable::wouldShadowSymbolsFrom(other)`: whether the two tables, not counting their
/// parents, share a name. The result does not depend on which table is walked.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L35-L57 (chrome/m156)
#[must_use]
pub fn would_shadow_symbols_from(pool: &IrPool, table: SymTabId, other: SymTabId) -> bool {
    let (mut small, mut large) = (pool.symbol_table(table), pool.symbol_table(other));
    if small.count() > large.count() {
        std::mem::swap(&mut small, &mut large);
    }
    small.names().any(|name| large.find_local(name).is_some())
}

/// `SymbolTable::addArrayDimension(context, type, arraySize)`: the type of `array_size` elements
/// of `ty`. An array type already in scope with the same name is reused. A built-in element type
/// gets its array type added as high in the table tree as the module boundary allows.
// Port of: src/sksl/ir/SkSLSymbolTable.cpp#L77-L108 (chrome/m156)
pub fn add_array_dimension(
    ctx: &mut Context,
    table: SymTabId,
    ty: TypeId,
    array_size: i32,
) -> TypeId {
    if array_size == 0 {
        return ty;
    }
    // If we are making an array of a builtin type, we add it as high as possible in the symbol
    // table tree (at the module boundary), to enable additional reuse of the array-type.
    let (parent, at_boundary) = {
        let t = ctx.pool.symbol_table(table);
        (t.parent, t.at_module_boundary)
    };
    if let Some(parent) = parent
        && !at_boundary
        && !ctx.config().is_builtin_code()
        && ctx.pool.ty(ty).is_builtin()
    {
        return add_array_dimension(ctx, parent, ty, array_size);
    }

    // Reuse an existing array type with this name if one already exists in scope. Skia checks the
    // symbol as a type without a test, but a program with duplicate names must not reuse a
    // non-type symbol, so only a type can match.
    let name = ctx.pool.ty(ty).array_name(array_size);
    if let Some(SymbolId::Type(existing)) = ctx.pool.find_symbol(table, &name) {
        let existing_ty = ctx.pool.ty(existing);
        // Verify the match: a program may declare duplicate names.
        if existing_ty.is_array() && ctx.pool.ty(ty).matches(existing_ty.component_type().id()) {
            return existing;
        }
    }

    // Add a new array type to the table.
    let array = Type::make_array_type(ctx, &name, ty, array_size);
    let id = ctx.pool.add_type(array);
    add_symbol(ctx, table, SymbolId::Type(id));
    id
}
