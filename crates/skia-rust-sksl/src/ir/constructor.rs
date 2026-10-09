// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLConstructor.{h,cpp} (`Constructor::Convert`, the constant
// values and comparisons of `AnyConstructor`, and `AnyConstructor::description`) and the data of
// the nine constructor classes (src/sksl/ir/SkSLConstructor*.h). Each class's `Convert`/`Make`
// lives in its own file (`constructor_array.rs`, …).
//
// The constant-folding and analysis helpers at the end of this file (`constant_value_or_null`,
// `is_compile_time_constant`, `is_same_expression_tree`, `get_constant_int`) are minimal copies of
// `ConstantFolder` (S8) and `Analysis` (S9a) functions that the constructors need. They are
// `pub(crate)` and marked as such, so S8/S9a can replace them with their own.

//! The constructor expressions (`AnyConstructor` and its subclasses).

use super::{
    ComparisonResult, Expression, ExpressionKind, IrPool,
    ids::{ExprId, TypeId},
};
use crate::context::Context;
use crate::operator::OperatorPrecedence;
use crate::position::Position;
use crate::string::Separator;

/// `SkSL::ConstructorArray`: `float[3](a, b, c)`.
#[doc(alias = "SkSL::ConstructorArray")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorArray {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `SkSL::ConstructorArrayCast`: `half[2](floatArray)`.
#[doc(alias = "SkSL::ConstructorArrayCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorArrayCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorCompound`: `float4(xy, z, w)`, `float2x2(…)`.
#[doc(alias = "SkSL::ConstructorCompound")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorCompound {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `SkSL::ConstructorCompoundCast`: `half4(float4Value)`.
#[doc(alias = "SkSL::ConstructorCompoundCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorCompoundCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorDiagonalMatrix`: `float2x2(1)`.
#[doc(alias = "SkSL::ConstructorDiagonalMatrix")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorDiagonalMatrix {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorMatrixResize`: `float3x3(float2x2Value)`.
#[doc(alias = "SkSL::ConstructorMatrixResize")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorMatrixResize {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorScalarCast`: `int(floatValue)`.
#[doc(alias = "SkSL::ConstructorScalarCast")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorScalarCast {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorSplat`: `float4(1)`.
#[doc(alias = "SkSL::ConstructorSplat")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorSplat {
    /// `argument()`.
    pub argument: ExprId,
}

/// `SkSL::ConstructorStruct`: `S(a, b)`.
#[doc(alias = "SkSL::ConstructorStruct")]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorStruct {
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `AnyConstructor::description`: `type(arg, arg, …)`.
// Port of: src/sksl/ir/SkSLConstructor.cpp#L228-L237 (chrome/m156)
#[doc(alias = "AnyConstructor::description")]
#[must_use]
pub fn any_constructor_description(pool: &IrPool, ty: TypeId, arguments: &[ExprId]) -> String {
    let mut result = pool.ty(ty).description() + "(";
    let mut separator = Separator::new();
    for &arg in arguments {
        result.push_str(separator.next_str());
        result.push_str(&pool.expression_description_with(arg, OperatorPrecedence::Sequence));
    }
    result.push(')');
    result
}

/// `Constructor::Convert`: converts `args` to a value of type `ty`, or reports why it cannot.
/// Returns `None` after an error.
// Port of: src/sksl/ir/SkSLConstructor.cpp#L151-L176 (chrome/m156)
#[doc(alias = "Constructor::Convert")]
pub fn convert(ctx: &mut Context, pos: Position, ty: TypeId, args: Vec<ExprId>) -> Option<ExprId> {
    if let [arg] = args[..] {
        let arg_ty = ctx.pool.expression(arg).ty;
        if ctx.pool.ty(arg_ty).matches(ty) && !ctx.pool.ty(ty).component_type().is_opaque() {
            // Don't generate redundant casts; if the expression is already of the correct type,
            // just return it as-is.
            set_position(&mut ctx.pool, arg, pos);
            return Some(arg);
        }
    }
    let (is_scalar, is_compound, is_array, is_struct) = {
        let t = ctx.pool.ty(ty);
        (
            t.is_scalar(),
            t.is_vector() || t.is_matrix(),
            t.is_array() && t.columns() > 0,
            t.is_struct() && !t.fields().is_empty(),
        )
    };
    if is_scalar {
        return ConstructorScalarCast::convert(ctx, pos, ty, &args);
    }
    if is_compound {
        return convert_compound_constructor(ctx, pos, ty, args);
    }
    if is_array {
        return ConstructorArray::convert(ctx, pos, ty, args);
    }
    if is_struct {
        return ConstructorStruct::convert(ctx, pos, ty, args);
    }
    let name = ctx.pool.ty(ty).display_name().to_owned();
    ctx.errors.error(pos, &format!("cannot construct '{name}'"));
    None
}

/// `convert_compound_constructor`: vector and matrix constructors, with their special cases for
/// a single argument.
// Port of: src/sksl/ir/SkSLConstructor.cpp#L28-L149 (chrome/m156)
fn convert_compound_constructor(
    ctx: &mut Context,
    pos: Position,
    ty: TypeId,
    mut args: Vec<ExprId>,
) -> Option<ExprId> {
    debug_assert!(ctx.pool.ty(ty).is_vector() || ctx.pool.ty(ty).is_matrix());

    // The meaning of a compound constructor containing a single argument varies significantly in
    // GLSL/SkSL, depending on the argument type.
    if let [first] = args[..] {
        let mut argument = first;
        let arg_ty = ctx.pool.expression(argument).ty;
        let type_is_vector = ctx.pool.ty(ty).is_vector();
        let type_is_matrix = ctx.pool.ty(ty).is_matrix();
        let arg_is_scalar = ctx.pool.ty(arg_ty).is_scalar();
        let arg_is_vector = ctx.pool.ty(arg_ty).is_vector();
        let arg_is_matrix = ctx.pool.ty(arg_ty).is_matrix();

        if type_is_vector
            && arg_is_vector
            && ctx
                .pool
                .ty(arg_ty)
                .component_type()
                .matches(ctx.pool.ty(ty).component_type().id())
            && ctx.pool.ty(arg_ty).slot_count() > ctx.pool.ty(ty).slot_count()
        {
            // Casting a vector-type into a smaller matching vector-type is a slice in GLSL. We
            // don't allow those casts in SkSL; recommend a swizzle instead. Only `.xy` and `.xyz`
            // are valid recommendations here, because `.x` would imply a scalar(vector) cast, and
            // nothing has more slots than `.xyzw`.
            let swizzle_hint = match ctx.pool.ty(ty).slot_count() {
                2 => "; use '.xy' instead",
                3 => "; use '.xyz' instead",
                _ => {
                    debug_assert!(false, "unexpected slicing cast");
                    ""
                }
            };
            let msg = format!(
                "'{}' is not a valid parameter to '{}' constructor{swizzle_hint}",
                ctx.pool.ty(arg_ty).display_name(),
                ctx.pool.ty(ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        if arg_is_scalar {
            // A constructor containing a single scalar is a splat (for vectors) or diagonal matrix
            // (for matrices). It's legal regardless of the scalar's type, so synthesize an explicit
            // conversion to the proper type. (This cast is a no-op if it's unnecessary; it can fail
            // if we're casting a literal that exceeds the limits of the type.)
            let component = ctx.pool.ty(ty).component_type().id();
            let typecast = ConstructorScalarCast::convert(ctx, pos, component, &args)?;

            // Matrix-from-scalar creates a diagonal matrix; vector-from-scalar creates a splat.
            return Some(if type_is_matrix {
                ConstructorDiagonalMatrix::make(ctx, pos, ty, typecast)
            } else {
                ConstructorSplat::make(ctx, pos, ty, typecast)
            });
        } else if arg_is_vector {
            // A vector constructor containing a single vector with the same number of columns is a
            // cast (e.g. float3 -> int3).
            if type_is_vector && ctx.pool.ty(arg_ty).columns() == ctx.pool.ty(ty).columns() {
                return Some(ConstructorCompoundCast::make(ctx, pos, ty, argument));
            }
        } else if arg_is_matrix {
            // A matrix constructor containing a single matrix can be a resize, typecast, or both.
            // GLSL lumps these into one category, but internally SkSL keeps them distinct.
            if type_is_matrix {
                // First, handle type conversion. If the component types differ, synthesize the
                // destination type with the argument's rows/columns. (This will be a no-op if it's
                // already the right type.)
                let columns = ctx.pool.ty(arg_ty).columns();
                let rows = ctx.pool.ty(arg_ty).rows();
                let typecast_type = ctx.pool.ty(ty).component_type().to_compound(columns, rows);
                argument = ConstructorCompoundCast::make(ctx, pos, typecast_type, argument);

                // Casting a matrix type into another matrix type is a resize.
                return Some(ConstructorMatrixResize::make(ctx, pos, ty, argument));
            }

            // A vector constructor containing a single matrix can be compound construction if the
            // matrix is 2x2 and the vector is 4-slot.
            if type_is_vector
                && ctx.pool.ty(ty).columns() == 4
                && ctx.pool.ty(arg_ty).slot_count() == 4
            {
                // Casting a 2x2 matrix to a vector is a form of compound construction. First,
                // reshape the matrix into a 4-slot vector of the same type.
                let vector_type = ctx.pool.ty(arg_ty).component_type().to_compound(4, 1);
                let vec_ctor = ConstructorCompound::make(ctx, pos, vector_type, args);

                // Then, add a typecast to the result expression to ensure the types match. This
                // will be a no-op if no typecasting is needed.
                return Some(ConstructorCompoundCast::make(ctx, pos, ty, vec_ctor));
            }
        }
    }

    // For more complex cases, we walk the argument list and fix up the arguments as needed.
    let expected = ctx.pool.ty(ty).rows() * ctx.pool.ty(ty).columns();
    let mut actual = 0;
    for arg in &mut args {
        let arg_ty = ctx.pool.expression(*arg).ty;
        let arg_is_scalar = ctx.pool.ty(arg_ty).is_scalar();
        let arg_is_vector = ctx.pool.ty(arg_ty).is_vector();
        if !arg_is_scalar && !arg_is_vector {
            let msg = format!(
                "'{}' is not a valid parameter to '{}' constructor",
                ctx.pool.ty(arg_ty).display_name(),
                ctx.pool.ty(ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        // Rely on Constructor::Convert to force this subexpression to the proper type. If it's a
        // literal, this will make sure it's the right type of literal. If an expression of matching
        // type, the expression will be returned as-is. If it's an expression of mismatched type,
        // this adds a cast.
        // (`columns()` is only defined for scalars and vectors, which the check above ensures.)
        let arg_columns = ctx.pool.ty(arg_ty).columns();
        let ctor_type = ctx.pool.ty(ty).component_type().to_compound(arg_columns, 1);
        *arg = convert(ctx, pos, ctor_type, vec![*arg])?;
        actual += ctx.pool.ty(ctor_type).columns();
    }

    if actual != expected {
        let msg = format!(
            "invalid arguments to '{}' constructor (expected {expected} scalars, but found {actual})",
            ctx.pool.ty(ty).display_name()
        );
        ctx.errors.error(pos, &msg);
        return None;
    }

    Some(ConstructorCompound::make(ctx, pos, ty, args))
}

/// Sets `pool[id].position = pos`, as Skia's `setPosition` (`fPosition = pos`) does.
pub(crate) fn set_position(pool: &mut IrPool, id: ExprId, pos: Position) {
    pool.expression_mut(id).position = pos;
}

impl Expression {
    /// `supportsConstantValues()`: whether `get_constant_value` may return a value for this kind.
    /// Literals and every constructor support constant values. The overrides of `IndexExpression`,
    /// `PrefixExpression` and `FunctionCall` come with S7b, S7c and S8, and are not here yet.
    // Port of: src/sksl/ir/SkSLLiteral.h#L126-L128 and src/sksl/ir/SkSLConstructor.h#L45
    // (chrome/m156).
    #[must_use]
    pub fn supports_constant_values(&self) -> bool {
        matches!(self.kind, ExpressionKind::Literal(_)) || self.is_any_constructor()
    }

    /// `getConstantValue(n)`: the `n`th slot of this expression, if it is a known constant.
    /// Returns `None` for a non-constant slot, and for kinds whose override is not yet ported.
    // Port of: src/sksl/ir/SkSLLiteral.h#L130-L133, src/sksl/ir/SkSLConstructor.cpp#L178-L190,
    // src/sksl/ir/SkSLConstructorSplat.h#L52-L55, src/sksl/ir/SkSLConstructorDiagonalMatrix.cpp
    // #L32-L45 and src/sksl/ir/SkSLConstructorMatrixResize.cpp#L31-L58 (chrome/m156).
    #[must_use]
    pub fn get_constant_value(&self, pool: &IrPool, n: usize) -> Option<f64> {
        match &self.kind {
            ExpressionKind::Literal(l) => {
                debug_assert_eq!(n, 0, "a literal has one slot");
                Some(l.value)
            }
            ExpressionKind::ConstructorSplat(s) => {
                pool.expression(s.argument).get_constant_value(pool, 0)
            }
            ExpressionKind::ConstructorDiagonalMatrix(d) => {
                let rows = matrix_dimension(pool, self.ty, true);
                let row = n % rows;
                let col = n / rows;
                if col == row {
                    pool.expression(d.argument).get_constant_value(pool, 0)
                } else {
                    Some(0.0)
                }
            }
            ExpressionKind::ConstructorMatrixResize(r) => {
                let rows = matrix_dimension(pool, self.ty, true);
                let row = n % rows;
                let col = n / rows;

                // GLSL resize matrices are of the form:
                //  |m m 0|
                //  |m m 0|
                //  |0 0 1|
                // Where `m` is the matrix being wrapped, and other cells contain the identity
                // matrix. Forward to the wrapped matrix if the position is in its bounds.
                let inner = pool.expression(r.argument);
                let inner_cols = matrix_dimension(pool, inner.ty, false);
                let inner_rows = matrix_dimension(pool, inner.ty, true);
                if col < inner_cols && row < inner_rows {
                    return inner.get_constant_value(pool, row + col * inner_rows);
                }
                // Synthesize an identity matrix for out-of-bounds positions.
                Some(if col == row { 1.0 } else { 0.0 })
            }
            _ => self.any_constructor_constant_value(pool, n),
        }
    }

    /// `AnyConstructor::getConstantValue(n)`: the slot is found among the arguments, in order.
    // Port of: src/sksl/ir/SkSLConstructor.cpp#L178-L190 (chrome/m156)
    fn any_constructor_constant_value(&self, pool: &IrPool, mut n: usize) -> Option<f64> {
        for &arg in self.any_constructor_arguments()? {
            let arg = pool.expression(arg);
            let arg_slots = pool.ty(arg.ty).slot_count();
            if n < arg_slots {
                return arg.get_constant_value(pool, n);
            }
            n -= arg_slots;
        }
        debug_assert!(
            false,
            "argument-list slot count doesn't match constructor-type slot count"
        );
        None
    }

    /// `compareConstant(other)`: whether this expression and `other` are known to be equal,
    /// unequal, or of unknown relation.
    // Port of: src/sksl/ir/SkSLLiteral.h#L113-L124 and src/sksl/ir/SkSLConstructor.cpp#L192-L216
    // (chrome/m156).
    #[must_use]
    // Skia compares the constant slots with `!=` on doubles, and so does this.
    #[allow(clippy::float_cmp)]
    pub fn compare_constant(&self, pool: &IrPool, other: &Expression) -> ComparisonResult {
        match &self.kind {
            ExpressionKind::Literal(l) => {
                let Some(other_lit) = other.as_literal() else {
                    return ComparisonResult::Unknown;
                };
                if pool.ty(self.ty).number_kind() != pool.ty(other.ty).number_kind() {
                    return ComparisonResult::Unknown;
                }
                if l.value == other_lit.value {
                    ComparisonResult::Equal
                } else {
                    ComparisonResult::NotEqual
                }
            }
            _ if self.is_any_constructor() => {
                debug_assert_eq!(
                    pool.ty(self.ty).slot_count(),
                    pool.ty(other.ty).slot_count()
                );
                if !other.supports_constant_values() {
                    return ComparisonResult::Unknown;
                }
                let exprs = pool.ty(self.ty).slot_count();
                for n in 0..exprs {
                    // Get the n'th subexpression from each side. If either one is null, return
                    // "unknown."
                    let Some(left) = self.get_constant_value(pool, n) else {
                        return ComparisonResult::Unknown;
                    };
                    let Some(right) = other.get_constant_value(pool, n) else {
                        return ComparisonResult::Unknown;
                    };
                    // Both sides are known and can be compared for equality directly.
                    // Skia compares the doubles with `!=`, so this does too.
                    if left != right {
                        return ComparisonResult::NotEqual;
                    }
                }
                ComparisonResult::Equal
            }
            _ => ComparisonResult::Unknown,
        }
    }
}

/// The rows (`rows == true`) or the columns of a matrix type, as `usize`. Matrices have at least
/// one of each, so the zero for a non-matrix is never used.
fn matrix_dimension(pool: &IrPool, ty: TypeId, rows: bool) -> usize {
    let t = pool.ty(ty);
    let dim = if rows { t.rows() } else { t.columns() };
    usize::try_from(dim).unwrap_or(0)
}
