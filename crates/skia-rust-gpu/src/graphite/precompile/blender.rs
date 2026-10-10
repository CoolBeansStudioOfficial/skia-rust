// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileBlender.h,
// src/gpu/graphite/precompile/PrecompileBlender.cpp and PrecompileBlenderPriv.h

//! `PrecompileBlender`, `PrecompileBlenders` and the blend-mode option list.

use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::known_runtime_effects::StableKey;

use crate::gpu::blend::get_porter_duff_blend_constants;
use crate::graphite::key_context::KeyContext;
use crate::graphite::key_helpers_ii::add_blend_mode;
use crate::graphite::precompile::base::{Combinable, PrecompileBaseImpl, PrecompileBaseType};
use crate::graphite::precompile::runtime_effect::PrecompileRuntimeEffects;

/// The virtual interface of `PrecompileBlender`: `asBlendMode` on top of the base interface.
pub(crate) trait BlenderImpl: PrecompileBaseImpl {
    /// `asBlendMode()`: the blend mode, when this blender is one.
    fn as_blend_mode(&self) -> Option<BlendMode> {
        None
    }
}

/// `PrecompileBlender`: a shared precompile blender.
#[doc(alias = "SkBlender")]
#[derive(Clone)]
pub struct PrecompileBlender {
    pub(crate) imp: Arc<dyn BlenderImpl>,
}

impl std::fmt::Debug for PrecompileBlender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileBlender")
            .field("type", &PrecompileBaseType::Blender)
            .finish_non_exhaustive()
    }
}

impl PrecompileBlender {
    /// `PrecompileBlenderPriv::asBlendMode()`.
    #[must_use]
    pub fn as_blend_mode(&self) -> Option<BlendMode> {
        self.imp.as_blend_mode()
    }

    /// `PrecompileBlenderPriv::numChildCombinations()`.
    #[must_use]
    pub fn num_child_combinations(&self) -> i32 {
        self.imp.num_child_combinations()
    }

    /// `PrecompileBlenderPriv::numCombinations()`.
    #[must_use]
    pub fn num_combinations(&self) -> i32 {
        self.imp.num_intrinsic_combinations() * self.imp.num_child_combinations()
    }

    /// `PrecompileBlenderPriv::addToKey()`.
    pub fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.imp.add_to_key(key_context, desired_combination);
    }
}

impl Combinable for PrecompileBlender {
    fn combinations(&self) -> i32 {
        self.num_combinations()
    }

    fn add_combination_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.add_to_key(key_context, desired_combination);
    }
}

/// `PrecompileBlendModeBlender`.
struct BlendModeBlender {
    blend_mode: BlendMode,
}

impl PrecompileBaseImpl for BlendModeBlender {
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert_eq!(desired_combination, 0); // The blend mode blender only ever has one combination
        add_blend_mode(key_context, self.blend_mode);
    }
}

impl BlenderImpl for BlendModeBlender {
    fn as_blend_mode(&self) -> Option<BlendMode> {
        Some(self.blend_mode)
    }
}

/// `PrecompileBlenders`: the factories for precompile blenders.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileBlenders;

impl PrecompileBlenders {
    /// `PrecompileBlenders::Mode`.
    #[must_use]
    #[doc(alias = "Mode")]
    pub fn mode(blend_mode: BlendMode) -> PrecompileBlender {
        PrecompileBlender {
            imp: Arc::new(BlendModeBlender { blend_mode }),
        }
    }

    /// `PrecompileBlenders::Arithmetic`.
    #[must_use]
    #[doc(alias = "Arithmetic")]
    pub fn arithmetic() -> Option<PrecompileBlender> {
        PrecompileRuntimeEffects::make_precompile_blender(
            PrecompileRuntimeEffects::known(StableKey::Arithmetic),
            &[],
        )
    }
}

/// `PrecompileBlenderList`: the blenders a blend-mode or blender option list expands to.
// Port of: src/gpu/graphite/precompile/PrecompileBlenderPriv.h (chrome/m156)
#[derive(Clone, Debug, Default)]
pub struct PrecompileBlenderList {
    fixed_blender_effects: Vec<PrecompileBlender>,
    has_porter_duff_blender: bool,
    has_hslc_blender: bool,
    num_combos: i32,
}

impl PrecompileBlenderList {
    /// `PrecompileBlenderList(SkSpan<const sk_sp<PrecompileBlender>>)`.
    // Port of: src/gpu/graphite/precompile/PrecompileBlender.cpp#L53-L88 (chrome/m156)
    #[must_use]
    pub fn from_blenders(blenders: &[Option<PrecompileBlender>]) -> Self {
        let mut list = Self::default();
        for b in blenders {
            match b {
                None => list.has_porter_duff_blender = true,
                Some(b) => {
                    if let Some(bm) = b.as_blend_mode() {
                        if !get_porter_duff_blend_constants(bm).is_empty() {
                            list.has_porter_duff_blender = true;
                        } else if is_hslc(bm) {
                            list.has_hslc_blender = true;
                        } else {
                            // No reduced shader snippet for this blend mode.
                            list.fixed_blender_effects.push(b.clone());
                            list.num_combos += 1;
                        }
                    } else {
                        list.fixed_blender_effects.push(b.clone());
                        list.num_combos += b.num_combinations();
                    }
                }
            }
        }
        if !list.has_porter_duff_blender
            && !list.has_hslc_blender
            && list.fixed_blender_effects.is_empty()
        {
            list.has_porter_duff_blender = true; // Fallback to kSrcOver
        }
        if list.has_porter_duff_blender {
            list.num_combos += 1;
        }
        if list.has_hslc_blender {
            list.num_combos += 1;
        }
        list
    }

    /// `PrecompileBlenderList(SkSpan<const SkBlendMode>)`.
    // Port of: src/gpu/graphite/precompile/PrecompileBlender.cpp#L90-L107 (chrome/m156)
    #[must_use]
    pub fn from_blend_modes(blend_modes: &[BlendMode]) -> Self {
        let mut list = Self::default();
        for &bm in blend_modes {
            if !get_porter_duff_blend_constants(bm).is_empty() {
                list.has_porter_duff_blender = true;
            } else if is_hslc(bm) {
                list.has_hslc_blender = true;
            } else {
                list.fixed_blender_effects
                    .push(PrecompileBlenders::mode(bm));
            }
        }
        if !list.has_porter_duff_blender
            && !list.has_hslc_blender
            && list.fixed_blender_effects.is_empty()
        {
            list.has_porter_duff_blender = true; // Fallback to kSrcOver
        }
        list.num_combos = i32::from(list.has_porter_duff_blender)
            + i32::from(list.has_hslc_blender)
            + i32::try_from(list.fixed_blender_effects.len()).unwrap_or(i32::MAX);
        list
    }

    /// `numCombinations()`.
    #[must_use]
    pub fn num_combinations(&self) -> i32 {
        self.num_combos
    }

    /// `selectOption(desiredCombination)`: the blender and its child combination.
    ///
    /// # Panics
    /// If `desired_combination` is out of range, or the list has no fixed blender to select
    /// when the Porter-Duff and HSLC options are used up (`SkUNREACHABLE`).
    // Port of: src/gpu/graphite/precompile/PrecompileBlender.cpp#L109-L131 (chrome/m156)
    #[must_use]
    pub fn select_option(&self, mut desired_combination: i32) -> (Option<PrecompileBlender>, i32) {
        debug_assert!(desired_combination >= 0 && desired_combination < self.num_combinations());
        if self.has_porter_duff_blender {
            if desired_combination == 0 {
                return (Some(PrecompileBlenders::mode(BlendMode::SrcOver)), 0);
            }
            desired_combination -= 1;
        }
        if self.has_hslc_blender {
            if desired_combination == 0 {
                return (Some(PrecompileBlenders::mode(BlendMode::Hue)), 0);
            }
            desired_combination -= 1;
        }
        // Port of: `PrecompileBase::SelectOption<PrecompileBlender>(fFixedBlenderEffects, ...)`.
        assert!(
            !self.fixed_blender_effects.is_empty(),
            "SkUNREACHABLE: no blender option for the combination"
        );
        for blender in &self.fixed_blender_effects {
            let count = blender.num_combinations();
            if desired_combination < count {
                return (Some(blender.clone()), desired_combination);
            }
            desired_combination -= count;
        }
        (None, 0)
    }
}

/// `bm >= SkBlendMode::kHue`: the separable-blend modes above `kXor` are the HSLC ones.
fn is_hslc(bm: BlendMode) -> bool {
    bm as u32 >= BlendMode::Hue as u32
}
