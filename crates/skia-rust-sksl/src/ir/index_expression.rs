// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLIndexExpression.{h,cpp} (data, `IndexType`, `Convert`,
// `Make` and `description`).

//! [`IndexExpression`]: `base[index]`.

use super::{
    ComponentArray, ExprId, Expression, ExpressionKind, IrPool, Swizzle, TypeId, TypeReference,
    add_array_dimension, constant_folder_stub, s7b_shims,
};
use crate::context::Context;
use crate::defines::SkslInt;
use crate::operator::OperatorPrecedence;
use crate::position::Position;

/// `Type::kUnsizedArray`: the column count of an array whose size is not known.
const UNSIZED_ARRAY: i32 = -1;

/// `SkSL::IndexExpression`.
// Port of: src/sksl/ir/SkSLIndexExpression.h#L25-L94 (chrome/m156)
#[doc(alias = "SkSL::IndexExpression")]
#[derive(Clone, Debug, PartialEq)]
pub struct IndexExpression {
    /// `base()`.
    pub base: ExprId,
    /// `index()`.
    pub index: ExprId,
}

/// `IndexExpression::IndexType`: the type of an element of a value of type `ty`. A matrix's
/// element is a column vector, and any other value's element is its component type.
// Port of: src/sksl/ir/SkSLIndexExpression.cpp#L46-L65 (chrome/m156)
#[must_use]
pub fn index_type(pool: &IrPool, ty: TypeId) -> TypeId {
    let t = pool.ty(ty);
    if t.is_matrix() {
        let rows = t.rows();
        if t.component_type().matches(TypeId::FLOAT) {
            match rows {
                2 => return TypeId::FLOAT2,
                3 => return TypeId::FLOAT3,
                4 => return TypeId::FLOAT4,
                _ => {}
            }
        } else if t.component_type().matches(TypeId::HALF) {
            match rows {
                2 => return TypeId::HALF2,
                3 => return TypeId::HALF3,
                4 => return TypeId::HALF4,
                _ => {}
            }
        }
    }
    t.component_type().id()
}

impl IndexExpression {
    /// `Convert(context, pos, base, index)`: builds `base[index]`, or an array type when `base`
    /// is a type name (`int[10]`). Reports errors and returns `None` on failure.
    // Port of: src/sksl/ir/SkSLIndexExpression.cpp#L67-L105 (chrome/m156)
    ///
    /// # Panics
    ///
    /// If `base` is a type name and the context has no current symbol table.
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        base: ExprId,
        index: ExprId,
    ) -> Option<ExprId> {
        // Convert an array type reference: `int[10]`.
        if let ExpressionKind::TypeReference(reference) = &ctx.pool.expression(base).kind {
            let base_type = reference.value;
            let array_size = s7b_shims::convert_array_size_expr(ctx, base_type, pos, index);
            if array_size == 0 {
                return None;
            }
            let table = ctx
                .symbol_table
                .expect("IndexExpression::Convert: no symbol table");
            // Skia passes the `SKSL_INT` size as an `int`; the same narrowing happens here.
            #[allow(clippy::cast_possible_truncation)]
            let array_size = array_size as i32;
            let array = add_array_dimension(ctx, table, base_type, array_size);
            return TypeReference::convert(ctx, pos, array);
        }

        // Convert an index expression with an expression inside of it: `arr[a * 3]`.
        let (base_ty, base_pos) = {
            let e = ctx.pool.expression(base);
            (e.ty, e.position)
        };
        let (is_array, is_matrix, is_vector, display) = {
            let t = ctx.pool.ty(base_ty);
            (
                t.is_array(),
                t.is_matrix(),
                t.is_vector(),
                t.display_name().to_owned(),
            )
        };
        if !is_array && !is_matrix && !is_vector {
            ctx.errors
                .error(base_pos, &format!("expected array, but found '{display}'"));
            return None;
        }

        let index_ty = ctx.pool.expression(index).ty;
        let index = if ctx.pool.ty(index_ty).is_integer() {
            index
        } else {
            s7b_shims::coerce_expression(ctx, TypeId::INT, index)?
        };

        // Perform compile-time bounds checking on constant-expression indices.
        if let Some(index_value) = constant_folder_stub::get_constant_int(&ctx.pool, index) {
            let index_pos = ctx.pool.expression(index).position;
            if index_out_of_range(ctx, index_pos, index_value, base) {
                return None;
            }
        }

        Some(Self::make(ctx, pos, base, index))
    }

    /// `Make(context, pos, base, index)`: builds `base[index]`. A constant index into a vector
    /// becomes a swizzle (`v[2]` is `v.z`). The operands must already be valid.
    ///
    /// Not ported: plucking a constant index out of an array constructor, and out of a matrix
    /// constructor. Both need `Analysis::HasSideEffects` (S9a) and `getConstantValue` (S7a and S8).
    // Port of: src/sksl/ir/SkSLIndexExpression.cpp#L107-L170 (chrome/m156), vector case
    pub fn make(ctx: &mut Context, pos: Position, base: ExprId, index: ExprId) -> ExprId {
        let base_ty = ctx.pool.expression(base).ty;
        if let Some(index_value) = constant_folder_stub::get_constant_int(&ctx.pool, index) {
            let index_pos = ctx.pool.expression(index).position;
            if !index_out_of_range(ctx, index_pos, index_value, base)
                && ctx.pool.ty(base_ty).is_vector()
            {
                // Constant array indexes on vectors can be converted to swizzles: `v[2]` --> `v.z`.
                // Swizzling is harmless and can unlock further simplifications for some base types.
                // The index is in range for the vector, so it fits in a component.
                #[allow(clippy::cast_possible_truncation)] // Mirrors Skia's `(int8_t)indexValue`.
                let component = index_value as i8;
                let mut components = ComponentArray::default();
                components.push(component);
                return Swizzle::make(ctx, pos, base, components);
            }
        }

        let ty = index_type(&ctx.pool, base_ty);
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Index(Self { base, index }),
        ))
    }

    /// `description()`: `base[index]`.
    // Port of: src/sksl/ir/SkSLIndexExpression.cpp#L172-L175 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}[{}]",
            pool.expression_description_with(self.base, OperatorPrecedence::Postfix),
            pool.expression_description_with(self.index, OperatorPrecedence::EXPRESSION)
        )
    }
}

/// `index_out_of_range(context, pos, index, base)`: reports an index outside the columns of
/// `base` (an unsized array has no bound), and returns whether it did.
// Port of: src/sksl/ir/SkSLIndexExpression.cpp#L32-L44 (chrome/m156)
fn index_out_of_range(ctx: &mut Context, pos: Position, index: SkslInt, base: ExprId) -> bool {
    let (base_ty, display) = {
        let e = ctx.pool.expression(base);
        let t = ctx.pool.ty(e.ty);
        (e.ty, t.display_name().to_owned())
    };
    if index >= 0 {
        let columns = i64::from(ctx.pool.ty(base_ty).columns());
        if columns == i64::from(UNSIZED_ARRAY) || index < columns {
            return false;
        }
    }
    ctx.errors
        .error(pos, &format!("index {index} out of range for '{display}'"));
    true
}
