// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Unit tests of the optimizer transforms (task `S13`). Each test compiles a small `SkSL` snippet,
//! runs one pass on it, and compares the element descriptions. Where a Skia golden shows the
//! optimized program, the test checks the same structure (`resources/sksl/shared/Dead*.sksl` with
//! `tests/sksl/shared/*.glsl`).

use std::sync::Arc;

use super::{
    add_const_to_var_modifiers, eliminate_dead_functions_in_program,
    eliminate_dead_global_variables_in_program, eliminate_dead_local_variables_in_program,
    eliminate_empty_statements, eliminate_unnecessary_braces, eliminate_unreachable_code,
    replace_splat_casts_with_swizzles, rewrite_indexed_swizzle,
};
use crate::analysis::{ProgramUsage, ProgramVisitor, get_usage, walk_statement};
use crate::compiler::{Compiler, ModuleParts};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::flavor::Flavor;
use crate::ir::{
    ElemId, ExprId, ExpressionKind, IrPool, ModifierFlags, Program, StatementKind, StmtId, VarId,
};
use crate::module_loader::ModuleLoader;
use crate::modules::ModuleType;
use crate::program_settings::{ProgramKind, ProgramSettings};

/// Compiles `src` as a program of `kind` without the optimizer, so that one pass can run alone.
/// The passes check the settings the optimizer would have enabled, so those are turned on.
fn compile_unoptimized(kind: ProgramKind, src: &str) -> Program {
    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    let settings = ProgramSettings {
        optimize: false,
        ..ProgramSettings::default()
    };
    let mut program = compiler
        .convert_program(kind, src.as_bytes(), settings)
        .expect("the snippet compiles");
    program.config.settings.optimize = true;
    program.config.settings.remove_dead_functions = true;
    program.config.settings.remove_dead_variables = true;
    program
}

/// Compiles `src` as a program of `kind` with the whole optimizer, as `skslc` does.
fn compile_optimized(kind: ProgramKind, src: &str) -> Program {
    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    compiler
        .convert_program(kind, src.as_bytes(), ProgramSettings::default())
        .expect("the snippet compiles")
}

/// Runs `pass` on the program's owned elements and its usage, as `Compiler::optimize` does.
fn with_owned(
    program: &mut Program,
    pass: impl FnOnce(&mut Context, &mut Vec<ElemId>, &mut ProgramUsage),
) {
    let mut usage = get_usage(program);
    let mut owned = std::mem::take(&mut program.owned_elements);
    let mut ctx = Context::new(ErrorReporter::no_op());
    ctx.with_program(program, |ctx| pass(ctx, &mut owned, &mut usage));
    program.owned_elements = owned;
}

/// The descriptions of the program's owned elements, in order.
fn owned_text(program: &Program) -> String {
    program
        .owned_elements
        .iter()
        .map(|&element| program.pool.element_description(element))
        .collect()
}

/// The names of the program's owned function definitions, in order.
fn owned_function_names(program: &Program) -> Vec<String> {
    program
        .owned_elements
        .iter()
        .filter_map(|&element| {
            let def = program.pool.element(element).as_function()?;
            Some(program.pool.function(def.declaration).name.to_string())
        })
        .collect()
}

/// Collects the statements' variable declarations and the expressions of one kind.
#[derive(Default)]
struct Collector {
    variables: Vec<VarId>,
    /// The `VarDeclaration` statements themselves, by id, in visiting order.
    declarations: Vec<StmtId>,
    index_expressions: Vec<ExprId>,
}

impl ProgramVisitor for Collector {
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::VarDeclaration(decl) = &pool.statement(stmt).kind {
            self.variables.push(decl.var);
            self.declarations.push(stmt);
        }
        walk_statement(self, pool, stmt)
    }

    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if matches!(pool.expression(expr).kind, ExpressionKind::Index(_)) {
            self.index_expressions.push(expr);
        }
        crate::analysis::walk_expression(self, pool, expr)
    }
}

fn collect(program: &Program) -> Collector {
    let mut collector = Collector::default();
    for element in program.elements() {
        collector.visit_program_element(&program.pool, element);
    }
    collector
}

/// `eliminate_unreachable_code` replaces the statements after a `return` with `Nop`s.
#[test]
fn unreachable_code_after_a_return_becomes_nops() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "int f(int x) {\n    return x;\n    x = 2;\n}\n",
    );
    with_owned(&mut program, |ctx, owned, usage| {
        eliminate_unreachable_code(ctx, owned, usage);
    });
    assert_eq!(owned_text(&program), "int f(int x) {\nreturn x;\n;\n}\n");
}

/// Both sides of an `if` must exit before the code after it is dead; one side is not enough.
#[test]
fn unreachable_code_respects_branches() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "int f(bool c, int x) {\n    if (c) { return 1; } else { x = 2; }\n    return x;\n}\n",
    );
    with_owned(&mut program, |ctx, owned, usage| {
        eliminate_unreachable_code(ctx, owned, usage);
    });
    let text = owned_text(&program);
    assert!(text.contains("return x;"), "{text}");
    assert!(!text.contains(";\n;"), "nothing was dead: {text}");
}

/// A return inside a loop does not exit the function on every path, so the code after the loop
/// stays.
#[test]
fn a_return_inside_a_loop_does_not_end_the_function() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "int h(int n) {\n    for (int i = 0; i < n; i++) { return i; }\n    return 3;\n}\n",
    );
    with_owned(&mut program, |ctx, owned, usage| {
        eliminate_unreachable_code(ctx, owned, usage);
    });
    assert!(owned_text(&program).contains("return 3;"));
}

/// `eliminate_dead_functions` removes the functions that are never called, and keeps `main`. The
/// optimized `DeadStripFunctions` golden keeps only `live_fn` and `unpremul`'s `half4` overload.
#[test]
fn dead_functions_are_stripped_as_in_the_dead_strip_golden() {
    let src = concat!(
        "uniform half4 colorGreen, colorRed;\n",
        "half4 dead_fn(half4 a, half4 b) { return a * b; }\n",
        "half4 live_fn(half4 a, half4 b) { return a + b; }\n",
        "half4 main(float2 coords) {\n",
        "    half4 a = live_fn(half4(3), half4(-5));\n",
        "    return a != half4(0) ? colorGreen : colorRed;\n",
        "}\n",
    );
    let mut program = compile_unoptimized(ProgramKind::Fragment, src);
    assert!(owned_function_names(&program).contains(&"dead_fn".to_owned()));
    with_owned(&mut program, |ctx, owned, usage| {
        // `main` is the only root; the shared list is not touched by this snippet's pass.
        let mut shared = Vec::new();
        let changed = eliminate_dead_functions_in_program(ctx, owned, &mut shared, usage);
        assert!(changed);
    });
    assert_eq!(owned_function_names(&program), ["live_fn", "main"]);
}

/// A local variable that is never read is removed, and its initializer stays only where it has
/// side effects.
#[test]
fn dead_locals_keep_only_their_side_effects() {
    let src = concat!(
        "float g() { return 1.0; }\n",
        "float f(float a) {\n",
        "    float pure = a + 1.0;\n",
        "    float effect = g();\n",
        "    return a;\n",
        "}\n",
    );
    let mut program = compile_unoptimized(ProgramKind::Fragment, src);
    with_owned(&mut program, |ctx, owned, usage| {
        let changed = eliminate_dead_local_variables_in_program(ctx, owned, usage);
        assert!(changed);
    });
    let text = owned_text(&program);
    assert!(!text.contains("pure"), "{text}");
    assert!(!text.contains("effect"), "{text}");
    assert!(text.contains("g();"), "the call must stay: {text}");
    assert!(text.contains("return a;"), "{text}");
}

/// The unused globals of `DeadGlobals` go, the uniforms stay, and `main` is the only function, as
/// the optimized golden `tests/sksl/shared/DeadGlobals.glsl` shows.
#[test]
fn dead_globals_are_removed_as_in_the_dead_globals_golden() {
    let src = concat!(
        "uniform half4 colorGreen, colorRed;\n",
        "const float Pi = 3.14;\n",
        "const float Alias1 = Pi;\n",
        "const float Alias2 = Alias1;\n",
        "half4 main(float2) {\n",
        "    return colorGreen;\n",
        "}\n",
    );
    let mut program = compile_unoptimized(ProgramKind::Fragment, src);
    // Repeat until no changes occur, as `Compiler::optimize` does.
    with_owned(&mut program, |ctx, owned, usage| {
        let mut shared = Vec::new();
        while eliminate_dead_global_variables_in_program(ctx, owned, &mut shared, usage) {}
    });
    let text = owned_text(&program);
    assert!(!text.contains("Pi"), "{text}");
    assert!(!text.contains("Alias"), "{text}");
    assert!(text.contains("colorGreen"), "{text}");
    assert!(text.contains("colorRed"), "{text}");
}

/// `eliminate_empty_statements` drops the `Nop`s from blocks.
#[test]
fn empty_statements_are_removed_from_blocks() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "void k(int x) {\n    { ; ; x = 2; }\n}\n",
    );
    with_owned(&mut program, |ctx, owned, _usage| {
        eliminate_empty_statements(ctx, owned);
    });
    // The inner block stays (this pass does not unwrap blocks). Skia's `Block::description` ends a
    // braced child with "}\n", and the enclosing block adds its own newline.
    assert_eq!(owned_text(&program), "void k(int x) {\n{\nx = 2;\n}\n\n}\n");
}

/// `eliminate_unnecessary_braces` unwraps single-statement blocks under `if`, `for` and `do`.
#[test]
fn braces_around_one_statement_are_removed() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "void k(bool c, int x) {\n    if (c) { x = 1; }\n}\n",
    );
    with_owned(&mut program, |ctx, owned, _usage| {
        eliminate_unnecessary_braces(ctx, owned);
    });
    assert_eq!(
        owned_text(&program),
        "void k(bool c, int x) {\nif (c) x = 1;\n}\n"
    );
}

/// An `else` that would bind to an inner `if` gets braces back: removing the braces from
/// `if (a) { if (b) x = 1; } else x = 2;` would make the `else` belong to `if (b)`.
#[test]
fn an_else_keeps_its_binding_when_braces_go() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "void m(bool a, bool b, int x) {\n    if (a) { if (b) x = 1; } else x = 2;\n}\n",
    );
    with_owned(&mut program, |ctx, owned, _usage| {
        eliminate_unnecessary_braces(ctx, owned);
    });
    let text = owned_text(&program);
    assert!(
        text.contains("if (a) {\nif (b) x = 1;\n}\n else x = 2;"),
        "{text}"
    );
}

/// `replace_splat_casts_with_swizzles` turns a splat of a variable into a swizzle, and keeps a
/// splat of an integer literal, which the swizzle syntax does not take.
#[test]
fn splats_of_variables_become_swizzles() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "half4 s(float f) { return half4(f) + half4(1); }\n",
    );
    with_owned(&mut program, |ctx, owned, _usage| {
        replace_splat_casts_with_swizzles(ctx, owned);
    });
    assert_eq!(
        owned_text(&program),
        "half4 s(float f) {\nreturn half(f).xxxx + half4(1.0);\n}\n"
    );
}

/// `rewrite_indexed_swizzle` turns `v.zyx[i]` into `v[int3(2, 1, 0)[i]]`.
#[test]
fn indexed_swizzles_index_a_constant_vector() {
    let mut program = compile_unoptimized(
        ProgramKind::Fragment,
        "float r(float4 v, int i) { return v.zyx[i]; }\n",
    );
    let index = collect(&program)
        .index_expressions
        .first()
        .copied()
        .expect("the snippet indexes a swizzle");
    let rewritten = {
        let mut ctx = Context::new(ErrorReporter::no_op());
        ctx.with_program(&mut program, |ctx| rewrite_indexed_swizzle(ctx, index))
    };
    let rewritten = rewritten.expect("the base is a swizzle");
    assert_eq!(
        program.pool.expression_description(rewritten),
        "v[int3(2, 1, 0)[i]]"
    );
}

/// `add_const_to_var_modifiers` adds `const` to a variable with one write and a constant initial
/// value, and to no other.
#[test]
fn const_is_added_only_to_single_write_constants() {
    let program = compile_unoptimized(
        ProgramKind::Fragment,
        "float f() {\n    float once = 1.0;\n    float twice = 1.0;\n    twice = 2.0;\n    return once + twice;\n}\n",
    );
    let usage = get_usage(&program);
    let vars = collect(&program).variables;
    assert_eq!(vars.len(), 2);
    let (once, twice) = (vars[0], vars[1]);
    let once_init = program.pool.variable(once).initial_value(&program.pool);
    let twice_init = program.pool.variable(twice).initial_value(&program.pool);
    assert!(add_const_to_var_modifiers(&program.pool, once, once_init, &usage).is_const());
    assert!(!add_const_to_var_modifiers(&program.pool, twice, twice_init, &usage).is_const());
    assert_eq!(
        add_const_to_var_modifiers(&program.pool, once, once_init, &usage),
        ModifierFlags::CONST
    );
}

/// Compiles `src` as a module of `kind` atop the Fragment module, as the minifier does.
fn compile_module(
    kind: ProgramKind,
    src: &str,
) -> (Compiler, ModuleParts, Arc<crate::modules::Module>) {
    let parent = ModuleLoader::for_flavor(Flavor::Standalone).fragment();
    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    let parts = compiler
        .compile_module_parts(kind, ModuleType::Unknown, src.as_bytes(), &parent)
        .expect("the module compiles");
    (compiler, parts, parent)
}

/// The descriptions of the module's own elements, in order.
fn module_text(parts: &ModuleParts) -> String {
    parts
        .elements
        .iter()
        .map(|&element| parts.pool.element_description(element))
        .collect()
}

/// `replace_const_vars_with_literals`: a constant read once is replaced by its value, since the
/// value is shorter than the declaration plus the name.
#[test]
fn constant_variables_are_replaced_by_their_values() {
    let (mut compiler, mut parts, parent) = compile_module(
        ProgramKind::Fragment,
        "const float K = 2.0;\nfloat use(float x) { return x * K; }\n",
    );
    assert!(compiler.optimize_module_before_minifying(
        ProgramKind::Fragment,
        &mut parts,
        &parent,
        true
    ));
    let text = module_text(&parts);
    assert!(text.contains("return a * 2.0;"), "{text}");
}

/// `rename_private_symbols` gives `$`-prefixed functions and every parameter and local a short
/// name. The name of a function is chosen before its parameters, so `$helper` becomes `$a` and
/// the parameter `x` becomes `a`, which makes the local `y` into `b`.
#[test]
fn private_functions_and_locals_get_short_names() {
    let (mut compiler, mut parts, parent) = compile_module(
        ProgramKind::Fragment,
        "float $helper(float x) { float y = x; return y; }\nfloat use2(float z) { return $helper(z); }\n",
    );
    assert!(compiler.optimize_module_before_minifying(
        ProgramKind::Fragment,
        &mut parts,
        &parent,
        true
    ));
    assert_eq!(
        module_text(&parts),
        "float $a(float a) {\nfloat b = a;\nreturn b;\n}\nfloat use2(float a) {\nreturn $a(a);\n}\n"
    );
}

/// `optimize_module_before_minifying` with `shrinkSymbols` off keeps the names, and still runs the
/// dead-code passes: an unused `$` global goes, and a public one stays.
#[test]
fn module_optimizer_keeps_public_globals_and_drops_private_ones() {
    let (mut compiler, mut parts, parent) = compile_module(
        ProgramKind::Fragment,
        "float $unused = 1.0;\nfloat keep = 2.0;\nfloat use(float x) { return x + keep; }\n",
    );
    assert!(compiler.optimize_module_before_minifying(
        ProgramKind::Fragment,
        &mut parts,
        &parent,
        false
    ));
    let text = module_text(&parts);
    assert!(!text.contains("$unused"), "{text}");
    assert!(text.contains("keep"), "{text}");
    assert!(text.contains("float use(float x)"), "{text}");
}

/// The whole optimizer on `DeadIfStatement`: the constant `if`s fold away, and the live return is
/// what the optimized golden `tests/sksl/shared/DeadIfStatement.glsl` shows. The `;` lines are the
/// Nops that folding and dead-code removal leave in a program: Skia removes empty statements only
/// from modules (`Transform::EliminateEmptyStatements`), and the GLSL generator skips Nops.
#[test]
fn dead_if_statement_optimizes_to_the_live_return() {
    let program = compile_optimized(
        ProgramKind::Fragment,
        concat!(
            "uniform half4 colorGreen, colorRed;\n",
            "half4 main(float2 coords) {\n",
            "    const bool x = true;\n",
            "    if (!x) return colorRed;\n",
            "    if (x) return colorGreen;\n",
            "    return colorRed;\n",
            "}\n",
        ),
    );
    assert_eq!(
        owned_text(&program),
        "uniform half4 colorGreen;uniform half4 colorRed;half4 main(float2 coords) {\n;\n;\nreturn colorGreen;\n;\n}\n"
    );
}

/// The whole optimizer on `DeadGlobals`: `Pi == Alias3` folds to true, and the unused constants
/// go, as the optimized golden `tests/sksl/shared/DeadGlobals.glsl` shows.
#[test]
fn dead_globals_optimize_to_the_live_return() {
    let program = compile_optimized(
        ProgramKind::Fragment,
        concat!(
            "uniform half4 colorGreen, colorRed;\n",
            "const float Pi = 3.14;\n",
            "const float Alias1 = Pi;\n",
            "const float Alias2 = Alias1;\n",
            "const float Alias3 = Alias2;\n",
            "const float Alias4 = Alias1;\n",
            "const float Alias5 = Pi;\n",
            "half4 main(float2) {\n",
            "    return (Pi == Alias3) ? colorGreen : colorRed;\n",
            "}\n",
        ),
    );
    assert_eq!(
        owned_text(&program),
        "uniform half4 colorGreen;uniform half4 colorRed;half4 main(float2 ) {\nreturn colorGreen;\n}\n"
    );
}

/// The statement that declares the variable named `name`, as its `declaringElement` gives it, and
/// whether that statement is one of the program's declarations.
fn declaring_statement_of(program: &Program, name: &str) -> (StmtId, bool) {
    let collector = collect(program);
    let pool = &program.pool;
    let var = collector
        .variables
        .iter()
        .copied()
        .find(|&var| &*pool.variable(var).name == name)
        .unwrap_or_else(|| panic!("`{name}` is declared"));
    let stmt = pool
        .variable(var)
        .var_declaration(pool)
        .unwrap_or_else(|| panic!("`{name}` has a declaring statement"));
    let reachable = collector.declarations.contains(&stmt);
    (stmt, reachable)
}

/// `hoist_switch_var_declarations_at_top_level` moves `float x = 2.0;` out of its case into a
/// scoped block and leaves `x = 2.0;` in the case. The moved `VarDeclaration` is a new statement,
/// so `x`'s `declaringElement` must follow it: Skia's pointer keeps naming the same object.
// Port of: src/sksl/transform/SkSLHoistSwitchVarDeclarationsAtTopLevel.cpp#L35-L137 (chrome/m156)
#[test]
fn hoisted_declarations_stay_reachable_from_their_variable() {
    let program = compile_unoptimized(
        ProgramKind::Fragment,
        concat!(
            "half4 main(float2 p) {\n",
            "    float total = 0.0;\n",
            "    switch (int(p.x)) {\n",
            "        case 0:\n",
            "            float x = 2.0;\n",
            "            total = x;\n",
            "            break;\n",
            "        default:\n",
            "            break;\n",
            "    }\n",
            "    return half4(total);\n",
            "}\n",
        ),
    );
    let (stmt, reachable) = declaring_statement_of(&program, "x");
    assert!(
        reachable,
        "the declaring statement of `x` is in the program"
    );
    let StatementKind::VarDeclaration(decl) = &program.pool.statement(stmt).kind else {
        panic!("`x` is declared by a VarDeclaration statement");
    };
    assert_eq!(
        decl.value, None,
        "the hoisted declaration has no initial value"
    );
    assert!(
        owned_text(&program).contains("x = 2.0;"),
        "the initial value became an assignment: {}",
        owned_text(&program)
    );
}

/// `Inliner::inline_statement` wraps the enclosing statement of a call in a block. For `float x =
/// helper(p.x);` that statement is `x`'s `VarDeclaration`, which moves to a new id, so `x`'s
/// `declaringElement` must follow it.
// Port of: src/sksl/SkSLInliner.cpp#L1162-L1163 (chrome/m156), the enclosing-statement move.
#[test]
fn inlined_call_keeps_the_declaration_of_its_enclosing_statement() {
    let program = compile_optimized(
        ProgramKind::Fragment,
        concat!(
            "float helper(float a) { return a * 2.0; }\n",
            "half4 main(float2 p) {\n",
            "    float x = helper(p.x);\n",
            "    return half4(x);\n",
            "}\n",
        ),
    );
    assert!(
        !owned_text(&program).contains("helper("),
        "the call is inlined: {}",
        owned_text(&program)
    );
    let (_, reachable) = declaring_statement_of(&program, "x");
    assert!(
        reachable,
        "the declaring statement of `x` is in the program"
    );
}
