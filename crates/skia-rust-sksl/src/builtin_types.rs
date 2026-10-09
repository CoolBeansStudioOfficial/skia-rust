// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLBuiltinTypes.{h,cpp}.

//! The built-in types, with fixed ids.
//!
//! Skia's `BuiltinTypes` is a process-wide object whose fields (`fFloat`, `fHalf4`, …) every
//! compilation shares, so types compare by pointer. Here the same types form a `static` table
//! in the constructor's order, and each field becomes a [`TypeId`] constant: `fTypes.fFloat`
//! is `TypeId::FLOAT`, `fTypes.fIVec2` is `TypeId::IVEC2`, `fTypes.fAtomic_uint` is
//! `TypeId::ATOMIC_UINT_ALIAS`. Every [`IrPool`](crate::ir::IrPool) resolves ids below
//! [`BUILTIN_TYPE_COUNT`] in this table.

use crate::compiler::Compiler;
use crate::ir::{NumberKind, SpvDim, TextureAccess, Type, TypeId, TypeKind};

const POISON_TAG: &str = Compiler::POISON_TAG;

const fn scalar(
    name: &'static str,
    abbrev: &'static str,
    number_kind: NumberKind,
    priority: i8,
    bit_width: i8,
) -> Type {
    Type::make_scalar_type(name, abbrev, number_kind, priority, bit_width)
}

const fn vector(name: &'static str, abbrev: &'static str, component: TypeId, columns: i8) -> Type {
    Type::make_vector_type(name, abbrev, component, columns)
}

const fn matrix(
    name: &'static str,
    abbrev: &'static str,
    component: TypeId,
    columns: i8,
    rows: i8,
) -> Type {
    Type::make_matrix_type(name, abbrev, component, columns, rows)
}

const fn special(name: &'static str, abbrev: &'static str, kind: TypeKind) -> Type {
    Type::make_special_type(name, abbrev, kind)
}

const fn literal(name: &'static str, scalar_type: TypeId, priority: i8) -> Type {
    Type::make_literal_type(name, scalar_type, priority)
}

const fn alias(name: &'static str, target: TypeId, abbrev: &'static str, kind: TypeKind) -> Type {
    Type::make_alias_type(name, target, abbrev, kind)
}

const fn texture(
    name: &'static str,
    dimensions: SpvDim,
    is_depth: bool,
    is_arrayed: bool,
    is_multisampled: bool,
    access: TextureAccess,
) -> Type {
    Type::make_texture_type(
        name,
        dimensions,
        is_depth,
        is_arrayed,
        is_multisampled,
        access,
    )
}

const fn sampler(name: &'static str, texture: TypeId) -> Type {
    Type::make_sampler_type(name, texture)
}

const fn generic(name: &'static str, types: &'static [TypeId], slot_type: TypeId) -> Type {
    Type::make_generic_type(name, types, slot_type)
}

const fn atomic(name: &'static str, abbrev: &'static str) -> Type {
    Type::make_atomic_type(name, abbrev)
}

/// Declares the `TypeId` constants and the table, in the same order.
macro_rules! builtin_types {
    ($($(#[$meta:meta])* $name:ident = $ctor:expr;)*) => {
        /// The table index of each built-in type.
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)] // Named as the constants.
        #[repr(u32)]
        enum Index {
            $($name,)*
            Count,
        }

        impl TypeId {
            $(
                $(#[$meta])*
                pub const $name: TypeId = TypeId(Index::$name as u32);
            )*
        }

        /// How many built-in types there are; ids below this name a built-in type.
        pub const BUILTIN_TYPE_COUNT: u32 = Index::Count as u32;

        /// The built-in types, indexed by their [`TypeId`].
        pub static BUILTIN_TYPES: [Type; BUILTIN_TYPE_COUNT as usize] = [$($ctor,)*];
    };
}

// Port of: src/sksl/SkSLBuiltinTypes.cpp#L22-L219 (chrome/m156)
builtin_types! {
    #[doc(alias = "fFloat")]
    FLOAT = scalar("float", "f", NumberKind::Float, 10, 32);
    #[doc(alias = "fFloat2")]
    FLOAT2 = vector("float2", "f2", TypeId::FLOAT, 2);
    #[doc(alias = "fFloat3")]
    FLOAT3 = vector("float3", "f3", TypeId::FLOAT, 3);
    #[doc(alias = "fFloat4")]
    FLOAT4 = vector("float4", "f4", TypeId::FLOAT, 4);
    #[doc(alias = "fHalf")]
    HALF = scalar("half", "h", NumberKind::Float, 9, 16);
    #[doc(alias = "fHalf2")]
    HALF2 = vector("half2", "h2", TypeId::HALF, 2);
    #[doc(alias = "fHalf3")]
    HALF3 = vector("half3", "h3", TypeId::HALF, 3);
    #[doc(alias = "fHalf4")]
    HALF4 = vector("half4", "h4", TypeId::HALF, 4);
    #[doc(alias = "fInt")]
    INT = scalar("int", "i", NumberKind::Signed, 7, 32);
    #[doc(alias = "fInt2")]
    INT2 = vector("int2", "i2", TypeId::INT, 2);
    #[doc(alias = "fInt3")]
    INT3 = vector("int3", "i3", TypeId::INT, 3);
    #[doc(alias = "fInt4")]
    INT4 = vector("int4", "i4", TypeId::INT, 4);
    #[doc(alias = "fUInt")]
    UINT = scalar("uint", "I", NumberKind::Unsigned, 6, 32);
    #[doc(alias = "fUInt2")]
    UINT2 = vector("uint2", "I2", TypeId::UINT, 2);
    #[doc(alias = "fUInt3")]
    UINT3 = vector("uint3", "I3", TypeId::UINT, 3);
    #[doc(alias = "fUInt4")]
    UINT4 = vector("uint4", "I4", TypeId::UINT, 4);
    #[doc(alias = "fShort")]
    SHORT = scalar("short", "s", NumberKind::Signed, 4, 16);
    #[doc(alias = "fShort2")]
    SHORT2 = vector("short2", "s2", TypeId::SHORT, 2);
    #[doc(alias = "fShort3")]
    SHORT3 = vector("short3", "s3", TypeId::SHORT, 3);
    #[doc(alias = "fShort4")]
    SHORT4 = vector("short4", "s4", TypeId::SHORT, 4);
    #[doc(alias = "fUShort")]
    USHORT = scalar("ushort", "S", NumberKind::Unsigned, 3, 16);
    #[doc(alias = "fUShort2")]
    USHORT2 = vector("ushort2", "S2", TypeId::USHORT, 2);
    #[doc(alias = "fUShort3")]
    USHORT3 = vector("ushort3", "S3", TypeId::USHORT, 3);
    #[doc(alias = "fUShort4")]
    USHORT4 = vector("ushort4", "S4", TypeId::USHORT, 4);
    #[doc(alias = "fBool")]
    BOOL = scalar("bool", "b", NumberKind::Boolean, 0, 1);
    #[doc(alias = "fBool2")]
    BOOL2 = vector("bool2", "b2", TypeId::BOOL, 2);
    #[doc(alias = "fBool3")]
    BOOL3 = vector("bool3", "b3", TypeId::BOOL, 3);
    #[doc(alias = "fBool4")]
    BOOL4 = vector("bool4", "b4", TypeId::BOOL, 4);
    #[doc(alias = "fInvalid")]
    INVALID = special("<INVALID>", "O", TypeKind::Other);
    #[doc(alias = "fPoison")]
    POISON = special(POISON_TAG, "P", TypeKind::Other);
    #[doc(alias = "fVoid")]
    VOID = special("void", "v", TypeKind::Void);
    #[doc(alias = "fFloatLiteral")]
    FLOAT_LITERAL = literal("$floatLiteral", TypeId::FLOAT, 8);
    #[doc(alias = "fIntLiteral")]
    INT_LITERAL = literal("$intLiteral", TypeId::INT, 5);
    #[doc(alias = "fFloat2x2")]
    FLOAT2X2 = matrix("float2x2", "f22", TypeId::FLOAT, 2, 2);
    #[doc(alias = "fFloat2x3")]
    FLOAT2X3 = matrix("float2x3", "f23", TypeId::FLOAT, 2, 3);
    #[doc(alias = "fFloat2x4")]
    FLOAT2X4 = matrix("float2x4", "f24", TypeId::FLOAT, 2, 4);
    #[doc(alias = "fFloat3x2")]
    FLOAT3X2 = matrix("float3x2", "f32", TypeId::FLOAT, 3, 2);
    #[doc(alias = "fFloat3x3")]
    FLOAT3X3 = matrix("float3x3", "f33", TypeId::FLOAT, 3, 3);
    #[doc(alias = "fFloat3x4")]
    FLOAT3X4 = matrix("float3x4", "f34", TypeId::FLOAT, 3, 4);
    #[doc(alias = "fFloat4x2")]
    FLOAT4X2 = matrix("float4x2", "f42", TypeId::FLOAT, 4, 2);
    #[doc(alias = "fFloat4x3")]
    FLOAT4X3 = matrix("float4x3", "f43", TypeId::FLOAT, 4, 3);
    #[doc(alias = "fFloat4x4")]
    FLOAT4X4 = matrix("float4x4", "f44", TypeId::FLOAT, 4, 4);
    #[doc(alias = "fHalf2x2")]
    HALF2X2 = matrix("half2x2", "h22", TypeId::HALF, 2, 2);
    #[doc(alias = "fHalf2x3")]
    HALF2X3 = matrix("half2x3", "h23", TypeId::HALF, 2, 3);
    #[doc(alias = "fHalf2x4")]
    HALF2X4 = matrix("half2x4", "h24", TypeId::HALF, 2, 4);
    #[doc(alias = "fHalf3x2")]
    HALF3X2 = matrix("half3x2", "h32", TypeId::HALF, 3, 2);
    #[doc(alias = "fHalf3x3")]
    HALF3X3 = matrix("half3x3", "h33", TypeId::HALF, 3, 3);
    #[doc(alias = "fHalf3x4")]
    HALF3X4 = matrix("half3x4", "h34", TypeId::HALF, 3, 4);
    #[doc(alias = "fHalf4x2")]
    HALF4X2 = matrix("half4x2", "h42", TypeId::HALF, 4, 2);
    #[doc(alias = "fHalf4x3")]
    HALF4X3 = matrix("half4x3", "h43", TypeId::HALF, 4, 3);
    #[doc(alias = "fHalf4x4")]
    HALF4X4 = matrix("half4x4", "h44", TypeId::HALF, 4, 4);
    #[doc(alias = "fVec2")]
    VEC2 = alias("vec2", TypeId::FLOAT2, "f2", TypeKind::Vector);
    #[doc(alias = "fVec3")]
    VEC3 = alias("vec3", TypeId::FLOAT3, "f3", TypeKind::Vector);
    #[doc(alias = "fVec4")]
    VEC4 = alias("vec4", TypeId::FLOAT4, "f4", TypeKind::Vector);
    #[doc(alias = "fIVec2")]
    IVEC2 = alias("ivec2", TypeId::INT2, "i2", TypeKind::Vector);
    #[doc(alias = "fIVec3")]
    IVEC3 = alias("ivec3", TypeId::INT3, "i3", TypeKind::Vector);
    #[doc(alias = "fIVec4")]
    IVEC4 = alias("ivec4", TypeId::INT4, "i4", TypeKind::Vector);
    #[doc(alias = "fUVec2")]
    UVEC2 = alias("uvec2", TypeId::UINT2, "I2", TypeKind::Vector);
    #[doc(alias = "fUVec3")]
    UVEC3 = alias("uvec3", TypeId::UINT3, "I3", TypeKind::Vector);
    #[doc(alias = "fUVec4")]
    UVEC4 = alias("uvec4", TypeId::UINT4, "I4", TypeKind::Vector);
    #[doc(alias = "fBVec2")]
    BVEC2 = alias("bvec2", TypeId::BOOL2, "b2", TypeKind::Vector);
    #[doc(alias = "fBVec3")]
    BVEC3 = alias("bvec3", TypeId::BOOL3, "b3", TypeKind::Vector);
    #[doc(alias = "fBVec4")]
    BVEC4 = alias("bvec4", TypeId::BOOL4, "b4", TypeKind::Vector);
    #[doc(alias = "fMat2")]
    MAT2 = alias("mat2", TypeId::FLOAT2X2, "f22", TypeKind::Matrix);
    #[doc(alias = "fMat3")]
    MAT3 = alias("mat3", TypeId::FLOAT3X3, "f33", TypeKind::Matrix);
    #[doc(alias = "fMat4")]
    MAT4 = alias("mat4", TypeId::FLOAT4X4, "f44", TypeKind::Matrix);
    #[doc(alias = "fMat2x2")]
    MAT2X2 = alias("mat2x2", TypeId::FLOAT2X2, "f22", TypeKind::Matrix);
    #[doc(alias = "fMat2x3")]
    MAT2X3 = alias("mat2x3", TypeId::FLOAT2X3, "f23", TypeKind::Matrix);
    #[doc(alias = "fMat2x4")]
    MAT2X4 = alias("mat2x4", TypeId::FLOAT2X4, "f24", TypeKind::Matrix);
    #[doc(alias = "fMat3x2")]
    MAT3X2 = alias("mat3x2", TypeId::FLOAT3X2, "f32", TypeKind::Matrix);
    #[doc(alias = "fMat3x3")]
    MAT3X3 = alias("mat3x3", TypeId::FLOAT3X3, "f33", TypeKind::Matrix);
    #[doc(alias = "fMat3x4")]
    MAT3X4 = alias("mat3x4", TypeId::FLOAT3X4, "f34", TypeKind::Matrix);
    #[doc(alias = "fMat4x2")]
    MAT4X2 = alias("mat4x2", TypeId::FLOAT4X2, "f42", TypeKind::Matrix);
    #[doc(alias = "fMat4x3")]
    MAT4X3 = alias("mat4x3", TypeId::FLOAT4X3, "f43", TypeKind::Matrix);
    #[doc(alias = "fMat4x4")]
    MAT4X4 = alias("mat4x4", TypeId::FLOAT4X4, "f44", TypeKind::Matrix);
    #[doc(alias = "fTexture2D_sample")]
    TEXTURE2D_SAMPLE =
        texture("$texture2D_sample", SpvDim::Dim2D, false, false, false, TextureAccess::Sample);
    #[doc(alias = "fTextureExternalOES")]
    TEXTURE_EXTERNAL_OES =
        texture("textureExternalOES", SpvDim::Dim2D, false, false, false, TextureAccess::Sample);
    #[doc(alias = "fTexture2DRect")]
    TEXTURE2D_RECT =
        texture("texture2DRect", SpvDim::Rect, false, false, false, TextureAccess::Sample);
    #[doc(alias = "fTexture2D")]
    TEXTURE2D =
        texture("texture2D", SpvDim::Dim2D, false, false, false, TextureAccess::ReadWrite);
    #[doc(alias = "fReadOnlyTexture2D")]
    READ_ONLY_TEXTURE2D =
        texture("readonlyTexture2D", SpvDim::Dim2D, false, false, false, TextureAccess::Read);
    #[doc(alias = "fWriteOnlyTexture2D")]
    WRITE_ONLY_TEXTURE2D =
        texture("writeonlyTexture2D", SpvDim::Dim2D, false, false, false, TextureAccess::Write);
    #[doc(alias = "fGenTexture2D")]
    GEN_TEXTURE2D = generic(
        "$genTexture2D",
        &[TypeId::READ_ONLY_TEXTURE2D, TypeId::WRITE_ONLY_TEXTURE2D, TypeId::TEXTURE2D],
        TypeId::TEXTURE2D,
    );
    #[doc(alias = "fReadableTexture2D")]
    READABLE_TEXTURE2D = generic(
        "$readableTexture2D",
        &[TypeId::READ_ONLY_TEXTURE2D, TypeId::INVALID, TypeId::TEXTURE2D],
        TypeId::TEXTURE2D,
    );
    #[doc(alias = "fWritableTexture2D")]
    WRITABLE_TEXTURE2D = generic(
        "$writableTexture2D",
        &[TypeId::INVALID, TypeId::WRITE_ONLY_TEXTURE2D, TypeId::TEXTURE2D],
        TypeId::TEXTURE2D,
    );
    #[doc(alias = "fSampler2D")]
    SAMPLER2D = sampler("sampler2D", TypeId::TEXTURE2D_SAMPLE);
    #[doc(alias = "fSamplerExternalOES")]
    SAMPLER_EXTERNAL_OES = sampler("samplerExternalOES", TypeId::TEXTURE_EXTERNAL_OES);
    #[doc(alias = "fSampler2DRect")]
    SAMPLER2D_RECT = sampler("sampler2DRect", TypeId::TEXTURE2D_RECT);
    #[doc(alias = "fSampler")]
    SAMPLER = special("sampler", "ss", TypeKind::SeparateSampler);
    #[doc(alias = "fSubpassInput")]
    SUBPASS_INPUT =
        texture("subpassInput", SpvDim::SubpassData, false, false, false, TextureAccess::Read);
    #[doc(alias = "fSubpassInputMS")]
    SUBPASS_INPUT_MS =
        texture("subpassInputMS", SpvDim::SubpassData, false, false, true, TextureAccess::Read);
    #[doc(alias = "fGenType")]
    GEN_TYPE = generic(
        "$genType",
        &[TypeId::FLOAT, TypeId::FLOAT2, TypeId::FLOAT3, TypeId::FLOAT4],
        TypeId::FLOAT,
    );
    #[doc(alias = "fGenHType")]
    GEN_HTYPE = generic(
        "$genHType",
        &[TypeId::HALF, TypeId::HALF2, TypeId::HALF3, TypeId::HALF4],
        TypeId::HALF,
    );
    #[doc(alias = "fGenIType")]
    GEN_ITYPE = generic(
        "$genIType",
        &[TypeId::INT, TypeId::INT2, TypeId::INT3, TypeId::INT4],
        TypeId::INT,
    );
    #[doc(alias = "fGenUType")]
    GEN_UTYPE = generic(
        "$genUType",
        &[TypeId::UINT, TypeId::UINT2, TypeId::UINT3, TypeId::UINT4],
        TypeId::UINT,
    );
    #[doc(alias = "fGenBType")]
    GEN_BTYPE = generic(
        "$genBType",
        &[TypeId::BOOL, TypeId::BOOL2, TypeId::BOOL3, TypeId::BOOL4],
        TypeId::BOOL,
    );
    #[doc(alias = "fMat")]
    MAT = generic(
        "$mat",
        &[
            TypeId::FLOAT2X2, TypeId::FLOAT2X3, TypeId::FLOAT2X4,
            TypeId::FLOAT3X2, TypeId::FLOAT3X3, TypeId::FLOAT3X4,
            TypeId::FLOAT4X2, TypeId::FLOAT4X3, TypeId::FLOAT4X4,
        ],
        TypeId::FLOAT,
    );
    #[doc(alias = "fHMat")]
    HMAT = generic(
        "$hmat",
        &[
            TypeId::HALF2X2, TypeId::HALF2X3, TypeId::HALF2X4,
            TypeId::HALF3X2, TypeId::HALF3X3, TypeId::HALF3X4,
            TypeId::HALF4X2, TypeId::HALF4X3, TypeId::HALF4X4,
        ],
        TypeId::HALF,
    );
    #[doc(alias = "fSquareMat")]
    SQUARE_MAT = generic(
        "$squareMat",
        &[TypeId::INVALID, TypeId::FLOAT2X2, TypeId::FLOAT3X3, TypeId::FLOAT4X4],
        TypeId::FLOAT,
    );
    #[doc(alias = "fSquareHMat")]
    SQUARE_HMAT = generic(
        "$squareHMat",
        &[TypeId::INVALID, TypeId::HALF2X2, TypeId::HALF3X3, TypeId::HALF4X4],
        TypeId::HALF,
    );
    #[doc(alias = "fVec")]
    VEC = generic(
        "$vec",
        &[TypeId::INVALID, TypeId::FLOAT2, TypeId::FLOAT3, TypeId::FLOAT4],
        TypeId::FLOAT,
    );
    #[doc(alias = "fHVec")]
    HVEC = generic(
        "$hvec",
        &[TypeId::INVALID, TypeId::HALF2, TypeId::HALF3, TypeId::HALF4],
        TypeId::HALF,
    );
    #[doc(alias = "fIVec")]
    IVEC = generic(
        "$ivec",
        &[TypeId::INVALID, TypeId::INT2, TypeId::INT3, TypeId::INT4],
        TypeId::INT,
    );
    #[doc(alias = "fUVec")]
    UVEC = generic(
        "$uvec",
        &[TypeId::INVALID, TypeId::UINT2, TypeId::UINT3, TypeId::UINT4],
        TypeId::UINT,
    );
    #[doc(alias = "fSVec")]
    SVEC = generic(
        "$svec",
        &[TypeId::INVALID, TypeId::SHORT2, TypeId::SHORT3, TypeId::SHORT4],
        TypeId::SHORT,
    );
    #[doc(alias = "fUSVec")]
    USVEC = generic(
        "$usvec",
        &[TypeId::INVALID, TypeId::USHORT2, TypeId::USHORT3, TypeId::USHORT4],
        TypeId::USHORT,
    );
    #[doc(alias = "fBVec")]
    BVEC = generic(
        "$bvec",
        &[TypeId::INVALID, TypeId::BOOL2, TypeId::BOOL3, TypeId::BOOL4],
        TypeId::BOOL,
    );
    #[doc(alias = "fSkCaps")]
    SK_CAPS = special("$sk_Caps", "O", TypeKind::Other);
    #[doc(alias = "fColorFilter")]
    COLOR_FILTER = special("colorFilter", "CF", TypeKind::ColorFilter);
    #[doc(alias = "fShader")]
    SHADER = special("shader", "SH", TypeKind::Shader);
    #[doc(alias = "fBlender")]
    BLENDER = special("blender", "B", TypeKind::Blender);
    #[doc(alias = "fAtomicUInt")]
    ATOMIC_UINT = atomic("atomicUint", "au");
    #[doc(alias = "fAtomic_uint")]
    ATOMIC_UINT_ALIAS = alias("atomic_uint", TypeId::ATOMIC_UINT, "au", TypeKind::Atomic);
}

#[cfg(test)]
mod tests {
    use super::{BUILTIN_TYPE_COUNT, BUILTIN_TYPES};
    use crate::ir::{IrPool, TypeClass, TypeId};

    #[test]
    fn table_order_matches_the_constants() {
        assert_eq!(BUILTIN_TYPE_COUNT, 112);
        let pool = IrPool::new();
        let named = [
            (TypeId::FLOAT, "float"),
            (TypeId::HALF4, "half4"),
            (TypeId::UINT3, "uint3"),
            (TypeId::USHORT, "ushort"),
            (TypeId::INVALID, "<INVALID>"),
            (TypeId::INT_LITERAL, "$intLiteral"),
            (TypeId::HALF4X4, "half4x4"),
            (TypeId::MAT2X4, "mat2x4"),
            (TypeId::SAMPLER, "sampler"),
            (TypeId::GEN_HTYPE, "$genHType"),
            (TypeId::USVEC, "$usvec"),
            (TypeId::SK_CAPS, "$sk_Caps"),
            (TypeId::ATOMIC_UINT_ALIAS, "atomic_uint"),
        ];
        for (id, name) in named {
            assert_eq!(pool.ty(id).name(), name);
        }
    }

    #[test]
    fn aliases_copy_their_targets_kind_and_abbreviation() {
        let pool = IrPool::new();
        for ty in &BUILTIN_TYPES {
            if let TypeClass::Alias { target } = ty.class {
                let target = pool.type_node(target);
                assert_eq!(ty.type_kind, target.type_kind, "{}", ty.name);
                assert_eq!(ty.abbreviated_name, target.abbreviated_name, "{}", ty.name);
            }
        }
    }
}
