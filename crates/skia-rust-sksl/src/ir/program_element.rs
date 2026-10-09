// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLProgramElement.h, src/sksl/ir/SkSLIRNode.h
// (`ProgramElementKind`), and the data and `description` of SkSLExtension.h,
// SkSLFunctionDefinition.h, SkSLFunctionPrototype.h, SkSLInterfaceBlock.{h,cpp},
// SkSLModifiersDeclaration.h and SkSLStructDefinition.{h,cpp}. Their `Convert`/`Make` are in files
// named after the Skia classes: `function_definition.rs` (S7c), `extension.rs`,
// `interface_block.rs`, `modifiers_declaration.rs`, `struct_definition.rs` (S7d), and
// `var_declarations.rs` for `GlobalVarDeclaration::make`.

//! [`ProgramElement`]: a top-level element of a program or module.

use super::{
    GlobalVarDeclaration, IrPool, Layout, ModifierFlags,
    ids::{ElemId, FnId, StmtId, TypeId, VarId},
};
use crate::position::Position;

/// `SkSL::ProgramElement`.
// Port of: src/sksl/ir/SkSLProgramElement.h#L19-L36 (chrome/m156)
#[doc(alias = "SkSL::ProgramElement")]
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramElement {
    /// `fPosition`.
    pub position: Position,
    /// The subclass and its data (`kind()`).
    pub kind: ProgramElementKind,
}

/// `ProgramElement::Kind`, carrying each subclass's data, in Skia's order.
// Port of: src/sksl/ir/SkSLIRNode.h#L20-L31 (chrome/m156)
#[doc(alias = "SkSL::ProgramElementKind")]
#[derive(Clone, Debug, PartialEq)]
pub enum ProgramElementKind {
    Extension(Extension),
    Function(FunctionDefinition),
    FunctionPrototype(FunctionPrototype),
    GlobalVar(GlobalVarDeclaration),
    InterfaceBlock(InterfaceBlock),
    Modifiers(ModifiersDeclaration),
    StructDefinition(StructDefinition),
}

/// `SkSL::Extension`: `#extension name : enable`.
// Port of: src/sksl/ir/SkSLExtension.h#L21-L58 (chrome/m156)
#[doc(alias = "SkSL::Extension")]
#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    /// `name()`.
    pub name: Box<str>,
}

/// `SkSL::FunctionDefinition`: a function with its body.
// Port of: src/sksl/ir/SkSLFunctionDefinition.h#L25-L88 (chrome/m156)
#[doc(alias = "SkSL::FunctionDefinition")]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDefinition {
    /// `declaration()`.
    pub declaration: FnId,
    /// `body()`: a `Block`.
    pub body: StmtId,
}

/// `SkSL::FunctionPrototype`: a forward declaration.
// Port of: src/sksl/ir/SkSLFunctionPrototype.h#L21-L41 (chrome/m156)
#[doc(alias = "SkSL::FunctionPrototype")]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionPrototype {
    /// `declaration()`.
    pub declaration: FnId,
}

/// `SkSL::InterfaceBlock`: `layout(…) uniform TypeName { fields } instanceName[size];`.
// Port of: src/sksl/ir/SkSLInterfaceBlock.h#L37-L104 (chrome/m156)
#[doc(alias = "SkSL::InterfaceBlock")]
#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceBlock {
    /// `var()`: the block's variable (its type is the block's struct, or an array of it).
    pub var: VarId,
}

/// `SkSL::ModifiersDeclaration`: a declaration of modifiers only (`layout(…) in;`).
// Port of: src/sksl/ir/SkSLModifiersDeclaration.h#L24-L65 (chrome/m156)
#[doc(alias = "SkSL::ModifiersDeclaration")]
#[derive(Clone, Debug, PartialEq)]
pub struct ModifiersDeclaration {
    /// `layout()`.
    pub layout: Layout,
    /// `modifierFlags()`.
    pub flags: ModifierFlags,
}

/// `SkSL::StructDefinition`: `struct Name { fields };`.
// Port of: src/sksl/ir/SkSLStructDefinition.h#L25-L61 (chrome/m156)
#[doc(alias = "SkSL::StructDefinition")]
#[derive(Clone, Debug, PartialEq)]
pub struct StructDefinition {
    /// `type()`.
    pub ty: TypeId,
}

impl FunctionPrototype {
    /// `FunctionPrototype(pos, declaration)`: a forward declaration. Skia has no factory for it,
    /// and the parser constructs it directly; this is the same construction, as an element id.
    // Port of: src/sksl/ir/SkSLFunctionPrototype.h#L21-L41 (chrome/m156)
    #[must_use]
    pub fn make(pool: &mut IrPool, pos: Position, declaration: FnId) -> ElemId {
        pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::FunctionPrototype(Self { declaration }),
        ))
    }
}

impl InterfaceBlock {
    /// `typeName()`: the name of the block's struct type.
    #[must_use]
    pub fn type_name<'a>(&self, pool: &'a IrPool) -> &'a str {
        pool.ty(pool.variable(self.var).ty).component_type().name()
    }

    /// `instanceName()`: the variable's name (empty for an anonymous block).
    #[must_use]
    pub fn instance_name<'a>(&self, pool: &'a IrPool) -> &'a str {
        &pool.variable(self.var).name
    }

    /// `arraySize()`: the array size, or 0 when the block is not an array.
    #[must_use]
    pub fn array_size(&self, pool: &IrPool) -> i32 {
        let ty = pool.ty(pool.variable(self.var).ty);
        if ty.is_array() { ty.columns() } else { 0 }
    }

    /// `description()`.
    // Port of: src/sksl/ir/SkSLInterfaceBlock.cpp#L140-L159 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let var = pool.variable(self.var);
        let mut result = format!(
            "{}{} {} {{\n",
            var.layout.description(),
            var.modifier_flags.description(),
            self.type_name(pool)
        );
        let mut struct_type = pool.ty(var.ty);
        if struct_type.is_array() {
            struct_type = struct_type.component_type();
        }
        for f in struct_type.fields() {
            result.push_str(&f.description(pool));
            result.push('\n');
        }
        result.push('}');
        let instance_name = self.instance_name(pool);
        if !instance_name.is_empty() {
            result.push(' ');
            result.push_str(instance_name);
            let array_size = self.array_size(pool);
            if array_size > 0 {
                result.push('[');
                result.push_str(&array_size.to_string());
                result.push(']');
            }
        }
        result + ";"
    }
}

impl StructDefinition {
    /// `description()`.
    // Port of: src/sksl/ir/SkSLStructDefinition.cpp#L38-L53 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let ty = pool.ty(self.ty);
        let mut s = format!("struct {} {{ ", ty.name());
        for f in ty.fields() {
            s.push_str(&f.layout.description());
            s.push_str(&f.modifier_flags.description());
            s.push(' ');
            s.push_str(&pool.ty(f.ty).description());
            s.push(' ');
            s.push_str(&f.name);
            s.push_str("; ");
        }
        s.push_str("};");
        s
    }
}

impl ProgramElement {
    /// A node of `kind` at `position`.
    #[must_use]
    pub fn new(position: Position, kind: ProgramElementKind) -> Self {
        Self { position, kind }
    }

    /// The `GlobalVarDeclaration` payload, if this is one.
    #[must_use]
    pub fn as_global_var(&self) -> Option<&GlobalVarDeclaration> {
        match &self.kind {
            ProgramElementKind::GlobalVar(g) => Some(g),
            _ => None,
        }
    }

    /// The `FunctionDefinition` payload, if this is one.
    #[must_use]
    pub fn as_function(&self) -> Option<&FunctionDefinition> {
        match &self.kind {
            ProgramElementKind::Function(f) => Some(f),
            _ => None,
        }
    }

    /// `description()`: the element as `SkSL` text.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        match &self.kind {
            ProgramElementKind::Extension(e) => format!("#extension {} : enable", e.name),
            ProgramElementKind::Function(f) => format!(
                "{} {}",
                pool.function(f.declaration).description(pool),
                pool.statement_description(f.body)
            ),
            ProgramElementKind::FunctionPrototype(f) => {
                pool.function(f.declaration).description(pool) + ";"
            }
            ProgramElementKind::GlobalVar(g) => g.description(pool),
            ProgramElementKind::InterfaceBlock(i) => i.description(pool),
            ProgramElementKind::Modifiers(m) => {
                format!(
                    "{}{};",
                    m.layout.padded_description(),
                    m.flags.description()
                )
            }
            ProgramElementKind::StructDefinition(s) => s.description(pool),
        }
    }
}

impl IrPool {
    /// `element->description()` for the element at `id`.
    #[must_use]
    pub fn element_description(&self, id: ElemId) -> String {
        self.element(id).description(self)
    }
}
