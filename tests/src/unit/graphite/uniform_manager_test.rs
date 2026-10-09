// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/UniformManagerTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::half::half_to_float;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::uniform::Uniform;
use skia_rust_gpu::graphite::uniform_manager::UniformManager;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{Reporter, def_test, errorf, reporter_assert};

const K_LAYOUTS: [Layout; 5] = [
    Layout::Std140,
    Layout::Std140F16,
    Layout::Std430,
    Layout::Std430F16,
    Layout::Metal,
];

// This list excludes SkSLTypes that we don't support in uniforms, like Bool, UInt or UShort.
const K_TYPES: [SkSLType; 18] = [
    SkSLType::Float,
    SkSLType::Float2,
    SkSLType::Float3,
    SkSLType::Float4,
    SkSLType::Half,
    SkSLType::Half2,
    SkSLType::Half3,
    SkSLType::Half4,
    SkSLType::Int,
    SkSLType::Int2,
    SkSLType::Int3,
    SkSLType::Int4,
    SkSLType::Float2x2,
    SkSLType::Float3x3,
    SkSLType::Float4x4,
    SkSLType::Half2x2,
    SkSLType::Half3x3,
    SkSLType::Half4x4,
];

const K_FLOATS: [f32; 16] = [
    1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
];

const K_HALFS: [u16; 16] = [
    0x3C00, 0x4000, 0x4200, 0x4400, 0x4500, 0x4600, 0x4700, 0x4800, 0x4880, 0x4900, 0x4980, 0x4A00,
    0x4A80, 0x4B00, 0x4B80, 0x4C00,
];

const K_INTS: [i32; 16] = [
    1, -2, 3, -4, 5, -6, 7, -8, 9, -10, 11, -12, 13, -14, 15, -16,
];

const K_ARRAY_SIZE: i32 = 3;

// Buffer large enough to hold a float4x4[3] array.
const K_BUFFER: [u8; 192] = [0; 192];

// The bytes of `values` in native byte order: the source data of `write_uniform`.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn u16_bytes(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn i32_bytes(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

// Reads the element at `index` of `data`, as the C++ reads `elements[index]`.
fn f32_at(data: &[u8], index: usize) -> f32 {
    let j = 4 * index;
    f32::from_ne_bytes([data[j], data[j + 1], data[j + 2], data[j + 3]])
}

fn u16_at(data: &[u8], index: usize) -> u16 {
    let j = 2 * index;
    u16::from_ne_bytes([data[j], data[j + 1]])
}

fn usize_of(x: i32) -> usize {
    usize::try_from(x).expect("sizes are non-negative")
}

fn element_size(layout: Layout, ty: SkSLType) -> usize {
    // Metal and the _F16 std extended layouts encodes half-precision uniforms in 16 bits.
    // Other layouts are expected to encode uniforms in 32 bits.
    let uses_half =
        layout == Layout::Metal || layout == Layout::Std140F16 || layout == Layout::Std430F16;
    if uses_half && !ty.is_full_precision_numeric_type() {
        2
    } else {
        4
    }
}

// Port of: tests/graphite/UniformManagerTest.cpp#L59-L74 (chrome/m156), the `DEF_GRAPHITE_TEST`
// body is in `uniform_manager_check_single_uniform` below.
// Port of: tests/graphite/UniformManagerTest.cpp#L59-L74 (chrome/m156)
def_test!(UniformManagerCheckSingleUniform, |r| {
    // Verify that the uniform manager can hold all the basic uniform types, in every layout.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty in K_TYPES {
            let expectations = [Uniform::new("uniform", ty)];
            #[cfg(debug_assertions)]
            mgr.set_expected_uniforms(&expectations, false);
            mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
            #[cfg(debug_assertions)]
            mgr.done_with_expected_uniforms();
            reporter_assert!(
                r,
                mgr.size() > 0,
                "Layout: {} - Type: {}",
                layout.as_str(),
                ty.as_str()
            );
            mgr.reset();
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L76-L105 (chrome/m156)
def_test!(UniformManagerCheckFloatEncoding, |r| {
    // Verify that the uniform manager encodes float data properly.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty in K_TYPES {
            // Only test scalar and vector floats. (Matrices can introduce padding between values.)
            let vec_length = ty.vec_length();
            if !ty.is_float_type() || vec_length < 1 {
                continue;
            }

            // Write our uniform float scalar/vector.
            let expectations = [Uniform::new("uniform", ty)];
            #[cfg(debug_assertions)]
            mgr.set_expected_uniforms(&expectations, false);
            mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
            #[cfg(debug_assertions)]
            mgr.done_with_expected_uniforms();

            // Read back the uniform data.
            let uniform_data = mgr.finish().to_vec();
            let element_size = element_size(layout, ty);
            let valid_data = if element_size == 4 {
                f32_bytes(&K_FLOATS)
            } else {
                u16_bytes(&K_HALFS)
            };
            let n = usize_of(vec_length) * element_size;
            reporter_assert!(r, uniform_data.len() >= n);
            reporter_assert!(
                r,
                uniform_data.get(..n) == Some(&valid_data[..n]),
                "Layout: {} - Type: {} float encoding failed",
                layout.as_str(),
                ty.as_str()
            );
            mgr.reset();
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L107-L134 (chrome/m156)
def_test!(UniformManagerCheckIntEncoding, |r| {
    // Verify that the uniform manager encodes int data properly.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty in K_TYPES {
            if !ty.is_integral_type() {
                continue;
            }

            // Write our uniform int scalar/vector.
            let expectations = [Uniform::new("uniform", ty)];
            #[cfg(debug_assertions)]
            mgr.set_expected_uniforms(&expectations, false);
            mgr.write_uniform(&expectations[0], &i32_bytes(&K_INTS));
            #[cfg(debug_assertions)]
            mgr.done_with_expected_uniforms();

            // Read back the uniform data.
            let uniform_data = mgr.finish().to_vec();
            let vec_length = usize_of(ty.vec_length());
            let element_size = element_size(layout, ty);
            let n = vec_length * element_size;
            reporter_assert!(r, uniform_data.len() >= n);
            reporter_assert!(
                r,
                uniform_data.get(..n) == Some(&i32_bytes(&K_INTS)[..n]),
                "Layout: {} - Type: {} int encoding failed",
                layout.as_str(),
                ty.as_str()
            );
            mgr.reset();
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L136-L166 (chrome/m156)
def_test!(UniformManagerCheckScalarVectorPacking, |r| {
    // Verify that the uniform manager can pack scalars and vectors of identical type correctly.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty in K_TYPES {
            let vec_length = ty.vec_length();
            if vec_length < 1 {
                continue;
            }

            // Write three matching uniforms.
            let expectations = [
                Uniform::new("a", ty),
                Uniform::new("b", ty),
                Uniform::new("c", ty),
            ];
            #[cfg(debug_assertions)]
            mgr.set_expected_uniforms(&expectations, false);
            mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
            mgr.write_uniform(&expectations[1], &f32_bytes(&K_FLOATS));
            mgr.write_uniform(&expectations[2], &f32_bytes(&K_FLOATS));
            #[cfg(debug_assertions)]
            mgr.done_with_expected_uniforms();

            // Verify the uniform data packing.
            let data_len = mgr.finish().len();
            let element_size = element_size(layout, ty);
            // Vec3s must be laid out as if they were vec4s.
            let effective_vec_length = if vec_length == 3 {
                4
            } else {
                usize_of(vec_length)
            };
            reporter_assert!(
                r,
                data_len == element_size * effective_vec_length * 3,
                "Layout: {} - Type: {} tight packing failed",
                layout.as_str(),
                ty.as_str()
            );
            mgr.reset();
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L168-L209 (chrome/m156)
def_test!(UniformManagerCheckMatrixPacking, |r| {
    // Verify that the uniform manager can pack matrices correctly.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty in K_TYPES {
            let matrix_size = ty.matrix_size();
            if matrix_size < 2 {
                continue;
            }

            // Write three matching uniforms.
            let expectations = [
                Uniform::new("a", ty),
                Uniform::new("b", ty),
                Uniform::new("c", ty),
            ];
            #[cfg(debug_assertions)]
            mgr.set_expected_uniforms(&expectations, false);
            mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
            mgr.write_uniform(&expectations[1], &f32_bytes(&K_FLOATS));
            mgr.write_uniform(&expectations[2], &f32_bytes(&K_FLOATS));
            #[cfg(debug_assertions)]
            mgr.done_with_expected_uniforms();

            // Verify the uniform data packing.
            let data_len = mgr.finish().len();
            // In std140-f16, pretend the element size is 4 since the underlying column vectors
            // are still forced to 16-byte alignment.
            let element_size = if layout == Layout::Std140F16 {
                4
            } else {
                element_size(layout, ty)
            };
            // In all layouts, mat3s burn 12 elements, not 9. In std140, mat2s burn 8 elements
            // instead of 4.
            let num_elements = if matrix_size == 3 {
                12
            } else if matrix_size == 2 && (layout == Layout::Std140 || layout == Layout::Std140F16)
            {
                8
            } else {
                usize_of(matrix_size * matrix_size)
            };
            reporter_assert!(
                r,
                data_len == element_size * num_elements * 3,
                "Layout: {} - Type: {} matrix packing failed",
                layout.as_str(),
                ty.as_str()
            );
            mgr.reset();
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L211-L328 (chrome/m156)
def_test!(UniformManagerCheckPaddingScalarVector, |r| {
    // Verify that the uniform manager properly adds padding between pairs of scalar/vector.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty1 in K_TYPES {
            let vec_length1 = ty1.vec_length();
            if vec_length1 < 1 {
                continue;
            }

            for ty2 in K_TYPES {
                let vec_length2 = ty2.vec_length();
                if vec_length2 < 1 {
                    continue;
                }

                // Write two scalar/vector uniforms.
                let expectations = [Uniform::new("a", ty1), Uniform::new("b", ty2)];
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&expectations, false);
                mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
                mgr.write_uniform(&expectations[1], &f32_bytes(&K_FLOATS));
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();

                // The expected packing varies depending on the bit-widths of each element.
                let element_size1 = element_size(layout, ty1);
                let element_size2 = element_size(layout, ty2);
                let layout_idx = usize::from(layout != Layout::Metal);
                let vl1 = usize_of(vec_length1);
                let vl2 = usize_of(vec_length2);

                if element_size1 == element_size2 {
                    // Elements in the array correspond to the element size (either 16 or 32 bits).
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // Metal (vec3 consumes vec4 size)
                        [
                            ["", "", "", "", ""],
                            ["", "AB", "A_BB", "A___BBBb", "A___BBBB"],
                            ["", "AAB_", "AABB", "AA__BBBb", "AA__BBBB"],
                            ["", "AAAaB___", "AAAaBB__", "AAAaBBBb", "AAAaBBBB"],
                            ["", "AAAAB___", "AAAABB__", "AAAABBBb", "AAAABBBB"],
                        ],
                        // std140 and std430 (vec3 aligns to vec4, but consumes only 3 elements)
                        [
                            ["", "", "", "", ""],
                            ["", "AB", "A_BB", "A___BBBb", "A___BBBB"],
                            ["", "AAB_", "AABB", "AA__BBBb", "AA__BBBB"],
                            ["", "AAAB", "AAA_BB__", "AAA_BBBb", "AAA_BBBB"],
                            ["", "AAAAB___", "AAAABB__", "AAAABBBb", "AAAABBBB"],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][vl1][vl2].len() * element_size1;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 2 && element_size2 == 4 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots in Metal)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // Metal (vec3 consumes vec4 size)
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "A_BB",
                                "A___BBBB",
                                "A_______BBBBBBbb",
                                "A_______BBBBBBBB",
                            ],
                            [
                                "",
                                "AABB",
                                "AA__BBBB",
                                "AA______BBBBBBbb",
                                "AA______BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAaBB__",
                                "AAAaBBBB",
                                "AAAa____BBBBBBbb",
                                "AAAa____BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAABB__",
                                "AAAABBBB",
                                "AAAA____BBBBBBbb",
                                "AAAA____BBBBBBBB",
                            ],
                        ],
                        // std140-f16 and std430-f16 (vec3 aligns to vec4 but consumes only 3)
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "A_BB",
                                "A___BBBB",
                                "A_______BBBBBBbb",
                                "A_______BBBBBBBB",
                            ],
                            [
                                "",
                                "AABB",
                                "AA__BBBB",
                                "AA______BBBBBBbb",
                                "AA______BBBBBBBB",
                            ],
                            [
                                "",
                                "AAA_BB__",
                                "AAA_BBBB",
                                "AAA_____BBBBBBbb",
                                "AAA_____BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAABB__",
                                "AAAABBBB",
                                "AAAA____BBBBBBbb",
                                "AAAA____BBBBBBBB",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][vl1][vl2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 4 && element_size2 == 2 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // Metal (vec3 consumes vec4 size)
                        [
                            ["", "", "", "", ""],
                            ["", "AAB_", "AABB", "AA__BBBb", "AA__BBBB"],
                            ["", "AAAAB___", "AAAABB__", "AAAABBBb", "AAAABBBB"],
                            [
                                "",
                                "AAAAAAaaB_______",
                                "AAAAAAaaBB______",
                                "AAAAAAaaBBBb____",
                                "AAAAAAaaBBBB____",
                            ],
                            [
                                "",
                                "AAAAAAAAB_______",
                                "AAAAAAAABB______",
                                "AAAAAAAABBBb____",
                                "AAAAAAAABBBB____",
                            ],
                        ],
                        // std140-f16 and std430-f16 (vec3 aligns to vec4 but consumes only 3)
                        [
                            ["", "", "", "", ""],
                            ["", "AAB_", "AABB", "AA__BBB_", "AA__BBBB"],
                            ["", "AAAAB___", "AAAABB__", "AAAABBB_", "AAAABBBB"],
                            [
                                "",
                                "AAAAAAB_",
                                "AAAAAABB",
                                "AAAAAA__BBB_____",
                                "AAAAAA__BBBB____",
                            ],
                            [
                                "",
                                "AAAAAAAAB_______",
                                "AAAAAAAABB______",
                                "AAAAAAAABBB_____",
                                "AAAAAAAABBBB____",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][vl1][vl2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else {
                    errorf!(
                        r,
                        "Unexpected element sizes: {} {}",
                        element_size1,
                        element_size2
                    );
                }
                mgr.reset();
            }
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L330-L471 (chrome/m156)
def_test!(UniformManagerCheckPaddingVectorMatrix, |r| {
    // Verify that the uniform manager properly adds padding between vectors and matrices.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty1 in K_TYPES {
            let vec_length1 = ty1.vec_length();
            if vec_length1 < 1 {
                continue;
            }

            for ty2 in K_TYPES {
                let mat_size2 = ty2.matrix_size();
                if mat_size2 < 2 {
                    continue;
                }

                // Write the scalar/vector and matrix uniforms.
                let expectations = [Uniform::new("a", ty1), Uniform::new("b", ty2)];
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&expectations, false);
                mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
                mgr.write_uniform(&expectations[1], &f32_bytes(&K_FLOATS));
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();

                // The expected packing varies depending on the bit-widths of each element.
                let element_size1 = element_size(layout, ty1);
                let element_size2 = element_size(layout, ty2);
                let mut layout_idx =
                    usize::from(layout != Layout::Std140 && layout != Layout::Std140F16);
                let vl1 = usize_of(vec_length1);
                let m2 = usize_of(mat_size2);

                if element_size1 == element_size2 {
                    // Elements in the array correspond to the element size (16 or 32 bits).
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 3] = [
                        // std140-f16
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "",
                                "A_______BBbb____BBbb____",
                                "A_______BBBb____BBBb____BBBb____",
                                "A_______BBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AA______BBbb____BBbb____",
                                "AA______BBBb____BBBb____BBBb____",
                                "AA______BBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AAAa____BBbb____BBbb____",
                                "AAAa____BBBb____BBBb____BBBb____",
                                "AAAa____BBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AAAA____BBbb____BBbb____",
                                "AAAA____BBBb____BBBb____BBBb____",
                                "AAAA____BBBB____BBBB____BBBB____BBBB____",
                            ],
                        ],
                        // std140
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "",
                                "A___BBbbBBbb",
                                "A___BBBbBBBbBBBb",
                                "A___BBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AA__BBbbBBbb",
                                "AA__BBBbBBBbBBBb",
                                "AA__BBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAaBBbbBBbb",
                                "AAAaBBBbBBBbBBBb",
                                "AAAaBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAABBbbBBbb",
                                "AAAABBBbBBBbBBBb",
                                "AAAABBBBBBBBBBBBBBBB",
                            ],
                        ],
                        // All other layouts
                        [
                            ["", "", "", "", ""],
                            ["", "", "A_BBBB", "A___BBBbBBBbBBBb", "A___BBBBBBBBBBBBBBBB"],
                            ["", "", "AABBBB", "AA__BBBbBBBbBBBb", "AA__BBBBBBBBBBBBBBBB"],
                            [
                                "",
                                "",
                                "AAAaBBBB",
                                "AAAaBBBbBBBbBBBb",
                                "AAAaBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAABBBB",
                                "AAAABBBbBBBbBBBb",
                                "AAAABBBBBBBBBBBBBBBB",
                            ],
                        ],
                    ];

                    if element_size1 != 2 || layout != Layout::Std140F16 {
                        layout_idx += 1;
                    }
                    let size = k_expected_layout[layout_idx][vl1][m2].len() * element_size1;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} vector-matrix padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 2 && element_size2 == 4 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // std140-f16
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "",
                                "A_______BBBB____BBBB____",
                                "A_______BBBBBBbbBBBBBBbbBBBBBBbb",
                                "A_______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AA______BBBB____BBBB____",
                                "AA______BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAa____BBBB____BBBB____",
                                "AAAa____BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AAAa____BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAA____BBBB____BBBB____",
                                "AAAA____BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AAAA____BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                        ],
                        // Metal and std430-f16
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "",
                                "A___BBBBBBBB",
                                "A_______BBBBBBbbBBBBBBbbBBBBBBbb",
                                "A_______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AA__BBBBBBBB",
                                "AA______BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAaBBBBBBBB",
                                "AAAa____BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AAAa____BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAABBBBBBBB",
                                "AAAA____BBBBBBbbBBBBBBbbBBBBBBbb",
                                "AAAA____BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][vl1][m2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} vector-matrix padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 4 && element_size2 == 2 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // std140-f16
                        [
                            ["", "", "", "", ""],
                            [
                                "",
                                "",
                                "AA______BB______BB______",
                                "AA______BBBb____BBBb____BBBb____",
                                "AA______BBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AAAA____BB______BB______",
                                "AAAA____BBBb____BBBb____BBBb____",
                                "AAAA____BBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AAAAAAaaBB______BB______",
                                "AAAAAAaaBBBb____BBBb____BBBb____",
                                "AAAAAAaaBBBB____BBBB____BBBB____BBBB____",
                            ],
                            [
                                "",
                                "",
                                "AAAAAAAABB______BB______",
                                "AAAAAAAABBBb____BBBb____BBBb____",
                                "AAAAAAAABBBB____BBBB____BBBB____BBBB____",
                            ],
                        ],
                        // Metal and std430-f16
                        [
                            ["", "", "", "", ""],
                            ["", "", "AABBBB", "AA__BBBbBBBbBBBb", "AA__BBBBBBBBBBBBBBBB"],
                            [
                                "",
                                "",
                                "AAAABBBB",
                                "AAAABBBbBBBbBBBb",
                                "AAAABBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAAAAaaBBBB____",
                                "AAAAAAaaBBBbBBBbBBBb____",
                                "AAAAAAaaBBBBBBBBBBBBBBBB",
                            ],
                            [
                                "",
                                "",
                                "AAAAAAAABBBB____",
                                "AAAAAAAABBBbBBBbBBBb____",
                                "AAAAAAAABBBBBBBBBBBBBBBB",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][vl1][m2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} vector-matrix padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                }
                mgr.reset();
            }
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L473-L612 (chrome/m156)
def_test!(UniformManagerCheckPaddingMatrixVector, |r| {
    // Verify that the uniform manager properly adds padding between matrices and vectors.
    for layout in K_LAYOUTS {
        let mut mgr = UniformManager::new(layout);

        for ty1 in K_TYPES {
            let mat_size1 = ty1.matrix_size();
            if mat_size1 < 2 {
                continue;
            }

            for ty2 in K_TYPES {
                let vec_length2 = ty2.vec_length();
                if vec_length2 < 1 {
                    continue;
                }

                // Write the scalar/vector and matrix uniforms.
                let expectations = [Uniform::new("a", ty1), Uniform::new("b", ty2)];
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&expectations, false);
                mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
                mgr.write_uniform(&expectations[1], &f32_bytes(&K_FLOATS));
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();

                // The expected packing varies depending on the bit-widths of each element.
                let element_size1 = element_size(layout, ty1);
                let element_size2 = element_size(layout, ty2);
                let mut layout_idx =
                    usize::from(layout != Layout::Std140 && layout != Layout::Std140F16);
                let m1 = usize_of(mat_size1);
                let vl2 = usize_of(vec_length2);

                if element_size1 == element_size2 {
                    // Elements in the array correspond to the element size (16 or 32 bits).
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 3] = [
                        // std140-f16 layout
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AAaa____AAaa____B_______",
                                "AAaa____AAaa____BB______",
                                "AAaa____AAaa____BBBb____",
                                "AAaa____AAaa____BBBB____",
                            ],
                            [
                                "",
                                "AAAa____AAAa____AAAa____B_______",
                                "AAAa____AAAa____AAAa____BB______",
                                "AAAa____AAAa____AAAa____BBBb____",
                                "AAAa____AAAa____AAAa____BBBB____",
                            ],
                            [
                                "",
                                "AAAA____AAAA____AAAA____AAAA____B_______",
                                "AAAA____AAAA____AAAA____AAAA____BB______",
                                "AAAA____AAAA____AAAA____AAAA____BBBb____",
                                "AAAA____AAAA____AAAA____AAAA____BBBB____",
                            ],
                        ],
                        // std140 layout
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AAaaAAaaB___",
                                "AAaaAAaaBB__",
                                "AAaaAAaaBBBb",
                                "AAaaAAaaBBBB",
                            ],
                            [
                                "",
                                "AAAaAAAaAAAaB___",
                                "AAAaAAAaAAAaBB__",
                                "AAAaAAAaAAAaBBBb",
                                "AAAaAAAaAAAaBBBB",
                            ],
                            [
                                "",
                                "AAAAAAAAAAAAAAAAB___",
                                "AAAAAAAAAAAAAAAABB__",
                                "AAAAAAAAAAAAAAAABBBb",
                                "AAAAAAAAAAAAAAAABBBB",
                            ],
                        ],
                        // All other layouts
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            ["", "AAAAB_", "AAAABB", "AAAABBBb", "AAAABBBB"],
                            [
                                "",
                                "AAAaAAAaAAAaB___",
                                "AAAaAAAaAAAaBB__",
                                "AAAaAAAaAAAaBBBb",
                                "AAAaAAAaAAAaBBBB",
                            ],
                            [
                                "",
                                "AAAAAAAAAAAAAAAAB___",
                                "AAAAAAAAAAAAAAAABB__",
                                "AAAAAAAAAAAAAAAABBBb",
                                "AAAAAAAAAAAAAAAABBBB",
                            ],
                        ],
                    ];

                    if element_size1 != 2 || layout != Layout::Std140F16 {
                        layout_idx += 1;
                    }
                    let size = k_expected_layout[layout_idx][m1][vl2].len() * element_size1;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} matrix-vector padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 2 && element_size2 == 4 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // std140-f16
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AA______AA______BB______",
                                "AA______AA______BBBB____",
                                "AA______AA______BBBBBBbb",
                                "AA______AA______BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAa____AAAa____AAAa____BB______",
                                "AAAa____AAAa____AAAa____BBBB____",
                                "AAAa____AAAa____AAAa____BBBBBBbb",
                                "AAAa____AAAa____AAAa____BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAA____AAAA____AAAA____AAAA____BB______",
                                "AAAA____AAAA____AAAA____AAAA____BBBB____",
                                "AAAA____AAAA____AAAA____AAAA____BBBBBBbb",
                                "AAAA____AAAA____AAAA____AAAA____BBBBBBBB",
                            ],
                        ],
                        // Metal and std430-f16
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AAAABB",
                                "AAAABBBB",
                                "AAAA____BBBBBBbb",
                                "AAAA____BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAaAAAaAAAaBB__",
                                "AAAaAAAaAAAaBBBB",
                                "AAAaAAAaAAAa____BBBBBBbb",
                                "AAAaAAAaAAAa____BBBBBBBB",
                            ],
                            [
                                "",
                                "AAAAAAAAAAAAAAAABB__",
                                "AAAAAAAAAAAAAAAABBBB",
                                "AAAAAAAAAAAAAAAABBBBBBbb",
                                "AAAAAAAAAAAAAAAABBBBBBBB",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][m1][vl2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} matrix-vector padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                } else if element_size1 == 4 && element_size2 == 2 {
                    // Elements in the array below correspond to 16 bits apiece.
                    // The expected uniform layout is listed as strings below.
                    // A/B: uniform values.
                    // a/b: padding as part of the uniform type (vec3 takes 4 slots)
                    // _  : padding between uniforms for alignment
                    let k_expected_layout: [[[&str; 5]; 5]; 2] = [
                        // std140-f16
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AAAA____AAAA____B_______",
                                "AAAA____AAAA____BB______",
                                "AAAA____AAAA____BBBb____",
                                "AAAA____AAAA____BBBB____",
                            ],
                            [
                                "",
                                "AAAAAAaaAAAAAAaaAAAAAAaaB_______",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBB______",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBBBb____",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBBBB____",
                            ],
                            [
                                "",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB_______",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABB______",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBb____",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBB____",
                            ],
                        ],
                        // Metal and std430-f16
                        [
                            ["", "", "", "", ""],
                            ["", "", "", "", ""],
                            [
                                "",
                                "AAAAAAAAB___",
                                "AAAAAAAABB__",
                                "AAAAAAAABBBb",
                                "AAAAAAAABBBB",
                            ],
                            [
                                "",
                                "AAAAAAaaAAAAAAaaAAAAAAaaB_______",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBB______",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBBBb____",
                                "AAAAAAaaAAAAAAaaAAAAAAaaBBBB____",
                            ],
                            [
                                "",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB_______",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABB______",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBb____",
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBB____",
                            ],
                        ],
                    ];
                    let size = k_expected_layout[layout_idx][m1][vl2].len() * 2;
                    let uniform_data_len = mgr.finish().len();
                    reporter_assert!(
                        r,
                        uniform_data_len == size,
                        "Layout: {} - Types: {}, {} matrix-vector padding test failed",
                        layout.as_str(),
                        ty1.as_str(),
                        ty2.as_str()
                    );
                }
                mgr.reset();
            }
        }
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L614-L680 (chrome/m156)
def_test!(UniformManagerMetalArrayLayout, |r| {
    let mut mgr = UniformManager::new(Layout::Metal);

    // Tests set up a uniform block with a single half (to force alignment) and an array of 3
    // elements. Test every type that can appear in an array.
    let k_buffer = K_BUFFER;
    let k_halfs = u16_bytes(&K_HALFS);
    // Each letter (A/B/a/b) corresponds to a single byte.
    // The expected uniform layout is listed as strings below.
    // A/B: uniform values.
    // a/b: padding as part of the uniform type.
    // _  : padding between uniforms for alignment.
    let k_expected_layout: &[&str] = &[
        // Each letter (A/B/a/b) corresponds to a single byte.
        // The expected uniform layout is listed as strings below.
        // A/B: uniform values.
        // a/b: padding as part of the uniform type.
        // _  : padding between uniforms for alignment.

        /* [half, float[3]]  */
        "AA__BBBBBBBBBBBB",
        /* [half, float2[3]] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3[3]] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4[3]] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half[3]]   */ "AABBBBBB",
        /* [half, half2[3]]  */ "AA__BBBBBBBBBBBB",
        /* [half, half3[3]]  */ "AA______BBBBBBbbBBBBBBbbBBBBBBbb",
        /* [half, half4[3]]  */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int[3]]    */ "AA__BBBBBBBBBBBB",
        /* [half, int2[3]]   */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int3[3]]   */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, int4[3]]   */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float2x2[3] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3x3[3] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4x4[3] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half2x2[3] */ "AA__BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half3x3[3] */
        "AA______BBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbb",
        /* [half, half4x4[3] */
        "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    ];
    for i in 0..k_expected_layout.len() {
        let array_type = K_TYPES[i];
        let expectations = [
            Uniform::new("a", SkSLType::Half),
            Uniform::new_array("b", array_type, K_ARRAY_SIZE),
        ];

        #[cfg(debug_assertions)]
        mgr.set_expected_uniforms(&expectations, false);
        mgr.write_uniform(&expectations[0], &k_halfs);
        mgr.write_uniform(&expectations[1], &k_buffer);
        #[cfg(debug_assertions)]
        mgr.done_with_expected_uniforms();

        let expected_size = k_expected_layout[i].len();
        let uniform_data_len = mgr.finish().len();
        reporter_assert!(
            r,
            uniform_data_len == expected_size,
            "array test {} for type {} failed - expected size: {}, actual size: {}",
            i,
            array_type.as_str(),
            expected_size,
            uniform_data_len
        );

        mgr.reset();
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L682-L748 (chrome/m156)
def_test!(UniformManagerStd430ArrayLayout, |r| {
    let mut mgr = UniformManager::new(Layout::Std430);

    // Tests set up a uniform block with a single half (to force alignment) and an array of 3
    // elements. Test every type that can appear in an array.
    let k_buffer = K_BUFFER;
    let k_halfs = u16_bytes(&K_HALFS);
    // Each letter (A/B/a/b) corresponds to a single byte.
    // The expected uniform layout is listed as strings below.
    // A/B: uniform values.
    // a/b: padding as part of the uniform type.
    // _  : padding between uniforms for alignment.
    let k_expected_layout: &[&str] = &[
        // Each letter (A/B/a/b) corresponds to a single byte.
        // The expected uniform layout is listed as strings below.
        // A/B: uniform values.
        // a/b: padding as part of the uniform type.
        // _  : padding between uniforms for alignment.

        /* [half, float[3]]  */
        "AA__BBBBBBBBBBBB",
        /* [half, float2[3]] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3[3]] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4[3]] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half[3]]   */ "AA__BBBBBBBBBBBB",
        /* [half, half2[3]]  */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half3[3]]  */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, half4[3]]  */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int[3]]    */ "AA__BBBBBBBBBBBB",
        /* [half, int2[3]]   */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int3[3]]   */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, int4[3]]   */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float2x2[3] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3x3[3] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4x4[3] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half2x2[3]  */ "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half3x3[3]  */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, half4x4[3]  */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    ];
    for i in 0..k_expected_layout.len() {
        let array_type = K_TYPES[i];
        let expectations = [
            Uniform::new("a", SkSLType::Half),
            Uniform::new_array("b", array_type, K_ARRAY_SIZE),
        ];

        #[cfg(debug_assertions)]
        mgr.set_expected_uniforms(&expectations, false);
        mgr.write_uniform(&expectations[0], &k_halfs);
        mgr.write_uniform(&expectations[1], &k_buffer);
        #[cfg(debug_assertions)]
        mgr.done_with_expected_uniforms();

        let expected_size = k_expected_layout[i].len();
        let uniform_data_len = mgr.finish().len();
        reporter_assert!(
            r,
            uniform_data_len == expected_size,
            "array test {} for type {} failed - expected size: {}, actual size: {}",
            i,
            array_type.as_str(),
            expected_size,
            uniform_data_len
        );

        mgr.reset();
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L750-L816 (chrome/m156)
def_test!(UniformManagerStd430F16ArrayLayout, |r| {
    let mut mgr = UniformManager::new(Layout::Std430F16);

    // Tests set up a uniform block with a single half (to force alignment) and an array of 3
    // elements. Test every type that can appear in an array.
    let k_buffer = K_BUFFER;
    let k_halfs = u16_bytes(&K_HALFS);
    // Each letter (A/B/a/b) corresponds to a single byte.
    // The expected uniform layout is listed as strings below.
    // A/B: uniform values.
    // a/b: padding as part of the uniform type.
    // _  : padding between uniforms for alignment.
    let k_expected_layout: &[&str] = &[
        // Each letter (A/B/a/b) corresponds to a single byte.
        // The expected uniform layout is listed as strings below.
        // A/B: uniform values.
        // a/b: padding as part of the uniform type.
        // _  : padding between uniforms for alignment.

        /* [half, float[3]]  */
        "AA__BBBBBBBBBBBB",
        /* [half, float2[3]] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3[3]] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4[3]] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half[3]]   */ "AABBBBBB",
        /* [half, half2[3]]  */ "AA__BBBBBBBBBBBB",
        /* [half, half3[3]]  */ "AA______BBBBBBbbBBBBBBbbBBBBBBbb",
        /* [half, half4[3]]  */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int[3]]    */ "AA__BBBBBBBBBBBB",
        /* [half, int2[3]]   */ "AA______BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, int3[3]]   */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, int4[3]]   */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float2x2[3] */ "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float3x3[3] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4x4[3] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half2x2[3]  */ "AA__BBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half3x3[3]  */
        "AA______BBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbbBBBBBBbb",
        /* [half, half4x4[3]  */
        "AA______BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    ];
    for i in 0..k_expected_layout.len() {
        let array_type = K_TYPES[i];
        let expectations = [
            Uniform::new("a", SkSLType::Half),
            Uniform::new_array("b", array_type, K_ARRAY_SIZE),
        ];

        #[cfg(debug_assertions)]
        mgr.set_expected_uniforms(&expectations, false);
        mgr.write_uniform(&expectations[0], &k_halfs);
        mgr.write_uniform(&expectations[1], &k_buffer);
        #[cfg(debug_assertions)]
        mgr.done_with_expected_uniforms();

        let expected_size = k_expected_layout[i].len();
        let uniform_data_len = mgr.finish().len();
        reporter_assert!(
            r,
            uniform_data_len == expected_size,
            "array test {} for type {} failed - expected size: {}, actual size: {}",
            i,
            array_type.as_str(),
            expected_size,
            uniform_data_len
        );

        mgr.reset();
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L818-L890 (chrome/m156)
def_test!(UniformManagerStd140ArrayLayout, |r| {
    let mut mgr = UniformManager::new(Layout::Std140);

    // Tests set up a uniform block with a single half (to force alignment) and an array of 3
    // elements. Test every type that can appear in an array.
    let k_buffer = K_BUFFER;
    let k_halfs = u16_bytes(&K_HALFS);
    // Each letter (A/B/a/b) corresponds to a single byte.
    // The expected uniform layout is listed as strings below.
    // A/B: uniform values.
    // a/b: padding as part of the uniform type.
    // _  : padding between uniforms for alignment.
    let k_expected_layout: &[&str] = &[
        // Each letter (A/B/a/b) corresponds to a single byte.
        // The expected uniform layout is listed as strings below.
        // A/B: uniform values.
        // a/b: padding as part of the uniform type.
        // _  : padding between uniforms for alignment.

        /* [half, float[3]]  */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, float2[3]] */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, float3[3]] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4[3]] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half[3]]   */
        "AA______________BBbbbbbbbbbbbbbbBBbbbbbbbbbbbbbbBBbbbbbbbbbbbbbb",
        /* [half, half2[3]]  */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, half3[3]]  */
        "AA______________BBBBBBbbbbbbbbbbBBBBBBbbbbbbbbbbBBBBBBbbbbbbbbbb",
        /* [half, half4[3]]  */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, int[3]]    */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, int2[3]]   */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, int3[3]]   */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, int4[3]]   */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float2x2[3] */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, float3x3[3] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4x4[3] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half2x2[3]  */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, half3x3[3]  */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, half4x4[3]  */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    ];
    for i in 0..k_expected_layout.len() {
        let array_type = K_TYPES[i];
        let expectations = [
            Uniform::new("a", SkSLType::Half),
            Uniform::new_array("b", array_type, K_ARRAY_SIZE),
        ];

        #[cfg(debug_assertions)]
        mgr.set_expected_uniforms(&expectations, false);
        mgr.write_uniform(&expectations[0], &k_halfs);
        mgr.write_uniform(&expectations[1], &k_buffer);
        #[cfg(debug_assertions)]
        mgr.done_with_expected_uniforms();

        let expected_size = k_expected_layout[i].len();
        let uniform_data_len = mgr.finish().len();
        reporter_assert!(
            r,
            uniform_data_len == expected_size,
            "array test {} for type {} failed - expected size: {}, actual size: {}",
            i,
            array_type.as_str(),
            expected_size,
            uniform_data_len
        );

        mgr.reset();
    }
});

// NOTE: Since arrays are aligned to 16 bytes, these cases end up matching kStd140 offsets
// Port of: tests/graphite/UniformManagerTest.cpp#L893-L965 (chrome/m156)
def_test!(UniformManagerStd140F16ArrayLayout, |r| {
    let mut mgr = UniformManager::new(Layout::Std140F16);

    // Tests set up a uniform block with a single half (to force alignment) and an array of 3
    // elements. Test every type that can appear in an array.
    let k_buffer = K_BUFFER;
    let k_halfs = u16_bytes(&K_HALFS);
    // Each letter (A/B/a/b) corresponds to a single byte.
    // The expected uniform layout is listed as strings below.
    // A/B: uniform values.
    // a/b: padding as part of the uniform type.
    // _  : padding between uniforms for alignment.
    let k_expected_layout: &[&str] = &[
        // Each letter (A/B/a/b) corresponds to a single byte.
        // The expected uniform layout is listed as strings below.
        // A/B: uniform values.
        // a/b: padding as part of the uniform type.
        // _  : padding between uniforms for alignment.

        /* [half, float[3]]  */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, float2[3]] */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, float3[3]] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4[3]] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half[3]]   */
        "AA______________BBbbbbbbbbbbbbbbBBbbbbbbbbbbbbbbBBbbbbbbbbbbbbbb",
        /* [half, half2[3]]  */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, half3[3]]  */
        "AA______________BBBBBBbbbbbbbbbbBBBBBBbbbbbbbbbbBBBBBBbbbbbbbbbb",
        /* [half, half4[3]]  */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, int[3]]    */
        "AA______________BBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbbBBBBbbbbbbbbbbbb",
        /* [half, int2[3]]   */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, int3[3]]   */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, int4[3]]   */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, float2x2[3] */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, float3x3[3] */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, float4x4[3] */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        /* [half, half2x2[3]  */
        "AA______________BBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbbBBBBBBBBbbbbbbbb",
        /* [half, half3x3[3]  */
        "AA______________BBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbbBBBBBBBBBBBBbbbb",
        /* [half, half4x4[3]  */
        "AA______________BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    ];
    for i in 0..k_expected_layout.len() {
        let array_type = K_TYPES[i];
        let expectations = [
            Uniform::new("a", SkSLType::Half),
            Uniform::new_array("b", array_type, K_ARRAY_SIZE),
        ];

        #[cfg(debug_assertions)]
        mgr.set_expected_uniforms(&expectations, false);
        mgr.write_uniform(&expectations[0], &k_halfs);
        mgr.write_uniform(&expectations[1], &k_buffer);
        #[cfg(debug_assertions)]
        mgr.done_with_expected_uniforms();

        let expected_size = k_expected_layout[i].len();
        let uniform_data_len = mgr.finish().len();
        reporter_assert!(
            r,
            uniform_data_len == expected_size,
            "array test {} for type {} failed - expected size: {}, actual size: {}",
            i,
            array_type.as_str(),
            expected_size,
            uniform_data_len
        );

        mgr.reset();
    }
});

// This test validates that the uniform data for matrix types get written out according to the
// layout expectations.
#[allow(clippy::float_cmp)] // The layout copies bits, so the exact comparison is the point.
fn expect_matrix(
    reporter: &mut Reporter,
    mgr: &mut UniformManager,
    ty: SkSLType,
    is_full_precision: bool,
    expected_size_in_bytes: usize,
    expected_offsets_in_primitives: &[usize],
) {
    let expectations = [Uniform::new("m", ty)];
    #[cfg(debug_assertions)]
    mgr.set_expected_uniforms(&expectations, false);
    mgr.write_uniform(&expectations[0], &f32_bytes(&K_FLOATS));
    #[cfg(debug_assertions)]
    mgr.done_with_expected_uniforms();

    let data = mgr.finish().to_vec();
    reporter_assert!(
        reporter,
        data.len() == expected_size_in_bytes,
        "{} layout size expected {}, got {}",
        ty.as_str(),
        expected_size_in_bytes,
        data.len()
    );

    if is_full_precision {
        for (index, &offset) in expected_offsets_in_primitives.iter().enumerate() {
            let el = f32_at(&data, offset);
            let expected = K_FLOATS[index];
            reporter_assert!(
                reporter,
                el == expected,
                "Incorrect {} element {} - expected {}, got {}",
                ty.as_str(),
                index,
                expected,
                el
            );
        }
    } else {
        for (index, &offset) in expected_offsets_in_primitives.iter().enumerate() {
            let el = u16_at(&data, offset);
            let expected = K_HALFS[index];
            reporter_assert!(
                reporter,
                el == expected,
                "Incorrect {} element {} - expected 0x{:04x}, got 0x{:04x}",
                ty.as_str(),
                index,
                expected,
                el
            );
        }
    }

    mgr.reset();
}

// Port of: tests/graphite/UniformManagerTest.cpp#L1011-L1025 (chrome/m156)
def_test!(UniformManagerStd140MatrixLayoutContents, |r| {
    let mut mgr = UniformManager::new(Layout::Std140);

    // 2x2
    for ty in [SkSLType::Float2x2, SkSLType::Half2x2] {
        expect_matrix(r, &mut mgr, ty, true, 32, &[0, 1, 4, 5]);
    }

    // 3x3
    for ty in [SkSLType::Float3x3, SkSLType::Half3x3] {
        expect_matrix(r, &mut mgr, ty, true, 48, &[0, 1, 2, 4, 5, 6, 8, 9, 10]);
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L1027-L1063 (chrome/m156)
def_test!(UniformManagerStd140F16MatrixLayoutContents, |r| {
    let mut mgr = UniformManager::new(Layout::Std140F16);

    // 2x2
    {
        expect_matrix(r, &mut mgr, SkSLType::Float2x2, true, 32, &[0, 1, 4, 5]);
        expect_matrix(r, &mut mgr, SkSLType::Half2x2, false, 32, &[0, 1, 8, 9]);
    }

    // 3x3
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float3x3,
            true,
            48,
            &[0, 1, 2, 4, 5, 6, 8, 9, 10],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half3x3,
            false,
            48,
            &[0, 1, 2, 8, 9, 10, 16, 17, 18],
        );
    }

    // 4x4
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float4x4,
            true,
            64,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half4x4,
            false,
            64,
            &[0, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19, 24, 25, 26, 27],
        );
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L1065-L1088 (chrome/m156)
def_test!(UniformManagerStd430MatrixLayoutContents, |r| {
    let mut mgr = UniformManager::new(Layout::Std430);

    // 2x2
    for ty in [SkSLType::Float2x2, SkSLType::Half2x2] {
        expect_matrix(r, &mut mgr, ty, true, 16, &[0, 1, 2, 3]);
    }

    // 3x3
    for ty in [SkSLType::Float3x3, SkSLType::Half3x3] {
        expect_matrix(r, &mut mgr, ty, true, 48, &[0, 1, 2, 4, 5, 6, 8, 9, 10]);
    }

    // 4x4
    for ty in [SkSLType::Float4x4, SkSLType::Half4x4] {
        expect_matrix(
            r,
            &mut mgr,
            ty,
            true,
            64,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L1090-L1126 (chrome/m156)
def_test!(UniformManagerStd430F16MatrixLayoutContents, |r| {
    let mut mgr = UniformManager::new(Layout::Std430F16);

    // 2x2
    {
        expect_matrix(r, &mut mgr, SkSLType::Float2x2, true, 16, &[0, 1, 2, 3]);
        expect_matrix(r, &mut mgr, SkSLType::Half2x2, false, 8, &[0, 1, 2, 3]);
    }

    // 3x3
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float3x3,
            true,
            48,
            &[0, 1, 2, 4, 5, 6, 8, 9, 10],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half3x3,
            false,
            24,
            &[0, 1, 2, 4, 5, 6, 8, 9, 10],
        );
    }

    // 4x4
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float4x4,
            true,
            64,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half4x4,
            false,
            32,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
    }
});

// Port of: tests/graphite/UniformManagerTest.cpp#L1128-L1164 (chrome/m156)
def_test!(UniformManagerMetalMatrixLayoutContents, |r| {
    let mut mgr = UniformManager::new(Layout::Metal);

    // 2x2
    {
        expect_matrix(r, &mut mgr, SkSLType::Float2x2, true, 16, &[0, 1, 2, 3]);
        expect_matrix(r, &mut mgr, SkSLType::Half2x2, false, 8, &[0, 1, 2, 3]);
    }

    // 3x3
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float3x3,
            true,
            48,
            &[0, 1, 2, 4, 5, 6, 8, 9, 10],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half3x3,
            false,
            24,
            &[0, 1, 2, 4, 5, 6, 8, 9, 10],
        );
    }

    // 4x4
    {
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Float4x4,
            true,
            64,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
        expect_matrix(
            r,
            &mut mgr,
            SkSLType::Half4x4,
            false,
            32,
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
    }
});

// These tests validate that substructs are written and aligned appropriately.
// One expected result per layout, in the order of K_LAYOUTS: the base alignment of the substruct,
// and its data as 16-bit units (`0` is padding).
struct TestCase {
    pre_struct: Vec<Uniform>,
    sub_struct: Vec<Uniform>,
    post_struct: Vec<Uniform>,
    expect: [(i32, Vec<u16>); 5],
    // If non-empty, holds base alignments for pre_struct base alignment as a struct.
    pre_struct_alignments: Vec<i32>,
}

// The values written to the fields: `gen16_bit` for 16-bit fields, `gen32_bit` for 32-bit ones.
fn gen16_bit(layout: Layout, base_value: u32) -> f32 {
    if skia_rust_gpu::graphite::uniform_manager::layout_rules::use_full_precision(layout) {
        f32::from_bits(base_value)
    } else {
        half_to_float(u16::try_from(base_value).expect("small test values"))
    }
}

fn gen32_bit(base_value: u32) -> f32 {
    f32::from_bits(base_value | (base_value << 16))
}

fn write_fields(mgr: &mut UniformManager, fields: &[Uniform], base_value: u32) -> u32 {
    let layout = mgr.layout();
    let mut base_value = base_value;
    let mut next = || {
        let v = base_value;
        base_value += 1;
        v
    };
    for f in fields {
        match f.ty() {
            SkSLType::Half => mgr.write_half(gen16_bit(layout, next())),
            SkSLType::Float => mgr.write_f32(gen32_bit(next())),
            SkSLType::Half2 => {
                let a = gen16_bit(layout, next());
                let b = gen16_bit(layout, next());
                mgr.write_half_vec([a, b]);
            }
            SkSLType::Float2 => {
                let a = gen32_bit(next());
                let b = gen32_bit(next());
                mgr.write_vec([a, b]);
            }
            SkSLType::Half3 => {
                let a = gen16_bit(layout, next());
                let b = gen16_bit(layout, next());
                let c = gen16_bit(layout, next());
                mgr.write_half_vec([a, b, c]);
            }
            SkSLType::Float3 => {
                let a = gen32_bit(next());
                let b = gen32_bit(next());
                let c = gen32_bit(next());
                mgr.write_vec([a, b, c]);
            }
            SkSLType::Half4 => {
                let a = gen16_bit(layout, next());
                let b = gen16_bit(layout, next());
                let c = gen16_bit(layout, next());
                let d = gen16_bit(layout, next());
                mgr.write_half_vec([a, b, c, d]);
            }
            SkSLType::Float4 => {
                let a = gen32_bit(next());
                let b = gen32_bit(next());
                let c = gen32_bit(next());
                let d = gen32_bit(next());
                mgr.write_vec([a, b, c, d]);
            }
            _ => unreachable!("StructLayout only writes half and float vectors"),
        }
    }
    base_value
}

// Port of: tests/graphite/UniformManagerTest.cpp#L1167-L1690 (chrome/m156)
def_test!(UniformManagerStructLayout, |r| {
    // skia-rust: not expressible in Rust: skiatest::ReporterContext labels (the harness has no
    // context stack), so the layout and case index are not added to the failure messages.
    let k_cases: Vec<TestCase> = vec![
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 2, 0, 3, 0, 4, 0]),
                (16, vec![1, 2, 3, 4, 0, 0, 0, 0]),
                (4, vec![1, 0, 2, 0, 3, 0, 4, 0]),
                (2, vec![1, 2, 3, 4]),
                (2, vec![1, 2, 3, 4]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float3),
                Uniform::new("u2", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4]),
                (16, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 0, 0, 0, 0, 0, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half3),
                Uniform::new("u2", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 2, 0, 3, 0, 4, 0]),
                (16, vec![1, 2, 3, 4, 0, 0, 0, 0]),
                (16, vec![1, 0, 2, 0, 3, 0, 4, 0]),
                (8, vec![1, 2, 3, 4]),
                (8, vec![1, 2, 3, 0, 4, 0, 0, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float2),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0, 0, 0, 0, 0]),
                (16, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0, 0, 0, 0, 0]),
                (8, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0]),
                (8, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0]),
                (8, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half2),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 0, 0, 0, 0, 0, 0]),
                (16, vec![1, 2, 3, 4, 5, 0, 0, 0]),
                (8, vec![1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 0, 0]),
                (4, vec![1, 2, 3, 4, 5, 0]),
                (4, vec![1, 2, 3, 4, 5, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float4),
                Uniform::new("u3", SkSLType::Float2),
                Uniform::new("u4", SkSLType::Float3),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0, 0, 0, 0, 8,
                        8, 9, 9, 10, 10, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0, 0, 0, 0, 8,
                        8, 9, 9, 10, 10, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0, 0, 0, 0, 8,
                        8, 9, 9, 10, 10, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0, 0, 0, 0, 8,
                        8, 9, 9, 10, 10, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0, 0, 0, 0, 8,
                        8, 9, 9, 10, 10, 0, 0,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half4),
                Uniform::new("u3", SkSLType::Half2),
                Uniform::new("u4", SkSLType::Half3),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 0, 0, 0, 0, 8,
                        0, 9, 0, 10, 0, 0, 0,
                    ],
                ),
                (16, vec![1, 0, 0, 0, 2, 3, 4, 5, 6, 7, 0, 0, 8, 9, 10, 0]),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 0, 0, 0, 0, 8,
                        0, 9, 0, 10, 0, 0, 0,
                    ],
                ),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 5, 6, 7, 0, 0, 8, 9, 10, 0]),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 5, 6, 7, 0, 0, 8, 9, 10, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5]),
                (4, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0]),
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 0, 0, 0, 0]),
                (4, vec![1, 0, 2, 0, 3, 0, 4, 0, 5, 0]),
                (2, vec![1, 2, 3, 4, 5]),
                (2, vec![1, 2, 3, 4, 5]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float3),
                Uniform::new("u2", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (16, vec![1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5]),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 0, 0, 5, 5, 0, 0, 0, 0, 0, 0,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half3),
                Uniform::new("u2", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0]),
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 0, 0, 0, 0]),
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0]),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 5]),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 0, 5, 0, 0, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float2),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 0, 0, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 0, 0, 0, 0,
                    ],
                ),
                (8, vec![1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0]),
                (8, vec![1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0]),
                (8, vec![1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half2),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0, 0, 0, 0, 0,
                    ],
                ),
                (16, vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 0, 0, 0]),
                (8, vec![1, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0]),
                (4, vec![1, 0, 2, 3, 4, 5, 6, 0]),
                (4, vec![1, 0, 2, 3, 4, 5, 6, 0]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float4),
                Uniform::new("u3", SkSLType::Float2),
                Uniform::new("u4", SkSLType::Float3),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half4),
                Uniform::new("u3", SkSLType::Half2),
                Uniform::new("u4", SkSLType::Half3),
            ],
            post_struct: vec![],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7,
                        0, 8, 0, 0, 0, 0, 0, 9, 0, 10, 0, 11, 0, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7,
                        0, 8, 0, 0, 0, 0, 0, 9, 0, 10, 0, 11, 0, 0, 0,
                    ],
                ),
                (
                    8,
                    vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0],
                ),
                (
                    8,
                    vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Float4)],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    4,
                    vec![
                        1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0, 0, 0, 0, 0, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    4,
                    vec![
                        1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0, 0, 0, 0, 0, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    4,
                    vec![
                        1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 0, 0, 0, 0, 0, 0, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Half4)],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 0, 0, 0, 0, 6, 7, 8, 9, 0, 0, 0, 0,
                    ],
                ),
                (
                    4,
                    vec![
                        1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 0, 0, 0, 0, 0, 0, 6, 0, 7, 0, 8, 0, 9, 0,
                    ],
                ),
                (2, vec![1, 2, 3, 4, 5, 0, 0, 0, 6, 7, 8, 9]),
                (2, vec![1, 2, 3, 4, 5, 0, 0, 0, 6, 7, 8, 9]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float3),
                Uniform::new("u2", SkSLType::Float),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Float4)],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 0, 0, 5, 5, 0, 0, 0, 0, 0, 0, 6,
                        6, 7, 7, 8, 8, 9, 9,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half3),
                Uniform::new("u2", SkSLType::Half),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Half4)],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 0, 0, 0, 0, 6, 7, 8, 9, 0, 0, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0,
                    ],
                ),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9]),
                (8, vec![1, 0, 0, 0, 2, 3, 4, 0, 5, 0, 0, 0, 6, 7, 8, 9]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float2),
                Uniform::new("u2", SkSLType::Float),
                Uniform::new("u3", SkSLType::Float),
                Uniform::new("u4", SkSLType::Float),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Float4)],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 0, 0, 0, 0, 7,
                        7, 8, 8, 9, 9, 10, 10,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 0, 0, 0, 0, 7,
                        7, 8, 8, 9, 9, 10, 10,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 7, 7, 8, 8, 9, 9, 10, 10,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 7, 7, 8, 8, 9, 9, 10, 10,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 1, 0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 0, 0, 7, 7, 8, 8, 9, 9, 10, 10,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half2),
                Uniform::new("u2", SkSLType::Half),
                Uniform::new("u3", SkSLType::Half),
                Uniform::new("u4", SkSLType::Half),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Half4)],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0, 0, 0, 0, 0, 7,
                        0, 8, 0, 9, 0, 10, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 0, 0, 0, 7, 8, 9, 10, 0, 0, 0, 0,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0, 7, 0, 8, 0, 9, 0, 10, 0,
                    ],
                ),
                (4, vec![1, 0, 2, 3, 4, 5, 6, 0, 7, 8, 9, 10]),
                (4, vec![1, 0, 2, 3, 4, 5, 6, 0, 7, 8, 9, 10]),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Float)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float4),
                Uniform::new("u3", SkSLType::Float2),
                Uniform::new("u4", SkSLType::Float3),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Float4)],
            expect: [
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0, 12, 12, 13, 13, 14, 14,
                        15, 15,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0, 12, 12, 13, 13, 14, 14,
                        15, 15,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0, 12, 12, 13, 13, 14, 14,
                        15, 15,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0, 12, 12, 13, 13, 14, 14,
                        15, 15,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 3, 3, 4, 4, 5, 5, 6, 6, 7,
                        7, 8, 8, 0, 0, 0, 0, 9, 9, 10, 10, 11, 11, 0, 0, 12, 12, 13, 13, 14, 14,
                        15, 15,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![Uniform::new("p1", SkSLType::Half)],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half4),
                Uniform::new("u3", SkSLType::Half2),
                Uniform::new("u4", SkSLType::Half3),
            ],
            post_struct: vec![Uniform::new("p2", SkSLType::Half4)],
            expect: [
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7,
                        0, 8, 0, 0, 0, 0, 0, 9, 0, 10, 0, 11, 0, 0, 0, 12, 0, 13, 0, 14, 0, 15, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0,
                        12, 13, 14, 15, 0, 0, 0, 0,
                    ],
                ),
                (
                    16,
                    vec![
                        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7,
                        0, 8, 0, 0, 0, 0, 0, 9, 0, 10, 0, 11, 0, 0, 0, 12, 0, 13, 0, 14, 0, 15, 0,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0, 12, 13, 14,
                        15,
                    ],
                ),
                (
                    8,
                    vec![
                        1, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 0, 12, 13, 14,
                        15,
                    ],
                ),
            ],
            pre_struct_alignments: vec![],
        },
        TestCase {
            pre_struct: vec![
                Uniform::new("p1", SkSLType::Float2),
                Uniform::new("p2", SkSLType::Float),
            ],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Float),
                Uniform::new("u2", SkSLType::Float),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 5, 5, 0, 0, 0, 0]),
                (16, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 5, 5, 0, 0, 0, 0]),
                (4, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 5, 5]),
                (4, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 5, 5]),
                (4, vec![1, 1, 2, 2, 3, 3, 0, 0, 4, 4, 5, 5]),
            ],
            pre_struct_alignments: vec![16, 16, 8, 8, 8],
        },
        TestCase {
            pre_struct: vec![
                Uniform::new("p1", SkSLType::Half2),
                Uniform::new("p2", SkSLType::Half),
            ],
            sub_struct: vec![
                Uniform::new("u1", SkSLType::Half),
                Uniform::new("u2", SkSLType::Half),
            ],
            post_struct: vec![],
            expect: [
                (16, vec![1, 0, 2, 0, 3, 0, 0, 0, 4, 0, 5, 0, 0, 0, 0, 0]),
                (16, vec![1, 2, 3, 0, 0, 0, 0, 0, 4, 5, 0, 0, 0, 0, 0, 0]),
                (4, vec![1, 0, 2, 0, 3, 0, 0, 0, 4, 0, 5, 0]),
                (2, vec![1, 2, 3, 0, 4, 5]),
                (2, vec![1, 2, 3, 0, 4, 5]),
            ],
            pre_struct_alignments: vec![16, 16, 8, 4, 4],
        },
    ];

    let mut data_match_failure_logged = false;
    for (l, &layout) in K_LAYOUTS.iter().enumerate() {
        for test in &k_cases {
            let (base_alignment, expected_data) = &test.expect[l];

            let mut mgr = UniformManager::new(layout);
            let mut base_value = 1;
            if !test.pre_struct.is_empty() {
                // pre-struct fields
                let pre_struct_is_struct = !test.pre_struct_alignments.is_empty();
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&test.pre_struct, pre_struct_is_struct);
                if pre_struct_is_struct {
                    mgr.begin_struct(test.pre_struct_alignments[l]);
                }
                base_value = write_fields(&mut mgr, &test.pre_struct, base_value);
                if pre_struct_is_struct {
                    mgr.end_struct();
                }
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();
            }
            if !test.sub_struct.is_empty() {
                // substruct fields
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&test.sub_struct, true);
                mgr.begin_struct(*base_alignment);
                base_value = write_fields(&mut mgr, &test.sub_struct, base_value);
                mgr.end_struct();
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();
            }
            if !test.post_struct.is_empty() {
                // post-struct fields
                #[cfg(debug_assertions)]
                mgr.set_expected_uniforms(&test.post_struct, false);
                base_value = write_fields(&mut mgr, &test.post_struct, base_value);
                #[cfg(debug_assertions)]
                mgr.done_with_expected_uniforms();
            }
            let _ = base_value;

            let data = mgr.finish().to_vec();
            let expected_bytes = u16_bytes(expected_data);

            let size_match = data.len() == expected_bytes.len();
            // To reduce logging/asserts, pretend contents "match" if the sizes differ since that
            // will already be triggering test failures.
            let contents_match = !size_match || data == expected_bytes;
            reporter_assert!(
                r,
                size_match,
                "Size mismatch between written ({}) and expected ({})",
                data.len(),
                expected_bytes.len()
            );
            reporter_assert!(
                r,
                contents_match,
                "Contents differ between written and expected"
            );

            if !contents_match && !data_match_failure_logged {
                // Print out actual and expected values once if it's only the contents that are
                // incorrect (don't bother printing contents if their lengths differ).
                eprintln!("Expected contents:");
                for (i, v) in expected_data.iter().enumerate() {
                    eprint!(
                        "{}{}",
                        if i % 8 == 0 {
                            " "
                        } else if i % 2 == 0 {
                            ","
                        } else {
                            ""
                        },
                        v
                    );
                }
                eprintln!("\nActual contents:");
                debug_assert!(data.len().is_multiple_of(8));
                for i in 0..data.len() / 2 {
                    let v = u16::from_ne_bytes([data[2 * i], data[2 * i + 1]]);
                    eprint!(
                        "{}{}",
                        if i % 8 == 0 {
                            " "
                        } else if i % 2 == 0 {
                            ","
                        } else {
                            ""
                        },
                        v
                    );
                }
                eprintln!("\n");
                data_match_failure_logged = true;
            }
        }
    }
});
