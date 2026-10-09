// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLUtil.{h,cpp} (`ShaderCaps` and `ShaderCapsFactory`).
//
// Not ported: the GLSL-only parts of `ShaderCaps` (the GLSL generation level and the extension and
// version strings, and the methods that read them). Only the GLSL generator reads them, and that
// generator is not ported. `type_to_sksltype` belongs with the runtime-effect uniform types (S18).

//! [`ShaderCaps`]: the device capabilities the compiler consults, and [`ShaderCapsFactory`].

use std::sync::OnceLock;

/// `SkSL::ShaderCaps`: which features the shading language target supports, and which driver
/// workarounds apply. Every field defaults as Skia's member initializers do.
// Port of: src/sksl/SkSLUtil.h#L17-L125 (chrome/m156)
#[doc(alias = "SkSL::ShaderCaps")]
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors Skia's `ShaderCaps` fields one for one.
pub struct ShaderCaps {
    /// `fFloatIs32Bits`: queried by `SkSLSetting` and affects generated code.
    pub float_is_32_bits: bool,
    /// `fFlatInterpolationSupport`.
    pub flat_interpolation_support: bool,
    /// `fUsesPrecisionModifiers`.
    pub uses_precision_modifiers: bool,
    /// `fDualSourceBlendingSupport`.
    pub dual_source_blending_support: bool,
    /// `fShaderDerivativeSupport`.
    pub shader_derivative_support: bool,
    /// `fExplicitTextureLodSupport`: `sampleGrad` and `sampleLod` without implicit derivatives.
    pub explicit_texture_lod_support: bool,
    /// `fIntegerSupport`: 32-bit integers with unsigned types and bitwise operations.
    pub integer_support: bool,
    /// `fNonsquareMatrixSupport`.
    pub nonsquare_matrix_support: bool,
    /// `fInverseHyperbolicSupport`: `asinh`, `acosh` and `atanh`.
    pub inverse_hyperbolic_support: bool,
    /// `fFBFetchSupport`.
    pub fb_fetch_support: bool,
    /// `fFBFetchNeedsCustomOutput`.
    pub fb_fetch_needs_custom_output: bool,
    /// `fNoPerspectiveInterpolationSupport`.
    pub no_perspective_interpolation_support: bool,
    /// `fSampleMaskSupport`.
    pub sample_mask_support: bool,
    /// `fExternalTextureSupport`.
    pub external_texture_support: bool,
    /// `fInfinitySupport`: `isinf()` exists and infinities follow IEEE rules.
    pub infinity_support: bool,
    /// `fBuiltinFMASupport`: `SkSL` generates polyfills when false.
    pub builtin_fma_support: bool,
    /// `fBuiltinDeterminantSupport`.
    pub builtin_determinant_support: bool,
    /// `fCanUseVoidInSequenceExpressions`.
    pub can_use_void_in_sequence_expressions: bool,
    /// `fCanUseMinAndAbsTogether`.
    pub can_use_min_and_abs_together: bool,
    /// `fCanUseFractForNegativeValues`.
    pub can_use_fract_for_negative_values: bool,
    /// `fMustForceNegatedAtanParamToFloat`.
    pub must_force_negated_atan_param_to_float: bool,
    /// `fMustForceNegatedLdexpParamToMultiply`.
    pub must_force_negated_ldexp_param_to_multiply: bool,
    /// `fAtan2ImplementedAsAtanYOverX`: a device computes `atan(y, x)` as `atan(y / x)`.
    pub atan2_implemented_as_atan_y_over_x: bool,
    /// `fMustDoOpBetweenFloorAndAbs`.
    pub must_do_op_between_floor_and_abs: bool,
    /// `fMustGuardDivisionEvenAfterExplicitZeroCheck`.
    pub must_guard_division_even_after_explicit_zero_check: bool,
    /// `fCanUseFragCoord`: false means `sk_FragCoord` must not query `gl_FragCoord`.
    pub can_use_frag_coord: bool,
    /// `fAddAndTrueToLoopCondition`.
    pub add_and_true_to_loop_condition: bool,
    /// `fUnfoldShortCircuitAsTernary`: `&&` and `||` are rewritten as ternaries.
    pub unfold_short_circuit_as_ternary: bool,
    /// `fEmulateAbsIntFunction`.
    pub emulate_abs_int_function: bool,
    /// `fRewriteDoWhileLoops`.
    pub rewrite_do_while_loops: bool,
    /// `fRewriteSwitchStatements`.
    pub rewrite_switch_statements: bool,
    /// `fRemovePowWithConstantExponent`.
    pub remove_pow_with_constant_exponent: bool,
    /// `fNoDefaultPrecisionForExternalSamplers`.
    pub no_default_precision_for_external_samplers: bool,
    /// `fRewriteMatrixVectorMultiply`.
    pub rewrite_matrix_vector_multiply: bool,
    /// `fRewriteMatrixComparisons`.
    pub rewrite_matrix_comparisons: bool,
    /// `fRemoveConstFromFunctionParameters`.
    pub remove_const_from_function_parameters: bool,
    /// `fPerlinNoiseRoundingFix`.
    pub perlin_noise_rounding_fix: bool,
    /// `fMustDeclareFragmentFrontFacing`.
    pub must_declare_fragment_front_facing: bool,
    /// `fForceStd430ArrayLayout`.
    pub force_std430_array_layout: bool,
    /// `fCannotUseRelaxedPrecisionOnImageSample`.
    pub cannot_use_relaxed_precision_on_image_sample: bool,
    /// `fVectorClampMinMaxSupport`.
    pub vector_clamp_min_max_support: bool,
}

impl Default for ShaderCaps {
    /// `ShaderCaps()`: Skia's member initializers, with every feature off unless it is on by
    /// default.
    fn default() -> Self {
        Self {
            float_is_32_bits: true,
            flat_interpolation_support: false,
            uses_precision_modifiers: false,
            dual_source_blending_support: false,
            shader_derivative_support: false,
            explicit_texture_lod_support: false,
            integer_support: false,
            nonsquare_matrix_support: false,
            inverse_hyperbolic_support: false,
            fb_fetch_support: false,
            fb_fetch_needs_custom_output: false,
            no_perspective_interpolation_support: false,
            sample_mask_support: false,
            external_texture_support: false,
            infinity_support: false,
            builtin_fma_support: true,
            builtin_determinant_support: true,
            can_use_void_in_sequence_expressions: true,
            can_use_min_and_abs_together: true,
            can_use_fract_for_negative_values: true,
            must_force_negated_atan_param_to_float: false,
            must_force_negated_ldexp_param_to_multiply: false,
            atan2_implemented_as_atan_y_over_x: false,
            must_do_op_between_floor_and_abs: false,
            must_guard_division_even_after_explicit_zero_check: false,
            can_use_frag_coord: true,
            add_and_true_to_loop_condition: false,
            unfold_short_circuit_as_ternary: false,
            emulate_abs_int_function: false,
            rewrite_do_while_loops: false,
            rewrite_switch_statements: false,
            remove_pow_with_constant_exponent: false,
            no_default_precision_for_external_samplers: false,
            rewrite_matrix_vector_multiply: false,
            rewrite_matrix_comparisons: false,
            remove_const_from_function_parameters: false,
            perlin_noise_rounding_fix: false,
            must_declare_fragment_front_facing: false,
            force_std430_array_layout: false,
            cannot_use_relaxed_precision_on_image_sample: false,
            vector_clamp_min_max_support: true,
        }
    }
}

/// `SkSL::ShaderCapsFactory`: the caps the tests and the standalone compiler use.
#[doc(alias = "SkSL::ShaderCapsFactory")]
#[derive(Debug)]
pub struct ShaderCapsFactory;

impl ShaderCapsFactory {
    /// `MakeShaderCaps()`: the caps of the standalone compiler, which is what a build without
    /// Ganesh uses (Skia: `SKSL_STANDALONE || !SK_GANESH`).
    // Port of: src/sksl/SkSLUtil.cpp#L19-L30 (chrome/m156)
    #[must_use]
    pub fn make_shader_caps() -> ShaderCaps {
        ShaderCaps {
            shader_derivative_support: true,
            explicit_texture_lod_support: true,
            flat_interpolation_support: true,
            no_perspective_interpolation_support: true,
            sample_mask_support: true,
            external_texture_support: true,
            ..ShaderCaps::default()
        }
    }

    /// `ShaderCapsFactory::Default()`: the standalone caps with derivatives on.
    // Port of: src/sksl/SkSLUtil.h#L108-L117 (chrome/m156)
    #[must_use]
    pub fn default_caps() -> &'static ShaderCaps {
        static CAPS: OnceLock<ShaderCaps> = OnceLock::new();
        CAPS.get_or_init(|| {
            let mut caps = Self::make_shader_caps();
            caps.shader_derivative_support = true;
            caps
        })
    }

    /// `ShaderCapsFactory::Standalone()`: the standalone caps, shared.
    // Port of: src/sksl/SkSLUtil.h#L119-L122 (chrome/m156)
    #[must_use]
    pub fn standalone() -> &'static ShaderCaps {
        static CAPS: OnceLock<ShaderCaps> = OnceLock::new();
        CAPS.get_or_init(Self::make_shader_caps)
    }
}

#[cfg(test)]
mod tests {
    use super::{ShaderCaps, ShaderCapsFactory};

    #[test]
    fn standalone_caps_enable_the_standalone_features() {
        let caps = ShaderCapsFactory::standalone();
        assert!(caps.shader_derivative_support);
        assert!(caps.external_texture_support);
        assert!(!caps.integer_support);
        assert!(caps.can_use_frag_coord);
        assert_eq!(*caps, ShaderCapsFactory::make_shader_caps());
        assert_ne!(*caps, ShaderCaps::default());
    }
}
