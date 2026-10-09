// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The WGSL generator on small programs. The goldens (`tests/sksl/**/*.wgsl`) are checked by
//! `sksl-golden-verify`; these tests pin the entry points of `to_wgsl`.

#![cfg(feature = "wgsl")]

use skia_rust_sksl::codegen::wgsl::{
    IncludeSyntheticCode, PrettyPrint, ValidateWgslProc, to_wgsl, to_wgsl_native,
};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::error_reporter::ErrorReporter;
use skia_rust_sksl::position::Position;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};
use skia_rust_sksl::util::ShaderCapsFactory;

fn convert(compiler: &mut Compiler, kind: ProgramKind, src: &str) -> skia_rust_sksl::ir::Program {
    let settings = ProgramSettings {
        force_no_rt_flip: true,
        ..ProgramSettings::default()
    };
    compiler
        .convert_program(kind, src.as_bytes(), settings)
        .expect("the program compiles")
}

#[test]
fn a_fragment_program_gets_a_stage_output_struct_and_an_entry_point() {
    let mut compiler = Compiler::new();
    let mut program = convert(
        &mut compiler,
        ProgramKind::Fragment,
        "void main() { sk_FragColor = half4(0); }",
    );
    let wgsl = to_wgsl(
        compiler.context_mut(),
        &mut program,
        ShaderCapsFactory::default_caps(),
        PrettyPrint::Yes,
        IncludeSyntheticCode::No,
        None,
    )
    .expect("the program converts");
    assert_eq!(
        wgsl,
        "diagnostic(off, derivative_uniformity);\n\
         diagnostic(off, chromium.unreachable_code);\n\
         enable f16;\n\
         struct FSOut {\n\
         \x20 @location(0) sk_FragColor: vec4<f16>,\n\
         };\n\
         fn _skslMain(_stageOut: ptr<function, FSOut>) {\n\
         \x20 {\n\
         \x20   (*_stageOut).sk_FragColor = vec4<f16>(0.0h);\n\
         \x20 }\n\
         }\n\
         @fragment fn main() -> FSOut {\n\
         \x20 var _stageOut: FSOut;\n\
         \x20 _skslMain(&_stageOut);\n\
         \x20 return _stageOut;\n\
         }\n"
    );
}

#[test]
fn without_pretty_printing_nothing_is_indented() {
    let mut compiler = Compiler::new();
    let mut program = convert(
        &mut compiler,
        ProgramKind::Fragment,
        "void main() { sk_FragColor = half4(1); }",
    );
    let wgsl = to_wgsl(
        compiler.context_mut(),
        &mut program,
        ShaderCapsFactory::default_caps(),
        PrettyPrint::No,
        IncludeSyntheticCode::No,
        None,
    )
    .expect("the program converts");
    assert!(!wgsl.contains("\n  "), "{wgsl}");
    assert!(wgsl.contains("@fragment fn main() -> FSOut {\n"), "{wgsl}");
}

#[test]
fn the_native_overload_prints_like_a_debug_build() {
    let mut compiler = Compiler::new();
    let mut program = convert(
        &mut compiler,
        ProgramKind::Fragment,
        "void main() { sk_FragColor = half4(0); }",
    );
    let wgsl = to_wgsl_native(
        compiler.context_mut(),
        &mut program,
        ShaderCapsFactory::default_caps(),
    )
    .expect("the program converts");
    assert_eq!(wgsl.contains("\n  @location(0)"), cfg!(debug_assertions));
}

#[test]
fn a_failing_validator_is_an_error_and_a_warning_is_noted() {
    fn reject(errors: &mut ErrorReporter, wgsl: &str, _warnings: &mut String) -> bool {
        errors.error(Position::default(), &format!("rejected {}", wgsl.len()));
        false
    }
    fn warn(_errors: &mut ErrorReporter, _wgsl: &str, warnings: &mut String) -> bool {
        warnings.push_str("careful");
        true
    }
    let src = "void main() { sk_FragColor = half4(0); }";
    for (validate, expect_ok) in [(reject as ValidateWgslProc, false), (warn, true)] {
        let mut compiler = Compiler::new();
        let mut program = convert(&mut compiler, ProgramKind::Fragment, src);
        let result = to_wgsl(
            compiler.context_mut(),
            &mut program,
            ShaderCapsFactory::default_caps(),
            PrettyPrint::Yes,
            IncludeSyntheticCode::No,
            Some(validate),
        );
        assert_eq!(result.is_some(), expect_ok);
        if let Some(wgsl) = result {
            assert!(
                wgsl.starts_with("/* Tint reported warnings. */\n\n"),
                "{wgsl}"
            );
        } else {
            assert_eq!(compiler.error_count(), 1);
        }
    }
}
