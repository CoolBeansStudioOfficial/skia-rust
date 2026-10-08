// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSetting.{h,cpp} (data, the capability names, `description`,
// `Convert`, `Make` and `toLiteral`).

//! [`Setting`]: `sk_Caps.flag`, a shader capability resolved at compile time.

use super::{Expression, ExpressionKind, Literal, TypeId, ids::ExprId};
use crate::context::Context;
use crate::position::Position;
use crate::program_settings::ProgramConfig;
use crate::util::ShaderCaps;

/// The `ShaderCaps` flag a [`Setting`] reads (Skia's `CapsPtr`, a `bool ShaderCaps::*` member
/// pointer), with the names of `caps_lookup_table`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CapsFlag {
    MustDoOpBetweenFloorAndAbs,
    MustGuardDivisionEvenAfterExplicitZeroCheck,
    Atan2ImplementedAsAtanYOverX,
    FloatIs32Bits,
    IntegerSupport,
    BuiltinDeterminantSupport,
    RewriteMatrixVectorMultiply,
    PerlinNoiseRoundingFix,
}

impl CapsFlag {
    /// Every flag, in `caps_lookup_table` order.
    pub const ALL: [Self; 8] = [
        Self::MustDoOpBetweenFloorAndAbs,
        Self::MustGuardDivisionEvenAfterExplicitZeroCheck,
        Self::Atan2ImplementedAsAtanYOverX,
        Self::FloatIs32Bits,
        Self::IntegerSupport,
        Self::BuiltinDeterminantSupport,
        Self::RewriteMatrixVectorMultiply,
        Self::PerlinNoiseRoundingFix,
    ];

    /// The flag's `sk_Caps` name.
    // Port of: src/sksl/ir/SkSLSetting.cpp#L30-L49 (chrome/m156)
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::MustDoOpBetweenFloorAndAbs => "mustDoOpBetweenFloorAndAbs",
            Self::MustGuardDivisionEvenAfterExplicitZeroCheck => {
                "mustGuardDivisionEvenAfterExplicitZeroCheck"
            }
            Self::Atan2ImplementedAsAtanYOverX => "atan2ImplementedAsAtanYOverX",
            Self::FloatIs32Bits => "floatIs32Bits",
            Self::IntegerSupport => "integerSupport",
            Self::BuiltinDeterminantSupport => "builtinDeterminantSupport",
            Self::RewriteMatrixVectorMultiply => "rewriteMatrixVectorMultiply",
            Self::PerlinNoiseRoundingFix => "PerlinNoiseRoundingFix",
        }
    }

    /// The flag named `name` (`caps_lookup_table().find(name)`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    /// `caps.*fCapsPtr`: the value of this flag in `caps`.
    #[must_use]
    pub fn value_in(self, caps: &ShaderCaps) -> bool {
        match self {
            Self::MustDoOpBetweenFloorAndAbs => caps.must_do_op_between_floor_and_abs,
            Self::MustGuardDivisionEvenAfterExplicitZeroCheck => {
                caps.must_guard_division_even_after_explicit_zero_check
            }
            Self::Atan2ImplementedAsAtanYOverX => caps.atan2_implemented_as_atan_y_over_x,
            Self::FloatIs32Bits => caps.float_is_32_bits,
            Self::IntegerSupport => caps.integer_support,
            Self::BuiltinDeterminantSupport => caps.builtin_determinant_support,
            Self::RewriteMatrixVectorMultiply => caps.rewrite_matrix_vector_multiply,
            Self::PerlinNoiseRoundingFix => caps.perlin_noise_rounding_fix,
        }
    }
}

/// `SkSL::Setting`: its type is `bool`.
// Port of: src/sksl/ir/SkSLSetting.h#L28-L74 (chrome/m156)
#[doc(alias = "SkSL::Setting")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    /// `capsPtr()`.
    pub caps: CapsFlag,
}

impl Setting {
    /// `Convert(context, pos, name)`: `sk_Caps.name`, for a known capability. Reports errors and
    /// returns `None` on failure.
    // Port of: src/sksl/ir/SkSLSetting.cpp#L63-L80 (chrome/m156)
    pub fn convert(ctx: &mut Context, pos: Position, name: &str) -> Option<ExprId> {
        if !ProgramConfig::allows_private_identifiers(ctx.config().kind) {
            ctx.errors.error(pos, "name 'sk_Caps' is reserved");
            return None;
        }
        let Some(caps) = CapsFlag::from_name(name) else {
            ctx.errors
                .error(pos, &format!("unknown capability flag '{name}'"));
            return None;
        };
        Some(Self::make(ctx, pos, caps))
    }

    /// `Make(context, pos, capsPtr)`: a reference to the capability `caps`. Its type is `bool`.
    // Port of: src/sksl/ir/SkSLSetting.cpp#L82-L86 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, caps: CapsFlag) -> ExprId {
        debug_assert!(ProgramConfig::allows_private_identifiers(ctx.config().kind));
        ctx.pool.add_expression(Expression::new(
            pos,
            TypeId::BOOL,
            ExpressionKind::Setting(Self { caps }),
        ))
    }

    /// `toLiteral(caps)`: the value of the capability in `caps`, as a `bool` literal at `pos`.
    /// `ty` is the expression's type (`bool`).
    // Port of: src/sksl/ir/SkSLSetting.cpp#L88-L90 (chrome/m156)
    #[must_use]
    pub fn to_literal(
        self,
        ctx: &mut Context,
        pos: Position,
        ty: TypeId,
        caps: &ShaderCaps,
    ) -> ExprId {
        Literal::make_bool(&mut ctx.pool, pos, self.caps.value_in(caps), ty)
    }

    /// `name()`.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.caps.name()
    }

    /// `description()`: `sk_Caps.name`.
    #[must_use]
    pub fn description(self) -> String {
        format!("sk_Caps.{}", self.name())
    }
}
