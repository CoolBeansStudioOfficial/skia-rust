// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorScalarCast.cpp (`ConstructorScalarCast::Convert`
// and `Make`).

//! [`ConstructorScalarCast`]'s factories.

use super::constructor::set_position;
use super::{
    ConstructorScalarCast, Expression, ExpressionKind, Literal,
    ids::{ExprId, TypeId},
};
use crate::constant_folder;
use crate::context::Context;
use crate::position::Position;

impl ConstructorScalarCast {
    /// `ConstructorScalarCast::Convert`: converts the single argument to the scalar type `raw_ty`
    /// (or its literal's scalar type). Returns `None` after an error.
    // Port of: src/sksl/ir/SkSLConstructorScalarCast.cpp#L23-L61 (chrome/m156)
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        raw_ty: TypeId,
        args: &[ExprId],
    ) -> Option<ExprId> {
        // As you might expect, scalar-cast constructors should only be created with scalar types.
        let ty = ctx.pool.ty(raw_ty).scalar_type_for_literal().id();
        debug_assert!(ctx.pool.ty(ty).is_scalar());

        if args.len() != 1 {
            let name = ctx.pool.ty(ty).display_name().to_owned();
            ctx.errors.error(
                pos,
                &format!(
                    "invalid arguments to '{name}' constructor, (expected exactly 1 argument, but found {})",
                    args.len()
                ),
            );
            return None;
        }
        let arg = args[0];
        let arg_ty = ctx.pool.expression(arg).ty;
        if !ctx.pool.ty(arg_ty).is_scalar() {
            // Casting a vector-type into its scalar component type is treated as a slice in GLSL.
            // We don't allow those casts in SkSL; recommend a .x swizzle instead.
            let mut swizzle_hint = "";
            if ctx.pool.ty(arg_ty).component_type().matches(ty) {
                if ctx.pool.ty(arg_ty).is_vector() {
                    swizzle_hint = "; use '.x' instead";
                } else if ctx.pool.ty(arg_ty).is_matrix() {
                    swizzle_hint = "; use '[0][0]' instead";
                }
            }
            let msg = format!(
                "'{}' is not a valid parameter to '{}' constructor{swizzle_hint}",
                ctx.pool.ty(arg_ty).display_name(),
                ctx.pool.ty(ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        }
        if ty.check_for_out_of_range_literal(ctx, arg) {
            return None;
        }

        Some(Self::make(ctx, pos, ty, arg))
    }

    /// `ConstructorScalarCast::Make`: a scalar of type `ty` from the scalar `arg`. A no-op cast
    /// returns `arg`, a cast of a literal is folded into a literal, and a cast of a `$…Literal`
    /// cast drops the inner cast.
    // Port of: src/sksl/ir/SkSLConstructorScalarCast.cpp#L63-L108 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_scalar());
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert!(ctx.pool.ty(ctx.pool.expression(arg).ty).is_scalar());

        // No cast required when the types match.
        let arg_ty = ctx.pool.expression(arg).ty;
        if ctx.pool.ty(arg_ty).matches(ty) {
            set_position(&mut ctx.pool, arg, pos);
            return arg;
        }

        // Look up the value of constant variables. This allows constant-expressions like
        // `int(zero)` to be replaced with a literal zero.
        let arg = constant_folder::make_constant_value_for_variable(ctx, pos, arg);

        // We can cast scalar literals at compile-time when possible. (If the resulting literal
        // would be out of range for its type, we report an error and return zero to minimize error
        // cascading. This can occur when code is inlined, so we can't necessarily catch it during
        // Convert. As such, it's not safe to return null or assert.)
        if let Some(literal) = ctx.pool.expression(arg).as_literal().copied() {
            let arg_pos = ctx.pool.expression(arg).position;
            let mut value = literal.value;
            if ty.check_for_out_of_range_literal_value(ctx, value, arg_pos) {
                value = 0.0;
            }
            return Literal::make(&mut ctx.pool, pos, value, ty);
        }

        // We allow scalar casts to abstract types `$floatLiteral` or `$intLiteral`. This can be
        // used to represent various expressions where SkSL still allows type flexibility. For
        // instance, the expression `float x = myBool ? 1 : 0` is allowed in SkSL despite the
        // apparent type mismatch, and the resolved type of expression `myBool ? 1 : 0` is actually
        // `$intLiteral`. This expression could also be rewritten as `$intLiteral(myBool)` to
        // replace a ternary with a cast.
        //
        // If we are casting an expression of the form `$intLiteral(...)` or `$floatLiteral(...)`,
        // we can eliminate the intermediate constructor-cast since it no longer adds value.
        let arg_node = ctx.pool.expression(arg);
        let inner = match &arg_node.kind {
            ExpressionKind::ConstructorScalarCast(inner_cast)
                if ctx.pool.ty(arg_node.ty).is_literal() =>
            {
                Some(inner_cast.argument)
            }
            _ => None,
        };
        if let Some(inner) = inner {
            return Self::make(ctx, pos, ty, inner);
        }

        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorScalarCast(ConstructorScalarCast { argument: arg }),
        ))
    }
}
