// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLPipelineStageTestbed.cpp (chrome/m156)

//! `SkSLPipelineStageTestbed`: one program, compiled and run through the pipeline-stage generator.
//!
//! The callbacks are the ones `skslc` uses for `.stage` files (`crate::tools::skslc::StageCallbacks`),
//! which the C++ test defines inline with the same behaviour.

use skia_rust_sksl::codegen::pipeline_stage::convert_program;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

use crate::tools::skslc::StageCallbacks;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/SkSLPipelineStageTestbed.cpp#L16-L41 (chrome/m156)
fn test(r: &mut Reporter, src: &str, kind: ProgramKind) {
    let mut compiler = Compiler::new();
    let settings = ProgramSettings::default();
    let Some(program) = compiler.convert_program(kind, src.as_bytes(), settings) else {
        errorf!(
            r,
            "Unexpected error compiling {}\n{}",
            src,
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        reporter_assert!(r, false);
        return;
    };
    let mut callbacks = StageCallbacks::default();
    convert_program(
        &program,
        "_coords",
        "_inColor",
        "_canvasColor",
        &mut callbacks,
    );
    reporter_assert!(r, !callbacks.output.is_empty());
}

def_test!(SkSLPipelineStageTestbed, |r| {
    // Add in your SkSL here.
    test(
        r,
        r"
             half4 main(float2) {
                 return half4(0);
             }
         ",
        ProgramKind::PrivateRuntimeShader,
    );
});
