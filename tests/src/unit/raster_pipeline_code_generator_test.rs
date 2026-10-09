// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RasterPipelineCodeGeneratorTest.cpp (chrome/m156)

//! `SkSL` programs compiled to Raster Pipeline stages, run over one pixel.
//!
//! Mapping notes: `SkArenaAlloc alloc(1000)` is an `ArenaAlloc`; `SkRasterPipeline pipeline(&alloc)`
//! is a `RasterPipeline`. The pixel `out` array and the program's slot memory are bound to the
//! memory slots the pipeline's stages name (`MemoryBindings`), the slot memory to
//! `SHADER_SCRATCH` as `docs/design/sksl.md` §6.5 describes.

// The ported tests keep the C++ declaration order and function lengths.
#![allow(
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::format_push_string
)]

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::color::Color4f;
use skia_rust_core::effect_priv::SHADER_SCRATCH;
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_simd::rp::contexts::MAX_STRIDE_HIGHP;
use skia_rust_sksl::codegen::rp::make_raster_pipeline_program;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings, Version};
use skia_rust_sksl::tracing::DebugTracePriv;

use crate::{Reporter, def_test, errorf, reporter_assert};

/// `SkColor4f{r, g, b, a}`.
fn color(r: f32, g: f32, b: f32, a: f32) -> Color4f {
    Color4f { r, g, b, a }
}

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L31-L105 (chrome/m156)
fn test(
    r: &mut Reporter,
    src: &str,
    uniforms: &[f32],
    starting_color: Color4f,
    expected_result: Option<Color4f>,
) {
    let mut compiler = Compiler::new();
    let settings = ProgramSettings {
        max_version_allowed: Version::K300,
        ..ProgramSettings::default()
    };
    let Some(mut program) =
        compiler.convert_program(ProgramKind::RuntimeColorFilter, src.as_bytes(), settings)
    else {
        errorf!(
            r,
            "Unexpected error compiling {}\n{}",
            src,
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        return;
    };
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    let Some(main) = main else {
        errorf!(r, "Program must have a 'main' function");
        return;
    };
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    pipeline.append_constant_color4f(&alloc, &starting_color);
    let debug_trace = DebugTracePriv::default();
    let raster_prog = make_raster_pipeline_program(&mut program, main, Some(debug_trace), false);
    if raster_prog.is_none() && expected_result.is_none() {
        // We didn't get a program, as expected. Test passes.
        return;
    }
    let (Some(raster_prog), Some(expected_result)) = (raster_prog, expected_result) else {
        if expected_result.is_some() {
            errorf!(r, "MakeRasterPipelineProgram failed");
        } else {
            errorf!(
                r,
                "MakeRasterPipelineProgram should have failed, but didn't"
            );
        }
        return;
    };

    // Append the SkSL program to the raster pipeline.
    raster_prog.append_stages(&mut pipeline, &alloc, None, uniforms);

    // Move the float values from RGBA into an 8888 memory buffer.
    let out_ctx = MemoryCtx::new(MemSlot(0));
    pipeline.append(Stage::Store8888(out_ctx));
    let mut out = [0_u8; 4 * MAX_STRIDE_HIGHP];
    let mut scratch = alloc.scratch_buffer();
    {
        let mut mem = MemoryBindings::new()
            .with(MemSlot(0), MemView::write(&mut out))
            .with(SHADER_SCRATCH, MemView::write(&mut scratch));
        pipeline.run(0, 0, 1, 1, &mut mem);
    }
    let out: Vec<u32> = out
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect();

    // Make sure the first pixel (exclusively) of `out` matches RGBA.
    let expected = expected_result.to_bytes();
    reporter_assert!(
        r,
        out[0] == expected,
        "Got:{:02X}{:02X}{:02X}{:02X} Expected:{:02X}{:02X}{:02X}{:02X}",
        (out[0] >> 24) & 0xFF,
        (out[0] >> 16) & 0xFF,
        (out[0] >> 8) & 0xFF,
        out[0] & 0xFF,
        (expected >> 24) & 0xFF,
        (expected >> 16) & 0xFF,
        (expected >> 8) & 0xFF,
        expected & 0xFF
    );

    // Make sure the rest of the pixels are untouched.
    for &pixel in &out[1..] {
        reporter_assert!(r, pixel == 0);
    }
}

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L107-L120 (chrome/m156)
def_test!(SkSLRasterPipelineCodeGeneratorNestedTernaryTest, |r| {
    // Add in your SkSL here.
    test(
        r,
        r"
             half4 main(half4) {
                 half three = 3, one = 1, two = 2;
                 half result = (three > (one > two ? 2.0 : 5.0)) ? 1.0 : 0.499;
                 return half4(result);
             }
         ",
        &[],
        color(0.0, 0.0, 0.0, 0.0),
        Some(color(0.499, 0.499, 0.499, 0.499)),
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L122-L142 (chrome/m156)
def_test!(SkSLRasterPipelineCodeGeneratorArithmeticTest, |r| {
    test(
        r,
        r"
            half4 main(half4) {
                const half4 colorGreen = half4(0,1,0,1), colorRed = half4(1,0,0,1);

                half a = 3.0, b = 4.0, c = a + b - 2.0;
                if (a*a + b*b == c*c*c/5.0) {
                    int A = 3, B = 4, C = A + B - 2;
                    if (A*A + B*B == C*C*C/5) {
                        return colorGreen;
                    }
                }

                return colorRed;
            }
         ",
        &[],
        color(0.0, 0.0, 0.0, 0.0),
        Some(color(0.0, 1.0, 0.0, 1.0)),
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L144-L159 (chrome/m156)
def_test!(SkSLRasterPipelineCodeGeneratorCoercedTypeTest, |r| {
    const UNIFORMS: [f32; 8] = [0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    test(
        r,
        r"
             uniform half4 colorGreen;
             uniform float4 colorRed;
             half4 main(half4 color) {
                 return ((colorGreen + colorRed) == float4(1.0, 1.0, 0.0, 2.0)) ? colorGreen
                                                                                : colorGreen.gr01;
             }
         ",
        &UNIFORMS,
        color(0.0, 0.0, 0.0, 0.0),
        Some(color(0.0, 1.0, 0.0, 1.0)),
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L161-L185 (chrome/m156)
def_test!(SkSLRasterPipelineCodeGeneratorIdentitySwizzle, |r| {
    const UNIFORMS: [f32; 8] = [0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    test(
        r,
        r"

uniform half4 colorGreen, colorRed;

const int SEVEN = 7, TEN = 10;
const half4x4 MATRIXFIVE = half4x4(5);

noinline bool verify_const_globals(int seven, int ten, half4x4 matrixFive) {
    return seven == 7 && ten == 10 && matrixFive == half4x4(5);
}

half4 main(float4) {
    return verify_const_globals(SEVEN, TEN, MATRIXFIVE) ? colorGreen : colorRed;
}

         ",
        &UNIFORMS,
        color(0.5, 1.0, 0.0, 0.25),
        Some(color(0.0, 1.0, 0.0, 1.0)),
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L187-L205 (chrome/m156)
def_test!(SkSLRasterPipelineCodeGeneratorBitwiseNotTest, |r| {
    const INT_UNIFORMS: [i32; 8] = [0, 12, 3456, 4_567_890, !0, !12, !3456, !4_567_890];
    // `SkSpan((const float*)kUniforms, std::size(kUniforms))`: the same bits, as floats.
    let uniforms = INT_UNIFORMS.map(|i| f32::from_bits(i.cast_unsigned()));
    test(
        r,
        r"
            uniform int4 value, expected;
            const half4 colorGreen = half4(0,1,0,1), colorRed = half4(1,0,0,1);

            half4 main(vec4) {
                return (~value.x    == expected.x     &&
                        ~value.xy   == expected.xy    &&
                        ~value.xyz  == expected.xyz   &&
                        ~value.xyzw == expected.xyzw) ? colorGreen : colorRed;
            }
         ",
        &uniforms,
        color(0.0, 0.0, 0.0, 0.0),
        Some(color(0.0, 1.0, 0.0, 1.0)),
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L207-L253 (chrome/m156)
def_test!(
    SkSLRasterPipelineCodeGeneratorComparisonIntrinsicTest,
    |r| {
        test(
            r,
            r"
            half4 main(vec4) {
                const half4 colorGreen = half4(0,1,0,1), colorRed = half4(1,0,0,1);
                half4 a = half4(1, 2, 0, 1),
                      b = half4(2, 2, 1, 0);
                int3  c = int3(1111, 3333, 5555),
                      d = int3(1111, 5555, 3333);
                uint2 e = uint2(1111111111u, 222),
                      f = uint2(3333333333u, 222);
                return (lessThan(a, b)         == bool4(true, false, true, false)  &&
                        lessThan(c, d)         == bool3(false, true, false)        &&
                        lessThan(e, f)         == bool2(true, false)               &&
                        greaterThan(a, b)      == bool4(false, false, false, true) &&
                        greaterThan(c, d)      == bool3(false, false, true)        &&
                        greaterThan(e, f)      == bool2(false, false)              &&
                        lessThanEqual(a, b)    == bool4(true, true, true, false)   &&
                        lessThanEqual(c, d)    == bool3(true, true, false)         &&
                        lessThanEqual(e, f)    == bool2(true, true)                &&
                        greaterThanEqual(a, b) == bool4(false, true, false, true)  &&
                        greaterThanEqual(c, d) == bool3(true, false, true)         &&
                        greaterThanEqual(e, f) == bool2(false, true)               &&
                        equal(a, b)            == bool4(false, true, false, false) &&
                        equal(c, d)            == bool3(true, false, false)        &&
                        equal(e, f)            == bool2(false, true)               &&
                        notEqual(a, b)         == bool4(true, false, true, true)   &&
                        notEqual(c, d)         == bool3(false, true, true)         &&
                        notEqual(e, f)         == bool2(true, false)               &&
                        max(a, b)              == half4(2, 2, 1, 1)                &&
                        max(c, d)              == int3(1111, 5555, 5555)           &&
                        max(e, f)              == uint2(3333333333u, 222)          &&
                        max(a, 1)              == half4(1, 2, 1, 1)                &&
                        max(c, 3333)           == int3(3333, 3333, 5555)           &&
                        max(e, 7777)           == uint2(1111111111u, 7777)         &&
                        min(a, b)              == half4(1, 2, 0, 0)                &&
                        min(c, d)              == int3(1111, 3333, 3333)           &&
                        min(e, f)              == uint2(1111111111u, 222)          &&
                        min(a, 1)              == half4(1, 1, 0, 1)                &&
                        min(c, 3333)           == int3(1111, 3333, 3333)           &&
                        min(e, 7777)           == uint2(7777, 222)) ? colorGreen : colorRed;
            }
         ",
            &[],
            color(0.0, 0.0, 0.0, 0.0),
            Some(color(0.0, 1.0, 0.0, 1.0)),
        );
    }
);

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L255-L277 (chrome/m156)
def_test!(
    SkSLRasterPipelineCodeGeneratorReturnComplexityKeyMismatchTest,
    |r| {
        test(
            r,
            r"
             noinline half callee() {
                 return 1.0;
             }

             noinline half4 caller(half value) {
                 half c = callee();
                 if (value > 2.0) {
                     return half4(0.0, 1.0, 0.0, 1.0); // green
                 }
                 return half4(1.0, 0.0, 0.0, 1.0); // red
             }

             half4 main(half4 coords) {
                 return caller(3.0);
             }
         ",
            &[],
            color(0.0, 0.0, 0.0, 0.0),
            Some(color(0.0, 1.0, 0.0, 1.0)),
        );
    }
);

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L279-L354 (chrome/m156)
def_test!(SkSLRasterPipelineSlotOverflow_355465305, |r| {
    const K_STRUCT_MEMBERS1: usize = 6200;
    const K_STRUCT_MEMBERS2: usize = 433;
    let mut src = String::new();
    src.push_str("struct M { float4x4 m");
    for i in 1..K_STRUCT_MEMBERS1 {
        src.push_str(&format!(",m{i}"));
    }
    src.push_str(";};");
    src.push_str("struct M2 { float4x4 m");
    for i in 1..K_STRUCT_MEMBERS2 {
        src.push_str(&format!(",m{i}"));
    }
    src.push_str(";};");
    src.push_str("M f() { M m; return m; }");
    const K_CONST_MEMBERS: usize = 40;
    src.push_str("struct T { float4x4 m0");
    for i in 1..K_CONST_MEMBERS {
        src.push_str(&format!(",m{i}"));
    }
    src.push_str(";};");
    src.push_str("const T K = T(");
    for i in 0..K_CONST_MEMBERS {
        if i > 0 {
            src.push(',');
        }
        src.push_str("mat4x4(1337)");
    }
    src.push_str(");");
    src.push_str("half4 main(half4 color) {");
    src.push_str("float4x4 a = M2(");
    for j in 0..K_STRUCT_MEMBERS2 {
        if j > 0 {
            src.push(',');
        }
        let num_add_ops = if j == K_STRUCT_MEMBERS1 - 1 { 23 } else { 25 };
        for i in 0..num_add_ops {
            if i > 0 {
                src.push('+');
            }
            src.push_str("f().m");
        }
    }
    src.push_str(").m;");
    src.push_str("return half4(a[0]+(K.m0+K.m1+K.m2+K.m3)[0]);");
    src.push('}');

    let mut compiler = Compiler::new();
    let Some(mut program) = compiler.convert_program(
        ProgramKind::RuntimeColorFilter,
        src.as_bytes(),
        ProgramSettings::default(),
    ) else {
        errorf!(
            r,
            "Unexpected error compiling {}\n{}",
            src,
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        return;
    };
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    let Some(main) = main else {
        errorf!(r, "Program must have a 'main' function");
        return;
    };
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    pipeline.append_constant_color4f(&alloc, &color(1.0, 1.0, 1.0, 1.0));
    let raster_prog = make_raster_pipeline_program(&mut program, main, None, false);
    // Ideally, this program would fail in the front-end, because of the number of slots needed
    // for expression evaluation. For now, it succeeds (but then fails in appendStages).
    let Some(raster_prog) = raster_prog else {
        errorf!(r, "MakeRasterPipelineProgram failed");
        return;
    };

    // Append the SkSL program to the raster pipeline.
    let success = raster_prog.append_stages(&mut pipeline, &alloc, None, &[]);
    reporter_assert!(
        r,
        !success,
        "appendStages should fail for very large program"
    );
});

// Port of: tests/RasterPipelineCodeGeneratorTest.cpp#L356-L382 (chrome/m156)
def_test!(SkSLRasterPipeline_ConvertProgram_b540157141, |r| {
    let src = r#"
        uniform shader c0;
        uniform shader c1;
        uniform shader c2;
        // The noinline is important for reproduction. It is also important that
        // one of the args be an "EffectChild" to cause fChildEffectMap to grow.
        noinline half4 f(shader s, float2 p) {
            return s.eval(p);
        }
        half4 main(float2 p) {
            return c0.eval(float2(f(c1, p).xy) + p);
        }
    "#;

    let mut compiler = Compiler::new();
    let settings = ProgramSettings::default();
    let program =
        compiler.convert_program(ProgramKind::PrivateRuntimeShader, src.as_bytes(), settings);
    let Some(mut program) = program else {
        panic!(
            "Unexpected error compiling {}",
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
    };
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    let Some(main) = main else {
        panic!("main is missing!?");
    };
    // With the buggy code, this triggered a UAF
    let raster_prog = make_raster_pipeline_program(&mut program, main, None, false);
    reporter_assert!(r, raster_prog.is_some());
});
