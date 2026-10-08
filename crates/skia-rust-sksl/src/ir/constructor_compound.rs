// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructorCompound.cpp (`ConstructorCompound::Make`,
// `MakeFromConstants`, `is_safe_to_eliminate` and `make_splat_from_arguments`).

//! [`ConstructorCompound`]'s factories.

use super::constructor::set_position;
use super::{
    ConstructorCompound, ConstructorSplat, Expression, ExpressionKind, IrPool, Literal,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::position::Position;
use crate::{analysis, constant_folder};

impl ConstructorCompound {
    /// `ConstructorCompound::Make`: a vector or matrix built from `args`. A no-op constructor
    /// returns its argument, nested compounds are flattened, and a compound of one repeated
    /// value becomes a splat. The flattening and splat rules run only when the optimizer is on.
    // Port of: src/sksl/ir/SkSLConstructorCompound.cpp#L78-L157 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, ty: TypeId, mut args: Vec<ExprId>) -> ExprId {
        debug_assert!(ctx.pool.ty(ty).is_allowed_in_es2_for(ctx));
        debug_assert_eq!(
            ctx.pool.ty(ty).slot_count(),
            args.iter()
                .map(|&a| ctx.pool.ty(ctx.pool.expression(a).ty).slot_count())
                .sum::<usize>()
        );

        // No-op compound constructors (containing a single argument of the same type) are
        // eliminated. (Even though this is a "compound constructor," we let scalars pass through
        // here; it's harmless to allow and simplifies call sites which need to narrow a vector and
        // may sometimes end up with a scalar.)
        if let [only] = args[..]
            && is_safe_to_eliminate(&ctx.pool, ty, only)
        {
            set_position(&mut ctx.pool, only, pos);
            return only;
        }
        // Beyond this point, the type must be a vector or matrix.
        debug_assert!(ctx.pool.ty(ty).is_vector() || ctx.pool.ty(ty).is_matrix());

        let optimize = ctx.config().settings.optimize;
        if optimize {
            // Find ConstructorCompounds embedded inside other ConstructorCompounds and flatten
            // them.
            //   -  float4(float2(1, 2), 3, 4)                -->  float4(1, 2, 3, 4)
            //   -  float4(w, float3(sin(x), cos(y), tan(z))) -->  float4(w, sin(x), cos(y), tan(z))
            //   -  mat2(float2(a, b), float2(c, d))          -->  mat2(a, b, c, d)

            // See how many fields we would have if composite constructors were flattened out.
            let fields: usize = args
                .iter()
                .map(|&arg| match &ctx.pool.expression(arg).kind {
                    ExpressionKind::ConstructorCompound(c) => c.arguments.len(),
                    _ => 1,
                })
                .sum();

            // If we added up more fields than we're starting with, we found at least one input
            // that can be flattened out.
            if fields > args.len() {
                let mut flattened = Vec::with_capacity(fields);
                for &arg in &args {
                    // For non-ConstructorCompound fields, move them over as-is. For
                    // ConstructorCompound fields, move over their inner arguments individually.
                    match &ctx.pool.expression(arg).kind {
                        ExpressionKind::ConstructorCompound(c) => {
                            flattened.extend_from_slice(&c.arguments);
                        }
                        _ => flattened.push(arg),
                    }
                }
                args = flattened;
            }
        }

        // Replace constant variables with their corresponding values, so `float2(one, two)` can
        // compile down to `float2(1.0, 2.0)` (the latter is a compile-time constant).
        for arg in &mut args {
            *arg = constant_folder::make_constant_value_for_variable(ctx, pos, *arg);
        }

        if optimize {
            // Reduce compound constructors to splats where possible.
            if let Some(splat) = make_splat_from_arguments(&ctx.pool, ty, &args) {
                let splat = ctx.pool.clone_expression(splat);
                return ConstructorSplat::make(ctx, pos, ty, splat);
            }
        }

        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::ConstructorCompound(ConstructorCompound { arguments: args }),
        ))
    }

    /// `ConstructorCompound::MakeFromConstants`: a compound of `ty` whose slots are the literal
    /// values in `values`, one per slot.
    // Port of: src/sksl/ir/SkSLConstructorCompound.cpp#L159-L169 (chrome/m156)
    #[must_use]
    pub fn make_from_constants(
        ctx: &mut Context,
        pos: Position,
        return_type: TypeId,
        values: &[f64],
    ) -> ExprId {
        let num_slots = ctx.pool.ty(return_type).slot_count();
        let component = ctx.pool.ty(return_type).component_type().id();
        let mut array = Vec::with_capacity(num_slots);
        for &value in &values[..num_slots] {
            array.push(Literal::make(&mut ctx.pool, pos, value, component));
        }
        Self::make(ctx, pos, return_type, array)
    }
}

/// `is_safe_to_eliminate`: a single argument that a constructor of `ty` would only repeat.
// Port of: src/sksl/ir/SkSLConstructorCompound.cpp#L28-L45 (chrome/m156)
fn is_safe_to_eliminate(pool: &IrPool, ty: TypeId, arg: ExprId) -> bool {
    let t = pool.ty(ty);
    if t.is_scalar() {
        // A scalar "compound type" with a single scalar argument is a no-op and can be eliminated.
        // (Pedantically, this isn't a compound at all, but it's harmless to allow and simplifies
        // call sites which need to narrow a vector and may sometimes end up with a scalar.)
        debug_assert!(pool.ty(pool.expression(arg).ty).matches(ty));
        return true;
    }
    // A vector compound constructor containing a single argument of matching type can trivially
    // be eliminated. A matrix or a converting vector is a meaningful single-argument compound.
    t.is_vector() && pool.ty(pool.expression(arg).ty).matches(ty)
}

/// `make_splat_from_arguments`: the value that every argument repeats, if the arguments are all
/// that value (scalars or splats of it). Matrices are never splats.
// Port of: src/sksl/ir/SkSLConstructorCompound.cpp#L47-L76 (chrome/m156)
fn make_splat_from_arguments(pool: &IrPool, ty: TypeId, args: &[ExprId]) -> Option<ExprId> {
    // Splats cannot represent a matrix.
    if pool.ty(ty).is_matrix() {
        return None;
    }
    let mut splat: Option<ExprId> = None;
    for &arg in args {
        let arg_expr = pool.expression(arg);
        // Arguments must only be scalars or splat constructors (which can only contain scalars).
        let expr = if pool.ty(arg_expr.ty).is_scalar() {
            arg
        } else if let ExpressionKind::ConstructorSplat(s) = &arg_expr.kind {
            s.argument
        } else {
            return None;
        };
        match splat {
            // On the first iteration, just remember the expression we encountered.
            None => splat = Some(expr),
            // On subsequent iterations, ensure that the expression we found matches the first
            // one. (IsSameExpressionTree always rejects an Expression with side effects.)
            Some(first) => {
                if !analysis::is_same_expression_tree(pool, expr, first) {
                    return None;
                }
            }
        }
    }
    splat
}
