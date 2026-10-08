// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLLiteral.{h,cpp}: the data, the accessors, the `Make*`
// factories and `description`. Its `compareConstant` and `getConstantValue` are in `constructor.rs`
// with the other constant values.

//! [`Literal`]: a float, integer or boolean constant.

use super::{
    Expression, ExpressionKind, IrPool,
    ids::{ExprId, TypeId},
};
use crate::defines::{SkslFloat, SkslInt};
use crate::position::Position;
use crate::skstd;

/// `SkSL::Literal`. The value is stored as a `double` whatever the type.
// Port of: src/sksl/ir/SkSLLiteral.h#L34-L136 (chrome/m156)
#[doc(alias = "SkSL::Literal")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Literal {
    /// `value()`.
    pub value: f64,
}

impl Literal {
    /// `Literal::Make(pos, value, type)`: a literal of `ty`, rounded to that type. A float is
    /// rounded to `float`, an integer is truncated, and a boolean is `value != 0`.
    // Port of: src/sksl/ir/SkSLLiteral.h#L81-L90 (chrome/m156)
    #[doc(alias = "SkSL::Literal::Make")]
    // The narrowing casts are Skia's `double` to `float` and `double` to `SKSL_INT` conversions.
    #[allow(clippy::cast_possible_truncation)]
    pub fn make(pool: &mut IrPool, pos: Position, value: f64, ty: TypeId) -> ExprId {
        if pool.ty(ty).is_float() {
            // A C++ `double` to `float` conversion rounds to nearest, as Rust's `as` does.
            Self::make_float(pool, pos, value as SkslFloat, ty)
        } else if pool.ty(ty).is_integer() {
            // The C++ `double` to `SKSL_INT` conversion truncates toward zero, as `as` does.
            Self::make_int(pool, pos, value as SkslInt, ty)
        } else {
            debug_assert!(pool.ty(ty).is_boolean());
            Self::make_bool(pool, pos, value != 0.0, ty)
        }
    }

    /// `Literal::MakeFloat(pos, value, type)`: a float literal of `ty`.
    // Port of: src/sksl/ir/SkSLLiteral.h#L48-L52 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeFloat")]
    pub fn make_float(pool: &mut IrPool, pos: Position, value: SkslFloat, ty: TypeId) -> ExprId {
        debug_assert!(pool.ty(ty).is_float());
        Self::add(pool, pos, f64::from(value), ty)
    }

    /// `Literal::MakeInt(pos, value, type)`: an integer literal of `ty`, which must hold `value`.
    // Port of: src/sksl/ir/SkSLLiteral.h#L59-L66 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeInt")]
    pub fn make_int(pool: &mut IrPool, pos: Position, value: SkslInt, ty: TypeId) -> ExprId {
        debug_assert!(pool.ty(ty).is_integer());
        // The stored value is a `double`, as in Skia. The range is checked against the type's
        // `double` limits.
        #[allow(clippy::cast_precision_loss)] // Mirrors Skia's `int64` to `double` store.
        let as_double = value as f64;
        debug_assert!(
            as_double >= pool.ty(ty).minimum_value(),
            "Value does not fit in type"
        );
        debug_assert!(
            as_double <= pool.ty(ty).maximum_value(),
            "Value does not fit in type"
        );
        Self::add(pool, pos, as_double, ty)
    }

    /// `Literal::MakeBool(pos, value, type)`: a boolean literal of `ty`, stored as 1.0 or 0.0.
    // Port of: src/sksl/ir/SkSLLiteral.h#L75-L79 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeBool")]
    pub fn make_bool(pool: &mut IrPool, pos: Position, value: bool, ty: TypeId) -> ExprId {
        debug_assert!(pool.ty(ty).is_boolean());
        Self::add(pool, pos, if value { 1.0 } else { 0.0 }, ty)
    }

    /// `Literal::MakeFloat(context, pos, value)`: a float literal of the abstract `$floatLiteral`
    /// type. The built-in type needs no context here.
    // Port of: src/sksl/ir/SkSLLiteral.h#L43-L46 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeFloat")]
    pub fn make_float_literal(pool: &mut IrPool, pos: Position, value: SkslFloat) -> ExprId {
        Self::make_float(pool, pos, value, TypeId::FLOAT_LITERAL)
    }

    /// `Literal::MakeInt(context, pos, value)`: an integer literal of the abstract `$intLiteral`
    /// type.
    // Port of: src/sksl/ir/SkSLLiteral.h#L54-L57 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeInt")]
    pub fn make_int_literal(pool: &mut IrPool, pos: Position, value: SkslInt) -> ExprId {
        Self::make_int(pool, pos, value, TypeId::INT_LITERAL)
    }

    /// `Literal::MakeBool(context, pos, value)`: a boolean literal.
    // Port of: src/sksl/ir/SkSLLiteral.h#L69-L72 (chrome/m156)
    #[doc(alias = "SkSL::Literal::MakeBool")]
    pub fn make_bool_literal(pool: &mut IrPool, pos: Position, value: bool) -> ExprId {
        Self::make_bool(pool, pos, value, TypeId::BOOL)
    }

    /// Adds a literal node of type `ty` holding `value`.
    fn add(pool: &mut IrPool, pos: Position, value: f64, ty: TypeId) -> ExprId {
        pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Literal(Self { value }),
        ))
    }

    /// `floatValue()`: `(SKSL_FLOAT)fValue`.
    // The cast mirrors Skia's `(SKSL_FLOAT)fValue`.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn float_value(self) -> SkslFloat {
        self.value as SkslFloat
    }

    /// `intValue()`: `(SKSL_INT)fValue`.
    // The cast mirrors Skia's `(SKSL_INT)fValue` (values are integral and in range).
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn int_value(self) -> SkslInt {
        self.value as SkslInt
    }

    /// `boolValue()`: `(bool)fValue`.
    #[must_use]
    pub fn bool_value(self) -> bool {
        self.value != 0.0
    }

    /// `description()`: `true`/`false`, the integer, or the float in `skstd::to_string` form
    /// (`1.0`, `0.5`, `1e+10`).
    // Port of: src/sksl/ir/SkSLLiteral.cpp#L13-L21 (chrome/m156)
    #[must_use]
    pub fn description(self, pool: &IrPool, ty: TypeId) -> String {
        let ty = pool.ty(ty);
        if ty.is_boolean() {
            return if self.bool_value() { "true" } else { "false" }.to_owned();
        }
        if ty.is_integer() {
            return self.int_value().to_string();
        }
        skstd::to_string_f32(self.float_value())
    }
}
