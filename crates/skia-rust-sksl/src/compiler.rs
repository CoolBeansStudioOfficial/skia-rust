// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLCompiler.{h,cpp}: the error text (`handleError`, `errorText`,
// `writeErrorCount`, `resetErrors`) and `POISON_TAG`, and the driver (`initializeContext`,
// `FinalizeSettings`, `convertProgram`, `compileModule`, `releaseProgram`, `finalize`; task S11).
// The optimizer (`optimize`, `optimizeModuleBeforeMinifying`) runs the dead-code transforms of
// task S13; the inliner comes with task S12.

//! [`Compiler`]: owns the compilation [`Context`] and formats its errors.

use std::sync::Arc;

use crate::analysis::{
    ProgramUsage, check_program_structure, do_finalization_checks, get_module_parts_usage,
    get_usage, validate_indexing_for_es2,
};
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::flavor::Flavor;
use crate::inliner::Inliner;
use crate::ir::{ElemId, IrPool, Program, ProgramInterface, SymTabId, SymbolTable};
use crate::module_loader::ModuleLoader;
use crate::modules::{Module, ModuleType};
use crate::parser::Parser;
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};
use crate::transform::{
    eliminate_dead_functions, eliminate_dead_functions_in_program, eliminate_dead_global_variables,
    eliminate_dead_global_variables_in_program, eliminate_dead_local_variables,
    eliminate_dead_local_variables_in_program, eliminate_empty_statements,
    eliminate_unnecessary_braces, eliminate_unreachable_code, find_and_declare_builtin_functions,
    find_and_declare_builtin_structs, find_and_declare_builtin_variables, rename_private_symbols,
    replace_const_vars_with_literals, replace_splat_casts_with_swizzles,
};

/// `SkSL::Compiler`.
///
/// The compiler owns its [`Context`] and the [`Flavor`] it compiles for (the module texts it
/// loads). Skia's static `sOptimizer`/`sInliner` overrides are not ported: they are tool flags
/// that default to `kDefault`, which every build this port reproduces uses.
// Port of: src/sksl/SkSLCompiler.h#L64-L225 (chrome/m156)
#[doc(alias = "SkSL::Compiler")]
#[derive(Debug)]
pub struct Compiler {
    context: Context,
    flavor: Flavor,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// `POISON_TAG`: the name of the poison type and the text of a poison expression. Errors
    /// whose message contains it are not reported.
    pub const POISON_TAG: &'static str = "<POISON>";

    /// `FRAGCOLOR_NAME`: the fragment output that `out location=0` may only declare.
    pub const FRAGCOLOR_NAME: &'static str = "sk_FragColor";
    /// `RTADJUST_NAME`: the uniform that makes the IR generator emit position-fixup expressions.
    pub const RTADJUST_NAME: &'static str = "sk_RTAdjust";

    /// A compiler for the library build of Skia (minified modules), whose context reports errors
    /// into the compiler's error text.
    #[must_use]
    pub fn new() -> Self {
        Self::with_flavor(Flavor::Library)
    }

    /// A compiler for `flavor`: `Flavor::Standalone` compiles like `skslc`.
    #[must_use]
    pub fn with_flavor(flavor: Flavor) -> Self {
        Self {
            context: Context::new(ErrorReporter::compiler()),
            flavor,
        }
    }

    /// The flavour whose module texts this compiler loads.
    #[must_use]
    pub fn flavor(&self) -> Flavor {
        self.flavor
    }

    /// `context()`.
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// `context()`, mutable.
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// `errorReporter()`: the context's current reporter.
    pub fn error_reporter(&mut self) -> &mut ErrorReporter {
        &mut self.context.errors
    }

    /// `errorCount()`.
    #[must_use]
    pub fn error_count(&self) -> i32 {
        self.context.errors.error_count()
    }

    /// `handleError(msg, pos)`: appends one formatted error to the error text (without counting
    /// it; [`ErrorReporter::error`] counts).
    pub fn handle_error(&mut self, msg: &str, pos: Position) {
        let reporter = &mut self.context.errors;
        let source = reporter.source_arc().clone();
        handle_error(error_text_mut(reporter), &source, msg.as_bytes(), pos);
    }

    /// `errorText(showCount)`: the accumulated error text (with the `N error(s)` line when
    /// `show_count`), then resets the errors. The exact bytes Skia produces: the messages and the
    /// echoed source lines are bytes, which need not be UTF-8.
    // Port of: src/sksl/SkSLCompiler.cpp#L521-L528 (chrome/m156)
    pub fn error_text_bytes(&mut self, show_count: bool) -> Vec<u8> {
        if show_count {
            self.write_error_count();
        }
        let result = std::mem::take(error_text_mut(&mut self.context.errors));
        self.reset_errors();
        result
    }

    /// `writeErrorCount()`: appends `N error` or `N errors` when there are errors.
    // Port of: src/sksl/SkSLCompiler.cpp#L530-L536 (chrome/m156)
    pub fn write_error_count(&mut self) {
        let count = self.error_count();
        if count != 0 {
            let line = format!(
                "{count}{}",
                if count == 1 { " error\n" } else { " errors\n" }
            );
            error_text_mut(&mut self.context.errors).extend_from_slice(line.as_bytes());
        }
    }

    /// `resetErrors()`: clears the error text and the error count.
    pub fn reset_errors(&mut self) {
        error_text_mut(&mut self.context.errors).clear();
        self.context.errors.reset_error_count();
    }
}

/// The driver: settings, the compile context, module selection, `convertProgram`,
/// `compileModule`, `releaseProgram`, `finalize` and `optimize`.
impl Compiler {
    /// `FinalizeSettings`: makes the settings valid for `kind`. Optimization switches that depend
    /// on a disabled parent are cleared, and runtime effects always allow narrowing conversions.
    // Port of: src/sksl/SkSLCompiler.cpp#L89-L125 (chrome/m156), without the static overrides.
    pub fn finalize_settings(settings: &mut ProgramSettings, kind: ProgramKind) {
        // Disable optimization settings that depend on a parent setting which has been disabled.
        settings.inline_threshold *= i32::from(settings.optimize);
        settings.remove_dead_functions &= settings.optimize;
        settings.remove_dead_variables &= settings.optimize;

        // Runtime effects always allow narrowing conversions.
        if ProgramConfig::is_runtime_effect(kind) {
            settings.allow_narrowing_conversions = true;
        }
    }

    /// `initializeContext`: sets up the configuration, the module, the source and a global symbol
    /// table atop the module's symbols. The context must be clean.
    // Port of: src/sksl/SkSLCompiler.cpp#L131-L165 (chrome/m156)
    fn initialize_context(
        &mut self,
        module: Arc<Module>,
        kind: ProgramKind,
        settings: ProgramSettings,
        source: Arc<[u8]>,
        module_type: ModuleType,
    ) {
        // Start the ErrorReporter with a clean slate.
        self.reset_errors();

        let mut finalized = settings;
        Self::finalize_settings(&mut finalized, kind);
        let context = &mut self.context;
        context.config = Some(ProgramConfig::new(module_type, kind, finalized));
        context.pool = IrPool::extend(module.pool.clone());
        context.errors.set_source_bytes(source);

        // Set up a clean symbol table atop the parent module's symbols.
        let mut globals =
            SymbolTable::new(Some(module.symbols), module_type != ModuleType::Program);
        globals.mark_module_boundary();
        context.symbol_table = Some(context.pool.add_symbol_table(globals));
        context.module = Some(module);
    }

    /// `cleanupContext`: clears the fields `initialize_context` set. The IR of the program or
    /// module is taken out before this runs.
    // Port of: src/sksl/SkSLCompiler.cpp#L167-L181 (chrome/m156)
    fn cleanup_context(&mut self) {
        let context = &mut self.context;
        context.config = None;
        context.module = None;
        context.errors.set_source_bytes(Arc::from(&b""[..]));
        context.symbol_table = None;
        context.pool = IrPool::default();
    }

    /// `moduleForProgramKind(kind)`: the module a program of that kind is compiled against, or
    /// `None` for the Graphite kinds, whose modules are not ported.
    // Port of: src/sksl/SkSLCompiler.cpp#L44-L64 (chrome/m156)
    #[must_use]
    pub fn module_for_program_kind(&self, kind: ProgramKind) -> Option<Arc<Module>> {
        let loader = ModuleLoader::for_flavor(self.flavor);
        Some(match kind {
            ProgramKind::Fragment => loader.fragment(),
            ProgramKind::Vertex => loader.vertex(),
            ProgramKind::Compute => loader.compute(),
            ProgramKind::PrivateRuntimeBlender
            | ProgramKind::PrivateRuntimeColorFilter
            | ProgramKind::PrivateRuntimeShader => loader.private_rt_shader(),
            ProgramKind::RuntimeColorFilter
            | ProgramKind::RuntimeShader
            | ProgramKind::RuntimeBlender
            | ProgramKind::MeshVertex
            | ProgramKind::MeshFragment => loader.public(),
            ProgramKind::GraphiteFragment | ProgramKind::GraphiteVertex => return None,
        })
    }

    /// `compileModule` without the post-load optimization: parses `source` as the module
    /// `module_type` atop `parent`. Returns the parts of the module (its pool, global symbols and
    /// elements), so that the loader can add the public aliases before freezing the pool.
    /// `None` when the module has errors.
    ///
    /// # Panics
    ///
    /// If the parser leaves no global symbol table for the module, which the parser never does.
    // Port of: src/sksl/SkSLCompiler.cpp#L183-L212 (chrome/m156), with the post-load inlining
    // of `shouldInline` left to `optimize_module_after_loading`.
    pub fn compile_module_parts(
        &mut self,
        kind: ProgramKind,
        module_type: ModuleType,
        source: &[u8],
        parent: &Arc<Module>,
    ) -> Option<ModuleParts> {
        debug_assert_eq!(self.error_count(), 0);
        // Compile the module from source, using default program settings (but no memory pooling).
        let settings = ProgramSettings {
            use_memory_pool: false,
            ..ProgramSettings::default()
        };
        let source: Arc<[u8]> = Arc::from(source);
        self.initialize_context(parent.clone(), kind, settings, source.clone(), module_type);
        let elements =
            Parser::new(&mut self.context, settings, kind, &source).module_inheriting_from();
        let symbols = self
            .context
            .symbol_table
            .expect("the module's global symbol table");
        let pool = std::mem::take(&mut self.context.pool);
        self.cleanup_context();
        if self.error_count() != 0 {
            return None;
        }
        Some(ModuleParts {
            pool,
            symbols,
            elements,
            module_type,
            source,
        })
    }

    /// `compileModule(kind, moduleType, source, parent, shouldInline)`.
    // Port of: src/sksl/SkSLCompiler.cpp#L183-L212 (chrome/m156)
    pub fn compile_module(
        &mut self,
        kind: ProgramKind,
        module_type: ModuleType,
        source: &[u8],
        parent: &Arc<Module>,
        should_inline: bool,
    ) -> Option<Arc<Module>> {
        let mut parts = self.compile_module_parts(kind, module_type, source, parent)?;
        if should_inline && !self.optimize_module_after_loading(kind, &mut parts, parent) {
            return None;
        }
        Some(parts.freeze(parent.clone()))
    }

    /// `optimizeModuleAfterLoading`: the inlining that Skia runs on a module after loading it.
    /// `parent` is the module that `module` inherits from.
    // Port of: src/sksl/SkSLCompiler.cpp#L315-L339 (chrome/m156)
    pub(crate) fn optimize_module_after_loading(
        &mut self,
        kind: ProgramKind,
        module: &mut ModuleParts,
        parent: &Arc<Module>,
    ) -> bool {
        debug_assert_eq!(self.error_count(), 0);

        // Create a temporary program configuration with default settings
        // (`AutoProgramConfig`), and lend the module's pool to the context.
        let config = ProgramConfig::new(module.module_type, kind, ProgramSettings::default());
        let saved_config = self.context.config.replace(config);
        std::mem::swap(&mut self.context.pool, &mut module.pool);

        let mut usage = get_module_parts_usage(&self.context.pool, &module.elements, parent);

        // Perform inline-candidate analysis and inline any functions deemed suitable.
        let mut inliner = Inliner::new();
        while self.error_count() == 0 {
            if !self.run_inliner(&mut inliner, &module.elements, module.symbols, &mut usage) {
                break;
            }
        }

        std::mem::swap(&mut self.context.pool, &mut module.pool);
        self.context.config = saved_config;
        self.error_count() == 0
    }

    /// `runInliner(inliner, elements, symbols, usage)`: the program's symbol table was taken out
    /// of the context when the program was bundled, but the inliner creates IR objects which may
    /// expect the context to hold a valid symbol table.
    // Port of: src/sksl/SkSLCompiler.cpp#L392-L408 (chrome/m156)
    fn run_inliner(
        &mut self,
        inliner: &mut Inliner,
        elements: &[ElemId],
        symbols: SymTabId,
        usage: &mut ProgramUsage,
    ) -> bool {
        debug_assert!(self.context.symbol_table.is_none());
        self.context.symbol_table = Some(symbols);
        let result = inliner.analyze(&mut self.context, elements, symbols, usage);
        self.context.symbol_table = None;
        result
    }

    /// `convertProgram(kind, programSource, settings)`: parses `source` as a program of `kind`
    /// atop its module, then releases it (finalization and optimization). `None` when the
    /// program has errors, which are in [`Compiler::error_text_bytes`].
    // Port of: src/sksl/SkSLCompiler.cpp#L214-L231 (chrome/m156)
    pub fn convert_program(
        &mut self,
        kind: ProgramKind,
        source: &[u8],
        settings: ProgramSettings,
    ) -> Option<Program> {
        // Load the module used by this ProgramKind.
        let module = self.module_for_program_kind(kind)?;
        let source: Arc<[u8]> = Arc::from(source);
        self.initialize_context(module, kind, settings, source.clone(), ModuleType::Program);
        // `Parser::programInheritingFrom` releases the program when parsing succeeded.
        let elements =
            Parser::new(&mut self.context, settings, kind, &source).program_inheriting_from();
        let program = elements.and_then(|elements| self.release_program(source.clone(), elements));
        self.cleanup_context();
        program
    }

    /// `releaseProgram`: takes the parsed elements, the context's pool, configuration and global
    /// symbols, and builds the program, then finalizes and optimizes it.
    // Port of: src/sksl/SkSLCompiler.cpp#L233-L251 (chrome/m156)
    fn release_program(&mut self, source: Arc<[u8]>, elements: Vec<ElemId>) -> Option<Program> {
        let config = *self.context.config();
        let symbols = self
            .context
            .symbol_table
            .take()
            .expect("the program's global symbol table");
        let pool = std::mem::take(&mut self.context.pool);
        let mut program = Program {
            source,
            config,
            pool,
            symbols,
            owned_elements: elements,
            shared_elements: Vec::new(),
            interface: ProgramInterface::default(),
        };
        // The configuration and the module are still set, so the finalization checks see them.
        let success = self.finalize(&mut program) && self.optimize(&mut program);
        success.then_some(program)
    }

    /// `finalize(program)`: copies the built-ins the program uses into it, runs the finalization
    /// checks, and (for strict ES2 programs) the indexing check and the program-structure check.
    ///
    /// # Panics
    ///
    /// If no module is set (finalization runs inside [`Compiler::convert_program`] only).
    // Port of: src/sksl/SkSLCompiler.cpp#L407-L440 (chrome/m156)
    pub fn finalize(&mut self, program: &mut Program) -> bool {
        let module = self
            .context
            .module
            .clone()
            .expect("finalize runs while a module is set");
        let mut usage = get_usage(program);

        // Copy all referenced built-in functions into the Program.
        find_and_declare_builtin_functions(program, &mut usage);
        // Variables defined in modules need their declaring elements added to the program.
        find_and_declare_builtin_variables(program, &mut usage);
        // Structs from module code need to be added to the program's shared elements.
        find_and_declare_builtin_structs(program, &mut usage, &module);

        let owned = program.owned_elements.clone();
        self.context.with_program(program, |ctx| {
            // Do one last correctness-check pass. This looks for dangling FunctionReference and
            // TypeReference expressions, and reports them as errors.
            do_finalization_checks(ctx, &usage, &owned);

            if ctx.config().strict_es2_mode() && ctx.errors.error_count() == 0 {
                // Enforce Appendix A, Section 5 of the GLSL ES 1.00 spec -- Indexing.
                for &element in &owned {
                    validate_indexing_for_es2(&ctx.pool, element, &mut ctx.errors);
                }
            }
            if ctx.errors.error_count() == 0 {
                check_program_structure(ctx, &owned);
            }
        });
        self.error_count() == 0
    }

    /// `optimize(program)`: the optimizer. Skia runs the inliner, then `EliminateUnreachableCode`,
    /// `EliminateDeadFunctions`, `EliminateDeadLocalVariables` and `EliminateDeadGlobalVariables`
    /// (`SkSLCompiler.cpp#L341-L381`). The inliner (S12) and the dead-code transforms (S13) run in
    /// `run_optimizer_passes`, in Skia's order.
    // Port of: src/sksl/SkSLCompiler.cpp#L341-L381 (chrome/m156)
    fn optimize(&mut self, program: &mut Program) -> bool {
        // The optimizer only needs to run when it is enabled.
        if !program.config.settings.optimize {
            return true;
        }
        self.run_optimizer_passes(program);
        self.error_count() == 0
    }

    /// The optimizer passes of `Compiler::optimize`, in Skia's order: the inliner (S12), then
    /// `EliminateUnreachableCode`, the dead-function and dead-variable loops (S13).
    // Port of: src/sksl/SkSLCompiler.cpp#L341-L381 (chrome/m156)
    fn run_optimizer_passes(&mut self, program: &mut Program) {
        // Run the inliner only once; it is expensive! Multiple passes can occasionally shake out
        // more wins, but it's diminishing returns.
        let mut inliner = Inliner::new();
        // Usage is computed from the program, and then kept up to date by the passes.
        let mut usage = get_usage(program);
        // The passes walk the element lists while the context lends them the program's pool.
        let mut owned = std::mem::take(&mut program.owned_elements);
        let mut shared = std::mem::take(&mut program.shared_elements);
        let symbols = program.symbols;
        // The program's symbol table was taken out of the context when the program was bundled;
        // `with_program` puts it back for the duration of the call, as `runInliner` does.
        self.context.with_program(program, |ctx| {
            inliner.analyze(ctx, &owned, symbols, &mut usage);

            // Unreachable code can confuse some drivers, so it's worth removing. (skbug.com/40043094)
            eliminate_unreachable_code(ctx, &owned, &mut usage);

            while eliminate_dead_functions_in_program(ctx, &mut owned, &mut shared, &mut usage) {
                // Removing dead functions may cause more functions to become unreferenced. Try again.
            }
            while eliminate_dead_local_variables_in_program(ctx, &owned, &mut usage) {
                // Removing dead variables may cause more variables to become unreferenced. Try again.
            }
            while eliminate_dead_global_variables_in_program(
                ctx,
                &mut owned,
                &mut shared,
                &mut usage,
            ) {
                // Repeat until no changes occur.
            }

            // Make sure that variables are still declared in the correct symbol tables. Skia runs
            // this check only in debug builds (`SkDEBUGCODE`), and it must not report to the
            // program's errors, so its reporter is swapped out and the result asserted.
            #[cfg(debug_assertions)]
            {
                let symbols = ctx.symbol_table.expect("the program's symbols are set");
                let saved = ctx.set_error_reporter(ErrorReporter::no_op());
                crate::analysis::check_symbol_table_correctness(ctx, symbols, &owned);
                let check = ctx.set_error_reporter(saved);
                debug_assert_eq!(
                    check.error_count(),
                    0,
                    "a variable is out of its symbol table"
                );
            }
        });
        program.owned_elements = owned;
        program.shared_elements = shared;

        // Make sure that program usage is still correct after the optimization pass is complete.
        debug_assert_eq!(usage, get_usage(program));
    }

    /// `optimizeModuleBeforeMinifying(kind, module, shrinkSymbols)`: the module optimizer that the
    /// minifier runs. `module` is the module's unfrozen parts and `parent` the module it extends.
    /// Returns false when an error was reported.
    // Port of: src/sksl/SkSLCompiler.cpp#L253-L304 (chrome/m156)
    // Called by the minifier (`tests/src/tools/sksl_minify.rs`, S24).
    pub fn optimize_module_before_minifying(
        &mut self,
        kind: ProgramKind,
        module: &mut ModuleParts,
        parent: &Module,
        shrink_symbols: bool,
    ) -> bool {
        debug_assert_eq!(self.error_count(), 0);

        // Create a temporary program configuration with default settings.
        let config = ProgramConfig::new(module.module_type, kind, ProgramSettings::default());
        let mut usage = get_module_parts_usage(&module.pool, &module.elements, parent);
        self.with_module_parts(module, config, |ctx, elements, symbols| {
            if shrink_symbols {
                // Assign shorter names to symbols as long as it won't change the external meaning
                // of the code.
                rename_private_symbols(ctx, symbols, elements, kind);

                // Replace constant variables with their literal values to save space.
                replace_const_vars_with_literals(ctx, elements, &mut usage);
            }

            // Remove any unreachable code.
            eliminate_unreachable_code(ctx, elements, &mut usage);

            // We can only remove dead functions from runtime shaders, since runtime-effect helper
            // functions are isolated from other parts of the program. In a module, an unreferenced
            // function is intended to be called by the code that includes the module.
            if kind == ProgramKind::RuntimeShader {
                while eliminate_dead_functions(ctx, elements, &mut usage) {
                    // Removing dead functions may cause more functions to become unreferenced. Try
                    // again.
                }
            }

            while eliminate_dead_local_variables(ctx, elements, &mut usage) {
                // Removing dead variables may cause more variables to become unreferenced. Try
                // again.
            }

            // Runtime shaders are isolated from other parts of the program via name mangling, so
            // we can eliminate public globals if they aren't referenced. Otherwise, we only
            // eliminate private globals (prefixed with `$`) to avoid changing the meaning of the
            // module code.
            let only_private_globals = !ProgramConfig::is_runtime_effect(kind);
            while eliminate_dead_global_variables(ctx, elements, &mut usage, only_private_globals) {
                // Repeat until no changes occur.
            }

            // We eliminate empty statements to avoid runs of `;;;;;;` caused by the previous passes.
            eliminate_empty_statements(ctx, elements);

            // We can eliminate `{}` around single-statement blocks.
            eliminate_unnecessary_braces(ctx, elements);

            // We can convert `float4(myFloat)` with `myFloat.xxxx` to save a few characters.
            replace_splat_casts_with_swizzles(ctx, elements);
        });

        // Make sure that program usage is still correct after the optimization pass is complete.
        debug_assert_eq!(
            usage,
            get_module_parts_usage(&module.pool, &module.elements, parent)
        );
        self.error_count() == 0
    }

    /// Runs `f` with the module's pool, its global symbols and `config` installed in the context,
    /// with the module's element list. The context's pool is handed back to the module afterwards.
    #[allow(dead_code)] // Used with `optimize_module_before_minifying` (see there).
    fn with_module_parts<R>(
        &mut self,
        module: &mut ModuleParts,
        config: ProgramConfig,
        f: impl FnOnce(&mut Context, &mut Vec<ElemId>, SymTabId) -> R,
    ) -> R {
        let saved_pool =
            std::mem::replace(&mut self.context.pool, std::mem::take(&mut module.pool));
        let saved_config = self.context.config.replace(config);
        let saved_symbols = self.context.symbol_table.replace(module.symbols);
        let result = f(&mut self.context, &mut module.elements, module.symbols);
        module.pool = std::mem::replace(&mut self.context.pool, saved_pool);
        self.context.config = saved_config;
        self.context.symbol_table = saved_symbols;
        result
    }
}

/// The parts of a module before it is frozen: the pool and symbols built while it was parsed.
#[derive(Debug)]
pub struct ModuleParts {
    pub pool: IrPool,
    pub symbols: SymTabId,
    pub elements: Vec<ElemId>,
    pub module_type: ModuleType,
    pub source: Arc<[u8]>,
}

impl ModuleParts {
    /// Freezes the pool into a [`Module`] that inherits from `parent`.
    #[must_use]
    pub fn freeze(self, parent: Arc<Module>) -> Arc<Module> {
        Arc::new(Module {
            parent: Some(parent),
            pool: self.pool.freeze(),
            symbols: self.symbols,
            elements: self.elements,
            module_type: self.module_type,
            source: self.source,
        })
    }
}

/// The compiler's error text inside its reporter.
///
/// # Panics
///
/// If the context's reporter is not the compiler's (a parser checkpoint has swapped it out).
fn error_text_mut(reporter: &mut ErrorReporter) -> &mut Vec<u8> {
    match reporter.sink_mut() {
        ErrorSink::Compiler { error_text } => error_text,
        _ => panic!("Compiler: the context's error reporter is not the compiler's"),
    }
}

/// `Compiler::handleError`: appends `error: <line>: <msg>`, then (when the position is in the
/// source) the source line and a caret run under the error's range.
///
/// The echoed line shows at most 100 characters before the error (`...` marks the cut) and at
/// most 100 after its end (`...` again, unless the line or the text ends first). Tabs print as
/// four spaces (four carets inside the range), NULs as one space. A range that runs past the
/// end of its line ends its carets with `...`.
///
/// # Panics
///
/// If `pos` lies outside `src` (positions come from the lexer, so they never do).
// Port of: src/sksl/SkSLCompiler.cpp#L441-L519 (chrome/m156)
#[doc(alias = "Compiler::handleError")]
pub fn handle_error(error_text: &mut Vec<u8>, src: &[u8], msg: &[u8], pos: Position) {
    error_text.extend_from_slice(b"error: ");
    let mut print_location = false;
    let src_len = i32::try_from(src.len()).unwrap_or(i32::MAX);
    if pos.valid() {
        let line = pos.line(src);
        print_location = pos.start_offset() < src_len;
        error_text.extend_from_slice(format!("{line}: ").as_bytes());
    }
    error_text.extend_from_slice(msg);
    error_text.push(b'\n');
    if print_location {
        const MAX_SURROUNDING_CHARS: i32 = 100;
        let byte = |i: i32| src[usize::try_from(i).expect("offset within the source")];

        // Find the beginning of the line.
        let mut line_start = pos.start_offset();
        while line_start > 0 {
            if byte(line_start - 1) == b'\n' {
                break;
            }
            line_start -= 1;
        }

        // We don't want to show more than 100 characters surrounding the error, so push the
        // line start forward and add a leading ellipsis if there would be more than this.
        let mut line_text: Vec<u8> = Vec::new();
        let mut caret_text: Vec<u8> = Vec::new();
        if pos.start_offset() - line_start > MAX_SURROUNDING_CHARS {
            line_start = pos.start_offset() - MAX_SURROUNDING_CHARS;
            line_text.extend_from_slice(b"...");
            caret_text.extend_from_slice(b"   ");
        }

        // Echo the line. Again, we don't want to show more than 100 characters after the end of
        // the error, so truncate with a trailing ellipsis if needed.
        let mut line_suffix: &[u8] = b"...\n";
        let mut line_stop = pos.end_offset() + MAX_SURROUNDING_CHARS;
        if line_stop >= src_len {
            line_stop = src_len - 1;
            line_suffix = b"\n"; // no ellipsis if we reach end-of-file
        }
        for i in line_start..line_stop {
            let c = byte(i);
            if c == b'\n' {
                line_suffix = b"\n"; // no ellipsis if we reach end-of-line
                break;
            }
            match c {
                b'\t' => line_text.extend_from_slice(b"    "),
                b'\0' => line_text.push(b' '),
                _ => line_text.push(c),
            }
        }
        error_text.extend_from_slice(&line_text);
        error_text.extend_from_slice(line_suffix);

        // Print the carets underneath it, pointing to the range in question.
        let mut i = line_start;
        while i < src_len {
            if i >= pos.end_offset() {
                break;
            }
            match byte(i) {
                b'\t' => caret_text.extend_from_slice(if i >= pos.start_offset() {
                    b"^^^^"
                } else {
                    b"    "
                }),
                b'\n' => {
                    debug_assert!(i >= pos.start_offset());
                    // Use an ellipsis if the error continues past the end of the line.
                    caret_text.extend_from_slice(if pos.end_offset() > i + 1 {
                        b"..."
                    } else {
                        b"^"
                    });
                    i = src_len;
                }
                _ => caret_text.push(if i >= pos.start_offset() { b'^' } else { b' ' }),
            }
            i += 1;
        }
        error_text.extend_from_slice(&caret_text);
        error_text.push(b'\n');
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Compiler;
    use crate::position::Position;

    /// The byte range of the `nth` (0-based) occurrence of `needle` in `src`.
    fn range_of(src: &str, needle: &str, nth: usize) -> Position {
        let start = src
            .match_indices(needle)
            .nth(nth)
            .expect("needle in source")
            .0;
        let start = i32::try_from(start).unwrap();
        let len = i32::try_from(needle.len()).unwrap();
        Position::range(start, start + len)
    }

    fn compiler_for(src: &str) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.error_reporter().set_source(Arc::from(src));
        compiler
    }

    /// The text `skslc` writes after its `### Compilation failed:\n\n` header.
    fn golden_body(golden: &str) -> &str {
        golden
            .strip_prefix("### Compilation failed:\n\n")
            .expect("an error golden")
    }

    #[test]
    fn error_text_matches_ossfuzz38140_golden() {
        // resources/sksl/errors/Ossfuzz38140.sksl and tests/sksl/errors/Ossfuzz38140.glsl.
        let src = concat!(
            "half4 blend_src_over(half4 src, half4 dst) {\n",
            "    return src + (1 - src.a)*dst;\n",
            "}\n",
            "\n",
            "half4 main(half4 src, half4 dst) {\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "}\n",
            "\n",
            "/*%%*\n",
            "differ only in modifiers\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 1: functions 'half4 blend_src_over(half4 src, half4 dst)' and '$pure half4 ",
            "blend_src_over(half4 src, half4 dst)' differ only in modifiers\n",
            "half4 blend_src_over(half4 src, half4 dst) {\n",
            "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "error: 2: unknown identifier 'src'\n",
            "    return src + (1 - src.a)*dst;\n",
            "           ^^^\n",
            "error: 2: unknown identifier 'src'\n",
            "    return src + (1 - src.a)*dst;\n",
            "                      ^^^\n",
            "error: 2: unknown identifier 'dst'\n",
            "    return src + (1 - src.a)*dst;\n",
            "                             ^^^\n",
            "error: 5: shader 'main' must be main() or main(float2)\n",
            "half4 main(half4 src, half4 dst) {\n",
            "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "error: 6: unknown identifier 'src'\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "                          ^^^\n",
            "error: 6: unknown identifier 'dst'\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "                                          ^^^\n",
            "7 errors\n",
        );
        let mut compiler = compiler_for(src);
        let errors = [
            (
                range_of(src, "half4 blend_src_over(half4 src, half4 dst)", 0),
                "functions 'half4 blend_src_over(half4 src, half4 dst)' and '$pure half4 \
                 blend_src_over(half4 src, half4 dst)' differ only in modifiers",
            ),
            (range_of(src, "src", 2), "unknown identifier 'src'"),
            (range_of(src, "src", 3), "unknown identifier 'src'"),
            (range_of(src, "dst", 1), "unknown identifier 'dst'"),
            (
                range_of(src, "half4 main(half4 src, half4 dst)", 0),
                "shader 'main' must be main() or main(float2)",
            ),
            (range_of(src, "src", 6), "unknown identifier 'src'"),
            (range_of(src, "dst", 3), "unknown identifier 'dst'"),
        ];
        for (pos, msg) in errors {
            compiler.context_mut().errors.error(pos, msg);
        }
        assert_eq!(compiler.error_count(), 7);
        assert_eq!(
            compiler.error_text_bytes(true),
            golden_body(golden).as_bytes()
        );
        // errorText resets the errors.
        assert_eq!(compiler.error_count(), 0);
        assert_eq!(compiler.error_text_bytes(true), b"");
    }

    #[test]
    fn error_text_marks_ranges_that_run_past_the_line() {
        // resources/sksl/errors/ForLoopOverflow.rts and tests/sksl/errors/ForLoopOverflow.glsl.
        let src = concat!(
            "half4 main(float2 coords) {\n",
            "    half arr[4];\n",
            "    for (int i = 2147483640; i < 2147483647; i += 100) {\n",
            "        arr[i - 2147483640] = half(1);\n",
            "    }\n",
            "    return half4(0);\n",
            "}\n",
            "\n",
            "/*%%*\n",
            "loop must guarantee termination in fewer iterations\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 3: loop must guarantee termination in fewer iterations\n",
            "    for (int i = 2147483640; i < 2147483647; i += 100) {\n",
            "    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^...\n",
            "1 error\n",
        );
        let mut compiler = compiler_for(src);
        let start = range_of(src, "for (", 0).start_offset();
        let end = range_of(src, "    }\n", 0).end_offset() - 1;
        compiler.context_mut().errors.error(
            Position::range(start, end),
            "loop must guarantee termination in fewer iterations",
        );
        assert_eq!(
            compiler.error_text_bytes(true),
            golden_body(golden).as_bytes()
        );
    }

    #[test]
    fn error_text_echoes_multi_line_messages() {
        // resources/sksl/errors/IllegalRecursionSimple.rts and its .glsl golden.
        let src = concat!(
            "// Expect 1 error\n",
            "\n",
            "// Simple recursion is not allowed, even with branching:\n",
            "int fibonacci(int n) { return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }\n",
            "\n",
            "/*%%*\n",
            "potential recursion (function call cycle) not allowed:\n",
            "\tint fibonacci(int n)\n",
            "\tint fibonacci(int n)\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 4: potential recursion (function call cycle) not allowed:\n",
            "\tint fibonacci(int n)\n",
            "\tint fibonacci(int n)\n",
            "int fibonacci(int n) { return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }\n",
            "                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "1 error\n",
        );
        let mut compiler = compiler_for(src);
        let body = range_of(
            src,
            "{ return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }",
            0,
        );
        compiler.context_mut().errors.error(
            body,
            "potential recursion (function call cycle) not allowed:\n\tint fibonacci(int n)\n\
             \tint fibonacci(int n)",
        );
        assert_eq!(
            compiler.error_text_bytes(true),
            golden_body(golden).as_bytes()
        );
    }

    #[test]
    fn error_text_cuts_long_lines_with_ellipses() {
        // The first two errors of tests/sksl/errors/Ossfuzz44561.glsl, from the first two lines
        // of resources/sksl/errors/Ossfuzz44561.sksl (which hold a DEL and a CR byte).
        let line1 = concat!(
            "void m(){ix;void[(0).r1(((5).ss0s.ss0s.sss0.ss0s+(5).ss0s.sss.00ss.ss0s.ss .ss0.",
            "ss00.ss0s+(5).ss0s.ss0.s0s.ss00.sssch (int) {case 0:{{{{{{{{{{{{{{{{{{{{{{{{\x7fe;",
            "void n(){;; int \rm;;half x;",
        );
        let src = format!("{line1}\nx*x++.ss1.ss;0;\n");
        let expected = format!(
            "error: 1: unknown identifier 'ix'\n{}...\n{}^^\n\
             error: 1: too many components in swizzle mask\n...{}\n{}^\n",
            &line1[..111],
            " ".repeat(9),
            &line1[16..],
            " ".repeat(103),
        );
        // The echoed texts above, as the golden spells them.
        assert!(expected.contains(
            "void m(){ix;void[(0).r1(((5).ss0s.ss0s.sss0.ss0s+(5).ss0s.sss.00ss.ss0s.ss .ss0.\
             ss00.ss0s+(5).ss0s.ss0.s0s.ss00...\n"
        ));
        let mut compiler = compiler_for(&src);
        compiler
            .context_mut()
            .errors
            .error(Position::range(9, 11), "unknown identifier 'ix'");
        compiler.context_mut().errors.error(
            Position::range(116, 117),
            "too many components in swizzle mask",
        );
        assert_eq!(compiler.error_text_bytes(false), expected.as_bytes());
    }

    #[test]
    fn poison_errors_and_invalid_positions() {
        let mut compiler = compiler_for("x");
        compiler
            .context_mut()
            .errors
            .error(Position::range(0, 1), "'<POISON>' is not a type");
        assert_eq!(compiler.error_count(), 0, "errors about poison are dropped");
        compiler
            .context_mut()
            .errors
            .error(Position::default(), "no position");
        // A position at the end of the text prints its line number but no source line.
        compiler
            .context_mut()
            .errors
            .error(Position::range(1, 1), "at end");
        assert_eq!(
            compiler.error_text_bytes(true),
            b"error: no position\nerror: 1: at end\n2 errors\n".as_slice()
        );
    }
}
