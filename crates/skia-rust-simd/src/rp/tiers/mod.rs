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
//! | `sse2` (x86-64) | `lanes::sse2` | `sse2` |
//! | `sse41` (x86-64) | `lanes::sse41` | `sse2,ssse3,sse4.1` |
//! | `model_sse2_host`, `model_sse2_amd_zen4` (feature `models`) | `lanes::model_sse2::*` | none |
//! | `model_sse41_host`, `model_sse41_amd_zen4` (feature `models`) | `lanes::model_sse41::*` | none |
//!
//! # Adding a tier
//! Copy `sse41.rs` (or a model file) with the new lane module and feature string, declare it
//! below, and add one line to [`run`]'s `dispatch!` (`native` with the tier's token, or
//! `model`).

use super::memory::{MemView, MemoryCtxPatch};
use super::ops::Stage;
#[cfg(any(test, feature = "models"))]
use crate::tier::Estimates;
use crate::tier::{Backend, Selection, Tier};

/// An instruction of a compiled program: a stage, or the `just_return` that ends it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Instr<'a> {
    Stage(Stage<'a>),
    /// `just_return`: ends the chunk.
    Return,
}

pub(crate) mod scalar;
#[cfg(target_arch = "x86_64")]
pub(crate) mod sse2;
#[cfg(target_arch = "x86_64")]
pub(crate) mod sse41;

#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse2_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse2_host;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse41_amd_zen4;
#[cfg(any(test, feature = "models"))]
pub(crate) mod model_sse41_host;

/// The selection a program compiled for `sel` runs on: `sel` itself, except for tiers whose
/// stage code is not instantiated yet.
///
/// Until tasks A2b/A2c instantiate them, a *native* `Ml3`/`Ml4` selection runs the tier Skia
/// would run without `SkOpts::Init`'s upgrade (the compile-time baseline: `Sse41` if the build
/// enables SSE4.1, else `Sse2`), and native `Neon` runs as `Scalar`, so that every host can run
/// pipelines. Only `detect()` produces those selections; GM checks force their tiers
/// explicitly, and an explicit model of a missing tier still panics in [`run`].
#[must_use]
pub(crate) fn effective(sel: Selection) -> Selection {
    match (sel.tier, sel.backend) {
        // TODO(A2b): remove once the Ml3/Ml4 tiers are instantiated.
        (Tier::Ml3 | Tier::Ml4, Backend::Native) => {
            Selection::native(if cfg!(target_feature = "sse4.1") {
                Tier::Sse41
            } else {
                Tier::Sse2
            })
        }
        // TODO(A2c): remove once the Neon tier is instantiated.
        (Tier::Neon, Backend::Native) => Selection::native(Tier::Scalar),
        _ => sel,
    }
}

/// Runs `prog` (highp or lowp) over `[x0, xlimit) × [y0, ylimit)` on the tier `sel` selects.
///
/// # Panics
/// If `sel` names a tier whose stage code is not instantiated yet (Ml3/Ml4: task A2b; Neon:
/// A2c), a native tier this host cannot run, a lowp program on `Scalar`, or a model without
/// the `models` feature.
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
    #[allow(unused_macros)] // no native SIMD tier is instantiated on some targets yet
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
                $tier::lowp::run(prog, x0, y0, xlimit, ylimit, views, patches)
            } else {
                $tier::highp::run(prog, x0, y0, xlimit, ylimit, views, patches)
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
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse2, Backend::Model(Estimates::Host)) => model!(model_sse2_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse2, Backend::Model(Estimates::AmdZen4)) => model!(model_sse2_amd_zen4),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse41, Backend::Model(Estimates::Host)) => model!(model_sse41_host),
        #[cfg(any(test, feature = "models"))]
        (Tier::Sse41, Backend::Model(Estimates::AmdZen4)) => model!(model_sse41_amd_zen4),
        _ => panic!("{sel}: no raster pipeline implementation in this build (tasks A2b/A2c)"),
    }
}
