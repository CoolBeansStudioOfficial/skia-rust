// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSetting.{h,cpp} (data, the capability names and
// `description`). `Convert`, `Make` and `toLiteral` (which read `ShaderCaps`) come with task S7b.

//! [`Setting`]: `sk_Caps.flag`, a shader capability resolved at compile time.

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
