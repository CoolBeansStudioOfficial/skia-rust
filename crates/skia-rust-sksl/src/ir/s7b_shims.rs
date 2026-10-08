// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Stand-ins for work that other `SkSL` tasks own, used by the operator expressions of S7b until
//! those tasks land.
//!
//! Each function names its owner and the Skia function it stands for. The S7b code calls the
//! stand-in exactly where Skia makes the call. When the owner lands, its function replaces the
//! stand-in at each call site, and nothing else in the S7b port changes.
//!
//! Where a stand-in leaves out a behavior, the omission is named in its doc comment. Those
//! behaviors are constant folding and compile-time simplification (S7a constructors and literals,
//! S8 folding). They change the shape of the tree, never whether an error is reported.

use super::{
    ComponentArray, ConstructorArrayCast, ConstructorCompound, ConstructorCompoundCast,
    ConstructorScalarCast, ConstructorSplat, ExprId, Expression, ExpressionKind, IrPool, Literal,
    ModifierFlags, TypeId, VariableRefKind, VariableStorage, constant_folder_stub,
};
use crate::context::Context;
use crate::defines::SkslInt;
use crate::error_reporter::ErrorReporter;
use crate::position::Position;

/// Allocates `kind` at `pos` with type `ty`.
fn add(ctx: &mut Context, pos: Position, ty: TypeId, kind: ExpressionKind) -> ExprId {
    ctx.pool.add_expression(Expression::new(pos, ty, kind))
}

/// S7a: `ConstructorScalarCast::Make(context, pos, type, arg)`.
///
/// Stand-in. It keeps Skia's rules that no cast is needed when the types match, and that a cast
/// of a literal is a literal (reporting the value if it is out of range for `ty`). It leaves out
/// the `$intLiteral(...)` cast-chain collapse, which needs S7a's constructors and S8.
// Port of: src/sksl/ir/SkSLConstructorScalarCast.cpp#L63-L106 (chrome/m156), partly
pub(crate) fn make_scalar_cast(
    ctx: &mut Context,
    pos: Position,
    ty: TypeId,
    arg: ExprId,
) -> ExprId {
    let (arg_ty, arg_pos, literal) = {
        let e = ctx.pool.expression(arg);
        (e.ty, e.position, e.as_literal().map(|lit| lit.value))
    };
    if ctx.pool.ty(arg_ty).matches(ty) {
        ctx.pool.expression_mut(arg).position = pos;
        return arg;
    }
    if let Some(mut value) = literal {
        // We can cast scalar literals at compile-time. An out-of-range value is reported and
        // replaced by zero, to keep error cascades down.
        if ty.check_for_out_of_range_literal_value(ctx, value, arg_pos) {
            value = 0.0;
        }
        return make_literal(ctx, pos, value, ty);
    }
    add(
        ctx,
        pos,
        ty,
        ExpressionKind::ConstructorScalarCast(ConstructorScalarCast { argument: arg }),
    )
}

/// S7a: `ConstructorCompoundCast::Make`. Stand-in: a plain node, with no folding.
pub(crate) fn make_compound_cast(
    ctx: &mut Context,
    pos: Position,
    ty: TypeId,
    arg: ExprId,
) -> ExprId {
    add(
        ctx,
        pos,
        ty,
        ExpressionKind::ConstructorCompoundCast(ConstructorCompoundCast { argument: arg }),
    )
}

/// S7a: `ConstructorArrayCast::Make`. Stand-in: a plain node, with no folding.
pub(crate) fn make_array_cast(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
    add(
        ctx,
        pos,
        ty,
        ExpressionKind::ConstructorArrayCast(ConstructorArrayCast { argument: arg }),
    )
}

/// S7a: `ConstructorSplat::Make(context, pos, type, arg)`.
///
/// Stand-in. A splat to a scalar type is the argument itself, as in Skia. Constant-variable
/// substitution (S8) is left out.
// Port of: src/sksl/ir/SkSLConstructorSplat.cpp#L14-L40 (chrome/m156), partly
pub(crate) fn make_splat(ctx: &mut Context, pos: Position, ty: TypeId, arg: ExprId) -> ExprId {
    if ctx.pool.ty(ty).is_scalar() {
        ctx.pool.expression_mut(arg).position = pos;
        return arg;
    }
    add(
        ctx,
        pos,
        ty,
        ExpressionKind::ConstructorSplat(ConstructorSplat { argument: arg }),
    )
}

/// S7a: `ConstructorCompound::Make(context, pos, type, args)`. Stand-in: a plain node.
pub(crate) fn make_compound(
    ctx: &mut Context,
    pos: Position,
    ty: TypeId,
    arguments: Vec<ExprId>,
) -> ExprId {
    add(
        ctx,
        pos,
        ty,
        ExpressionKind::ConstructorCompound(ConstructorCompound { arguments }),
    )
}

/// S7a: `Literal::Make(pos, value, type)`. Stand-in: stores `value` as given. Skia also converts
/// it to the type's range, which only matters for values outside the type. None of S7b's callers
/// produce one.
pub(crate) fn make_literal(ctx: &mut Context, pos: Position, value: f64, ty: TypeId) -> ExprId {
    add(ctx, pos, ty, ExpressionKind::Literal(Literal { value }))
}

/// S7a: `Literal::MakeBool(pos, value, type)`.
pub(crate) fn make_bool_literal(
    ctx: &mut Context,
    pos: Position,
    value: bool,
    ty: TypeId,
) -> ExprId {
    make_literal(ctx, pos, if value { 1.0 } else { 0.0 }, ty)
}

/// S7a: `Type::coerceExpression(expr, context)`: converts `expr` to the type `ty`, or reports why
/// it cannot. Returns `None` after an error (Skia's `nullptr`).
// Port of: src/sksl/ir/SkSLType.cpp#L1280-L1308 (chrome/m156)
pub(crate) fn coerce_expression(ctx: &mut Context, ty: TypeId, expr: ExprId) -> Option<ExprId> {
    if Expression::is_incomplete(ctx, expr) {
        return None;
    }
    let (expr_ty, pos) = {
        let e = ctx.pool.expression(expr);
        (e.ty, e.position)
    };
    if ctx.pool.ty(expr_ty).matches(ty) {
        return Some(expr);
    }
    let allow_narrowing = ctx.config().settings.allow_narrowing_conversions;
    if !ctx
        .pool
        .ty(expr_ty)
        .coercion_cost(ty)
        .is_possible(allow_narrowing)
    {
        let msg = format!(
            "expected '{}', but found '{}'",
            ctx.pool.ty(ty).display_name(),
            ctx.pool.ty(expr_ty).display_name()
        );
        ctx.errors.error(pos, &msg);
        return None;
    }
    let (is_scalar, is_vector_or_matrix, is_array, display) = {
        let target = ctx.pool.ty(ty);
        (
            target.is_scalar(),
            target.is_vector() || target.is_matrix(),
            target.is_array(),
            target.display_name().to_owned(),
        )
    };
    if is_scalar {
        return Some(make_scalar_cast(ctx, pos, ty, expr));
    }
    if is_vector_or_matrix {
        return Some(make_compound_cast(ctx, pos, ty, expr));
    }
    if is_array {
        return Some(make_array_cast(ctx, pos, ty, expr));
    }
    ctx.errors
        .error(pos, &format!("cannot construct '{display}'"));
    None
}

/// S7a: `Type::convertArraySize(context, arrayPos, size)` with an expression size. It coerces the
/// size to `int` and requires a constant integer. Returns 0 after an error.
// Port of: src/sksl/ir/SkSLType.cpp#L1369-L1384 (chrome/m156)
pub(crate) fn convert_array_size_expr(
    ctx: &mut Context,
    ty: TypeId,
    array_pos: Position,
    size: ExprId,
) -> SkslInt {
    let Some(size) = coerce_expression(ctx, TypeId::INT, size) else {
        return 0;
    };
    let size_pos = ctx.pool.expression(size).position;
    let Some(count) = constant_folder_stub::get_constant_int(&ctx.pool, size) else {
        ctx.errors.error(size_pos, "array size must be an integer");
        return 0;
    };
    ty.convert_array_size_value(ctx, array_pos, size_pos, count)
}

/// S7a: the expression form of `Type::checkForOutOfRangeLiteral(context, expr)`. It reports a
/// literal in `expr` that does not fit the component type of `ty`, and returns whether it found
/// one.
///
/// Stand-in: only a plain `Literal` is checked. Skia also checks the slots of splats and compound
/// constructors, through `getConstantValue` (S7a and S8).
// Port of: src/sksl/ir/SkSLType.cpp#L1314-L1337 (chrome/m156), literal case only
pub(crate) fn check_for_out_of_range_literal_expr(
    ctx: &mut Context,
    ty: TypeId,
    expr: ExprId,
) -> bool {
    let base = ctx.pool.ty(ty).component_type().id();
    if !ctx.pool.ty(base).is_number() {
        return false;
    }
    let value_id = constant_folder_stub::get_constant_value_for_variable(&ctx.pool, expr);
    let (value, pos) = {
        let e = ctx.pool.expression(value_id);
        match e.as_literal() {
            Some(lit) => (lit.value, e.position),
            None => return false,
        }
    };
    base.check_for_out_of_range_literal_value(ctx, value, pos)
}

/// `Analysis::IsAssignable(expr, &info, errors)`: whether `expr` can be written to. Returns that
/// result and, when it is true, the `VariableReference` the write goes to (`info.fAssignedVar`).
///
/// Stand-in for the analysis that S9a owns. The logic and messages are Skia's
/// `IsAssignableVisitor`.
// Port of: src/sksl/SkSLAnalysis.cpp#L261-L336 (chrome/m156), and #L551-L554
pub(crate) fn is_assignable(
    pool: &IrPool,
    errors: &mut ErrorReporter,
    expr: ExprId,
) -> (bool, Option<ExprId>) {
    let old_error_count = errors.error_count();
    let mut assigned = None;
    visit_assignable(pool, errors, expr, None, &mut assigned);
    (errors.error_count() == old_error_count, assigned)
}

/// `IsAssignableVisitor::visitExpression(expr, fieldAccess)`.
fn visit_assignable(
    pool: &IrPool,
    errors: &mut ErrorReporter,
    expr: ExprId,
    field_access: Option<ExprId>,
    assigned: &mut Option<ExprId>,
) {
    let node = pool.expression(expr);
    let position = node.position;
    match &node.kind {
        ExpressionKind::VariableReference(var_ref) => {
            let var = pool.variable(var_ref.variable);
            let field_name = || match field_access {
                Some(field) => pool.expression_description(field),
                None => var.name.to_string(),
            };
            if var.modifier_flags.is_const() || var.modifier_flags.is_uniform() {
                let msg = format!("cannot modify immutable variable '{}'", field_name());
                errors.error(position, &msg);
            } else if var.storage == VariableStorage::Global
                && var.modifier_flags.intersects(ModifierFlags::IN)
            {
                let msg = format!("cannot modify pipeline input variable '{}'", field_name());
                errors.error(position, &msg);
            } else {
                debug_assert!(assigned.is_none());
                *assigned = Some(expr);
            }
        }
        ExpressionKind::FieldAccess(f) => {
            visit_assignable(pool, errors, f.base, Some(expr), assigned);
        }
        ExpressionKind::Swizzle(swizzle) => {
            check_swizzle_write(errors, position, swizzle.components);
            visit_assignable(pool, errors, swizzle.base, field_access, assigned);
        }
        ExpressionKind::Index(index) => {
            visit_assignable(pool, errors, index.base, field_access, assigned);
        }
        ExpressionKind::Poison(_) => {}
        _ => errors.error(position, "cannot assign to this expression"),
    }
}

/// `IsAssignableVisitor::checkSwizzleWrite`: a swizzle may not write one field twice.
fn check_swizzle_write(errors: &mut ErrorReporter, position: Position, components: ComponentArray) {
    let mut bits = 0_i32;
    for &idx in components.as_slice() {
        let bit = 1_i32 << idx;
        if bits & bit != 0 {
            errors.error(
                position,
                "cannot write to the same swizzle field more than once",
            );
            break;
        }
        bits |= bit;
    }
}

/// `Analysis::UpdateVariableRefKind(expr, kind, errors)`: checks that `expr` can be assigned to,
/// and sets the reference kind of the variable it writes to. Returns false after an error.
///
/// Stand-in for the analysis that S9a owns.
// Port of: src/sksl/SkSLAnalysis.cpp#L556-L572 (chrome/m156)
pub(crate) fn update_variable_ref_kind(
    ctx: &mut Context,
    expr: ExprId,
    kind: VariableRefKind,
) -> bool {
    let (ok, assigned) = is_assignable(&ctx.pool, &mut ctx.errors, expr);
    if !ok {
        return false;
    }
    let Some(var_ref) = assigned else {
        let (pos, desc) = {
            let e = ctx.pool.expression(expr);
            (e.position, e.description(&ctx.pool))
        };
        ctx.errors
            .error(pos, &format!("can't assign to expression '{desc}'"));
        return false;
    };
    if let ExpressionKind::VariableReference(var_ref_node) =
        &mut ctx.pool.expression_mut(var_ref).kind
    {
        var_ref_node.ref_kind = kind;
    }
    true
}
