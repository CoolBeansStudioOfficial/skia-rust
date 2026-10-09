// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLContext.{h,cpp}.

//! [`Context`]: the state that every IR conversion function reads and mutates.

use std::sync::Arc;

use crate::error_reporter::ErrorReporter;
use crate::ir::{IrPool, Program, SymTabId};
use crate::modules::Module;
use crate::program_settings::ProgramConfig;

/// `SkSL::Context`.
///
/// Skia passes `const Context&` everywhere and mutates through its pointers. Here conversion
/// functions take `&mut Context`, which owns everything they touch: the pool being built, the
/// error reporter, the configuration, the module and the current symbol table. Skia's
/// `fTypes` (`BuiltinTypes`) is the set of `TypeId` constants.
// Port of: src/sksl/SkSLContext.h#L22-L50 (chrome/m156)
#[doc(alias = "SkSL::Context")]
#[derive(Debug)]
pub struct Context {
    /// `fConfig`: the configuration of the program or module being compiled.
    pub config: Option<ProgramConfig>,
    /// `fErrors`: the current error reporter.
    pub errors: ErrorReporter,
    /// `fModule`: the module with the built-in declarations.
    pub module: Option<Arc<Module>>,
    /// `fSymbolTable`: the current symbol table (in `pool` or its parents).
    pub symbol_table: Option<SymTabId>,
    /// The pool new IR is allocated in (the program's or module's; Skia's thread-attached
    /// `Pool`).
    pub pool: IrPool,
}

impl Context {
    /// `Context(types, errors)`, with an empty root pool.
    #[must_use]
    pub fn new(errors: ErrorReporter) -> Self {
        Self {
            config: None,
            errors,
            module: None,
            symbol_table: None,
            pool: IrPool::new(),
        }
    }

    /// `fConfig`, which must be set while code is compiled.
    ///
    /// # Panics
    ///
    /// If no configuration is set (Skia: `SkASSERT(context.fConfig)`).
    #[must_use]
    pub fn config(&self) -> &ProgramConfig {
        self.config
            .as_ref()
            .expect("Context: no ProgramConfig is set")
    }

    /// `setErrorReporter(e)`: installs `errors` and returns the previous reporter (the parser's
    /// checkpoints swap a forwarding reporter in and back out).
    pub fn set_error_reporter(&mut self, errors: ErrorReporter) -> ErrorReporter {
        std::mem::replace(&mut self.errors, errors)
    }

    /// Runs `f` with `program`'s pool, configuration and symbol table installed in this
    /// context, then hands them back to the program. Optimization and finalization passes
    /// (Skia: `Transform::X(program)` with `*program.fContext`) run this way.
    pub fn with_program<R>(&mut self, program: &mut Program, f: impl FnOnce(&mut Self) -> R) -> R {
        let pool = std::mem::take(&mut program.pool);
        let saved_pool = std::mem::replace(&mut self.pool, pool);
        let saved_config = self.config.replace(program.config);
        let saved_symbols = self.symbol_table.replace(program.symbols);
        let result = f(self);
        program.pool = std::mem::replace(&mut self.pool, saved_pool);
        if let Some(config) = std::mem::replace(&mut self.config, saved_config) {
            program.config = config;
        }
        self.symbol_table = saved_symbols;
        result
    }
}
