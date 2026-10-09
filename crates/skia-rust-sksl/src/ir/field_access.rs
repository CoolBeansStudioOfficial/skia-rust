// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFieldAccess.{h,cpp} (data, `description`, `Convert`, `Make`
// and `initialSlot`).

//! [`FieldAccess`]: `base.field`.

use super::{
    Expression, ExpressionKind, IrPool, MethodReference, Setting, SymbolId,
    ids::{ExprId, TypeId},
};
use crate::analysis;
use crate::constant_folder;
use crate::context::Context;
use crate::operator::OperatorPrecedence;
use crate::position::Position;

/// `SkSL::FieldAccessOwnerKind`: whether the base is a named value or an anonymous interface
/// block.
#[doc(alias = "SkSL::FieldAccessOwnerKind")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FieldAccessOwnerKind {
    #[default]
    Default,
    /// The base is an anonymous interface block, so the field is accessed by its bare name.
    AnonymousInterfaceBlock,
}

/// `SkSL::FieldAccess`. Its type is the field's type.
// Port of: src/sksl/ir/SkSLFieldAccess.h#L33-L105 (chrome/m156)
#[doc(alias = "SkSL::FieldAccess")]
#[derive(Clone, Debug, PartialEq)]
pub struct FieldAccess {
    /// `base()`.
    pub base: ExprId,
    /// `fieldIndex()`.
    pub field_index: usize,
    /// `ownerKind()`.
    pub owner_kind: FieldAccessOwnerKind,
}

/// `extract_field`: the field `field_index` of a struct constructor, when none of the other
/// arguments has side effects.
// Port of: src/sksl/ir/SkSLFieldAccess.cpp#L65-L82 (chrome/m156)
fn extract_field(
    ctx: &mut Context,
    pos: Position,
    args: &[ExprId],
    field_index: usize,
) -> Option<ExprId> {
    // Confirm that the fields that are being removed are side-effect free.
    for (index, &arg) in args.iter().enumerate() {
        if field_index == index {
            continue;
        }
        if analysis::has_side_effects(&ctx.pool, arg) {
            return None;
        }
    }

    // Return the desired field.
    Some(ctx.pool.clone_expression_at(args[field_index], pos))
}

impl FieldAccess {
    /// `Convert(context, pos, base, field)`: `base.field`. Dispatches a method of an effect child,
    /// a field of a struct, and a capability of `sk_Caps`. Reports errors and returns `None` on
    /// failure.
    // Port of: src/sksl/ir/SkSLFieldAccess.cpp#L31-L63 (chrome/m156)
    ///
    /// # Panics
    ///
    /// If the context has no current symbol table (Skia dereferences `fSymbolTable`).
    pub fn convert(ctx: &mut Context, pos: Position, base: ExprId, field: &str) -> Option<ExprId> {
        let base_ty = ctx.pool.expression(base).ty;
        let (is_effect_child, is_struct, is_caps, display) = {
            let t = ctx.pool.ty(base_ty);
            (
                t.is_effect_child(),
                t.is_struct(),
                t.matches(TypeId::SK_CAPS),
                t.display_name().to_owned(),
            )
        };
        if is_effect_child {
            // Turn the field name into a free function name, prefixed with '$':
            let method_name = format!("${field}");
            let table = ctx
                .symbol_table
                .expect("FieldAccess::Convert: no symbol table");
            if let Some(SymbolId::FunctionDeclaration(function)) =
                ctx.pool.find_symbol(table, &method_name)
            {
                return Some(MethodReference::make(ctx, pos, base, function));
            }
            ctx.errors.error(
                pos,
                &format!("type '{display}' has no method named '{field}'"),
            );
            return None;
        }
        if is_struct {
            let index = ctx
                .pool
                .ty(base_ty)
                .fields()
                .iter()
                .position(|f| &*f.name == field);
            if let Some(index) = index {
                return Some(Self::make(
                    ctx,
                    pos,
                    base,
                    index,
                    FieldAccessOwnerKind::Default,
                ));
            }
        }
        if is_caps {
            return Setting::convert(ctx, pos, field);
        }

        ctx.errors.error(
            pos,
            &format!("type '{display}' does not have a field named '{field}'"),
        );
        None
    }

    /// `Make(context, pos, base, fieldIndex, ownerKind)`: the access to field `field_index` of the
    /// struct `base`. The field's type is the expression's type.
    ///
    // Port of: src/sksl/ir/SkSLFieldAccess.cpp#L84-L103 (chrome/m156)
    #[must_use]
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        base: ExprId,
        field_index: usize,
        owner_kind: FieldAccessOwnerKind,
    ) -> ExprId {
        let base_ty = ctx.pool.expression(base).ty;
        debug_assert!(ctx.pool.ty(base_ty).is_struct());
        debug_assert!(field_index < ctx.pool.ty(base_ty).fields().len());

        // Replace `knownStruct.field` with the field's value if there are no side-effects involved.
        let expr = constant_folder::get_constant_value_for_variable(&ctx.pool, base);
        if let ExpressionKind::ConstructorStruct(ctor) = ctx.pool.expression(expr).kind.clone()
            && let Some(field) = extract_field(ctx, pos, &ctor.arguments, field_index)
        {
            return field;
        }

        let ty = ctx.pool.ty(base_ty).fields()[field_index].ty;
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::FieldAccess(Self {
                base,
                field_index,
                owner_kind,
            }),
        ))
    }

    /// `initialSlot()`: the first slot of the field within the base's slots.
    // Port of: src/sksl/ir/SkSLFieldAccess.cpp#L106-L114 (chrome/m156)
    #[must_use]
    pub fn initial_slot(&self, pool: &IrPool) -> usize {
        let fields = pool.ty(pool.expression(self.base).ty).fields();
        fields[..self.field_index]
            .iter()
            .map(|f| pool.ty(f.ty).slot_count())
            .sum()
    }

    /// `description()`: `base.name`, or the bare `name` when the base prints as nothing.
    // Port of: src/sksl/ir/SkSLFieldAccess.cpp#L116-L122 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut f = pool.expression_description_with(self.base, OperatorPrecedence::Postfix);
        if !f.is_empty() {
            f.push('.');
        }
        let base_type = pool.expression(self.base).ty;
        f + &pool.ty(base_type).fields()[self.field_index].name
    }
}
