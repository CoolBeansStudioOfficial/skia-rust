// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLTest.cpp (chrome/m156)

//! `SkSLTest`: every `resources/sksl` test shader, compiled as a runtime shader and run.
//!
//! Each `SKSL_TEST` is one `def_test!` that runs what the C++ macro defines, in this order:
//! `_CPU` (if the entry is flagged `CPU`: the shader is compiled with `MakeForShader`, drawn into
//! a 2x2 raster surface, with and without optimization, and every pixel must be green), `_RP` and
//! `_Clone`. The Ganesh and Graphite variants are GPU tests, which this project does not run.
//!
//! Mapping notes:
//! - `SkRuntimeEffectPriv::VarAsUniform` only decides the uniform's name and size here. The size
//!   in floats of a uniform is its type's slot count, so `_RP` computes it directly.
//! - `RPCallbacks` appends constant colors through the pipeline it is handed (`Callbacks` passes
//!   the sink to each callback).

// The ported tests keep the C++ declaration order and function lengths.
// (`kWidth`/`kHeight` are `int`s used as sizes, indices and scalars: the casts mirror the C++.)
#![allow(
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::format_push_string,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::effect_priv::SHADER_SCRATCH;
use skia_rust_core::paint::Paint;
use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{Options, RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::shaders;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;
use skia_rust_simd::rp::contexts::MAX_STRIDE_HIGHP;
use skia_rust_sksl::analysis::{ProgramVisitor, walk_expression};
use skia_rust_sksl::codegen::rp::{Callbacks, make_raster_pipeline_program};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::ir::{ExprId, IrPool, ProgramElementKind, StatementKind};
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings, Version};
use skia_rust_sksl::tracing::DebugTracePriv;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/SkSLTest.cpp#L47-L48 (chrome/m156)
const K_WIDTH: usize = 2;
const K_HEIGHT: usize = 2;

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
    if crate::resources::resource_dir().is_none() {
        // CI test jobs run without `third_party/skia`: skip rather than fail (the manifest job,
        // which has the Skia checkout, runs these for real).
        eprintln!("todo: skipping {test_file}, Skia resource directory not found");
        return None;
    }
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

// Port of: tests/SkSLTest.cpp#L195-L224 (chrome/m156)
fn bitmap_from_shader(
    r: &mut Reporter,
    surface: &mut Surface<'_>,
    effect: &RuntimeEffect,
) -> Bitmap {
    let mut builder = RuntimeShaderBuilder::new(effect.clone());
    for data in &K_UNIFORM_DATA {
        let mut uniform = builder.uniform(data.name);
        if uniform.var().is_some() {
            uniform.set_f32(data.span);
        }
    }

    {
        let mut green = builder.child("shaderGreen");
        if green.child().is_some() {
            green.assign(shaders::color(Color::GREEN));
        }
    }

    {
        let mut red = builder.child("shaderRed");
        if red.child().is_some() {
            red.assign(shaders::color(Color::RED));
        }
    }

    let Some(shader) = builder.make_shader(None) else {
        return Bitmap::new();
    };

    surface.canvas().clear(Color::BLACK);

    let mut paint_shader = Paint::default();
    paint_shader.set_shader(shader);
    surface.canvas().draw_rect(
        Rect::from_wh(K_WIDTH as f32, K_HEIGHT as f32),
        &paint_shader,
    );

    let mut bitmap = Bitmap::new();
    reporter_assert!(r, bitmap.try_alloc_pixels_info(&surface.image_info(), None));
    reporter_assert!(r, surface.read_pixels_to_bitmap(&mut bitmap, (0, 0)));
    bitmap
}

// Port of: tests/SkSLTest.cpp#L349-L419 (chrome/m156)
// (`failure_is_expected` only disables GPU permutations, so it is not ported.)
fn test_one_permutation(
    r: &mut Reporter,
    device_name: &str,
    backend_api: &str,
    surface: &mut Surface<'_>,
    test_file: &str,
    permutation_suffix: &str,
    options: &Options<'_>,
) {
    let Some(shader_string) = load_source(r, test_file, permutation_suffix) else {
        return;
    };
    if shader_string.is_empty() {
        return;
    }
    let shader_string = String::from_utf8_lossy(&shader_string).into_owned();
    let effect = match RuntimeEffect::make_for_shader(&shader_string, Some(options)) {
        Ok(effect) => effect,
        Err(error_text) => {
            errorf!(r, "{}{}: {}", test_file, permutation_suffix, error_text);
            return;
        }
    };

    let bitmap = bitmap_from_shader(r, surface, &effect);
    if bitmap.is_empty() {
        errorf!(
            r,
            "{}{}: Unable to build shader",
            test_file,
            permutation_suffix
        );
        return;
    }

    let mut success = true;
    let mut color = [[Color::TRANSPARENT; K_WIDTH]; K_HEIGHT];
    for (y, row) in color.iter_mut().enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = bitmap.get_color((x as i32, y as i32));
            if *pixel != Color::GREEN {
                success = false;
            }
        }
    }

    if !success {
        const _: () = assert!(K_WIDTH == 2);
        const _: () = assert!(K_HEIGHT == 2);

        let channels = |c: Color| format!("{:02X}{:02X}{:02X}{:02X}", c.r(), c.g(), c.b(), c.a());
        let message = format!(
            "Expected{permutation_suffix}: solid green. Actual output from {device_name} using \
             {backend_api}:\nRRGGBBAA RRGGBBAA\n{} {}\n{} {}",
            channels(color[0][0]),
            channels(color[0][1]),
            channels(color[1][0]),
            channels(color[1][1]),
        );

        errorf!(r, "{}", message);
    }
}

// Port of: tests/SkSLTest.cpp#L421-L441 (chrome/m156)
fn test_permutations(
    r: &mut Reporter,
    device_name: &str,
    backend_api: &str,
    surface: &mut Surface<'_>,
    test_file: &str,
    strict_es2: bool,
    private_access: bool,
) {
    let mut options = if strict_es2 {
        Options::default()
    } else {
        runtime_effect_priv::es3_options()
    };
    if private_access {
        runtime_effect_priv::allow_private_access(&mut options);
    }
    options.force_unoptimized = false;
    test_one_permutation(
        r,
        device_name,
        backend_api,
        surface,
        test_file,
        "",
        &options,
    );

    options.force_unoptimized = true;
    test_one_permutation(
        r,
        device_name,
        backend_api,
        surface,
        test_file,
        " (Unoptimized)",
        &options,
    );
}

// Port of: tests/SkSLTest.cpp#L443-L457 (chrome/m156)
fn test_cpu(r: &mut Reporter, test_file: &str, flags: SkSLTestFlags) {
    assert!(flags.has(SkSLTestFlags::CPU));

    // Create a raster-backed surface.
    let mut surface =
        surfaces::raster_n32_premul((K_WIDTH as i32, K_HEIGHT as i32)).expect("a raster surface");
    let private_access = flags.has(SkSLTestFlags::PRIV);

    test_permutations(
        r,
        "CPU",
        "SkRP",
        &mut surface,
        test_file,
        /* strict_es2 */ true,
        private_access,
    );
}

/// `SKSL_TEST`: `_CPU` (if the entry is flagged `CPU`), `_RP` and `_Clone`. (The Ganesh and
/// Graphite variants are GPU tests.)
// Port of: tests/SkSLTest.cpp#L1026-L1031 (chrome/m156)
macro_rules! sksl_test {
    ($flags:expr, $name:ident, $path:literal) => {
        def_test!($name, |r| {
            if $flags.has(SkSLTestFlags::CPU) {
                test_cpu(r, $path, $flags);
            }
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

// Port of: tests/SkSLTest.cpp#L1064-L1064 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ArraySizeFolding,
    "folding/ArraySizeFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1065-L1065 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    AssignmentOps,
    "folding/AssignmentOps.rts"
);

// Port of: tests/SkSLTest.cpp#L1066-L1066 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, BoolFolding, "folding/BoolFolding.rts");

// Port of: tests/SkSLTest.cpp#L1067-L1067 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, CastFolding, "folding/CastFolding.rts");

// Port of: tests/SkSLTest.cpp#L1068-L1068 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntFoldingES2,
    "folding/IntFoldingES2.rts"
);

// Port of: tests/SkSLTest.cpp#L1069-L1069 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntFoldingES3,
    "folding/IntFoldingES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1070-L1070 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, FloatFolding, "folding/FloatFolding.rts");

// Port of: tests/SkSLTest.cpp#L1071-L1071 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, LogicalNot, "folding/LogicalNot.rts");

// Port of: tests/SkSLTest.cpp#L1072-L1072 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixFoldingES2,
    "folding/MatrixFoldingES2.rts"
);

// Port of: tests/SkSLTest.cpp#L1073-L1073 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixFoldingES3,
    "folding/MatrixFoldingES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1074-L1074 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixNoOpFolding,
    "folding/MatrixNoOpFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1075-L1075 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixScalarNoOpFolding,
    "folding/MatrixScalarNoOpFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1076-L1076 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixVectorNoOpFolding,
    "folding/MatrixVectorNoOpFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1077-L1077 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Negation, "folding/Negation.rts");

// Port of: tests/SkSLTest.cpp#L1078-L1078 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    PreserveSideEffects,
    "folding/PreserveSideEffects.rts"
);

// Port of: tests/SkSLTest.cpp#L1079-L1079 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SelfAssignment,
    "folding/SelfAssignment.rts"
);

// Port of: tests/SkSLTest.cpp#L1080-L1080 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ShortCircuitBoolFolding,
    "folding/ShortCircuitBoolFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1081-L1081 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructFieldFolding,
    "folding/StructFieldFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1082-L1082 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructFieldNoFolding,
    "folding/StructFieldNoFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1083-L1083 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchCaseFolding,
    "folding/SwitchCaseFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1084-L1084 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleFolding,
    "folding/SwizzleFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1085-L1085 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryFolding,
    "folding/TernaryFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1086-L1086 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VectorScalarFolding,
    "folding/VectorScalarFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1087-L1087 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VectorVectorFolding,
    "folding/VectorVectorFolding.rts"
);

// Port of: tests/SkSLTest.cpp#L1089-L1089 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    CommaExpressionsAllowInlining,
    "inliner/CommaExpressionsAllowInlining.sksl"
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

// Port of: tests/SkSLTest.cpp#L1092-L1092 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ForBodyMustBeInlinedIntoAScope,
    "inliner/ForBodyMustBeInlinedIntoAScope.sksl"
);

// Port of: tests/SkSLTest.cpp#L1093-L1093 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ForInitializerExpressionsCanBeInlined,
    "inliner/ForInitializerExpressionsCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1094-L1094 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ForWithoutReturnInsideCanBeInlined,
    "inliner/ForWithoutReturnInsideCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1095-L1095 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ForWithReturnInsideCannotBeInlined,
    "inliner/ForWithReturnInsideCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1096-L1096 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfBodyMustBeInlinedIntoAScope,
    "inliner/IfBodyMustBeInlinedIntoAScope.sksl"
);

// Port of: tests/SkSLTest.cpp#L1097-L1097 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfElseBodyMustBeInlinedIntoAScope,
    "inliner/IfElseBodyMustBeInlinedIntoAScope.sksl"
);

// Port of: tests/SkSLTest.cpp#L1098-L1098 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfElseChainWithReturnsCanBeInlined,
    "inliner/IfElseChainWithReturnsCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1099-L1099 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfTestCanBeInlined,
    "inliner/IfTestCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1100-L1100 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfWithReturnsCanBeInlined,
    "inliner/IfWithReturnsCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1101-L1101 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineKeywordOverridesThreshold,
    "inliner/InlineKeywordOverridesThreshold.sksl"
);

// Port of: tests/SkSLTest.cpp#L1102-L1102 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlinerAvoidsVariableNameOverlap,
    "inliner/InlinerAvoidsVariableNameOverlap.sksl"
);

// Port of: tests/SkSLTest.cpp#L1103-L1103 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlinerElidesTempVarForReturnsInsideBlock,
    "inliner/InlinerElidesTempVarForReturnsInsideBlock.sksl"
);

// Port of: tests/SkSLTest.cpp#L1104-L1104 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlinerUsesTempVarForMultipleReturns,
    "inliner/InlinerUsesTempVarForMultipleReturns.sksl"
);

// Port of: tests/SkSLTest.cpp#L1105-L1105 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlinerUsesTempVarForReturnsInsideBlockWithVar,
    "inliner/InlinerUsesTempVarForReturnsInsideBlockWithVar.sksl"
);

// Port of: tests/SkSLTest.cpp#L1106-L1106 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineThreshold,
    "inliner/InlineThreshold.sksl"
);

// Port of: tests/SkSLTest.cpp#L1107-L1107 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    InlineUnscopedVariable,
    "inliner/InlineUnscopedVariable.sksl"
);

// Port of: tests/SkSLTest.cpp#L1108-L1108 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineWithModifiedArgument,
    "inliner/InlineWithModifiedArgument.sksl"
);

// Port of: tests/SkSLTest.cpp#L1109-L1109 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineWithNestedBigCalls,
    "inliner/InlineWithNestedBigCalls.sksl"
);

// Port of: tests/SkSLTest.cpp#L1110-L1110 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineWithUnmodifiedArgument,
    "inliner/InlineWithUnmodifiedArgument.sksl"
);

// Port of: tests/SkSLTest.cpp#L1111-L1111 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InlineWithUnnecessaryBlocks,
    "inliner/InlineWithUnnecessaryBlocks.sksl"
);

// Port of: tests/SkSLTest.cpp#L1112-L1112 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicNameCollision,
    "inliner/IntrinsicNameCollision.sksl"
);

// Port of: tests/SkSLTest.cpp#L1113-L1113 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ModifiedArrayParametersCannotBeInlined,
    "inliner/ModifiedArrayParametersCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1114-L1114 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ModifiedStructParametersCannotBeInlined,
    "inliner/ModifiedStructParametersCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1115-L1115 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, NoInline, "inliner/NoInline.sksl");

// Port of: tests/SkSLTest.cpp#L1116-L1116 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ShortCircuitEvaluationsCannotInlineRightHandSide,
    "inliner/ShortCircuitEvaluationsCannotInlineRightHandSide.sksl"
);

// Port of: tests/SkSLTest.cpp#L1117-L1117 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    StaticSwitchInline,
    "inliner/StaticSwitch.sksl"
);

// Port of: tests/SkSLTest.cpp#L1118-L1118 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructsCanBeInlinedSafely,
    "inliner/StructsCanBeInlinedSafely.sksl"
);

// Port of: tests/SkSLTest.cpp#L1119-L1119 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleCanBeInlinedDirectly,
    "inliner/SwizzleCanBeInlinedDirectly.sksl"
);

// Port of: tests/SkSLTest.cpp#L1120-L1120 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryResultsCannotBeInlined,
    "inliner/TernaryResultsCannotBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1121-L1121 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryTestCanBeInlined,
    "inliner/TernaryTestCanBeInlined.sksl"
);

// Port of: tests/SkSLTest.cpp#L1122-L1122 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TrivialArgumentsInlineDirectly,
    "inliner/TrivialArgumentsInlineDirectly.sksl"
);

// Port of: tests/SkSLTest.cpp#L1123-L1123 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    TrivialArgumentsInlineDirectlyES3,
    "inliner/TrivialArgumentsInlineDirectlyES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1124-L1124 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TypeShadowing,
    "inliner/TypeShadowing.sksl"
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

// Port of: tests/SkSLTest.cpp#L1128-L1128 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicAbsFloat,
    "intrinsics/AbsFloat.sksl"
);

// Port of: tests/SkSLTest.cpp#L1129-L1129 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicAbsInt,
    "intrinsics/AbsInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1130-L1130 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicAny, "intrinsics/Any.sksl");

// Port of: tests/SkSLTest.cpp#L1131-L1131 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicAll, "intrinsics/All.sksl");

// Port of: tests/SkSLTest.cpp#L1132-L1132 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicCeil, "intrinsics/Ceil.sksl");

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

// Port of: tests/SkSLTest.cpp#L1135-L1135 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicClampFloat,
    "intrinsics/ClampFloat.sksl"
);

// Port of: tests/SkSLTest.cpp#L1136-L1136 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicCross, "intrinsics/Cross.sksl");

// Port of: tests/SkSLTest.cpp#L1137-L1137 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicDegrees,
    "intrinsics/Degrees.sksl"
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

// Port of: tests/SkSLTest.cpp#L1141-L1141 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicDot, "intrinsics/Dot.sksl");

// Port of: tests/SkSLTest.cpp#L1142-L1142 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicFract, "intrinsics/Fract.sksl");

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

// Port of: tests/SkSLTest.cpp#L1145-L1145 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicFloor, "intrinsics/Floor.sksl");

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

// Port of: tests/SkSLTest.cpp#L1149-L1149 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicLength,
    "intrinsics/Length.sksl"
);

// Port of: tests/SkSLTest.cpp#L1150-L1150 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicMatrixCompMultES2,
    "intrinsics/MatrixCompMultES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1151-L1151 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicMatrixCompMultES3,
    "intrinsics/MatrixCompMultES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1152-L1152 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicMaxFloat,
    "intrinsics/MaxFloat.sksl"
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

// Port of: tests/SkSLTest.cpp#L1155-L1155 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicMinFloat,
    "intrinsics/MinFloat.sksl"
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

// Port of: tests/SkSLTest.cpp#L1158-L1158 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicMixFloatES2,
    "intrinsics/MixFloatES2.sksl"
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

// Port of: tests/SkSLTest.cpp#L1161-L1161 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicNot, "intrinsics/Not.sksl");

// Port of: tests/SkSLTest.cpp#L1162-L1162 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    IntrinsicOuterProduct,
    "intrinsics/OuterProduct.sksl"
);

// Port of: tests/SkSLTest.cpp#L1163-L1163 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicRadians,
    "intrinsics/Radians.sksl"
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

// Port of: tests/SkSLTest.cpp#L1166-L1166 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicSaturate,
    "intrinsics/Saturate.sksl"
);

// Port of: tests/SkSLTest.cpp#L1167-L1167 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IntrinsicSignFloat,
    "intrinsics/SignFloat.sksl"
);

// Port of: tests/SkSLTest.cpp#L1168-L1168 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntrinsicSignInt,
    "intrinsics/SignInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1169-L1169 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicSqrt, "intrinsics/Sqrt.sksl");

// Port of: tests/SkSLTest.cpp#L1170-L1170 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IntrinsicStep, "intrinsics/Step.sksl");

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

// Port of: tests/SkSLTest.cpp#L1176-L1176 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ChildEffectSimple,
    "runtime/ChildEffectSimple.rts"
);

// Port of: tests/SkSLTest.cpp#L1177-L1177 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU.or(SkSLTestFlags::PRIV),
    ChildEffectSpecializationFanOut,
    "runtime/ChildEffectSpecializationFanOut.privrts"
);

// Port of: tests/SkSLTest.cpp#L1178-L1178 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    Commutative,
    "runtime/Commutative.rts"
);

// Port of: tests/SkSLTest.cpp#L1179-L1179 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, DivideByZero, "runtime/DivideByZero.rts");

// Port of: tests/SkSLTest.cpp#L1180-L1180 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionParameterAliasingFirst,
    "runtime/FunctionParameterAliasingFirst.rts"
);

// Port of: tests/SkSLTest.cpp#L1181-L1181 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionParameterAliasingSecond,
    "runtime/FunctionParameterAliasingSecond.rts"
);

// Port of: tests/SkSLTest.cpp#L1182-L1182 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IfElseBinding,
    "runtime/IfElseBinding.rts"
);

// Port of: tests/SkSLTest.cpp#L1183-L1183 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    IncrementDisambiguation,
    "runtime/IncrementDisambiguation.rts"
);

// Port of: tests/SkSLTest.cpp#L1184-L1184 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, LoopFloat, "runtime/LoopFloat.rts");

// Port of: tests/SkSLTest.cpp#L1185-L1185 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, LoopInt, "runtime/LoopInt.rts");

// Port of: tests/SkSLTest.cpp#L1186-L1186 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    Ossfuzz418486361,
    "runtime/Ossfuzz418486361.rts"
);

// Port of: tests/SkSLTest.cpp#L1187-L1187 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Ossfuzz52603, "runtime/Ossfuzz52603.rts");

// Port of: tests/SkSLTest.cpp#L1188-L1188 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    QualifierOrder,
    "runtime/QualifierOrder.rts"
);

// Port of: tests/SkSLTest.cpp#L1189-L1189 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    PrecisionQualifiers,
    "runtime/PrecisionQualifiers.rts"
);

// Port of: tests/SkSLTest.cpp#L1190-L1190 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SharedFunctions,
    "runtime/SharedFunctions.rts"
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

// Port of: tests/SkSLTest.cpp#L1200-L1200 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ArrayFollowedByScalar,
    "shared/ArrayFollowedByScalar.sksl"
);

// Port of: tests/SkSLTest.cpp#L1201-L1201 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, ArrayTypes, "shared/ArrayTypes.sksl");

// Port of: tests/SkSLTest.cpp#L1202-L1202 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Assignment, "shared/Assignment.sksl");

// Port of: tests/SkSLTest.cpp#L1203-L1203 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    CastsRoundTowardZero,
    "shared/CastsRoundTowardZero.sksl"
);

// Port of: tests/SkSLTest.cpp#L1204-L1204 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    CommaMixedTypes,
    "shared/CommaMixedTypes.sksl"
);

// Port of: tests/SkSLTest.cpp#L1205-L1205 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    CommaSideEffects,
    "shared/CommaSideEffects.sksl"
);

// Port of: tests/SkSLTest.cpp#L1206-L1206 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    CompileTimeConstantVariables,
    "shared/CompileTimeConstantVariables.sksl"
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

// Port of: tests/SkSLTest.cpp#L1209-L1209 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, ConstantIf, "shared/ConstantIf.sksl");

// Port of: tests/SkSLTest.cpp#L1210-L1210 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ConstArray,
    "shared/ConstArray.sksl"
);

// Port of: tests/SkSLTest.cpp#L1211-L1211 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ConstVariableComparison,
    "shared/ConstVariableComparison.sksl"
);

// Port of: tests/SkSLTest.cpp#L1212-L1212 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, DeadGlobals, "shared/DeadGlobals.sksl");

// Port of: tests/SkSLTest.cpp#L1213-L1213 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DeadLoopVariable,
    "shared/DeadLoopVariable.sksl"
);

// Port of: tests/SkSLTest.cpp#L1214-L1214 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    DeadIfStatement,
    "shared/DeadIfStatement.sksl"
);

// Port of: tests/SkSLTest.cpp#L1215-L1215 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, DeadReturn, "shared/DeadReturn.sksl");

// Port of: tests/SkSLTest.cpp#L1216-L1216 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DeadReturnES3,
    "shared/DeadReturnES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1217-L1217 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    DeadStripFunctions,
    "shared/DeadStripFunctions.sksl"
);

// Port of: tests/SkSLTest.cpp#L1218-L1218 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    DependentInitializers,
    "shared/DependentInitializers.sksl"
);

// Port of: tests/SkSLTest.cpp#L1219-L1219 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    DoubleNegation,
    "shared/DoubleNegation.sksl"
);

// Port of: tests/SkSLTest.cpp#L1220-L1220 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    DoWhileControlFlow,
    "shared/DoWhileControlFlow.sksl"
);

// Port of: tests/SkSLTest.cpp#L1221-L1221 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    EmptyBlocksES2,
    "shared/EmptyBlocksES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1222-L1222 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    EmptyBlocksES3,
    "shared/EmptyBlocksES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1223-L1223 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ForLoopControlFlow,
    "shared/ForLoopControlFlow.sksl"
);

// Port of: tests/SkSLTest.cpp#L1224-L1224 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ForLoopMultipleInitES3,
    "shared/ForLoopMultipleInitES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1225-L1225 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ForLoopShadowing,
    "shared/ForLoopShadowing.sksl"
);

// Port of: tests/SkSLTest.cpp#L1226-L1226 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionAnonymousParameters,
    "shared/FunctionAnonymousParameters.sksl"
);

// Port of: tests/SkSLTest.cpp#L1227-L1227 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionArgTypeMatch,
    "shared/FunctionArgTypeMatch.sksl"
);

// Port of: tests/SkSLTest.cpp#L1228-L1228 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionReturnTypeMatch,
    "shared/FunctionReturnTypeMatch.sksl"
);

// Port of: tests/SkSLTest.cpp#L1229-L1229 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Functions, "shared/Functions.sksl");

// Port of: tests/SkSLTest.cpp#L1230-L1230 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    FunctionPrototype,
    "shared/FunctionPrototype.sksl"
);

// Port of: tests/SkSLTest.cpp#L1231-L1231 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    GeometricIntrinsics,
    "shared/GeometricIntrinsics.sksl"
);

// Port of: tests/SkSLTest.cpp#L1232-L1232 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, HelloWorld, "shared/HelloWorld.sksl");

// Port of: tests/SkSLTest.cpp#L1233-L1233 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Hex, "shared/Hex.sksl");

// Port of: tests/SkSLTest.cpp#L1234-L1234 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    HexUnsigned,
    "shared/HexUnsigned.sksl"
);

// Port of: tests/SkSLTest.cpp#L1235-L1235 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, IfStatement, "shared/IfStatement.sksl");

// Port of: tests/SkSLTest.cpp#L1236-L1236 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InoutParameters,
    "shared/InoutParameters.sksl"
);

// Port of: tests/SkSLTest.cpp#L1237-L1237 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    InoutParamsAreDistinct,
    "shared/InoutParamsAreDistinct.sksl"
);

// Port of: tests/SkSLTest.cpp#L1238-L1238 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    IntegerDivisionES3,
    "shared/IntegerDivisionES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1239-L1239 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    LogicalAndShortCircuit,
    "shared/LogicalAndShortCircuit.sksl"
);

// Port of: tests/SkSLTest.cpp#L1240-L1240 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    LogicalOrShortCircuit,
    "shared/LogicalOrShortCircuit.sksl"
);

// Port of: tests/SkSLTest.cpp#L1241-L1241 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Matrices, "shared/Matrices.sksl");

// Port of: tests/SkSLTest.cpp#L1242-L1242 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatricesNonsquare,
    "shared/MatricesNonsquare.sksl"
);

// Port of: tests/SkSLTest.cpp#L1243-L1243 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixConstructorsES2,
    "shared/MatrixConstructorsES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1244-L1244 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixConstructorsES3,
    "shared/MatrixConstructorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1245-L1245 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixEquality,
    "shared/MatrixEquality.sksl"
);

// Port of: tests/SkSLTest.cpp#L1246-L1246 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixIndexLookup,
    "shared/MatrixIndexLookup.sksl"
);

// Port of: tests/SkSLTest.cpp#L1247-L1247 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixIndexStore,
    "shared/MatrixIndexStore.sksl"
);

// Port of: tests/SkSLTest.cpp#L1248-L1248 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixOpEqualsES2,
    "shared/MatrixOpEqualsES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1249-L1249 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    MatrixOpEqualsES3,
    "shared/MatrixOpEqualsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1250-L1250 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixScalarMath,
    "shared/MatrixScalarMath.sksl"
);

// Port of: tests/SkSLTest.cpp#L1251-L1251 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixSwizzleStore,
    "shared/MatrixSwizzleStore.sksl"
);

// Port of: tests/SkSLTest.cpp#L1252-L1252 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MatrixToVectorCast,
    "shared/MatrixToVectorCast.sksl"
);

// Port of: tests/SkSLTest.cpp#L1253-L1253 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    MultipleAssignments,
    "shared/MultipleAssignments.sksl"
);

// Port of: tests/SkSLTest.cpp#L1254-L1254 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, NumberCasts, "shared/NumberCasts.sksl");

// Port of: tests/SkSLTest.cpp#L1255-L1255 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    NestedComparisonIntrinsics,
    "shared/NestedComparisonIntrinsics.sksl"
);

// Port of: tests/SkSLTest.cpp#L1256-L1256 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, OperatorsES2, "shared/OperatorsES2.sksl");

// Port of: tests/SkSLTest.cpp#L1257-L1257 (chrome/m156)
sksl_test!(
    SkSLTestFlags::GPU_ES3,
    OperatorsES3,
    "shared/OperatorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1258-L1258 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Ossfuzz36852, "shared/Ossfuzz36852.sksl");

// Port of: tests/SkSLTest.cpp#L1259-L1259 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, OutParams, "shared/OutParams.sksl");

// Port of: tests/SkSLTest.cpp#L1260-L1260 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    OutParamsAreDistinct,
    "shared/OutParamsAreDistinct.sksl"
);

// Port of: tests/SkSLTest.cpp#L1261-L1261 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    OutParamsAreDistinctFromGlobal,
    "shared/OutParamsAreDistinctFromGlobal.sksl"
);

// Port of: tests/SkSLTest.cpp#L1262-L1262 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    OutParamsFunctionCallInArgument,
    "shared/OutParamsFunctionCallInArgument.sksl"
);

// Port of: tests/SkSLTest.cpp#L1263-L1263 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    OutParamsDoubleSwizzle,
    "shared/OutParamsDoubleSwizzle.sksl"
);

// Port of: tests/SkSLTest.cpp#L1264-L1264 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    PostfixExpressions,
    "shared/PostfixExpressions.sksl"
);

// Port of: tests/SkSLTest.cpp#L1265-L1265 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    PrefixExpressionsES2,
    "shared/PrefixExpressionsES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1266-L1266 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    PrefixExpressionsES3,
    "shared/PrefixExpressionsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1267-L1267 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ReservedInGLSLButAllowedInSkSL,
    "shared/ReservedInGLSLButAllowedInSkSL.sksl"
);

// Port of: tests/SkSLTest.cpp#L1268-L1268 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, ResizeMatrix, "shared/ResizeMatrix.sksl");

// Port of: tests/SkSLTest.cpp#L1269-L1269 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ResizeMatrixNonsquare,
    "shared/ResizeMatrixNonsquare.sksl"
);

// Port of: tests/SkSLTest.cpp#L1270-L1270 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ReturnsValueOnEveryPathES2,
    "shared/ReturnsValueOnEveryPathES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1271-L1271 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ReturnsValueOnEveryPathES3,
    "shared/ReturnsValueOnEveryPathES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1272-L1272 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    ScalarConversionConstructorsES2,
    "shared/ScalarConversionConstructorsES2.sksl"
);

// Port of: tests/SkSLTest.cpp#L1273-L1273 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    ScalarConversionConstructorsES3,
    "shared/ScalarConversionConstructorsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1274-L1274 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, ScopedSymbol, "shared/ScopedSymbol.sksl");

// Port of: tests/SkSLTest.cpp#L1275-L1275 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StackingVectorCasts,
    "shared/StackingVectorCasts.sksl"
);

// Port of: tests/SkSLTest.cpp#L1276-L1276 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU.or(SkSLTestFlags::GPU_ES3),
    StaticSwitch,
    "shared/StaticSwitch.sksl"
);

// Port of: tests/SkSLTest.cpp#L1277-L1277 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructArrayFollowedByScalar,
    "shared/StructArrayFollowedByScalar.sksl"
);

// Port of: tests/SkSLTest.cpp#L1278-L1278 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructIndexLookup,
    "shared/StructIndexLookup.sksl"
);

// Port of: tests/SkSLTest.cpp#L1279-L1279 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructIndexStore,
    "shared/StructIndexStore.sksl"
);

// Port of: tests/SkSLTest.cpp#L1281-L1281 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3,
    StructComparison,
    "shared/StructComparison.sksl"
);

// Port of: tests/SkSLTest.cpp#L1282-L1282 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    StructsInFunctions,
    "shared/StructsInFunctions.sksl"
);

// Port of: tests/SkSLTest.cpp#L1283-L1283 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, Switch, "shared/Switch.sksl");

// Port of: tests/SkSLTest.cpp#L1284-L1284 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchDefaultOnly,
    "shared/SwitchDefaultOnly.sksl"
);

// Port of: tests/SkSLTest.cpp#L1285-L1285 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchWithFallthrough,
    "shared/SwitchWithFallthrough.sksl"
);

// Port of: tests/SkSLTest.cpp#L1286-L1286 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchWithFallthroughAndVarDecls,
    "shared/SwitchWithFallthroughAndVarDecls.sksl"
);

// Port of: tests/SkSLTest.cpp#L1287-L1287 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchWithFallthroughGroups,
    "shared/SwitchWithFallthroughGroups.sksl"
);

// Port of: tests/SkSLTest.cpp#L1288-L1288 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwitchWithLoops,
    "shared/SwitchWithLoops.sksl"
);

// Port of: tests/SkSLTest.cpp#L1289-L1289 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwitchWithLoopsES3,
    "shared/SwitchWithLoopsES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1290-L1290 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleAsLValue,
    "shared/SwizzleAsLValue.sksl"
);

// Port of: tests/SkSLTest.cpp#L1291-L1291 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwizzleAsLValueES3,
    "shared/SwizzleAsLValueES3.sksl"
);

// Port of: tests/SkSLTest.cpp#L1292-L1292 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleBoolConstants,
    "shared/SwizzleBoolConstants.sksl"
);

// Port of: tests/SkSLTest.cpp#L1293-L1293 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleByConstantIndex,
    "shared/SwizzleByConstantIndex.sksl"
);

// Port of: tests/SkSLTest.cpp#L1294-L1294 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    SwizzleByIndex,
    "shared/SwizzleByIndex.sksl"
);

// Port of: tests/SkSLTest.cpp#L1295-L1295 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleConstants,
    "shared/SwizzleConstants.sksl"
);

// Port of: tests/SkSLTest.cpp#L1296-L1296 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleIndexLookup,
    "shared/SwizzleIndexLookup.sksl"
);

// Port of: tests/SkSLTest.cpp#L1297-L1297 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleIndexStore,
    "shared/SwizzleIndexStore.sksl"
);

// Port of: tests/SkSLTest.cpp#L1298-L1298 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, SwizzleLTRB, "shared/SwizzleLTRB.sksl");

// Port of: tests/SkSLTest.cpp#L1299-L1299 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, SwizzleOpt, "shared/SwizzleOpt.sksl");

// Port of: tests/SkSLTest.cpp#L1300-L1300 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleScalar,
    "shared/SwizzleScalar.sksl"
);

// Port of: tests/SkSLTest.cpp#L1301-L1301 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleScalarBool,
    "shared/SwizzleScalarBool.sksl"
);

// Port of: tests/SkSLTest.cpp#L1302-L1302 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    SwizzleScalarInt,
    "shared/SwizzleScalarInt.sksl"
);

// Port of: tests/SkSLTest.cpp#L1303-L1303 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TemporaryIndexLookup,
    "shared/TemporaryIndexLookup.sksl"
);

// Port of: tests/SkSLTest.cpp#L1304-L1304 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryAsLValueEntirelyFoldable,
    "shared/TernaryAsLValueEntirelyFoldable.sksl"
);

// Port of: tests/SkSLTest.cpp#L1305-L1305 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryAsLValueFoldableTest,
    "shared/TernaryAsLValueFoldableTest.sksl"
);

// Port of: tests/SkSLTest.cpp#L1306-L1306 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryComplexNesting,
    "shared/TernaryComplexNesting.sksl"
);

// Port of: tests/SkSLTest.cpp#L1307-L1307 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryExpression,
    "shared/TernaryExpression.sksl"
);

// Port of: tests/SkSLTest.cpp#L1308-L1308 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryNesting,
    "shared/TernaryNesting.sksl"
);

// Port of: tests/SkSLTest.cpp#L1309-L1309 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernaryOneZeroOptimization,
    "shared/TernaryOneZeroOptimization.sksl"
);

// Port of: tests/SkSLTest.cpp#L1310-L1310 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    TernarySideEffects,
    "shared/TernarySideEffects.sksl"
);

// Port of: tests/SkSLTest.cpp#L1311-L1311 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    UnaryPositiveNegative,
    "shared/UnaryPositiveNegative.sksl"
);

// Port of: tests/SkSLTest.cpp#L1312-L1312 (chrome/m156)
sksl_test!(SkSLTestFlags::CPU, UniformArray, "shared/UniformArray.sksl");

// Port of: tests/SkSLTest.cpp#L1313-L1313 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    UniformMatrixArray,
    "shared/UniformMatrixArray.sksl"
);

// Port of: tests/SkSLTest.cpp#L1314-L1314 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    UniformMatrixResize,
    "shared/UniformMatrixResize.sksl"
);

// Port of: tests/SkSLTest.cpp#L1315-L1315 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    UnusedVariables,
    "shared/UnusedVariables.sksl"
);

// Port of: tests/SkSLTest.cpp#L1316-L1316 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VectorConstructors,
    "shared/VectorConstructors.sksl"
);

// Port of: tests/SkSLTest.cpp#L1317-L1317 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VectorToMatrixCast,
    "shared/VectorToMatrixCast.sksl"
);

// Port of: tests/SkSLTest.cpp#L1318-L1318 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VectorScalarMath,
    "shared/VectorScalarMath.sksl"
);

// Port of: tests/SkSLTest.cpp#L1319-L1319 (chrome/m156)
sksl_test!(
    SkSLTestFlags::ES3.or(SkSLTestFlags::GPU_ES3),
    WhileLoopControlFlow,
    "shared/WhileLoopControlFlow.sksl"
);

// Port of: tests/SkSLTest.cpp#L1321-L1321 (chrome/m156)
sksl_test!(
    SkSLTestFlags::CPU,
    VoidInSequenceExpressions,
    "workarounds/VoidInSequenceExpressions.sksl"
);
