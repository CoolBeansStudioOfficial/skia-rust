// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::uniform::K_NON_ARRAY;
use skia_rust_gpu::graphite::uniform_manager::UniformOffsetCalculator;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{def_test, reporter_assert};

// Used to test the exact alignment and size of an individual type. Returns the alignment and size
// as a pair.
// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L13-L23 (chrome/m156)
fn calculate_alignment_and_size(layout: Layout, ty: SkSLType, array_count: i32) -> (i32, i32) {
    // Set the start offset at 1 to force alignment.
    const K_START: i32 = 1;
    let mut calc = UniformOffsetCalculator::for_top_level(layout, K_START);
    let alignment = calc.advance_offset(ty, array_count);
    (alignment, calc.size() - alignment)
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L25-L41 (chrome/m156)
fn calculate_struct_alignment_and_size(
    layout: Layout,
    fields: &[SkSLType],
    array_count: i32,
) -> (i32, i32) {
    // Set the start offset at 1 to force alignment.
    const K_START: i32 = 1;
    let mut outer = UniformOffsetCalculator::for_top_level(layout, K_START);

    let mut substruct = UniformOffsetCalculator::for_struct(layout);
    for &f in fields {
        substruct.advance_offset(f, K_NON_ARRAY);
    }

    let alignment = outer.advance_struct(&substruct, array_count);
    debug_assert_eq!(alignment, substruct.required_alignment());
    (alignment, outer.size() - alignment)
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L43-L58 (chrome/m156), `EXPECT`
macro_rules! expect {
    ($r:expr, $layout:expr, $type:expr, $expected_alignment:expr, $expected_size:expr $(,)?) => {{
        let (alignment, size) = calculate_alignment_and_size($layout, $type, K_NON_ARRAY);
        reporter_assert!(
            $r,
            alignment == $expected_alignment,
            "incorrect alignment for type '{}': expected {}, found {}",
            $type.as_str(),
            $expected_alignment,
            alignment
        );
        reporter_assert!(
            $r,
            size == $expected_size,
            "incorrect size for type '{}': expected {}, found {}",
            $type.as_str(),
            $expected_size,
            size
        );
    }};
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L60-L80 (chrome/m156), `EXPECT_ARRAY`
macro_rules! expect_array {
    (
        $r:expr,
        $layout:expr,
        $count:expr,
        $type:expr,
        $expected_alignment:expr,
        $expected_stride:expr,
        $expected_size:expr $(,)?
    ) => {{
        let (alignment, size) = calculate_alignment_and_size($layout, $type, $count);
        let stride = size / $count;
        reporter_assert!(
            $r,
            alignment == $expected_alignment,
            "incorrect alignment for type '{}': expected {}, found {}",
            $type.as_str(),
            $expected_alignment,
            alignment
        );
        reporter_assert!(
            $r,
            size == $expected_size,
            "incorrect size for type '{}': expected {}, found {}",
            $type.as_str(),
            $expected_size,
            size
        );
        reporter_assert!(
            $r,
            stride == $expected_stride,
            "incorrect stride for type '{}': expected {}, found {}",
            $type.as_str(),
            $expected_stride,
            stride
        );
    }};
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L82-L101 (chrome/m156), `EXPECT_STRUCT`
macro_rules! expect_struct {
    (
        $r:expr,
        $layout:expr,
        $expected_alignment:expr,
        $expected_size:expr,
        [$($field:expr),* $(,)?] $(,)?
    ) => {{
        let (alignment, size) =
            calculate_struct_alignment_and_size($layout, &[$($field),*], K_NON_ARRAY);
        reporter_assert!(
            $r,
            alignment == $expected_alignment,
            "incorrect alignment for struct: expected {}, found {}",
            $expected_alignment,
            alignment
        );
        reporter_assert!(
            $r,
            size == $expected_size,
            "incorrect size for struct: expected {}, found {}",
            $expected_size,
            size
        );
        reporter_assert!(
            $r,
            size % alignment == 0,
            "struct size must be a multiple of alignment"
        );
    }};
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L103-L134 (chrome/m156),
// `EXPECT_STRUCT_ARRAY`
macro_rules! expect_struct_array {
    (
        $r:expr,
        $layout:expr,
        $count:expr,
        $expected_alignment:expr,
        $expected_stride:expr,
        [$($field:expr),* $(,)?] $(,)?
    ) => {{
        let (alignment, size) =
            calculate_struct_alignment_and_size($layout, &[$($field),*], $count);
        let stride = size / $count;
        reporter_assert!(
            $r,
            alignment == $expected_alignment,
            "incorrect alignment for struct: expected {}, found {}",
            $expected_alignment,
            alignment
        );
        reporter_assert!(
            $r,
            size == $count * $expected_stride,
            "incorrect size for struct array: expected {}, found {}",
            $count * $expected_stride,
            size
        );
        reporter_assert!(
            $r,
            stride == $expected_stride,
            "incorrect stride for struct: expected {}, found {}",
            $expected_stride,
            stride
        );
        reporter_assert!(
            $r,
            stride % alignment == 0,
            "struct stride must be a multiple of alignment"
        );
    }};
}

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L136-L170 (chrome/m156)
def_test!(UniformOffsetCalculatorMetalBasicTypesTest, |r| {
    let k_layout = Layout::Metal;

    // scalars: int, float, half (unsigned types are disallowed)
    expect!(r, k_layout, SkSLType::Int, 4, 4);
    expect!(r, k_layout, SkSLType::Float, 4, 4);
    expect!(r, k_layout, SkSLType::Half, 2, 2);

    // int2, float2, half2
    expect!(r, k_layout, SkSLType::Int2, 8, 8);
    expect!(r, k_layout, SkSLType::Float2, 8, 8);
    expect!(r, k_layout, SkSLType::Half2, 4, 4);

    // int3, float3, half3 (unlike std430, size is also rounded up)
    expect!(r, k_layout, SkSLType::Int3, 16, 16);
    expect!(r, k_layout, SkSLType::Float3, 16, 16);
    expect!(r, k_layout, SkSLType::Half3, 8, 8);

    // int4, float4, half4
    expect!(r, k_layout, SkSLType::Int4, 16, 16);
    expect!(r, k_layout, SkSLType::Float4, 16, 16);
    expect!(r, k_layout, SkSLType::Half4, 8, 8);

    // float2x2, half2x2
    expect!(r, k_layout, SkSLType::Float2x2, 8, 16);
    expect!(r, k_layout, SkSLType::Half2x2, 4, 8);

    // float3x3, half3x3
    expect!(r, k_layout, SkSLType::Float3x3, 16, 48);
    expect!(r, k_layout, SkSLType::Half3x3, 8, 24);

    // float4x4, half4x4
    expect!(r, k_layout, SkSLType::Float4x4, 16, 64);
    expect!(r, k_layout, SkSLType::Half4x4, 8, 32);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L172-L207 (chrome/m156)
def_test!(UniformOffsetCalculatorMetalArrayTest, |r| {
    let k_layout = Layout::Metal;
    let k_count: i32 = 3;

    // int[3], float[3], half[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Float, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Half, 2, 2, 6);

    // int2[3], float2[3], half2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Float2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Half2, 4, 4, 12);

    // int3[3], float3[3], half3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half3, 8, 8, 24);

    // int4[3], float4[3], half4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half4, 8, 8, 24);

    // float2x2[3], half2x2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float2x2, 8, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half2x2, 4, 8, 24);

    // float3x3[3], half3x3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float3x3, 16, 48, 144);
    expect_array!(r, k_layout, k_count, SkSLType::Half3x3, 8, 24, 72);

    // float4x4[3], half4x4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float4x4, 16, 64, 192);
    expect_array!(r, k_layout, k_count, SkSLType::Half4x4, 8, 32, 96);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L209-L236 (chrome/m156)
def_test!(UniformOffsetCalculatorMetalStructTest, |r| {
    let k_layout = Layout::Metal;
    let k_count: i32 = 3;

    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float4, SkSLType::Float3]);
    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float3, SkSLType::Float]);
    expect_struct!(r, k_layout, 8, 16, [SkSLType::Float, SkSLType::Float2]);
    expect_struct!(r, k_layout, 4, 4, [SkSLType::Float]);
    expect_struct!(
        r,
        k_layout,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct!(r, k_layout, 4, 8, [SkSLType::Half2, SkSLType::Int]);

    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        32,
        [SkSLType::Float4, SkSLType::Float3]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        8,
        16,
        [SkSLType::Float, SkSLType::Float2]
    );
    expect_struct_array!(r, k_layout, k_count, 4, 4, [SkSLType::Float]);
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct_array!(r, k_layout, k_count, 4, 8, [SkSLType::Half2, SkSLType::Int]);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L238-L272 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430BasicTypesTest, |r| {
    let k_layout = Layout::Std430;

    // scalars: int, float, half (unsigned types are disallowed)
    expect!(r, k_layout, SkSLType::Int, 4, 4);
    expect!(r, k_layout, SkSLType::Float, 4, 4);
    expect!(r, k_layout, SkSLType::Half, 4, 4);

    // int2, float2, half2
    expect!(r, k_layout, SkSLType::Int2, 8, 8);
    expect!(r, k_layout, SkSLType::Float2, 8, 8);
    expect!(r, k_layout, SkSLType::Half2, 8, 8);

    // int3, float3, half3 (size is not rounded up for non-arrays of vec3s)
    expect!(r, k_layout, SkSLType::Int3, 16, 12);
    expect!(r, k_layout, SkSLType::Float3, 16, 12);
    expect!(r, k_layout, SkSLType::Half3, 16, 12);

    // int4, float4, half4
    expect!(r, k_layout, SkSLType::Int4, 16, 16);
    expect!(r, k_layout, SkSLType::Float4, 16, 16);
    expect!(r, k_layout, SkSLType::Half4, 16, 16);

    // float2x2, half2x2
    expect!(r, k_layout, SkSLType::Float2x2, 8, 16);
    expect!(r, k_layout, SkSLType::Half2x2, 8, 16);

    // float3x3, half3x3
    expect!(r, k_layout, SkSLType::Float3x3, 16, 48);
    expect!(r, k_layout, SkSLType::Half3x3, 16, 48);

    // float4x4, half4x4
    expect!(r, k_layout, SkSLType::Float4x4, 16, 64);
    expect!(r, k_layout, SkSLType::Half4x4, 16, 64);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L274-L309 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430ArrayTest, |r| {
    let k_layout = Layout::Std430;
    let k_count: i32 = 3;

    // int[3], float[3], half[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Float, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Half, 4, 4, 12);

    // int2[3], float2[3], half2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Float2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Half2, 8, 8, 24);

    // int3[3], float3[3], half3[3] (stride is rounded up in arrays)
    expect_array!(r, k_layout, k_count, SkSLType::Int3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half3, 16, 16, 48);

    // int4[3], float4[3], half4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half4, 16, 16, 48);

    // float2x2[3], half2x2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float2x2, 8, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half2x2, 8, 16, 48);

    // float3x3[3], half3x3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float3x3, 16, 48, 144);
    expect_array!(r, k_layout, k_count, SkSLType::Half3x3, 16, 48, 144);

    // float4x4[3], half4x4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float4x4, 16, 64, 192);
    expect_array!(r, k_layout, k_count, SkSLType::Half4x4, 16, 64, 192);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L311-L338 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430StructTest, |r| {
    let k_layout = Layout::Std430;
    let k_count: i32 = 3;

    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float4, SkSLType::Float3]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float3, SkSLType::Float]);
    expect_struct!(r, k_layout, 8, 16, [SkSLType::Float, SkSLType::Float2]);
    expect_struct!(r, k_layout, 4, 4, [SkSLType::Float]);
    expect_struct!(
        r,
        k_layout,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct!(r, k_layout, 8, 16, [SkSLType::Half2, SkSLType::Int]);

    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        32,
        [SkSLType::Float4, SkSLType::Float3]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        8,
        16,
        [SkSLType::Float, SkSLType::Float2]
    );
    expect_struct_array!(r, k_layout, k_count, 4, 4, [SkSLType::Float]);
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        8,
        16,
        [SkSLType::Half2, SkSLType::Int]
    );
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L340-L374 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430_F16BasicTypesTest, |r| {
    let k_layout = Layout::Std430F16;

    // scalars: int, float, half (unsigned types are disallowed)
    expect!(r, k_layout, SkSLType::Int, 4, 4);
    expect!(r, k_layout, SkSLType::Float, 4, 4);
    expect!(r, k_layout, SkSLType::Half, 2, 2);

    // int2, float2, half2
    expect!(r, k_layout, SkSLType::Int2, 8, 8);
    expect!(r, k_layout, SkSLType::Float2, 8, 8);
    expect!(r, k_layout, SkSLType::Half2, 4, 4);

    // int3, float3, half3 (size is not rounded up for non-arrays of vec3s)
    expect!(r, k_layout, SkSLType::Int3, 16, 12);
    expect!(r, k_layout, SkSLType::Float3, 16, 12);
    expect!(r, k_layout, SkSLType::Half3, 8, 6);

    // int4, float4, half4
    expect!(r, k_layout, SkSLType::Int4, 16, 16);
    expect!(r, k_layout, SkSLType::Float4, 16, 16);
    expect!(r, k_layout, SkSLType::Half4, 8, 8);

    // float2x2, half2x2
    expect!(r, k_layout, SkSLType::Float2x2, 8, 16);
    expect!(r, k_layout, SkSLType::Half2x2, 4, 8);

    // float3x3, half3x3
    expect!(r, k_layout, SkSLType::Float3x3, 16, 48);
    expect!(r, k_layout, SkSLType::Half3x3, 8, 24);

    // float4x4, half4x4
    expect!(r, k_layout, SkSLType::Float4x4, 16, 64);
    expect!(r, k_layout, SkSLType::Half4x4, 8, 32);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L376-L411 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430_F16ArrayTest, |r| {
    let k_layout = Layout::Std430F16;
    let k_count: i32 = 3;

    // int[3], float[3], half[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Float, 4, 4, 12);
    expect_array!(r, k_layout, k_count, SkSLType::Half, 2, 2, 6);

    // int2[3], float2[3], half2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Float2, 8, 8, 24);
    expect_array!(r, k_layout, k_count, SkSLType::Half2, 4, 4, 12);

    // int3[3], float3[3], half3[3] (stride is rounded up in arrays)
    expect_array!(r, k_layout, k_count, SkSLType::Int3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half3, 8, 8, 24);

    // int4[3], float4[3], half4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half4, 8, 8, 24);

    // float2x2[3], half2x2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float2x2, 8, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half2x2, 4, 8, 24);

    // float3x3[3], half3x3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float3x3, 16, 48, 144);
    expect_array!(r, k_layout, k_count, SkSLType::Half3x3, 8, 24, 72);

    // float4x4[3], half4x4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float4x4, 16, 64, 192);
    expect_array!(r, k_layout, k_count, SkSLType::Half4x4, 8, 32, 96);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L413-L440 (chrome/m156)
def_test!(UniformOffsetCalculatorStd430_F16StructTest, |r| {
    let k_layout = Layout::Std430F16;
    let k_count: i32 = 3;

    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float4, SkSLType::Float3]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float3, SkSLType::Float]);
    expect_struct!(r, k_layout, 8, 16, [SkSLType::Float, SkSLType::Float2]);
    expect_struct!(r, k_layout, 4, 4, [SkSLType::Float]);
    expect_struct!(
        r,
        k_layout,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct!(r, k_layout, 4, 8, [SkSLType::Half2, SkSLType::Int]);

    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        32,
        [SkSLType::Float4, SkSLType::Float3]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        8,
        16,
        [SkSLType::Float, SkSLType::Float2]
    );
    expect_struct_array!(r, k_layout, k_count, 4, 4, [SkSLType::Float]);
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        4,
        12,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct_array!(r, k_layout, k_count, 4, 8, [SkSLType::Half2, SkSLType::Int]);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L442-L476 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140BasicTypesTest, |r| {
    let k_layout = Layout::Std140;

    // scalars: int, float, half (unsigned types are disallowed)
    expect!(r, k_layout, SkSLType::Int, 4, 4);
    expect!(r, k_layout, SkSLType::Float, 4, 4);
    expect!(r, k_layout, SkSLType::Half, 4, 4);

    // int2, float2, half2
    expect!(r, k_layout, SkSLType::Int2, 8, 8);
    expect!(r, k_layout, SkSLType::Float2, 8, 8);
    expect!(r, k_layout, SkSLType::Half2, 8, 8);

    // int3, float3, half3 (size is not rounded up for non-arrays of vec3s)
    expect!(r, k_layout, SkSLType::Int3, 16, 12);
    expect!(r, k_layout, SkSLType::Float3, 16, 12);
    expect!(r, k_layout, SkSLType::Half3, 16, 12);

    // int4, float4, half4
    expect!(r, k_layout, SkSLType::Int4, 16, 16);
    expect!(r, k_layout, SkSLType::Float4, 16, 16);
    expect!(r, k_layout, SkSLType::Half4, 16, 16);

    // float2x2, half2x2
    expect!(r, k_layout, SkSLType::Float2x2, 16, 32);
    expect!(r, k_layout, SkSLType::Half2x2, 16, 32);

    // float3x3, half3x3
    expect!(r, k_layout, SkSLType::Float3x3, 16, 48);
    expect!(r, k_layout, SkSLType::Half3x3, 16, 48);

    // float4x4, half4x4
    expect!(r, k_layout, SkSLType::Float4x4, 16, 64);
    expect!(r, k_layout, SkSLType::Half4x4, 16, 64);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L478-L513 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140ArrayTest, |r| {
    let k_layout = Layout::Std140;
    let k_count: i32 = 3;

    // int[3], float[3], half[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half, 16, 16, 48);

    // int2[3], float2[3], half2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int2, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float2, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half2, 16, 16, 48);

    // int3[3], float3[3], half3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half3, 16, 16, 48);

    // int4[3], float4[3], half4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half4, 16, 16, 48);

    // float2x2[3], half2x2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float2x2, 16, 32, 96);
    expect_array!(r, k_layout, k_count, SkSLType::Half2x2, 16, 32, 96);

    // float3x3[3], half3x3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float3x3, 16, 48, 144);
    expect_array!(r, k_layout, k_count, SkSLType::Half3x3, 16, 48, 144);

    // float4x4[3], half4x4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float4x4, 16, 64, 192);
    expect_array!(r, k_layout, k_count, SkSLType::Half4x4, 16, 64, 192);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L515-L542 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140StructTest, |r| {
    let k_layout = Layout::Std140;
    let k_count: i32 = 3;

    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float4, SkSLType::Float3]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float3, SkSLType::Float]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float, SkSLType::Float2]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float]);
    expect_struct!(
        r,
        k_layout,
        16,
        16,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Half2, SkSLType::Int]);

    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        32,
        [SkSLType::Float4, SkSLType::Float3]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Float, SkSLType::Float2]
    );
    expect_struct_array!(r, k_layout, k_count, 16, 16, [SkSLType::Float]);
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Half2, SkSLType::Int]
    );
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L544-L578 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140_F16BasicTypesTest, |r| {
    let k_layout = Layout::Std140F16;

    // scalars: int, float, half (unsigned types are disallowed)
    expect!(r, k_layout, SkSLType::Int, 4, 4);
    expect!(r, k_layout, SkSLType::Float, 4, 4);
    expect!(r, k_layout, SkSLType::Half, 2, 2);

    // int2, float2, half2
    expect!(r, k_layout, SkSLType::Int2, 8, 8);
    expect!(r, k_layout, SkSLType::Float2, 8, 8);
    expect!(r, k_layout, SkSLType::Half2, 4, 4);

    // int3, float3, half3 (size is not rounded up for non-arrays of vec3s)
    expect!(r, k_layout, SkSLType::Int3, 16, 12);
    expect!(r, k_layout, SkSLType::Float3, 16, 12);
    expect!(r, k_layout, SkSLType::Half3, 8, 6);

    // int4, float4, half4
    expect!(r, k_layout, SkSLType::Int4, 16, 16);
    expect!(r, k_layout, SkSLType::Float4, 16, 16);
    expect!(r, k_layout, SkSLType::Half4, 8, 8);

    // float2x2, half2x2
    expect!(r, k_layout, SkSLType::Float2x2, 16, 32);
    expect!(r, k_layout, SkSLType::Half2x2, 16, 32);

    // float3x3, half3x3
    expect!(r, k_layout, SkSLType::Float3x3, 16, 48);
    expect!(r, k_layout, SkSLType::Half3x3, 16, 48);

    // float4x4, half4x4
    expect!(r, k_layout, SkSLType::Float4x4, 16, 64);
    expect!(r, k_layout, SkSLType::Half4x4, 16, 64);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L583-L618 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140_F16ArrayTest, |r| {
    let k_layout = Layout::Std140F16;
    let k_count: i32 = 3;

    // int[3], float[3], half[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half, 16, 16, 48);

    // int2[3], float2[3], half2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int2, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float2, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half2, 16, 16, 48);

    // int3[3], float3[3], half3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float3, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half3, 16, 16, 48);

    // int4[3], float4[3], half4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Int4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Float4, 16, 16, 48);
    expect_array!(r, k_layout, k_count, SkSLType::Half4, 16, 16, 48);

    // float2x2[3], half2x2[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float2x2, 16, 32, 96);
    expect_array!(r, k_layout, k_count, SkSLType::Half2x2, 16, 32, 96);

    // float3x3[3], half3x3[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float3x3, 16, 48, 144);
    expect_array!(r, k_layout, k_count, SkSLType::Half3x3, 16, 48, 144);

    // float4x4[3], half4x4[3]
    expect_array!(r, k_layout, k_count, SkSLType::Float4x4, 16, 64, 192);
    expect_array!(r, k_layout, k_count, SkSLType::Half4x4, 16, 64, 192);
});

// Port of: tests/graphite/UniformOffsetCalculatorTest.cpp#L620-L647 (chrome/m156)
def_test!(UniformOffsetCalculatorStd140_F16StructTest, |r| {
    let k_layout = Layout::Std140F16;
    let k_count: i32 = 3;

    expect_struct!(r, k_layout, 16, 32, [SkSLType::Float4, SkSLType::Float3]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float3, SkSLType::Float]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float, SkSLType::Float2]);
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Float]);
    expect_struct!(
        r,
        k_layout,
        16,
        16,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct!(r, k_layout, 16, 16, [SkSLType::Half2, SkSLType::Int]);

    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        32,
        [SkSLType::Float4, SkSLType::Float3]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Float, SkSLType::Float2]
    );
    expect_struct_array!(r, k_layout, k_count, 16, 16, [SkSLType::Float]);
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Float, SkSLType::Float, SkSLType::Int]
    );
    expect_struct_array!(
        r,
        k_layout,
        k_count,
        16,
        16,
        [SkSLType::Half2, SkSLType::Int]
    );
});
