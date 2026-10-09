// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Compile tests of the `SkSL` front end (tasks S10 and S11): each case is an input from
//! `resources/sksl/errors` and the golden from `tests/sksl/errors` that `skslc` wrote for it.
//! The goldens list the exact error text, so the parser, the IR conversions, the finalization
//! checks and the error formatting are compared byte for byte, through the library's
//! [`Compiler::convert_program`] and [`ModuleLoader`] (the compiler driver, `docs/design/sksl.md`
//! §4.6–§4.7).
//!
//! The four goldens whose errors come from the inliner (`*InlinedIndexOutOfRange`,
//! `OverflowInlinedLiteral`) are compared like the rest.

use std::fs;
use std::path::{Path, PathBuf};

use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::flavor::Flavor;
use skia_rust_sksl::module_loader::ModuleLoader;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

/// The settings `skslc` gives every program so the RT-flip checks pass.
fn settings() -> ProgramSettings {
    ProgramSettings {
        rt_flip_offset: 16384,
        rt_flip_set: 0,
        rt_flip_binding: 0,
        ..ProgramSettings::default()
    }
}

/// The result of compiling a program: whether it compiled, and the compiler's error text.
struct Compiled {
    accepted: bool,
    error_text: Vec<u8>,
}

/// `skslc`'s compile of one input, as the standalone build (original module sources).
fn compile_program(kind: ProgramKind, text: &[u8]) -> Compiled {
    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    let accepted = compiler.convert_program(kind, text, settings()).is_some();
    Compiled {
        accepted,
        error_text: compiler.error_text_bytes(true),
    }
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/errors")
}

/// The program kind `skslc` picks from the input's extension.
fn kind_for_extension(ext: &str) -> ProgramKind {
    match ext {
        "vert" => ProgramKind::Vertex,
        "frag" | "sksl" => ProgramKind::Fragment,
        "mvert" => ProgramKind::MeshVertex,
        "mfrag" => ProgramKind::MeshFragment,
        "compute" => ProgramKind::Compute,
        "rtb" => ProgramKind::RuntimeBlender,
        "rtcf" => ProgramKind::RuntimeColorFilter,
        "rts" => ProgramKind::RuntimeShader,
        other => panic!("unknown extension {other}"),
    }
}

/// Goldens whose error comes from a GLSL code generator, not the front end: `samplerExternalOES`
/// needs `ShaderCaps::fExternalTextureSupport`, which only `SkSLGLSLCodeGenerator.cpp#L1612`
/// checks. The GLSL generator is out of scope (`docs/design/sksl.md` R7), so the compiler must
/// accept these inputs, and their golden is `excluded` in the manifest.
const GLSL_GENERATOR_ERRORS: &[&str] = &["SamplerExternalOES"];

#[test]
fn builtin_modules_load() {
    // Every module of both flavours compiles from its embedded text.
    for flavor in [Flavor::Library, Flavor::Standalone] {
        let loader = ModuleLoader::for_flavor(flavor);
        let _ = (
            loader.root(),
            loader.shared(),
            loader.gpu(),
            loader.fragment(),
            loader.vertex(),
            loader.compute(),
            loader.public(),
            loader.private_rt_shader(),
        );
    }
}

#[test]
fn error_goldens_match() {
    let dir = data_dir();
    let mut cases: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.extension().is_some_and(|ext| ext == "glsl") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("file stem")
            .to_string_lossy()
            .into_owned();
        // An input without a golden is a test that Skia's `dm` runs but `skslc` does not.
        if path.with_extension("glsl").exists() {
            cases.push((name, path));
        }
    }
    cases.sort();
    assert!(cases.len() > 100, "only {} cases", cases.len());
    // `PARSER_CASE=Name cargo test ...` runs one case.
    if let Ok(only) = std::env::var("PARSER_CASE") {
        cases.retain(|(name, _)| *name == only);
    }

    let mut failures = Vec::new();
    for (name, path) in &cases {
        let ext = path.extension().expect("extension").to_string_lossy();
        // Skia's strings are bytes: a fuzzer's input and its golden need not be UTF-8.
        let text = fs::read(path).expect("input");
        let golden = fs::read(path.with_extension("glsl")).expect("golden");
        let expected = golden
            .strip_prefix(b"### Compilation failed:\n\n")
            .unwrap_or_else(|| panic!("{name}: not an error golden"));
        let kind = kind_for_extension(&ext);
        let Ok(compiled) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_program(kind, &text)
        })) else {
            failures.push(format!("{name}: the compiler panicked"));
            continue;
        };
        if GLSL_GENERATOR_ERRORS.contains(&name.as_str()) {
            if !compiled.accepted {
                failures.push(format!(
                    "{name}: deferred to a later phase, but the compiler reported:\n{}",
                    String::from_utf8_lossy(&compiled.error_text)
                ));
            }
        } else if compiled.error_text != expected {
            failures.push(format!(
                "{name}: error text differs\n--- expected\n{}--- got\n{}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(&compiled.error_text)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// The position of `needle` in `haystack`, as bytes.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The messages `SkSLErrorTest` expects, from the `/*%%* ... *%%*/` comment of a test input.
fn expected_messages(text: &str) -> Vec<String> {
    let start = text.find("/*%%*").expect("an expectation comment") + "/*%%*".len();
    let end = text
        .find("*%%*/")
        .expect("the end of the expectation comment");
    text[start..end]
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `check_expected_errors` of `tests/SkSLErrorTest.cpp`: every expected message must appear in the
/// reported bytes, in order. The list is not necessarily exhaustive.
fn missing_expected_errors(expected: &[String], reported: &[u8]) -> Vec<String> {
    let mut remaining = reported;
    let mut missing = Vec::new();
    for message in expected {
        match find_bytes(remaining, message.as_bytes()) {
            Some(pos) => remaining = &remaining[pos + message.len()..],
            None => missing.push(message.clone()),
        }
    }
    missing
}

/// The error tests of `resources/sksl/runtime_errors` (`SkSLRuntimeShaderErrorTest` and its
/// siblings): each file lists messages that the compiler must report, in order. Every expected
/// message must be reported.
#[test]
fn runtime_error_expectations_match() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/runtime_errors");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    files.sort();
    assert!(files.len() > 30, "only {} files", files.len());
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let ext = path.extension().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(path).expect("input");
        let expected = expected_messages(&text);
        let compiled = compile_program(kind_for_extension(&ext), text.as_bytes());
        let missing = missing_expected_errors(&expected, &compiled.error_text);
        if !missing.is_empty() {
            failures.push(format!(
                "{name}: missing {missing:?}\n{}",
                String::from_utf8_lossy(&compiled.error_text)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} files failed:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}
