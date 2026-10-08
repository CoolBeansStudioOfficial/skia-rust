// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLVariable.{h,cpp} (the data, the declaration links and
// `description`). `Convert`, `Make`'s checks and `MakeScratchVariable` come with task S6.

//! [`Variable`]: a variable symbol (global, local, parameter or interface block).

use super::{
    IrPool, Layout, ModifierFlags, StatementKind, VarDeclaration, add_symbol,
    ids::{ElemId, ExprId, StmtId, SymTabId, SymbolId, TypeId, VarId},
};
use crate::compiler::Compiler;
use crate::context::Context;
use crate::intrinsic_list::find_intrinsic_kind;
use crate::mangler::Mangler;
use crate::position::Position;
use crate::program_settings::ProgramConfig;

/// `SkSL::VariableStorage`.
#[doc(alias = "SkSL::VariableStorage")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariableStorage {
    Global,
    InterfaceBlock,
    Local,
    Parameter,
}

/// The node that declares a variable (Skia's `fDeclaringElement`, an `IRNode*` that is either a
/// `VarDeclaration` or a `GlobalVarDeclaration`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclaringElement {
    /// A `VarDeclaration` statement (locals and parameters with initializers).
    VarDeclaration(StmtId),
    /// A `GlobalVarDeclaration` program element.
    GlobalVarDeclaration(ElemId),
}

/// `SkSL::Variable` (and its `ExtendedVariable` subclass, which adds a layout, a mangled name and
/// an interface block).
// Port of: src/sksl/ir/SkSLVariable.h#L44-L175 (chrome/m156)
#[doc(alias = "SkSL::Variable")]
#[doc(alias = "SkSL::ExtendedVariable")]
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// `fPosition`.
    pub position: Position,
    /// `fModifiersPosition`.
    pub modifiers_position: Position,
    /// `fModifierFlags`.
    pub modifier_flags: ModifierFlags,
    /// The symbol name.
    pub name: Box<str>,
    /// The variable's type.
    pub ty: TypeId,
    /// `fBuiltin`.
    pub builtin: bool,
    /// `fStorage`.
    pub storage: VariableStorage,
    /// `fDeclaringElement`.
    pub declaring_element: Option<DeclaringElement>,
    /// Whether this is an `ExtendedVariable` (`Variable::Make` picks one when the type is an
    /// interface block, a mangled name is given or the layout is not empty).
    pub extended: bool,
    /// `ExtendedVariable::fLayout` (empty for a plain variable).
    pub layout: Layout,
    /// `ExtendedVariable::fMangledName` (empty when not mangled).
    pub mangled_name: Box<str>,
    /// `ExtendedVariable::fInterfaceBlockElement`.
    pub interface_block: Option<ElemId>,
}

impl Variable {
    /// The `Variable` constructor (a plain variable: empty layout, no mangled name).
    #[must_use]
    pub fn new(
        position: Position,
        modifiers_position: Position,
        modifier_flags: ModifierFlags,
        name: impl Into<Box<str>>,
        ty: TypeId,
        builtin: bool,
        storage: VariableStorage,
    ) -> Self {
        Self {
            position,
            modifiers_position,
            modifier_flags,
            name: name.into(),
            ty,
            builtin,
            storage,
            declaring_element: None,
            extended: false,
            layout: Layout::new(),
            mangled_name: "".into(),
            interface_block: None,
        }
    }

    /// `layout()`.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// `mangledName()`: the mangled name of an extended variable, otherwise the name.
    #[must_use]
    pub fn mangled_name(&self) -> &str {
        if self.mangled_name.is_empty() {
            &self.name
        } else {
            &self.mangled_name
        }
    }

    /// `varDeclaration()`: the `VarDeclaration` statement that declares this variable (for a
    /// global, the one inside its `GlobalVarDeclaration`).
    #[must_use]
    pub fn var_declaration(&self, pool: &IrPool) -> Option<StmtId> {
        match self.declaring_element? {
            DeclaringElement::VarDeclaration(stmt) => Some(stmt),
            DeclaringElement::GlobalVarDeclaration(elem) => {
                pool.element(elem).as_global_var().map(|g| g.declaration)
            }
        }
    }

    /// `globalVarDeclaration()`.
    #[must_use]
    pub fn global_var_declaration(&self) -> Option<ElemId> {
        match self.declaring_element? {
            DeclaringElement::GlobalVarDeclaration(elem) => Some(elem),
            DeclaringElement::VarDeclaration(_) => None,
        }
    }

    /// `initialValue()`: the initializer of the declaring `VarDeclaration`, if any.
    #[must_use]
    pub fn initial_value(&self, pool: &IrPool) -> Option<ExprId> {
        let decl = self.var_declaration(pool)?;
        match &pool.statement(decl).kind {
            StatementKind::VarDeclaration(v) => v.value,
            _ => None,
        }
    }

    /// `setVarDeclaration`: records the declaring statement unless one is already known.
    pub fn set_var_declaration(&mut self, declaration: StmtId) {
        if self.declaring_element.is_none() {
            self.declaring_element = Some(DeclaringElement::VarDeclaration(declaration));
        }
    }

    /// `setGlobalVarDeclaration`.
    pub fn set_global_var_declaration(&mut self, global: ElemId) {
        self.declaring_element = Some(DeclaringElement::GlobalVarDeclaration(global));
    }

    /// `setInterfaceBlock`.
    ///
    /// # Panics
    ///
    /// For a plain variable (Skia: `SkUNREACHABLE`), or if it already has an interface block.
    pub fn set_interface_block(&mut self, element: ElemId) {
        assert!(self.extended, "setInterfaceBlock on a plain Variable");
        assert!(
            self.interface_block.is_none(),
            "interface block already set"
        );
        self.interface_block = Some(element);
    }

    /// `description()`: `layout (…) modifiers type name`.
    // Port of: src/sksl/ir/SkSLVariable.h#L134-L137 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}{}{} {}",
            self.layout.padded_description(),
            self.modifier_flags.padded_description(),
            pool.ty(self.ty).display_name(),
            self.name
        )
    }
}

impl Variable {
    /// `Variable::Convert`: checks a variable declaration and adds the variable to the pool. The
    /// caller decides which symbol table it belongs to. Skia's `namePos` parameter is unused in
    /// its body, so it is not taken here.
    ///
    /// # Panics
    ///
    /// If a name that overlaps an intrinsic is declared with no current symbol table (Skia
    /// requires `context.fSymbolTable` there).
    // Port of: src/sksl/ir/SkSLVariable.cpp#L63-L118 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Convert` parameter list, in order.
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        modifiers_pos: Position,
        layout: Layout,
        mut flags: ModifierFlags,
        ty: TypeId,
        name: &str,
        storage: VariableStorage,
    ) -> VarId {
        let kind = ctx.config().kind;
        if layout.location == 0
            && layout.index == 0
            && flags.intersects(ModifierFlags::OUT)
            && ProgramConfig::is_fragment(kind)
            && name != Compiler::FRAGCOLOR_NAME
        {
            ctx.errors.error(
                modifiers_pos,
                "out location=0, index=0 is reserved for sk_FragColor",
            );
        }
        if ctx.pool.ty(ty).is_unsized_array()
            && storage != VariableStorage::InterfaceBlock
            && storage != VariableStorage::Parameter
        {
            ctx.errors
                .error(pos, "unsized arrays are not permitted here");
        }
        if ProgramConfig::is_compute(kind)
            && layout.builtin == -1
            && storage == VariableStorage::Global
        {
            if flags.intersects(ModifierFlags::IN) {
                ctx.errors
                    .error(pos, "pipeline inputs not permitted in compute shaders");
            } else if flags.intersects(ModifierFlags::OUT) {
                ctx.errors
                    .error(pos, "pipeline outputs not permitted in compute shaders");
            }
        }
        if storage == VariableStorage::Parameter
            && (flags & (ModifierFlags::OUT | ModifierFlags::IN)) == ModifierFlags::IN
        {
            // The `in` modifier on function parameters is implicit, so `in float x` is `float x`.
            // This keeps overload matching by parameter types unambiguous.
            flags.remove(ModifierFlags::OUT | ModifierFlags::IN);
        }

        // Invent a mangled name for the variable, if it needs one.
        let mangled_name = if let Some(rest) = name.strip_prefix('$') {
            // The $ prefix will fail to compile in GLSL, so replace it with `sk_Priv`.
            format!("sk_Priv{rest}")
        } else if find_intrinsic_kind(name).is_some() {
            // A user name that overlaps an intrinsic would hide the intrinsic. Such a name is
            // legal, so mangle it to avoid the collision.
            let table = ctx
                .symbol_table
                .expect("Variable::Convert: no current symbol table");
            Mangler::new().unique_name(name, &ctx.pool, table)
        } else {
            String::new()
        };
        let builtin = ctx.config().is_builtin_code();
        Self::make(
            &mut ctx.pool,
            pos,
            modifiers_pos,
            layout,
            flags,
            ty,
            name,
            mangled_name,
            builtin,
            storage,
        )
    }

    /// `Variable::Make`: allocates a variable in `pool`. Skia builds an `ExtendedVariable` when the
    /// type is an interface block's, or the variable has a mangled name or a layout. Here every
    /// variable holds those fields, and `extended` records which class Skia would have built.
    // Port of: src/sksl/ir/SkSLVariable.cpp#L120-L151 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Make` parameter list, in order.
    pub fn make(
        pool: &mut IrPool,
        pos: Position,
        modifiers_pos: Position,
        layout: Layout,
        flags: ModifierFlags,
        ty: TypeId,
        name: &str,
        mangled_name: String,
        builtin: bool,
        storage: VariableStorage,
    ) -> VarId {
        // The `in` modifier on function parameters is implicit and should have been removed.
        debug_assert!(
            !(storage == VariableStorage::Parameter
                && (flags & (ModifierFlags::OUT | ModifierFlags::IN)) == ModifierFlags::IN)
        );
        let extended = pool.ty(ty).component_type().is_interface_block()
            || !mangled_name.is_empty()
            || layout != Layout::new();
        let mut variable = Variable::new(pos, modifiers_pos, flags, name, ty, builtin, storage);
        variable.extended = extended;
        variable.layout = layout;
        variable.mangled_name = mangled_name.into();
        pool.add_variable(variable)
    }
}

/// `Variable::ScratchVariable`: a local variable made by [`Variable::make_scratch_variable`], and
/// its declaration.
#[doc(alias = "SkSL::Variable::ScratchVariable")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScratchVariable {
    /// `fVarSymbol`.
    pub var: VarId,
    /// `fVarDecl`: the `VarDeclaration` statement.
    pub decl: StmtId,
}

impl Variable {
    /// `Variable::MakeScratchVariable`: creates a local scratch variable with a fresh name, adds it
    /// to `symbol_table`, and makes its declaration. Useful when doing IR rewrites, such as inlining
    /// a function call.
    // Port of: src/sksl/ir/SkSLVariable.cpp#L181-L217 (chrome/m156)
    #[must_use]
    pub fn make_scratch_variable(
        ctx: &mut Context,
        mangler: &mut Mangler,
        base_name: &str,
        ty: TypeId,
        symbol_table: SymTabId,
        initial_value: Option<ExprId>,
    ) -> ScratchVariable {
        // $floatLiteral or $intLiteral aren't real types that we can use for scratch variables, so
        // replace them if they ever appear here. If this happens, we likely forgot to coerce a type
        // somewhere during compilation.
        let ty = if ctx.pool.ty(ty).is_literal() {
            debug_assert!(false, "found a $literal type in MakeScratchVariable");
            ctx.pool.ty(ty).scalar_type_for_literal().id()
        } else {
            ty
        };

        // Provide our new variable with a unique name, and add it to our symbol table.
        let name = mangler.unique_name(base_name, &ctx.pool, symbol_table);
        let builtin = ctx.pool.symbol_table(symbol_table).is_builtin();
        let pos = initial_value.map_or_else(Position::default, |value| {
            ctx.pool.expression(value).position
        });

        // Create our new variable and add it to the symbol table. Its type is the one given, which
        // may be an array.
        let var = ctx.pool.add_variable(Variable::new(
            pos,
            Position::default(),
            ModifierFlags::empty(),
            name.as_str(),
            ty,
            builtin,
            VariableStorage::Local,
        ));

        // If we are creating an array type, reduce it to base type plus array-size.
        let (decl_type, array_size) = if ctx.pool.ty(ty).is_array() {
            (
                ctx.pool.ty(ty).component_type().id(),
                ctx.pool.ty(ty).columns(),
            )
        } else {
            (ty, 0)
        };
        // Create our variable declaration.
        let decl = VarDeclaration::make(&mut ctx.pool, var, decl_type, array_size, initial_value);
        add_symbol(ctx, symbol_table, SymbolId::Variable(var));
        ScratchVariable { var, decl }
    }
}
