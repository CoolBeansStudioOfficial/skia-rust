// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLVarDeclarations.{h,cpp} (data and `description`).
// `ErrorCheck`, `Convert` and `Make` come with task S7d.

//! [`VarDeclaration`] (a statement) and [`GlobalVarDeclaration`] (a program element).

use super::{
    IrPool,
    ids::{ExprId, StmtId, TypeId, VarId},
};

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
