// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionDeclaration.{h,cpp} (the data, accessors and
// `description`). The constructor's `main` parameter rules, `Convert`, `mangledName`,
// `matches` and `determineFinalTypes` come with task S7c.

//! [`FunctionDeclaration`]: a function symbol, one link of an overload chain.

use super::{
    IrPool, ModifierFlags,
    ids::{ElemId, FnId, TypeId, VarId},
};
use crate::intrinsic_list::IntrinsicKind;
use crate::modules::ModuleType;
use crate::position::Position;
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
