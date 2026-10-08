// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorArray.cpp (`ConstructorArray::Convert`, `Make`).

//! [`ConstructorArray`]'s factories.

use super::{
    ConstructorArray, ConstructorArrayCast, Expression, ExpressionKind,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;

impl ConstructorArray {
    /// `ConstructorArray::Convert`: builds an array of `ty` from `args`, or reports why it cannot
    /// (ES2 strict mode, atomics, element count). A single argument of a coercible array type is
    /// a cast instead.
    // Port of: src/sksl/ir/SkSLConstructorArray.cpp#L23-L77 (chrome/m156)
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        ty: TypeId,
        mut args: Vec<ExprId>,
    ) -> Option<ExprId> {
        debug_assert!(ctx.pool.ty(ty).is_array() && ctx.pool.ty(ty).columns() > 0);

        // ES2 doesn't support first-class array types.
        if ctx.config().strict_es2_mode() {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!("construction of array type '{name}' is not supported"),
            );
            return None;
        }

        // An array of atomics cannot be constructed.
        if ctx.pool.ty(ty).is_or_contains_atomic() {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!("construction of array type '{name}' with atomic member is not allowed"),
            );
            return None;
        }

        // If there is a single argument containing an array of matching size and the types are
        // coercible, this is actually a cast, i.e. `half[10](myFloat10Array)`. This isn't a GLSL
        // feature, but the Pipeline stage code generator needs this functionality so that code
        // which was originally compiled with "allow narrowing conversions" enabled can be later
        // recompiled without narrowing conversions (we patch over these conversions with an
        // explicit cast).
        if let [only] = args[..] {
            let expr_ty = ctx.pool.expression(only).ty;
            let expr_is_array = ctx.pool.ty(expr_ty).is_array();
            if expr_is_array && ctx.pool.ty(expr_ty).can_coerce_to(ty, true) {
                return Some(ConstructorArrayCast::make(ctx, pos, ty, only));
            }
        }

        // Check that the number of constructor arguments matches the array size.
        let columns = ctx.pool.ty(ty).columns();
        if usize::try_from(columns).ok() != Some(args.len()) {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!(
                    "invalid arguments to '{name}' constructor (expected {columns} elements, but found {})",
                    args.len()
                ),
            );
            return None;
        }

        // Convert each constructor argument to the array's component type.
        let base_type = ctx.pool.ty(ty).component_type().id();
        for argument in &mut args {
            *argument = base_type.coerce_expression(ctx, *argument)?;
        }

        Some(Self::make(ctx, pos, ty, args))
    }

    /// `ConstructorArray::Make`: an array constructor node of `ty`. The arguments have already
    /// been coerced to the component type.
    // Port of: src/sksl/ir/SkSLConstructorArray.cpp#L79-L94 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, args: Vec<ExprId>) -> ExprId {
        debug_assert!(!ctx.config().strict_es2_mode());
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert_eq!(
            ctx.pool.ty(ty).columns(),
            i32::try_from(args.len()).unwrap_or(-1)
        );
        debug_assert!(!ctx.pool.ty(ty).is_or_contains_atomic());
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorArray(ConstructorArray { arguments: args }),
        ))
    }
}
