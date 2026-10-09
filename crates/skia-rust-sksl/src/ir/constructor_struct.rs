// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorStruct.cpp (`ConstructorStruct::Convert`, `Make`).

//! [`ConstructorStruct`]'s factories.

use super::{
    ConstructorStruct, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;

impl ConstructorStruct {
    /// `ConstructorStruct::Convert`: builds a struct of `ty` from `args`, one per field, or
    /// reports why it cannot (count, atomic members). Each argument is coerced to its field type.
    // Port of: src/sksl/ir/SkSLConstructorStruct.cpp#L23-L75 (chrome/m156)
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        ty: TypeId,
        mut args: Vec<ExprId>,
    ) -> Option<ExprId> {
        debug_assert!(ctx.pool.ty(ty).is_struct() && !ctx.pool.ty(ty).fields().is_empty());
        let field_types: Vec<TypeId> = ctx.pool.ty(ty).fields().iter().map(|f| f.ty).collect();

        // Check that the number of constructor arguments matches the field count.
        if field_types.len() != args.len() {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!(
                    "invalid arguments to '{name}' constructor (expected {} elements, but found {})",
                    field_types.len(),
                    args.len()
                ),
            );
            return None;
        }

        // A struct with atomic members cannot be constructed.
        if ctx.pool.ty(ty).is_or_contains_atomic() {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!("construction of struct type '{name}' with atomic member is not allowed"),
            );
            return None;
        }

        // Convert each constructor argument to the struct's field type.
        for (argument, field_type) in args.iter_mut().zip(field_types) {
            *argument = field_type.coerce_expression(ctx, *argument)?;
        }

        Some(Self::make(ctx, pos, ty, args))
    }

    /// `ConstructorStruct::Make`: a struct constructor node of `ty`. The arguments have already
    /// been coerced to the field types.
    // Port of: src/sksl/ir/SkSLConstructorStruct.cpp#L77-L86 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, args: Vec<ExprId>) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert!(!ctx.pool.ty(ty).is_or_contains_atomic());
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorStruct(ConstructorStruct { arguments: args }),
        ))
    }
}
