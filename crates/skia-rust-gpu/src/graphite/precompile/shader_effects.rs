// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/precompile/PrecompileShadersPriv.h and the runtime-effect
// backed shaders of src/gpu/graphite/precompile/PrecompileShader.cpp (CTM, blur, matrix
// convolution, morphology, displacement and lighting).

//! The `PrecompileShadersPriv` shaders: wrappers that run a Skia-known runtime effect around
//! their wrapped shaders' combinations.

use std::sync::Arc;

use skia_rust_core::known_runtime_effects::StableKey;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::runtime_effect_priv;

use crate::graphite::key_context::KeyContext;
use crate::graphite::key_helpers_ii::{RuntimeEffectBlock, RuntimeEffectShaderData};
use crate::graphite::precompile::base::{PrecompileBaseImpl, add_to_key, sum_combinations};
use crate::graphite::precompile::runtime_effect::PrecompileRuntimeEffects;
use crate::graphite::precompile::shader::{
    PrecompileShader, PrecompileShaders, ShaderImpl, ctm_node, shaders_as_options,
};
use crate::graphite::precompile::shader_image::{ALL_TILE_MODES, ImageShaderFlags};

/// The runtime-effect block of a shader-wrapping node: its effect and the wrapped child.
fn begin_effect_block(key_context: &KeyContext<'_>, effect: &RuntimeEffect) {
    RuntimeEffectBlock::begin_block(
        key_context,
        &RuntimeEffectShaderData {
            effect: effect.clone(),
            uniforms: None,
        },
    );
}

/// `PrecompileBlurShader`: `kIDs` lists the twelve blur effects, in order.
struct BlurShader {
    wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
}

const BLUR_IDS: [StableKey; 12] = [
    StableKey::OneDBlur4,
    StableKey::OneDBlur8,
    StableKey::OneDBlur12,
    StableKey::OneDBlur16,
    StableKey::OneDBlur20,
    StableKey::OneDBlur28,
    StableKey::TwoDBlur4,
    StableKey::TwoDBlur8,
    StableKey::TwoDBlur12,
    StableKey::TwoDBlur16,
    StableKey::TwoDBlur20,
    StableKey::TwoDBlur28,
];

impl PrecompileBaseImpl for BlurShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        i32::try_from(BLUR_IDS.len()).unwrap_or(i32::MAX)
    }

    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let num_intrinsic = self.num_intrinsic_combinations();
        let desired_blur = usize::try_from(desired_combination % num_intrinsic).unwrap_or(0);
        let desired_wrapped = desired_combination / num_intrinsic;
        debug_assert!(desired_wrapped < self.num_wrapped_combos);
        let effect = PrecompileRuntimeEffects::known(BLUR_IDS[desired_blur]);
        debug_assert_eq!(effect.children().len(), 1);
        begin_effect_block(key_context, &effect);
        add_to_key(
            &key_context.for_runtime_effect(&effect, 0),
            &self.wrapped,
            desired_wrapped,
        );
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for BlurShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        let effect = PrecompileRuntimeEffects::known(StableKey::OneDBlur4);
        runtime_effect_priv::always_opaque(&effect)
    }
}

/// `PrecompileShadersPriv::Blur(wrapped)`.
#[must_use]
pub(crate) fn blur(wrapped: PrecompileShader) -> PrecompileShader {
    let wrapped = vec![Some(wrapped)];
    let num_wrapped_combos = sum_combinations(&wrapped);
    PrecompileShader::from_imp(Arc::new(BlurShader {
        wrapped,
        num_wrapped_combos,
    }))
}

/// `PrecompileMatrixConvolutionShader`.
struct MatrixConvolutionShader {
    wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
    raw_image_shader: Vec<Option<PrecompileShader>>,
    num_raw_image_shader_combos: i32,
}

impl PrecompileBaseImpl for MatrixConvolutionShader {
    fn num_intrinsic_combinations(&self) -> i32 {
        1 + 2 * self.num_raw_image_shader_combos
    }

    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let mut desired_texture_combination = 0;
        let desired_wrapped = desired_combination % self.num_wrapped_combos;
        let mut remaining = desired_combination / self.num_wrapped_combos;
        let stable_key = if remaining == 0 {
            StableKey::MatrixConvUniforms
        } else {
            const TEXTURE_BASED: [StableKey; 2] =
                [StableKey::MatrixConvTexSm, StableKey::MatrixConvTexLg];
            remaining -= 1;
            let key = TEXTURE_BASED[usize::try_from(remaining % 2).unwrap_or(0)];
            desired_texture_combination = remaining / 2;
            debug_assert!(desired_texture_combination < self.num_raw_image_shader_combos);
            key
        };
        let effect = PrecompileRuntimeEffects::known(stable_key);
        begin_effect_block(key_context, &effect);
        add_to_key(
            &key_context.for_runtime_effect(&effect, 0),
            &self.wrapped,
            desired_wrapped,
        );
        if stable_key == StableKey::MatrixConvUniforms {
            debug_assert_eq!(effect.children().len(), 1);
        } else {
            debug_assert_eq!(effect.children().len(), 2);
            add_to_key(
                &key_context.for_runtime_effect(&effect, 1),
                &self.raw_image_shader,
                desired_texture_combination,
            );
        }
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for MatrixConvolutionShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        let effect = PrecompileRuntimeEffects::known(StableKey::MatrixConvUniforms);
        runtime_effect_priv::always_opaque(&effect)
    }
}

/// `PrecompileShadersPriv::MatrixConvolution(wrapped)`.
#[must_use]
pub(crate) fn matrix_convolution(wrapped: PrecompileShader) -> PrecompileShader {
    let wrapped = vec![Some(wrapped)];
    let num_wrapped_combos = sum_combinations(&wrapped);
    // The default `RawImage()` arguments: `kExcludeCubic` and `kAllTileModes`.
    let raw_image_shader = vec![Some(PrecompileShaders::raw_image(
        ImageShaderFlags::EXCLUDE_CUBIC,
        &[],
        &ALL_TILE_MODES,
    ))];
    let num_raw_image_shader_combos = sum_combinations(&raw_image_shader);
    PrecompileShader::from_imp(Arc::new(MatrixConvolutionShader {
        wrapped,
        num_wrapped_combos,
        raw_image_shader,
        num_raw_image_shader_combos,
    }))
}

/// `PrecompileMorphologyShader`.
struct MorphologyShader {
    wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
    stable_key: StableKey,
}

impl PrecompileBaseImpl for MorphologyShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_wrapped_combos);
        let effect = PrecompileRuntimeEffects::known(self.stable_key);
        debug_assert_eq!(effect.children().len(), 1);
        begin_effect_block(key_context, &effect);
        add_to_key(
            &key_context.for_runtime_effect(&effect, 0),
            &self.wrapped,
            desired_combination,
        );
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for MorphologyShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        let effect = PrecompileRuntimeEffects::known(self.stable_key);
        runtime_effect_priv::always_opaque(&effect)
    }
}

/// `PrecompileShadersPriv::LinearMorphology(wrapped)`.
#[must_use]
pub(crate) fn linear_morphology(wrapped: PrecompileShader) -> PrecompileShader {
    morphology(wrapped, StableKey::LinearMorphology)
}

/// `PrecompileShadersPriv::SparseMorphology(wrapped)`.
#[must_use]
pub(crate) fn sparse_morphology(wrapped: PrecompileShader) -> PrecompileShader {
    morphology(wrapped, StableKey::SparseMorphology)
}

fn morphology(wrapped: PrecompileShader, stable_key: StableKey) -> PrecompileShader {
    debug_assert!(
        stable_key == StableKey::LinearMorphology || stable_key == StableKey::SparseMorphology
    );
    let wrapped = vec![Some(wrapped)];
    let num_wrapped_combos = sum_combinations(&wrapped);
    PrecompileShader::from_imp(Arc::new(MorphologyShader {
        wrapped,
        num_wrapped_combos,
        stable_key,
    }))
}

/// `PrecompileDisplacementShader`.
struct DisplacementShader {
    displacement: Vec<Option<PrecompileShader>>,
    color: Vec<Option<PrecompileShader>>,
    num_displacement_combos: i32,
    num_color_combos: i32,
}

impl PrecompileBaseImpl for DisplacementShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_displacement_combos * self.num_color_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let desired_displacement = desired_combination % self.num_displacement_combos;
        let desired_color = desired_combination / self.num_displacement_combos;
        debug_assert!(desired_color < self.num_color_combos);
        let effect = PrecompileRuntimeEffects::known(StableKey::Displacement);
        debug_assert_eq!(effect.children().len(), 2);
        begin_effect_block(key_context, &effect);
        add_to_key(
            &key_context.for_runtime_effect(&effect, 0),
            &self.displacement,
            desired_displacement,
        );
        add_to_key(
            &key_context.for_runtime_effect(&effect, 1),
            &self.color,
            desired_color,
        );
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for DisplacementShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        let effect = PrecompileRuntimeEffects::known(StableKey::Displacement);
        runtime_effect_priv::always_opaque(&effect)
    }
}

/// `PrecompileShadersPriv::Displacement(displacement, color)`.
#[must_use]
pub(crate) fn displacement(
    displacement: PrecompileShader,
    color: PrecompileShader,
) -> PrecompileShader {
    let displacement = vec![Some(displacement)];
    let color = vec![Some(color)];
    PrecompileShader::from_imp(Arc::new(DisplacementShader {
        num_displacement_combos: sum_combinations(&displacement),
        num_color_combos: sum_combinations(&color),
        displacement,
        color,
    }))
}

/// `PrecompileLightingShader`.
struct LightingShader {
    wrapped: Vec<Option<PrecompileShader>>,
    num_wrapped_combos: i32,
}

impl PrecompileBaseImpl for LightingShader {
    fn num_child_combinations(&self) -> i32 {
        self.num_wrapped_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_wrapped_combos);
        let normal_effect = PrecompileRuntimeEffects::known(StableKey::Normal);
        let lighting_effect = PrecompileRuntimeEffects::known(StableKey::Lighting);
        debug_assert!(normal_effect.children().len() == 1 && lighting_effect.children().len() == 1);
        let lighting_context = key_context.for_runtime_effect(&lighting_effect, 0);
        let normal_context = lighting_context.for_runtime_effect(&normal_effect, 0);
        begin_effect_block(key_context, &lighting_effect);
        begin_effect_block(&lighting_context, &normal_effect);
        add_to_key(&normal_context, &self.wrapped, desired_combination);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

impl ShaderImpl for LightingShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        let effect = PrecompileRuntimeEffects::known(StableKey::Lighting);
        runtime_effect_priv::always_opaque(&effect)
    }
}

/// `PrecompileShadersPriv::Lighting(wrapped)`.
#[must_use]
pub(crate) fn lighting(wrapped: PrecompileShader) -> PrecompileShader {
    let wrapped = vec![Some(wrapped)];
    let num_wrapped_combos = sum_combinations(&wrapped);
    PrecompileShader::from_imp(Arc::new(LightingShader {
        wrapped,
        num_wrapped_combos,
    }))
}

/// `PrecompileShadersPriv::CTM(wrapped)`: the wrapped shaders under an identity local matrix.
// Port of: src/gpu/graphite/precompile/PrecompileShader.cpp#L1070-L1100 (chrome/m156)
#[must_use]
pub(crate) fn ctm(wrapped: &[PrecompileShader]) -> PrecompileShader {
    ctm_node(shaders_as_options(wrapped))
}
