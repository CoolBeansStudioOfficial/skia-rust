// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.{h,cpp}.

//! The WGSL code generator (`ToWGSL`), behind the crate feature `wgsl`.
//!
//! The text it writes is the golden text of `tests/sksl/**/*.wgsl`: indentation, line breaks and
//! float formatting are part of the contract. Skia validates the result with Tint (a function
//! pointer, `ValidateWGSLProc`); the validator is a parameter here too, and this crate has none.
//!
//! The generator edits the IR in three places, as Skia does: it builds a `!` expression for a
//! `do`-`while` test, it makes `Setting` literals, and it synthesizes the interface block of the
//! non-block uniforms (`writeNonBlockUniformsForTests`). The program's pool is lent to the
//! [`Context`] for the duration of [`to_wgsl`] (`Context::with_program`).

// The generator mirrors Skia's string-building code statement by statement, so these pedantic
// lints are allowed for the whole module: `expr += &format!(..)` is Skia's `expr += String::printf`,
// `debug_assert!(a == b)` is its `SkASSERT(a == b)`, the long functions are Skia's (one arm per
// intrinsic and expression kind), and `x = "(".to_owned()` is `expr = "("`.
#![allow(
    clippy::format_push_string,
    clippy::manual_assert_eq,
    clippy::too_many_lines,
    clippy::assigning_clones
)]

mod deps;
mod expressions;
mod intrinsics;
mod lvalue;
mod statements;
mod types;
mod uniforms;

use std::collections::HashMap;
use std::mem;

use self::deps::{
    Deps, ProgramRequirements, collect_pipeline_io_vars, resolve_program_requirements,
};
use self::types::{
    Builtin, Delimiter, PtrAddressSpace, SAMPLER_SUFFIX, SK_POINTSIZE_BUILTIN, SK_POSITION_BUILTIN,
    TEXTURE_SUFFIX, builtin_from_sksl_name, delimiter_to_str, is_reserved_word,
    pipeline_struct_prefix, to_ptr_type, to_wgsl_type, to_wgsl_type_simple, type_is_low_precision,
    wgsl_builtin_name, wgsl_builtin_type,
};
use self::uniforms::FieldPolyfillInfo;
use crate::analysis::{ProgramUsage, get_usage};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::ir::{
    ElemId, Field, FnId, Layout, ModifierFlags, Program, ProgramElementKind, ProgramInterface,
    StmtId, TypeId, TypeKind, VarId, VariableStorage,
};
use crate::position::Position;
use crate::program_settings::ProgramConfig;
use crate::util::ShaderCaps;

/// `PrettyPrint`: whether the output is indented.
#[doc(alias = "SkSL::PrettyPrint")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrettyPrint {
    No,
    Yes,
}

/// `IncludeSyntheticCode`: whether to add the code that lets a runtime effect or test program
/// pass a WGSL validator (a fragment entry point, a stand-in coordinate argument).
#[doc(alias = "SkSL::IncludeSyntheticCode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncludeSyntheticCode {
    No,
    Yes,
}

/// `ValidateWGSLProc`: validates generated WGSL. It reports an error through the reporter and
/// returns false if the program is not valid; `warnings` receives any warnings.
#[doc(alias = "SkSL::ValidateWGSLProc")]
pub type ValidateWgslProc = fn(&mut ErrorReporter, &str, &mut String) -> bool;

/// `ToWGSL(program, caps, out, pp, isc, validateWGSL)`: converts a `Program` into WGSL code.
/// Returns the code, or `None` if errors were reported (to `ctx.errors`).
///
/// With a validator, a program that does not validate is an error too, and any warnings it
/// reports are noted at the top of the output.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4929-L4965 (chrome/m156)
#[doc(alias = "ToWGSL")]
pub fn to_wgsl(
    ctx: &mut Context,
    program: &mut Program,
    caps: &ShaderCaps,
    pretty_print: PrettyPrint,
    include_synthetic_code: IncludeSyntheticCode,
    validate_wgsl: Option<ValidateWgslProc>,
) -> Option<String> {
    ctx.errors.set_source_bytes(program.source.clone());
    let usage = get_usage(program);
    let elements: Vec<ElemId> = program.elements().collect();
    let interface = program.interface;
    let (mut result, wgsl) = ctx.with_program(program, |ctx| {
        let mut cg = WgslCodeGenerator::new(
            ctx,
            caps,
            usage,
            elements,
            interface,
            pretty_print,
            include_synthetic_code,
        );
        let ok = cg.generate_code();
        (ok, cg.out)
    });
    let mut output = String::new();
    if let (true, Some(validate)) = (result, validate_wgsl) {
        let mut warnings = String::new();
        result = validate(&mut ctx.errors, &wgsl, &mut warnings);
        if !warnings.is_empty() {
            output.push_str("/* Tint reported warnings. */\n\n");
        }
        output.push_str(&wgsl);
    } else {
        output = wgsl;
    }
    ctx.errors.set_source_bytes(std::sync::Arc::from(&b""[..]));

    result.then_some(output)
}

/// `ToWGSL(program, caps, NativeShader* out)` (and the `OutputStream` overload it calls): pretty
/// printing only in debug builds (`SK_DEBUG`), no synthetic code and no validator.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4967-L4985 (chrome/m156)
#[doc(alias = "ToWGSL")]
pub fn to_wgsl_native(
    ctx: &mut Context,
    program: &mut Program,
    caps: &ShaderCaps,
) -> Option<String> {
    let default_print_opts = if cfg!(debug_assertions) {
        PrettyPrint::Yes
    } else {
        PrettyPrint::No
    };
    to_wgsl(
        ctx,
        program,
        caps,
        default_print_opts,
        IncludeSyntheticCode::No,
        None,
    )
}

/// `SkSL::WGSLCodeGenerator`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L88-L463 (chrome/m156)
#[allow(clippy::struct_excessive_bools)] // Mirrors the generator's independent state flags.
struct WgslCodeGenerator<'a> {
    // `CodeGenerator`: the program's pool and configuration live in `ctx`.
    ctx: &'a mut Context,
    caps: &'a ShaderCaps,
    /// `fProgram.elements()`.
    elements: Vec<ElemId>,
    /// `fProgram.fInterface`.
    interface: ProgramInterface,
    /// `fProgram.fUsage`.
    usage: ProgramUsage,
    /// `fOut`: the stream being written to.
    out: String,

    pretty_print: PrettyPrint,
    gen_synthetic_code: IncludeSyntheticCode,

    /// `fHeader`: code in the header appears before the main body of code.
    header: String,

    /// `fInterfaceBlockNameMap`: unique names for anonymous interface blocks, based on the type.
    interface_block_name_map: HashMap<TypeId, String>,

    /// `fRequirements`: the functions which use stage inputs/outputs as well as required WGSL
    /// extensions.
    requirements: ProgramRequirements,
    pipeline_inputs: Vec<VarId>,
    pipeline_outputs: Vec<VarId>,

    /// `fF32Polyfills` and `fF16Polyfills`: whether we have written the polyfill for `inverse()`
    /// and `outerProduct()` for a given matrix type.
    f32_polyfills: WrittenPolyfills,
    f16_polyfills: WrittenPolyfills,

    /// `fFieldPolyfillMap`: uniform polyfill support in cases where WGSL and std140 disagree.
    /// In std140 layout, matrices need to be represented as arrays of @align(16) vectors, and
    /// array elements are wrapped in a struct containing a single @align(16) element. Arrays of
    /// matrices combine both wrappers. These wrapper structs are unpacked into natively-typed
    /// globals at the shader entrypoint. The key is the struct type and the field's index.
    /// Skia's `THashMap` is iterated only through a sort by replacement name.
    field_polyfill_map: HashMap<(TypeId, usize), FieldPolyfillInfo>,

    /// `fSyntheticGlobalUniformsBlock`.
    synthetic_global_uniforms_block: Option<ElemId>,

    // Output processing state.
    indentation: i32,
    at_line_start: bool,
    has_unconditional_return: bool,
    at_function_scope: bool,
    /// `fNeedsConstEvalWorkaround`: see skbug.com/40045457.
    needs_const_eval_workaround: bool,
    conditional_scope_depth: i32,
    local_size_x: i32,
    local_size_y: i32,
    local_size_z: i32,

    scratch_count: i32,
}

/// `WGSLCodeGenerator::WrittenPolyfills`: indexed by row or column count - 2, i.e. [2,3,4] ->
/// idx [0,1,2].
#[derive(Debug, Default)]
struct WrittenPolyfills {
    inverse: [bool; 3],
    outer_product: [[bool; 3]; 3],
}

/// `AssembleMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssembleMode {
    /// Will either be inlined expression or let to avoid const-eval.
    Auto,
    /// Will be inlined if trivial, or a let so reuse by caller is free.
    UsedMultipleTimes,
    /// Will be written to a let.
    ForceLet,
}

impl<'a> WgslCodeGenerator<'a> {
    fn new(
        ctx: &'a mut Context,
        caps: &'a ShaderCaps,
        usage: ProgramUsage,
        elements: Vec<ElemId>,
        interface: ProgramInterface,
        pretty_print: PrettyPrint,
        gen_synthetic_code: IncludeSyntheticCode,
    ) -> Self {
        Self {
            ctx,
            caps,
            elements,
            interface,
            usage,
            out: String::new(),
            pretty_print,
            gen_synthetic_code,
            header: String::new(),
            interface_block_name_map: HashMap::new(),
            requirements: ProgramRequirements::default(),
            pipeline_inputs: Vec::new(),
            pipeline_outputs: Vec::new(),
            f32_polyfills: WrittenPolyfills::default(),
            f16_polyfills: WrittenPolyfills::default(),
            field_polyfill_map: HashMap::new(),
            synthetic_global_uniforms_block: None,
            indentation: 0,
            at_line_start: false,
            has_unconditional_return: false,
            at_function_scope: false,
            needs_const_eval_workaround: false,
            conditional_scope_depth: 0,
            local_size_x: 1,
            local_size_y: 1,
            local_size_z: 1,
            scratch_count: 0,
        }
    }

    /// The kind of the program (`fProgram.fConfig->fKind`).
    fn program_kind(&self) -> crate::program_settings::ProgramKind {
        self.ctx.config().kind
    }

    /// `generateCode()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1397-L1441 (chrome/m156)
    fn generate_code(&mut self) -> bool {
        // The resources of a WGSL program are structured in the following way:
        // - Stage attribute inputs and outputs are bundled inside synthetic structs called
        //   VSIn/VSOut/FSIn/FSOut/CSIn.
        // - All uniform and storage type resources are declared in global scope.
        self.preprocess_program();

        {
            // `AutoOutputStream outputToHeader(this, &fHeader, &fIndentation)`.
            mem::swap(&mut self.out, &mut self.header);
            let old_indentation = mem::replace(&mut self.indentation, 0);
            self.write_enables();
            self.write_stage_input_struct();
            self.write_stage_output_struct();
            self.write_uniforms_and_buffers();
            self.write_non_block_uniforms_for_tests();
            self.indentation = old_indentation;
            mem::swap(&mut self.out, &mut self.header);
        }
        let body;
        {
            // Emit the program body: `AutoOutputStream outputToBody(this, &body, &fIndentation)`.
            let old_out = mem::take(&mut self.out);
            let old_indentation = mem::replace(&mut self.indentation, 0);
            let mut main_func: Option<ElemId> = None;
            for e in self.elements.clone() {
                self.write_program_element(e);

                if let ProgramElementKind::Function(f) = &self.ctx.pool.element(e).kind
                    && self.ctx.pool.function(f.declaration).is_main
                {
                    main_func = Some(e);
                }
            }

            // At the bottom of the program body, emit the entrypoint function.
            // The entrypoint relies on state that has been collected while we emitted the rest of
            // the program, so it's important to do it last to make sure we don't miss anything.
            if let Some(main_func) = main_func {
                self.write_entry_point(main_func);
            }
            body = mem::replace(&mut self.out, old_out);
            self.indentation = old_indentation;
        }

        // `write_stringstream(fHeader, *fOut)`.
        self.out.push_str(&self.header);
        self.out.push_str(&body);

        self.write_uniform_polyfills();

        self.ctx.errors.error_count() == 0
    }

    /// `preprocessProgram()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1648-L1652 (chrome/m156)
    fn preprocess_program(&mut self) {
        let pool = &self.ctx.pool;
        self.requirements = resolve_program_requirements(pool, &self.elements);
        self.pipeline_inputs = collect_pipeline_io_vars(pool, &self.elements, ModifierFlags::IN);
        self.pipeline_outputs = collect_pipeline_io_vars(pool, &self.elements, ModifierFlags::OUT);
    }

    /// `write(s)`: writes output content while correctly handling indentation.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1654-L1666 (chrome/m156)
    fn write(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if self.at_line_start && self.pretty_print == PrettyPrint::Yes {
            for _ in 0..self.indentation {
                self.out.push_str("  ");
            }
        }
        self.out.push_str(s);
        self.at_line_start = false;
    }

    /// `writeLine(s)`.
    fn write_line(&mut self, s: &str) {
        self.write(s);
        self.out.push('\n');
        self.at_line_start = true;
    }

    /// `finishLine()`.
    fn finish_line(&mut self) {
        if !self.at_line_start {
            self.write_line("");
        }
    }

    /// `assembleName(name)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1680-L1692 (chrome/m156)
    fn assemble_name(&mut self, name: &str) -> String {
        if name.is_empty() {
            // WGSL doesn't allow anonymous function parameters.
            let result = format!("_skAnonymous{}", self.scratch_count);
            self.scratch_count += 1;
            return result;
        }
        // Add `R_` before reserved names to avoid any potential reserved-word conflict.
        if name.starts_with("_sk") || name.starts_with("R_") || is_reserved_word(name) {
            format!("R_{name}")
        } else {
            name.to_owned()
        }
    }

    /// `writeVariableDecl(layout, type, name, delimiter)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1694-L1700 (chrome/m156)
    fn write_variable_decl(
        &mut self,
        layout: &Layout,
        ty: TypeId,
        name: &str,
        delimiter: Delimiter,
    ) {
        let name = self.assemble_name(name);
        self.write(&name);
        let decl = format!(": {}", to_wgsl_type(self.ctx, ty, Some(layout), false));
        self.write(&decl);
        self.write_line(delimiter_to_str(delimiter));
    }

    /// `writePipelineIODeclaration(layout, type, modifiers, name, delimiter)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1702-L1742 (chrome/m156)
    fn write_pipeline_io_declaration(
        &mut self,
        layout: &Layout,
        ty: TypeId,
        modifiers: ModifierFlags,
        name: &str,
        delimiter: Delimiter,
    ) {
        // In WGSL, an entry-point IO parameter is "one of either a built-in value or assigned a
        // location". However, some SkSL declarations, specifically sk_FragColor, can contain both
        // a location and a builtin modifier. In addition, WGSL doesn't have a built-in equivalent
        // for sk_FragColor as it relies on the user-defined location for a render target.
        //
        // Instead of special-casing sk_FragColor, we just give higher precedence to a location
        // modifier if a declaration happens to both have a location and it's a built-in.
        //
        // Also see:
        // https://www.w3.org/TR/WGSL/#input-output-locations
        // https://www.w3.org/TR/WGSL/#attribute-location
        // https://www.w3.org/TR/WGSL/#builtin-inputs-outputs
        if layout.location >= 0 {
            self.write_user_defined_io_decl(layout, ty, modifiers, name, delimiter);
            return;
        }
        if layout.builtin >= 0 {
            if layout.builtin == SK_POINTSIZE_BUILTIN {
                // WebGPU does not support the point-size builtin, but we silently replace it with
                // a global variable when it is used, instead of reporting an error.
                return;
            }
            if let Some(builtin) = builtin_from_sksl_name(layout.builtin) {
                // Builtin IO parameters should only have in/out modifiers, which are then
                // implicit in the generated WGSL, hence why writeBuiltinIODecl does not need them
                // passed in.
                debug_assert!((modifiers & !(ModifierFlags::IN | ModifierFlags::OUT)).is_empty());
                self.write_builtin_io_decl(name, builtin, delimiter);
                return;
            }
        }
        self.ctx.errors.error(
            Position::default(),
            &format!("declaration '{name}' is not supported"),
        );
    }

    /// `writeUserDefinedIODecl(layout, type, flags, name, delimiter)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1744-L1774 (chrome/m156)
    fn write_user_defined_io_decl(
        &mut self,
        layout: &Layout,
        ty: TypeId,
        flags: ModifierFlags,
        name: &str,
        delimiter: Delimiter,
    ) {
        self.write(&format!("@location({}) ", layout.location));

        // @blend_src is only allowed when doing dual-source blending, and only on color
        // attachment 0.
        if layout.location == 0 && layout.index >= 0 && self.interface.output_secondary_color {
            self.write(&format!("@blend_src({}) ", layout.index));
        }

        // "User-defined IO of scalar or vector integer type must always be specified as
        // @interpolate(flat)" (see https://www.w3.org/TR/WGSL/#interpolation)
        let (is_integer, is_integer_vector) = {
            let t = self.ctx.pool.ty(ty);
            (
                t.is_integer(),
                t.is_vector() && t.component_type().is_integer(),
            )
        };
        if flags.is_flat() || is_integer || is_integer_vector {
            // We can use 'either' to hint to WebGPU that we don't care about the provoking vertex
            // and avoid any expensive shader or data rewriting to ensure 'first'. Skia has a
            // long-standing policy to only use flat shading when it's constant for a primitive so
            // the vertex doesn't matter. See https://www.w3.org/TR/WGSL/#interpolation-sampling-either
            self.write("@interpolate(flat, either) ");
        } else if flags.contains(ModifierFlags::NO_PERSPECTIVE) {
            self.write("@interpolate(linear) ");
        }

        self.write_variable_decl(layout, ty, name, delimiter);
    }

    /// `writeBuiltinIODecl(type, name, builtin, delimiter)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1776-L1785 (chrome/m156)
    fn write_builtin_io_decl(&mut self, name: &str, builtin: Builtin, delimiter: Delimiter) {
        self.write(wgsl_builtin_name(builtin));
        self.write(" ");
        let name = self.assemble_name(name);
        self.write(&name);
        self.write(": ");
        let ty = wgsl_builtin_type(builtin, self.ctx);
        self.write(ty);
        self.write_line(delimiter_to_str(delimiter));
    }

    /// `writeFunction(f)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1787-L1867 (chrome/m156)
    fn write_function(&mut self, declaration: FnId, body: StmtId) {
        self.has_unconditional_return = false;
        self.conditional_scope_depth = 0;

        debug_assert!(!self.at_function_scope);
        self.at_function_scope = true;

        // WGSL parameters are immutable and are considered as taking no storage, but SkSL
        // parameters are real variables. To work around this, we make var-based copies of
        // parameters. It's wasteful to make a copy of every single parameter--even if the
        // compiler can eventually optimize them all away, that takes time and generates bloated
        // code. So, we only make parameter copies if the variable is actually written-to.
        let parameters = self.ctx.pool.function(declaration).parameters.clone();
        let mut param_needs_dedicated_storage = vec![true; parameters.len()];

        for (index, &param_id) in parameters.iter().enumerate() {
            let param = self.ctx.pool.variable(param_id);
            if self.ctx.pool.ty(param.ty).is_opaque() || param.name.is_empty() {
                // Opaque-typed or anonymous parameters don't need dedicated storage.
                param_needs_dedicated_storage[index] = false;
                continue;
            }

            let counts = self.usage.get_variable(param_id);
            if param.modifier_flags.contains(ModifierFlags::OUT) || counts.write == 0 {
                // Variables which are never written-to don't need dedicated storage.
                // Out-parameters are passed as pointers; the pointer itself is never modified, so
                // it doesn't need dedicated storage.
                param_needs_dedicated_storage[index] = false;
            }
        }

        self.write_function_declaration(declaration, &param_needs_dedicated_storage);
        self.write_line(" {");
        self.indentation += 1;

        // The parameters were given generic names like `_skParam1`, because WGSL parameters don't
        // have storage and are immutable. If mutability is required, we create variables here;
        // otherwise, we create properly-named `let` aliases.
        for (index, &param_id) in parameters.iter().enumerate() {
            if param_needs_dedicated_storage[index] {
                self.write("var ");
                let mangled = self.ctx.pool.variable(param_id).mangled_name().to_owned();
                let name = self.assemble_name(&mangled);
                self.write(&name);
                self.write(" = _skParam");
                self.write(&index.to_string());
                self.write_line(";");
            }
        }

        self.write_block(body);

        // If conditional_scope_depth isn't zero, we have an unbalanced +1 or -1 when updating the
        // depth.
        debug_assert!(self.conditional_scope_depth == 0);
        let return_type = self.ctx.pool.function(declaration).return_type;
        if !self.has_unconditional_return && !self.ctx.pool.ty(return_type).is_void() {
            self.write("return ");
            let ty = to_wgsl_type_simple(self.ctx, return_type);
            self.write(&ty);
            self.write_line("();");
        }

        self.indentation -= 1;
        self.write_line("}");

        debug_assert!(self.at_function_scope);
        self.at_function_scope = false;
    }

    /// `writeFunctionDeclaration(decl, paramNeedsDedicatedStorage)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1869-L1952 (chrome/m156)
    fn write_function_declaration(
        &mut self,
        declaration: FnId,
        param_needs_dedicated_storage: &[bool],
    ) {
        self.write("fn ");
        let (is_main, mangled_name, parameters, return_type) = {
            let pool = &self.ctx.pool;
            let decl = pool.function(declaration);
            (
                decl.is_main,
                decl.mangled_name(pool),
                decl.parameters.clone(),
                decl.return_type,
            )
        };
        if is_main {
            self.write("_skslMain(");
        } else {
            let name = self.assemble_name(&mangled_name);
            self.write(&name);
            self.write("(");
        }
        let mut separator = crate::string::Separator::new();
        if self.write_function_dependency_params(declaration) {
            separator.next_str(); // update the separator as parameters have been written
        }
        for (index, &param_id) in parameters.iter().enumerate() {
            self.write(separator.next_str());

            let param = self.ctx.pool.variable(param_id).clone();
            let (is_opaque, is_sampler, is_unsized_array) = {
                let t = self.ctx.pool.ty(param.ty);
                (t.is_opaque(), t.is_sampler(), t.is_unsized_array())
            };
            if is_opaque {
                debug_assert!(!param_needs_dedicated_storage[index]);
                if is_sampler {
                    // Create parameters for both the texture and associated sampler.
                    self.write(&param.name);
                    self.write(TEXTURE_SUFFIX);
                    self.write(": texture_2d<f32>, ");
                    self.write(&param.name);
                    self.write(SAMPLER_SUFFIX);
                    self.write(": sampler");
                } else {
                    // Create a parameter for the opaque object.
                    self.write(&param.name);
                    self.write(": ");
                    let ty = to_wgsl_type(self.ctx, param.ty, Some(&param.layout), false);
                    self.write(&ty);
                }
            } else {
                if param_needs_dedicated_storage[index] || param.name.is_empty() {
                    // Create an unnamed parameter. If the parameter needs dedicated storage, it
                    // will later be assigned a `var` in the function body. (If it's anonymous, a
                    // var isn't needed.)
                    self.write("_skParam");
                    self.write(&index.to_string());
                } else {
                    // Use the name directly from the SkSL program.
                    let name = self.assemble_name(&param.name);
                    self.write(&name);
                }
                self.write(": ");
                if is_unsized_array {
                    // Creates a storage address space pointer for unsized array parameters.
                    // The buffer the array resides in must be marked `readonly` to have the array
                    // be used in function parameters, since access modes in wgsl must exactly
                    // match.
                    self.write("ptr<storage, ");
                    let ty = to_wgsl_type(self.ctx, param.ty, Some(&param.layout), false);
                    self.write(&ty);
                    self.write(", read>");
                } else if param.modifier_flags.contains(ModifierFlags::OUT) {
                    // Declare an "out" function parameter as a pointer.
                    let ty = to_ptr_type(
                        self.ctx,
                        param.ty,
                        Some(&param.layout),
                        PtrAddressSpace::Function,
                    );
                    self.write(&ty);
                } else {
                    let ty = to_wgsl_type(self.ctx, param.ty, Some(&param.layout), false);
                    self.write(&ty);
                }
            }
        }
        self.write(")");
        if !self.ctx.pool.ty(return_type).is_void() {
            self.write(" -> ");
            let ty = to_wgsl_type_simple(self.ctx, return_type);
            self.write(&ty);
        }
    }

    /// `writeEntryPoint(main)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1954-L2118 (chrome/m156)
    fn write_entry_point(&mut self, main: ElemId) {
        let (main_position, main_decl) = {
            let element = self.ctx.pool.element(main);
            let ProgramElementKind::Function(f) = &element.kind else {
                unreachable!("the entry point is a function definition");
            };
            (element.position, f.declaration)
        };
        debug_assert!(self.ctx.pool.function(main_decl).is_main);
        let program_kind = self.program_kind();

        let init_polyfill = self
            .field_polyfill_map
            .values()
            .any(|info| info.was_accessed);

        if self.gen_synthetic_code == IncludeSyntheticCode::Yes
            && ProgramConfig::is_runtime_shader(program_kind)
        {
            // Synthesize a basic entrypoint which just calls straight through to main.
            // This is only used by skslc and just needs to pass the WGSL validator; Skia won't
            // ever emit functions like this.
            self.write("@fragment fn main(@location(0) _coords: vec2<f32>) -> @location(0) ");
            let return_type = self.ctx.pool.function(main_decl).return_type;
            let ty = to_wgsl_type_simple(self.ctx, return_type);
            self.write(&ty);
            self.write_line(" {");

            self.indentation += 1;
            if init_polyfill {
                self.write_line("_skInitializePolyfilledUniforms();");
            }
            self.write_line("return _skslMain(_coords);");
            self.indentation -= 1;
            self.write_line("}");
            return;
        }

        // The input and output parameters for a vertex/fragment stage entry point function have
        // the FSIn/FSOut/VSIn/VSOut/CSIn struct types that have been synthesized in
        // generateCode(). An entrypoint always has a predictable signature and acts as a
        // trampoline to the user-defined main function.
        if ProgramConfig::is_vertex(program_kind) {
            self.write("@vertex");
        } else if ProgramConfig::is_fragment(program_kind) {
            self.write("@fragment");
        } else if ProgramConfig::is_compute(program_kind) {
            self.write("@compute @workgroup_size(");
            self.write(&self.local_size_x.to_string());
            self.write(", ");
            self.write(&self.local_size_y.to_string());
            self.write(", ");
            self.write(&self.local_size_z.to_string());
            self.write(")");
        } else {
            self.ctx
                .errors
                .error(Position::default(), "program kind not supported");
            return;
        }

        self.write(" fn main(");
        // The stage input struct is a parameter passed to main().
        if self.needs_stage_input_struct() {
            self.write("_stageIn: ");
            self.write(pipeline_struct_prefix(program_kind));
            self.write("In");
        }
        // The stage output struct is returned from main().
        if self.needs_stage_output_struct() {
            self.write(") -> ");
            self.write(pipeline_struct_prefix(program_kind));
            self.write_line("Out {");
        } else {
            self.write_line(") {");
        }
        // Initialize polyfilled matrix uniforms if any were used.
        self.indentation += 1;
        if init_polyfill {
            self.write_line("_skInitializePolyfilledUniforms();");
        }

        // Declare the stage output struct.
        if self.needs_stage_output_struct() {
            self.write("var _stageOut: ");
            self.write(pipeline_struct_prefix(program_kind));
            self.write_line("Out;");
        }

        // We are compiling a Runtime Effect as a fragment shader, for testing purposes. We assign
        // the result from _skslMain into sk_FragColor if the user-defined main returns a color.
        // This doesn't actually matter, but it is more indicative of what a real program would do.
        // `addImplicitFragColorWrite` from Transform::FindAndDeclareBuiltinVariables has already
        // injected sk_FragColor into our stage outputs even if it wasn't explicitly referenced.
        if self.gen_synthetic_code == IncludeSyntheticCode::Yes
            && ProgramConfig::is_fragment(program_kind)
        {
            let return_type = self.ctx.pool.function(main_decl).return_type;
            if self.ctx.pool.ty(return_type).matches(TypeId::HALF4) {
                self.write("_stageOut.sk_FragColor = ");
            }
        }

        // Generate a function call to the user-defined main.
        self.write("_skslMain(");
        let mut separator = crate::string::Separator::new();
        if let Some(deps) = self.requirements.dependencies.get(&main_decl).copied() {
            if deps.contains(Deps::PIPELINE_INPUTS) {
                self.write(separator.next_str());
                self.write("_stageIn");
            }
            if deps.contains(Deps::PIPELINE_OUTPUTS) {
                self.write(separator.next_str());
                self.write("&_stageOut");
            }
        }

        if self.gen_synthetic_code == IncludeSyntheticCode::Yes
            && let Some(v) = self.ctx.pool.function(main_decl).main_coords_parameter()
        {
            // We are compiling a Runtime Effect as a fragment shader, for testing purposes.
            // We need to synthesize a coordinates parameter, but the coordinates don't matter.
            debug_assert!(ProgramConfig::is_fragment(program_kind));
            let ty = self.ctx.pool.variable(v).ty;
            if !self.ctx.pool.ty(ty).matches(TypeId::FLOAT2) {
                let description = self.ctx.pool.ty(ty).description();
                self.ctx.errors.error(
                    main_position,
                    &format!("main function has unsupported parameter: {description}"),
                );
                return;
            }
            self.write(separator.next_str());
            self.write("/*fragcoord*/ vec2<f32>()");
        }

        self.write_line(");");

        if self.needs_stage_output_struct() {
            // Return the stage output struct.
            self.write_line("return _stageOut;");
        }

        self.indentation -= 1;
        self.write_line("}");
    }

    /// `writeProgramElement(e)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4472-L4509 (chrome/m156)
    fn write_program_element(&mut self, e: ElemId) {
        let kind = self.ctx.pool.element(e).kind.clone();
        match kind {
            ProgramElementKind::GlobalVar(d) => {
                self.write_global_var_declaration(d.declaration);
            }
            ProgramElementKind::StructDefinition(s) => {
                self.write_struct_definition(s.ty);
            }
            // WGSL supports extensions via the "enable" directive
            // (https://www.w3.org/TR/WGSL/#enable-extensions-sec ). While we could easily emit
            // this directive, we should first ensure that all possible SkSL extension names are
            // converted to their appropriate WGSL extension.
            //
            // All interface block declarations are handled explicitly as the "program header" in
            // generateCode().
            //
            // A WGSL function declaration must contain its body and the function name is in scope
            // for the entire program (see https://www.w3.org/TR/WGSL/#function-declaration and
            // https://www.w3.org/TR/WGSL/#declaration-and-scope).
            //
            // As such, we don't emit function prototypes.
            ProgramElementKind::Extension(_)
            | ProgramElementKind::InterfaceBlock(_)
            | ProgramElementKind::FunctionPrototype(_) => {}
            ProgramElementKind::Function(f) => {
                self.write_function(f.declaration, f.body);
            }
            ProgramElementKind::Modifiers(m) => {
                self.write_modifiers_declaration(e, &m);
            }
        }
    }

    /// `writeTextureOrSampler(var, bindingLocation, suffix, wgslType)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4511-L4533 (chrome/m156)
    fn write_texture_or_sampler(
        &mut self,
        var: VarId,
        binding_location: i32,
        suffix: &str,
        wgsl_type: &str,
    ) {
        let (ty, set, mangled_name, declaration) = {
            let pool = &self.ctx.pool;
            let v = pool.variable(var);
            (
                v.ty,
                v.layout.set,
                v.mangled_name().to_owned(),
                v.var_declaration(pool),
            )
        };
        if self.ctx.pool.ty(ty).dimensions() != crate::ir::SpvDim::Dim2D {
            // Skia currently only uses 2D textures.
            let position =
                declaration.map_or(Position::default(), |d| self.ctx.pool.statement(d).position);
            self.ctx
                .errors
                .error(position, "unsupported texture dimensions");
            return;
        }

        self.write("@group(");
        self.write(&set.max(0).to_string());
        self.write(") @binding(");
        self.write(&binding_location.to_string());
        self.write(") var ");
        let name = self.assemble_name(&mangled_name);
        self.write(&name);
        self.write(suffix);
        self.write(": ");
        self.write(wgsl_type);
        self.write_line(";");
    }

    /// `writeGlobalVarDeclaration(d)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4535-L4610 (chrome/m156)
    fn write_global_var_declaration(&mut self, declaration: StmtId) {
        let (var_id, value) = match &self.ctx.pool.statement(declaration).kind {
            crate::ir::StatementKind::VarDeclaration(v) => (v.var, v.value),
            _ => unreachable!("a global variable declaration holds a VarDeclaration"),
        };
        let var = self.ctx.pool.variable(var_id).clone();
        if var
            .modifier_flags
            .intersects(ModifierFlags::IN | ModifierFlags::OUT)
            || self.is_in_global_uniforms(&var)
        {
            // Pipeline stage I/O parameters and top-level (non-block) uniforms are handled
            // specially in generateCode().
            return;
        }

        let var_kind = self.ctx.pool.ty(var.ty).type_kind;
        if var_kind == TypeKind::Sampler {
            // If the sampler binding was unassigned, provide a scratch value; this will make
            // golden-output tests pass, but will not actually be usable for drawing.
            let sampler_location = if var.layout.sampler >= 0 {
                var.layout.sampler
            } else {
                let location = 10000 + self.scratch_count;
                self.scratch_count += 1;
                location
            };
            self.write_texture_or_sampler(var_id, sampler_location, SAMPLER_SUFFIX, "sampler");

            // If the texture binding was unassigned, provide a scratch value (for golden-output
            // tests).
            let texture_location = if var.layout.texture >= 0 {
                var.layout.texture
            } else {
                let location = 10000 + self.scratch_count;
                self.scratch_count += 1;
                location
            };
            self.write_texture_or_sampler(
                var_id,
                texture_location,
                TEXTURE_SUFFIX,
                "texture_2d<f32>",
            );
            return;
        }

        if var_kind == TypeKind::Texture {
            // If a binding location was unassigned, provide a scratch value (for golden-output
            // tests).
            let texture_location = if var.layout.binding >= 0 {
                var.layout.binding
            } else {
                let location = 10000 + self.scratch_count;
                self.scratch_count += 1;
                location
            };
            // For a texture without an associated sampler, we don't apply a suffix.
            let ty = to_wgsl_type(self.ctx, var.ty, Some(&var.layout), false);
            self.write_texture_or_sampler(var_id, texture_location, "", &ty);
            return;
        }

        let mut initializer = String::new();
        if let Some(value) = value {
            // We assume here that the initial-value expression will not emit any helper
            // statements. Initial-value expressions are required to pass IsConstantExpression,
            // which limits the blast radius to constructors, literals, and other constant
            // values/variables.
            initializer.push_str(" = ");
            initializer.push_str(&self.assemble_expression(
                value,
                crate::operator::OperatorPrecedence::Assignment,
                AssembleMode::Auto,
            ));
        }

        if var.modifier_flags.is_const() {
            self.write("const ");
        } else if var.modifier_flags.is_workgroup() {
            self.write("var<workgroup> ");
        } else if var.modifier_flags.is_pixel_local() {
            self.write("var<pixel_local> ");
        } else {
            self.write("var<private> ");
        }
        let name = self.assemble_name(var.mangled_name());
        self.write(&name);
        let ty = format!(
            ": {}",
            to_wgsl_type(self.ctx, var.ty, Some(&var.layout), false)
        );
        self.write(&ty);
        self.write(&initializer);
        self.write_line(";");
    }

    /// `writeStructDefinition(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4612-L4617 (chrome/m156)
    fn write_struct_definition(&mut self, ty: TypeId) {
        let name = self.ctx.pool.ty(ty).display_name().to_owned();
        self.write_line(&format!("struct {name} {{"));
        self.write_fields(ty, None);
        self.write_line("};");
    }

    /// `writeModifiersDeclaration(modifiers)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4619-L4639 (chrome/m156)
    fn write_modifiers_declaration(
        &mut self,
        element: ElemId,
        modifiers: &crate::ir::ModifiersDeclaration,
    ) {
        use crate::ir::LayoutFlags;
        let mut flags = modifiers.layout.flags;
        flags &=
            !(LayoutFlags::LOCAL_SIZE_X | LayoutFlags::LOCAL_SIZE_Y | LayoutFlags::LOCAL_SIZE_Z);
        if !flags.is_empty() {
            let position = self.ctx.pool.element(element).position;
            self.ctx.errors.error(position, "unsupported declaration");
            return;
        }

        if modifiers.layout.local_size_x >= 0 {
            self.local_size_x = modifiers.layout.local_size_x;
        }
        if modifiers.layout.local_size_y >= 0 {
            self.local_size_y = modifiers.layout.local_size_y;
        }
        if modifiers.layout.local_size_z >= 0 {
            self.local_size_z = modifiers.layout.local_size_z;
        }
    }

    /// `writeFields(fields, memoryLayout)`: writes the WGSL struct fields for `SkSL` structs and
    /// interface blocks. Enforces WGSL address space layout constraints
    /// (<https://www.w3.org/TR/WGSL/#address-space-layout-constraints>) if a `layout` is
    /// provided. A struct that does not need to be host-shareable does not require a `layout`.
    /// `struct_type` is the struct whose fields they are.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4641-L4708 (chrome/m156)
    fn write_fields(
        &mut self,
        struct_type: TypeId,
        memory_layout: Option<crate::memory_layout::MemoryLayout>,
    ) {
        let struct_type = self.ctx.pool.ty(struct_type).resolve().id();
        let fields: Vec<Field> = self.ctx.pool.ty(struct_type).fields().to_vec();
        self.indentation += 1;

        for (index, field) in fields.iter().enumerate() {
            if let Some(memory_layout) = memory_layout
                && !memory_layout.is_supported(self.ctx.pool.ty(field.ty))
            {
                // Reject types that aren't supported by the memory layout.
                let name = self.ctx.pool.ty(field.ty).name().to_owned();
                self.ctx.errors.error(
                    field.position,
                    &format!("type '{name}' is not permitted here"),
                );
                return;
            }

            // Prepend @size(n) to enforce the offsets from the SkSL layout. (This is effectively
            // a gadget that we can use to insert padding between elements.)
            if index + 1 < fields.len() {
                let this_field_offset = field.layout.offset;
                let next_field_offset = fields[index + 1].layout.offset;
                if index == 0 && this_field_offset > 0 {
                    self.ctx
                        .errors
                        .error(field.position, "field must have an offset of zero");
                    return;
                }
                if this_field_offset >= 0 && next_field_offset > this_field_offset {
                    self.write("@size(");
                    self.write(&(next_field_offset - this_field_offset).to_string());
                    self.write(") ");
                }
            }

            let name = self.assemble_name(&field.name);
            self.write(&name);
            self.write(": ");
            if let Some(info) = self.field_polyfill_map.get(&(struct_type, index)) {
                let (is_array, is_matrix) = (info.is_array, info.is_matrix);
                let (abbreviated, columns, rows) = {
                    let t = self.ctx.pool.ty(field.ty);
                    (
                        t.abbreviated_name,
                        if is_array || is_matrix {
                            t.columns()
                        } else {
                            0
                        },
                        if is_matrix && !is_array { t.rows() } else { 0 },
                    )
                };
                if is_array {
                    // This properly handles arrays of matrices, as well as arrays of other
                    // primitives.
                    debug_assert!(self.ctx.pool.ty(field.ty).is_array());
                    self.write("array<_skArrayElement_");
                    self.write(abbreviated);
                    self.write(", ");
                    self.write(&columns.to_string());
                    self.write(">");
                } else if is_matrix {
                    self.write("_skMatrix");
                    self.write(&columns.to_string());
                    self.write(&rows.to_string());
                    let low = type_is_low_precision(self.ctx, field.ty);
                    self.write(if low { "h" } else { "f" });
                }
            } else {
                let ty = to_wgsl_type(self.ctx, field.ty, Some(&field.layout), false);
                self.write(&ty);
            }
            self.write_line(",");
        }

        self.indentation -= 1;
    }

    /// `writeEnables()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4710-L4727 (chrome/m156)
    fn write_enables(&mut self) {
        self.write_line("diagnostic(off, derivative_uniformity);");
        self.write_line("diagnostic(off, chromium.unreachable_code);");

        if !self.ctx.config().settings.force_high_precision {
            // Always turn on f16, even if technically a given program might not use half values.
            self.write_line("enable f16;");
        }
        if self.requirements.pixel_local_extension {
            self.write_line("enable chromium_experimental_pixel_local;");
        }
        if self.interface.use_last_frag_color {
            self.write_line("enable chromium_experimental_framebuffer_fetch;");
        }
        if self.interface.output_secondary_color {
            self.write_line("enable dual_source_blending;");
        }
    }

    /// `needsStageInputStruct()`: it is illegal to declare a struct with no members; we can't
    /// emit a placeholder empty stage input struct.
    fn needs_stage_input_struct(&self) -> bool {
        !self.pipeline_inputs.is_empty()
    }

    /// `writeStageInputStruct()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4735-L4766 (chrome/m156)
    fn write_stage_input_struct(&mut self) {
        if !self.needs_stage_input_struct() {
            return;
        }

        let struct_name_prefix = pipeline_struct_prefix(self.program_kind());
        debug_assert_ne!(struct_name_prefix, "");

        self.write("struct ");
        self.write(struct_name_prefix);
        self.write_line("In {");
        self.indentation += 1;

        for v in self.pipeline_inputs.clone() {
            let var = self.ctx.pool.variable(v).clone();
            if self.ctx.pool.ty(var.ty).is_interface_block() {
                let fields: Vec<Field> = self.ctx.pool.ty(var.ty).fields().to_vec();
                for f in &fields {
                    self.write_pipeline_io_declaration(
                        &f.layout,
                        f.ty,
                        f.modifier_flags,
                        &f.name,
                        Delimiter::Comma,
                    );
                }
            } else {
                self.write_pipeline_io_declaration(
                    &var.layout,
                    var.ty,
                    var.modifier_flags,
                    var.mangled_name(),
                    Delimiter::Comma,
                );
            }
        }

        self.indentation -= 1;
        self.write_line("};");
    }

    /// `needsStageOutputStruct()`: it is illegal to declare a struct with no members. However,
    /// vertex programs will _always_ have an output stage in WGSL, because the spec requires them
    /// to emit `@builtin(position)`. So we always synthesize a reference to `sk_Position` even if
    /// the program doesn't need it.
    fn needs_stage_output_struct(&self) -> bool {
        !self.pipeline_outputs.is_empty() || ProgramConfig::is_vertex(self.program_kind())
    }

    /// `writeStageOutputStruct()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4777-L4838 (chrome/m156)
    fn write_stage_output_struct(&mut self) {
        if !self.needs_stage_output_struct() {
            return;
        }

        let kind = self.program_kind();
        let struct_name_prefix = pipeline_struct_prefix(kind);
        debug_assert_ne!(struct_name_prefix, "");

        self.write("struct ");
        self.write(struct_name_prefix);
        self.write_line("Out {");
        self.indentation += 1;

        let mut declared_position_builtin = false;
        let mut requires_point_size_builtin = false;
        for v in self.pipeline_outputs.clone() {
            let var = self.ctx.pool.variable(v).clone();
            if self.ctx.pool.ty(var.ty).is_interface_block() {
                let fields: Vec<Field> = self.ctx.pool.ty(var.ty).fields().to_vec();
                for f in &fields {
                    self.write_pipeline_io_declaration(
                        &f.layout,
                        f.ty,
                        f.modifier_flags,
                        &f.name,
                        Delimiter::Comma,
                    );
                    if f.layout.builtin == SK_POSITION_BUILTIN {
                        declared_position_builtin = true;
                    } else if f.layout.builtin == SK_POINTSIZE_BUILTIN {
                        // sk_PointSize is explicitly not supported by `builtin_from_sksl_name` so
                        // writePipelineIODeclaration will never write it. We mark it here if the
                        // declaration is needed so we can synthesize it below.
                        requires_point_size_builtin = true;
                    }
                }
            } else {
                self.write_pipeline_io_declaration(
                    &var.layout,
                    var.ty,
                    var.modifier_flags,
                    var.mangled_name(),
                    Delimiter::Comma,
                );
            }
        }

        // A vertex program must include the `position` builtin in its entrypoint's return type.
        let position_builtin_required = ProgramConfig::is_vertex(kind);
        if position_builtin_required && !declared_position_builtin {
            self.write_line("@builtin(position) sk_Position: vec4<f32>,");
        }

        self.indentation -= 1;
        self.write_line("};");

        // In WebGPU/WGSL, the vertex stage does not support a point-size output and the size
        // of a point primitive is always 1 pixel (see https://github.com/gpuweb/gpuweb/issues/332).
        //
        // There isn't anything we can do to emulate this correctly at this stage so we synthesize
        // a placeholder global variable that has no effect. Programs should not rely on
        // sk_PointSize when using the Dawn backend.
        if ProgramConfig::is_vertex(kind) && requires_point_size_builtin {
            self.write_line("/* unsupported */ var<private> sk_PointSize: f32;");
        }
    }

    /// `functionDependencyArgs(f)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4991-L5006 (chrome/m156)
    fn function_dependency_args(&self, f: FnId) -> String {
        let mut args = String::new();
        if let Some(deps) = self.requirements.dependencies.get(&f).copied()
            && !deps.is_empty()
        {
            let mut separator = "";
            if deps.contains(Deps::PIPELINE_INPUTS) {
                args.push_str("_stageIn");
                separator = ", ";
            }
            if deps.contains(Deps::PIPELINE_OUTPUTS) {
                args.push_str(separator);
                args.push_str("_stageOut");
            }
        }
        args
    }

    /// `writeFunctionDependencyParams(f)`: for a given function declaration, writes out any
    /// implicitly required pipeline stage arguments based on the function's pre-determined
    /// dependencies. These are expected to be written out as the first parameters for a function
    /// that requires them. Returns true if any arguments were written.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L5008-L5034 (chrome/m156)
    fn write_function_dependency_params(&mut self, f: FnId) -> bool {
        let Some(deps) = self.requirements.dependencies.get(&f).copied() else {
            return false;
        };
        if deps.is_empty() {
            return false;
        }

        let struct_name_prefix = pipeline_struct_prefix(self.program_kind());
        if struct_name_prefix.is_empty() {
            return false;
        }
        let mut separator = "";
        if deps.contains(Deps::PIPELINE_INPUTS) {
            self.write("_stageIn: ");
            separator = ", ";
            self.write(struct_name_prefix);
            self.write("In");
        }
        if deps.contains(Deps::PIPELINE_OUTPUTS) {
            self.write(separator);
            self.write("_stageOut: ptr<function, ");
            self.write(struct_name_prefix);
            self.write("Out>");
        }
        true
    }

    /// `is_in_global_uniforms(var)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1194-L1199 (chrome/m156)
    fn is_in_global_uniforms(&self, var: &crate::ir::Variable) -> bool {
        var.storage == VariableStorage::Global
            && var.modifier_flags.is_uniform()
            && !self.ctx.pool.ty(var.ty).is_opaque()
            && var.interface_block.is_none()
    }
}
