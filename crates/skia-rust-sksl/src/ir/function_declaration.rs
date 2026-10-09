// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionDeclaration.{h,cpp}: the data, accessors, the
// constructor's `main` parameter rules, `Convert`, `mangledName`, `matches`, `determineFinalTypes`
// and the signature checks that `Convert` runs.

//! [`FunctionDeclaration`]: a function symbol, one link of an overload chain.

use super::{
    ExprId, IrPool, LayoutFlags, ModifierFlags, Modifiers, SymbolId, add_symbol,
    ids::{ElemId, FnId, TypeId, VarId},
};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::intrinsic_list::{IntrinsicKind, find_intrinsic_kind};
use crate::modules::ModuleType;
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind};
use crate::string::Separator;

/// `SkSL::FunctionDeclaration`.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.h#L36-L160 (chrome/m156)
#[doc(alias = "SkSL::FunctionDeclaration")]
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors Skia's fields one for one.
pub struct FunctionDeclaration {
    /// `fPosition`.
    pub position: Position,
    /// The symbol name.
    pub name: Box<str>,
    /// `fDefinition`: the `FunctionDefinition` element, once the body is known.
    pub definition: Option<ElemId>,
    /// `fNextOverload`: the next function with the same name.
    pub next_overload: Option<FnId>,
    /// `fParameters`.
    pub parameters: Vec<VarId>,
    /// `fReturnType`.
    pub return_type: TypeId,
    /// `fModifierFlags`.
    pub modifier_flags: ModifierFlags,
    /// `fIntrinsicKind` (`None` is `kNotIntrinsic`).
    pub intrinsic_kind: Option<IntrinsicKind>,
    /// `fModuleType`: the module that declared the function, or `Program`.
    pub module_type: ModuleType,
    /// `fIsMain`.
    pub is_main: bool,
    /// `fHasMainCoordsParameter`.
    pub has_main_coords_parameter: bool,
    /// `fHasMainInputColorParameter`.
    pub has_main_input_color_parameter: bool,
    /// `fHasMainDestColorParameter`.
    pub has_main_dest_color_parameter: bool,
}

impl FunctionDeclaration {
    /// The `FunctionDeclaration` constructor. Records whether the function is `main` and which of
    /// its parameters are a runtime effect's coordinate or colour inputs.
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L230-L272 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's constructor parameter list, in order.
    pub fn new(
        pool: &IrPool,
        config: &ProgramConfig,
        position: Position,
        modifier_flags: ModifierFlags,
        name: &str,
        parameters: Vec<VarId>,
        return_type: TypeId,
        intrinsic_kind: Option<IntrinsicKind>,
    ) -> Self {
        let is_main = name == "main";
        let mut decl = Self {
            position,
            name: name.into(),
            definition: None,
            next_overload: None,
            parameters,
            return_type,
            modifier_flags,
            intrinsic_kind,
            module_type: config.module_type,
            is_main,
            has_main_coords_parameter: false,
            has_main_input_color_parameter: false,
            has_main_dest_color_parameter: false,
        };
        let mut builtin_color_index = 0;
        for &param in &decl.parameters {
            // Keep track of arguments to main for runtime effects.
            if !is_main {
                continue;
            }
            let param_ty = pool.variable(param).ty;
            if ProgramConfig::is_runtime_shader(config.kind)
                || ProgramConfig::is_fragment(config.kind)
            {
                // If this is a runtime shader, a float2 param is supposed to be the coords. For
                // testing purposes, we have .sksl inputs that are treated as both runtime effects
                // and fragment shaders. To make that work, fragment shaders are allowed to have a
                // coords parameter as well.
                if type_is_valid_for_coords(pool, param_ty) {
                    decl.has_main_coords_parameter = true;
                }
            } else if ProgramConfig::is_runtime_color_filter(config.kind)
                || ProgramConfig::is_runtime_blender(config.kind)
            {
                // If this is a runtime color filter or blender, the params are an input color,
                // followed by a destination color for blenders.
                if type_is_valid_for_color(pool, param_ty) {
                    match builtin_color_index {
                        0 => decl.has_main_input_color_parameter = true,
                        1 => decl.has_main_dest_color_parameter = true,
                        _ => {} // unknown color parameter
                    }
                    builtin_color_index += 1;
                }
            }
        }
        decl
    }

    /// `setDefinition`: records the body and clears the intrinsic kind (a defined function is
    /// not an intrinsic).
    pub fn set_definition(&mut self, definition: ElemId) {
        self.definition = Some(definition);
        self.intrinsic_kind = None;
    }

    /// `isBuiltin`: declared by a module.
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.module_type != ModuleType::Program
    }

    /// `isIntrinsic`.
    #[must_use]
    pub fn is_intrinsic(&self) -> bool {
        self.intrinsic_kind.is_some()
    }

    /// `getMainCoordsParameter`.
    #[must_use]
    pub fn main_coords_parameter(&self) -> Option<VarId> {
        self.has_main_coords_parameter.then(|| self.parameters[0])
    }

    /// `getMainInputColorParameter`.
    #[must_use]
    pub fn main_input_color_parameter(&self) -> Option<VarId> {
        self.has_main_input_color_parameter
            .then(|| self.parameters[0])
    }

    /// `getMainDestColorParameter`.
    #[must_use]
    pub fn main_dest_color_parameter(&self) -> Option<VarId> {
        self.has_main_dest_color_parameter
            .then(|| self.parameters[1])
    }

    /// `FunctionDeclaration::Convert`: checks a function's signature against the program kind and
    /// the functions already declared, and adds it to the current symbol table. Returns the
    /// existing declaration when the signature repeats one (a prototype before its definition).
    /// Returns `None` after reporting an error.
    ///
    /// # Panics
    ///
    /// If no current symbol table is set (Skia dereferences `context.fSymbolTable`).
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L557-L602 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        modifiers: &Modifiers,
        name: &str,
        parameters: &[VarId],
        return_type_pos: Position,
        return_type: TypeId,
    ) -> Option<FnId> {
        // No layout flag is permissible on a function.
        modifiers
            .layout
            .check_permitted_layout(ctx, pos, LayoutFlags::empty());

        // If requested, apply the `noinline` modifier to every function. This allows us to test
        // Runtime Effects without any inlining, even when the code is later added to a paint.
        let mut modifier_flags = modifiers.flags;
        let config = *ctx.config();
        if config.settings.force_no_inline {
            modifier_flags.remove(ModifierFlags::INLINE);
            modifier_flags.insert(ModifierFlags::NO_INLINE);
        }

        let is_main = name == "main";
        let intrinsic_kind = if config.is_builtin_code() {
            find_intrinsic_kind(name)
        } else {
            None
        };
        if !check_modifiers(ctx, modifiers.position, modifier_flags)
            || !check_return_type(ctx, return_type_pos, return_type)
            || !check_parameters(ctx, parameters, modifier_flags, intrinsic_kind)
            || (is_main
                && !check_main_signature(
                    &mut ctx.errors,
                    &ctx.pool,
                    config.kind,
                    pos,
                    return_type,
                    parameters,
                ))
        {
            return None;
        }
        let Ok(existing) = find_existing_declaration(
            ctx,
            &config,
            pos,
            modifier_flags,
            intrinsic_kind,
            name,
            parameters,
            return_type_pos,
            return_type,
        ) else {
            return None;
        };
        if existing.is_some() {
            return existing;
        }

        let table = ctx
            .symbol_table
            .expect("FunctionDeclaration::Convert: no current symbol table");
        let decl = Self::new(
            &ctx.pool,
            &config,
            pos,
            modifier_flags,
            name,
            parameters.to_vec(),
            return_type,
            intrinsic_kind,
        );
        let id = ctx.pool.add_function(decl);
        add_symbol(ctx, table, SymbolId::FunctionDeclaration(id));
        Some(id)
    }

    /// `mangledName`: the name a generator emits for this function. Builtins without a definition
    /// and `main` keep their names; the rest encode their return and parameter types.
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L604-L622 (chrome/m156)
    #[must_use]
    pub fn mangled_name(&self, pool: &IrPool) -> String {
        if (self.is_builtin() && self.definition.is_none()) || self.is_main {
            // Builtins without a definition (like `sin` or `sqrt`) must use their real names.
            return self.name.to_string();
        }
        // Built-in functions can have a $ prefix, which will fail to compile in GLSL. Remove the
        // $ and add a unique mangling specifier, so user code can't conflict with the name.
        let (name, builtin_marker) = match self.name.strip_prefix('$') {
            Some(rest) => (rest, "Q"), // a unique, otherwise-unused mangle character
            None => (&*self.name, ""),
        };
        // Rename function to `funcname_returntypeparamtypes`.
        let mut result = format!(
            "{name}_{builtin_marker}{}",
            pool.ty(self.return_type).abbreviated_name
        );
        for &p in &self.parameters {
            result.push_str(pool.ty(pool.variable(p).ty).abbreviated_name);
        }
        result
    }

    /// `matches(f)`: the same name and parameter types (the return type and modifiers are
    /// ignored).
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L624-L637 (chrome/m156)
    #[must_use]
    pub fn matches(&self, pool: &IrPool, other: &FunctionDeclaration) -> bool {
        if self.name != other.name || self.parameters.len() != other.parameters.len() {
            return false;
        }
        self.parameters
            .iter()
            .zip(&other.parameters)
            .all(|(&p, &o)| pool.ty(pool.variable(p).ty).matches(pool.variable(o).ty))
    }

    /// `determineFinalTypes(arguments)`: the parameter types and return type once the generic
    /// (`$genType`) parameters are fixed by the arguments. Returns `None` when no generic type
    /// accepts the arguments (Skia's `false`).
    ///
    /// # Panics
    ///
    /// If `arguments` does not have one entry per parameter (Skia asserts this).
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L639-L676 (chrome/m156)
    #[must_use]
    pub fn determine_final_types(
        &self,
        pool: &IrPool,
        arguments: &[ExprId],
    ) -> Option<(Vec<TypeId>, TypeId)> {
        debug_assert_eq!(arguments.len(), self.parameters.len());
        let mut parameter_types = Vec::with_capacity(arguments.len());
        let mut generic_index: Option<usize> = None;
        for (&arg, &param) in arguments.iter().zip(&self.parameters) {
            // Non-generic parameters are final as-is.
            let parameter_type = pool.variable(param).ty;
            if !pool.ty(parameter_type).is_generic() {
                parameter_types.push(parameter_type);
                continue;
            }
            // We use the first generic parameter we find to lock in the generic index; e.g. if we
            // find `float3` here, all `$genType`s will be assumed to be `float3`.
            if generic_index.is_none() {
                // The passed-in type wasn't a match for ANY of the generic possibilities. This
                // function isn't a match at all.
                generic_index = Some(find_generic_index(
                    pool,
                    pool.expression(arg).ty,
                    parameter_type,
                    /*allow_narrowing=*/ true,
                )?);
            }
            let index = generic_index.expect("the generic index was set above");
            parameter_types.push(pool.ty(parameter_type).coercible_types()[index]);
        }
        // Apply the generic index to our return type.
        let return_type = if pool.ty(self.return_type).is_generic() {
            // We don't support functions with a generic return type and no other generics.
            let index = generic_index?;
            pool.ty(self.return_type).coercible_types()[index]
        } else {
            self.return_type
        };
        Some((parameter_types, return_type))
    }

    /// `description()`: `modifiers returnType name(param, …)`.
    // Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L544-L555 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = if self.modifier_flags.is_empty() {
            String::new()
        } else {
            self.modifier_flags.description() + " "
        };
        result.push_str(pool.ty(self.return_type).display_name());
        result.push(' ');
        result.push_str(&self.name);
        result.push('(');
        let mut separator = Separator::new();
        for &p in &self.parameters {
            result.push_str(separator.next_str());
            result.push_str(&pool.variable(p).description(pool));
        }
        result.push(')');
        result
    }
}

/// `check_modifiers`: the modifiers a function may carry, and that `inline` and `noinline` are
/// not both set.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L31-L43 (chrome/m156)
fn check_modifiers(ctx: &mut Context, pos: Position, modifier_flags: ModifierFlags) -> bool {
    let mut permitted = ModifierFlags::INLINE | ModifierFlags::NO_INLINE;
    if ctx.config().is_builtin_code() {
        permitted |= ModifierFlags::ES3 | ModifierFlags::PURE | ModifierFlags::EXPORT;
    }
    modifier_flags.check_permitted_flags(ctx, pos, permitted);
    if modifier_flags.is_inline() && modifier_flags.is_no_inline() {
        ctx.errors
            .error(pos, "functions cannot be both 'inline' and 'noinline'");
        return false;
    }
    true
}

/// `check_return_type`: arrays, opaque types and (in strict ES2) arrays inside structs cannot be
/// returned.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L45-L67 (chrome/m156)
fn check_return_type(ctx: &mut Context, pos: Position, return_type: TypeId) -> bool {
    let (is_array, contains_array, opaque, display) = {
        let ty = ctx.pool.ty(return_type);
        (
            ty.is_array(),
            ty.is_or_contains_array(),
            ty.component_type().is_opaque(),
            ty.display_name().to_owned(),
        )
    };
    if is_array {
        ctx.errors
            .error(pos, &format!("functions may not return type '{display}'"));
        return false;
    }
    if ctx.config().strict_es2_mode() && contains_array {
        ctx.errors
            .error(pos, "functions may not return structs containing arrays");
        return false;
    }
    if !ctx.config().is_builtin_code() && opaque {
        ctx.errors.error(
            pos,
            &format!("functions may not return opaque type '{display}'"),
        );
        return false;
    }
    true
}

/// `check_parameters`: the modifiers, layout and effect-child rules of each parameter.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L69-L118 (chrome/m156)
fn check_parameters(
    ctx: &mut Context,
    parameters: &[VarId],
    modifier_flags: ModifierFlags,
    intrinsic_kind: Option<IntrinsicKind>,
) -> bool {
    // Check modifiers on each function parameter.
    for &param in parameters {
        let (ty, param_flags, layout, position, modifiers_position) = {
            let p = ctx.pool.variable(param);
            (
                p.ty,
                p.modifier_flags,
                p.layout,
                p.position,
                p.modifiers_position,
            )
        };
        let (is_opaque, is_storage, is_read_only_texture, is_effect_child, display) = {
            let t = ctx.pool.ty(ty);
            (
                t.is_opaque(),
                t.is_storage_texture(),
                t.is_read_only_texture(),
                t.is_effect_child(),
                t.display_name().to_owned(),
            )
        };
        let mut permitted_flags = ModifierFlags::CONST | ModifierFlags::IN;
        let mut permitted_layout_flags = LayoutFlags::empty();
        if !is_opaque {
            permitted_flags |= ModifierFlags::OUT;
        }
        if is_storage {
            // We allow `readonly`, `writeonly` and `layout(pixel-format)` on storage textures.
            permitted_flags |= ModifierFlags::READ_ONLY | ModifierFlags::WRITE_ONLY;
            permitted_layout_flags |= LayoutFlags::ALL_PIXEL_FORMATS;

            // Intrinsics are allowed to accept any pixel format, but user code must explicitly
            // specify a pixel format like `layout(rgba32f)`.
            if intrinsic_kind.is_none() && !layout.flags.intersects(LayoutFlags::ALL_PIXEL_FORMATS)
            {
                ctx.errors.error(
                    position,
                    "storage texture parameters must specify a pixel format layout-qualifier",
                );
                return false;
            }
        } else if is_read_only_texture {
            // Readonly textures can be either storage textures (which specify a pixel format) or
            // sampled textures (which do not).
            permitted_flags |= ModifierFlags::READ_ONLY;
            permitted_layout_flags |= LayoutFlags::ALL_PIXEL_FORMATS;
        }
        param_flags.check_permitted_flags(ctx, modifiers_position, permitted_flags);
        layout.check_permitted_layout(ctx, modifiers_position, permitted_layout_flags);

        // Public Runtime Effects aren't allowed to pass shader/colorFilter/blender types to
        // function calls. You can pass other opaque types to functions safely; this restriction is
        // specific to "child" objects.
        if !ProgramConfig::allows_private_identifiers(ctx.config().kind) && is_effect_child {
            ctx.errors.error(
                position,
                &format!("parameters of type '{display}' not allowed"),
            );
            return false;
        }

        // Pure functions should not change any state, and should be safe to eliminate if their
        // result is not used; this is incompatible with out-parameters, so we forbid it here.
        // (We don't exhaustively guard against pure functions changing global state in other ways,
        // though, since they aren't allowed in user code.)
        if modifier_flags.is_pure() && param_flags.intersects(ModifierFlags::OUT) {
            ctx.errors.error(
                modifiers_position,
                "pure functions cannot have out parameters",
            );
            return false;
        }
    }
    true
}

/// `type_is_valid_for_color`: a four-component float vector.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L120-L122 (chrome/m156)
fn type_is_valid_for_color(pool: &IrPool, ty: TypeId) -> bool {
    let t = pool.ty(ty);
    t.is_vector() && t.columns() == 4 && t.component_type().is_float()
}

/// `type_is_valid_for_coords`: a two-component high-precision float vector.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L124-L127 (chrome/m156)
fn type_is_valid_for_coords(pool: &IrPool, ty: TypeId) -> bool {
    let t = pool.ty(ty);
    t.is_vector() && t.high_precision() && t.columns() == 2 && t.component_type().is_float()
}

/// `check_main_signature`: the parameters and return type `main` must have for each program kind.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L139-L270 (chrome/m156)
#[allow(clippy::too_many_lines)] // One switch over the program kinds, as in Skia.
fn check_main_signature(
    errors: &mut ErrorReporter,
    pool: &IrPool,
    kind: ProgramKind,
    pos: Position,
    return_type: TypeId,
    parameters: &[VarId],
) -> bool {
    let is_attributes = |ty: TypeId| {
        let t = pool.ty(ty);
        t.is_struct() && t.name() == "Attributes"
    };
    let is_varyings = |ty: TypeId| {
        let t = pool.ty(ty);
        t.is_struct() && t.name() == "Varyings"
    };
    let param = |i: usize| pool.variable(parameters[i]);
    let param_is_coords = |i: usize| {
        type_is_valid_for_coords(pool, param(i).ty) && param(i).modifier_flags.is_empty()
    };
    let param_is_color =
        |i: usize| type_is_valid_for_color(pool, param(i).ty) && param(i).modifier_flags.is_empty();
    let param_is_const_in_attributes =
        |i: usize| is_attributes(param(i).ty) && param(i).modifier_flags == ModifierFlags::CONST;
    let param_is_const_in_varyings =
        |i: usize| is_varyings(param(i).ty) && param(i).modifier_flags == ModifierFlags::CONST;
    let param_is_out_color = |i: usize| {
        type_is_valid_for_color(pool, param(i).ty) && param(i).modifier_flags == ModifierFlags::OUT
    };
    let n = parameters.len();
    let color_return_error = "'main' must return: 'vec4', 'float4', or 'half4'";

    match kind {
        ProgramKind::RuntimeColorFilter | ProgramKind::PrivateRuntimeColorFilter => {
            // (half4|float4) main(half4|float4)
            if !type_is_valid_for_color(pool, return_type) {
                errors.error(pos, color_return_error);
                return false;
            }
            if !(n == 1 && param_is_color(0)) {
                errors.error(pos, "'main' parameter must be 'vec4', 'float4', or 'half4'");
                return false;
            }
        }
        ProgramKind::RuntimeShader | ProgramKind::PrivateRuntimeShader => {
            // (half4|float4) main(float2)
            if !type_is_valid_for_color(pool, return_type) {
                errors.error(pos, color_return_error);
                return false;
            }
            if !(n == 1 && param_is_coords(0)) {
                errors.error(pos, "'main' parameter must be 'float2' or 'vec2'");
                return false;
            }
        }
        ProgramKind::RuntimeBlender | ProgramKind::PrivateRuntimeBlender => {
            // (half4|float4) main(half4|float4, half4|float4)
            if !type_is_valid_for_color(pool, return_type) {
                errors.error(pos, color_return_error);
                return false;
            }
            if !(n == 2 && param_is_color(0) && param_is_color(1)) {
                errors.error(
                    pos,
                    "'main' parameters must be (vec4|float4|half4, vec4|float4|half4)",
                );
                return false;
            }
        }
        ProgramKind::MeshVertex => {
            // Varyings main(const Attributes)
            if !is_varyings(return_type) {
                errors.error(pos, "'main' must return 'Varyings'.");
                return false;
            }
            if !(n == 1 && param_is_const_in_attributes(0)) {
                errors.error(pos, "'main' parameter must be 'const Attributes'.");
                return false;
            }
        }
        ProgramKind::MeshFragment => {
            // float2 main(const Varyings) -or- float2 main(const Varyings, out half4|float4)
            if !type_is_valid_for_coords(pool, return_type) {
                errors.error(pos, "'main' must return: 'vec2' or 'float2'");
                return false;
            }
            if !((n == 1 && param_is_const_in_varyings(0))
                || (n == 2 && param_is_const_in_varyings(0) && param_is_out_color(1)))
            {
                errors.error(
                    pos,
                    "'main' parameters must be (const Varyings, (out (half4|float4))?)",
                );
                return false;
            }
        }
        ProgramKind::Fragment | ProgramKind::GraphiteFragment => {
            let valid_params = n == 0 || (n == 1 && param_is_coords(0));
            if !valid_params {
                errors.error(pos, "shader 'main' must be main() or main(float2)");
                return false;
            }
        }
        ProgramKind::Vertex | ProgramKind::GraphiteVertex | ProgramKind::Compute => {
            if !pool.ty(return_type).matches(TypeId::VOID) {
                errors.error(pos, "'main' must return 'void'");
                return false;
            }
            if n != 0 {
                errors.error(pos, "shader 'main' must have zero parameters");
                return false;
            }
        }
    }
    true
}

/// `find_generic_index`: given a concrete type and a generic type (`$genType`), the index of the
/// concrete type within the generic's list of coercible types, or `None`.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L144-L157 (chrome/m156)
fn find_generic_index(
    pool: &IrPool,
    concrete: TypeId,
    generic: TypeId,
    allow_narrowing: bool,
) -> Option<usize> {
    pool.ty(generic)
        .coercible_types()
        .iter()
        .position(|&candidate| pool.ty(concrete).can_coerce_to(candidate, allow_narrowing))
}

/// `type_generically_matches`: the types match, or `concrete` is one of `maybe_generic`'s
/// candidates.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L159-L166 (chrome/m156)
fn type_generically_matches(pool: &IrPool, concrete: TypeId, maybe_generic: TypeId) -> bool {
    if pool.ty(maybe_generic).is_generic() {
        find_generic_index(pool, concrete, maybe_generic, false).is_some()
    } else {
        pool.ty(concrete).matches(maybe_generic)
    }
}

/// `parameters_match`: a parameter list matches a previously declared one, where the declared
/// one may use generic types that must resolve to one consistent index.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L172-L213 (chrome/m156)
fn parameters_match(pool: &IrPool, params: &[VarId], other_params: &[VarId]) -> bool {
    // If the param lists are different lengths, they're definitely not a match.
    if params.len() != other_params.len() {
        return false;
    }
    // Figure out a consistent generic index (or bail if we find a contradiction).
    let mut generic_index: Option<usize> = None;
    for (&p, &o) in params.iter().zip(other_params) {
        let param_type = pool.variable(p).ty;
        let other_type = pool.variable(o).ty;
        if pool.ty(other_type).is_generic() {
            // The type wasn't a match for this generic at all; these params can't be a match.
            let Some(index_for_this_param) =
                find_generic_index(pool, param_type, other_type, false)
            else {
                return false;
            };
            // The generic index mismatches from what we determined on a previous parameter.
            if generic_index.is_some_and(|g| g != index_for_this_param) {
                return false;
            }
            generic_index = Some(index_for_this_param);
        }
    }

    // Now that we've determined a generic index (if we needed one), do a parameter check.
    for (&p, &o) in params.iter().zip(other_params) {
        let param_type = pool.variable(p).ty;
        let mut other_type = pool.variable(o).ty;
        // Make generic types concrete.
        if pool.ty(other_type).is_generic() {
            let index = generic_index.expect("a generic parameter set the generic index");
            other_type = pool.ty(other_type).coercible_types()[index];
        }
        // Detect type mismatches.
        if !pool.ty(param_type).matches(other_type) {
            return false;
        }
    }
    true
}

/// `find_existing_declaration`: looks for an earlier declaration with the same parameter types,
/// reporting errors for an incompatible one. `Ok(Some(f))` is the declaration to reuse,
/// `Ok(None)` is no earlier declaration, and `Err(())` is an error that was reported.
// Port of: src/sksl/ir/SkSLFunctionDeclaration.cpp#L215-L296 (chrome/m156)
#[allow(clippy::too_many_arguments)] // Mirrors Skia's helper parameter list, in order.
fn find_existing_declaration(
    ctx: &mut Context,
    config: &ProgramConfig,
    pos: Position,
    modifier_flags: ModifierFlags,
    intrinsic_kind: Option<IntrinsicKind>,
    name: &str,
    parameters: &[VarId],
    return_type_pos: Position,
    return_type: TypeId,
) -> Result<Option<FnId>, ()> {
    let table = ctx
        .symbol_table
        .expect("FunctionDeclaration::Convert: no current symbol table");
    // Skia's `invalidDeclDescription` builds this declaration for its messages. It is a temporary
    // that is never added to the pool.
    let prototype = FunctionDeclaration::new(
        &ctx.pool,
        config,
        pos,
        modifier_flags,
        name,
        parameters.to_vec(),
        return_type,
        intrinsic_kind,
    );
    let Some(entry) = ctx.pool.find_symbol(table, name) else {
        return Ok(None);
    };
    let SymbolId::FunctionDeclaration(entry) = entry else {
        ctx.errors
            .error(pos, &format!("symbol '{name}' was already defined"));
        return Err(());
    };
    let mut existing = None;
    let mut other_id = Some(entry);
    while let Some(other) = other_id {
        other_id = ctx.pool.function(other).next_overload;
        if !parameters_match(&ctx.pool, parameters, &ctx.pool.function(other).parameters) {
            continue;
        }
        let other_decl = ctx.pool.function(other);
        let other_return_type = other_decl.return_type;
        let other_params = other_decl.parameters.clone();
        let other_flags = other_decl.modifier_flags;
        let other_is_intrinsic = other_decl.is_intrinsic();
        let other_description = other_decl.description(&ctx.pool);
        if !type_generically_matches(&ctx.pool, return_type, other_return_type) {
            let invalid = prototype.description(&ctx.pool);
            ctx.errors.error(
                return_type_pos,
                &format!(
                    "functions '{invalid}' and '{other_description}' differ only in return type"
                ),
            );
            return Err(());
        }
        for (i, (&p, &o)) in parameters.iter().zip(&other_params).enumerate() {
            let (p_flags, p_layout, p_pos) = {
                let v = ctx.pool.variable(p);
                (v.modifier_flags, v.layout, v.position)
            };
            let (o_flags, o_layout) = {
                let v = ctx.pool.variable(o);
                (v.modifier_flags, v.layout)
            };
            if p_flags != o_flags || p_layout != o_layout {
                ctx.errors.error(
                    p_pos,
                    &format!(
                        "modifiers on parameter {} differ between declaration and definition",
                        i + 1
                    ),
                );
                return Err(());
            }
        }
        if other_is_intrinsic {
            ctx.errors.error(
                pos,
                &format!("duplicate definition of intrinsic function '{name}'"),
            );
            return Err(());
        }
        if modifier_flags != other_flags {
            let invalid = prototype.description(&ctx.pool);
            ctx.errors.error(
                pos,
                &format!(
                    "functions '{invalid}' and '{other_description}' differ only in modifiers"
                ),
            );
            return Err(());
        }
        existing = Some(other);
        break;
    }
    if existing.is_none() && ctx.pool.function(entry).is_main {
        ctx.errors.error(pos, "duplicate definition of 'main'");
        return Err(());
    }
    Ok(existing)
}

#[cfg(test)]
mod tests {
    use super::{FunctionDeclaration, IntrinsicKind};
    use crate::context::Context;
    use crate::error_reporter::{ErrorReporter, ErrorSink};
    use crate::ir::{
        Layout, ModifierFlags, Modifiers, SymbolId, SymbolTable, TypeId, VarId, Variable,
        VariableStorage,
    };
    use crate::modules::ModuleType;
    use crate::position::Position;
    use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

    fn context(kind: ProgramKind, module_type: ModuleType) -> Context {
        let mut ctx = Context::new(ErrorReporter::forwarding());
        ctx.config = Some(ProgramConfig::new(
            module_type,
            kind,
            ProgramSettings::default(),
        ));
        ctx.symbol_table = Some(ctx.pool.add_symbol_table(SymbolTable::new(None, false)));
        ctx
    }

    fn errors(ctx: &Context) -> Vec<String> {
        match ctx.errors.sink() {
            ErrorSink::Forwarding { errors } => errors
                .iter()
                .map(|(msg, _)| String::from_utf8(msg.clone()).expect("a UTF-8 message"))
                .collect(),
            other => panic!("expected a forwarding reporter, found {other:?}"),
        }
    }

    fn param(ctx: &mut Context, name: &str, ty: TypeId) -> VarId {
        ctx.pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            ModifierFlags::empty(),
            name,
            ty,
            false,
            VariableStorage::Parameter,
        ))
    }

    fn convert(
        ctx: &mut Context,
        name: &str,
        params: &[VarId],
        return_type: TypeId,
    ) -> Option<crate::ir::FnId> {
        let modifiers = Modifiers::default();
        FunctionDeclaration::convert(
            ctx,
            Position::default(),
            &modifiers,
            name,
            params,
            Position::default(),
            return_type,
        )
    }

    #[test]
    fn main_of_a_fragment_program_takes_no_arguments_or_a_float2() {
        let mut ctx = context(ProgramKind::Fragment, ModuleType::Program);
        let float3 = param(&mut ctx, "c", TypeId::FLOAT3);
        assert!(convert(&mut ctx, "main", &[float3], TypeId::VOID).is_none());
        assert_eq!(
            errors(&ctx),
            ["shader 'main' must be main() or main(float2)"]
        );

        let mut ctx = context(ProgramKind::Fragment, ModuleType::Program);
        let float2 = param(&mut ctx, "coords", TypeId::FLOAT2);
        let main = convert(&mut ctx, "main", &[float2], TypeId::VOID).expect("main is valid");
        assert!(ctx.pool.function(main).has_main_coords_parameter);
        assert_eq!(errors(&ctx), Vec::<String>::new());
    }

    #[test]
    fn main_of_a_runtime_shader_returns_a_color_from_coords() {
        let mut ctx = context(ProgramKind::RuntimeShader, ModuleType::Program);
        let coords = param(&mut ctx, "xy", TypeId::FLOAT2);
        assert!(convert(&mut ctx, "main", &[coords], TypeId::FLOAT4).is_some());

        let mut ctx = context(ProgramKind::RuntimeShader, ModuleType::Program);
        let coords = param(&mut ctx, "xy", TypeId::FLOAT2);
        assert!(convert(&mut ctx, "main", &[coords], TypeId::FLOAT2).is_none());
        assert_eq!(
            errors(&ctx),
            ["'main' must return: 'vec4', 'float4', or 'half4'"]
        );
    }

    #[test]
    fn main_may_be_redeclared_but_not_overloaded() {
        let mut ctx = context(ProgramKind::Fragment, ModuleType::Program);
        let main = convert(&mut ctx, "main", &[], TypeId::VOID).expect("main()");
        // The same signature is the earlier declaration, reused.
        assert_eq!(convert(&mut ctx, "main", &[], TypeId::VOID), Some(main));
        // A different signature may not overload `main`.
        let coords = param(&mut ctx, "xy", TypeId::FLOAT2);
        assert!(convert(&mut ctx, "main", &[coords], TypeId::VOID).is_none());
        assert_eq!(errors(&ctx), ["duplicate definition of 'main'"]);
    }

    #[test]
    fn inline_and_noinline_conflict() {
        let mut ctx = context(ProgramKind::Compute, ModuleType::Program);
        let modifiers = Modifiers {
            position: Position::default(),
            layout: Layout::new(),
            flags: ModifierFlags::INLINE | ModifierFlags::NO_INLINE,
        };
        let decl = FunctionDeclaration::convert(
            &mut ctx,
            Position::default(),
            &modifiers,
            "helper",
            &[],
            Position::default(),
            TypeId::VOID,
        );
        assert!(decl.is_none());
        assert_eq!(
            errors(&ctx),
            ["functions cannot be both 'inline' and 'noinline'"]
        );
    }

    #[test]
    fn overloads_chain_and_mangle_by_parameter_type() {
        let mut ctx = context(ProgramKind::Compute, ModuleType::Program);
        let a = param(&mut ctx, "a", TypeId::FLOAT);
        let first = convert(&mut ctx, "scale", &[a], TypeId::FLOAT).expect("first overload");
        let b = param(&mut ctx, "b", TypeId::INT);
        let second = convert(&mut ctx, "scale", &[b], TypeId::INT).expect("second overload");
        assert_eq!(ctx.pool.function(second).next_overload, Some(first));
        assert_eq!(ctx.pool.function(first).mangled_name(&ctx.pool), "scale_ff");
        assert_eq!(
            ctx.pool.function(second).mangled_name(&ctx.pool),
            "scale_ii"
        );
        // A repeated signature reuses the existing declaration.
        let c = param(&mut ctx, "c", TypeId::FLOAT);
        assert_eq!(convert(&mut ctx, "scale", &[c], TypeId::FLOAT), Some(first));
        assert_eq!(errors(&ctx), Vec::<String>::new());
    }

    #[test]
    fn a_name_that_is_not_a_function_is_already_defined() {
        let mut ctx = context(ProgramKind::Compute, ModuleType::Program);
        let table = ctx.symbol_table.expect("table");
        let var = param(&mut ctx, "value", TypeId::FLOAT);
        crate::ir::add_symbol(&mut ctx, table, SymbolId::Variable(var));
        assert!(convert(&mut ctx, "value", &[], TypeId::VOID).is_none());
        assert_eq!(errors(&ctx), ["symbol 'value' was already defined"]);
    }

    #[test]
    fn intrinsics_only_resolve_in_builtin_code() {
        let mut ctx = context(ProgramKind::Compute, ModuleType::SkslShared);
        let a = param(&mut ctx, "a", TypeId::FLOAT);
        let abs = convert(&mut ctx, "abs", &[a], TypeId::FLOAT).expect("builtin abs");
        assert_eq!(
            ctx.pool.function(abs).intrinsic_kind,
            Some(IntrinsicKind::Abs)
        );
        // A builtin without a definition keeps its name when mangled.
        assert_eq!(ctx.pool.function(abs).mangled_name(&ctx.pool), "abs");
    }

    #[test]
    fn mangled_names_mark_dollar_names_in_user_code() {
        let mut ctx = context(ProgramKind::Compute, ModuleType::Program);
        let a = param(&mut ctx, "a", TypeId::FLOAT);
        let helper = convert(&mut ctx, "$helper", &[a], TypeId::FLOAT).expect("helper");
        assert_eq!(
            ctx.pool.function(helper).mangled_name(&ctx.pool),
            "helper_Qff"
        );
    }
}
