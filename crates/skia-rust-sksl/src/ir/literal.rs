// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLLiteral.{h,cpp} (data, accessors and `description`). The
// `Make*` factories, `compareConstant` and `getConstantValue` come with task S7a.

//! [`Literal`]: a float, integer or boolean constant.

use super::{IrPool, ids::TypeId};
use crate::defines::{SkslFloat, SkslInt};
use crate::skstd;

/// `SkSL::Literal`. The value is stored as a `double` whatever the type.
// Port of: src/sksl/ir/SkSLLiteral.h#L28-L131 (chrome/m156)
#[doc(alias = "SkSL::Literal")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Literal {
    /// `value()`.
    pub value: f64,
}

impl Literal {
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
