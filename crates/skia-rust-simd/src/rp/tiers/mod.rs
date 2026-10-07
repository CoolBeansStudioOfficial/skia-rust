// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The stage code stamped per tier, and the dispatch from a [`Selection`] to a tier's
//! interpreter (design §2.3, §2.5).
//!
//! Each instantiation is one file here that brings a lane module into scope as `lanes`, defines
//! `tier_fn!` (the tier's `#[target_feature]`) and `si!` (`tier_fn!` + `#[inline]`, Skia's
//! `SI`), and mounts the shared stage sources with `#[path]`:
//!
//! | Module | Lanes | Features |
//! |---|---|---|
//! | `scalar` | `lanes::scalar` (highp only) | none |
//! | `sse2`, `sse41`, `ml3`, `ml4` (x86-64) | `lanes::{sse2,sse41,ml3,ml4}` | the tier token's `FEATURES` |
//! | `neon` (aarch64) | `lanes::neon` | `neon` |
//! | `model_{sse2,sse41,ml3,ml4}_{host,amd_zen4}`, `model_neon_{host,arm}` (feature `models`) | `lanes::model_*::*` | none |
//!
//! # Adding a tier
//! Copy `sse41.rs` (or a model file) with the new lane module and feature string, declare it
//! below, and add one line to [`run`]'s `dispatch!` (`native` with the tier's token, or
//! `model`).

use super::memory::{MemView, MemoryCtxPatch};
use super::ops::Stage;
#[allow(unused_imports)] // unused on targets without native SIMD tiers or models (wasm32)
use crate::tier::Backend;
#[cfg(any(test, feature = "models"))]
use crate::tier::Estimates;
use crate::tier::{Selection, Tier};

/// An instruction of a compiled program: a stage, or the `just_return` that ends it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Instr<'a> {
    Stage(Stage<'a>),
    /// `just_return`: ends the chunk.
    Return,
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod ml3;
#[cfg(target_arch = "x86_64")]
pub(crate) mod ml4;
#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
pub(crate) mod scalar;
#[cfg(target_arch = "x86_64")]
pub(crate) mod sse2;
#[cfg(target_arch = "x86_64")]
pub(crate) mod sse41;

#[cfg(any(test, feature = "models"))]
pub(crate) mod model_ml3_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_ml3_host;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_ml4_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_ml4_host;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_neon_arm;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_neon_host;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse2_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse2_host;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse41_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse41_host;

/// Runs `prog` (highp or lowp) over `[x0, xlimit) × [y0, ylimit)` on the tier `sel` selects.
///
/// # Panics
/// If `sel` is a native tier this host cannot run (or another architecture's), a lowp program
/// on `Scalar`, or a model without the `models` feature (see [`Selection::check`]).
#[allow(clippy::too_many_arguments)] // start_pipeline's arguments
pub(crate) fn run(
    sel: Selection,
    lowp: bool,
    prog: &[Instr<'_>],
    x0: usize,
    y0: usize,
    xlimit: usize,
    ylimit: usize,
    views: &mut [Option<MemView<'_>>],
    patches: &mut [MemoryCtxPatch],
) {
    /// Calls a native tier's interpreter after taking the tier's token.
    #[allow(unused_macros)] // no native SIMD tier on wasm32
    macro_rules! native {
        ($tier:ident, $token:ident) => {{
            let _token = crate::cpu::$token::get()
                .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
            if lowp {
                // SAFETY: `lowp::run` enables exactly `$token::FEATURES` (the tier file's
                // `tier_fn!`), and `_token` exists only because those features were detected at
                // run time.
                unsafe { $tier::lowp::run(prog, x0, y0, xlimit, ylimit, views, patches) }
            } else {
                // SAFETY: as above, for `highp::run`.
                unsafe { $tier::highp::run(prog, x0, y0, xlimit, ylimit, views, patches) }
            }
        }};
    }
    /// Calls a model tier's interpreter (no target features).
    #[cfg(any(test, feature = "models"))]
    macro_rules! model {
        ($tier:ident) => {{
            if lowp {
                $tier::lowp::run(prog, x0, y0, xlimit, ylimit, views, patches);
            } else {
                $tier::highp::run(prog, x0, y0, xlimit, ylimit, views, patches);
            }
        }};
    }

    match (sel.tier, sel.backend) {
        (Tier::Scalar, _) => {
            assert!(!lowp, "Scalar has no lowp pipeline");
            scalar::highp::run(prog, x0, y0, xlimit, ylimit, views, patches);
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse2, Backend::Native) => native!(sse2, Sse2Token),
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse41, Backend::Native) => native!(sse41, Sse41Token),
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml3, Backend::Native) => native!(ml3, Ml3Token),
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml4, Backend::Native) => native!(ml4, Ml4Token),
        #[cfg(target_arch = "aarch64")]
        (Tier::Neon, Backend::Native) => native!(neon, NeonToken),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse2, Backend::Model(Estimates::Host)) => model!(model_sse2_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse2, Backend::Model(Estimates::AmdZen4)) => {
            model!(model_sse2_amd_zen4);
        }
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse41, Backend::Model(Estimates::Host)) => model!(model_sse41_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse41, Backend::Model(Estimates::AmdZen4)) => {
            model!(model_sse41_amd_zen4);
        }
        #[cfg(any(test, feature = "models"))]
        (Tier::Ml3, Backend::Model(Estimates::Host)) => model!(model_ml3_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Ml3, Backend::Model(Estimates::AmdZen4)) => model!(model_ml3_amd_zen4),
        #[cfg(any(test, feature = "models"))]
        (Tier::Ml4, Backend::Model(Estimates::Host)) => model!(model_ml4_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Ml4, Backend::Model(Estimates::AmdZen4)) => model!(model_ml4_amd_zen4),
        #[cfg(any(test, feature = "models"))]
        (Tier::Neon, Backend::Model(Estimates::Host)) => model!(model_neon_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Neon, Backend::Model(Estimates::Arm)) => model!(model_neon_arm),
        _ => panic!("{sel}: no raster pipeline implementation in this build"),
    }
}
