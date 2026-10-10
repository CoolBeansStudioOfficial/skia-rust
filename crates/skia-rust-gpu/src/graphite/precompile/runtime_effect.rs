// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileRuntimeEffect.h and
// src/gpu/graphite/precompile/PrecompileRuntimeEffect.cpp

//! `PrecompileRuntimeEffects`: precompile shaders, color filters and blenders built from a
//! runtime effect and per-child option lists.

use std::sync::Arc;

use skia_rust_core::color::PMColor4f;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::runtime_effect::{ChildType, RuntimeEffect};
use skia_rust_core::runtime_effect_priv;

use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use crate::graphite::key_context::KeyContext;
use crate::graphite::key_helpers::SolidColorShaderBlock;
use crate::graphite::key_helpers_ii::{
    RuntimeEffectBlock, RuntimeEffectShaderData, add_fixed_blend_mode,
};
use crate::graphite::precompile::base::{
    Combinable, PrecompileBaseImpl, select_option, sum_combinations,
};
use crate::graphite::precompile::blender::{BlenderImpl, PrecompileBlender};
use crate::graphite::precompile::color_filter::{ColorFilterImpl, PrecompileColorFilter};
use crate::graphite::precompile::shader::{PrecompileShader, ShaderImpl};
use skia_rust_core::blend_mode::BlendMode;

impl std::fmt::Debug for PrecompileBaseHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PrecompileBaseHandle")
            .field(&self.child_type())
            .finish()
    }
}

/// A child option of a precompile runtime effect: a shader, color filter or blender.
/// (`PrecompileBase` used as a child, where the other node kinds are not valid.)
#[derive(Clone)]
pub enum PrecompileBaseHandle {
    /// A `PrecompileShader` child.
    Shader(PrecompileShader),
    /// A `PrecompileColorFilter` child.
    ColorFilter(PrecompileColorFilter),
    /// A `PrecompileBlender` child.
    Blender(PrecompileBlender),
}

impl PrecompileBaseHandle {
    /// The `SkRuntimeEffect::ChildType` this node can fill.
    fn child_type(&self) -> ChildType {
        match self {
            Self::Shader(_) => ChildType::Shader,
            Self::ColorFilter(_) => ChildType::ColorFilter,
            Self::Blender(_) => ChildType::Blender,
        }
    }
}

impl Combinable for PrecompileBaseHandle {
    fn combinations(&self) -> i32 {
        match self {
            Self::Shader(s) => s.num_combinations(),
            Self::ColorFilter(c) => c.num_combinations(),
            Self::Blender(b) => b.num_combinations(),
        }
    }

    fn add_combination_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        match self {
            Self::Shader(s) => s.add_to_key(key_context, desired_combination),
            Self::ColorFilter(c) => c.add_to_key(key_context, desired_combination),
            Self::Blender(b) => b.add_to_key(key_context, desired_combination),
        }
    }
}

/// `PrecompileRTEffectBase<Base>`: the state and `addToKey` shared by the three kinds.
// Port of: src/gpu/graphite/precompile/PrecompileRuntimeEffect.cpp#L80-L152 (chrome/m156)
struct RuntimeEffectNode {
    effect: RuntimeEffect,
    child_options: Vec<Vec<Option<PrecompileBaseHandle>>>,
    num_slot_combinations: Vec<i32>,
    num_child_combinations: i32,
}

impl RuntimeEffectNode {
    fn new(effect: RuntimeEffect, child_options: &[Vec<Option<PrecompileBaseHandle>>]) -> Self {
        let num_slot_combinations: Vec<i32> = child_options
            .iter()
            .map(|option_set| sum_combinations(option_set))
            .collect();
        let num_child_combinations = num_slot_combinations.iter().product();
        debug_assert_eq!(child_options.len(), effect.children().len());
        Self {
            effect,
            child_options: child_options.to_vec(),
            num_slot_combinations,
            num_child_combinations,
        }
    }
}

impl PrecompileBaseImpl for RuntimeEffectNode {
    fn num_child_combinations(&self) -> i32 {
        self.num_child_combinations
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_child_combinations);
        let children = self.effect.children();
        let shader_data = RuntimeEffectShaderData {
            effect: self.effect.clone(),
            uniforms: None,
        };
        if !RuntimeEffectBlock::begin_block(key_context, &shader_data) {
            RuntimeEffectBlock::add_no_op_effect(key_context, &self.effect);
            return;
        }
        let mut remaining_combinations = desired_combination;
        for (row_index, slot_options) in self.child_options.iter().enumerate() {
            let num_slot_combinations = self.num_slot_combinations[row_index];
            let slot_option = remaining_combinations % num_slot_combinations;
            remaining_combinations /= num_slot_combinations;
            let (option, child_options) = select_option(slot_options, slot_option);
            let child_context = key_context.for_runtime_effect(&self.effect, row_index);
            if let Some(option) = option {
                option.add_combination_to_key(&child_context, child_options);
            } else {
                {
                    debug_assert_eq!(child_options, 0);
                    match children[row_index].ty() {
                        ChildType::Shader => SolidColorShaderBlock::add_block(
                            &child_context,
                            &PMColor4f {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            },
                        ),
                        ChildType::ColorFilter => key_context
                            .paint_params_key_builder()
                            .borrow_mut()
                            .add_block(BuiltInCodeSnippetID::PriorOutput),
                        ChildType::Blender => {
                            add_fixed_blend_mode(&child_context, BlendMode::SrcOver);
                        }
                    }
                }
            }
        }
        RuntimeEffectBlock::handle_intrinsics(key_context, &self.effect);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

/// `PrecompileRTShader`.
struct RuntimeEffectShader(RuntimeEffectNode);

impl PrecompileBaseImpl for RuntimeEffectShader {
    fn num_child_combinations(&self) -> i32 {
        self.0.num_child_combinations()
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.0.add_to_key(key_context, desired_combination);
    }
}

impl ShaderImpl for RuntimeEffectShader {
    fn is_opaque(&self, _desired_combination: i32) -> bool {
        runtime_effect_priv::always_opaque(&self.0.effect)
    }
}

/// `PrecompileRTColorFilter`.
struct RuntimeEffectColorFilter(RuntimeEffectNode);

impl PrecompileBaseImpl for RuntimeEffectColorFilter {
    fn num_child_combinations(&self) -> i32 {
        self.0.num_child_combinations()
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.0.add_to_key(key_context, desired_combination);
    }
}

impl ColorFilterImpl for RuntimeEffectColorFilter {
    fn is_alpha_unchanged(&self, _desired_option: i32) -> bool {
        runtime_effect_priv::is_alpha_unchanged(&self.0.effect)
    }
}

/// `PrecompileRTBlender`.
struct RuntimeEffectBlender(RuntimeEffectNode);

impl PrecompileBaseImpl for RuntimeEffectBlender {
    fn num_child_combinations(&self) -> i32 {
        self.0.num_child_combinations()
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.0.add_to_key(key_context, desired_combination);
    }
}

impl BlenderImpl for RuntimeEffectBlender {}

/// `children_are_valid`: one option set per child, each option of the child's kind.
// Port of: src/gpu/graphite/precompile/PrecompileRuntimeEffect.cpp#L63-L77 (chrome/m156)
fn children_are_valid(
    effect: &RuntimeEffect,
    child_options: &[Vec<Option<PrecompileBaseHandle>>],
) -> bool {
    let child_info = effect.children();
    if child_options.len() != child_info.len() {
        return false;
    }
    child_info
        .iter()
        .zip(child_options)
        .all(|(info, option_set)| {
            option_set
                .iter()
                .flatten()
                .all(|option| option.child_type() == info.ty())
        })
}

/// `PrecompileRuntimeEffects`: the factories for precompile runtime-effect nodes.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileRuntimeEffects;

impl PrecompileRuntimeEffects {
    /// The Skia-known runtime effect for `key`.
    ///
    /// # Panics
    /// If `key` is not a known runtime effect of this build.
    #[must_use]
    pub fn known(key: StableKey) -> RuntimeEffect {
        get_known_runtime_effect(key)
            .cloned()
            .expect("GetKnownRuntimeEffect returns a built-in effect")
    }

    /// `PrecompileRuntimeEffects::MakePrecompileShader`.
    #[must_use]
    pub fn make_precompile_shader(
        effect: RuntimeEffect,
        child_options: &[Vec<Option<PrecompileBaseHandle>>],
    ) -> Option<PrecompileShader> {
        if !effect.allow_shader() || !children_are_valid(&effect, child_options) {
            return None;
        }
        Some(PrecompileShader::from_imp(Arc::new(RuntimeEffectShader(
            RuntimeEffectNode::new(effect, child_options),
        ))))
    }

    /// `PrecompileRuntimeEffects::MakePrecompileColorFilter`.
    #[must_use]
    pub fn make_precompile_color_filter(
        effect: RuntimeEffect,
        child_options: &[Vec<Option<PrecompileBaseHandle>>],
    ) -> Option<PrecompileColorFilter> {
        if !effect.allow_color_filter() || !children_are_valid(&effect, child_options) {
            return None;
        }
        Some(PrecompileColorFilter {
            imp: Arc::new(RuntimeEffectColorFilter(RuntimeEffectNode::new(
                effect,
                child_options,
            ))),
        })
    }

    /// `PrecompileRuntimeEffects::MakePrecompileBlender`.
    #[must_use]
    pub fn make_precompile_blender(
        effect: RuntimeEffect,
        child_options: &[Vec<Option<PrecompileBaseHandle>>],
    ) -> Option<PrecompileBlender> {
        if !effect.allow_blender() || !children_are_valid(&effect, child_options) {
            return None;
        }
        Some(PrecompileBlender {
            imp: Arc::new(RuntimeEffectBlender(RuntimeEffectNode::new(
                effect,
                child_options,
            ))),
        })
    }
}
