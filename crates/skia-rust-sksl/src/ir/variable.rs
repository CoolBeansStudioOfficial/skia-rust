// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLVariable.{h,cpp} (the data, the declaration links and
// `description`). `Convert`, `Make`'s checks and `MakeScratchVariable` come with task S6.

//! [`Variable`]: a variable symbol (global, local, parameter or interface block).

use super::{
    IrPool, Layout, ModifierFlags, StatementKind,
    ids::{ElemId, ExprId, StmtId, TypeId},
};
use crate::position::Position;

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
