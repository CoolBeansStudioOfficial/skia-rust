// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileBase.h,
// src/gpu/graphite/precompile/PrecompileBaseComplete.h and PrecompileBasePriv.h

//! The shared machinery of every precompile node: the combination count and the
//! `SelectOption` / `AddToKey` walks over option lists.

use crate::graphite::key_context::KeyContext;

/// `PrecompileBase::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(alias = "PrecompileBase::Type")]
pub enum PrecompileBaseType {
    /// `kBlender`.
    Blender,
    /// `kColorFilter`.
    ColorFilter,
    /// `kImageFilter`.
    ImageFilter,
    /// `kMaskFilter`.
    MaskFilter,
    /// `kShader`.
    Shader,
}

/// The virtual interface of `PrecompileBase` (`numIntrinsicCombinations`,
/// `numChildCombinations`, `addToKey`). Nodes are shared between threads, so implementors are
/// `Send + Sync`.
#[doc(alias = "PrecompileBase")]
pub trait PrecompileBaseImpl: Send + Sync {
    /// `numIntrinsicCombinations()`.
    fn num_intrinsic_combinations(&self) -> i32 {
        1
    }

    /// `numChildCombinations()`.
    fn num_child_combinations(&self) -> i32 {
        1
    }

    /// `addToKey(keyContext, desiredCombination)`.
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32);
}

/// `PrecompileBase::numCombinations()`: intrinsic times child combinations.
// Port of: include/gpu/graphite/precompile/PrecompileBase.h (chrome/m156)
#[doc(alias = "numCombinations")]
pub fn num_combinations<T: PrecompileBaseImpl + ?Sized>(node: &T) -> i32 {
    node.num_intrinsic_combinations() * node.num_child_combinations()
}

/// A handle to a precompile node that can sit in an option list. A `None` option is a null
/// `sk_sp`, which counts as one combination and adds nothing to a key.
pub trait Combinable: Clone {
    /// `priv().numCombinations()`.
    fn combinations(&self) -> i32;

    /// `priv().addToKey(keyContext, desiredCombination)`.
    fn add_combination_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32);
}

/// `PrecompileBase::SelectOption`: walks `options`, each a null option counting as one
/// combination, and returns the option that holds `desired_option` with its child index.
// Port of: src/gpu/graphite/precompile/PrecompileBaseComplete.h#L16-L28 (chrome/m156)
#[doc(alias = "SelectOption")]
#[must_use]
pub fn select_option<T: Combinable>(
    options: &[Option<T>],
    mut desired_option: i32,
) -> (Option<T>, i32) {
    for option in options {
        let count = option.as_ref().map_or(1, Combinable::combinations);
        if desired_option < count {
            return (option.clone(), desired_option);
        }
        desired_option -= count;
    }
    (None, 0)
}

/// `PrecompileBase::AddToKey`: selects the option for `desired_option` and adds it to the key.
// Port of: src/gpu/graphite/precompile/PrecompileBaseComplete.h#L30-L39 (chrome/m156)
#[doc(alias = "AddToKey")]
pub fn add_to_key<T: Combinable>(
    key_context: &KeyContext<'_>,
    options: &[Option<T>],
    desired_option: i32,
) {
    let (option, child_options) = select_option(options, desired_option);
    if let Some(option) = option {
        option.add_combination_to_key(key_context, child_options);
    }
}

/// The sum of `numCombinations()` over an option list, a null option counting as one.
// Port of: the `fNum*Combos` accumulation loops in src/gpu/graphite/precompile/PrecompileShader.cpp
// (chrome/m156)
#[must_use]
pub fn sum_combinations<T: Combinable>(options: &[Option<T>]) -> i32 {
    options
        .iter()
        .map(|option| option.as_ref().map_or(1, Combinable::combinations))
        .sum()
}
