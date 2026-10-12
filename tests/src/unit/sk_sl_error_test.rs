// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLErrorTest.cpp (chrome/m156)

//! `SkSLErrorTest`: every `resources/sksl/errors` and `resources/sksl/runtime_errors` shader must
//! fail to compile, and the compiler's error text must contain each `/*%%* … *%%*/` expectation,
//! in order.
//!
//! Each `DEF_TEST` is one `def_test!`. It walks one directory for one extension, with one program
//! kind, and runs [`test_expect_fail`] on each file.
//!
//! Mapping notes:
//! - `SkOSFile::Iter` returns the directory entries in `readdir` order, which is unspecified.
//!   [`iterate_dir`] sorts them, so a failure is reported in the same order on every host.
//! - `GetResourcePath` on a missing directory yields no entries, so the C++ test would pass
//!   without running anything. [`skip_without_resources!`] skips instead, so the run is visible.

use crate::resources::{get_resource_as_data, resource_dir};
use crate::{Reporter, def_test, errorf, infof, skip_without_resources};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

/// `strstr` on a NUL-terminated view: the bytes up to the first NUL, as `c_str()` gives them.
fn c_str_bytes(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(nul) => &bytes[..nul],
        None => bytes,
    }
}

/// `bytes` with every CRLF pair replaced by LF (the line endings of Skia's repository).
fn lf_line_endings(bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.contains(&b'\r') {
        return bytes;
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter().copied().peekable();
    while let Some(b) = iter.next() {
        if b == b'\r' && iter.peek() == Some(&b'\n') {
            continue;
        }
        out.push(b);
    }
    out
}

/// The first position of `needle` in `haystack` (`strstr`/`std::string::find`).
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// `Basename` of a `/`-separated path (`SkOSPath::Basename`), as text for a message.
fn basename(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `get_expected_errors`: the lines between the first `/*%%*` and the next `*%%*/`, with the empty
/// lines dropped (`std::getline` keeps the text between newlines, and empty ones are popped).
// Port of: tests/SkSLErrorTest.cpp#L30-L55 (chrome/m156)
fn get_expected_errors(shader: &[u8]) -> Vec<Vec<u8>> {
    // Error expectations are embedded in the source with a special *%%* marker, like so:
    //
    //     /*%%*
    //     expected 'foo', but found 'bar'
    //     'baz' is not a valid identifier
    //     *%%*/
    //
    // Extract them from the shader text.
    const EXPECTED_ERRORS_START: &[u8] = b"/*%%*";
    const EXPECTED_ERRORS_END: &[u8] = b"*%%*/";
    let mut expected_errors = Vec::new();
    // The C++ works on `shaderString.c_str()`, which stops at the first NUL.
    let text = c_str_bytes(shader);
    if let Some(start) = find_bytes(text, EXPECTED_ERRORS_START) {
        let body = &text[start + EXPECTED_ERRORS_START.len()..];
        if let Some(end) = find_bytes(body, EXPECTED_ERRORS_END) {
            expected_errors.extend(
                body[..end]
                    .split(|&b| b == b'\n')
                    .filter(|line| !line.is_empty())
                    .map(<[u8]>::to_vec),
            );
        }
    }
    expected_errors
}

/// `check_expected_errors`: each expectation must appear in `reported_errors`, after the one before
/// it, and everything up to and including a match is consumed.
// Port of: tests/SkSLErrorTest.cpp#L57-L84 (chrome/m156)
fn check_expected_errors(
    r: &mut Reporter,
    test_file: &str,
    expected_errors: &[Vec<u8>],
    reported_errors: Vec<u8>,
) {
    // Verify that the SkSL compiler actually emitted the expected error messages.
    // The list of expectations isn't necessarily exhaustive, though.
    let original_errors = reported_errors.clone();
    let mut reported_errors = reported_errors;
    let mut report_original_errors = false;
    for expected_error in expected_errors {
        // If this error wasn't reported, trigger an error.
        match find_bytes(&reported_errors, c_str_bytes(expected_error)) {
            None => {
                errorf!(
                    r,
                    "{}: Expected an error that wasn't reported:\n{}\n",
                    basename(test_file),
                    String::from_utf8_lossy(expected_error)
                );
                report_original_errors = true;
            }
            Some(pos) => {
                // We found the error that we expected to have. Remove that error from our report,
                // and everything preceding it as well. This ensures that we don't match the same
                // error twice, and that errors are reported in the order we expect.
                reported_errors.drain(..pos + expected_error.len());
            }
        }
    }

    if report_original_errors {
        errorf!(
            r,
            "{}: The following errors were reported:\n{}\n",
            basename(test_file),
            String::from_utf8_lossy(&original_errors)
        );
    }
}

/// `test_expect_fail`: compiles `test_file` as `kind` and checks that it fails with the expected
/// errors.
// Port of: tests/SkSLErrorTest.cpp#L86-L145 (chrome/m156)
fn test_expect_fail(r: &mut Reporter, test_file: &str, kind: ProgramKind) {
    // In a size-optimized build, there are a handful of errors which report differently, or not at
    // all. Skip over those tests.
    const TESTS_TO_SKIP: &[&str] = &[
        // These are tests that have been deleted, but which may still show up (and fail) on tasks,
        // because the resources directory isn't properly cleaned up. (b/40044088)
        "sksl/errors/InvalidBackendBindingFlagsGL.sksl",
        "sksl/errors/InvalidThreadgroupRTS.rts",
        "sksl/errors/LastFragColorWithoutCaps.sksl",
        "sksl/errors/MeshFragmentWithShader.mfrag",
        "sksl/errors/MeshFragmentWithBlender.mfrag",
        "sksl/errors/MeshFragmentWithColorFilter.mfrag",
        "sksl/errors/StaticIfTest.sksl",
        "sksl/errors/StaticSwitchConditionalBreak.sksl",
        "sksl/errors/StaticSwitchTest.sksl",
        "sksl/errors/StaticSwitchWithConditionalBreak.sksl",
        "sksl/errors/StaticSwitchWithConditionalContinue.sksl",
        "sksl/errors/StaticSwitchWithConditionalReturn.sksl",
        "sksl/errors/ComputeUniform.compute",
        "sksl/errors/DuplicateBinding.compute",
        "sksl/errors/InvalidThreadgroupCompute.compute",
        "sksl/errors/UnspecifiedBinding.compute",
        "sksl/runtime_errors/ReservedNameISampler2D.rts",
        "sksl/runtime_errors/ProgramTooLarge_BlocklessLoops.rts",
        "sksl/runtime_errors/ProgramTooLarge_Extreme.rts",
        "sksl/runtime_errors/ProgramTooLarge_FlatLoop.rts",
        "sksl/runtime_errors/ProgramTooLarge_Functions.rts",
        "sksl/runtime_errors/ProgramTooLarge_NestedLoops.rts",
        "sksl/runtime_errors/ProgramTooLarge_SplitLoops.rts",
        // `#ifdef SK_ENABLE_OPTIMIZE_SIZE` entries: skipped there, but this port builds the
        // default (non-size-optimized) configuration, so they are not listed.
    ];
    if TESTS_TO_SKIP.contains(&test_file) {
        infof!(r, "{}: skipped in SK_ENABLE_OPTIMIZE_SIZE mode", test_file);
        return;
    }

    let Some(shader) = get_resource_as_data(test_file) else {
        errorf!(r, "{}: Unable to load file", basename(test_file));
        return;
    };
    // Skia's tree stores these files with LF endings; a Windows checkout with `core.autocrlf`
    // turns them into CRLF, which would leave a `\r` on every expected line. Read them as stored.
    let shader = lf_line_endings(shader);

    let expected_errors = get_expected_errors(&shader);

    // Compile the code.
    let mut compiler = Compiler::new();
    let settings = ProgramSettings::default();
    let program = compiler.convert_program(kind, &shader, settings);

    // If the code actually generated a working program, we've already failed.
    if program.is_some() {
        errorf!(
            r,
            "{}: Expected failure, but compiled successfully",
            basename(test_file)
        );
        return;
    }

    check_expected_errors(
        r,
        test_file,
        &expected_errors,
        compiler.error_text_bytes(true),
    );
}

/// `iterate_dir`: runs `run` on each file of `directory` (a `resources` path ending in `/`) whose
/// name ends with `extension`, with the path joined to the name.
///
/// The entries are sorted by name (see the module notes).
// Port of: tests/SkSLErrorTest.cpp#L147-L158 (chrome/m156), with SkOSFile::Iter's suffix match.
fn iterate_dir(directory: &str, extension: &str, mut run: impl FnMut(&str)) {
    let Some(mut dir) = resource_dir() else {
        return;
    };
    for component in directory.split('/').filter(|c| !c.is_empty()) {
        dir.push(component);
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(extension))
        .collect();
    names.sort();
    for name in names {
        let path = format!("{}/{}", directory.trim_end_matches('/'), name);
        run(&path);
    }
}

def_test!(SkSLErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".sksl", |path| {
        test_expect_fail(r, path, ProgramKind::Fragment);
    });
});

def_test!(SkSLComputeErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".compute", |path| {
        test_expect_fail(r, path, ProgramKind::Compute);
    });
});

def_test!(SkSLMeshVertexErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".mvert", |path| {
        test_expect_fail(r, path, ProgramKind::MeshVertex);
    });
});

def_test!(SkSLMeshFragmentErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".mfrag", |path| {
        test_expect_fail(r, path, ProgramKind::MeshFragment);
    });
});

def_test!(SkSLRuntimeShaderErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".rts", |path| {
        test_expect_fail(r, path, ProgramKind::RuntimeShader);
    });
    iterate_dir("sksl/runtime_errors/", ".rts", |path| {
        test_expect_fail(r, path, ProgramKind::RuntimeShader);
    });
});

def_test!(SkSLPrivateRuntimeShaderErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/errors/", ".privrts", |path| {
        test_expect_fail(r, path, ProgramKind::PrivateRuntimeShader);
    });
    iterate_dir("sksl/runtime_errors/", ".privrts", |path| {
        test_expect_fail(r, path, ProgramKind::PrivateRuntimeShader);
    });
});

def_test!(SkSLRuntimeColorFilterErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/runtime_errors/", ".rtcf", |path| {
        test_expect_fail(r, path, ProgramKind::RuntimeColorFilter);
    });
});

def_test!(SkSLRuntimeBlenderErrorTest, |r| {
    skip_without_resources!();
    iterate_dir("sksl/runtime_errors/", ".rtb", |path| {
        test_expect_fail(r, path, ProgramKind::RuntimeBlender);
    });
});
