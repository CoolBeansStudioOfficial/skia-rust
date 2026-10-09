// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLPipelineStageCodeGenerator.{h,cpp} (chrome/m156).

//! The pipeline-stage code generator (`SkSL::PipelineStage::ConvertProgram`): turns a program into
//! the SkSL-like text that a fragment processor embeds. The text is handed back piece by piece
//! through [`Callbacks`], which declare uniforms, globals, structs and functions, and rewrite the
//! child samples and colour-space conversions.
//!
//! Skia's `PipelineStageCodeGenerator` writes into a `StringStream` that `AutoOutputBuffer` swaps
//! for a fresh one; [`Generator::capture`] does the same with a `String`.

// The ported functions keep the shape of the C++ ones: long switches over node kinds.
#![allow(clippy::too_many_lines)]

use std::collections::HashMap;

use crate::analysis::{
    ParameterMatchesFn, SpecializationIndex, SpecializationInfo, SpecializedFunctionKey,
    SpecializedParameters, UNSPECIALIZED, find_functions_to_specialize,
    find_specialization_index_for_call, find_specialized_parameters_for_function,
    get_parameter_mappings_for_function,
};
use crate::intrinsic_list::IntrinsicKind;
use crate::ir::{
    BinaryExpression, Block, ChildCall, ElemId, ExprId, Expression, ExpressionKind, FieldAccess,
    FieldAccessOwnerKind, FnId, ForStatement, FunctionCall, FunctionDefinition,
    GlobalVarDeclaration, IfStatement, IrPool, ModifierFlags, PostfixExpression, PrefixExpression,
    Program, ProgramElementKind, StatementKind, StmtId, StructDefinition, Swizzle,
    TernaryExpression, TypeId, TypeKind, VarDeclaration, VarId, Variable, VariableReference,
};
use crate::modules::ModuleType;
use crate::operator::OperatorPrecedence;
use crate::program_settings::ProgramConfig;
use crate::string::Separator;

/// `SkSL::PipelineStage::Callbacks`: how the embedding code receives the generated text, and how
/// it names things. The defaults are the C++ defaults (`main`, no mangling).
// Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.h#L23-L41 (chrome/m156)
#[doc(alias = "SkSL::PipelineStage::Callbacks")]
pub trait Callbacks {
    /// `getMainName`.
    fn get_main_name(&mut self) -> String {
        String::from("main")
    }

    /// `getMangledName`.
    fn get_mangled_name(&mut self, name: &str) -> String {
        name.to_owned()
    }

    /// `defineFunction`: `declaration` is the signature, `body` the statements inside the braces.
    fn define_function(&mut self, declaration: &str, body: &str, is_main: bool);

    /// `declareFunction`: a prototype.
    fn declare_function(&mut self, declaration: &str);

    /// `defineStruct`.
    fn define_struct(&mut self, definition: &str);

    /// `declareGlobal`.
    fn declare_global(&mut self, declaration: &str);

    /// `declareUniform`: emits the uniform (or not) and returns the name the code should use.
    fn declare_uniform(&mut self, pool: &IrPool, decl: &VarDeclaration) -> String;

    /// `sampleShader`: the text of a shader sample at `coords`.
    fn sample_shader(&mut self, index: i32, coords: &str) -> String;

    /// `sampleColorFilter`: the text of a colour-filter sample of `color`.
    fn sample_color_filter(&mut self, index: i32, color: &str) -> String;

    /// `sampleBlender`: the text of a blend of `src` and `dst`.
    fn sample_blender(&mut self, index: i32, src: &str, dst: &str) -> String;

    /// `toLinearSrgb`.
    fn to_linear_srgb(&mut self, color: &str) -> String;

    /// `fromLinearSrgb`. The name keeps Skia's callback name; it takes `&mut self` like the others.
    #[allow(clippy::wrong_self_convention)] // Skia's `fromLinearSrgb` callback, by name.
    fn from_linear_srgb(&mut self, color: &str) -> String;
}

/// `SkSL::PipelineStage::ConvertProgram`: generates `program` for use in an embedding fragment
/// processor. References to `main`'s coords parameter become `sample_coords`, to its input colour
/// `input_color`, and to its dest colour `dest_color`.
// Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L889-L899 (chrome/m156)
#[doc(alias = "SkSL::PipelineStage::ConvertProgram")]
pub fn convert_program(
    program: &Program,
    sample_coords: &str,
    input_color: &str,
    dest_color: &str,
    callbacks: &mut dyn Callbacks,
) {
    let mut generator = Generator {
        program,
        sample_coords,
        input_color,
        dest_color,
        callbacks,
        spec_info: SpecializationInfo::default(),
        active_spec_index: UNSPECIALIZED,
        active_specialization: None,
        variable_names: HashMap::new(),
        struct_names: HashMap::new(),
        function_names: HashMap::new(),
        buffer: String::new(),
        cast_returns_to_half: false,
        current_function: None,
    };
    generator.generate_code();
}

/// `PipelineStageCodeGenerator`.
// Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L68-L176 (chrome/m156)
struct Generator<'a, 'c> {
    program: &'a Program,
    sample_coords: &'a str,
    input_color: &'a str,
    dest_color: &'a str,
    callbacks: &'c mut dyn Callbacks,

    spec_info: SpecializationInfo,
    active_spec_index: SpecializationIndex,
    active_specialization: Option<SpecializedParameters>,

    variable_names: HashMap<VarId, String>,
    struct_names: HashMap<TypeId, String>,
    function_names: HashMap<SpecializedFunctionKey, String>,

    buffer: String,
    cast_returns_to_half: bool,
    current_function: Option<FnId>,
}

/// `Precedence`, as the C++ generator names it.
type Precedence = OperatorPrecedence;

impl<'a> Generator<'a, '_> {
    /// The program's pool. It outlives the generator, so it is not borrowed from `self`.
    fn pool(&self) -> &'a IrPool {
        &self.program.pool
    }

    fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    fn write_line(&mut self, s: &str) {
        self.buffer.push_str(s);
        self.buffer.push('\n');
    }

    /// `AutoOutputBuffer`: runs `f` with a fresh buffer and returns what it wrote.
    fn capture(&mut self, f: impl FnOnce(&mut Self)) -> String {
        let saved = std::mem::take(&mut self.buffer);
        f(self);
        std::mem::replace(&mut self.buffer, saved)
    }

    /// `typeName`: the name of a type in the generated code, with struct renames applied.
    fn type_name(&self, raw: TypeId) -> String {
        let ty = self.pool().ty(raw).resolve().scalar_type_for_literal();
        if ty.is_array() {
            // This is necessary so that name mangling on arrays-of-structs works properly.
            let mut array_name = self.type_name(ty.component_type().id());
            array_name.push('[');
            array_name.push_str(&ty.columns().to_string());
            array_name.push(']');
            return array_name;
        }
        match self.struct_names.get(&ty.id()) {
            Some(name) => name.clone(),
            None => ty.name().to_owned(),
        }
    }

    fn write_type(&mut self, ty: TypeId) {
        let name = self.type_name(ty);
        self.write(&name);
    }

    /// `functionName`: the name of `decl` in a specialization, mangled through the callbacks.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L370-L398 (chrome/m156)
    fn function_name(&mut self, decl_id: FnId, spec_index: SpecializationIndex) -> String {
        let pool = self.pool();
        let decl = pool.function(decl_id);
        if decl.is_main {
            return self.callbacks.get_main_name();
        }

        // Intrinsics and functions from sksl_shared do not use name mangling.
        if decl.is_intrinsic() || decl.module_type == ModuleType::SkslShared {
            return decl.name.to_string();
        }

        let key = SpecializedFunctionKey {
            declaration: decl_id,
            specialization_index: spec_index,
        };
        if let Some(name) = self.function_names.get(&key) {
            return name.clone();
        }

        let mut specialized_name = decl.name.to_string();

        // For specialized functions, tack on `_param1_param2` to the function name.
        get_parameter_mappings_for_function(
            pool,
            decl_id,
            &self.spec_info,
            spec_index,
            |_, _, expr| {
                specialized_name.push('_');
                specialized_name.push_str(&pool.expression(expr).description(pool));
            },
        );

        let mangled_name = self.callbacks.get_mangled_name(&specialized_name);
        self.function_names.insert(key, mangled_name.clone());
        mangled_name
    }

    /// `writeChildCall`: a sample of a child effect, through the callbacks.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L178-L250 (chrome/m156)
    fn write_child_call(&mut self, c: &ChildCall) {
        let pool = self.pool();
        let mut child = c.child;
        // A specialized child is a reference to the caller's child variable.
        if let Some(ExpressionKind::VariableReference(r)) = self
            .active_specialization
            .as_ref()
            .and_then(|active| active.get(&child))
            .map(|&specialized_child| &pool.expression(specialized_child).kind)
        {
            child = r.variable;
        }

        let arguments = &c.arguments;
        let mut index = 0;
        let mut found = false;
        for element in self.program.elements() {
            if let ProgramElementKind::GlobalVar(global) = &pool.element(element).kind {
                let decl = global_var_declaration(pool, global);
                if decl.var == child {
                    found = true;
                } else if pool.ty(pool.variable(decl.var).ty).is_effect_child() {
                    index += 1;
                }
            }
            if found {
                break;
            }
        }
        debug_assert!(found, "a child call names a child declaration");

        // Shaders require a coordinate argument. Color filters require a color argument.
        // Blenders require two color arguments.
        let sample_output = match pool.ty(pool.variable(c.child).ty).type_kind {
            TypeKind::Shader => {
                debug_assert_eq!(arguments.len(), 1);
                let coords =
                    self.capture(|g| g.write_expression(arguments[0], Precedence::Sequence));
                self.callbacks.sample_shader(index, &coords)
            }
            TypeKind::ColorFilter => {
                debug_assert_eq!(arguments.len(), 1);
                let color =
                    self.capture(|g| g.write_expression(arguments[0], Precedence::Sequence));
                self.callbacks.sample_color_filter(index, &color)
            }
            TypeKind::Blender => {
                debug_assert_eq!(arguments.len(), 2);
                let src = self.capture(|g| g.write_expression(arguments[0], Precedence::Sequence));
                let dst = self.capture(|g| g.write_expression(arguments[1], Precedence::Sequence));
                self.callbacks.sample_blender(index, &src, &dst)
            }
            _ => {
                // SkDEBUGFAILF("cannot sample from type ..."): nothing is written.
                debug_assert!(false, "cannot sample from this type");
                String::new()
            }
        };
        self.write(&sample_output);
    }

    /// `writeFunctionCall`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L252-L300 (chrome/m156)
    fn write_function_call(&mut self, c: &FunctionCall) {
        let pool = self.pool();
        let function = c.function;
        let decl = pool.function(function);

        if matches!(
            decl.intrinsic_kind,
            Some(IntrinsicKind::ToLinearSrgb | IntrinsicKind::FromLinearSrgb)
        ) {
            debug_assert_eq!(c.arguments.len(), 1);
            let color = self.capture(|g| g.write_expression(c.arguments[0], Precedence::Sequence));
            let text = if decl.intrinsic_kind == Some(IntrinsicKind::ToLinearSrgb) {
                self.callbacks.to_linear_srgb(&color)
            } else {
                self.callbacks.from_linear_srgb(&color)
            };
            self.write(&text);
            return;
        }

        // Look up the specialization data, if any, needed for this function call.
        let call_index =
            find_specialization_index_for_call(c, &self.spec_info, self.active_spec_index);
        let specialized_params =
            find_specialized_parameters_for_function(pool, function, &self.spec_info);

        let name = self.function_name(function, call_index);
        self.write(&name);
        self.write("(");
        let mut separator = Separator::new();
        for (arg_idx, &argument) in c.arguments.iter().enumerate() {
            // If this parameter is specialized, it is baked into the destination function and
            // should not be passed along as an argument.
            if specialized_params.get(arg_idx).copied().unwrap_or(false) {
                continue;
            }
            // This is a regular argument and must be passed normally.
            self.write(separator.next_str());
            self.write_expression(argument, Precedence::Sequence);
        }
        self.write(")");
    }

    /// `writeVariableReference`.
    fn write_variable_reference(&mut self, r: &VariableReference) {
        let pool = self.pool();
        let var = r.variable;

        if let Some(function) = self.current_function {
            let decl = pool.function(function);
            if decl.main_coords_parameter() == Some(var) {
                let text = self.sample_coords;
                self.write(text);
                return;
            }
            if decl.main_input_color_parameter() == Some(var) {
                let text = self.input_color;
                self.write(text);
                return;
            }
            if decl.main_dest_color_parameter() == Some(var) {
                let text = self.dest_color;
                self.write(text);
                return;
            }
        }

        let name = match self.variable_names.get(&var) {
            Some(name) => name.clone(),
            None => pool.variable(var).name.to_string(),
        };
        self.write(&name);
    }

    /// `writeIfStatement`.
    fn write_if_statement(&mut self, stmt: &IfStatement) {
        self.write("if (");
        self.write_expression(stmt.test, Precedence::EXPRESSION);
        self.write(") ");
        self.write_statement(stmt.if_true);
        if let Some(if_false) = stmt.if_false {
            self.write(" else ");
            self.write_statement(if_false);
        }
    }

    /// `writeReturnStatement`.
    fn write_return_statement(&mut self, expression: Option<ExprId>) {
        self.write("return");
        if let Some(expression) = expression {
            self.write(" ");
            if self.cast_returns_to_half {
                self.write("half4(");
            }
            self.write_expression(expression, Precedence::EXPRESSION);
            if self.cast_returns_to_half {
                self.write(")");
            }
        }
        self.write(";");
    }

    /// `writeSwitchStatement`, over the switch's value and its `SwitchCase` statements.
    fn write_switch_statement(&mut self, value: ExprId, cases: &[StmtId]) {
        let pool = self.pool();
        self.write("switch (");
        self.write_expression(value, Precedence::EXPRESSION);
        self.write_line(") {");
        for &stmt in cases {
            let StatementKind::SwitchCase(case) = &pool.statement(stmt).kind else {
                unreachable!("a switch's cases are SwitchCase statements");
            };
            if case.is_default {
                self.write_line("default:");
            } else {
                self.write("case ");
                self.write(&case.value.to_string());
                self.write_line(":");
            }
            if !pool.statement(case.statement).is_empty(pool) {
                self.write_statement(case.statement);
                self.write_line("");
            }
        }
        self.write_line("");
        self.write("}");
    }

    /// `functionDeclaration`: the signature of `decl`, with a mangled name and the modifiers of the
    /// function and its parameters. Skips the parameters that are specialized.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L440-L462 (chrome/m156)
    fn function_declaration(&mut self, decl_id: FnId) -> String {
        let pool = self.pool();
        let decl = pool.function(decl_id);
        let return_type = self.type_name(decl.return_type);
        let name = self.function_name(decl_id, self.active_spec_index);
        let mut declaration = format!(
            "{}{}{} {}(",
            if decl.modifier_flags.is_inline() {
                "inline "
            } else {
                ""
            },
            if decl.modifier_flags.is_no_inline() {
                "noinline "
            } else {
                ""
            },
            return_type,
            name
        );

        let mut separator = Separator::new();
        for &param in &decl.parameters {
            // Skip past parameters that we are specializing.
            let param_is_specialized = self
                .active_specialization
                .as_ref()
                .is_some_and(|active| active.contains_key(&param));
            if !param_is_specialized {
                let var = pool.variable(param);
                declaration.push_str(separator.next_str());
                declaration.push_str(&Self::modifier_string(var.modifier_flags));
                declaration.push_str(&self.typed_variable(var.ty, &var.name));
            }
        }
        declaration.push(')');
        declaration
    }

    /// `writeFunction`: emits the definition of a function, once per specialization.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L400-L438 (chrome/m156)
    fn write_function(&mut self, f: &FunctionDefinition) {
        let pool = self.pool();
        let decl = pool.function(f.declaration);
        // Don't re-emit functions from sksl_shared. (Functions from the `sksl_rt_shader` module
        // won't be visible once the shader is converted into a pipeline stage, so we do emit
        // those.)
        if decl.module_type == ModuleType::SkslShared {
            return;
        }

        debug_assert!(self.current_function.is_none());
        self.current_function = Some(f.declaration);

        // We allow public SkSL's main() to return half4 _or_ float4 (i.e. vec4). When we emit our
        // code in the processor, the surrounding code is going to expect half4, so we explicitly
        // cast any returns (from main) to half4. This is only strictly necessary if the return
        // type is float4, but we inject it unconditionally as a defensive measure, since it is
        // free and harmless.
        if decl.is_main && !ProgramConfig::is_mesh(self.program.config.kind) {
            self.cast_returns_to_half = true;
        }

        let body_children = block_children(pool, f.body);
        self.for_each_specialization(f.declaration, |g| {
            // Assemble the function body into a separate output stream.
            let body = g.capture(|g| {
                for &stmt in body_children {
                    g.write_statement(stmt);
                    g.write_line("");
                }
            });

            // Emit the function.
            let declaration = g.function_declaration(f.declaration);
            g.callbacks
                .define_function(&declaration, &body, decl.is_main);
        });

        if decl.is_main {
            self.cast_returns_to_half = false;
        }

        self.current_function = None;
    }

    /// `writeFunctionDeclaration`: prototypes a function, once per specialization.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L464-L471 (chrome/m156)
    fn write_function_declaration(&mut self, decl_id: FnId) {
        let decl = self.pool().function(decl_id);
        if !decl.is_main && decl.module_type != ModuleType::SkslShared {
            self.for_each_specialization(decl_id, |g| {
                let prototype = g.function_declaration(decl_id) + ";";
                g.callbacks.declare_function(&prototype);
            });
        }
    }

    /// `forEachSpecialization`: runs `f` once for each specialization of `decl`, with that
    /// specialization active, or once with none active when `decl` is not specialized.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L473-L498 (chrome/m156)
    fn for_each_specialization(&mut self, decl: FnId, mut f: impl FnMut(&mut Self)) {
        // Save off the current specialization.
        let prev_index = self.active_spec_index;
        let prev_specialization = self.active_specialization.clone();

        if let Some(specializations) = self.spec_info.specialization_map.get(&decl).cloned() {
            // Invoke the callback for each specialization.
            for (index, specialization) in specializations.into_iter().enumerate() {
                self.active_spec_index = i32::try_from(index).expect("few specializations");
                self.active_specialization = Some(specialization);
                f(self);
            }
        } else {
            // This function isn't specialized, so emit its declaration normally.
            self.active_spec_index = UNSPECIALIZED;
            self.active_specialization = None;
            f(self);
        }

        // Restore the previous specialization.
        self.active_spec_index = prev_index;
        self.active_specialization = prev_specialization;
    }

    /// `writeGlobalVarDeclaration`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L500-L523 (chrome/m156)
    fn write_global_var_declaration(&mut self, g: &GlobalVarDeclaration) {
        let pool = self.pool();
        let decl = global_var_declaration(pool, g);
        let var = pool.variable(decl.var);

        if var.builtin || pool.ty(var.ty).is_opaque() {
            // Don't re-declare these. (eg, sk_FragCoord, or fragmentProcessor children)
        } else if var.modifier_flags.is_uniform() {
            let uniform_name = self.callbacks.declare_uniform(pool, decl);
            self.variable_names.insert(decl.var, uniform_name);
        } else {
            let mangled_name = self.callbacks.get_mangled_name(&var.name);
            let mut declaration = Self::modifier_string(var.modifier_flags)
                + &self.typed_variable(var.ty, &mangled_name);
            if let Some(value) = decl.value {
                let text = self.capture(|g| g.write_expression(value, Precedence::EXPRESSION));
                declaration.push_str(" = ");
                declaration.push_str(&text);
            }
            declaration.push_str(";\n");
            self.callbacks.declare_global(&declaration);
            self.variable_names.insert(decl.var, mangled_name);
        }
    }

    /// `writeStructDefinition`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L525-L535 (chrome/m156)
    fn write_struct_definition(&mut self, s: &StructDefinition) {
        let ty = self.pool().ty(s.ty);
        let mangled_name = self.callbacks.get_mangled_name(ty.display_name());
        let mut definition = format!("struct {mangled_name} {{\n");
        for field in ty.fields() {
            definition.push_str(&self.typed_variable(field.ty, &field.name));
            definition.push_str(";\n");
        }
        definition.push_str("};\n");
        self.struct_names.insert(s.ty, mangled_name);
        self.callbacks.define_struct(&definition);
    }

    /// `writeProgramElementFirstPass`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L537-L560 (chrome/m156)
    fn write_program_element_first_pass(&mut self, element: ElemId) {
        let pool = self.pool();
        match &pool.element(element).kind {
            ProgramElementKind::GlobalVar(g) => self.write_global_var_declaration(g),
            ProgramElementKind::Function(f) => self.write_function_declaration(f.declaration),
            // Skip this; we're already emitting prototypes for every FunctionDefinition. (See the
            // function case in the second pass.)
            ProgramElementKind::FunctionPrototype(_) => {}
            ProgramElementKind::StructDefinition(s) => self.write_struct_definition(s),
            // SkDEBUGFAILF("unsupported program element"): nothing is written.
            ProgramElementKind::Extension(_)
            | ProgramElementKind::InterfaceBlock(_)
            | ProgramElementKind::Modifiers(_) => {
                debug_assert!(false, "unsupported program element");
            }
        }
    }

    /// `writeProgramElementSecondPass`.
    fn write_program_element_second_pass(&mut self, element: ElemId) {
        let pool = self.pool();
        if let ProgramElementKind::Function(f) = &pool.element(element).kind {
            self.write_function(f);
        }
    }

    /// `typedVariable`: a declaration of `name` with `ty`, handling arrays (`float x[2]`).
    fn typed_variable(&self, ty: TypeId, name: &str) -> String {
        let ty_ref = self.pool().ty(ty);
        let base_type = if ty_ref.is_array() {
            ty_ref.component_type().id()
        } else {
            ty
        };
        let mut decl = self.type_name(base_type) + " " + name;
        if ty_ref.is_array() {
            decl.push('[');
            decl.push_str(&ty_ref.columns().to_string());
            decl.push(']');
        }
        decl
    }

    /// `modifierString`: the `const`, `in` and `out` keywords of a variable.
    fn modifier_string(flags: ModifierFlags) -> String {
        let mut result = String::new();
        if flags.is_const() {
            result.push_str("const ");
        }
        if flags.contains(ModifierFlags::IN) && flags.contains(ModifierFlags::OUT) {
            result.push_str("inout ");
        } else if flags.contains(ModifierFlags::IN) {
            result.push_str("in ");
        } else if flags.contains(ModifierFlags::OUT) {
            result.push_str("out ");
        }
        result
    }

    /// `writeVarDeclaration`.
    fn write_var_declaration(&mut self, var: &VarDeclaration) {
        let pool = self.pool();
        let variable = pool.variable(var.var);
        let modifiers = Self::modifier_string(variable.modifier_flags);
        let declared = self.typed_variable(variable.ty, &variable.name);
        self.write(&modifiers);
        self.write(&declared);
        if let Some(value) = var.value {
            self.write(" = ");
            self.write_expression(value, Precedence::EXPRESSION);
        }
        self.write(";");
    }

    /// `writeStatement`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L771-L815 (chrome/m156)
    fn write_statement(&mut self, id: StmtId) {
        let pool = self.pool();
        match &pool.statement(id).kind {
            StatementKind::Block(b) => self.write_block(b),
            StatementKind::Break(_) => self.write("break;"),
            StatementKind::Continue(_) => self.write("continue;"),
            StatementKind::Expression(e) => {
                self.write_expression(e.expression, Precedence::Statement);
                self.write(";");
            }
            StatementKind::Do(d) => {
                self.write("do ");
                self.write_statement(d.statement);
                self.write(" while (");
                self.write_expression(d.test, Precedence::EXPRESSION);
                self.write(");");
            }
            StatementKind::For(f) => self.write_for_statement(f),
            StatementKind::If(stmt) => self.write_if_statement(stmt),
            StatementKind::Return(r) => self.write_return_statement(r.expression),
            StatementKind::Switch(s) => {
                let cases = s.cases(pool);
                self.write_switch_statement(s.value, cases);
            }
            StatementKind::VarDeclaration(v) => self.write_var_declaration(v),
            // SkDEBUGFAIL("Unsupported control flow"): nothing is written.
            StatementKind::Discard(_) => debug_assert!(false, "Unsupported control flow"),
            StatementKind::Nop(_) => self.write(";"),
            // A switch case is written by its switch, and nothing else reaches here.
            StatementKind::SwitchCase(_) => debug_assert!(false, "a switch case outside a switch"),
        }
    }

    /// `writeBlock`.
    fn write_block(&mut self, b: &Block) {
        let pool = self.pool();
        // Write scope markers if this block is a scope, or if the block is empty (since we need to
        // emit something here to make the code valid).
        let is_scope = b.is_scope() || b.is_empty(pool);
        if is_scope {
            self.write_line("{");
        }
        for &stmt in &b.children {
            if !pool.statement(stmt).is_empty(pool) {
                self.write_statement(stmt);
                self.write_line("");
            }
        }
        if is_scope {
            self.write("}");
        }
    }

    /// `writeForStatement`.
    fn write_for_statement(&mut self, f: &ForStatement) {
        let pool = self.pool();
        // Emit loops of the form 'for(;test;)' as 'while(test)', which is probably how they started.
        if f.initializer.is_none() && f.test.is_some() && f.next.is_none() {
            self.write("while (");
            if let Some(test) = f.test {
                self.write_expression(test, Precedence::EXPRESSION);
            }
            self.write(") ");
            self.write_statement(f.statement);
            return;
        }

        self.write("for (");
        match f.initializer {
            Some(init) if !pool.statement(init).is_empty(pool) => self.write_statement(init),
            _ => self.write("; "),
        }
        if let Some(test) = f.test {
            self.write_expression(test, Precedence::EXPRESSION);
        }
        self.write("; ");
        if let Some(next) = f.next {
            self.write_expression(next, Precedence::EXPRESSION);
        }
        self.write(") ");
        self.write_statement(f.statement);
    }

    /// `writeTernaryExpression`.
    fn write_ternary_expression(&mut self, t: &TernaryExpression, parent: Precedence) {
        if Precedence::Ternary >= parent {
            self.write("(");
        }
        self.write_expression(t.test, Precedence::Ternary);
        self.write(" ? ");
        self.write_expression(t.if_true, Precedence::Ternary);
        self.write(" : ");
        self.write_expression(t.if_false, Precedence::Ternary);
        if Precedence::Ternary >= parent {
            self.write(")");
        }
    }

    /// `writeBinaryExpression`.
    fn write_binary_expression(&mut self, b: &BinaryExpression, parent: Precedence) {
        let op = b.operator;
        let precedence = op.binary_precedence();
        if precedence >= parent {
            self.write("(");
        }
        self.write_expression(b.left, precedence);
        self.write(op.operator_name());
        self.write_expression(b.right, precedence);
        if precedence >= parent {
            self.write(")");
        }
    }

    /// `writePrefixExpression`.
    fn write_prefix_expression(&mut self, p: &PrefixExpression, parent: Precedence) {
        if Precedence::Prefix >= parent {
            self.write("(");
        }
        self.write(p.operator.tight_operator_name());
        self.write_expression(p.operand, Precedence::Prefix);
        if Precedence::Prefix >= parent {
            self.write(")");
        }
    }

    /// `writePostfixExpression`.
    fn write_postfix_expression(&mut self, p: &PostfixExpression, parent: Precedence) {
        if Precedence::Postfix >= parent {
            self.write("(");
        }
        self.write_expression(p.operand, Precedence::Postfix);
        self.write(p.operator.tight_operator_name());
        if Precedence::Postfix >= parent {
            self.write(")");
        }
    }

    /// `writeIndexExpression`.
    fn write_index_expression(&mut self, base: ExprId, index: ExprId) {
        self.write_expression(base, Precedence::Postfix);
        self.write("[");
        self.write_expression(index, Precedence::EXPRESSION);
        self.write("]");
    }

    /// `writeFieldAccess`.
    fn write_field_access(&mut self, f: &FieldAccess) {
        let pool = self.pool();
        if f.owner_kind == FieldAccessOwnerKind::Default {
            self.write_expression(f.base, Precedence::Postfix);
            self.write(".");
        }
        let base_type = pool.expression(f.base).ty;
        let name = &pool.ty(base_type).fields()[f.field_index].name;
        self.write(name);
    }

    /// `writeSwizzle`.
    fn write_swizzle(&mut self, swizzle: &Swizzle) {
        self.write_expression(swizzle.base, Precedence::Postfix);
        self.write(".");
        self.write(&Swizzle::mask_string(swizzle.components.as_slice()));
    }

    /// `writeAnyConstructor`: `type(args…)`.
    fn write_any_constructor(&mut self, expr: &Expression) {
        self.write_type(expr.ty);
        self.write("(");
        let mut separator = Separator::new();
        for &arg in expr.any_constructor_arguments().unwrap_or(&[]) {
            self.write(separator.next_str());
            self.write_expression(arg, Precedence::Sequence);
        }
        self.write(")");
    }

    /// `writeExpression`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L587-L642 (chrome/m156)
    fn write_expression(&mut self, id: ExprId, parent: Precedence) {
        let pool = self.pool();
        let expr = pool.expression(id);
        match &expr.kind {
            ExpressionKind::Binary(b) => self.write_binary_expression(b, parent),
            ExpressionKind::Literal(_) | ExpressionKind::Setting(_) => {
                let text = expr.description(pool);
                self.write(&text);
            }
            ExpressionKind::ChildCall(c) => self.write_child_call(c),
            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorArrayCast(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorCompoundCast(_)
            | ExpressionKind::ConstructorDiagonalMatrix(_)
            | ExpressionKind::ConstructorMatrixResize(_)
            | ExpressionKind::ConstructorScalarCast(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_) => self.write_any_constructor(expr),
            ExpressionKind::Empty(_) => self.write("false"),
            ExpressionKind::FieldAccess(f) => self.write_field_access(f),
            ExpressionKind::FunctionCall(c) => self.write_function_call(c),
            ExpressionKind::Prefix(p) => self.write_prefix_expression(p, parent),
            ExpressionKind::Postfix(p) => self.write_postfix_expression(p, parent),
            ExpressionKind::Swizzle(s) => self.write_swizzle(s),
            ExpressionKind::VariableReference(r) => self.write_variable_reference(r),
            ExpressionKind::Ternary(t) => self.write_ternary_expression(t, parent),
            ExpressionKind::Index(i) => self.write_index_expression(i.base, i.index),
            // SkDEBUGFAILF("unsupported expression"): nothing is written.
            _ => debug_assert!(false, "unsupported expression"),
        }
    }

    /// `generateCode`.
    // Port of: src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp#L870-L887 (chrome/m156)
    fn generate_code(&mut self) {
        let pool = self.pool();
        let elements: Vec<ElemId> = self.program.elements().collect();

        // Search for functions which require specialization due to passing child effects as
        // parameters.
        let is_child_parameter = |param: &Variable| pool.ty(param.ty).is_effect_child();
        let matches: ParameterMatchesFn<'_> = &is_child_parameter;
        find_functions_to_specialize(pool, &elements, &mut self.spec_info, matches);

        // Write all the program elements except for functions; prototype all the functions.
        for &element in &elements {
            self.write_program_element_first_pass(element);
        }

        // We always place FunctionDefinition elements last, because the inliner likes to move
        // function bodies around. After inlining, code can inadvertently move upwards, above
        // ProgramElements that the code relies on.
        for &element in &elements {
            self.write_program_element_second_pass(element);
        }
    }
}

/// The `VarDeclaration` that a global declaration statement wraps.
fn global_var_declaration<'p>(pool: &'p IrPool, g: &GlobalVarDeclaration) -> &'p VarDeclaration {
    match &pool.statement(g.declaration).kind {
        StatementKind::VarDeclaration(decl) => decl,
        _ => unreachable!("a global declaration wraps a VarDeclaration"),
    }
}

/// The children of a block statement (a function body).
fn block_children(pool: &IrPool, stmt: StmtId) -> &[StmtId] {
    match &pool.statement(stmt).kind {
        StatementKind::Block(Block { children, .. }) => children,
        _ => unreachable!("a function body is a block"),
    }
}
