// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLProgramKind.h, src/sksl/SkSLProgramSettings.h and
// include/sksl/SkSLVersion.h.

//! What kind of program is compiled, and with which settings.

use crate::defines::DEFAULT_INLINE_THRESHOLD;
use crate::modules::ModuleType;

/// `SkSL::Version`: the `SkSL` language version (`#version 100` or `#version 300`).
// Port of: include/sksl/SkSLVersion.h#L13-L25 (chrome/m156)
#[doc(alias = "SkSL::Version")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Version {
    /// Desktop GLSL 1.10, GLSL ES 1.00, WebGL 1.0.
    #[default]
    K100,
    /// Desktop GLSL 3.30, GLSL ES 3.00, WebGL 2.0.
    K300,
}

/// `SkSL::ProgramKind`.
// Port of: src/sksl/SkSLProgramKind.h#L16-L32 (chrome/m156)
#[doc(alias = "SkSL::ProgramKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramKind {
    Fragment,
    Vertex,
    Compute,
    GraphiteFragment,
    GraphiteVertex,
    /// Runtime effect only suitable as `SkColorFilter`.
    RuntimeColorFilter,
    /// Runtime effect only suitable as `SkShader`.
    RuntimeShader,
    /// Runtime effect only suitable as `SkBlender`.
    RuntimeBlender,
    /// Runtime color filter with the public restrictions lifted.
    PrivateRuntimeColorFilter,
    /// Runtime shader with the public restrictions lifted.
    PrivateRuntimeShader,
    /// Runtime blender with the public restrictions lifted.
    PrivateRuntimeBlender,
    /// Vertex portion of a custom mesh.
    MeshVertex,
    /// Fragment portion of a custom mesh.
    MeshFragment,
}

/// `SkSL::ProgramSettings`: options that control how a program is compiled.
// Port of: src/sksl/SkSLProgramSettings.h#L24-L84 (chrome/m156)
#[doc(alias = "SkSL::ProgramSettings")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors Skia's settings struct field for field.
pub struct ProgramSettings {
    /// If true, the destination fragment color can be read from `sk_FragColor` (GLSL
    /// framebuffer fetch).
    pub frag_color_is_in_out: bool,
    /// If true, all halfs are forced to be floats.
    pub force_high_precision: bool,
    /// If true, add -0.5 bias to LOD of all texture lookups.
    pub sharpen_textures: bool,
    /// If true, `sk_FragCoord`, the `dFdy` gradient and `sk_Clockwise` ignore the RT flip.
    pub force_no_rt_flip: bool,
    /// The offset of the RT-flip uniform in the uniform buffer, if one is needed.
    pub rt_flip_offset: i32,
    /// The binding of the RT-flip uniform buffer (SPIR-V).
    pub rt_flip_binding: i32,
    /// The set of the RT-flip uniform buffer (SPIR-V).
    pub rt_flip_set: i32,
    /// The set used for uniforms without `layout(set=…)`.
    pub default_uniform_set: i32,
    /// The binding used for uniforms without `layout(binding=…)`.
    pub default_uniform_binding: i32,
    /// Enables the optimizer.
    pub optimize: bool,
    /// (Requires `optimize`) Removes uncalled functions other than `main()`.
    pub remove_dead_functions: bool,
    /// (Requires `optimize`) Removes variables that are never used.
    pub remove_dead_variables: bool,
    /// (Requires `optimize`) When greater than zero, enables the inliner with this threshold.
    pub inline_threshold: i32,
    /// If true, every function gets the `noinline` modifier.
    pub force_no_inline: bool,
    /// If true, implicit conversions to lower precision numeric types are allowed.
    pub allow_narrowing_conversions: bool,
    /// If true, debug builds validate SPIR-V output (not used by skia-rust).
    pub validate_spirv: bool,
    /// If true, synthetic uniforms use push constant syntax.
    pub use_vulkan_push_constants_for_ganesh_rt_adjust: bool,
    /// The highest `SkSL` version a program may use.
    pub max_version_allowed: Version,
    /// Accepted and ignored: skia-rust has no `SkSL::Pool` (`docs/design/sksl.md` §4.2).
    pub use_memory_pool: bool,
}

impl Default for ProgramSettings {
    fn default() -> Self {
        Self {
            frag_color_is_in_out: false,
            force_high_precision: false,
            sharpen_textures: false,
            force_no_rt_flip: false,
            rt_flip_offset: -1,
            rt_flip_binding: -1,
            rt_flip_set: -1,
            default_uniform_set: 0,
            default_uniform_binding: 0,
            optimize: true,
            remove_dead_functions: true,
            remove_dead_variables: true,
            inline_threshold: DEFAULT_INLINE_THRESHOLD,
            force_no_inline: false,
            allow_narrowing_conversions: false,
            validate_spirv: true,
            use_vulkan_push_constants_for_ganesh_rt_adjust: false,
            max_version_allowed: Version::K100,
            use_memory_pool: true,
        }
    }
}

/// `SkSL::ProgramConfig`: the kind, settings and version of the code being compiled.
// Port of: src/sksl/SkSLProgramSettings.h#L89-L178 (chrome/m156)
#[doc(alias = "SkSL::ProgramConfig")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramConfig {
    /// The built-in module being compiled, or [`ModuleType::Program`] for a program.
    pub module_type: ModuleType,
    /// `fKind`.
    pub kind: ProgramKind,
    /// `fSettings`.
    pub settings: ProgramSettings,
    /// The version from the `#version` directive (`fRequiredSkSLVersion`).
    pub required_sksl_version: Version,
}

impl ProgramConfig {
    /// A configuration for `kind` with default settings and version 100.
    #[must_use]
    pub fn new(module_type: ModuleType, kind: ProgramKind, settings: ProgramSettings) -> Self {
        Self {
            module_type,
            kind,
            settings,
            required_sksl_version: Version::K100,
        }
    }

    /// `isBuiltinCode`: whether a built-in module is being compiled.
    #[must_use]
    pub fn is_builtin_code(&self) -> bool {
        self.module_type != ModuleType::Program
    }

    /// `enforcesSkSLVersion`: runtime effects are held to their `#version`.
    #[must_use]
    pub fn enforces_sksl_version(&self) -> bool {
        Self::is_runtime_effect(self.kind)
    }

    /// `strictES2Mode`.
    #[must_use]
    pub fn strict_es2_mode(&self) -> bool {
        self.settings.max_version_allowed == Version::K100
            && self.required_sksl_version == Version::K100
            && self.enforces_sksl_version()
    }

    /// `versionDescription`: the `#version` line that `Program::description` starts with.
    #[must_use]
    pub fn version_description(&self) -> &'static str {
        if self.enforces_sksl_version() {
            match self.required_sksl_version {
                Version::K100 => return "#version 100\n",
                Version::K300 => return "#version 300\n",
            }
        }
        ""
    }

    /// `IsFragment`.
    #[must_use]
    pub fn is_fragment(kind: ProgramKind) -> bool {
        matches!(kind, ProgramKind::Fragment | ProgramKind::GraphiteFragment)
    }

    /// `IsVertex`.
    #[must_use]
    pub fn is_vertex(kind: ProgramKind) -> bool {
        matches!(kind, ProgramKind::Vertex | ProgramKind::GraphiteVertex)
    }

    /// `IsCompute`.
    #[must_use]
    pub fn is_compute(kind: ProgramKind) -> bool {
        kind == ProgramKind::Compute
    }

    /// `IsRuntimeEffect`.
    #[must_use]
    pub fn is_runtime_effect(kind: ProgramKind) -> bool {
        matches!(
            kind,
            ProgramKind::RuntimeColorFilter
                | ProgramKind::RuntimeShader
                | ProgramKind::RuntimeBlender
                | ProgramKind::PrivateRuntimeColorFilter
                | ProgramKind::PrivateRuntimeShader
                | ProgramKind::PrivateRuntimeBlender
                | ProgramKind::MeshVertex
                | ProgramKind::MeshFragment
        )
    }

    /// `IsRuntimeShader`.
    #[must_use]
    pub fn is_runtime_shader(kind: ProgramKind) -> bool {
        matches!(
            kind,
            ProgramKind::RuntimeShader | ProgramKind::PrivateRuntimeShader
        )
    }

    /// `IsRuntimeColorFilter`.
    #[must_use]
    pub fn is_runtime_color_filter(kind: ProgramKind) -> bool {
        matches!(
            kind,
            ProgramKind::RuntimeColorFilter | ProgramKind::PrivateRuntimeColorFilter
        )
    }

    /// `IsRuntimeBlender`.
    #[must_use]
    pub fn is_runtime_blender(kind: ProgramKind) -> bool {
        matches!(
            kind,
            ProgramKind::RuntimeBlender | ProgramKind::PrivateRuntimeBlender
        )
    }

    /// `IsMesh`.
    #[must_use]
    pub fn is_mesh(kind: ProgramKind) -> bool {
        matches!(kind, ProgramKind::MeshVertex | ProgramKind::MeshFragment)
    }

    /// `AllowsPrivateIdentifiers`: whether `$`-names and `sk_Caps` may appear.
    #[must_use]
    pub fn allows_private_identifiers(kind: ProgramKind) -> bool {
        !matches!(
            kind,
            ProgramKind::RuntimeColorFilter
                | ProgramKind::RuntimeShader
                | ProgramKind::RuntimeBlender
                | ProgramKind::MeshVertex
                | ProgramKind::MeshFragment
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ProgramConfig, ProgramKind, ProgramSettings, Version};
    use crate::modules::ModuleType;

    #[test]
    fn version_description_only_for_runtime_effects() {
        let mut config = ProgramConfig::new(
            ModuleType::Program,
            ProgramKind::RuntimeShader,
            ProgramSettings::default(),
        );
        assert_eq!(config.version_description(), "#version 100\n");
        assert!(config.strict_es2_mode());
        config.required_sksl_version = Version::K300;
        assert_eq!(config.version_description(), "#version 300\n");
        assert!(!config.strict_es2_mode());
        config.kind = ProgramKind::Fragment;
        assert_eq!(config.version_description(), "");
        assert!(!config.is_builtin_code());
    }
}
