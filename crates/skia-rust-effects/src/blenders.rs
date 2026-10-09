// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkBlenders.h, src/effects/SkBlenders.cpp

//! `SkBlenders`: the blender factories that are not blend modes.
//!
//! skia-rust: `Arithmetic` is built from the `Arithmetic` known runtime effect (`SkSL`).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::data::Data;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::scalar::Scalar;

/// `SkBlenders::Arithmetic`: the blend `k1*src*dst + k2*src + k3*dst + k4`, clamped to `[0, 1]`
/// (premultiplied when `enforce_premul`). Near-trivial coefficients give a blend mode instead.
// Port of: src/effects/SkBlenders.cpp#L11-L54 (chrome/m156)
#[doc(alias = "SkBlenders::Arithmetic")]
#[must_use]
pub fn arithmetic(k1: f32, k2: f32, k3: f32, k4: f32, enforce_premul: bool) -> Option<Blender> {
    if !(k1.is_finite() && k2.is_finite() && k3.is_finite() && k4.is_finite()) {
        return None;
    }

    // Are we nearly a SkBlendMode?
    let table = [
        ([0.0_f32, 1.0, 0.0, 0.0], BlendMode::Src),
        ([0.0, 0.0, 1.0, 0.0], BlendMode::Dst),
        ([0.0, 0.0, 0.0, 0.0], BlendMode::Clear),
    ];
    for ([t1, t2, t3, t4], mode) in table {
        if f32::nearly_equal(k1, t1, None)
            && f32::nearly_equal(k2, t2, None)
            && f32::nearly_equal(k3, t3, None)
            && f32::nearly_equal(k4, t4, None)
        {
            return Some(Blender::mode(mode));
        }
    }

    // If we get here, we need the actual blender effect.
    let arithmetic_effect = get_known_runtime_effect(StableKey::Arithmetic)?;
    let array = [k1, k2, k3, k4, if enforce_premul { 0.0_f32 } else { 1.0 }];
    let mut bytes = Vec::with_capacity(size_of_val(&array));
    for value in array {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    arithmetic_effect.make_blender(Data::new_copy(&bytes), &[])
}
