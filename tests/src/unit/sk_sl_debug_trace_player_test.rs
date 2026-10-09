// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLDebugTracePlayerTest.cpp (chrome/m156)

//! `SkSLDebugTracePlayer`: small programs are compiled with debug traces, run over one pixel, and
//! their traces are played back step by step.
//!
//! Mapping notes: `sk_sp<DebugTracePriv>` is an `Arc<DebugTracePriv>`, `LineNumberMap` is a
//! `HashMap<i32, i32>`, and the `SkRasterPipeline` that runs the program is a `RasterPipeline`.

// The ported tests keep the C++ declaration order and function lengths.
#![allow(clippy::too_many_lines, clippy::items_after_statements)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::effect_priv::SHADER_SCRATCH;
use skia_rust_core::raster_pipeline::{MemView, MemoryBindings, RasterPipeline};
use skia_rust_sksl::codegen::rp::make_raster_pipeline_program;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};
use skia_rust_sksl::tracing::DebugTracePriv;
use skia_rust_sksl::tracing::player::{DebugTracePlayer, VariableData};

use crate::{Reporter, def_test, errorf, reporter_assert};

/// `make_trace`: compiles `src` (unoptimized) with trace ops, runs it on the pixel at (0.5, 0.5),
/// and returns its debug trace.
fn make_trace(r: &mut Reporter, src: &str) -> Arc<DebugTracePriv> {
    let mut compiler = Compiler::new();
    let settings = ProgramSettings {
        optimize: false,
        ..ProgramSettings::default()
    };
    let Some(mut program) =
        compiler.convert_program(ProgramKind::RuntimeShader, src.as_bytes(), settings)
    else {
        errorf!(r, "Unexpected error compiling {}", src);
        return Arc::new(DebugTracePriv::default());
    };
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    reporter_assert!(r, main.is_some());
    let Some(main) = main else {
        return Arc::new(DebugTracePriv::default());
    };

    // Compile our program.
    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    let debug_trace = DebugTracePriv::default();
    let raster_prog = make_raster_pipeline_program(&mut program, main, Some(debug_trace), true);
    reporter_assert!(r, raster_prog.is_some());
    let Some(raster_prog) = raster_prog else {
        return Arc::new(DebugTracePriv::default());
    };
    let trace = raster_prog
        .debug_trace_handle()
        .expect("a program made with a debug trace has one");

    // Append the SkSL program to the raster pipeline, and run it at xy=(0.5, 0.5).
    static COORDINATES: [f32; 4] = [0.5, 0.5, 0.0, 1.0];
    pipeline.append_constant_color(&alloc, &COORDINATES);
    raster_prog.append_stages(&mut pipeline, &alloc, None, &[]);
    let mut scratch = alloc.scratch_buffer();
    let mut mem = MemoryBindings::new().with(SHADER_SCRATCH, MemView::write(&mut scratch));
    pipeline.run(0, 0, 1, 1, &mut mem);

    trace
}

/// `make_stack_string`: the names of the functions on the call stack, outermost first.
fn make_stack_string(trace: &DebugTracePriv, player: &DebugTracePlayer) -> String {
    let call_stack = player.get_call_stack();
    let mut text = String::new();
    let mut separator = "";
    for frame in call_stack {
        text += separator;
        separator = " -> ";

        match usize::try_from(frame) {
            Ok(index) if index < trace.func_info.len() => {
                text += &trace.func_info[index].name;
            }
            _ => text += "???",
        }
    }
    text
}

/// `make_vars_string`: the variables, with `##` marking the ones written by the last step.
fn make_vars_string(trace: &DebugTracePriv, vars: &[VariableData]) -> String {
    let mut text = String::new();
    let mut separator = "";
    for var in vars {
        text += separator;
        separator = ", ";

        let Ok(slot) = usize::try_from(var.slot_index) else {
            text += "???";
            continue;
        };
        if slot >= trace.slot_info.len() {
            text += "???";
            continue;
        }

        let info = &trace.slot_info[slot];
        text += if var.dirty { "##" } else { "" };
        text += &info.name;
        text += &trace.get_slot_component_suffix(slot);
        text += " = ";
        text += &trace.slot_value_to_string(slot, var.value);
    }
    text
}

/// `make_local_vars_string`: the variables of the innermost stack frame.
fn make_local_vars_string(trace: &DebugTracePriv, player: &DebugTracePlayer) -> String {
    let frame = player.get_stack_depth() - 1;
    make_vars_string(trace, &player.get_local_variables(frame))
}

/// `make_global_vars_string`: the variables of the global scope.
fn make_global_vars_string(trace: &DebugTracePriv, player: &DebugTracePlayer) -> String {
    make_vars_string(trace, &player.get_global_variables())
}

def_test!(SkSLTracePlayerCanResetToNull, |r| {
    let mut player = DebugTracePlayer::default();
    player.reset(None);

    // We should be in a reasonable state.
    reporter_assert!(r, player.cursor() == 0);
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(r, player.get_line_numbers_reached().is_empty());
});

def_test!(SkSLTracePlayerHelloWorld, |r| {
    let trace = make_trace(
        r,
        r"                       // Line 1
half4 main(float2 xy) {   // Line 2
    return half4(2 + 2);  // Line 3
}                         // Line 4
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    // We have not started tracing yet.
    reporter_assert!(r, player.cursor() == 0);
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 1)])
    );

    player.step();

    // We should now be inside main.
    reporter_assert!(r, player.cursor() > 0);
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(r, player.get_local_variables(0).len() == 2); // xy

    player.step();

    // We have now completed the trace.
    reporter_assert!(r, player.cursor() > 0);
    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 4, ##[main].result.y = 4, ##[main].result.z = ",
                "4, ##[main].result.w = 4"
            )
    );
});

def_test!(SkSLTracePlayerReset, |r| {
    let trace = make_trace(
        r,
        r"                       // Line 1
half4 main(float2 xy) {   // Line 2
    return half4(2 + 2);  // Line 3
}                         // Line 4
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    // We have not started tracing yet.
    reporter_assert!(r, player.cursor() == 0);
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(r, player.get_global_variables().is_empty());

    player.step();

    // We should now be inside main.
    reporter_assert!(r, player.cursor() > 0);
    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(r, player.get_local_variables(0).len() == 2); // xy

    player.reset(Some(Arc::clone(&trace)));

    // We should be back to square one.
    reporter_assert!(r, player.cursor() == 0);
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(r, player.get_global_variables().is_empty());
});

def_test!(SkSLTracePlayerFunctions, |r| {
    let trace = make_trace(
        r,
        r"                             // Line 1
int fnB() {                     // Line 2
    return 2 + 2;               // Line 3
}                               // Line 4
int fnA() {                     // Line 5
    return fnB();               // Line 6
}                               // Line 7
half4 main(float2 xy) {         // Line 8
    return half4(fnA());        // Line 9
}                               // Line 10
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    // We have not started tracing yet.
    reporter_assert!(r, player.cursor() == 0);
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 1), (6, 1), (9, 1)])
    );

    player.step();

    // We should now be inside main.
    reporter_assert!(r, !player.trace_has_completed());
    reporter_assert!(r, player.get_current_line() == 9);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(r, player.get_global_variables().is_empty());
    reporter_assert!(r, player.get_local_variables(0).len() == 2); // xy

    player.step_over();

    // We should now have completed execution.
    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(r, player.get_current_line() == -1);
    reporter_assert!(r, player.get_call_stack().is_empty());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 4, ##[main].result.y = 4, ##[main].result.z = ",
                "4, ##[main].result.w = 4"
            )
    );

    // Watch the stack grow and shrink as single-step.
    player.reset(Some(Arc::clone(&trace)));
    player.step();

    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(r, player.get_current_line_in_stack_frame(0) == 9);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    reporter_assert!(r, make_global_vars_string(&trace, &player).is_empty());
    player.step();

    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fnA()"
    );
    reporter_assert!(r, player.get_current_line_in_stack_frame(0) == 9);
    reporter_assert!(r, player.get_current_line_in_stack_frame(1) == 6);
    reporter_assert!(r, make_local_vars_string(&trace, &player).is_empty());
    reporter_assert!(r, make_global_vars_string(&trace, &player).is_empty());
    player.step();

    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fnA() -> int fnB()"
    );
    reporter_assert!(r, make_local_vars_string(&trace, &player).is_empty());
    reporter_assert!(r, make_global_vars_string(&trace, &player).is_empty());
    reporter_assert!(r, player.get_current_line_in_stack_frame(0) == 9);
    reporter_assert!(r, player.get_current_line_in_stack_frame(1) == 6);
    reporter_assert!(r, player.get_current_line_in_stack_frame(2) == 3);
    player.step();

    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fnA()"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##[fnB].result = 4"
    );
    reporter_assert!(r, make_global_vars_string(&trace, &player).is_empty());
    player.step();

    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##[fnA].result = 4, xy.x = 0.5, xy.y = 0.5"
    );
    reporter_assert!(r, make_global_vars_string(&trace, &player).is_empty());

    player.step();
    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 4, ##[main].result.y = 4, ##[main].result.z = ",
                "4, ##[main].result.w = 4"
            )
    );
});

def_test!(SkSLTracePlayerVariables, |r| {
    let trace = make_trace(
        r,
        r"                                   // Line 1
float func() {                        // Line 2
    float x = 4, y = 5, z = 6;        // Line 3
    return z;                         // Line 4
}                                     // Line 5
half4 main(float2 xy) {               // Line 6
    int a = 123;                      // Line 7
    bool b = true;                    // Line 8
    func();                           // Line 9
    float4 c = float4(0, 0.5, 1, -1); // Line 10
    float3x3 d = float3x3(2);         // Line 11
    return half4(a);                  // Line 12
}                                     // Line 13
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    reporter_assert!(
        r,
        *player.get_line_numbers_reached()
            == HashMap::from([
                (3, 1),
                (4, 1),
                (7, 1),
                (8, 1),
                (9, 1),
                (10, 1),
                (11, 1),
                (12, 1)
            ])
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 7);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 8);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##a = 123, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 9);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##b = true, a = 123, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> float func()"
    );
    reporter_assert!(r, make_local_vars_string(&trace, &player).is_empty());
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> float func()"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##z = 6, ##y = 5, ##x = 4"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 9);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == "##[func].result = 6, b = true, a = 123, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 10);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "b = true, a = 123, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 11);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "##c.x = 0, ##c.y = 0.5, ##c.z = 1, ##c.w = -1, b = true, a = 123, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 12);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "##d[0][0] = 2, ##d[0][1] = 0, ##d[0][2] = 0, ",
                "##d[1][0] = 0, ##d[1][1] = 2, ##d[1][2] = 0, ",
                "##d[2][0] = 0, ##d[2][1] = 0, ##d[2][2] = 2, ",
                "c.x = 0, c.y = 0.5, c.z = 1, c.w = -1, b = true, a = 123, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );

    player.step();
    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(r, make_stack_string(&trace, &player).is_empty());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 123, ##[main].result.y = 123, ",
                "##[main].result.z = 123, ##[main].result.w = 123"
            )
    );
});

def_test!(SkSLTracePlayerVariableGroups, |r| {
    let trace = make_trace(
        r,
        r"                                   // Line 1
struct S { int x, y, z; };            // Line 2
half4 main(float2 xy) {               // Line 3
    S s;                              // Line 4
    int arr[3];                       // Line 5
    s.y = 1;                          // Line 6
    arr[1] = 2;                       // Line 7
    s.x = 3;                          // Line 8
    arr[2] = 4;                       // Line 9
    return half4(0);                  // Line 10
}                                     // Line 11
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == "##s.x = 0, ##s.y = 0, ##s.z = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 6);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "##arr[0] = 0, ##arr[1] = 0, ##arr[2] = 0, s.x = 0, s.y = 0, s.z = ",
                "0, xy.x = 0.5, xy.y = 0.5"
            )
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 7);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "s.x = 0, ##s.y = 1, s.z = 0, arr[0] = 0, arr[1] = 0, arr[2] = 0, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 8);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "arr[0] = 0, ##arr[1] = 2, arr[2] = 0, s.x = 0, s.y = 1, s.z = 0, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 9);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "##s.x = 3, s.y = 1, s.z = 0, arr[0] = 0, arr[1] = 2, arr[2] = 0, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 10);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player)
            == concat!(
                "arr[0] = 0, arr[1] = 2, ##arr[2] = 4, s.x = 3, s.y = 1, s.z = 0, ",
                "xy.x = 0.5, xy.y = 0.5"
            )
    );
});

def_test!(SkSLTracePlayerIfStatement, |r| {
    let trace = make_trace(
        r,
        r"                      // Line 1
half4 main(float2 xy) {  // Line 2
    int val;             // Line 3
    if (true) {          // Line 4
        int temp = 1;    // Line 5
        val = temp;      // Line 6
    } else {             // Line 7
        val = 2;         // Line 8
    }                    // Line 9
    if (false) {         // Line 10
        int temp = 3;    // Line 11
        val = temp;      // Line 12
    } else {             // Line 13
        val = 4;         // Line 14
    }                    // Line 15
    return half4(val);   // Line 16
}                        // Line 17
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    reporter_assert!(
        r,
        *player.get_line_numbers_reached()
            == HashMap::from([(3, 1), (4, 1), (5, 1), (6, 1), (10, 1), (14, 1), (16, 1)])
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "val = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 6);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##temp = 1, val = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    // We skip over the false-branch.
    reporter_assert!(r, player.get_current_line() == 10);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    // We skip over the true-branch.
    reporter_assert!(r, player.get_current_line() == 14);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "val = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 16);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 4, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 4, ##[main].result.y = 4, ##[main].result.z = ",
                "4, ##[main].result.w = 4"
            )
    );
});

def_test!(SkSLTracePlayerForLoop, |r| {
    let trace = make_trace(
        r,
        r"                                // Line 1
half4 main(float2 xy) {            // Line 2
    int val = 0;                   // Line 3
    for (int x = 1; x < 3; ++x) {  // Line 4
        val = x;                   // Line 5
    }                              // Line 6
    return half4(val);             // Line 7
}                                  // Line 8
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));

    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 1), (4, 3), (5, 2), (7, 1)])
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 3), (5, 2), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 2), (5, 2), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 2), (5, 1), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##x = 1, val = 0, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 1), (5, 1), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 1, x = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 1), (5, 0), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##x = 2, val = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 0), (5, 0), (7, 1)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##val = 2, x = 2, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 7);
    reporter_assert!(
        r,
        *player.get_line_numbers_reached() == HashMap::from([(3, 0), (4, 0), (5, 0), (7, 0)])
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "val = 2, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 2, ##[main].result.y = 2, ##[main].result.z = ",
                "2, ##[main].result.w = 2"
            )
    );
});

def_test!(SkSLTracePlayerStepOut, |r| {
    let trace = make_trace(
        r,
        r"                      // Line 1
int fn() {               // Line 2
    int a = 11;          // Line 3
    int b = 22;          // Line 4
    int c = 33;          // Line 5
    int d = 44;          // Line 6
    return d;            // Line 7
}                        // Line 8
half4 main(float2 xy) {  // Line 9
    return half4(fn());  // Line 10
}                        // Line 11
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    reporter_assert!(
        r,
        *player.get_line_numbers_reached()
            == HashMap::from([(3, 1), (4, 1), (5, 1), (6, 1), (7, 1), (10, 1)])
    );
    player.step();

    // We should now be inside main.
    reporter_assert!(r, player.get_current_line() == 10);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    player.step();

    // We should now be inside fn.
    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fn()"
    );
    reporter_assert!(r, make_local_vars_string(&trace, &player).is_empty());
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fn()"
    );
    reporter_assert!(r, make_local_vars_string(&trace, &player) == "##a = 11");
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy) -> int fn()"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##b = 22, a = 11"
    );
    player.step_out();

    // We should now be back inside main(), right where we left off.
    reporter_assert!(r, player.get_current_line() == 10);
    reporter_assert!(
        r,
        make_stack_string(&trace, &player) == "half4 main(float2 xy)"
    );
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##[fn].result = 44, xy.x = 0.5, xy.y = 0.5"
    );
    player.step_out();

    reporter_assert!(r, player.trace_has_completed());
    reporter_assert!(
        r,
        make_global_vars_string(&trace, &player)
            == concat!(
                "##[main].result.x = 44, ##[main].result.y = 44, ##[main].result.z ",
                "= 44, ##[main].result.w = 44"
            )
    );
});

def_test!(SkSLTracePlayerVariableScope, |r| {
    let trace = make_trace(
        r,
        r"                         // Line 1
half4 main(float2 xy) {     // Line 2
    int a = 1;              // Line 3
    {                       // Line 4
        int b = 2;          // Line 5
        {                   // Line 6
            int c = 3;      // Line 7
        }                   // Line 8
        int d = 4;          // Line 9
    }                       // Line 10
    int e = 5;              // Line 11
    {                       // Line 12
        int f = 6;          // Line 13
        {                   // Line 14
            int g = 7;      // Line 15
        }                   // Line 16
        int h = 8;          // Line 17
    }                       // Line 18
    int i = 9;              // Line 19
    return half4(0);        // Line 20
}
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    reporter_assert!(
        r,
        *player.get_line_numbers_reached()
            == HashMap::from([
                (3, 1),
                (5, 1),
                (7, 1),
                (9, 1),
                (11, 1),
                (13, 1),
                (15, 1),
                (17, 1),
                (19, 1),
                (20, 1)
            ])
    );
    player.step();

    // We should now be inside main.
    reporter_assert!(r, player.get_current_line() == 3);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##xy.x = 0.5, ##xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 5);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 7);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##b = 2, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 9);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "b = 2, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 11);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 13);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##e = 5, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 15);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##f = 6, e = 5, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 17);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "f = 6, e = 5, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 19);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "e = 5, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.get_current_line() == 20);
    reporter_assert!(
        r,
        make_local_vars_string(&trace, &player) == "##i = 9, e = 5, a = 1, xy.x = 0.5, xy.y = 0.5"
    );
    player.step();

    reporter_assert!(r, player.trace_has_completed());
});

def_test!(SkSLTracePlayerNestedBlocks, |r| {
    let trace = make_trace(
        r,
        r"                         // Line 1
half4 main(float2 xy) {     // Line 2
    {{{{{{{                 // Line 3
            int a, b;       // Line 4
    }}}}}}}                 // Line 5
    return half4(0);        // Line 6
}
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    player.step();

    reporter_assert!(r, player.get_current_line() == 6);
    player.step();

    reporter_assert!(r, player.trace_has_completed());
});

def_test!(SkSLTracePlayerSwitchStatement, |r| {
    let trace = make_trace(
        r,
        r"                         // Line 1
half4 main(float2 xy) {     // Line 2
    int x = 2;              // Line 3
    switch (x) {            // Line 4
        case 1:             // Line 5
            break;          // Line 6
        case 2:             // Line 7
            ++x;            // Line 8
        case 3:             // Line 9
            break;          // Line 10
        case 4:             // Line 11
    }                       // Line 12
    return half4(x);        // Line 13
}
",
    );
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.step();

    reporter_assert!(r, player.get_current_line() == 3);
    player.step();

    reporter_assert!(r, player.get_current_line() == 4);
    player.step();

    reporter_assert!(r, player.get_current_line() == 8);
    player.step();

    reporter_assert!(r, player.get_current_line() == 10);
    player.step();

    reporter_assert!(r, player.get_current_line() == 13);
    player.step();

    reporter_assert!(r, player.trace_has_completed());
});

def_test!(SkSLTracePlayerBreakpoint, |r| {
    let trace = make_trace(
        r,
        r"                                // Line 1
int counter = 0;                   // Line 2
void func() {                      // Line 3
    --counter;                     // Line 4   BREAKPOINT 4 5
}                                  // Line 5
half4 main(float2 xy) {            // Line 6
    for (int x = 1; x <= 3; ++x) { // Line 7
        ++counter;                 // Line 8   BREAKPOINT 1 2 3
    }                              // Line 9
    func();                        // Line 10
    func();                        // Line 11
    ++counter;                     // Line 12  BREAKPOINT 6
    return half4(counter);         // Line 13
}                                  // Line 14
",
    );
    // Run the simulation with a variety of breakpoints set.
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.set_breakpoints(HashSet::from([8, 13, 20]));
    player.run();
    reporter_assert!(r, player.get_current_line() == 8);

    player.run();
    reporter_assert!(r, player.get_current_line() == 8);

    player.set_breakpoints(HashSet::from([1, 4, 8]));
    player.run();
    reporter_assert!(r, player.get_current_line() == 8);

    player.run();
    reporter_assert!(r, player.get_current_line() == 4);

    player.set_breakpoints(HashSet::from([4, 12, 14]));
    player.run();
    reporter_assert!(r, player.get_current_line() == 4);

    player.run();
    reporter_assert!(r, player.get_current_line() == 12);

    player.run();
    reporter_assert!(r, player.trace_has_completed());

    // Run the simulation again with no breakpoints set. We should reach the end of the trace
    // instantly.
    player.reset(Some(Arc::clone(&trace)));
    player.set_breakpoints(HashSet::new());
    reporter_assert!(r, !player.trace_has_completed());

    player.run();
    reporter_assert!(r, player.trace_has_completed());
});

def_test!(SkSLTracePlayerStepOverWithBreakpoint, |r| {
    let trace = make_trace(
        r,
        r"                         // Line 1
int counter = 0;            // Line 2
void func() {               // Line 3
    ++counter;              // Line 4   BREAKPOINT
}                           // Line 5
half4 main(float2 xy) {     // Line 6
    func();                 // Line 7
    return half4(counter);  // Line 8
}                           // Line 9
",
    );
    // Try stepping over with no breakpoint set; we will step over.
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.step();
    reporter_assert!(r, player.get_current_line() == 7);

    player.step_over();
    reporter_assert!(r, player.get_current_line() == 8);

    // Try stepping over with a breakpoint set; we will stop at the breakpoint.
    player.reset(Some(Arc::clone(&trace)));
    player.set_breakpoints(HashSet::from([4]));
    player.step();
    reporter_assert!(r, player.get_current_line() == 7);

    player.step_over();
    reporter_assert!(r, player.get_current_line() == 4);
});

def_test!(SkSLTracePlayerStepOutWithBreakpoint, |r| {
    let trace = make_trace(
        r,
        r"                         // Line 1
int counter = 0;            // Line 2
void func() {               // Line 3
    ++counter;              // Line 4
    ++counter;              // Line 5
    ++counter;              // Line 6   BREAKPOINT
}                           // Line 7
half4 main(float2 xy) {     // Line 8
    func();                 // Line 9
    return half4(counter);  // Line 10
}                           // Line 11
",
    );
    // Try stepping out with no breakpoint set; we will step out.
    let mut player = DebugTracePlayer::default();
    player.reset(Some(Arc::clone(&trace)));
    player.step();
    reporter_assert!(r, player.get_current_line() == 9);

    player.step();
    reporter_assert!(r, player.get_current_line() == 4);

    player.step_out();
    reporter_assert!(r, player.get_current_line() == 9);

    // Try stepping out with a breakpoint set; we will stop at the breakpoint.
    player.reset(Some(Arc::clone(&trace)));
    player.set_breakpoints(HashSet::from([6]));
    player.step();
    reporter_assert!(r, player.get_current_line() == 9);

    player.step();
    reporter_assert!(r, player.get_current_line() == 4);

    player.step_out();
    reporter_assert!(r, player.get_current_line() == 6);
});
