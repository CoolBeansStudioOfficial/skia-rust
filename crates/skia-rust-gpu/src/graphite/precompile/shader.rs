// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileShader.h,
// src/gpu/graphite/precompile/PrecompileShader.cpp, PrecompileShaderPriv.h and
// PrecompileShadersPriv.h

//! `PrecompileShader` and the `PrecompileShaders` factories. The image, YUV-image and
//! runtime-effect-backed shaders live in `shader_image` and `shader_effects`.

use std::sync::Arc;

use bitflags::bitflags;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::known_runtime_effects::StableKey;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shaders::shader_base::GradientType;
use skia_rust_core::size::ISize;
use skia_rust_effects::gradient::Interpolation;

use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use crate::graphite::key_context::KeyContext;
use crate::graphite::key_helpers::{
    CoordClampData, CoordClampShaderBlock, GradientData, GradientShaderBlocks, LMShaderData,
    LocalMatrixShaderBlock, PerlinNoiseData, PerlinNoiseShaderBlock, PerlinNoiseType,
    SolidColorShaderBlock,
};
use crate::graphite::key_helpers_ii::{
    BlendComposeBlock, ColorSpaceTransformBlock, ColorSpaceTransformData, RuntimeEffectBlock,
    RuntimeEffectShaderData, add_blend_mode, compose,
};
use crate::graphite::precompile::base::{
    Combinable, PrecompileBaseImpl, PrecompileBaseType, add_to_key, select_option, sum_combinations,
};
use crate::graphite::precompile::blender::{PrecompileBlender, PrecompileBlenderList};
use crate::graphite::precompile::color_filter::PrecompileColorFilter;
use crate::graphite::precompile::runtime_effect::PrecompileRuntimeEffects;

bitflags! {
    /// `PrecompileShaders::GradientShaderFlags`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct GradientShaderFlags: u16 {
        /// `kNone`.
        const NONE = 0;
        /// `kSmall`: the gradient with four stops.
        const SMALL = 1 << 1;
        /// `kMedium`: the gradient with eight stops.
        const MEDIUM = 1 << 2;
        /// `kLarge`: the gradient with more stops than fit inline.
        const LARGE = 1 << 3;
        /// `kAll`.
        const ALL = Self::SMALL.bits() | Self::MEDIUM.bits() | Self::LARGE.bits();
        /// `kNoLarge`.
        const NO_LARGE = Self::SMALL.bits() | Self::MEDIUM.bits();
    }
}

/// The virtual interface of `PrecompileShader`: `isConstant`, `isOpaque` and
/// `isALocalMatrixShader` on top of the base interface.
pub(crate) trait ShaderImpl: PrecompileBaseImpl {
    /// `isConstant(desiredCombination)`.
    fn is_constant(&self, _desired_combination: i32) -> bool {
        false
    }

    /// `isOpaque(desiredCombination)`.
    fn is_opaque(&self, desired_combination: i32) -> bool;

    /// `isALocalMatrixShader()`.
    fn is_a_local_matrix_shader(&self) -> bool {
        false
    }

    /// The local-matrix node this is, for `makeWithLocalMatrix`.
    fn as_local_matrix(&self) -> Option<&LocalMatrixShader> {
        None
    }
}

/// `PrecompileShader`: a shared precompile shader.
#[doc(alias = "SkShader")]
#[derive(Clone)]
pub struct PrecompileShader {
    pub(crate) imp: Arc<dyn ShaderImpl>,
}

impl std::fmt::Debug for PrecompileShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileShader")
            .field("type", &PrecompileBaseType::Shader)
            .finish_non_exhaustive()
    }
}

impl PrecompileShader {
    /// Wraps a shader implementation.
    pub(crate) fn from_imp(imp: Arc<dyn ShaderImpl>) -> Self {
        Self { imp }
    }

    /// `PrecompileBase::numCombinations()`.
    #[must_use]
    pub fn num_combinations(&self) -> i32 {
        self.imp.num_intrinsic_combinations() * self.imp.num_child_combinations()
    }

    /// `priv().numChildCombinations()`.
    #[must_use]
    pub fn num_child_combinations(&self) -> i32 {
        self.imp.num_child_combinations()
    }

    /// `priv().addToKey(keyContext, desiredCombination)`.
    pub fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.imp.add_to_key(key_context, desired_combination);
    }

    /// `priv().isOpaque(desiredCombination)`.
    #[must_use]
    pub fn is_opaque(&self, desired_combination: i32) -> bool {
        self.imp.is_opaque(desired_combination)
    }

    /// `priv().isConstant(desiredCombination)`.
    #[must_use]
    pub fn is_constant(&self, desired_combination: i32) -> bool {
        self.imp.is_constant(desired_combination)
    }

    /// `priv().isALocalMatrixShader()`.
    #[must_use]
    pub fn is_a_local_matrix_shader(&self) -> bool {
        self.imp.is_a_local_matrix_shader()
    }

    /// `PrecompileShader::makeWithLocalMatrix(isPerspective)`.
    // Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L1043-L1058 (chrome/m156)
    #[must_use]
    pub fn make_with_local_matrix(&self, is_perspective: bool) -> Self {
        if let Some(lm) = self.imp.as_local_matrix() {
            if is_perspective && !lm.flags.contains(LocalMatrixFlags::IS_PERSPECTIVE) {
                return LocalMatrixShader::shader(
                    lm.wrapped.clone(),
                    lm.flags | LocalMatrixFlags::IS_PERSPECTIVE,
                );
            }
            return self.clone();
        }
        PrecompileShaders::local_matrix(std::slice::from_ref(self), is_perspective)
    }

    /// `PrecompileShader::makeWithColorFilter(cf)`: a `None` filter returns this shader.
    #[must_use]
    pub fn make_with_color_filter(&self, cf: Option<PrecompileColorFilter>) -> Self {
        match cf {
            None => self.clone(),
            Some(cf) => PrecompileShaders::color_filter(std::slice::from_ref(self), &[cf]),
        }
    }

    /// `PrecompileShader::makeWithWorkingColorSpace(inputCS, outputCS)`.
    #[must_use]
    pub fn make_with_working_color_space(
        &self,
        input_cs: Option<ColorSpace>,
        output_cs: Option<ColorSpace>,
    ) -> Self {
        if input_cs.is_none() && output_cs.is_none() {
            return self.clone();
        }
        PrecompileShaders::working_color_space_explicit(
            std::slice::from_ref(self),
            &[(input_cs, output_cs)],
        )
    }
}

impl Combinable for PrecompileShader {
    fn combinations(&self) -> i32 {
        self.num_combinations()
    }

    fn add_combination_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.add_to_key(key_context, desired_combination);
    }
}

// Port of: the `PrecompileShader::Type` glue in PrecompileShaderPriv.h (chrome/m156)
pub(crate) fn shaders_as_options(shaders: &[PrecompileShader]) -> Vec<Option<PrecompileShader>> {
    shaders.iter().cloned().map(Some).collect()
}

/// `PrecompileEmptyShader`.
struct EmptyShader;

impl PrecompileBaseImpl for EmptyShader {
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert_eq!(desired_combination, 0); // The empty shader only ever has one combination
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::PriorOutput);
    }
}

impl ShaderImpl for EmptyShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        false
    }
}

/// `PrecompileColorShader`.
struct ColorShader;

impl PrecompileBaseImpl for ColorShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        2
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination == 0 || desired_combination == 1);
        SolidColorShaderBlock::add_block(
            key_context,
            &PMColor4f {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        );
    }
}

impl ShaderImpl for ColorShader {
    fn is_constant(&self, desired_combination: i32) -> bool {
        debug_assert!(desired_combination == 0 || desired_combination == 1);
        true
    }

    fn is_opaque(&self, desired_combination: i32) -> bool {
        debug_assert!(desired_combination == 0 || desired_combination == 1);
        desired_combination == 1
    }
}

/// `PrecompileBlendShader`.
struct BlendShader {
    blender_options: PrecompileBlenderList,
    dst_options: Vec<Option<PrecompileShader>>,
    src_options: Vec<Option<PrecompileShader>>,
    num_dst_combos: i32,
    num_src_combos: i32,
}

impl BlendShader {
    fn new(
        blenders: PrecompileBlenderList,
        dsts: &[PrecompileShader],
        srcs: &[PrecompileShader],
    ) -> Self {
        let dst_options = shaders_as_options(dsts);
        let src_options = shaders_as_options(srcs);
        Self {
            blender_options: blenders,
            num_dst_combos: sum_combinations(&dst_options),
            num_src_combos: sum_combinations(&src_options),
            dst_options,
            src_options,
        }
    }

    // Port of: PrecompileBlendShader::getChildCombinations (chrome/m156)
    fn child_combinations(&self, desired_combination: i32) -> (i32, i32, i32) {
        let desired_dst = desired_combination % self.num_dst_combos;
        let mut remaining = desired_combination / self.num_dst_combos;
        let desired_src = remaining % self.num_src_combos;
        remaining /= self.num_src_combos;
        let desired_blend = remaining;
        (desired_dst, desired_src, desired_blend)
    }
}

impl PrecompileBaseImpl for BlendShader {
    fn num_child_combinations(&self) -> i32 {
        self.blender_options.num_combinations() * self.num_dst_combos * self.num_src_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let (desired_dst, desired_src, desired_blend) =
            self.child_combinations(desired_combination);
        let (blender, blender_combination) = self.blender_options.select_option(desired_blend);
        let blender = blender.expect("a blender option");
        let blend_mode = blender.as_blend_mode();
        if blend_mode.is_some() {
            BlendComposeBlock::begin_block(key_context);
        } else {
            let blend_effect = PrecompileRuntimeEffects::known(StableKey::Blend);
            RuntimeEffectBlock::begin_block(
                key_context,
                &RuntimeEffectShaderData {
                    effect: blend_effect,
                    uniforms: None,
                },
            );
        }
        add_to_key(key_context, &self.src_options, desired_src);
        add_to_key(key_context, &self.dst_options, desired_dst);
        if let Some(bm) = blend_mode {
            debug_assert_eq!(blender_combination, 0);
            add_blend_mode(key_context, bm);
        } else {
            blender.add_to_key(key_context, blender_combination);
        }
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block(); // BlendComposeBlock or RuntimeEffectBlock
    }
}

impl ShaderImpl for BlendShader {
    // Port of: PrecompileBlendShader::isOpaque (chrome/m156)
    fn is_opaque(&self, desired_combination: i32) -> bool {
        let (desired_dst, desired_src, desired_blend) =
            self.child_combinations(desired_combination);
        let (dst, dst_combination) = select_option(&self.dst_options, desired_dst);
        let (src, src_combination) = select_option(&self.src_options, desired_src);
        let (blender, _blender_combination) = self.blender_options.select_option(desired_blend);
        let blender = blender.expect("a blender option");
        if let Some(bm) = blender.as_blend_mode() {
            let Some((src_coeff, dst_coeff)) = bm.as_coeff() else {
                return false;
            };
            let src_is_opaque = src.as_ref().is_some_and(|s| s.is_opaque(src_combination));
            let dst_is_opaque = dst.as_ref().is_some_and(|d| d.is_opaque(dst_combination));
            let coeff_is_opaque = |coeff: skia_rust_core::blend_mode::BlendModeCoeff| {
                use skia_rust_core::blend_mode::BlendModeCoeff as C;
                let src_alpha = coeff == C::SA || coeff == C::SC;
                let dst_alpha = coeff == C::DA || coeff == C::DC;
                coeff == C::One || (src_alpha && src_is_opaque) || (dst_alpha && dst_is_opaque)
            };
            (src_is_opaque && coeff_is_opaque(src_coeff))
                || (dst_is_opaque && coeff_is_opaque(dst_coeff))
        } else {
            let blend_effect = PrecompileRuntimeEffects::known(StableKey::Blend);
            skia_rust_core::runtime_effect_priv::always_opaque(&blend_effect)
        }
    }
}

/// `PrecompileCoordClampShader`.
struct CoordClampShader {
    shaders: Vec<Option<PrecompileShader>>,
    num_shader_combos: i32,
}

impl PrecompileBaseImpl for CoordClampShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_shader_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_shader_combos);
        let ignored = Rect::from_xywh(0.0, 0.0, 256.0, 256.0); // ignored bc we're precompiling
        CoordClampShaderBlock::begin_block(key_context, &CoordClampData::new(ignored));
        add_to_key(key_context, &self.shaders, desired_combination);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for CoordClampShader {
    fn is_opaque(&self, desired_combination: i32) -> bool {
        // Port of: PrecompileCoordClampShader::isOpaque (chrome/m156)
        let (child, child_option) = select_option(&self.shaders, desired_combination);
        child.is_some_and(|c| c.is_opaque(child_option))
    }
}

bitflags! {
    /// `PrecompileLocalMatrixShader::Flags`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct LocalMatrixFlags: u8 {
        /// `kNone`.
        const NONE = 0b00;
        /// `kIsPerspective`.
        const IS_PERSPECTIVE = 0b01;
        /// `kIncludeWithOutVariant`.
        const INCLUDE_WITHOUT_VARIANT = 0b10;
    }
}

/// `PrecompileLocalMatrixShader`.
pub(crate) struct LocalMatrixShader {
    pub(crate) wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
    pub(crate) flags: LocalMatrixFlags,
}

const NUM_LM_INTRINSIC_COMBINATIONS: i32 = 2;
const WITH_LOCAL_MATRIX: i32 = 1;

impl LocalMatrixShader {
    /// Makes the shader handle for the wrapped list and flags.
    pub(crate) fn shader(
        wrapped: Vec<Option<PrecompileShader>>,
        flags: LocalMatrixFlags,
    ) -> PrecompileShader {
        let num_wrapped_combos = sum_combinations(&wrapped);
        PrecompileShader::from_imp(Arc::new(Self {
            wrapped,
            num_wrapped_combos,
            flags,
        }))
    }

    // Port of: PrecompileLocalMatrixShader::getChildCombinations (chrome/m156)
    fn child_combinations(&self, desired_combination: i32) -> (i32, i32) {
        debug_assert!(desired_combination < self.num_combinations_local());
        let (desired_lm, desired_wrapped) = if self
            .flags
            .contains(LocalMatrixFlags::INCLUDE_WITHOUT_VARIANT)
        {
            (
                desired_combination % NUM_LM_INTRINSIC_COMBINATIONS,
                desired_combination / NUM_LM_INTRINSIC_COMBINATIONS,
            )
        } else {
            (WITH_LOCAL_MATRIX, desired_combination)
        };
        debug_assert!(desired_wrapped < self.num_wrapped_combos);
        (desired_lm, desired_wrapped)
    }

    fn num_combinations_local(&self) -> i32 {
        self.num_intrinsic_combinations_local() * self.num_wrapped_combos
    }

    fn num_intrinsic_combinations_local(&self) -> i32 {
        if self
            .flags
            .contains(LocalMatrixFlags::INCLUDE_WITHOUT_VARIANT)
        {
            NUM_LM_INTRINSIC_COMBINATIONS
        } else {
            1 // just kWithLocalMatrix
        }
    }
}

impl PrecompileBaseImpl for LocalMatrixShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        self.num_intrinsic_combinations_local()
    }

    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let (desired_lm, desired_wrapped) = self.child_combinations(desired_combination);
        if desired_lm == WITH_LOCAL_MATRIX {
            let mut matrix = Matrix::i().clone();
            if self.flags.contains(LocalMatrixFlags::IS_PERSPECTIVE) {
                matrix.set_persp_x(0.1);
            }
            let lm_shader_data = LMShaderData::new(matrix);
            LocalMatrixShaderBlock::begin_block(key_context, &lm_shader_data);
        }
        add_to_key(key_context, &self.wrapped, desired_wrapped);
        if desired_lm == WITH_LOCAL_MATRIX {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .end_block();
        }
    }
}

impl ShaderImpl for LocalMatrixShader {
    fn is_constant(&self, desired_combination: i32) -> bool {
        let (_, desired_wrapped) = self.child_combinations(desired_combination);
        let (wrapped, wrapped_combination) = select_option(&self.wrapped, desired_wrapped);
        wrapped.is_some_and(|w| w.is_constant(wrapped_combination))
    }

    fn is_opaque(&self, desired_combination: i32) -> bool {
        let (_, desired_wrapped) = self.child_combinations(desired_combination);
        let (wrapped, wrapped_combination) = select_option(&self.wrapped, desired_wrapped);
        wrapped.is_some_and(|w| w.is_opaque(wrapped_combination))
    }

    fn is_a_local_matrix_shader(&self) -> bool {
        true
    }

    fn as_local_matrix(&self) -> Option<&LocalMatrixShader> {
        Some(self)
    }
}

/// `PrecompileCTMShader`: the wrapped shaders under an identity local matrix.
struct CtmShader {
    wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
}

impl PrecompileBaseImpl for CtmShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_wrapped_combos);
        let ignored = LMShaderData::new(Matrix::i().clone());
        LocalMatrixShaderBlock::begin_block(key_context, &ignored);
        add_to_key(key_context, &self.wrapped, desired_combination);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for CtmShader {
    fn is_constant(&self, desired_combination: i32) -> bool {
        let (child, child_combination) = select_option(&self.wrapped, desired_combination);
        child.is_some_and(|c| c.is_constant(child_combination))
    }

    fn is_opaque(&self, desired_combination: i32) -> bool {
        let (child, child_combination) = select_option(&self.wrapped, desired_combination);
        child.is_some_and(|c| c.is_opaque(child_combination))
    }
}

/// `PrecompileShadersPriv::CTM`: the wrapped shaders under an identity local matrix.
// Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L1100-L1128 (chrome/m156)
pub(crate) fn ctm_node(wrapped: Vec<Option<PrecompileShader>>) -> PrecompileShader {
    let num_wrapped_combos = sum_combinations(&wrapped);
    PrecompileShader::from_imp(Arc::new(CtmShader {
        wrapped,
        num_wrapped_combos,
    }))
}

/// `PrecompileColorFilterShader`.
struct ColorFilterShader {
    shaders: Vec<Option<PrecompileShader>>,
    color_filters: Vec<Option<PrecompileColorFilter>>,
    num_shader_combos: i32,
    num_color_filter_combos: i32,
}

impl ColorFilterShader {
    // Port of: PrecompileColorFilterShader::getChildCombinations (chrome/m156)
    fn child_combinations(&self, desired_combination: i32) -> (i32, i32) {
        let desired_shader = desired_combination % self.num_shader_combos;
        let desired_color_filter = desired_combination / self.num_shader_combos;
        debug_assert!(desired_color_filter < self.num_color_filter_combos);
        (desired_shader, desired_color_filter)
    }
}

impl PrecompileBaseImpl for ColorFilterShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_shader_combos * self.num_color_filter_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let (desired_shader, desired_color_filter) = self.child_combinations(desired_combination);
        compose(
            key_context,
            || add_to_key(key_context, &self.shaders, desired_shader),
            || add_to_key(key_context, &self.color_filters, desired_color_filter),
        );
    }
}

impl ShaderImpl for ColorFilterShader {
    fn is_opaque(&self, desired_combination: i32) -> bool {
        let (desired_shader, desired_color_filter) = self.child_combinations(desired_combination);
        let (shader, shader_combination) = select_option(&self.shaders, desired_shader);
        let (filter, filter_combination) = select_option(&self.color_filters, desired_color_filter);
        // Port note: the C++ passes `desiredColorFilterCombination` (not the filter's own
        // child index) to `isAlphaUnchanged`; the port keeps that.
        let _ = filter_combination;
        shader.is_some_and(|s| s.is_opaque(shader_combination))
            && filter.is_none_or(|f| f.is_alpha_unchanged(desired_color_filter))
    }
}

/// `PrecompileWorkingColorSpaceShader`.
struct WorkingColorSpaceShader {
    shaders: Vec<Option<PrecompileShader>>,
    color_spaces: Vec<(Option<ColorSpace>, Option<ColorSpace>)>,
    num_shader_combos: i32,
}

impl WorkingColorSpaceShader {
    fn new(
        shaders: Vec<Option<PrecompileShader>>,
        color_spaces: Vec<(Option<ColorSpace>, Option<ColorSpace>)>,
    ) -> Self {
        let mut color_spaces = color_spaces;
        if color_spaces.is_empty() {
            color_spaces.push((None, None)); // encode identity
        }
        let num_shader_combos = sum_combinations(&shaders);
        Self {
            shaders,
            color_spaces,
            num_shader_combos,
        }
    }
}

impl PrecompileBaseImpl for WorkingColorSpaceShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_shader_combos * i32::try_from(self.color_spaces.len()).unwrap_or(i32::MAX)
    }

    // Port of: PrecompileWorkingColorSpaceShader::addToKey (chrome/m156)
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let desired_shader = desired_combination % self.num_shader_combos;
        let desired_cs = usize::try_from(desired_combination / self.num_shader_combos).unwrap_or(0);
        let (input, output) = &self.color_spaces[desired_cs];
        if input.is_none() && output.is_none() {
            add_to_key(key_context, &self.shaders, desired_shader);
            return;
        }
        let dst_info = key_context.dst_color_info();
        let dst_at = dst_info.alpha_type();
        let dst_cs = dst_info
            .color_space_ref()
            .cloned()
            .unwrap_or_else(ColorSpace::new_srgb);
        let input_cs = input.clone().unwrap_or_else(|| dst_cs.clone());
        let output_cs = output.clone().unwrap_or_else(|| input_cs.clone());
        let cs_context = key_context.with_extra_flags(
            crate::graphite::key_context::KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM,
        );
        let working_at = dst_at;
        let working_info = skia_rust_core::image_info::ColorInfo::new(
            dst_info.color_type(),
            working_at,
            Some(input_cs),
        );
        let working_context = cs_context.with_color_info(&working_info);
        compose(
            &cs_context,
            || add_to_key(&working_context, &self.shaders, desired_shader),
            || {
                let data = ColorSpaceTransformData::from_color_spaces(
                    Some(&output_cs),
                    working_at,
                    Some(&dst_cs),
                    dst_at,
                );
                ColorSpaceTransformBlock::add_block(&cs_context, &data);
            },
        );
    }
}

impl ShaderImpl for WorkingColorSpaceShader {
    fn is_opaque(&self, desired_combination: i32) -> bool {
        let desired_shader = desired_combination % self.num_shader_combos;
        let (child, child_combination) = select_option(&self.shaders, desired_shader);
        child.is_some_and(|c| c.is_opaque(child_combination))
    }
}

/// `PrecompilePerlinNoiseShader`.
struct PerlinNoiseShader;

impl PrecompileBaseImpl for PerlinNoiseShader {
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert_eq!(desired_combination, 0); // The Perlin noise shader only ever has one combination
        let ignored = PerlinNoiseData::new(
            PerlinNoiseType::FractalNoise,
            Point::new(0.0, 0.0),
            2,
            ISize::new(1, 1),
        );
        PerlinNoiseShaderBlock::add_block(key_context, &ignored);
    }
}

impl ShaderImpl for PerlinNoiseShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        false
    }
}

/// `get_gradient_intermediate_cs`: the intermediate color space a gradient interpolates in,
/// for a destination color space.
// Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L580-L592 (chrome/m156)
fn get_gradient_intermediate_cs(
    dst_color_space: Option<&ColorSpace>,
    interpolation: Interpolation,
) -> Option<ColorSpace> {
    use skia_rust_effects::gradient::{Colors, Gradient};
    use skia_rust_effects::gradient_base_shader::Color4fXformer;
    use skia_rust_effects::linear_gradient::LinearGradient;

    let pts = [
        skia_rust_core::point::Point::new(0.0, 0.0),
        skia_rust_core::point::Point::new(1.0, 0.0),
    ];
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 1.0), // SkColors::kBlack
        Color4f::new(1.0, 1.0, 1.0, 1.0), // SkColors::kWhite
    ];
    let pos = [0.0_f32, 1.0];
    let gradient = Gradient::new(
        Colors::new(
            &colors,
            Some(&pos),
            skia_rust_core::tile_mode::TileMode::Clamp,
            None,
        ),
        interpolation,
    );
    let shader = LinearGradient::new(&pts, &gradient);
    let xformed = Color4fXformer::new(shader.base(), dst_color_space);
    xformed.intermediate_color_space
}

/// `PrecompileGradientShader`.
struct GradientShader {
    gradient_type: GradientType,
    interpolation: Interpolation,
    stop_variants: Vec<usize>,
}

impl PrecompileBaseImpl for GradientShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        2 * i32::try_from(self.stop_variants.len()).unwrap_or(0)
    }

    // Port of: PrecompileGradientShader::addToKey (chrome/m156)
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let num_stop_variants = self.stop_variants.len();
        let grad_uses_storage = key_context.caps().storage_buffer_support();
        let desired_stop = usize::try_from(desired_combination).unwrap_or(0) % num_stop_variants;
        let grad_data = GradientData::new_precompile(
            self.gradient_type,
            self.stop_variants[desired_stop],
            grad_uses_storage,
        );
        let dst_cs = key_context.dst_color_info().color_space_ref().cloned();
        let intermediate_cs = get_gradient_intermediate_cs(dst_cs.as_ref(), self.interpolation);
        let dst_cs_or_srgb = dst_cs.unwrap_or_else(ColorSpace::new_srgb);
        let cs_data = ColorSpaceTransformData::from_color_spaces(
            intermediate_cs.as_ref(),
            AlphaType::Premul,
            Some(&dst_cs_or_srgb),
            AlphaType::Premul,
        );
        compose(
            key_context,
            || GradientShaderBlocks::add_block(key_context, &grad_data),
            || ColorSpaceTransformBlock::add_block(key_context, &cs_data),
        );
    }
}

impl ShaderImpl for GradientShader {
    fn is_opaque(&self, desired_combination: i32) -> bool {
        debug_assert!(desired_combination < self.num_intrinsic_combinations());
        usize::try_from(desired_combination).unwrap_or(0) < self.stop_variants.len()
    }
}

/// `PrecompileShaders`: the factories for precompile shaders.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileShaders;

impl PrecompileShaders {
    /// `PrecompileShaders::Empty()`.
    #[must_use]
    pub fn empty() -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(EmptyShader))
    }

    /// `PrecompileShaders::Color()`.
    #[must_use]
    pub fn color() -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(ColorShader))
    }

    /// `PrecompileShaders::Color(SkColorSpace)`: the color space does not change the key.
    #[must_use]
    pub fn color_in(_color_space: ColorSpace) -> PrecompileShader {
        Self::color()
    }

    /// `PrecompileShaders::Blend(blendModes, dsts, srcs)`.
    #[must_use]
    pub fn blend_modes(
        blend_modes: &[BlendMode],
        dsts: &[PrecompileShader],
        srcs: &[PrecompileShader],
    ) -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(BlendShader::new(
            PrecompileBlenderList::from_blend_modes(blend_modes),
            dsts,
            srcs,
        )))
    }

    /// `PrecompileShaders::Blend(blenders, dsts, srcs)`.
    #[must_use]
    pub fn blend(
        blenders: &[Option<PrecompileBlender>],
        dsts: &[PrecompileShader],
        srcs: &[PrecompileShader],
    ) -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(BlendShader::new(
            PrecompileBlenderList::from_blenders(blenders),
            dsts,
            srcs,
        )))
    }

    /// `PrecompileShaders::CoordClamp(shaders)`.
    #[must_use]
    pub fn coord_clamp(shaders: &[PrecompileShader]) -> PrecompileShader {
        let shaders = shaders_as_options(shaders);
        let num_shader_combos = sum_combinations(&shaders);
        PrecompileShader::from_imp(Arc::new(CoordClampShader {
            shaders,
            num_shader_combos,
        }))
    }

    /// `PrecompileShaders::MakeFractalNoise()`.
    #[must_use]
    #[doc(alias = "MakeFractalNoise")]
    pub fn make_fractal_noise() -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(PerlinNoiseShader))
    }

    /// `PrecompileShaders::MakeTurbulence()`.
    #[must_use]
    #[doc(alias = "MakeTurbulence")]
    pub fn make_turbulence() -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(PerlinNoiseShader))
    }

    /// `PrecompileShaders::LinearGradient(flags, interpolation)`.
    #[must_use]
    pub fn linear_gradient(
        flags: GradientShaderFlags,
        interpolation: Interpolation,
    ) -> PrecompileShader {
        Self::local_matrix(
            &[gradient_shader(GradientType::Linear, flags, interpolation)],
            false,
        )
    }

    /// `PrecompileShaders::RadialGradient(flags, interpolation)`.
    #[must_use]
    pub fn radial_gradient(
        flags: GradientShaderFlags,
        interpolation: Interpolation,
    ) -> PrecompileShader {
        Self::local_matrix(
            &[gradient_shader(GradientType::Radial, flags, interpolation)],
            false,
        )
    }

    /// `PrecompileShaders::TwoPointConicalGradient(flags, interpolation)`.
    #[must_use]
    pub fn two_point_conical_gradient(
        flags: GradientShaderFlags,
        interpolation: Interpolation,
    ) -> PrecompileShader {
        Self::local_matrix(
            &[gradient_shader(GradientType::Conical, flags, interpolation)],
            false,
        )
    }

    /// `PrecompileShaders::SweepGradient(flags, interpolation)`.
    #[must_use]
    pub fn sweep_gradient(
        flags: GradientShaderFlags,
        interpolation: Interpolation,
    ) -> PrecompileShader {
        Self::local_matrix(
            &[gradient_shader(GradientType::Sweep, flags, interpolation)],
            false,
        )
    }

    /// `PrecompileShaders::LocalMatrix(wrapped, isPerspective)`.
    #[must_use]
    pub fn local_matrix(wrapped: &[PrecompileShader], is_perspective: bool) -> PrecompileShader {
        LocalMatrixShader::shader(
            shaders_as_options(wrapped),
            if is_perspective {
                LocalMatrixFlags::IS_PERSPECTIVE
            } else {
                LocalMatrixFlags::NONE
            },
        )
    }

    /// `PrecompileShadersPriv::LocalMatrixBothVariants(wrapped)`.
    #[must_use]
    pub fn local_matrix_both_variants(wrapped: &[PrecompileShader]) -> PrecompileShader {
        LocalMatrixShader::shader(
            shaders_as_options(wrapped),
            LocalMatrixFlags::INCLUDE_WITHOUT_VARIANT,
        )
    }

    /// `PrecompileShaders::ColorFilter(shaders, colorFilters)`.
    #[must_use]
    pub fn color_filter(
        shaders: &[PrecompileShader],
        color_filters: &[PrecompileColorFilter],
    ) -> PrecompileShader {
        let shaders = shaders_as_options(shaders);
        let color_filters: Vec<Option<PrecompileColorFilter>> =
            color_filters.iter().cloned().map(Some).collect();
        let num_shader_combos = sum_combinations(&shaders);
        let num_color_filter_combos = sum_combinations(&color_filters);
        PrecompileShader::from_imp(Arc::new(ColorFilterShader {
            shaders,
            color_filters,
            num_shader_combos,
            num_color_filter_combos,
        }))
    }

    /// `PrecompileShaders::WorkingColorSpace(shaders, inputSpaces, outputSpaces)`.
    #[must_use]
    pub fn working_color_space(
        shaders: &[PrecompileShader],
        input_spaces: &[ColorSpace],
        output_spaces: &[ColorSpace],
    ) -> PrecompileShader {
        let input_spaces: Vec<Option<ColorSpace>> = if input_spaces.is_empty() {
            vec![None]
        } else {
            input_spaces.iter().cloned().map(Some).collect()
        };
        let output_spaces: Vec<Option<ColorSpace>> = if output_spaces.is_empty() {
            vec![None]
        } else {
            output_spaces.iter().cloned().map(Some).collect()
        };
        let mut pairs = Vec::with_capacity(input_spaces.len() * output_spaces.len());
        for i in &input_spaces {
            for o in &output_spaces {
                pairs.push((i.clone(), o.clone()));
            }
        }
        PrecompileShader::from_imp(Arc::new(WorkingColorSpaceShader::new(
            shaders_as_options(shaders),
            pairs,
        )))
    }

    /// `PrecompileShaders::WorkingColorSpaceExplicit(shaders, inputAndOutputSpaces)`.
    #[must_use]
    pub fn working_color_space_explicit(
        shaders: &[PrecompileShader],
        input_and_output_spaces: &[(Option<ColorSpace>, Option<ColorSpace>)],
    ) -> PrecompileShader {
        PrecompileShader::from_imp(Arc::new(WorkingColorSpaceShader::new(
            shaders_as_options(shaders),
            input_and_output_spaces.to_vec(),
        )))
    }
}

/// `gradient_shader`: the `PrecompileGradientShader` for one gradient type.
fn gradient_shader(
    gradient_type: GradientType,
    flags: GradientShaderFlags,
    interpolation: Interpolation,
) -> PrecompileShader {
    let mut stop_variants = Vec::new();
    if flags.contains(GradientShaderFlags::SMALL) {
        stop_variants.push(4);
    }
    if flags.contains(GradientShaderFlags::MEDIUM) {
        stop_variants.push(8);
    }
    if flags.contains(GradientShaderFlags::LARGE) {
        stop_variants.push(GradientData::NUM_INTERNAL_STORAGE_STOPS + 1);
    }
    PrecompileShader::from_imp(Arc::new(GradientShader {
        gradient_type,
        interpolation,
        stop_variants,
    }))
}
