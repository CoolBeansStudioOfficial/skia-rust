// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLRenamePrivateSymbols.cpp (chrome/m156).

//! [`rename_private_symbols`]: gives the private functions and the local variables of a module
//! short names, and strips `$export` from its functions.

use super::{ProgramWriter, walk_program_element_mut};
use crate::analysis::SymbolTableStackBuilder;
use crate::context::Context;
use crate::ir::{
    ElemId, FnId, IrPool, ModifierFlags, ProgramElementKind, StatementKind, StmtId, SymTabId,
    SymbolId, VarId, rename_symbol,
};
use crate::program_settings::{ProgramConfig, ProgramKind};

/// The letters that `FindShortNameForSymbol` tries, in order: lower case, then upper case.
const LETTERS: [&str; 52] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x", "y", "z", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L",
    "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
];

/// `SymbolRenamer::FindShortNameForSymbol`: the first name, with `name_prefix`, that is not
/// already in `table` (or its parents). A single letter is tried before a pair of letters.
// Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L73-L103 (chrome/m156)
fn find_short_name_for_symbol(
    pool: &IrPool,
    table: SymTabId,
    name_prefix: &str,
    symbol_name: &str,
) -> String {
    // Try any single-letter option.
    for letter in LETTERS {
        let name = format!("{name_prefix}{letter}");
        if pool.find_symbol(table, &name).is_none() {
            return name;
        }
    }

    // Try every two-letter option.
    for letter_a in LETTERS {
        for letter_b in LETTERS {
            let name = format!("{name_prefix}{letter_a}{letter_b}");
            if pool.find_symbol(table, &name).is_none() {
                return name;
            }
        }
    }

    // We struck out. Somehow, all 2704 two-letter names have been claimed.
    #[cfg(debug_assertions)]
    panic!("Unable to find unique name for '{symbol_name}'");
    #[cfg(not(debug_assertions))]
    symbol_name.to_owned()
}

/// `strip_export_flag`: removes `$export` from every overload of the function `function` in
/// `symbols`.
// Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L42-L56 (chrome/m156)
fn strip_export_flag(ctx: &mut Context, function: FnId, symbols: SymTabId) {
    let name = ctx.pool.function(function).name.clone();
    // Remove `$export` from every overload of this function.
    let mut next = match ctx.pool.find_symbol(symbols, &name) {
        Some(SymbolId::FunctionDeclaration(first)) => Some(first),
        _ => None,
    };
    while let Some(overload) = next {
        let declaration = ctx.pool.function_mut(overload);
        declaration.modifier_flags.remove(ModifierFlags::EXPORT);
        next = declaration.next_overload;
    }
}

/// `SymbolRenamer`: renames the private functions and the local variables.
struct SymbolRenamer {
    /// `fSymbolTableStack`: the module's table, and the tables of the scopes being visited.
    symbol_table_stack: Vec<SymTabId>,
    /// `fKind`.
    kind: ProgramKind,
}

impl SymbolRenamer {
    /// The table of the innermost scope.
    fn top(&self) -> SymTabId {
        *self
            .symbol_table_stack
            .last()
            .expect("the symbol-table stack is never empty")
    }

    /// `minifyVariableName`: gives `var` a short name in the innermost scope. Anonymous variables
    /// (unnamed parameters) have nothing to shorten.
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L105-L132 (chrome/m156)
    fn minify_variable_name(&mut self, ctx: &mut Context, var: VarId) {
        let name = ctx.pool.variable(var).name.clone();
        // Some variables are associated with anonymous parameters--these don't have names and
        // aren't present in the symbol table. Their names are already empty so there's no way to
        // shrink them further.
        if name.is_empty() {
            return;
        }

        // Ensure that this variable is properly set up in the symbol table.
        let symbols = self.top();
        debug_assert_eq!(
            ctx.pool.find_symbol(symbols, &name),
            Some(SymbolId::Variable(var)),
            "symbol table missing '{name}'"
        );

        // Look for a new name for this symbol.
        // Note: we always rename _every_ variable, even ones with single-letter names. This is a
        // safeguard: if we claimed a name like `i`, and then the program itself contained an `i`
        // later on, in a nested SymbolTable, the two names would clash. By always renaming
        // everything, we can ignore that problem.
        let short_name = find_short_name_for_symbol(&ctx.pool, symbols, "", &name);
        debug_assert!(ctx.pool.find_symbol(symbols, &short_name).is_none());

        // Update the symbol's name.
        rename_symbol(ctx, symbols, SymbolId::Variable(var), &short_name);
    }

    /// `functionNameCanBeMinifiedSafely`: a runtime effect's only external function is `main`; a
    /// module's private functions are `$`-prefixed and not `$export`ed.
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L151-L160 (chrome/m156)
    fn function_name_can_be_minified_safely(&self, ctx: &Context, function: FnId) -> bool {
        let declaration = ctx.pool.function(function);
        if ProgramConfig::is_runtime_effect(self.kind) {
            // The only externally-accessible function in a runtime effect is main().
            !declaration.is_main
        } else {
            // We will only minify $private_functions, and only ones not marked as $export.
            declaration.name.starts_with('$') && !declaration.modifier_flags.is_export()
        }
    }

    /// `minifyFunctionName`: gives the private function `function` a shorter name. Every overload
    /// is renamed at once.
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L134-L149 (chrome/m156)
    fn minify_function_name(&mut self, ctx: &mut Context, function: FnId) {
        // Look for a new name for this function.
        let name_prefix = if ProgramConfig::is_runtime_effect(self.kind) {
            ""
        } else {
            "$"
        };
        let symbols = self.top();
        let name = ctx.pool.function(function).name.clone();
        let short_name = find_short_name_for_symbol(&ctx.pool, symbols, name_prefix, &name);
        debug_assert!(ctx.pool.find_symbol(symbols, &short_name).is_none());

        if short_name.len() < name.len() {
            // Update the function's name. (If the function has overloads, this will rename all of
            // them at once.)
            let symbol = ctx
                .pool
                .find_symbol(symbols, &name)
                .expect("a function is in the symbol table of its scope");
            rename_symbol(ctx, symbols, symbol, &short_name);
        }
    }

    /// `minifyPrototype`: a prototype without a definition loses the names of its parameters.
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L177-L190 (chrome/m156)
    fn minify_prototype(ctx: &mut Context, function: FnId) {
        if ctx.pool.function(function).definition.is_some() {
            // This function is defined somewhere; this isn't just a loose prototype.
            return;
        }
        // Eliminate the names of each function parameter. The parameter names aren't in the
        // symbol table's name lookup map at all. All we need to do is blank out their names.
        let parameters = ctx.pool.function(function).parameters.clone();
        for param in parameters {
            ctx.pool.variable_mut(param).name = "".into();
        }
    }

    /// `minifyFunction`: renames the function if it is private, then its parameters, in the scope
    /// of its body.
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L162-L175 (chrome/m156)
    fn minify_function(&mut self, ctx: &mut Context, function: FnId, body: StmtId) {
        // If the function is private, minify its name.
        if self.function_name_can_be_minified_safely(ctx, function) {
            self.minify_function_name(ctx, function);
        }

        // Minify the names of each function parameter.
        let builder =
            SymbolTableStackBuilder::new(&ctx.pool, Some(body), &mut self.symbol_table_stack);
        let parameters = ctx.pool.function(function).parameters.clone();
        for param in parameters {
            self.minify_variable_name(ctx, param);
        }
        builder.finish(&mut self.symbol_table_stack);
    }
}

impl ProgramWriter for SymbolRenamer {
    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L192-L205 (chrome/m156)
    fn visit_program_element(&mut self, ctx: &mut Context, element: ElemId) -> bool {
        match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => {
                let (function, body) = (def.declaration, def.body);
                self.minify_function(ctx, function, body);
                walk_program_element_mut(self, ctx, element)
            }
            ProgramElementKind::FunctionPrototype(proto) => {
                let function = proto.declaration;
                Self::minify_prototype(ctx, function);
                walk_program_element_mut(self, ctx, element)
            }
            _ => false,
        }
    }

    // Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L207-L217 (chrome/m156)
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        let builder =
            SymbolTableStackBuilder::new(&ctx.pool, Some(stmt), &mut self.symbol_table_stack);
        if let StatementKind::VarDeclaration(decl) = &ctx.pool.statement(stmt).kind {
            // Minify the variable's name.
            let var = decl.var;
            self.minify_variable_name(ctx, var);
        }

        let result = self.visit_statement(ctx, stmt);
        builder.finish(&mut self.symbol_table_stack);
        result
    }
}

/// `Transform::RenamePrivateSymbols(Context&, Module&, ProgramUsage*, ProgramKind)`: renames the
/// private functions and local variables of the module whose global table is `symbols`, then
/// strips `$export` from the module's functions. The usage is not read by the renamer, so it is
/// not a parameter here.
// Port of: src/sksl/transform/SkSLRenamePrivateSymbols.cpp#L58-L242 (chrome/m156)
#[doc(alias = "SkSL::Transform::RenamePrivateSymbols")]
pub fn rename_private_symbols(
    ctx: &mut Context,
    symbols: SymTabId,
    elements: &[ElemId],
    kind: ProgramKind,
) {
    // Rename local variables and private functions.
    let mut renamer = SymbolRenamer {
        symbol_table_stack: vec![symbols],
        kind,
    };
    for &element in elements {
        renamer.visit_program_element(ctx, element);
    }

    // Strip off modifier `$export` from every function. (Only the minifier checks this flag, so we
    // can remove it without affecting the meaning of the code.)
    for &element in elements {
        let function = match &ctx.pool.element(element).kind {
            ProgramElementKind::Function(def) => def.declaration,
            _ => continue,
        };
        if ctx.pool.function(function).modifier_flags.is_export() {
            strip_export_flag(ctx, function, symbols);
        }
    }
}
