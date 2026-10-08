// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/skslc/Main.cpp (the parts the in-scope `tests/sksl` goldens use).

//! `skslc` as a library: the command-line behaviour that decides what a golden is compiled as.
//!
//! The program kind comes from the input's extension, the output format from the output's, and
//! `/*#pragma settings …*/` in the input selects the shader caps and the program settings. The
//! front end runs (`skia_rust_sksl::compiler::Compiler::convert_program`, `docs/design/sksl.md`
//! §4.7), so a program that fails to compile yields the golden's error text. A program that
//! compiles needs a code generator, which is not ported, so [`skslc`] returns
//! [`SkslcError::NotPorted`] for it; the golden harness reports those goldens as ignored.

// Port of: tools/skslc/Main.cpp#L349-L490 (chrome/m156), for `detect_shader_settings`.

use std::fmt;

use skia_rust_sksl::codegen::pipeline_stage::{
    Callbacks as PipelineStageCallbacks, convert_program as convert_pipeline_stage,
};
use skia_rust_sksl::codegen::rp::make_raster_pipeline_program;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::defines::DEFAULT_INLINE_THRESHOLD;
use skia_rust_sksl::flavor::Flavor;
use skia_rust_sksl::ir::{IrPool, Program, VarDeclaration};
use skia_rust_sksl::position::Position;
use skia_rust_sksl::program_settings::{ProgramKind as CompilerKind, ProgramSettings, Version};
use skia_rust_sksl::shader_utils::pretty_print;
use skia_rust_sksl::tracing::DebugTracePriv;

/// `SkSL::ProgramKind`, for the kinds `skslc` accepts (`Main.cpp#L546-L566`).
#[doc(alias = "SkSL::ProgramKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramKind {
    Vertex,
    Fragment,
    MeshVertex,
    MeshFragment,
    Compute,
    RuntimeBlender,
    RuntimeColorFilter,
    RuntimeShader,
    PrivateRuntimeShader,
}

/// The program kind of an input, by its extension (`Main.cpp#L546-L566`). `None` when the name
/// has no known suffix.
#[must_use]
// Skia compares extensions case-sensitively (`skstd::ends_with`), and so does skslc.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn program_kind_for_input(path: &str) -> Option<ProgramKind> {
    Some(if path.ends_with(".vert") {
        ProgramKind::Vertex
    } else if path.ends_with(".frag") || path.ends_with(".sksl") {
        ProgramKind::Fragment
    } else if path.ends_with(".mvert") {
        ProgramKind::MeshVertex
    } else if path.ends_with(".mfrag") {
        ProgramKind::MeshFragment
    } else if path.ends_with(".compute") {
        ProgramKind::Compute
    } else if path.ends_with(".rtb") {
        ProgramKind::RuntimeBlender
    } else if path.ends_with(".rtcf") {
        ProgramKind::RuntimeColorFilter
    } else if path.ends_with(".rts") {
        ProgramKind::RuntimeShader
    } else if path.ends_with(".privrts") {
        ProgramKind::PrivateRuntimeShader
    } else {
        return None;
    })
}

/// The output formats `skslc` writes, by the output's suffix (`Main.cpp#L640-L800`). The GLSL,
/// Metal, HLSL and SPIR-V formats are listed because their goldens exist; they are out of scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Spirv,
    /// `.asm.frag`, `.asm.vert` or `.asm.comp`.
    SpirvAssembly,
    Glsl,
    Metal,
    Hlsl,
    Wgsl,
    /// `.skrp`: the Raster Pipeline dump.
    Skrp,
    /// `.stage`: the pipeline-stage text.
    Stage,
}

/// The format of an output, by its suffix. `None` when the name has no known suffix.
#[must_use]
// Skia compares extensions case-sensitively (`skstd::ends_with`), and so does skslc.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn output_format_for(path: &str) -> Option<OutputFormat> {
    Some(if path.ends_with(".spirv") {
        OutputFormat::Spirv
    } else if path.ends_with(".asm.frag")
        || path.ends_with(".asm.vert")
        || path.ends_with(".asm.comp")
    {
        OutputFormat::SpirvAssembly
    } else if path.ends_with(".glsl") {
        OutputFormat::Glsl
    } else if path.ends_with(".metal") {
        OutputFormat::Metal
    } else if path.ends_with(".hlsl") {
        OutputFormat::Hlsl
    } else if path.ends_with(".wgsl") {
        OutputFormat::Wgsl
    } else if path.ends_with(".skrp") {
        OutputFormat::Skrp
    } else if path.ends_with(".stage") {
        OutputFormat::Stage
    } else {
        return None;
    })
}

/// The `ProgramSettings` fields and shader caps that `/*#pragma settings …*/` can set.
#[derive(Clone, Debug, PartialEq, Eq)]
// The pragma words are independent switches, as in Skia's ProgramSettings.
#[allow(clippy::struct_excessive_bools)]
pub struct PragmaSettings {
    /// The `ShaderCapsFactory` function the pragma named (`Factory::Default()`, `Version110()`,
    /// …), by its name. `None` keeps `ShaderCapsFactory::Standalone()`. When several are named,
    /// the last one applied wins, as in Skia.
    pub caps: Option<&'static str>,
    /// `fAllowNarrowingConversions`.
    pub allow_narrowing_conversions: bool,
    /// `fForceHighPrecision`.
    pub force_high_precision: bool,
    /// `fInlineThreshold`: `None` keeps the default, `Some(0)` after `NoInline` or `NoOptimize`,
    /// and `Some(i32::MAX)` after `InlineThresholdMax`.
    pub inline_threshold: Option<i32>,
    /// `fOptimize`: false after `NoOptimize` or `DebugTrace`.
    pub optimize: bool,
    /// `fForceNoRTFlip`.
    pub force_no_rt_flip: bool,
    /// `fSharpenTextures`.
    pub sharpen_textures: bool,
    /// Whether `DebugTrace` asked for a debug trace (`std::make_unique<DebugTracePriv>`).
    pub debug_trace: bool,
}

impl Default for PragmaSettings {
    fn default() -> Self {
        Self {
            caps: None,
            allow_narrowing_conversions: false,
            force_high_precision: false,
            inline_threshold: None,
            optimize: true,
            force_no_rt_flip: false,
            sharpen_textures: false,
            debug_trace: false,
        }
    }
}

impl PragmaSettings {
    /// The `ProgramSettings` these pragmas give, before `skslc` sets the rt-flip fields. A
    /// `NoInline` or `NoOptimize` pragma sets the threshold to zero; without one the default
    /// threshold stays.
    #[must_use]
    pub fn program_settings(&self) -> ProgramSettings {
        ProgramSettings {
            allow_narrowing_conversions: self.allow_narrowing_conversions,
            force_high_precision: self.force_high_precision,
            sharpen_textures: self.sharpen_textures,
            force_no_rt_flip: self.force_no_rt_flip,
            optimize: self.optimize,
            inline_threshold: self.inline_threshold.unwrap_or(DEFAULT_INLINE_THRESHOLD),
            ..ProgramSettings::default()
        }
    }
}

/// The first position of `needle` in `haystack`.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One suffix the pragma understands, and what it sets.
#[derive(Clone, Copy)]
enum Rule {
    Caps(&'static str),
    AllowNarrowingConversions,
    ForceHighPrecision,
    NoInline,
    NoOptimize,
    NoRtFlip,
    InlineThresholdMax,
    Sharpen,
    DebugTrace,
}

/// The suffixes, in the order `detect_shader_settings` checks them within one pass.
const RULES: &[(&str, Rule)] = &[
    (
        " AddAndTrueToLoopCondition",
        Rule::Caps("AddAndTrueToLoopCondition"),
    ),
    (
        " CannotUseFractForNegativeValues",
        Rule::Caps("CannotUseFractForNegativeValues"),
    ),
    (" CannotUseFragCoord", Rule::Caps("CannotUseFragCoord")),
    (
        " CannotUseMinAndAbsTogether",
        Rule::Caps("CannotUseMinAndAbsTogether"),
    ),
    (
        " CannotUseVoidInSequenceExpressions",
        Rule::Caps("CannotUseVoidInSequenceExpressions"),
    ),
    (" DualSourceBlending", Rule::Caps("DualSourceBlending")),
    (" Default", Rule::Caps("Default")),
    (
        " EmulateAbsIntFunction",
        Rule::Caps("EmulateAbsIntFunction"),
    ),
    (
        " FramebufferFetchSupport",
        Rule::Caps("FramebufferFetchSupport"),
    ),
    (
        " MustGuardDivisionEvenAfterExplicitZeroCheck",
        Rule::Caps("MustGuardDivisionEvenAfterExplicitZeroCheck"),
    ),
    (
        " MustDeclareFragmentFrontFacing",
        Rule::Caps("MustDeclareFragmentFrontFacing"),
    ),
    (
        " MustForceNegatedAtanParamToFloat",
        Rule::Caps("MustForceNegatedAtanParamToFloat"),
    ),
    (
        " MustForceNegatedLdexpParamToMultiply",
        Rule::Caps("MustForceNegatedLdexpParamToMultiply"),
    ),
    (
        " NoBuiltinDeterminantSupport",
        Rule::Caps("NoBuiltinDeterminantSupport"),
    ),
    (" NoBuiltinFMASupport", Rule::Caps("NoBuiltinFMASupport")),
    (
        " NoExternalTextureSupport",
        Rule::Caps("NoExternalTextureSupport"),
    ),
    (
        " RemovePowWithConstantExponent",
        Rule::Caps("RemovePowWithConstantExponent"),
    ),
    (" RewriteDoWhileLoops", Rule::Caps("RewriteDoWhileLoops")),
    (
        " RewriteSwitchStatements",
        Rule::Caps("RewriteSwitchStatements"),
    ),
    (
        " RewriteMatrixVectorMultiply",
        Rule::Caps("RewriteMatrixVectorMultiply"),
    ),
    (
        " RewriteMatrixComparisons",
        Rule::Caps("RewriteMatrixComparisons"),
    ),
    (
        " ShaderDerivativeExtensionString",
        Rule::Caps("ShaderDerivativeExtensionString"),
    ),
    (
        " UnfoldShortCircuitAsTernary",
        Rule::Caps("UnfoldShortCircuitAsTernary"),
    ),
    (
        " UsesPrecisionModifiers",
        Rule::Caps("UsesPrecisionModifiers"),
    ),
    (" Version110", Rule::Caps("Version110")),
    (" Version450Core", Rule::Caps("Version450Core")),
    (
        " AllowNarrowingConversions",
        Rule::AllowNarrowingConversions,
    ),
    (" ForceHighPrecision", Rule::ForceHighPrecision),
    (" NoInline", Rule::NoInline),
    (" NoOptimize", Rule::NoOptimize),
    (" NoRTFlip", Rule::NoRtFlip),
    (" InlineThresholdMax", Rule::InlineThresholdMax),
    (" Sharpen", Rule::Sharpen),
    (" DebugTrace", Rule::DebugTrace),
];

/// `detect_shader_settings` found a setting it does not know. Carries the unread text, as the
/// `Unrecognized #pragma settings` message does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnrecognizedSetting(pub String);

impl fmt::Display for UnrecognizedSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unrecognized #pragma settings: {}", self.0)
    }
}

/// `detect_shader_settings`: reads the first `/*#pragma settings …*/` comment in `text`.
///
/// Settings may come in any order, so the suffixes are consumed from the end of the list,
/// repeating passes until nothing is left. An input without the pragma has the defaults.
///
/// # Errors
///
/// Returns [`UnrecognizedSetting`] when a word in the pragma matches no known setting.
// Port of: tools/skslc/Main.cpp#L349-L490 (chrome/m156)
pub fn detect_shader_settings(text: &[u8]) -> Result<PragmaSettings, UnrecognizedSetting> {
    const PRAGMA: &[u8] = b"/*#pragma settings ";
    let mut settings = PragmaSettings::default();
    let Some(start) = find_bytes(text, PRAGMA) else {
        return Ok(settings);
    };
    // Keep the space before the first item: each item is matched as a suffix that includes it.
    let body_start = start + PRAGMA.len() - 1;
    let Some(body_len) = find_bytes(&text[body_start..], b"*/") else {
        return Ok(settings);
    };
    let mut body = &text[body_start..body_start + body_len];
    loop {
        let starting_length = body.len();
        for &(suffix, rule) in RULES {
            if let Some(rest) = body.strip_suffix(suffix.as_bytes()) {
                body = rest;
                apply(rule, &mut settings);
            }
        }
        if body.is_empty() {
            break;
        }
        if body.len() == starting_length {
            return Err(UnrecognizedSetting(
                String::from_utf8_lossy(body).into_owned(),
            ));
        }
    }
    Ok(settings)
}

fn apply(rule: Rule, settings: &mut PragmaSettings) {
    match rule {
        Rule::Caps(name) => settings.caps = Some(name),
        Rule::AllowNarrowingConversions => settings.allow_narrowing_conversions = true,
        Rule::ForceHighPrecision => settings.force_high_precision = true,
        Rule::NoInline => settings.inline_threshold = Some(0),
        Rule::NoOptimize => {
            settings.optimize = false;
            settings.inline_threshold = Some(0);
        }
        Rule::NoRtFlip => settings.force_no_rt_flip = true,
        Rule::InlineThresholdMax => settings.inline_threshold = Some(i32::MAX),
        Rule::Sharpen => settings.sharpen_textures = true,
        Rule::DebugTrace => {
            settings.optimize = false;
            settings.debug_trace = true;
        }
    }
}

/// Why [`skslc`] did not produce output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkslcError {
    /// The input name ends in no known program-kind suffix (`kInputError`).
    Input(String),
    /// The output name ends in no known format suffix (`kConfigurationError`).
    Configuration(String),
    /// The pragma names a setting this port does not know.
    Pragma(UnrecognizedSetting),
    /// The compiler stage that produces this output is not ported yet (`docs/design/sksl.md`
    /// §10, tasks S5–S11 for the front end, S13–S17 for the Raster Pipeline output).
    NotPorted(&'static str),
}

impl fmt::Display for SkslcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(path) => write!(
                f,
                "input filename must end in '.vert', '.frag', '.mvert', '.mfrag', '.compute', \
                 '.rtb', '.rtcf', '.rts', '.privrts', or '.sksl' (got '{path}')"
            ),
            Self::Configuration(path) => write!(
                f,
                "expected output path to end with one of: .glsl, .html, .metal, .hlsl, .wgsl, \
                 .spirv, .asm.vert, .asm.frag, .asm.comp, .skrp, .stage (got '{path}')"
            ),
            Self::Pragma(e) => e.fmt(f),
            Self::NotPorted(what) => write!(f, "not ported yet: {what}"),
        }
    }
}

impl std::error::Error for SkslcError {}

/// `skslc <input> <output> [--settings|--nosettings]` as a function: the same checks and the same
/// settings detection as the tool, on the input's `text`. `honor_settings` is `--settings` (the
/// default) or `--nosettings`.
///
/// When the compiler fails, `skslc` writes this header and the compiler's error text.
// Port of: tools/skslc/Main.cpp#L593-L599 (chrome/m156), `emitCompileError`.
const COMPILE_FAILED_HEADER: &[u8] = b"### Compilation failed:\n\n";

/// `skslc <input> <output> [--settings|--nosettings]` as a function: the same checks, the same
/// settings detection and the same front-end run as the tool, on the input's `text` (bytes, since
/// a golden's input need not be UTF-8). `honor_settings` is `--settings` (the default) or
/// `--nosettings`.
///
/// When the program does not compile, the result holds the bytes `skslc` writes to `output`: the
/// `### Compilation failed:` header and the compiler's error text (`Main.cpp`'s
/// `emitCompileError`). When the program compiles, the output needs a code generator, which is
/// not ported yet, so the call returns [`SkslcError::NotPorted`].
///
/// # Errors
///
/// Returns the reason for every failure `skslc` reports, in the order it checks them.
// Port of: tools/skslc/Main.cpp#L535-L735 (chrome/m156), for the front end and the error path.
// Skia compares extensions case-sensitively (`skstd::ends_with`), and so does skslc.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn skslc(
    input: &str,
    text: &[u8],
    output: &str,
    honor_settings: bool,
) -> Result<Vec<u8>, SkslcError> {
    let kind = program_kind_for_input(input).ok_or_else(|| SkslcError::Input(input.to_owned()))?;
    let format =
        output_format_for(output).ok_or_else(|| SkslcError::Configuration(output.to_owned()))?;
    let pragma = if honor_settings {
        detect_shader_settings(text).map_err(SkslcError::Pragma)?
    } else {
        PragmaSettings::default()
    };

    let mut settings = pragma.program_settings();
    // This tells the compiler where the rt-flip uniform will live should it be required. For
    // testing purposes we don't care where that is, but the compiler will report an error if we
    // leave them at their default invalid values, or if the offset overlaps another uniform.
    settings.rt_flip_offset = 16384;
    settings.rt_flip_set = 0;
    settings.rt_flip_binding = 0;
    match format {
        OutputFormat::Wgsl => settings.force_no_rt_flip = true,
        OutputFormat::Skrp => settings.max_version_allowed = Version::K300,
        _ => {}
    }

    let mut kind = compiler_kind(kind);
    if format == OutputFormat::Skrp {
        // `compileProgramAsRuntimeShader`: a runtime shader cannot be a vertex program, and a
        // `.sksl` or `.frag` input is compiled as a private runtime shader.
        if kind == CompilerKind::Vertex {
            return Ok(compile_failed(
                "Runtime shaders do not support vertex programs\n",
            ));
        }
        if kind == CompilerKind::Fragment {
            kind = CompilerKind::PrivateRuntimeShader;
        }
    }

    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    if let Some(mut program) = compiler.convert_program(kind, text, settings) {
        if format == OutputFormat::Skrp {
            if let Some(output) = write_skrp(&mut compiler, &mut program, pragma.debug_trace) {
                return Ok(output);
            }
        } else if format == OutputFormat::Stage {
            return Ok(write_stage(&program));
        } else {
            // The program compiled. The output itself is written by a code generator, which is
            // not ported (the WGSL and GLSL back ends).
            return Err(SkslcError::NotPorted(
                "the code generator for this output format (docs/design/sksl.md S26)",
            ));
        }
    }
    let mut output_bytes = COMPILE_FAILED_HEADER.to_vec();
    output_bytes.extend_from_slice(&compiler.error_text_bytes(true));
    Ok(output_bytes)
}

/// The `.skrp` writer of `skslc`: generates the Raster Pipeline program of `main` and dumps it
/// with the instruction counts. `None` when it reports an error (the caller then writes the
/// compiler's error text).
// Port of: tools/skslc/Main.cpp#L707-L727 (chrome/m156)
fn write_skrp(
    compiler: &mut Compiler,
    program: &mut Program,
    want_trace_ops: bool,
) -> Option<Vec<u8>> {
    let main = program
        .get_function("main")
        .and_then(|f| program.pool.function(f).definition);
    let Some(main) = main else {
        compiler
            .error_reporter()
            .error(Position::default(), "code has no entrypoint");
        return None;
    };
    let Some(raster_prog) = make_raster_pipeline_program(
        program,
        main,
        Some(DebugTracePriv::default()),
        want_trace_ops,
    ) else {
        compiler
            .error_reporter()
            .error(Position::default(), "code is not supported");
        return None;
    };
    Some(raster_prog.dump_with(true, true).into_bytes())
}

/// The `.stage` writer of `skslc`: the pipeline-stage text of `program`, pretty-printed.
// Port of: tools/skslc/Main.cpp#L729-L790 (chrome/m156), the `.stage` branch.
fn write_stage(program: &Program) -> Vec<u8> {
    let mut callbacks = StageCallbacks::default();
    // The .stage output looks almost like valid SkSL, but not quite. Children are sampled by
    // index, not name, and the input color and coords keep their names as `_inColor`/`_coords`.
    convert_pipeline_stage(
        program,
        "_coords",
        "_inColor",
        "_canvasColor",
        &mut callbacks,
    );
    pretty_print(callbacks.output.as_bytes())
}

/// The callbacks of `skslc`'s `.stage` writer: the text is collected in `output`, and every child
/// is named `child_<index>`.
// Port of: tools/skslc/Main.cpp#L729-L790 (the `Callbacks` class of the `.stage` branch, chrome/m156).
#[derive(Debug, Default)]
pub struct StageCallbacks {
    /// The unformatted text (`fOutput`).
    pub output: String,
}

impl PipelineStageCallbacks for StageCallbacks {
    fn get_mangled_name(&mut self, name: &str) -> String {
        format!("{name}_0")
    }

    fn declare_uniform(&mut self, pool: &IrPool, decl: &VarDeclaration) -> String {
        self.output.push_str(&decl.description(pool));
        pool.variable(decl.var).name.to_string()
    }

    fn define_function(&mut self, declaration: &str, body: &str, _is_main: bool) {
        self.output.push_str(declaration);
        self.output.push('{');
        self.output.push_str(body);
        self.output.push('}');
    }

    fn declare_function(&mut self, declaration: &str) {
        self.output.push_str(declaration);
    }

    fn define_struct(&mut self, definition: &str) {
        self.output.push_str(definition);
    }

    fn declare_global(&mut self, declaration: &str) {
        self.output.push_str(declaration);
    }

    fn sample_shader(&mut self, index: i32, coords: &str) -> String {
        format!("child_{index}.eval({coords})")
    }

    fn sample_color_filter(&mut self, index: i32, color: &str) -> String {
        format!("child_{index}.eval({color})")
    }

    fn sample_blender(&mut self, index: i32, src: &str, dst: &str) -> String {
        format!("child_{index}.eval({src}, {dst})")
    }

    fn to_linear_srgb(&mut self, color: &str) -> String {
        format!("toLinearSrgb({color})")
    }

    fn from_linear_srgb(&mut self, color: &str) -> String {
        format!("fromLinearSrgb({color})")
    }
}

/// The bytes `skslc` writes when it fails with `error_text`.
fn compile_failed(error_text: &str) -> Vec<u8> {
    let mut bytes = COMPILE_FAILED_HEADER.to_vec();
    bytes.extend_from_slice(error_text.as_bytes());
    bytes
}

/// The `skia_rust_sksl` kind of a `skslc` input kind (`Main.cpp#L546-L566`).
fn compiler_kind(kind: ProgramKind) -> CompilerKind {
    match kind {
        ProgramKind::Vertex => CompilerKind::Vertex,
        ProgramKind::Fragment => CompilerKind::Fragment,
        ProgramKind::MeshVertex => CompilerKind::MeshVertex,
        ProgramKind::MeshFragment => CompilerKind::MeshFragment,
        ProgramKind::Compute => CompilerKind::Compute,
        ProgramKind::RuntimeBlender => CompilerKind::RuntimeBlender,
        ProgramKind::RuntimeColorFilter => CompilerKind::RuntimeColorFilter,
        ProgramKind::RuntimeShader => CompilerKind::RuntimeShader,
        ProgramKind::PrivateRuntimeShader => CompilerKind::PrivateRuntimeShader,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OutputFormat, PragmaSettings, ProgramKind, SkslcError, detect_shader_settings,
        output_format_for, program_kind_for_input, skslc,
    };

    #[test]
    fn program_kind_comes_from_the_input_extension() {
        assert_eq!(
            program_kind_for_input("a/b.sksl"),
            Some(ProgramKind::Fragment)
        );
        assert_eq!(
            program_kind_for_input("b.frag"),
            Some(ProgramKind::Fragment)
        );
        assert_eq!(
            program_kind_for_input("b.privrts"),
            Some(ProgramKind::PrivateRuntimeShader)
        );
        assert_eq!(
            program_kind_for_input("b.rts"),
            Some(ProgramKind::RuntimeShader)
        );
        assert_eq!(program_kind_for_input("b.txt"), None);
    }

    #[test]
    fn output_format_comes_from_the_output_extension() {
        assert_eq!(output_format_for("x.skrp"), Some(OutputFormat::Skrp));
        assert_eq!(
            output_format_for("x.asm.comp"),
            Some(OutputFormat::SpirvAssembly)
        );
        assert_eq!(output_format_for("x.minified.sksl"), None);
    }

    #[test]
    fn missing_pragma_keeps_the_defaults() {
        assert_eq!(
            detect_shader_settings(b"half4 main() {}"),
            Ok(PragmaSettings::default())
        );
    }

    #[test]
    fn pragma_words_are_consumed_in_any_order() {
        let text = "/*#pragma settings NoOptimize Default Sharpen*/\nhalf4 main() {}";
        let s = detect_shader_settings(text.as_bytes()).unwrap();
        assert_eq!(s.caps, Some("Default"));
        assert!(s.sharpen_textures);
        assert!(!s.optimize);
        assert_eq!(s.inline_threshold, Some(0));
        let text = "/*#pragma settings Version110 AllowNarrowingConversions*/";
        let s = detect_shader_settings(text.as_bytes()).unwrap();
        assert_eq!(s.caps, Some("Version110"));
        assert!(s.allow_narrowing_conversions);
    }

    #[test]
    fn unknown_pragma_word_is_an_error() {
        let text = "/*#pragma settings Sharpen Bogus*/";
        let err = detect_shader_settings(text.as_bytes()).unwrap_err();
        assert_eq!(err.0, " Sharpen Bogus");
    }

    #[test]
    fn skslc_validates_before_the_compiler() {
        assert!(matches!(
            skslc("x.txt", b"", "x.skrp", true),
            Err(SkslcError::Input(_))
        ));
        assert!(matches!(
            skslc("x.sksl", b"", "x.minified.sksl", true),
            Err(SkslcError::Configuration(_))
        ));
        assert!(matches!(
            skslc("x.sksl", b"/*#pragma settings Nope*/", "x.skrp", true),
            Err(SkslcError::Pragma(_))
        ));
        // With --nosettings the pragma is not read at all: the (empty) program compiles, and has
        // no `main`.
        assert_eq!(
            skslc("x.sksl", b"/*#pragma settings Nope*/", "x.skrp", false).unwrap(),
            b"### Compilation failed:\n\nerror: code has no entrypoint\n1 error\n"
        );
        // The other generators are not ported.
        assert!(matches!(
            skslc(
                "x.sksl",
                b"half4 main(float2 p) { return half4(1); }",
                "x.wgsl",
                true
            ),
            Err(SkslcError::NotPorted(_))
        ));
    }

    #[test]
    fn a_program_is_dumped_as_raster_pipeline_instructions() {
        let out = skslc(
            "x.sksl",
            b"half4 main(float2 p) { return half4(1); }",
            "x.skrp",
            true,
        )
        .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains(" instructions\n"), "{out}");
        assert!(out.contains("init_lane_masks"), "{out}");
    }

    #[test]
    fn a_program_the_generator_rejects_is_a_compile_error() {
        // `sinh` has no Raster Pipeline implementation.
        let out = skslc(
            "x.sksl",
            b"half4 main(float2 p) { return half4(half(sinh(p.x))); }",
            "x.skrp",
            true,
        )
        .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.starts_with("### Compilation failed:\n\nerror: code is not supported\n"),
            "{out}"
        );
    }

    #[test]
    fn a_compile_error_is_written_as_the_golden_text() {
        let text = b"half4 main() { return x; }";
        let out = skslc("x.sksl", text, "x.glsl", true).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("### Compilation failed:\n\nerror: 1: unknown identifier 'x'\n"));
        assert!(out.ends_with("1 error\n"));
    }

    #[test]
    fn a_runtime_shader_cannot_be_a_vertex_program() {
        let out = skslc("x.vert", b"void main() {}", "x.skrp", true).unwrap();
        assert_eq!(
            out,
            b"### Compilation failed:\n\nRuntime shaders do not support vertex programs\n"
        );
    }
}
