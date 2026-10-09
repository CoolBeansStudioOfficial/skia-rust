// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLMemoryLayoutTest.cpp (chrome/m156)

//! The memory layout rules (std140, std430 and the WGSL layouts) against the sizes, alignments
//! and strides Skia's test expects.
//!
//! Skia's `BuiltinTypes` object is the static table in `skia_rust_sksl::builtin_types`, so
//! there is nothing to construct for it; `context.fTypes.fX` is `TypeId::X`. A `unique_ptr<Type>`
//! is a `Type` added to the context's pool, named by its `TypeId`.
//!
//! Skia fills each `TArray` with `emplace_back` calls in turn, so the port pushes the same way: the
//! `vec_init_then_push` allows keep that order.

use skia_rust_sksl::context::Context;
use skia_rust_sksl::error_reporter::ErrorReporter;
use skia_rust_sksl::ir::{Field, Layout, ModifierFlags, Type, TypeId};
use skia_rust_sksl::memory_layout::{MemoryLayout, Standard};
use skia_rust_sksl::modules::ModuleType;
use skia_rust_sksl::position::Position;
use skia_rust_sksl::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

use crate::{def_test, reporter_assert};

// Port of: tests/SkSLMemoryLayoutTest.cpp#L28-L125 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_std140,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::Std140);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // basic types
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 1 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 2 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 3 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::BOOL4)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 1 == layout.alignment(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 2 == layout.alignment(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::BOOL4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // struct 1
        let mut fields1: Vec<Field> = Vec::new();
        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT3,
        });
        let s1 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s1",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(s1)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s1)));

        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT,
        });
        let s2 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s2",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(s2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s2)));

        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::BOOL,
        });
        let s3 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s3",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(s3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s3)));

        // struct 2
        let mut fields2: Vec<Field> = Vec::new();
        fields2.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::INT,
        });
        let s4 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s4",
                fields2.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(s4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s4)));

        fields2.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        let s5 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s5",
                fields2.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(s5)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s5)));

        // arrays
        let array1 = {
            let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 64 == layout.size(context.pool.ty(array1)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array1)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(array1)));

        let array2 = {
            let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 64 == layout.size(context.pool.ty(array2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array2)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(array2)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L127-L224 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_std430,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::Std430);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // basic types
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 1 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 2 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 3 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::BOOL4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 1 == layout.alignment(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 2 == layout.alignment(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::BOOL4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // struct 1
        let mut fields1: Vec<Field> = Vec::new();
        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT3,
        });
        let s1 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s1",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(s1)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s1)));

        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT,
        });
        let s2 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s2",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(s2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s2)));

        fields1.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::BOOL,
        });
        let s3 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s3",
                fields1.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(s3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s3)));

        // struct 2
        let mut fields2: Vec<Field> = Vec::new();
        fields2.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::INT,
        });
        let s4 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s4",
                fields2.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 4 == layout.size(context.pool.ty(s4)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(s4)));

        fields2.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        let s5 = {
            let ty = Type::make_struct_type(
                &mut context,
                Position::default(),
                "s5",
                fields2.clone(),
                false,
            );
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(s5)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(s5)));

        // arrays
        let array1 = {
            let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 16 == layout.size(context.pool.ty(array1)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(array1)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(array1)));

        let array2 = {
            let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 64 == layout.size(context.pool.ty(array2)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array2)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(array2)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L226-L516 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLUniform_Base,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::WgslUniformBase);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // The values here are taken from https://www.w3.org/TR/WGSL/#alignment-and-size, table titled
        // "Alignment and size for host-shareable types". WGSL does not have an i16 type, so short and
        // unsigned-short integer types are treated as full-size integers in WGSL.

        // scalars (i32, u32, f32, f16)
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::HALF)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF)));

        // vec2<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2)));

        // vec3<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::HALF3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3)));

        // vec4<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4)));

        // mat2x2<f32>, mat2x2<f16>
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X2)));

        // mat3x2<f32>, mat3x2<f16>
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X2)));

        // mat4x2<f32>, mat4x2<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X2)));

        // mat2x3<f32>, mat2x3<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF2X3)));

        // mat3x3<f32>, mat3x3<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF3X3)));

        // mat4x3<f32>, mat4x3<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF4X3)));

        // mat2x4<f32>, mat2x4<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF2X4)));

        // mat3x4<f32>, mat3x4<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF3X4)));

        // mat4x4<f32>, mat4x4<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF4X4)));

        // atomic<u32>
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // bool is not a host-shareable type and returns 0 for WGSL.
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL4)));

        // Arrays
        // array<f32, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<f16, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "half[4]", TypeId::HALF, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec2<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float2[4]", TypeId::FLOAT2, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float3[4]", TypeId::FLOAT3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec4<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<mat3x3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "mat3[4]", TypeId::FLOAT3X3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 192 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 48 == layout.stride(context.pool.ty(array)));
        }

        // Structs A and B from example in https://www.w3.org/TR/WGSL/#structure-member-layout, with
        // offsets adjusted for uniform address space constraints.
        //
        // struct A {        //            align(roundUp(16, 8))  size(roundUp(16, 24))
        //     u: f32,       // offset(0)  align(4)               size(4)
        //     v: f32,       // offset(4)  align(4)               size(4)
        //     w: vec2<f32>, // offset(8)  align(8)               size(8)
        //     x: f32        // offset(16) align(4)               size(4)
        //     // padding    // offset(20)                        size(12)
        // }
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "u".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "v".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "w".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "x".into(),
            ty: TypeId::FLOAT,
        });
        let structA = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "A", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(structA)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structA)));
        fields = Vec::new();

        // struct B {          //             align(16) size(208)
        //     a: vec2<f32>,   // offset(0)   align(8)  size(8)
        //     // padding      // offset(8)             size(8)
        //     b: vec3<f32>,   // offset(16)  align(16) size(12)
        //     c: f32,         // offset(28)  align(4)  size(4)
        //     d: f32,         // offset(32)  align(4)  size(4)
        //     // padding      // offset(36)            size(12)
        //     e: A,           // offset(48)  align(16) size(32)
        //     f: vec3<f32>,   // offset(80)  align(16) size(12)
        //     // padding      // offset(92)            size(4)
        //     g: array<A, 3>, // offset(96)  align(16) size(96)
        //     h: i32          // offset(192) align(4)  size(4)
        //     // padding      // offset(196)           size(12)
        // }
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "d".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "e".into(),
            ty: structA,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "f".into(),
            ty: TypeId::FLOAT3,
        });
        let array = {
            let ty = Type::make_array_type(&context, "A[3]", structA, 3);
            context.pool.add_type(ty)
        };
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "g".into(),
            ty: array,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "h".into(),
            ty: TypeId::INT,
        });
        let structB = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "B", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 208 == layout.size(context.pool.ty(structB)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structB)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L518-L808 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLUniform_EnableF16,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::WgslUniformEnableF16);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // The values here are taken from https://www.w3.org/TR/WGSL/#alignment-and-size, table titled
        // "Alignment and size for host-shareable types". WGSL does not have an i16 type, so short and
        // unsigned-short integer types are treated as full-size integers in WGSL.

        // scalars (i32, u32, f32, f16)
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 2 == layout.size(context.pool.ty(TypeId::HALF)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 2 == layout.alignment(context.pool.ty(TypeId::HALF)));

        // vec2<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::HALF2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF2)));

        // vec3<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 6 == layout.size(context.pool.ty(TypeId::HALF3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3)));

        // vec4<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4)));

        // mat2x2<f32>, mat2x2<f16>
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF2X2)));

        // mat3x2<f32>, mat3x2<f16>
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF3X2)));

        // mat4x2<f32>, mat4x2<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF4X2)));

        // mat2x3<f32>, mat2x3<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X3)));

        // mat3x3<f32>, mat3x3<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X3)));

        // mat4x3<f32>, mat4x3<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X3)));

        // mat2x4<f32>, mat2x4<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X4)));

        // mat3x4<f32>, mat3x4<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X4)));

        // mat4x4<f32>, mat4x4<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X4)));

        // atomic<u32>
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // bool is not a host-shareable type and returns 0 for WGSL.
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL4)));

        // Arrays
        // array<f32, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<f16, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "half[4]", TypeId::HALF, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec2<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float2[4]", TypeId::FLOAT2, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float3[4]", TypeId::FLOAT3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec4<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<mat3x3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "mat3[4]", TypeId::FLOAT3X3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 192 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 48 == layout.stride(context.pool.ty(array)));
        }

        // Structs A and B from example in https://www.w3.org/TR/WGSL/#structure-member-layout, with
        // offsets adjusted for uniform address space constraints.
        //
        // struct A {        //            align(roundUp(16, 8))  size(roundUp(16, 24))
        //     u: f32,       // offset(0)  align(4)               size(4)
        //     v: f32,       // offset(4)  align(4)               size(4)
        //     w: vec2<f32>, // offset(8)  align(8)               size(8)
        //     x: f32        // offset(16) align(4)               size(4)
        //     // padding    // offset(20)                        size(12)
        // }
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "u".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "v".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "w".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "x".into(),
            ty: TypeId::FLOAT,
        });
        let structA = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "A", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 32 == layout.size(context.pool.ty(structA)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structA)));
        fields = Vec::new();

        // struct B {          //             align(16) size(208)
        //     a: vec2<f32>,   // offset(0)   align(8)  size(8)
        //     // padding      // offset(8)             size(8)
        //     b: vec3<f32>,   // offset(16)  align(16) size(12)
        //     c: f32,         // offset(28)  align(4)  size(4)
        //     d: f32,         // offset(32)  align(4)  size(4)
        //     // padding      // offset(36)            size(12)
        //     e: A,           // offset(48)  align(16) size(32)
        //     f: vec3<f32>,   // offset(80)  align(16) size(12)
        //     // padding      // offset(92)            size(4)
        //     g: array<A, 3>, // offset(96)  align(16) size(96)
        //     h: i32          // offset(192) align(4)  size(4)
        //     // padding      // offset(196)           size(12)
        // }
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "d".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "e".into(),
            ty: structA,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "f".into(),
            ty: TypeId::FLOAT3,
        });
        let array = {
            let ty = Type::make_array_type(&context, "A[3]", structA, 3);
            context.pool.add_type(ty)
        };
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "g".into(),
            ty: array,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "h".into(),
            ty: TypeId::INT,
        });
        let structB = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "B", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 208 == layout.size(context.pool.ty(structB)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structB)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L810-L1098 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLStorage_Base,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::WgslStorageBase);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // The values here are taken from https://www.w3.org/TR/WGSL/#alignment-and-size, table titled
        // "Alignment and size for host-shareable types".

        // scalars (i32, u32, f32, f16)
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::HALF)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF)));

        // vec2<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2)));

        // vec3<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::HALF3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3)));

        // vec4<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4)));

        // mat2x2<f32>, mat2x2<f16>
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X2)));

        // mat3x2<f32>, mat3x2<f16>
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X2)));

        // mat4x2<f32>, mat4x2<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X2)));

        // mat2x3<f32>, mat2x3<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF2X3)));

        // mat3x3<f32>, mat3x3<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF3X3)));

        // mat4x3<f32>, mat4x3<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF4X3)));

        // mat2x4<f32>, mat2x4<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF2X4)));

        // mat3x4<f32>, mat3x4<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF3X4)));

        // mat4x4<f32>, mat4x4<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::HALF4X4)));

        // atomic<u32>
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // bool is not a host-shareable type and returns 0 for WGSL.
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL4)));

        // Arrays
        // array<f32, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 16 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.stride(context.pool.ty(array)));
        }
        // array<f16, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "half[4]", TypeId::HALF, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 16 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.stride(context.pool.ty(array)));
        }
        // array<vec2<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float2[4]", TypeId::FLOAT2, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 32 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 8 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 8 == layout.stride(context.pool.ty(array)));
        }
        // array<vec3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float3[4]", TypeId::FLOAT3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec4<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<mat3x3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "mat3[4]", TypeId::FLOAT3X3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 192 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 48 == layout.stride(context.pool.ty(array)));
        }

        // Structs A and B from example in https://www.w3.org/TR/WGSL/#structure-member-layout
        //
        // struct A {        //            align(8)               size(24)
        //     u: f32,       // offset(0)  align(4)               size(4)
        //     v: f32,       // offset(4)  align(4)               size(4)
        //     w: vec2<f32>, // offset(8)  align(8)               size(8)
        //     x: f32        // offset(16) align(4)               size(4)
        //     // padding    // offset(20)                        size(4)
        // }
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "u".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "v".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "w".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "x".into(),
            ty: TypeId::FLOAT,
        });
        let structA = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "A", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 24 == layout.size(context.pool.ty(structA)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(structA)));
        fields = Vec::new();

        // struct B {          //             align(16) size(160)
        //     a: vec2<f32>,   // offset(0)   align(8)  size(8)
        //     // padding      // offset(8)             size(8)
        //     b: vec3<f32>,   // offset(16)  align(16) size(12)
        //     c: f32,         // offset(28)  align(4)  size(4)
        //     d: f32,         // offset(32)  align(4)  size(4)
        //     // padding      // offset(36)            size(4)
        //     e: A,           // offset(40)  align(8)  size(24)
        //     f: vec3<f32>,   // offset(64)  align(16) size(12)
        //     // padding      // offset(76)            size(4)
        //     g: array<A, 3>, // offset(80)  align(16) size(72)
        //     h: i32          // offset(152) align(4)  size(4)
        //     // padding      // offset(156)           size(4)
        // }
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "d".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "e".into(),
            ty: structA,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "f".into(),
            ty: TypeId::FLOAT3,
        });
        let array = {
            let ty = Type::make_array_type(&context, "A[3]", structA, 3);
            context.pool.add_type(ty)
        };
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "g".into(),
            ty: array,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "h".into(),
            ty: TypeId::INT,
        });
        let structB = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "B", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 160 == layout.size(context.pool.ty(structB)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structB)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L1100-L1388 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLStorage_EnableF16,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        let layout = MemoryLayout::new(Standard::WgslStorageEnableF16);
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        // The values here are taken from https://www.w3.org/TR/WGSL/#alignment-and-size, table titled
        // "Alignment and size for host-shareable types".

        // scalars (i32, u32, f32, f16)
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 2 == layout.size(context.pool.ty(TypeId::HALF)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, 2 == layout.alignment(context.pool.ty(TypeId::HALF)));

        // vec2<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::HALF2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF2)));

        // vec3<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 6 == layout.size(context.pool.ty(TypeId::HALF3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3)));

        // vec4<T>, T: i32, u32, f32, f16
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4)));

        // mat2x2<f32>, mat2x2<f16>
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 8 == layout.size(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF2X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF2X2)));

        // mat3x2<f32>, mat3x2<f16>
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 12 == layout.size(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF3X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF3X2)));

        // mat4x2<f32>, mat4x2<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 4 == layout.alignment(context.pool.ty(TypeId::HALF4X2)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, 4 == layout.stride(context.pool.ty(TypeId::HALF4X2)));

        // mat2x3<f32>, mat2x3<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X3)));

        // mat3x3<f32>, mat3x3<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X3)));

        // mat4x3<f32>, mat4x3<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X3)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X3)));

        // mat2x4<f32>, mat2x4<f16>
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 16 == layout.size(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF2X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF2X4)));

        // mat3x4<f32>, mat3x4<f16>
        reporter_assert!(r, 48 == layout.size(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 24 == layout.size(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF3X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF3X4)));

        // mat4x4<f32>, mat4x4<f16>
        reporter_assert!(r, 64 == layout.size(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 32 == layout.size(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(TypeId::HALF4X4)));
        reporter_assert!(r, 16 == layout.stride(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, 8 == layout.stride(context.pool.ty(TypeId::HALF4X4)));

        // atomic<u32>
        reporter_assert!(r, 4 == layout.size(context.pool.ty(TypeId::ATOMIC_UINT)));
        reporter_assert!(
            r,
            4 == layout.alignment(context.pool.ty(TypeId::ATOMIC_UINT))
        );

        // bool is not a host-shareable type and returns 0 for WGSL.
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, 0 == layout.size(context.pool.ty(TypeId::BOOL4)));

        // Arrays
        // array<f32, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float[4]", TypeId::FLOAT, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 16 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 4 == layout.stride(context.pool.ty(array)));
        }
        // array<f16, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "half[4]", TypeId::HALF, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 8 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 2 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 2 == layout.stride(context.pool.ty(array)));
        }
        // array<vec2<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float2[4]", TypeId::FLOAT2, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 32 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 8 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 8 == layout.stride(context.pool.ty(array)));
        }
        // array<vec3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float3[4]", TypeId::FLOAT3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<vec4<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "float4[4]", TypeId::FLOAT4, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 64 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.stride(context.pool.ty(array)));
        }
        // array<mat3x3<f32>, 4>
        {
            let array = {
                let ty = Type::make_array_type(&context, "mat3[4]", TypeId::FLOAT3X3, 4);
                context.pool.add_type(ty)
            };
            reporter_assert!(r, 192 == layout.size(context.pool.ty(array)));
            reporter_assert!(r, 16 == layout.alignment(context.pool.ty(array)));
            reporter_assert!(r, 48 == layout.stride(context.pool.ty(array)));
        }

        // Structs A and B from example in https://www.w3.org/TR/WGSL/#structure-member-layout
        //
        // struct A {        //            align(8)               size(24)
        //     u: f32,       // offset(0)  align(4)               size(4)
        //     v: f32,       // offset(4)  align(4)               size(4)
        //     w: vec2<f32>, // offset(8)  align(8)               size(8)
        //     x: f32        // offset(16) align(4)               size(4)
        //     // padding    // offset(20)                        size(4)
        // }
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "u".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "v".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "w".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "x".into(),
            ty: TypeId::FLOAT,
        });
        let structA = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "A", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 24 == layout.size(context.pool.ty(structA)));
        reporter_assert!(r, 8 == layout.alignment(context.pool.ty(structA)));
        fields = Vec::new();

        // struct B {          //             align(16) size(160)
        //     a: vec2<f32>,   // offset(0)   align(8)  size(8)
        //     // padding      // offset(8)             size(8)
        //     b: vec3<f32>,   // offset(16)  align(16) size(12)
        //     c: f32,         // offset(28)  align(4)  size(4)
        //     d: f32,         // offset(32)  align(4)  size(4)
        //     // padding      // offset(36)            size(4)
        //     e: A,           // offset(40)  align(8)  size(24)
        //     f: vec3<f32>,   // offset(64)  align(16) size(12)
        //     // padding      // offset(76)            size(4)
        //     g: array<A, 3>, // offset(80)  align(16) size(72)
        //     h: i32          // offset(152) align(4)  size(4)
        //     // padding      // offset(156)           size(4)
        // }
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT2,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::FLOAT3,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "c".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "d".into(),
            ty: TypeId::FLOAT,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "e".into(),
            ty: structA,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "f".into(),
            ty: TypeId::FLOAT3,
        });
        let array = {
            let ty = Type::make_array_type(&context, "A[3]", structA, 3);
            context.pool.add_type(ty)
        };
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "g".into(),
            ty: array,
        });
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "h".into(),
            ty: TypeId::INT,
        });
        let structB = {
            let ty = Type::make_struct_type(&mut context, Position::default(), "B", fields, false);
            context.pool.add_type(ty)
        };
        reporter_assert!(r, 160 == layout.size(context.pool.ty(structB)));
        reporter_assert!(r, 16 == layout.alignment(context.pool.ty(structB)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L1390-L1415 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLUnsupportedTypes,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        let testArray = {
            let ty = Type::make_array_type(&context, "bool[3]", TypeId::BOOL, 3);
            context.pool.add_type(ty)
        };

        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "foo".into(),
            ty: testArray,
        });
        let testStruct = {
            let ty =
                Type::make_struct_type(&mut context, Position::default(), "Test", fields, false);
            context.pool.add_type(ty)
        };

        let layout = MemoryLayout::new(Standard::WgslUniformEnableF16);
        reporter_assert!(r, !layout.is_supported(context.pool.ty(TypeId::BOOL)));
        reporter_assert!(r, !layout.is_supported(context.pool.ty(TypeId::BOOL2)));
        reporter_assert!(r, !layout.is_supported(context.pool.ty(TypeId::BOOL3)));
        reporter_assert!(r, !layout.is_supported(context.pool.ty(TypeId::BOOL4)));
        reporter_assert!(r, !layout.is_supported(context.pool.ty(testArray)));
        reporter_assert!(r, !layout.is_supported(context.pool.ty(testStruct)));
    }
);

// Port of: tests/SkSLMemoryLayoutTest.cpp#L1417-L1511 (chrome/m156)
def_test!(
    #[allow(clippy::vec_init_then_push)]
    SkSLMemoryLayoutTest_WGSLSupportedTypes,
    |r| {
        let mut context = Context::new(ErrorReporter::testing_only_abort());
        context.config = Some(ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::Fragment,
            ProgramSettings::default(),
        ));

        let testArray = {
            let ty = Type::make_array_type(&context, "float[3]", TypeId::FLOAT, 3);
            context.pool.add_type(ty)
        };

        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "foo".into(),
            ty: testArray,
        });
        let testStruct = {
            let ty =
                Type::make_struct_type(&mut context, Position::default(), "Test", fields, false);
            context.pool.add_type(ty)
        };

        let layout = MemoryLayout::new(Standard::WgslUniformEnableF16);

        // scalars (i32, u32, f32, f16)
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::INT)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::UINT)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::SHORT)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::USHORT)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF)));

        // vec2<T>, T: i32, u32, f32, f16
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::INT2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::UINT2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::SHORT2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::USHORT2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF2)));

        // vec3<T>, T: i32, u32, f32, f16
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::INT3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::UINT3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::SHORT3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::USHORT3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF3)));

        // vec4<T>, T: i32, u32, f32, f16
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::INT4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::UINT4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::SHORT4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::USHORT4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF4)));

        // mat2x2<f32>, mat2x2<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT2X2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF2X2)));

        // mat3x2<f32>, mat3x2<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT3X2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF3X2)));

        // mat4x2<f32>, mat4x2<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT4X2)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF4X2)));

        // mat2x3<f32>, mat2x3<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT2X3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF2X3)));

        // mat3x3<f32>, mat3x3<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT3X3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF3X3)));

        // mat4x3<f32>, mat4x3<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT4X3)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF4X3)));

        // mat2x4<f32>, mat2x4<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT2X4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF2X4)));

        // mat3x4<f32>, mat3x4<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT3X4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF3X4)));

        // mat4x4<f32>, mat4x4<f16>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::FLOAT4X4)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::HALF4X4)));

        // atomic<u32>
        reporter_assert!(r, layout.is_supported(context.pool.ty(TypeId::ATOMIC_UINT)));

        // arrays and structs
        reporter_assert!(r, layout.is_supported(context.pool.ty(testArray)));
        reporter_assert!(r, layout.is_supported(context.pool.ty(testStruct)));
    }
);
