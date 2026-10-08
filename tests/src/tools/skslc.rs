// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/skslc/Main.cpp (the parts the in-scope `tests/sksl` goldens use).

//! `skslc` as a library: the command-line behaviour that decides what a golden is compiled as.
//!
//! The program kind comes from the input's extension, the output format from the output's, and
//! `/*#pragma settings …*/` in the input selects the shader caps and the program settings. The
//! compiler itself (the front end and the back ends) is not ported yet, so [`skslc`] validates
//! its arguments and then returns [`SkslcError::NotPorted`]. The golden harness reports those
//! goldens as ignored until the compiler lands (`docs/design/sksl.md` §9).

// Port of: tools/skslc/Main.cpp#L349-L490 (chrome/m156), for `detect_shader_settings`.

use std::fmt;

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
pub fn detect_shader_settings(text: &str) -> Result<PragmaSettings, UnrecognizedSetting> {
    const PRAGMA: &str = "/*#pragma settings ";
    let mut settings = PragmaSettings::default();
    let Some(start) = text.find(PRAGMA) else {
        return Ok(settings);
    };
    // Keep the space before the first item: each item is matched as a suffix that includes it.
    let body_start = start + PRAGMA.len() - 1;
    let Some(body_len) = text[body_start..].find("*/") else {
        return Ok(settings);
    };
    let mut body = text[body_start..body_start + body_len].to_owned();
    loop {
        let starting_length = body.len();
        for &(suffix, rule) in RULES {
            if let Some(rest) = body.strip_suffix(suffix) {
                let len = rest.len();
                body.truncate(len);
                apply(rule, &mut settings);
            }
        }
        if body.is_empty() {
            break;
        }
        if body.len() == starting_length {
            return Err(UnrecognizedSetting(body));
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
/// On success it returns the bytes `skslc` writes to `output`. Until the compiler is ported every
/// well-formed call returns [`SkslcError::NotPorted`].
///
/// # Errors
///
/// Returns the reason for every failure `skslc` reports, in the order it checks them.
// Skia compares extensions case-sensitively (`skstd::ends_with`), and so does skslc.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn skslc(
    input: &str,
    text: &str,
    output: &str,
    honor_settings: bool,
) -> Result<Vec<u8>, SkslcError> {
    let _kind = program_kind_for_input(input).ok_or_else(|| SkslcError::Input(input.to_owned()))?;
    let _format =
        output_format_for(output).ok_or_else(|| SkslcError::Configuration(output.to_owned()))?;
    let _settings = if honor_settings {
        detect_shader_settings(text).map_err(SkslcError::Pragma)?
    } else {
        PragmaSettings::default()
    };
    Err(SkslcError::NotPorted(
        "the SkSL compiler (front end: docs/design/sksl.md S5-S11)",
    ))
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
            detect_shader_settings("half4 main() {}"),
            Ok(PragmaSettings::default())
        );
    }

    #[test]
    fn pragma_words_are_consumed_in_any_order() {
        let text = "/*#pragma settings NoOptimize Default Sharpen*/\nhalf4 main() {}";
        let s = detect_shader_settings(text).unwrap();
        assert_eq!(s.caps, Some("Default"));
        assert!(s.sharpen_textures);
        assert!(!s.optimize);
        assert_eq!(s.inline_threshold, Some(0));
        let text = "/*#pragma settings Version110 AllowNarrowingConversions*/";
        let s = detect_shader_settings(text).unwrap();
        assert_eq!(s.caps, Some("Version110"));
        assert!(s.allow_narrowing_conversions);
    }

    #[test]
    fn unknown_pragma_word_is_an_error() {
        let text = "/*#pragma settings Sharpen Bogus*/";
        let err = detect_shader_settings(text).unwrap_err();
        assert_eq!(err.0, " Sharpen Bogus");
    }

    #[test]
    fn skslc_validates_before_the_compiler() {
        assert!(matches!(
            skslc("x.txt", "", "x.skrp", true),
            Err(SkslcError::Input(_))
        ));
        assert!(matches!(
            skslc("x.sksl", "", "x.minified.sksl", true),
            Err(SkslcError::Configuration(_))
        ));
        assert!(matches!(
            skslc("x.sksl", "/*#pragma settings Nope*/", "x.skrp", true),
            Err(SkslcError::Pragma(_))
        ));
        // With --nosettings the pragma is not read at all.
        assert!(matches!(
            skslc("x.sksl", "/*#pragma settings Nope*/", "x.skrp", false),
            Err(SkslcError::NotPorted(_))
        ));
    }
}
