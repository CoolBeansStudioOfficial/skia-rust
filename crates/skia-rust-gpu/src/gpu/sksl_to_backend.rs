// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/SkSLToBackend.{h,cpp} (chrome/m156), WGSL only

//! [`sksl_to_backend`]: the wrapper around the `SkSL` compiler with useful logging and error
//! handling. The text output is the only one ported (WGSL, [`sksl_to_wgsl`]): the SPIR-V and MSL
//! generators are not part of the port.
//!
//! The port follows Skia's non-`SK_DEBUG` build, the build the goldens come from: the source is
//! not pretty-printed before it is compiled and the WGSL is not indented. The `SK_PRINT_*`
//! debugging switches are not ported.

use skia_rust_sksl::codegen::wgsl::{IncludeSyntheticCode, PrettyPrint, to_wgsl};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::context::Context;
use skia_rust_sksl::ir::{Program, ProgramInterface};
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};
use skia_rust_sksl::util::ShaderCaps;

use crate::gpu::shader_error_handler::ShaderErrorHandler;

/// The `toBackend` function pointer of `SkSLToBackend`: generates the native shader text of a
/// program, reporting errors to the context.
// Port of: src/gpu/SkSLToBackend.h#L33-L34 (chrome/m156)
pub type ToBackend = fn(&mut Context, &mut Program, &ShaderCaps) -> Option<String>;

/// `ToWGSL(program, caps, NativeShader*)` as a non-`SK_DEBUG` build runs it: no pretty printing,
/// no synthetic code and no validator.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4967-L4985 (chrome/m156)
fn to_wgsl_release(
    context: &mut Context,
    program: &mut Program,
    caps: &ShaderCaps,
) -> Option<String> {
    to_wgsl(
        context,
        program,
        caps,
        PrettyPrint::No,
        IncludeSyntheticCode::No,
        None,
    )
}

/// `SkSLToBackend(caps, toBackend, backendLabel, sksl, programKind, settings, output, outInterface,
/// errorHandler)`. Returns the native shader text, or `None` after reporting the compile errors to
/// the handler.
// Port of: src/gpu/SkSLToBackend.cpp#L22-L84 (chrome/m156)
#[doc(alias = "SkSLToBackend")]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
pub fn sksl_to_backend(
    caps: &ShaderCaps,
    to_backend: ToBackend,
    _backend_label: Option<&str>,
    sksl: &str,
    program_kind: ProgramKind,
    settings: ProgramSettings,
    out_interface: Option<&mut ProgramInterface>,
    error_handler: &dyn ShaderErrorHandler,
) -> Option<String> {
    let mut compiler = Compiler::new();
    let program = compiler.convert_program(program_kind, sksl.as_bytes(), settings);
    let output = program.and_then(|mut program| {
        let output = to_backend(compiler.context_mut(), &mut program, caps)?;
        if let Some(out_interface) = out_interface {
            *out_interface = program.interface;
        }
        Some(output)
    });
    if output.is_none() {
        let errors = compiler.error_text_bytes(true);
        error_handler.compile_error(sksl, &String::from_utf8_lossy(&errors), false);
    }
    output
}

/// `SkSLToBackend` with the WGSL generator, as the Dawn pipelines call it
/// (`DawnCompileWGSLShaderModule`).
// Port of: src/gpu/SkSLToBackend.cpp#L22-L84 (chrome/m156)
#[doc(alias = "SkSLToWGSL")]
pub fn sksl_to_wgsl(
    caps: &ShaderCaps,
    sksl: &str,
    program_kind: ProgramKind,
    settings: ProgramSettings,
    out_interface: Option<&mut ProgramInterface>,
    error_handler: &dyn ShaderErrorHandler,
) -> Option<String> {
    sksl_to_backend(
        caps,
        to_wgsl_release,
        Some("WGSL"),
        sksl,
        program_kind,
        settings,
        out_interface,
        error_handler,
    )
}
