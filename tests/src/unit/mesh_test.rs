// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MeshTest.cpp (chrome/m156)

//! Custom mesh specifications and meshes: the validation of attributes, varyings, uniforms and
//! children, the errors the `SkSL` compiler reports for the shaders, and the varying analysis.

use std::fmt::Write as _;
use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color;
use skia_rust_core::color_filters;
use skia_rust_core::mesh::mesh_priv;
use skia_rust_core::mesh::meshes::make_vertex_buffer;
use skia_rust_core::mesh::{
    Attribute, AttributeType, Mesh, MeshSpecification, Mode, Varying, VaryingType,
};
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::uniform::Flags;
use skia_rust_core::runtime_effect::{ChildPtr, UniformType};
use skia_rust_core::shaders;

use crate::{Reporter, def_test, errorf, reporter_assert};

/// `attr_type_str`.
fn attr_type_str(ty: AttributeType) -> &'static str {
    match ty {
        AttributeType::Float => "float",
        AttributeType::Float2 => "float2",
        AttributeType::Float3 => "float3",
        AttributeType::Float4 => "float4",
        AttributeType::UByte4Unorm => "ubyte4_unorm",
    }
}

/// `var_type_str`.
fn var_type_str(ty: VaryingType) -> &'static str {
    match ty {
        VaryingType::Float => "float",
        VaryingType::Float2 => "float2",
        VaryingType::Float3 => "float3",
        VaryingType::Float4 => "float4",
        VaryingType::Half => "half",
        VaryingType::Half2 => "half2",
        VaryingType::Half3 => "half3",
        VaryingType::Half4 => "half4",
    }
}

/// `make_description`.
fn make_description(
    attributes: &[Attribute],
    stride: usize,
    varyings: &[Varying],
    vs: &str,
    fs: &str,
) -> String {
    const MAX: usize = 10;
    // Writing to a String cannot fail.
    let mut result = String::new();
    let _ = writeln!(
        result,
        "Attributes (count={}, stride={}):",
        attributes.len(),
        stride
    );
    for a in attributes.iter().take(MAX) {
        let _ = writeln!(
            result,
            " {{{:<10}, {:>3}, \"{}\"}}",
            attr_type_str(a.ty),
            a.offset,
            a.name
        );
    }
    if MAX < attributes.len() {
        result += " ...\n";
    }
    let _ = writeln!(result, "Varyings (count={}):", varyings.len());
    for v in varyings.iter().take(MAX) {
        let _ = writeln!(result, " {{{:>5}, \"{}\"}}", var_type_str(v.ty), v.name);
    }
    if MAX < varyings.len() {
        result += " ...\n";
    }
    let _ = write!(result, "\n--VS--\n{vs}\n------\n");
    let _ = write!(result, "\n--FS--\n{fs}\n------\n");
    result
}

/// `check_for_failure`: the specification must fail, with an error that contains
/// `expected_error_substring` if one is given.
fn check_for_failure(
    r: &mut Reporter,
    attributes: &[Attribute],
    stride: usize,
    varyings: &[Varying],
    vs: &str,
    fs: &str,
    expected_error_substring: Option<&str>,
) -> bool {
    let result = MeshSpecification::make(attributes, stride, varyings, vs, fs);
    if result.specification.is_some() {
        errorf!(
            r,
            "Expected to fail but succeeded:\n{}",
            make_description(attributes, stride, varyings, vs, fs)
        );
        return false;
    }
    if let Some(expected) = expected_error_substring
        && !result.error.contains(expected)
    {
        errorf!(
            r,
            "    Expected: {}\nActual error: {}\n",
            expected,
            result.error
        );
        return false;
    }
    true
}

/// `check_for_success`: the specification must succeed, and is returned.
fn check_for_success(
    r: &mut Reporter,
    attributes: &[Attribute],
    stride: usize,
    varyings: &[Varying],
    vs: &str,
    fs: &str,
) -> Option<Arc<MeshSpecification>> {
    let result = MeshSpecification::make(attributes, stride, varyings, vs, fs);
    if let Some(spec) = result.specification {
        reporter_assert!(r, result.error.is_empty());
        return Some(spec);
    }
    errorf!(
        r,
        "Expected to succeed but failed:\n{}Error:\n{}",
        make_description(attributes, stride, varyings, vs, fs),
        result.error
    );
    None
}

/// `check_for_success` with the specification discarded.
fn check_success(
    r: &mut Reporter,
    attributes: &[Attribute],
    stride: usize,
    varyings: &[Varying],
    vs: &str,
    fs: &str,
) -> bool {
    check_for_success(r, attributes, stride, varyings, vs, fs).is_some()
}

// Simple valid strings to make specifications
const K_VALID_VS: &str = r"
Varyings main(const Attributes attrs) {
    Varyings v;
    return v;
}";

// There are multiple valid VS signatures.
const K_VALID_FSES: [&str; 2] = [
    "float2 main(const Varyings varyings) { return float2(10); }",
    r"
            float2 main(const Varyings varyings, out half4 color) {
                color = half4(.2);
                return float2(10);
            }
        ",
];

// Simple valid attributes, stride, and varyings to make specifications
fn k_valid_attrs() -> Vec<Attribute> {
    vec![Attribute {
        ty: AttributeType::Float4,
        offset: 0,
        name: String::from("pos"),
    }]
}

const K_VALID_STRIDE: usize = 4 * 4;

fn k_valid_varyings() -> Vec<Varying> {
    vec![Varying {
        ty: VaryingType::Float2,
        name: String::from("uv"),
    }]
}

// Port of: tests/MeshTest.cpp#L167-L178 (chrome/m156)
def_test!(MeshSpec_Valid, |r| {
    for valid_fs in K_VALID_FSES {
        if !check_success(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            K_VALID_VS,
            valid_fs,
        ) {
            return;
        }
    }
});

// Port of: tests/MeshTest.cpp#L180-L254 (chrome/m156)
def_test!(MeshSpec_InvalidSignature, |r| {
    const K_VS_BODY: &str = "{ return float2(10); }";

    const K_INVALID_VS_SIGS: [&str; 5] = [
        "float3   main(const Attributes attrs)",   // bad return
        "Varyings main(Attributes attrs)",         // non-const Attributes
        "Varyings main(out Attributes attrs)",     // out Varyings
        "Varyings main()",                         // no Attributes
        "Varyings main(const Varyings v, float2)", // extra arg
    ];

    const K_NO_COLOR_FS_BODY: &str = "{ return float2(10); }";

    const K_INVALID_NO_COLOR_FS_SIGS: [&str; 6] = [
        "half2  main(const Varyings v)",      // bad return
        "float2 main(const Attributes v)",    // wrong param type
        "float2 main(inout Varyings attrs)",  // inout Varyings
        "float2 main(Varyings v)",            // non-const Varyings
        "float2 main()",                      // no args
        "float2 main(const Varyings, float)", // extra arg
    ];

    const K_COLOR_FS_BODY: &str = "{ color = half4(.2); return float2(10); }";

    const K_INVALID_COLOR_FS_SIGS: [&str; 6] = [
        "half2  main(const Varyings v, out half4 color)", // bad return
        "float2 main(const Attributes v, out half4 color)", // wrong first param type
        "float2 main(const Varyings v, out half3 color)", // wrong second param type
        "float2 main(out   Varyings v, out half4 color)", // out Varyings
        "float2 main(const Varyings v, half4 color)",     // in color
        "float2 main(const Varyings v, out half4 color, float)", // extra arg
    ];

    for vs_sig in K_INVALID_VS_SIGS {
        let invalid_vs = format!("{vs_sig} {K_VS_BODY}");
        for valid_fs in K_VALID_FSES {
            if !check_for_failure(
                r,
                &k_valid_attrs(),
                K_VALID_STRIDE,
                &k_valid_varyings(),
                &invalid_vs,
                valid_fs,
                None,
            ) {
                return;
            }
        }
    }

    for no_color_fs_sig in K_INVALID_NO_COLOR_FS_SIGS {
        let invalid_fs = format!("{no_color_fs_sig} {K_NO_COLOR_FS_BODY}");
        if !check_for_failure(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            K_VALID_VS,
            &invalid_fs,
            None,
        ) {
            return;
        }
    }

    for color_fs_sig in K_INVALID_COLOR_FS_SIGS {
        let invalid_fs = format!("{color_fs_sig} {K_COLOR_FS_BODY}");
        if !check_for_failure(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            K_VALID_VS,
            &invalid_fs,
            None,
        ) {
            return;
        }
    }
});

// We allow the optional out color from the FS to either be float4 or half4
// Port of: tests/MeshTest.cpp#L256-L270 (chrome/m156)
def_test!(MeshSpec_Float4Color, |r| {
    const K_FLOAT4_FS: &str = r"
            float2 main(const Varyings varyings, out float4 color) {
                color = float4(.2); return float2(10);
            }
        ";
    check_success(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_VALID_VS,
        K_FLOAT4_FS,
    );
});

// Port of: tests/MeshTest.cpp#L272-L306 (chrome/m156)
def_test!(MeshSpec_DisallowsChildEffectInVertex, |r| {
    const K_CHILD_EFFECTS: [&str; 3] = [
        "uniform shader myshader;",
        "uniform colorFilter mycolorfilter;",
        "uniform blender myblender;",
    ];

    for global in K_CHILD_EFFECTS {
        let vs_with_child = format!("{global}{K_VALID_VS}");
        let fs_with_child = format!("{global}{}", K_VALID_FSES[0]);

        if !check_for_failure(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            &vs_with_child,
            K_VALID_FSES[0],
            Some("effects are not permitted in mesh vertex shaders"),
        ) {
            return;
        }

        if !check_for_failure(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            &vs_with_child,
            &fs_with_child,
            Some("effects are not permitted in mesh vertex shaders"),
        ) {
            return;
        }
    }
});

// Port of: tests/MeshTest.cpp#L308-L328 (chrome/m156)
def_test!(MeshSpec_AllowsChildEffectInFragment, |r| {
    const K_CHILD_EFFECTS: [&str; 3] = [
        "uniform shader myshader;",
        "uniform colorFilter mycolorfilter; uniform shader myshader;",
        "uniform shader myshader; uniform blender myblender; uniform colorFilter mycolorfilter;",
    ];

    for global in K_CHILD_EFFECTS {
        let fs_with_child = format!("{global}{}", K_VALID_FSES[0]);

        if !check_success(
            r,
            &k_valid_attrs(),
            K_VALID_STRIDE,
            &k_valid_varyings(),
            K_VALID_VS,
            &fs_with_child,
        ) {
            return;
        }
    }
});

// Port of: tests/MeshTest.cpp#L330-L351 (chrome/m156)
def_test!(MeshSpec_FindChild, |r| {
    let fs_with_child = format!(
        "uniform shader myshader;uniform blender myblender;uniform colorFilter mycolorfilter;{}",
        K_VALID_FSES[0]
    );

    let Some(mesh_spec) = check_for_success(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_VALID_VS,
        &fs_with_child,
    ) else {
        return;
    };

    reporter_assert!(r, mesh_spec.find_child("myshader").unwrap().index() == 0);
    reporter_assert!(r, mesh_spec.find_child("myblender").unwrap().index() == 1);
    reporter_assert!(
        r,
        mesh_spec.find_child("mycolorfilter").unwrap().index() == 2
    );
    reporter_assert!(r, mesh_spec.find_child("missing").is_none());
});

const K_VERTEX_COUNT: usize = 4;

// Port of: tests/MeshTest.cpp#L353-L433 (chrome/m156)
def_test!(Mesh_ChildEffectsMatchSpec, |r| {
    // `test` in the C++ source: builds the specification, then the mesh with `children`.
    let test =
        |r: &mut Reporter, prefix: &str, children: &[ChildPtr], expected_error: Option<&str>| {
            let fs_with_child = format!("{prefix}{}", K_VALID_FSES[0]);

            let Some(mesh_spec) = check_for_success(
                r,
                &k_valid_attrs(),
                K_VALID_STRIDE,
                &k_valid_varyings(),
                K_VALID_VS,
                &fs_with_child,
            ) else {
                return;
            };

            let vertex_buffer = make_vertex_buffer(None, K_VALID_STRIDE * K_VERTEX_COUNT);
            let result = Mesh::make(
                Some(mesh_spec),
                Mode::TriangleStrip,
                Some(vertex_buffer),
                K_VERTEX_COUNT,
                0,
                None,
                children,
                Rect::new_empty(),
            );

            if let Some(expected) = expected_error {
                reporter_assert!(r, !result.mesh.is_valid());
                reporter_assert!(
                    r,
                    result.error.contains(expected),
                    "Expected: '{}'\n  Actual: '{}'\n",
                    expected,
                    result.error
                );
            } else {
                reporter_assert!(r, result.mesh.is_valid());
                reporter_assert!(
                    r,
                    result.error.is_empty(),
                    "Expected: no errors\n  Actual: '{}'\n",
                    result.error
                );
            }
        };

    let child_shader = [ChildPtr::Shader(shaders::color(Color::BLACK))];
    let child_filter = [ChildPtr::ColorFilter(color_filters::linear_to_srgb_gamma())];
    let child_blender = [ChildPtr::Blender(Blender::mode(BlendMode::SrcOver))];
    let child_null = [ChildPtr::Empty];

    // These are expected to report a count mismatch.
    test(
        r,
        "uniform shader myshader;",
        &[],
        Some("The mesh specification declares 1 child effects, but the mesh supplies 0."),
    );
    test(
        r,
        "",
        &child_shader,
        Some("The mesh specification declares 0 child effects, but the mesh supplies 1."),
    );

    // These are expected to report a type mismatch.
    test(
        r,
        "uniform shader myshader;",
        &child_filter,
        Some("Child effect 'myshader' was specified as a shader, but passed as a color filter."),
    );
    test(
        r,
        "uniform shader myshader;",
        &child_blender,
        Some("Child effect 'myshader' was specified as a shader, but passed as a blender."),
    );
    test(
        r,
        "uniform colorFilter myfilter;",
        &child_shader,
        Some("Child effect 'myfilter' was specified as a color filter, but passed as a shader."),
    );
    test(
        r,
        "uniform colorFilter myfilter;",
        &child_blender,
        Some("Child effect 'myfilter' was specified as a color filter, but passed as a blender."),
    );
    test(
        r,
        "uniform blender myblender;",
        &child_shader,
        Some("Child effect 'myblender' was specified as a blender, but passed as a shader."),
    );
    test(
        r,
        "uniform blender myblender;",
        &child_filter,
        Some("Child effect 'myblender' was specified as a blender, but passed as a color filter."),
    );

    // Null children are supported.
    test(r, "uniform shader myshader;", &child_null, None);
    test(r, "uniform shader myfilter;", &child_null, None);
    test(r, "uniform shader myblender;", &child_null, None);

    // Properly-typed child effects are supported.
    test(r, "uniform shader myshader;", &child_shader, None);
    test(r, "uniform colorFilter myfilter;", &child_filter, None);
    test(r, "uniform blender myblender;", &child_blender, None);
});

/// An expected `SkRuntimeEffect::Uniform` in the test's table: the fields the test compares.
struct ExpectedUniform {
    name: &'static str,
    offset: usize,
    ty: UniformType,
    count: i32,
    flags: Flags,
}

/// One case of `MeshSpec_ValidUniforms`: the VS and FS uniform declarations, and the uniforms the
/// specification should report.
struct UniformTestCase {
    vs_uniform_decls: Vec<&'static str>,
    fs_uniform_decls: Vec<&'static str>,
    expectations: Vec<ExpectedUniform>,
}

/// `make_uni`: a uniform with `count` elements (an array if `count` is non-zero).
fn make_uni(
    ty: UniformType,
    name: &'static str,
    offset: usize,
    flags: Flags,
    count: i32,
) -> ExpectedUniform {
    if count != 0 {
        ExpectedUniform {
            name,
            offset,
            ty,
            count,
            flags: flags | Flags::ARRAY,
        }
    } else {
        assert!(!flags.contains(Flags::ARRAY));
        ExpectedUniform {
            name,
            offset,
            ty,
            count: 1,
            flags,
        }
    }
}

// Port of: tests/MeshTest.cpp#L435-L663 (chrome/m156)
def_test!(
    // One test in Skia, ported as written.
    #[allow(clippy::too_many_lines)]
    MeshSpec_ValidUniforms,
    |r| {
        let k_vert = Flags::VERTEX;
        let k_frag = Flags::FRAGMENT;
        let k_color = Flags::COLOR;
        let k_halfp = Flags::HALF_PRECISION;

        // Each test case is a set of VS and FS uniform declarations followed and the expected output
        // of SkMeshSpecification::uniforms().
        let k_test_cases: Vec<UniformTestCase> = vec![
            // A single VS uniform.
            UniformTestCase {
                vs_uniform_decls: vec!["uniform float x;"],
                fs_uniform_decls: vec![],
                expectations: vec![make_uni(UniformType::Float, "x", 0, k_vert, 0)],
            },
            // A single FS uniform.
            UniformTestCase {
                vs_uniform_decls: vec![],
                fs_uniform_decls: vec!["uniform float2 v;"],
                expectations: vec![make_uni(UniformType::Float2, "v", 0, k_frag, 0)],
            },
            // A single uniform in both that uses color layout.
            UniformTestCase {
                vs_uniform_decls: vec!["layout(color) uniform float4 color;"],
                fs_uniform_decls: vec!["layout(color) uniform float4 color;"],
                expectations: vec![make_uni(
                    UniformType::Float4,
                    "color",
                    0,
                    k_vert | k_frag | k_color,
                    0,
                )],
            },
            // A shared uniform after an unshared vertex uniform
            UniformTestCase {
                vs_uniform_decls: vec![
                    "layout(color) uniform float4 color;",
                    "              uniform float x[5];",
                ],
                fs_uniform_decls: vec!["uniform float x[5];"],
                expectations: vec![
                    make_uni(UniformType::Float4, "color", 0, k_vert | k_color, 0),
                    make_uni(UniformType::Float, "x", 16, k_vert | k_frag, 5),
                ],
            },
            // A shared uniform before an unshared vertex uniform
            UniformTestCase {
                vs_uniform_decls: vec!["uniform half x[2];", "uniform int  y;"],
                fs_uniform_decls: vec!["uniform half x[2];"],
                expectations: vec![
                    make_uni(UniformType::Float, "x", 0, k_vert | k_frag | k_halfp, 2),
                    make_uni(UniformType::Int, "y", 8, k_vert, 0),
                ],
            },
            // A shared uniform after an unshared fragment uniform
            UniformTestCase {
                vs_uniform_decls: vec!["uniform float3x3 m;"],
                fs_uniform_decls: vec!["uniform int2     i2;", "uniform float3x3 m;"],
                expectations: vec![
                    make_uni(UniformType::Float3x3, "m", 0, k_vert | k_frag, 0),
                    make_uni(UniformType::Int2, "i2", 36, k_frag, 0),
                ],
            },
            // A shared uniform before an unshared fragment uniform
            UniformTestCase {
                vs_uniform_decls: vec!["uniform half4x4 m[4];"],
                fs_uniform_decls: vec!["uniform half4x4  m[4];", "uniform int3    i3[1];"],
                expectations: vec![
                    make_uni(UniformType::Float4x4, "m", 0, k_vert | k_frag | k_halfp, 4),
                    make_uni(UniformType::Int3, "i3", 256, k_frag, 1),
                ],
            },
            // Complex case with 2 shared uniforms that are declared in the opposite order.
            UniformTestCase {
                vs_uniform_decls: vec![
                    "uniform float   x;uniform half4x4 m[4];",     // m shared
                    "uniform int2    i2[2];uniform float3  v[8];", // v shared
                    "uniform int3    i3;",
                ],
                fs_uniform_decls: vec![
                    "uniform float   y;uniform float3  v[8];",     // v shared
                    "uniform int4    i4[2];uniform half4x4 m[4];", // m shared
                    "uniform int     i;",
                ],
                expectations: vec![
                    make_uni(UniformType::Float, "x", 0, k_vert, 0),
                    make_uni(UniformType::Float4x4, "m", 4, k_vert | k_frag | k_halfp, 4),
                    make_uni(UniformType::Int2, "i2", 260, k_vert, 2),
                    make_uni(UniformType::Float3, "v", 276, k_vert | k_frag, 8),
                    make_uni(UniformType::Int3, "i3", 372, k_vert, 0),
                    make_uni(UniformType::Float, "y", 384, k_frag, 0),
                    make_uni(UniformType::Int4, "i4", 388, k_frag, 2),
                    make_uni(UniformType::Int, "i", 420, k_frag, 0),
                ],
            },
        ];

        for c in &k_test_cases {
            let vs = format!("{}{K_VALID_VS}", c.vs_uniform_decls.concat());
            let fs = format!("{}{}", c.fs_uniform_decls.concat(), K_VALID_FSES[0]);

            let attrs = k_valid_attrs();
            let varys = k_valid_varyings();
            let Some(spec) = check_for_success(r, &attrs, K_VALID_STRIDE, &varys, &vs, &fs) else {
                return;
            };
            let desc = make_description(&attrs, K_VALID_STRIDE, &varys, &vs, &fs);
            let uniforms = spec.uniforms();
            if uniforms.len() != c.expectations.len() {
                errorf!(
                    r,
                    "Expected {} uniforms but actually {}:\n{}",
                    c.expectations.len(),
                    uniforms.len(),
                    desc
                );
                return;
            }
            for (actual, expected) in uniforms.iter().zip(&c.expectations) {
                let name = actual.name();
                if name != expected.name {
                    errorf!(
                        r,
                        "Actual uniform name ({}) does not match expected name ({})",
                        name,
                        expected.name
                    );
                    return;
                }
                if actual.ty() != expected.ty {
                    errorf!(
                        r,
                        "Uniform {}: Actual type ({:?}) does not match expected type ({:?})",
                        name,
                        actual.ty(),
                        expected.ty
                    );
                    return;
                }
                if actual.count() != expected.count {
                    errorf!(
                        r,
                        "Uniform {}: Actual count ({}) does not match expected count ({})",
                        name,
                        actual.count(),
                        expected.count
                    );
                    return;
                }
                if actual.flags() != expected.flags {
                    errorf!(
                        r,
                        "Uniform {}: Actual flags ({:#06x}) do not match expected flags ({:#06x})",
                        name,
                        actual.flags().bits(),
                        expected.flags.bits()
                    );
                    return;
                }
                if actual.offset() != expected.offset {
                    errorf!(
                        r,
                        "Uniform {}: Actual offset ({}) does not match expected offset ({})",
                        name,
                        actual.offset(),
                        expected.offset
                    );
                    return;
                }
            }
        }
    }
);

// Port of: tests/MeshTest.cpp#L665-L702 (chrome/m156)
def_test!(MeshSpec_InvalidUniforms, |r| {
    // We assume general uniform declarations are broadly tested generically in SkSL. Here we are
    // concerned with agreement between VS and FS declarations, which is a unique aspect of
    // SkMeshSpecification.

    // Each test case is a fs and vs uniform declaration with the same name but some other
    // difference that should make them incompatible.
    const K_TEST_CASES: [(&str, &str); 4] = [
        // different types
        ("uniform float x;", "uniform int x;"),
        // array vs non-array
        ("uniform float2x2 m[1];", "uniform float2x2 m;"),
        // array count mismatch
        ("uniform int3 i[1];", "uniform int3 i[2];"),
        // layout difference
        (
            "layout(color) uniform float4 color;",
            "uniform float4 color;",
        ),
    ];

    for reverse in [false, true] {
        for (u1, u2) in K_TEST_CASES {
            let (u1, u2) = if reverse { (u2, u1) } else { (u1, u2) };
            let vs = format!("{u1}{K_VALID_VS}");
            let fs = format!("{u2}{}", K_VALID_FSES[0]);

            if !check_for_failure(
                r,
                &k_valid_attrs(),
                K_VALID_STRIDE,
                &k_valid_varyings(),
                &vs,
                &fs,
                None,
            ) {
                return;
            }
        }
    }
});

// Port of: tests/MeshTest.cpp#L704-L746 (chrome/m156)
def_test!(MeshSpec_MissingMain, |r| {
    const K_HELPER: &str = "float2 swiz(float2 x) { return z.yx; }";
    // Empty VS
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        "",
        K_VALID_FSES[0],
        None,
    ) {
        return;
    }
    // VS with helper function but no main
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_HELPER,
        K_VALID_FSES[0],
        None,
    ) {
        return;
    }
    // Empty FS
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_VALID_VS,
        "",
        None,
    ) {
        return;
    }
    // VS with helper function but no main
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_VALID_VS,
        K_HELPER,
        None,
    ) {
        return;
    }
});

// Port of: tests/MeshTest.cpp#L748-L756 (chrome/m156)
def_test!(MeshSpec_ZeroAttributes, |r| {
    // We require at least one attribute
    check_for_failure(
        r,
        &[],
        K_VALID_STRIDE,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L758-L766 (chrome/m156)
def_test!(MeshSpec_ZeroVaryings, |r| {
    // Varyings are not required.
    check_success(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &[],
        K_VALID_VS,
        K_VALID_FSES[0],
    );
});

// Port of: tests/MeshTest.cpp#L768-L798 (chrome/m156)
def_test!(MeshSpec_InvalidStride, |r| {
    // Zero stride
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        0,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    ) {
        return;
    }
    // Unaligned
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE + 1,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    ) {
        return;
    }
    // Too large
    if !check_for_failure(
        r,
        &k_valid_attrs(),
        1 << 20,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    ) {
        return;
    }
});

// Port of: tests/MeshTest.cpp#L800-L841 (chrome/m156)
def_test!(MeshSpec_InvalidOffset, |r| {
    {
        // offset isn't aligned
        let attributes = [Attribute {
            ty: AttributeType::Float4,
            offset: 1,
            name: String::from("var"),
        }];
        if !check_for_failure(
            r,
            &attributes,
            32,
            &k_valid_varyings(),
            K_VALID_VS,
            K_VALID_FSES[0],
            None,
        ) {
            return;
        }
    }
    {
        // straddles stride boundary
        let attributes = [
            Attribute {
                ty: AttributeType::Float4,
                offset: 0,
                name: String::from("var"),
            },
            Attribute {
                ty: AttributeType::Float2,
                offset: 16,
                name: String::from("var"),
            },
        ];
        if !check_for_failure(
            r,
            &attributes,
            20,
            &k_valid_varyings(),
            K_VALID_VS,
            K_VALID_FSES[0],
            None,
        ) {
            return;
        }
    }
    {
        // straddles stride boundary with attempt to overflow
        let attributes = [Attribute {
            ty: AttributeType::Float,
            offset: usize::MAX - 3,
            name: String::from("var"),
        }];
        if !check_for_failure(
            r,
            &attributes,
            4,
            &k_valid_varyings(),
            K_VALID_VS,
            K_VALID_FSES[0],
            None,
        ) {
            return;
        }
    }
});

// Port of: tests/MeshTest.cpp#L843-L856 (chrome/m156)
def_test!(MeshSpec_TooManyAttributes, |r| {
    const K_N: usize = 500;
    let attrs: Vec<Attribute> = (0..K_N)
        .map(|i| Attribute {
            ty: AttributeType::Float4,
            offset: 0,
            name: format!("attr{i}"),
        })
        .collect();
    check_for_failure(
        r,
        &attrs,
        4 * 4,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L858-L871 (chrome/m156)
def_test!(MeshSpec_TooManyVaryings, |r| {
    const K_N: usize = 500;
    let varyings: Vec<Varying> = (0..K_N)
        .map(|i| Varying {
            ty: VaryingType::Float4,
            name: format!("varying{i}"),
        })
        .collect();
    check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L873-L884 (chrome/m156)
def_test!(MeshSpec_DuplicateAttributeNames, |r| {
    let attributes = [
        Attribute {
            ty: AttributeType::Float4,
            offset: 0,
            name: String::from("var"),
        },
        Attribute {
            ty: AttributeType::Float2,
            offset: 16,
            name: String::from("var"),
        },
    ];
    check_for_failure(
        r,
        &attributes,
        24,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L886-L899 (chrome/m156)
def_test!(MeshSpec_DuplicateVaryingNames, |r| {
    let varyings = [
        Varying {
            ty: VaryingType::Float4,
            name: String::from("var"),
        },
        Varying {
            ty: VaryingType::Float3,
            name: String::from("var"),
        },
    ];
    check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

const K_SNEAKY_NAME: &str = "name; float3 sneaky";

// Port of: tests/MeshTest.cpp#L901-L911 (chrome/m156)
def_test!(MeshSpec_SneakyExtraAttribute, |r| {
    let attributes = [Attribute {
        ty: AttributeType::Float4,
        offset: 0,
        name: String::from(K_SNEAKY_NAME),
    }];
    check_for_failure(
        r,
        &attributes,
        16,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L913-L923 (chrome/m156)
def_test!(MeshSpec_SneakyExtraVarying, |r| {
    let varyings = [Varying {
        ty: VaryingType::Float4,
        name: String::from(K_SNEAKY_NAME),
    }];
    check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L925-L936 (chrome/m156)
def_test!(MeshSpec_AllowsFloat2PositionVarying, |r| {
    // Position varying can be explicit if it is float2
    let varyings = [Varying {
        ty: VaryingType::Float2,
        name: String::from("position"),
    }];
    check_success(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
    );
});

// Port of: tests/MeshTest.cpp#L938-L949 (chrome/m156)
def_test!(MeshSpec_InvalidPositionType, |r| {
    // Position varying can be explicit but it must be float2
    let varyings = [Varying {
        ty: VaryingType::Float4,
        name: String::from("position"),
    }];
    check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L951-L961 (chrome/m156)
def_test!(MeshSpec_EmptyAttributeName, |r| {
    let attributes = [Attribute {
        ty: AttributeType::Float4,
        offset: 0,
        name: String::new(),
    }];
    check_for_failure(
        r,
        &attributes,
        16,
        &k_valid_varyings(),
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

// Port of: tests/MeshTest.cpp#L963-L973 (chrome/m156)
def_test!(MeshSpec_EmptyVaryingName, |r| {
    let varyings = [Varying {
        ty: VaryingType::Float4,
        name: String::new(),
    }];
    check_for_failure(
        r,
        &k_valid_attrs(),
        K_VALID_STRIDE,
        &varyings,
        K_VALID_VS,
        K_VALID_FSES[0],
        None,
    );
});

/// The attributes of the passthrough and unused-varying tests.
fn passthrough_attributes() -> [Attribute; 3] {
    [
        Attribute {
            ty: AttributeType::Float2,
            offset: 0,
            name: String::from("position"),
        },
        Attribute {
            ty: AttributeType::Float2,
            offset: 8,
            name: String::from("uv"),
        },
        Attribute {
            ty: AttributeType::UByte4Unorm,
            offset: 16,
            name: String::from("color"),
        },
    ]
}

/// The varyings of the passthrough and unused-varying tests.
fn passthrough_varyings() -> [Varying; 3] {
    [
        Varying {
            ty: VaryingType::Float2,
            name: String::from("position"),
        },
        Varying {
            ty: VaryingType::Float2,
            name: String::from("uv"),
        },
        Varying {
            ty: VaryingType::Half4,
            name: String::from("color"),
        },
    ]
}

/// The vertex shader of the passthrough and unused-varying tests.
const PASSTHROUGH_VS: &str = r"
            Varyings main(const Attributes a) {
                Varyings v;
                v.uv       = a.uv;
                v.position = a.position;
                v.color    = a.color;
                return v;
            }
    ";

// Port of: tests/MeshTest.cpp#L975-L1089 (chrome/m156)
def_test!(
    // One test in Skia, ported as written.
    #[allow(clippy::too_many_lines)]
    MeshSpecVaryingPassthrough,
    |r| {
        let check = |r: &mut Reporter, fs: &str, passthrough_attr: Option<&str>| {
            let result = MeshSpecification::make(
                &passthrough_attributes(),
                24,
                &passthrough_varyings(),
                PASSTHROUGH_VS,
                fs,
            );
            let Some(spec) = result.specification else {
                errorf!(r, "{}\n{}", fs, result.error);
                return;
            };
            let idx = mesh_priv::passthrough_local_coords_varying_index(&spec);
            let actual_attr: String = if let Ok(index) = usize::try_from(idx) {
                spec.attributes()[index].name.clone()
            } else {
                String::from("<none>")
            };
            match passthrough_attr {
                None => {
                    if idx >= 0 {
                        errorf!(
                            r,
                            "Expected no passthrough coords attribute, found {}.\n{}",
                            actual_attr,
                            fs
                        );
                    }
                }
                Some(expected) => {
                    if actual_attr != expected {
                        errorf!(
                            r,
                            "Expected {} as passthrough coords attribute, found {}.\n{}",
                            expected,
                            actual_attr,
                            fs
                        );
                    }
                }
            }
        };

        // Simple
        check(
            r,
            r"float2 main(const Varyings v) {
                  return v.uv;
              }",
            Some("uv"),
        );

        // Simple, using position
        check(
            r,
            r"float2 main(const Varyings v) {
                  return v.position;
              }",
            Some("position"),
        );

        // Simple, with output color
        check(
            r,
            r"float2 main(const Varyings v, out half4 color) {
                  color = v.color;
                  return v.uv;
              }",
            Some("uv"),
        );

        // Three returns, all the same.
        check(
            r,
            r"uniform int selector;
             float2 main(const Varyings v, out half4 color) {
                  if (selector == 0) {
                      color = half4(1, 0, 0, 1);
                      return v.position;
                  }
                  if (selector == 1) {
                      color = half4(1, 1, 0, 1);
                      return v.position;
                  }
                  color = half4(1, 0, 1, 1);
                  return v.position;
             }",
            Some("position"),
        );

        // Three returns, one not like the others
        check(
            r,
            r"uniform int selector;
             float2 main(const Varyings v, out half4 color) {
                  if (selector == 0) {
                      color = color.bgra;
                      return v.position;
                  }
                  if (selector == 1) {
                      color = half4(1);
                      return v.uv;
                  }
                  color = color;
                  return v.position;
             }",
            None,
        );

        // Swizzles aren't handled (yet?).
        check(
            r,
            r"float2 main(const Varyings v) {
                  return v.uv.yx;
              }",
            None,
        );

        // Return from non-main fools us?
        check(
            r,
            r"noinline half4 get_color(const Varyings v) { return v.color; }
             float2 main(const Varyings v, out half4 color) {
                  color = get_color(v);
                  return v.position;
              }",
            Some("position"),
        );
    }
);

// Port of: tests/MeshTest.cpp#L1091-L1246 (chrome/m156)
def_test!(
    // One test in Skia, ported as written.
    #[allow(clippy::too_many_lines)]
    MeshSpecUnusedVaryings,
    |r| {
        let check =
            |r: &mut Reporter, fs: &str, position_dead: bool, uv_dead: bool, color_dead: bool| {
                let result = MeshSpecification::make(
                    &passthrough_attributes(),
                    24,
                    &passthrough_varyings(),
                    PASSTHROUGH_VS,
                    fs,
                );
                let Some(spec) = result.specification else {
                    errorf!(r, "{}\n{}", fs, result.error);
                    return;
                };

                let position_actually_dead = mesh_priv::varying_is_dead(&spec, 0);
                let uv_actually_dead = mesh_priv::varying_is_dead(&spec, 1);
                let color_actually_dead = mesh_priv::varying_is_dead(&spec, 2);
                let str_of = |dead: bool| if dead { "dead" } else { "not dead" };
                if position_actually_dead != position_dead {
                    errorf!(
                        r,
                        "Expected position to be detected {} but it is detected {}.\n{}",
                        str_of(position_dead),
                        str_of(position_actually_dead),
                        fs
                    );
                }
                if uv_actually_dead != uv_dead {
                    errorf!(
                        r,
                        "Expected uv to be detected {} but it is detected {}.\n{}",
                        str_of(uv_dead),
                        str_of(uv_actually_dead),
                        fs
                    );
                }
                if color_actually_dead != color_dead {
                    errorf!(
                        r,
                        "Expected color to be detected {} but it is detected {}.\n{}",
                        str_of(color_dead),
                        str_of(color_actually_dead),
                        fs
                    );
                }
            };

        // Simple
        check(
            r,
            r"float2 main(const Varyings v) {
                 return v.uv;
             }",
            true,
            true,
            true,
        );

        // Simple, using position
        check(
            r,
            r"float2 main(const Varyings v) {
                 return v.position;
             }",
            true,
            true,
            true,
        );

        // Two returns that are both passthrough of the same varying
        check(
            r,
            r"float2 main(const Varyings v, out half4 color) {
                 if (v.color.r > 0.5) {
                     color = v.color;
                     return v.uv;
                 } else {
                     color = 2*color;
                     return v.uv;
                 }
             }",
            true,
            true,
            false,
        );

        // Two returns that are both passthrough of the different varyings and unused other varying
        check(
            r,
            r"float2 main(const Varyings v, out half4 color) {
                 if (v.position.x > 10) {
                     color = half4(0);
                     return v.uv;
                 } else {
                     color = half4(1);
                     return v.position;
                 }
             }",
            false,
            false,
            true,
        );

        // Passthrough but we also use the varying elsewhere
        check(
            r,
            r"float2 main(const Varyings v, out half4 color) {
                 color = half4(v.uv.x, 0, 0, 1);
                 return v.uv;
             }",
            true,
            false,
            true,
        );

        // Use two varyings is a return statement
        check(
            r,
            r"float2 main(const Varyings v) {
                  return v.uv + v.position;
              }",
            false,
            false,
            true,
        );

        // Slightly more complicated varying use.
        check(
            r,
            r"noinline vec2 get_pos(const Varyings v) { return v.position; }
             noinline half4 identity(half4 c) { return c; }
             float2 main(const Varyings v, out half4 color) {
                 color = identity(v.color);
                 return v.uv + get_pos(v);
             }",
            false,
            false,
            false,
        );

        // Go through assignment to another Varyings.
        check(
            r,
            r"float2 main(const Varyings v) {
                 Varyings otherVaryings;
                 otherVaryings = v;
                 return otherVaryings.uv;
             }",
            true,
            false,
            true,
        );

        // We're not very smart. We just look for any use of the field in any Varyings value and don't
        // do any data flow analysis.
        check(
            r,
            r"float2 main(const Varyings v) {
                 Varyings otherVaryings;
                 otherVaryings.uv       = half2(5);
                 otherVaryings.position = half2(10);
                 return otherVaryings.position;
             }",
            false,
            false,
            true,
        );
    }
);
