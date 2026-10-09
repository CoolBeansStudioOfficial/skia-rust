// Copyright 2021 Google LLC
// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLFindAndDeclareBuiltinFunctions.cpp,
// SkSLFindAndDeclareBuiltinVariables.cpp and SkSLFindAndDeclareBuiltinStructs.cpp (chrome/m156).

//! The three transforms that `Compiler::finalize` runs first. A program refers to built-in
//! functions, variables and structs that live in its module chain; these transforms copy the
//! definitions it uses into the program's shared elements, so that the program is self-contained.
//! Each one also records the new elements in the program's [`ProgramUsage`].

use crate::analysis::ProgramUsage;
use crate::compiler::Compiler;
use crate::intrinsic_list::IntrinsicKind;
use crate::ir::{
    ElemId, FnId, IrPool, Program, ProgramElementKind, RtFlip, StatementKind, SymbolId, TypeId,
    VarId,
};
use crate::modules::Module;
use crate::program_settings::ProgramConfig;

/// `SK_FRAGCOORD_BUILTIN`, the `layout(builtin=…)` of `sk_FragCoord`.
// Port of: src/sksl/SkSLCompiler.h#L26-L29 (chrome/m156)
const SK_FRAGCOORD_BUILTIN: i32 = 15;
/// `SK_CLOCKWISE_BUILTIN`, the `layout(builtin=…)` of `sk_Clockwise`.
// Port of: src/sksl/SkSLCompiler.h#L26-L29 (chrome/m156)
const SK_CLOCKWISE_BUILTIN: i32 = 17;
/// `SK_LASTFRAGCOLOR_BUILTIN`, the `layout(builtin=…)` of `sk_LastFragColor`.
// Port of: src/sksl/SkSLCompiler.h#L26-L29 (chrome/m156)
const SK_LASTFRAGCOLOR_BUILTIN: i32 = 10008;
/// `SK_SECONDARYFRAGCOLOR_BUILTIN`, the `layout(builtin=…)` of `sk_SecondaryFragColor`.
// Port of: src/sksl/SkSLCompiler.h#L26-L29 (chrome/m156)
const SK_SECONDARYFRAGCOLOR_BUILTIN: i32 = 10012;

/// The `(name, description)` key that orders the built-in functions of the shared elements.
fn function_key(pool: &IrPool, definition: ElemId) -> (String, String) {
    let ProgramElementKind::Function(def) = &pool.element(definition).kind else {
        unreachable!("a function definition element");
    };
    let decl = pool.function(def.declaration);
    (decl.name.to_string(), decl.description(pool))
}

/// Adds `elem` to `elements` unless it is already there (Skia's `std::find` before `push_back`).
fn push_unique<T: PartialEq>(elements: &mut Vec<T>, elem: T) {
    if !elements.contains(&elem) {
        elements.push(elem);
    }
}

/// `Transform::FindAndDeclareBuiltinFunctions`: copies every built-in function that the program
/// calls (directly or through other built-ins) into the program's shared elements.
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinFunctions.cpp#L28-L84 (chrome/m156)
pub fn find_and_declare_builtin_functions(program: &mut Program, usage: &mut ProgramUsage) {
    let mut added_builtins: Vec<ElemId> = Vec::new();
    loop {
        // Find all the built-ins referenced by the program but not yet included in the code.
        // Skia iterates its call-count table here; the set of definitions found does not depend
        // on the order, and the new ones are sorted below, so the table is visited in id order.
        let num_builtins_at_start = added_builtins.len();
        let mut calls: Vec<(FnId, i32)> = usage
            .call_counts
            .iter()
            .map(|(&function, &count)| (function, count))
            .collect();
        calls.sort_unstable_by_key(|&(function, _)| function);
        for (function, count) in calls {
            let decl = program.pool.function(function);
            if !decl.is_builtin() || count == 0 {
                // Not a built-in; skip it.
                continue;
            }
            if decl.intrinsic_kind == Some(IntrinsicKind::DFdy)
                && !program.config.settings.force_no_rt_flip
            {
                // Programs that invoke the `dFdy` intrinsic will need the RTFlip input.
                program.interface.rt_flip_uniform |= RtFlip::DERIVATIVE;
            }
            if let Some(definition) = decl.definition {
                // Make sure we only add a built-in function once.
                push_unique(&mut added_builtins, definition);
            }
        }
        if added_builtins.len() == num_builtins_at_start {
            // If we didn't reference any more built-in functions than before, we're done.
            break;
        }
        // Sort the referenced builtin functions into a consistent order; we sort backwards
        // because we add elements to the shared-elements in reverse order at the end.
        let pool = &program.pool;
        added_builtins[num_builtins_at_start..]
            .sort_by_key(|&a| std::cmp::Reverse(function_key(pool, a)));
        // Update the ProgramUsage to track all these newly discovered functions.
        let usage_call_counts = usage.call_counts.len();
        for &definition in &added_builtins[num_builtins_at_start..] {
            usage.add_element(&program.pool, definition);
        }
        if usage.call_counts.len() == usage_call_counts {
            // If we aren't making any more unique function calls than before, we're done.
            break;
        }
    }
    // Insert the new functions into the program's shared elements, right at the front. They are
    // added in reverse so that the deepest dependencies are added to the top.
    let front: Vec<ElemId> = added_builtins.into_iter().rev().collect();
    program.shared_elements.splice(0..0, front);
}

/// `ProgramUsage`-independent part of `addDeclaringElement(const Symbol*)`: the element that
/// declares a built-in variable, if the variable has one (a global or an interface block).
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinVariables.cpp#L28-L55 (chrome/m156)
fn declaring_element(pool: &IrPool, var: VarId) -> Option<ElemId> {
    let variable = pool.variable(var);
    variable
        .global_var_declaration()
        .or(variable.interface_block)
}

/// The name a declaring element is sorted by: a global's variable, or an interface block's
/// instance variable (`InterfaceBlock::instanceName()`).
fn declaring_element_name(pool: &IrPool, elem: ElemId) -> String {
    match &pool.element(elem).kind {
        ProgramElementKind::GlobalVar(decl) => {
            let StatementKind::VarDeclaration(var_decl) = &pool.statement(decl.declaration).kind
            else {
                unreachable!("a global variable declaration");
            };
            pool.variable(var_decl.var).name.to_string()
        }
        ProgramElementKind::InterfaceBlock(block) => pool.variable(block.var).name.to_string(),
        _ => unreachable!("a builtin variable's declaring element"),
    }
}

/// `Transform::FindAndDeclareBuiltinVariables`: copies every built-in variable the program uses
/// (and `sk_FragColor` when `main` returns a colour) into the program's shared elements, and sets
/// the program's interface flags from the ones it reads.
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinVariables.cpp#L57-L187 (chrome/m156)
pub fn find_and_declare_builtin_variables(program: &mut Program, usage: &mut ProgramUsage) {
    let mut new_elements: Vec<ElemId> = Vec::new();
    let kind = program.config.kind;
    if ProgramConfig::is_fragment(kind) {
        // Find main() in the program and check its return type. If it's half4, we treat that as
        // an implicit write to sk_FragColor and add a reference.
        add_implicit_frag_color_write(program, &mut new_elements);
    }

    // Scan all the variables used by the program and declare any built-ins.
    let mut vars: Vec<VarId> = usage.variable_counts.keys().copied().collect();
    vars.sort_unstable();
    for var in vars {
        let variable = program.pool.variable(var);
        if !variable.builtin {
            continue;
        }
        if let Some(elem) = declaring_element(&program.pool, var) {
            push_unique(&mut new_elements, elem);
        }
        let force_no_rt_flip = program.config.settings.force_no_rt_flip;
        match variable.layout.builtin {
            // Set the RTFlip program input if we find sk_FragCoord or sk_Clockwise.
            SK_FRAGCOORD_BUILTIN if !force_no_rt_flip => {
                program.interface.rt_flip_uniform |= RtFlip::FRAG_COORD;
            }
            SK_CLOCKWISE_BUILTIN if !force_no_rt_flip => {
                program.interface.rt_flip_uniform |= RtFlip::CLOCKWISE;
            }
            // Set the UseLastFragColor program input if we find sk_LastFragColor.
            SK_LASTFRAGCOLOR_BUILTIN => program.interface.use_last_frag_color = true,
            // Set secondary color output if we find sk_SecondaryFragColor.
            SK_SECONDARYFRAGCOLOR_BUILTIN => program.interface.output_secondary_color = true,
            _ => {}
        }
    }

    // Sort the new elements: by kind (globals before interface blocks), then by name.
    new_elements.sort_by_key(|&elem| {
        (
            element_rank(&program.pool, elem),
            declaring_element_name(&program.pool, elem),
        )
    });

    // Add all the newly-declared elements to the program, and update ProgramUsage to match.
    for &element in &new_elements {
        usage.add_element(&program.pool, element);
    }
    program.shared_elements.splice(0..0, new_elements);
}

/// The rank of a declaring element's kind, in `ProgramElement::Kind` order.
fn element_rank(pool: &IrPool, elem: ElemId) -> u8 {
    match pool.element(elem).kind {
        ProgramElementKind::GlobalVar(_) => 3,
        ProgramElementKind::InterfaceBlock(_) => 4,
        _ => unreachable!("a builtin variable's declaring element"),
    }
}

/// `addImplicitFragColorWrite`: when `main` returns `half4`, the program writes `sk_FragColor`,
/// so its declaring element is needed even when the source never names it.
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinVariables.cpp#L36-L52 (chrome/m156)
fn add_implicit_frag_color_write(program: &Program, new_elements: &mut Vec<ElemId>) {
    for &elem in &program.owned_elements {
        let ProgramElementKind::Function(def) = &program.pool.element(elem).kind else {
            continue;
        };
        let decl = program.pool.function(def.declaration);
        if decl.is_main {
            if program.pool.ty(decl.return_type).matches(TypeId::HALF4) {
                // We synthesize writes to sk_FragColor if main() returns a color, even if it's
                // otherwise unreferenced.
                if let Some(SymbolId::Variable(var)) = program
                    .pool
                    .find_builtin_symbol(program.symbols, Compiler::FRAGCOLOR_NAME)
                    && let Some(decl) = declaring_element(&program.pool, var)
                {
                    push_unique(new_elements, decl);
                }
            }
            // Now that main() has been found, we can stop scanning.
            break;
        }
    }
}

/// `get_struct_definitions_from_module`: the struct definitions of the module chain that the
/// program counts, root module first, so that structs keep their order in the module hierarchy.
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinStructs.cpp#L28-L50 (chrome/m156)
fn get_struct_definitions_from_module(
    pool: &IrPool,
    usage: &ProgramUsage,
    module: &Module,
    added_struct_defs: &mut Vec<ElemId>,
) {
    if let Some(parent) = &module.parent {
        get_struct_definitions_from_module(pool, usage, parent, added_struct_defs);
    }
    for &elem in &module.elements {
        if let ProgramElementKind::StructDefinition(def) = &pool.element(elem).kind
            && usage
                .struct_counts
                .get(&def.ty)
                .is_some_and(|&count| count > 0)
        {
            added_struct_defs.push(elem);
        }
    }
}

/// `Transform::FindAndDeclareBuiltinStructs`: copies the module structs the program uses into its
/// shared elements, when the program uses any built-in struct at all.
// Port of: src/sksl/transform/SkSLFindAndDeclareBuiltinStructs.cpp#L52-L76 (chrome/m156)
pub fn find_and_declare_builtin_structs(
    program: &mut Program,
    usage: &mut ProgramUsage,
    module: &Module,
) {
    // Check if the program references any builtin structs at all. Like Skia, this looks at every
    // struct the usage table has seen, whatever its count.
    let contains_builtin_struct = usage
        .struct_counts
        .keys()
        .any(|&ty| program.pool.ty(ty).is_builtin());
    if !contains_builtin_struct {
        return;
    }
    // Visit all of our modules to find struct definitions that were referenced by ProgramUsage.
    let mut added_struct_defs: Vec<ElemId> = Vec::new();
    get_struct_definitions_from_module(&program.pool, usage, module, &mut added_struct_defs);
    // Copy the struct definitions into our shared elements, and update ProgramUsage to match.
    for &element in &added_struct_defs {
        usage.add_element(&program.pool, element);
    }
    program.shared_elements.splice(0..0, added_struct_defs);
}
