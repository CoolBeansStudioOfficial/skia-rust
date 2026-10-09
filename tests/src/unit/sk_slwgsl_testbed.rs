// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLWGSLTestbed.cpp (chrome/m156)

//! A place to try a `SkSL` program through the WGSL generator.
//!
//! Mapping notes: `SkSL::NativeShader` is the `String` that `to_wgsl_native` returns.

use skia_rust_sksl::codegen::wgsl::to_wgsl_native;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};
use skia_rust_sksl::util::ShaderCapsFactory;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/SkSLWGSLTestbed.cpp#L20-L37 (chrome/m156)
fn test(r: &mut Reporter, src: &str, kind: ProgramKind) {
    let mut compiler = Compiler::new();
    let settings = ProgramSettings::default();
    let program = compiler.convert_program(kind, src.as_bytes(), settings);
    if let Some(mut program) = program {
        let output = to_wgsl_native(
            compiler.context_mut(),
            &mut program,
            ShaderCapsFactory::default_caps(),
        );
        reporter_assert!(r, output.is_some());
        reporter_assert!(r, output.is_some_and(|text| !text.is_empty()));
        // SkDebugf("WGSL output:\n\n%s", output.fText.c_str());
    } else {
        eprintln!(
            "Unexpected error compiling {}\n{}",
            src,
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
        reporter_assert!(r, false);
    }
}

// Port of: tests/SkSLWGSLTestbed.cpp#L39-L47 (chrome/m156)
def_test!(SkSLWGSLTestbed, |r| {
    // Add in your SkSL here.
    test(
        r,
        r"
             void main() {
                 sk_FragColor = half4(0);
             }
         ",
        ProgramKind::Fragment,
    );
});
