// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/ShaderErrorHandler.h, src/gpu/ShaderErrorHandler.cpp (chrome/m156)

//! [`ShaderErrorHandler`]: reports errors when compiling shaders.

use std::fmt::Write as _;

/// Abstract class to report errors when compiling shaders.
// Port of: include/gpu/ShaderErrorHandler.h#L17-L35 (chrome/m156)
#[doc(alias = "skgpu::ShaderErrorHandler")]
pub trait ShaderErrorHandler: Send + Sync {
    /// `compileError(shader, errors, shaderWasCached)`. (Skia's two-argument overload, kept for
    /// older clients, is what the default three-argument one forwards to; here there is one.)
    fn compile_error(&self, shader: &str, errors: &str, shader_was_cached: bool);
}

/// `SkShaderUtils::BuildShaderErrorMessage(shader, errors)`.
// Port of: src/utils/SkShaderUtils.cpp#L277-L285 (chrome/m156)
#[doc(alias = "BuildShaderErrorMessage")]
#[must_use]
pub fn build_shader_error_message(shader: &str, errors: &str) -> String {
    let mut abort_text = String::from("Shader compilation error\n------------------------\n");
    // `VisitLineByLine`: `SkStrSplit` in strict mode keeps empty lines.
    for (i, line) in shader.split('\n').enumerate() {
        let _ = writeln!(abort_text, "{:4}\t{line}", i + 1);
    }
    let _ = write!(abort_text, "Errors:\n{errors}");
    abort_text
}

/// The handler used when none is set: it prints the failure and asserts in debug builds.
// Port of: src/gpu/ShaderErrorHandler.cpp#L16-L28 (chrome/m156)
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultShaderErrorHandler;

impl ShaderErrorHandler for DefaultShaderErrorHandler {
    fn compile_error(&self, shader: &str, errors: &str, _shader_was_cached: bool) {
        let message = build_shader_error_message(shader, errors);
        for line in message.split('\n') {
            eprintln!("{line}");
        }
        debug_assert!(false, "Shader compilation failed!\n\n{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::build_shader_error_message;

    #[test]
    fn the_message_numbers_the_source_lines() {
        assert_eq!(
            build_shader_error_message("a\n\nb", "oops"),
            "Shader compilation error\n------------------------\n   1\ta\n   2\t\n   3\tb\nErrors:\noops"
        );
    }
}
