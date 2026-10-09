// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLVarDeclarations.{h,cpp} (data, `description`, `ErrorCheck`,
// `ErrorCheckAndCoerce`, `Convert` and `Make`).

//! [`VarDeclaration`] (a statement) and [`GlobalVarDeclaration`] (a program element).

use super::{
    ElemId, IrPool, Layout, LayoutFlags, ModifierFlags, Modifiers, ProgramElement,
    ProgramElementKind, Statement, StatementKind, TypeId, TypeKind, Variable, VariableStorage,
    add_symbol,
    ids::{ExprId, StmtId, SymbolId, VarId},
};
use crate::analysis;
use crate::compiler::Compiler;
use crate::context::Context;
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind};

/// `check_valid_uniform_type`: a `uniform` must have a type that the program kind allows.
// Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L24-L65 (chrome/m156)
fn check_valid_uniform_type(ctx: &mut Context, pos: Position, t: TypeId) -> bool {
    let kind = ctx.config().kind;
    let type_name = ctx.pool.ty(t).display_name().to_owned();
    let report = |ctx: &mut Context| {
        ctx.errors.error(
            pos,
            &format!("variables of type '{type_name}' may not be uniform"),
        );
    };
    // In Runtime Effects we only allow a restricted set of types: shader, blender, colorFilter,
    // 32-bit signed integers, 16-bit and 32-bit floats, and their vector/square-matrix composites.
    if ProgramConfig::is_runtime_effect(kind) {
        let ty = ctx.pool.ty(t);
        // `shader`, `blender`, `colorFilter`
        if ty.is_effect_child() {
            return true;
        }
        // `int`, `int2`, `int3`, `int4`
        let ct = ty.component_type();
        if ct.is_signed() && ct.bit_width() == 32 && (ty.is_scalar() || ty.is_vector()) {
            return true;
        }
        // `float`, `float2`, `float3`, `float4`, `float2x2`, `float3x3`, `float4x4`
        // `half`, `half2`, `half3`, `half4`, `half2x2`, `half3x3`, `half4x4`
        if ct.is_float()
            && (ty.is_scalar() || ty.is_vector() || (ty.is_matrix() && ty.rows() == ty.columns()))
        {
            return true;
        }
        // Everything else is an error.
        report(ctx);
        return false;
    }
    let mut error_position = Position::default();
    if !ctx
        .pool
        .ty(t)
        .is_allowed_in_uniform(Some(&mut error_position))
    {
        report(ctx);
        if error_position.valid() {
            ctx.errors.error(error_position, "caused by:");
        }
        return false;
    }
    true
}

impl VarDeclaration {
    /// `VarDeclaration::ErrorCheck`: checks the modifiers, base type and storage for compatibility
    /// with one another, and reports each conflict. It is called by `ErrorCheckAndCoerce`, and
    /// explicitly while processing interface block fields.
    #[allow(clippy::too_many_arguments)]
    // Mirrors Skia's `ErrorCheck` parameter list, in order.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L96-L262 (chrome/m156)
    // Skia's `ErrorCheck` is one function of this length, and its checks run in its order.
    #[allow(clippy::too_many_lines)]
    pub fn error_check(
        ctx: &mut Context,
        pos: Position,
        modifiers_position: Position,
        layout: &Layout,
        flags: ModifierFlags,
        ty: TypeId,
        base_type: TypeId,
        storage: VariableStorage,
    ) {
        debug_assert!(if ctx.pool.ty(ty).is_array() {
            ctx.pool
                .ty(base_type)
                .matches(ctx.pool.ty(ty).component_type().id())
        } else {
            ctx.pool.ty(base_type).matches(ty)
        });
        let kind = ctx.config().kind;
        let base = ctx.pool.ty(base_type);
        let base_name = base.display_name().to_owned();
        let base_component = base.component_type();
        if base_component.is_opaque()
            && !base_component.is_atomic()
            && storage != VariableStorage::Global
        {
            ctx.errors.error(
                pos,
                &format!("variables of type '{base_name}' must be global"),
            );
        }
        if flags.intersects(ModifierFlags::IN) && base.is_matrix() {
            ctx.errors
                .error(pos, "'in' variables may not have matrix type");
        }
        let ty_unsized = ctx.pool.ty(ty).is_unsized_array();
        if flags.intersects(ModifierFlags::IN) && ty_unsized {
            ctx.errors
                .error(pos, "'in' variables may not have unsized array type");
        }
        if flags.intersects(ModifierFlags::OUT) && ty_unsized {
            ctx.errors
                .error(pos, "'out' variables may not have unsized array type");
        }
        if flags.intersects(ModifierFlags::IN) && flags.is_uniform() {
            ctx.errors
                .error(pos, "'in uniform' variables not permitted");
        }
        if flags.is_read_only() && flags.is_write_only() {
            ctx.errors.error(
                pos,
                "'readonly' and 'writeonly' qualifiers cannot be combined",
            );
        }
        if flags.is_uniform() && flags.is_buffer() {
            ctx.errors
                .error(pos, "'uniform buffer' variables not permitted");
        }
        if flags.is_workgroup() && flags.intersects(ModifierFlags::IN | ModifierFlags::OUT) {
            ctx.errors
                .error(pos, "in / out variables may not be declared workgroup");
        }
        if flags.is_uniform() {
            check_valid_uniform_type(ctx, pos, base_type);
        }
        let base = ctx.pool.ty(base_type);
        let effect_child = base.is_effect_child();
        if effect_child && !flags.is_uniform() {
            ctx.errors.error(
                pos,
                &format!("variables of type '{base_name}' must be uniform"),
            );
        }
        if effect_child && kind == ProgramKind::MeshVertex {
            ctx.errors
                .error(pos, "effects are not permitted in mesh vertex shaders");
        }
        if base.is_or_contains_atomic() {
            // An atomic variable (or a struct or an array that contains an atomic member) must be
            // either:
            //   a. Declared as a workgroup-shared variable, OR
            //   b. Declared as the member of writable storage buffer block (i.e. has no readonly
            //   restriction).
            //
            // The checks below will enforce these two rules on all declarations. If the variable is
            // not declared with the workgroup modifier, then it must be declared in the interface
            // block storage. If this is the declaration for an interface block that contains an
            // atomic member, then it must have the `buffer` modifier and no `readonly` modifier.
            let is_block_member = storage == VariableStorage::InterfaceBlock;
            let is_writable_storage_buffer = flags.is_buffer() && !flags.is_read_only();
            let permitted_here = if base.is_interface_block() {
                is_writable_storage_buffer
            } else {
                is_block_member
            };
            if !flags.is_workgroup() && !permitted_here {
                ctx.errors.error(
                    pos,
                    "atomics are only permitted in workgroup variables and writable storage blocks",
                );
            }
        }
        if layout.flags.intersects(LayoutFlags::COLOR) {
            if !ProgramConfig::is_runtime_effect(kind) {
                ctx.errors
                    .error(pos, "'layout(color)' is only permitted in runtime effects");
            }
            if !flags.is_uniform() {
                ctx.errors.error(
                    pos,
                    "'layout(color)' is only permitted on 'uniform' variables",
                );
            }
            // A vector of three or four floats.
            let valid_color_xform_type = {
                let t = ctx.pool.ty(base_type);
                t.is_vector()
                    && t.component_type().is_float()
                    && (t.columns() == 3 || t.columns() == 4)
            };
            if !valid_color_xform_type {
                ctx.errors.error(
                    pos,
                    &format!("'layout(color)' is not permitted on variables of type '{base_name}'"),
                );
            }
        }
        let mut permitted = ModifierFlags::CONST
            | ModifierFlags::HIGHP
            | ModifierFlags::MEDIUMP
            | ModifierFlags::LOWP;
        if storage == VariableStorage::Global {
            // Uniforms are allowed in all programs
            permitted |= ModifierFlags::UNIFORM;
            // No other modifiers are allowed in runtime effects.
            if !ProgramConfig::is_runtime_effect(kind) {
                let base = ctx.pool.ty(base_type);
                if base.is_interface_block() {
                    // Interface blocks allow `buffer`.
                    permitted |= ModifierFlags::BUFFER;
                    if flags.is_buffer() {
                        // Only storage blocks allow `readonly` and `writeonly`.
                        // (`readonly` and `writeonly` textures are converted to separate types via
                        // applyAccessQualifiers.)
                        permitted |= ModifierFlags::READ_ONLY | ModifierFlags::WRITE_ONLY;
                    }
                    // It is an error for an unsized array to appear anywhere but the last member
                    // of a "buffer" block.
                    let fields = base.fields();
                    let illegal_range_end = i64::try_from(fields.len()).unwrap_or(i64::MAX)
                        - i64::from(flags.is_buffer());
                    let misplaced: Vec<Position> = fields
                        .iter()
                        .take(usize::try_from(illegal_range_end.max(0)).unwrap_or(0))
                        .filter(|f| ctx.pool.ty(f.ty).is_unsized_array())
                        .map(|f| f.position)
                        .collect();
                    for position in misplaced {
                        ctx.errors.error(
                            position,
                            "unsized array must be the last member of a storage block",
                        );
                    }
                }
                let base = ctx.pool.ty(base_type);
                if !base.is_opaque() {
                    // Only non-opaque types allow `in` and `out`.
                    permitted |= ModifierFlags::IN | ModifierFlags::OUT;
                }
                if ProgramConfig::is_fragment(kind)
                    && base.is_struct()
                    && !base.is_interface_block()
                {
                    // Only structs in fragment shaders allow `pixel_local`.
                    permitted |= ModifierFlags::PIXEL_LOCAL;
                }
                if ProgramConfig::is_compute(kind) {
                    // Only compute shaders allow `workgroup`.
                    if !base.is_opaque() || base.is_atomic() {
                        permitted |= ModifierFlags::WORKGROUP;
                    }
                } else {
                    // Only vertex/fragment shaders allow `flat` and `noperspective`.
                    permitted |= ModifierFlags::FLAT | ModifierFlags::NO_PERSPECTIVE;
                }
            }
        }
        // `kAll` is all bits set, so that every layout flag is permitted until one is removed.
        let mut permitted_layout = LayoutFlags::from_bits_retain(-1);
        // Pixel format modifiers are:
        //  - Required on dedicated storage textures (writeonly / readwrite).
        //  - Optional on readonly textures.
        //  - Forbidden on all other types (samplers, subpass inputs, numeric types, etc.).
        let base = ctx.pool.ty(base_type);
        if base.is_storage_texture() {
            if !layout.flags.intersects(LayoutFlags::ALL_PIXEL_FORMATS) {
                ctx.errors
                    .error(pos, "storage textures must declare a pixel format");
            }
        } else if base.is_read_only_texture() {
            // Readonly textures can be either storage textures (with format) or sampled textures
            // (without format).
        } else {
            permitted_layout.remove(LayoutFlags::ALL_PIXEL_FORMATS);
        }
        // The `texture` and `sampler` modifiers can be present respectively on a texture and
        // sampler or simultaneously on a combined image-sampler but they are not permitted on any
        // other type.
        match base.type_kind {
            TypeKind::Sampler => {
                // Both texture and sampler flags are permitted
            }
            TypeKind::Texture => {
                permitted_layout.remove(LayoutFlags::SAMPLER);
            }
            TypeKind::SeparateSampler => {
                permitted_layout.remove(LayoutFlags::TEXTURE);
            }
            _ => {
                permitted_layout.remove(LayoutFlags::TEXTURE | LayoutFlags::SAMPLER);
            }
        }
        // We don't allow 'binding' or 'set' on normal uniform variables, only on textures,
        // samplers, and interface blocks (holding uniform variables). They're also only allowed at
        // global scope, not on interface block fields (or locals/parameters).
        let permit_binding_and_set = matches!(
            base.type_kind,
            TypeKind::Sampler | TypeKind::SeparateSampler | TypeKind::Texture
        ) || base.is_interface_block();
        if storage != VariableStorage::Global || (flags.is_uniform() && !permit_binding_and_set) {
            permitted_layout
                .remove(LayoutFlags::BINDING | LayoutFlags::SET | LayoutFlags::ALL_BACKENDS);
        }
        if ProgramConfig::is_runtime_effect(kind) {
            // Disallow all layout flags except 'color' in runtime effects
            permitted_layout &= LayoutFlags::COLOR;
        }
        // The `push_constant` flag isn't allowed on in-variables, out-variables, bindings or sets.
        if layout
            .flags
            .intersects(LayoutFlags::SET | LayoutFlags::BINDING)
            || flags.intersects(ModifierFlags::IN | ModifierFlags::OUT)
        {
            permitted_layout.remove(LayoutFlags::PUSH_CONSTANT);
        }
        // The `builtin` layout flag is only allowed in modules.
        if !ctx.config().is_builtin_code() {
            permitted_layout.remove(LayoutFlags::BUILTIN);
        }
        flags.check_permitted_flags(ctx, modifiers_position, permitted);
        layout.check_permitted_layout(ctx, modifiers_position, permitted_layout);
    }

    /// `VarDeclaration::ErrorCheckAndCoerce`: checks a variable's declaration, and coerces its
    /// initial value to the variable's type. Returns false when the declaration is invalid.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L264-L339 (chrome/m156)
    // Skia's `ErrorCheckAndCoerce` is one function of this length, and its checks run in its order.
    #[allow(clippy::too_many_lines)]
    fn error_check_and_coerce(
        ctx: &mut Context,
        var: VarId,
        base_type: TypeId,
        value: &mut Option<ExprId>,
    ) -> bool {
        let (var_pos, modifiers_pos, layout, flags, var_ty, storage) = {
            let v = ctx.pool.variable(var);
            (
                v.position,
                v.modifiers_position,
                v.layout,
                v.modifier_flags,
                v.ty,
                v.storage,
            )
        };
        if ctx.pool.ty(base_type).matches(TypeId::INVALID) {
            ctx.errors.error(var_pos, "invalid type");
            return false;
        }
        if ctx.pool.ty(base_type).is_void() {
            ctx.errors
                .error(var_pos, "variables of type 'void' are not allowed");
            return false;
        }
        Self::error_check(
            ctx,
            var_pos,
            modifiers_pos,
            &layout,
            flags,
            var_ty,
            base_type,
            storage,
        );
        if let Some(init) = *value {
            let init_pos = ctx.pool.expression(init).position;
            let var_type_name = ctx.pool.ty(var_ty).display_name().to_owned();
            if ctx.pool.ty(var_ty).is_opaque() || ctx.pool.ty(var_ty).is_or_contains_atomic() {
                ctx.errors.error(
                    init_pos,
                    &format!("opaque type '{var_type_name}' cannot use initializer expressions"),
                );
                return false;
            }
            if flags.intersects(ModifierFlags::IN) {
                ctx.errors.error(
                    init_pos,
                    "'in' variables cannot use initializer expressions",
                );
                return false;
            }
            if flags.is_uniform() {
                ctx.errors.error(
                    init_pos,
                    "'uniform' variables cannot use initializer expressions",
                );
                return false;
            }
            if storage == VariableStorage::InterfaceBlock {
                ctx.errors.error(
                    init_pos,
                    "initializers are not permitted on interface block fields",
                );
                return false;
            }
            if ctx.config().strict_es2_mode() && ctx.pool.ty(var_ty).is_or_contains_array() {
                ctx.errors.error(
                    init_pos,
                    "initializers are not permitted on arrays (or structs containing arrays)",
                );
                return false;
            }
            match var_ty.coerce_expression(ctx, init) {
                Some(coerced) => *value = Some(coerced),
                None => return false,
            }
        }
        if flags.is_const() {
            let Some(init) = *value else {
                ctx.errors
                    .error(var_pos, "'const' variables must be initialized");
                return false;
            };
            if !analysis::is_constant_expression(&ctx.pool, init) {
                ctx.errors.error(
                    ctx.pool.expression(init).position,
                    "'const' variable initializer must be a constant expression",
                );
                return false;
            }
        }
        if storage == VariableStorage::InterfaceBlock && ctx.pool.ty(var_ty).is_opaque() {
            let name = ctx.pool.ty(var_ty).display_name().to_owned();
            ctx.errors.error(
                var_pos,
                &format!("opaque type '{name}' is not permitted in an interface block"),
            );
            return false;
        }
        if storage == VariableStorage::Global
            && let Some(init) = *value
            && !analysis::is_constant_expression(&ctx.pool, init)
        {
            ctx.errors.error(
                ctx.pool.expression(init).position,
                "global variable initializer must be a constant expression",
            );
            return false;
        }
        true
    }

    /// `VarDeclaration::Convert(context, overallPos, modifiers, type, namePos, name, storage,
    /// value)`: declares a variable that does not exist yet. The variable joins the current symbol
    /// table, and its initial value is checked and coerced. Errors are reported to `ctx`.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L341-L357 (chrome/m156)
    // Skia's `namePos` is unused by `Variable::Convert`, so it is not taken here.
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Convert` parameter list, in order.
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        overall_pos: Position,
        modifiers: &Modifiers,
        ty: TypeId,
        name: &str,
        storage: VariableStorage,
        value: Option<ExprId>,
    ) -> Option<StmtId> {
        // Parameter declaration-statements do not exist in the grammar (unlike, say, K&R C).
        debug_assert_ne!(storage, VariableStorage::Parameter);
        let var = Variable::convert(
            ctx,
            overall_pos,
            modifiers.position,
            modifiers.layout,
            modifiers.flags,
            ty,
            name,
            storage,
        );
        Self::convert_variable(ctx, var, value)
    }

    /// `VarDeclaration::Convert(context, std::unique_ptr<Variable>, value)`: declares a variable
    /// that already exists. The variable joins the current symbol table, after the checks of
    /// `Convert` (which returns `None` on error, and leaves the variable out of the table).
    ///
    /// # Panics
    ///
    /// If no symbol table is current.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L359-L400 (chrome/m156)
    #[must_use]
    pub fn convert_variable(
        ctx: &mut Context,
        var: VarId,
        value: Option<ExprId>,
    ) -> Option<StmtId> {
        let mut base_type = ctx.pool.variable(var).ty;
        let mut array_size = 0;
        if ctx.pool.ty(base_type).is_array() {
            array_size = ctx.pool.ty(base_type).columns();
            base_type = ctx.pool.ty(base_type).component_type().id();
        }
        let mut value = value;
        if !Self::error_check_and_coerce(ctx, var, base_type, &mut value) {
            return None;
        }
        let decl = Self::make(&mut ctx.pool, var, base_type, array_size, value);
        let (storage, name, pos, var_ty) = {
            let v = ctx.pool.variable(var);
            (v.storage, v.name.clone(), v.position, v.ty)
        };
        let table = ctx
            .symbol_table
            .expect("VarDeclaration::Convert: no current symbol table");
        if storage == VariableStorage::Global || storage == VariableStorage::InterfaceBlock {
            // Check if this globally-scoped variable name overlaps an existing symbol name.
            if ctx.pool.find_symbol(table, &name).is_some() {
                ctx.errors
                    .error(pos, &format!("symbol '{name}' was already defined"));
                // The declaration is destroyed, so it stops being the variable's declaration.
                ctx.pool.variable_mut(var).declaring_element = None;
                return None;
            }
            // `sk_RTAdjust` is special, and makes the IR generator emit position-fixup expressions.
            if &*name == Compiler::RTADJUST_NAME && !ctx.pool.ty(var_ty).matches(TypeId::FLOAT4) {
                ctx.errors.error(pos, "sk_RTAdjust must have type 'float4'");
                ctx.pool.variable_mut(var).declaring_element = None;
                return None;
            }
        }
        add_symbol(ctx, table, SymbolId::Variable(var));
        Some(decl)
    }

    /// `VarDeclaration::Make`: a declaration of `var` with the given base type, array size and
    /// initial value. The symbol table is left as it is, and the variable records this statement.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L402-L424 (chrome/m156)
    #[must_use]
    pub fn make(
        pool: &mut IrPool,
        var: VarId,
        base_type: TypeId,
        array_size: i32,
        value: Option<ExprId>,
    ) -> StmtId {
        let pos = pool.variable(var).position;
        let decl = pool.add_statement(Statement::new(
            pos,
            StatementKind::VarDeclaration(Self {
                var,
                base_type,
                array_size,
                value,
            }),
        ));
        pool.variable_mut(var).set_var_declaration(decl);
        decl
    }
}

/// `SkSL::VarDeclaration`: `type name[arraySize] = value;`. Its position is the variable's.
///
/// Skia's destructor detaches the variable (`detachDeadVarDeclaration`); pool nodes are never
/// destroyed, so there is nothing to detach.
// Port of: src/sksl/ir/SkSLVarDeclarations.h#L34-L128 (chrome/m156)
#[doc(alias = "SkSL::VarDeclaration")]
#[derive(Clone, Debug, PartialEq)]
pub struct VarDeclaration {
    /// `var()`.
    pub var: VarId,
    /// `baseType()`: the element type for an array declaration.
    pub base_type: TypeId,
    /// `arraySize()`: zero means "not an array".
    pub array_size: i32,
    /// `value()`.
    pub value: Option<ExprId>,
}

impl VarDeclaration {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLVarDeclarations.cpp#L82-L94 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let var = pool.variable(self.var);
        let mut result = format!(
            "{}{}{} {}",
            var.layout.padded_description(),
            var.modifier_flags.padded_description(),
            pool.ty(self.base_type).description(),
            var.name
        );
        if self.array_size > 0 {
            result.push('[');
            result.push_str(&self.array_size.to_string());
            result.push(']');
        }
        if let Some(value) = self.value {
            result.push_str(" = ");
            result.push_str(&pool.expression_description(value));
        }
        result.push(';');
        result
    }
}

/// `SkSL::GlobalVarDeclaration`: a `VarDeclaration` at global scope. Its position is the
/// declaration's.
// Port of: src/sksl/ir/SkSLVarDeclarations.h#L130-L170 (chrome/m156)
#[doc(alias = "SkSL::GlobalVarDeclaration")]
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalVarDeclaration {
    /// `declaration()`: a `VarDeclaration` statement.
    pub declaration: StmtId,
}

impl GlobalVarDeclaration {
    /// `varDeclaration()`.
    ///
    /// # Panics
    ///
    /// If the declaration is not a `VarDeclaration`.
    #[must_use]
    pub fn var_declaration<'a>(&self, pool: &'a IrPool) -> &'a VarDeclaration {
        pool.statement(self.declaration)
            .as_var_declaration()
            .expect("GlobalVarDeclaration must hold a VarDeclaration")
    }

    /// `description()`: the declaration's.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        pool.statement_description(self.declaration)
    }
}

impl GlobalVarDeclaration {
    /// `GlobalVarDeclaration(decl)`: a program element that declares a global variable. The
    /// variable records this element as its declaration.
    ///
    /// # Panics
    ///
    /// If `declaration` is not a `VarDeclaration`.
    // Port of: src/sksl/ir/SkSLVarDeclarations.h#L130-L150 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, declaration: StmtId) -> ElemId {
        let pos = pool.statement(declaration).position;
        let var = pool
            .statement(declaration)
            .as_var_declaration()
            .expect("GlobalVarDeclaration must hold a VarDeclaration")
            .var;
        let element = pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::GlobalVar(Self { declaration }),
        ));
        pool.variable_mut(var).set_global_var_declaration(element);
        element
    }
}
