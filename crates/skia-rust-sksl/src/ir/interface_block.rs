// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLInterfaceBlock.{h,cpp} (`Convert` and `Make`).

//! The factories of [`InterfaceBlock`].

use super::{
    Field, FieldSymbol, InterfaceBlock, Modifiers, ProgramElement, ProgramElementKind, Type,
    VarDeclaration, Variable, VariableStorage, add_array_dimension, add_symbol,
    ids::{ElemId, SymbolId, TypeId, VarId},
    struct_definition::add_type_symbol,
};
use crate::compiler::Compiler;
use crate::context::Context;
use crate::position::Position;
use crate::program_settings::ProgramConfig;

impl InterfaceBlock {
    /// `InterfaceBlock::Convert`: a block of `fields` named `type_name`, with the instance
    /// `var_name` (and an array size, when `array_size` is positive). The block's variable is
    /// global, so it is error-checked as one.
    ///
    /// # Panics
    ///
    /// If no symbol table is current.
    // Port of: src/sksl/ir/SkSLInterfaceBlock.cpp#L46-L110 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors Skia's `Convert` parameter list, in order.
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        modifiers: &Modifiers,
        type_name: &str,
        fields: Vec<Field>,
        var_name: &str,
        array_size: i32,
    ) -> Option<ElemId> {
        let kind = ctx.config().kind;
        if !ProgramConfig::is_fragment(kind)
            && !ProgramConfig::is_vertex(kind)
            && !ProgramConfig::is_compute(kind)
        {
            ctx.errors.error(
                pos,
                "interface blocks are not allowed in this kind of program",
            );
            return None;
        }

        // Find sk_RTAdjust and error out if it's not of type `float4`.
        if let Some(rt_adjust) = fields.iter().find(|f| &*f.name == Compiler::RTADJUST_NAME)
            && !ctx.pool.ty(rt_adjust.ty).matches(TypeId::FLOAT4)
        {
            ctx.errors
                .error(rt_adjust.position, "sk_RTAdjust must have type 'float4'");
            return None;
        }

        // Build a struct type corresponding to the passed-in fields.
        let struct_type = Type::make_struct_type(ctx, pos, type_name, fields, true);
        let base_type = add_type_symbol(ctx, struct_type);

        // Array-ify the type if necessary.
        let mut ty = base_type;
        if array_size > 0 {
            let size = base_type.convert_array_size_value(ctx, pos, pos, i64::from(array_size));
            if size == 0 {
                return None;
            }
            let table = ctx
                .symbol_table
                .expect("an interface block is declared in a symbol table");
            let size = i32::try_from(size).expect("a converted array size fits the declared int");
            ty = add_array_dimension(ctx, table, base_type, size);
        }

        // Error-check the interface block as if it were being declared as a global variable.
        VarDeclaration::error_check(
            ctx,
            pos,
            modifiers.position,
            &modifiers.layout,
            modifiers.flags,
            ty,
            base_type,
            VariableStorage::Global,
        );

        // Create a global variable for the Interface Block.
        let var = Variable::convert(
            ctx,
            pos,
            modifiers.position,
            modifiers.layout,
            modifiers.flags,
            ty,
            var_name,
            VariableStorage::Global,
        );
        Some(Self::make(ctx, pos, var))
    }

    /// `InterfaceBlock::Make`: the element for `variable`, which is added to the current symbol
    /// table. An anonymous block adds each of its fields as a symbol instead.
    ///
    /// # Panics
    ///
    /// If no symbol table is current.
    // Port of: src/sksl/ir/SkSLInterfaceBlock.cpp#L111-L138 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, variable: VarId) -> ElemId {
        let (is_anonymous, field_positions) = {
            let var = ctx.pool.variable(variable);
            let component = ctx.pool.ty(var.ty).component_type();
            debug_assert!(component.is_interface_block());
            let positions: Vec<Position> = component.fields().iter().map(|f| f.position).collect();
            (var.name.is_empty(), positions)
        };
        let table = ctx
            .symbol_table
            .expect("an interface block is added to the current symbol table");
        if is_anonymous {
            // This interface block is anonymous. Add each field to the top-level symbol table.
            for (index, position) in field_positions.into_iter().enumerate() {
                let symbol = FieldSymbol::new(&ctx.pool, position, variable, index);
                let field = ctx.pool.add_field_symbol(symbol);
                add_symbol(ctx, table, SymbolId::Field(field));
            }
        } else {
            // Add the global variable to the top-level symbol table.
            add_symbol(ctx, table, SymbolId::Variable(variable));
        }
        let element = ctx.pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::InterfaceBlock(Self { var: variable }),
        ));
        ctx.pool.variable_mut(variable).set_interface_block(element);
        element
    }
}
