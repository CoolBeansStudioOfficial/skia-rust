// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSymbolTable.{h,cpp} (storage, lookup and
// `injectWithoutOwnership`). Adding with duplicate checks, renaming, removal, `insertNewParent`,
// `addArrayDimension`, `instantiateSymbolRef` and `wouldShadowSymbolsFrom` come with task S6.

//! [`SymbolTable`]: maps names to symbols, with a parent chain.

use std::collections::HashMap;

use super::{
    IrPool,
    ids::{SymTabId, SymbolId},
};

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
