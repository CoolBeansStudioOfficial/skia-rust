// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLMangler.{h,cpp}.

//! [`Mangler`]: makes the unique names the inliner gives its temporaries (`_0_x`, `_1_result`).

use crate::ir::{IrPool, SymTabId};

/// `SkSL::Mangler`.
// Port of: src/sksl/SkSLMangler.h#L18-L32 (chrome/m156)
#[doc(alias = "SkSL::Mangler")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mangler {
    counter: i32,
}

/// The size of Skia's name buffer: names are cut to fit 256 bytes.
const UNIQUE_NAME_CAPACITY: usize = 256;

impl Mangler {
    /// A mangler whose counter starts at 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.counter = 0;
    }

    /// `uniqueName(baseName, symbolTable)`: `_<counter>_<baseName>`, with the counter advanced
    /// until the name is not found in `symbol_table` (or its parents). A leading `$` and an
    /// earlier mangler prefix (`_12_`) are stripped first; a lone leading `_` is dropped.
    // Port of: src/sksl/SkSLMangler.cpp#L18-L74 (chrome/m156)
    pub fn unique_name(
        &mut self,
        base_name: &str,
        pool: &IrPool,
        symbol_table: SymTabId,
    ) -> String {
        let mut base_name = base_name.as_bytes();
        // Private names might begin with a $. Strip that off.
        if let Some(rest) = base_name.strip_prefix(b"$") {
            base_name = rest;
        }
        // The inliner runs more than once, so the base name might already have been mangled
        // and have a prefix like "_123_x". Strip that prefix off to make the generated code
        // easier to read.
        if base_name.first() == Some(&b'_') {
            // Bytes past the end read as the NUL terminator, as in Skia.
            let at = |i: usize| base_name.get(i).copied().unwrap_or(0);
            // Determine if we have a string of digits.
            let mut offset = 1;
            while at(offset).is_ascii_digit() {
                offset += 1;
            }
            // If we found digits, another underscore, and anything else, that's the mangler
            // prefix. Strip it off.
            if offset > 1 && at(offset) == b'_' && at(offset + 1) != 0 {
                base_name = &base_name[offset + 1..];
            } else {
                // This name doesn't contain a mangler prefix, but it does start with an
                // underscore. OpenGL disallows two consecutive underscores anywhere in the
                // string, and we'll be adding one as part of the mangler prefix, so strip the
                // leading underscore.
                base_name = &base_name[1..];
            }
        }
        // Append a unique numeric prefix to avoid name overlap. Check the symbol table to make
        // sure we're not reusing an existing name.
        loop {
            let mut unique_name = format!("_{}_", self.counter).into_bytes();
            self.counter += 1;
            let copy_len = base_name
                .len()
                .min(UNIQUE_NAME_CAPACITY.saturating_sub(unique_name.len()));
            unique_name.extend_from_slice(&base_name[..copy_len]);
            let unique_name = String::from_utf8_lossy(&unique_name).into_owned();
            if pool.find_symbol(symbol_table, &unique_name).is_none() {
                return unique_name;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mangler;
    use crate::ir::{
        IrPool, ModifierFlags, SymbolId, SymbolTable, TypeId, Variable, VariableStorage,
    };
    use crate::position::Position;

    #[test]
    fn unique_names_count_up_and_strip_old_prefixes() {
        let mut pool = IrPool::new();
        let table = pool.add_symbol_table(SymbolTable::new(None, false));
        let mut mangler = Mangler::new();
        // As in the inliner goldens (tests/sksl/inliner/*.glsl): `_0_x`, `_1_c`, …
        assert_eq!(mangler.unique_name("x", &pool, table), "_0_x");
        assert_eq!(mangler.unique_name("$private", &pool, table), "_1_private");
        // An earlier mangler prefix is replaced, not stacked.
        assert_eq!(mangler.unique_name("_0_x", &pool, table), "_2_x");
        // A lone leading underscore is dropped (no `__` in GLSL).
        assert_eq!(mangler.unique_name("_foo", &pool, table), "_3_foo");
        // `_12_` alone is not a prefix: nothing follows it.
        assert_eq!(mangler.unique_name("_12_", &pool, table), "_4_12_");
        // Names already in the symbol table (or its parents) are skipped.
        let taken = pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            ModifierFlags::empty(),
            "_5_y",
            TypeId::INT,
            false,
            VariableStorage::Local,
        ));
        pool.inject_symbol(table, SymbolId::Variable(taken));
        let child = pool.add_symbol_table(SymbolTable::new(Some(table), false));
        assert_eq!(mangler.unique_name("y", &pool, child), "_6_y");
        mangler.reset();
        assert_eq!(mangler.unique_name("y", &pool, child), "_0_y");
    }

    #[test]
    fn long_names_are_cut_to_256_bytes() {
        let mut pool = IrPool::new();
        let table = pool.add_symbol_table(SymbolTable::new(None, false));
        let long = "n".repeat(300);
        let name = Mangler::new().unique_name(&long, &pool, table);
        assert_eq!(name.len(), 256);
        assert!(name.starts_with("_0_nnn"));
    }
}
