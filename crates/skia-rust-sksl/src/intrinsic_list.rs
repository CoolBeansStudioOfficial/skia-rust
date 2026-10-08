// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLIntrinsicList.{h,cpp}.

//! [`IntrinsicKind`]: every intrinsic function `SkSL` supports.

/// Declares [`IntrinsicKind`] from `SKSL_INTRINSIC_LIST`, in Skia's order.
macro_rules! intrinsic_list {
    ($($variant:ident => $name:literal,)*) => {
        /// `SkSL::IntrinsicKind`: one of the intrinsics in `SKSL_INTRINSIC_LIST`, in Skia's order.
        /// Skia's `kNotIntrinsic` is `None` in an `Option<IntrinsicKind>`.
        #[doc(alias = "SkSL::IntrinsicKind")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum IntrinsicKind {
            $(
                #[doc = concat!("`", $name, "`")]
                $variant,
            )*
        }

        impl IntrinsicKind {
            /// Every intrinsic, in `SKSL_INTRINSIC_LIST` order.
            pub const ALL: &'static [IntrinsicKind] = &[$(IntrinsicKind::$variant,)*];

            /// The intrinsic's `SkSL` name (`"abs"`, `"dFdx"`, …).
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(IntrinsicKind::$variant => $name,)*
                }
            }
        }
    };
}

// Port of: src/sksl/SkSLIntrinsicList.h#L16-L125 (chrome/m156)
intrinsic_list! {
    Abs => "abs",
    Acosh => "acosh",
    Acos => "acos",
    All => "all",
    Any => "any",
    Asinh => "asinh",
    Asin => "asin",
    Atanh => "atanh",
    Atan => "atan",
    AtomicAdd => "atomicAdd",
    AtomicLoad => "atomicLoad",
    AtomicMax => "atomicMax",
    AtomicMin => "atomicMin",
    AtomicStore => "atomicStore",
    BitCount => "bitCount",
    Ceil => "ceil",
    Clamp => "clamp",
    Cosh => "cosh",
    Cos => "cos",
    Cross => "cross",
    Degrees => "degrees",
    Determinant => "determinant",
    DFdx => "dFdx",
    DFdy => "dFdy",
    Distance => "distance",
    Dot => "dot",
    Equal => "equal",
    Eval => "eval",
    Exp2 => "exp2",
    Exp => "exp",
    Faceforward => "faceforward",
    FindLsb => "findLSB",
    FindMsb => "findMSB",
    FloatBitsToInt => "floatBitsToInt",
    FloatBitsToUint => "floatBitsToUint",
    Floor => "floor",
    Fma => "fma",
    Fract => "fract",
    Frexp => "frexp",
    FromLinearSrgb => "fromLinearSrgb",
    Fwidth => "fwidth",
    GreaterThanEqual => "greaterThanEqual",
    GreaterThan => "greaterThan",
    IntBitsToFloat => "intBitsToFloat",
    Inversesqrt => "inversesqrt",
    Inverse => "inverse",
    Isinf => "isinf",
    Isnan => "isnan",
    Ldexp => "ldexp",
    Length => "length",
    LessThanEqual => "lessThanEqual",
    LessThan => "lessThan",
    Log2 => "log2",
    Log => "log",
    MatrixCompMult => "matrixCompMult",
    MatrixInverse => "matrixInverse",
    Max => "max",
    Min => "min",
    Mix => "mix",
    Modf => "modf",
    Mod => "mod",
    Normalize => "normalize",
    NotEqual => "notEqual",
    OuterProduct => "outerProduct",
    PackHalf2x16 => "packHalf2x16",
    PackSnorm2x16 => "packSnorm2x16",
    PackSnorm4x8 => "packSnorm4x8",
    PackUnorm2x16 => "packUnorm2x16",
    PackUnorm4x8 => "packUnorm4x8",
    Pow => "pow",
    Radians => "radians",
    Reflect => "reflect",
    Refract => "refract",
    RoundEven => "roundEven",
    Round => "round",
    Sample => "sample",
    SampleGrad => "sampleGrad",
    SampleLod => "sampleLod",
    Saturate => "saturate",
    Sign => "sign",
    Sinh => "sinh",
    Sin => "sin",
    Smoothstep => "smoothstep",
    Sqrt => "sqrt",
    Step => "step",
    StorageBarrier => "storageBarrier",
    SubpassLoad => "subpassLoad",
    Tanh => "tanh",
    Tan => "tan",
    TextureHeight => "textureHeight",
    TextureRead => "textureRead",
    TextureSize => "textureSize",
    TextureWidth => "textureWidth",
    TextureWrite => "textureWrite",
    ToLinearSrgb => "toLinearSrgb",
    Transpose => "transpose",
    Trunc => "trunc",
    UintBitsToFloat => "uintBitsToFloat",
    UnpackHalf2x16 => "unpackHalf2x16",
    UnpackSnorm2x16 => "unpackSnorm2x16",
    UnpackSnorm4x8 => "unpackSnorm4x8",
    UnpackUnorm2x16 => "unpackUnorm2x16",
    UnpackUnorm4x8 => "unpackUnorm4x8",
    WorkgroupBarrier => "workgroupBarrier",
    WorkgroupUniformLoad => "workgroupUniformLoad",
}

/// `FindIntrinsicKind`: the intrinsic named `function_name` (a leading `$` is ignored), or `None`
/// (`kNotIntrinsic`).
// Port of: src/sksl/SkSLIntrinsicList.cpp#L14-L34 (chrome/m156). Skia's `GetIntrinsicMap` is a
// lookup-only `THashMap`; a scan of the fixed list gives the same answers.
#[doc(alias = "FindIntrinsicKind")]
#[must_use]
pub fn find_intrinsic_kind(function_name: &str) -> Option<IntrinsicKind> {
    let name = function_name.strip_prefix('$').unwrap_or(function_name);
    IntrinsicKind::ALL
        .iter()
        .copied()
        .find(|k| k.name() == name)
}

#[cfg(test)]
mod tests {
    use super::{IntrinsicKind, find_intrinsic_kind};

    #[test]
    fn finds_intrinsics_by_name() {
        assert_eq!(IntrinsicKind::ALL.len(), 105);
        assert_eq!(find_intrinsic_kind("abs"), Some(IntrinsicKind::Abs));
        assert_eq!(find_intrinsic_kind("$eval"), Some(IntrinsicKind::Eval));
        assert_eq!(find_intrinsic_kind("dFdx"), Some(IntrinsicKind::DFdx));
        assert_eq!(find_intrinsic_kind("findLSB"), Some(IntrinsicKind::FindLsb));
        assert_eq!(
            find_intrinsic_kind("workgroupUniformLoad"),
            Some(IntrinsicKind::WorkgroupUniformLoad)
        );
        assert_eq!(find_intrinsic_kind("main"), None);
    }
}
