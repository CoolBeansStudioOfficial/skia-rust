// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLTest.cpp (chrome/m156)

//! `SkSLTest`: every `resources/sksl` test shader, compiled as a runtime shader and run.
//!
//! Each `SKSL_TEST` is one `def_test!` that runs what the C++ macro defines, in this order:
//! `_CPU` (if the entry is flagged `CPU`), `_RP` and `_Clone`. Only the entries without the `CPU`
//! flag are here so far: their `_CPU` test does not exist, and `_RP` and `_Clone` need only the
//! front end and the Raster Pipeline generator. The `CPU`-flagged entries need
//! `SkRuntimeEffect::MakeForShader` and a raster surface (task S18) and are added with it. The
//! Ganesh and Graphite variants are GPU tests, which this project does not run.
//!
//! Mapping notes:
//! - `SkRuntimeEffectPriv::VarAsUniform` only decides the uniform's name and size here. The size
//!   in floats of a uniform is its type's slot count, so `_RP` computes it directly.
//! - `RPCallbacks` appends constant colors through the pipeline it is handed (`Callbacks` passes
//!   the sink to each callback).

// The ported tests keep the C++ declaration order and function lengths.
#![allow(
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::format_push_string
)]

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::effect_priv::SHADER_SCRATCH;
use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_simd::rp::contexts::MAX_STRIDE_HIGHP;
use skia_rust_sksl::analysis::{ProgramVisitor, walk_expression};
use skia_rust_sksl::codegen::rp::{Callbacks, make_raster_pipeline_program};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::ir::{ExprId, IrPool, ProgramElementKind, StatementKind};
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings, Version};
use skia_rust_sksl::tracing::DebugTracePriv;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert};

/// `SkSLTestFlag`, for the flags the CPU tests look at (`GPU` and `UsesNonFinite` only matter to
/// the Ganesh and Graphite variants, so they are not represented).
// Port of: tests/SkSLTest.cpp#L66-L99 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
struct SkSLTestFlags(u32);

impl SkSLTestFlags {
    /// `SkSLTestFlag::CPU`.
    const CPU: Self = Self(1 << 0);
    /// `SkSLTestFlag::ES3`: must pass when executed directly on the CPU via Raster Pipeline.
    const ES3: Self = Self(1 << 1);
    /// `SkSLTestFlag::GPU_ES3`.
    const GPU_ES3: Self = Self(1 << 3);
    /// `SkSLTestFlag::Priv`: rely on `AllowPrivateAccess` support in the runtime effect.
    const PRIV: Self = Self(1 << 5);

    /// `flags | other`.
    const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `flags & flag`.
    const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

// Port of: tests/SkSLTest.cpp#L110-L112 (chrome/m156)
const fn is_strict_es2(flags: SkSLTestFlags) -> bool {
    !flags.has(SkSLTestFlags::GPU_ES3) && !flags.has(SkSLTestFlags::ES3)
}

/// `UniformData`.
struct UniformData {
    name: &'static str,
    span: &'static [f32],
}

// Port of: tests/SkSLTest.cpp#L129-L166 (chrome/m156)
const K_UNIFORM_COLOR_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const K_UNIFORM_COLOR_RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const K_UNIFORM_COLOR_GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const K_UNIFORM_COLOR_BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const K_UNIFORM_COLOR_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const K_UNIFORM_TEST_INPUTS: [f32; 4] = [-1.25, 0.0, 0.75, 2.25];
const K_UNIFORM_UNKNOWN_INPUT: [f32; 1] = [1.0];
const K_UNIFORM_TEST_MATRIX_2X2: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
const K_UNIFORM_TEST_MATRIX_3X3: [f32; 9] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
const K_UNIFORM_TEST_MATRIX_4X4: [f32; 16] = [
    1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
];
const K_UNIFORM_TEST_ARRAY: [f32; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const K_UNIFORM_TEST_ARRAY_NEGATIVE: [f32; 5] = [-1.0, -2.0, -3.0, -4.0, -5.0];

// float3x3[3]
const K_UNIFORM_TEST_MATRIX_ARRAY: [f32; 27] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

// Port of: tests/SkSLTest.cpp#L167-L181 (chrome/m156)
const K_UNIFORM_DATA: [UniformData; 13] = [
    UniformData {
        name: "colorBlack",
        span: &K_UNIFORM_COLOR_BLACK,
    },
    UniformData {
        name: "colorRed",
        span: &K_UNIFORM_COLOR_RED,
    },
    UniformData {
        name: "colorGreen",
        span: &K_UNIFORM_COLOR_GREEN,
    },
    UniformData {
        name: "colorBlue",
        span: &K_UNIFORM_COLOR_BLUE,
    },
    UniformData {
        name: "colorWhite",
        span: &K_UNIFORM_COLOR_WHITE,
    },
    UniformData {
        name: "testInputs",
        span: &K_UNIFORM_TEST_INPUTS,
    },
    UniformData {
        name: "unknownInput",
        span: &K_UNIFORM_UNKNOWN_INPUT,
    },
    UniformData {
        name: "testMatrix2x2",
        span: &K_UNIFORM_TEST_MATRIX_2X2,
    },
    UniformData {
        name: "testMatrix3x3",
        span: &K_UNIFORM_TEST_MATRIX_3X3,
    },
    UniformData {
        name: "testMatrix4x4",
        span: &K_UNIFORM_TEST_MATRIX_4X4,
    },
    UniformData {
        name: "testArray",
        span: &K_UNIFORM_TEST_ARRAY,
    },
    UniformData {
        name: "testArrayNegative",
        span: &K_UNIFORM_TEST_ARRAY_NEGATIVE,
    },
    UniformData {
        name: "testMatrixArray",
        span: &K_UNIFORM_TEST_MATRIX_ARRAY,
    },
];

/// `load_source`: the shader's source, or `None` (after reporting) if it cannot be loaded.
// Port of: tests/SkSLTest.cpp#L233-L242 (chrome/m156)
fn load_source(r: &mut Reporter, test_file: &str, permutation_suffix: &str) -> Option<Vec<u8>> {
    let resource_path = format!("sksl/{test_file}");
    let Some(shader_data) = get_resource_as_data(&resource_path) else {
        errorf!(
            r,
            "{}{}: Unable to load file",
            test_file,
            permutation_suffix
        );
        return None;
    };
    Some(shader_data)
}

// Port of: tests/SkSLTest.cpp#L809-L845 (chrome/m156)
fn test_clone(r: &mut Reporter, test_file: &str, flags: SkSLTestFlags) {
    let Some(shader_string) = load_source(r, test_file, "") else {
        return;
    };
    if shader_string.is_empty() {
        return;
    }
    let settings = ProgramSettings {
        // TODO(skbug.com/40042585): Can we just put the correct #version in the source files that
        // need this?
        max_version_allowed: if is_strict_es2(flags) {
            Version::K100
        } else {
            Version::K300
        },
        ..ProgramSettings::default()
    };
    let kind = if flags.has(SkSLTestFlags::PRIV) {
        ProgramKind::PrivateRuntimeShader
    } else {
        ProgramKind::RuntimeShader
    };
    let mut compiler = Compiler::new();
    let Some(mut program) = compiler.convert_program(kind, &shader_string, settings) else {
        errorf!(
            r,
            "{}",
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        return;
    };

    // Clone every expression in the program, and ensure that its clone generates the same
    // description as the original.
    struct CloneVisitor {
        expressions: Vec<ExprId>,
    }

    impl ProgramVisitor for CloneVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            self.expressions.push(expr);
            walk_expression(self, pool, expr)
        }
    }

    // (Cloning allocates in the program's pool, so the visit collects the expressions first.)
    let mut visitor = CloneVisitor {
        expressions: Vec::new(),
    };
    visitor.visit(&program);
    for expr in visitor.expressions {
        let original = program.pool.expression_description(expr);
        let cloned_expr = program.pool.clone_expression(expr);
        let cloned = program.pool.expression_description(cloned_expr);
        reporter_assert!(
            r,
            original == cloned,
            "Mismatch after clone!\nOriginal: {}\nCloned: {}\n",
            original,
            cloned
        );
    }
}

// Port of: tests/SkSLTest.cpp#L847-L851 (chrome/m156)
fn report_rp_pass(r: &mut Reporter, test_file: &str, flags: SkSLTestFlags) {
    if !flags.has(SkSLTestFlags::CPU) && !flags.has(SkSLTestFlags::ES3) {
        errorf!(r, "NEW: {}", test_file);
    }
}

// Port of: tests/SkSLTest.cpp#L853-L861 (chrome/m156)
fn report_rp_fail(r: &mut Reporter, test_file: &str, flags: SkSLTestFlags, reason: &str) {
    if flags.has(SkSLTestFlags::CPU) || flags.has(SkSLTestFlags::ES3) {
        errorf!(r, "{}: {}", test_file, reason);
    }
}

/// `RPCallbacks`: implements the `shaderGreen` and `shaderRed` shaders.
// Port of: tests/SkSLTest.cpp#L936-L972 (chrome/m156)
struct RpCallbacks<'a> {
    alloc: &'a ArenaAlloc,
    child_effects: &'a [String],
}

impl<'a> Callbacks<'a, RasterPipeline<'a>> for RpCallbacks<'a> {
    fn append_shader(&mut self, p: &mut RasterPipeline<'a>, index: i32) -> bool {
        let name = &self.child_effects[usize::try_from(index).expect("child index")];
        if name == "shaderGreen" {
            const K_COLOR_GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
            p.append_constant_color(self.alloc, &K_COLOR_GREEN);
            return true;
        }
        if name == "shaderRed" {
            const K_COLOR_RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
            p.append_constant_color(self.alloc, &K_COLOR_RED);
            return true;
        }
        panic!("unrecognized RP effect");
    }

    fn append_color_filter(&mut self, _: &mut RasterPipeline<'a>, _: i32) -> bool {
        panic!("unsupported RP callback");
    }

    fn append_blender(&mut self, _: &mut RasterPipeline<'a>, _: i32) -> bool {
        panic!("unsupported RP callback");
    }

    fn to_linear_srgb(&mut self, _: &mut RasterPipeline<'a>, _: MemPtr) {
        panic!("unsupported RP callback");
    }

    fn from_linear_srgb(&mut self, _: &mut RasterPipeline<'a>, _: MemPtr) {
        panic!("unsupported RP callback");
    }
}

// Port of: tests/SkSLTest.cpp#L863-L990 (chrome/m156)
fn test_raster_pipeline(r: &mut Reporter, test_file: &str, flags: SkSLTestFlags) {
    let Some(shader_string) = load_source(r, test_file, "") else {
        return;
    };
    if shader_string.is_empty() {
        return;
    }

    // In Raster Pipeline, we can compile and run test shaders directly, without involving a
    // surface at all.
    let mut compiler = Compiler::new();
    let settings = ProgramSettings {
        max_version_allowed: Version::K300,
        ..ProgramSettings::default()
    };
    let kind = if flags.has(SkSLTestFlags::PRIV) {
        ProgramKind::PrivateRuntimeShader
    } else {
        ProgramKind::RuntimeShader
    };
    let Some(mut program) = compiler.convert_program(kind, &shader_string, settings) else {
        errorf!(
            r,
            "{}: Unexpected compilation error\n{}",
            test_file,
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        return;
    };
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    let Some(main) = main else {
        errorf!(r, "{}: Program must have a 'main' function", test_file);
        return;
    };

    // Match up uniforms from the program against our list of test uniforms, and build up a data
    // buffer of uniform floats.
    let mut uniforms: Vec<(String, usize)> = Vec::new();
    let mut child_effects: Vec<String> = Vec::new();

    for elem in program.elements() {
        // Variables (uniform, etc.)
        let ProgramElementKind::GlobalVar(global) = &program.pool.element(elem).kind else {
            continue;
        };
        let StatementKind::VarDeclaration(var_decl) =
            &program.pool.statement(global.declaration).kind
        else {
            panic!("a global variable declaration holds a VarDeclaration");
        };
        let var = program.pool.variable(var_decl.var);
        let var_type = program.pool.ty(var.ty);

        // Keep track of child effects.
        if var_type.is_effect_child() {
            child_effects.push(var.name.to_string());
            continue;
        }
        // 'uniform' variables: `SkRuntimeEffectPriv::VarAsUniform` gives the name and the size
        // (`sizeInBytes`, which is four bytes per slot).
        if var.modifier_flags.is_uniform() {
            uniforms.push((var.name.to_string(), var_type.slot_count()));
        }
    }

    let mut uniform_values: Vec<f32> = Vec::new();
    for (name, slot_count) in &uniforms {
        let mut found_match = false;
        for data in &K_UNIFORM_DATA {
            if data.name == name {
                debug_assert_eq!(data.span.len(), *slot_count);
                found_match = true;
                uniform_values.extend_from_slice(data.span);
                break;
            }
        }
        if !found_match {
            report_rp_fail(r, test_file, flags, "unsupported uniform");
            return;
        }
    }

    // Compile our program.
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    let debug_trace = DebugTracePriv::default();
    let Some(raster_prog) =
        make_raster_pipeline_program(&mut program, main, Some(debug_trace), false)
    else {
        report_rp_fail(r, test_file, flags, "code is not supported");
        return;
    };

    // Create callbacks which implement `shaderGreen` and `shaderRed` shaders. Fortunately, these
    // are trivial to implement directly in Raster Pipeline.
    let mut callbacks = RpCallbacks {
        alloc: &alloc,
        child_effects: &child_effects,
    };

    // Append the SkSL program to the raster pipeline.
    pipeline.append_constant_color(&alloc, &[0.0, 0.0, 0.0, 0.0]);
    raster_prog.append_stages(&mut pipeline, &alloc, Some(&mut callbacks), &uniform_values);

    // Move the float values from RGBA into an 8888 memory buffer.
    let out_ctx = MemoryCtx::new(MemSlot(0));
    pipeline.append(Stage::Store8888(out_ctx));
    let mut out_bytes = [0_u8; 4 * MAX_STRIDE_HIGHP];
    let mut scratch = alloc.scratch_buffer();
    {
        let mut mem = MemoryBindings::new()
            .with(MemSlot(0), MemView::write(&mut out_bytes))
            .with(SHADER_SCRATCH, MemView::write(&mut scratch));
        pipeline.run(0, 0, 1, 1, &mut mem);
    }
    let out0 = u32::from_ne_bytes(out_bytes[..4].try_into().expect("four bytes"));

    // Make sure the first pixel (exclusively) of `out` is green. If the program compiled
    // successfully, we expect it to run without error, and will assert if it doesn't.
    let expected = 0xFF00_FF00_u32;
    if out0 != expected {
        errorf!(
            r,
            "{}: Raster Pipeline failed. Expected solid green, got ARGB:{:02X}{:02X}{:02X}{:02X}",
            test_file,
            (out0 >> 24) & 0xFF,
            (out0 >> 16) & 0xFF,
            (out0 >> 8) & 0xFF,
            out0 & 0xFF
        );
        return;
    }

    // Success!
    report_rp_pass(r, test_file, flags);
}

/// `SKSL_TEST` for an entry without the `CPU` flag: `_RP`, then `_Clone`. (`_CPU` does not exist
/// for it, and the Ganesh and Graphite variants are GPU tests.)
// Port of: tests/SkSLTest.cpp#L1026-L1031 (chrome/m156)
macro_rules! sksl_test {
    ($flags:expr, $name:ident, $path:literal) => {
        def_test!($name, |r| {
            test_raster_pipeline(r, $path, $flags);
            test_clone(r, $path, $flags);
        });
    };
}

// Port of: tests/SkSLTest.cpp#L1063-L1063 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ArrayFolding,
    "folding/ArrayFolding.sksl"
);

// Port of: tests/SkSLTest.cpp#L1069-L1069 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntFoldingES3,
    "folding/IntFoldingES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1073-L1073 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixFoldingES3,
    "folding/MatrixFoldingES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1090-L1090 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DoWhileBodyMustBeInlinedIntoAScope,
    "inliner/DoWhileBodyMustBeInlinedIntoAScope.sksl"
);

// Port of: tests/SkSLTest.cpp#L1091-L1091 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DoWhileTestCannotBeInlined,
    "inliner/DoWhileTestCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1093-L1093 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ForInitializerExpressionsCanBeInlined,
    "inliner/ForInitializerExpressionsCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1107-L1107 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    InlineUnscopedVariable,
    "inliner/InlineUnscopedVariable.sksl"
);

// Port of: tests/SkSLTest.cpp#L1117-L1117 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    StaticSwitchInline,
    "inliner/StaticSwitch.sksl"
);

// Port of: tests/SkSLTest.cpp#L1123-L1123 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    TrivialArgumentsInlineDirectlyES3,
    "inliner/TrivialArgumentsInlineDirectlyES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1125-L1125 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    WhileBodyMustBeInlinedIntoAScope,
    "inliner/WhileBodyMustBeInlinedIntoAScope.sksl"
);

// Port of: tests/SkSLTest.cpp#L1126-L1126 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    WhileTestCannotBeInlined,
    "inliner/WhileTestCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1129-L1129 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicAbsInt,
    "intrinsics/AbsInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1133-L1133 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicClampInt,
    "intrinsics/ClampInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1134-L1134 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicClampUInt,
    "intrinsics/ClampUInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1138-L1138 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicDeterminant,
    "intrinsics/Determinant.sksl"
);

// Port of: tests/SkSLTest.cpp#L1139-L1139 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicDFdx,
    "intrinsics/DFdx.sksl"
);

// Port of: tests/SkSLTest.cpp#L1140-L1140 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicDFdy,
    "intrinsics/DFdy.sksl"
);

// Port of: tests/SkSLTest.cpp#L1143-L1143 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicFloatBitsToInt,
    "intrinsics/FloatBitsToInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1144-L1144 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicFloatBitsToUint,
    "intrinsics/FloatBitsToUint.sksl"
);

// Port of: tests/SkSLTest.cpp#L1146-L1146 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicFwidth,
    "intrinsics/Fwidth.sksl"
);

// Port of: tests/SkSLTest.cpp#L1147-L1147 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicIntBitsToFloat,
    "intrinsics/IntBitsToFloat.sksl"
);

// Port of: tests/SkSLTest.cpp#L1148-L1148 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicIsInf,
    "intrinsics/IsInf.sksl"
);

// Port of: tests/SkSLTest.cpp#L1151-L1151 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMatrixCompMultES3,
    "intrinsics/MatrixCompMultES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1153-L1153 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMaxInt,
    "intrinsics/MaxInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1154-L1154 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMaxUint,
    "intrinsics/MaxUint.sksl"
);

// Port of: tests/SkSLTest.cpp#L1156-L1156 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMinInt,
    "intrinsics/MinInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1157-L1157 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMinUint,
    "intrinsics/MinUint.sksl"
);

// Port of: tests/SkSLTest.cpp#L1159-L1159 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMixFloatES3,
    "intrinsics/MixFloatES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1160-L1160 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicModf,
    "intrinsics/Modf.sksl"
);

// Port of: tests/SkSLTest.cpp#L1162-L1162 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicOuterProduct,
    "intrinsics/OuterProduct.sksl"
);

// Port of: tests/SkSLTest.cpp#L1164-L1164 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicRound,
    "intrinsics/Round.sksl"
);

// Port of: tests/SkSLTest.cpp#L1165-L1165 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicRoundEven,
    "intrinsics/RoundEven.sksl"
);

// Port of: tests/SkSLTest.cpp#L1168-L1168 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicSignInt,
    "intrinsics/SignInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1171-L1171 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicTrunc,
    "intrinsics/Trunc.sksl"
);

// Port of: tests/SkSLTest.cpp#L1172-L1172 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicTranspose,
    "intrinsics/Transpose.sksl"
);

// Port of: tests/SkSLTest.cpp#L1173-L1173 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicUintBitsToFloat,
    "intrinsics/UintBitsToFloat.sksl"
);

// Port of: tests/SkSLTest.cpp#L1175-L1175 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ArrayNarrowingConversions,
    "runtime/ArrayNarrowingConversions.rts"
);

// Port of: tests/SkSLTest.cpp#L1178-L1178 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    Commutative,
    "runtime/Commutative.rts"
);

// Port of: tests/SkSLTest.cpp#L1192-L1192 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    RecursiveComparison_Arrays,
    "runtime/RecursiveComparison_Arrays.rts"
);

// Port of: tests/SkSLTest.cpp#L1193-L1193 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    RecursiveComparison_Structs,
    "runtime/RecursiveComparison_Structs.rts"
);

// Port of: tests/SkSLTest.cpp#L1194-L1194 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    RecursiveComparison_Types,
    "runtime/RecursiveComparison_Types.rts"
);

// Port of: tests/SkSLTest.cpp#L1195-L1195 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    RecursiveComparison_Vectors,
    "runtime/RecursiveComparison_Vectors.rts"
);

// Port of: tests/SkSLTest.cpp#L1197-L1197 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ArrayCast,
    "shared/ArrayCast.sksl"
);

// Port of: tests/SkSLTest.cpp#L1198-L1198 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ArrayComparison,
    "shared/ArrayComparison.sksl"
);

// Port of: tests/SkSLTest.cpp#L1199-L1199 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ArrayConstructors,
    "shared/ArrayConstructors.sksl"
);

// Port of: tests/SkSLTest.cpp#L1207-L1207 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ConstantCompositeAccessViaConstantIndex,
    "shared/ConstantCompositeAccessViaConstantIndex.sksl"
);

// Port of: tests/SkSLTest.cpp#L1208-L1208 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ConstantCompositeAccessViaDynamicIndex,
    "shared/ConstantCompositeAccessViaDynamicIndex.sksl"
);

// Port of: tests/SkSLTest.cpp#L1210-L1210 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ConstArray,
    "shared/ConstArray.sksl"
);

// Port of: tests/SkSLTest.cpp#L1213-L1213 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DeadLoopVariable,
    "shared/DeadLoopVariable.sksl"
);

// Port of: tests/SkSLTest.cpp#L1216-L1216 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DeadReturnES3,
    "shared/DeadReturnES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1220-L1220 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DoWhileControlFlow,
    "shared/DoWhileControlFlow.sksl"
);

// Port of: tests/SkSLTest.cpp#L1222-L1222 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    EmptyBlocksES3,
    "shared/EmptyBlocksES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1224-L1224 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ForLoopMultipleInitES3,
    "shared/ForLoopMultipleInitES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1234-L1234 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    HexUnsigned,
    "shared/HexUnsigned.sksl"
);

// Port of: tests/SkSLTest.cpp#L1238-L1238 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntegerDivisionES3,
    "shared/IntegerDivisionES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1242-L1242 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatricesNonsquare,
    "shared/MatricesNonsquare.sksl"
);

// Port of: tests/SkSLTest.cpp#L1244-L1244 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixConstructorsES3,
    "shared/MatrixConstructorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1249-L1249 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixOpEqualsES3,
    "shared/MatrixOpEqualsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1257-L1257 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    OperatorsES3,
    "shared/OperatorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1262-L1262 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    OutParamsFunctionCallInArgument,
    "shared/OutParamsFunctionCallInArgument.sksl"
);

// Port of: tests/SkSLTest.cpp#L1266-L1266 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    PrefixExpressionsES3,
    "shared/PrefixExpressionsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1269-L1269 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ResizeMatrixNonsquare,
    "shared/ResizeMatrixNonsquare.sksl"
);

// Port of: tests/SkSLTest.cpp#L1271-L1271 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ReturnsValueOnEveryPathES3,
    "shared/ReturnsValueOnEveryPathES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1273-L1273 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ScalarConversionConstructorsES3,
    "shared/ScalarConversionConstructorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1281-L1281 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3,
    StructComparison,
    "shared/StructComparison.sksl"
);

// Port of: tests/SkSLTest.cpp#L1289-L1289 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwitchWithLoopsES3,
    "shared/SwitchWithLoopsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1291-L1291 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwizzleAsLValueES3,
    "shared/SwizzleAsLValueES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1294-L1294 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwizzleByIndex,
    "shared/SwizzleByIndex.sksl"
);

// Port of: tests/SkSLTest.cpp#L1319-L1319 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    WhileLoopControlFlow,
    "shared/WhileLoopControlFlow.sksl"
);
